//! One item through the whole engineer graph, from the queue to main, and the architect's wakes beside it: a scripted
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

/// How many returns of a cell in a row the flow sends back before it pauses the work.
fn pause_after() -> usize {
    let relations = db::relations(db::SCHEMA).unwrap();
    let f = flow::compile(
        flow::FLOW,
        &flow::Context {
            relations,
            tables: db::index_of(db::SCHEMA).unwrap(),
        },
    )
    .unwrap();
    f.graph(engine::ENGINEER)
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
        let pick: BTreeMap<String, String> = pick.into_iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
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
        set.entry("progress".into())
            .or_insert(json!(format!("round {n} at {}", r.cell)));
        Reply {
            output: Some(example(&r.schema, "", &pick, &set)),
            session,
            model: Some(r.model.clone()),
            usage,
            compactions,
            error: None,
            command: Vec::new(),
        }
    }

    fn version(&self, _cli: &config::Cli, _config_dir: Option<&Path>) -> String {
        "scripted".to_string()
    }
}

impl Scripted {
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

    fn gh(&self, _dir: &Path, args: &[String]) -> Result<(i32, String), Failure> {
        Ok((0, args.join(" ")))
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
        "design.choose",
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
        "design.rethink",
        Box::new(|_: &Request, _: usize| (vec![("outcome.better", "no")], Vec::new())),
    );
    s.insert(
        "design.adversarial_needed",
        Box::new(|_: &Request, _: usize| (vec![("outcome.adversarial", "no")], Vec::new())),
    );
    // Implementing: the first round changes the code and asks for ci; the second hands in the end.
    s.insert(
        "implement",
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
        "test.table_review",
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

/// Keeps project p's architect from waking for slots free while a test looks at something else: a matter of slots
/// free was handled just now, by a wake over before the test, so the engine raises no other for an hour.
fn quiet(e: &mut Engine) {
    let v = e.flow_now;
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO works (item, flow_version, graph, at, ended, project)
             VALUES (NULL, ?1, 'architect', 'record', 'over before the test', 'p')",
            [v],
        )?;
        let wake = tx.last_insert_rowid();
        tx.execute(
            "INSERT INTO rounds (work, n, cell, input, output, transition, config_commit, started_at, ended_at)
             VALUES (?1, 1, 'handle.what_runs', '{}', '{}', 'done', 'c', ?2, ?2)",
            params![wake, db::now()],
        )?;
        tx.execute(
            "INSERT INTO architect_matters (project, kind, what, body, at, handled_work, handled_round)
             VALUES ('p', 'slots_free', 'not yet', '{}', ?1, ?2, 1)",
            params![db::now(), wake],
        )?;
        Ok(())
    })
    .unwrap();
}

/// The engine over a setup, with what it says kept, and one item in the queue; the architect wakes for no advance.
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
    assert_eq!(at, "end", "{log}");
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
        "start:started",
        "implement:tools",
        "tools:ran",
        "test.full_ci:review_first",
        "test.blind_review:done",
        "merge.queue:merged",
        "watch_main:green",
        "end:-",
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
    let columns = input_of("test.table_review")["engine"]["columns"].to_string();
    assert!(!columns.contains("a gap in what is under test"), "{columns}");
    assert!(!columns.contains("cross-project dependency"), "{columns}");
    assert!(columns.contains("unsure facts"), "{columns}");
    // The review line's session is given the worktree once, at its first round, the blind review.
    let blind = input_of("test.blind_review");
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
    assert_eq!(at, "research.stands", "{log}");
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
    assert!(m.text.contains("cell research.stands"), "{}", m.text);
    drop(r);
    assert_eq!(state_of(), before);
    let _ = std::fs::remove_dir_all(&s.root);
}

// While the merge gate waits for the owner's ruling on the design card, the next round offers no round: it says what
// the work waits for, and which design card. The owner's confirmation alone lets the work through.
#[test]
fn the_merge_gate_waiting_for_a_ruling_offers_no_round() {
    let s = setup("gate-wait");
    let (mut e, said, item) = start(&s, scripts());
    for _ in 0..2 {
        e.step(None).unwrap();
    }
    let log = said.borrow().join("\n");
    let (work, _, _) = state(&e, item).unwrap();
    let wt: String =
        e.db.conn()
            .query_row("SELECT worktree FROM works WHERE id = ?1", [work], |r| r.get(0))
            .unwrap();
    let wt = PathBuf::from(wt);
    write(&wt, "design/engine.md", "Decision: a new one.\n");
    git(&wt, &["add", "-A"]);
    git(&wt, &["commit", "-q", "-m", "A decision"]);
    let head = git(&wt, &["rev-parse", "HEAD"]);
    let card =
        e.db.write(Actor::Engine, |tx| {
            let round: i64 = tx.query_row("SELECT max(n) FROM rounds WHERE work = ?1", [work], |r| r.get(0))?;
            // The round that handed the card in left the head with the decision.
            tx.execute(
                "UPDATE rounds SET input = json_set(input, '$.engine.head_after', ?3) WHERE work = ?1 AND n = ?2",
                params![work, round, head],
            )?;
            tx.execute(
                "INSERT INTO design_cards (work, round, body, status) VALUES (?1, ?2, '[]', 'open')",
                [work, round],
            )?;
            let card = tx.last_insert_rowid();
            tx.execute("UPDATE works SET at = 'merge.gate' WHERE id = ?1", [work])?;
            Ok(card)
        })
        .unwrap();
    let next = engine::next_round(&mut e, work).unwrap();
    assert_eq!(next.cell, "merge.gate", "{log}");
    assert_eq!(
        next.waits.as_deref(),
        Some(format!("the owner's ruling on design card {card}").as_str()),
        "{log}"
    );
    assert_eq!(next.design_card, Some(card));
    // The owner confirms, and the gate waits no longer.
    let confirm = api::Ruling {
        decisions: vec![api::DecisionRuling {
            decision: 1,
            verdict: "confirm".into(),
            text: None,
            option: None,
        }],
    };
    api::rule(&mut e.db, card, &confirm).unwrap();
    let next = engine::next_round(&mut e, work).unwrap();
    assert_eq!((next.waits, next.design_card), (None, None), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A cell that may not change the design paths is sent back for a change an earlier attempt of it made and left on the
// branch, with a reason that names that commit and says to revert it; once reverted, the cell's return is taken.
#[test]
fn an_earlier_attempts_design_change_is_named_and_reverted() {
    let s = setup("earlier-attempt");
    let mut agents = scripts();
    agents.insert(
        "design.rethink",
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
            (vec![("outcome.better", "no")], Vec::new())
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    let rethinks = |e: &Engine| -> Vec<(Option<String>, Option<String>)> {
        let mut st =
            e.db.conn()
                .prepare(
                    "SELECT r.transition, r.rejected FROM rounds r JOIN works w ON w.id = r.work
                     WHERE w.item = ?1 AND r.cell = 'design.rethink' AND r.ended_at IS NOT NULL ORDER BY r.work, r.n",
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
        "design.rethink",
        Box::new(move |r: &Request, n: usize| {
            if n == 1 {
                main_moves_on(&root, &[("design/main.md", "Decision: main's.\n")], "Main's decision");
                merge_main(&r.worktree);
            }
            (vec![("outcome.better", "no")], Vec::new())
        }),
    );
    agents.insert(
        "implement",
        implementing_over_a_merge(
            s.root.clone(),
            &[],
            &[("design/later.md", "Decision: main's later one.\n")],
        ),
    );
    let (mut e, said, item) = start(&s, agents);
    step_to(&mut e, item, "implement");
    let rethinks = ended_at(&e, item, "design.rethink");
    assert_eq!(rethinks.len(), 1, "{rethinks:?}\n{}", said.borrow().join("\n"));
    assert!(rethinks[0].0.is_some(), "{rethinks:?}");
    let (work, _, _) = state(&e, item).unwrap();
    let wt = worktree_of(&e, work);
    card_at(&mut e, work, &git(&wt, &["rev-parse", "HEAD"]));
    for _ in 0..10 {
        if ended_at(&e, item, "implement")
            .iter()
            .any(|(t, why)| t.as_deref() == Some("done") || why.is_some())
        {
            break;
        }
        e.step(None).unwrap();
    }
    let log = said.borrow().join("\n");
    let implemented = ended_at(&e, item, "implement");
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
        "design.rethink",
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
            (vec![("outcome.better", "no")], Vec::new())
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    for _ in 0..40 {
        if ended_at(&e, item, "design.rethink").iter().any(|(t, _)| t.is_some()) {
            break;
        }
        e.step(None).unwrap();
    }
    let log = said.borrow().join("\n");
    let rounds = ended_at(&e, item, "design.rethink");
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
        "implement",
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
    step_to(&mut e, item, "implement");
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
        if ended_at(&e, item, "implement").iter().any(|(_, why)| why.is_some()) {
            break;
        }
        e.step(None).unwrap();
    }
    let log = said.borrow().join("\n");
    let why = ended_at(&e, item, "implement")
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
    let implement = agents.remove("implement").unwrap();
    agents.insert(
        "implement",
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

// While a work waits for the owner's answer to an ask, its next round offers no round: it says which ask; once the
// ask is answered, the round that takes the answer is offered.
#[test]
fn an_ask_waiting_for_its_answer_offers_no_round() {
    let s = setup("ask-wait");
    let (mut e, _, item) = start(&s, scripts());
    e.step(None).unwrap();
    let (work, _, _) = state(&e, item).unwrap();
    let ask =
        e.db.write(Actor::Engine, |tx| {
            tx.execute("UPDATE works SET at = 'ask.owner' WHERE id = ?1", [work])?;
            let round = next_round(tx, work)?;
            tx.execute(
                "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES (?1, ?2, 'ask.owner', '{}', 'c', 1)",
                [work, round],
            )?;
            tx.execute(
                "INSERT INTO owner_asks (work, round, body) VALUES (?1, ?2, '{\"to\":\"owner\"}')",
                [work, round],
            )?;
            Ok(tx.last_insert_rowid())
        })
        .unwrap();
    let next = engine::next_round(&mut e, work).unwrap();
    assert_eq!(
        next.waits.as_deref(),
        Some(format!("your answer to ask {ask}").as_str())
    );
    assert_eq!(next.ask, Some(ask));
    e.db.write(Actor::Owner, |tx| {
        tx.execute(
            "UPDATE owner_asks SET answer = '{\"by\":\"owner\",\"text\":\"done\"}' WHERE id = ?1",
            [ask],
        )?;
        Ok(())
    })
    .unwrap();
    let next = engine::next_round(&mut e, work).unwrap();
    assert_eq!((next.waits, next.ask), (None, None));
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
                 VALUES (?1, ?2, 'test.full_ci', '{}', ?3, 'next', 'c', 1, 2)",
                params![work, next_round(tx, work)?, output.to_string()],
            )?;
            tx.execute("UPDATE works SET at = 'test.fix' WHERE id = ?1", [work])?;
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
    // The rounds of works on items: the wake `quiet` recorded before the test is not counted.
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
    assert_eq!(first.cell.as_deref(), Some("start"));
    assert_eq!(rounds(&e), 1, "{log}");
    assert_ne!(at, "start", "{log}");
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
        "research.problem",
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
    assert!(at != "start" && at != "research.problem", "{log}");
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
        ("research.problem", "The item's example does not run as written.", None)
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
    assert_eq!(at, "research.stands", "{log}");
    let session: i64 =
        e.db.conn()
            .query_row(
                "SELECT id FROM sessions WHERE work = ?1 AND line = 'engineer'",
                [work],
                |r| r.get(0),
            )
            .unwrap();
    // The engine stops while cell `research.stands` runs.
    let cut = e
        .db
        .write(Actor::Engine, |tx| {
            let n = next_round(tx, work)?;
            tx.execute(
                "INSERT INTO rounds (work, n, cell, session, input, config_commit, started_at) VALUES (?1, ?2, 'research.stands', ?3, '{}', 'c', 1)",
                params![work, n, session],
            )?;
            Ok(n)
        })
        .unwrap();
    e.step(None).unwrap();
    let rounds = agent_rounds(&e, item);
    let (id, cell, input) = rounds.last().unwrap();
    assert_eq!(cell, "research.stands");
    let m = input["message"].as_str().unwrap();
    assert!(
        m.starts_with(&format!(
            "# {id} · cell research.stands\n\nR{work}-{cut} was cut off midway. First look at the worktree"
        )),
        "{m}"
    );
    assert_eq!(input["conditional"], json!([]));
    assert!(input["call"]["session"]["resumed"].is_string(), "{input}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// When the flow moves to a new version, a work on the old one is not taken over when the engine starts: the owner still
// answers its asks. Once it goes on, it stops, saying why, and its item gets a new work at once: it takes over the
// branch, the worktree and the pull request, its cell `research.problem` is told what exists, and what the owner
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
            .query_row(
                "SELECT input FROM rounds WHERE work = ?1 AND cell = 'research.problem'",
                [new],
                |r| r.get(0),
            )
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
    e.pause(work, "cell research.problem: the return does not fit").unwrap();
    let (ended, why) = rejected(&e, left);
    assert!(ended.is_some());
    assert_eq!(
        why,
        json!({ "error": "cell research.problem: the return does not fit" })
    );
    let told: String =
        e.db.conn()
            .query_row("SELECT text FROM engine_notices WHERE work = ?1", [work], |r| r.get(0))
            .unwrap();
    assert!(
        told.ends_with("is paused: cell research.problem: the return does not fit"),
        "{told}"
    );
    // An earlier engine paused without closing the round; the owner lets the work go on.
    let earlier = open_round(&mut e);
    api::resume(&mut e.db, work).unwrap();
    e.step(None).unwrap();
    let log = said.borrow().join("\n");
    let (ended, why) = rejected(&e, earlier);
    assert!(ended.is_some(), "{log}");
    assert_eq!(
        why,
        json!({ "error": "the work was paused: cell research.problem: the return does not fit" }),
        "{log}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// A query an agent asks for runs in the tool cell, and the tool cell's return, the rows included, fits the schema the
// compiled flow gives cell `tools`: the engine checks its own tool cell's return against it and pauses the work when it
// does not, so a result the flow does not describe fails here.
#[test]
fn a_query_runs_in_the_tool_cell_and_its_return_fits_the_flow() {
    let s = setup("query");
    let mut agents = scripts();
    let implement = agents.remove("implement").unwrap();
    agents.insert(
        "implement",
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
                 WHERE w.item = ?1 AND r.cell = 'tools' AND r.transition IS NOT NULL ORDER BY r.work, r.n LIMIT 1",
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
    let schema = g.cell("tools").and_then(|c| c.schema.clone()).unwrap();
    assert!(flow::valid(&schema, &output), "{output}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The next round of a work at a gate whose need the project does not answer is the engine's own answer, as the round
// then is: the preview says why no agent is asked, and builds no message.
#[test]
fn the_next_round_at_a_gate_the_project_cannot_meet_is_the_engines_answer() {
    let s = setup("gate-preview");
    let (mut e, said, item) = start(&s, scripts());
    for _ in 0..40 {
        if state(&e, item).is_some_and(|(_, at, _)| at == "design.gap_under_test") {
            break;
        }
        e.step(None).unwrap();
    }
    let log = said.borrow().join("\n");
    let (work, at, _) = state(&e, item).unwrap();
    assert_eq!(at, "design.gap_under_test", "{log}");
    let next = engine::next_round(&mut e, work).unwrap();
    assert_eq!((next.kind.as_str(), &next.message, &next.line), ("agent", &None, &None));
    let why = next.not_asked.unwrap_or_default();
    assert!(
        why.starts_with("Not asked: this question needs what this project does not have."),
        "{why}"
    );
    e.step(None).unwrap();
    let (output, input): (String, String) =
        e.db.conn()
            .query_row(
                "SELECT output, input FROM rounds WHERE work = ?1 AND cell = 'design.gap_under_test'",
                [work],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
    let output: Value = serde_json::from_str(&output).unwrap();
    assert_eq!(output["outcome"]["answer"]["why"], json!(why), "{output}");
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
                    "INSERT INTO works (item, flow_version, graph, at) VALUES (?1, ?2, 'engineer', 'research.problem')",
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
// `research.problem` is told where it came from.
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
    let max = pause_after();
    scripts.insert(
        "research.problem",
        Box::new(move |_: &Request, n: usize| {
            let bad = vec![("outcome", json!("the JSON text of an object"))];
            (Vec::new(), if n <= max { bad } else { Vec::new() })
        }),
    );
    let (mut e, said, item) = start(&s, scripts);
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
    assert!(
        why.starts_with("3 returns of cell research.problem in a row were sent back"),
        "{why}"
    );
    assert!(why.contains("outcome must be an object, and is a string"), "{why}");
    api::resume(&mut e.db, id).unwrap();
    for _ in 0..3 {
        e.tick().unwrap();
    }
    let log = said.borrow().join("\n");
    let (_, at, paused) = state(&e, item).unwrap();
    assert_eq!(paused, None, "{log}");
    assert_ne!(at, "research.problem", "{log}");
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
        if agent_rounds(&e, item).iter().any(|r| r.1 == "test.blind_review") {
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
    let (first, second) = (&at("research.problem"), &at("research.stands"));
    let (work, _, _) = state(&e, item).unwrap();
    let workspace = format!("work:{work}");
    for a in [
        "format:common",
        "format:research.problem",
        "inputs:research.problem",
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
    assert_eq!(given(second, "format:research.stands"), Some(true));
    // The project guide is an input every agent's cell has: given in a session's first round, before anything else,
    // referred to after, and given again to the review's session.
    let guide = |input: &Value| -> Option<(String, bool)> {
        input["conditional"].as_array()?.iter().find_map(|m| {
            let a = m["address"].as_str()?;
            a.ends_with("·engine·project_guide")
                .then(|| (a.to_string(), m["given"].as_bool().unwrap_or(false)))
        })
    };
    let (address, sent) = guide(first).expect("the project guide in cell research.problem's round");
    assert!(sent, "{address}");
    let m = first["message"].as_str().unwrap();
    let at_guide = m.find(&format!("### {address}")).unwrap();
    assert!(at_guide < m.find("### format:common").unwrap(), "{m}");
    assert_eq!(guide(second).map(|g| g.1), Some(false));
    assert_eq!(guide(&at("test.blind_review")).map(|g| g.1), Some(true));
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
    let design = &rounds.iter().rev().find(|r| r.1 == "design.choose").unwrap().0;
    let acceptance = format!("{design}·design.choose·acceptance");
    assert_eq!(
        given(&at("design.adversarial_needed"), &acceptance),
        Some(false),
        "{}",
        at("design.adversarial_needed")
    );
    // Implementing starts a new session, which holds nothing: the acceptance is given whole again.
    assert_eq!(given(&at("implement"), &acceptance), Some(true), "{}", at("implement"));
    assert_eq!(
        given(&at("test.blind_review"), &acceptance),
        Some(true),
        "{}",
        at("test.blind_review")
    );
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
        "research.stands",
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
    assert_eq!(
        &cells[..3],
        ["research.problem", "research.stands", "design.compare"],
        "{log}"
    );
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
    let understanding = format!("{}·research.problem·understanding", rounds[0].0);
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
    assert_eq!(
        &cells[..3],
        ["research.problem", "research.stands", "design.compare"],
        "{log}"
    );
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
    assert_eq!(
        &cells[..3],
        ["research.problem", "research.stands", "design.compare"],
        "{log}"
    );
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
    assert_eq!(at, "research.stands", "{log}");
    let paused = paused.unwrap_or_else(|| panic!("the work did not pause:\n{log}"));
    assert!(
        paused.starts_with(&format!(
            "cell research.stands: the message of R{work}-3 was not sent, since it would repeat what its session holds: \
             rule:ground (12 characters) would be given whole again, though the session got the same content in \
             R{work}-2 as rule:elsewhere"
        )),
        "{paused}"
    );
    assert_eq!(paused_by(&e, work).as_deref(), Some("engine"));
    assert_eq!(rounds_count(&e), before, "nothing was sent:\n{log}");
    let notices: i64 = e
        .db
        .conn()
        .query_row(
            "SELECT count(*) FROM engine_notices WHERE work = ?1 AND text LIKE '%is paused: cell research.stands: the message%'",
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
        "research.problem",
        Box::new(|_: &Request, _: usize| (Vec::new(), vec![("outcome", json!("not an object"))])),
    );
    let (mut e, said, item) = start(&s, scripts);
    e.step(None).unwrap();
    let (work, _, _) = state(&e, item).unwrap();
    let relations = db::relations(db::SCHEMA).unwrap();
    let mut f = flow::compile(
        flow::FLOW,
        &flow::Context {
            relations,
            tables: db::index_of(db::SCHEMA).unwrap(),
        },
    )
    .unwrap();
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
    assert_eq!(at, "research.problem", "{log}");
    assert!(
        paused
            .as_deref()
            .is_some_and(|p| p.starts_with("2 returns of cell research.problem in a row were sent back")),
        "{paused:?}\n{log}"
    );
    // After a return sent back the message only says why, and that the return is to be rewritten.
    let rounds = agent_rounds(&e, item);
    let (id, _, again) = &rounds[1];
    let m = again["message"].as_str().unwrap();
    assert!(
        m.starts_with(&format!(
            "# {id} · cell research.problem\n\nYour return was sent back: "
        )) && m.ends_with("\nRewrite it (format:research.problem).\n"),
        "{m}"
    );
    assert_eq!(again["conditional"], json!([]));
    let _ = std::fs::remove_dir_all(&s.root);
}

/// The engine over a setup whose cell `research.problem` ends its first `times` runs without a return, the CLI
/// reporting `report`.
fn failing(name: &str, report: String, times: usize) -> (Setup, Engine, Rc<RefCell<Vec<String>>>, i64) {
    let s = setup(name);
    let mut scripts = scripts();
    scripts.insert(
        "research.problem",
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
    assert_ne!(at, "research.problem", "{log}");
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
    assert_ne!(at, "research.problem");
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
        if state(&e, item).unwrap().1 != "research.problem" {
            break;
        }
    }
    let log = said.borrow().join("\n");
    let (_, at, paused) = state(&e, item).unwrap();
    assert_ne!(at, "research.problem", "{log}");
    assert_eq!(paused, None, "{log}");
    assert_eq!(failed_kinds(&e), ["transient", "transient", "transient"]);
    assert_eq!(sent_back(&e), 0);
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
    assert_ne!(at, "research.problem", "{log}");
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
    assert_eq!(at, "research.problem", "{log}");
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
        if state(&e, item).unwrap().1 != "research.problem" {
            break;
        }
    }
    let log = said.borrow().join("\n");
    let (_, at, paused) = state(&e, item).unwrap();
    assert_eq!(paused, None, "{log}");
    assert_ne!(at, "research.problem", "{log}");
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
    // The owner pauses the work on the page while the round of cell `research.problem` runs.
    scripts.insert(
        "research.problem",
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
    assert_eq!(at, "research.stands", "{log}");
    assert_eq!(paused.as_deref(), Some(db::PAUSED_BY_OWNER), "{log}");
    assert_eq!(paused_by(&e, work).as_deref(), Some("owner"));
    let took: Option<String> =
        e.db.conn()
            .query_row(
                "SELECT transition FROM rounds WHERE work = ?1 AND cell = 'research.problem'",
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
    e.pause(work, "cell research.stands: the return does not fit").unwrap();
    assert_eq!(paused_by(&e, work).as_deref(), Some("engine"));
    let log = said.borrow().join("\n");
    assert_eq!(turns(&mut e, &c, 2), Vec::<i64>::new(), "{log}");
    assert_eq!(
        state(&e, item).unwrap().2.as_deref(),
        Some("cell research.stands: the return does not fit")
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
    e.pause(work, "cell research.stands: the return does not fit").unwrap();
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
    e.pause(work, "cell research.problem: the return does not fit").unwrap();
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
    assert_eq!(cells, ["start"]);
    assert_eq!(paused_by(&e, new).as_deref(), Some("owner"));
    assert_eq!(e.step(Some(new)).unwrap().work, None);
    let _ = std::fs::remove_dir_all(&s.root);
}

// The architects' wakes.

/// The project an architect's round is for, by the directory it works in, `worktrees/architect/<project>`.
fn project_of(r: &Request) -> String {
    r.worktree
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The architect's cells a wake would not get through with their first answers, scripted: the pick takes the first
/// matter of its project's queue.
fn picks_first(s: &Setup, scripts: &mut BTreeMap<&'static str, Script>) {
    let path = config::database(&s.data);
    scripts.insert(
        "pick.choose",
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

/// The wakes of every architect, oldest first.
fn wakes(e: &Engine) -> Vec<i64> {
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

/// Whether a wake has reached its end.
fn wake_done(e: &mut Engine, wake: i64) -> bool {
    let w = e.work(wake).unwrap();
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

/// The owner says something to project p's architect.
fn say(e: &mut Engine, text: &str) {
    db::say_to_architect(&mut e.db, "p", None, None, text).unwrap();
}

// The owner's words wake project p's architect, whose wake goes once through the whole graph: it wakes, finds its new
// session has not understood the project, understands it, judges the policy and what runs, takes the lone matter on
// its path, whose item change is collected; the batch is done, its review finds the change sound, and the engine makes
// it as the architect; the rethink goes on, nothing is left, and the wake wraps up and ends, referred to the state of
// things its session wrote. A field the path cell may leave out and wrote as null is taken as left out.
#[test]
fn a_wake_goes_through_understanding_a_matter_its_review_and_its_wrap_up() {
    let s = setup("wake");
    let mut agents = scripts();
    let change = json!({ "kind": "edit_item", "item": "1", "part": "title", "text": "Parse the input first",
                         "reason": "Clearer." });
    let given = change.clone();
    agents.insert(
        "handle.owner_words",
        Box::new(move |_: &Request, _: usize| {
            (
                Vec::new(),
                vec![(
                    "outcome.effects",
                    json!({ "item_changes": [given.clone()], "notes": null }),
                )],
            )
        }),
    );
    agents.insert(
        "batch.review",
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
    say(&mut e, "Make the parser's item clearer.");
    let woke = e.step(None).unwrap();
    let log = said.borrow().join("\n");
    assert_eq!(woke.cell.as_deref(), Some("wake.start"), "{log}");
    let wake = woke.work.unwrap();
    let project: Option<String> =
        e.db.conn()
            .query_row("SELECT project FROM works WHERE id = ?1", [wake], |r| r.get(0))
            .unwrap();
    assert_eq!(project.as_deref(), Some("p"));
    assert!(
        steps_until(&mut e, 40, |e| wake_done(e, wake)),
        "{}",
        said.borrow().join("\n")
    );
    let log = said.borrow().join("\n");
    assert_eq!(
        path_of(&e, wake),
        [
            "wake.start",
            "wake.understood",
            "understand.state",
            "understand.policy",
            "understand.fit",
            "pick.anything",
            "pick.route",
            "handle.owner_words",
            "handle.batch_done",
            "batch.review",
            "batch.rethink",
            "pick.anything",
            "wrap_up",
            "record"
        ],
        "{log}"
    );
    let rows: Vec<Option<String>> = cells_of(&e, wake).into_iter().map(|c| c.1).collect();
    assert_eq!(rows[1].as_deref(), Some("not_understood"));
    assert_eq!(rows[8].as_deref(), Some("review"));
    assert_eq!(rows[9].as_deref(), Some("ok"));
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
                [wake],
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
        &[&wake],
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
    assert!(schemas.iter().any(|x| x.0 == "batch.rethink"), "{schemas:?}");
    assert!(schemas.iter().all(|x| x.1 == schemas[0].1), "{schemas:?}");
    // The wrap-up is referred to the state of things its session wrote at the understanding, not given it again.
    let wrap_up = &inputs_at(&e, wake, "wrap_up")[0];
    assert_eq!(
        given_ending(wrap_up, "·understand.state·state"),
        Some(false),
        "{wrap_up}"
    );
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
                "INSERT INTO works (item, flow_version, graph, at) VALUES (?1, ?2, 'engineer', 'research.problem')",
                params![item, v],
            )?;
            Ok(tx.last_insert_rowid())
        })
        .unwrap();
    let wake = wake_at(&mut e, "understand.state");
    let mut lines = 0;
    for id in [work, wake] {
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

// A wake goes on in the session the project's last wake left: one that understood the project in it goes from waking
// straight to the queue, and does not understand it again.
#[test]
fn a_resumed_session_goes_from_waking_to_the_queue() {
    let s = setup("resumed");
    let (mut e, said) = engine(&s, scripts());
    quiet(&mut e);
    an_item(&mut e);
    say(&mut e, "Look at the queue.");
    let first = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 30, |e| wake_done(e, first)),
        "{}",
        said.borrow().join("\n")
    );
    say(&mut e, "Look again.");
    let second = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 30, |e| wake_done(e, second)),
        "{}",
        said.borrow().join("\n")
    );
    let log = said.borrow().join("\n");
    assert_eq!(
        path_of(&e, second),
        [
            "wake.start",
            "wake.understood",
            "pick.anything",
            "pick.route",
            "handle.owner_words",
            "handle.batch_done",
            "batch.rethink",
            "pick.anything",
            "wrap_up",
            "record"
        ],
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
    assert_eq!(sessions[1].0, "wake");
    assert_eq!(sessions[0].1, sessions[1].1);
    let _ = std::fs::remove_dir_all(&s.root);
}

// An engineer's question to its project's architect is a matter of its queue, which wakes it; the work waits at its
// question, holding no slot. The architect finds the crux, weighs the ways and replies, and the reply takes the work on
// from there unchanged.
#[test]
fn an_engineer_s_question_is_answered_through_its_crux_and_ways() {
    let s = setup("question");
    let mut agents = scripts();
    agents.insert(
        "research.problem",
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
        "handle.ways",
        Box::new(move |_: &Request, _: usize| (Vec::new(), vec![("outcome.reply", given.clone())])),
    );
    let (mut e, said, item) = start(&s, agents);
    let question = |e: &Engine| -> Option<(Option<String>, Option<i64>)> {
        e.db.conn()
            .query_row(
                "SELECT r.output, r.ended_at FROM rounds r JOIN works w ON w.id = r.work
                 WHERE w.item = ?1 AND r.cell = 'ask.architect' ORDER BY r.n DESC LIMIT 1",
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
    let woke = e.step(None).unwrap();
    let wake = woke.work.unwrap();
    let replied = steps_until(&mut e, 60, |e| {
        question(e).is_some_and(|q| q.1.is_some()) && wake_done(e, wake)
    });
    let log = said.borrow().join("\n");
    assert!(replied, "{log}");
    let path = path_of(&e, wake);
    let at = path.iter().position(|c| c == "handle.crux").expect(&log);
    assert_eq!(
        &path[at - 1..at + 3],
        ["pick.route", "handle.crux", "handle.ways", "handle.batch_done"],
        "{log}"
    );
    // The ways were given the crux by its address in the session, not again.
    let ways = message_at(&e, wake, "handle.ways", false);
    assert!(ways.contains("·handle.crux·crux"), "{ways}");
    assert!(ways.contains("What is asked is whether to split the work"), "{ways}");
    let (output, _) = question(&e).unwrap();
    let output: Value = serde_json::from_str(&output.unwrap()).unwrap();
    assert_eq!(output, json!({ "reply": reply }));
    let (work, _, _) = state(&e, item).unwrap();
    let (took, next): (String, String) =
        e.db.conn()
            .query_row(
                "SELECT q.transition, r.cell FROM rounds q JOIN rounds r ON r.work = q.work AND r.n = q.n + 1
                 WHERE q.work = ?1 AND q.cell = 'ask.architect'",
                [work],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
    assert_eq!((took.as_str(), next.as_str()), ("answered", "ask.way_on"), "{log}");
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
        "handle.owner_words",
        Box::new(|_: &Request, _: usize| {
            let change = json!({ "kind": "edit_item", "item": "1", "part": "title", "text": "parser", "reason": "r" });
            (
                Vec::new(),
                vec![("outcome.effects", json!({ "item_changes": [change] }))],
            )
        }),
    );
    agents.insert(
        "batch.review",
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
        "batch.revise",
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
    let wake = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 40, |e| wake_done(e, wake)),
        "{}",
        said.borrow().join("\n")
    );
    let log = said.borrow().join("\n");
    let path = path_of(&e, wake);
    let at = path.iter().position(|c| c == "batch.review").expect(&log);
    assert_eq!(
        &path[at..at + 4],
        ["batch.review", "batch.revise", "batch.review", "batch.rethink"],
        "{log}"
    );
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

// An ask of the owner from a cell on a matter's path holds only that matter: the wake goes on with the next and ends;
// the owner's answer puts the matter back in the queue, which wakes the architect, and it goes back to the cell that
// asked, which is given the answer.
#[test]
fn an_ask_holds_only_its_matter_and_the_answer_goes_back_to_the_cell_that_asked() {
    let s = setup("ask-holds");
    let mut agents = scripts();
    picks_first(&s, &mut agents);
    agents.insert(
        "handle.owner_words",
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
    say(&mut e, "Hold the release?");
    say(&mut e, "Look at the queue.");
    let wake = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 40, |e| wake_done(e, wake)),
        "{}",
        said.borrow().join("\n")
    );
    let log = said.borrow().join("\n");
    let path = path_of(&e, wake);
    let asked = path.iter().position(|c| c == "ask.owner").expect(&log);
    assert_eq!(
        &path[asked - 1..asked + 6],
        [
            "handle.owner_words",
            "ask.owner",
            "handle.batch_done",
            "pick.anything",
            "pick.route",
            "handle.owner_words",
            "handle.batch_done"
        ],
        "{log}"
    );
    assert_eq!(cells_of(&e, wake)[asked].1.as_deref(), Some("held"));
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
    assert_eq!((held.2.as_deref(), held.3), (Some("handle.owner_words"), None), "{log}");
    assert_eq!(
        matters(&e),
        [
            ("owner_words".to_string(), false, false),
            ("owner_words".to_string(), false, true)
        ],
        "{log}"
    );
    // The owner answers: the matter is back, a wake starts, and the matter goes back to the cell that asked.
    let yes = api::Answer {
        text: "Yes, hold it.".into(),
        why: None,
        reply: None,
        choice: None,
    };
    api::answer(&mut e.db, ask, &yes).unwrap();
    let again = e.step(None).unwrap().work.unwrap();
    assert_ne!(again, wake);
    assert!(
        steps_until(&mut e, 40, |e| wake_done(e, again)),
        "{}",
        said.borrow().join("\n")
    );
    let log = said.borrow().join("\n");
    let path = path_of(&e, again);
    let at = path.iter().position(|c| c == "pick.route").expect(&log);
    assert_eq!(
        &path[at..at + 3],
        ["pick.route", "handle.owner_words", "handle.batch_done"],
        "{log}"
    );
    assert_eq!(cells_of(&e, again)[at].1.as_deref(), Some("to_owner_words"));
    let answered = message_at(&e, again, "handle.owner_words", false);
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
        "handle.take_in",
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
        "search",
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
    let wake = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 40, |e| wake_done(e, wake)),
        "{}",
        said.borrow().join("\n")
    );
    let log = said.borrow().join("\n");
    let path = path_of(&e, wake);
    let at = path.iter().position(|c| c == "search").expect(&log);
    assert_eq!(
        &path[at - 2..at + 3],
        [
            "handle.what_is_it",
            "handle.take_in",
            "search",
            "handle.take_in",
            "handle.batch_done"
        ],
        "{log}"
    );
    let (line, model): (String, String) =
        e.db.conn()
            .query_row(
                "SELECT s.line, r.model FROM rounds r JOIN sessions s ON s.id = r.session
                 WHERE r.work = ?1 AND r.cell = 'search'",
                [wake],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
    assert_eq!((line.as_str(), model.as_str()), ("observer", "m-cheap"));
    let back = message_at(&e, wake, "handle.take_in", true);
    assert!(back.contains("It records the same slowness."), "{back}");
    // The observer writes what the owner reads in the owner's language, as every line does.
    let search = message_at(&e, wake, "search", false);
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

// When the architect judges the policy may no longer govern, it is set again before anything else: new works of the
// project are held while a study is made for the owner; the owner says the policy stays, the hold is let go, and the
// wake goes on to judge what runs.
#[test]
fn a_policy_set_again_is_studied_and_the_owner_says_it_stays() {
    let s = setup("policy-stays");
    let mut agents = scripts();
    agents.insert(
        "understand.policy",
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
    let wake = e.step(None).unwrap().work.unwrap();
    let asked = |e: &mut Engine| -> Option<i64> {
        e.db.conn()
            .query_row(
                "SELECT id FROM owner_asks WHERE work = ?1 AND json_extract(body, '$.kind') = 'study'",
                [wake],
                |r| r.get(0),
            )
            .ok()
    };
    assert!(
        steps_until(&mut e, 20, |e| asked(e).is_some()),
        "{}",
        said.borrow().join("\n")
    );
    let log = said.borrow().join("\n");
    assert_eq!(
        path_of(&e, wake),
        [
            "wake.start",
            "wake.understood",
            "understand.state",
            "understand.policy",
            "policy.study",
            "policy.owner_reads"
        ],
        "{log}"
    );
    let holds = |e: &Engine| -> Vec<(String, String)> {
        let mut st =
            e.db.conn()
                .prepare("SELECT project, reason FROM dispatch_holds ORDER BY reason")
                .unwrap();
        st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(|x| x.unwrap())
            .collect()
    };
    assert_eq!(holds(&e), [("p".to_string(), "policy".to_string())]);
    let stays = api::Answer {
        text: "Keep it.".into(),
        why: None,
        reply: None,
        choice: Some("policy_stays".into()),
    };
    let ask = asked(&mut e).unwrap();
    api::answer(&mut e.db, ask, &stays).unwrap();
    assert!(
        steps_until(&mut e, 30, |e| wake_done(e, wake)),
        "{}",
        said.borrow().join("\n")
    );
    let log = said.borrow().join("\n");
    let rows = cells_of(&e, wake);
    let at = rows
        .iter()
        .position(|c| c.0 == "policy.owner_reads" && c.1.is_some())
        .expect(&log);
    assert_eq!(rows[at].1.as_deref(), Some("policy_stays"), "{log}");
    assert_eq!(rows[at + 1].0, "understand.fit", "{log}");
    assert_eq!(holds(&e), Vec::<(String, String)>::new(), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// Each project has its architect: a message from project p's architect is a matter of project q's architect's queue,
// which wakes q's architect in a wake of its own; q's answer comes back to p's queue as a message.
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
        "handle.owner_words",
        Box::new(|_: &Request, _: usize| {
            let message = json!({ "project": "q", "text": "Does q still need the old format?" });
            (Vec::new(), vec![("outcome.effects", json!({ "messages": [message] }))])
        }),
    );
    agents.insert(
        "handle.answer_architect",
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
        ws.len() >= 3 && ws.iter().all(|(w, _)| wake_done(e, *w))
    };
    assert!(steps_until(&mut e, 120, done_all), "{}", said.borrow().join("\n"));
    let log = said.borrow().join("\n");
    let ws = projects(&e);
    let names: Vec<&str> = ws.iter().map(|w| w.1.as_str()).collect();
    assert_eq!(names, ["p", "q", "p"], "{log}");
    assert!(
        path_of(&e, ws[1].0).contains(&"handle.answer_architect".to_string()),
        "{log}"
    );
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

// Each side report is a matter of the kind report in its project's architect's queue, queued once, which wakes the
// architect; the queue marks one urgent.
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
        "research.problem",
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
    let woke = e.step(work).unwrap();
    let log = said.borrow().join("\n");
    assert_eq!(woke.cell.as_deref(), Some("wake.start"), "{log}");
    let wake = woke.work.unwrap();
    assert!(
        steps_until(&mut e, 40, |e| wake_done(e, wake)),
        "{}",
        said.borrow().join("\n")
    );
    let log = said.borrow().join("\n");
    assert!(path_of(&e, wake).contains(&"handle.what_is_it".to_string()), "{log}");
    assert_eq!(matters(&e), [("report".to_string(), false, true)], "{log}");
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
        "implement",
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
        "handle.card_sound",
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
        "handle.owner_words",
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
    let woke = e.step(Some(work)).unwrap();
    let log = said.borrow().join("\n");
    assert_eq!(woke.cell.as_deref(), Some("wake.start"), "{log}");
    let wake = woke.work.unwrap();
    let stopped = |e: &mut Engine| state(e, item).is_some_and(|x| x.2.is_some());
    assert!(steps_until(&mut e, 40, stopped), "{}", said.borrow().join("\n"));
    let why = format!("{}: It waits for the parser.", db::STOPPED_BY_ARCHITECT);
    assert_eq!(state(&e, item).unwrap().2, Some(why.clone()));
    assert_eq!(paused_by(&e, work).as_deref(), Some("engine"));
    assert!(
        steps_until(&mut e, 40, |e| wake_done(e, wake)),
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
    let ran = |e: &Engine, wake: i64, cell: &str| -> Option<(String, Value)> {
        let (model, input): (String, String) =
            e.db.conn()
                .query_row(
                    "SELECT model, input FROM rounds WHERE work = ?1 AND cell = ?2 ORDER BY n LIMIT 1",
                    params![wake, cell],
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
    let wake = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 10, |e| ran(e, wake, "understand.state").is_some()),
        "{}",
        said.borrow().join("\n")
    );
    let (model, call) = ran(&e, wake, "understand.state").unwrap();
    let want = (
        "m-two".to_string(),
        json!("2"),
        Some("m-two".to_string()),
        Some("high".to_string()),
    );
    assert_eq!(at_level(&model, &call), want);
    assert!(
        steps_until(&mut e, 40, |e| wake_done(e, wake)),
        "{}",
        said.borrow().join("\n")
    );
    let reading =
        e.db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO works (item, flow_version, graph, at, project)
                 SELECT NULL, flow_version, graph, 'policy.fresh_read', project FROM works WHERE id = ?1",
                [wake],
            )?;
            Ok(tx.last_insert_rowid())
        })
        .unwrap();
    e.step_work(reading).unwrap();
    let log = said.borrow().join("\n");
    let (model, call) = ran(&e, reading, "policy.fresh_read").expect(&log);
    assert_eq!(at_level(&model, &call), want, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// While slots are free and nothing of a project is ready to start, the engine queues a matter of slots free for its
// architect, which wakes it; once one is handled, no other comes for an hour.
#[test]
fn slots_free_with_nothing_ready_wake_the_architect_at_most_once_an_hour() {
    let s = setup("slots-free");
    let (mut e, said) = engine(&s, scripts());
    let woke = e.step(None).unwrap();
    let log = said.borrow().join("\n");
    assert_eq!(woke.cell.as_deref(), Some("wake.start"), "{log}");
    let wake = woke.work.unwrap();
    let body: String =
        e.db.conn()
            .query_row(
                "SELECT body FROM architect_matters WHERE kind = 'slots_free'",
                [],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(body, r#"{"free":1}"#);
    assert!(
        steps_until(&mut e, 40, |e| wake_done(e, wake)),
        "{}",
        said.borrow().join("\n")
    );
    assert!(path_of(&e, wake).contains(&"handle.what_runs".to_string()));
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
    assert_eq!(again.cell.as_deref(), Some("wake.start"), "{log}");
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

// With every work paused, the owner steps a project's architect: the first step starts a wake, paused by the owner,
// and runs its first round; each later step runs the wake's next round, as a step of the wake by its work id does, and
// the wake stays paused in the owner's hands. With nothing to wake for, a step runs nothing.
#[test]
fn the_owner_steps_the_architect_one_round_at_a_time_with_every_work_paused() {
    let s = setup("step-architect");
    let (mut e, said) = engine(&s, scripts());
    quiet(&mut e);
    assert_eq!(e.step_architect("p").unwrap().work, None);
    say(&mut e, "Look at the queue.");
    let first = e.step_architect("p").unwrap();
    let log = said.borrow().join("\n");
    assert_eq!(
        (first.cell.as_deref(), first.rounds.len()),
        (Some("wake.start"), 1),
        "{log}"
    );
    let wake = first.work.unwrap();
    assert_eq!(paused_by(&e, wake).as_deref(), Some("owner"));
    assert_eq!(e.step(None).unwrap().work, None);
    let second = e.step_work(wake).unwrap();
    let log = said.borrow().join("\n");
    assert_eq!(
        (second.cell.as_deref(), second.rounds.len()),
        (Some("wake.understood"), 1),
        "{log}"
    );
    let third = e.step_architect("p").unwrap();
    let log = said.borrow().join("\n");
    assert_eq!(
        (third.work, third.cell.as_deref(), third.rounds.len()),
        (Some(wake), Some("understand.state"), 1),
        "{log}"
    );
    assert_eq!(paused_by(&e, wake).as_deref(), Some("owner"));
    assert_eq!(wakes(&e), [wake]);
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
                "INSERT INTO works (item, flow_version, graph, at) VALUES (?1, ?2, 'engineer', 'end')",
                params![item, version],
            )?;
            let w = tx.last_insert_rowid();
            let implemented = match followup {
                Some(f) => json!({ "outcome": { "followup": f } }),
                None => json!({ "outcome": {} }),
            };
            let merged_round = json!({ "result": { "verdict": "merged", "merge_commit": merged } });
            for (n, cell, output, row) in [
                (1, "implement", implemented, "done"),
                (2, "merge.queue", merged_round, "merged"),
            ] {
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
    step_to(&mut e, item, "test.next_step");
    let (work, _, _) = state(&e, item).unwrap();
    let lines = line_sessions(&e, work, "engineer");
    assert_eq!(lines.len(), 2, "{lines:?}\n{}", said.borrow().join("\n"));
    assert_eq!((lines[0].0.as_str(), lines[0].1), ("line", true), "{lines:?}");
    assert!(
        lines[0].2.starts_with("research.problem,") && !lines[0].2.contains("implement"),
        "{lines:?}"
    );
    assert_eq!(lines[1], ("cell".to_string(), false, "implement,implement".to_string()));
    let rounds = agent_rounds(&e, item);
    let implements: Vec<&Value> = rounds.iter().filter(|r| r.1 == "implement").map(|r| &r.2).collect();
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
        "·research.problem·understanding",
        "·design.choose·chosen",
        "·design.choose·estimate",
        "·design.cross_project·progress",
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
    move_to(&mut e, work, "test.fix");
    e.step(None).unwrap();
    let log = said.borrow().join("\n");
    let lines = line_sessions(&e, work, "engineer");
    assert_eq!(lines.len(), 3, "{lines:?}\n{log}");
    assert!(lines[1].1, "{lines:?}");
    assert_eq!(
        (lines[2].0.as_str(), lines[2].1, lines[2].2.as_str()),
        ("cell", false, "test.fix"),
        "{log}"
    );
    move_to(&mut e, work, "test.fix");
    e.step(None).unwrap();
    let lines = line_sessions(&e, work, "engineer");
    assert_eq!(lines.len(), 3, "{lines:?}\n{}", said.borrow().join("\n"));
    assert_eq!(lines[2].2, "test.fix,test.fix");
    let rounds = agent_rounds(&e, item);
    let fixes: Vec<&Value> = rounds.iter().filter(|r| r.1 == "test.fix").map(|r| &r.2).collect();
    for end in [
        item_address.as_str(),
        "·design.choose·acceptance",
        "·design.choose·design",
        "·implement·conditions",
    ] {
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
        "research.stands",
        Box::new(|_: &Request, _: usize| (Vec::new(), vec![("@usage", used(1.0, 499_997))])),
    );
    let (mut e, said, item) = start(&s, scripts);
    agent_rounds_to(&mut e, item, 2);
    let (work, at, _) = state(&e, item).unwrap();
    assert_eq!(at, "design.compare", "{}", said.borrow().join("\n"));
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
            ("line".to_string(), true, "research.problem,research.stands".to_string()),
            ("limit".to_string(), false, "design.compare".to_string())
        ],
        "{log}"
    );
    let rounds = agent_rounds(&e, item);
    let two = &rounds[2].2;
    for a in ["format:common", "format:design.compare", "rule:ground"] {
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
        "research.problem",
        Box::new(|_: &Request, _: usize| (Vec::new(), vec![("@usage", used(1.0, 9_000))])),
    );
    scripts.insert(
        "research.stands",
        Box::new(|_: &Request, _: usize| (Vec::new(), vec![("@usage", used(1.5, 9_500))])),
    );
    scripts.insert(
        "design.compare",
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
    assert_eq!(at, "design.compare", "{}", said.borrow().join("\n"));
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
        assert_eq!(
            *waited,
            json!({ "cell": "design.compare", "on": "pause", "what": "to look at it" })
        );
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
                "SELECT json_extract(usage, '$.cost_usd') FROM rounds WHERE work = ?1 AND cell = 'design.compare'",
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
        "design.adversarial_needed",
        Box::new(|_: &Request, _: usize| (vec![("outcome.adversarial", "no")], vec![("@usage", used(1.0, 9_000))])),
    );
    let (mut e, said, item) = start(&s, scripts);
    step_to(&mut e, item, "implement");
    let (work, _, _) = state(&e, item).unwrap();
    pause_by_owner(&mut e, work, "to look at it");
    let from = last_call(&e, work);
    assert_eq!(e.keep_warm(from + 55 * 60_000 + 1_000, None).unwrap(), 0);
    // At a cell that goes on in the session, a round of the session that runs keeps the ping off.
    move_to(&mut e, work, "design.adversarial_needed");
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
                 VALUES (?1, ?2, 'design.adversarial_needed', ?3, '{}', 'c', ?4)",
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
        "research.stands",
        Box::new(|_: &Request, _: usize| (Vec::new(), vec![("@usage", used(1.0, 9_000))])),
    );
    let (mut e, said, item) = start(&s, scripts);
    agent_rounds_to(&mut e, item, 2);
    let (work, at, _) = state(&e, item).unwrap();
    assert_eq!(at, "design.compare", "{}", said.borrow().join("\n"));
    let ask =
        e.db.write(Actor::Engine, |tx| {
            tx.execute(
                "UPDATE works SET at = 'ask.owner', resume_at = 'design.compare', waiting = '[\"design.compare\"]' WHERE id = ?1",
                [work],
            )?;
            let round = next_round(tx, work)?;
            tx.execute(
                "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES (?1, ?2, 'ask.owner', '{}', 'c', 1)",
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
        json!({ "cell": "ask.owner", "on": "owner", "what": format!("the owner's answer to ask {ask}") })
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
        "research.stands",
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
    assert_eq!(cell, "research.stands", "{}", said.borrow().join("\n"));
    let (file, text) = read.borrow().clone().expect("the file the hook gives again");
    // The session holds the shared formats, so the message only refers to them; the file gives them whole.
    assert_eq!(given(input, "format:common"), Some(false));
    assert!(
        text.starts_with(&format!("Your session was compacted in the middle of {id}"))
            && text.contains(&format!("# {id} · cell research.stands"))
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
        "research.stands",
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
    assert_eq!(cell, "research.stands", "{log}");
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
            ("line".to_string(), true, "research.problem,research.stands".to_string()),
            ("model".to_string(), false, "design.compare".to_string())
        ],
        "{log}"
    );
    let model: String =
        e.db.conn()
            .query_row(
                "SELECT model FROM rounds WHERE work = ?1 AND cell = 'design.compare'",
                [work],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(model, "m-other");
    let _ = std::fs::remove_dir_all(&s.root);
}

// Before the owner approves a new policy, `policy.holds_up` asks of the draft the question `understand.policy` asks of
// the policy in force: one that holds goes on to the owner's approval, and one that would need setting again at once
// goes back to correct the text.
#[test]
fn the_draft_policy_is_asked_whether_it_would_need_setting_again_at_once() {
    for (verdict, then) in [
        ("holds", "policy.owner_approves"),
        ("needs_setting_again", "policy.correct"),
    ] {
        let s = setup(&format!("p11-{verdict}"));
        let mut agents = scripts();
        agents.insert(
            "policy.holds_up",
            Box::new(move |_: &Request, _: usize| (vec![("outcome.report", verdict)], Vec::new())),
        );
        let (mut e, said) = engine(&s, agents);
        quiet(&mut e);
        let v = e.flow_now;
        let wake =
            e.db.write(Actor::Engine, |tx| {
                tx.execute(
                    "INSERT INTO works (item, flow_version, graph, at, project) VALUES (NULL, ?1, 'architect', 'policy.holds_up', 'p')",
                    [v],
                )?;
                Ok(tx.last_insert_rowid())
            })
            .unwrap();
        e.step_work(wake).unwrap();
        let log = said.borrow().join("\n");
        let (at, transition): (String, Option<String>) = e
            .db
            .conn()
            .query_row(
                "SELECT w.at, (SELECT r.transition FROM rounds r WHERE r.work = w.id AND r.cell = 'policy.holds_up')
                     FROM works w WHERE w.id = ?1",
                [wake],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!((at.as_str(), transition.as_deref()), (then, Some(verdict)), "{log}");
        let _ = std::fs::remove_dir_all(&s.root);
    }
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

// At a project's first wake its architect is told what happened in the week before it, as at a later wake since the
// last; its board holds every item not done: one whose last work stopped, queued again, with why; one whose work asks
// the owner, at the stage of the cell it stopped at, its pause said once; and one whose work runs tools, at the stage
// of the cell that waits for them.
#[test]
fn the_first_wake_is_told_the_week_before_it_and_every_item_not_done() {
    let s = setup("first-wake");
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
                "INSERT INTO works (item, flow_version, graph, at, ended) VALUES (?1, ?2, 'engineer', 'implement', 'stopped: by hand')",
                params![stopped, v],
            )?;
            tx.execute(
                "INSERT INTO works (item, flow_version, graph, at, resume_at, paused, paused_by)
                 VALUES (?1, ?2, 'engineer', 'ask.owner', 'implement', 'later', 'owner')",
                params![asking, v],
            )?;
            tx.execute(
                "INSERT INTO works (item, flow_version, graph, at, waiting, paused, paused_by)
                 VALUES (?1, ?2, 'engineer', 'tools', '[\"implement\"]', 'later', 'owner')",
                params![tools, v],
            )?;
            tx.execute(
                "INSERT INTO works (item, flow_version, graph, at) VALUES (?1, ?2, 'engineer', 'end')",
                params![merged, v],
            )?;
            tx.execute(
                "INSERT INTO rounds (work, n, cell, input, output, transition, config_commit, started_at, ended_at)
                 VALUES (?1, 1, 'merge.queue', '{}', '{\"result\":{\"merge_commit\":\"abc\"}}', 'merged', 'c', ?2, ?2)",
                params![tx.last_insert_rowid(), ago],
            )?;
            Ok((stopped, asking, tools))
        })
        .unwrap();
    say(&mut e, "Look at the items.");
    let wake = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 10, |e| path_of(e, wake).iter().any(|c| c == "understand.state")),
        "{}",
        said.borrow().join("\n")
    );
    let m = message_at(&e, wake, "understand.state", false);
    let since = given_value(&m, "·engine·since_last_wake").expect(&m);
    assert_eq!(since["first_wake"], true, "{since}");
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

// The numbers the architect is given are measured up to the start of its wake, so they hold through it: the wake's own
// rounds change nothing, and the next cell that reads them is only referred to them. What each week spent is split
// between the items' works and the architect's wakes.
#[test]
fn the_numbers_hold_through_a_wake_and_split_what_was_spent() {
    let s = setup("numbers");
    let (mut e, said) = engine(&s, scripts());
    quiet(&mut e);
    let item = an_item(&mut e);
    let v = e.flow_now;
    let ago = db::now() - 3_600_000;
    e.db.write(Actor::Engine, |tx| {
        tx.execute("UPDATE rounds SET usage = '{\"cost_usd\":0.25}' WHERE cell = 'handle.what_runs'", [])?;
        tx.execute(
            "INSERT INTO works (item, flow_version, graph, at, ended) VALUES (?1, ?2, 'engineer', 'implement', 'stopped: by hand')",
            params![item, v],
        )?;
        tx.execute(
            "INSERT INTO rounds (work, n, cell, input, transition, config_commit, usage, started_at, ended_at)
             VALUES (?1, 1, 'implement', '{}', 'tools', 'c', '{\"cost_usd\":0.5}', ?2, ?2)",
            params![tx.last_insert_rowid(), ago],
        )?;
        Ok(())
    })
    .unwrap();
    say(&mut e, "Look at the numbers.");
    let wake = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 10, |e| path_of(e, wake).iter().any(|c| c == "understand.fit")),
        "{}",
        said.borrow().join("\n")
    );
    let understanding = message_at(&e, wake, "understand.state", false);
    let numbers = given_value(&understanding, "·engine·numbers").expect(&understanding);
    assert_eq!(
        numbers["weeks"][0]["spent_usd"],
        json!({ "items": 0.5, "architect": 0.25 }),
        "{numbers}"
    );
    let policy_right = &inputs_at(&e, wake, "understand.policy")[0];
    assert_eq!(
        given_ending(policy_right, "·engine·numbers"),
        Some(false),
        "{policy_right}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// A round of the understanding that asks for tools hands in no state of things: until the cell answers, the project
// has none, as at its first wake, and the cell's next round is still told it is the first wake. The cell's own inputs
// that have nothing are listed as none; the tool results, which every cell may have, only once there are some.
#[test]
fn a_round_of_the_understanding_that_asks_for_tools_changes_neither_the_state_nor_the_task() {
    let s = setup("understand-tools");
    let mut agents = scripts();
    agents.insert(
        "understand.state",
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
    let wake = e.step(None).unwrap().work.unwrap();
    let took = |row: &'static str| {
        move |e: &mut Engine| {
            cells_of(e, wake)
                .iter()
                .any(|c| c.0 == "understand.state" && c.1.as_deref() == Some(row))
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
    let (first, second) = (
        message_at(&e, wake, "understand.state", false),
        message_at(&e, wake, "understand.state", true),
    );
    for m in [&first, &second] {
        assert!(m.contains("This is the project's first wake"), "{m}");
    }
    let w = e.work(wake).unwrap();
    let g = e.graph_of(&w).unwrap();
    let numbered: Vec<&flow::CompiledInput> = g
        .cell("understand.state")
        .unwrap()
        .inputs
        .iter()
        .filter(|x| !x.from.iter().all(|s| *s == flow::format::Src::Itself))
        .collect();
    let number = |of: &dyn Fn(&flow::CompiledInput) -> bool| numbered.iter().position(|x| of(x)).unwrap() + 1;
    let constitution = number(&|x| x.from == [flow::format::Src::Engine("constitution".into())]);
    let tools = number(&|x| x.name.as_deref() == Some("Tool results"));
    let listed = |m: &str, n: usize| work_from(m).into_iter().find(|l| l.starts_with(&format!("{n}. ")));
    assert_eq!(
        listed(&first, constitution),
        Some(format!("{constitution}. none")),
        "{first}"
    );
    assert_eq!(listed(&first, tools), None, "{first}");
    assert!(
        listed(&second, tools).is_some_and(|l| l.contains("·tools·")),
        "{second}"
    );
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
        "policy.write",
        Box::new(|_: &Request, _: usize| {
            let policy = json!({ "goal": "Ship the parser.", "why": "Users wait for it.",
                                 "points": [{ "no": 1, "text": "Parse before checking." }], "stages": ["Lex."],
                                 "signals": [{ "what": "Speed", "how": "merged" }], "scope": "The front end." });
            let probes = json!([{ "id": "x", "ask": "What comes first?", "points": [1], "expect": "Point 1." }]);
            (Vec::new(), vec![("outcome.policy", policy), ("outcome.probes", probes)])
        }),
    );
    agents.insert(
        "policy.judge_read",
        Box::new(|_: &Request, n: usize| {
            if n > 1 {
                return (Vec::new(), Vec::new());
            }
            let gaps = json!([{ "what": "The probes x and y", "seen": "It hesitated on x.", "missing": "Point 1." }]);
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
        "policy.correct",
        Box::new(|_: &Request, n: usize| {
            let probe = |id: &str| json!({ "id": id, "ask": "What first?", "points": [1], "expect": "Point 1." });
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
    let v = e.flow_now;
    let wake =
        e.db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO works (item, flow_version, graph, at, project) VALUES (NULL, ?1, 'architect', 'policy.write', 'p')",
                [v],
            )?;
            Ok(tx.last_insert_rowid())
        })
        .unwrap();
    let judged = |e: &Engine| {
        cells_of(e, wake)
            .iter()
            .filter(|c| c.0 == "policy.judge_read" && c.1.is_some())
            .count()
    };
    for _ in 0..20 {
        if judged(&e) == 2 {
            break;
        }
        e.step_work(wake).unwrap();
    }
    let log = said.borrow().join("\n");
    assert_eq!(judged(&e), 2, "{log}\n{:?}", cells_of(&e, wake));
    let (judged, corrected) = (
        inputs_at(&e, wake, "policy.judge_read"),
        inputs_at(&e, wake, "policy.correct"),
    );
    for input in [&judged[0], &corrected[0]] {
        assert_eq!(given_ending(input, "·policy.write·policy"), Some(false), "{input}");
        assert_eq!(given_ending(input, "·policy.write·probes"), Some(false), "{input}");
    }
    // The correction changed a passage: the text the engine put together from it is given whole, the new probes are
    // the session's own return.
    assert_eq!(
        given_ending(&judged[1], "·policy.correct·revision.probes"),
        Some(false),
        "{}",
        judged[1]
    );
    assert_eq!(
        given_ending(&judged[1], "·policy.assemble·draft.policy"),
        Some(true),
        "{}",
        judged[1]
    );
    let text: Vec<Value> = engine::query_json(
        e.db.conn(),
        "SELECT policy FROM policy_text WHERE work = ?1",
        &[&wake],
        1,
    )
    .unwrap()
    .iter()
    .map(|r| r["policy"]["goal"].clone())
    .collect();
    assert_eq!(text, [json!("Ship the parser first.")]);
    let rejected: Vec<String> = engine::query_json(
        e.db.conn(),
        "SELECT rejected FROM rounds WHERE work = ?1 AND cell = 'policy.correct' AND rejected IS NOT NULL",
        &[&wake],
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

/// A wake of project p's architect at a cell, with no round yet.
fn wake_at(e: &mut Engine, cell: &str) -> i64 {
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

/// The line of the next message of a work that lists its input by number.
fn next_listed(e: &mut Engine, work: i64, number: usize) -> String {
    let m = engine::next_message(e, work).unwrap().text;
    work_from(&m)
        .into_iter()
        .find(|l| l.starts_with(&format!("{number}. ")))
        .unwrap_or_else(|| panic!("{m}"))
}

// `understand.fit` is given the study of a new policy and the owner's answer to it only when the owner's latest answer
// to a study in the wake said the policy in force stays, not when the owner was satisfied with the study.
#[test]
fn the_study_reaches_the_fit_question_only_when_the_owner_said_the_policy_stays() {
    let s = setup("stays");
    let (mut e, _) = engine(&s, scripts());
    quiet(&mut e);
    let wake = wake_at(&mut e, "understand.fit");
    let study =
        json!({ "outcome": { "study": { "intent": "Faster parsing." } }, "progress": "Studied.", "tell_owner": [] });
    let ruling = |choice: &str| json!({ "ruling": { "choice": choice, "text": "Seen." } });
    e.db.write(Actor::Engine, |tx| {
        record_round(tx, wake, 1, "policy.study", &study, "studied")?;
        record_round(tx, wake, 2, "policy.owner_reads", &ruling("satisfied"), "satisfied")
    })
    .unwrap();
    assert_eq!(next_listed(&mut e, wake, 8), "8. none");
    e.db.write(Actor::Engine, |tx| {
        record_round(
            tx,
            wake,
            3,
            "policy.owner_reads",
            &ruling("policy_stays"),
            "policy_stays",
        )
    })
    .unwrap();
    let stays = next_listed(&mut e, wake, 8);
    assert!(
        stays.contains("·policy.study·study") && stays.contains(&format!("R{wake}-3·policy.owner_reads·ruling")),
        "{stays}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// `understand.policy` is given only the signals of the policy in force that the engine measures, each with its numbers:
// a signal that names a measure of the numbers by its name. When the policy has none such, the input is none.
#[test]
fn the_policy_s_signals_come_only_when_the_engine_measures_them_with_their_numbers() {
    let s = setup("signals");
    let (mut e, _) = engine(&s, scripts());
    quiet(&mut e);
    let set = |e: &mut Engine, signals: Value| {
        let v = e.flow_now;
        let policy = json!({ "goal": "g", "why": "w", "points": [{ "no": 1, "text": "t" }], "stages": ["s"],
                             "signals": signals, "scope": "all" });
        let written =
            json!({ "outcome": { "policy": policy, "probes": [] }, "progress": "Written.", "tell_owner": [] });
        e.db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO works (item, flow_version, graph, at, ended, project)
                 VALUES (NULL, ?1, 'architect', 'wake.start', 'over', 'p')",
                [v],
            )?;
            let set = tx.last_insert_rowid();
            record_round(tx, set, 1, "policy.write", &written, "written")?;
            record_round(tx, set, 2, "policy.takes_effect", &json!({}), "new_architect")
        })
        .unwrap();
    };
    set(
        &mut e,
        json!([{ "what": "Fewer items merged", "how": "merged below 1 a week" },
               { "what": "The owner is unhappy", "how": "something the owner says" }]),
    );
    let wake = wake_at(&mut e, "understand.policy");
    let m = engine::next_message(&mut e, wake).unwrap().text;
    let signals = given_value(&m, "·engine·policy_signals").expect(&m);
    assert_eq!(
        signals["1"]["numbers"]["merged"]["by_week"],
        json!([0, 0, 0, 0]),
        "{signals}"
    );
    assert_eq!(signals.get("2"), None, "{signals}");
    set(
        &mut e,
        json!([{ "what": "The owner is unhappy", "how": "something the owner says" }]),
    );
    assert_eq!(next_listed(&mut e, wake, 3), "3. none");
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

// While a wake's batch is open, the engine starts no work on an item a change the batch collected names; once the
// batch's changes are made, the item starts as ready_items says.
#[test]
fn an_item_a_batch_changes_does_not_start_until_the_batch_ends() {
    let s = setup("batch-hold");
    let (mut e, said) = engine(&s, scripts());
    quiet(&mut e);
    let item = a_ready_item(&mut e, "Lex it");
    let wake = wake_at(&mut e, "pick.anything");
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "UPDATE works SET paused = 'later', paused_by = 'owner' WHERE id = ?1",
            [wake],
        )?;
        record_round(tx, wake, 1, "handle.owner_words", &json!({}), "done")?;
        let m = a_matter(tx, "Order the items.", None, json!({}))?;
        let change = json!({ "kind": "add_dependency", "item": item.to_string(), "on": "99", "dependency": "blocking",
                             "reason": "r" });
        tx.execute(
            "INSERT INTO architect_item_changes (work, round, matter, change, state)
             VALUES (?1, 1, ?2, ?3, 'collected')",
            params![wake, m, change.to_string()],
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

// A work that waits for the items its item depends on (`ask.wait_items`) goes on at once to choose the way on
// (`ask.way_on`) when one of them is cancelled, told which, why and what it needed of it; its item no longer depends on
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
                "INSERT INTO works (item, flow_version, graph, at) VALUES (?1, ?2, 'engineer', 'ask.wait_items')",
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
    assert_eq!(at(&e), "ask.wait_items", "{}", said.borrow().join("\n"));
    let projects = e.config.config.projects.clone();
    e.db.write(Actor::Owner, |tx| {
        engine::ending::cancel_item(tx, lexer, "no longer needed", "owner", &projects).map_err(db::Error::Abort)
    })
    .unwrap();
    e.step_work(work).unwrap();
    assert_eq!(at(&e), "ask.way_on", "{}", said.borrow().join("\n"));
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

// `understand.fit` hands in one row for each place to set straight, with every step it means to take there, and the
// engine queues the row as one matter, which says its steps.
#[test]
fn a_place_to_set_straight_is_one_matter_with_its_steps() {
    let s = setup("straight");
    let mut agents = scripts();
    agents.insert(
        "understand.fit",
        Box::new(|_: &Request, _: usize| {
            let fixes = json!([{ "where": "The policy's second stage", "kind": "missing", "wrong": "No item does it.",
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
    let wake = wake_at(&mut e, "understand.state");
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
        e.step_work(wake).unwrap();
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
        "handle.owner_words",
        Box::new(move |_: &Request, _: usize| {
            (
                Vec::new(),
                vec![("outcome.effects", json!({ "item_changes": changes.clone() }))],
            )
        }),
    );
    agents.insert(
        "batch.review",
        Box::new(|_: &Request, _: usize| {
            let ok = |n: i64| json!({ "change": n, "verdict": "ok", "why": "Sound." });
            (Vec::new(), vec![("outcome.review", json!([ok(1), ok(2)]))])
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    let first = an_item(&mut e);
    say(&mut e, "Measure the spend before parsing.");
    let wake = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 40, |e| wake_done(e, wake)),
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
// the engine as it looks at the queue, saying why, and never reaches `handle.card_sound`.
#[test]
fn a_design_card_confirmed_or_replaced_is_closed_without_a_look() {
    let s = setup("settled-card");
    let (mut e, said) = engine(&s, scripts());
    quiet(&mut e);
    let item = an_item(&mut e);
    let v = e.flow_now;
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO works (item, flow_version, graph, at, ended) VALUES (?1, ?2, 'engineer', 'implement', 'stopped: by hand')",
            params![item, v],
        )?;
        let work = tx.last_insert_rowid();
        record_round(tx, work, 1, "implement", &json!({}), "done")?;
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
    let wake = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 30, |e| wake_done(e, wake)),
        "{}",
        said.borrow().join("\n")
    );
    let log = said.borrow().join("\n");
    assert!(!path_of(&e, wake).contains(&"handle.card_sound".to_string()), "{log}");
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
    assert!(closed.iter().all(|c| c.0 == "pick.anything"), "{closed:?}");
    assert!(
        closed[0].1.contains("confirmed by the owner") && closed[0].1.contains("replaced by another"),
        "{closed:?}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// The items a matter concerns are the one it names, the one its fix names and those its words name: a number in its
// material, such as a pull request's, brings in no item.
#[test]
fn a_matter_s_items_are_not_taken_from_numbers_in_its_material() {
    let s = setup("matter-items");
    let (mut e, _) = engine(&s, scripts());
    quiet(&mut e);
    let named = a_ready_item(&mut e, "Lex it");
    let other = a_ready_item(&mut e, "Print it");
    let wake = wake_at(&mut e, "handle.owner_words");
    e.db.write(Actor::Engine, |tx| {
        let body = json!({ "said": format!("See PR #{other} for the parser.") });
        let m = a_matter(tx, &format!("Look at item {named}."), None, body)?;
        record_round(
            tx,
            wake,
            1,
            "pick.anything",
            &json!({ "queue": { "verdict": "one", "matter": m } }),
            "one",
        )
    })
    .unwrap();
    let m = engine::next_message(&mut e, wake).unwrap().text;
    let items = given_value(&m, "·engine·matter_items").expect(&m);
    let keys: Vec<&String> = items.as_object().unwrap().keys().collect();
    assert_eq!(keys, [&format!("#{named}")], "{items}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A side report a work handed in before, word for word, is not queued again.
#[test]
fn a_side_report_handed_in_again_is_queued_once() {
    let s = setup("report-again");
    let mut agents = scripts();
    agents.insert(
        "research.problem",
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

// A new session of the architect's line that would start in the middle of a wake, here at a matter's path, holds no
// state of things: the wake goes back to understanding the project first, in the new session, and the matter picked
// goes back to the queue, to be picked again and handled.
#[test]
fn a_new_session_in_the_middle_of_a_wake_understands_the_project_first() {
    let s = setup("new-session-midway");
    let mut agents = scripts();
    picks_first(&s, &mut agents);
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    say(&mut e, "One.");
    say(&mut e, "Two.");
    let wake = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 20, |e| e.work(wake).unwrap().pos.at == "handle.owner_words"),
        "{}",
        said.borrow().join("\n")
    );
    e.sessions.ask_new(api::Fresh::Architect("p".into()));
    assert!(
        steps_until(&mut e, 40, |e| wake_done(e, wake)),
        "{}",
        said.borrow().join("\n")
    );
    let log = said.borrow().join("\n");
    let path = path_of(&e, wake);
    // The wake was at `handle.owner_words`, the matter picked, when the new session was asked for.
    let back = path.iter().rposition(|c| c == "wake.understood").expect(&log);
    assert_eq!(
        &path[back - 1..back + 3],
        ["pick.route", "wake.understood", "understand.state", "understand.policy"],
        "{path:?}\n{log}"
    );
    assert!(path[back..].contains(&"handle.owner_words".to_string()), "{path:?}");
    let lines = line_sessions(&e, wake, "architect");
    assert_eq!(lines.len(), 2, "{lines:?}\n{log}");
    assert!(lines[1].2.starts_with("understand.state,"), "{lines:?}");
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
        "handle.owner_words",
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
    let wake = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 40, |e| wake_done(e, wake)),
        "{}",
        said.borrow().join("\n")
    );
    let rejected: Vec<String> = engine::query_json(
        e.db.conn(),
        "SELECT rejected FROM rounds WHERE work = ?1 AND cell = 'handle.owner_words' AND rejected IS NOT NULL",
        &[&wake],
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

// The architect's wakes are taken the same way: started while every work is paused, a wake starts when a matter is
// due and goes on; the owner's Pause of the architect pauses the wake, and its Step leaves the wake paused by the owner.
#[test]
fn the_architect_the_owner_starts_wakes_and_goes_on_while_every_work_is_paused() {
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
    let wake = moved[0];
    assert!(moved.iter().all(|w| *w == wake), "{moved:?}");
    assert_eq!(paused_by(&e, wake), None);
    // Paused, the wake takes no step, and no other wake starts.
    c.pause(&api::Fresh::Architect("p".into()));
    api::pause_wake(&mut e.db, "p").unwrap();
    assert_eq!(turns(&mut e, &c, 2), Vec::<i64>::new());
    assert_eq!(paused_by(&e, wake).as_deref(), Some("owner"));
    // Started again, its pause is lifted; a round asked for leaves it paused by the owner.
    c.start(api::Fresh::Architect("p".into()), false);
    assert_eq!(turns(&mut e, &c, 1), [wake]);
    assert_eq!(paused_by(&e, wake), None);
    c.ask_step_architect("p", false).unwrap();
    let stepped = turn(&mut e, &c, None);
    assert_eq!(stepped.work, Some(wake));
    assert_eq!(paused_by(&e, wake).as_deref(), Some("owner"));
    assert!(c.asked().architects_started.is_empty());
    assert_eq!(wakes(&e), [wake]);
    let _ = std::fs::remove_dir_all(&s.root);
}

// The owner's Step or Start can ask for a new session: the round starts one for its cell's line, recorded as the
// owner's, given everything its cell needs; the session before ends and is otherwise left as it is, and the line's
// later rounds go on in the new one. For the architect, the new session is its wake's.
#[test]
fn the_owner_asks_for_a_new_session_with_a_step_or_a_start() {
    let s = setup("own-new-session");
    let (mut e, said, item) = start(&s, scripts());
    agent_rounds_to(&mut e, item, 2);
    let (work, at, _) = state(&e, item).unwrap();
    assert_eq!(at, "design.compare", "{}", said.borrow().join("\n"));
    let c = api::Control::default();
    c.ask_step(work, true).unwrap();
    turn(&mut e, &c, None);
    let log = said.borrow().join("\n");
    let lines = line_sessions(&e, work, "engineer");
    assert_eq!(
        lines,
        [
            ("line".to_string(), true, "research.problem,research.stands".to_string()),
            ("owner".to_string(), false, "design.compare".to_string())
        ],
        "{log}"
    );
    let rounds = agent_rounds(&e, item);
    let two = &rounds[2].2;
    for a in ["format:common", "format:design.compare", "rule:ground"] {
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

// The architect's Step in a new session starts one for its wake, recorded as the owner's; the wake's next rounds go on
// in it, the understanding of the project first, since the session is new in the middle of the wake.
#[test]
fn the_owner_asks_for_a_new_session_for_the_architect_s_wake() {
    let s = setup("own-new-wake-session");
    let (mut e, said) = engine(&s, scripts());
    quiet(&mut e);
    let c = api::Control::default();
    say(&mut e, "Look at the queue.");
    c.start(api::Fresh::Architect("p".into()), false);
    // Waking and finding the session has not understood the project take the engine's own steps; then the first round.
    let wake = turns(&mut e, &c, 3)[0];
    let log = said.borrow().join("\n");
    let lines = line_sessions(&e, wake, "architect");
    assert_eq!(lines.len(), 1, "{lines:?}\n{log}");
    c.pause(&api::Fresh::Architect("p".into()));
    c.ask_step_architect("p", true).unwrap();
    turn(&mut e, &c, None);
    for _ in 0..2 {
        c.ask_step_architect("p", false).unwrap();
        turn(&mut e, &c, None);
    }
    let log = said.borrow().join("\n");
    assert_eq!(
        path_of(&e, wake),
        [
            "wake.start",
            "wake.understood",
            "understand.state",
            "wake.understood",
            "understand.state",
            "understand.policy"
        ],
        "{log}"
    );
    let lines = line_sessions(&e, wake, "architect");
    assert_eq!(lines.len(), 2, "{lines:?}\n{log}");
    assert!(lines[0].1, "{lines:?}");
    assert_eq!((lines[1].0.as_str(), lines[1].1), ("owner", false), "{lines:?}");
    assert_eq!(lines[1].2, "understand.state,understand.policy", "{lines:?}\n{log}");
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
        "research.stands",
        Box::new(|_: &Request, _: usize| (Vec::new(), vec![("@usage", used(1.0, 9_000))])),
    );
    let (mut e, said, item) = start(&s, scripts);
    agent_rounds_to(&mut e, item, 2);
    let (work, at, _) = state(&e, item).unwrap();
    assert_eq!(at, "design.compare", "{}", said.borrow().join("\n"));
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
        json!({ "cell": "design.compare", "on": "pause", "what": "the owner paused every work" })
    );
    // The owner's switch is kept in the database, as the owner's, across a restart.
    assert!(api::keep_warm_paused(e.db.conn()).unwrap());
    api::set_keep_warm_paused(&mut e.db, false).unwrap();
    drop(e);
    let (e, _) = engine(&s, BTreeMap::new());
    assert!(!api::keep_warm_paused(e.db.conn()).unwrap());
    let _ = std::fs::remove_dir_all(&s.root);
}
