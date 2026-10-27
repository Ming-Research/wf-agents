//! The tool cells: the tools agents ask for (cell 13), the full CI on the draft (7), the merge queue (9) and watching
//! main (10). Each attempt of a tool is a row of tool_runs under the cell's round. A tool that fails is tried twice
//! more; a refusal is a result, not a failure, and is not tried again.

use std::cell::Cell;
use std::path::Path;
use std::time::Duration;

use db::Actor;
use db::rusqlite::params;
use flow::CompiledGraph;
use serde_json::{Value, json};

use crate::hub::{self, CheckRun, Failure};
use crate::inputs::Gathered;
use crate::work::{self, Item, RoundId, Work};
use crate::{Engine, git};

/// How many times a tool is tried before it is given up.
const ATTEMPTS: i64 = 3;

/// The longest output given inline; a longer one is saved and its path given.
const LONG: usize = 20_000;

fn sql(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// Runs one tool up to [`ATTEMPTS`] times, each attempt a row of tool_runs; gives what the last attempt gave, or the
/// failure with what each attempt gave.
fn attempts(
    e: &Engine,
    round: RoundId,
    tool: &str,
    args: &Value,
    mut f: impl FnMut() -> Result<Value, Failure>,
) -> Result<Value, (Failure, String)> {
    let mut tried = Vec::new();
    for n in 1..=ATTEMPTS {
        let now = db::now();
        e.db.conn()
            .execute(
                "INSERT INTO tool_runs (work, round, tool, args, attempt, queued_at, started_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
                params![round.work, round.n, tool, args.to_string(), n, now],
            )
            .map_err(|x| (Failure::other(x.to_string()), String::new()))?;
        let id = e.db.conn().last_insert_rowid();
        let got = f();
        let result = match &got {
            Ok(v) => v.clone(),
            Err(x) => json!({ "failed": { "kind": x.kind, "what": x.what } }),
        };
        let files: Vec<String> = result
            .pointer("/files")
            .and_then(Value::as_array)
            .map(|fs| fs.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
            .unwrap_or_default();
        let _ = e.db.conn().execute(
            "UPDATE tool_runs SET result = ?2, files = ?3, ended_at = ?4 WHERE id = ?1",
            params![id, result.to_string(), json!(files).to_string(), db::now()],
        );
        match got {
            Ok(v) => return Ok(v),
            Err(x) => {
                tried.push(format!("attempt {n}: {}", x.what));
                if n == ATTEMPTS {
                    return Err((x, format!("Tried {ATTEMPTS} times. {}", tried.join("; "))));
                }
                std::thread::sleep(Duration::from_secs(5 * n as u64));
            }
        }
    }
    unreachable!("the last attempt returns")
}

/// A text given inline when it is short, or saved under the data directory and given by its path.
fn inline_or_saved(e: &Engine, round: RoundId, n: usize, text: String) -> String {
    if text.len() <= LONG {
        return text;
    }
    let dir = e.data.join("tool-output");
    let path = dir.join(format!("{round}-{n}.txt"));
    match std::fs::create_dir_all(dir).and_then(|_| std::fs::write(&path, &text)) {
        Ok(()) => format!(
            "Too long to give here ({} bytes); saved to {}",
            text.len(),
            path.display()
        ),
        Err(x) => {
            let mut end = LONG;
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            format!("{}\n[cut at {end} bytes; saving the rest failed: {x}]", &text[..end])
        }
    }
}

/// The rows a query found as the flow's query tool gives them: the rows themselves, each an object, when they are short
/// enough to give here; otherwise none, and what `save` gives for them written one JSON object per line, which says
/// where they were saved.
fn rows_given(rows: Vec<Value>, save: impl FnOnce(String) -> String) -> (Vec<Value>, Option<String>) {
    let text = rows.iter().map(Value::to_string).collect::<Vec<_>>().join("\n");
    if text.len() <= LONG {
        (rows, None)
    } else {
        (Vec::new(), Some(save(text)))
    }
}

/// Splits a command line into words as a shell would, without running anything: quotes group, a backslash escapes.
pub fn words(line: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut has = false;
    let mut quote: Option<char> = None;
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some('"'), '\\') | (None, '\\') => {
                if let Some(n) = chars.next() {
                    cur.push(n);
                }
            }
            (Some(_), c) => cur.push(c),
            (None, '\'' | '"') => {
                quote = Some(c);
                has = true;
            }
            (None, c) if c.is_whitespace() => {
                if has || !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                    has = false;
                }
            }
            (None, c) => cur.push(c),
        }
    }
    if quote.is_some() {
        return Err("a quote is not closed".into());
    }
    if has || !cur.is_empty() {
        out.push(cur);
    }
    Ok(out)
}

/// Why a gh command may not run, or none when it may: it must start with an entry of the allow list, and `api` may
/// only read.
pub fn gh_refusal(allow: &[String], args: &[String]) -> Option<String> {
    let joined = args.join(" ");
    let allowed = allow.iter().any(|a| {
        if a.ends_with('/') {
            joined.starts_with(a.as_str())
        } else {
            joined == *a || joined.starts_with(&format!("{a} "))
        }
    });
    if !allowed {
        return Some(format!(
            "Not on the allow list in the engine's configuration: {}",
            allow.join(", ")
        ));
    }
    if args.first().is_some_and(|a| a == "api") {
        let writes = ["-X", "--method", "-f", "-F", "--field", "--raw-field", "--input"];
        if let Some(w) = args
            .iter()
            .find(|a| writes.iter().any(|x| a == x || a.starts_with(&format!("{x}="))))
        {
            return Some(format!("gh api may only read: {w} makes it write"));
        }
    }
    None
}

/// The repository's name on GitHub, owner/name.
fn repo(project: &config::Project) -> String {
    project.repo.clone()
}

/// The workflows of the full CI that belong to the project itself; those of downstream projects are named
/// owner/name:workflow.
fn own_full(project: &config::Project) -> (Vec<String>, Vec<String>) {
    project.ci.full.iter().cloned().partition(|w| !w.contains(':'))
}

/// The checks that count against a merge: those merging needs (all of them when none is named), less the known red.
fn counted(checks: &[CheckRun], project: &config::Project, known_red: &[String]) -> Vec<CheckRun> {
    checks
        .iter()
        .filter(|c| project.ci.merge_checks.is_empty() || project.ci.merge_checks.contains(&c.name))
        .filter(|c| !known_red.contains(&c.name))
        .cloned()
        .collect()
}

fn reds(checks: &[CheckRun]) -> Vec<String> {
    checks
        .iter()
        .filter(|c| c.result != "green")
        .map(|c| c.name.clone())
        .collect()
}

fn known_red(e: &Engine, project: &str) -> Result<Vec<String>, String> {
    let mut st =
        e.db.conn()
            .prepare("SELECT check_name FROM known_red_checks WHERE project = ?1")
            .map_err(sql)?;
    st.query_map([project], |r| r.get(0))
        .map_err(sql)?
        .collect::<Result<_, _>>()
        .map_err(sql)
}

/// Every check of the project's own full CI on a commit: the latest run of each.
fn full_checks(e: &Engine, project: &config::Project, sha: &str) -> Result<Vec<CheckRun>, Failure> {
    let (own, downstream) = own_full(project);
    if !downstream.is_empty() {
        return Err(Failure::other(format!(
            "this version of the engine runs no downstream project's workflow: {}",
            downstream.join(", ")
        )));
    }
    let mut out = Vec::new();
    for wf in own {
        out.extend(e.github.checks(&repo(project), sha, &wf)?);
    }
    Ok(out)
}

/// Runs the red checks again on the same commit, and gives the latest run of every check.
fn rerun_full(e: &Engine, project: &config::Project, sha: &str, first: &[CheckRun]) -> Result<Vec<CheckRun>, Failure> {
    let (own, _) = own_full(project);
    let mut out = Vec::new();
    for wf in own {
        out.extend(e.github.rerun(&repo(project), sha, &wf)?);
    }
    // A check the rerun did not give again keeps its first result.
    for c in first {
        if !out.iter().any(|x| x.name == c.name) {
            out.push(c.clone());
        }
    }
    Ok(out)
}

fn not_run(field: &str, (f, tried): (Failure, String)) -> Value {
    json!({ field: { "kind": "not_run", "failure": f.kind, "what": f.what, "tried": tried } })
}

fn checks_json(cs: &[CheckRun]) -> Value {
    serde_json::to_value(cs).unwrap_or(Value::Null)
}

/// Records the head pushed and the PR as soon as they are on GitHub, so a crash cannot lose them.
fn record_push(e: &mut Engine, w: &Work, head: Option<&str>, pr: Option<i64>) -> Result<(), String> {
    e.db.write(Actor::Engine, |tx| {
        if let Some(h) = head {
            tx.execute("UPDATE works SET pushed_head = ?2 WHERE id = ?1", params![w.id, h])?;
        }
        if let Some(n) = pr {
            tx.execute("UPDATE works SET pr = ?2 WHERE id = ?1", params![w.id, n])?;
        }
        Ok(())
    })
    .map_err(sql)
}

/// A test binary in a cargo test log: what it is, the tests it ran and how long it took as a whole.
struct Binary {
    name: String,
    tests: Vec<String>,
    finished: Option<String>,
}

/// The test binaries a cargo test log shows, in the order they ran. Each line is read as a reader sees it: without the
/// job, the step and the time gh puts before it, and without colour escapes.
fn binaries(log: &str) -> Vec<Binary> {
    let mut out: Vec<Binary> = Vec::new();
    for line in hub::clean_log(log).lines() {
        let l = line.trim();
        if let Some(rest) = l.strip_prefix("Running ") {
            let (source, deps) = rest.split_once(" (").unwrap_or((rest, ""));
            let file = deps.trim_end_matches(')').rsplit('/').next().unwrap_or_default();
            let krate = match file.rsplit_once('-') {
                Some((k, hash)) if hash.chars().all(|c| c.is_ascii_hexdigit()) => k,
                _ => file,
            };
            let name = if krate.is_empty() {
                source.to_string()
            } else {
                format!("{krate} ({source})")
            };
            out.push(Binary {
                name,
                tests: Vec::new(),
                finished: None,
            });
        } else if let Some(krate) = l.strip_prefix("Doc-tests ") {
            out.push(Binary {
                name: format!("the doc tests of {krate}"),
                tests: Vec::new(),
                finished: None,
            });
        } else if let Some(rest) = l.strip_prefix("test result: ") {
            if let (Some(b), Some((_, time))) = (out.last_mut(), rest.split_once("finished in ")) {
                b.finished = Some(time.trim().to_string());
            }
        } else if let Some(rest) = l.strip_prefix("test ")
            && let Some((name, _)) = rest.split_once(" ... ")
            && let Some(b) = out.last_mut()
        {
            b.tests.push(name.trim().to_string());
        }
    }
    out
}

/// How long each named test took, as far as the logs of the fast CI's jobs tell: stable `cargo test` times no test on
/// its own, so each named test is given the time of the whole test binary that holds it, said as that. A test not
/// found says how many tests the logs showed, and a log that could not be read says why.
fn test_times(tests: &[String], logs: &[Result<String, String>]) -> Vec<Value> {
    let found: Vec<Binary> = logs
        .iter()
        .filter_map(|l| l.as_ref().ok())
        .flat_map(|l| binaries(l))
        .collect();
    let unread: Vec<&str> = logs
        .iter()
        .filter_map(|l| l.as_ref().err().map(String::as_str))
        .collect();
    let ran: usize = found.iter().map(|b| b.tests.len()).sum();
    let missing = if found.is_empty() && !unread.is_empty() {
        format!("the logs of the fast CI could not be read: {}", unread.join("; "))
    } else {
        format!(
            "not found among the {ran} tests in {} test binaries the logs of the fast CI show",
            found.len()
        )
    };
    let holds = |full: &str, named: &str| {
        full == named || full.ends_with(&format!("::{named}")) || named.ends_with(&format!("::{full}"))
    };
    tests
        .iter()
        .map(|named| {
            let time = found
                .iter()
                .find(|b| b.tests.iter().any(|t| holds(t, named)))
                .map(|b| match &b.finished {
                    Some(t) => format!(
                        "{t} for the whole test binary {}, which holds it with {} tests run; cargo test does not time each test on its own",
                        b.name,
                        b.tests.len()
                    ),
                    None => format!("the test binary {} that holds it reported no time", b.name),
                })
                .unwrap_or_else(|| missing.clone());
            json!({ "test": named, "time": time })
        })
        .collect()
}

// Cell 13.

/// The tools a round asked for, as the tool cell takes them from its input.
pub fn asked(gathered: &[Gathered]) -> Vec<Value> {
    gathered
        .first()
        .and_then(|x| x.value.clone())
        .map(|v| match v {
            Value::Array(xs) => xs,
            o => vec![o],
        })
        .unwrap_or_default()
}

/// What running one tool asked for will do, in a line, for the owner to see before it runs.
pub fn describe(req: &Value) -> String {
    let text = |k: &str| req[k].as_str().unwrap_or_default().to_string();
    let short = |h: String| h.chars().take(7).collect::<String>();
    let list = |k: &str| {
        req[k]
            .as_array()
            .map(|xs| xs.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", "))
            .unwrap_or_default()
    };
    match req["tool"].as_str().unwrap_or_default() {
        "ci" => {
            let pr = if req.get("pr").is_some() {
                "; sets the PR's title and description"
            } else {
                ""
            };
            format!(
                "ci: pushes {} and runs the fast CI (tests: {}){pr}",
                short(text("head")),
                list("tests")
            )
        }
        "ci_before" => format!(
            "ci_before: runs the fast CI on {} with these files from it on main: {}",
            short(text("head")),
            list("files")
        ),
        "gh" => format!("gh {}", text("command").trim_start_matches("gh ")),
        "query" => {
            let sql: String = text("sql").split_whitespace().collect::<Vec<_>>().join(" ");
            let cut: String = sql.chars().take(160).collect();
            format!("query: {cut}{}", if cut.len() < sql.len() { "…" } else { "" })
        }
        "measure" => "measure: does not run in this version of the engine".to_string(),
        other => other.to_string(),
    }
}

/// Runs the tools the calling cell asked for. Coming back from the architect, it runs only the tools that failed,
/// and merges in what ran before.
pub fn run_tools(
    e: &mut Engine,
    w: &Work,
    item: &Item,
    round: RoundId,
    gathered: &[Gathered],
) -> Result<Value, String> {
    let project = e.project_of(w)?;
    let asked = asked(gathered);
    let mut results: Vec<Value> = Vec::new();
    let mut only: Option<Vec<String>> = None;
    if w.pos.from.as_deref() == Some("15") {
        let last = work::rounds_at(e.db.conn(), w.id, "13")?
            .into_iter()
            .find(|r| r.transition.is_some());
        if let Some(out) = last.and_then(|r| r.output) {
            results = out["results"].as_array().cloned().unwrap_or_default();
            only = out["failed"].as_array().map(|fs| {
                fs.iter()
                    .filter_map(|f| f["tool"].as_str().map(str::to_string))
                    .collect()
            });
        }
    }
    let mut order: Vec<&Value> = asked.iter().collect();
    // ci pushes first, so ci_before and the others can rely on the push.
    order.sort_by_key(|r| r["tool"] != "ci");
    let mut failed: Vec<Value> = Vec::new();
    let pr = Cell::new(w.pr);
    let mut recorded = w.pr;
    let mut pushed = w.pushed_head.clone();
    for (n, req) in order.into_iter().enumerate() {
        let tool = req["tool"].as_str().unwrap_or_default().to_string();
        if only.as_ref().is_some_and(|o| !o.contains(&tool)) {
            continue;
        }
        let got = match tool.as_str() {
            "ci" => ci(e, w, &project, item, round, req, &mut pushed, &pr),
            "ci_before" => ci_before(e, w, &project, round, n, req, pushed.as_deref()),
            "measure" => Ok(json!({ "push": {
                "verdict": "refused",
                "why": "This version of the engine has no measurement workflow; measure does not run"
            } })),
            "gh" => gh(e, w, &project, round, n, req),
            "query" => query(e, round, n, req),
            other => Ok(json!({ "error": format!("no such tool: {other}") })),
        };
        match got {
            Ok(mut v) => {
                if let Some(o) = v.as_object_mut() {
                    o.insert("tool".into(), Value::String(tool));
                }
                results.push(v);
            }
            Err((f, tried)) => failed.push(json!({ "tool": tool, "kind": f.kind, "what": f.what, "tried": tried })),
        }
        if pr.get() != recorded {
            record_push(e, w, None, pr.get())?;
            recorded = pr.get();
        }
    }
    Ok(json!({ "results": results, "failed": failed }))
}

/// Why a push is refused, or none: the head must be the worktree's HEAD with nothing uncommitted, follow the last
/// push, and carry no other branch than main.
fn push_refusal(wt: &Path, head: &str, pushed: Option<&str>, main: &str) -> Option<String> {
    let Some(sha) = git::resolve(wt, head) else {
        return Some(format!("There is no commit {head} in the worktree"));
    };
    let local = git::head(wt).unwrap_or_default();
    if sha != local {
        return Some(format!(
            "The head is not the one handed in: the worktree's HEAD is {local}"
        ));
    }
    if !git::is_clean(wt).unwrap_or(false) {
        return Some("The worktree has uncommitted changes; commit them, or they do not count".into());
    }
    if let Some(p) = pushed
        && !git::is_ancestor(wt, p, &sha)
    {
        return Some(format!("It does not follow the last push, {p}"));
    }
    let theirs = format!("origin/{main}");
    let merges = git::git(wt, &["rev-list", "--merges", "--parents", &format!("{theirs}..{sha}")]).unwrap_or_default();
    for line in merges.lines() {
        let ids: Vec<&str> = line.split_whitespace().collect();
        for parent in ids.iter().skip(2) {
            if !git::is_ancestor(wt, parent, &theirs) {
                return Some(format!(
                    "It carries another branch: the merge {} brings in {parent}",
                    ids[0]
                ));
            }
        }
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn ci(
    e: &Engine,
    w: &Work,
    project: &config::Project,
    item: &Item,
    round: RoundId,
    req: &Value,
    pushed: &mut Option<String>,
    pr: &Cell<Option<i64>>,
) -> Result<Value, (Failure, String)> {
    let wt = w
        .worktree
        .clone()
        .ok_or_else(|| (Failure::other("the work has no worktree"), String::new()))?;
    let branch = w.branch.clone().unwrap_or_default();
    let head = req["head"].as_str().unwrap_or_default();
    if let Some(why) = push_refusal(&wt, head, pushed.as_deref(), &project.main) {
        return Ok(json!({ "push": { "verdict": "refused", "why": why } }));
    }
    let sha = git::resolve(&wt, head).unwrap_or_default();
    let title_body = req.get("pr").map(|p| {
        (
            p["title"].as_str().unwrap_or(&item.title).to_string(),
            p["body"].as_str().unwrap_or_default().to_string(),
        )
    });
    let fallback = (item.title.clone(), format!("{}\n\nWhy: {}", item.problem, item.why));
    let tests: Vec<String> = req["tests"]
        .as_array()
        .map(|ts| ts.iter().filter_map(|t| t.as_str().map(str::to_string)).collect())
        .unwrap_or_default();
    let got = attempts(e, round, "ci", req, || {
        hub::push(&wt, &sha, &format!("refs/heads/{branch}"), false)?;
        let tb = title_body
            .clone()
            .or_else(|| pr.get().is_none().then(|| fallback.clone()));
        let n = e.github.pull_request(
            &repo(project),
            &branch,
            &project.main,
            tb.as_ref().map(|(t, b)| (t.as_str(), b.as_str())),
            pr.get(),
        )?;
        pr.set(Some(n));
        let checks = e.github.checks(&repo(project), &sha, &project.ci.fast)?;
        let logs: Vec<Result<String, String>> = checks
            .iter()
            .map(|c| {
                e.github
                    .job_log(&repo(project), c)
                    .map_err(|f| format!("{}: {}", c.name, f.what))
            })
            .collect();
        let times = test_times(&tests, &logs);
        Ok(
            json!({ "push": { "verdict": "pushed", "head": sha, "checks": checks_json(&checks), "times": times }, "pr": n }),
        )
    })?;
    *pushed = Some(sha.clone());
    let mut v = got;
    if let Some(o) = v.as_object_mut() {
        o.remove("pr");
    }
    Ok(v)
}

fn ci_before(
    e: &Engine,
    w: &Work,
    project: &config::Project,
    round: RoundId,
    n: usize,
    req: &Value,
    pushed: Option<&str>,
) -> Result<Value, (Failure, String)> {
    let refused = |why: String| Ok(json!({ "ran": { "verdict": "refused", "why": why } }));
    let wt = w
        .worktree
        .clone()
        .ok_or_else(|| (Failure::other("the work has no worktree"), String::new()))?;
    let head = req["head"].as_str().unwrap_or_default();
    let Some(sha) = git::resolve(&wt, head) else {
        return refused(format!("There is no commit {head}"));
    };
    if !pushed.is_some_and(|p| git::is_ancestor(&wt, &sha, p)) {
        return refused(format!("The head {sha} is not pushed; hand it in together with ci"));
    }
    let files: Vec<String> = req["files"]
        .as_array()
        .map(|fs| fs.iter().filter_map(|f| f.as_str().map(str::to_string)).collect())
        .unwrap_or_default();
    for f in &files {
        if !git::under(f, &project.test_paths) {
            return refused(format!(
                "{f} is outside the test paths: {}",
                project.test_paths.join(", ")
            ));
        }
        if git::git(&wt, &["cat-file", "-e", &format!("{sha}:{f}")]).is_err() {
            return refused(format!("{f} is not on the head {sha}"));
        }
    }
    let base = git::git(&wt, &["merge-base", &sha, &format!("origin/{}", project.main)])
        .map(|s| s.trim().to_string())
        .map_err(|x| (Failure::other(x), String::new()))?;
    let refname = format!("refs/heads/wf-agents-before/{}-{}-{n}", round.work, round.n);
    attempts(e, round, "ci_before", req, || {
        let commit = git::files_onto(&wt, &files, &sha, &base).map_err(Failure::other)?;
        hub::push(&wt, &commit, &refname, true)?;
        let checks = e.github.checks(&repo(project), &commit, &project.ci.fast);
        let _ = hub::delete_ref(&wt, &refname);
        Ok(json!({ "ran": { "verdict": "ran", "checks": checks_json(&checks?) } }))
    })
}

fn gh(
    e: &Engine,
    w: &Work,
    project: &config::Project,
    round: RoundId,
    n: usize,
    req: &Value,
) -> Result<Value, (Failure, String)> {
    let line = req["command"].as_str().unwrap_or_default();
    let mut args = match words(line) {
        Ok(a) => a,
        Err(why) => {
            return Ok(
                json!({ "ran": { "verdict": "not_allowed", "why": format!("The command cannot be read: {why}") } }),
            );
        }
    };
    if args.first().is_some_and(|a| a == "gh") {
        args.remove(0);
    }
    if let Some(why) = gh_refusal(&e.config.config.install.gh.allow, &args) {
        return Ok(json!({ "ran": { "verdict": "not_allowed", "why": why } }));
    }
    let (code, out) = attempts(e, round, "gh", req, || {
        let dir = w.worktree.as_deref().unwrap_or(&project.checkout);
        let (code, out) = e.github.gh(dir, &args)?;
        Ok(json!([code, out]))
    })
    .map(|v| {
        (
            v[0].as_i64().unwrap_or(-1),
            v[1].as_str().unwrap_or_default().to_string(),
        )
    })?;
    Ok(json!({ "ran": { "verdict": "ran", "exit": code, "output": inline_or_saved(e, round, n, out) } }))
}

fn query(e: &Engine, round: RoundId, n: usize, req: &Value) -> Result<Value, (Failure, String)> {
    let text = req["sql"].as_str().unwrap_or_default().to_string();
    let limits = e.config.config.install.query.clone();
    let path = config::database(&e.data);
    attempts(e, round, "query", req, || {
        let run = || -> Result<(Vec<Value>, bool), String> {
            let conn = db::open_for_agents(&path, Duration::from_millis(limits.timeout_ms)).map_err(sql)?;
            let max = limits.max_rows as usize;
            let mut rows = work::query_json(&conn, &text, &[], max + 1)?;
            let cut = rows.len() > max;
            rows.truncate(max);
            Ok((rows, cut))
        };
        Ok(match run() {
            Ok((rows, cut)) => {
                let (rows, saved) = rows_given(rows, |text| inline_or_saved(e, round, n, text));
                let mut ran = json!({ "verdict": "ran", "rows": rows, "cut": cut });
                if let Some(s) = saved {
                    ran["saved"] = Value::String(s);
                }
                json!({ "ran": ran })
            }
            Err(why) => json!({ "ran": { "verdict": "error", "error": why } }),
        })
    })
}

// Cell 7.

/// Catches the branch up with main, pushes it, and runs the full CI on it; a red check is run again once.
pub fn full_ci(e: &mut Engine, w: &Work, item: &Item, round: RoundId) -> Result<Value, String> {
    let project = e.project_of(w)?;
    let wt = w.worktree.clone().ok_or("the work has no worktree")?;
    if !git::is_clean(&wt)? {
        return Ok(not_run(
            "result",
            (
                Failure::other("the worktree has uncommitted changes; the engine does not catch up over them"),
                String::new(),
            ),
        ));
    }
    let head = match git::catch_up(&wt, &project.main)? {
        Ok(h) => h,
        Err(files) => return Ok(json!({ "result": { "kind": "conflict", "files": files } })),
    };
    let branch = w.branch.clone().unwrap_or_default();
    let pr = Cell::new(w.pr);
    let args = json!({ "head": head });
    let got = attempts(e, round, "full_ci", &args, || {
        if w.pushed_head.as_deref() != Some(head.as_str()) {
            hub::push(&wt, &head, &format!("refs/heads/{branch}"), false)?;
        }
        if pr.get().is_none() {
            let body = format!("{}\n\nWhy: {}", item.problem, item.why);
            pr.set(Some(e.github.pull_request(
                &repo(&project),
                &branch,
                &project.main,
                Some((item.title.as_str(), body.as_str())),
                None,
            )?));
        }
        let first = full_checks(e, &project, &head)?;
        Ok(json!({ "first": checks_json(&first) }))
    });
    record_push(e, w, Some(&head), pr.get())?;
    let first: Vec<CheckRun> = match got {
        Ok(v) => serde_json::from_value(v["first"].clone()).unwrap_or_default(),
        Err(x) => return Ok(not_run("result", x)),
    };
    let known = known_red(e, &item.project)?;
    if reds(&counted(&first, &project, &known)).is_empty() {
        return Ok(json!({ "result": { "kind": "green", "head": head } }));
    }
    let again = match attempts(e, round, "full_ci", &args, || {
        rerun_full(e, &project, &head, &first).map(|cs| checks_json(&cs))
    }) {
        Ok(v) => serde_json::from_value::<Vec<CheckRun>>(v).unwrap_or_default(),
        Err(x) => return Ok(not_run("result", x)),
    };
    let kind = if reds(&counted(&again, &project, &known)).is_empty() {
        "flaky"
    } else {
        "red"
    };
    Ok(json!({ "result": { "kind": kind, "checks": checks_json(&again) } }))
}

// Cell 9.

/// Catches the PR up with main; when that moved the head, runs the full CI on it again; then merges it.
pub fn merge(e: &mut Engine, w: &Work, item: &Item, round: RoundId) -> Result<Value, String> {
    let project = e.project_of(w)?;
    let wt = w.worktree.clone().ok_or("the work has no worktree")?;
    let Some(pr) = w.pr else {
        return Ok(not_run(
            "result",
            (Failure::other("the work has no PR to merge"), String::new()),
        ));
    };
    let head = match git::catch_up(&wt, &project.main)? {
        Ok(h) => h,
        Err(files) => return Ok(json!({ "result": { "kind": "conflict", "files": files } })),
    };
    let branch = w.branch.clone().unwrap_or_default();
    let args = json!({ "head": head, "pr": pr });
    let checked = attempts(e, round, "merge", &args, || {
        if w.pushed_head.as_deref() != Some(head.as_str()) {
            hub::push(&wt, &head, &format!("refs/heads/{branch}"), false)?;
        }
        Ok(checks_json(&full_checks(e, &project, &head)?))
    });
    record_push(e, w, Some(&head), None)?;
    let mut checks: Vec<CheckRun> = match checked {
        Ok(v) => serde_json::from_value(v).unwrap_or_default(),
        Err(x) => return Ok(not_run("result", x)),
    };
    let known = known_red(e, &item.project)?;
    if !reds(&counted(&checks, &project, &known)).is_empty() {
        checks = match attempts(e, round, "merge", &args, || {
            rerun_full(e, &project, &head, &checks).map(|cs| checks_json(&cs))
        }) {
            Ok(v) => serde_json::from_value(v).unwrap_or_default(),
            Err(x) => return Ok(not_run("result", x)),
        };
        if !reds(&counted(&checks, &project, &known)).is_empty() {
            return Ok(json!({ "result": { "kind": "red", "checks": checks_json(&checks) } }));
        }
    }
    match attempts(e, round, "merge", &args, || {
        e.github.merge(&repo(&project), pr).map(Value::String)
    }) {
        Ok(Value::String(sha)) => Ok(json!({ "result": { "kind": "merged", "merge_commit": sha } })),
        Ok(other) => Err(format!("the merge gave {other}")),
        Err(x) => Ok(not_run("result", x)),
    }
}

// Cell 10.

/// Watches the full CI on the merge commit. On red it runs the red checks again, looks in the table of known red
/// checks, and runs them on main's commit before the merge, in that order.
pub fn watch_main(e: &mut Engine, w: &Work, g: &CompiledGraph, item: &Item, round: RoundId) -> Result<Value, String> {
    let project = e.project_of(w)?;
    if project.deploy.is_some() {
        return Ok(not_run(
            "main",
            (
                Failure::other("this version of the engine does not watch deployments, and this project deploys"),
                String::new(),
            ),
        ));
    }
    let merged = crate::inputs::last_return(e, w, g, "9", "result")?
        .and_then(|r| r["result"]["merge_commit"].as_str().map(str::to_string))
        .ok_or("the work has no merge commit")?;
    let wt = w.worktree.clone().unwrap_or_else(|| project.checkout.clone());
    let args = json!({ "merge_commit": merged });
    let first = match attempts(e, round, "watch_main", &args, || {
        full_checks(e, &project, &merged).map(|c| checks_json(&c))
    }) {
        Ok(v) => serde_json::from_value::<Vec<CheckRun>>(v).unwrap_or_default(),
        Err(x) => return Ok(not_run("main", x)),
    };
    let required = |cs: &[CheckRun]| counted(cs, &project, &[]);
    if reds(&required(&first)).is_empty() {
        return Ok(json!({ "main": { "kind": "green" } }));
    }
    let again = match attempts(e, round, "watch_main", &args, || {
        rerun_full(e, &project, &merged, &first).map(|c| checks_json(&c))
    }) {
        Ok(v) => serde_json::from_value::<Vec<CheckRun>>(v).unwrap_or_default(),
        Err(x) => return Ok(not_run("main", x)),
    };
    let red = reds(&required(&again));
    if red.is_empty() {
        return Ok(json!({ "main": { "kind": "flaky" } }));
    }
    let known = known_red(e, &item.project)?;
    if red.iter().all(|c| known.contains(c)) {
        return Ok(json!({ "main": { "kind": "known_red" } }));
    }
    let _ = git::git(&wt, &["fetch", "-q", "origin", &project.main]);
    let before = git::resolve(&wt, &format!("{merged}^1")).ok_or("main's commit before the merge is not known here")?;
    let earlier = match attempts(e, round, "watch_main", &json!({ "before": before }), || {
        full_checks(e, &project, &before).map(|c| checks_json(&c))
    }) {
        Ok(v) => serde_json::from_value::<Vec<CheckRun>>(v).unwrap_or_default(),
        Err(x) => return Ok(not_run("main", x)),
    };
    let red_before: Vec<&CheckRun> = earlier
        .iter()
        .filter(|c| red.contains(&c.name) && c.result != "green")
        .collect();
    if !red_before.is_empty() {
        let link = red_before.iter().map(|c| c.run.as_str()).collect::<Vec<_>>().join(" ");
        return Ok(json!({ "main": {
            "kind": "environment_changed",
            "checks": checks_json(&again),
            "before": format!("{before} {link}")
        } }));
    }
    let latest: Vec<CheckRun> = again.into_iter().filter(|c| red.contains(&c.name)).collect();
    Ok(json!({ "main": { "kind": "broken_by_merge", "checks": checks_json(&latest) } }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_command_line_is_split_as_a_shell_would() {
        assert_eq!(
            words("pr view 12 --json title").unwrap(),
            ["pr", "view", "12", "--json", "title"]
        );
        assert_eq!(
            words(r#"api 'repos/a/b/pulls?state=all' --jq ".[] | .title""#).unwrap(),
            ["api", "repos/a/b/pulls?state=all", "--jq", ".[] | .title"]
        );
        assert_eq!(words(r#"a "" b\ c"#).unwrap(), ["a", "", "b c"]);
        assert!(words("pr view 'x").is_err());
    }

    #[test]
    fn a_named_test_is_given_the_time_of_the_test_binary_that_holds_it() {
        let log = "     Running unittests src/lib.rs (target/debug/deps/engine-0a1b2c3d4e5f6789)
running 2 tests
test tools::tests::a_command_line_is_split_as_a_shell_would ... ok
test tools::tests::only_allowed_gh_commands_run_and_api_only_reads ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

     Running tests/e2e.rs (target/debug/deps/e2e-99aa88bb77cc66dd)
running 1 test
test a_takeover_waits_for_a_free_slot_at_the_head_of_the_queue ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 16 filtered out; finished in 68.41s
";
        let named = vec![
            "a_takeover_waits_for_a_free_slot_at_the_head_of_the_queue".to_string(),
            "engine::tools::tests::a_command_line_is_split_as_a_shell_would".to_string(),
            "missing".to_string(),
        ];
        let times = test_times(&named, &[Ok(log.to_string())]);
        assert_eq!(
            times[0]["time"],
            "68.41s for the whole test binary e2e (tests/e2e.rs), which holds it with 1 tests run; cargo test does not time each test on its own"
        );
        assert!(
            times[1]["time"]
                .as_str()
                .unwrap()
                .starts_with("0.01s for the whole test binary engine (unittests src/lib.rs)")
        );
        assert_eq!(
            times[2],
            json!({ "test": "missing", "time": "not found among the 3 tests in 2 test binaries the logs of the fast CI show" })
        );
        let unread = test_times(&named[..1], &[Err("check: HTTP 404".to_string())]);
        assert_eq!(
            unread[0]["time"],
            "the logs of the fast CI could not be read: check: HTTP 404"
        );
    }

    // Lines copied from a real job log, as gh gives it: the job, the step and the time before each line, and the
    // colour escapes cargo writes, their escape character written "^[" by gh.
    #[test]
    fn a_named_test_is_found_in_a_real_job_log() {
        let log = concat!(
            "check\tSet up job\t\u{feff}2026-10-27T11:11:11.3443351Z Current runner version: '2.338.0'\n",
            "check\tTest\t2026-10-27T11:12:23.7450531Z ^[[1m^[[92m     Running^[[0m unittests src/lib.rs (target/debug/deps/api-665d544996ddbb0e)\n",
            "check\tTest\t2026-10-27T11:12:23.7464421Z \n",
            "check\tTest\t2026-10-27T11:12:23.7464632Z running 21 tests\n",
            "check\tTest\t2026-10-27T11:12:23.7470165Z test control::tests::a_round_of_one_work_is_asked_only_while_holding_and_once_at_a_time ... ok\n",
            "check\tTest\t2026-10-27T11:12:53.2568286Z test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 29.51s\n",
        );
        let named = ["a_round_of_one_work_is_asked_only_while_holding_and_once_at_a_time".to_string()];
        let times = test_times(&named, &[Ok(log.to_string())]);
        assert_eq!(
            times[0]["time"],
            "29.51s for the whole test binary api (unittests src/lib.rs), which holds it with 1 tests run; cargo test does not time each test on its own"
        );
    }

    #[test]
    fn a_query_gives_its_rows_as_objects_and_saves_them_one_per_line_when_too_long() {
        let rows = vec![
            json!({ "id": 206, "title": "Say \"done\"" }),
            json!({ "id": 207, "title": null }),
        ];
        let given = rows_given(rows.clone(), |_| unreachable!("short rows are given here"));
        assert_eq!(given, (rows, None));
        let long: Vec<Value> = (0..2000).map(|i| json!({ "id": i, "text": "x".repeat(20) })).collect();
        let kept = std::cell::RefCell::new(String::new());
        let (given, saved) = rows_given(long, |text| {
            *kept.borrow_mut() = text;
            "saved to f".to_string()
        });
        assert!(given.is_empty());
        assert_eq!(saved.as_deref(), Some("saved to f"));
        let kept = kept.into_inner();
        assert_eq!(kept.lines().count(), 2000);
        assert_eq!(kept.lines().next(), Some(r#"{"id":0,"text":"xxxxxxxxxxxxxxxxxxxx"}"#));
    }

    #[test]
    fn only_allowed_gh_commands_run_and_api_only_reads() {
        let allow: Vec<String> = ["pr view", "run view", "api repos/"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let w = |s: &str| words(s).unwrap();
        assert_eq!(gh_refusal(&allow, &w("pr view 12")), None);
        assert_eq!(gh_refusal(&allow, &w("api repos/a/b/pulls")), None);
        assert!(gh_refusal(&allow, &w("pr viewer")).is_some());
        assert!(gh_refusal(&allow, &w("pr merge 12")).is_some());
        assert!(gh_refusal(&allow, &w("api repos/a/b/issues -f title=x")).is_some());
        assert!(gh_refusal(&allow, &w("api repos/a/b/pulls/1 --method=PATCH")).is_some());
        assert!(gh_refusal(&allow, &w("api graphql")).is_some());
    }
}
