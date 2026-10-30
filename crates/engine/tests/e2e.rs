//! One item through the whole engineer graph, from the queue to main, and the architect's works beside it: a scripted
//! agent stands in for claude and a scripted GitHub for gh, over real git repositories in a temporary directory. Every
//! return the agent hands in is built from the cell's own schema, so a change to the flow that the engine does not
//! follow fails here.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use db::Actor;
use db::rusqlite::params;
use engine::agent::{Agents, Reply, Request};
use engine::hub::{CheckRun, Failure, GitHub};
use engine::{Advance, Engine};
use serde_json::{Value, json};

/// How many returns of a cell in a row the flow the engine runs sends back before it pauses the work.
fn pause_after(e: &mut Engine) -> usize {
    let now = e.flow_now;
    e.flow(now)
        .unwrap()
        .graph(engine::ENGINEER)
        .unwrap()
        .rounds
        .as_ref()
        .unwrap()
        .sent_back
        .pause_after as usize
}

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "init.defaultBranch=main",
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn write(dir: &Path, file: &str, text: &str) {
    let p = dir.join(file);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
}

/// A value that fits a schema the compiler writes: required fields only, the first branch and value unless `pick`
/// names one at that path (dotted from the return's top, `[]` for a list's items), and `set` taken whole.
fn example(s: &Value, path: &str, pick: &BTreeMap<String, String>, set: &BTreeMap<String, Value>) -> Value {
    if let Some(v) = set.get(path) {
        return v.clone();
    }
    let o = s.as_object().unwrap();
    if let Some(c) = o.get("const") {
        return c.clone();
    }
    if let Some(e) = o.get("enum").and_then(Value::as_array) {
        return match pick.get(path) {
            Some(p) => json!(p),
            None => e[0].clone(),
        };
    }
    for key in ["oneOf", "anyOf"] {
        if let Some(alts) = o.get(key).and_then(Value::as_array) {
            let wanted = pick.get(path);
            let marked = |b: &Value, w: &str| {
                b["properties"]
                    .as_object()
                    .is_some_and(|p| p.values().any(|v| v["const"] == w))
                    || b["required"] == json!([w])
            };
            let alt = match wanted {
                Some(w) => alts
                    .iter()
                    .find(|b| marked(b, w))
                    .unwrap_or_else(|| panic!("{path}: no branch {w}")),
                None => &alts[0],
            };
            return example(alt, path, pick, set);
        }
    }
    let child = |k: &str| {
        if path.is_empty() {
            k.to_string()
        } else {
            format!("{path}.{k}")
        }
    };
    match o.get("type").and_then(Value::as_str) {
        Some("string") => json!("x"),
        Some("integer") => json!(1),
        Some("boolean") => json!(false),
        Some("array") => {
            let n = o.get("minItems").and_then(Value::as_u64).unwrap_or(0);
            let item = &o["items"];
            Value::Array((0..n).map(|_| example(item, &format!("{path}[]"), pick, set)).collect())
        }
        Some("object") => {
            let required: Vec<&str> = o
                .get("required")
                .and_then(Value::as_array)
                .map(|r| r.iter().filter_map(Value::as_str).collect())
                .unwrap_or_default();
            let mut out = serde_json::Map::new();
            for (k, v) in o
                .get("properties")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default()
            {
                let p = child(&k);
                if required.contains(&k.as_str()) || set.contains_key(&p) || pick.contains_key(&p) {
                    out.insert(k.clone(), example(&v, &p, pick, set));
                }
            }
            Value::Object(out)
        }
        other => panic!("{path}: a schema of no known type: {other:?}"),
    }
}

/// What the scripted agent hands in at a cell, and does in the worktree before. Under `@error` it hands in nothing,
/// as a run that failed with that report; under `@compactions`, the session was compacted that many times; under
/// `@usage`, what the CLI reports the round used. The script `@ping` answers a keep-warm ping, its `@usage` what the
/// ping used.
type Script = Box<dyn Fn(&Request, usize) -> (Vec<(&'static str, &'static str)>, Vec<(&'static str, Value)>)>;

struct Scripted {
    scripts: BTreeMap<&'static str, Script>,
    rounds: RefCell<BTreeMap<String, usize>>,
}

impl Agents for Scripted {
    fn run(&self, r: &Request, _activity: &dyn Fn(&str)) -> Reply {
        if r.replay.is_some() {
            return self.ping(r);
        }
        if r.bare {
            return self.repair(r);
        }
        let n = {
            let mut rounds = self.rounds.borrow_mut();
            let n = rounds.entry(r.cell.clone()).or_default();
            *n += 1;
            *n
        };
        let (pick, set) = match self.scripts.get(r.cell.as_str()) {
            Some(f) => f(r, n),
            None => (Vec::new(), Vec::new()),
        };
        let mut pick: BTreeMap<String, String> =
            pick.into_iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        // The architect's turn in a talk with the owner: a reply to what the owner said, else done, the owner having
        // ended it, unless the script says otherwise; a return sent back is handed in again the same way.
        if r.cell == "H8" && !pick.contains_key("outcome") && !pick.contains_key("outcome.talk") {
            let said = r.prompt.contains("The owner said something you have not answered yet");
            pick.insert("outcome.talk".into(), if said { "reply" } else { "done" }.into());
        }
        let mut set: BTreeMap<String, Value> = set.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
        let session = r.resume.clone().unwrap_or_else(|| r.new_session.clone());
        if let Some(e) = set.get("@error") {
            return Reply {
                session,
                usage: json!({ "started": true }),
                error: e.as_str().map(str::to_string),
                ..Reply::default()
            };
        }
        let compactions = set.remove("@compactions").and_then(|c| c.as_i64()).unwrap_or(0);
        let usage = set.remove("@usage").unwrap_or_else(|| json!({ "cost_usd": 0.01 }));
        set.entry("side_reports".into()).or_insert(json!([]));
        set.entry("tell_owner".into()).or_insert(json!([]));
        set.entry("flow_feedback".into()).or_insert(json!("none"));
        set.entry("progress".into())
            .or_insert(json!(format!("round {n} at {}", r.cell)));
        let fault = set.remove("@unparsed");
        let as_text = set.remove("@outcome_text").is_some();
        let mut output = example(&r.schema, "", &pick, &set);
        if let Some(fault) = fault.as_ref().and_then(Value::as_str) {
            let raw = broken(&output, fault);
            let why = match fault {
                "outcome_text" => "Output does not match required schema: /outcome: must be object",
                _ => "StructuredOutput was called with input that could not be parsed as JSON.",
            };
            return Reply {
                session,
                model: Some(r.model.clone()),
                usage,
                error: Some(
                    "error_max_structured_output_retries: Failed to provide valid structured output after 3 attempts"
                        .into(),
                ),
                unparsed: vec![engine::agent::Unparsed {
                    id: "toolu_1".into(),
                    length: Some(raw.chars().count()),
                    why: Some(why.into()),
                    raw,
                }],
                ..Reply::default()
            };
        }
        // A return whose outcome is a string holding its JSON, as a command line that did not check it hands it in.
        if as_text {
            output["outcome"] = json!(output["outcome"].to_string());
        }
        Reply {
            output: Some(output),
            session,
            model: Some(r.model.clone()),
            usage,
            compactions,
            ..Reply::default()
        }
    }

    fn version(&self, _cli: &config::Cli, _config_dir: Option<&Path>) -> String {
        "scripted".to_string()
    }
}

/// A return as an agent would have broken it writing it by hand: `drop_last_brace`, with the fields every return holds
/// inside `outcome` and its last brace left out; `unescaped_quote`, with a bare quote in its progress; `cut_off`, half
/// of it; `outcome_text`, with `outcome` a string holding its JSON; `whole`, as it is.
fn broken(output: &Value, fault: &str) -> String {
    match fault {
        "outcome_text" => {
            let mut v = output.clone();
            v["outcome"] = json!(v["outcome"].to_string());
            v.to_string()
        }
        "drop_last_brace" => {
            let mut v = output.clone();
            let o = v.as_object_mut().unwrap();
            let moved: Vec<(String, Value)> = ["tell_owner", "progress"]
                .iter()
                .filter_map(|k| o.shift_remove(*k).map(|x| (k.to_string(), x)))
                .collect();
            o["outcome"].as_object_mut().unwrap().extend(moved);
            let t = v.to_string();
            t[..t.len() - 1].to_string()
        }
        "unescaped_quote" => output.to_string().replacen(" at ", " \"at\" ", 1),
        "cut_off" => {
            let t = output.to_string();
            t.chars().take(t.chars().count() / 2).collect()
        }
        _ => output.to_string(),
    }
}

impl Scripted {
    /// A repair of a broken return, the command line's bare call: the script `@repair` answers it, under `@result` with
    /// its text and `@usage` what it cost; with no script, the text given back as it was.
    fn repair(&self, r: &Request) -> Reply {
        let n = {
            let mut rounds = self.rounds.borrow_mut();
            let n = rounds.entry("@repair".to_string()).or_default();
            *n += 1;
            *n
        };
        let (_, set) = self.scripts.get("@repair").map(|f| f(r, n)).unwrap_or_default();
        let set: BTreeMap<&str, Value> = set.into_iter().collect();
        let text = r
            .prompt
            .split("The text:\n")
            .nth(1)
            .unwrap_or_default()
            .trim()
            .to_string();
        Reply {
            session: r.new_session.clone(),
            model: Some(r.model.clone()),
            usage: set
                .get("@usage")
                .cloned()
                .unwrap_or_else(|| json!({ "session_cost_usd": 0.002 })),
            result: Some(set.get("@result").and_then(Value::as_str).map_or(text, str::to_string)),
            ..Reply::default()
        }
    }

    /// A keep-warm ping: what the script `@ping` says it used, or a read of the session's cache.
    fn ping(&self, r: &Request) -> Reply {
        let n = {
            let mut rounds = self.rounds.borrow_mut();
            let n = rounds.entry("@ping".to_string()).or_default();
            *n += 1;
            *n
        };
        let (_, set) = self.scripts.get("@ping").map(|f| f(r, n)).unwrap_or_default();
        let set: BTreeMap<&str, Value> = set.into_iter().collect();
        let session = r.resume.clone().unwrap_or_default();
        if let Some(e) = set.get("@error") {
            return Reply {
                session,
                error: e.as_str().map(str::to_string),
                ..Reply::default()
            };
        }
        let usage = set.get("@usage").cloned().unwrap_or_else(|| {
            json!({ "usage": { "input_tokens": 3, "cache_read_input_tokens": 9_000, "cache_creation_input_tokens": 20,
                               "output_tokens": 5 } })
        });
        Reply {
            output: Some(json!({})),
            session,
            model: Some("m".into()),
            usage,
            ..Reply::default()
        }
    }
}

/// GitHub as the engine sees it, over a bare repository: CI is always green, and a merge squashes the branch onto
/// main.
struct FakeHub {
    origin: PathBuf,
    scratch: PathBuf,
    prs: RefCell<BTreeMap<i64, String>>,
}

impl GitHub for FakeHub {
    fn pull_request(
        &self,
        _repo: &str,
        branch: &str,
        _base: &str,
        _title_body: Option<(&str, &str)>,
        existing: Option<i64>,
    ) -> Result<i64, Failure> {
        if let Some(n) = existing {
            return Ok(n);
        }
        let mut prs = self.prs.borrow_mut();
        if let Some((n, _)) = prs.iter().find(|(_, b)| *b == branch) {
            return Ok(*n);
        }
        let n = prs.len() as i64 + 1;
        prs.insert(n, branch.to_string());
        Ok(n)
    }

    fn checks(&self, _repo: &str, sha: &str, workflow: &str) -> Result<Vec<CheckRun>, Failure> {
        Ok(vec![CheckRun {
            name: "check".into(),
            result: "green".into(),
            run: format!("https://example.invalid/{workflow}/{sha}"),
            log_tail: None,
            log_path: None,
        }])
    }

    fn rerun(&self, repo: &str, sha: &str, workflow: &str) -> Result<Vec<CheckRun>, Failure> {
        self.checks(repo, sha, workflow)
    }

    fn merge(&self, _repo: &str, pr: i64) -> Result<String, Failure> {
        let branch = self
            .prs
            .borrow()
            .get(&pr)
            .cloned()
            .ok_or_else(|| Failure::other("no such PR"))?;
        let dir = self.scratch.join(format!("merge-{pr}"));
        git(
            &self.scratch,
            &["clone", "-q", &self.origin.to_string_lossy(), &dir.to_string_lossy()],
        );
        git(&dir, &["merge", "--squash", &format!("origin/{branch}")]);
        git(&dir, &["commit", "-q", "-m", &format!("Merge PR {pr}")]);
        git(&dir, &["push", "-q", "origin", "HEAD:main"]);
        Ok(git(&dir, &["rev-parse", "HEAD"]))
    }

    fn close(&self, _repo: &str, _pr: i64, _comment: &str) -> Result<(), Failure> {
        Ok(())
    }

    /// Says the command and the directory it ran in, where the real gh reads which repository it is about.
    fn gh(&self, dir: &Path, args: &[String]) -> Result<(i32, String), Failure> {
        Ok((0, format!("{} in {}", args.join(" "), dir.display())))
    }
}

/// The ids of the columns the review table lists in a prompt.
fn columns(prompt: &str) -> Vec<String> {
    let Some(start) = prompt.find("## Review table") else {
        return Vec::new();
    };
    prompt[start..]
        .lines()
        .skip(1)
        .skip_while(|l| l.trim().is_empty())
        .take_while(|l| !l.trim().is_empty())
        .filter_map(|l| l.split_once(". ").map(|(id, _)| id.to_string()))
        .collect()
}

struct Setup {
    root: PathBuf,
    data: PathBuf,
    origin: PathBuf,
}

fn setup(name: &str) -> Setup {
    let root = std::env::temp_dir().join(format!("wf-agents-e2e-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let origin = root.join("origin.git");
    git(&root, &["init", "-q", "--bare", &origin.to_string_lossy()]);
    let checkout = root.join("checkout");
    git(
        &root,
        &["clone", "-q", &origin.to_string_lossy(), &checkout.to_string_lossy()],
    );
    write(&checkout, "AGENTS.md", "How to work here.\n");
    write(&checkout, "design/overview.md", "# Overview\nThe design.\n");
    write(&checkout, "src/lib.txt", "old\n");
    git(&checkout, &["add", "-A"]);
    git(&checkout, &["commit", "-q", "-m", "start"]);
    git(&checkout, &["push", "-q", "origin", "HEAD:main"]);
    let data = root.join("data");
    std::fs::create_dir_all(&data).unwrap();
    write(
        &data,
        "install.toml",
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
    );
    write(
        &data,
        "projects/p.toml",
        &format!(
            r#"
repo = "o/p"
checkout = "{}"
design_goal = "simple"
design_paths = ["design/"]
test_paths = ["tests/"]
rules = ["ground"]
[ci]
fast = "ci"
full = ["ci"]
merge_checks = ["check"]
"#,
            checkout.display()
        ),
    );
    write(&data, "rules/ground.md", "Facts first.\n");
    write(&data, ".gitignore", "db/\nworktrees/\nlogs/\n");
    git(&data, &["init", "-q"]);
    git(&data, &["add", "-A"]);
    git(&data, &["commit", "-q", "-m", "configuration"]);
    engine::database::create(&data).unwrap();
    Setup { root, data, origin }
}

fn scripts() -> BTreeMap<&'static str, Script> {
    let mut s: BTreeMap<&'static str, Script> = BTreeMap::new();
    s.insert(
        "D2",
        Box::new(|_: &Request, _: usize| {
            (
                vec![
                    ("outcome.design_tree", "unchanged"),
                    ("outcome.measure", "not_measured"),
                ],
                vec![(
                    "outcome.acceptance",
                    json!([{ "by": "test", "id": "A1", "condition": "The feature is there." }]),
                )],
            )
        }),
    );
    s.insert(
        "D3",
        Box::new(|_: &Request, _: usize| (vec![("outcome.better", "present_best")], Vec::new())),
    );
    s.insert(
        "D9",
        Box::new(|_: &Request, _: usize| (vec![("outcome.adversarial", "not_needed")], Vec::new())),
    );
    // Implementing: the first round changes the code and asks for ci; the second hands in the end.
    s.insert(
        "I1",
        Box::new(|r: &Request, n: usize| {
            let wt = &r.worktree;
            if n == 1 {
                write(wt, "src/lib.txt", "old\nfeature\n");
                git(wt, &["add", "-A"]);
                git(wt, &["commit", "-q", "-m", "Add the feature"]);
                let head = git(wt, &["rev-parse", "HEAD"]);
                let tools = json!([{ "tool": "ci", "head": head, "tests": ["feature"], "pr": { "title": "Add the feature", "body": "It adds it." } }]);
                return (vec![("outcome", "tools")], vec![("outcome.tools", tools)]);
            }
            let head = git(wt, &["rev-parse", "HEAD"]);
            (
                Vec::new(),
                vec![
                    ("outcome.head", json!(head)),
                    ("outcome.conditions", json!([{ "how": "tested", "id": "A1", "tests": ["feature"] }])),
                ],
            )
        }),
    );
    s.insert(
        "T4",
        Box::new(|r: &Request, _: usize| {
            let cols: Vec<Value> = columns(&r.prompt)
                .into_iter()
                .map(|id| json!({ "id": id, "verdict": "pass", "evidence": { "files": [], "ran": [], "nodes": [] } }))
                .collect();
            (
                vec![("outcome.no_adversarial", "stands")],
                vec![("outcome.columns", json!(cols))],
            )
        }),
    );
    s
}

/// Keeps project p's architect from starting a work for slots free while a test looks at something else: a matter of
/// slots free was handled just now, by an architect work over before the test, so the engine raises no other for an
/// hour.
fn quiet(e: &mut Engine) {
    let v = e.flow_now;
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO works (item, flow_version, graph, at, ended, project)
             VALUES (NULL, ?1, 'architect', 'E2', 'over before the test', 'p')",
            [v],
        )?;
        let arch = tx.last_insert_rowid();
        tx.execute(
            "INSERT INTO rounds (work, n, cell, input, output, transition, config_commit, started_at, ended_at)
             VALUES (?1, 1, 'H10', '{}', '{}', 'done', 'c', ?2, ?2)",
            params![arch, db::now()],
        )?;
        tx.execute(
            "INSERT INTO architect_matters (project, kind, what, body, at, handled_work, handled_round)
             VALUES ('p', 'slots_free', 'not yet', '{}', ?1, ?2, 1)",
            params![db::now(), arch],
        )?;
        Ok(())
    })
    .unwrap();
}

/// The engine over a setup, with what it says kept, and one item in the queue; no advance starts an architect work.
fn start(s: &Setup, scripts: BTreeMap<&'static str, Script>) -> (Engine, Rc<RefCell<Vec<String>>>, i64) {
    let (mut e, said) = engine(s, scripts);
    quiet(&mut e);
    let item = e
        .db
        .write(Actor::Owner, |tx| {
            tx.execute(
                "INSERT INTO items (project, title, problem, why, deliver, level) VALUES ('p', 'Add the feature', 'It is missing.', 'People need it.', 'change', 'l')",
                [],
            )?;
            let id = tx.last_insert_rowid();
            tx.execute("INSERT INTO dispatch_queue (item, position) VALUES (?1, 1.0)", params![id])?;
            Ok(id)
        })
        .unwrap();
    (e, said, item)
}

/// The engine over a setup as it starts, with what it says kept.
fn engine(s: &Setup, scripts: BTreeMap<&'static str, Script>) -> (Engine, Rc<RefCell<Vec<String>>>) {
    let agents = Scripted {
        scripts,
        rounds: RefCell::new(BTreeMap::new()),
    };
    let hub = FakeHub {
        origin: s.origin.clone(),
        scratch: s.root.clone(),
        prs: RefCell::new(BTreeMap::new()),
    };
    let committed = config::committed(&s.data).unwrap();
    let mut e = Engine::new(&s.data, committed, Box::new(agents), Box::new(hub)).unwrap();
    e.retry_wait = std::time::Duration::ZERO;
    let said = Rc::new(RefCell::new(Vec::<String>::new()));
    let log = said.clone();
    e.say = Box::new(move |x| log.borrow_mut().push(x.to_string()));
    (e, said)
}

fn state(e: &Engine, item: i64) -> Option<(i64, String, Option<String>)> {
    e.db.conn()
        .query_row("SELECT id, at, paused FROM works WHERE item = ?1", [item], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .ok()
}

#[test]
fn an_item_goes_from_the_queue_to_main() {
    let s = setup("main");
    let (mut e, said, item) = start(&s, scripts());
    let mut done = false;
    for _ in 0..200 {
        e.tick().unwrap();
        if let Some((id, _, paused)) = &state(&e, item) {
            assert!(
                paused.is_none(),
                "the work paused: {paused:?}\n{}",
                said.borrow().join("\n")
            );
            if e.advance(*id).unwrap() == Advance::Stopped {
                done = true;
                break;
            }
        }
    }
    let log = said.borrow().join("\n");
    assert!(done, "the work did not end:\n{log}");
    let (at, pr): (String, Option<i64>) =
        e.db.conn()
            .query_row("SELECT at, pr FROM works WHERE item = ?1", [item], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
    assert_eq!(at, "E1", "{log}");
    assert_eq!(pr, Some(1));
    // Done at its end, the work let go of what it held: no session goes on, and its pull request, merged, is not
    // closed.
    let going: i64 =
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM sessions s JOIN works w ON w.id = s.work WHERE w.item = ?1 AND s.ended IS NULL",
                [item],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(going, 0, "{log}");
    let closing: i64 =
        e.db.conn()
            .query_row("SELECT count(*) FROM outbox WHERE kind = 'close_pr'", [], |r| r.get(0))
            .unwrap();
    assert_eq!(closing, 0, "{log}");
    let main = git(
        &s.root,
        &["--git-dir", &s.origin.to_string_lossy(), "show", "main:src/lib.txt"],
    );
    assert_eq!(main, "old\nfeature");
    let cells: Vec<String> = engine::query_json(
        e.db.conn(),
        "SELECT cell, transition FROM rounds WHERE work = (SELECT id FROM works WHERE item = ?1) ORDER BY n",
        &[&item],
        1000,
    )
    .unwrap()
    .iter()
    .map(|r| {
        format!(
            "{}:{}",
            r["cell"].as_str().unwrap_or_default(),
            r["transition"].as_str().unwrap_or("-")
        )
    })
    .collect();
    for c in [
        "S1:started",
        "I1:tools",
        "X1:ran",
        "T2:review_first",
        "T3:done",
        "M2:merged",
        "L1:green",
        "E1:-",
    ] {
        assert!(cells.iter().any(|x| x == c), "no round {c}: {cells:?}\n{log}");
    }
    let rejected: i64 =
        e.db.conn()
            .query_row("SELECT count(*) FROM rounds WHERE rejected IS NOT NULL", [], |r| {
                r.get(0)
            })
            .unwrap();
    assert_eq!(rejected, 0, "{log}");
    // The table review reads the worktree only, and has no column for a check the engine answered itself because
    // the project lacks what it needs (here a gap in what is under test, and a dependency on another project).
    let input_of = |cell: &str| -> Value {
        let input: String =
            e.db.conn()
                .query_row(
                    "SELECT input FROM rounds WHERE cell = ?1 ORDER BY work, n LIMIT 1",
                    [cell],
                    |r| r.get(0),
                )
                .unwrap();
        db::put_back(e.db.conn(), &serde_json::from_str(&input).unwrap())
    };
    let columns = input_of("T4")["engine"]["columns"].to_string();
    assert!(!columns.contains("a gap in what is under test"), "{columns}");
    assert!(!columns.contains("cross-project dependency"), "{columns}");
    assert!(columns.contains("unsure facts"), "{columns}");
    // The review line's session is given the worktree once, at its first round, the blind review.
    let blind = input_of("T3");
    let message = blind["message"].as_str().unwrap_or_default();
    assert!(
        message.contains("you only read it, and change and commit nothing there"),
        "{message}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// Building a work's next message is a dry run beside an engine that may be running: with the built-in flow newer than
// the work's, it records no flow version, stops no work, starts none, and writes nothing at all.
#[test]
fn building_a_message_writes_nothing() {
    let s = setup("dry");
    let (mut e, said, item) = start(&s, scripts());
    for _ in 0..2 {
        e.step(None).unwrap();
    }
    let log = said.borrow().join("\n");
    let (work, at, _) = state(&e, item).unwrap();
    assert_eq!(at, "R2", "{log}");
    // The work runs on a flow version the engine no longer has built in.
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO flow_versions (digest, compiled) SELECT 'older', compiled FROM flow_versions WHERE id = 1",
            [],
        )?;
        let v = tx.last_insert_rowid();
        tx.execute("UPDATE works SET flow_version = ?2 WHERE id = ?1", params![work, v])?;
        Ok(())
    })
    .unwrap();
    drop(e);
    let path = config::database(&s.data);
    let state_of = || -> Vec<i64> {
        let c = db::open_read(&path).unwrap();
        let n = |q: &str| -> i64 { c.query_row(q, [], |r| r.get(0)).unwrap() };
        vec![
            n("SELECT count(*) FROM flow_versions"),
            n("SELECT count(*) FROM works"),
            n("SELECT count(*) FROM works WHERE ended IS NOT NULL"),
            n("SELECT count(*) FROM rounds"),
            n("SELECT count(*) FROM changes"),
            n("SELECT count(*) FROM engine_notices"),
            n("SELECT count(*) FROM outbox"),
        ]
    };
    let before = state_of();
    let agents = Scripted {
        scripts: scripts(),
        rounds: RefCell::new(BTreeMap::new()),
    };
    let hub = FakeHub {
        origin: s.origin.clone(),
        scratch: s.root.clone(),
        prs: RefCell::new(BTreeMap::new()),
    };
    let committed = config::committed(&s.data).unwrap();
    let mut r = Engine::reading(&s.data, committed, Box::new(agents), Box::new(hub)).unwrap();
    let m = engine::next_message(&mut r, work).unwrap();
    assert!(m.text.contains("cell R2"), "{}", m.text);
    drop(r);
    assert_eq!(state_of(), before);
    let _ = std::fs::remove_dir_all(&s.root);
}

// A cell that may not change the design paths is sent back for a change an earlier attempt of it made and left on the
// branch, with a reason that names that commit and says to revert it; once reverted, the cell's return is taken.
#[test]
fn an_earlier_attempts_design_change_is_named_and_reverted() {
    let s = setup("earlier-attempt");
    let mut agents = scripts();
    agents.insert(
        "D3",
        Box::new(|r: &Request, n: usize| {
            let wt = &r.worktree;
            if n == 1 {
                write(wt, "design/sneaked.md", "Decision: not here.\n");
                git(wt, &["add", "-A"]);
                git(wt, &["commit", "-q", "-m", "Sneak a decision in"]);
            }
            if n == 3 {
                let sha = git(wt, &["log", "--format=%H", "--grep", "Sneak a decision in"]);
                git(wt, &["revert", "--no-edit", sha.trim()]);
            }
            (vec![("outcome.better", "present_best")], Vec::new())
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    let rethinks = |e: &Engine| -> Vec<(Option<String>, Option<String>)> {
        let mut st =
            e.db.conn()
                .prepare(
                    "SELECT r.transition, r.rejected FROM rounds r JOIN works w ON w.id = r.work
                     WHERE w.item = ?1 AND r.cell = 'D3' AND r.ended_at IS NOT NULL ORDER BY r.work, r.n",
                )
                .unwrap();
        st.query_map([item], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(|x| x.unwrap())
            .collect()
    };
    for _ in 0..40 {
        if rethinks(&e).iter().any(|(t, _)| t.is_some()) {
            break;
        }
        e.step(None).unwrap();
    }
    let log = said.borrow().join("\n");
    let rounds = rethinks(&e);
    assert_eq!(rounds.len(), 3, "{rounds:?}\n{log}");
    let first = rounds[0].1.clone().unwrap_or_default();
    assert!(
        first.contains("this round changed the design paths: design/sneaked.md"),
        "{first}"
    );
    let second = rounds[1].1.clone().unwrap_or_default();
    assert!(
        second.contains("an earlier attempt of this cell") && second.contains("Sneak a decision in"),
        "{second}"
    );
    assert!(second.contains("revert them"), "{second}");
    assert!(!second.contains("this round changed"), "{second}");
    assert!(rounds[2].0.is_some(), "{rounds:?}");
    let _ = std::fs::remove_dir_all(&s.root);
}

/// Puts a commit on main that writes the files given, as another pull request merged meanwhile would.
fn main_moves_on(root: &Path, files: &[(&str, &str)], message: &str) {
    let dir = root.join("elsewhere");
    if !dir.exists() {
        let origin = root.join("origin.git");
        git(
            root,
            &["clone", "-q", &origin.to_string_lossy(), &dir.to_string_lossy()],
        );
    }
    git(&dir, &["fetch", "-q", "origin", "main"]);
    git(&dir, &["reset", "-q", "--hard", "origin/main"]);
    for (f, text) in files {
        write(&dir, f, text);
    }
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", message]);
    git(&dir, &["push", "-q", "origin", "HEAD:main"]);
}

/// Merges main as it now is into a worktree's branch.
fn merge_main(wt: &Path) {
    git(wt, &["fetch", "-q", "origin", "main"]);
    git(wt, &["merge", "-q", "--no-edit", "origin/main"]);
}

/// The rounds of a cell that ended, in order, each with the row it took and why it was sent back.
fn ended_at(e: &Engine, item: i64, cell: &str) -> Vec<(Option<String>, Option<String>)> {
    let mut st =
        e.db.conn()
            .prepare(
                "SELECT r.transition, r.rejected FROM rounds r JOIN works w ON w.id = r.work
                 WHERE w.item = ?1 AND r.cell = ?2 AND r.ended_at IS NOT NULL ORDER BY r.work, r.n",
            )
            .unwrap();
    st.query_map(params![item, cell], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(|x| x.unwrap())
        .collect()
}

/// Steps the engine until the item's work is at the cell.
fn step_to(e: &mut Engine, item: i64, cell: &str) {
    for _ in 0..40 {
        if state(e, item).is_some_and(|(_, at, _)| at == cell) {
            return;
        }
        e.step(None).unwrap();
    }
    panic!("the work did not come to {cell}");
}

fn worktree_of(e: &Engine, work: i64) -> PathBuf {
    let wt: String =
        e.db.conn()
            .query_row("SELECT worktree FROM works WHERE id = ?1", [work], |r| r.get(0))
            .unwrap();
    PathBuf::from(wt)
}

/// Gives the work a design card that matched the design paths at the head given: the card is put on the work's
/// latest round that recorded the head it left, which is made to have left that head.
fn card_at(e: &mut Engine, work: i64, head: &str) {
    e.db.write(Actor::Engine, |tx| {
        let round: i64 = tx.query_row(
            "SELECT max(n) FROM rounds WHERE work = ?1 AND json_extract(input, '$.engine.head_after') IS NOT NULL",
            [work],
            |r| r.get(0),
        )?;
        tx.execute(
            "UPDATE rounds SET input = json_set(input, '$.engine.head_after', ?3) WHERE work = ?1 AND n = ?2",
            params![work, round, head],
        )?;
        tx.execute(
            "INSERT INTO design_cards (work, round, body, status) VALUES (?1, ?2, '[]', 'open')",
            [work, round],
        )?;
        Ok(())
    })
    .unwrap();
}

/// Implementing as `scripts` has it, except that the first round, after committing the feature with the files in
/// `own`, merges main moved on by the files in `main` before it asks for ci on the merge.
fn implementing_over_a_merge(
    root: PathBuf,
    own: &'static [(&'static str, &'static str)],
    main: &'static [(&'static str, &'static str)],
) -> Script {
    Box::new(move |r: &Request, n: usize| {
        let wt = &r.worktree;
        if n == 1 {
            write(wt, "src/lib.txt", "old\nfeature\n");
            for (f, text) in own {
                write(wt, f, text);
            }
            git(wt, &["add", "-A"]);
            git(wt, &["commit", "-q", "-m", "Add the feature"]);
            main_moves_on(&root, main, "Main moves on");
            merge_main(wt);
            let head = git(wt, &["rev-parse", "HEAD"]);
            let tools = json!([{ "tool": "ci", "head": head, "tests": ["feature"], "pr": { "title": "Add the feature", "body": "It adds it." } }]);
            return (vec![("outcome", "tools")], vec![("outcome.tools", tools)]);
        }
        let head = git(wt, &["rev-parse", "HEAD"]);
        (
            Vec::new(),
            vec![
                ("outcome.head", json!(head)),
                (
                    "outcome.conditions",
                    json!([{ "how": "tested", "id": "A1", "tests": ["feature"] }]),
                ),
            ],
        )
    })
}

// What a merge of main brings in is not the work's own change: a cell that may not change the design paths takes a
// return whose round merged main with changes to them, and a work with a design card hands in no new one for the
// changes to the design paths that a merge of main brought in after the card.
#[test]
fn a_merge_of_main_is_not_the_works_own_design_change() {
    let s = setup("merge-main");
    let mut agents = scripts();
    let root = s.root.clone();
    agents.insert(
        "D3",
        Box::new(move |r: &Request, n: usize| {
            if n == 1 {
                main_moves_on(&root, &[("design/main.md", "Decision: main's.\n")], "Main's decision");
                merge_main(&r.worktree);
            }
            (vec![("outcome.better", "present_best")], Vec::new())
        }),
    );
    agents.insert(
        "I1",
        implementing_over_a_merge(
            s.root.clone(),
            &[],
            &[("design/later.md", "Decision: main's later one.\n")],
        ),
    );
    let (mut e, said, item) = start(&s, agents);
    step_to(&mut e, item, "I1");
    let rethinks = ended_at(&e, item, "D3");
    assert_eq!(rethinks.len(), 1, "{rethinks:?}\n{}", said.borrow().join("\n"));
    assert!(rethinks[0].0.is_some(), "{rethinks:?}");
    let (work, _, _) = state(&e, item).unwrap();
    let wt = worktree_of(&e, work);
    card_at(&mut e, work, &git(&wt, &["rev-parse", "HEAD"]));
    for _ in 0..10 {
        if ended_at(&e, item, "I1")
            .iter()
            .any(|(t, why)| t.as_deref() == Some("done") || why.is_some())
        {
            break;
        }
        e.step(None).unwrap();
    }
    let log = said.borrow().join("\n");
    let implemented = ended_at(&e, item, "I1");
    assert!(
        implemented.iter().all(|(_, why)| why.is_none()),
        "{implemented:?}\n{log}"
    );
    assert!(
        implemented.iter().any(|(t, _)| t.as_deref() == Some("done")),
        "{implemented:?}\n{log}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// A cell's own change to the design paths is still sent back when its round also merged main, and the reason names
// only its own: the files it changed, and the commits an earlier attempt of it left, never main's.
#[test]
fn a_cells_own_design_change_counts_beside_a_merge_of_main() {
    let s = setup("own-beside-merge");
    let mut agents = scripts();
    let root = s.root.clone();
    agents.insert(
        "D3",
        Box::new(move |r: &Request, n: usize| {
            let wt = &r.worktree;
            if n == 1 {
                write(wt, "design/sneaked.md", "Decision: not here.\n");
                git(wt, &["add", "-A"]);
                git(wt, &["commit", "-q", "-m", "Sneak a decision in"]);
                main_moves_on(&root, &[("design/main.md", "Decision: main's.\n")], "Main's decision");
                merge_main(wt);
            }
            if n == 3 {
                let sha = git(wt, &["log", "--format=%H", "--grep", "Sneak a decision in"]);
                git(wt, &["revert", "--no-edit", sha.trim()]);
            }
            (vec![("outcome.better", "present_best")], Vec::new())
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    for _ in 0..40 {
        if ended_at(&e, item, "D3").iter().any(|(t, _)| t.is_some()) {
            break;
        }
        e.step(None).unwrap();
    }
    let log = said.borrow().join("\n");
    let rounds = ended_at(&e, item, "D3");
    assert_eq!(rounds.len(), 3, "{rounds:?}\n{log}");
    let first = rounds[0].1.clone().unwrap_or_default();
    assert!(
        first.contains("this round changed the design paths: design/sneaked.md"),
        "{first}"
    );
    assert!(!first.contains("design/main.md"), "{first}");
    let second = rounds[1].1.clone().unwrap_or_default();
    assert!(
        second.contains("an earlier attempt of this cell") && second.contains("Sneak a decision in"),
        "{second}"
    );
    assert!(!second.contains("Main's decision"), "{second}");
    assert!(!second.contains("design/main.md"), "{second}");
    assert!(rounds[2].0.is_some(), "{rounds:?}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A work with a design card hands in a new one for the design paths it changed itself after the card, and for a
// design path it had changed that main changed too and a merge brought in; not for one only main changed.
#[test]
fn a_design_path_the_work_and_main_both_changed_needs_a_new_design_card() {
    let s = setup("both-changed");
    let mut agents = scripts();
    agents.insert(
        "I1",
        implementing_over_a_merge(
            s.root.clone(),
            &[("design/own.md", "Decision: the work's own.\n")],
            &[
                ("design/overview.md", "# The overview\nThe design.\n"),
                ("design/main.md", "Decision: main's.\n"),
            ],
        ),
    );
    let (mut e, said, item) = start(&s, agents);
    step_to(&mut e, item, "I1");
    let (work, _, _) = state(&e, item).unwrap();
    let wt = worktree_of(&e, work);
    write(
        &wt,
        "design/overview.md",
        "# Overview\nThe design.\nThe work's decision.\n",
    );
    git(&wt, &["add", "-A"]);
    git(&wt, &["commit", "-q", "-m", "The work's decision"]);
    card_at(&mut e, work, &git(&wt, &["rev-parse", "HEAD"]));
    for _ in 0..10 {
        if ended_at(&e, item, "I1").iter().any(|(_, why)| why.is_some()) {
            break;
        }
        e.step(None).unwrap();
    }
    let log = said.borrow().join("\n");
    let why = ended_at(&e, item, "I1")
        .into_iter()
        .find_map(|(_, why)| why)
        .unwrap_or_default();
    assert!(
        why.contains("the design paths changed since the last design card (design/overview.md, design/own.md)"),
        "{why}\n{log}"
    );
    assert!(!why.contains("design/main.md"), "{why}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The advisor is the engineer with a higher model: an item's advisor round runs on the level one above the item's,
// with that level's command line and model.
#[test]
fn the_advisor_runs_one_level_above_the_item() {
    let s = setup("advisor-level");
    let install = std::fs::read_to_string(s.data.join("install.toml")).unwrap();
    write(
        &s.data,
        "install.toml",
        &format!("{install}\n[agents.levels.m]\ndesc = \"one level up\"\ncli = \"claude\"\nmodel = \"m-up\"\n"),
    );
    git(&s.data, &["commit", "-q", "-am", "a level above"]);
    let mut agents = scripts();
    let implement = agents.remove("I1").unwrap();
    agents.insert(
        "I1",
        Box::new(move |r: &Request, n: usize| {
            if n == 1 {
                return (vec![("outcome", "ask"), ("outcome.ask", "advisor")], Vec::new());
            }
            implement(r, n - 1)
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    let advised = |e: &Engine| -> Option<(String, String)> {
        e.db.conn()
            .query_row(
                "SELECT r.model, s.line FROM rounds r JOIN works w ON w.id = r.work JOIN sessions s ON s.id = r.session
                 WHERE w.item = ?1 AND s.line = 'advisor' ORDER BY r.work, r.n LIMIT 1",
                [item],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .ok()
    };
    for _ in 0..60 {
        if advised(&e).is_some() {
            break;
        }
        e.step(None).unwrap();
    }
    let log = said.borrow().join("\n");
    assert_eq!(advised(&e), Some(("m-up".to_string(), "advisor".to_string())), "{log}");
    let engineer: String =
        e.db.conn()
            .query_row(
                "SELECT r.model FROM rounds r JOIN works w ON w.id = r.work JOIN sessions s ON s.id = r.session
                 WHERE w.item = ?1 AND s.line = 'engineer' ORDER BY r.work, r.n LIMIT 1",
                [item],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(engineer, "m", "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A value of one branch is the cell as its newest return left it: a conflict the full CI reported is in the fix
// table only while the newest run of that cell still reports it.
#[test]
fn a_settled_row_leaves_the_fix_table() {
    let s = setup("settled");
    let (mut e, _, item) = start(&s, scripts());
    for _ in 0..3 {
        e.step(None).unwrap();
    }
    let (work, _, _) = state(&e, item).unwrap();
    let conflict = json!({ "result": { "kind": "conflict", "files": ["src/conflicted_here.rs"] } });
    let green = json!({ "result": { "kind": "green", "head": "abc" } });
    let add = |e: &mut Engine, output: &Value| {
        e.db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO rounds (work, n, cell, input, output, transition, config_commit, started_at, ended_at)
                 VALUES (?1, ?2, 'T2', '{}', ?3, 'next', 'c', 1, 2)",
                params![work, next_round(tx, work)?, output.to_string()],
            )?;
            tx.execute("UPDATE works SET at = 'T6' WHERE id = ?1", [work])?;
            Ok(())
        })
        .unwrap();
    };
    add(&mut e, &conflict);
    let m = engine::next_message(&mut e, work).unwrap().text;
    assert!(m.contains("src/conflicted_here.rs"), "{m}");
    add(&mut e, &green);
    let m = engine::next_message(&mut e, work).unwrap().text;
    assert!(!m.contains("src/conflicted_here.rs"), "{m}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A single step runs exactly one round: starting the work is the first, and each later one is the next round of the
// work that has a step to take.
#[test]
fn a_single_step_runs_exactly_one_round() {
    let s = setup("step");
    let (mut e, said, item) = start(&s, scripts());
    // The rounds of works on items: the architect work `quiet` recorded before the test is not counted.
    let rounds = |e: &Engine| -> i64 {
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM rounds r JOIN works w ON w.id = r.work WHERE w.graph = 'engineer'",
                [],
                |r| r.get(0),
            )
            .unwrap()
    };
    let first = e.step(None).unwrap();
    let log = said.borrow().join("\n");
    let (work, at, _) = state(&e, item).unwrap_or_else(|| panic!("no work started:\n{log}"));
    assert_eq!(first.work, Some(work), "{log}");
    assert_eq!(first.started_item, Some(item));
    assert_eq!(first.cell.as_deref(), Some("S1"));
    assert_eq!(rounds(&e), 1, "{log}");
    assert_ne!(at, "S1", "{log}");
    let mut last = first.work;
    for n in 2..=4 {
        let next = e.step(last).unwrap();
        let log = said.borrow().join("\n");
        assert_eq!(next.work, Some(work), "{log}");
        assert_eq!(next.started_item, None);
        assert_eq!(next.rounds.len(), 1, "{next:?}\n{log}");
        assert_eq!(rounds(&e), n, "{log}");
        last = next.work;
    }
    let _ = std::fs::remove_dir_all(&s.root);
}

// What an agent tells the owner is kept with its round, unread, and the owner is told; the work goes on.
#[test]
fn an_agent_tells_the_owner_and_the_work_goes_on() {
    let s = setup("tell");
    let mut scripts = scripts();
    scripts.insert(
        "R1",
        Box::new(|_: &Request, _: usize| {
            (
                Vec::new(),
                vec![("tell_owner", json!(["The item's example does not run as written."]))],
            )
        }),
    );
    let (mut e, said, item) = start(&s, scripts);
    for _ in 0..4 {
        e.step(None).unwrap();
    }
    let log = said.borrow().join("\n");
    let (_, at, paused) = state(&e, item).unwrap();
    assert_eq!(paused, None, "{log}");
    assert!(at != "S1" && at != "R1", "{log}");
    let (cell, text, read): (String, String, Option<i64>) = e
        .db
        .conn()
        .query_row(
            "SELECT r.cell, m.text, m.read_at FROM owner_messages m JOIN rounds r ON r.work = m.work AND r.n = m.round",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(
        (cell.as_str(), text.as_str(), read),
        ("R1", "The item's example does not run as written.", None)
    );
    assert_eq!(told(&s, "tells you: The item's example"), 1);
    let _ = std::fs::remove_dir_all(&s.root);
}

/// The latest work of an item: its id, cell, worktree, branch, pull request and why it ended.
type WorkRow = (i64, String, Option<String>, Option<String>, Option<i64>, Option<String>);

fn works_of(e: &Engine, item: i64) -> Vec<WorkRow> {
    let mut st =
        e.db.conn()
            .prepare("SELECT id, at, worktree, branch, pr, ended FROM works WHERE item = ?1 ORDER BY id")
            .unwrap();
    st.query_map([item], |r| {
        Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))
    })
    .unwrap()
    .map(|x| x.unwrap())
    .collect()
}

// When the engine stopped in the middle of a round, the same work goes on in the same session: the round is cut off,
// and the next one is only told so and where to look.
#[test]
fn a_round_cut_off_by_the_engine_goes_on_in_the_same_session() {
    let s = setup("cut");
    let (mut e, said, item) = start(&s, scripts());
    for _ in 0..2 {
        e.step(None).unwrap();
    }
    let log = said.borrow().join("\n");
    let (work, at, _) = state(&e, item).unwrap();
    assert_eq!(at, "R2", "{log}");
    let session: i64 =
        e.db.conn()
            .query_row(
                "SELECT id FROM sessions WHERE work = ?1 AND line = 'engineer'",
                [work],
                |r| r.get(0),
            )
            .unwrap();
    // The engine stops while cell `R2` runs.
    let cut = e
        .db
        .write(Actor::Engine, |tx| {
            let n = next_round(tx, work)?;
            tx.execute(
                "INSERT INTO rounds (work, n, cell, session, input, config_commit, started_at) VALUES (?1, ?2, 'R2', ?3, '{}', 'c', 1)",
                params![work, n, session],
            )?;
            Ok(n)
        })
        .unwrap();
    e.step(None).unwrap();
    let rounds = agent_rounds(&e, item);
    let (id, cell, input) = rounds.last().unwrap();
    assert_eq!(cell, "R2");
    let m = input["message"].as_str().unwrap();
    assert!(
        m.starts_with(&format!(
            "# {id} · cell R2\n\nR{work}-{cut} was cut off midway. First look at the worktree"
        )),
        "{m}"
    );
    assert_eq!(input["conditional"], json!([]));
    assert!(input["call"]["session"]["resumed"].is_string(), "{input}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// When the flow moves to a new version, a work on the old one is not taken over when the engine starts: the owner still
// answers its asks. Once it goes on, it stops, saying why, and its item gets a new work at once: it takes over the
// branch, the worktree and the pull request, its cell `R1` is told what exists, and what the owner
// answered stays the owner's answer. An item whose works ended can be queued again.
#[test]
fn a_new_flow_version_takes_a_work_over_when_it_next_goes_on() {
    let s = setup("version");
    let (mut e, said, item) = start(&s, scripts());
    for _ in 0..80 {
        e.tick().unwrap();
        if works_of(&e, item)[0].4.is_some() {
            break;
        }
    }
    let log = said.borrow().join("\n");
    let (old, _, worktree, branch, pr, _) = works_of(&e, item)[0].clone();
    assert!(pr.is_some() && worktree.is_some() && branch.is_some(), "{log}");
    // The work ran on a flow version the engine no longer has built in.
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO flow_versions (digest, compiled) SELECT 'older', compiled FROM flow_versions WHERE id = 1",
            [],
        )?;
        let v = tx.last_insert_rowid();
        tx.execute("UPDATE works SET flow_version = ?2 WHERE id = ?1", params![old, v])?;
        Ok(())
    })
    .unwrap();
    // It put two asks to the owner, and a question to the architect.
    let asks: Vec<i64> =
        e.db.write(Actor::Engine, |tx| {
            let round: i64 = tx.query_row("SELECT max(n) FROM rounds WHERE work = ?1", [old], |r| r.get(0))?;
            let mut asks = Vec::new();
            for _ in 0..2 {
                tx.execute(
                    "INSERT INTO owner_asks (work, round, body) VALUES (?1, ?2, '{\"to\": \"owner\"}')",
                    [old, round],
                )?;
                asks.push(tx.last_insert_rowid());
            }
            db::queue_matter(
                tx,
                &db::Matter {
                    project: "p".into(),
                    kind: "engineer_question".into(),
                    detail: Some("other_question".into()),
                    what: format!("Work {old} on item {item} asks which name to keep"),
                    body: json!({ "to": "architect", "work": old }),
                    item: Some(item),
                    raised: Some((old, round)),
                },
            )?;
            Ok(asks)
        })
        .unwrap();
    drop(e);
    let (mut e, said) = engine(&s, scripts());
    let works = works_of(&e, item);
    assert_eq!(works.len(), 1, "{works:?}");
    assert_eq!(works[0].5, None);
    let answer = api::Answer {
        text: "Keep the old name.".into(),
        why: None,
        reply: None,
        choice: None,
        to: None,
    };
    api::answer(&mut e.db, asks[0], &answer).unwrap();
    let first = e.step(None).unwrap();
    let log = said.borrow().join("\n");
    let works = works_of(&e, item);
    assert_eq!(works.len(), 2, "{works:?}\n{log}");
    assert!(
        works[0]
            .5
            .as_deref()
            .is_some_and(|w| w.starts_with("stopped: the flow moved to version")),
        "{works:?}"
    );
    let new = works[1].0;
    assert_eq!((first.work, first.started_item), (Some(new), Some(item)), "{log}");
    let by = |e: &Engine, ask: i64| -> String {
        e.db.conn()
            .query_row(
                "SELECT json_extract(answer, '$.by') FROM owner_asks WHERE id = ?1",
                [ask],
                |r| r.get(0),
            )
            .unwrap()
    };
    assert_eq!((by(&e, asks[0]), by(&e, asks[1])), ("owner".into(), "engine".into()));
    // The work stopped as every work ends: its sessions ended and its question left the architect's queue, while the
    // new work took over its worktree, branch and pull request.
    let going: i64 =
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM sessions WHERE work = ?1 AND ended IS NULL",
                [old],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(going, 0);
    let withdrawn: String =
        e.db.conn()
            .query_row("SELECT withdrawn FROM architect_matters WHERE work = ?1", [old], |r| {
                r.get(0)
            })
            .unwrap();
    assert!(
        withdrawn.starts_with("the work that raised it ended (stopped: the flow moved to version"),
        "{withdrawn}"
    );
    e.step(None).unwrap();
    let log = said.borrow().join("\n");
    let works = works_of(&e, item);
    assert_eq!(
        (&works[1].2, &works[1].3, works[1].4),
        (&worktree, &branch, pr),
        "{log}"
    );
    let input: String =
        e.db.conn()
            .query_row("SELECT input FROM rounds WHERE work = ?1 AND cell = 'R1'", [new], |r| {
                r.get(0)
            })
            .unwrap();
    let input: Value = db::put_back(e.db.conn(), &serde_json::from_str(&input).unwrap());
    let m = input["message"].as_str().unwrap();
    for want in [
        "This item was worked on before.".to_string(),
        format!("- Work {old} "),
        format!("WHERE work = {old} ORDER BY n"),
        "git log --oneline origin/main..HEAD".to_string(),
        format!("pr view {} --comments", pr.unwrap()),
        "it was half done before".to_string(),
        format!("- Work {old} asked the architect, and had no answer when it ended, so the question was withdrawn"),
    ] {
        assert!(m.contains(&want), "{want}\n{m}");
    }
    // Once its works ended, the item can be queued again.
    e.db.write(Actor::Owner, |tx| {
        tx.execute("UPDATE works SET ended = 'stopped by hand' WHERE id = ?1", [new])?;
        tx.execute("INSERT INTO dispatch_queue (item, position) VALUES (?1, 1.0)", [item])?;
        Ok(())
    })
    .unwrap();
    let ready: i64 =
        e.db.conn()
            .query_row("SELECT item FROM ready_items", [], |r| r.get(0))
            .unwrap();
    assert_eq!(ready, item);
    let _ = std::fs::remove_dir_all(&s.root);
}

// A work the engine pauses because a step cannot be taken closes the round the step left open, with the reason, and
// tells the owner once. A round an earlier engine left open when it paused is closed with the pause once the owner
// lets the work go on, and the cell runs again.
#[test]
fn a_paused_work_closes_its_open_round_and_tells_the_owner() {
    let s = setup("paused");
    let (mut e, said, item) = start(&s, scripts());
    for _ in 0..2 {
        e.step(None).unwrap();
    }
    let (work, at, _) = state(&e, item).unwrap();
    let open_round = |e: &mut Engine| -> i64 {
        e.db.write(Actor::Engine, |tx| {
            let n = next_round(tx, work)?;
            tx.execute(
                "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES (?1, ?2, ?3, '{}', 'c', 1)",
                params![work, n, at],
            )?;
            Ok(n)
        })
        .unwrap()
    };
    let rejected = |e: &Engine, round: i64| -> (Option<i64>, Value) {
        let (ended, rejected): (Option<i64>, Option<String>) =
            e.db.conn()
                .query_row(
                    "SELECT ended_at, rejected FROM rounds WHERE work = ?1 AND n = ?2",
                    [work, round],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .unwrap();
        (
            ended,
            rejected
                .map(|x| serde_json::from_str(&x).unwrap())
                .unwrap_or(Value::Null),
        )
    };
    let left = open_round(&mut e);
    e.pause(work, "cell R1: the return does not fit").unwrap();
    let (ended, why) = rejected(&e, left);
    assert!(ended.is_some());
    assert_eq!(why, json!({ "error": "cell R1: the return does not fit" }));
    let told: String =
        e.db.conn()
            .query_row("SELECT text FROM engine_notices WHERE work = ?1", [work], |r| r.get(0))
            .unwrap();
    assert!(told.ends_with("is paused: cell R1: the return does not fit"), "{told}");
    // An earlier engine paused without closing the round; the owner lets the work go on.
    let earlier = open_round(&mut e);
    api::resume(&mut e.db, work).unwrap();
    e.step(None).unwrap();
    let log = said.borrow().join("\n");
    let (ended, why) = rejected(&e, earlier);
    assert!(ended.is_some(), "{log}");
    assert_eq!(
        why,
        json!({ "error": "the work was paused: cell R1: the return does not fit" }),
        "{log}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// A query an agent asks for runs in the tool cell, and the tool cell's return, the rows included, fits the schema the
// compiled flow gives cell `X1`: the engine checks its own tool cell's return against it and pauses the work when it
// does not, so a result the flow does not describe fails here.
#[test]
fn a_query_runs_in_the_tool_cell_and_its_return_fits_the_flow() {
    let s = setup("query");
    let mut agents = scripts();
    let implement = agents.remove("I1").unwrap();
    agents.insert(
        "I1",
        Box::new(move |r: &Request, n: usize| {
            if n == 1 {
                let tools = json!([{ "tool": "query", "sql": "SELECT id, title FROM items" }]);
                return (vec![("outcome", "tools")], vec![("outcome.tools", tools)]);
            }
            implement(r, n - 1)
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    let ran = |e: &Engine| -> Option<String> {
        e.db.conn()
            .query_row(
                "SELECT r.output FROM rounds r JOIN works w ON w.id = r.work
                 WHERE w.item = ?1 AND r.cell = 'X1' AND r.transition IS NOT NULL ORDER BY r.work, r.n LIMIT 1",
                [item],
                |r| r.get(0),
            )
            .ok()
    };
    for _ in 0..60 {
        if ran(&e).is_some() || state(&e, item).is_some_and(|(_, _, paused)| paused.is_some()) {
            break;
        }
        e.step(None).unwrap();
    }
    let log = said.borrow().join("\n");
    let (work, _, paused) = state(&e, item).unwrap();
    assert_eq!(paused, None, "{log}");
    let output: Value = serde_json::from_str(&ran(&e).expect(&log)).unwrap();
    let result = &output["results"][0];
    assert_eq!(result["tool"], "query", "{output}");
    let rows = result["ran"]["rows"].as_array().cloned().unwrap_or_default();
    assert!(rows.iter().any(|r| r["title"] == "Add the feature"), "{output}");
    let w = e.work(work).unwrap();
    let g = e.graph_of(&w).unwrap();
    let schema = g.cell("X1").and_then(|c| c.schema.clone()).unwrap();
    assert!(flow::valid(&schema, &output), "{output}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The round of a work at a gate whose need the project does not answer is the engine's own answer: it says why no
// agent is asked, and no agent runs.
#[test]
fn a_gate_the_project_cannot_meet_is_the_engines_answer() {
    let s = setup("gate-preview");
    let (mut e, said, item) = start(&s, scripts());
    for _ in 0..40 {
        if state(&e, item).is_some_and(|(_, at, _)| at == "D4") {
            break;
        }
        e.step(None).unwrap();
    }
    let log = said.borrow().join("\n");
    let (work, at, _) = state(&e, item).unwrap();
    assert_eq!(at, "D4", "{log}");
    e.step(None).unwrap();
    let (output, input): (String, String) =
        e.db.conn()
            .query_row(
                "SELECT output, input FROM rounds WHERE work = ?1 AND cell = 'D4'",
                [work],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
    let output: Value = serde_json::from_str(&output).unwrap();
    let why = output["outcome"]["answer"]["why"].as_str().unwrap_or_default();
    assert!(
        why.starts_with("Not asked: this question needs what this project does not have."),
        "{output}"
    );
    let input = db::put_back(e.db.conn(), &serde_json::from_str(&input).unwrap());
    assert_eq!(input, json!({ "engine": { "not_applicable": "under_test" } }));
    let _ = std::fs::remove_dir_all(&s.root);
}

// A new work that takes over from one the new flow version stopped takes a slot only when one is free; until then its
// item waits at the head of the queue, ahead of the items queued before, in the order of the works it replaces.
#[test]
fn a_takeover_waits_for_a_free_slot_at_the_head_of_the_queue() {
    let s = setup("takeover-slots");
    let (mut e, _, queued) = start(&s, scripts());
    // Two items had works running on a flow version the engine no longer has built in; the machine has one slot.
    let items: Vec<i64> =
        e.db.write(Actor::Owner, |tx| {
            tx.execute(
                "INSERT INTO flow_versions (digest, compiled) SELECT 'older', compiled FROM flow_versions WHERE id = 1",
                [],
            )?;
            let v = tx.last_insert_rowid();
            let mut items = Vec::new();
            for title in ["First", "Second"] {
                tx.execute(
                    "INSERT INTO items (project, title, problem, why, deliver, level)
                     VALUES ('p', ?1, 'It is missing.', 'People need it.', 'change', 'l')",
                    [title],
                )?;
                let item = tx.last_insert_rowid();
                tx.execute(
                    "INSERT INTO works (item, flow_version, graph, at) VALUES (?1, ?2, 'engineer', 'R1')",
                    params![item, v],
                )?;
                items.push(item);
            }
            Ok(items)
        })
        .unwrap();
    drop(e);
    let (mut e, said) = engine(&s, scripts());
    let ready = |e: &Engine| -> Vec<i64> {
        let mut st = e.db.conn().prepare("SELECT item FROM ready_items").unwrap();
        st.query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect()
    };
    // Nothing is taken over as the engine starts; both are as soon as works go on, before the item queued before.
    assert_eq!(works_of(&e, items[0]).len(), 1);
    let stepped = e.step(None).unwrap();
    let log = said.borrow().join("\n");
    assert_eq!(stepped.started_item, Some(items[0]), "{log}");
    // The first takes the slot; the second waits ahead of the item queued before.
    assert_eq!(works_of(&e, items[0]).len(), 2, "{log}");
    assert_eq!(works_of(&e, items[1]).len(), 1, "{log}");
    assert_eq!(ready(&e), vec![items[1], queued], "{log}");
    assert_eq!(e.free_slots().unwrap(), 0);
    // Once the slot is free, the second starts, before the item queued before.
    let first = works_of(&e, items[0])[1].0;
    e.db.write(Actor::Owner, |tx| {
        tx.execute("UPDATE works SET ended = 'stopped by hand' WHERE id = ?1", [first])?;
        Ok(())
    })
    .unwrap();
    let stepped = e.step(None).unwrap();
    let log = said.borrow().join("\n");
    assert_eq!(stepped.started_item, Some(items[1]), "{log}");
    // The first, whose work was stopped, waits again in the architect's order, after the item the owner queued.
    assert_eq!(ready(&e), vec![queued, items[0]], "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// An item imported with a branch and its pull request takes them over in its first work, and its cell
// `R1` is told where it came from.
#[test]
fn an_imported_item_takes_over_its_branch_and_pull_request() {
    let s = setup("imported");
    let checkout = s.root.join("checkout");
    git(&checkout, &["checkout", "-q", "-b", "outside/feature"]);
    write(&checkout, "src/lib.txt", "half done\n");
    git(&checkout, &["commit", "-q", "-am", "Half of the feature"]);
    git(&checkout, &["push", "-q", "origin", "outside/feature"]);
    let tip = git(&checkout, &["rev-parse", "HEAD"]).trim().to_string();
    git(&checkout, &["checkout", "-q", "main"]);
    git(&checkout, &["branch", "-q", "-D", "outside/feature"]);
    let (mut e, said, item) = start(&s, scripts());
    e.db.write(Actor::Owner, |tx| {
        tx.execute(
            "UPDATE items SET imported_from = 'item 41 of the earlier board', imported_branch = 'outside/feature',
             imported_pr = 9 WHERE id = ?1",
            [item],
        )?;
        Ok(())
    })
    .unwrap();
    e.step(None).unwrap();
    e.step(None).unwrap();
    let log = said.borrow().join("\n");
    let works = works_of(&e, item);
    assert_eq!(
        (works[0].3.as_deref(), works[0].4),
        (Some("outside/feature"), Some(9)),
        "{log}"
    );
    let head = git(Path::new(works[0].2.as_deref().unwrap()), &["rev-parse", "HEAD"]);
    assert_eq!(head.trim(), tip);
    let rounds = agent_rounds(&e, item);
    let m = rounds[0].2["message"].as_str().unwrap();
    assert!(
        m.contains("imported from outside this flow: item 41 of the earlier board"),
        "{m}"
    );
    assert!(m.contains("pr view 9 --comments"), "{m}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// Three returns of a cell sent back in a row pause the work; once the owner lets it go on, it counts afresh.
#[test]
fn a_work_let_go_on_after_returns_were_sent_back_counts_afresh() {
    let s = setup("resume");
    let mut scripts = scripts();
    // As many returns as the flow sends back before it pauses, read once the engine has the flow.
    let max = Rc::new(std::cell::Cell::new(0));
    let bad_until = max.clone();
    scripts.insert(
        "R1",
        Box::new(move |_: &Request, n: usize| {
            let bad = vec![("outcome", json!("the JSON text of an object"))];
            (Vec::new(), if n <= bad_until.get() { bad } else { Vec::new() })
        }),
    );
    let (mut e, said, item) = start(&s, scripts);
    max.set(pause_after(&mut e));
    let mut paused = None;
    for _ in 0..20 {
        e.tick().unwrap();
        if let Some((id, _, Some(why))) = state(&e, item) {
            paused = Some((id, why));
            break;
        }
    }
    let log = said.borrow().join("\n");
    let (id, why) = paused.unwrap_or_else(|| panic!("the work did not pause:\n{log}"));
    assert!(why.starts_with("3 returns of cell R1 in a row were sent back"), "{why}");
    assert!(why.contains("outcome must be an object, and is a string"), "{why}");
    api::resume(&mut e.db, id).unwrap();
    for _ in 0..3 {
        e.tick().unwrap();
    }
    let log = said.borrow().join("\n");
    let (_, at, paused) = state(&e, item).unwrap();
    assert_eq!(paused, None, "{log}");
    assert_ne!(at, "R1", "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

/// Each agent round of the item's works, oldest first: its name, its cell, and what the engine recorded it gave.
fn agent_rounds(e: &Engine, item: i64) -> Vec<(String, String, Value)> {
    engine::query_json(
        e.db.conn(),
        "SELECT 'R' || r.work || '-' || r.n AS name, r.cell, r.input FROM rounds r JOIN works w ON w.id = r.work
         WHERE w.item = ?1 AND r.session IS NOT NULL ORDER BY r.work, r.n",
        &[&item],
        1000,
    )
    .unwrap()
    .into_iter()
    .map(|r| {
        (
            r["name"].as_str().unwrap().to_string(),
            r["cell"].as_str().unwrap().to_string(),
            db::put_back(e.db.conn(), &r["input"]),
        )
    })
    .collect()
}

/// The number the work's next round gets.
fn next_round(tx: &db::rusqlite::Transaction<'_>, work: i64) -> Result<i64, db::Error> {
    Ok(tx.query_row(
        "SELECT coalesce(max(n), 0) + 1 FROM rounds WHERE work = ?1",
        [work],
        |r| r.get(0),
    )?)
}

/// Whether a round's message gave the conditional input at an address, or only referred to it; none when it has none there.
fn given(input: &Value, address: &str) -> Option<bool> {
    input["conditional"]
        .as_array()?
        .iter()
        .find(|m| m["address"] == address)
        .and_then(|m| m["given"].as_bool())
}

/// The hash of the conditional input at an address in a round's message.
fn hash_of(input: &Value, address: &str) -> Option<String> {
    input["conditional"]
        .as_array()?
        .iter()
        .find(|m| m["address"] == address)
        .and_then(|m| m["hash"].as_str().map(str::to_string))
}

// Every message starts with its round's address; within a session a conditional input is given once and referred to
// after;
// the database is an index; and the locked acceptance is the one conditional input the design handed in, held by the
// engineer's session and given to the review's.
#[test]
fn a_message_gives_each_conditional_input_once_in_a_session() {
    let s = setup("conditional");
    let (mut e, said, item) = start(&s, scripts());
    for _ in 0..100 {
        e.tick().unwrap();
        if agent_rounds(&e, item).iter().any(|r| r.1 == "T3") {
            break;
        }
    }
    let log = said.borrow().join("\n");
    let rounds = agent_rounds(&e, item);
    let at = |cell: &str| -> Value {
        rounds
            .iter()
            .find(|r| r.1 == cell)
            .map(|r| r.2.clone())
            .unwrap_or_else(|| panic!("no round at {cell}:\n{log}"))
    };
    for (id, cell, input) in &rounds {
        let m = input["message"].as_str().unwrap();
        assert!(m.starts_with(&format!("# {id} · cell {cell} · ")), "{m}");
    }
    let (first, second) = (&at("R1"), &at("R2"));
    let (work, _, _) = state(&e, item).unwrap();
    let workspace = format!("work:{work}");
    for a in [
        "format:common",
        "format:R1",
        "inputs:R1",
        "rule:ground",
        "rule:general",
        "rule:engineering",
        workspace.as_str(),
    ] {
        assert_eq!(given(first, a), Some(true), "{a}");
    }
    for a in ["format:common", "rule:ground", workspace.as_str()] {
        assert_eq!(given(second, a), Some(false), "{a}");
    }
    // The inputs to work from are listed by their numbers, which `inputs:<cell>` explains once in a session.
    let m = first["message"].as_str().unwrap();
    assert!(m.contains("\n1. item:"), "{m}");
    assert!(m.contains(&format!("Worktree: {workspace}")), "{m}");
    assert_eq!(given(second, "format:R2"), Some(true));
    // The project guide is an input every agent's cell has: given in a session's first round, before anything else,
    // referred to after, and given again to the review's session.
    let guide = |input: &Value| -> Option<(String, bool)> {
        input["conditional"].as_array()?.iter().find_map(|m| {
            let a = m["address"].as_str()?;
            a.ends_with("·engine·project_guide")
                .then(|| (a.to_string(), m["given"].as_bool().unwrap_or(false)))
        })
    };
    let (address, sent) = guide(first).expect("the project guide in cell R1's round");
    assert!(sent, "{address}");
    let m = first["message"].as_str().unwrap();
    let at_guide = m.find(&format!("### {address}")).unwrap();
    assert!(at_guide < m.find("### format:common").unwrap(), "{m}");
    assert_eq!(guide(second).map(|g| g.1), Some(false));
    assert_eq!(guide(&at("T3")).map(|g| g.1), Some(true));
    let m = second["message"].as_str().unwrap();
    assert!(!m.contains("### rule:ground") && m.contains("rule:ground"), "{m}");
    let m = first["message"].as_str().unwrap();
    // The index of the database is in the query tool's description, which the return formats give.
    assert!(!m.contains("db:tables"), "{m}");
    assert!(given(first, "db:tables").is_none());
    // The formats name the owner's language; the engine writes the configured one beside it.
    assert!(m.contains("`owner.language` (English)"), "{m}");
    assert!(m.contains("SELECT sql FROM sqlite_master WHERE name = '<name>'"), "{m}");
    assert!(m.contains("- items (id, project, title, "), "{m}");
    assert!(!m.contains("CREATE TABLE"), "{m}");
    let design = &rounds.iter().rev().find(|r| r.1 == "D2").unwrap().0;
    let acceptance = format!("{design}·D2·acceptance");
    assert_eq!(given(&at("D9"), &acceptance), Some(false), "{}", at("D9"));
    // Implementing starts a new session, which holds nothing: the acceptance is given whole again.
    assert_eq!(given(&at("I1"), &acceptance), Some(true), "{}", at("I1"));
    assert_eq!(given(&at("T3"), &acceptance), Some(true), "{}", at("T3"));
    assert!(
        !rounds
            .iter()
            .any(|r| r.2["conditional"].to_string().contains("item_acceptance"))
    );
    let call = &second["call"];
    assert_eq!(call["cli"], "scripted");
    assert!(call["level"].is_string(), "{call}");
    assert_eq!(call["engine"], engine::VERSION);
    assert!(call["session"]["resumed"].is_string(), "{call}");
    assert!(first["call"]["session"]["started"].is_string());
    let command: Vec<&str> = call["command"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert!(
        command.contains(&"--json-schema") && command.contains(&"--resume"),
        "{call}"
    );
    assert!(
        call["env"]["CLAUDE_CONFIG_DIR"]
            .as_str()
            .is_some_and(|d| d.ends_with("agents/claude")),
        "{call}"
    );
    // The record keeps each large text once: the shared formats of each line (the engineer's and the review's) and
    // the structured-output schema are in contents once, however many rounds and sessions were given them, and a
    // round's record refers to them.
    let kept = |like: &str| -> i64 {
        e.db.conn()
            .query_row("SELECT count(*) FROM contents WHERE text LIKE ?1", [like], |r| r.get(0))
            .unwrap()
    };
    assert_eq!(kept("The forms every agent's return shares.%"), 2);
    assert_eq!(kept("{\"type\":\"object\"%"), 1);
    let raw: String =
        e.db.conn()
            .query_row(
                "SELECT input FROM rounds WHERE 'R' || work || '-' || n = ?1",
                [&rounds[0].0],
                |r| r.get(0),
            )
            .unwrap();
    assert!(
        !raw.contains("The forms every agent's return shares.") && raw.contains("{\"content\":"),
        "{raw}"
    );
    // The agents' configuration directory is kept out of the data directory's git.
    let ignore = std::fs::read_to_string(s.data.join("agents/claude/.gitignore")).unwrap();
    assert_eq!(ignore, "*\n");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A rule that changed since the session got it is given as what changed in it, under its address; after a compaction the
// session holds nothing, and the next round is given what it needs again whole, the session's own earlier returns
// included.
#[test]
fn a_changed_rule_gives_what_changed_and_a_compaction_gives_everything_again() {
    let s = setup("again");
    let mut scripts = scripts();
    scripts.insert(
        "R2",
        Box::new(|_: &Request, _: usize| (Vec::new(), vec![("@compactions", json!(1))])),
    );
    let (mut e, said, item) = start(&s, scripts);
    e.step(None).unwrap();
    e.step(None).unwrap();
    write(&s.data, "rules/ground.md", "Facts first, then the rest.\n");
    git(&s.data, &["add", "-A"]);
    git(&s.data, &["commit", "-q", "-m", "a changed rule"]);
    e.step(None).unwrap();
    e.step(None).unwrap();
    let log = said.borrow().join("\n");
    let rounds = agent_rounds(&e, item);
    let cells: Vec<&str> = rounds.iter().map(|r| r.1.as_str()).collect();
    assert_eq!(&cells[..3], ["R1", "R2", "D1"], "{log}");
    let (a, b, c) = (&rounds[0].2, &rounds[1].2, &rounds[2].2);
    assert_eq!(given(b, "rule:ground"), Some(true));
    assert_ne!(hash_of(a, "rule:ground"), hash_of(b, "rule:ground"));
    let changes = |input: &Value| {
        input["conditional"]
            .as_array()
            .and_then(|xs| xs.iter().find(|m| m["address"] == "rule:ground"))
            .map(|m| m["changes"] == true)
    };
    assert_eq!((changes(b), changes(c)), (Some(true), Some(false)));
    assert_eq!(
        given_text(b, "rule:ground").unwrap().trim_end(),
        format!(
            "What changed since rule:ground, given in {}, as a unified diff of its lines:\n@@ -1,1 +1,1 @@\n\
             -Facts first.\n+Facts first, then the rest.",
            rounds[0].0
        )
    );
    assert_eq!(given(b, "format:common"), Some(false));
    assert_eq!(given(c, "format:common"), Some(true));
    assert_eq!(given(c, "rule:ground"), Some(true));
    assert_eq!(hash_of(b, "rule:ground"), hash_of(c, "rule:ground"));
    let understanding = format!("{}·R1·understanding", rounds[0].0);
    assert_eq!(given(c, &understanding), Some(true), "{c}");
    let m = c["message"].as_str().unwrap();
    assert!(m.contains(&format!("compacted in {}", rounds[1].0)), "{m}");
    let _ = std::fs::remove_dir_all(&s.root);
}

/// The project guide in a round's message: its address, and whether the message gave it.
fn guide_in(input: &Value) -> Option<(String, bool)> {
    input["conditional"].as_array()?.iter().find_map(|m| {
        let a = m["address"].as_str()?;
        a.ends_with("·engine·project_guide")
            .then(|| (a.to_string(), m["given"].as_bool().unwrap_or(false)))
    })
}

/// What a round's message gives under an address.
fn given_text(input: &Value, address: &str) -> Option<String> {
    let m = input["message"].as_str()?;
    let heading = format!("### {address}\n\n");
    let rest = &m[m.find(&heading)? + heading.len()..];
    let end = rest.find("\n### ").or_else(|| rest.find("\n## ")).unwrap_or(rest.len());
    Some(rest[..end].to_string())
}

// The project guide is read from main as the work's branch merged it: the work's own edits to its design tree and to
// AGENTS.md, committed or not, never make it a new one, so its session is not given it again.
#[test]
fn a_works_own_edits_to_its_design_tree_do_not_give_the_guide_again() {
    let s = setup("own-guide");
    let (mut e, said, item) = start(&s, scripts());
    e.step(None).unwrap();
    e.step(None).unwrap();
    let (work, _, _) = state(&e, item).unwrap();
    let wt = worktree_of(&e, work);
    write(&wt, "design/added.md", "# Added\nThe work's own node.\n");
    git(&wt, &["add", "-A"]);
    git(&wt, &["commit", "-q", "-m", "Add a node"]);
    write(&wt, "AGENTS.md", "Edited, not committed.\n");
    e.step(None).unwrap();
    e.step(None).unwrap();
    let log = said.borrow().join("\n");
    let rounds = agent_rounds(&e, item);
    let cells: Vec<&str> = rounds.iter().map(|r| r.1.as_str()).collect();
    assert_eq!(&cells[..3], ["R1", "R2", "D1"], "{log}");
    let (address, sent) = guide_in(&rounds[0].2).unwrap();
    assert!(sent, "{address}");
    for (id, _, input) in &rounds[1..3] {
        assert_eq!(guide_in(input), Some((address.clone(), false)), "{id}");
        let m = input["message"].as_str().unwrap();
        assert!(
            !m.contains("design/added.md") && !m.contains("Edited, not committed."),
            "{m}"
        );
    }
    let _ = std::fs::remove_dir_all(&s.root);
}

// When main changed the project guide since the version a session holds, and the branch merged it, the session is
// given only the entries that differ, under the new version's address, and holds the new version after.
#[test]
fn a_changed_main_gives_only_what_changed_in_the_guide() {
    let s = setup("main-guide");
    let (mut e, said, item) = start(&s, scripts());
    e.step(None).unwrap();
    e.step(None).unwrap();
    let (work, _, _) = state(&e, item).unwrap();
    main_moves_on(
        &s.root,
        &[("design/later.md", "# Later\nMain's later node.\n")],
        "Main's later node",
    );
    merge_main(&worktree_of(&e, work));
    e.step(None).unwrap();
    e.step(None).unwrap();
    let log = said.borrow().join("\n");
    let rounds = agent_rounds(&e, item);
    let cells: Vec<&str> = rounds.iter().map(|r| r.1.as_str()).collect();
    assert_eq!(&cells[..3], ["R1", "R2", "D1"], "{log}");
    let (first, _) = guide_in(&rounds[0].2).unwrap();
    let (changed, sent) = guide_in(&rounds[1].2).unwrap();
    assert!(sent, "{changed}");
    assert_eq!(changed, format!("{}·engine·project_guide", rounds[1].0));
    let text = given_text(&rounds[1].2, &changed).unwrap();
    assert!(
        text.starts_with(&format!(
            "What changed since {first}, given in {}: each entry here takes the place of the one there, and null \
             removes it. ",
            rounds[0].0
        )),
        "{text}"
    );
    assert!(
        text.trim_end()
            .ends_with(r##"{"design_tree":{"design/later.md":"# Later"}}"##),
        "{text}"
    );
    assert!(
        !text.contains("How to work here.") && !text.contains("design/overview.md"),
        "{text}"
    );
    assert_eq!(guide_in(&rounds[2].2), Some((changed, false)));
    let _ = std::fs::remove_dir_all(&s.root);
}

// A message that would give its session whole what the session already holds is not sent: the work is paused by the
// engine, with a reason naming the input, its size and the round the session got it in, and the owner is told. The
// session's record is made to hold the rule under another address, as a fault of the addresses would.
#[test]
fn a_repeat_in_a_session_pauses_the_work_and_tells_the_owner() {
    let s = setup("repeat");
    let (mut e, said, item) = start(&s, scripts());
    e.step(None).unwrap();
    e.step(None).unwrap();
    let (work, _, _) = state(&e, item).unwrap();
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "UPDATE rounds SET input = replace(input, '\"address\":\"rule:ground\"', '\"address\":\"rule:elsewhere\"')
             WHERE work = ?1 AND n = 2",
            [work],
        )?;
        Ok(())
    })
    .unwrap();
    let before = rounds_count(&e);
    e.step(None).unwrap();
    let log = said.borrow().join("\n");
    let (_, at, paused) = state(&e, item).unwrap();
    assert_eq!(at, "R2", "{log}");
    let paused = paused.unwrap_or_else(|| panic!("the work did not pause:\n{log}"));
    assert!(
        paused.starts_with(&format!(
            "cell R2: the message of R{work}-3 was not sent, since it would repeat what its session holds: \
             rule:ground (12 characters) would be given whole again, though the session got the same content in \
             R{work}-2 as rule:elsewhere"
        )),
        "{paused}"
    );
    assert_eq!(paused_by(&e, work).as_deref(), Some("engine"));
    assert_eq!(rounds_count(&e), before, "nothing was sent:\n{log}");
    let notices: i64 =
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM engine_notices WHERE work = ?1 AND text LIKE '%is paused: cell R2: the message%'",
                [work],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(notices, 1);
    let _ = std::fs::remove_dir_all(&s.root);
}

// How many returns sent back pause a work is the number the flow the work runs on says.
#[test]
fn returns_sent_back_pause_the_work_after_as_many_as_its_flow_says() {
    let s = setup("pause");
    let mut scripts = scripts();
    scripts.insert(
        "R1",
        Box::new(|_: &Request, _: usize| (Vec::new(), vec![("outcome", json!("not an object"))])),
    );
    let (mut e, said, item) = start(&s, scripts);
    e.step(None).unwrap();
    let (work, _, _) = state(&e, item).unwrap();
    // The flow the engine compiled, sending back two returns instead of its own number.
    let now = e.flow_now;
    let mut f = e.flow(now).unwrap().clone();
    f.graphs[0].rounds.as_mut().unwrap().sent_back.pause_after = 2;
    let (digest, text) = (f.digest(), f.json());
    // The engine runs that flow, so that the work on it is not taken over.
    e.flow_now =
        e.db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO flow_versions (digest, compiled) VALUES (?1, ?2)",
                params![digest, text],
            )?;
            let v = tx.last_insert_rowid();
            tx.execute("UPDATE works SET flow_version = ?2 WHERE id = ?1", params![work, v])?;
            Ok(v)
        })
        .unwrap();
    for _ in 0..6 {
        e.tick().unwrap();
        if state(&e, item).unwrap().2.is_some() {
            break;
        }
    }
    let log = said.borrow().join("\n");
    let (_, at, paused) = state(&e, item).unwrap();
    assert_eq!(at, "R1", "{log}");
    assert!(
        paused
            .as_deref()
            .is_some_and(|p| p.starts_with("2 returns of cell R1 in a row were sent back")),
        "{paused:?}\n{log}"
    );
    // After a return sent back the message only says why, and that the return is to be rewritten.
    let rounds = agent_rounds(&e, item);
    let (id, _, again) = &rounds[1];
    let m = again["message"].as_str().unwrap();
    assert!(
        m.starts_with(&format!("# {id} · cell R1\n\nYour return was sent back: "))
            && m.ends_with("\nRewrite it (format:R1).\n"),
        "{m}"
    );
    assert_eq!(again["conditional"], json!([]));
    let _ = std::fs::remove_dir_all(&s.root);
}

/// The engine over a setup whose cell `R1` ends its first `times` runs without a return, the CLI
/// reporting `report`.
fn failing(name: &str, report: String, times: usize) -> (Setup, Engine, Rc<RefCell<Vec<String>>>, i64) {
    let s = setup(name);
    let mut scripts = scripts();
    scripts.insert(
        "R1",
        Box::new(move |_: &Request, n: usize| {
            let fail = vec![("@error", json!(report))];
            (Vec::new(), if n <= times { fail } else { Vec::new() })
        }),
    );
    let (e, said, item) = start(&s, scripts);
    (s, e, said, item)
}

/// The kind of each failed run, in order.
fn failed_kinds(e: &Engine) -> Vec<String> {
    engine::query_json(
        e.db.conn(),
        "SELECT json_extract(rejected, '$.failed.kind') AS kind FROM rounds
         WHERE json_extract(rejected, '$.failed') IS NOT NULL ORDER BY work, n",
        &[],
        100,
    )
    .unwrap()
    .iter()
    .map(|r| r["kind"].as_str().unwrap_or_default().to_string())
    .collect()
}

/// How many returns were sent back: rounds rejected that neither failed nor were cut off.
fn sent_back(e: &Engine) -> i64 {
    e.db.conn()
        .query_row(
            "SELECT count(*) FROM rounds WHERE rejected IS NOT NULL AND json_extract(rejected, '$.failed') IS NULL
             AND json_extract(rejected, '$.interrupted') IS NULL",
            [],
            |r| r.get(0),
        )
        .unwrap()
}

fn rounds_count(e: &Engine) -> i64 {
    e.db.conn()
        .query_row("SELECT count(*) FROM rounds", [], |r| r.get(0))
        .unwrap()
}

/// The lines of what the owner was told that name a word.
fn told(s: &Setup, word: &str) -> usize {
    std::fs::read_to_string(s.data.join("notifications.log"))
        .unwrap_or_default()
        .lines()
        .filter(|l| l.contains(word))
        .count()
}

// A usage limit stops every work until the time the CLI says it resets, the owner told once; then the work goes on
// from its cell, and the failed run is not a return sent back.
#[test]
fn a_usage_limit_stops_every_work_until_it_resets() {
    let reset = db::now() / 1000 + 3600;
    let (s, mut e, said, item) = failing("limit", format!("Claude AI usage limit reached|{reset}"), 1);
    e.tick().unwrap();
    let log = said.borrow().join("\n");
    let h = e.halt.clone().unwrap_or_else(|| panic!("no wait:\n{log}"));
    assert_eq!((h.kind.as_str(), h.until), ("usage_limit", reset * 1000));
    let before = rounds_count(&e);
    e.tick().unwrap();
    e.tick().unwrap();
    assert_eq!(rounds_count(&e), before);
    assert_eq!(told(&s, "usage_limit"), 1);
    e.halt.as_mut().unwrap().until = 0;
    e.tick().unwrap();
    let log = said.borrow().join("\n");
    let (_, at, paused) = state(&e, item).unwrap();
    assert_eq!(paused, None, "{log}");
    assert_ne!(at, "R1", "{log}");
    assert_eq!(failed_kinds(&e), ["usage_limit"]);
    assert_eq!(sent_back(&e), 0);
    assert_eq!(told(&s, "usage_limit"), 1);
    let _ = std::fs::remove_dir_all(&s.root);
}

// With no network every work waits until the engine looks again; then the work goes on from its cell.
#[test]
fn no_network_stops_every_work_until_it_is_back() {
    let report = "API Error: Connection error. (getaddrinfo ENOTFOUND api.anthropic.com)".to_string();
    let (s, mut e, said, item) = failing("offline", report, 1);
    e.tick().unwrap();
    let log = said.borrow().join("\n");
    let h = e.halt.clone().unwrap_or_else(|| panic!("no wait:\n{log}"));
    assert_eq!(h.kind, "offline");
    assert!(h.until > db::now());
    let before = rounds_count(&e);
    e.tick().unwrap();
    assert_eq!(rounds_count(&e), before);
    e.halt.as_mut().unwrap().until = 0;
    e.tick().unwrap();
    let (_, at, paused) = state(&e, item).unwrap();
    assert_eq!(paused, None);
    assert_ne!(at, "R1");
    assert_eq!(failed_kinds(&e), ["offline"]);
    assert_eq!(sent_back(&e), 0);
    assert_eq!(told(&s, "offline"), 1);
    let _ = std::fs::remove_dir_all(&s.root);
}

// A passing failure runs the same round again until it passes: the work neither pauses nor asks anyone, and none of
// it is a return sent back.
#[test]
fn a_passing_failure_runs_again_until_it_passes() {
    let (s, mut e, said, item) = failing("transient", "API Error: 529 Overloaded".to_string(), 3);
    for _ in 0..10 {
        e.tick().unwrap();
        if state(&e, item).unwrap().1 != "R1" {
            break;
        }
    }
    let log = said.borrow().join("\n");
    let (_, at, paused) = state(&e, item).unwrap();
    assert_ne!(at, "R1", "{log}");
    assert_eq!(paused, None, "{log}");
    assert_eq!(failed_kinds(&e), ["transient", "transient", "transient"]);
    assert_eq!(sent_back(&e), 0);
    // The round after a failed run is told its kind only, never what the command line reported, which may quote the
    // run's own return.
    let (work, _, _) = state(&e, item).unwrap();
    let again = message_at(&e, work, "R1", true);
    assert!(
        again.contains("ended without a return (transient)") && !again.contains("529"),
        "{again}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

/// A work whose cell `R1` breaks its first return as `fault` says, the script `@repair` answering a repair of it as
/// `repair` says, if given, and every later return of `R1` as `later` says, if given.
fn unparsed_at_r1(
    name: &str,
    fault: &'static str,
    repair: Option<Script>,
    later: Option<&'static str>,
) -> (Setup, Engine, Rc<RefCell<Vec<String>>>, i64) {
    let s = setup(name);
    let mut agents = scripts();
    agents.insert(
        "R1",
        Box::new(move |_: &Request, n: usize| match (n, later) {
            (1, _) => (Vec::new(), vec![("@unparsed", json!(fault))]),
            (_, Some(f)) => (Vec::new(), vec![("@unparsed", json!(f))]),
            _ => (Vec::new(), Vec::new()),
        }),
    );
    if let Some(r) = repair {
        agents.insert("@repair", r);
    }
    let (e, said, item) = start(&s, agents);
    (s, e, said, item)
}

/// What the record of a work's first round at `R1` keeps of the repair of its return, with its contents put back.
fn repair_record(e: &Engine, work: i64) -> (Value, Value) {
    let (input, usage): (String, Option<String>) =
        e.db.conn()
            .query_row(
                "SELECT input, usage FROM rounds WHERE work = ?1 AND cell = 'R1' ORDER BY n LIMIT 1",
                [work],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
    let input = db::put_back(e.db.conn(), &serde_json::from_str(&input).unwrap());
    let usage = usage.map(|u| serde_json::from_str(&u).unwrap()).unwrap_or(Value::Null);
    (input["engine"]["unparsed"].clone(), usage)
}

/// Takes steps until the work on the item has left `R1` or paused.
fn past_r1(e: &mut Engine, item: i64) {
    for _ in 0..6 {
        e.step(None).unwrap();
        if state(e, item).is_some_and(|(_, at, paused)| at != "R1" || paused.is_some()) {
            break;
        }
    }
}

// A return that lacks only its last brace, its fields every return holds written inside `outcome`, is closed by the
// engine within its round, with those fields moved up beside `outcome`: the round takes its row, nothing is run again,
// and no repair is asked of a model.
#[test]
fn a_return_that_lacks_only_its_last_brace_is_closed_within_its_round() {
    let (s, mut e, said, item) = unparsed_at_r1("unparsed-close", "drop_last_brace", None, None);
    past_r1(&mut e, item);
    let log = said.borrow().join("\n");
    let (work, at, paused) = state(&e, item).unwrap();
    assert_eq!((at.as_str(), paused), ("R2", None), "{log}");
    assert_eq!(rows_at(&e, work, "R1"), ["done"], "{log}");
    assert!(failed_kinds(&e).is_empty(), "{log}");
    let (record, _) = repair_record(&e, work);
    assert_eq!(record["closed"]["added"], "}", "{record}");
    assert!(
        record.get("repair").is_none() && record.get("again").is_none(),
        "{record}"
    );
    assert!(
        record["attempts"][0]["text"]
            .as_str()
            .is_some_and(|t| t.contains("\"tell_owner\"")),
        "{record}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

/// What the first round of a work at `R1` records of a form taken from the JSON text of it: the field it was.
fn taken_from_text(e: &Engine, work: i64) -> Option<String> {
    e.db.conn()
        .query_row(
            "SELECT json_extract(input, '$.engine.taken_from_text') FROM rounds
             WHERE work = ?1 AND cell = 'R1' ORDER BY n LIMIT 1",
            [work],
            |r| r.get(0),
        )
        .unwrap()
}

// A return whose outcome the agent wrote as a string holding its JSON is taken with the outcome that JSON holds: the
// round takes its row, and records that its outcome was taken from the text.
#[test]
fn an_outcome_handed_in_as_its_json_text_is_taken_as_the_object() {
    let s = setup("outcome-text");
    let mut agents = scripts();
    agents.insert(
        "R1",
        Box::new(|_: &Request, n: usize| match n {
            1 => (Vec::new(), vec![("@outcome_text", json!(true))]),
            _ => (Vec::new(), Vec::new()),
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    past_r1(&mut e, item);
    let log = said.borrow().join("\n");
    let (work, at, paused) = state(&e, item).unwrap();
    assert_eq!((at.as_str(), paused), ("R2", None), "{log}");
    assert_eq!(rows_at(&e, work, "R1"), ["done"], "{log}");
    assert_eq!(sent_back(&e), 0, "{log}");
    assert_eq!(taken_from_text(&e, work).as_deref(), Some("outcome"), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A run the command line gave up on because its last attempt held the outcome as a string of its JSON is taken from
// that attempt within its round, with the outcome its JSON holds: nothing is repaired or handed in again, and the
// round records both the attempt and that its outcome was taken from the text.
#[test]
fn an_attempt_refused_for_its_outcome_as_json_text_is_taken_within_its_round() {
    let (s, mut e, said, item) = unparsed_at_r1("unparsed-outcome-text", "outcome_text", None, None);
    past_r1(&mut e, item);
    let log = said.borrow().join("\n");
    let (work, at, paused) = state(&e, item).unwrap();
    assert_eq!((at.as_str(), paused), ("R2", None), "{log}");
    assert_eq!(rows_at(&e, work, "R1"), ["done"], "{log}");
    assert!(failed_kinds(&e).is_empty(), "{log}");
    let (record, _) = repair_record(&e, work);
    assert_eq!(record["form_as_text"], true, "{record}");
    assert!(
        record.get("repair").is_none() && record.get("again").is_none(),
        "{record}"
    );
    assert_eq!(taken_from_text(&e, work).as_deref(), Some("outcome"), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A return with a bare quote in a text is repaired by the lowest level, once, and the repair is taken when it parses,
// fits and keeps every other character; the round's cost holds the repair's.
#[test]
fn a_repair_that_keeps_the_content_is_taken_within_the_round() {
    let escapes: Script = Box::new(|r: &Request, _: usize| {
        let text = r.prompt.split("The text:\n").nth(1).unwrap_or_default().trim();
        let fixed = text.replacen(" \"at\" ", " \\\"at\\\" ", 1);
        (
            Vec::new(),
            vec![
                ("@result", json!(fixed)),
                ("@usage", json!({ "session_cost_usd": 0.002 })),
            ],
        )
    });
    let (s, mut e, said, item) = unparsed_at_r1("unparsed-repair", "unescaped_quote", Some(escapes), None);
    past_r1(&mut e, item);
    let log = said.borrow().join("\n");
    let (work, at, _) = state(&e, item).unwrap();
    assert_eq!(at, "R2", "{log}");
    let (record, usage) = repair_record(&e, work);
    assert_eq!(record["repair"]["taken"], true, "{record}");
    assert_eq!(record["repair"]["cost_usd"], 0.002, "{record}");
    assert!(record.get("again").is_none(), "{record}");
    // The scripted round costs 0.01, and the repair beside it.
    let cost = usage["cost_usd"].as_f64().unwrap_or_default();
    assert!((cost - 0.012).abs() < 1e-9, "{usage}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A repair that changes the content is refused, and the agent hands its return in again in its own session, which is
// taken as any return; the round records both.
#[test]
fn a_repair_that_changes_the_content_is_refused_and_the_return_handed_in_again() {
    let rewords: Script = Box::new(|r: &Request, _: usize| {
        let text = r.prompt.split("The text:\n").nth(1).unwrap_or_default().trim();
        (
            Vec::new(),
            vec![("@result", json!(text.replacen(" \"at\" ", " in ", 1)))],
        )
    });
    let (s, mut e, said, item) = unparsed_at_r1("unparsed-again", "unescaped_quote", Some(rewords), None);
    past_r1(&mut e, item);
    let log = said.borrow().join("\n");
    let (work, at, _) = state(&e, item).unwrap();
    assert_eq!(at, "R2", "{log}");
    let (record, _) = repair_record(&e, work);
    assert!(
        record["repair"]["refused"]
            .as_str()
            .is_some_and(|w| w.contains("changed")),
        "{record}"
    );
    assert_eq!(record["again"]["handed_in"], true, "{record}");
    assert!(
        record["again"]["message"]
            .as_str()
            .is_some_and(|m| m.contains("could not be parsed as JSON")),
        "{record}"
    );
    assert_eq!(rows_at(&e, work, "R1"), ["done"], "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A return cut off is never repaired: the agent is asked to hand it in again, and when that cannot be parsed either,
// the work pauses, the owner is told, and the round is closed as a failed run of its kind.
#[test]
fn a_return_neither_repaired_nor_handed_in_again_pauses_the_work() {
    let (s, mut e, said, item) = unparsed_at_r1("unparsed-pause", "cut_off", None, Some("cut_off"));
    past_r1(&mut e, item);
    let log = said.borrow().join("\n");
    let (work, at, paused) = state(&e, item).unwrap();
    assert_eq!(at, "R1", "{log}");
    assert!(
        paused.is_some_and(|p| p.contains("could not be parsed as JSON")),
        "{log}"
    );
    assert_eq!(failed_kinds(&e), ["unparsed"], "{log}");
    let (record, _) = repair_record(&e, work);
    assert_eq!(
        record["repair"]["not_tried"], "the last attempt's text was cut off",
        "{record}"
    );
    assert_eq!(record["again"]["handed_in"], false, "{record}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// An expired login stops every work, the owner told once, until a round runs again: the owner logs in and writes
// nothing, the work neither pauses nor asks, and the failed run is not a return sent back.
#[test]
fn an_expired_login_stops_every_work_until_a_round_runs_again() {
    let (s, mut e, said, item) = failing("login", "Not logged in · Please run /login".to_string(), 1);
    e.tick().unwrap();
    let log = said.borrow().join("\n");
    let h = e.halt.clone().unwrap_or_else(|| panic!("no wait:\n{log}"));
    assert_eq!(h.kind, "cannot_run");
    assert!(h.until > db::now());
    let before = rounds_count(&e);
    e.tick().unwrap();
    assert_eq!(rounds_count(&e), before);
    assert_eq!(told(&s, "cannot_run"), 1);
    e.halt.as_mut().unwrap().until = 0;
    e.tick().unwrap();
    let log = said.borrow().join("\n");
    let (_, at, paused) = state(&e, item).unwrap();
    assert_eq!(paused, None, "{log}");
    assert_ne!(at, "R1", "{log}");
    assert_eq!(failed_kinds(&e), ["cannot_run"]);
    assert_eq!(sent_back(&e), 0);
    let asks: i64 =
        e.db.conn()
            .query_row("SELECT count(*) FROM owner_asks", [], |r| r.get(0))
            .unwrap();
    assert_eq!(asks, 0);
    let _ = std::fs::remove_dir_all(&s.root);
}

// A prompt too long for the model pauses the work and tells the owner at once; once the owner lets the work go on,
// the same cell runs again.
#[test]
fn a_prompt_too_long_pauses_the_work_until_the_owner_lets_it_go_on() {
    let (s, mut e, said, item) = failing("too_long", "Prompt is too long".to_string(), 1);
    for _ in 0..5 {
        e.tick().unwrap();
        if state(&e, item).unwrap().2.is_some() {
            break;
        }
    }
    let log = said.borrow().join("\n");
    let (work, at, paused) = state(&e, item).unwrap();
    assert_eq!(at, "R1", "{log}");
    let paused = paused.unwrap_or_else(|| panic!("the work did not pause:\n{log}"));
    assert!(paused.contains("prompt_too_long"), "{paused}");
    // The owner is told when the outbox is carried out, at the start of the next pass; the paused work stays.
    e.tick().unwrap();
    assert_eq!(told(&s, "is paused"), 1);
    assert_eq!(failed_kinds(&e), ["prompt_too_long"]);
    assert_eq!(sent_back(&e), 0);
    api::resume(&mut e.db, work).unwrap();
    for _ in 0..5 {
        e.tick().unwrap();
        if state(&e, item).unwrap().1 != "R1" {
            break;
        }
    }
    let log = said.borrow().join("\n");
    let (_, at, paused) = state(&e, item).unwrap();
    assert_eq!(paused, None, "{log}");
    assert_ne!(at, "R1", "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

/// Who paused a work, while it is paused.
fn paused_by(e: &Engine, work: i64) -> Option<String> {
    e.db.conn()
        .query_row("SELECT paused_by FROM works WHERE id = ?1", [work], |r| r.get(0))
        .unwrap()
}

// A work the owner pauses while its round runs stops at the end of that round: the round ends as usual and the work
// moves on, then takes no step by itself until the owner lets it go on. While the round runs, the engine says whose
// round it is.
#[test]
fn a_work_the_owner_pauses_stops_at_the_end_of_its_round() {
    let s = setup("pause_round");
    let mut scripts = scripts();
    let path = config::database(&s.data);
    // The owner pauses the work on the page while the round of cell `R1` runs.
    scripts.insert(
        "R1",
        Box::new(move |_: &Request, _: usize| {
            let mut db = db::Db::open(&path).unwrap();
            let work: i64 = db
                .conn()
                .query_row("SELECT id FROM works WHERE ended IS NULL", [], |r| r.get(0))
                .unwrap();
            api::pause(&mut db, work, None).unwrap();
            (Vec::new(), Vec::new())
        }),
    );
    let (mut e, said, item) = start(&s, scripts);
    let busy = Rc::new(RefCell::new(Vec::new()));
    let seen = busy.clone();
    e.busy = Box::new(move |w| seen.borrow_mut().push(w));
    let first = e.step(None).unwrap();
    e.step(first.work).unwrap();
    let log = said.borrow().join("\n");
    let (work, at, paused) = state(&e, item).unwrap();
    assert_eq!(at, "R2", "{log}");
    assert_eq!(paused.as_deref(), Some(db::PAUSED_BY_OWNER), "{log}");
    assert_eq!(paused_by(&e, work).as_deref(), Some("owner"));
    let took: Option<String> =
        e.db.conn()
            .query_row(
                "SELECT transition FROM rounds WHERE work = ?1 AND cell = 'R1'",
                [work],
                |r| r.get(0),
            )
            .unwrap();
    assert!(took.is_some(), "{log}");
    assert_eq!(*busy.borrow(), [Some(work), None, Some(work), None]);
    // Paused, it takes no step by itself.
    let n = rounds_count(&e);
    assert_eq!(e.step(Some(work)).unwrap().work, None);
    assert_eq!(rounds_count(&e), n);
    // Once the owner lets it go on, it does.
    api::resume(&mut e.db, work).unwrap();
    assert_eq!(e.step(Some(work)).unwrap().work, Some(work));
    assert_eq!(rounds_count(&e), n + 1);
    let _ = std::fs::remove_dir_all(&s.root);
}

// The owner's Start all lifts no work's own pause: a work the owner paused, and one the engine paused for a problem,
// stay paused until the owner lets each go on by itself.
#[test]
fn start_all_lifts_no_work_s_own_pause() {
    let s = setup("start_all");
    let (mut e, said, item) = start(&s, scripts());
    let work = e.step(None).unwrap().work.unwrap();
    api::pause(&mut e.db, work, None).unwrap();
    let c = api::Control::default();
    c.set_paused(false);
    let n = rounds_count(&e);
    assert_eq!(turns(&mut e, &c, 2), Vec::<i64>::new());
    assert_eq!(rounds_count(&e), n);
    assert_eq!(paused_by(&e, work).as_deref(), Some("owner"));
    api::resume(&mut e.db, work).unwrap();
    assert_eq!(e.step(Some(work)).unwrap().work, Some(work));
    assert_eq!(rounds_count(&e), n + 1);
    e.pause(work, "cell R2: the return does not fit").unwrap();
    assert_eq!(paused_by(&e, work).as_deref(), Some("engine"));
    let log = said.borrow().join("\n");
    assert_eq!(turns(&mut e, &c, 2), Vec::<i64>::new(), "{log}");
    assert_eq!(
        state(&e, item).unwrap().2.as_deref(),
        Some("cell R2: the return does not fit")
    );
    api::resume(&mut e.db, work).unwrap();
    assert_eq!(e.step(Some(work)).unwrap().work, Some(work));
    let _ = std::fs::remove_dir_all(&s.root);
}

// The owner's step runs exactly one round of a paused work, which stays paused in the owner's hands. A work the
// engine paused is taken into them: its cell runs again, counting afresh as once the owner lets a work go on, and it
// stays paused by the owner.
#[test]
fn a_step_runs_one_round_of_a_paused_work_which_stays_paused() {
    let s = setup("step_paused");
    let (mut e, said, item) = start(&s, scripts());
    let work = e.step(None).unwrap().work.unwrap();
    api::pause(&mut e.db, work, None).unwrap();
    let (_, before, _) = state(&e, item).unwrap();
    let stepped = e.step_work(work).unwrap();
    let log = said.borrow().join("\n");
    assert_eq!(
        (stepped.work, stepped.rounds.len()),
        (Some(work), 1),
        "{stepped:?}\n{log}"
    );
    let (_, at, paused) = state(&e, item).unwrap();
    assert_ne!(at, before, "{log}");
    assert_eq!(paused.as_deref(), Some(db::PAUSED_BY_OWNER));
    assert_eq!(e.step(Some(work)).unwrap().work, None);
    e.pause(work, "cell R2: the return does not fit").unwrap();
    let stepped = e.step_work(work).unwrap();
    let log = said.borrow().join("\n");
    assert_eq!(stepped.rounds.len(), 1, "{stepped:?}\n{log}");
    assert_eq!(state(&e, item).unwrap().2.as_deref(), Some(db::PAUSED_BY_OWNER));
    assert_eq!(paused_by(&e, work).as_deref(), Some("owner"));
    let lifted: i64 =
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM changes WHERE table_name = 'works' AND actor = 'owner'
             AND json_extract(old_row, '$.paused_by') = 'engine' AND json_extract(new_row, '$.paused') IS NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(lifted, 1);
    let _ = std::fs::remove_dir_all(&s.root);
}

// A work's pause, and who paused it, are kept when the engine stops and starts again: a paused work takes no step by
// itself after the restart.
#[test]
fn a_work_s_pause_is_kept_across_a_restart() {
    let s = setup("pause_restart");
    let (mut e, _, item) = start(&s, scripts());
    let work = e.step(None).unwrap().work.unwrap();
    api::pause(&mut e.db, work, Some("Wait for the release.")).unwrap();
    drop(e);
    let (mut e, _) = engine(&s, scripts());
    assert_eq!(state(&e, item).unwrap().2.as_deref(), Some("Wait for the release."));
    assert_eq!(paused_by(&e, work).as_deref(), Some("owner"));
    assert_eq!(e.step(None).unwrap().work, None);
    api::resume(&mut e.db, work).unwrap();
    e.pause(work, "cell R1: the return does not fit").unwrap();
    drop(e);
    let (mut e, _) = engine(&s, scripts());
    assert_eq!(paused_by(&e, work).as_deref(), Some("engine"));
    assert_eq!(e.step(None).unwrap().work, None);
    let _ = std::fs::remove_dir_all(&s.root);
}

// A paused work on an older flow version is not taken over while nothing makes it go on; the owner's step takes it
// over, runs the first round of the new work, and leaves the new work paused in the owner's hands.
#[test]
fn a_step_takes_a_work_on_an_older_flow_version_over_and_keeps_it_paused() {
    let s = setup("step_takeover");
    let (mut e, said, item) = start(&s, scripts());
    let first = e.step(None).unwrap();
    let old = first.work.unwrap();
    e.step(first.work).unwrap();
    api::pause(&mut e.db, old, None).unwrap();
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO flow_versions (digest, compiled) SELECT 'older', compiled FROM flow_versions WHERE id = 1",
            [],
        )?;
        let v = tx.last_insert_rowid();
        tx.execute("UPDATE works SET flow_version = ?2 WHERE id = ?1", params![old, v])?;
        Ok(())
    })
    .unwrap();
    drop(e);
    let (mut e, _) = engine(&s, scripts());
    assert_eq!(e.step(None).unwrap().work, None);
    assert_eq!(works_of(&e, item).len(), 1);
    let stepped = e.step_work(old).unwrap();
    let log = said.borrow().join("\n");
    let works = works_of(&e, item);
    assert_eq!(works.len(), 2, "{works:?}\n{log}");
    assert!(
        works[0]
            .5
            .as_deref()
            .is_some_and(|w| w.starts_with("stopped: the flow moved to version")),
        "{works:?}"
    );
    let new = works[1].0;
    assert_eq!((stepped.work, stepped.started_item), (Some(new), Some(item)));
    let cells: Vec<String> = {
        let mut st =
            e.db.conn()
                .prepare("SELECT cell FROM rounds WHERE work = ?1 ORDER BY n")
                .unwrap();
        st.query_map([new], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect()
    };
    assert_eq!(cells, ["S1"]);
    assert_eq!(paused_by(&e, new).as_deref(), Some("owner"));
    assert_eq!(e.step(Some(new)).unwrap().work, None);
    let _ = std::fs::remove_dir_all(&s.root);
}

// The architects' works.

/// The project an architect's round is for, by the directory it works in, `worktrees/architect/<project>`.
fn project_of(r: &Request) -> String {
    r.worktree
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The architect's cells an architect work would not get through with their first answers, scripted: the pick takes the
/// first matter of its project's queue.
fn picks_first(s: &Setup, scripts: &mut BTreeMap<&'static str, Script>) {
    let path = config::database(&s.data);
    scripts.insert(
        "Q1",
        Box::new(move |r: &Request, _: usize| {
            let c = db::open_read(&path).unwrap();
            let first: i64 = c
                .query_row(
                    "SELECT matter FROM architect_queue WHERE project = ?1 ORDER BY matter LIMIT 1",
                    [project_of(r)],
                    |x| x.get(0),
                )
                .unwrap();
            (Vec::new(), vec![("outcome.pick.matter", json!(first))])
        }),
    );
}

/// The works of every architect, oldest first.
fn architect_works(e: &Engine) -> Vec<i64> {
    let mut st =
        e.db.conn()
            .prepare("SELECT id FROM works WHERE graph = 'architect' AND ended IS NULL ORDER BY id")
            .unwrap();
    st.query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect()
}

/// The architects' matters but the one that keeps them quiet, in the order they came: each one's kind, whether it is
/// in a queue now, and whether it is handled.
fn matters(e: &Engine) -> Vec<(String, bool, bool)> {
    let mut st =
        e.db.conn()
            .prepare(
                "SELECT m.kind, m.id IN (SELECT matter FROM architect_queue), m.handled_round IS NOT NULL
                 FROM architect_matters m WHERE m.what <> 'not yet' ORDER BY m.id",
            )
            .unwrap();
    st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .map(|x| x.unwrap())
        .collect()
}

/// Takes steps of the engine, going round the works, until `done` holds or `max` steps were taken; gives whether it
/// held.
fn steps_until(e: &mut Engine, max: usize, done: impl Fn(&mut Engine) -> bool) -> bool {
    let mut after = None;
    for _ in 0..max {
        if done(e) {
            return true;
        }
        after = e.step(after).unwrap().work;
    }
    done(e)
}

/// Whether an architect work has reached its end.
fn architect_work_done(e: &mut Engine, work: i64) -> bool {
    let w = e.work(work).unwrap();
    e.done(&w).unwrap()
}

/// The cells a work's rounds were at, in order, each with the row it took.
fn cells_of(e: &Engine, work: i64) -> Vec<(String, Option<String>)> {
    let mut st =
        e.db.conn()
            .prepare("SELECT cell, transition FROM rounds WHERE work = ?1 ORDER BY n")
            .unwrap();
    st.query_map([work], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(|x| x.unwrap())
        .collect()
}

/// The cells a work's rounds were at, in order.
fn path_of(e: &Engine, work: i64) -> Vec<String> {
    cells_of(e, work).into_iter().map(|c| c.0).collect()
}

/// The message a work's round at a cell was sent, the first or the last.
fn message_at(e: &Engine, work: i64, cell: &str, last: bool) -> String {
    let order = if last { "DESC" } else { "" };
    let input: String =
        e.db.conn()
            .query_row(
                &format!("SELECT input FROM rounds WHERE work = ?1 AND cell = ?2 ORDER BY n {order} LIMIT 1"),
                params![work, cell],
                |r| r.get(0),
            )
            .unwrap();
    let input = db::put_back(e.db.conn(), &serde_json::from_str(&input).unwrap());
    input["message"].as_str().unwrap_or_default().to_string()
}

/// Puts an item of project p no work runs on, in no queue; new works of p are held, as its architect holds them, so
/// that none starts on it while the test looks at the architect.
fn an_item(e: &mut Engine) -> i64 {
    let id =
        e.db.write(Actor::Owner, |tx| {
            tx.execute(
                "INSERT INTO items (project, title, problem, why, deliver, level)
                 VALUES ('p', 'Parse it', 'No parser.', 'Needed.', 'change', 'l')",
                [],
            )?;
            Ok(tx.last_insert_rowid())
        })
        .unwrap();
    e.db.write(Actor::Architect, |tx| {
        tx.execute(
            "INSERT INTO dispatch_holds (project, reason, since) VALUES ('p', 'architect', 1)",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    id
}

/// The owner says something to project p's architect and ends the talk at once, as Done does: the architect's first
/// turn of it is its last.
fn say(e: &mut Engine, text: &str) {
    db::say_to_architect(&mut e.db, "p", None, None, text).unwrap();
    e.db.write(Actor::Owner, |tx| {
        tx.execute(
            "UPDATE architect_matters SET talk_ended = ?1 WHERE id = (SELECT max(id) FROM architect_matters)",
            [db::now()],
        )?;
        Ok(())
    })
    .unwrap();
}

/// Whether a work stands at a cell, its last round having taken a row there.
fn stands_at(e: &Engine, work: i64, cell: &str) -> bool {
    let at: String =
        e.db.conn()
            .query_row("SELECT at FROM works WHERE id = ?1", [work], |r| r.get(0))
            .unwrap();
    at == cell
}

// The owner's words start a work of project p's architect, which goes once through the whole graph: it starts, finds
// its new session has not understood the project, understands it, judges the policy and what runs, and takes the
// owner's words first, a talk: it replies, in the owner's one-to-one, and waits for the owner, who ends the talk; its
// last turn hands in an item change, which is collected; the batch is done, its review finds the change sound, and the
// engine makes it as the architect; the rethink goes on, nothing is left, and the work wraps up and ends, referred to
// the state of things its session wrote. A field the path cell may leave out and wrote as null is taken as left out.
#[test]
fn an_architect_work_goes_through_understanding_a_matter_its_review_and_its_wrap_up() {
    let s = setup("architect-work");
    let mut agents = scripts();
    let change = json!({ "kind": "edit_item", "item": "1", "part": "title", "text": "Parse the input first",
                         "reason": "Clearer." });
    let given = change.clone();
    agents.insert(
        "H8",
        Box::new(move |r: &Request, _: usize| {
            if !r.prompt.contains("The owner ended the talk") {
                let reply = vec![
                    ("outcome.talk.text", json!("Which part is unclear?")),
                    (
                        "outcome.talk.if_we_end",
                        json!("If we end here I will retitle it as an action."),
                    ),
                ];
                return (vec![("outcome.talk", "reply")], reply);
            }
            (
                vec![("outcome.talk", "done")],
                vec![(
                    "outcome.effects",
                    json!({ "item_changes": [given.clone()], "notes": null }),
                )],
            )
        }),
    );
    agents.insert(
        "B2",
        Box::new(|_: &Request, _: usize| {
            (
                Vec::new(),
                vec![(
                    "outcome.review",
                    json!([{ "change": 1, "verdict": "ok", "why": "It reads better." }]),
                )],
            )
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    db::say_to_architect(&mut e.db, "p", None, None, "Make the parser's item clearer.").unwrap();
    let started = e.step(None).unwrap();
    let log = said.borrow().join("\n");
    assert_eq!(started.cell.as_deref(), Some("S1"), "{log}");
    let arch = started.work.unwrap();
    let project: Option<String> =
        e.db.conn()
            .query_row("SELECT project FROM works WHERE id = ?1", [arch], |r| r.get(0))
            .unwrap();
    assert_eq!(project.as_deref(), Some("p"));
    // The architect replies and waits for the owner, its reply the architect's message in the one-to-one.
    let waits =
        |e: &mut Engine| stands_at(e, arch, "H11") && cells_of(e, arch).last().unwrap().1.as_deref() == Some("waits");
    assert!(steps_until(&mut e, 40, waits), "{}", said.borrow().join("\n"));
    assert!(e.step(None).unwrap().rounds.is_empty());
    let replied: String =
        e.db.conn()
            .query_row("SELECT text FROM owner_messages ORDER BY id DESC LIMIT 1", [], |r| {
                r.get(0)
            })
            .unwrap();
    assert_eq!(
        replied,
        "Which part is unclear?\n\nIf we end here I will retitle it as an action."
    );
    api::end_talk(&mut e.db, "p").unwrap();
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, arch)),
        "{}",
        said.borrow().join("\n")
    );
    let log = said.borrow().join("\n");
    assert_eq!(
        path_of(&e, arch),
        [
            "S1", "U1", "P1", "N1", "Q0", "Q2", "H8", "H11", "H11", "H8", "B1", "B2", "B4", "Q0", "E1", "E2"
        ],
        "{log}"
    );
    let rows: Vec<Option<String>> = cells_of(&e, arch).into_iter().map(|c| c.1).collect();
    assert_eq!(rows[0].as_deref(), Some("not_understood"));
    assert_eq!(rows[4].as_deref(), Some("owner_words"));
    assert_eq!(
        rows[6..9].iter().map(|r| r.as_deref()).collect::<Vec<_>>(),
        [Some("replied"), Some("waits"), Some("ended")]
    );
    assert_eq!(rows[9].as_deref(), Some("done"));
    assert_eq!(rows[10].as_deref(), Some("review"));
    assert_eq!(rows[11].as_deref(), Some("ok"));
    // The change is made, as the architect's, and logged as made with the matter it came from.
    let title: String =
        e.db.conn()
            .query_row("SELECT title FROM items WHERE id = 1", [], |r| r.get(0))
            .unwrap();
    assert_eq!(title, "Parse the input first", "{log}");
    let actor: String =
        e.db.conn()
            .query_row(
                "SELECT actor FROM changes WHERE table_name = 'items' AND json_extract(new_row, '$.title') = ?1",
                [&title],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(actor, "architect");
    let (state, logged): (String, String) =
        e.db.conn()
            .query_row("SELECT state, change FROM architect_item_changes", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
    assert_eq!(state, "made");
    assert_eq!(serde_json::from_str::<Value>(&logged).unwrap(), change);
    assert_eq!(matters(&e), [("owner_words".to_string(), false, true)], "{log}");
    // The review ran on its own line, whose session the batch ended.
    let review: (String, Option<i64>) =
        e.db.conn()
            .query_row(
                "SELECT line, ended FROM sessions WHERE work = ?1 AND line = 'architect_review'",
                [arch],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
    assert!(review.1.is_some(), "{review:?}");
    // Every round of the architect's line gives the CLI the same schema, the rethink's included, which keeps the
    // session's prompt cache.
    let schemas: Vec<(String, Value)> = engine::query_json(
        e.db.conn(),
        "SELECT r.cell, r.input FROM rounds r JOIN sessions s ON s.id = r.session
         WHERE r.work = ?1 AND s.line = 'architect' ORDER BY r.n",
        &[&arch],
        100,
    )
    .unwrap()
    .iter()
    .map(|r| {
        let args = r["input"]["call"]["command"].as_array().cloned().unwrap_or_default();
        let at = args.iter().position(|a| a == "--json-schema").unwrap();
        (r["cell"].as_str().unwrap().to_string(), args[at + 1].clone())
    })
    .collect();
    assert!(schemas.iter().any(|x| x.0 == "B4"), "{schemas:?}");
    assert!(schemas.iter().all(|x| x.1 == schemas[0].1), "{schemas:?}");
    // The wrap-up is referred to the state of things its session wrote at the understanding, not given it again.
    let wrap_up = &inputs_at(&e, arch, "E1")[0];
    assert_eq!(given_ending(wrap_up, "·U1·state"), Some(false), "{wrap_up}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// Every agent's cell of a line, in every graph of the flow, gives the command line what it caches before the session
// the same way, so that the line's session is read from the prompt cache at every change of cell: the program and its
// arguments, the model, the structured-output schema, the tools and commands refused and the directories added, with
// the configuration directory and where it runs. The effort may differ, as far as the flow's check lets it, and the
// round's own session and hooks do; a cell on no line has a session of its own every round.
#[test]
fn every_cell_of_a_line_gives_the_command_line_the_same_cached_prefix() {
    let s = setup("prefix");
    let (mut e, _) = engine(&s, scripts());
    let item = a_ready_item(&mut e, "Parse it");
    let v = e.flow_now;
    let work =
        e.db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO works (item, flow_version, graph, at) VALUES (?1, ?2, 'engineer', 'R1')",
                params![item, v],
            )?;
            Ok(tx.last_insert_rowid())
        })
        .unwrap();
    let arch = architect_work_at(&mut e, "U1");
    let mut lines = 0;
    for id in [work, arch] {
        let w = e.work(id).unwrap();
        let g = e.graph_of(&w).unwrap();
        let item = e.subject(&w).unwrap();
        let mut by_line: BTreeMap<String, Vec<(String, Vec<String>)>> = BTreeMap::new();
        for cell in g.cells.iter().filter(|c| c.kind == "agent") {
            let Some(line) = &cell.line else { continue };
            let r = engine::cell_request(&e, &w, &g, cell, item.as_ref()).unwrap();
            let mut prefix = vec![r.worktree.display().to_string(), format!("{:?}", r.config_dir)];
            let mut words = engine::agent::command_line(&r).into_iter();
            while let Some(x) = words.next() {
                if matches!(x.as_str(), "--effort" | "--settings" | "--resume" | "--session-id") {
                    words.next();
                    continue;
                }
                prefix.push(x);
            }
            by_line.entry(line.clone()).or_default().push((cell.id.clone(), prefix));
        }
        for (line, cells) in &by_line {
            lines += 1;
            let (first, want) = &cells[0];
            for (cell, got) in cells {
                assert_eq!(
                    got, want,
                    "line {line}: cell {cell} gives the command line another prefix than cell {first}"
                );
            }
        }
    }
    // The engineer's graph has four lines, the architect's three.
    assert_eq!(lines, 7);
    let _ = std::fs::remove_dir_all(&s.root);
}

// A work of the architect goes on in the session the project's last architect work left: one that understood the
// project in it goes from starting straight to the queue, and does not understand it again.
#[test]
fn a_resumed_session_goes_from_starting_to_the_queue() {
    let s = setup("resumed");
    let (mut e, said) = engine(&s, scripts());
    quiet(&mut e);
    an_item(&mut e);
    say(&mut e, "Look at the queue.");
    let first = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 30, |e| architect_work_done(e, first)),
        "{}",
        said.borrow().join("\n")
    );
    say(&mut e, "Look again.");
    let second = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 30, |e| architect_work_done(e, second)),
        "{}",
        said.borrow().join("\n")
    );
    let log = said.borrow().join("\n");
    assert_eq!(
        path_of(&e, second),
        ["S1", "Q0", "Q2", "H8", "B1", "B4", "Q0", "E1", "E2"],
        "{log}"
    );
    let sessions: Vec<(String, String)> = {
        let mut st = e
            .db
            .conn()
            .prepare(
                "SELECT started, cli_session FROM sessions WHERE line = 'architect' AND work IN (?1, ?2) ORDER BY id",
            )
            .unwrap();
        st.query_map([first, second], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(|x| x.unwrap())
            .collect()
    };
    assert_eq!(sessions.len(), 2, "{sessions:?}");
    assert_eq!(sessions[1].0, "carried");
    assert_eq!(sessions[0].1, sessions[1].1);
    let _ = std::fs::remove_dir_all(&s.root);
}

// An engineer's question to its project's architect is a matter of its queue, which starts a work of the architect; the
// engineer's work waits at its question, holding no slot. The architect finds the crux, weighs the ways and replies,
// and the reply takes the work on from there unchanged.
#[test]
fn an_engineer_s_question_is_answered_through_its_crux_and_ways() {
    let s = setup("question");
    let mut agents = scripts();
    agents.insert(
        "R1",
        Box::new(|_: &Request, n: usize| {
            if n == 1 {
                return (
                    vec![
                        ("outcome", "ask"),
                        ("outcome.ask", "architect"),
                        ("outcome.ask.points[].kind", "split"),
                    ],
                    vec![(
                        "outcome.ask.points[].what",
                        json!("The work holds two separate changes."),
                    )],
                );
            }
            (Vec::new(), Vec::new())
        }),
    );
    let reply = json!({ "text": "Keep it one work.", "why": "Both changes share their tests." });
    let given = reply.clone();
    agents.insert(
        "H3",
        Box::new(move |_: &Request, _: usize| (Vec::new(), vec![("outcome.reply", given.clone())])),
    );
    let (mut e, said, item) = start(&s, agents);
    let question = |e: &Engine| -> Option<(Option<String>, Option<i64>)> {
        e.db.conn()
            .query_row(
                "SELECT r.output, r.ended_at FROM rounds r JOIN works w ON w.id = r.work
                 WHERE w.item = ?1 AND r.cell = 'A2' ORDER BY r.n DESC LIMIT 1",
                [item],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .ok()
    };
    assert!(steps_until(&mut e, 10, |e| question(e).is_some()));
    let log = said.borrow().join("\n");
    assert_eq!(matters(&e), [("engineer_question".to_string(), true, false)], "{log}");
    let (detail, project): (String, String) =
        e.db.conn()
            .query_row(
                "SELECT detail, project FROM architect_matters WHERE kind = 'engineer_question'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
    assert_eq!((detail.as_str(), project.as_str()), ("split", "p"));
    assert_eq!(e.free_slots().unwrap(), 1, "{log}");
    let started = e.step(None).unwrap();
    let arch = started.work.unwrap();
    let replied = steps_until(&mut e, 60, |e| {
        question(e).is_some_and(|q| q.1.is_some()) && architect_work_done(e, arch)
    });
    let log = said.borrow().join("\n");
    assert!(replied, "{log}");
    let path = path_of(&e, arch);
    let at = path.iter().position(|c| c == "H2").expect(&log);
    assert_eq!(&path[at - 1..at + 3], ["Q2", "H2", "H3", "B1"], "{log}");
    // The ways were given the crux by its address in the session, not again.
    let ways = message_at(&e, arch, "H3", false);
    assert!(ways.contains("·H2·crux"), "{ways}");
    assert!(ways.contains("What is asked is whether to split the work"), "{ways}");
    let (output, _) = question(&e).unwrap();
    let output: Value = serde_json::from_str(&output.unwrap()).unwrap();
    assert_eq!(output, json!({ "reply": reply }));
    let (work, _, _) = state(&e, item).unwrap();
    let (took, next): (String, String) =
        e.db.conn()
            .query_row(
                "SELECT q.transition, r.cell FROM rounds q JOIN rounds r ON r.work = q.work AND r.n = q.n + 1
                 WHERE q.work = ?1 AND q.cell = 'A2'",
                [work],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
    assert_eq!((took.as_str(), next.as_str()), ("answered", "A4"), "{log}");
    assert_eq!(matters(&e), [("engineer_question".to_string(), false, true)], "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The review of a batch finds an item change to fix: the revision hands in the whole set again, the review looks at it
// again, and the revised change is made; what was handed in first is kept as revised, not made.
#[test]
fn an_item_change_the_review_rejects_is_revised_and_then_made() {
    let s = setup("revise");
    let mut agents = scripts();
    agents.insert(
        "H8",
        Box::new(|_: &Request, _: usize| {
            let change = json!({ "kind": "edit_item", "item": "1", "part": "title", "text": "parser", "reason": "r" });
            (
                Vec::new(),
                vec![("outcome.effects", json!({ "item_changes": [change] }))],
            )
        }),
    );
    agents.insert(
        "B2",
        Box::new(|_: &Request, n: usize| {
            let row = if n == 1 {
                json!({ "change": 1, "verdict": "fix", "about": "constitution_and_policy",
                        "what": "The title is no action.", "how": "Write it as one." })
            } else {
                json!({ "change": 1, "verdict": "ok", "why": "Now it is an action." })
            };
            (Vec::new(), vec![("outcome.review", json!([row]))])
        }),
    );
    let path = config::database(&s.data);
    agents.insert(
        "B3",
        Box::new(move |_: &Request, _: usize| {
            let c = db::open_read(&path).unwrap();
            let matter: i64 = c
                .query_row(
                    "SELECT matter FROM architect_item_changes WHERE state = 'collected'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            let change = json!({ "kind": "edit_item", "item": "1", "part": "title", "text": "Write the parser",
                                 "reason": "An action, as the review asked." });
            (
                Vec::new(),
                vec![("outcome.changes", json!([{ "matter": matter, "change": change }]))],
            )
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    say(&mut e, "Name the parser's item.");
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, arch)),
        "{}",
        said.borrow().join("\n")
    );
    let log = said.borrow().join("\n");
    let path = path_of(&e, arch);
    let at = path.iter().position(|c| c == "B2").expect(&log);
    assert_eq!(&path[at..at + 4], ["B2", "B3", "B2", "B4"], "{log}");
    let title: String =
        e.db.conn()
            .query_row("SELECT title FROM items WHERE id = 1", [], |r| r.get(0))
            .unwrap();
    assert_eq!(title, "Write the parser", "{log}");
    let states: Vec<String> = {
        let mut st =
            e.db.conn()
                .prepare("SELECT state FROM architect_item_changes ORDER BY id")
                .unwrap();
        st.query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect()
    };
    assert_eq!(states, ["revised", "made"]);
    let _ = std::fs::remove_dir_all(&s.root);
}

// An ask of the owner from a cell on a matter's path holds only that matter: the architect's work goes on with the next
// and ends; the owner's answer puts the matter back in the queue, which starts a work of the architect, and it goes
// back to the cell that asked, which is given the answer.
#[test]
fn an_ask_holds_only_its_matter_and_the_answer_goes_back_to_the_cell_that_asked() {
    let s = setup("ask-holds");
    let mut agents = scripts();
    picks_first(&s, &mut agents);
    agents.insert(
        "H9",
        Box::new(|r: &Request, _: usize| {
            if r.prompt.contains("Hold the release?") && !r.prompt.contains("Yes, hold it.") {
                return (
                    vec![
                        ("outcome", "ask"),
                        ("outcome.ask", "owner"),
                        ("outcome.ask.points[].kind", "other"),
                    ],
                    vec![(
                        "outcome.ask.points[].what",
                        json!("Should the release wait for the parser?"),
                    )],
                );
            }
            (Vec::new(), Vec::new())
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    for text in ["Hold the release?", "Look at the queue."] {
        let m = db::Matter {
            project: "p".into(),
            kind: "other_architect".into(),
            detail: Some("message".into()),
            what: format!("The architect of q says: {text}"),
            body: json!({ "text": text }),
            ..db::Matter::default()
        };
        e.db.write(Actor::Engine, |tx| db::queue_matter(tx, &m)).unwrap();
    }
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, arch)),
        "{}",
        said.borrow().join("\n")
    );
    let log = said.borrow().join("\n");
    let path = path_of(&e, arch);
    let asked = path.iter().position(|c| c == "A3").expect(&log);
    assert_eq!(
        &path[asked - 1..asked + 6],
        ["H9", "A3", "B1", "Q0", "Q2", "H9", "B1"],
        "{log}"
    );
    assert_eq!(cells_of(&e, arch)[asked].1.as_deref(), Some("held"));
    // The held matter is out of the queue, not handled, waiting for the ask; the other is handled.
    let held: (i64, Option<i64>, Option<String>, Option<i64>) =
        e.db.conn()
            .query_row(
                "SELECT id, asked, resume_cell, handled_round FROM architect_matters WHERE what LIKE '%release%'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
    let ask = held.1.expect(&log);
    assert_eq!((held.2.as_deref(), held.3), (Some("H9"), None), "{log}");
    assert_eq!(
        matters(&e),
        [
            ("other_architect".to_string(), false, false),
            ("other_architect".to_string(), false, true)
        ],
        "{log}"
    );
    // The owner answers: the matter is back, an architect work starts, and the matter goes back to the cell that asked.
    let yes = api::Answer {
        text: "Yes, hold it.".into(),
        why: None,
        reply: None,
        choice: None,
        to: None,
    };
    api::answer(&mut e.db, ask, &yes).unwrap();
    let again = e.step(None).unwrap().work.unwrap();
    assert_ne!(again, arch);
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, again)),
        "{}",
        said.borrow().join("\n")
    );
    let log = said.borrow().join("\n");
    let path = path_of(&e, again);
    let at = path.iter().position(|c| c == "Q2").expect(&log);
    assert_eq!(&path[at..at + 3], ["Q2", "H9", "B1"], "{log}");
    assert_eq!(cells_of(&e, again)[at].1.as_deref(), Some("to_answer_architect"));
    let answered = message_at(&e, again, "H9", false);
    assert!(answered.contains("Yes, hold it."), "{answered}");
    let handled: Option<i64> =
        e.db.conn()
            .query_row(
                "SELECT handled_work FROM architect_matters WHERE id = ?1",
                [held.0],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(handled, Some(again), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The owner's words are a talk the architect takes first: once its wait for the owner's reply runs out, the talk goes
// back to the queue, held there, and the architect goes on; the owner's next words make it due again, first, and its
// next turn is given them in the session that holds the talk, the words taken into the talk; only the owner ends it,
// here while it is held, and its last turn hands in what it came to.
#[test]
fn a_talk_whose_wait_runs_out_is_held_until_the_owner_says_more() {
    let s = setup("talk-held");
    let install = std::fs::read_to_string(s.data.join("install.toml")).unwrap();
    write(
        &s.data,
        "install.toml",
        &format!("{install}\n[agents]\nowner_reply_wait = 0\n"),
    );
    git(&s.data, &["commit", "-q", "-am", "no wait for the owner's reply"]);
    let (mut e, said) = engine(&s, scripts());
    quiet(&mut e);
    an_item(&mut e);
    db::say_to_architect(&mut e.db, "p", None, None, "Should the parser come first?").unwrap();
    let first = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, first)),
        "{}",
        said.borrow().join("\n")
    );
    let log = said.borrow().join("\n");
    let path = path_of(&e, first);
    let at = path.iter().position(|c| c == "H8").expect(&log);
    assert_eq!(&path[at..at + 3], ["H8", "H11", "B1"], "{log}");
    assert_eq!(cells_of(&e, first)[at + 1].1.as_deref(), Some("no_reply"), "{log}");
    let talk: (i64, Option<i64>, Option<i64>) =
        e.db.conn()
            .query_row(
                "SELECT id, talk_held, handled_round FROM architect_matters WHERE kind = 'owner_words'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
    assert!(talk.1.is_some() && talk.2.is_none(), "{talk:?}");
    assert_eq!(matters(&e), [("owner_words".to_string(), false, false)], "{log}");
    assert_eq!(e.step(None).unwrap().work, None, "{log}");
    // The owner says more: the talk is due again, first, its next turn given the new words.
    db::say_to_architect(&mut e.db, "p", None, None, "And the tests?").unwrap();
    let second = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, second)),
        "{}",
        said.borrow().join("\n")
    );
    let log = said.borrow().join("\n");
    assert_eq!(path_of(&e, second)[..5], ["S1", "Q0", "Q2", "H8", "H11"], "{log}");
    assert!(message_at(&e, second, "H8", false).contains("And the tests?"), "{log}");
    assert_eq!(
        matters(&e),
        [
            ("owner_words".to_string(), false, false),
            ("owner_words".to_string(), false, true)
        ],
        "{log}"
    );
    // The owner ends the talk while it is held: its last turn hands in what it came to.
    api::end_talk(&mut e.db, "p").unwrap();
    let third = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, third)),
        "{}",
        said.borrow().join("\n")
    );
    let log = said.borrow().join("\n");
    assert_eq!(rows_at(&e, third, "H8"), ["done"], "{log}");
    assert!(
        message_at(&e, third, "H8", false).contains("The owner ended the talk"),
        "{log}"
    );
    assert!(matters(&e).iter().all(|m| m.2), "{log}");
    assert!(api::end_talk(&mut e.db, "p").is_err());
    let _ = std::fs::remove_dir_all(&s.root);
}

// A cell on a matter's path has the project's open observations searched, by the observer at its own level and line,
// and goes on with the candidates: here a report taken in is merged into the observation found, reported once more.
#[test]
fn the_search_of_observations_hands_back_candidates() {
    let s = setup("search");
    let install = std::fs::read_to_string(s.data.join("install.toml")).unwrap();
    let levels = "[agents.levels.cheap]\ndesc = \"searching\"\ncli = \"claude\"\nmodel = \"m-cheap\"\n\
                  [agents]\nobserver = \"cheap\"\n";
    write(&s.data, "install.toml", &format!("{install}\n{levels}"));
    git(&s.data, &["commit", "-q", "-am", "the observer's level"]);
    let mut agents = scripts();
    agents.insert(
        "H5",
        Box::new(|_: &Request, n: usize| {
            if n == 1 {
                let search = json!({ "look_for": "A slow parse of large files.", "tags": [], "purpose": "same" });
                return (vec![("outcome", "search")], vec![("outcome.search", search)]);
            }
            let merge = json!({ "kind": "merge", "into": 1, "seen": "Seen again in the parser.", "refs": [],
                                "reason": "The same slowness." });
            (
                vec![("outcome.take.way", "merged")],
                vec![("outcome.effects", json!({ "observation_changes": [merge] }))],
            )
        }),
    );
    agents.insert(
        "X2",
        Box::new(|_: &Request, _: usize| {
            (
                Vec::new(),
                vec![(
                    "outcome.candidates",
                    json!([{ "id": 1, "why": "It records the same slowness." }]),
                )],
            )
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    e.db.write(Actor::Architect, |tx| {
        tx.execute(
            "INSERT INTO observations (project, what, source) VALUES ('p', 'Parsing large files is slow.', 'A report.')",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    // A side report of a work of the project, as the engine queues one.
    e.db.write(Actor::Engine, |tx| {
        let m = db::Matter {
            project: "p".into(),
            kind: "report".into(),
            detail: Some("other".into()),
            what: "A side report: the parser is slow on large files.".into(),
            body: json!({ "report": { "kind": "other", "what": "The parser is slow on large files.", "urgent": false } }),
            item: Some(1),
            raised: None,
        };
        db::queue_matter(tx, &m)
    })
    .unwrap();
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, arch)),
        "{}",
        said.borrow().join("\n")
    );
    let log = said.borrow().join("\n");
    let path = path_of(&e, arch);
    let at = path.iter().position(|c| c == "X2").expect(&log);
    assert_eq!(&path[at - 2..at + 3], ["H4", "H5", "X2", "H5", "B1"], "{log}");
    let (line, model): (String, String) =
        e.db.conn()
            .query_row(
                "SELECT s.line, r.model FROM rounds r JOIN sessions s ON s.id = r.session
                 WHERE r.work = ?1 AND r.cell = 'X2'",
                [arch],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
    assert_eq!((line.as_str(), model.as_str()), ("observer", "m-cheap"));
    let back = message_at(&e, arch, "H5", true);
    assert!(back.contains("It records the same slowness."), "{back}");
    // The observer writes what the owner reads in the owner's language, as every line does.
    let search = message_at(&e, arch, "X2", false);
    assert!(search.contains("`owner.language` (English)"), "{search}");
    let times: (i64, String) =
        e.db.conn()
            .query_row("SELECT times_reported, refs FROM observations WHERE id = 1", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
    assert_eq!(times.0, 2, "{log}");
    assert!(times.1.contains("Seen again in the parser."), "{}", times.1);
    let _ = std::fs::remove_dir_all(&s.root);
}

// When the architect judges the policy may no longer govern, it sets a new one in its own work and session: new works
// of the project are held, it studies the direction and puts the study before the owner as an ask that holds only the
// setting's matter, and goes on with its queue meanwhile, here the owner's words. The owner says the policy stays: the
// matter comes back with the answer to the cell that put the ask, which ends the setting, lets new works start again
// and goes on to what to do next.
#[test]
fn a_policy_set_again_is_studied_in_the_architect_s_work_and_the_owner_says_it_stays() {
    let s = setup("policy-stays");
    let mut agents = scripts();
    agents.insert(
        "P1",
        Box::new(|_: &Request, n: usize| {
            if n == 1 {
                return (vec![("outcome.verdict", "set_again")], Vec::new());
            }
            (Vec::new(), Vec::new())
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    say(&mut e, "Is the policy still right?");
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 40, |e| study_asked(e).is_some()),
        "{}",
        said.borrow().join("\n")
    );
    let log = said.borrow().join("\n");
    assert_eq!(path_of(&e, arch)[..5], ["S1", "U1", "P1", "P2", "P3"], "{log}");
    assert_eq!(holds(&e), [("p".to_string(), "policy".to_string())]);
    // The ask holds the setting's matter, out of the queue until the answer comes, which goes back to the cell that put
    // it; the work of the architect stops on no blocker.
    let m = setting_matter(&e);
    let ask = study_asked(&e);
    let held: (Option<i64>, Option<String>) =
        e.db.conn()
            .query_row(
                "SELECT asked, resume_cell FROM architect_matters WHERE id = ?1",
                [m],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
    assert_eq!((held.0, held.1.as_deref()), (ask, Some("P3")), "{log}");
    assert_eq!(open_blocker(&e, arch), None, "{log}");
    // The architect goes on with its queue meanwhile: the owner's words, and the work ends.
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, arch)),
        "{}",
        said.borrow().join("\n")
    );
    let log = said.borrow().join("\n");
    assert_eq!(rows_at(&e, arch, "H8"), ["done"], "{log}");
    let stays = api::Answer {
        text: "Keep it.".into(),
        why: None,
        reply: None,
        choice: Some("policy_stays".into()),
        to: None,
    };
    api::answer(&mut e.db, ask.unwrap(), &stays).unwrap();
    let settled = move |e: &mut Engine| -> bool {
        e.db.conn()
            .query_row(
                "SELECT handled_round IS NOT NULL FROM architect_matters WHERE id = ?1",
                [m],
                |r| r.get(0),
            )
            .unwrap()
    };
    assert!(steps_until(&mut e, 40, settled), "{}", said.borrow().join("\n"));
    let log = said.borrow().join("\n");
    assert_eq!(setting_rows(&e, "P3"), ["held", "policy_stays"], "{log}");
    assert_eq!(holds(&e), Vec::<(String, String)>::new(), "{log}");
    // A new work of the architect took the matter back to the study's ask, and thought what to do next after it.
    let (work, n): (i64, i64) =
        e.db.conn()
            .query_row(
                "SELECT handled_work, handled_round FROM architect_matters WHERE id = ?1",
                [m],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
    assert!(work > arch, "{log}");
    assert!(
        steps_until(&mut e, 5, |e| path_of(e, work).len() > n as usize),
        "{}",
        said.borrow().join("\n")
    );
    let log = said.borrow().join("\n");
    let after: Vec<String> = cells_of(&e, work)
        .into_iter()
        .skip(n as usize - 1)
        .map(|c| c.0)
        .take(2)
        .collect();
    assert_eq!(after, ["P3", "N1"], "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

/// What stops project p's new works from starting, by project and reason.
fn holds(e: &Engine) -> Vec<(String, String)> {
    let mut st =
        e.db.conn()
            .prepare("SELECT project, reason FROM dispatch_holds ORDER BY reason")
            .unwrap();
    st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(|x| x.unwrap())
        .collect()
}

// A new policy set in the architect's works takes effect in the session it was studied and written in: the owner's view
// of the study and approval of the text come back with the setting's matter to the cells that put them, new architects
// read the text, each in a session of its own, the approved text becomes the policy in force, new works start again,
// and what to do next under it follows in the same session, which a new policy ends no longer.
#[test]
fn a_new_policy_takes_effect_and_the_architect_goes_on_in_its_session() {
    let s = setup("policy-next");
    let mut agents = scripts();
    agents.insert(
        "P1",
        Box::new(|_: &Request, n: usize| {
            if n == 1 {
                return (vec![("outcome.verdict", "set_again")], Vec::new());
            }
            (Vec::new(), Vec::new())
        }),
    );
    agents.insert("P4", writes_a_policy());
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    say(&mut e, "Is the policy still right?");
    let first = e.step(None).unwrap().work.unwrap();
    // The owner answers once the architect's work went on with its queue to its end, so that the setting's matter is
    // the one matter that comes back.
    assert!(
        steps_until(&mut e, 40, |e| study_asked(e).is_some()
            && architect_work_done(e, first)),
        "{}",
        said.borrow().join("\n")
    );
    let answer = |choice: &str| api::Answer {
        text: "Fine.".into(),
        why: None,
        reply: None,
        choice: Some(choice.into()),
        to: None,
    };
    let ask = study_asked(&e).unwrap();
    api::answer(&mut e.db, ask, &answer("satisfied")).unwrap();
    assert!(
        steps_until(&mut e, 120, |e| approval_asked(e).is_some()),
        "{}",
        said.borrow().join("\n")
    );
    let ask = approval_asked(&e).unwrap();
    api::answer(&mut e.db, ask, &answer("approved")).unwrap();
    let m = setting_matter(&e);
    let settled = move |e: &mut Engine| -> bool {
        let n: Option<i64> =
            e.db.conn()
                .query_row("SELECT handled_round FROM architect_matters WHERE id = ?1", [m], |r| {
                    r.get(0)
                })
                .unwrap();
        n.is_some()
    };
    assert!(steps_until(&mut e, 40, settled), "{}", said.borrow().join("\n"));
    // The work that ended the setting goes on to what to do next.
    let (work, n): (i64, i64) =
        e.db.conn()
            .query_row(
                "SELECT handled_work, handled_round FROM architect_matters WHERE id = ?1",
                [m],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
    assert!(
        steps_until(&mut e, 5, |e| path_of(e, work).len() > n as usize),
        "{}",
        said.borrow().join("\n")
    );
    let log = said.borrow().join("\n");
    assert_eq!(setting_rows(&e, "P3"), ["held", "satisfied"], "{log}");
    assert_eq!(setting_rows(&e, "P11"), ["held", "approved"], "{log}");
    assert_eq!(setting_rows(&e, "P12"), ["next"], "{log}");
    let policy: Value =
        e.db.conn()
            .query_row("SELECT policy FROM policy_current WHERE project = 'p'", [], |r| {
                r.get::<_, String>(0)
            })
            .map(|p| serde_json::from_str(&p).unwrap())
            .unwrap();
    assert_eq!(policy["direction"], "Ship the parser.", "{log}");
    assert_eq!(holds(&e), Vec::<(String, String)>::new(), "{log}");
    // From the study to what to do next under the new policy, every round of the architect's line went on in one
    // session, across the works the setting went on in; the project was understood once.
    let sessions: Vec<(String, String)> = {
        let mut st =
            e.db.conn()
                .prepare(
                    "SELECT r.cell, s.cli_session FROM rounds r JOIN works w ON w.id = r.work
                     JOIN sessions s ON s.id = r.session
                     WHERE w.graph = 'architect' AND w.project = 'p' AND s.line = 'architect' AND r.work >= ?1
                     ORDER BY r.work, r.n",
                )
                .unwrap();
        st.query_map([first], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(|x| x.unwrap())
            .collect()
    };
    let cells: Vec<&str> = sessions.iter().map(|x| x.0.as_str()).collect();
    assert_eq!(cells.iter().filter(|c| **c == "U1").count(), 1, "{cells:?}");
    let p12 = cells.iter().rposition(|c| *c == "N1").expect(&log);
    assert!(
        cells[..p12].contains(&"P2") && cells[..p12].contains(&"P10"),
        "{cells:?}"
    );
    assert!(sessions.iter().all(|x| x.1 == sessions[0].1), "{sessions:?}");
    // The new architects that read it were each in a session of their own.
    let readers: i64 =
        e.db.conn()
            .query_row(
                "SELECT count(DISTINCT s.cli_session) FROM rounds r JOIN sessions s ON s.id = r.session
                 WHERE r.cell = 'P6'",
                [],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(readers, 2, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

/// What an agent's round of an architect saw as it ran: its request, and in its working directory the text of
/// `src/lib.txt`, the commit it stood at and whether a file left there earlier was still there.
struct Saw {
    r: Request,
    lib: String,
    head: String,
    left: bool,
}

/// Scripts that keep what the rounds of the cells named saw; the study of a policy asks gh at its first round.
fn keep_what_they_saw(agents: &mut BTreeMap<&'static str, Script>, cells: &[&'static str]) -> Rc<RefCell<Vec<Saw>>> {
    let seen = Rc::new(RefCell::new(Vec::<Saw>::new()));
    for &cell in cells {
        let kept = seen.clone();
        let then = agents.remove(cell);
        agents.insert(
            cell,
            Box::new(move |r: &Request, n: usize| {
                let wt = &r.worktree;
                kept.borrow_mut().push(Saw {
                    r: r.clone(),
                    lib: std::fs::read_to_string(wt.join("src/lib.txt")).unwrap_or_default(),
                    head: if wt.join(".git").exists() {
                        git(wt, &["rev-parse", "HEAD"])
                    } else {
                        String::new()
                    },
                    left: wt.join("left.txt").exists(),
                });
                if r.cell == "P2" && n == 1 {
                    let tools = json!([{ "tool": "gh", "command": "pr view 1" }]);
                    return (vec![("outcome", "tools")], vec![("outcome.tools", tools)]);
                }
                then.as_ref().map(|f| f(r, n)).unwrap_or_default()
            }),
        );
    }
    seen
}

// Every line of a project's architect reads the project in the one worktree of its own, detached at main and brought to
// the latest main before each round, whatever was changed there dropped: the setting of a new policy too, which edits
// nothing and changes no repository; gh for a work on no item runs in that worktree, where gh knows the repository.
#[test]
fn an_architect_s_lines_read_the_latest_main_in_a_worktree_of_its_own() {
    let s = setup("architect-worktrees");
    let mut agents = scripts();
    agents.insert(
        "P1",
        Box::new(|_: &Request, _: usize| (vec![("outcome.verdict", "set_again")], Vec::new())),
    );
    let seen = keep_what_they_saw(&mut agents, &["U1", "P2"]);
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    let arch = architect_work_at(&mut e, "U1");
    let worktrees = s.data.join("worktrees");
    let architect = worktrees.join("architect").join("p");
    main_moves_on(&s.root, &[("src/lib.txt", "one\n")], "One");
    for _ in 0..5 {
        if seen.borrow().len() >= 2 {
            break;
        }
        e.advance(arch).unwrap();
    }
    let main = git(&s.origin, &["rev-parse", "main"]);
    {
        let seen = seen.borrow();
        let log = said.borrow().join("\n");
        assert_eq!(seen.len(), 2, "{log}");
        let (u1, p2) = (&seen[0], &seen[1]);
        assert_eq!((u1.r.cell.as_str(), p2.r.cell.as_str()), ("U1", "P2"), "{log}");
        for saw in [u1, p2] {
            assert_eq!(saw.r.worktree, architect);
            assert_eq!((saw.lib.as_str(), saw.head.as_str()), ("one\n", main.as_str()));
            // The project's checkout is no longer a directory added beside where it runs.
            assert_eq!(saw.r.readable.len(), 2, "{:?}", saw.r.readable);
        }
        for t in config::EDIT_TOOLS {
            assert!(p2.r.refused.iter().any(|x| x == t), "{t}: {:?}", p2.r.refused);
        }
        for g in config::GIT_CHANGES {
            assert!(p2.r.refused.contains(&format!("Bash(git {g} *)")), "git {g}");
        }
        // The session is told where it works at its first round, and the study, in the same session, is pointed to
        // it.
        assert!(
            u1.r.prompt.contains(&u1.r.worktree.display().to_string()),
            "{}",
            u1.r.prompt
        );
        assert!(p2.r.prompt.contains("Worktree: work:architect"), "{}", p2.r.prompt);
    }
    assert!(!worktrees.join("policy").exists());
    e.advance(arch).unwrap();
    let ran: Value =
        e.db.conn()
            .query_row(
                "SELECT output FROM rounds WHERE work = ?1 AND cell = 'X1' AND transition IS NOT NULL",
                [arch],
                |r| r.get::<_, String>(0),
            )
            .map(|o| serde_json::from_str(&o).unwrap())
            .unwrap();
    assert_eq!(
        ran["results"][0]["ran"]["output"],
        format!("pr view 1 in {}", architect.display()),
        "{ran}"
    );
    // What is changed in the worktree is dropped as it is brought to the latest main for the next round.
    write(&architect, "src/lib.txt", "changed\n");
    write(&architect, "left.txt", "left\n");
    main_moves_on(&s.root, &[("src/lib.txt", "two\n")], "Two");
    for _ in 0..5 {
        if seen.borrow().len() > 2 {
            break;
        }
        e.advance(arch).unwrap();
    }
    let seen = seen.borrow();
    let log = said.borrow().join("\n");
    let next = seen.get(2).expect(&log);
    assert_eq!(next.r.worktree, architect, "{log}");
    assert_eq!(
        (next.lib.as_str(), next.head.as_str(), next.left),
        ("two\n", git(&s.origin, &["rev-parse", "main"]).as_str(), false),
        "{log}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// The new architect that reads a policy knows only what it is given: it runs in an empty directory, is told of no
// worktree, and edits nothing, as the architect's line.
#[test]
fn the_new_architect_that_reads_a_policy_runs_in_an_empty_directory() {
    let s = setup("reader-directory");
    let mut agents = scripts();
    agents.insert("P4", writes_a_policy());
    let seen = keep_what_they_saw(&mut agents, &["P6"]);
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    let setting = setting_at(&mut e, "P4");
    for _ in 0..30 {
        if !seen.borrow().is_empty() {
            break;
        }
        e.step_work(setting).unwrap();
    }
    let seen = seen.borrow();
    let log = said.borrow().join("\n");
    let p6 = seen.first().expect(&log);
    let dir = s.data.join("worktrees").join("P6").join("p");
    assert_eq!(p6.r.worktree, dir, "{log}");
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0);
    assert!(!p6.r.prompt.contains("Worktree:"), "{}", p6.r.prompt);
    assert!(p6.r.refused.iter().any(|x| x == "Edit"), "{:?}", p6.r.refused);
    let _ = std::fs::remove_dir_all(&s.root);
}

// Each project has its architect: a message from project p's architect is a matter of project q's architect's queue,
// which starts a work of q's architect of its own; q's answer comes back to p's queue as a message.
#[test]
fn two_projects_architects_send_each_other_messages() {
    let s = setup("two-architects");
    let q = s.root.join("q-checkout");
    git(
        &s.root,
        &["clone", "-q", &s.origin.to_string_lossy(), &q.to_string_lossy()],
    );
    let p_file = std::fs::read_to_string(s.data.join("projects/p.toml")).unwrap();
    let checkout = s.root.join("checkout");
    write(
        &s.data,
        "projects/q.toml",
        &p_file
            .replace("o/p", "o/q")
            .replace(&checkout.display().to_string(), &q.display().to_string()),
    );
    git(&s.data, &["add", "-A"]);
    git(&s.data, &["commit", "-q", "-m", "a second project"]);
    let mut agents = scripts();
    agents.insert(
        "H8",
        Box::new(|_: &Request, _: usize| {
            let message = json!({ "project": "q", "text": "Does q still need the old format?" });
            (Vec::new(), vec![("outcome.effects", json!({ "messages": [message] }))])
        }),
    );
    agents.insert(
        "H9",
        Box::new(|r: &Request, _: usize| {
            if project_of(r) == "q" {
                return (Vec::new(), vec![("outcome.answer.reply", json!("No, drop it."))]);
            }
            (Vec::new(), Vec::new())
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    e.db.write(Actor::Architect, |tx| {
        tx.execute(
            "INSERT INTO dispatch_holds (project, reason, since) VALUES ('q', 'architect', 1)",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    say(&mut e, "Ask q about the old format.");
    let projects = |e: &Engine| -> Vec<(i64, String)> {
        let mut st =
            e.db.conn()
                .prepare("SELECT id, project FROM works WHERE graph = 'architect' AND ended IS NULL ORDER BY id")
                .unwrap();
        st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(|x| x.unwrap())
            .collect()
    };
    let done_all = |e: &mut Engine| {
        let ws = projects(e);
        ws.len() >= 3 && ws.iter().all(|(w, _)| architect_work_done(e, *w))
    };
    assert!(steps_until(&mut e, 120, done_all), "{}", said.borrow().join("\n"));
    let log = said.borrow().join("\n");
    let ws = projects(&e);
    let names: Vec<&str> = ws.iter().map(|w| w.1.as_str()).collect();
    assert_eq!(names, ["p", "q", "p"], "{log}");
    assert!(path_of(&e, ws[1].0).contains(&"H9".to_string()), "{log}");
    let messages: Vec<(String, String, String)> = {
        let mut st =
            e.db.conn()
                .prepare(
                    "SELECT project, detail, body ->> 'from' FROM architect_matters WHERE kind = 'other_architect'
                     ORDER BY id",
                )
                .unwrap();
        st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap()
            .map(|x| x.unwrap())
            .collect()
    };
    assert_eq!(
        messages,
        [
            ("q".to_string(), "message".to_string(), "p".to_string()),
            ("p".to_string(), "reply".to_string(), "q".to_string())
        ],
        "{log}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// Each side report is a matter of the kind report in its project's architect's queue, queued once, which starts a work
// of the architect; the queue marks one urgent.
#[test]
fn side_reports_come_to_the_architect_as_reports() {
    let s = setup("side-reports");
    let mut agents = scripts();
    let reporting = |r: Value| -> Script {
        Box::new(move |_: &Request, n: usize| {
            let reports = if n == 1 { json!([r.clone()]) } else { json!([]) };
            (Vec::new(), vec![("side_reports", reports)])
        })
    };
    agents.insert(
        "R1",
        reporting(json!({ "kind": "duplicate", "what": "Item 7 does the same.", "urgent": true })),
    );
    let (mut e, said, _) = start(&s, agents);
    let work = e.step(None).unwrap().work;
    e.step(work).unwrap();
    let log = said.borrow().join("\n");
    assert_eq!(matters(&e), [("report".to_string(), true, false)], "{log}");
    let (detail, urgent): (String, i64) =
        e.db.conn()
            .query_row("SELECT detail, urgent FROM architect_queue", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
    assert_eq!((detail.as_str(), urgent), ("duplicate", 1));
    let started = e.step(work).unwrap();
    let log = said.borrow().join("\n");
    assert_eq!(started.cell.as_deref(), Some("S1"), "{log}");
    let arch = started.work.unwrap();
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, arch)),
        "{}",
        said.borrow().join("\n")
    );
    let log = said.borrow().join("\n");
    assert!(path_of(&e, arch).contains(&"H4".to_string()), "{log}");
    assert_eq!(matters(&e), [("report".to_string(), false, true)], "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// What a round says was wrong with its cell is flow feedback: it joins its project's architect's flow feedback, one
// matter of the kind report that holds it by graph and cell, and starts no work of an architect; none adds nothing.
#[test]
fn flow_feedback_waits_for_the_architect_by_cell_and_starts_no_work_of_it() {
    let s = setup("flow-feedback");
    let mut agents = scripts();
    let saying = |text: &'static str| -> Script {
        Box::new(move |_: &Request, n: usize| {
            let said = if n == 1 { json!(text) } else { json!("none") };
            (Vec::new(), vec![("flow_feedback", said)])
        })
    };
    agents.insert("R1", saying("The problem and the why say the same."));
    agents.insert("R2", saying("No outcome fits a report."));
    let (mut e, said, item) = start(&s, agents);
    for _ in 0..4 {
        e.step(None).unwrap();
    }
    let log = said.borrow().join("\n");
    assert_eq!(ended_at(&e, item, "R2").len(), 1, "{log}");
    assert!(architect_works(&e).is_empty(), "{log}");
    assert_eq!(matters(&e), [("report".to_string(), true, false)], "{log}");
    let (detail, what, body): (String, String, String) =
        e.db.conn()
            .query_row(
                "SELECT detail, what, body FROM architect_matters WHERE kind = 'report'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
    assert_eq!(detail, "flow_feedback");
    assert_eq!(what, "Flow feedback of 2 rounds, at cells R1 (engineer), R2 (engineer)");
    let body: Value = serde_json::from_str(&body).unwrap();
    let by_cell = &body["feedback"]["engineer"];
    assert_eq!(by_cell.as_object().unwrap().len(), 2, "{body}");
    assert_eq!(by_cell["R1"][0]["said"], "The problem and the why say the same.");
    assert_eq!(by_cell["R1"][0]["item"], item);
    assert_eq!(by_cell["R2"][0]["said"], "No outcome fits a report.");
    let _ = std::fs::remove_dir_all(&s.root);
}

// Flow feedback one project's architect forwards to another's is marked as flow feedback: it waits in the other's queue
// and starts no work of an architect, as the project's own does.
#[test]
fn flow_feedback_forwarded_to_another_architect_starts_no_work_of_it() {
    let s = setup("forwarded-feedback");
    let q = s.root.join("q-checkout");
    git(
        &s.root,
        &["clone", "-q", &s.origin.to_string_lossy(), &q.to_string_lossy()],
    );
    let p_file = std::fs::read_to_string(s.data.join("projects/p.toml")).unwrap();
    let checkout = s.root.join("checkout");
    write(
        &s.data,
        "projects/q.toml",
        &p_file
            .replace("o/p", "o/q")
            .replace(&checkout.display().to_string(), &q.display().to_string()),
    );
    git(&s.data, &["add", "-A"]);
    git(&s.data, &["commit", "-q", "-m", "a second project"]);
    let mut agents = scripts();
    picks_first(&s, &mut agents);
    agents.insert(
        "H5",
        Box::new(|r: &Request, _: usize| {
            if project_of(r) != "p" {
                return (Vec::new(), Vec::new());
            }
            let message = json!({ "project": "q", "text": "Cell R1's problem and why say the same." });
            (
                vec![("outcome.take.way", "forwarded")],
                vec![("outcome.effects", json!({ "messages": [message] }))],
            )
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    let feedback =
        json!({ "feedback": { "engineer": { "R1": [{ "said": "The problem and the why say the same." }] } } });
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO dispatch_holds (project, reason, since) VALUES ('q', 'architect', 1)",
            [],
        )?;
        tx.execute(
            "INSERT INTO architect_matters (project, kind, detail, what, body, at)
             VALUES ('p', 'report', 'flow_feedback', 'Flow feedback of 1 round, at cell R1 (engineer)', ?1, 1)",
            [feedback.to_string()],
        )?;
        Ok(())
    })
    .unwrap();
    say(&mut e, "Look at the flow feedback too.");
    let p_done = |e: &mut Engine| match architect_works(e).first().copied() {
        Some(w) => architect_work_done(e, w),
        None => false,
    };
    assert!(steps_until(&mut e, 80, p_done), "{}", said.borrow().join("\n"));
    for _ in 0..5 {
        e.step(None).unwrap();
    }
    let log = said.borrow().join("\n");
    let started: Vec<String> = {
        let mut st =
            e.db.conn()
                .prepare(
                    "SELECT project FROM works WHERE graph = 'architect' AND ended IS NOT 'over before the test'
                     ORDER BY id",
                )
                .unwrap();
        st.query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect()
    };
    assert_eq!(started, ["p"], "{log}");
    let (detail, from): (String, String) = e
        .db
        .conn()
        .query_row(
            "SELECT detail, body ->> 'from' FROM architect_matters WHERE kind = 'other_architect' AND project = 'q'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!((detail.as_str(), from.as_str()), ("flow_feedback", "p"), "{log}");
    let waiting: i64 =
        e.db.conn()
            .query_row("SELECT count(*) FROM architect_queue WHERE project = 'q'", [], |r| {
                r.get(0)
            })
            .unwrap();
    assert_eq!(waiting, 1, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A project whose configuration turns flow feedback off is not asked for it: neither the cell's schema nor the command
// line's holds the field, and nothing comes to the architect.
#[test]
fn a_project_with_flow_feedback_off_is_not_asked_for_it() {
    let s = setup("flow-feedback-off");
    let project = std::fs::read_to_string(s.data.join("projects/p.toml")).unwrap();
    write(
        &s.data,
        "projects/p.toml",
        &project.replace("[ci]", "flow_feedback = false\n[ci]"),
    );
    git(&s.data, &["commit", "-q", "-am", "no flow feedback"]);
    let mut agents = scripts();
    agents.insert(
        "R1",
        Box::new(|_: &Request, _: usize| (Vec::new(), vec![("flow_feedback", json!("Said anyway."))])),
    );
    let (mut e, said, item) = start(&s, agents);
    let work = e.step(None).unwrap().work.unwrap();
    let w = e.work(work).unwrap();
    let g = e.graph_of(&w).unwrap();
    let subject = e.subject(&w).unwrap();
    let r1 = g.cell("R1").unwrap();
    assert!(r1.schema.as_ref().unwrap()["properties"].get("flow_feedback").is_none());
    let req = engine::cell_request(&e, &w, &g, r1, subject.as_ref()).unwrap();
    assert!(
        req.envelope["properties"].get("flow_feedback").is_none(),
        "{}",
        req.envelope
    );
    e.step(Some(work)).unwrap();
    let log = said.borrow().join("\n");
    assert_eq!(ended_at(&e, item, "R1"), [(Some("done".to_string()), None)], "{log}");
    assert!(matters(&e).is_empty(), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A design card a work hands in comes to its project's architect's queue. The architect does not rule on it: it looks
// at whether the card holds, and when it does not, the engine tells the work's engineer what is wrong. The card's
// rulings are the owner's alone.
#[test]
fn a_design_card_the_architect_finds_unsound_is_told_to_the_engineer() {
    let s = setup("architect-card");
    let mut agents = scripts();
    agents.insert(
        "I1",
        Box::new(|r: &Request, n: usize| {
            let wt = &r.worktree;
            if n == 1 {
                write(wt, "src/lib.txt", "old\nfeature\n");
                write(wt, "design/engine.md", "Decision: a new one.\n");
                git(wt, &["add", "-A"]);
                git(wt, &["commit", "-q", "-m", "Add the feature"]);
                let head = git(wt, &["rev-parse", "HEAD"]);
                let tools = json!([{ "tool": "ci", "head": head, "tests": ["feature"], "pr": { "title": "Add the feature", "body": "It adds it." } }]);
                return (vec![("outcome", "tools")], vec![("outcome.tools", tools)]);
            }
            let head = git(wt, &["rev-parse", "HEAD"]);
            (
                vec![("outcome.design_card", "")],
                vec![
                    ("outcome.head", json!(head)),
                    ("outcome.conditions", json!([{ "how": "tested", "id": "A1", "tests": ["feature"] }])),
                    ("outcome.design_card[].node", json!("design/engine.md")),
                ],
            )
        }),
    );
    let look = "Decision 1 chose an approach that does not hold in the whole: the parser already does this.";
    agents.insert(
        "H6",
        Box::new(move |_: &Request, _: usize| {
            (
                vec![("outcome.card", "unsound")],
                vec![("outcome.card.wrong", json!(look))],
            )
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    let told = |e: &mut Engine| -> bool {
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM interjections WHERE speaker = 'architect'",
                [],
                |r| r.get::<_, i64>(0),
            )
            .unwrap()
            > 0
    };
    assert!(steps_until(&mut e, 150, told), "{}", said.borrow().join("\n"));
    let log = said.borrow().join("\n");
    assert_eq!(matters(&e)[0].0, "design_card", "{log}");
    let (work, _, _) = state(&e, item).unwrap();
    let said_to: String =
        e.db.conn()
            .query_row(
                "SELECT text FROM interjections WHERE work = ?1 AND speaker = 'architect' AND line = 'engineer'",
                [work],
                |r| r.get(0),
            )
            .unwrap();
    assert!(said_to.ends_with(look), "{said_to}");
    let (status, rulings): (String, String) =
        e.db.conn()
            .query_row("SELECT status, rulings FROM design_cards", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
    assert_eq!((status.as_str(), rulings.as_str()), ("open", "[]"), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The architect pauses a work in its effects: the engine pauses it, with a reason that says the architect stopped it
// and why. Every work let go on, it stays stopped; the owner's Start of that work lets it go on, which is the owner's
// release of it: the architect is told in the owner's words.
#[test]
fn a_work_the_architect_stops_stays_stopped_through_start_all() {
    let s = setup("architect-stop");
    let mut agents = scripts();
    agents.insert(
        "H8",
        Box::new(|_: &Request, n: usize| {
            if n == 1 {
                let pause = json!({ "kind": "pause_work", "item": 1, "reason": "It waits for the parser." });
                return (Vec::new(), vec![("outcome.effects", json!({ "actions": [pause] }))]);
            }
            (Vec::new(), Vec::new())
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    let work = e.step(None).unwrap().work.unwrap();
    say(&mut e, "Stop the feature until the parser is in.");
    let started = e.step(Some(work)).unwrap();
    let log = said.borrow().join("\n");
    assert_eq!(started.cell.as_deref(), Some("S1"), "{log}");
    let arch = started.work.unwrap();
    let stopped = |e: &mut Engine| state(e, item).is_some_and(|x| x.2.is_some());
    assert!(steps_until(&mut e, 40, stopped), "{}", said.borrow().join("\n"));
    let why = format!("{}: It waits for the parser.", db::STOPPED_BY_ARCHITECT);
    assert_eq!(state(&e, item).unwrap().2, Some(why.clone()));
    assert_eq!(paused_by(&e, work).as_deref(), Some("engine"));
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, arch)),
        "{}",
        said.borrow().join("\n")
    );
    let n = rounds_count(&e);
    let log = said.borrow().join("\n");
    assert_eq!(e.step(Some(work)).unwrap().work, None, "{log}");
    assert_eq!((rounds_count(&e), state(&e, item).unwrap().2), (n, Some(why)), "{log}");
    assert_eq!(
        api::resume(&mut e.db, work).unwrap(),
        format!("work {work} goes on; the architect, which had stopped it, is told")
    );
    let told: String =
        e.db.conn()
            .query_row(
                "SELECT body ->> 'text' FROM architect_matters WHERE kind = 'owner_words' ORDER BY id DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(
        told,
        format!("I started work {work}, which you had stopped (It waits for the parser): it goes on.")
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// The architect runs at the level the installation names for it, and so do the review of its item changes and the new
// architect that reads a policy, on no line: their rounds run with the named level's model, not the top level's, at
// the effort their cell declares whatever the model, and record the level in their call.
#[test]
fn the_architect_runs_at_the_level_the_installation_names() {
    let s = setup("architect-level");
    let install = std::fs::read_to_string(s.data.join("install.toml")).unwrap();
    let levels = "[agents.levels.2]\ndesc = \"testing\"\ncli = \"claude\"\nmodel = \"m-two\"\n\
                  [agents.levels.4]\ndesc = \"running\"\ncli = \"claude\"\nmodel = \"m-four\"\n\
                  [agents]\narchitect = \"2\"\n";
    write(&s.data, "install.toml", &format!("{install}\n{levels}"));
    git(&s.data, &["commit", "-q", "-am", "the architect's level"]);
    let (mut e, said) = engine(&s, scripts());
    quiet(&mut e);
    let ran = |e: &Engine, work: i64, cell: &str| -> Option<(String, Value)> {
        let (model, input): (String, String) =
            e.db.conn()
                .query_row(
                    "SELECT model, input FROM rounds WHERE work = ?1 AND cell = ?2 ORDER BY n LIMIT 1",
                    params![work, cell],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .ok()?;
        let input = db::put_back(e.db.conn(), &serde_json::from_str(&input).unwrap());
        Some((model, input["call"].clone()))
    };
    let at_level = |model: &str, call: &Value| -> (String, Value, Option<String>, Option<String>) {
        let command: Vec<String> = call["command"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|a| a.as_str().map(str::to_string))
            .collect();
        let after = |flag: &str| {
            command
                .iter()
                .position(|a| a == flag)
                .and_then(|i| command.get(i + 1).cloned())
        };
        (
            model.to_string(),
            call["level"].clone(),
            after("--model"),
            after("--effort"),
        )
    };
    say(&mut e, "Look at the queue.");
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 10, |e| ran(e, arch, "U1").is_some()),
        "{}",
        said.borrow().join("\n")
    );
    let (model, call) = ran(&e, arch, "U1").unwrap();
    let want = (
        "m-two".to_string(),
        json!("2"),
        Some("m-two".to_string()),
        Some("high".to_string()),
    );
    assert_eq!(at_level(&model, &call), want);
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, arch)),
        "{}",
        said.borrow().join("\n")
    );
    let reading = setting_at(&mut e, "P6");
    e.step_work(reading).unwrap();
    let log = said.borrow().join("\n");
    let (model, call) = ran(&e, reading, "P6").expect(&log);
    assert_eq!(at_level(&model, &call), want, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// While slots are free and fewer of a project's items are ready to start than slots are free, here none, the engine
// queues a matter of slots free for its architect, which starts a work of it; once one is handled, no other comes for
// an hour.
#[test]
fn slots_free_with_nothing_ready_start_a_work_of_the_architect_at_most_once_an_hour() {
    let s = setup("slots-free");
    let (mut e, said) = engine(&s, scripts());
    let started = e.step(None).unwrap();
    let log = said.borrow().join("\n");
    assert_eq!(started.cell.as_deref(), Some("S1"), "{log}");
    let arch = started.work.unwrap();
    let body: String =
        e.db.conn()
            .query_row(
                "SELECT body FROM architect_matters WHERE kind = 'slots_free'",
                [],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(body, r#"{"free":1,"ready":0}"#);
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, arch)),
        "{}",
        said.borrow().join("\n")
    );
    assert!(path_of(&e, arch).contains(&"H10".to_string()));
    assert_eq!(e.step(None).unwrap().work, None);
    let count = |e: &Engine| -> i64 {
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM architect_matters WHERE kind = 'slots_free'",
                [],
                |r| r.get(0),
            )
            .unwrap()
    };
    assert_eq!(count(&e), 1);
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "UPDATE rounds SET ended_at = ended_at - 7200000
             WHERE (work, n) IN (SELECT handled_work, handled_round FROM architect_matters WHERE kind = 'slots_free')",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    let again = e.step(None).unwrap();
    let log = said.borrow().join("\n");
    assert_eq!(again.cell.as_deref(), Some("S1"), "{log}");
    assert_eq!(count(&e), 2);
    let _ = std::fs::remove_dir_all(&s.root);
}

// A hold of a project's new starts stops new works of it from starting from the queue, and the works running go on;
// once it is released, new works start again.
#[test]
fn a_dispatch_hold_stops_new_works_and_the_running_ones_go_on() {
    let s = setup("dispatch-hold");
    let install = std::fs::read_to_string(s.data.join("install.toml")).unwrap();
    write(&s.data, "install.toml", &install.replace("slots = 1", "slots = 2"));
    git(&s.data, &["commit", "-q", "-am", "two slots"]);
    let (mut e, said, first) = start(&s, scripts());
    let hold = |e: &mut Engine, on: bool| {
        e.db.write(Actor::Architect, |tx| {
            if on {
                tx.execute(
                    "INSERT INTO dispatch_holds (project, reason, since) VALUES ('p', 'architect', 1)",
                    [],
                )?;
            } else {
                tx.execute("DELETE FROM dispatch_holds", [])?;
            }
            Ok(())
        })
        .unwrap();
    };
    hold(&mut e, true);
    assert_eq!(e.step(None).unwrap().work, None);
    assert!(state(&e, first).is_none());
    hold(&mut e, false);
    let started = e.step(None).unwrap();
    assert_eq!(started.started_item, Some(first), "{}", said.borrow().join("\n"));
    let work = started.work;
    let second =
        e.db.write(Actor::Owner, |tx| {
            tx.execute(
                "INSERT INTO items (project, title, problem, why, deliver, level)
                 VALUES ('p', 'Second', 'x', 'y', 'change', 'l')",
                [],
            )?;
            let id = tx.last_insert_rowid();
            tx.execute("INSERT INTO dispatch_queue (item, position) VALUES (?1, 2.0)", [id])?;
            Ok(id)
        })
        .unwrap();
    hold(&mut e, true);
    let went_on = e.step(work).unwrap();
    let log = said.borrow().join("\n");
    assert_eq!((went_on.work, went_on.started_item), (work, None), "{log}");
    assert!(state(&e, second).is_none(), "{log}");
    hold(&mut e, false);
    assert_eq!(e.step(work).unwrap().started_item, Some(second));
    let _ = std::fs::remove_dir_all(&s.root);
}

// With every work paused, the owner steps a project's architect: the first step starts a work of the architect, paused
// by the owner, and runs its first round; each later step runs the work's next round, as a step of the work by its id
// does, and the work stays paused in the owner's hands. With nothing to start a work for, a step runs nothing.
#[test]
fn the_owner_steps_the_architect_one_round_at_a_time_with_every_work_paused() {
    let s = setup("step-architect");
    let (mut e, said) = engine(&s, scripts());
    quiet(&mut e);
    assert_eq!(e.step_architect("p").unwrap().work, None);
    say(&mut e, "Look at the queue.");
    let first = e.step_architect("p").unwrap();
    let log = said.borrow().join("\n");
    assert_eq!((first.cell.as_deref(), first.rounds.len()), (Some("S1"), 1), "{log}");
    let arch = first.work.unwrap();
    assert_eq!(paused_by(&e, arch).as_deref(), Some("owner"));
    assert_eq!(e.step(None).unwrap().work, None);
    let second = e.step_work(arch).unwrap();
    let log = said.borrow().join("\n");
    assert_eq!((second.cell.as_deref(), second.rounds.len()), (Some("U1"), 1), "{log}");
    let third = e.step_architect("p").unwrap();
    let log = said.borrow().join("\n");
    assert_eq!(
        (third.work, third.cell.as_deref(), third.rounds.len()),
        (Some(arch), Some("P1"), 1),
        "{log}"
    );
    assert_eq!(paused_by(&e, arch).as_deref(), Some("owner"));
    assert_eq!(architect_works(&e), [arch]);
    let _ = std::fs::remove_dir_all(&s.root);
}

// A work that delivered a report ends with its what to do next, which reaches its project's architect as a report of
// its own: the text as written, the report's paths under research/ and the commit the work merged; a work that handed
// in no what to do next queues none.
#[test]
fn a_report_s_what_to_do_next_reaches_the_architect_once_its_work_ends() {
    let s = setup("report-followup");
    let checkout = s.root.join("checkout");
    write(&checkout, "research/report.md", "# Report\nWhat was found.\n");
    git(&checkout, &["add", "-A"]);
    git(&checkout, &["commit", "-q", "-m", "the report"]);
    let merged = git(&checkout, &["rev-parse", "HEAD"]).trim().to_string();
    let (mut e, said) = engine(&s, scripts());
    quiet(&mut e);
    let at_end = |e: &mut Engine, followup: Option<&str>| -> i64 {
        e.db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO items (project, title, problem, why, deliver, level)
                 VALUES ('p', 'Study the parser', 'x', 'y', 'report', 'l')",
                [],
            )?;
            let item = tx.last_insert_rowid();
            let version: i64 = tx.query_row("SELECT max(id) FROM flow_versions", [], |r| r.get(0))?;
            tx.execute(
                "INSERT INTO works (item, flow_version, graph, at) VALUES (?1, ?2, 'engineer', 'E1')",
                params![item, version],
            )?;
            let w = tx.last_insert_rowid();
            let implemented = match followup {
                Some(f) => json!({ "outcome": { "followup": f } }),
                None => json!({ "outcome": {} }),
            };
            let merged_round = json!({ "result": { "verdict": "merged", "merge_commit": merged } });
            for (n, cell, output, row) in [(1, "I1", implemented, "done"), (2, "M2", merged_round, "merged")] {
                tx.execute(
                    "INSERT INTO rounds (work, n, cell, input, output, transition, config_commit, started_at, ended_at)
                     VALUES (?1, ?2, ?3, '{}', ?4, ?5, 'c', ?2, ?2)",
                    params![w, n, cell, output.to_string(), row],
                )?;
            }
            Ok(w)
        })
        .unwrap()
    };
    type Queued = (String, Option<i64>, Option<i64>, Option<i64>, Value);
    let followups = |e: &Engine| -> Vec<Queued> {
        let mut st =
            e.db.conn()
                .prepare(
                    "SELECT project, item, work, round, body FROM architect_matters
                     WHERE kind = 'report' AND detail = 'followup'",
                )
                .unwrap();
        st.query_map([], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                serde_json::from_str(&r.get::<_, String>(4)?).unwrap(),
            ))
        })
        .unwrap()
        .map(|x| x.unwrap())
        .collect()
    };
    let with = at_end(&mut e, Some("Open an item to measure the parser."));
    e.step_work(with).unwrap();
    let log = said.borrow().join("\n");
    let item: i64 =
        e.db.conn()
            .query_row("SELECT item FROM works WHERE id = ?1", [with], |r| r.get(0))
            .unwrap();
    assert_eq!(
        followups(&e),
        [(
            "p".to_string(),
            Some(item),
            Some(with),
            Some(1),
            json!({
                "followup": "Open an item to measure the parser.",
                "reports": ["research/report.md"],
                "commit": merged,
            })
        )],
        "{log}"
    );
    let without = at_end(&mut e, None);
    e.step_work(without).unwrap();
    assert_eq!(followups(&e).len(), 1, "{}", said.borrow().join("\n"));
    let _ = std::fs::remove_dir_all(&s.root);
}

/// What a keep-warm ping was given: the command line it ran, and its message.
type Replayed = (Vec<String>, String);

/// The sessions of a work's line, oldest first: why each started, whether it ended, and the cells of its rounds.
fn line_sessions(e: &Engine, work: i64, line: &str) -> Vec<(String, bool, String)> {
    let mut st =
        e.db.conn()
            .prepare(
                "SELECT s.started, s.ended IS NOT NULL, coalesce((SELECT group_concat(x.cell, ',') FROM
                   (SELECT r.cell FROM rounds r WHERE r.session = s.id ORDER BY r.n) x), '')
                 FROM sessions s WHERE s.work = ?1 AND s.line = ?2 ORDER BY s.id",
            )
            .unwrap();
    st.query_map(params![work, line], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .map(|x| x.unwrap())
        .collect()
}

/// When the engineer's latest round of a work ended: the session's last call.
fn last_call(e: &Engine, work: i64) -> i64 {
    e.db.conn()
        .query_row(
            "SELECT max(r.ended_at) FROM rounds r JOIN sessions s ON s.id = r.session
             WHERE s.work = ?1 AND s.line = 'engineer'",
            [work],
            |r| r.get(0),
        )
        .unwrap()
}

/// A work's keep-warm pings, oldest first: where it waited, and what each used.
fn pings_of(e: &Engine, work: i64) -> Vec<(Value, Option<Value>)> {
    engine::query_json(
        e.db.conn(),
        "SELECT p.waited, p.usage FROM pings p JOIN sessions s ON s.id = p.session WHERE s.work = ?1 ORDER BY p.id",
        &[&work],
        1000,
    )
    .unwrap()
    .into_iter()
    .map(|r| (r["waited"].clone(), Some(r["usage"].clone()).filter(|u| !u.is_null())))
    .collect()
}

/// What the CLI reports a round or a ping used: the session's cost so far, and calls that read and wrote the cache.
fn used(session_cost: f64, read: i64) -> Value {
    json!({ "session_cost_usd": session_cost, "usage": {
        "input_tokens": 3, "cache_read_input_tokens": read, "cache_creation_input_tokens": 500, "output_tokens": 50,
        "iterations": [{ "input_tokens": 3, "cache_read_input_tokens": read, "cache_creation_input_tokens": 500 }],
    } })
}

fn pause_by_owner(e: &mut Engine, work: i64, why: &str) {
    e.db.write(Actor::Owner, |tx| {
        tx.execute(
            "UPDATE works SET paused = ?2, paused_by = 'owner' WHERE id = ?1",
            params![work, why],
        )?;
        Ok(())
    })
    .unwrap();
}

fn let_go_on(e: &mut Engine, work: i64) {
    e.db.write(Actor::Owner, |tx| {
        tx.execute("UPDATE works SET paused = NULL, paused_by = NULL WHERE id = ?1", [work])?;
        Ok(())
    })
    .unwrap();
}

fn move_to(e: &mut Engine, work: i64, cell: &str) {
    e.db.write(Actor::Engine, |tx| {
        tx.execute("UPDATE works SET at = ?2 WHERE id = ?1", params![work, cell])?;
        Ok(())
    })
    .unwrap();
}

/// Steps the engine until the item's work has run so many agent rounds.
fn agent_rounds_to(e: &mut Engine, item: i64, n: usize) {
    for _ in 0..10 {
        if agent_rounds(e, item).len() == n {
            return;
        }
        e.step(None).unwrap();
    }
    panic!("the work did not run {n} agent rounds");
}

/// Whether a round's message gave the conditional input whose address ends so, or only referred to it; none when it has
/// none such.
fn given_ending(input: &Value, end: &str) -> Option<bool> {
    input["conditional"]
        .as_array()?
        .iter()
        .find(|m| m["address"].as_str().is_some_and(|a| a.ends_with(end)))
        .and_then(|m| m["given"].as_bool())
}

// The engineer's line starts a new session at the first round of implementing and at the first round of fixing: the
// session before ends, every other round goes on in the latest session, and a later round of either cell goes on in
// the session it started. The new session is given what the work came to before it: at implementing the item, how
// the work is understood, the approach chosen and the checks after design; at fixing the item, the locked acceptance,
// the design and what implementing handed in. A round that goes on in the session is not given them again.
#[test]
fn the_engineer_starts_a_new_session_at_implementing_and_at_fixing() {
    let s = setup("rotate");
    let (mut e, said, item) = start(&s, scripts());
    step_to(&mut e, item, "T7");
    let (work, _, _) = state(&e, item).unwrap();
    let lines = line_sessions(&e, work, "engineer");
    assert_eq!(lines.len(), 2, "{lines:?}\n{}", said.borrow().join("\n"));
    assert_eq!((lines[0].0.as_str(), lines[0].1), ("line", true), "{lines:?}");
    assert!(lines[0].2.starts_with("R1,") && !lines[0].2.contains("I1"), "{lines:?}");
    assert_eq!(lines[1], ("cell".to_string(), false, "I1,I1".to_string()));
    let rounds = agent_rounds(&e, item);
    let implements: Vec<&Value> = rounds.iter().filter(|r| r.1 == "I1").map(|r| &r.2).collect();
    assert_eq!(given(implements[0], "format:common"), Some(true));
    assert!(
        implements[0]["call"]["session"]["started"].is_string(),
        "{}",
        implements[0]["call"]
    );
    assert_eq!(given(implements[1], "format:common"), Some(false));
    assert!(
        implements[1]["call"]["session"]["resumed"].is_string(),
        "{}",
        implements[1]["call"]
    );
    let item_address = format!("item:{item}");
    for end in [
        item_address.as_str(),
        "·R1·understanding",
        "·D2·chosen",
        "·D2·estimate",
        "·D7·answer",
    ] {
        assert_eq!(
            given_ending(implements[0], end),
            Some(true),
            "{end}: {}",
            implements[0]["conditional"]
        );
        assert_eq!(
            given_ending(implements[1], end),
            Some(false),
            "{end}: {}",
            implements[1]["conditional"]
        );
    }
    move_to(&mut e, work, "T6");
    e.step(None).unwrap();
    let log = said.borrow().join("\n");
    let lines = line_sessions(&e, work, "engineer");
    assert_eq!(lines.len(), 3, "{lines:?}\n{log}");
    assert!(lines[1].1, "{lines:?}");
    assert_eq!(
        (lines[2].0.as_str(), lines[2].1, lines[2].2.as_str()),
        ("cell", false, "T6"),
        "{log}"
    );
    move_to(&mut e, work, "T6");
    e.step(None).unwrap();
    let lines = line_sessions(&e, work, "engineer");
    assert_eq!(lines.len(), 3, "{lines:?}\n{}", said.borrow().join("\n"));
    assert_eq!(lines[2].2, "T6,T6");
    let rounds = agent_rounds(&e, item);
    let fixes: Vec<&Value> = rounds.iter().filter(|r| r.1 == "T6").map(|r| &r.2).collect();
    for end in [item_address.as_str(), "·D2·acceptance", "·D2·design", "·I1·conditions"] {
        assert_eq!(
            given_ending(fixes[0], end),
            Some(true),
            "{end}: {}",
            fixes[0]["conditional"]
        );
        assert_ne!(
            given_ending(fixes[1], end),
            Some(true),
            "{end}: {}",
            fixes[1]["conditional"]
        );
    }
    let _ = std::fs::remove_dir_all(&s.root);
}

// A line's session whose context is past the configuration's limit is replaced at the line's next round by a new one,
// which is given everything its cell needs; the session before ends. While the work waits before that round, the
// session is not kept warm: the next round starts a new one anyway.
#[test]
fn a_session_past_the_context_limit_is_replaced_at_the_lines_next_round() {
    let s = setup("limit");
    let mut scripts = scripts();
    scripts.insert(
        "R2",
        Box::new(|_: &Request, _: usize| (Vec::new(), vec![("@usage", used(1.0, 499_997))])),
    );
    let (mut e, said, item) = start(&s, scripts);
    agent_rounds_to(&mut e, item, 2);
    let (work, at, _) = state(&e, item).unwrap();
    assert_eq!(at, "D1", "{}", said.borrow().join("\n"));
    pause_by_owner(&mut e, work, "to look at it");
    let from = last_call(&e, work);
    assert_eq!(e.keep_warm(from + 55 * 60_000 + 1_000, None).unwrap(), 0);
    let_go_on(&mut e, work);
    e.step(None).unwrap();
    let log = said.borrow().join("\n");
    let lines = line_sessions(&e, work, "engineer");
    assert_eq!(
        lines,
        [
            ("line".to_string(), true, "R1,R2".to_string()),
            ("limit".to_string(), false, "D1".to_string())
        ],
        "{log}"
    );
    let rounds = agent_rounds(&e, item);
    let two = &rounds[2].2;
    for a in ["format:common", "format:D1", "rule:ground"] {
        assert_eq!(given(two, a), Some(true), "{a}");
    }
    assert!(two["call"]["session"]["started"].is_string(), "{}", two["call"]);
    let _ = std::fs::remove_dir_all(&s.root);
}

// A work that waits has the session its next round goes on in pinged 55 minutes after the session's last call, while
// the cache still holds it, and every 55 minutes after, at most 39 times in a wait. Each ping runs the last round's
// command line again, resuming the session, records where the work waited and for what, and costs what the session
// grew by, which the next round's cost leaves out.
#[test]
fn a_waiting_session_is_kept_warm_until_the_wait_has_cost_one_rewrite() {
    let s = setup("warm");
    let mut scripts = scripts();
    scripts.insert(
        "R1",
        Box::new(|_: &Request, _: usize| (Vec::new(), vec![("@usage", used(1.0, 9_000))])),
    );
    scripts.insert(
        "R2",
        Box::new(|_: &Request, _: usize| (Vec::new(), vec![("@usage", used(1.5, 9_500))])),
    );
    scripts.insert(
        "D1",
        Box::new(|_: &Request, _: usize| (Vec::new(), vec![("@usage", used(3.0, 10_000))])),
    );
    let seen: Rc<RefCell<Vec<Replayed>>> = Rc::default();
    let pinged = seen.clone();
    scripts.insert(
        "@ping",
        Box::new(move |r: &Request, n: usize| {
            pinged
                .borrow_mut()
                .push((r.replay.clone().unwrap_or_default(), r.prompt.clone()));
            let total = 1.5 + 0.02 * n as f64;
            let usage = json!({ "session_cost_usd": total, "usage": {
                "input_tokens": 3, "cache_read_input_tokens": 9_500, "cache_creation_input_tokens": 20, "output_tokens": 5,
            } });
            (Vec::new(), vec![("@usage", usage)])
        }),
    );
    let (mut e, said, item) = start(&s, scripts);
    agent_rounds_to(&mut e, item, 2);
    let (work, at, _) = state(&e, item).unwrap();
    assert_eq!(at, "D1", "{}", said.borrow().join("\n"));
    pause_by_owner(&mut e, work, "to look at it");
    let from = last_call(&e, work);
    // Not before 55 minutes, and not once the cache has let the session go.
    assert_eq!(e.keep_warm(from + 54 * 60_000, None).unwrap(), 0);
    assert_eq!(e.keep_warm(from + 60 * 60_000, None).unwrap(), 0);
    let mut now = from + 55 * 60_000;
    assert_eq!(e.keep_warm(now, None).unwrap(), 1);
    assert_eq!(e.keep_warm(now + 60_000, None).unwrap(), 0);
    for _ in 0..40 {
        now += 55 * 60_000 + 1_000;
        e.keep_warm(now, None).unwrap();
    }
    let log = said.borrow().join("\n");
    let pings = pings_of(&e, work);
    assert_eq!(pings.len(), 39, "{log}");
    for (waited, _) in &pings {
        assert_eq!(*waited, json!({ "cell": "D1", "on": "pause", "what": "to look at it" }));
    }
    // Each ping cost what the session grew by since the call before it.
    let costs: Vec<f64> = pings
        .iter()
        .map(|(_, u)| u.as_ref().unwrap()["cost_usd"].as_f64().unwrap())
        .collect();
    assert!(costs.iter().all(|c| (c - 0.02).abs() < 1e-9), "{costs:?}");
    // The ping ran the last round's command line, resuming its session, with only the message changed.
    let rounds = agent_rounds(&e, item);
    let recorded: Vec<String> = rounds[1].2["call"]["command"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a.as_str().unwrap().to_string())
        .collect();
    let (replayed, prompt) = seen.borrow()[0].clone();
    assert_eq!(replayed, recorded);
    assert_eq!(replayed[replayed.len() - 2], "--resume");
    let schema = replayed.iter().position(|a| a == "--json-schema").unwrap();
    assert!(replayed[schema + 1].starts_with('{'), "{replayed:?}");
    assert!(prompt.contains("Keep-warm ping"), "{prompt}");
    // The round after the wait costs what the session grew by since the last ping.
    let_go_on(&mut e, work);
    e.step(None).unwrap();
    let cost: f64 =
        e.db.conn()
            .query_row(
                "SELECT json_extract(usage, '$.cost_usd') FROM rounds WHERE work = ?1 AND cell = 'D1'",
                [work],
                |r| r.get(0),
            )
            .unwrap();
    assert!((cost - (3.0 - 1.5 - 0.02 * 39.0)).abs() < 1e-9, "{cost}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// No ping goes to a session whose next round starts a new one, at a cell the line starts one at, nor beside a round of
// the session that runs.
#[test]
fn no_ping_before_a_new_session_or_beside_a_round() {
    let s = setup("no-warm");
    let mut scripts = scripts();
    scripts.insert(
        "D9",
        Box::new(|_: &Request, _: usize| {
            (
                vec![("outcome.adversarial", "not_needed")],
                vec![("@usage", used(1.0, 9_000))],
            )
        }),
    );
    let (mut e, said, item) = start(&s, scripts);
    step_to(&mut e, item, "I1");
    let (work, _, _) = state(&e, item).unwrap();
    pause_by_owner(&mut e, work, "to look at it");
    let from = last_call(&e, work);
    assert_eq!(e.keep_warm(from + 55 * 60_000 + 1_000, None).unwrap(), 0);
    // At a cell that goes on in the session, a round of the session that runs keeps the ping off.
    move_to(&mut e, work, "D9");
    let session: i64 =
        e.db.conn()
            .query_row(
                "SELECT id FROM sessions WHERE work = ?1 AND line = 'engineer' AND ended IS NULL",
                [work],
                |r| r.get(0),
            )
            .unwrap();
    let open =
        e.db.write(Actor::Engine, |tx| {
            let n = next_round(tx, work)?;
            tx.execute(
                "INSERT INTO rounds (work, n, cell, session, input, config_commit, started_at)
                 VALUES (?1, ?2, 'D9', ?3, '{}', 'c', ?4)",
                params![work, n, session, from],
            )?;
            Ok(n)
        })
        .unwrap();
    assert_eq!(e.keep_warm(from + 55 * 60_000 + 1_000, None).unwrap(), 0);
    e.db.write(Actor::Engine, |tx| {
        tx.execute("DELETE FROM rounds WHERE work = ?1 AND n = ?2", [work, open])?;
        Ok(())
    })
    .unwrap();
    assert_eq!(
        e.keep_warm(from + 55 * 60_000 + 1_000, None).unwrap(),
        1,
        "{}",
        said.borrow().join("\n")
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// A work waiting for the owner's answer has the session of the line that asked kept warm, and the ping says the work
// waited at the owner's cell for the owner's answer to that ask.
#[test]
fn a_ping_says_where_the_work_waited_and_for_what() {
    let s = setup("warm-owner");
    let mut scripts = scripts();
    scripts.insert(
        "R2",
        Box::new(|_: &Request, _: usize| (Vec::new(), vec![("@usage", used(1.0, 9_000))])),
    );
    let (mut e, said, item) = start(&s, scripts);
    agent_rounds_to(&mut e, item, 2);
    let (work, at, _) = state(&e, item).unwrap();
    assert_eq!(at, "D1", "{}", said.borrow().join("\n"));
    let ask =
        e.db.write(Actor::Engine, |tx| {
            tx.execute(
                "UPDATE works SET at = 'A3', resume_at = 'D1', waiting = '[\"D1\"]' WHERE id = ?1",
                [work],
            )?;
            let round = next_round(tx, work)?;
            tx.execute(
                "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES (?1, ?2, 'A3', '{}', 'c', 1)",
                [work, round],
            )?;
            tx.execute(
                "INSERT INTO owner_asks (work, round, body) VALUES (?1, ?2, '{\"to\":\"owner\"}')",
                [work, round],
            )?;
            Ok(tx.last_insert_rowid())
        })
        .unwrap();
    let from = last_call(&e, work);
    assert_eq!(
        e.keep_warm(from + 55 * 60_000 + 1_000, None).unwrap(),
        1,
        "{}",
        said.borrow().join("\n")
    );
    let pings = pings_of(&e, work);
    assert_eq!(
        pings[0].0,
        json!({ "cell": "A3", "on": "owner", "what": format!("the owner's answer to ask {ask}") })
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

/// The file a round gives its session again from after a compaction, and what it held while the round ran.
type GivenAgain = (PathBuf, String);

// A compaction within a round leaves the session holding nothing: the round's command line has a hook give the session
// again at once what the cell is given, whole, from a file that is there while the round runs and gone after it.
#[test]
fn a_compaction_within_a_round_gives_the_cells_inputs_again() {
    let s = setup("regive");
    let mut scripts = scripts();
    let read: Rc<RefCell<Option<GivenAgain>>> = Rc::default();
    let seen = read.clone();
    scripts.insert(
        "R2",
        Box::new(move |r: &Request, _: usize| {
            let file = r.regive.clone().unwrap();
            let text = std::fs::read_to_string(&file).unwrap_or_default();
            *seen.borrow_mut() = Some((file, text));
            (Vec::new(), Vec::new())
        }),
    );
    let (mut e, said, item) = start(&s, scripts);
    agent_rounds_to(&mut e, item, 2);
    let rounds = agent_rounds(&e, item);
    let (id, cell, input) = &rounds[1];
    assert_eq!(cell, "R2", "{}", said.borrow().join("\n"));
    let (file, text) = read.borrow().clone().expect("the file the hook gives again");
    // The session holds the shared formats, so the message only refers to them; the file gives them whole.
    assert_eq!(given(input, "format:common"), Some(false));
    assert!(
        text.starts_with(&format!("Your session was compacted in the middle of {id}"))
            && text.contains(&format!("# {id} · cell R2"))
            && text.contains("### format:common"),
        "{text}"
    );
    let command: Vec<String> = input["call"]["command"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a.as_str().unwrap().to_string())
        .collect();
    let at = command.iter().position(|a| a == "--settings").unwrap();
    let hooks: Value = serde_json::from_str(&command[at + 1]).unwrap();
    assert_eq!(hooks["hooks"]["SessionStart"][0]["matcher"], "compact");
    assert!(!file.exists(), "{}", file.display());
    let _ = std::fs::remove_dir_all(&s.root);
}

// An interjection that comes while a round runs is handed in after the agent's next tool use, by the hook the round's
// command line runs: it is this round's, recorded as delivered in it, and the round's answer to it is taken.
#[test]
fn an_interjection_during_a_round_is_handed_in_and_answered_in_it() {
    let s = setup("during");
    let mut scripts = scripts();
    let data = s.data.clone();
    scripts.insert(
        "R2",
        Box::new(move |r: &Request, _: usize| {
            // What the hook does after a tool use, while the owner interjects.
            let mut db = engine::database::open(&data).unwrap();
            db.write(Actor::Owner, |tx| {
                tx.execute(
                    "INSERT INTO interjections (work, line, speaker, text) VALUES (?1, 'engineer', 'owner', 'Why?')",
                    [r.work],
                )?;
                Ok(())
            })
            .unwrap();
            let n: i64 = db
                .conn()
                .query_row("SELECT max(n) FROM rounds WHERE work = ?1", [r.work], |x| x.get(0))
                .unwrap();
            assert!(r.deliver.as_deref().is_some_and(|d| d.contains("'deliver'")));
            sessions::interject::deliver(&mut db, r.work, n, "engineer")
                .unwrap()
                .unwrap();
            (
                vec![("outcome", "answers")],
                vec![(
                    "outcome.answers",
                    json!([{ "to": "owner", "text": "Because the log says so." }]),
                )],
            )
        }),
    );
    let (mut e, said, item) = start(&s, scripts);
    agent_rounds_to(&mut e, item, 2);
    let log = said.borrow().join("\n");
    let rounds = agent_rounds(&e, item);
    let (id, cell, input) = &rounds[1];
    assert_eq!(cell, "R2", "{log}");
    let (work, _, _) = state(&e, item).unwrap();
    let (transition, delivered): (Option<String>, Option<i64>) =
        e.db.conn()
            .query_row(
                "SELECT r.transition, (SELECT i.delivered_round FROM interjections i WHERE i.work = r.work)
                 FROM rounds r WHERE r.work = ?1 AND 'R' || r.work || '-' || r.n = ?2",
                params![work, id],
                |x| Ok((x.get(0)?, x.get(1)?)),
            )
            .unwrap();
    assert_eq!(transition.as_deref(), Some("answers"), "{log}");
    assert_eq!(delivered.map(|n| format!("R{work}-{n}")), Some(id.clone()));
    assert_eq!(input["engine"]["interjections"], json!([1]), "{input}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// When the model a line's rounds run on changes, as when the owner moves the item to another tier, the line's next
// round starts a new session: a model's prompt cache holds nothing of a session another model ran, so the session would
// be written again whole, and a new one is given only what its cell needs.
#[test]
fn a_new_model_starts_a_new_session_at_the_lines_next_round() {
    let s = setup("model");
    let install = std::fs::read_to_string(s.data.join("install.toml")).unwrap();
    let other = "[agents.levels.m]\ndesc = \"another tier\"\ncli = \"claude\"\nmodel = \"m-other\"\n";
    write(&s.data, "install.toml", &format!("{install}\n{other}"));
    git(&s.data, &["commit", "-q", "-am", "another tier"]);
    let (mut e, said, item) = start(&s, scripts());
    agent_rounds_to(&mut e, item, 2);
    let (work, _, _) = state(&e, item).unwrap();
    // The owner moves the item to the other tier on its page: the change is the owner's, in the item's history.
    let done = api::set_level(&mut e.db, &s.data, item, "m").unwrap();
    assert_eq!(done, format!("item {item} is at tier m"));
    let by: String =
        e.db.conn()
            .query_row(
                "SELECT actor FROM changes WHERE table_name = 'items' AND json_extract(new_row, '$.level') = 'm'",
                [],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(by, "owner");
    e.step(None).unwrap();
    let log = said.borrow().join("\n");
    assert_eq!(
        line_sessions(&e, work, "engineer"),
        [
            ("line".to_string(), true, "R1,R2".to_string()),
            ("model".to_string(), false, "D1".to_string())
        ],
        "{log}"
    );
    let model: String =
        e.db.conn()
            .query_row(
                "SELECT model FROM rounds WHERE work = ?1 AND cell = 'D1'",
                [work],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(model, "m-other");
    let _ = std::fs::remove_dir_all(&s.root);
}

// Before the owner approves a new policy, `P10` asks of the draft the question `P1` asks of
// the policy in force: one that holds goes on to the owner's approval, and one that would need setting again at once
// goes back to correct the text.
#[test]
fn the_draft_policy_is_asked_whether_it_would_need_setting_again_at_once() {
    let verdicts = [("holds", "P11"), ("needs_setting_again", "P8")];
    let s = setup("p11");
    let mut agents = scripts();
    // The first draft holds; the second, in a setting of its own, would need setting again.
    agents.insert(
        "P10",
        Box::new(move |_: &Request, n: usize| (vec![("outcome.report", verdicts[n - 1].0)], Vec::new())),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    for (verdict, then) in verdicts {
        let work = setting_at(&mut e, "P10");
        let judged = |e: &Engine| {
            rows_at(e, work, "P10")
                .into_iter()
                .filter(|t| t != "session_anew")
                .collect::<Vec<_>>()
        };
        for _ in 0..20 {
            if !judged(&e).is_empty() {
                break;
            }
            e.step_work(work).unwrap();
        }
        let log = said.borrow().join("\n");
        let at = e.work(work).unwrap().pos.at;
        assert_eq!((at.as_str(), judged(&e)), (then, vec![verdict.to_string()]), "{log}");
    }
    let _ = std::fs::remove_dir_all(&s.root);
}

/// What a message gave whole under the address that ends with `end`, read back; none when it gave nothing there.
fn given_value(message: &str, end: &str) -> Option<Value> {
    message.split("\n### ").skip(1).find_map(|part| {
        let (address, rest) = part.split_once('\n')?;
        if !address.ends_with(end) {
            return None;
        }
        serde_json::from_str(rest.trim_start().lines().next()?).ok()
    })
}

/// The lines of a message's list of the inputs to work from.
fn work_from(message: &str) -> Vec<String> {
    message
        .split("## Work from\n\n")
        .nth(1)
        .unwrap_or_default()
        .lines()
        .take_while(|l| !l.trim().is_empty())
        .map(str::to_string)
        .collect()
}

/// The inputs of a work's rounds at a cell, as they were recorded, oldest first.
fn inputs_at(e: &Engine, work: i64, cell: &str) -> Vec<Value> {
    engine::query_json(
        e.db.conn(),
        "SELECT input FROM rounds WHERE work = ?1 AND cell = ?2 ORDER BY n",
        &[&work, &cell],
        100,
    )
    .unwrap()
    .iter()
    .map(|r| db::put_back(e.db.conn(), &r["input"]))
    .collect()
}

// At a project's first architect work its architect is told what happened in the week before it, as at a later work
// since the last; its board holds every item not done: one whose last work stopped, queued again, with why; one whose
// work asks the owner, at the stage of the cell it stopped at, its pause said once; and one whose work runs tools, at
// the stage of the cell that waits for them.
#[test]
fn the_architect_s_first_work_is_told_the_week_before_it_and_every_item_not_done() {
    let s = setup("first-work");
    let (mut e, said) = engine(&s, scripts());
    let merged = an_item(&mut e);
    let v = e.flow_now;
    let ago = db::now() - 24 * 3_600_000;
    let (stopped, asking, tools) =
        e.db.write(Actor::Owner, |tx| {
            let item = |title: &str| -> Result<i64, db::Error> {
                tx.execute(
                    "INSERT INTO items (project, title, problem, why, deliver, level)
                     VALUES ('p', ?1, 'Missing.', 'Needed.', 'change', 'l')",
                    [title],
                )?;
                Ok(tx.last_insert_rowid())
            };
            let (stopped, asking, tools) = (item("Lex it")?, item("Print it")?, item("Test it")?);
            tx.execute(
                "INSERT INTO works (item, flow_version, graph, at, ended) VALUES (?1, ?2, 'engineer', 'I1', 'stopped: by hand')",
                params![stopped, v],
            )?;
            tx.execute(
                "INSERT INTO works (item, flow_version, graph, at, resume_at, paused, paused_by)
                 VALUES (?1, ?2, 'engineer', 'A3', 'I1', 'later', 'owner')",
                params![asking, v],
            )?;
            tx.execute(
                "INSERT INTO works (item, flow_version, graph, at, waiting, paused, paused_by)
                 VALUES (?1, ?2, 'engineer', 'X1', '[\"I1\"]', 'later', 'owner')",
                params![tools, v],
            )?;
            tx.execute(
                "INSERT INTO works (item, flow_version, graph, at) VALUES (?1, ?2, 'engineer', 'E1')",
                params![merged, v],
            )?;
            tx.execute(
                "INSERT INTO rounds (work, n, cell, input, output, transition, config_commit, started_at, ended_at)
                 VALUES (?1, 1, 'M2', '{}', '{\"result\":{\"merge_commit\":\"abc\"}}', 'merged', 'c', ?2, ?2)",
                params![tx.last_insert_rowid(), ago],
            )?;
            Ok((stopped, asking, tools))
        })
        .unwrap();
    say(&mut e, "Look at the items.");
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 10, |e| path_of(e, arch).iter().any(|c| c == "U1")),
        "{}",
        said.borrow().join("\n")
    );
    let m = message_at(&e, arch, "U1", false);
    let since = given_value(&m, "·engine·since_last_work").expect(&m);
    assert_eq!(since["first_work"], true, "{since}");
    assert_eq!(since["merged"][0]["item"], merged, "{since}");
    let board = given_value(&m, "·engine·board").expect(&m);
    let row = |id: i64| board["items"][format!("#{id}")].clone();
    assert_eq!(row(merged), Value::Null, "{board}");
    assert_eq!(row(stopped)["last_work_ended"], "stopped: by hand", "{board}");
    assert_eq!(row(stopped)["ready"], true, "{board}");
    assert_eq!(row(stopped).get("work"), None, "{board}");
    assert_eq!(row(asking)["stage"], "implement", "{board}");
    assert_eq!(row(asking)["paused"], "later", "{board}");
    assert_eq!(row(asking).get("waits"), None, "{board}");
    assert_eq!(row(tools)["stage"], "implement", "{board}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The numbers the architect is given are measured up to the start of its work, its own rounds counting for nothing, and
// what each week spent is split between the items' works and the architect's works.
#[test]
fn the_numbers_hold_through_an_architect_work_and_split_what_was_spent() {
    let s = setup("numbers");
    let (mut e, said) = engine(&s, scripts());
    quiet(&mut e);
    let item = an_item(&mut e);
    let v = e.flow_now;
    let ago = db::now() - 3_600_000;
    e.db.write(Actor::Engine, |tx| {
        tx.execute("UPDATE rounds SET usage = '{\"cost_usd\":0.25}' WHERE cell = 'H10'", [])?;
        tx.execute(
            "INSERT INTO works (item, flow_version, graph, at, ended) VALUES (?1, ?2, 'engineer', 'I1', 'stopped: by hand')",
            params![item, v],
        )?;
        tx.execute(
            "INSERT INTO rounds (work, n, cell, input, transition, config_commit, usage, started_at, ended_at)
             VALUES (?1, 1, 'I1', '{}', 'tools', 'c', '{\"cost_usd\":0.5}', ?2, ?2)",
            params![tx.last_insert_rowid(), ago],
        )?;
        Ok(())
    })
    .unwrap();
    say(&mut e, "Look at the numbers.");
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 10, |e| path_of(e, arch).iter().any(|c| c == "N1")),
        "{}",
        said.borrow().join("\n")
    );
    let understanding = message_at(&e, arch, "U1", false);
    let numbers = given_value(&understanding, "·engine·numbers").expect(&understanding);
    assert_eq!(
        numbers["weeks"][0]["spent_usd"],
        json!({ "items": 0.5, "architect": 0.25 }),
        "{numbers}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

/// A work of the engineer's graph at a cell on a new item of a project, paused by the owner so that the engine leaves
/// it be, with rounds it ran, each a cell, the row it took, when it started and when it ended (none while under way).
fn a_work_that_ran(
    tx: &db::rusqlite::Transaction<'_>,
    v: i64,
    project: &str,
    at: &str,
    rounds: &[(&str, Option<&str>, i64, Option<i64>)],
) -> Result<i64, db::Error> {
    tx.execute(
        "INSERT INTO items (project, title, problem, why, deliver, level) VALUES (?1, 't', 'x', 'y', 'change', 'l')",
        [project],
    )?;
    let item = tx.last_insert_rowid();
    tx.execute(
        "INSERT INTO works (item, flow_version, graph, at, paused, paused_by)
         VALUES (?1, ?2, 'engineer', ?3, 'later', 'owner')",
        params![item, v, at],
    )?;
    let work = tx.last_insert_rowid();
    for (n, (cell, row, from, to)) in rounds.iter().enumerate() {
        tx.execute(
            "INSERT INTO rounds (work, n, cell, input, transition, config_commit, started_at, ended_at)
             VALUES (?1, ?2, ?3, '{}', ?4, 'c', ?5, ?6)",
            params![work, n as i64 + 1, cell, row, from, to],
        )?;
    }
    Ok(work)
}

// The numbers the architect is given say, week by week, the most works of the project that ran together and for how
// long two or more did, and how many of its works ended without a merge, by kind. A work waiting for the architect or
// the owner does not run, whatever the version's id for the cell; a work that merged never ends unmerged, whatever
// becomes of its item; another project's works count for nothing; and a week with none has zeros, not missing values.
#[test]
fn the_numbers_say_how_many_works_ran_together_and_ended_without_a_merge() {
    use engine::ending::{End, EndKind, end};
    let s = setup("numbers-together");
    let (mut e, said) = engine(&s, scripts());
    quiet(&mut e);
    an_item(&mut e);
    let v = e.flow_now;
    let projects = e.config.config.projects.clone();
    let (hour, day) = (3_600_000, 24 * 3_600_000);
    let now = db::now();
    e.db.write(Actor::Engine, |tx| {
        // Two that run, for two of their hours together; a third waits for the owner, a fourth by the id the cell had
        // before, both under way for longer; and another project's runs beside them.
        a_work_that_ran(
            tx,
            v,
            "p",
            "I1",
            &[("I1", Some("tools"), now - 10 * hour, Some(now - 6 * hour))],
        )?;
        a_work_that_ran(
            tx,
            v,
            "p",
            "I1",
            &[("I1", Some("tools"), now - 8 * hour, Some(now - 4 * hour))],
        )?;
        a_work_that_ran(tx, v, "p", "A3", &[("A3", None, now - 9 * hour, None)])?;
        a_work_that_ran(tx, v, "p", "16", &[("16", None, now - 30 * hour, None)])?;
        a_work_that_ran(
            tx,
            v,
            "q",
            "I1",
            &[("I1", Some("tools"), now - 9 * hour, Some(now - 5 * hour))],
        )?;
        // The week before: one that ran alone.
        a_work_that_ran(
            tx,
            v,
            "p",
            "I1",
            &[("I1", None, now - 8 * day, Some(now - 8 * day + hour))],
        )?;
        // Ended: by the flow moving, by its item cancelled, one that merged and then had its item cancelled, and
        // another project's.
        let moved = a_work_that_ran(tx, v, "p", "I1", &[])?;
        end(
            tx,
            moved,
            End::TakenOver(EndKind::FlowMoved, "stopped: moved"),
            &projects,
        )
        .map_err(db::Error::Abort)?;
        let cancelled = a_work_that_ran(tx, v, "p", "I1", &[])?;
        end(
            tx,
            cancelled,
            End::Stopped(EndKind::ItemCancelled, "item cancelled: x"),
            &projects,
        )
        .map_err(db::Error::Abort)?;
        let merged = a_work_that_ran(
            tx,
            v,
            "p",
            "I1",
            &[("M2", Some("merged"), now - 3 * hour, Some(now - 3 * hour))],
        )?;
        end(
            tx,
            merged,
            End::Stopped(EndKind::ItemCancelled, "item cancelled: y"),
            &projects,
        )
        .map_err(db::Error::Abort)?;
        let other = a_work_that_ran(tx, v, "q", "I1", &[])?;
        end(
            tx,
            other,
            End::TakenOver(EndKind::FlowMoved, "stopped: moved"),
            &projects,
        )
        .map_err(db::Error::Abort)?;
        // At the end of the graph with all the work given to sub-items, so with no merge, this week and the week
        // before; and one that delivers a report, which is merged through a pull request and has its merged round.
        let to_sub_items = |from: i64, to: i64| {
            [
                ("T7", Some("all_to_sub_items"), from - hour, Some(from)),
                ("E1", None, from, Some(to)),
            ]
        };
        a_work_that_ran(tx, v, "p", "E1", &to_sub_items(now - 3 * hour, now - 2 * hour))?;
        a_work_that_ran(tx, v, "p", "E1", &to_sub_items(now - 8 * day, now - 8 * day))?;
        let report = a_work_that_ran(
            tx,
            v,
            "p",
            "E1",
            &[
                ("M2", Some("merged"), now - 3 * hour, Some(now - 3 * hour)),
                ("E1", None, now - hour, Some(now - hour)),
            ],
        )?;
        tx.execute(
            "UPDATE items SET deliver = 'report' WHERE id = (SELECT item FROM works WHERE id = ?1)",
            [report],
        )?;
        Ok(())
    })
    .unwrap();
    say(&mut e, "Look at the numbers.");
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 10, |e| path_of(e, arch).iter().any(|c| c == "N1")),
        "{}",
        said.borrow().join("\n")
    );
    let understanding = message_at(&e, arch, "U1", false);
    let numbers = given_value(&understanding, "·engine·numbers").expect(&understanding);
    let weeks = |key: &str| -> Vec<Value> { (0..4).map(|k| numbers["weeks"][k][key].clone()).collect() };
    let together = |most: i64, seconds: f64| json!({ "most_works": most, "seconds_two_or_more": seconds });
    let zeros = json!({ "flow_moved": 0, "item_cancelled": 0, "started_over": 0, "no_merge_at_end": 0, "other": 0,
                        "total": 0 });
    assert_eq!(
        weeks("running_together"),
        [
            together(2, 7200.0),
            together(1, 0.0),
            together(0, 0.0),
            together(0, 0.0)
        ],
        "{numbers}"
    );
    let ended = |kinds: Value| {
        let mut x = zeros.clone();
        let mut total = 0;
        for (k, n) in kinds.as_object().unwrap() {
            x[k] = n.clone();
            total += n.as_i64().unwrap();
        }
        x["total"] = json!(total);
        x
    };
    assert_eq!(
        weeks("ended_unmerged"),
        [
            ended(json!({ "flow_moved": 1, "item_cancelled": 1, "no_merge_at_end": 1 })),
            ended(json!({ "no_merge_at_end": 1 })),
            zeros.clone(),
            zeros.clone()
        ],
        "{numbers}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// A round of the understanding that asks for tools hands in no state of things: until the cell answers, the project has
// none, as at its architect's first work, and the cell's next round is still told it is the first work. The cell's own
// inputs that have nothing are listed as none; the tool results, which every cell may have, only once there are some.
#[test]
fn a_round_of_the_understanding_that_asks_for_tools_changes_neither_the_state_nor_the_task() {
    let s = setup("understand-tools");
    let mut agents = scripts();
    agents.insert(
        "U1",
        Box::new(|_: &Request, n: usize| {
            if n == 1 {
                let tools = json!([{ "tool": "query", "sql": "SELECT id FROM items" }]);
                return (vec![("outcome", "tools")], vec![("outcome.tools", tools)]);
            }
            (Vec::new(), Vec::new())
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    say(&mut e, "Look.");
    let arch = e.step(None).unwrap().work.unwrap();
    let took = |row: &'static str| {
        move |e: &mut Engine| {
            cells_of(e, arch)
                .iter()
                .any(|c| c.0 == "U1" && c.1.as_deref() == Some(row))
        }
    };
    let states = |e: &Engine| -> i64 {
        e.db.conn()
            .query_row("SELECT count(*) FROM architect_state WHERE project = 'p'", [], |r| {
                r.get(0)
            })
            .unwrap()
    };
    assert!(steps_until(&mut e, 10, took("tools")), "{}", said.borrow().join("\n"));
    assert_eq!(states(&e), 0, "{}", said.borrow().join("\n"));
    assert!(
        steps_until(&mut e, 10, took("understood")),
        "{}",
        said.borrow().join("\n")
    );
    assert_eq!(states(&e), 1);
    let (first, second) = (message_at(&e, arch, "U1", false), message_at(&e, arch, "U1", true));
    for m in [&first, &second] {
        assert!(m.contains("This is your first work on the project"), "{m}");
    }
    let w = e.work(arch).unwrap();
    let g = e.graph_of(&w).unwrap();
    let numbered: Vec<&flow::CompiledInput> = g
        .cell("U1")
        .unwrap()
        .inputs
        .iter()
        .filter(|x| !x.from.iter().all(|s| *s == flow::format::Src::Itself))
        .collect();
    let number = |of: &dyn Fn(&flow::CompiledInput) -> bool| numbered.iter().position(|x| of(x)).unwrap() + 1;
    let before = number(&|x| {
        x.from
            == [flow::format::Src::Db {
                relation: "architect_state".into(),
                by: Some("project".into()),
            }]
    });
    let tools = number(&|x| x.name.as_deref() == Some("Tool results"));
    let listed = |m: &str, n: usize| work_from(m).into_iter().find(|l| l.starts_with(&format!("{n}. ")));
    assert_eq!(listed(&first, before), Some(format!("{before}. none")), "{first}");
    assert_eq!(listed(&first, tools), None, "{first}");
    assert!(listed(&second, tools).is_some_and(|l| l.contains("·X1·")), "{second}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The policy being set is given to the cells that judge and correct it as the architect's own return that wrote it,
// which its session holds and is only referred to. A correction hands in only the passages it changes, and the text
// the engine puts together from it is given whole once. A probe the judgment of a reading lists as answered wrongly is
// not used again, every such probe named at once; one the judgment only mentions stays.
#[test]
fn the_draft_policy_is_referred_to_and_only_probes_answered_wrongly_go() {
    let s = setup("draft");
    let mut agents = scripts();
    agents.insert(
        "P4",
        Box::new(|_: &Request, _: usize| {
            let policy = json!({ "direction": "Ship the parser.", "why": "Users wait for it.",
                                 "principles": [{ "no": 1, "text": "Parse before checking." }],
                                 "signals": [{ "what": "Speed", "how": "merged" }] });
            let probes =
                json!([{ "id": "x", "ask": "What comes first?", "principles": [1], "expect": "Principle 1." }]);
            (Vec::new(), vec![("outcome.policy", policy), ("outcome.probes", probes)])
        }),
    );
    agents.insert(
        "P7",
        Box::new(|_: &Request, n: usize| {
            if n > 1 {
                return (Vec::new(), Vec::new());
            }
            let gaps =
                json!([{ "what": "The probes x and y", "seen": "It hesitated on x.", "missing": "Principle 1." }]);
            (
                vec![("outcome.verdict", "gaps")],
                vec![
                    ("outcome.verdict.gaps", gaps),
                    ("outcome.verdict.wrong_probes", json!(["y", "z"])),
                ],
            )
        }),
    );
    agents.insert(
        "P8",
        Box::new(|_: &Request, n: usize| {
            let probe =
                |id: &str| json!({ "id": id, "ask": "What first?", "principles": [1], "expect": "Principle 1." });
            let probes = if n == 1 {
                json!([probe("x"), probe("y"), probe("z")])
            } else {
                json!([probe("x")])
            };
            let edits = json!([{ "kind": "change", "old": "Ship the parser.", "new": "Ship the parser first." }]);
            (
                Vec::new(),
                vec![("outcome.revision.probes", probes), ("outcome.revision.edits", edits)],
            )
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    let work = setting_at(&mut e, "P4");
    let judged = |e: &Engine| {
        cells_of(e, work)
            .iter()
            .filter(|c| c.0 == "P7" && c.1.is_some())
            .count()
    };
    for _ in 0..40 {
        if judged(&e) == 2 {
            break;
        }
        e.step_work(work).unwrap();
    }
    let log = said.borrow().join("\n");
    assert_eq!(judged(&e), 2, "{log}\n{:?}", cells_of(&e, work));
    let (judged, corrected) = (inputs_at(&e, work, "P7"), inputs_at(&e, work, "P8"));
    for input in [&judged[0], &corrected[0]] {
        assert_eq!(given_ending(input, "·P4·policy"), Some(false), "{input}");
        assert_eq!(given_ending(input, "·P4·probes"), Some(false), "{input}");
    }
    // The correction changed a passage: the text the engine put together from it is given whole, the new probes are
    // the session's own return.
    assert_eq!(
        given_ending(&judged[1], "·P8·revision.probes"),
        Some(false),
        "{}",
        judged[1]
    );
    assert_eq!(
        given_ending(&judged[1], "·P5·draft.policy"),
        Some(true),
        "{}",
        judged[1]
    );
    let text = draft_direction(&e);
    assert_eq!(text, [json!("Ship the parser first.")]);
    let rejected: Vec<String> = engine::query_json(
        e.db.conn(),
        "SELECT rejected FROM rounds WHERE work = ?1 AND cell = 'P8' AND rejected IS NOT NULL",
        &[&work],
        10,
    )
    .unwrap()
    .iter()
    .map(|r| r["rejected"].to_string())
    .collect();
    assert_eq!(rejected.len(), 1, "{rejected:?}");
    assert!(rejected[0].contains("not used again: y, z"), "{rejected:?}");
    let _ = std::fs::remove_dir_all(&s.root);
}

/// The scripted writing of a policy whose one principle has one probe, `x`, which a scripted reader answers.
fn writes_a_policy() -> Script {
    Box::new(|_: &Request, _: usize| {
        let policy = json!({ "direction": "Ship the parser.", "why": "Users wait for it.",
                             "principles": [{ "no": 1, "text": "Parse before checking." }],
                             "signals": [{ "what": "Speed", "how": "merged" }] });
        let probes = json!([{ "id": "x", "ask": "What comes first?", "principles": [1], "expect": "Principle 1." }]);
        (Vec::new(), vec![("outcome.policy", policy), ("outcome.probes", probes)])
    })
}

/// The rows a work's rounds at a cell took, oldest first; a round that took none is left out.
fn rows_at(e: &Engine, work: i64, cell: &str) -> Vec<String> {
    cells_of(e, work)
        .into_iter()
        .filter(|c| c.0 == cell)
        .filter_map(|c| c.1)
        .collect()
}

/// The ask of the owner to approve a policy the architect put, not answered yet.
fn approval_asked(e: &Engine) -> Option<i64> {
    e.db.conn()
        .query_row(
            "SELECT id FROM owner_asks WHERE answer IS NULL AND json_extract(body, '$.kind') = 'policy_approval'",
            [],
            |r| r.get(0),
        )
        .ok()
}

/// The direction of the text of the policy being set, as the setting's latest version has it.
fn draft_direction(e: &Engine) -> Vec<Value> {
    engine::query_json(
        e.db.conn(),
        "SELECT policy FROM policy_text WHERE matter = ?1",
        &[&setting_matter(e)],
        1,
    )
    .unwrap()
    .iter()
    .map(|r| r["policy"]["direction"].clone())
    .collect()
}

// A reading that answers every probe rightly passes, though wording should change that the reader read rightly anyway:
// the engine puts the minor fix into the text without a new reading, and the reading counts towards the two passes.
#[test]
fn a_minor_fix_a_reading_finds_is_put_in_without_a_new_reading() {
    let s = setup("minor-fix");
    let mut agents = scripts();
    agents.insert("P4", writes_a_policy());
    agents.insert(
        "P7",
        Box::new(|_: &Request, n: usize| {
            if n > 1 {
                return (Vec::new(), Vec::new());
            }
            let edits = json!([{ "old": "Ship the parser.", "new": "Ship the parser first." }]);
            (
                vec![("outcome.verdict", "minor_fix")],
                vec![("outcome.verdict.edits", edits), ("outcome.verdict.gaps", json!([]))],
            )
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    let work = setting_at(&mut e, "P4");
    for _ in 0..40 {
        if e.work(work).unwrap().pos.at == "P11" {
            break;
        }
        e.step_work(work).unwrap();
    }
    let log = said.borrow().join("\n");
    assert_eq!(e.work(work).unwrap().pos.at, "P11", "{log}\n{:?}", cells_of(&e, work));
    assert_eq!(rows_at(&e, work, "P7"), ["minor_fix", "understood"], "{log}");
    assert_eq!(
        rows_at(&e, work, "P5"),
        ["new_session", "fixed", "new_session"],
        "{log}"
    );
    assert_eq!(rows_at(&e, work, "P9"), ["one_more", "two_passed"], "{log}");
    assert_eq!(rows_at(&e, work, "P6").len(), 2, "{log}");
    assert_eq!(draft_direction(&e), [json!("Ship the parser first.")]);
    let _ = std::fs::remove_dir_all(&s.root);
}

// Once the text was corrected three times since it was written, a reading that leaves gaps goes on with them to the
// check of the draft and to the owner, who may send the text back for one more correction without a new study: the
// setting's matter comes back to the approval with the answer, the correction is given the owner's words, and the next
// reading that leaves gaps goes to the owner again.
#[test]
fn a_text_corrected_three_times_goes_with_its_gaps_to_the_owner() {
    let s = setup("text-limit");
    let mut agents = scripts();
    agents.insert("P4", writes_a_policy());
    agents.insert(
        "P7",
        Box::new(|_: &Request, _: usize| {
            let gaps = json!([{ "what": "Principle 1", "seen": "Two readers asked what it weighs.",
                               "missing": "What it weighs." }]);
            (
                vec![("outcome.verdict", "gaps")],
                vec![
                    ("outcome.verdict.gaps", gaps),
                    ("outcome.verdict.wrong_probes", json!(["y"])),
                ],
            )
        }),
    );
    agents.insert(
        "P8",
        Box::new(|_: &Request, n: usize| {
            let old = if n == 1 {
                "Parse before checking.".to_string()
            } else {
                format!("Parse before checking, {}.", n - 1)
            };
            let edits = json!([{ "kind": "change", "old": old, "new": format!("Parse before checking, {n}.") }]);
            (
                Vec::new(),
                vec![
                    ("outcome.revision.edits", edits),
                    ("outcome.revision.kept", json!(["x"])),
                    ("outcome.revision.probes", json!([])),
                ],
            )
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    let work = setting_at(&mut e, "P4");
    for _ in 0..60 {
        if approval_asked(&e).is_some() {
            break;
        }
        e.step_work(work).unwrap();
    }
    let log = said.borrow().join("\n");
    let ask = approval_asked(&e).unwrap_or_else(|| panic!("{log}\n{:?}", cells_of(&e, work)));
    assert_eq!(rows_at(&e, work, "P7"), ["gaps", "gaps", "gaps", "gaps_left"], "{log}");
    assert_eq!(
        rows_at(&e, work, "P8"),
        ["corrected", "corrected", "corrected"],
        "{log}"
    );
    assert_eq!(rows_at(&e, work, "P10"), ["holds"], "{log}");
    // The probe kept by name stays the version's.
    let probes: Vec<Value> = engine::query_json(
        e.db.conn(),
        "SELECT probes FROM policy_draft WHERE matter = ?1",
        &[&setting_matter(&e)],
        1,
    )
    .unwrap()
    .iter()
    .flat_map(|r| r["probes"].as_array().cloned().unwrap_or_default())
    .map(|p| p["id"].clone())
    .collect();
    assert_eq!(probes, [json!("x")]);
    // The owner decides by the button alone: what to fix is what the owner said in their talks, none here, and the
    // architect judges by itself.
    let fix = api::Answer {
        text: String::new(),
        why: None,
        reply: None,
        choice: Some("fix_text".into()),
        to: None,
    };
    api::answer(&mut e.db, ask, &fix).unwrap();
    for _ in 0..30 {
        if approval_asked(&e).is_some() {
            break;
        }
        e.step_work(work).unwrap();
    }
    let log = said.borrow().join("\n");
    assert!(approval_asked(&e).is_some(), "{log}\n{:?}", cells_of(&e, work));
    assert_eq!(rows_at(&e, work, "P11"), ["held", "fix_text", "held"], "{log}");
    assert_eq!(rows_at(&e, work, "P8").len(), 4, "{log}");
    assert_eq!(
        rows_at(&e, work, "P7").last().map(String::as_str),
        Some("gaps_left"),
        "{log}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

/// The open blocker of a work, by its number; none when it has none.
fn open_blocker(e: &Engine, work: i64) -> Option<i64> {
    e.db.conn()
        .query_row(
            "SELECT id FROM blockers WHERE work = ?1 AND ended IS NULL",
            [work],
            |r| r.get(0),
        )
        .ok()
}

/// The architect's messages to the owner in its one-to-one, oldest first, each with who sent it: its line.
fn one_to_one_messages(e: &Engine) -> Vec<(String, String)> {
    let mut st = e
        .db
        .conn()
        .prepare(
            "SELECT sender, text FROM conversation_entries WHERE conversation = 'architect:p' AND source = 'message'
                 ORDER BY at, id",
        )
        .unwrap();
    st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(|x| x.unwrap())
        .collect()
}

/// The ask of the owner to read the study of a new policy the architect put, not answered yet.
fn study_asked(e: &Engine) -> Option<i64> {
    e.db.conn()
        .query_row(
            "SELECT id FROM owner_asks WHERE answer IS NULL AND json_extract(body, '$.kind') = 'study'",
            [],
            |r| r.get(0),
        )
        .ok()
}

// What the owner says to the architect while the study of a new policy waits for the owner's reading is the
// architect's to answer, as any words of the owner: with no work of the architect running, the words come to its queue
// and start a work, which answers them, and the ask of the study stays open; the owner's view of the study after them
// takes the setting on.
#[test]
fn the_owner_s_words_while_the_study_waits_are_answered_and_the_ask_stays_open() {
    let s = setup("words-study");
    let mut agents = scripts();
    agents.insert(
        "H8",
        Box::new(|_: &Request, _: usize| (Vec::new(), vec![("tell_owner", json!(["It weighs the parser first."]))])),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    let setting = setting_at(&mut e, "P2");
    assert!(
        steps_until(&mut e, 60, |e| study_asked(e).is_some()
            && architect_work_done(e, setting)),
        "{}\n{:?}",
        said.borrow().join("\n"),
        cells_of(&e, setting)
    );
    let ask = study_asked(&e);
    let m = setting_matter(&e);
    say(&mut e, "Why the parser first?");
    let answered = |e: &mut Engine| {
        let handled: i64 =
            e.db.conn()
                .query_row(
                    "SELECT count(*) FROM architect_matters WHERE kind = 'owner_words' AND handled_round IS NOT NULL",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
        handled > 0
    };
    assert!(steps_until(&mut e, 40, answered), "{}", said.borrow().join("\n"));
    let log = said.borrow().join("\n");
    assert!(
        one_to_one_messages(&e).contains(&("architect".to_string(), "It weighs the parser first.".to_string())),
        "{log}"
    );
    // The study still waits for the owner's view, which still holds the setting's matter.
    assert_eq!(study_asked(&e), ask, "{log}");
    let held: Option<i64> =
        e.db.conn()
            .query_row("SELECT asked FROM architect_matters WHERE id = ?1", [m], |r| r.get(0))
            .unwrap();
    assert_eq!(held, ask, "{log}");
    let satisfied = api::Answer {
        text: "Go on.".into(),
        why: None,
        reply: None,
        choice: Some("satisfied".into()),
        to: None,
    };
    api::answer(&mut e.db, ask.unwrap(), &satisfied).unwrap();
    assert!(
        steps_until(&mut e, 40, |e| !setting_rows(e, "P4").is_empty()),
        "{}",
        said.borrow().join("\n")
    );
    assert_eq!(setting_rows(&e, "P3"), ["held", "satisfied"]);
    let _ = std::fs::remove_dir_all(&s.root);
}

// A return sent back at a cell on no line, whose every round is a new session, is given the whole message again, and
// why the return before it was sent back.
#[test]
fn a_new_session_after_a_return_sent_back_is_told_why() {
    let s = setup("sent-back-anew");
    let mut agents = scripts();
    agents.insert("P4", writes_a_policy());
    agents.insert(
        "P6",
        Box::new(|_: &Request, n: usize| {
            if n == 1 {
                return (Vec::new(), vec![("outcome.reading.answers", json!([]))]);
            }
            (Vec::new(), Vec::new())
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    let work = setting_at(&mut e, "P4");
    for _ in 0..30 {
        if rows_at(&e, work, "P6").len() == 1 {
            break;
        }
        e.step_work(work).unwrap();
    }
    let log = said.borrow().join("\n");
    assert_eq!(rows_at(&e, work, "P6").len(), 1, "{log}\n{:?}", cells_of(&e, work));
    let again = message_at(&e, work, "P6", true);
    assert!(
        again.contains("was sent back: ") && again.contains("## Work from"),
        "{again}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// The study of a new policy is given the project's design goal, the owner's own words on where it goes, beside the
// constitution, as a work of the architect is given its project's settings.
#[test]
fn the_study_of_a_new_policy_is_given_the_design_goal() {
    let s = setup("study-goal");
    let (mut e, _) = engine(&s, scripts());
    quiet(&mut e);
    let setting = setting_at(&mut e, "P2");
    let m = engine::next_message(&mut e, setting).unwrap().text;
    let goal = work_from(&m).into_iter().nth(2).unwrap_or_default();
    assert!(goal.starts_with("3. ") && !goal.ends_with("none"), "{goal}\n{m}");
    assert!(m.contains("simple"), "{m}");
    let _ = std::fs::remove_dir_all(&s.root);
}

/// Records a round of a work that ended at a cell with a return and the row it took.
fn record_round(
    tx: &db::rusqlite::Transaction<'_>,
    work: i64,
    n: i64,
    cell: &str,
    output: &Value,
    row: &str,
) -> Result<(), db::Error> {
    tx.execute(
        "INSERT INTO rounds (work, n, cell, input, output, transition, config_commit, started_at, ended_at)
         VALUES (?1, ?2, ?3, '{}', ?4, ?5, 'c', ?6, ?6)",
        params![work, n, cell, output.to_string(), row, db::now()],
    )?;
    Ok(())
}

/// A work of project p's architect at a cell, with no round yet.
fn architect_work_at(e: &mut Engine, cell: &str) -> i64 {
    let v = e.flow_now;
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO works (item, flow_version, graph, at, project) VALUES (NULL, ?1, 'architect', ?2, 'p')",
            params![v, cell],
        )?;
        Ok(tx.last_insert_rowid())
    })
    .unwrap()
}

/// A setting of project p's policy at a cell, with no round there yet: a work of p's architect whose check of the
/// policy in force set it again and raised the setting's matter, which the work has in hand, new works of p held. A
/// setting under way before it is withdrawn, since one runs at a time.
fn setting_at(e: &mut Engine, cell: &str) -> i64 {
    let v = e.flow_now;
    let again = json!({ "outcome": { "against": [], "verdict": { "verdict": "set_again",
                        "reasons": [{ "kind": "no_policy", "seen": "s", "why": "No policy yet." }] } },
                        "progress": "Set again.", "tell_owner": [] });
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "UPDATE architect_matters SET withdrawn = 'over before the next setting'
             WHERE kind = 'set_policy' AND handled_round IS NULL AND withdrawn IS NULL",
            [],
        )?;
        tx.execute(
            "INSERT INTO works (item, flow_version, graph, at, project) VALUES (NULL, ?1, 'architect', ?2, 'p')",
            params![v, cell],
        )?;
        let work = tx.last_insert_rowid();
        record_round(tx, work, 1, "P1", &again, "set_again")?;
        db::queue_matter(
            tx,
            &db::Matter {
                project: "p".into(),
                kind: "set_policy".into(),
                detail: None,
                what: "Set a new policy.".into(),
                body: json!({}),
                item: None,
                raised: Some((work, 1)),
            },
        )?;
        tx.execute(
            "INSERT OR IGNORE INTO dispatch_holds (project, reason, since) VALUES ('p', 'policy', 1)",
            [],
        )?;
        Ok(work)
    })
    .unwrap()
}

/// The matter of project p's setting of its policy raised last.
fn setting_matter(e: &Engine) -> i64 {
    e.db.conn()
        .query_row(
            "SELECT max(id) FROM architect_matters WHERE project = 'p' AND kind = 'set_policy'",
            [],
            |r| r.get(0),
        )
        .unwrap()
}

/// The rows the rounds of project p's architect works at a cell took, oldest first, in whichever work.
fn setting_rows(e: &Engine, cell: &str) -> Vec<String> {
    engine::query_json(
        e.db.conn(),
        "SELECT r.transition FROM rounds r JOIN works w ON w.id = r.work
         WHERE w.graph = 'architect' AND w.project = 'p' AND r.cell = ?1 AND r.transition IS NOT NULL
         ORDER BY r.work, r.n",
        &[&cell],
        100,
    )
    .unwrap()
    .iter()
    .filter_map(|r| r["transition"].as_str().map(str::to_string))
    .collect()
}

/// The line of the next message of a work that lists its input by number.
fn next_listed(e: &mut Engine, work: i64, number: usize) -> String {
    let m = engine::next_message(e, work).unwrap().text;
    work_from(&m)
        .into_iter()
        .find(|l| l.starts_with(&format!("{number}. ")))
        .unwrap_or_else(|| panic!("{m}"))
}

// `N1` is given the study of a new policy and the owner's answer to it only when the owner's latest view of a study, in
// the setting of the policy that ended last, said the policy in force stays, not when the owner was satisfied.
#[test]
fn the_study_reaches_the_fit_question_only_when_the_owner_said_the_policy_stays() {
    let s = setup("stays");
    let (mut e, _) = engine(&s, scripts());
    quiet(&mut e);
    let arch = architect_work_at(&mut e, "N1");
    let setting = setting_at(&mut e, "P4");
    let study =
        json!({ "outcome": { "study": { "intent": "Faster parsing." } }, "progress": "Studied.", "tell_owner": [] });
    let ruling = |choice: &str| json!({ "ruling": { "choice": choice, "text": "Seen." } });
    e.db.write(Actor::Engine, |tx| {
        record_round(tx, setting, 2, "P2", &study, "studied")?;
        record_round(tx, setting, 3, "P3", &json!({}), "held")?;
        record_round(tx, setting, 4, "P3", &ruling("satisfied"), "satisfied")
    })
    .unwrap();
    assert_eq!(next_listed(&mut e, arch, 9), "9. none");
    e.db.write(Actor::Engine, |tx| {
        record_round(tx, setting, 5, "P3", &ruling("policy_stays"), "policy_stays")
    })
    .unwrap();
    let stays = next_listed(&mut e, arch, 9);
    assert!(stays.contains("policy_stays") && !stays.ends_with("none"), "{stays}");
    let m = engine::next_message(&mut e, arch).unwrap().text;
    assert!(m.contains("Faster parsing."), "{m}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// `P1` is given only the signals of the policy in force that the engine measures, each with its numbers:
// a signal that names a measure of the numbers by its name. When the policy has none such, the input is none.
#[test]
fn the_policy_s_signals_come_only_when_the_engine_measures_them_with_their_numbers() {
    let s = setup("signals");
    let (mut e, _) = engine(&s, scripts());
    quiet(&mut e);
    let set = |e: &mut Engine, signals: Value| {
        let v = e.flow_now;
        let policy = json!({ "direction": "d", "why": "w", "principles": [{ "no": 1, "text": "t" }],
                             "signals": signals });
        let written =
            json!({ "outcome": { "policy": policy, "probes": [] }, "progress": "Written.", "tell_owner": [] });
        e.db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO works (item, flow_version, graph, at, ended, project)
                 VALUES (NULL, ?1, 'architect', 'S1', 'over', 'p')",
                [v],
            )?;
            let set = tx.last_insert_rowid();
            record_round(tx, set, 1, "P4", &written, "written")?;
            record_round(tx, set, 2, "P12", &json!({}), "next")
        })
        .unwrap();
    };
    set(
        &mut e,
        json!([{ "what": "Fewer items merged", "how": "merged below 1 a week" },
               { "what": "The owner is unhappy", "how": "something the owner says" }]),
    );
    let arch = architect_work_at(&mut e, "P1");
    let m = engine::next_message(&mut e, arch).unwrap().text;
    let signals = given_value(&m, "·engine·policy_signals").expect(&m);
    assert_eq!(
        signals["1"]["numbers"]["merged"]["by_week"],
        json!([0, 0, 0, 0]),
        "{signals}"
    );
    assert_eq!(signals.get("2"), None, "{signals}");
    // The works running together and the works ended unmerged are measures too: a signal that names them has their
    // numbers for each of the four weeks, zero where nothing ran or ended.
    set(
        &mut e,
        json!([{ "what": "Works run in parallel", "how": "running together at most 2 works" },
               { "what": "Works end without a merge", "how": "ended unmerged above 1 a week" }]),
    );
    let m = engine::next_message(&mut e, arch).unwrap().text;
    let signals = given_value(&m, "·engine·policy_signals").expect(&m);
    let ran = json!({ "most_works": 0, "seconds_two_or_more": 0.0 });
    assert_eq!(
        signals["1"]["numbers"]["running_together"]["by_week"],
        json!([ran, ran, ran, ran]),
        "{signals}"
    );
    let ended = json!({ "flow_moved": 0, "item_cancelled": 0, "started_over": 0, "no_merge_at_end": 0, "other": 0,
                        "total": 0 });
    assert_eq!(
        signals["2"]["numbers"]["ended_unmerged"]["by_week"],
        json!([ended, ended, ended, ended]),
        "{signals}"
    );
    set(
        &mut e,
        json!([{ "what": "The owner is unhappy", "how": "something the owner says" }]),
    );
    assert_eq!(next_listed(&mut e, arch, 3), "3. none");
    let _ = std::fs::remove_dir_all(&s.root);
}

/// An item of project p, ready to start: in no queue, held by nothing.
fn a_ready_item(e: &mut Engine, title: &str) -> i64 {
    e.db.write(Actor::Owner, |tx| {
        tx.execute(
            "INSERT INTO items (project, title, problem, why, deliver, level)
             VALUES ('p', ?1, 'Missing.', 'Needed.', 'change', 'l')",
            [title],
        )?;
        Ok(tx.last_insert_rowid())
    })
    .unwrap()
}

/// A matter of project p's architect's queue, as the owner's words.
fn a_matter(tx: &db::rusqlite::Transaction<'_>, what: &str, item: Option<i64>, body: Value) -> Result<i64, db::Error> {
    db::queue_matter(
        tx,
        &db::Matter {
            project: "p".into(),
            kind: "owner_words".into(),
            detail: None,
            what: what.into(),
            body,
            item,
            raised: None,
        },
    )
}

// While an architect work's batch is open, the engine starts no work on an item a change the batch collected names;
// once the batch's changes are made, the item starts as ready_items says.
#[test]
fn an_item_a_batch_changes_does_not_start_until_the_batch_ends() {
    let s = setup("batch-hold");
    let (mut e, said) = engine(&s, scripts());
    quiet(&mut e);
    let item = a_ready_item(&mut e, "Lex it");
    let arch = architect_work_at(&mut e, "Q0");
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "UPDATE works SET paused = 'later', paused_by = 'owner' WHERE id = ?1",
            [arch],
        )?;
        record_round(tx, arch, 1, "H8", &json!({}), "done")?;
        let m = a_matter(tx, "Order the items.", None, json!({}))?;
        let change = json!({ "kind": "add_dependency", "item": item.to_string(), "on": "99", "dependency": "blocking",
                             "reason": "r" });
        tx.execute(
            "INSERT INTO architect_item_changes (work, round, matter, change, state)
             VALUES (?1, 1, ?2, ?3, 'collected')",
            params![arch, m, change.to_string()],
        )?;
        Ok(())
    })
    .unwrap();
    let held = e.step(None).unwrap();
    assert_eq!(held.started_item, None, "{}", said.borrow().join("\n"));
    e.db.write(Actor::Engine, |tx| {
        tx.execute("UPDATE architect_item_changes SET state = 'made'", [])?;
        Ok(())
    })
    .unwrap();
    let started = e.step(None).unwrap();
    assert_eq!(started.started_item, Some(item), "{}", said.borrow().join("\n"));
    let _ = std::fs::remove_dir_all(&s.root);
}

// An item that depends on another by a blocking dependency does not start, though the owner queued it first, while the
// other is not merged: the other starts instead.
#[test]
fn an_item_waits_to_start_for_what_it_depends_on_by_a_blocking_dependency() {
    let s = setup("blocking");
    let (mut e, said) = engine(&s, scripts());
    quiet(&mut e);
    let library = a_ready_item(&mut e, "Make the lexer");
    let parser = a_ready_item(&mut e, "Parse with it");
    e.db.write(Actor::Owner, |tx| {
        db::add_dependency(tx, parser, library, "its lexer", "blocking")?;
        tx.execute("INSERT INTO dispatch_queue (item, position) VALUES (?1, 1.0)", [parser])?;
        Ok(())
    })
    .unwrap();
    let started = e.step(None).unwrap();
    assert_eq!(started.started_item, Some(library), "{}", said.borrow().join("\n"));
    let _ = std::fs::remove_dir_all(&s.root);
}

// A work that waits for the items its item depends on (`A6`) goes on at once to choose the way on
// (`A4`) when one of them is cancelled, told which, why and what it needed of it; its item no longer depends on
// the cancelled one.
#[test]
fn a_work_waiting_for_a_cancelled_item_goes_on_to_choose_the_way_on() {
    let s = setup("cancelled-dependency");
    let (mut e, said) = engine(&s, scripts());
    quiet(&mut e);
    let lexer = a_ready_item(&mut e, "Make the lexer");
    let parser = a_ready_item(&mut e, "Parse with it");
    let v = e.flow_now;
    let work =
        e.db.write(Actor::Engine, |tx| {
            db::add_dependency(tx, parser, lexer, "its tokens", "parallel")?;
            tx.execute(
                "INSERT INTO works (item, flow_version, graph, at) VALUES (?1, ?2, 'engineer', 'A6')",
                params![parser, v],
            )?;
            Ok(tx.last_insert_rowid())
        })
        .unwrap();
    let at = |e: &Engine| -> String {
        e.db.conn()
            .query_row("SELECT at FROM works WHERE id = ?1", [work], |r| r.get(0))
            .unwrap()
    };
    e.step_work(work).unwrap();
    assert_eq!(at(&e), "A6", "{}", said.borrow().join("\n"));
    let projects = e.config.config.projects.clone();
    e.db.write(Actor::Owner, |tx| {
        engine::ending::cancel_item(tx, lexer, "no longer needed", "owner", &projects).map_err(db::Error::Abort)
    })
    .unwrap();
    e.step_work(work).unwrap();
    assert_eq!(at(&e), "A4", "{}", said.borrow().join("\n"));
    let told: String =
        e.db.conn()
            .query_row("SELECT text FROM interjections WHERE work = ?1", [work], |r| r.get(0))
            .unwrap();
    assert!(
        told.contains("its tokens") && told.contains("no longer needed"),
        "{told}"
    );
    let links: i64 =
        e.db.conn()
            .query_row("SELECT count(*) FROM item_links", [], |r| r.get(0))
            .unwrap();
    assert_eq!(links, 0);
    let _ = std::fs::remove_dir_all(&s.root);
}

// `N1` hands in one row for each place to set straight, with every step it means to take there, and the
// engine queues the row as one matter, which says its steps.
#[test]
fn a_place_to_set_straight_is_one_matter_with_its_steps() {
    let s = setup("straight");
    let mut agents = scripts();
    agents.insert(
        "N1",
        Box::new(|_: &Request, _: usize| {
            let fixes = json!([{ "where": "The policy's second stage", "items": [], "works": [], "kind": "missing",
                                 "wrong": "No item does it.",
                                 "steps": ["Make the items lex and parse", "Rank them before #5",
                                           "Have #5 wait for parse"] }]);
            (
                vec![("outcome.fit", "some_to_change")],
                vec![("outcome.fit.fixes", fixes)],
            )
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    // A new session understands the project from its first question on.
    let arch = architect_work_at(&mut e, "U1");
    let straight = |e: &Engine| -> Vec<String> {
        engine::query_json(
            e.db.conn(),
            "SELECT what FROM architect_matters WHERE kind = 'set_straight'",
            &[],
            10,
        )
        .unwrap()
        .iter()
        .filter_map(|r| r["what"].as_str().map(str::to_string))
        .collect()
    };
    for _ in 0..6 {
        if !straight(&e).is_empty() {
            break;
        }
        e.step_work(arch).unwrap();
    }
    assert_eq!(
        straight(&e),
        [
            "To set straight, The policy's second stage: Make the items lex and parse; Rank them before #5; Have #5 wait \
          for parse"
        ],
        "{}",
        said.borrow().join("\n")
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// An item change may name an item an earlier change of the batch creates by its key: a new item is placed before
// another in the order works start in, and the engine gives it its number as it makes the batch.
#[test]
fn a_change_names_an_item_the_batch_creates_by_its_key() {
    let s = setup("keys");
    let mut agents = scripts();
    let changes = json!([
        { "kind": "create_item", "key": "spend", "project": "p", "title": "Measure the spend", "problem": "Unknown.",
          "why": "Cost.", "acceptance": ["It is measured."], "level": "l", "deliver": "change", "reason": "r" },
        { "kind": "move", "item": "spend", "to": { "place": "before", "item": "1" }, "reason": "r" },
    ]);
    agents.insert(
        "H8",
        Box::new(move |_: &Request, _: usize| {
            (
                Vec::new(),
                vec![("outcome.effects", json!({ "item_changes": changes.clone() }))],
            )
        }),
    );
    agents.insert(
        "B2",
        Box::new(|_: &Request, _: usize| {
            let ok = |n: i64| json!({ "change": n, "verdict": "ok", "why": "Sound." });
            (Vec::new(), vec![("outcome.review", json!([ok(1), ok(2)]))])
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    let first = an_item(&mut e);
    say(&mut e, "Measure the spend before parsing.");
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, arch)),
        "{}",
        said.borrow().join("\n")
    );
    let order: Vec<(i64, String)> = engine::query_json(
        e.db.conn(),
        "SELECT id, title FROM items WHERE project = 'p' ORDER BY rank NULLS LAST, id",
        &[],
        10,
    )
    .unwrap()
    .iter()
    .map(|r| (r["id"].as_i64().unwrap(), r["title"].as_str().unwrap().to_string()))
    .collect();
    let log = said.borrow().join("\n");
    assert_eq!(order.len(), 2, "{log}");
    assert_eq!(order[0].1, "Measure the spend", "{log}");
    assert_eq!(order[1].0, first, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A design card matter whose card needs no look any longer, confirmed by the owner or replaced by another, is closed by
// the engine as it looks at the queue, saying why, and never reaches `H6`.
#[test]
fn a_design_card_confirmed_or_replaced_is_closed_without_a_look() {
    let s = setup("settled-card");
    let (mut e, said) = engine(&s, scripts());
    quiet(&mut e);
    let item = an_item(&mut e);
    let v = e.flow_now;
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO works (item, flow_version, graph, at, ended) VALUES (?1, ?2, 'engineer', 'I1', 'stopped: by hand')",
            params![item, v],
        )?;
        let work = tx.last_insert_rowid();
        record_round(tx, work, 1, "I1", &json!({}), "done")?;
        let confirmed = json!([{ "by": "owner", "at": 1, "decisions": [{ "decision": 1, "verdict": "confirm" }] }]);
        tx.execute(
            "INSERT INTO design_cards (work, round, body, status, rulings) VALUES (?1, 1, '[{\"node\":\"a\"}]', 'ruled', ?2)",
            params![work, confirmed.to_string()],
        )?;
        tx.execute(
            "INSERT INTO design_cards (work, round, body, status) VALUES (?1, 1, '[{\"node\":\"a\"}]', 'replaced')",
            [work],
        )?;
        for card in [1, 2] {
            db::queue_matter(
                tx,
                &db::Matter {
                    project: "p".into(),
                    kind: "design_card".into(),
                    detail: None,
                    what: format!("Design card {card} to look at"),
                    body: json!({ "design_card": card }),
                    item: Some(item),
                    raised: None,
                },
            )?;
        }
        Ok(())
    })
    .unwrap();
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 30, |e| architect_work_done(e, arch)),
        "{}",
        said.borrow().join("\n")
    );
    let log = said.borrow().join("\n");
    assert!(!path_of(&e, arch).contains(&"H6".to_string()), "{log}");
    let closed: Vec<(String, String)> = engine::query_json(
        e.db.conn(),
        "SELECT r.cell, json_extract(r.output, '$.progress') AS said FROM architect_matters m
         JOIN rounds r ON r.work = m.handled_work AND r.n = m.handled_round WHERE m.kind = 'design_card'",
        &[],
        10,
    )
    .unwrap()
    .iter()
    .map(|r| {
        (
            r["cell"].as_str().unwrap().to_string(),
            r["said"].as_str().unwrap_or_default().to_string(),
        )
    })
    .collect();
    assert_eq!(closed.len(), 2, "{log}");
    assert!(closed.iter().all(|c| c.0 == "Q0"), "{closed:?}");
    assert!(
        closed[0].1.contains("confirmed by the owner") && closed[0].1.contains("replaced by another"),
        "{closed:?}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// The items a matter concerns are read from its fields only: the item it concerns, and in its fix the item, the items
// and the item of each work it names. A number in a text, its words', a step's or a pull request's, brings in no item.
#[test]
fn a_matter_s_items_are_read_from_its_fields_never_its_text() {
    let s = setup("matter-items");
    let (mut e, _) = engine(&s, scripts());
    quiet(&mut e);
    let concerned = a_ready_item(&mut e, "Lex it");
    let fixed = a_ready_item(&mut e, "Parse it");
    let listed = a_ready_item(&mut e, "Check it");
    let worked = a_ready_item(&mut e, "Run it");
    let only_said = a_ready_item(&mut e, "Print it");
    let arch = architect_work_at(&mut e, "H8");
    let v = e.flow_now;
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO works (item, flow_version, graph, at) VALUES (?1, ?2, 'engineer', 'R1')",
            params![worked, v],
        )?;
        let work = tx.last_insert_rowid();
        let fix = json!({ "item": fixed, "items": [listed], "works": [work],
                          "steps": [format!("Move item {only_said} first, as matter #{only_said} said.")] });
        let body = json!({ "fix": fix, "said": format!("See PR #{only_said} for the parser.") });
        let m = a_matter(tx, &format!("Look at item {only_said}."), Some(concerned), body)?;
        record_round(
            tx,
            arch,
            1,
            "Q0",
            &json!({ "queue": { "verdict": "one", "matter": m } }),
            "one",
        )
    })
    .unwrap();
    let m = engine::next_message(&mut e, arch).unwrap().text;
    let items = given_value(&m, "·engine·matter_items").expect(&m);
    let keys: Vec<&String> = items.as_object().unwrap().keys().collect();
    let want: Vec<String> = [concerned, fixed, listed, worked]
        .iter()
        .map(|i| format!("#{i}"))
        .collect();
    assert_eq!(keys, want.iter().collect::<Vec<_>>(), "{items}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A side report a work handed in before, word for word, is not queued again.
#[test]
fn a_side_report_handed_in_again_is_queued_once() {
    let s = setup("report-again");
    let mut agents = scripts();
    agents.insert(
        "R1",
        Box::new(|_: &Request, n: usize| {
            let r = |what: &str| json!({ "kind": "other", "what": what, "urgent": false });
            let reports = if n == 1 {
                json!([
                    r("The parser is slow."),
                    r(" The parser is slow.\n"),
                    r("It needs a machine.")
                ])
            } else {
                json!([])
            };
            (Vec::new(), vec![("side_reports", reports)])
        }),
    );
    let (mut e, said, _) = start(&s, agents);
    let work = e.step(None).unwrap().work;
    e.step(work).unwrap();
    let log = said.borrow().join("\n");
    assert_eq!(
        matters(&e),
        [("report".to_string(), true, false), ("report".to_string(), true, false)],
        "{log}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// A new session of the architect's line that would start in the middle of an architect work, here at a matter's path,
// holds no state of things: the work goes back to understanding the project first, in the new session, and the matter
// picked goes back to the queue, to be picked again and handled.
#[test]
fn a_new_session_in_the_middle_of_an_architect_work_understands_the_project_first() {
    let s = setup("new-session-midway");
    let mut agents = scripts();
    picks_first(&s, &mut agents);
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    say(&mut e, "One.");
    say(&mut e, "Two.");
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 20, |e| e.work(arch).unwrap().pos.at == "H8"),
        "{}",
        said.borrow().join("\n")
    );
    e.sessions.ask_new(api::Fresh::Architect("p".into()));
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, arch)),
        "{}",
        said.borrow().join("\n")
    );
    let log = said.borrow().join("\n");
    let path = path_of(&e, arch);
    // The work was at `H8`, the matter picked, when the new session was asked for: it took the row for that there.
    let back = path.iter().rposition(|c| c == "S1").expect(&log);
    assert_eq!(&path[back - 1..back + 3], ["H8", "S1", "U1", "P1"], "{path:?}\n{log}");
    assert!(path[back..].contains(&"H8".to_string()), "{path:?}");
    let lines = line_sessions(&e, arch, "architect");
    assert_eq!(lines.len(), 2, "{lines:?}\n{log}");
    assert!(lines[1].2.starts_with("U1,"), "{lines:?}");
    assert_eq!(
        matters(&e),
        [
            ("owner_words".to_string(), false, true),
            ("owner_words".to_string(), false, true)
        ],
        "{log}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// An observation an item takes in is folded into that item, not closed: a close that says it was folded is sent back,
// saying to fold it.
#[test]
fn an_observation_closed_as_folded_is_sent_back_to_be_folded() {
    let s = setup("close-folded");
    let mut agents = scripts();
    agents.insert(
        "H8",
        Box::new(|_: &Request, n: usize| {
            if n > 1 {
                return (Vec::new(), Vec::new());
            }
            let close = json!({ "kind": "close", "id": 1, "why": "folded", "text": "The new item takes it in.",
                                "reason": "An item does it." });
            (
                Vec::new(),
                vec![("outcome.effects", json!({ "observation_changes": [close] }))],
            )
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    e.db.write(Actor::Architect, |tx| {
        tx.execute(
            "INSERT INTO observations (project, what, source) VALUES ('p', 'Parsing is slow.', 'A report.')",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    say(&mut e, "Fold the slowness into an item.");
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, arch)),
        "{}",
        said.borrow().join("\n")
    );
    let rejected: Vec<String> = engine::query_json(
        e.db.conn(),
        "SELECT rejected FROM rounds WHERE work = ?1 AND cell = 'H8' AND rejected IS NOT NULL",
        &[&arch],
        10,
    )
    .unwrap()
    .iter()
    .map(|r| r["rejected"].to_string())
    .collect();
    assert_eq!(rejected.len(), 1, "{rejected:?}");
    assert!(rejected[0].contains("fold_observations"), "{rejected:?}");
    let closed: Option<String> =
        e.db.conn()
            .query_row("SELECT closed FROM observations WHERE id = 1", [], |r| r.get(0))
            .unwrap();
    assert_eq!(closed, None);
    let _ = std::fs::remove_dir_all(&s.root);
}

// A new observation of a project that has tags carries one of them at least: one with no tag is sent back, saying
// which tags there are, and the return handed in again with a tag records it.
#[test]
fn an_observation_with_no_tag_is_sent_back() {
    let s = setup("observation-untagged");
    let project = std::fs::read_to_string(s.data.join("projects/p.toml")).unwrap();
    write(
        &s.data,
        "projects/p.toml",
        &project.replace(
            "[ci]",
            "observation_tags = [{ name = \"perf\", desc = \"How fast it runs\" }]\n[ci]",
        ),
    );
    git(&s.data, &["commit", "-q", "-am", "observation tags"]);
    let mut agents = scripts();
    agents.insert(
        "H8",
        Box::new(|_: &Request, n: usize| {
            if n > 2 {
                return (Vec::new(), Vec::new());
            }
            let tags = if n == 1 { json!([]) } else { json!(["perf"]) };
            let new = json!({ "kind": "new", "what": "Parsing large files is slow.", "tags": tags,
                              "source": "The owner's words.", "refs": [], "reason": "Worth keeping." });
            (
                Vec::new(),
                vec![("outcome.effects", json!({ "observation_changes": [new] }))],
            )
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    say(&mut e, "Parsing large files is slow; keep it in mind.");
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, arch)),
        "{}",
        said.borrow().join("\n")
    );
    let rejected: Vec<String> = engine::query_json(
        e.db.conn(),
        "SELECT rejected FROM rounds WHERE work = ?1 AND cell = 'H8' AND rejected IS NOT NULL",
        &[&arch],
        10,
    )
    .unwrap()
    .iter()
    .map(|r| r["rejected"].to_string())
    .collect();
    assert_eq!(rejected.len(), 1, "{rejected:?}");
    assert!(
        rejected[0].contains("observation change 1: it carries no tag") && rejected[0].contains("perf"),
        "{rejected:?}"
    );
    let tags: Vec<Value> = engine::query_json(e.db.conn(), "SELECT tags FROM observations", &[], 10)
        .unwrap()
        .iter()
        .map(|r| r["tags"].clone())
        .collect();
    assert_eq!(tags, [json!(["perf"])], "{tags:?}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The owner's Start, Pause and Step of a work and of a project's architect, beside the global pause and start.

/// One turn of the engine's loop as `wf-agents run` takes it, which these tests play: the new sessions the owner asked
/// for handed to the session manager, the pauses of the works the owner started lifted once each may hold a slot, then
/// the step the owner's asks call for.
fn turn(e: &mut Engine, c: &api::Control, after: Option<i64>) -> engine::Stepped {
    let asked = c.asked();
    for w in c.take_new_sessions() {
        e.sessions.ask_new(w);
    }
    let (done, architects) = e
        .release_started(&asked.releasing, &asked.architects_releasing)
        .unwrap();
    c.released(&done, &architects);
    match asked.next() {
        api::Next::Step(w) => {
            let s = e.step_work(w).unwrap();
            c.stepped(Some(w));
            s
        }
        api::Next::Architect(p) => {
            let s = e.step_architect(&p).unwrap();
            c.stepped(s.work);
            s
        }
        api::Next::Pass => e.step(after).unwrap(),
        api::Next::Started => e
            .step_started(after, &asked.started, &asked.architects_started)
            .unwrap(),
        api::Next::Wait => engine::Stepped::default(),
    }
}

/// Takes turns of the loop, each after the work the last moved, and gives the works that moved, in order.
fn turns(e: &mut Engine, c: &api::Control, n: usize) -> Vec<i64> {
    let mut moved = Vec::new();
    let mut after = None;
    for _ in 0..n {
        let s = turn(e, c, after);
        if let Some(w) = s.work.filter(|_| !s.rounds.is_empty()) {
            moved.push(w);
        }
        after = s.work.or(after);
    }
    moved
}

/// The owner's Pause of a work, as the page sends it: the work's own pause written as the owner's, its Start over.
fn owner_pauses(e: &mut Engine, c: &api::Control, work: i64) {
    c.pause(&api::Fresh::Work(work));
    api::pause(&mut e.db, work, None).unwrap();
}

/// A setup whose one machine has two slots, and an engine over it with two items in the queue.
fn two_works(name: &str) -> (Setup, Engine, Rc<RefCell<Vec<String>>>) {
    let s = setup(name);
    let install = std::fs::read_to_string(s.data.join("install.toml")).unwrap();
    write(&s.data, "install.toml", &install.replace("slots = 1", "slots = 2"));
    git(&s.data, &["commit", "-q", "-am", "two slots"]);
    let (mut e, said, _) = start(&s, scripts());
    e.db.write(Actor::Owner, |tx| {
        tx.execute(
            "INSERT INTO items (project, title, problem, why, deliver, level) VALUES ('p', 'Add another', 'x', 'y', 'change', 'l')",
            [],
        )?;
        let id = tx.last_insert_rowid();
        tx.execute("INSERT INTO dispatch_queue (item, position) VALUES (?1, 2.0)", params![id])?;
        Ok(())
    })
    .unwrap();
    (s, e, said)
}

// Each work follows the global pause and start unless the owner started or paused it, and the later action wins: a
// work the owner starts goes on while every work is paused, and follows the global pause again once every work is
// paused after; a work the owner pauses stays paused through Start all, until the owner starts it; a work the owner
// takes one round of is paused by the owner after it.
#[test]
fn the_owner_s_later_start_or_pause_of_a_work_wins_over_the_global_state() {
    let (s, mut e, said) = two_works("own-start");
    let c = api::Control::default();
    // Every work paused, as the engine starts: nothing moves.
    assert_eq!(turns(&mut e, &c, 2), Vec::<i64>::new());
    c.set_paused(false);
    let moved = turns(&mut e, &c, 2);
    let log = said.borrow().join("\n");
    assert_eq!(moved.len(), 2, "{log}");
    let (one, two) = (moved[0], moved[1]);
    assert_ne!(one, two, "{log}");
    // Paused again: nothing moves, until the owner starts one work, which alone goes on.
    c.set_paused(true);
    assert_eq!(turns(&mut e, &c, 2), Vec::<i64>::new());
    c.start(api::Fresh::Work(one), false);
    let log = said.borrow().join("\n");
    assert_eq!(turns(&mut e, &c, 3), [one, one, one], "{log}");
    // A later global pause makes it follow the global state again.
    c.set_paused(true);
    assert_eq!(turns(&mut e, &c, 2), Vec::<i64>::new());
    // The owner pauses the other work: Start all runs the first and leaves the one the owner paused.
    owner_pauses(&mut e, &c, two);
    c.set_paused(false);
    let moved = turns(&mut e, &c, 3);
    assert!(!moved.is_empty() && !moved.contains(&two), "{moved:?}");
    assert_eq!(paused_by(&e, two).as_deref(), Some("owner"));
    // Its Start lifts its pause, as the owner, and it goes on.
    c.start(api::Fresh::Work(two), false);
    let moved = turns(&mut e, &c, 4);
    let log = said.borrow().join("\n");
    assert!(moved.contains(&two), "{moved:?}\n{log}");
    assert_eq!(paused_by(&e, two), None);
    assert!(c.asked().releasing.is_empty());
    let lifted: String = e
        .db
        .conn()
        .query_row(
            "SELECT actor FROM changes WHERE table_name = 'works' AND json_extract(old_row, '$.paused_by') = 'owner'
                 AND json_extract(new_row, '$.paused') IS NULL ORDER BY id DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(lifted, "owner");
    // One round the owner asks for runs whatever the global state, and leaves the work paused by the owner, which
    // Start all does not lift.
    c.set_paused(true);
    c.ask_step(one, false).unwrap();
    let stepped = turn(&mut e, &c, None);
    assert_eq!((stepped.work, stepped.rounds.len()), (Some(one), 1), "{stepped:?}");
    assert_eq!(paused_by(&e, one).as_deref(), Some("owner"));
    c.set_paused(false);
    let moved = turns(&mut e, &c, 3);
    assert!(!moved.contains(&one), "{moved:?}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A work the owner starts that is paused holds no slot: its pause is lifted only once its machine has a slot free, and
// until then it stays paused, its Start waiting.
#[test]
fn a_work_the_owner_starts_still_needs_a_free_slot() {
    let s = setup("own-start-slot");
    let (mut e, said, _) = start(&s, scripts());
    let c = api::Control::default();
    c.set_paused(false);
    let first = turn(&mut e, &c, None).work.unwrap();
    owner_pauses(&mut e, &c, first);
    e.db.write(Actor::Owner, |tx| {
        tx.execute(
            "INSERT INTO items (project, title, problem, why, deliver, level) VALUES ('p', 'Add another', 'x', 'y', 'change', 'l')",
            [],
        )?;
        let id = tx.last_insert_rowid();
        tx.execute("INSERT INTO dispatch_queue (item, position) VALUES (?1, 2.0)", params![id])?;
        Ok(())
    })
    .unwrap();
    let second = turn(&mut e, &c, Some(first)).work.unwrap();
    let log = said.borrow().join("\n");
    assert_ne!(second, first, "{log}");
    c.set_paused(true);
    c.start(api::Fresh::Work(first), false);
    assert_eq!(turns(&mut e, &c, 2), Vec::<i64>::new());
    assert_eq!(paused_by(&e, first).as_deref(), Some("owner"));
    assert!(c.asked().releasing.contains(&first));
    // The other work paused frees its slot: the first goes on.
    owner_pauses(&mut e, &c, second);
    let moved = turns(&mut e, &c, 2);
    let log = said.borrow().join("\n");
    assert_eq!(moved.first(), Some(&first), "{log}");
    assert_eq!(paused_by(&e, first), None);
    let _ = std::fs::remove_dir_all(&s.root);
}

// The owner's Start of a work the architect stopped is the owner's release of it: its pause is lifted as the owner's,
// and the architect is told in the owner's words.
#[test]
fn the_owner_s_start_of_a_work_the_architect_stopped_tells_the_architect() {
    let s = setup("own-release");
    let (mut e, said, item) = start(&s, scripts());
    let c = api::Control::default();
    c.set_paused(false);
    let work = turn(&mut e, &c, None).work.unwrap();
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "UPDATE works SET paused = ?2, paused_by = 'engine' WHERE id = ?1",
            params![work, format!("{}: it waits for the parser", db::STOPPED_BY_ARCHITECT)],
        )?;
        Ok(())
    })
    .unwrap();
    c.set_paused(true);
    c.start(api::Fresh::Work(work), false);
    let moved = turns(&mut e, &c, 1);
    let log = said.borrow().join("\n");
    assert_eq!(moved, [work], "{log}");
    assert_eq!(paused_by(&e, work), None);
    let told: (String, Option<i64>) =
        e.db.conn()
            .query_row(
                "SELECT body ->> 'text', item FROM architect_matters WHERE kind = 'owner_words'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
    assert_eq!(
        told,
        (
            format!("I started work {work}, which you had stopped (it waits for the parser): it goes on."),
            Some(item)
        )
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// The architect's works are taken the same way: started while every work is paused, a work of the architect starts when
// a matter is due and goes on; the owner's Pause of the architect pauses the work, and its Step leaves the work paused
// by the owner.
#[test]
fn the_architect_the_owner_starts_works_and_goes_on_while_every_work_is_paused() {
    let s = setup("own-start-architect");
    let (mut e, said) = engine(&s, scripts());
    quiet(&mut e);
    let c = api::Control::default();
    say(&mut e, "Look at the queue.");
    assert_eq!(turns(&mut e, &c, 2), Vec::<i64>::new());
    c.start(api::Fresh::Architect("p".into()), false);
    let moved = turns(&mut e, &c, 3);
    let log = said.borrow().join("\n");
    assert_eq!(moved.len(), 3, "{log}");
    let arch = moved[0];
    assert!(moved.iter().all(|w| *w == arch), "{moved:?}");
    assert_eq!(paused_by(&e, arch), None);
    // Paused, the work takes no step, and no other work of the architect starts.
    c.pause(&api::Fresh::Architect("p".into()));
    api::pause_architect(&mut e.db, "p").unwrap();
    assert_eq!(turns(&mut e, &c, 2), Vec::<i64>::new());
    assert_eq!(paused_by(&e, arch).as_deref(), Some("owner"));
    // Started again, its pause is lifted; a round asked for leaves it paused by the owner.
    c.start(api::Fresh::Architect("p".into()), false);
    assert_eq!(turns(&mut e, &c, 1), [arch]);
    assert_eq!(paused_by(&e, arch), None);
    c.ask_step_architect("p", false).unwrap();
    let stepped = turn(&mut e, &c, None);
    assert_eq!(stepped.work, Some(arch));
    assert_eq!(paused_by(&e, arch).as_deref(), Some("owner"));
    assert!(c.asked().architects_started.is_empty());
    assert_eq!(architect_works(&e), [arch]);
    let _ = std::fs::remove_dir_all(&s.root);
}

// The owner's Step or Start can ask for a new session: the round starts one for its cell's line, recorded as the
// owner's, given everything its cell needs; the session before ends and is otherwise left as it is, and the line's
// later rounds go on in the new one. For the architect, the new session is its work's.
#[test]
fn the_owner_asks_for_a_new_session_with_a_step_or_a_start() {
    let s = setup("own-new-session");
    let (mut e, said, item) = start(&s, scripts());
    agent_rounds_to(&mut e, item, 2);
    let (work, at, _) = state(&e, item).unwrap();
    assert_eq!(at, "D1", "{}", said.borrow().join("\n"));
    let c = api::Control::default();
    c.ask_step(work, true).unwrap();
    turn(&mut e, &c, None);
    let log = said.borrow().join("\n");
    let lines = line_sessions(&e, work, "engineer");
    assert_eq!(
        lines,
        [
            ("line".to_string(), true, "R1,R2".to_string()),
            ("owner".to_string(), false, "D1".to_string())
        ],
        "{log}"
    );
    let rounds = agent_rounds(&e, item);
    let two = &rounds[2].2;
    for a in ["format:common", "format:D1", "rule:ground"] {
        assert_eq!(given(two, a), Some(true), "{a}");
    }
    assert!(two["call"]["session"]["started"].is_string(), "{}", two["call"]);
    // Started as it is, the work's next round goes on in that session; started in a new session, the next starts one.
    c.start(api::Fresh::Work(work), false);
    let n = agent_rounds(&e, item).len();
    for _ in 0..10 {
        if agent_rounds(&e, item).len() > n {
            break;
        }
        turn(&mut e, &c, None);
    }
    let lines = line_sessions(&e, work, "engineer");
    assert_eq!(lines.len(), 2, "{lines:?}\n{}", said.borrow().join("\n"));
    owner_pauses(&mut e, &c, work);
    c.start(api::Fresh::Work(work), true);
    let n = agent_rounds(&e, item).len();
    for _ in 0..10 {
        if agent_rounds(&e, item).len() > n {
            break;
        }
        turn(&mut e, &c, None);
    }
    let lines = line_sessions(&e, work, "engineer");
    let log = said.borrow().join("\n");
    assert_eq!(lines.len(), 3, "{lines:?}\n{log}");
    assert!(lines[1].1, "{lines:?}");
    assert_eq!((lines[2].0.as_str(), lines[2].1), ("owner", false), "{lines:?}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The architect's Step in a new session starts one for its work, recorded as the owner's; the work's next rounds go on
// in it, the understanding of the project first, since the session is new in the middle of the work: the cell it would
// have started at takes the row back to understanding.
#[test]
fn the_owner_asks_for_a_new_session_for_the_architect_s_work() {
    let s = setup("own-new-architect-work-session");
    let (mut e, said) = engine(&s, scripts());
    quiet(&mut e);
    let c = api::Control::default();
    say(&mut e, "Look at the queue.");
    c.start(api::Fresh::Architect("p".into()), false);
    // Starting takes the engine's own step, which finds the session has not understood the project; then the first
    // round.
    let arch = turns(&mut e, &c, 2)[0];
    let log = said.borrow().join("\n");
    let lines = line_sessions(&e, arch, "architect");
    assert_eq!(lines.len(), 1, "{lines:?}\n{log}");
    c.pause(&api::Fresh::Architect("p".into()));
    c.ask_step_architect("p", true).unwrap();
    turn(&mut e, &c, None);
    for _ in 0..3 {
        c.ask_step_architect("p", false).unwrap();
        turn(&mut e, &c, None);
    }
    let log = said.borrow().join("\n");
    assert_eq!(path_of(&e, arch), ["S1", "U1", "P1", "S1", "U1", "P1"], "{log}");
    let lines = line_sessions(&e, arch, "architect");
    assert_eq!(lines.len(), 2, "{lines:?}\n{log}");
    assert!(lines[0].1, "{lines:?}");
    assert_eq!((lines[1].0.as_str(), lines[1].1), ("owner", false), "{lines:?}");
    assert_eq!(lines[1].2, "U1,P1", "{lines:?}\n{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// While every work is paused, a work the pause holds waits on it: its session is kept warm as the session manager's
// rules say, unless the owner switched keeping warm off, which holds across restarts; a work the owner started runs,
// and is not kept warm for the pause.
#[test]
fn sessions_are_kept_warm_while_every_work_is_paused_unless_the_owner_switches_it_off() {
    let s = setup("warm-paused");
    let mut scripts = scripts();
    scripts.insert(
        "R2",
        Box::new(|_: &Request, _: usize| (Vec::new(), vec![("@usage", used(1.0, 9_000))])),
    );
    let (mut e, said, item) = start(&s, scripts);
    agent_rounds_to(&mut e, item, 2);
    let (work, at, _) = state(&e, item).unwrap();
    assert_eq!(at, "D1", "{}", said.borrow().join("\n"));
    let from = last_call(&e, work);
    let due = from + 55 * 60_000 + 1_000;
    // Works going on wait on nothing.
    assert_eq!(e.keep_warm(due, None).unwrap(), 0);
    let paused = |warm: bool, runs: &[i64]| engine::GlobalPause {
        warm,
        runs: runs.iter().copied().collect(),
        architects: Default::default(),
    };
    assert_eq!(e.keep_warm(due, Some(&paused(false, &[]))).unwrap(), 0);
    assert_eq!(e.keep_warm(due, Some(&paused(true, &[work]))).unwrap(), 0);
    assert_eq!(
        e.keep_warm(due, Some(&paused(true, &[]))).unwrap(),
        1,
        "{}",
        said.borrow().join("\n")
    );
    assert_eq!(
        pings_of(&e, work)[0].0,
        json!({ "cell": "D1", "on": "pause", "what": "the owner paused every work" })
    );
    // The owner's switch is kept in the database, as the owner's, across a restart.
    assert!(api::keep_warm_paused(e.db.conn()).unwrap());
    api::set_keep_warm_paused(&mut e.db, false).unwrap();
    drop(e);
    let (e, _) = engine(&s, BTreeMap::new());
    assert!(!api::keep_warm_paused(e.db.conn()).unwrap());
    let _ = std::fs::remove_dir_all(&s.root);
}

/// The same sequence of numbers for the same seed: the random tests' dice.
struct Dice(std::cell::Cell<u64>);

impl Dice {
    fn new(seed: u64) -> Dice {
        Dice(std::cell::Cell::new(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1))
    }

    /// A number below `n`.
    fn roll(&self, n: u64) -> u64 {
        let mut x = self.0.get();
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0.set(x);
        x % n.max(1)
    }
}

/// What a round hands in: its picks and what it sets.
type Picked = (Vec<(&'static str, &'static str)>, Vec<(&'static str, Value)>);

/// An ask of `to`, of a kind that role may be asked.
fn an_ask(to: &'static str) -> Picked {
    let kind = match to {
        "advisor" => "unsure",
        "architect" => "split",
        _ => "permission",
    };
    (
        vec![
            ("outcome", "ask"),
            ("outcome.ask", to),
            ("outcome.ask.points[].kind", kind),
        ],
        vec![("outcome.ask.points[].what", json!(format!("A point for the {to}.")))],
    )
}

/// What a round hands in, at random: an ask of the advisor, the architect or the owner, or its own answer.
fn random_ask(dice: &Dice) -> Picked {
    match dice.roll(10) {
        0 | 1 => an_ask("advisor"),
        2 | 3 => an_ask("architect"),
        4 => an_ask("owner"),
        _ => (Vec::new(), Vec::new()),
    }
}

/// A work of the engineer's graph not ended, at random, if there is one.
fn a_work(e: &Engine, dice: &Dice) -> Option<i64> {
    let ids: Vec<i64> = engine::query_json(
        e.db.conn(),
        "SELECT id FROM works WHERE graph = 'engineer' AND ended IS NULL ORDER BY id",
        &[],
        1000,
    )
    .unwrap()
    .iter()
    .filter_map(|r| r["id"].as_i64())
    .collect();
    (!ids.is_empty()).then(|| ids[dice.roll(ids.len() as u64) as usize])
}

/// The asks of the owner that engineers' works put and the owner has not answered.
fn open_asks(e: &Engine) -> Vec<i64> {
    engine::query_json(
        e.db.conn(),
        "SELECT a.id FROM owner_asks a JOIN works w ON w.id = a.work
         WHERE a.answer IS NULL AND w.graph = 'engineer' ORDER BY a.id",
        &[],
        100,
    )
    .unwrap()
    .iter()
    .filter_map(|r| r["id"].as_i64())
    .collect()
}

/// One random sequence of events over three items of project p, beside project q, related to it: steps of the engine,
/// whose rounds ask the advisor, the architect and the owner, hand questions on (the advisor to the architect or the
/// owner, an architect to the owner or to the other project's architect, the engineer to the owner after waiting
/// long), and reply, sometimes with an item to wait for, of either project; the owner's answers, which now and then
/// hand the question back to the architect; the architects' hours, past their limit; interjections; cancels; and
/// takeovers by a new flow version. After each, every chain of blockers closes.
fn random_events(seed: u64, events: usize) {
    let s = setup(&format!("blockers-{seed}"));
    let install = std::fs::read_to_string(s.data.join("install.toml")).unwrap();
    write(
        &s.data,
        "install.toml",
        &format!(
            "{}\n[agents.levels.m]\ndesc = \"one level up\"\ncli = \"claude\"\nmodel = \"m-up\"\n",
            install.replace("slots = 1", "slots = 2")
        ),
    );
    let q = s.root.join("q-checkout");
    git(
        &s.root,
        &["clone", "-q", &s.origin.to_string_lossy(), &q.to_string_lossy()],
    );
    let p_file = std::fs::read_to_string(s.data.join("projects/p.toml")).unwrap();
    let checkout = s.root.join("checkout");
    write(
        &s.data,
        "projects/p.toml",
        &p_file.replace("rules = [\"ground\"]", "rules = [\"ground\"]\nother_projects = [\"q\"]"),
    );
    write(
        &s.data,
        "projects/q.toml",
        &p_file
            .replace("o/p", "o/q")
            .replace(&checkout.display().to_string(), &q.display().to_string())
            .replace("rules = [\"ground\"]", "rules = [\"ground\"]\nother_projects = [\"p\"]"),
    );
    git(&s.data, &["add", "-A"]);
    git(
        &s.data,
        &["commit", "-q", "-m", "a level above, two slots, and a related project"],
    );
    let dice = Rc::new(Dice::new(seed));
    let mut agents = scripts();
    for cell in ["R1", "D1", "T3"] {
        let d = dice.clone();
        agents.insert(cell, Box::new(move |_: &Request, _: usize| random_ask(&d)));
    }
    // The advisor answers, or hands the question to the architect or the owner.
    let d = dice.clone();
    agents.insert(
        "A1",
        Box::new(move |_: &Request, _: usize| match d.roll(4) {
            0 => an_ask("architect"),
            1 => an_ask("owner"),
            _ => (Vec::new(), Vec::new()),
        }),
    );
    let path = config::database(&s.data);
    // An architect replies, now and then with an item to wait for, made after the work's own so that no cycle closes,
    // of either project; or hands the question to the owner, or to the other project's architect.
    let (d, p) = (dice.clone(), path.clone());
    agents.insert(
        "H3",
        Box::new(move |r: &Request, _: usize| {
            let c = db::open_read(&p).unwrap();
            let project = project_of(r);
            let asked: Option<i64> = c
                .query_row(
                    "SELECT m.item FROM architect_matters m
                     WHERE m.project = ?1 AND m.kind = 'engineer_question' AND m.handled_round IS NULL
                       AND m.withdrawn IS NULL
                     ORDER BY m.id LIMIT 1",
                    [&project],
                    |x| x.get(0),
                )
                .ok()
                .flatten();
            let order = if d.roll(2) == 0 { "ASC" } else { "DESC" };
            let later: Option<i64> = asked.and_then(|i| {
                c.query_row(
                    &format!("SELECT id FROM items WHERE id > ?1 AND cancelled IS NULL ORDER BY id {order} LIMIT 1"),
                    [i],
                    |x| x.get(0),
                )
                .ok()
            });
            let other = if project == "p" { "q" } else { "p" };
            let mut reply = json!({ "text": "Go on as you planned.", "why": "It holds." });
            match d.roll(6) {
                0 => reply["hand_over"] = json!({ "to": "owner", "why": "Only the owner decides this." }),
                1 => reply["hand_over"] = json!({ "to": "architect", "project": other, "why": "It is theirs." }),
                2 => {
                    if let Some(i) = later {
                        reply["waits"] =
                            json!([{ "key": i.to_string(), "title": "the other", "delivers": "its part" }]);
                    }
                }
                _ => {}
            }
            (Vec::new(), vec![("outcome.reply", reply)])
        }),
    );
    // Choosing the way on waits when the reply says to and carries on otherwise, but now and then chooses wrong,
    // which asks the owner.
    let (d, p) = (dice.clone(), path.clone());
    agents.insert(
        "A4",
        Box::new(move |r: &Request, _: usize| {
            let c = db::open_read(&p).unwrap();
            let waits: Option<String> = c
                .query_row(
                    "SELECT json_extract(output, '$.reply.waits') FROM rounds WHERE work = ?1 AND cell = 'A2'
                       AND transition IS NOT NULL ORDER BY n DESC LIMIT 1",
                    [r.work],
                    |x| x.get(0),
                )
                .ok()
                .flatten();
            let next = match (waits.is_some(), d.roll(8)) {
                (true, 0) | (false, 1..) => "carry_on",
                _ => "wait_for_items",
            };
            (vec![("outcome.next", next)], Vec::new())
        }),
    );
    // The pick takes the first matter that can be picked: flow feedback that came after this work started waits.
    let p = path.clone();
    agents.insert(
        "Q1",
        Box::new(move |r: &Request, _: usize| {
            let c = db::open_read(&p).unwrap();
            let first: i64 = c
                .query_row(
                    "SELECT matter FROM architect_queue
                     WHERE project = ?1 AND NOT (kind IN ('report', 'other_architect') AND detail = 'flow_feedback')
                     ORDER BY matter LIMIT 1",
                    [project_of(r)],
                    |x| x.get(0),
                )
                .unwrap();
            (Vec::new(), vec![("outcome.pick.matter", json!(first))])
        }),
    );
    // Whether to wait longer, at random.
    let d = dice.clone();
    agents.insert(
        "A7",
        Box::new(move |_: &Request, _: usize| {
            let v = ["wait_on", "to_owner", "withdraw"][d.roll(3) as usize];
            (vec![("outcome.longer", v)], Vec::new())
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    let mut items: Vec<i64> = (0..3)
        .map(|i| {
            e.db.write(Actor::Owner, |tx| {
                tx.execute(
                    "INSERT INTO items (project, title, problem, why, deliver, level)
                     VALUES ('p', ?1, 'It is missing.', 'People need it.', 'change', 'l')",
                    [format!("Add part {i}")],
                )?;
                let id = tx.last_insert_rowid();
                tx.execute(
                    "INSERT INTO dispatch_queue (item, position) VALUES (?1, ?2)",
                    params![id, i as f64],
                )?;
                Ok(id)
            })
            .unwrap()
        })
        .collect();
    // An item of q, which p's works may be had wait for; q starts no work of its own.
    let theirs =
        e.db.write(Actor::Owner, |tx| {
            tx.execute(
                "INSERT INTO items (project, title, problem, why, deliver, level)
                 VALUES ('q', 'Their part', 'It is missing.', 'p needs it.', 'change', 'l')",
                [],
            )?;
            tx.execute(
                "INSERT INTO dispatch_holds (project, reason, since, why) VALUES ('q', 'architect', 1, 'Not now.')",
                [],
            )?;
            Ok(tx.last_insert_rowid())
        })
        .unwrap();
    items.push(theirs);
    let mut after = None;
    let mut versions = 0;
    let mut log = Vec::new();
    for n in 0..events {
        let event = match dice.roll(100) {
            0..=11 => match open_asks(&e).first() {
                Some(ask) => {
                    let back = dice.roll(3) == 0;
                    let answer = api::Answer {
                        text: "Go on.".into(),
                        why: None,
                        reply: None,
                        choice: None,
                        to: back.then(|| "architect".to_string()),
                    };
                    match api::answer(&mut e.db, *ask, &answer) {
                        Ok(_) if back => format!("the owner hands ask {ask} back to the architect"),
                        Ok(_) => format!("the owner answers ask {ask}"),
                        // An ask an architect asked, which holds no task, is not handed back.
                        Err(why) => format!("the owner cannot answer ask {ask} so: {why}"),
                    }
                }
                None => "no ask to answer".into(),
            },
            12..=19 => match a_work(&e, &dice) {
                Some(w) => {
                    e.db.write(Actor::Owner, |tx| {
                        tx.execute(
                            "INSERT INTO interjections (work, line, speaker, text) VALUES (?1, 'engineer', 'owner', ?2)",
                            params![w, "Mind the tests."],
                        )?;
                        Ok(())
                    })
                    .unwrap();
                    format!("the owner interjects into work {w}")
                }
                None => "no work to interject into".into(),
            },
            20..=22 if n > events / 3 => {
                let item = items[dice.roll(items.len() as u64) as usize];
                let projects = e.config.config.projects.clone();
                e.db.write(Actor::Owner, |tx| {
                    engine::ending::cancel_item(tx, item, "no longer needed", "owner", &projects)
                        .map_err(db::Error::Abort)
                })
                .unwrap();
                format!("the owner cancels item {item}")
            }
            23..=25 => match a_work(&e, &dice) {
                Some(w) => {
                    versions += 1;
                    let now = e.flow_now;
                    e.db.write(Actor::Engine, |tx| {
                        tx.execute(
                            "INSERT INTO flow_versions (digest, compiled)
                             SELECT ?2, compiled FROM flow_versions WHERE id = ?1",
                            params![now, format!("older {versions}")],
                        )?;
                        let v = tx.last_insert_rowid();
                        tx.execute("UPDATE works SET flow_version = ?2 WHERE id = ?1", params![w, v])?;
                        Ok(())
                    })
                    .unwrap();
                    format!("work {w} is on an older flow version")
                }
                None => "no work to take over".into(),
            },
            // The architects had hours to act, in rounds of a work over by now: a question they hold is past its limit.
            26..=28 => {
                let v = e.flow_now;
                let now = db::now();
                e.db.write(Actor::Engine, |tx| {
                    for project in ["p", "q"] {
                        tx.execute(
                            "INSERT INTO works (item, flow_version, graph, at, ended, project)
                             VALUES (NULL, ?1, 'architect', 'E2', 'over before', ?2)",
                            params![v, project],
                        )?;
                        let arch = tx.last_insert_rowid();
                        tx.execute(
                            "INSERT INTO rounds (work, n, cell, input, output, transition, config_commit, started_at,
                               ended_at)
                             VALUES (?1, 1, 'H10', '{}', '{}', 'done', 'c', ?2, ?3)",
                            params![arch, now - 3 * 3_600_000, now],
                        )?;
                    }
                    Ok(())
                })
                .unwrap();
                "the architects spent three hours".into()
            }
            _ => {
                let stepped = e.step(after).unwrap();
                after = stepped.work;
                format!("a step of work {:?}: {:?}", stepped.work, stepped.cell)
            }
        };
        log.push(event);
        let broken = engine::blockers::invariants(&mut e).unwrap();
        assert!(
            broken.is_empty(),
            "seed {seed}, event {n}: {broken:?}\nevents:\n{}\nengine:\n{}",
            log.join("\n"),
            said.borrow().join("\n")
        );
    }
    // Nothing the engine could not do paused a work.
    let paused = engine::query_json(
        e.db.conn(),
        "SELECT id, paused FROM works WHERE paused_by = 'engine'",
        &[],
        100,
    )
    .unwrap();
    assert!(
        paused.is_empty(),
        "seed {seed}: {paused:?}\nevents:\n{}\nengine:\n{}",
        log.join("\n"),
        said.borrow().join("\n")
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

macro_rules! random_events {
    ($($name:ident: $seed:expr),*) => {$(
        #[test]
        fn $name() {
            random_events($seed, 70);
        }
    )*};
}

// Random sequences of events (asks of the advisor, the architect and the owner, hand-overs between them and between
// two related projects' architects, the architects' replies, the owner's answers, interjections, cancels, ends and
// takeovers by a new flow version) leave every chain of blockers closed after each event: every stopped task has
// exactly one open blocker, raised at the cell it stopped at and held by the holder of the cell it stands at; every
// open blocker's holder exists, in whichever project; no ended task has an open blocker; the items waited for form no
// cycle, across projects; and a chain of hand-overs between architects always has a current holder.
random_events!(
    every_chain_of_blockers_closes_under_random_events_1: 1,
    every_chain_of_blockers_closes_under_random_events_2: 2,
    every_chain_of_blockers_closes_under_random_events_3: 3,
    every_chain_of_blockers_closes_under_random_events_4: 4,
    every_chain_of_blockers_closes_under_random_events_5: 5
);
