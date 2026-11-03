//! The tool cells: the tools agents ask for (cell `X1`, in each graph), the full CI on the draft (`T2`),
//! the merge queue (`M2`) and watching main (`L1`). Each attempt of a tool is a row of tool_runs under
//! the cell's round. A tool that fails is tried twice more; a refusal is a result, not a failure, and is not tried
//! again.

use std::cell::Cell;
use std::path::Path;
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use db::Actor;
use db::rusqlite::params;
use flow::CompiledGraph;
use serde_json::{Value, json};

use crate::hub::{self, CheckRun, Failure};
use crate::inputs::Gathered;
use crate::work::{self, Item, RoundId, Work};
use crate::{Engine, git, test_name};
use sessions::worktree::Unfit;

/// How many times a tool is tried before it is given up.
const ATTEMPTS: i64 = 3;

/// The longest output given inline; a longer one is saved and its path given.
const LONG: usize = 20_000;

/// Where the temporary branches ci_before pushes the code before the change to are: a project's fast CI that takes
/// the tests to run leaves them alone on push, and runs on one only as ci_before asks it to.
const BEFORE: &str = "wf-agents-before";

/// Where the temporary refs `measure` pushes the code to measure to are: the fast CI leaves them alone on push, and a
/// project's measurement workflow runs on one only as `measure` asks it to.
const MEASURE: &str = "wf-agents-measure";

/// What `measure` refuses with when the project's configuration lists no measurement workflow.
const NO_MEASURE: &str = "This project's configuration lists no measurement workflow (ci.measure), so measure does \
    not run. Do not wait for it: where your answer has a measure field, answer not_measured and say in why how you \
    judged without a measurement.";

fn sql(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// Runs one tool up to [`ATTEMPTS`] times, each attempt a row of tool_runs; gives what the last attempt gave, or the
/// failure with what each attempt gave. An attempt is given the engine, so that what it must keep before a long wait it
/// can write ([`record_push`]).
fn attempts(
    e: &mut Engine,
    round: RoundId,
    tool: &str,
    args: &Value,
    mut f: impl FnMut(&mut Engine) -> Result<Value, Failure>,
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
        let got = f(e);
        let result = match &got {
            Ok(v) => v.clone(),
            Err(x) => json!({ "failed": { "kind": x.kind, "what": x.what } }),
        };
        // The files a tool kept: its result's `files`, or, for a tool whose result is a `push`, that one's.
        let files: Vec<String> = ["/files", "/push/files"]
            .iter()
            .find_map(|at| result.pointer(at).and_then(Value::as_array))
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
                // While the network is down no attempt can do better: the step waits for the network instead. A request
                // GitHub refused itself is refused the same way again.
                if n == ATTEMPTS || x.kind == "rejected" || (x.kind == "network_or_api" && e.network_down()) {
                    return Err((x, format!("Tried {n} times. {}", tried.join("; "))));
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

/// The flags that make a gh `api` request write or take its request from elsewhere; the tool refuses a request that has
/// one, and the description of the gh tool in the flow names each of them.
const API_WRITES: [&str; 7] = ["-X", "--method", "-f", "-F", "--field", "--raw-field", "--input"];

/// The short flags of gh `api` that take a value, which in a group of short flags (`-iXPOST`) end it: the rest of the
/// group is the value.
const API_SHORT_WITH_VALUE: &str = "FfHXpqt";

/// Whether an argument of gh `api` is one of the [`API_WRITES`] as gh reads it: a long flag by itself or with `=` and
/// its value; a short flag at the start of the argument or in a group of short flags, with its value attached or not
/// (`-XPOST`, `-fname=value`, `-iXPOST`).
fn api_write(arg: &str) -> bool {
    if let Some(long) = arg.strip_prefix("--") {
        let name = long.split('=').next().unwrap_or_default();
        return API_WRITES.iter().any(|x| x.strip_prefix("--") == Some(name));
    }
    let Some(shorts) = arg.strip_prefix('-') else {
        return false;
    };
    for c in shorts.chars() {
        if API_WRITES
            .iter()
            .any(|x| x.strip_prefix('-').is_some_and(|s| s.chars().eq([c])))
        {
            return true;
        }
        if API_SHORT_WITH_VALUE.contains(c) {
            break;
        }
    }
    false
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
    if args.first().is_some_and(|a| a == "api")
        && let Some(w) = args.iter().skip(1).find(|a| api_write(a))
    {
        return Some(format!("gh api may only read: {w} makes it write"));
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
    full_checks_on(e.github(), project, sha, None)
}

/// Every check of the project's own full CI on a commit of a work's branch. Where the project's CI runs on demand, a
/// commit that has a run of a full workflow reads it, waiting for it if it is still going, whoever made it; one that
/// has none is run by hand first, on the branch. The fast workflow is not among the project's full workflows, and a
/// run of it says nothing of the full CI. Else the pushes of the branch made the runs, and this only waits for them.
fn work_full_checks(e: &Engine, project: &config::Project, branch: &str, sha: &str) -> Result<Vec<CheckRun>, Failure> {
    full_checks_on(e.github(), project, sha, Some(branch))
}

/// The latest run of each of the project's own full workflows on a commit, read when it is there, and, given the
/// branch the commit is the head of and the project's CI running on demand, made when it is not.
fn full_checks_on(
    github: &dyn hub::GitHub,
    project: &config::Project,
    sha: &str,
    branch: Option<&str>,
) -> Result<Vec<CheckRun>, Failure> {
    let (own, downstream) = own_full(project);
    if !downstream.is_empty() {
        return Err(Failure::other(format!(
            "this version of the engine runs no downstream project's workflow: {}",
            downstream.join(", ")
        )));
    }
    let repo = repo(project);
    let mut out = Vec::new();
    for wf in own {
        if let Some(branch) = branch.filter(|_| project.ci.on_demand)
            && github.latest_run_id(&repo, sha, &wf)?.is_none()
        {
            github.dispatch(&repo, &wf, branch, &[]).map_err(dispatch_failed)?;
        }
        out.extend(github.checks(&repo, sha, &wf)?);
    }
    Ok(out)
}

/// What a failed dispatch is told with, when the workflow it asked for is not on the branch: a branch made before the
/// workflow existed has none, and takes it with main.
const DISPATCH_HINT: &str = "If the workflow is not on the branch, which one made before the workflow existed does not \
                             have, merge origin/main into the branch and hand the head in again";

fn dispatch_failed(f: Failure) -> Failure {
    // A request GitHub refused (an input the workflow lacks, say) is told as it is: the workflow is there.
    if f.kind == "rejected" {
        return f;
    }
    Failure {
        kind: f.kind,
        what: format!("{}. {DISPATCH_HINT}", f.what),
    }
}

/// What a wait for the run that a push to a temporary branch should make is told when none came: the project's CI may
/// not run on those branches, which its configuration then says.
const BEFORE_HINT: &str = "If the project's CI does not run on the temporary branches the code before the change is \
                           put on, set runs_on_before_branches = false or name tests_input in its configuration";

fn no_run_on_before_branch(f: Failure) -> Failure {
    if f.kind != "ci_did_not_start" {
        return f;
    }
    Failure {
        kind: f.kind,
        what: format!("{}. {BEFORE_HINT}", f.what),
    }
}

/// The checks of the fast CI on the code before the change, which is `commit` on the temporary `branch`: the run of
/// the tests `listed` that the engine makes by hand where the workflow takes them (`named` is its input), else the
/// run the push of the commit made.
fn before_checks(
    github: &dyn hub::GitHub,
    project: &config::Project,
    branch: &str,
    commit: &str,
    named: Option<&str>,
    listed: &str,
) -> Result<Vec<CheckRun>, Failure> {
    let (repo, fast) = (repo(project), project.ci.fast.as_str());
    match named {
        Some(input) => {
            github
                .dispatch(&repo, fast, branch, &[(input, listed)])
                .map_err(dispatch_failed)?;
            github.checks(&repo, commit, fast)
        }
        None => github.checks(&repo, commit, fast).map_err(no_run_on_before_branch),
    }
}

/// The fast CI on a commit of a work's branch, and what it ran: the engine's own run of the fast workflow with the
/// tests named when the project's CI runs on demand, and then only the run of that workflow this made, never one that
/// was there before; else the run the push of the commit made, the whole of the fast CI.
fn fast_checks(
    github: &dyn hub::GitHub,
    project: &config::Project,
    branch: &str,
    sha: &str,
    tests: &[String],
) -> Result<(&'static str, Vec<CheckRun>), Failure> {
    let (repo, fast) = (repo(project), project.ci.fast.as_str());
    if !project.ci.on_demand {
        return Ok(("whole", github.checks(&repo, sha, fast)?));
    }
    let input = project.ci.tests_input.as_deref().ok_or_else(|| {
        Failure::other("the project's CI runs on demand but names no input that takes the tests to run")
    })?;
    // The runs of the workflow on the commit there are now are not the one asked for: that is numbered above them.
    let before = github.latest_run_id(&repo, sha, fast)?;
    github
        .dispatch(&repo, fast, branch, &[(input, tests.join(" ").as_str())])
        .map_err(dispatch_failed)?;
    let run = if tests.is_empty() { "lint" } else { "named_and_lint" };
    Ok((run, github.checks_after(&repo, sha, fast, before)?))
}

/// The checks a merge needs that a run of the full CI does not have, as red checks that say so, where the project's CI
/// runs on demand: a run without them is not a run of the full CI, whatever else it ran, and a merge must not take
/// the absence of a check for a pass. Those the table of known red checks excuses are not asked for.
fn missing_checks(checks: &[CheckRun], project: &config::Project, known_red: &[String]) -> Vec<CheckRun> {
    if !project.ci.on_demand {
        return Vec::new();
    }
    project
        .ci
        .merge_checks
        .iter()
        .filter(|n| !known_red.contains(n) && !checks.iter().any(|c| &c.name == *n))
        .map(|n| CheckRun {
            name: n.clone(),
            result: "red".to_string(),
            run: String::new(),
            log_tail: Some(format!(
                "The run of the full CI has no check named {n}, which a merge needs"
            )),
            log_path: None,
        })
        .collect()
}

/// Whether a check of the project's full CI is red beside the change: in the project's table of known red checks, or
/// red on main as the worktree last fetched it. What a work says its change did not cause must be one or the other.
/// None when main's run of the check cannot be read: main is not known in the worktree, its checks cannot be fetched,
/// or the check did not run there.
pub(crate) fn red_beside_the_change(
    e: &Engine,
    name: &str,
    project: &config::Project,
    wt: &Path,
    check: &str,
) -> Result<Option<bool>, String> {
    if known_red(e, name)?.iter().any(|k| k == check) {
        return Ok(Some(true));
    }
    let Some(main) = git::resolve(wt, &format!("origin/{}", project.main)) else {
        return Ok(None);
    };
    let Ok(checks) = full_checks(e, project, &main) else {
        return Ok(None);
    };
    Ok(checks.iter().find(|c| c.name == check).map(|c| c.result == "red"))
}

/// Runs the red checks again on the same commit, and gives the latest run of every check.
fn rerun_full(e: &Engine, project: &config::Project, sha: &str, first: &[CheckRun]) -> Result<Vec<CheckRun>, Failure> {
    let (own, _) = own_full(project);
    let mut out = Vec::new();
    for wf in own {
        out.extend(e.github().rerun(&repo(project), sha, &wf)?);
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
    // A request GitHub refused is not one of the kinds the flow lists for a tool that did not run: it is told as `other`,
    // with GitHub's words.
    let (kind, what) = if f.kind == "rejected" {
        ("other", format!("GitHub refused the request: {}", f.what))
    } else {
        (f.kind, f.what)
    };
    json!({ field: { "kind": "not_run", "failure": kind, "what": what, "tried": tried } })
}

fn checks_json(cs: &[CheckRun]) -> Value {
    serde_json::to_value(cs).unwrap_or(Value::Null)
}

/// Records the head pushed and the PR, a PR the moment it is opened, before the tool that opened it waits for CI, so
/// that whatever ends the work afterwards knows the PR to close, though the engine dies in that wait and a cancel is the
/// first thing it carries out when it starts again. The head a crash loses does no harm: the next push follows it.
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

/// A test as a cargo-nextest log ends it: how it ended, how long it took, its test binary and its name, read from a
/// line such as `PASS [   1.234s] (  3/520) engine::e2e a_takeover_waits_for_a_free_slot_at_the_head_of_the_queue`.
struct Timed {
    status: String,
    time: String,
    binary: String,
    test: String,
}

/// How nextest says a test ended, besides a signal (SIGSEGV and the like). Its other lines with a time (SLOW, a
/// retry's TRY, the Summary) end no test.
const ENDED: [&str; 6] = ["PASS", "FAIL", "TIMEOUT", "LEAK", "LEAK-FAIL", "ABORT"];

/// The tests a cargo-nextest log shows ended, in the order they ended, each once. Each line is read as a reader sees
/// it: without the job, the step and the time gh puts before it, and without colour escapes.
fn timed(log: &str) -> Vec<Timed> {
    let mut out: Vec<Timed> = Vec::new();
    for line in hub::clean_log(log).lines() {
        let Some((status, rest)) = line.trim().split_once(" [") else {
            continue;
        };
        if !ENDED.contains(&status) && !status.starts_with("SIG") {
            continue;
        }
        let Some((time, rest)) = rest.split_once(']') else {
            continue;
        };
        let time = time.trim();
        if time.strip_suffix('s').is_none_or(|t| t.parse::<f64>().is_err()) {
            continue;
        }
        // Nextest may count the tests before each one's binary: "(  3/520)".
        let rest = rest.trim_start();
        let rest = match rest.strip_prefix('(').and_then(|r| r.split_once(')')) {
            Some((_, after)) => after.trim_start(),
            None => rest,
        };
        let Some((binary, test)) = rest.split_once(' ') else {
            continue;
        };
        let test = test.trim();
        // The summary at the end names the tests that did not pass again.
        if out.iter().any(|t| t.binary == binary && t.test == test) {
            continue;
        }
        out.push(Timed {
            status: status.to_string(),
            time: time.to_string(),
            binary: binary.to_string(),
            test: test.to_string(),
        });
    }
    out
}

/// What became of each named test in a run, as the logs of its jobs tell, cargo-nextest writing a line as each test
/// ends: passed, with its time; failed, with how it ended and its time; or not run, when no log shows it, as when its
/// job stopped at an earlier red test or there is no such test, so that a test the run did not reach is never read
/// as measured; and when no log could be read, unread, with why. A test is named as [`test_name`] says.
fn test_results(tests: &[String], logs: &[Result<String, String>]) -> Vec<Value> {
    let found: Vec<Timed> = logs
        .iter()
        .filter_map(|l| l.as_ref().ok())
        .flat_map(|l| timed(l))
        .collect();
    let unread: Vec<&str> = logs
        .iter()
        .filter_map(|l| l.as_ref().err().map(String::as_str))
        .collect();
    let holds = |t: &Timed, named: &str| test_name::matches_run(named, &t.binary, &t.test);
    tests
        .iter()
        .map(|named| match found.iter().find(|t| holds(t, named)) {
            Some(t) if t.status == "PASS" => json!({ "test": named, "result": "passed", "time": t.time }),
            Some(t) => json!({ "test": named, "result": "failed", "time": t.time, "ended": t.status }),
            None if found.is_empty() && !unread.is_empty() => json!({
                "test": named, "result": "unread",
                "why": format!("the logs of the run could not be read: {}", unread.join("; ")),
            }),
            None => json!({ "test": named, "result": "not_run" }),
        })
        .collect()
}

// Cell `X1`.

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

/// What a tool gave, or how it failed with what each attempt gave.
type Ran = Result<Value, (Failure, String)>;

/// Runs the tools the calling cell asked for. Coming back from the architect, it runs only the tools that failed,
/// and merges in what ran before. A work of an architect is on no item: it runs what reads.
pub fn run_tools(
    e: &mut Engine,
    w: &Work,
    item: Option<&Item>,
    round: RoundId,
    gathered: &[Gathered],
) -> Result<Value, String> {
    let project = match item {
        Some(_) => Some(e.project_of(w)?),
        None => None,
    };
    let asked = asked(gathered);
    let mut results: Vec<Value> = Vec::new();
    let mut only: Option<Vec<String>> = None;
    if w.pos.from.as_deref() == Some("A2") {
        let last = work::rounds_at(e.db.conn(), w.id, "X1")?
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
    // ci first, so that the numbers name the requests in the order they used to run in.
    order.sort_by_key(|r| r["tool"] != "ci");
    // Each request with its place in that order, which names its temporary branch and output file and orders its result.
    let picked: Vec<(usize, &Value, String)> = order
        .into_iter()
        .enumerate()
        .map(|(n, req)| (n, req, req["tool"].as_str().unwrap_or_default().to_string()))
        .filter(|(_, _, tool)| !only.as_ref().is_some_and(|o| !o.contains(tool)))
        .collect();
    let mut got: Vec<(usize, String, Ran)> = Vec::new();
    let mut pushed = w.pushed_head.clone();
    let pushing = item.zip(project.as_ref());
    if let Some((item, project)) = pushing {
        // A tool starts once what it may depend on is there: ci_before on its head being pushed by ci, not on ci's run,
        // so each runs on a thread of its own beside ci's wait for its CI; the others, which may read what both did,
        // after both.
        let befores: Vec<&(usize, &Value, String)> = picked.iter().filter(|(_, _, t)| t == "ci_before").collect();
        let mut contexts = Vec::new();
        for _ in &befores {
            contexts.push(e.fork()?);
        }
        let start = pushed.clone();
        std::thread::scope(|s| {
            let (senders, receivers): (Vec<mpsc::Sender<String>>, Vec<Receiver<String>>) =
                befores.iter().map(|_| mpsc::channel()).unzip();
            let threads: Vec<_> = befores
                .iter()
                .zip(receivers)
                .zip(contexts)
                .map(|((&&(n, req, ref tool), told), mut ctx)| {
                    let start = start.as_deref();
                    let run = s.spawn(move || ci_before(&mut ctx, w, project, round, n, req, start, Some(&told)));
                    (n, tool.clone(), run)
                })
                .collect();
            let pr = Cell::new(w.pr);
            let tell = |sha: &str| {
                for s in &senders {
                    let _ = s.send(sha.to_string());
                }
            };
            for (n, req, tool) in picked.iter().filter(|(_, _, t)| t == "ci") {
                got.push((
                    *n,
                    tool.clone(),
                    ci(e, w, project, item, round, req, &mut pushed, &pr, &tell),
                ));
            }
            // ci has pushed all it will: a ci_before still waiting for its head is refused.
            drop(senders);
            for (n, tool, run) in threads {
                let ran = run.join().unwrap_or_else(|p| {
                    let why = p
                        .downcast_ref::<&str>()
                        .map(|s| s.to_string())
                        .or_else(|| p.downcast_ref::<String>().cloned())
                        .unwrap_or_default();
                    Err((
                        Failure::other(format!("ci_before stopped: its thread panicked: {why}")),
                        String::new(),
                    ))
                });
                got.push((n, tool, ran));
            }
        });
    }
    for (n, req, tool) in &picked {
        let n = *n;
        let ran = match (tool.as_str(), pushing) {
            ("ci" | "ci_before", Some(_)) => continue,
            ("measure", Some((_, project))) => measure(e, w, project, round, n, req, pushed.as_deref()),
            ("gh", _) => gh(e, w, project.as_ref(), round, n, req),
            ("query", _) => query(e, round, n, req),
            (other, _) => Ok(json!({ "error": format!("no such tool for this work: {other}") })),
        };
        got.push((n, tool.clone(), ran));
    }
    got.sort_by_key(|(n, _, _)| *n);
    let mut failed: Vec<Value> = Vec::new();
    for (_, tool, ran) in got {
        match ran {
            Ok(mut v) => {
                if let Some(o) = v.as_object_mut() {
                    o.insert("tool".into(), Value::String(tool));
                }
                results.push(v);
            }
            Err((f, tried)) => failed.push(json!({ "tool": tool, "kind": f.kind, "what": f.what, "tried": tried })),
        }
    }
    Ok(json!({ "results": results, "failed": failed }))
}

/// Why a push is refused, or none: the head must be the worktree's HEAD with nothing uncommitted, follow the last
/// push, and carry no other branch than main.
fn push_refusal(wt: &Path, head: &str, pushed: Option<&str>, main: &str) -> Option<String> {
    // The session manager says whether the head handed in is the clean worktree's.
    let sha = match sessions::worktree::handed_in(wt, head) {
        Ok(sha) => sha,
        Err(Unfit::NoCommit) => return Some(format!("There is no commit {head} in the worktree")),
        Err(Unfit::NotHead(local)) => {
            return Some(format!(
                "The head is not the one handed in: the worktree's HEAD is {local}"
            ));
        }
        Err(Unfit::Uncommitted) => {
            return Some("The worktree has uncommitted changes; commit them, or they do not count".into());
        }
        Err(Unfit::Git(e)) => return Some(format!("Git could not read the worktree: {e}")),
    };
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

/// The title and description a pull request is opened with when the engineer has handed in none, at its first push
/// (`ci`) or when the full CI opens it (`full_ci`): fixed English text naming the item by its number, and none of the
/// item's own text, which is in the owner's language, since everything in a pull request is in English. The engineer's
/// `pr` at a later push replaces both.
fn pr_fallback(item: &Item) -> (String, String) {
    (
        format!("Item {}", item.id),
        format!(
            "The pull request of item {}. Its title and description are the engineer's to hand in, and replace these \
             when it does.",
            item.id
        ),
    )
}

/// Pushes the head and waits for its fast CI; `told` hears each head as soon as it is pushed, before the wait, so that a
/// run of the code before the change can start meanwhile.
#[allow(clippy::too_many_arguments)]
fn ci(
    e: &mut Engine,
    w: &Work,
    project: &config::Project,
    item: &Item,
    round: RoundId,
    req: &Value,
    pushed: &mut Option<String>,
    pr: &Cell<Option<i64>>,
    told: &dyn Fn(&str),
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
    let fallback = pr_fallback(item);
    let title_body = req.get("pr").map(|p| {
        (
            p["title"].as_str().unwrap_or(&fallback.0).to_string(),
            p["body"].as_str().unwrap_or_default().to_string(),
        )
    });
    let tests: Vec<String> = req["tests"]
        .as_array()
        .map(|ts| ts.iter().filter_map(|t| t.as_str().map(str::to_string)).collect())
        .unwrap_or_default();
    let got = attempts(e, round, "ci", req, |e| {
        hub::push(&wt, &sha, &format!("refs/heads/{branch}"), false)?;
        told(&sha);
        let tb = title_body
            .clone()
            .or_else(|| pr.get().is_none().then(|| fallback.clone()));
        let n = e.github().pull_request(
            &repo(project),
            &branch,
            &project.main,
            tb.as_ref().map(|(t, b)| (t.as_str(), b.as_str())),
            pr.get(),
        )?;
        // Known only once written: an attempt whose write failed leaves it unknown, and the next writes it.
        if pr.get() != Some(n) {
            record_push(e, w, None, Some(n)).map_err(Failure::other)?;
            pr.set(Some(n));
        }
        // A pull request that was open already, one a work took over among them, keeps the base it was opened with.
        hub::ensure_base(e.github(), &repo(project), n, &project.main)?;
        let (run, checks) = fast_checks(e.github(), project, &branch, &sha, &tests)?;
        let logs: Vec<Result<String, String>> = checks
            .iter()
            .map(|c| {
                e.github()
                    .job_log(&repo(project), c)
                    .map_err(|f| format!("{}: {}", c.name, f.what))
            })
            .collect();
        let tests = test_results(&tests, &logs);
        let mut push = json!({
            "verdict": "pushed", "head": sha, "run": run, "checks": checks_json(&checks), "tests": tests
        });
        let artifact = project.ci.generated_artifact.as_deref();
        if let Some(g) = generated(e.github(), &repo(project), artifact, &checks, &wt) {
            push["generated"] = g;
        }
        Ok(json!({ "push": push, "pr": n }))
    });
    // GitHub refused the request to run the fast CI itself, such as an input the workflow does not take: trying again
    // cannot help, and the engineer, who can change the request, is told what GitHub said.
    let got = match got {
        Err((f, _)) if f.kind == "rejected" => {
            return Ok(json!({ "push": { "verdict": "refused", "why": format!(
                "GitHub refused to run {}: {}", project.ci.fast, f.what
            ) } }));
        }
        other => other?,
    };
    *pushed = Some(sha.clone());
    let mut v = got;
    if let Some(o) = v.as_object_mut() {
        o.remove("pr");
    }
    Ok(v)
}

/// What the engineer is told of the generated files put in its worktree.
const GENERATED: &str = "What the run generated from your head, put in your worktree uncommitted: commit these files \
                         with your next commit, and the check comparing them with the committed ones passes";

/// The files a red run of the fast CI generated that differ from the worktree's, written there uncommitted for the
/// engineer to commit: those of the artifact the project's configuration names (`generated_artifact`), each at its
/// path in the repository. None when the run is green, the project names no such artifact, the run has none, or each
/// of its files is as the worktree has it; what could not be fetched or written is said.
fn generated(
    github: &dyn hub::GitHub,
    repo: &str,
    artifact: Option<&str>,
    checks: &[CheckRun],
    wt: &Path,
) -> Option<Value> {
    let name = artifact?;
    let red = checks.iter().find(|c| c.result != "green")?;
    let files = match github.artifact(repo, red, name) {
        Ok(fs) => fs?,
        Err(f) => {
            return Some(json!({
                "files": [],
                "note": format!("The run's generated files could not be fetched: {}", f.what),
            }));
        }
    };
    let mut put: Vec<String> = Vec::new();
    for (path, bytes) in files {
        let rel = Path::new(&path);
        let to = wt.join(rel);
        let written = if !rel.components().all(|c| matches!(c, std::path::Component::Normal(_))) {
            Err(format!("{path} is not a path in the repository"))
        } else if std::fs::read(&to).is_ok_and(|b| b == bytes) {
            continue;
        } else {
            to.parent()
                .map_or(Ok(()), std::fs::create_dir_all)
                .and_then(|()| std::fs::write(&to, &bytes))
                .map_err(|x| format!("{path}: {x}"))
        };
        if let Err(why) = written {
            return Some(json!({
                "files": put,
                "note": format!("Not every file the run generated could be put in your worktree: {why}"),
            }));
        }
        put.push(path);
    }
    (!put.is_empty()).then(|| json!({ "files": put, "note": GENERATED }))
}

/// A request for ci_before as the run records it: its head is the commit it resolved to, whatever the request called it.
fn recorded_request(req: &Value, sha: &str) -> Value {
    let mut recorded = req.clone();
    recorded["head"] = json!(sha);
    recorded
}

/// Whether a pushed head covers `sha`: the push recorded when the round began, or one the round's ci reports as it pushes
/// (`told`), waited for until ci has pushed everything it will.
fn pushed_covering(wt: &Path, sha: &str, pushed: Option<&str>, told: Option<&Receiver<String>>) -> bool {
    if pushed.is_some_and(|p| git::is_ancestor(wt, sha, p)) {
        return true;
    }
    let Some(told) = told else { return false };
    while let Ok(p) = told.recv() {
        if git::is_ancestor(wt, sha, &p) {
            return true;
        }
    }
    false
}

/// Runs the tests named on the code before the change; `pushed` is the head pushed when the round began, and `told`
/// hears the heads the round's ci pushes, so that this runs beside ci's wait for its own CI rather than after it.
#[allow(clippy::too_many_arguments)]
fn ci_before(
    e: &mut Engine,
    w: &Work,
    project: &config::Project,
    round: RoundId,
    n: usize,
    req: &Value,
    pushed: Option<&str>,
    told: Option<&Receiver<String>>,
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
    if !pushed_covering(&wt, &sha, pushed, told) {
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
    let tests: Vec<String> = req["tests"]
        .as_array()
        .map(|ts| ts.iter().filter_map(|t| t.as_str().map(str::to_string)).collect())
        .unwrap_or_default();
    // Where the fast CI takes the tests to run, it runs exactly the named ones, each to its end, by hand on the
    // temporary branch; else the push runs the whole of it, which stops at a job's first red test. A push runs it only
    // where the project says its fast CI runs on those branches: else nothing would run and the wait for a run would be
    // in vain, so this refuses before it pushes.
    let named = project.ci.tests_input.as_deref();
    if named.is_none() && !project.ci.runs_on_before_branches {
        return refused(
            "The project's fast CI does not run on the temporary branches the code before the change is put on (its \
             configuration sets runs_on_before_branches to false) and takes no tests to run on their own (it names \
             no tests_input), so it cannot run the tests you list there; nothing was pushed"
                .to_string(),
        );
    }
    let listed = tests.join(" ");
    let tag = format!("{}-{}-{n}", round.work, round.n);
    let branch = format!("{BEFORE}/{tag}");
    let refname = format!("refs/heads/{branch}");
    // The run is recorded with the commit the head resolved to now: a head written as `HEAD` or a branch name names
    // another commit later, and the check of which files changed since the run compares against this one.
    let recorded = recorded_request(req, &sha);
    attempts(e, round, "ci_before", &recorded, |e| {
        let commit = git::files_onto(&wt, &files, &sha, &base, &tag).map_err(Failure::other)?;
        hub::push(&wt, &commit, &refname, true)?;
        let ran = before_checks(e.github(), project, &branch, &commit, named, &listed);
        // GitHub refused the request itself: trying again cannot help, and the engineer is told what GitHub said.
        if let Err(f) = &ran
            && f.kind == "rejected"
        {
            let _ = hub::delete_ref(&wt, &refname);
            return Ok(json!({ "ran": { "verdict": "refused", "why": format!(
                "GitHub refused to run {}: {}", project.ci.fast, f.what
            ) } }));
        }
        let logs: Vec<Result<String, String>> = ran
            .iter()
            .flatten()
            .map(|c| {
                e.github()
                    .job_log(&repo(project), c)
                    .map_err(|f| format!("{}: {}", c.name, f.what))
            })
            .collect();
        let _ = hub::delete_ref(&wt, &refname);
        let checks = ran?;
        Ok(json!({ "ran": {
            "verdict": "ran",
            "run": if named.is_some() { "named" } else { "whole" },
            "checks": checks_json(&checks),
            "tests": test_results(&tests, &logs),
        } }))
    })
}

/// What a measure request asks for: a workflow the project lists, and the inputs given to it, each as its name and its
/// value.
type MeasureAsked<'a> = (&'a config::MeasureWorkflow, Vec<(String, String)>);

/// The workflow a measure request asks for among those the project lists, with its inputs as `name=value` lines taken
/// apart; or why the request cannot run: the project lists no workflow, it does not list this one, or an input is not
/// one the workflow takes, is given twice or is not a `name=value` line.
fn measure_request<'a>(listed: &'a [config::MeasureWorkflow], req: &Value) -> Result<MeasureAsked<'a>, String> {
    if listed.is_empty() {
        return Err(NO_MEASURE.to_string());
    }
    let asked = req["workflow"].as_str().unwrap_or_default().trim();
    let Some(wf) = listed.iter().find(|m| m.workflow == asked) else {
        let names: Vec<&str> = listed.iter().map(|m| m.workflow.as_str()).collect();
        return Err(format!(
            "{asked:?} is not a measurement workflow of this project (ci.measure); they are: {}",
            names.join(", ")
        ));
    };
    let takes = if wf.inputs.is_empty() {
        "it takes no input".to_string()
    } else {
        format!("it takes: {}", wf.inputs.join(", "))
    };
    let mut inputs: Vec<(String, String)> = Vec::new();
    for line in req["inputs"].as_str().unwrap_or_default().lines() {
        if line.trim().is_empty() {
            continue;
        }
        let Some((name, value)) = line.split_once('=') else {
            return Err(format!(
                "The input line {line:?} is not name=value; give one input on each line, and a value on one line"
            ));
        };
        let name = name.trim();
        if !wf.inputs.iter().any(|i| i == name) {
            return Err(format!("{name:?} is not an input of {}; {takes}", wf.workflow));
        }
        if inputs.iter().any(|(n, _)| n == name) {
            return Err(format!("The input {name} is given twice"));
        }
        inputs.push((name.to_string(), value.trim().to_string()));
    }
    Ok((wf, inputs))
}

/// Writes a file, and the directories above it.
fn save_file(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, bytes)
}

/// What a finished measurement run left, saved in a directory under the data directory, where the engineer's tools read
/// it: the log of each of its jobs, and, when the workflow names an artifact it uploads, the files of that. Gives the
/// paths saved, and what could not be fetched or written.
fn keep_measurement(
    github: &dyn hub::GitHub,
    dir: &Path,
    repo: &str,
    artifact: Option<&str>,
    checks: &[CheckRun],
) -> (Vec<String>, Vec<String>) {
    let (mut kept, mut missed) = (Vec::new(), Vec::new());
    for (i, c) in checks.iter().enumerate() {
        let name: String = c
            .name
            .chars()
            .map(|x| if x.is_ascii_alphanumeric() { x } else { '-' })
            .collect();
        let path = dir.join(format!("job-{}-{name}.log", i + 1));
        match github.job_log(repo, c) {
            Ok(log) => match save_file(&path, hub::clean_log(&log).as_bytes()) {
                Ok(()) => kept.push(path.display().to_string()),
                Err(x) => missed.push(format!("the log of {} could not be saved: {x}", c.name)),
            },
            Err(f) => missed.push(format!("the log of {} could not be fetched: {}", c.name, f.what)),
        }
    }
    // Every job of a run belongs to the run whose artifact it is.
    if let (Some(name), Some(c)) = (artifact, checks.first()) {
        match github.artifact(repo, c, name) {
            Ok(Some(files)) => {
                for (rel, bytes) in files {
                    let at = Path::new(&rel);
                    if !at.components().all(|x| matches!(x, std::path::Component::Normal(_))) {
                        missed.push(format!("{rel} of the artifact {name} is not a path under it"));
                        continue;
                    }
                    let path = dir.join(format!("artifact-{name}")).join(at);
                    match save_file(&path, &bytes) {
                        Ok(()) => kept.push(path.display().to_string()),
                        Err(x) => missed.push(format!("{rel} of the artifact {name} could not be saved: {x}")),
                    }
                }
            }
            Ok(None) => missed.push(format!("the run uploaded no artifact {name}")),
            Err(f) => missed.push(format!("the artifact {name} could not be fetched: {}", f.what)),
        }
    }
    (kept, missed)
}

/// Runs a measurement workflow of the project, by hand, on the code the engineer handed in, and gives its logs and its
/// artifact. The code goes to a temporary ref as a commit of its own, so that the run found by the commit is this
/// one's and not an earlier measurement of the same head, and no push of the work's branch is made or needed. A refusal
/// is a result, and every request, refused or not, is a row of tool_runs, which is what the checks and the views of a
/// work's measurements read. A workflow that ends red has run: `ok` says it did not run to the end.
fn measure(
    e: &mut Engine,
    w: &Work,
    project: &config::Project,
    round: RoundId,
    n: usize,
    req: &Value,
    pushed: Option<&str>,
) -> Result<Value, (Failure, String)> {
    let wt = w
        .worktree
        .clone()
        .ok_or_else(|| (Failure::other("the work has no worktree"), String::new()))?;
    let head = req["head"].as_str().unwrap_or_default();
    let asked = measure_request(&project.ci.measure, req).and_then(|asked| {
        match push_refusal(&wt, head, pushed, &project.main) {
            Some(why) => Err(why),
            None => Ok(asked),
        }
    });
    let (wf, inputs) = match asked {
        Ok(asked) => asked,
        Err(why) => {
            return attempts(e, round, "measure", req, |_| {
                Ok(json!({ "push": { "verdict": "refused", "why": why } }))
            });
        }
    };
    let sha = git::resolve(&wt, head).unwrap_or_default();
    let branch = format!("{MEASURE}/{}-{}-{n}", round.work, round.n);
    let refname = format!("refs/heads/{branch}");
    let inputs: Vec<(&str, &str)> = inputs.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    // A try again must not start another measurement, which takes as long as the workflow does: the commit is made
    // once, and once the workflow has been dispatched a try again only waits for its run again.
    let mut commit: Option<String> = None;
    let mut dispatched = false;
    let got = attempts(e, round, "measure", req, |e| {
        let at = match commit.clone() {
            Some(at) => at,
            None => {
                let message = format!("Measurement of {sha} for round {round}, request {n}, at {}", db::now());
                let at = git::commit_over(&wt, &sha, &message).map_err(Failure::other)?;
                commit = Some(at.clone());
                at
            }
        };
        let ran = if dispatched {
            e.github().checks(&repo(project), &at, &wf.workflow)
        } else {
            hub::push(&wt, &at, &refname, true)
                .and_then(|()| e.github().dispatch(&repo(project), &wf.workflow, &branch, &inputs))
                .and_then(|()| {
                    dispatched = true;
                    e.github().checks(&repo(project), &at, &wf.workflow)
                })
        };
        let checks = match ran {
            Ok(checks) => checks,
            // GitHub refused the request itself, such as an input the workflow requires and the request lacks: trying
            // again cannot help, and the engineer, who can change the request, is told what GitHub said.
            Err(f) if f.kind == "rejected" => {
                return Ok(json!({ "push": { "verdict": "refused", "why": format!(
                    "GitHub refused to run {}: {}", wf.workflow, f.what
                ) } }));
            }
            // A run that did not start or did not finish in time is not tried again, which would start another and
            // wait as long again; it is a measurement that did not run to the end, and says why.
            Err(f) if matches!(f.kind, "ci_did_not_start" | "timed_out") => {
                return Ok(json!({ "push": {
                    "verdict": "ran", "head": sha, "run": "", "ok": "not_run", "files": [], "note": f.what,
                } }));
            }
            Err(f) => return Err(f),
        };
        let dir = e.data.join("tool-output").join(format!("{round}-{n}-measure"));
        let (files, missed) = keep_measurement(e.github(), &dir, &repo(project), wf.artifact.as_deref(), &checks);
        let whole = !checks.is_empty() && checks.iter().all(|c| c.result == "green");
        // The link to the run, not to its first job.
        let run = checks
            .first()
            .map(|c| c.run.split("/job/").next().unwrap_or(&c.run))
            .unwrap_or_default();
        let mut push = json!({
            "verdict": "ran",
            "head": sha,
            "run": run,
            "ok": if whole { "ran" } else { "not_run" },
            "files": files,
        });
        if checks.is_empty() {
            // GitHub gives a run no job when it cannot read the workflow file (a syntax error, say): there is no link
            // and no log, and this is all the engineer has to go on.
            push["note"] = Value::String(format!(
                "The run of {} had no job, so it left no log and no artifact; GitHub does this for a workflow file it \
                 cannot read, so look at the workflow file on the head measured",
                wf.workflow
            ));
        } else if !missed.is_empty() {
            push["note"] = Value::String(format!(
                "Not everything the run left could be kept: {}",
                missed.join("; ")
            ));
        }
        Ok(json!({ "push": push }))
    });
    // The run is over, or did not come, or no try will: the ref has served.
    let _ = hub::delete_ref(&wt, &refname);
    got
}

fn gh(
    e: &mut Engine,
    w: &Work,
    project: Option<&config::Project>,
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
    // A work runs gh in its worktree, where gh knows the repository; a work on no item, in the worktree of its
    // project's architect for its graph, made when it is not there.
    let dir = match (&w.worktree, project) {
        (Some(wt), _) => wt.clone(),
        (None, Some(p)) => p.checkout.clone(),
        (None, None) => {
            let architect = w.project.as_deref().unwrap_or_default();
            let dir = crate::architect::worktree(&e.data, &w.graph, architect);
            if dir.join(".git").exists() {
                dir
            } else {
                crate::architect::refresh(e, &w.graph, architect)
                    .map(|(dir, _)| dir)
                    .map_err(|x| (Failure::other(x), String::new()))?
            }
        }
    };
    let (code, out) = attempts(e, round, "gh", req, |e| {
        let (code, out) = e.github().gh(&dir, &args)?;
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

fn query(e: &mut Engine, round: RoundId, n: usize, req: &Value) -> Result<Value, (Failure, String)> {
    let text = req["sql"].as_str().unwrap_or_default().to_string();
    let limits = e.config.config.install.query.clone();
    let path = config::database(&e.data);
    attempts(e, round, "query", req, |e| {
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
            Err(why) => {
                let error = match named_columns(&text, &why) {
                    Some(hint) => format!("{why}. {hint}"),
                    None => why,
                };
                json!({ "ran": { "verdict": "error", "error": error } })
            }
        })
    })
}

/// What a query that names a column or a table there is not is told beside its error, from the index of the tables
/// and views agents read that `format:common` gives them: the columns of each one the query names, and for a table
/// there is not, the name of every one; none for any other error. In the sixth trial agents guessed columns
/// (`work_state.paused_by`, `rounds.round`, `architect_queue.id`) that the index, in a message of 80,000 characters,
/// listed, and each guess cost a round; told the columns, the next round names them right.
fn named_columns(sql: &str, error: &str) -> Option<String> {
    let no_table = error.contains("no such table");
    if !no_table && !error.contains("no such column") {
        return None;
    }
    let index = db::index_of(db::SCHEMA).ok()?;
    let words: std::collections::BTreeSet<String> = sql
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .map(str::to_lowercase)
        .collect();
    let named: Vec<String> = index
        .iter()
        .filter(|(name, _, _)| words.contains(name))
        .map(|(name, columns, _)| format!("{name} ({})", columns.join(", ")))
        .collect();
    let mut said = Vec::new();
    if !named.is_empty() {
        said.push(format!(
            "The columns of the tables and views it names: {}",
            named.join("; ")
        ));
    }
    if no_table {
        let all: Vec<&str> = index.iter().map(|(name, _, _)| name.as_str()).collect();
        said.push(format!("The tables and views there are: {}", all.join(", ")));
    }
    (!said.is_empty()).then(|| said.join(". "))
}

// Cell `T2`.

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
    let got = attempts(e, round, "full_ci", &args, |e| {
        if w.pushed_head.as_deref() != Some(head.as_str()) {
            hub::push(&wt, &head, &format!("refs/heads/{branch}"), false)?;
        }
        if pr.get().is_none() {
            let (title, body) = pr_fallback(item);
            let n = e.github().pull_request(
                &repo(&project),
                &branch,
                &project.main,
                Some((title.as_str(), body.as_str())),
                None,
            )?;
            // Known only once written, as in `ci`.
            record_push(e, w, None, Some(n)).map_err(Failure::other)?;
            pr.set(Some(n));
            // The pull request found open from the branch, when a run that stopped had opened it, keeps its base.
            hub::ensure_base(e.github(), &repo(&project), n, &project.main)?;
        }
        let first = work_full_checks(e, &project, &branch, &head)?;
        Ok(json!({ "first": checks_json(&first) }))
    });
    record_push(e, w, Some(&head), None)?;
    let first: Vec<CheckRun> = match got {
        Ok(v) => serde_json::from_value(v["first"].clone()).unwrap_or_default(),
        Err(x) => return Ok(not_run("result", x)),
    };
    let known = known_red(e, &item.project)?;
    if reds(&counted(&first, &project, &known)).is_empty() {
        // A run that lacks a check a merge needs is red, and running it again does not give it the check.
        let lacking = missing_checks(&first, &project, &known);
        if lacking.is_empty() {
            return Ok(json!({ "result": { "kind": "green", "head": head } }));
        }
        let all: Vec<CheckRun> = first.iter().cloned().chain(lacking).collect();
        return Ok(json!({ "result": { "kind": "red", "checks": checks_json(&all) } }));
    }
    let again = match attempts(e, round, "full_ci", &args, |e| {
        rerun_full(e, &project, &head, &first).map(|cs| checks_json(&cs))
    }) {
        Ok(v) => serde_json::from_value::<Vec<CheckRun>>(v).unwrap_or_default(),
        Err(x) => return Ok(not_run("result", x)),
    };
    // What a rerun does not give is a check the run never had.
    let lacking = missing_checks(&again, &project, &known);
    let kind = if reds(&counted(&again, &project, &known)).is_empty() && lacking.is_empty() {
        "flaky"
    } else {
        "red"
    };
    let all: Vec<CheckRun> = again.into_iter().chain(lacking).collect();
    Ok(json!({ "result": { "kind": kind, "checks": checks_json(&all) } }))
}

// Cell `M2`.

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
    let checked = attempts(e, round, "merge", &args, |e| {
        if w.pushed_head.as_deref() != Some(head.as_str()) {
            hub::push(&wt, &head, &format!("refs/heads/{branch}"), false)?;
        }
        Ok(checks_json(&work_full_checks(e, &project, &branch, &head)?))
    });
    record_push(e, w, Some(&head), None)?;
    let mut checks: Vec<CheckRun> = match checked {
        Ok(v) => serde_json::from_value(v).unwrap_or_default(),
        Err(x) => return Ok(not_run("result", x)),
    };
    let known = known_red(e, &item.project)?;
    if !reds(&counted(&checks, &project, &known)).is_empty() {
        checks = match attempts(e, round, "merge", &args, |e| {
            rerun_full(e, &project, &head, &checks).map(|cs| checks_json(&cs))
        }) {
            Ok(v) => serde_json::from_value(v).unwrap_or_default(),
            Err(x) => return Ok(not_run("result", x)),
        };
    }
    // Red after the rerun, or without a check a merge needs, which a rerun does not give: not merged.
    let lacking = missing_checks(&checks, &project, &known);
    if !reds(&counted(&checks, &project, &known)).is_empty() || !lacking.is_empty() {
        let all: Vec<CheckRun> = checks.into_iter().chain(lacking).collect();
        return Ok(json!({ "result": { "kind": "red", "checks": checks_json(&all) } }));
    }
    // A stop asked until here wins: the merge cannot be undone, and every wait above is behind it. One asked after this
    // line loses: the loop finds the merge when it comes to carry the stop out, and drops it.
    if db::stop_asked(e.db.conn(), w.id).map_err(sql)? {
        return Ok(json!({ "result": { "kind": "stopped" } }));
    }
    // The pull request merges into the project's main, whatever base it was opened with, or does not merge.
    match attempts(e, round, "merge", &args, |e| {
        hub::merge_into(e.github(), &repo(&project), pr, &project.main).map(Value::String)
    }) {
        Ok(Value::String(sha)) => Ok(json!({ "result": { "kind": "merged", "merge_commit": sha } })),
        Ok(other) => Err(format!("the merge gave {other}")),
        Err(x) => Ok(not_run("result", x)),
    }
}

// Cell `L1`.

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
    let merged = crate::inputs::last_return(e, w, g, "M2", "result")?
        .and_then(|r| r["result"]["merge_commit"].as_str().map(str::to_string))
        .ok_or("the work has no merge commit")?;
    let wt = w.worktree.clone().unwrap_or_else(|| project.checkout.clone());
    let args = json!({ "merge_commit": merged });
    let first = match attempts(e, round, "watch_main", &args, |e| {
        full_checks(e, &project, &merged).map(|c| checks_json(&c))
    }) {
        Ok(v) => serde_json::from_value::<Vec<CheckRun>>(v).unwrap_or_default(),
        Err(x) => return Ok(not_run("main", x)),
    };
    let required = |cs: &[CheckRun]| counted(cs, &project, &[]);
    if reds(&required(&first)).is_empty() {
        return Ok(json!({ "main": { "kind": "green" } }));
    }
    let again = match attempts(e, round, "watch_main", &args, |e| {
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
    let _ = git::git(&wt, &["fetch", "-q", "origin", &config::main_refspec(&project.main)]);
    let before = git::resolve(&wt, &format!("{merged}^1")).ok_or("main's commit before the merge is not known here")?;
    let earlier = match attempts(e, round, "watch_main", &json!({ "before": before }), |e| {
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

    // Item #217, R241-41: a ci_before run is recorded with the commit its head resolved to. A head written as `HEAD`
    // names another commit once the engineer commits again, and the check of which test files changed after the run
    // would compare the head with itself and see nothing changed.
    #[test]
    fn a_ci_before_run_is_recorded_with_the_commit_its_head_resolved_to() {
        let req = json!({ "tool": "ci_before", "head": "HEAD", "files": ["t/a.rs"], "tests": ["a"] });
        let sha = "0123456789abcdef0123456789abcdef01234567";
        let recorded = recorded_request(&req, sha);
        assert_eq!(recorded["head"], sha);
        assert_eq!(recorded["files"], req["files"]);
        assert_eq!(recorded["tests"], req["tests"]);
        assert_eq!(req["head"], "HEAD");
    }

    // A pull request the engineer gave no title or description is named by its item's number, and holds none of the
    // item's own text, which is in the owner's language.
    #[test]
    fn a_pull_request_without_the_engineer_s_title_holds_no_text_of_its_item() {
        let item = Item {
            id: 12,
            project: "p".into(),
            title: "TITLE-TEXT".into(),
            problem: "PROBLEM-TEXT".into(),
            why: "WHY-TEXT".into(),
            approach: Some("APPROACH-TEXT".into()),
            deliver: "change".into(),
            level: "l".into(),
            acceptance: None,
            acceptance_round: None,
            cancelled: None,
            imported_from: None,
            imported_branch: None,
            imported_pr: None,
            route: "normal".into(),
        };
        let (title, body) = pr_fallback(&item);
        assert_eq!(title, "Item 12");
        assert!(body.contains("item 12"), "{body}");
        assert!(!format!("{title} {body}").contains("-TEXT"), "{title} {body}");
    }

    // A query that names a column there is not is told the columns of the tables and views it names; one that names a
    // table there is not, every table's and view's name too; any other error, nothing more.
    #[test]
    fn a_query_naming_what_is_not_there_is_told_the_columns() {
        let hint = named_columns(
            "SELECT q.id FROM architect_queue q JOIN works w ON w.project = q.project",
            "no such column: q.id",
        )
        .unwrap();
        assert!(
            hint.starts_with("The columns of the tables and views it names: ")
                && hint.contains("architect_queue (project, matter, kind,")
                && hint.contains("works (id,"),
            "{hint}"
        );
        assert!(!hint.contains("The tables and views there are"), "{hint}");
        let hint = named_columns("SELECT * FROM work_rounds", "no such table: work_rounds").unwrap();
        assert!(
            hint.starts_with("The tables and views there are: ") && hint.contains("rounds"),
            "{hint}"
        );
        assert_eq!(named_columns("SELECT 1", "interrupted"), None);
    }

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

    // Each named test is given what became of it in any job's log: passed with its time, failed with how it ended,
    // or not run when no log shows it, as when its job stopped at an earlier red test; unread when no log could be
    // read.
    #[test]
    fn a_named_test_is_given_its_result_from_any_job_s_log() {
        let first = "    Starting 3 tests across 2 binaries (5 tests skipped)
        PASS [   0.007s] (1/3) engine tools::tests::a_command_line_is_split_as_a_shell_would
        SLOW [> 30.000s] (───────) engine::e2e a_takeover_waits_for_a_free_slot_at_the_head_of_the_queue
        PASS [  35.134s] (2/3) engine::e2e a_takeover_waits_for_a_free_slot_at_the_head_of_the_queue
     TIMEOUT [  60.003s] (3/3) engine::e2e a_work_s_pause_is_kept_across_a_restart
------------
     Summary [  60.010s] 3 tests run: 2 passed, 1 timed out, 5 skipped
     TIMEOUT [  60.003s] (3/3) engine::e2e a_work_s_pause_is_kept_across_a_restart
";
        let second = "        FAIL [   1.250s] (1/1) flow::negative no_such_view\n";
        let named = vec![
            "a_takeover_waits_for_a_free_slot_at_the_head_of_the_queue".to_string(),
            "engine::tools::tests::a_command_line_is_split_as_a_shell_would".to_string(),
            "engine::e2e a_work_s_pause_is_kept_across_a_restart".to_string(),
            "no_such_view".to_string(),
            "missing".to_string(),
        ];
        let results = test_results(&named, &[Ok(first.to_string()), Ok(second.to_string())]);
        assert_eq!(
            results[0],
            json!({ "test": named[0], "result": "passed", "time": "35.134s" })
        );
        assert_eq!(results[1]["time"], "0.007s");
        assert_eq!(
            (&results[2]["result"], &results[2]["time"], &results[2]["ended"]),
            (&json!("failed"), &json!("60.003s"), &json!("TIMEOUT"))
        );
        assert_eq!(results[3]["ended"], "FAIL");
        assert_eq!(results[4], json!({ "test": "missing", "result": "not_run" }));
        let unread = test_results(&named[..1], &[Err("check: HTTP 404".to_string())]);
        assert_eq!(unread[0]["result"], "unread");
        assert_eq!(
            unread[0]["why"],
            "the logs of the run could not be read: check: HTTP 404"
        );
    }

    // Lines copied from a real job log, as gh gives it: the job, the step and the time before each line, and the
    // colour escapes nextest writes, their escape character written "^[" by gh.
    #[test]
    fn a_named_test_is_found_in_a_real_job_log() {
        let log = concat!(
            "test 1 of 3\tSet up job\t\u{feff}2026-10-30T00:54:03.1443351Z Current runner version: '2.338.0'\n",
            "test 1 of 3\tTest\t2026-10-30T00:56:55.6846661Z ^[[32;1m    Starting^[[0m ^[[1m141^[[0m tests across ^[[1m14^[[0m binaries (^[[1m270^[[0m tests ^[[33;1mskipped^[[0m)\n",
            "test 1 of 3\tTest\t2026-10-30T00:57:30.8321137Z ^[[32;1m        PASS^[[0m [   0.007s] ( 41/141) ^[[35;1mengine^[[0m ^[[36mtools::tests^[[0m^[[36m::^[[0m^[[34;1ma_command_line_is_split_as_a_shell_would^[[0m\n",
            "test 1 of 3\tTest\t2026-10-30T00:59:08.9861466Z ^[[33;1m        SLOW^[[0m [> 30.000s] (───────) ^[[35;1mengine::e2e^[[0m ^[[34;1ma_takeover_waits_for_a_free_slot_at_the_head_of_the_queue^[[0m\n",
            "test 1 of 3\tTest\t2026-10-30T00:59:14.1170419Z ^[[32;1m        PASS^[[0m [  35.134s] ( 64/141) ^[[35;1mengine::e2e^[[0m ^[[34;1ma_takeover_waits_for_a_free_slot_at_the_head_of_the_queue^[[0m\n",
        );
        let named = [
            "tools::tests::a_command_line_is_split_as_a_shell_would".to_string(),
            "a_takeover_waits_for_a_free_slot_at_the_head_of_the_queue".to_string(),
        ];
        let results = test_results(&named, &[Ok(log.to_string())]);
        assert_eq!(results[0]["time"], "0.007s");
        assert_eq!(results[1]["time"], "35.134s");
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

    /// GitHub whose runs hold one artifact, of the name given, with the files given, counting the times it is asked for
    /// an artifact; its jobs have a log, but the job `unreadable`'s, and it is asked for nothing else.
    struct OneArtifact {
        name: &'static str,
        files: Vec<hub::ArtifactFile>,
        asked: std::sync::atomic::AtomicUsize,
    }

    impl hub::GitHub for OneArtifact {
        fn pull_request(
            &self,
            _repo: &str,
            _branch: &str,
            _base: &str,
            _title_body: Option<(&str, &str)>,
            _existing: Option<i64>,
        ) -> Result<i64, Failure> {
            unreachable!("no pull request is opened here")
        }

        fn checks(&self, _repo: &str, _sha: &str, _workflow: &str) -> Result<Vec<CheckRun>, Failure> {
            unreachable!("no check runs here")
        }

        fn rerun(&self, _repo: &str, _sha: &str, _workflow: &str) -> Result<Vec<CheckRun>, Failure> {
            unreachable!("no check runs here")
        }

        fn base(&self, _repo: &str, _pr: i64) -> Result<String, Failure> {
            unreachable!("no pull request is read here")
        }

        fn set_base(&self, _repo: &str, _pr: i64, _base: &str) -> Result<(), Failure> {
            unreachable!("no pull request is changed here")
        }

        fn merge(&self, _repo: &str, _pr: i64) -> Result<String, Failure> {
            unreachable!("nothing merges here")
        }

        fn close(&self, _repo: &str, _pr: i64, _comment: &str) -> Result<(), Failure> {
            unreachable!("no pull request is closed here")
        }

        fn gh(&self, _dir: &Path, _args: &[String]) -> Result<(i32, String), Failure> {
            unreachable!("no gh command runs here")
        }

        fn job_log(&self, _repo: &str, check: &CheckRun) -> Result<String, Failure> {
            if check.name == "unreadable" {
                return Err(Failure::other("HTTP 404"));
            }
            // As gh gives a line of a job's log: the job, the step and the time before the text.
            Ok(format!(
                "{}\tRun\t2026-11-03T00:00:00.0000000Z the log of {}\n",
                check.name, check.name
            ))
        }

        fn artifact(
            &self,
            _repo: &str,
            _check: &CheckRun,
            name: &str,
        ) -> Result<Option<Vec<hub::ArtifactFile>>, Failure> {
            self.asked.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok((name == self.name).then(|| self.files.clone()))
        }
    }

    // The files a red run generated that differ from the worktree's are put there, uncommitted, and named with what to
    // do with them; a file as the worktree has it is left alone, and nothing is fetched from a green run or for a
    // project that names no such artifact.
    #[test]
    fn a_red_run_s_generated_files_that_differ_are_put_in_the_worktree() {
        let wt = std::env::temp_dir().join(format!("wf-agents-generated-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&wt);
        std::fs::create_dir_all(wt.join("client/src")).unwrap();
        std::fs::write(wt.join("client/src/api.ts"), "export type A = { a: number, };\n").unwrap();
        std::fs::write(wt.join("same.txt"), "same\n").unwrap();
        let hub = OneArtifact {
            name: "generated",
            files: vec![
                (
                    "client/src/api.ts".to_string(),
                    b"export type A = { a: string, };\n".to_vec(),
                ),
                ("same.txt".to_string(), b"same\n".to_vec()),
            ],
            asked: std::sync::atomic::AtomicUsize::new(0),
        };
        let check = |result: &str| CheckRun {
            name: "lint".to_string(),
            result: result.to_string(),
            run: "https://github.com/o/r/actions/runs/1/job/2".to_string(),
            log_tail: None,
            log_path: None,
        };
        let green = [check("green")];
        let red = [check("green"), check("red")];
        assert_eq!(generated(&hub, "o/r", Some("generated"), &green, &wt), None);
        assert_eq!(generated(&hub, "o/r", None, &red, &wt), None);
        assert_eq!(hub.asked.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert_eq!(generated(&hub, "o/r", Some("other"), &red, &wt), None);
        assert_eq!(
            generated(&hub, "o/r", Some("generated"), &red, &wt),
            Some(json!({ "files": ["client/src/api.ts"], "note": GENERATED }))
        );
        assert_eq!(
            std::fs::read_to_string(wt.join("client/src/api.ts")).unwrap(),
            "export type A = { a: string, };\n"
        );
        // Once they are there, nothing differs.
        assert_eq!(generated(&hub, "o/r", Some("generated"), &red, &wt), None);
        let _ = std::fs::remove_dir_all(&wt);
    }

    fn workflow(name: &str, inputs: &[&str], artifact: Option<&str>) -> config::MeasureWorkflow {
        config::MeasureWorkflow {
            workflow: name.to_string(),
            inputs: inputs.iter().map(|i| i.to_string()).collect(),
            artifact: artifact.map(str::to_string),
        }
    }

    // A measure request runs only a workflow the project lists, with inputs that workflow takes, each once, as
    // `name=value` lines; anything else is refused with what the project does list, and a project that lists none says
    // so and says what to answer instead.
    #[test]
    fn a_measure_request_runs_only_what_the_project_lists() {
        let listed = [
            workflow("measure.yml", &["command", "base"], Some("measurement")),
            workflow("quick.yml", &[], None),
        ];
        let ask = |wf: &str, inputs: &str| measure_request(&listed, &json!({ "workflow": wf, "inputs": inputs }));
        // A value is what follows the first `=`, with its ends trimmed; a blank line is no input.
        let (wf, inputs) = ask("measure.yml", "command = cargo test -- a=b \n\nbase=abc123\n").unwrap();
        assert_eq!(wf.workflow, "measure.yml");
        assert_eq!(
            inputs,
            [
                ("command".to_string(), "cargo test -- a=b".to_string()),
                ("base".to_string(), "abc123".to_string())
            ]
        );
        assert_eq!(ask("quick.yml", "").unwrap().1, []);
        let why = ask("other.yml", "").unwrap_err();
        assert!(
            why.contains("\"other.yml\"") && why.ends_with("they are: measure.yml, quick.yml"),
            "{why}"
        );
        let why = ask("measure.yml", "pairs=3").unwrap_err();
        assert!(
            why.contains("\"pairs\"") && why.ends_with("it takes: command, base"),
            "{why}"
        );
        assert!(ask("quick.yml", "base=x").unwrap_err().ends_with("it takes no input"));
        assert!(
            ask("measure.yml", "base=a\nbase=b")
                .unwrap_err()
                .contains("given twice")
        );
        assert!(ask("measure.yml", "command").unwrap_err().contains("is not name=value"));
        // Nothing listed: the text names the setting and the answer to give.
        let why = measure_request(&[], &json!({ "workflow": "measure.yml", "inputs": "" })).unwrap_err();
        assert!(why.contains("ci.measure") && why.contains("not_measured"), "{why}");
    }

    // What a measurement run left is saved in a directory: the log of each job as a reader wants it, and the files of the
    // artifact the workflow names, each under it; what could not be fetched or is not a path under the artifact is said,
    // and the rest is kept. A workflow naming no artifact asks for none.
    #[test]
    fn a_measurement_run_s_logs_and_artifact_are_kept_and_what_is_missing_is_said() {
        let root = std::env::temp_dir().join(format!("wf-agents-measurement-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let dir = root.join("run");
        let hub = OneArtifact {
            name: "measurement",
            files: vec![
                ("pairs.tsv".to_string(), b"1\t2\n".to_vec()),
                ("nested/runs.tsv".to_string(), b"x\n".to_vec()),
                ("../escape.txt".to_string(), b"no\n".to_vec()),
            ],
            asked: std::sync::atomic::AtomicUsize::new(0),
        };
        let check = |name: &str| CheckRun {
            name: name.to_string(),
            result: "green".to_string(),
            run: "https://github.com/o/r/actions/runs/1/job/2".to_string(),
            log_tail: None,
            log_path: None,
        };
        let checks = [check("measure it"), check("unreadable")];
        let (kept, missed) = keep_measurement(&hub, &dir, "o/r", Some("measurement"), &checks);
        let at = |rel: &str| dir.join(rel).display().to_string();
        assert_eq!(
            kept,
            [
                at("job-1-measure-it.log"),
                at("artifact-measurement/pairs.tsv"),
                at("artifact-measurement/nested/runs.tsv")
            ]
        );
        assert_eq!(
            std::fs::read_to_string(dir.join("job-1-measure-it.log")).unwrap(),
            "the log of measure it"
        );
        assert_eq!(
            std::fs::read(dir.join("artifact-measurement/nested/runs.tsv")).unwrap(),
            b"x\n"
        );
        assert_eq!(missed.len(), 2, "{missed:?}");
        assert!(
            missed[0].contains("unreadable") && missed[0].contains("HTTP 404"),
            "{missed:?}"
        );
        assert!(missed[1].contains("../escape.txt"), "{missed:?}");
        assert!(!root.join("escape.txt").exists() && !dir.join("escape.txt").exists());
        // An artifact the run did not upload is said; a workflow naming none asks for none.
        let (kept, missed) = keep_measurement(&hub, &dir, "o/r", Some("other"), &checks[..1]);
        assert_eq!(kept, [at("job-1-measure-it.log")]);
        assert_eq!(missed, ["the run uploaded no artifact other"]);
        let before = hub.asked.load(std::sync::atomic::Ordering::SeqCst);
        keep_measurement(&hub, &dir, "o/r", None, &checks[..1]);
        assert_eq!(hub.asked.load(std::sync::atomic::Ordering::SeqCst), before);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The tool of that name in the flow, as `flow/flow.json` writes it.
    fn flow_tool(name: &str) -> Value {
        let flow: Value = serde_json::from_str(flow::FLOW).unwrap();
        flow["tools"]
            .as_array()
            .and_then(|ts| ts.iter().find(|t| t["name"] == name))
            .cloned()
            .unwrap_or_else(|| panic!("the flow has no tool {name}"))
    }

    // What agents are told of the gh tool is what the code does: the allow list is the setting `gh.allow`, whose value
    // the engine writes beside it, not a list copied into the flow; and `api` is told as the code refuses it, each flag
    // that makes it write named, no GraphQL, and no log of run view downloaded (the engine downloads none).
    #[test]
    fn the_gh_tool_is_described_as_the_code_runs_it() {
        let gh = flow_tool("gh");
        let (desc, runs) = (gh["desc"].as_str().unwrap(), gh["runs"].as_str().unwrap());
        assert!(desc.contains("`gh.allow`"), "{desc}");
        for copied in [
            "pr view",
            "pr diff",
            "pr checks",
            "pr list",
            "run view",
            "run list",
            "issue view",
            "issue list",
            "repo view",
            "release view",
            "search",
        ] {
            assert!(
                !desc.contains(copied) && !runs.contains(copied),
                "{copied}: {desc} {runs}"
            );
        }
        for flag in API_WRITES {
            assert!(desc.contains(&format!("`{flag}`")), "{flag}: {desc}");
        }
        for gone in ["GraphQL", "--method GET"] {
            assert!(!desc.contains(gone) && !runs.contains(gone), "{gone}: {desc} {runs}");
        }
        assert!(!runs.contains("download"), "{runs}");
        // The flags are those the code refuses.
        let allow = ["api repos/".to_string()];
        for flag in API_WRITES {
            let args: Vec<String> = ["api", "repos/o/r/pulls", flag, "x"]
                .iter()
                .map(|a| a.to_string())
                .collect();
            assert!(
                gh_refusal(&allow, &args).is_some_and(|why| why.contains(flag)),
                "{flag}"
            );
        }
    }

    // What agents are told of the measure tool is what the code does: the workflow is one the project's configuration
    // lists, the setting `project.ci.measure`, whose value the engine writes beside it; the tool does not wait for a
    // machine that measures one run at a time, and keeps its files as a list.
    #[test]
    fn the_measure_tool_is_described_as_the_code_runs_it() {
        let measure = flow_tool("measure");
        let in_ = |name: &str| {
            measure["in"]
                .as_array()
                .and_then(|fs| fs.iter().find(|f| f["name"] == name))
                .cloned()
                .unwrap_or_else(|| panic!("measure takes no {name}"))
        };
        let text = format!("{} {} {}", measure["desc"], measure["runs"], in_("workflow")["desc"]);
        assert!(text.contains("`project.ci.measure`"), "{text}");
        assert!(!text.contains("one run at a time"), "{text}");
        assert!(in_("inputs")["desc"].as_str().unwrap().contains("name=value"));
        let ran = measure["out"][0]["branches"]
            .as_array()
            .and_then(|bs| bs.iter().find(|b| b["value"] == "ran"))
            .unwrap();
        let files = ran["fields"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["name"] == "files")
            .unwrap();
        assert_eq!(files["type"], "list");
        // The check of an answer of measured looks at the whole work's runs, as the code does, not at one cell's.
        let flow: Value = serde_json::from_str(flow::FLOW).unwrap();
        let check = flow["engine"]["checks"]["measure_matches_runs"].as_str().unwrap();
        assert!(
            check.contains("a run of this work's measure tool") && !check.contains("in this cell"),
            "{check}"
        );
    }

    // A request of api only reads: a write flag is refused whichever way gh takes it, apart from its value
    // (`-X POST`, `-XPOST`), joined to it (`-fname=value`, `--method=PATCH`) or in a group of short flags (`-iXPOST`);
    // a read, with the flags that only shape the output, runs.
    #[test]
    fn a_write_flag_of_api_is_refused_however_it_is_given() {
        let allow = ["api repos/".to_string()];
        let refusal = |line: &str| gh_refusal(&allow, &words(line).unwrap());
        for write in [
            "api repos/o/r/issues -X POST",
            "api repos/o/r/issues -XPOST",
            "api repos/o/r/issues -XDELETE",
            "api repos/o/r/issues -fname=value",
            "api repos/o/r/issues -Fa=b",
            "api repos/o/r/issues -iXPOST",
            "api repos/o/r/issues --method=PATCH",
            "api repos/o/r/issues --field a=b",
            "api repos/o/r/issues --raw-field=a=b",
            "api repos/o/r/issues --input=body.json",
        ] {
            assert!(
                refusal(write).is_some_and(|why| why.contains("may only read")),
                "{write}"
            );
        }
        for read in [
            "api repos/o/r/pulls/1",
            "api repos/o/r/pulls/1 -i",
            "api repos/o/r/pulls --paginate -q .[].title",
            "api repos/o/r/pulls -q.[].fix -H Accept:text/plain",
            "api repos/o/r/pulls -t {{.title}} --jq=.[]",
        ] {
            assert_eq!(refusal(read), None, "{read}");
        }
    }

    // The full CI and the measurement workflow leave the engine's temporary refs alone on push: a ref the engine pushes
    // to run a workflow on by hand must not start the full CI as well, or the measurement's own push runs. The full CI
    // runs on a push to main only; the measurement workflow ignores the refs by their prefixes.
    #[test]
    fn the_workflows_leave_the_engine_s_temporary_refs_alone_on_push() {
        let ci = include_str!("../../../.github/workflows/ci.yml");
        assert!(
            ci.contains("on:\n  push:\n    branches:\n      - main\n  workflow_dispatch:\n"),
            "ci.yml runs on more than a push to main"
        );
        let measure = include_str!("../../../.github/workflows/measure.yml");
        for prefix in [BEFORE, MEASURE] {
            let ignored = format!("      - '{prefix}/**'\n");
            assert!(measure.contains(&ignored), "measure.yml does not ignore {prefix}/**");
        }
    }
}

// The runs of the fast and the full CI where the project's CI runs on demand, against GitHub in memory: the fast run
// the engine asks for, the full run a merge waits for, and that no head merges on the fast run's word.
#[cfg(test)]
mod ci_on_demand {
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::Hooks;
    use crate::agent::{Agents, Next, Reply, Request, Seen};

    /// The commit the pure tests run on.
    const HEAD: &str = "c0ffee";

    /// One run of a workflow on a commit, with the checks it holds.
    struct Run {
        id: i64,
        sha: String,
        workflow: String,
        checks: Vec<CheckRun>,
    }

    /// What a workflow run by hand holds: the name of each check and its result.
    type Made = Vec<(String, String)>;

    /// A workflow run by hand: the workflow, the branch and the inputs.
    type Dispatch = (String, String, Vec<(String, String)>);

    #[derive(Default)]
    struct State {
        /// The runs that can be seen now.
        seen: Vec<Run>,
        /// The runs made by hand that are not to be seen yet: GitHub shows one a moment after it was asked for, so one
        /// of these comes into sight each time a wait finds no run it takes.
        coming: Vec<Run>,
        /// What a run of a workflow made by hand holds.
        makes: BTreeMap<String, Made>,
        /// What a run of a workflow that a push makes holds: a commit with no run of it gets one when it is looked for,
        /// as a push to a branch the workflow runs on does.
        on_push: BTreeMap<String, Made>,
        /// The log of every job, where the tests ended are read from.
        log: Option<String>,
        /// What GitHub says in refusing every request to run a workflow by hand, when it does.
        rejects: Option<String>,
        /// The head of a branch, where no repository tells.
        heads: BTreeMap<String, String>,
        dispatched: Vec<Dispatch>,
        reruns: usize,
        merges: usize,
        /// The number of the last run made.
        made: i64,
    }

    /// GitHub in memory. Runs are numbered in the order they are made, and a wait takes the newest run it sees above
    /// the number it is given, so that a wait that is given none takes the run that was there before.
    #[derive(Clone)]
    struct Runs {
        state: Arc<Mutex<State>>,
        /// The repository that tells the head of a branch; none, and the test says.
        origin: Option<PathBuf>,
    }

    fn checks_of(workflow: &str, id: i64, checks: &[(String, String)]) -> Vec<CheckRun> {
        checks
            .iter()
            .map(|(name, result)| CheckRun {
                name: name.clone(),
                result: result.clone(),
                run: format!("https://example.invalid/{workflow}/runs/{id}"),
                log_tail: None,
                log_path: None,
            })
            .collect()
    }

    fn made(checks: &[(&str, &str)]) -> Made {
        checks.iter().map(|(n, r)| (n.to_string(), r.to_string())).collect()
    }

    impl Runs {
        fn new(origin: Option<PathBuf>) -> Runs {
            Runs {
                state: Arc::default(),
                origin,
            }
        }

        fn state(&self) -> std::sync::MutexGuard<'_, State> {
            self.state.lock().unwrap()
        }

        /// A run there is already on a commit, such as one a push made.
        fn has_run(&self, sha: &str, workflow: &str, checks: &[(&str, &str)]) {
            let mut s = self.state();
            s.made += 1;
            let id = s.made;
            let checks = checks_of(workflow, id, &made(checks));
            s.seen.push(Run {
                id,
                sha: sha.to_string(),
                workflow: workflow.to_string(),
                checks,
            });
        }

        /// What a run of the workflow made by hand holds from now on.
        fn makes(&self, workflow: &str, checks: &[(&str, &str)]) {
            self.state().makes.insert(workflow.to_string(), made(checks));
        }

        /// What a run of the workflow that a push makes holds from now on.
        fn on_push(&self, workflow: &str, checks: &[(&str, &str)]) {
            self.state().on_push.insert(workflow.to_string(), made(checks));
        }

        /// The log every job of a run has.
        fn logs(&self, log: &str) {
            self.state().log = Some(log.to_string());
        }

        /// From now on GitHub refuses every request to run a workflow by hand with these words; none, it accepts them.
        fn rejects(&self, why: Option<&str>) {
            self.state().rejects = why.map(str::to_string);
        }

        fn at(&self, branch: &str, sha: &str) {
            self.state().heads.insert(branch.to_string(), sha.to_string());
        }

        fn head_of(&self, branch: &str) -> String {
            match &self.origin {
                Some(origin) => git::git(origin, &["rev-parse", &format!("refs/heads/{branch}")])
                    .unwrap()
                    .trim()
                    .to_string(),
                None => self.state().heads[branch].clone(),
            }
        }

        /// No run, and nothing done, as at the start.
        fn clear(&self) {
            let mut s = self.state();
            s.seen.clear();
            s.coming.clear();
            s.dispatched.clear();
            s.reruns = 0;
            s.merges = 0;
        }

        fn dispatched(&self) -> Vec<Dispatch> {
            self.state().dispatched.clone()
        }

        fn merges(&self) -> usize {
            self.state().merges
        }
    }

    impl hub::GitHub for Runs {
        fn pull_request(
            &self,
            _repo: &str,
            _branch: &str,
            _base: &str,
            _title_body: Option<(&str, &str)>,
            _existing: Option<i64>,
        ) -> Result<i64, Failure> {
            Ok(1)
        }

        fn checks(&self, repo: &str, sha: &str, workflow: &str) -> Result<Vec<CheckRun>, Failure> {
            self.checks_after(repo, sha, workflow, None)
        }

        fn checks_after(
            &self,
            _repo: &str,
            sha: &str,
            workflow: &str,
            after: Option<i64>,
        ) -> Result<Vec<CheckRun>, Failure> {
            let mut s = self.state();
            loop {
                let newest = s
                    .seen
                    .iter()
                    .filter(|r| r.sha == sha && r.workflow == workflow && after.is_none_or(|a| r.id > a))
                    .max_by_key(|r| r.id);
                if let Some(r) = newest {
                    return Ok(r.checks.clone());
                }
                if s.coming.is_empty() && after.is_none() && s.on_push.contains_key(workflow) {
                    s.made += 1;
                    let id = s.made;
                    let checks = checks_of(workflow, id, &s.on_push[workflow]);
                    s.seen.push(Run {
                        id,
                        sha: sha.to_string(),
                        workflow: workflow.to_string(),
                        checks,
                    });
                    continue;
                }
                if s.coming.is_empty() {
                    return Err(Failure {
                        kind: "ci_did_not_start",
                        what: format!("no run of {workflow} on {sha}"),
                    });
                }
                let next = s.coming.remove(0);
                s.seen.push(next);
            }
        }

        fn job_log(&self, _repo: &str, _check: &CheckRun) -> Result<String, Failure> {
            self.state()
                .log
                .clone()
                .ok_or_else(|| Failure::other("no job logs here"))
        }

        fn latest_run_id(&self, _repo: &str, sha: &str, workflow: &str) -> Result<Option<i64>, Failure> {
            let s = self.state();
            Ok(s.seen
                .iter()
                .filter(|r| r.sha == sha && r.workflow == workflow)
                .map(|r| r.id)
                .max())
        }

        fn dispatch(&self, _repo: &str, workflow: &str, branch: &str, inputs: &[(&str, &str)]) -> Result<(), Failure> {
            let sha = self.head_of(branch);
            let mut s = self.state();
            if let Some(why) = s.rejects.clone() {
                // The request was made, and refused.
                let inputs = inputs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
                s.dispatched.push((workflow.to_string(), branch.to_string(), inputs));
                return Err(Failure::rejected(why));
            }
            let Some(holds) = s.makes.get(workflow).cloned() else {
                return Err(Failure::network(format!(
                    "could not find any workflows named {workflow}"
                )));
            };
            s.made += 1;
            let id = s.made;
            s.coming.push(Run {
                id,
                sha,
                workflow: workflow.to_string(),
                checks: checks_of(workflow, id, &holds),
            });
            let inputs = inputs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
            s.dispatched.push((workflow.to_string(), branch.to_string(), inputs));
            Ok(())
        }

        fn rerun(&self, repo: &str, sha: &str, workflow: &str) -> Result<Vec<CheckRun>, Failure> {
            self.state().reruns += 1;
            self.checks(repo, sha, workflow)
        }

        fn base(&self, _repo: &str, _pr: i64) -> Result<String, Failure> {
            Ok("main".to_string())
        }

        fn set_base(&self, _repo: &str, _pr: i64, _base: &str) -> Result<(), Failure> {
            Ok(())
        }

        fn merge(&self, _repo: &str, _pr: i64) -> Result<String, Failure> {
            self.state().merges += 1;
            Ok("0123abc".to_string())
        }

        fn close(&self, _repo: &str, _pr: i64, _comment: &str) -> Result<(), Failure> {
            Ok(())
        }

        fn gh(&self, _dir: &Path, _args: &[String]) -> Result<(i32, String), Failure> {
            unreachable!("no gh command runs here")
        }
    }

    /// A project whose fast workflow is `fast` and whose full CI is the workflow `ci` with the check `check`.
    fn project(on_demand: bool, tests_input: Option<&str>) -> config::Project {
        serde_json::from_value(json!({
            "repo": "o/p", "checkout": "/checkout", "design_goal": "simple", "test_paths": ["tests/"],
            "ci": {
                "fast": "fast", "full": ["ci"], "merge_checks": ["check"],
                "tests_input": tests_input, "on_demand": on_demand,
            }
        }))
        .unwrap()
    }

    fn tests(names: &[&str]) -> Vec<String> {
        names.iter().map(|n| n.to_string()).collect()
    }

    fn results(checks: &[CheckRun]) -> Vec<(&str, &str)> {
        checks.iter().map(|c| (c.name.as_str(), c.result.as_str())).collect()
    }

    // The fast CI is the run the dispatch made: it asks for exactly the tests named, as the input names them, on the
    // work's branch, and takes the run numbered above the newest there was, never one there was before, whether that
    // one is red or green.
    #[test]
    fn the_fast_ci_is_the_run_the_dispatch_made_and_never_one_there_was() {
        let p = project(true, Some("tests"));
        for (before, now) in [("red", "green"), ("green", "red")] {
            let runs = Runs::new(None);
            runs.at("work", HEAD);
            runs.has_run(HEAD, "fast", &[("lint", before), ("fast", before)]);
            runs.makes("fast", &[("lint", now), ("fast", now)]);
            let named = tests(&["a_test", "engine::another"]);
            let (run, checks) = fast_checks(&runs, &p, "work", HEAD, &named).unwrap();
            assert_eq!(run, "named_and_lint");
            assert_eq!(
                results(&checks),
                [("lint", now), ("fast", now)],
                "after a run that was {before}"
            );
            let inputs = vec![("tests".to_string(), "a_test engine::another".to_string())];
            assert_eq!(runs.dispatched(), [("fast".to_string(), "work".to_string(), inputs)]);
        }
    }

    // With no test named it is the lint only, asked for with an empty input; with no run before, any run is the one.
    #[test]
    fn the_fast_ci_with_no_test_named_is_the_lint_only() {
        let runs = Runs::new(None);
        runs.at("work", HEAD);
        runs.makes("fast", &[("lint", "green"), ("fast", "green")]);
        let (run, checks) = fast_checks(&runs, &project(true, Some("tests")), "work", HEAD, &[]).unwrap();
        assert_eq!(run, "lint");
        assert_eq!(results(&checks), [("lint", "green"), ("fast", "green")]);
        let inputs = vec![("tests".to_string(), String::new())];
        assert_eq!(runs.dispatched(), [("fast".to_string(), "work".to_string(), inputs)]);
    }

    // Where the project's CI does not run on demand, the push made the run: nothing is run by hand, and the run is
    // the whole of the fast CI.
    #[test]
    fn the_fast_ci_not_on_demand_is_the_run_of_the_push() {
        let runs = Runs::new(None);
        runs.has_run(HEAD, "fast", &[("lint", "green"), ("test 1 of 3", "red")]);
        for input in [None, Some("tests")] {
            let (run, checks) = fast_checks(&runs, &project(false, input), "work", HEAD, &tests(&["a_test"])).unwrap();
            assert_eq!(run, "whole");
            assert_eq!(results(&checks), [("lint", "green"), ("test 1 of 3", "red")]);
        }
        assert!(runs.dispatched().is_empty());
    }

    // A workflow the branch does not have fails to run, and the failure says what to do about a branch made before the
    // workflow existed.
    #[test]
    fn a_workflow_the_branch_lacks_fails_saying_to_merge_main_in() {
        let runs = Runs::new(None);
        runs.at("work", HEAD);
        let f = fast_checks(&runs, &project(true, Some("tests")), "work", HEAD, &tests(&["a"])).unwrap_err();
        assert_eq!(f.kind, "network_or_api");
        assert!(f.what.contains("could not find any workflows named fast"), "{}", f.what);
        assert!(f.what.contains("merge origin/main into the branch"), "{}", f.what);
        // A request GitHub refused is told as it is: the workflow is there, and the hint would mislead.
        let refused = dispatch_failed(Failure::rejected("Unexpected inputs provided: [\"tests\"]"));
        assert_eq!(refused, Failure::rejected("Unexpected inputs provided: [\"tests\"]"));
    }

    // A head that only the fast CI has run is not run in full by it: the full CI is run by hand, and its checks are what
    // is read, red as well as green.
    #[test]
    fn a_head_with_only_a_fast_run_has_the_full_ci_run_by_hand() {
        let p = project(true, Some("tests"));
        for now in ["red", "green"] {
            let runs = Runs::new(None);
            runs.at("work", HEAD);
            runs.has_run(HEAD, "fast", &[("lint", "green"), ("fast", "green")]);
            runs.makes("ci", &[("lint", "green"), ("check", now)]);
            let checks = full_checks_on(&runs, &p, HEAD, Some("work")).unwrap();
            assert_eq!(results(&checks), [("lint", "green"), ("check", now)]);
            assert_eq!(runs.dispatched(), [("ci".to_string(), "work".to_string(), vec![])]);
        }
    }

    // A head with a run of the full CI is read, not run again; a run of the commit before it is not the head's.
    #[test]
    fn a_head_with_a_full_run_is_read_and_another_commit_s_run_is_not() {
        let p = project(true, Some("tests"));
        let runs = Runs::new(None);
        runs.at("work", HEAD);
        runs.has_run(HEAD, "ci", &[("check", "green")]);
        runs.has_run("older", "ci", &[("check", "red")]);
        let checks = full_checks_on(&runs, &p, HEAD, Some("work")).unwrap();
        assert_eq!(results(&checks), [("check", "green")]);
        assert!(runs.dispatched().is_empty());
        // The head moved on, as when a catch-up with main made a new commit: it has no run, so one is made.
        runs.at("work", "newer");
        runs.makes("ci", &[("check", "green")]);
        full_checks_on(&runs, &p, "newer", Some("work")).unwrap();
        assert_eq!(runs.dispatched(), [("ci".to_string(), "work".to_string(), vec![])]);
    }

    // Where the project's CI does not run on demand, the full CI is never run by hand: a commit with no run waits for
    // the one its push should have made.
    #[test]
    fn the_full_ci_not_on_demand_is_only_waited_for() {
        let runs = Runs::new(None);
        runs.at("work", HEAD);
        runs.makes("ci", &[("check", "green")]);
        let failed = full_checks_on(&runs, &project(false, None), HEAD, Some("work")).unwrap_err();
        assert_eq!(failed.kind, "ci_did_not_start");
        assert!(runs.dispatched().is_empty());
        runs.has_run(HEAD, "ci", &[("check", "red")]);
        let checks = full_checks_on(&runs, &project(false, None), HEAD, Some("work")).unwrap();
        assert_eq!(results(&checks), [("check", "red")]);
        assert!(runs.dispatched().is_empty());
    }

    // A run without a check merging needs is not the full CI: it is red for that check, where the CI runs on demand;
    // else nothing changes. A check the known red table excuses is not asked for.
    #[test]
    fn a_run_without_a_check_merging_needs_is_red_for_it() {
        let have = [CheckRun {
            name: "lint".to_string(),
            result: "green".to_string(),
            run: String::new(),
            log_tail: None,
            log_path: None,
        }];
        let lacking = missing_checks(&have, &project(true, Some("tests")), &[]);
        assert_eq!(results(&lacking), [("check", "red")]);
        assert!(lacking[0].log_tail.as_deref().unwrap().contains("no check named check"));
        assert!(missing_checks(&have, &project(false, None), &[]).is_empty());
        assert!(missing_checks(&have, &project(true, Some("tests")), &["check".to_string()]).is_empty());
        let has = [CheckRun {
            name: "check".to_string(),
            ..have[0].clone()
        }];
        assert!(missing_checks(&has, &project(true, Some("tests")), &[]).is_empty());
    }

    struct NoAgents;

    impl Agents for NoAgents {
        fn run(&self, _r: &Request, _watch: &dyn Fn(Seen) -> Next) -> Reply {
            unreachable!("no agent runs here")
        }
    }

    /// An engine over a repository, with a work at the merge: its worktree on the branch `work`, one commit past main,
    /// pushed, with its pull request; and GitHub in memory, where the project's CI runs on demand.
    struct Rig {
        e: Engine,
        runs: Runs,
        root: PathBuf,
        origin: PathBuf,
        wt: PathBuf,
        head: String,
        item: i64,
        work: i64,
    }

    impl Drop for Rig {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn commit(dir: &Path, message: &str) -> String {
        git::git(dir, &["add", "-A"]).unwrap();
        let identity = ["-c", "user.name=t", "-c", "user.email=t@t"];
        let args = [&identity[..], &["commit", "-q", "-m", message]].concat();
        git::git(dir, &args).unwrap();
        git::git(dir, &["rev-parse", "HEAD"]).unwrap().trim().to_string()
    }

    /// The project's `[ci]` where its CI runs on demand.
    const ON_DEMAND: &str = "fast = \"fast\"\nfull = [\"ci\"]\nmerge_checks = [\"check\"]\ntests_input = \"tests\"\n\
                             on_demand = true\n";

    fn rig(name: &str) -> Rig {
        rig_with(name, ON_DEMAND)
    }

    /// A rig whose project's `[ci]` table holds `ci`.
    fn rig_with(name: &str, ci: &str) -> Rig {
        let root = std::env::temp_dir().join(format!("wf-agents-ci-on-demand-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let (origin, wt, data) = (root.join("origin.git"), root.join("worktree"), root.join("data"));
        let path = |p: &Path| p.to_string_lossy().into_owned();
        git::git(&root, &["init", "-q", "--bare", &path(&origin)]).unwrap();
        git::git(&root, &["clone", "-q", &path(&origin), &path(&wt)]).unwrap();
        std::fs::write(wt.join("a.txt"), "main\n").unwrap();
        commit(&wt, "start");
        git::git(&wt, &["push", "-q", "origin", "HEAD:main"]).unwrap();
        git::git(&wt, &["checkout", "-q", "-b", "work"]).unwrap();
        std::fs::write(wt.join("a.txt"), "main\nwork\n").unwrap();
        let head = commit(&wt, "work");
        git::git(&wt, &["push", "-q", "origin", "work"]).unwrap();
        std::fs::create_dir_all(data.join("projects")).unwrap();
        std::fs::write(
            data.join("install.toml"),
            r#"
[owner]
language = "English"
[items]
title_max = 40
[machines]
workstation = "here"
heavy = "builds"
poll_interval = 60
[machines.hosts.here]
slots = 1
[agents.clis.claude]
kind = "claude"
command = "claude"
[agents.levels.l]
desc = "all work"
cli = "claude"
model = "m"
[gh]
allow = ["pr view"]
[query]
max_rows = 10
timeout_ms = 1000
"#,
        )
        .unwrap();
        std::fs::write(
            data.join("projects/p.toml"),
            format!(
                "repo = \"o/p\"\ncheckout = \"{}\"\ndesign_goal = \"simple\"\ntest_paths = [\"tests/\"]\n[ci]\n{ci}",
                wt.display()
            ),
        )
        .unwrap();
        std::fs::write(data.join(".gitignore"), "db/\nworktrees/\nlogs/\n").unwrap();
        git::git(&data, &["init", "-q"]).unwrap();
        commit(&data, "configuration");
        crate::database::create(&data).unwrap();
        let runs = Runs::new(Some(origin.clone()));
        let committed = config::committed(&data).unwrap();
        let mut e = Engine::new(
            &data,
            committed,
            Box::new(NoAgents),
            Box::new(runs.clone()),
            Hooks::default(),
        )
        .unwrap();
        e.retry_wait = Duration::ZERO;
        let flow_now = e.flow_now;
        let worktree = path(&wt);
        let (item, work) =
            e.db.write(Actor::Engine, |tx| {
                tx.execute(
                    "INSERT INTO items (project, title, problem, why, deliver, level)
                     VALUES ('p', 'Add', 'x', 'y', 'change', 'l')",
                    [],
                )?;
                let item = tx.last_insert_rowid();
                tx.execute(
                    "INSERT INTO works (item, flow_version, graph, at, worktree, branch, pr, pushed_head)
                     VALUES (?1, ?2, 'engineer', 'M2', ?3, 'work', 1, ?4)",
                    params![item, flow_now, worktree, head],
                )?;
                let work = tx.last_insert_rowid();
                tx.execute(
                    "INSERT INTO rounds (work, n, cell, input, config_commit, started_at)
                     VALUES (?1, 1, 'M2', '{}', 'c', 1)",
                    [work],
                )?;
                Ok((item, work))
            })
            .unwrap();
        Rig {
            e,
            runs,
            root,
            origin,
            wt,
            head,
            item,
            work,
        }
    }

    impl Rig {
        fn run(&mut self, tool: fn(&mut Engine, &Work, &Item, RoundId) -> Result<Value, String>) -> Value {
            let w = Work::load(self.e.db.conn(), self.work, None).unwrap();
            let item = Item::load(self.e.db.conn(), self.item).unwrap();
            let round = RoundId { work: self.work, n: 1 };
            tool(&mut self.e, &w, &item, round).unwrap()
        }

        /// The tool `ci` asked for the head with the tests named.
        fn ci(&mut self, tests: &[&str]) -> Value {
            let w = Work::load(self.e.db.conn(), self.work, None).unwrap();
            let item = Item::load(self.e.db.conn(), self.item).unwrap();
            let project = self.e.project_of(&w).unwrap();
            let round = RoundId { work: self.work, n: 1 };
            let req = json!({ "tool": "ci", "head": self.head, "tests": tests });
            let (mut pushed, pr) = (w.pushed_head.clone(), Cell::new(w.pr));
            ci(&mut self.e, &w, &project, &item, round, &req, &mut pushed, &pr, &|_| {}).unwrap()
        }

        /// The tool `ci_before` asked to run the tests on the code before the change.
        fn ci_before(&mut self, tests: &[&str]) -> Value {
            let w = Work::load(self.e.db.conn(), self.work, None).unwrap();
            let project = self.e.project_of(&w).unwrap();
            let round = RoundId { work: self.work, n: 1 };
            let req = json!({ "tool": "ci_before", "head": self.head, "files": [], "tests": tests });
            ci_before(
                &mut self.e,
                &w,
                &project,
                round,
                0,
                &req,
                Some(self.head.as_str()),
                None,
            )
            .unwrap()
        }
    }

    // A head not yet pushed when the round began counts once ci tells it was pushed, and is refused once ci has pushed
    // all it will without covering it.
    #[test]
    fn ci_before_waits_for_the_head_ci_pushes() {
        let dir = std::env::temp_dir().join(format!("wf-agents-covering-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let run = |args: &[&str]| crate::git::git(&dir, args).unwrap();
        run(&["init", "-q"]);
        let who = ["-c", "user.name=t", "-c", "user.email=t@t"];
        run(&[&who[..], &["commit", "-q", "--allow-empty", "-m", "a"]].concat());
        let a = run(&["rev-parse", "HEAD"]).trim().to_string();
        run(&[&who[..], &["commit", "-q", "--allow-empty", "-m", "b"]].concat());
        let b = run(&["rev-parse", "HEAD"]).trim().to_string();
        // Pushed already when the round began: no wait.
        assert!(pushed_covering(&dir, &a, Some(&b), None));
        // Told by ci, from another thread, after the start: first a head that does not cover it, then one that does.
        let (told, hears) = mpsc::channel();
        let pushes = [a.clone(), b.clone()];
        let teller = std::thread::spawn(move || pushes.into_iter().for_each(|p| told.send(p).unwrap()));
        assert!(pushed_covering(&dir, &b, None, Some(&hears)));
        teller.join().unwrap();
        // ci pushed nothing that covers it, and is done: refused, not waited for forever.
        let (told, hears) = mpsc::channel();
        told.send(a.clone()).unwrap();
        drop(told);
        assert!(!pushed_covering(&dir, &b, Some(&a), Some(&hears)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    // The tool ci asks for the fast CI on the head with the tests named, and gives the run it made: its head and its
    // checks as before, and what the run was, the tests named and the lint; the red run there was before is not it.
    fn the_tool_ci_gives_the_fast_run_it_made_and_what_it_ran(r: &mut Rig) {
        r.runs.clear();
        r.runs.has_run(&r.head, "fast", &[("lint", "red"), ("fast", "red")]);
        r.runs.makes("fast", &[("lint", "green"), ("fast", "green")]);
        let v = r.ci(&["a_test", "engine::b"]);
        assert_eq!(v["push"]["verdict"], "pushed", "{v}");
        assert_eq!(v["push"]["head"], r.head.as_str());
        assert_eq!(v["push"]["run"], "named_and_lint");
        let checks = v["push"]["checks"].as_array().unwrap();
        assert!(checks.iter().all(|c| c["result"] == "green"), "{v}");
        let inputs = vec![("tests".to_string(), "a_test engine::b".to_string())];
        assert_eq!(r.runs.dispatched(), [("fast".to_string(), "work".to_string(), inputs)]);
        // Naming no test, it is the lint only.
        r.runs.clear();
        let v = r.ci(&[]);
        assert_eq!(v["push"]["run"], "lint", "{v}");
    }

    /// The project's `[ci]` where the fast CI takes no tests to run on their own.
    const NO_TESTS_INPUT: &str = "fast = \"ci\"\nfull = [\"ci\"]\nmerge_checks = [\"check\"]\n";

    // A project whose fast CI takes no tests to run and says it does not run on the temporary branches cannot have the
    // tests run on the code before the change: the tool refuses, saying so, before it pushes anything or runs anything.
    #[test]
    fn the_tool_ci_before_refuses_a_project_whose_fast_ci_does_not_run_there_and_pushes_nothing() {
        let mut r = rig_with(
            "tool-ci-before-refused",
            &format!("{NO_TESTS_INPUT}runs_on_before_branches = false\n"),
        );
        let v = r.ci_before(&["a_test"]);
        assert_eq!(v["ran"]["verdict"], "refused", "{v}");
        let why = v["ran"]["why"].as_str().unwrap();
        assert!(
            why.contains("runs_on_before_branches") && why.contains("tests_input"),
            "{why}"
        );
        assert!(r.runs.dispatched().is_empty());
        let branches = git::git(&r.origin, &["branch", "--list", "--format=%(refname:short)"]).unwrap();
        assert!(!branches.contains(BEFORE), "something was pushed:\n{branches}");
    }

    // A project whose fast CI takes no tests to run and does not say it stays off the temporary branches has the
    // tests run there as it always had: the push is the run, nothing is run by hand, and each test named is read from
    // the logs of the whole run, which says what ran.
    #[test]
    fn the_tool_ci_before_waits_for_the_push_run_of_a_project_that_takes_no_tests_input() {
        let mut r = rig_with("tool-ci-before-whole", NO_TESTS_INPUT);
        r.runs.on_push("ci", &[("test 1 of 1", "red")]);
        r.runs.logs(
            "        PASS [   0.007s] (1/2) engine tools::tests::a_test\n\
                     FAIL [   0.010s] (2/2) engine tools::tests::b_test\n",
        );
        let v = r.ci_before(&["a_test", "b_test", "c_test"]);
        assert_eq!(v["ran"]["verdict"], "ran", "{v}");
        assert_eq!(v["ran"]["run"], "whole");
        assert!(r.runs.dispatched().is_empty(), "nothing is run by hand");
        let got: Vec<(&str, &str)> = v["ran"]["tests"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| (t["test"].as_str().unwrap(), t["result"].as_str().unwrap()))
            .collect();
        assert_eq!(got, [("a_test", "passed"), ("b_test", "failed"), ("c_test", "not_run")]);
        // The temporary branch is gone once the run is read.
        let branches = git::git(&r.origin, &["branch", "--list", "--format=%(refname:short)"]).unwrap();
        assert!(!branches.contains(BEFORE), "the branch was left:\n{branches}");
    }

    // Where no run comes for the push, the failure tells which two settings of the project's configuration to look at.
    #[test]
    fn the_checks_before_the_change_without_a_push_run_name_the_settings_to_change() {
        let runs = Runs::new(None);
        let p = project(false, None);
        let failure = before_checks(&runs, &p, "before/1-1-0", HEAD, None, "a_test").unwrap_err();
        assert_eq!(failure.kind, "ci_did_not_start");
        assert!(
            failure.what.contains("runs_on_before_branches") && failure.what.contains("tests_input"),
            "{}",
            failure.what
        );
        // Any other failure, and a run that came, are as they were.
        runs.on_push("fast", &[("lint", "green")]);
        let checks = before_checks(&runs, &p, "before/1-1-0", HEAD, None, "a_test").unwrap();
        assert_eq!(results(&checks), [("lint", "green")]);
        assert!(runs.dispatched().is_empty());
        let other = no_run_on_before_branch(Failure::network("gh: down".to_string()));
        assert_eq!((other.kind, other.what.as_str()), ("network_or_api", "gh: down"));
    }

    // A project whose fast CI takes the tests to run has them run by hand on the temporary branch, exactly them, and
    // the tool says so; the switch is not read.
    #[test]
    fn the_tool_ci_before_runs_the_named_tests_by_hand_for_a_project_that_takes_a_tests_input() {
        let mut r = rig_with(
            "tool-ci-before-named",
            &format!("{ON_DEMAND}runs_on_before_branches = false\n"),
        );
        r.runs.makes("fast", &[("lint", "green"), ("fast", "green")]);
        let v = r.ci_before(&["a_test"]);
        assert_eq!(v["ran"]["verdict"], "ran", "{v}");
        assert_eq!(v["ran"]["run"], "named");
        let [(workflow, branch, inputs)] = &r.runs.dispatched()[..] else {
            panic!("one run by hand: {:?}", r.runs.dispatched());
        };
        assert_eq!(workflow, "fast");
        assert!(branch.starts_with(BEFORE), "{branch}");
        assert_eq!(inputs, &[("tests".to_string(), "a_test".to_string())]);
    }

    // No head merges on the fast CI's word. With the fast CI green on the head and no run of the full CI, the merge
    // makes the full run, and a red `check`, or a run without a `check`, is not merged, rerun once where it can be; only
    // a green `check` is.
    fn a_head_the_fast_ci_passed_merges_only_on_a_green_full_run(r: &mut Rig) {
        let head = r.head.clone();
        let by_hand = [("ci".to_string(), "work".to_string(), vec![])];
        for (holds, kind, reruns) in [
            (vec![("lint", "green"), ("check", "red")], "red", 1),
            (vec![("lint", "green")], "red", 0),
            (vec![("lint", "green"), ("check", "green")], "merged", 0),
        ] {
            r.runs.clear();
            r.runs.has_run(&head, "fast", &[("lint", "green"), ("fast", "green")]);
            r.runs.makes("ci", &holds);
            let v = r.run(merge);
            assert_eq!(v["result"]["kind"], kind, "full run {holds:?}: {v}");
            assert_eq!(r.runs.merges(), usize::from(kind == "merged"), "full run {holds:?}");
            assert_eq!(r.runs.state().reruns, reruns, "full run {holds:?}");
            assert_eq!(r.runs.dispatched(), by_hand, "full run {holds:?}");
            if holds.len() == 1 {
                let named = v["result"]["checks"].as_array().unwrap();
                assert!(
                    named.iter().any(|c| c["name"] == "check" && c["result"] == "red"),
                    "a check merging needs that the run lacks is told red: {v}"
                );
            }
        }
    }

    // A head the full CI ran on, as when the draft's full CI just did, is merged on that run: it is not run again.
    fn a_head_with_a_full_run_merges_on_it_without_a_second_run(r: &mut Rig) {
        r.runs.clear();
        r.runs.has_run(&r.head, "ci", &[("lint", "green"), ("check", "green")]);
        let v = r.run(merge);
        assert_eq!(v["result"]["kind"], "merged", "{v}");
        assert!(r.runs.dispatched().is_empty());
        assert_eq!(r.runs.merges(), 1);
        // The same run being red, it is not merged.
        r.runs.clear();
        r.runs.has_run(&r.head, "ci", &[("check", "red")]);
        let v = r.run(merge);
        assert_eq!(v["result"]["kind"], "red", "{v}");
        assert_eq!(r.runs.merges(), 0);
        assert!(r.runs.dispatched().is_empty());
    }

    // The head that caught up with main is a new commit: the full run of the head before it says nothing of it, and one
    // is made for it.
    #[test]
    fn a_head_that_caught_up_with_main_is_run_in_full_again() {
        let mut r = rig("merge-caught-up");
        let before = r.head.clone();
        r.runs.has_run(&before, "ci", &[("check", "green")]);
        r.runs.makes("ci", &[("check", "red")]);
        // Main moves on: a commit by another work.
        let other = r.root.join("other");
        git::git(
            &r.root,
            &["clone", "-q", &r.origin.to_string_lossy(), &other.to_string_lossy()],
        )
        .unwrap();
        git::git(&other, &["checkout", "-q", "main"]).unwrap();
        std::fs::write(other.join("b.txt"), "other\n").unwrap();
        commit(&other, "another work");
        git::git(&other, &["push", "-q", "origin", "HEAD:main"]).unwrap();
        let v = r.run(merge);
        let now = git::git(&r.wt, &["rev-parse", "HEAD"]).unwrap().trim().to_string();
        assert_ne!(now, before, "the catch-up made a new head");
        assert_eq!(r.runs.dispatched(), [("ci".to_string(), "work".to_string(), vec![])]);
        assert_eq!(v["result"]["kind"], "red", "{v}");
        assert_eq!(r.runs.merges(), 0);
    }

    // The full CI on the draft makes the run of the full CI the head has none of, whatever the fast CI said, and tells
    // red as red.
    fn the_full_ci_on_the_draft_runs_the_full_workflow_and_not_the_fast_one(r: &mut Rig) {
        let head = r.head.clone();
        for (holds, kind) in [
            (vec![("check", "red")], "red"),
            (vec![("lint", "green")], "red"),
            (vec![("check", "green")], "green"),
        ] {
            r.runs.clear();
            r.runs.has_run(&head, "fast", &[("lint", "green"), ("fast", "green")]);
            r.runs.makes("ci", &holds);
            let v = r.run(full_ci);
            assert_eq!(v["result"]["kind"], kind, "full run {holds:?}: {v}");
            assert_eq!(
                r.runs.dispatched(),
                [("ci".to_string(), "work".to_string(), vec![])],
                "full run {holds:?}"
            );
        }
    }

    // A request GitHub refuses itself, such as an input the workflow does not take, is asked once, never again: `ci`
    // tells the engineer what GitHub said as a refusal; the full CI on the draft and the merge, which have no refusal
    // of their own, as a tool that did not run, of the kind `other` the flow lists, and nothing is merged.
    fn a_request_github_refuses_is_asked_once_and_told(r: &mut Rig) {
        let why = "HTTP 422: Unexpected inputs provided: [\"tests\"]";
        r.runs.clear();
        r.runs.rejects(Some(why));
        let v = r.ci(&["a_test"]);
        assert_eq!(v["push"]["verdict"], "refused", "{v}");
        let said = v["push"]["why"].as_str().unwrap();
        assert!(
            said.contains("GitHub refused to run fast") && said.contains(why),
            "{said}"
        );
        assert_eq!(r.runs.dispatched().len(), 1, "asked again");
        type Tool = fn(&mut Engine, &Work, &Item, RoundId) -> Result<Value, String>;
        let tools: [(Tool, &str); 2] = [(full_ci, "full_ci"), (merge, "merge")];
        for (tool, name) in tools {
            r.runs.clear();
            r.runs.has_run(&r.head, "fast", &[("lint", "green"), ("fast", "green")]);
            let v = r.run(tool);
            assert_eq!(v["result"]["kind"], "not_run", "{name}: {v}");
            assert_eq!(v["result"]["failure"], "other", "{name}: {v}");
            assert!(v["result"]["what"].as_str().unwrap().contains(why), "{name}: {v}");
            assert_eq!(r.runs.dispatched().len(), 1, "{name} asked again");
            assert_eq!(r.runs.merges(), 0, "{name}");
        }
        r.runs.rejects(None);
    }

    // The tools that make and read the runs of the fast and the full CI, each against the GitHub in memory that the
    // case before it left cleared, on one engine: its start (compiling the flow) is most of what each costs, and the
    // cases share nothing else. Each case is a function of its own above, that says what it shows. The case where
    // main moves on is apart: it moves the head, which the others stand on.
    #[test]
    fn the_runs_of_the_fast_and_the_full_ci_are_made_and_read_apart() {
        let mut r = rig("runs-apart");
        the_tool_ci_gives_the_fast_run_it_made_and_what_it_ran(&mut r);
        the_full_ci_on_the_draft_runs_the_full_workflow_and_not_the_fast_one(&mut r);
        a_head_the_fast_ci_passed_merges_only_on_a_green_full_run(&mut r);
        a_head_with_a_full_run_merges_on_it_without_a_second_run(&mut r);
        a_request_github_refuses_is_asked_once_and_told(&mut r);
    }
}
