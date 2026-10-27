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
    engine::database::create(&data).unwrap();
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
                    ("outcome.head", json!(head)),
                    ("outcome.conditions", json!([{ "how": "tested", "id": "A1", "tests": ["feature"] }])),
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

/// Keeps the architect from waking for an advance while a test looks at something else: an advance waits in its
/// queue, due only long after the test, so the engine raises no other.
fn quiet(e: &mut Engine) {
    let later = db::now() + 10 * 365 * 24 * 3_600_000;
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO architect_matters (kind, what, body, at, due) VALUES ('advance', 'not yet', '{}', ?1, ?2)",
            params![db::now(), later],
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
    assert_eq!(at, "20", "{log}");
    assert_eq!(pr, Some(1));
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
                    "SELECT input FROM rounds WHERE cell = ?1 ORDER BY work, n LIMIT 1",
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
            tx.execute("UPDATE works SET at = '19' WHERE id = ?1", [work])?;
            Ok(card)
        })
        .unwrap();
    let next = engine::next_round(&mut e, work).unwrap();
    assert_eq!(next.cell, "19", "{log}");
    assert_eq!(
        next.waits.as_deref(),
        Some(format!("the architect's look at design card {card}").as_str()),
        "{log}"
    );
    assert_eq!(next.design_card, Some(card));
    // The architect disagrees: the engineer is told, and the gate waits for the owner.
    let disagree = [db::Verdict {
        decision: 1,
        verdict: "disagree".into(),
        option: None,
        text: Some("It does not fit the whole.".into()),
    }];
    e.db.write(Actor::Architect, |tx| {
        db::rule_design_card(tx, card, "architect", &disagree).map_err(db::Error::Abort)
    })
    .unwrap();
    let said: String =
        e.db.conn()
            .query_row(
                "SELECT text FROM interjections WHERE work = ?1 AND speaker = 'architect'",
                [work],
                |r| r.get(0),
            )
            .unwrap();
    assert!(said.starts_with(&format!("Ruling on design card {card}:")), "{said}");
    // The engineer has taken the interjection in.
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "UPDATE interjections SET delivered_round = (SELECT max(n) FROM rounds WHERE work = ?1) WHERE work = ?1",
            [work],
        )?;
        Ok(())
    })
    .unwrap();
    let next = engine::next_round(&mut e, work).unwrap();
    assert_eq!(
        next.waits.as_deref(),
        Some(format!("the owner's ruling on design card {card}").as_str()),
        "{log}"
    );
    assert_eq!(next.design_card, Some(card));
    // Where the two differ, the owner's word goes: the owner confirms, and the gate waits no longer.
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
        "2b",
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
    let at_2b = |e: &Engine| -> Vec<(Option<String>, Option<String>)> {
        let mut st =
            e.db.conn()
                .prepare(
                    "SELECT r.transition, r.rejected FROM rounds r JOIN works w ON w.id = r.work
                     WHERE w.item = ?1 AND r.cell = '2b' AND r.ended_at IS NOT NULL ORDER BY r.work, r.n",
                )
                .unwrap();
        st.query_map([item], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(|x| x.unwrap())
            .collect()
    };
    for _ in 0..40 {
        if at_2b(&e).iter().any(|(t, _)| t.is_some()) {
            break;
        }
        e.step(None).unwrap();
    }
    let log = said.borrow().join("\n");
    let rounds = at_2b(&e);
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
        "2b",
        Box::new(move |r: &Request, n: usize| {
            if n == 1 {
                main_moves_on(&root, &[("design/main.md", "Decision: main's.\n")], "Main's decision");
                merge_main(&r.worktree);
            }
            (vec![("outcome.better", "no")], Vec::new())
        }),
    );
    agents.insert(
        "4",
        implementing_over_a_merge(
            s.root.clone(),
            &[],
            &[("design/later.md", "Decision: main's later one.\n")],
        ),
    );
    let (mut e, said, item) = start(&s, agents);
    step_to(&mut e, item, "4");
    let at_2b = ended_at(&e, item, "2b");
    assert_eq!(at_2b.len(), 1, "{at_2b:?}\n{}", said.borrow().join("\n"));
    assert!(at_2b[0].0.is_some(), "{at_2b:?}");
    let (work, _, _) = state(&e, item).unwrap();
    let wt = worktree_of(&e, work);
    card_at(&mut e, work, &git(&wt, &["rev-parse", "HEAD"]));
    for _ in 0..10 {
        if ended_at(&e, item, "4")
            .iter()
            .any(|(t, why)| t.as_deref() == Some("done") || why.is_some())
        {
            break;
        }
        e.step(None).unwrap();
    }
    let log = said.borrow().join("\n");
    let at_4 = ended_at(&e, item, "4");
    assert!(at_4.iter().all(|(_, why)| why.is_none()), "{at_4:?}\n{log}");
    assert!(
        at_4.iter().any(|(t, _)| t.as_deref() == Some("done")),
        "{at_4:?}\n{log}"
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
        "2b",
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
        if ended_at(&e, item, "2b").iter().any(|(t, _)| t.is_some()) {
            break;
        }
        e.step(None).unwrap();
    }
    let log = said.borrow().join("\n");
    let rounds = ended_at(&e, item, "2b");
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
        "4",
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
    step_to(&mut e, item, "4");
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
        if ended_at(&e, item, "4").iter().any(|(_, why)| why.is_some()) {
            break;
        }
        e.step(None).unwrap();
    }
    let log = said.borrow().join("\n");
    let why = ended_at(&e, item, "4")
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
            tx.execute("UPDATE works SET at = '16' WHERE id = ?1", [work])?;
            let round = next_round(tx, work)?;
            tx.execute(
                "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES (?1, ?2, '16', '{}', 'c', 1)",
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
                 VALUES (?1, ?2, '7', '{}', ?3, 'next', 'c', 1, 2)",
                params![work, next_round(tx, work)?, output.to_string()],
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
            let n = next_round(tx, work)?;
            tx.execute(
                "INSERT INTO rounds (work, n, cell, session, input, config_commit, started_at) VALUES (?1, ?2, '1a', ?3, '{}', 'c', 1)",
                params![work, n, session],
            )?;
            Ok(n)
        })
        .unwrap();
    e.step(None).unwrap();
    let rounds = agent_rounds(&e, item);
    let (id, cell, input) = rounds.last().unwrap();
    assert_eq!(cell, "1a");
    let m = input["message"].as_str().unwrap();
    assert!(
        m.starts_with(&format!(
            "# {id} · cell 1a\n\nR{work}-{cut} was cut off midway. First look at the worktree"
        )),
        "{m}"
    );
    assert_eq!(input["conditional"], json!([]));
    assert!(input["call"]["session"]["resumed"].is_string(), "{input}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// When the flow moves to a new version, a work on the old one is not taken over when the engine starts: the owner
// still answers its asks. Once it goes on, it stops, saying why, and its item gets a new work at once: it takes over
// the branch, the worktree and the pull request, its cell 1 is told what exists, and what the owner answered stays
// the owner's answer. An item whose works ended can be queued again.
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
    // It put two asks to the owner.
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
        approved: None,
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
        format!("WHERE work = {old} ORDER BY n"),
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
    api::resume(&mut e.db, work).unwrap();
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
                 WHERE w.item = ?1 AND r.cell = '13' AND r.transition IS NOT NULL ORDER BY r.work, r.n LIMIT 1",
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
    api::resume(&mut e.db, id).unwrap();
    for _ in 0..3 {
        e.tick().unwrap();
    }
    let log = said.borrow().join("\n");
    let (_, at, paused) = state(&e, item).unwrap();
    assert_eq!(paused, None, "{log}");
    assert_ne!(at, "1", "{log}");
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
        assert!(m.starts_with(&format!("# {id} · cell {cell} · ")), "{m}");
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
    // The project guide is an input every agent's cell has: given in a session's first round, before anything else,
    // referred to after, and given again to the review's session.
    let guide = |input: &Value| -> Option<(String, bool)> {
        input["conditional"].as_array()?.iter().find_map(|m| {
            let a = m["address"].as_str()?;
            a.ends_with("·engine·project_guide")
                .then(|| (a.to_string(), m["given"].as_bool().unwrap_or(false)))
        })
    };
    let (address, sent) = guide(first).expect("the project guide in cell 1's round");
    assert!(sent, "{address}");
    let m = first["message"].as_str().unwrap();
    let at_guide = m.find(&format!("### {address}")).unwrap();
    assert!(at_guide < m.find("### format:common").unwrap(), "{m}");
    assert_eq!(guide(second).map(|g| g.1), Some(false));
    assert_eq!(guide(&at("5")).map(|g| g.1), Some(true));
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
    let design = &rounds.iter().rev().find(|r| r.1 == "2a").unwrap().0;
    let acceptance = format!("{design}·2a·acceptance");
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
    let understanding = format!("{}·1·understanding", rounds[0].0);
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
    assert_eq!(&cells[..3], ["1", "1a", "2"], "{log}");
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
    assert_eq!(&cells[..3], ["1", "1a", "2"], "{log}");
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
    assert_eq!(at, "1a", "{log}");
    let paused = paused.unwrap_or_else(|| panic!("the work did not pause:\n{log}"));
    assert!(
        paused.starts_with(&format!(
            "cell 1a: the message of R{work}-3 was not sent, since it would repeat what its session holds: \
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
                "SELECT count(*) FROM engine_notices WHERE work = ?1 AND text LIKE '%is paused: cell 1a: the message%'",
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
        m.starts_with(&format!("# {id} · cell 1\n\nYour return was sent back: "))
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
    api::resume(&mut e.db, work).unwrap();
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
    // The owner pauses the work on the page while the round of cell 1 runs.
    scripts.insert(
        "1",
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
    assert_eq!(at, "1a", "{log}");
    assert_eq!(paused.as_deref(), Some(db::PAUSED_BY_OWNER), "{log}");
    assert_eq!(paused_by(&e, work).as_deref(), Some("owner"));
    let took: Option<String> =
        e.db.conn()
            .query_row(
                "SELECT transition FROM rounds WHERE work = ?1 AND cell = '1'",
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

// The owner's Start all lets go on the works the owner paused, and leaves paused a work the engine paused for a
// problem, until the owner lets that one go on by itself.
#[test]
fn start_all_leaves_a_work_the_engine_paused() {
    let s = setup("start_all");
    let (mut e, said, item) = start(&s, scripts());
    let work = e.step(None).unwrap().work.unwrap();
    api::pause(&mut e.db, work, None).unwrap();
    let n = rounds_count(&e);
    assert_eq!(e.step(Some(work)).unwrap().work, None);
    assert_eq!(api::start_all(&mut e.db).unwrap(), "1 works paused by the owner go on");
    assert_eq!(e.step(Some(work)).unwrap().work, Some(work));
    assert_eq!(rounds_count(&e), n + 1);
    e.pause(work, "cell 1a: the return does not fit").unwrap();
    assert_eq!(paused_by(&e, work).as_deref(), Some("engine"));
    assert_eq!(api::start_all(&mut e.db).unwrap(), "0 works paused by the owner go on");
    let log = said.borrow().join("\n");
    assert_eq!(e.step(Some(work)).unwrap().work, None, "{log}");
    assert_eq!(
        state(&e, item).unwrap().2.as_deref(),
        Some("cell 1a: the return does not fit")
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
    e.pause(work, "cell 1a: the return does not fit").unwrap();
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
    e.pause(work, "cell 1: the return does not fit").unwrap();
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
    assert_eq!(cells, ["0"]);
    assert_eq!(paused_by(&e, new).as_deref(), Some("owner"));
    assert_eq!(e.step(Some(new)).unwrap().work, None);
    let _ = std::fs::remove_dir_all(&s.root);
}

// The architect's wakes.

/// The architect's sixth question, scripted: it picks the first matter of its queue, or finds nothing to do when the
/// queue is empty.
fn picks_first(s: &Setup, scripts: &mut BTreeMap<&'static str, Script>) {
    let path = config::database(&s.data);
    scripts.insert(
        "A6",
        Box::new(move |_: &Request, _: usize| {
            let c = db::open_read(&path).unwrap();
            let first: Option<(i64, String)> = c
                .query_row(
                    "SELECT matter, kind FROM architect_queue ORDER BY matter LIMIT 1",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .ok();
            match first {
                Some((m, kind)) => {
                    let kind: &'static str = Box::leak(kind.into_boxed_str());
                    (
                        vec![("outcome.pick", "picked"), ("outcome.pick.chosen.about", kind)],
                        vec![("outcome.pick.chosen.queued", json!(m))],
                    )
                }
                None => (vec![("outcome.pick", "nothing_to_do")], Vec::new()),
            }
        }),
    );
}

/// The architect's wakes, oldest first.
fn wakes(e: &Engine) -> Vec<i64> {
    let mut st =
        e.db.conn()
            .prepare("SELECT id FROM works WHERE graph = 'architect' ORDER BY id")
            .unwrap();
    st.query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect()
}

/// The architect's matters but the advance that keeps it quiet, in the order they came: each one's kind, whether it is
/// in the queue now, and whether it is handled.
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

/// Puts an item no work runs on, and in no queue.
fn an_item(e: &mut Engine) -> i64 {
    e.db.write(Actor::Owner, |tx| {
        tx.execute(
            "INSERT INTO items (project, title, problem, why, deliver, level)
             VALUES ('p', 'Parse it', 'No parser.', 'Needed.', 'change', 'l')",
            [],
        )?;
        Ok(tx.last_insert_rowid())
    })
    .unwrap()
}

// An engineer's question to the architect is a matter of its queue, which wakes it; the work waits at its question,
// holding no slot, until the architect replies with engine_op, and the reply takes the work on from there unchanged.
#[test]
fn an_engineer_s_question_wakes_the_architect_whose_reply_takes_the_work_on() {
    let s = setup("question");
    let mut agents = scripts();
    agents.insert(
        "1",
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
    picks_first(&s, &mut agents);
    let reply = json!({ "text": "Keep it one work.", "why": "Both changes share their tests." });
    let given = reply.clone();
    agents.insert(
        "A8",
        Box::new(move |_: &Request, n: usize| {
            if n == 1 {
                let tools = json!([{ "tool": "engine_op", "op": "reply", "item": 1, "reply": given.clone() }]);
                return (vec![("outcome", "tools")], vec![("outcome.tools", tools)]);
            }
            (Vec::new(), Vec::new())
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    let question = |e: &Engine| -> Option<(Option<String>, Option<i64>)> {
        e.db.conn()
            .query_row(
                "SELECT r.output, r.ended_at FROM rounds r JOIN works w ON w.id = r.work
                 WHERE w.item = ?1 AND r.cell = '15' ORDER BY r.n DESC LIMIT 1",
                [item],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .ok()
    };
    assert!(steps_until(&mut e, 10, |e| question(e).is_some()));
    let log = said.borrow().join("\n");
    assert_eq!(matters(&e), [("split".to_string(), true, false)], "{log}");
    let (what, of): (String, i64) =
        e.db.conn()
            .query_row(
                "SELECT what, item FROM architect_matters WHERE kind = 'split'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
    assert_eq!(of, item);
    assert!(what.contains("(split): The work holds two separate changes."), "{what}");
    // The work waits at its question holding no slot, and the architect wakes.
    assert_eq!(e.free_slots().unwrap(), 1, "{log}");
    let woke = e.step(None).unwrap();
    let log = said.borrow().join("\n");
    assert_eq!(woke.cell.as_deref(), Some("A0"), "{log}");
    let wake = woke.work.unwrap();
    assert_eq!(wakes(&e), [wake]);
    let replied = steps_until(&mut e, 80, |e| {
        question(e).is_some_and(|q| q.1.is_some()) && wake_done(e, wake)
    });
    let log = said.borrow().join("\n");
    assert!(replied, "{log}");
    let (output, _) = question(&e).unwrap();
    let output: Value = serde_json::from_str(&output.unwrap()).unwrap();
    assert_eq!(output, json!({ "reply": reply }));
    // The reply took the work from its question to choosing the way on.
    let (work, _, _) = state(&e, item).unwrap();
    let (took, next): (String, String) =
        e.db.conn()
            .query_row(
                "SELECT q.transition, r.cell FROM rounds q JOIN rounds r ON r.work = q.work AND r.n = q.n + 1
                 WHERE q.work = ?1 AND q.cell = '15'",
                [work],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
    assert_eq!((took.as_str(), next.as_str()), ("answered", "11"), "{log}");
    assert_eq!(matters(&e), [("split".to_string(), false, true)], "{log}");
    assert_eq!(wakes(&e), [wake], "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// Each side report is a matter of the architect's queue, queued once: an urgent one is due at once and wakes the
// architect, any other an hour after it came, when it wakes the architect again.
#[test]
fn side_reports_wake_the_architect_an_urgent_one_at_once_and_the_rest_an_hour_on() {
    let s = setup("side-reports");
    let mut agents = scripts();
    agents.insert(
        "1",
        Box::new(|_: &Request, n: usize| {
            let reports = if n == 1 {
                json!([
                    { "kind": "duplicate", "what": "Item 7 does the same.", "urgent": true },
                    { "kind": "machine_or_permission_later", "what": "It needs the timing machine later.", "urgent": false }
                ])
            } else {
                json!([])
            };
            (Vec::new(), vec![("side_reports", reports)])
        }),
    );
    picks_first(&s, &mut agents);
    let (mut e, said, _) = start(&s, agents);
    let work = e.step(None).unwrap().work;
    e.step(work).unwrap();
    let log = said.borrow().join("\n");
    assert_eq!(
        matters(&e),
        [
            ("duplicate".to_string(), true, false),
            ("other_question".to_string(), false, false)
        ],
        "{log}"
    );
    let due: Option<i64> =
        e.db.conn()
            .query_row(
                "SELECT due - at FROM architect_matters WHERE kind = 'other_question'",
                [],
                |r| r.get(0),
            )
            .unwrap();
    // An hour after it came, the time it came being read a moment after the hour was counted from.
    assert!(due.is_some_and(|d| (3_590_000..=3_600_000).contains(&d)), "{due:?}");
    let woke = e.step(work).unwrap();
    assert_eq!(woke.cell.as_deref(), Some("A0"), "{log}");
    let wake = woke.work.unwrap();
    assert!(
        steps_until(&mut e, 60, |e| wake_done(e, wake)),
        "{}",
        said.borrow().join("\n")
    );
    // Each report is queued once; the one not urgent wakes no one until its hour has passed.
    let log = said.borrow().join("\n");
    assert_eq!(
        matters(&e),
        [
            ("duplicate".to_string(), false, true),
            ("other_question".to_string(), false, false)
        ],
        "{log}"
    );
    e.step(None).unwrap();
    assert_eq!(wakes(&e), [wake]);
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "UPDATE architect_matters SET due = ?1 WHERE kind = 'other_question'",
            [db::now() - 1],
        )?;
        Ok(())
    })
    .unwrap();
    let again = e.step(None).unwrap();
    let log = said.borrow().join("\n");
    assert_eq!(again.cell.as_deref(), Some("A0"), "{log}");
    assert_eq!(wakes(&e).len(), 2, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The owner's words wake the architect; a next look it sets at its wrap-up comes to its queue when the wake ends, a
// look at a time once that time has come and a look at an item once its work ends, and the one due wakes it again.
#[test]
fn the_owner_s_words_wake_the_architect_and_a_next_look_wakes_it_again_when_due() {
    let s = setup("next-look");
    let mut agents = scripts();
    picks_first(&s, &mut agents);
    agents.insert(
        "A10",
        Box::new(|_: &Request, n: usize| {
            let looks = if n == 1 {
                json!([
                    { "when": { "kind": "at", "time": "2020-01-01T00:00Z" }, "what": "See whether the parser landed." },
                    { "when": { "kind": "item_done", "item": 1 }, "what": "Look at the item once its work ends." }
                ])
            } else {
                json!([])
            };
            (Vec::new(), vec![("outcome.next_look", looks)])
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    assert_eq!(e.step(None).unwrap().work, None);
    db::say_to_architect(&mut e.db, "Put the parser first. It blocks the rest.").unwrap();
    let woke = e.step(None).unwrap();
    let log = said.borrow().join("\n");
    assert_eq!(woke.cell.as_deref(), Some("A0"), "{log}");
    let wake = woke.work.unwrap();
    assert!(
        steps_until(&mut e, 40, |e| wake_done(e, wake)),
        "{}",
        said.borrow().join("\n")
    );
    let log = said.borrow().join("\n");
    assert_eq!(
        matters(&e),
        [
            ("owner_words".to_string(), false, true),
            ("next_look".to_string(), true, false),
            ("next_look".to_string(), false, false)
        ],
        "{log}"
    );
    let (due, due_item): (Option<i64>, Option<i64>) =
        e.db.conn()
            .query_row(
                "SELECT max(due), max(due_item) FROM architect_matters WHERE kind = 'next_look'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
    assert_eq!((due, due_item), (Some(1_577_836_800_000), Some(1)));
    let again = e.step(None).unwrap();
    let log = said.borrow().join("\n");
    assert_eq!(again.cell.as_deref(), Some("A0"), "{log}");
    assert_eq!(wakes(&e).len(), 2, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A design card a work hands in comes to the architect's queue, which wakes it; its ruling, made with engine_op, is
// kept beside the owner's, and a decision it disagrees with reaches the work's engineer as an interjection.
#[test]
fn a_design_card_wakes_the_architect_whose_ruling_is_kept_and_reaches_the_engineer() {
    let s = setup("architect-card");
    let mut agents = scripts();
    agents.insert(
        "4",
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
    picks_first(&s, &mut agents);
    let path = config::database(&s.data);
    agents.insert(
        "A8",
        Box::new(move |_: &Request, n: usize| {
            if n == 1 {
                let card: i64 = db::open_read(&path)
                    .unwrap()
                    .query_row("SELECT max(id) FROM design_cards", [], |r| r.get(0))
                    .unwrap();
                let verdicts = json!([{ "decision": 1, "verdict": "disagree", "why": "It does not fit the whole." }]);
                let tools = json!([{ "tool": "engine_op", "op": "rule_design_card", "design_card": card, "verdicts": verdicts }]);
                return (vec![("outcome", "tools")], vec![("outcome.tools", tools)]);
            }
            (Vec::new(), Vec::new())
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    let ruled = |e: &mut Engine| -> bool {
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM design_cards c, json_each(c.rulings) r
                 WHERE json_extract(r.value, '$.by') = 'architect'",
                [],
                |r| r.get::<_, i64>(0),
            )
            .unwrap()
            > 0
    };
    assert!(steps_until(&mut e, 150, ruled), "{}", said.borrow().join("\n"));
    let log = said.borrow().join("\n");
    let (card, status, rulings): (i64, String, String) =
        e.db.conn()
            .query_row("SELECT id, status, rulings FROM design_cards", [], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .unwrap();
    let rulings: Value = serde_json::from_str(&rulings).unwrap();
    assert_eq!(status, "open", "{log}");
    assert_eq!(rulings[0]["by"], "architect");
    assert_eq!(
        rulings[0]["decisions"],
        json!([{ "decision": 1, "verdict": "disagree", "text": "It does not fit the whole." }])
    );
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
    assert_eq!(
        said_to,
        format!("Ruling on design card {card}:\n- decision 1 (design/engine.md): disagree: It does not fit the whole.")
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// While fewer items are ready to start than there are slots, the engine queues an advance for the architect, which
// wakes it; once one is handled, no other comes for an hour.
#[test]
fn an_advance_wakes_the_architect_at_most_once_an_hour_while_too_few_items_are_ready() {
    let s = setup("advance");
    let mut agents = scripts();
    picks_first(&s, &mut agents);
    let (mut e, said) = engine(&s, agents);
    let woke = e.step(None).unwrap();
    let log = said.borrow().join("\n");
    assert_eq!(woke.cell.as_deref(), Some("A0"), "{log}");
    let wake = woke.work.unwrap();
    let body: String =
        e.db.conn()
            .query_row("SELECT body FROM architect_matters WHERE kind = 'advance'", [], |r| {
                r.get(0)
            })
            .unwrap();
    assert_eq!(body, r#"{"slots":1,"ready":0}"#);
    assert!(
        steps_until(&mut e, 40, |e| wake_done(e, wake)),
        "{}",
        said.borrow().join("\n")
    );
    assert_eq!(e.step(None).unwrap().work, None);
    let advances = |e: &Engine| -> i64 {
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM architect_matters WHERE kind = 'advance'",
                [],
                |r| r.get(0),
            )
            .unwrap()
    };
    assert_eq!(advances(&e), 1);
    // An hour after it was handled, another.
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "UPDATE rounds SET ended_at = ended_at - 7200000
             WHERE (work, n) IN (SELECT handled_work, handled_round FROM architect_matters WHERE kind = 'advance')",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    let again = e.step(None).unwrap();
    let log = said.borrow().join("\n");
    assert_eq!(again.cell.as_deref(), Some("A0"), "{log}");
    assert_eq!(advances(&e), 2);
    let _ = std::fs::remove_dir_all(&s.root);
}

// A dispatch hold stops new works from starting from the queue, and the works running go on; once it is released,
// new works start again.
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
                tx.execute("INSERT INTO dispatch_holds (reason, since) VALUES ('architect', 1)", [])?;
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

// With every work paused, the owner steps the architect: the first step starts a wake, paused by the owner, and runs
// its first round; each later step runs the wake's next round, as a step of the wake by its work id does, and the wake
// stays paused in the owner's hands. With nothing to wake for, a step runs nothing.
#[test]
fn the_owner_steps_the_architect_one_round_at_a_time_with_every_work_paused() {
    let s = setup("step-architect");
    let (mut e, said) = engine(&s, scripts());
    quiet(&mut e);
    assert_eq!(e.step_architect().unwrap().work, None);
    db::say_to_architect(&mut e.db, "Look at the queue.").unwrap();
    let first = e.step_architect().unwrap();
    let log = said.borrow().join("\n");
    assert_eq!((first.cell.as_deref(), first.rounds.len()), (Some("A0"), 1), "{log}");
    let wake = first.work.unwrap();
    assert_eq!(paused_by(&e, wake).as_deref(), Some("owner"));
    // Paused, the wake takes no step by itself, and no other wake starts.
    assert_eq!(e.step(None).unwrap().work, None);
    let second = e.step_work(wake).unwrap();
    let log = said.borrow().join("\n");
    assert_eq!((second.cell.as_deref(), second.rounds.len()), (Some("A1"), 1), "{log}");
    let third = e.step_architect().unwrap();
    let log = said.borrow().join("\n");
    assert_eq!(
        (third.work, third.cell.as_deref(), third.rounds.len()),
        (Some(wake), Some("A2"), 1),
        "{log}"
    );
    assert_eq!(paused_by(&e, wake).as_deref(), Some("owner"));
    assert_eq!(wakes(&e), [wake]);
    let _ = std::fs::remove_dir_all(&s.root);
}

// The changes the architect collects go to the owner in one ask at its wrap-up, holding the matter they belong to; the
// owner approves some, and the approval comes back as a matter of its own: db_write writes a change approved, and
// refuses one that was not, writing nothing.
#[test]
fn db_write_writes_the_changes_approved_and_refuses_the_rest() {
    let s = setup("db-write");
    let mut agents = scripts();
    picks_first(&s, &mut agents);
    let edit = json!({ "kind": "edit_item", "item": 1, "part": "title", "text": "Parse the input first", "reason": "Clearer." });
    let hold = json!({ "kind": "hold", "item": 1, "reason": "Not now." });
    let (e1, h1) = (edit.clone(), hold.clone());
    agents.insert(
        "A8",
        Box::new(move |_: &Request, n: usize| match n {
            1 => (Vec::new(), vec![("outcome.changes", json!([e1.clone(), h1.clone()]))]),
            2 => (
                vec![("outcome", "tools")],
                vec![(
                    "outcome.tools",
                    json!([{ "tool": "db_write", "changes": [h1.clone()] }]),
                )],
            ),
            3 => (
                vec![("outcome", "tools")],
                vec![(
                    "outcome.tools",
                    json!([{ "tool": "db_write", "changes": [e1.clone()] }]),
                )],
            ),
            _ => (Vec::new(), Vec::new()),
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    db::say_to_architect(&mut e.db, "Make the parser's item clearer.").unwrap();
    let wake = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 40, |e| wake_done(e, wake)),
        "{}",
        said.borrow().join("\n")
    );
    let log = said.borrow().join("\n");
    // The changes wait for the owner in one ask, and hold the owner's words they belong to.
    let (ask, body): (i64, String) =
        e.db.conn()
            .query_row("SELECT id, body FROM owner_asks WHERE work = ?1", [wake], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
    let body: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(body["kind"], "changes", "{log}");
    assert_eq!(body["changes"][0]["change"], edit);
    assert_eq!(body["changes"][1]["change"], hold);
    let held: (Option<String>, Option<i64>) =
        e.db.conn()
            .query_row(
                "SELECT held, held_by FROM architect_matters WHERE kind = 'owner_words'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
    assert_eq!(held, (Some("approval".to_string()), Some(ask)), "{log}");
    let approve = api::Answer {
        text: "The item is not held yet.".into(),
        why: None,
        reply: None,
        choice: Some("approve_some".into()),
        approved: Some(vec![1]),
    };
    api::answer(&mut e.db, ask, &approve).unwrap();
    let again = e.step(None).unwrap();
    assert_eq!(again.cell.as_deref(), Some("A0"));
    let second = again.work.unwrap();
    assert!(
        steps_until(&mut e, 40, |e| wake_done(e, second)),
        "{}",
        said.borrow().join("\n")
    );
    let log = said.borrow().join("\n");
    let results: Vec<String> = engine::query_json(
        e.db.conn(),
        "SELECT json_extract(result, '$.written.verdict') AS verdict FROM tool_runs WHERE tool = 'db_write' ORDER BY id",
        &[],
        10,
    )
    .unwrap()
    .iter()
    .map(|r| r["verdict"].as_str().unwrap_or_default().to_string())
    .collect();
    assert_eq!(results, ["refused", "written"], "{log}");
    let (title, held): (String, Option<String>) =
        e.db.conn()
            .query_row("SELECT title, held FROM items WHERE id = 1", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
    assert_eq!((title.as_str(), held), ("Parse the input first", None), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}
