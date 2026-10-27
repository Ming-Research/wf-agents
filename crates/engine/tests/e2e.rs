//! One item through the whole engineer graph, from the queue to main: a scripted agent stands in for claude and a
//! scripted GitHub for gh, over real git repositories in a temporary directory. Every return the agent hands in is
//! built from the cell's own schema, so a change to the flow that the engine does not follow fails here.

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
/// as a run that failed with that report; under `@compactions`, the session was compacted that many times.
type Script = Box<dyn Fn(&Request, usize) -> (Vec<(&'static str, &'static str)>, Vec<(&'static str, Value)>)>;

struct Scripted {
    scripts: BTreeMap<&'static str, Script>,
    rounds: RefCell<BTreeMap<String, usize>>,
}

impl Agents for Scripted {
    fn run(&self, r: &Request, _activity: &dyn Fn(&str)) -> Reply {
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
        set.entry("side_reports".into()).or_insert(json!([]));
        set.entry("tell_owner".into()).or_insert(json!([]));
        set.entry("progress".into())
            .or_insert(json!(format!("round {n} at {}", r.cell)));
        Reply {
            output: Some(example(&r.schema, "", &pick, &set)),
            session,
            model: Some(r.model.clone()),
            usage: json!({ "cost_usd": 0.01 }),
            compactions,
            error: None,
            command: Vec::new(),
        }
    }

    fn version(&self, _cli: &config::Cli, _config_dir: Option<&Path>) -> String {
        "scripted".to_string()
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
    Setup { root, data, origin }
}

fn scripts() -> BTreeMap<&'static str, Script> {
    let mut s: BTreeMap<&'static str, Script> = BTreeMap::new();
    s.insert(
        "2a",
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
        "2b",
        Box::new(|_: &Request, _: usize| (vec![("outcome.better", "no")], Vec::new())),
    );
    s.insert(
        "3f",
        Box::new(|_: &Request, _: usize| (vec![("outcome.adversarial", "no")], Vec::new())),
    );
    // Implementing: the first round changes the code and asks for ci; the second hands in the end.
    s.insert(
        "4",
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
                    ("outcome.end.head", json!(head)),
                    ("outcome.end.conditions", json!([{ "how": "tested", "id": "A1", "tests": ["feature"] }])),
                ],
            )
        }),
    );
    s.insert(
        "5a",
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

/// The engine over a setup, with what it says kept, and one item in the queue.
fn start(s: &Setup, scripts: BTreeMap<&'static str, Script>) -> (Engine, Rc<RefCell<Vec<String>>>, i64) {
    let (mut e, said) = engine(s, scripts);
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
    assert_eq!(at, "20", "{log}");
    assert_eq!(pr, Some(1));
    let main = git(
        &s.root,
        &["--git-dir", &s.origin.to_string_lossy(), "show", "main:src/lib.txt"],
    );
    assert_eq!(main, "old\nfeature");
    let cells: Vec<String> = engine::query_json(
        e.db.conn(),
        "SELECT cell, transition FROM rounds WHERE work = (SELECT id FROM works WHERE item = ?1) ORDER BY id",
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
        "0:started",
        "4:tools",
        "13:ran",
        "7:review_first",
        "5:done",
        "9:merged",
        "10:green",
        "20:-",
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
                    "SELECT input FROM rounds WHERE cell = ?1 ORDER BY id LIMIT 1",
                    [cell],
                    |r| r.get(0),
                )
                .unwrap();
        db::put_back(e.db.conn(), &serde_json::from_str(&input).unwrap())
    };
    let columns = input_of("5a")["engine"]["columns"].to_string();
    assert!(!columns.contains("a gap in what is under test"), "{columns}");
    assert!(!columns.contains("cross-project dependency"), "{columns}");
    assert!(columns.contains("unsure facts"), "{columns}");
    // The review line's session is given the worktree once, at its first round, the blind review.
    let blind = input_of("5");
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
    assert_eq!(at, "1a", "{log}");
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
    assert!(m.text.contains("cell 1a"), "{}", m.text);
    drop(r);
    assert_eq!(state_of(), before);
    let _ = std::fs::remove_dir_all(&s.root);
}

// While the merge gate waits for the owner's ruling on the design card, the next round offers no round: it says what
// the work waits for, and which design card.
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
    let card =
        e.db.write(Actor::Engine, |tx| {
            let round: i64 = tx.query_row("SELECT max(id) FROM rounds WHERE work = ?1", [work], |r| r.get(0))?;
            tx.execute(
                "INSERT INTO design_cards (round, body, status) VALUES (?1, '[]', 'open')",
                [round],
            )?;
            let card = tx.last_insert_rowid();
            tx.execute("UPDATE works SET at = '19' WHERE id = ?1", [work])?;
            Ok(card)
        })
        .unwrap();
    let next = engine::next_round(&mut e, work).unwrap();
    assert_eq!(next.cell, "19", "{log}");
    assert_eq!(
        next.waits.as_deref(),
        Some(format!("the owner's ruling on design card {card}").as_str()),
        "{log}"
    );
    assert_eq!(next.design_card, Some(card));
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
    let implement = agents.remove("4").unwrap();
    agents.insert(
        "4",
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
                 WHERE w.item = ?1 AND s.line = 'advisor' ORDER BY r.id LIMIT 1",
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
                 WHERE w.item = ?1 AND s.line = 'engineer' ORDER BY r.id LIMIT 1",
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
            tx.execute("UPDATE works SET at = '16' WHERE id = ?1", [work])?;
            tx.execute(
                "INSERT INTO rounds (work, cell, input, config_commit, started_at) VALUES (?1, '16', '{}', 'c', 1)",
                [work],
            )?;
            let round = tx.last_insert_rowid();
            tx.execute(
                "INSERT INTO owner_asks (round, body) VALUES (?1, '{\"to\":\"owner\"}')",
                [round],
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
                "INSERT INTO rounds (work, cell, input, output, transition, config_commit, started_at, ended_at)
                 VALUES (?1, '7', '{}', ?2, 'next', 'c', 1, 2)",
                params![work, output.to_string()],
            )?;
            tx.execute("UPDATE works SET at = '8' WHERE id = ?1", [work])?;
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
    let rounds = |e: &Engine| -> i64 {
        e.db.conn()
            .query_row("SELECT count(*) FROM rounds", [], |r| r.get(0))
            .unwrap()
    };
    let first = e.step(None).unwrap();
    let log = said.borrow().join("\n");
    let (work, at, _) = state(&e, item).unwrap_or_else(|| panic!("no work started:\n{log}"));
    assert_eq!(first.work, Some(work), "{log}");
    assert_eq!(first.started_item, Some(item));
    assert_eq!(first.cell.as_deref(), Some("0"));
    assert_eq!(rounds(&e), 1, "{log}");
    assert_ne!(at, "0", "{log}");
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
        "1",
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
    assert!(at != "0" && at != "1", "{log}");
    let (cell, text, read): (String, String, Option<i64>) =
        e.db.conn()
            .query_row(
                "SELECT r.cell, m.text, m.read_at FROM owner_messages m JOIN rounds r ON r.id = m.round",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
    assert_eq!(
        (cell.as_str(), text.as_str(), read),
        ("1", "The item's example does not run as written.", None)
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
    assert_eq!(at, "1a", "{log}");
    let session: i64 =
        e.db.conn()
            .query_row(
                "SELECT id FROM sessions WHERE work = ?1 AND line = 'engineer'",
                [work],
                |r| r.get(0),
            )
            .unwrap();
    // The engine stops while cell 1a runs.
    let cut = e
        .db
        .write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO rounds (work, cell, session, input, config_commit, started_at) VALUES (?1, '1a', ?2, '{}', 'c', 1)",
                params![work, session],
            )?;
            Ok(tx.last_insert_rowid())
        })
        .unwrap();
    e.step(None).unwrap();
    let rounds = agent_rounds(&e, item);
    let (id, cell, input) = rounds.last().unwrap();
    assert_eq!(cell, "1a");
    let m = input["message"].as_str().unwrap();
    assert!(
        m.starts_with(&format!(
            "# R{id} · cell 1a\n\nR{cut} was cut off midway. First look at the worktree"
        )),
        "{m}"
    );
    assert_eq!(input["conditional"], json!([]));
    assert!(input["call"]["session"]["resumed"].is_string(), "{input}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// When the flow moves to a new version, a work running on the old one stops, saying why, and its item gets a new
// work at once: it takes over the branch, the worktree and the pull request, and its cell 1 is told what exists. An
// item whose works ended can be queued again.
#[test]
fn a_new_flow_version_stops_a_work_and_its_item_starts_again_from_what_exists() {
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
    drop(e);
    let (mut e, said) = engine(&s, scripts());
    let works = works_of(&e, item);
    assert_eq!(works.len(), 2, "{works:?}");
    assert!(
        works[0]
            .5
            .as_deref()
            .is_some_and(|w| w.starts_with("stopped: the flow moved to version")),
        "{works:?}"
    );
    let new = works[1].0;
    assert_eq!(works[1].1, "0");
    e.step(None).unwrap();
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
            .query_row("SELECT input FROM rounds WHERE work = ?1 AND cell = '1'", [new], |r| {
                r.get(0)
            })
            .unwrap();
    let input: Value = db::put_back(e.db.conn(), &serde_json::from_str(&input).unwrap());
    let m = input["message"].as_str().unwrap();
    for want in [
        "This item was worked on before.".to_string(),
        format!("- Work {old} "),
        format!("WHERE work = {old} ORDER BY id"),
        "git log --oneline origin/main..HEAD".to_string(),
        format!("pr view {} --comments", pr.unwrap()),
        "it was half done before".to_string(),
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
            tx.execute(
                "INSERT INTO rounds (work, cell, input, config_commit, started_at) VALUES (?1, ?2, '{}', 'c', 1)",
                params![work, at],
            )?;
            Ok(tx.last_insert_rowid())
        })
        .unwrap()
    };
    let rejected = |e: &Engine, round: i64| -> (Option<i64>, Value) {
        let (ended, rejected): (Option<i64>, Option<String>) =
            e.db.conn()
                .query_row("SELECT ended_at, rejected FROM rounds WHERE id = ?1", [round], |r| {
                    Ok((r.get(0)?, r.get(1)?))
                })
                .unwrap();
        (
            ended,
            rejected
                .map(|x| serde_json::from_str(&x).unwrap())
                .unwrap_or(Value::Null),
        )
    };
    let left = open_round(&mut e);
    e.pause(work, "cell 1: the return does not fit").unwrap();
    let (ended, why) = rejected(&e, left);
    assert!(ended.is_some());
    assert_eq!(why, json!({ "error": "cell 1: the return does not fit" }));
    let told: String =
        e.db.conn()
            .query_row("SELECT text FROM engine_notices WHERE work = ?1", [work], |r| r.get(0))
            .unwrap();
    assert!(told.ends_with("is paused: cell 1: the return does not fit"), "{told}");
    // An earlier engine paused without closing the round; the owner lets the work go on.
    let earlier = open_round(&mut e);
    e.db.write(Actor::Owner, |tx| {
        tx.execute("UPDATE works SET paused = NULL WHERE id = ?1", [work])?;
        Ok(())
    })
    .unwrap();
    e.step(None).unwrap();
    let log = said.borrow().join("\n");
    let (ended, why) = rejected(&e, earlier);
    assert!(ended.is_some(), "{log}");
    assert_eq!(
        why,
        json!({ "error": "the work was paused: cell 1: the return does not fit" }),
        "{log}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// A query an agent asks for runs in the tool cell, and the tool cell's return, the rows included, fits the schema the
// compiled flow gives cell 13: the engine checks its own tool cell's return against it and pauses the work when it
// does not, so a result the flow does not describe fails here.
#[test]
fn a_query_runs_in_the_tool_cell_and_its_return_fits_the_flow() {
    let s = setup("query");
    let mut agents = scripts();
    let implement = agents.remove("4").unwrap();
    agents.insert(
        "4",
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
                 WHERE w.item = ?1 AND r.cell = '13' AND r.transition IS NOT NULL ORDER BY r.id LIMIT 1",
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
    let rows = result["ran"]["rows"].as_str().unwrap_or_default();
    assert!(rows.lines().any(|l| l.contains("Add the feature")), "{output}");
    let w = e.work(work).unwrap();
    let g = e.graph_of(&w).unwrap();
    let schema = g.cell("13").and_then(|c| c.schema.clone()).unwrap();
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
        if state(&e, item).is_some_and(|(_, at, _)| at == "3a") {
            break;
        }
        e.step(None).unwrap();
    }
    let log = said.borrow().join("\n");
    let (work, at, _) = state(&e, item).unwrap();
    assert_eq!(at, "3a", "{log}");
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
                "SELECT output, input FROM rounds WHERE work = ?1 AND cell = '3a'",
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
                    "INSERT INTO works (item, flow_version, graph, at) VALUES (?1, ?2, 'engineer', '1')",
                    params![item, v],
                )?;
                items.push(item);
            }
            Ok(items)
        })
        .unwrap();
    drop(e);
    let (mut e, said) = engine(&s, scripts());
    let log = said.borrow().join("\n");
    let ready = |e: &Engine| -> Vec<i64> {
        let mut st = e.db.conn().prepare("SELECT item FROM ready_items").unwrap();
        st.query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect()
    };
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
    assert_eq!(ready(&e), vec![queued], "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// An item imported with a branch and its pull request takes them over in its first work, and its cell 1 is told
// where it came from.
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
        "1",
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
    assert!(why.starts_with("3 returns of cell 1 in a row were sent back"), "{why}");
    assert!(why.contains("outcome must be an object, and is a string"), "{why}");
    e.db.write(Actor::Owner, |tx| {
        tx.execute("UPDATE works SET paused = NULL WHERE id = ?1", [id])?;
        Ok(())
    })
    .unwrap();
    for _ in 0..3 {
        e.tick().unwrap();
    }
    let log = said.borrow().join("\n");
    let (_, at, paused) = state(&e, item).unwrap();
    assert_eq!(paused, None, "{log}");
    assert_ne!(at, "1", "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

/// Each agent round of the item's work, oldest first: its number, its cell, and what the engine recorded it gave.
fn agent_rounds(e: &Engine, item: i64) -> Vec<(i64, String, Value)> {
    engine::query_json(
        e.db.conn(),
        "SELECT r.id, r.cell, r.input FROM rounds r JOIN works w ON w.id = r.work
         WHERE w.item = ?1 AND r.session IS NOT NULL ORDER BY r.id",
        &[&item],
        1000,
    )
    .unwrap()
    .into_iter()
    .map(|r| {
        (
            r["id"].as_i64().unwrap(),
            r["cell"].as_str().unwrap().to_string(),
            db::put_back(e.db.conn(), &r["input"]),
        )
    })
    .collect()
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
        if agent_rounds(&e, item).iter().any(|r| r.1 == "5") {
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
        assert!(m.starts_with(&format!("# R{id} · cell {cell} · ")), "{m}");
    }
    let (first, second) = (&at("1"), &at("1a"));
    let (work, _, _) = state(&e, item).unwrap();
    let workspace = format!("work:{work}");
    for a in [
        "format:common",
        "format:1",
        "inputs:1",
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
    assert_eq!(given(second, "format:1a"), Some(true));
    let m = second["message"].as_str().unwrap();
    assert!(!m.contains("### rule:ground") && m.contains("rule:ground"), "{m}");
    let m = first["message"].as_str().unwrap();
    // The index of the database is in the query tool's description, which the return formats give.
    assert!(!m.contains("db:tables"), "{m}");
    assert!(given(first, "db:tables").is_none());
    // The formats name the owner's language; the engine writes the configured one beside it.
    assert!(m.contains("`owner.language` (English)"), "{m}");
    assert!(m.contains("SELECT sql FROM sqlite_master WHERE name = '<name>'"), "{m}");
    assert!(m.contains("- items: A piece of work to do"), "{m}");
    assert!(!m.contains("CREATE TABLE"), "{m}");
    let design = rounds.iter().rev().find(|r| r.1 == "2a").unwrap().0;
    let acceptance = format!("R{design}·2a·acceptance");
    assert_eq!(given(&at("3f"), &acceptance), Some(false), "{}", at("3f"));
    assert_eq!(given(&at("4"), &acceptance), Some(false), "{}", at("4"));
    assert_eq!(given(&at("5"), &acceptance), Some(true), "{}", at("5"));
    assert!(
        !rounds
            .iter()
            .any(|r| r.2["conditional"].to_string().contains("item_acceptance"))
    );
    let call = &second["call"];
    assert_eq!(call["cli"], "scripted");
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
    // The record keeps each large text once: the shared formats and the structured-output schema are in contents
    // once, however many rounds and sessions were given them, and a round's record refers to them.
    let kept = |like: &str| -> i64 {
        e.db.conn()
            .query_row("SELECT count(*) FROM contents WHERE text LIKE ?1", [like], |r| r.get(0))
            .unwrap()
    };
    assert_eq!(kept("The forms every agent's return shares.%"), 1);
    assert_eq!(kept("{\"type\":\"object\"%"), 1);
    let raw: String =
        e.db.conn()
            .query_row("SELECT input FROM rounds WHERE id = ?1", [rounds[0].0], |r| r.get(0))
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

// A conditional input whose content changed is given again under its address; after a compaction the session holds nothing,
// and the next round is given what it needs again, the session's own earlier returns included.
#[test]
fn a_changed_conditional_input_is_given_again_and_so_is_everything_after_a_compaction() {
    let s = setup("again");
    let mut scripts = scripts();
    scripts.insert(
        "1a",
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
    assert_eq!(&cells[..3], ["1", "1a", "2"], "{log}");
    let (a, b, c) = (&rounds[0].2, &rounds[1].2, &rounds[2].2);
    assert_eq!(given(b, "rule:ground"), Some(true));
    assert_ne!(hash_of(a, "rule:ground"), hash_of(b, "rule:ground"));
    assert_eq!(given(b, "format:common"), Some(false));
    assert_eq!(given(c, "format:common"), Some(true));
    assert_eq!(given(c, "rule:ground"), Some(true));
    assert_eq!(hash_of(b, "rule:ground"), hash_of(c, "rule:ground"));
    let understanding = format!("R{}·1·understanding", rounds[0].0);
    assert_eq!(given(c, &understanding), Some(true), "{c}");
    let m = c["message"].as_str().unwrap();
    assert!(m.contains(&format!("compacted in R{}", rounds[1].0)), "{m}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// How many returns sent back pause a work is the number the flow the work runs on says.
#[test]
fn returns_sent_back_pause_the_work_after_as_many_as_its_flow_says() {
    let s = setup("pause");
    let mut scripts = scripts();
    scripts.insert(
        "1",
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
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO flow_versions (digest, compiled) VALUES (?1, ?2)",
            params![digest, text],
        )?;
        let v = tx.last_insert_rowid();
        tx.execute("UPDATE works SET flow_version = ?2 WHERE id = ?1", params![work, v])?;
        Ok(())
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
    assert_eq!(at, "1", "{log}");
    assert!(
        paused
            .as_deref()
            .is_some_and(|p| p.starts_with("2 returns of cell 1 in a row were sent back")),
        "{paused:?}\n{log}"
    );
    // After a return sent back the message only says why, and that the return is to be rewritten.
    let rounds = agent_rounds(&e, item);
    let (id, _, again) = &rounds[1];
    let m = again["message"].as_str().unwrap();
    assert!(
        m.starts_with(&format!("# R{id} · cell 1\n\nYour return was sent back: "))
            && m.ends_with("\nRewrite it (format:1).\n"),
        "{m}"
    );
    assert_eq!(again["conditional"], json!([]));
    let _ = std::fs::remove_dir_all(&s.root);
}

/// The engine over a setup whose cell 1 ends its first `times` runs without a return, the CLI reporting `report`.
fn failing(name: &str, report: String, times: usize) -> (Setup, Engine, Rc<RefCell<Vec<String>>>, i64) {
    let s = setup(name);
    let mut scripts = scripts();
    scripts.insert(
        "1",
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
         WHERE json_extract(rejected, '$.failed') IS NOT NULL ORDER BY id",
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
    assert_ne!(at, "1", "{log}");
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
    assert_ne!(at, "1");
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
        if state(&e, item).unwrap().1 != "1" {
            break;
        }
    }
    let log = said.borrow().join("\n");
    let (_, at, paused) = state(&e, item).unwrap();
    assert_ne!(at, "1", "{log}");
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
    assert_ne!(at, "1", "{log}");
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
    assert_eq!(at, "1", "{log}");
    let paused = paused.unwrap_or_else(|| panic!("the work did not pause:\n{log}"));
    assert!(paused.contains("prompt_too_long"), "{paused}");
    // The owner is told when the outbox is carried out, at the start of the next pass; the paused work stays.
    e.tick().unwrap();
    assert_eq!(told(&s, "is paused"), 1);
    assert_eq!(failed_kinds(&e), ["prompt_too_long"]);
    assert_eq!(sent_back(&e), 0);
    e.db.write(db::Actor::Owner, |tx| {
        tx.execute("UPDATE works SET paused = NULL WHERE id = ?1", [work])?;
        Ok(())
    })
    .unwrap();
    for _ in 0..5 {
        e.tick().unwrap();
        if state(&e, item).unwrap().1 != "1" {
            break;
        }
    }
    let log = said.borrow().join("\n");
    let (_, at, paused) = state(&e, item).unwrap();
    assert_eq!(paused, None, "{log}");
    assert_ne!(at, "1", "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}
