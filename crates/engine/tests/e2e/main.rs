//! One item through the whole engineer graph, from the queue to main, and the architect's works beside it: a scripted
//! agent stands in for claude and a scripted GitHub for gh, over real git repositories in a temporary directory. Every
//! return the agent hands in is built from the cell's own schema, so a change to the flow that the engine does not
//! follow fails here.
//!
//! The tests are grouped by what they test, a module each: a new test goes in the module of what it tests, or in
//! a module of its own, with the helpers only it uses. What more than one module uses is here.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use db::Actor;
use db::rusqlite::params;
use engine::agent::{Agents, Call, Next, Reply, Request, Seen};
use engine::flow_cost::Effect;
use engine::hub::{CheckRun, Failure, GitHub};
use engine::{Advance, Ask, Engine, GlobalPause, Go, Hooks, Passed, Stepped};
use serde_json::{Value, json};

mod architect;
mod asks;
mod batches;
mod cancels;
mod decisions;
mod declared_checks;
mod deep_research;
mod dependency_sources;
mod dependency_sources_left_behind;
mod failures;
mod flow_versions;
mod guard;
mod history;
mod holds;
mod in_effect;
mod interjection_hook;
mod line_sessions;
mod lock_update;
mod lock_update_corners;
mod messages;
mod notebook;
mod observations;
mod open_questions;
mod owner;
mod policy;
mod project_decisions;
mod questions;
mod random;
mod reading_main;
mod retries;
mod returns;
mod review;
mod routes;
mod side_reports;
mod slots;
mod takeovers;
mod talk;
mod tests_changed;
mod threads;
mod tools;

/// The engine's loop as these tests take it: each call is one pass of it ([`Engine::pass`]), which dispatches what the
/// owner lets go on, each step on a thread of its own, and waits for every job to end, as `wf-agents run --once` does.
trait Passes {
    /// One pass while every work goes on: what its first step that moved came to. A work it leaves gathering the
    /// history (`C1`, `C2`, `G3`) has that round taken too, in passes of their own: these tests are about the cells
    /// either side of it, and those about gathering look at its rounds.
    fn step(&mut self, after: Option<i64>) -> Result<Stepped, String>;
    /// One pass while every work goes on, a gathering of the history taken with it as [`Passes::step`] takes it:
    /// whether a step moved or a work started.
    fn tick(&mut self) -> Result<bool, String>;
    /// The passes that take the rounds of the works left gathering the history.
    fn gather(&mut self) -> Result<(), String>;
    /// The owner's round of one work while every other work is paused.
    fn step_work(&mut self, work: i64) -> Result<Stepped, String>;
    /// The owner's round of a project's architect while every work is paused.
    fn step_architect(&mut self, project: &str) -> Result<Stepped, String>;
    /// One pass in which only `work` goes on, every other work held by the global pause: whether it moved, waits or
    /// stopped.
    fn advance(&mut self, work: i64) -> Result<Advance, String>;
}

/// The owner's global pause, every work held but those given and the works of the architects of the projects given.
fn held_but(runs: &[i64], architects: &[&str]) -> Option<GlobalPause> {
    Some(GlobalPause {
        warm: true,
        runs: runs.iter().copied().collect(),
        architects: architects.iter().map(|p| (*p).to_string()).collect(),
    })
}

/// What a pass came to, as one step: its first step that moved, with every round the pass started or ended.
fn first_moved(p: &Passed) -> Stepped {
    Stepped {
        work: p.work(),
        started_item: p.started_item(),
        rounds: p.rounds(),
        cell: p.cell(),
        asked: None,
    }
}

/// What the owner's round asked as `ask` came to in a pass: its step, or the round taken without one.
fn asked_in(p: &Passed, ask: &Ask) -> Stepped {
    p.jobs
        .iter()
        .find(|s| s.asked.as_ref() == Some(ask))
        .cloned()
        .unwrap_or_default()
}

impl Passes for Engine {
    fn step(&mut self, _after: Option<i64>) -> Result<Stepped, String> {
        let moved = first_moved(&self.pass(&Go::default())?);
        self.gather()?;
        Ok(moved)
    }

    fn tick(&mut self) -> Result<bool, String> {
        let p = self.pass(&Go::default())?;
        self.gather()?;
        Ok(!p.rounds().is_empty() || p.started_item().is_some())
    }

    fn gather(&mut self) -> Result<(), String> {
        for _ in 0..3 {
            let gathering: i64 = self
                .db
                .conn()
                .query_row(
                    "SELECT count(*) FROM works WHERE at IN ('C1', 'C2', 'G3') AND ended IS NULL AND paused IS NULL",
                    [],
                    |r| r.get(0),
                )
                .map_err(|e| e.to_string())?;
            if gathering == 0 {
                break;
            }
            self.pass(&Go::default())?;
        }
        Ok(())
    }

    fn step_work(&mut self, work: i64) -> Result<Stepped, String> {
        let go = Go {
            pause: held_but(&[], &[]),
            steps: vec![work],
            architects: Vec::new(),
        };
        Ok(asked_in(&self.pass(&go)?, &Ask::Work(work)))
    }

    fn step_architect(&mut self, project: &str) -> Result<Stepped, String> {
        let go = Go {
            pause: held_but(&[], &[]),
            steps: Vec::new(),
            architects: vec![project.to_string()],
        };
        Ok(asked_in(&self.pass(&go)?, &Ask::Architect(project.to_string())))
    }

    fn advance(&mut self, work: i64) -> Result<Advance, String> {
        let go = Go {
            pause: held_but(&[work], &[]),
            ..Go::default()
        };
        let p = self.pass(&go)?;
        let w = self.work(work)?;
        if w.ended.is_some() || w.paused.is_some() || self.done(&w)? {
            return Ok(Advance::Stopped);
        }
        if p.rounds().iter().any(|r| r.work == work) {
            return Ok(Advance::Moved);
        }
        Ok(Advance::Waits(format!("work {work} did not move")))
    }
}

/// The keep-warm pings due at `now`, as the loop dispatches them each as its work's job ([`Engine::keep_warm`]), sent
/// and recorded once every job has ended: how many were dispatched.
fn warm_now(e: &mut Engine, now: i64, pause: Option<&GlobalPause>) -> Result<usize, String> {
    let n = e.keep_warm(now, pause)?;
    e.drain()?;
    Ok(n)
}

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
/// `@usage`, what the CLI reports the round used; under `@stream`, what the run's stream tells on the way ([`seen`]),
/// each thing told to the round's watcher, which may stop the run there. The script `@ping` answers a keep-warm ping,
/// its `@usage` what the ping used.
type Script =
    Box<dyn Fn(&Request, usize) -> (Vec<(&'static str, &'static str)>, Vec<(&'static str, Value)>) + Send + Sync>;

struct Scripted {
    scripts: BTreeMap<&'static str, Script>,
    rounds: Mutex<BTreeMap<String, usize>>,
}

/// What one entry of a script's `@stream` tells: a call by `call` and `prompt`, with `output` and `subagent` when the
/// script says; a tool use by `tool` and `digest`; or a compaction.
fn seen(entry: &Value, model: &str) -> Seen {
    if let Some(id) = entry["call"].as_str() {
        return Seen::Call(Call {
            id: id.to_string(),
            model: Some(model.to_string()),
            read: entry["prompt"].as_i64().unwrap_or(0),
            output: entry["output"].as_i64().unwrap_or(0),
            subagent: entry["subagent"] == true,
            ..Call::default()
        });
    }
    match entry["tool"].as_str() {
        Some(name) => Seen::Tool {
            name: name.to_string(),
            on: entry["on"].as_str().map(str::to_string),
            digest: entry["digest"].as_u64().unwrap_or(0),
        },
        None => Seen::Compacted,
    }
}

impl Agents for Scripted {
    fn run(&self, r: &Request, watch: &dyn Fn(Seen) -> Next) -> Reply {
        if r.replay.is_some() {
            return self.ping(r);
        }
        if r.bare {
            return self.repair(r);
        }
        let n = {
            let mut rounds = self.rounds.lock().unwrap();
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
        // The understanding and the plan are handed in by their round, unless the script says otherwise.
        match r.cell.as_str() {
            "B1" => {
                set.entry("outcome.understanding".into())
                    .or_insert(json!(format!("The understanding at round {n}.")));
            }
            "B2" => {
                set.entry("outcome.notes".into())
                    .or_insert(json!(format!("The plan at round {n}.")));
            }
            _ => {}
        }
        // A fork of a session goes on as the new session it was named, as the CLI's fork does.
        let session = match &r.resume {
            Some(_) if r.fork => r.new_session.clone(),
            Some(s) => s.clone(),
            None => r.new_session.clone(),
        };
        let stream = set
            .remove("@stream")
            .and_then(|s| s.as_array().cloned())
            .unwrap_or_default();
        for entry in &stream {
            if let Next::Stop(why) = watch(seen(entry, &r.model)) {
                return Reply {
                    session,
                    usage: json!({ "started": true }),
                    error: Some(why.clone()),
                    stopped: Some(why),
                    ..Reply::default()
                };
            }
        }
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
        // Where the return was read from, as the command line tells it: `@from` is `tool` or `text`; with none the
        // reply tells none, as the replies of most tests do not.
        let from = match set.remove("@from").as_ref().and_then(Value::as_str) {
            Some("tool") => Some(engine::agent::Source::Tool),
            Some("text") => Some(engine::agent::Source::Text),
            _ => None,
        };
        // A field of another form than the return's, written as null beside the form: the field's name.
        let foreign_null = set.remove("@foreign_null");
        // Attempts the command line refused before it took the return, each by why.
        let refused: Vec<engine::agent::Refused> = set
            .remove("@refused")
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default()
            .into_iter()
            .enumerate()
            .map(|(i, why)| engine::agent::Refused {
                id: format!("toolu_{i}"),
                raw: "{}".into(),
                length: None,
                why: why.as_str().map(str::to_string),
            })
            .collect();
        let mut output = example(&r.schema, "", &pick, &set);
        if let Some(fault) = fault.as_ref().and_then(Value::as_str) {
            let raw = broken(&output, fault);
            let why = match fault {
                "outcome_text" | "outcome_text_brace" => {
                    "Output does not match required schema: /outcome: must be object"
                }
                "inside" => "Output does not match required schema: root: must have required property 'tell_owner'",
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
                refused: vec![engine::agent::Refused {
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
        if let Some(field) = foreign_null.as_ref().and_then(Value::as_str) {
            output["outcome"][field] = Value::Null;
        }
        Reply {
            output: Some(output),
            output_from: from,
            session,
            model: Some(r.model.clone()),
            usage,
            compactions,
            refused,
            ..Reply::default()
        }
    }

    fn version(&self, _cli: &config::Cli, _config_dir: Option<&Path>) -> String {
        "scripted".to_string()
    }
}

/// A return as an agent would have broken it writing it by hand: `inside`, with the fields every return holds inside
/// `outcome`; `drop_last_brace`, so and with its last brace left out; `unescaped_quote`, with a bare quote in its
/// progress; `cut_off`, half of it; `outcome_text`, with `outcome` a string holding its JSON; `outcome_text_brace`, so
/// and with a closing brace too many after that JSON, as a form whose JSON did not parse reaches the command line;
/// `unescaped_quote_outcome_text`, with a bare quote in its progress and `outcome` a string holding its JSON; `whole`,
/// as it is.
fn broken(output: &Value, fault: &str) -> String {
    let as_text = |after: &str| {
        let mut v = output.clone();
        v["outcome"] = json!(format!("{}{after}", v["outcome"]));
        v.to_string()
    };
    let inside = || {
        let mut v = output.clone();
        let o = v.as_object_mut().unwrap();
        let moved: Vec<(String, Value)> = ["tell_owner", "progress"]
            .iter()
            .filter_map(|k| o.shift_remove(*k).map(|x| (k.to_string(), x)))
            .collect();
        o["outcome"].as_object_mut().unwrap().extend(moved);
        v.to_string()
    };
    match fault {
        "outcome_text" => as_text(""),
        "outcome_text_brace" => as_text("}"),
        "unescaped_quote_outcome_text" => as_text("").replacen(" at ", " \"at\" ", 1),
        "inside" => inside(),
        "drop_last_brace" => {
            let t = inside();
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
            let mut rounds = self.rounds.lock().unwrap();
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
            let mut rounds = self.rounds.lock().unwrap();
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

/// How a pull request stands on the fake GitHub.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PrState {
    Open,
    Merged,
    Closed,
}

/// A pull request on the fake GitHub: the branch it is from, the branch it merges into, how it stands, and the commit it
/// merged as, once it has.
#[derive(Clone)]
struct Pr {
    branch: String,
    base: String,
    state: PrState,
    merged_as: Option<String>,
}

/// The pull requests of the fake GitHub, by number, which a test can read and change as GitHub keeps them.
type Prs = Arc<Mutex<BTreeMap<i64, Pr>>>;

/// A gate the scripted agent or GitHub waits at while a test keeps it shut, counting the calls that came to it; open
/// unless shut. A call waits a minute at most, so that a test whose engine never lets the gate open fails, not hangs.
#[derive(Clone, Default)]
struct Gate(Arc<(Mutex<GateState>, Condvar)>);

#[derive(Default)]
struct GateState {
    shut: bool,
    came: usize,
}

impl Gate {
    fn shut(&self) {
        self.0.0.lock().unwrap().shut = true;
    }

    fn open(&self) {
        self.0.0.lock().unwrap().shut = false;
        self.0.1.notify_all();
    }

    /// Comes to the gate, and waits while it is shut.
    fn pass(&self) {
        let (state, cv) = &*self.0;
        let mut s = state.lock().unwrap();
        s.came += 1;
        cv.notify_all();
        let (s, _) = cv.wait_timeout_while(s, Duration::from_secs(60), |s| s.shut).unwrap();
        drop(s);
    }

    /// Comes to the gate, and waits until `n` calls came to it, or ten seconds passed.
    fn meet(&self, n: usize) {
        let (state, cv) = &*self.0;
        let mut s = state.lock().unwrap();
        s.came += 1;
        cv.notify_all();
        let (s, _) = cv
            .wait_timeout_while(s, Duration::from_secs(10), |s| s.came < n)
            .unwrap();
        drop(s);
    }

    /// How many calls came to it.
    fn came(&self) -> usize {
        self.0.0.lock().unwrap().came
    }
}

/// A workflow the fake GitHub was asked to run by hand: the branch it was asked to run on, the commit that branch had on
/// the remote then (none if it had none), and the inputs given.
#[derive(Clone, Debug)]
struct ByHand {
    workflow: String,
    branch: String,
    commit: Option<String>,
    inputs: Vec<(String, String)>,
    /// How many times the run was waited for.
    waits: usize,
}

/// The runs by hand the fake GitHub was asked for, in order, which a test reads. A run whose `command` input is `red`
/// ends red, one whose `command` is `down` fails to start, as GitHub does when it cannot be reached, one whose `command`
/// is `reject` is refused as GitHub refuses a request that lacks an input the workflow requires, one whose `command` is
/// `timeout` does not finish in time, one whose `command` is `nostart` never gets a run, one whose `command` is `nojobs`
/// has a run with no job, one whose `command` is `flaky_dispatch` fails to reach GitHub the first time it is asked for,
/// and one whose `command` is `flaky` is run, but the first wait for it fails to reach GitHub. The input is read one line
/// at a time, as a workflow reads the tests the engine names, each followed by a line break.
type ByHands = Arc<Mutex<Vec<ByHand>>>;

/// Whether the inputs of a run by hand give `command` as `c`, on a line of its own.
fn commanded(inputs: &[(String, String)], c: &str) -> bool {
    inputs
        .iter()
        .any(|(k, v)| k == "command" && v.lines().any(|l| l.trim() == c))
}

/// GitHub as the engine sees it, over a bare repository: CI is green, once `ci` lets each check through, a pull
/// request keeps the base it was opened with until it is set, a merge, once `merging` lets it through, squashes the
/// branch onto the pull request's base and fails for a pull request that is not open, as `gh pr ready` does, and closing a
/// pull request that merged fails, as `gh pr close` does. A workflow
/// run by hand is recorded in `by_hand`, and its run has a log and an artifact `measurement`.
struct FakeHub {
    origin: PathBuf,
    scratch: PathBuf,
    prs: Prs,
    ci: Gate,
    merging: Gate,
    closing: Gate,
    by_hand: ByHands,
    watch_main: Arc<AtomicBool>,
}

impl FakeHub {
    /// The run by hand whose commit is the one given.
    fn run_by_hand(&self, commit: &str) -> Option<ByHand> {
        let runs = self.by_hand.lock().unwrap();
        runs.iter().find(|d| d.commit.as_deref() == Some(commit)).cloned()
    }

    /// The run by hand a check is of: its link ends with the run's commit.
    fn run_of(&self, check: &CheckRun) -> Option<ByHand> {
        let runs = self.by_hand.lock().unwrap();
        runs.iter()
            .find(|d| d.commit.as_deref().is_some_and(|c| check.run.ends_with(c)))
            .cloned()
    }

    /// What GitHub does to merge an open pull request: squashes its branch onto its base on the remote, marks it merged
    /// and keeps the commit it made. It is a function of the pull requests and the remote, not of the hub, so that a
    /// test can merge a pull request on GitHub without the engine's merge call, as when the answer of that call is
    /// lost. `gh pr ready`, the first thing the engine's real merge does, refuses a pull request that is not open,
    /// whether it merged or was closed, and so does this, with gh's own words.
    fn squash_merge(origin: &Path, scratch: &Path, prs: &Prs, repo: &str, pr: i64) -> Result<String, Failure> {
        let Pr {
            branch, base, state, ..
        } = prs
            .lock()
            .unwrap()
            .get(&pr)
            .cloned()
            .ok_or_else(|| Failure::other("no such PR"))?;
        if state != PrState::Open {
            return Err(Failure::network(format!(
                "gh pr ready {pr} -R {repo}: X Pull request {repo}#{pr} is closed. Only draft pull requests can be \
                 marked as \"ready for review\""
            )));
        }
        let dir = scratch.join(format!("merge-{pr}"));
        git(
            scratch,
            &[
                "clone",
                "-q",
                "-b",
                &base,
                &origin.to_string_lossy(),
                &dir.to_string_lossy(),
            ],
        );
        git(&dir, &["merge", "--squash", &format!("origin/{branch}")]);
        // A branch whose change main holds already, as when two works of the scripted agent make the same change,
        // squashes to nothing, and merges as an empty commit.
        git(
            &dir,
            &["commit", "-q", "--allow-empty", "-m", &format!("Merge PR {pr}")],
        );
        git(&dir, &["push", "-q", "origin", &format!("HEAD:{base}")]);
        let sha = git(&dir, &["rev-parse", "HEAD"]);
        if let Some(p) = prs.lock().unwrap().get_mut(&pr) {
            p.state = PrState::Merged;
            p.merged_as = Some(sha.clone());
        }
        Ok(sha)
    }
}

impl GitHub for FakeHub {
    fn pull_request(
        &self,
        _repo: &str,
        branch: &str,
        base: &str,
        _title_body: Option<(&str, &str)>,
        existing: Option<i64>,
    ) -> Result<i64, Failure> {
        if let Some(n) = existing {
            return Ok(n);
        }
        let mut prs = self.prs.lock().unwrap();
        if let Some((n, _)) = prs.iter().find(|(_, p)| p.branch == branch) {
            return Ok(*n);
        }
        let n = prs.last_key_value().map_or(1, |(n, _)| n + 1);
        prs.insert(
            n,
            Pr {
                branch: branch.to_string(),
                base: base.to_string(),
                state: PrState::Open,
                merged_as: None,
            },
        );
        Ok(n)
    }

    fn base(&self, _repo: &str, pr: i64) -> Result<String, Failure> {
        self.prs
            .lock()
            .unwrap()
            .get(&pr)
            .map(|p| p.base.clone())
            .ok_or_else(|| Failure::other("no such PR"))
    }

    fn set_base(&self, _repo: &str, pr: i64, base: &str) -> Result<(), Failure> {
        let mut prs = self.prs.lock().unwrap();
        let p = prs.get_mut(&pr).ok_or_else(|| Failure::other("no such PR"))?;
        p.base = base.to_string();
        Ok(())
    }

    fn checks(&self, _repo: &str, sha: &str, workflow: &str) -> Result<Vec<CheckRun>, Failure> {
        self.ci.pass();
        let command = |c: &str| self.run_by_hand(sha).is_some_and(|d| commanded(&d.inputs, c));
        if command("timeout") {
            return Err(Failure {
                kind: "timed_out",
                what: format!("the run of {workflow} on {sha} did not finish in 10800 s"),
            });
        }
        if command("nostart") {
            return Err(Failure {
                kind: "ci_did_not_start",
                what: format!("no run of {workflow} on {sha} after 600 s"),
            });
        }
        // A run that failed as it started, as GitHub does for a workflow file it cannot read, has no job at all.
        if command("nojobs") {
            return Ok(Vec::new());
        }
        if command("flaky") {
            let mut runs = self.by_hand.lock().unwrap();
            let waits = runs.iter_mut().find(|d| d.commit.as_deref() == Some(sha)).map(|d| {
                d.waits += 1;
                d.waits
            });
            if waits == Some(1) {
                return Err(Failure::network("gh run list: connection reset"));
            }
        }
        let red = command("red") || self.regression_red(sha);
        Ok(vec![CheckRun {
            name: "check".into(),
            result: if red { "red" } else { "green" }.into(),
            run: format!("https://example.invalid/{workflow}/{sha}"),
            log_tail: red.then(|| format!("test {REGRESSION} ... FAILED")),
            log_path: None,
            skipped: false,
        }])
    }

    fn dispatch(&self, _repo: &str, workflow: &str, branch: &str, inputs: &[(&str, &str)]) -> Result<(), Failure> {
        let on_remote = Command::new("git")
            .arg("--git-dir")
            .arg(&self.origin)
            .args(["rev-parse", "--verify", "-q", &format!("refs/heads/{branch}")])
            .output()
            .unwrap();
        let inputs: Vec<(String, String)> = inputs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        let down = commanded(&inputs, "down");
        let reject = commanded(&inputs, "reject");
        // A dispatch that fails to reach GitHub the first time it is made for a branch, and not the second.
        let flaky =
            commanded(&inputs, "flaky_dispatch") && !self.by_hand.lock().unwrap().iter().any(|d| d.branch == branch);
        self.by_hand.lock().unwrap().push(ByHand {
            workflow: workflow.to_string(),
            branch: branch.to_string(),
            commit: on_remote
                .status
                .success()
                .then(|| String::from_utf8_lossy(&on_remote.stdout).trim().to_string()),
            inputs,
            waits: 0,
        });
        if down || flaky {
            return Err(Failure::network("GitHub cannot be reached"));
        }
        if reject {
            return Err(Failure::rejected(format!(
                "gh workflow run {workflow}: could not create workflow dispatch event: HTTP 422: Required input \
                 'base' not provided"
            )));
        }
        Ok(())
    }

    fn job_log(&self, _repo: &str, check: &CheckRun) -> Result<String, Failure> {
        match self.run_of(check) {
            Some(_) => Ok(format!(
                "check\tRun\t2026-11-03T00:00:00.0000000Z the log of {}",
                check.run
            )),
            None => Err(Failure::other("no job logs here")),
        }
    }

    fn artifact(
        &self,
        _repo: &str,
        check: &CheckRun,
        name: &str,
    ) -> Result<Option<Vec<engine::hub::ArtifactFile>>, Failure> {
        // Every run reports its tests' results; its check's link ends with its commit.
        if name == RESULTS {
            let sha = check.run.rsplit('/').next().unwrap_or_default();
            return Ok(Some(vec![("junit.xml".to_string(), self.junit(sha).into_bytes())]));
        }
        match self.run_of(check) {
            Some(_) => Ok((name == "measurement").then(|| vec![("pairs.tsv".to_string(), b"1\t2\t3\n".to_vec())])),
            None => Err(Failure::other("no artifacts here")),
        }
    }

    fn rerun(&self, repo: &str, sha: &str, workflow: &str) -> Result<Vec<CheckRun>, Failure> {
        self.checks(repo, sha, workflow)
    }

    fn branch_head(&self, _repo: &str, branch: &str) -> Result<String, Failure> {
        if !self.watch_main.load(Ordering::SeqCst) {
            return Err(Failure::other("main is not watched in this test"));
        }
        Ok(git(&self.origin, &["rev-parse", branch]))
    }

    fn finished_checks(&self, repo: &str, sha: &str, workflow: &str) -> Result<Option<Vec<CheckRun>>, Failure> {
        if !self.watch_main.load(Ordering::SeqCst) {
            return Err(Failure::other("main is not watched in this test"));
        }
        self.checks(repo, sha, workflow).map(Some)
    }

    fn merge_commit(&self, _repo: &str, pr: i64) -> Result<Option<String>, Failure> {
        self.prs
            .lock()
            .unwrap()
            .get(&pr)
            .map(|p| p.merged_as.clone())
            .ok_or_else(|| Failure::other("no such PR"))
    }

    fn merge(&self, repo: &str, pr: i64) -> Result<String, Failure> {
        self.merging.pass();
        FakeHub::squash_merge(&self.origin, &self.scratch, &self.prs, repo, pr)
    }

    fn close(&self, _repo: &str, pr: i64, _comment: &str) -> Result<(), Failure> {
        self.closing.pass();
        let mut prs = self.prs.lock().unwrap();
        let p = prs.get_mut(&pr).ok_or_else(|| Failure::other("no such PR"))?;
        if p.state == PrState::Merged {
            return Err(Failure::other("a merged pull request cannot be closed"));
        }
        p.state = PrState::Closed;
        Ok(())
    }

    /// Says the command and the directory it ran in, where the real gh reads which repository it is about.
    fn gh(&self, dir: &Path, args: &[String]) -> Result<(i32, String), Failure> {
        Ok((0, format!("{} in {}", args.join(" "), dir.display())))
    }
}

/// The regression test a bug fix adds, as a file in the test paths that names it.
const REGRESSION: &str = "regression";

/// The artifact every run of the fake GitHub's CI reports its tests' results in, as the project's configuration names it.
const RESULTS: &str = "test-results";

impl FakeHub {
    /// The results a run of the commit reports, as JUnit: the feature's test, which passes, and the regression test,
    /// which fails where the commit's CI is red for it.
    fn junit(&self, sha: &str) -> String {
        let regression = if self.regression_red(sha) {
            "<failure message=\"the last line is dropped\"/>"
        } else {
            ""
        };
        format!(
            "<testsuites><testsuite name=\"e2e\"><testcase name=\"feature\" time=\"0.001\"/>\
             <testcase name=\"{REGRESSION}\" time=\"0.001\">{regression}</testcase></testsuite></testsuites>"
        )
    }

    /// Whether the commit holds the regression test and not the fix it tests, the word `fixed` in `src/lib.txt`: the
    /// CI of such a commit is red, as a regression test run on the code before its fix is. No other test makes either.
    fn regression_red(&self, sha: &str) -> bool {
        let shown = |path: &str| {
            std::process::Command::new("git")
                .arg("--git-dir")
                .arg(&self.origin)
                .args(["show", &format!("{sha}:{path}")])
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        };
        shown(&format!("tests/{REGRESSION}.txt")).is_some()
            && shown("src/lib.txt").is_none_or(|lib| !lib.contains("fixed"))
    }
}

struct Setup {
    root: PathBuf,
    data: PathBuf,
    origin: PathBuf,
    /// The pull requests on the fake GitHub, kept across the engines a test starts, as GitHub keeps them.
    prs: Prs,
    /// Where the fake GitHub's checks wait while a test keeps CI from coming back.
    ci: Gate,
    /// Where the fake GitHub's merges wait while a test keeps a merge from coming back; it counts the merge calls.
    merging: Gate,
    /// Where the fake GitHub's closing of a pull request waits while a test keeps it from coming back.
    closing: Gate,
    /// The workflows the fake GitHub was asked to run by hand.
    by_hand: ByHands,
    /// Whether the fake GitHub tells the engine's watch of main where main is and how its full CI ran there: only in a
    /// test of the watch, so that no other test's CI is asked more than it counts on.
    watch_main: Arc<AtomicBool>,
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
test_paths = ["tests/"]
rules = ["ground"]
[ci]
fast = "ci"
full = ["ci"]
merge_checks = ["check"]
results = {{ artifact = "{RESULTS}" }}
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
    Setup {
        root,
        data,
        origin,
        prs: Prs::default(),
        ci: Gate::default(),
        merging: Gate::default(),
        closing: Gate::default(),
        by_hand: ByHands::default(),
        watch_main: Arc::new(AtomicBool::new(false)),
    }
}

fn scripts() -> BTreeMap<&'static str, Script> {
    let mut s: BTreeMap<&'static str, Script> = BTreeMap::new();
    // The research finds the work can start as it is, with a plan and its acceptance.
    s.insert(
        "R1",
        Box::new(|_: &Request, _: usize| {
            (
                vec![("outcome.can_start", "yes"), ("outcome.departs", "no")],
                vec![
                    (
                        "outcome.can_start.acceptance",
                        json!([{ "by": "test", "id": "A1", "condition": "The feature is there." }]),
                    ),
                    ("outcome.points", json!([])),
                ],
            )
        }),
    );
    // A work that goes to deep research measures nothing, and its design keeps that acceptance.
    s.insert(
        "D1",
        Box::new(|_: &Request, _: usize| (vec![("outcome.measure", "not_measured")], Vec::new())),
    );
    s.insert(
        "D2",
        Box::new(|_: &Request, _: usize| {
            (
                Vec::new(),
                vec![(
                    "outcome.design.acceptance",
                    json!([{ "by": "test", "id": "A1", "condition": "The feature is there." }]),
                )],
            )
        }),
    );
    // Implementing: a work's first round changes the code and asks for ci; the next hands in the end. Which it is the
    // work's branch says, by the commit the first made, since several works may implement, each in its own worktree:
    // the feature, or, when main holds it already from another work, a part of its own.
    s.insert(
        "I1",
        Box::new(|r: &Request, _: usize| {
            let wt = &r.worktree;
            let done = git(wt, &["log", "--format=%s"]).lines().any(|l| l == "Add the feature");
            if !done {
                let lib = std::fs::read_to_string(wt.join("src/lib.txt")).unwrap_or_default();
                if lib.contains("feature") {
                    write(wt, &format!("src/part-{}.txt", r.work), "feature\n");
                } else {
                    write(wt, "src/lib.txt", "old\nfeature\n");
                }
                git(wt, &["add", "-A"]);
                git(wt, &["commit", "-q", "-m", "Add the feature"]);
                let head = git(wt, &["rev-parse", "HEAD"]);
                let tools = json!([{ "tool": "ci", "head": head, "tests": ["feature"], "pr": { "title": "Add the feature", "body": "It adds it." } }]);
                return (vec![("outcome", "tools")], vec![("outcome.tools", tools)]);
            }
            let head = git(wt, &["rev-parse", "HEAD"]);
            (
                vec![("outcome.departs", "no")],
                vec![
                    ("outcome.head", json!(head)),
                    ("outcome.conditions", json!([{ "how": "tested", "id": "A1", "tests": ["feature"] }])),
                ],
            )
        }),
    );
    // The review's details find nothing to fix, and nothing that can go wrong silently.
    s.insert(
        "T3",
        Box::new(|_: &Request, _: usize| (vec![("outcome.silent", "no"), ("outcome.must_fix", "no")], Vec::new())),
    );
    s
}

/// The default scripts with the research finding that it cannot settle the approach: deep research takes the work.
fn designing() -> BTreeMap<&'static str, Script> {
    let mut s = scripts();
    s.insert(
        "R1",
        Box::new(|_: &Request, _: usize| {
            (
                vec![("outcome.can_start", "no"), ("outcome.departs", "no")],
                vec![("outcome.points", json!([]))],
            )
        }),
    );
    s
}

/// The review's details finding nothing to fix and a change of the kinds that go wrong silently: adversarial testing
/// runs next.
fn adversarial() -> Script {
    Box::new(|_: &Request, _: usize| (vec![("outcome.silent", "yes"), ("outcome.must_fix", "no")], Vec::new()))
}

/// The default script of `cell`, setting besides what `extra` gives for its `n`th round, such as what the round used.
fn adding(cell: &str, extra: impl Fn(usize) -> Vec<(&'static str, Value)> + Send + Sync + 'static) -> Script {
    let base = scripts()
        .remove(cell)
        .unwrap_or_else(|| Box::new(|_: &Request, _: usize| (Vec::new(), Vec::new())));
    Box::new(move |r: &Request, n: usize| {
        let (pick, mut set) = base(r, n);
        set.extend(extra(n));
        (pick, set)
    })
}

/// Keeps project p's architect from starting a work for slots free while a test looks at something else: a matter of
/// slots free was handled just now, by an architect work over before the test, so the engine raises no other for an
/// hour.
fn quiet(e: &mut Engine) {
    let v = e.flow_now;
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO works (item, flow_version, graph, at, ended, project)
             VALUES (NULL, ?1, 'architect', 'R1', 'over before the test', 'p')",
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
fn start(s: &Setup, scripts: BTreeMap<&'static str, Script>) -> (Engine, Arc<Mutex<Vec<String>>>, i64) {
    start_with(s, scripts, Hooks::default())
}

/// The engine over a setup as [`start`] makes it, telling `hooks` what runs.
fn start_with(
    s: &Setup,
    scripts: BTreeMap<&'static str, Script>,
    hooks: Hooks,
) -> (Engine, Arc<Mutex<Vec<String>>>, i64) {
    let (mut e, said) = engine_with(s, scripts, hooks);
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
fn engine(s: &Setup, scripts: BTreeMap<&'static str, Script>) -> (Engine, Arc<Mutex<Vec<String>>>) {
    engine_with(s, scripts, Hooks::default())
}

/// The engine over a setup as it starts, telling `hooks` what runs, with what it says kept.
fn engine_with(s: &Setup, scripts: BTreeMap<&'static str, Script>, hooks: Hooks) -> (Engine, Arc<Mutex<Vec<String>>>) {
    let agents = Scripted {
        scripts,
        rounds: Mutex::new(BTreeMap::new()),
    };
    let hub = FakeHub {
        origin: s.origin.clone(),
        scratch: s.root.clone(),
        prs: s.prs.clone(),
        ci: s.ci.clone(),
        merging: s.merging.clone(),
        closing: s.closing.clone(),
        by_hand: s.by_hand.clone(),
        watch_main: s.watch_main.clone(),
    };
    let committed = config::committed(&s.data).unwrap();
    let said = Arc::new(Mutex::new(Vec::<String>::new()));
    let log = said.clone();
    let hooks = Hooks {
        say: Arc::new(move |x: &str| log.lock().unwrap().push(x.to_string())),
        ..hooks
    };
    let mut e = Engine::new(&s.data, committed, Box::new(agents), Box::new(hub), hooks).unwrap();
    e.retry_wait = std::time::Duration::ZERO;
    (e, said)
}

fn state(e: &Engine, item: i64) -> Option<(i64, String, Option<String>)> {
    e.db.conn()
        .query_row("SELECT id, at, paused FROM works WHERE item = ?1", [item], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .ok()
}

/// Makes, in a clone of the origin, the commit that writes the files given on top of main as it is now, as another pull
/// request merged meanwhile would, and gives the clone. The commit is not on main until it is pushed from there
/// ([`main_moves_on`]).
fn commit_on_main(root: &Path, files: &[(&str, &str)], message: &str) -> PathBuf {
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
    dir
}

/// Puts a commit on main that writes the files given, as another pull request merged meanwhile would.
fn main_moves_on(root: &Path, files: &[(&str, &str)], message: &str) {
    let dir = commit_on_main(root, files, message);
    git(&dir, &["push", "-q", "origin", "HEAD:main"]);
}

/// Names `CONSTITUTION.md` as project p's constitution in its configuration.
fn names_a_constitution(s: &Setup) {
    let project = std::fs::read_to_string(s.data.join("projects/p.toml")).unwrap();
    write(
        &s.data,
        "projects/p.toml",
        &project.replace(
            "rules = [\"ground\"]\n",
            "rules = [\"ground\"]\nconstitution = \"CONSTITUTION.md\"\n",
        ),
    );
    git(&s.data, &["commit", "-q", "-am", "a constitution"]);
}

/// Has project p's owner approve every change under `src/` before it merges.
fn owner_approves_src(s: &Setup) {
    let path = s.data.join("projects/p.toml");
    let text = std::fs::read_to_string(&path).unwrap();
    let text = text.replacen(
        "rules = [\"ground\"]\n",
        "rules = [\"ground\"]\nowner_approves = [\"src/\"]\n",
        1,
    );
    write(&s.data, "projects/p.toml", &text);
    git(&s.data, &["commit", "-q", "-am", "the owner approves src/"]);
}

/// The work's latest ask of the owner not answered yet.
fn open_ask(e: &Engine, work: i64) -> Option<(i64, Value)> {
    e.db.conn()
        .query_row(
            "SELECT id, body FROM owner_asks WHERE work = ?1 AND answer IS NULL ORDER BY id DESC LIMIT 1",
            [work],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)),
        )
        .ok()
        .map(|(id, body)| (id, serde_json::from_str(&body).unwrap()))
}

/// The owner's answer to an ask, in the owner's words.
fn owner_answers(e: &mut Engine, ask: i64, text: &str) {
    let answer = api::Answer {
        text: text.into(),
        speaker: "owner".into(),
        relayed: None,
        why: None,
        reply: None,
        to: None,
        images: Vec::new(),
    };
    api::answer(&mut e.db, ask, &answer).unwrap();
}

/// The latest matter that asks the architect to review the work's design.
fn review_matter(e: &Engine, work: i64) -> Option<i64> {
    e.db.conn()
        .query_row(
            "SELECT id FROM architect_matters WHERE work = ?1 AND kind = 'engineer_question' AND detail = 'design_review'
             ORDER BY id DESC LIMIT 1",
            [work],
            |r| r.get(0),
        )
        .ok()
}

/// The architect's reply to a matter, taken off the queue as if a work of the architect had it in hand.
fn architect_replies(e: &mut Engine, matter: i64, text: &str) {
    e.db.write(Actor::Architect, |tx| {
        tx.execute(
            "UPDATE architect_matters SET handled_work = (SELECT min(id) FROM works WHERE graph = 'architect'),
               handled_round = 1, reply = ?2 WHERE id = ?1",
            params![matter, json!({ "text": text, "why": "It fits the plan." }).to_string()],
        )?;
        Ok(())
    })
    .unwrap();
}

/// What a script hands in: the choices it makes and the fields it sets.
type Handed = (Vec<(&'static str, &'static str)>, Vec<(&'static str, Value)>);

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
    let rounds: Vec<String> =
        e.db.conn()
            .prepare(
                "SELECT r.cell || ' ' || coalesce(r.transition, coalesce(r.rejected, 'running')) FROM rounds r
             JOIN works w ON w.id = r.work WHERE w.item = ?1 ORDER BY r.work, r.n",
            )
            .unwrap()
            .query_map([item], |r| r.get(0))
            .unwrap()
            .map(|x| x.unwrap())
            .collect();
    panic!(
        "the work did not come to {cell}: {:?}, by {}",
        state(e, item),
        rounds.join("; ")
    );
}

fn worktree_of(e: &Engine, work: i64) -> PathBuf {
    let wt: String =
        e.db.conn()
            .query_row("SELECT worktree FROM works WHERE id = ?1", [work], |r| r.get(0))
            .unwrap();
    PathBuf::from(wt)
}

/// The transitions of a cell's rounds that ended, in order.
fn transitions(e: &Engine, item: i64, cell: &str) -> Vec<String> {
    ended_at(e, item, cell).into_iter().filter_map(|(t, _)| t).collect()
}

/// Steps the engine until `done` holds, or fails saying what the work did instead.
fn step_until(e: &mut Engine, item: i64, said: &Arc<Mutex<Vec<String>>>, what: &str, done: impl Fn(&Engine) -> bool) {
    for _ in 0..150 {
        if done(e) {
            return;
        }
        e.step(None).unwrap();
    }
    let rounds: Vec<String> =
        e.db.conn()
            .prepare(
                "SELECT r.cell || ' ' || coalesce(r.transition, coalesce(r.rejected, 'running')) FROM rounds r
                 JOIN works w ON w.id = r.work WHERE w.item = ?1 ORDER BY r.work, r.n",
            )
            .unwrap()
            .query_map([item], |r| r.get(0))
            .unwrap()
            .map(|x| x.unwrap())
            .collect();
    panic!(
        "{what} did not come about in 150 steps: {:?}, by {}\n{}",
        state(e, item),
        rounds.join("; "),
        said.lock().unwrap().join("\n")
    );
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

/// Puts works on a flow version before the one the engine runs: a copy of that one, its engineer's graph changed as
/// `change` says. Gives the version.
fn older(e: &mut Engine, works: &[i64], change: impl FnOnce(&mut flow::CompiledGraph)) -> i64 {
    older_graph(e, works, engine::ENGINEER, change)
}

/// Puts works on a flow version before the one the engine runs, as [`older`] does, its graph `graph` changed; each call
/// stores a version of its own.
fn older_graph(e: &mut Engine, works: &[i64], graph: &str, change: impl FnOnce(&mut flow::CompiledGraph)) -> i64 {
    let now = e.flow_now;
    let mut f = flow::Compiled::clone(&e.flow(now).unwrap());
    change(f.graphs.iter_mut().find(|g| g.name == graph).unwrap());
    let text = f.json();
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO flow_versions (digest, compiled)
             VALUES ('older ' || (SELECT coalesce(max(id), 0) + 1 FROM flow_versions), ?1)",
            [&text],
        )?;
        let v = tx.last_insert_rowid();
        for w in works {
            tx.execute("UPDATE works SET flow_version = ?2 WHERE id = ?1", params![w, v])?;
        }
        Ok(v)
    })
    .unwrap()
}

/// A change of a flow version at a cell that keeps it from being the same cell: its first row renamed.
fn renamed(g: &mut flow::CompiledGraph, cell: &str) {
    let c = g.cells.iter_mut().find(|c| c.id == cell).unwrap();
    c.rows[0].name = format!("{} before", c.rows[0].name);
}

/// The session a round of a work ran in.
fn session_of(e: &Engine, work: i64, cell: &str) -> i64 {
    e.db.conn()
        .query_row(
            "SELECT session FROM rounds WHERE work = ?1 AND cell = ?2 ORDER BY n DESC LIMIT 1",
            params![work, cell],
            |r| r.get(0),
        )
        .unwrap()
}

/// The rounds of a work at the cell that brings a work on an older flow version to the one the engine runs, `Y1`, each
/// with the row it took and its output.
fn brought(e: &Engine, work: i64) -> Vec<(String, Value)> {
    let mut st =
        e.db.conn()
            .prepare("SELECT transition, output FROM rounds WHERE work = ?1 AND cell = 'Y1' ORDER BY n")
            .unwrap();
    st.query_map([work], |r| {
        Ok((r.get::<_, Option<String>>(0)?, r.get::<_, Option<String>>(1)?))
    })
    .unwrap()
    .map(|x| {
        let (took, output) = x.unwrap();
        let output = output.and_then(|o| serde_json::from_str(&o).ok());
        (took.unwrap_or_default(), output.unwrap_or(Value::Null))
    })
    .collect()
}

/// What the engine told the owner of its own accord about a work, oldest first.
fn notices_of(e: &Engine, work: i64) -> Vec<String> {
    let mut st =
        e.db.conn()
            .prepare("SELECT text FROM engine_notices WHERE work = ?1 ORDER BY id")
            .unwrap();
    st.query_map([work], |r| r.get(0))
        .unwrap()
        .map(|x| x.unwrap())
        .collect()
}

/// The version a work is on and the one it moved from, if it moved.
fn versions_of(e: &Engine, work: i64) -> (i64, Option<i64>) {
    e.db.conn()
        .query_row(
            "SELECT flow_version, moved_from FROM works WHERE id = ?1",
            [work],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap()
}

/// A version before the one the engine runs in which cell I1 was another way, and what the version the engine runs
/// finds changed of it.
type Changed = (&'static str, fn(&mut flow::CompiledGraph));

/// The owner's word to the work's engineer, waiting to be delivered.
fn owner_says(e: &mut Engine, work: i64, text: &str) {
    e.db.write(Actor::Owner, |tx| {
        tx.execute(
            "INSERT INTO interjections (work, line, speaker, text) VALUES (?1, 'engineer', 'owner', ?2)",
            params![work, text],
        )?;
        Ok(())
    })
    .unwrap();
}

/// Each agent round of the item's works but the gathering of the background, oldest first: its name, its cell, and what
/// the engine recorded it gave.
fn agent_rounds(e: &Engine, item: i64) -> Vec<(String, String, Value)> {
    engine::query_json(
        e.db.conn(),
        "SELECT 'R' || r.work || '-' || r.n AS name, r.cell, r.input FROM rounds r JOIN works w ON w.id = r.work
         WHERE w.item = ?1 AND r.session IS NOT NULL AND r.cell <> 'R0' ORDER BY r.work, r.n",
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

/// The system prompt a round's command line carried, as its record keeps it; none for a round that carried none.
fn system_text(input: &Value) -> Option<String> {
    let command = input["call"]["command"].as_array()?;
    let at = command.iter().position(|a| a == "--append-system-prompt")?;
    command.get(at + 1)?.as_str().map(str::to_string)
}

/// Whether the round that started its session gave it a part of its system prompt, by the part's address.
fn in_system(input: &Value, address: &str) -> bool {
    input["system"]
        .as_array()
        .is_some_and(|xs| xs.iter().any(|x| x["address"] == address))
}

/// The project guide in a round's message: its address, and whether the message gave it.
fn guide_in(input: &Value) -> Option<(String, bool)> {
    input["conditional"].as_array()?.iter().find_map(|m| {
        let a = m["address"].as_str()?;
        a.ends_with("·engine·project_guide")
            .then(|| (a.to_string(), m["given"].as_bool().unwrap_or(false)))
    })
}

/// The engine over a setup whose cell `R0`, the work's first agent's, ends its first `times` runs without a return,
/// the CLI reporting `report`.
fn failing(name: &str, report: String, times: usize) -> (Setup, Engine, Arc<Mutex<Vec<String>>>, i64) {
    let s = setup(name);
    let mut scripts = scripts();
    scripts.insert(
        "R0",
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
        .query_row(
            "SELECT count(*) FROM rounds WHERE cell NOT IN ('C1', 'C2', 'G3')",
            [],
            |r| r.get(0),
        )
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

/// The rows of tool_runs of a tool in an item's work, in order: the attempt, the result and the files kept.
fn tool_runs(e: &Engine, item: i64, tool: &str) -> Vec<(i64, Value, Vec<String>)> {
    let mut st =
        e.db.conn()
            .prepare(
                "SELECT t.attempt, t.result, t.files FROM tool_runs t JOIN works w ON w.id = t.work
                 WHERE w.item = ?1 AND t.tool = ?2 ORDER BY t.id",
            )
            .unwrap();
    st.query_map(params![item, tool], |r| {
        Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
    })
    .unwrap()
    .map(|x| {
        let (attempt, result, files) = x.unwrap();
        (
            attempt,
            serde_json::from_str(&result).unwrap(),
            serde_json::from_str(&files).unwrap(),
        )
    })
    .collect()
}

/// Who paused a work, while it is paused.
fn paused_by(e: &Engine, work: i64) -> Option<String> {
    e.db.conn()
        .query_row("SELECT paused_by FROM works WHERE id = ?1", [work], |r| r.get(0))
        .unwrap()
}

// The architects' works.

/// The project an architect's round is for, by the directory it works in, `worktrees/architect/<project>`.
fn project_of(r: &Request) -> String {
    r.worktree
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The works of every architect, oldest first.
fn architect_works(e: &Engine) -> Vec<i64> {
    let mut st =
        e.db.conn()
            .prepare("SELECT id FROM works WHERE graph = 'architect' AND ended IS NULL ORDER BY id")
            .unwrap();
    st.query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect()
}

/// The architects' matters but the one that keeps them quiet and those at the head of the queue, the check of the policy
/// and the plan of what to do next, which a new session's understanding queues, in the order they came: each one's
/// kind, whether it is in a queue now, and whether it is handled.
fn matters(e: &Engine) -> Vec<(String, bool, bool)> {
    let mut st =
        e.db.conn()
            .prepare(
                "SELECT m.kind, m.id IN (SELECT matter FROM architect_queue), m.handled_round IS NOT NULL
                 FROM architect_matters m
                 WHERE m.what <> 'not yet' AND m.kind NOT IN ('check_policy', 'make_plan') ORDER BY m.id",
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
    input_at(e, work, cell, last)["message"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

/// The message a work's round at a cell was sent, the first or the last, as the round recorded it: its text and the
/// conditional inputs it gave or referred to.
fn input_at(e: &Engine, work: i64, cell: &str, last: bool) -> Value {
    let order = if last { "DESC" } else { "" };
    let input: String =
        e.db.conn()
            .query_row(
                &format!("SELECT input FROM rounds WHERE work = ?1 AND cell = ?2 ORDER BY n {order} LIMIT 1"),
                params![work, cell],
                |r| r.get(0),
            )
            .unwrap();
    db::put_back(e.db.conn(), &serde_json::from_str(&input).unwrap())
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

/// The owner says something to project p's architect in their one-to-one.
fn say(e: &mut Engine, text: &str) {
    db::say_to_architect(&mut e.db, "p", None, None, text, &[], db::Speaker::Owner).unwrap();
}

/// Moves everything the project's architect did and was told back in time by `ms`, as if it had happened that long
/// before: its rounds, and the matters it was given. What the cache of its session holds is as old as its last call.
fn age_architect(e: &mut Engine, ms: i64) {
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "UPDATE rounds SET started_at = started_at - ?1, ended_at = ended_at - ?1",
            [ms],
        )?;
        tx.execute("UPDATE architect_matters SET at = at - ?1", [ms])?;
        Ok(())
    })
    .unwrap();
}

/// The CLI sessions of the architect's line in two works, oldest first: why each started and its CLI session.
fn architect_sessions(e: &Engine, first: i64, second: i64) -> Vec<(String, String)> {
    let mut st = e
        .db
        .conn()
        .prepare("SELECT started, cli_session FROM sessions WHERE line = 'architect' AND work IN (?1, ?2) ORDER BY id")
        .unwrap();
    st.query_map([first, second], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(|x| x.unwrap())
        .collect()
}

/// Works of project p's architect run one after the other under `agents`: each of `talks` is the owner's words that
/// start a work, said `age` milliseconds after the work before it ended (that work and what it was told made that much
/// older; the first has none before it). The first words are said five seconds before the first work starts, so that
/// no round of it begins in the millisecond they were said in. With `rebuild`, a policy takes effect before each work after
/// the first, which reads the project anew. Gives the works' ids, oldest first.
fn architect_works_in(
    name: &str,
    agents: BTreeMap<&'static str, Script>,
    talks: &[(i64, &str)],
    rebuild: bool,
) -> (Setup, Engine, Arc<Mutex<Vec<String>>>, Vec<i64>) {
    let s = setup(name);
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    let mut works = Vec::new();
    for (i, (age, words)) in talks.iter().enumerate() {
        if i > 0 {
            age_architect(&mut e, *age);
            if rebuild {
                a_new_policy_takes_effect(&mut e);
            }
        }
        say(&mut e, words);
        if i == 0 {
            e.db.write(Actor::Engine, |tx| {
                tx.execute(
                    "UPDATE architect_matters SET at = at - 5000
                     WHERE id = (SELECT max(id) FROM architect_matters)",
                    [],
                )?;
                Ok(())
            })
            .unwrap();
        }
        let work = e.step(None).unwrap().work.unwrap();
        assert!(
            steps_until(&mut e, 60, |e| architect_work_done(e, work)),
            "{}",
            said.lock().unwrap().join("\n")
        );
        works.push(work);
    }
    (s, e, said, works)
}

// Capability reports: an agent says the system lacks what its work needs, a defect of the system and not of the agent.

/// A first round of the item's work that asks the architect about a capability gap, saying what, with no kind.
fn asks_about_a_gap(what: &'static str) -> Script {
    Box::new(move |_: &Request, n: usize| {
        if n > 1 {
            return (Vec::new(), Vec::new());
        }
        (
            vec![("outcome", "ask"), ("outcome.ask", "architect")],
            vec![
                ("outcome.ask.points[].what", json!(what)),
                ("outcome.ask.points[].tried", json!("Ran it: the shell found nothing.")),
            ],
        )
    })
}

/// How the latest round of a work at its question to the architect (`A2`) ended, by the row it took; none while open.
fn question_took(e: &Engine, work: i64) -> Option<String> {
    e.db.conn()
        .query_row(
            "SELECT transition FROM rounds WHERE work = ?1 AND cell = 'A2' ORDER BY n DESC LIMIT 1",
            [work],
            |r| r.get(0),
        )
        .ok()
        .flatten()
}

/// The lines the architect told a work's engineer, which need no answer (`informs`), in order.
fn lines_told(e: &Engine, work: i64) -> Vec<String> {
    engine::query_json(
        e.db.conn(),
        "SELECT text FROM interjections WHERE work = ?1 AND speaker = 'architect' AND informs = 1 ORDER BY id",
        &[&work],
        10,
    )
    .unwrap()
    .iter()
    .map(|r| r["text"].as_str().unwrap().to_string())
    .collect()
}

/// The engine of a test whose item's work asks the architect about a capability gap at its first round, with the
/// architect's ways (`H1`) and any other cell scripted as given, run until the work of the architect it started ended
/// and the work's question took its row: the engine, what it said, the item, its work and the architect's work.
fn a_gap_asked(
    s: &Setup,
    ask: Script,
    scripted: Vec<(&'static str, Script)>,
) -> (Engine, Arc<Mutex<Vec<String>>>, i64, i64, i64) {
    let mut agents = scripts();
    agents.insert("R1", ask);
    for (cell, script) in scripted {
        agents.insert(cell, script);
    }
    let (mut e, said, item) = start(s, agents);
    let asked = |e: &mut Engine| -> bool {
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM rounds r JOIN works w ON w.id = r.work WHERE w.item = ?1 AND r.cell = 'A2'",
                [item],
                |r| r.get::<_, i64>(0),
            )
            .unwrap()
            > 0
    };
    assert!(steps_until(&mut e, 10, asked), "{}", said.lock().unwrap().join("\n"));
    let work = state(&e, item).unwrap().0;
    let arch = e.step(None).unwrap().work.unwrap();
    // The work takes the reply, or the architect's pause holds it at its question until the owner starts it.
    let done = steps_until(&mut e, 80, |e| {
        architect_work_done(e, arch) && (question_took(e, work).is_some() || paused_by(e, work).is_some())
    });
    assert!(done, "{}", said.lock().unwrap().join("\n"));
    (e, said, item, work, arch)
}

/// What a script hands in: the branches it picks and the values it sets, by their paths.
type Hands = (Vec<(&'static str, &'static str)>, Vec<(&'static str, Value)>);

/// What a scripted architect's cell hands in to ask the owner a question.
fn asks_the_owner(question: &str) -> Hands {
    (
        vec![("outcome", "ask"), ("outcome.ask", "owner")],
        vec![("outcome.ask.question", json!(question))],
    )
}

/// Queues a message from project q's architect to project p's.
fn from_q(e: &mut Engine, text: &str) -> i64 {
    let m = db::Matter {
        project: "p".into(),
        kind: "other_architect".into(),
        detail: Some("message".into()),
        what: format!("The architect of q says: {text}"),
        body: json!({ "text": text }),
        ..db::Matter::default()
    };
    e.db.write(Actor::Engine, |tx| db::queue_matter(tx, &m)).unwrap()
}

/// The matters of project p's architect at the head of its queue, the check of the policy and the plan of what to do
/// next, in the order they came: each one's kind, whether it is handled, and each cell that asked for it with why.
type AtHead = (String, bool, Vec<(String, String)>);

fn at_head(e: &Engine) -> Vec<AtHead> {
    engine::query_json(
        e.db.conn(),
        "SELECT kind, handled_round IS NOT NULL AS handled, body FROM architect_matters
         WHERE project = 'p' AND kind IN ('check_policy', 'make_plan') ORDER BY id",
        &[],
        100,
    )
    .unwrap()
    .iter()
    .map(|r| {
        let asked = r["body"]["asked_by"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(|a| {
                (
                    a["cell"].as_str().unwrap_or_default().to_string(),
                    a["why"].as_str().unwrap_or_default().to_string(),
                )
            })
            .collect();
        (
            r["kind"].as_str().unwrap_or_default().to_string(),
            r["handled"] == json!(1),
            asked,
        )
    })
    .collect()
}

/// Takes the work's steps alone, every other work held, until `done` says so.
fn advance_until(e: &mut Engine, work: i64, done: impl Fn(&Engine) -> bool) {
    for _ in 0..80 {
        if done(e) {
            return;
        }
        e.advance(work).unwrap();
    }
}

/// The sessions of a work's line, oldest first: why each started, whether it ended, and the cells of its rounds.
/// A setup whose tier runs a model the session manager has prices for, by which it judges how many keep-warm pings a
/// wait is worth.
fn setup_priced(name: &str) -> Setup {
    let s = setup(name);
    let install = std::fs::read_to_string(s.data.join("install.toml"))
        .unwrap()
        .replace("model = \"m\"", "model = \"claude-sonnet-5-5\"");
    write(&s.data, "install.toml", &install);
    git(&s.data, &["commit", "-q", "-am", "a model with prices"]);
    s
}

/// A setup of an installation in Chinese whose titles may be 24 wide at most, as the owner sets it: 12 Chinese
/// characters, or 24 letters of English.
fn setup_titled(name: &str) -> Setup {
    let s = setup(name);
    let install = std::fs::read_to_string(s.data.join("install.toml"))
        .unwrap()
        .replace("language = \"English\"", "language = \"Chinese\"")
        .replace("title_max = 40", "title_max = 24");
    write(&s.data, "install.toml", &install);
    git(&s.data, &["commit", "-q", "-am", "titles 24 wide"]);
    s
}

/// A title of `n` Chinese characters, made from a number: the repository holds no letter of another language, not even
/// as an escape.
fn chinese(n: usize) -> String {
    char::from_u32(0x4e2d).unwrap().to_string().repeat(n)
}

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

/// A PNG and a JPEG the owner sends with words, as the page hands them in.
fn two_images() -> Vec<api::ImageSent> {
    let sent = |media_type: &str, bytes: &[u8]| api::ImageSent {
        media_type: media_type.into(),
        data: db::attachments::base64(bytes),
    };
    vec![
        sent("image/png", b"\x89PNG\r\n\x1a\n a screenshot"),
        sent("image/jpeg", b"\xff\xd8\xff\xe0 a photo"),
    ]
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

/// Each section of project p's architect's notebook as it stands, in the sections' order by name: the section, its
/// text and the cell of the round that wrote it.
fn notebook(e: &Engine) -> Vec<(String, String, String)> {
    let mut st =
        e.db.conn()
            .prepare(
                "SELECT n.section, n.text, r.cell FROM notebook n JOIN rounds r ON r.work = n.work AND r.n = n.round
                 WHERE n.project = 'p' ORDER BY n.section",
            )
            .unwrap();
    st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .map(|x| x.unwrap())
        .collect()
}

/// Why each return of a work's rounds at a cell was sent back, in order.
fn why_sent_back(e: &Engine, work: i64, cell: &str) -> Vec<String> {
    engine::query_json(
        e.db.conn(),
        "SELECT rejected FROM rounds WHERE work = ?1 AND cell = ?2 AND rejected IS NOT NULL ORDER BY n",
        &[&work, &cell],
        10,
    )
    .unwrap()
    .iter()
    .map(|r| r["rejected"].to_string())
    .collect()
}

/// The rows a work's rounds at a cell took, oldest first; a round that took none is left out.
fn rows_at(e: &Engine, work: i64, cell: &str) -> Vec<String> {
    cells_of(e, work)
        .into_iter()
        .filter(|c| c.0 == cell)
        .filter_map(|c| c.1)
        .collect()
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

// The owner's Start, Pause and Step of a work and of a project's architect, beside the global pause and start.

/// One turn of the engine's loop as `wf-agents run` takes it, which these tests play, every job waited for: the owner's
/// Start of a work taken over handed to the work that took it over, the new sessions the owner asked for handed to the
/// session manager, the pauses of the works the owner started lifted once each may hold a slot, then one pass with what
/// the owner asked, after which the rounds the owner asked for are done with. Gives what the round the owner asked for
/// came to, or else the first step that moved.
fn turn(e: &mut Engine, c: &api::Control) -> Stepped {
    let p = pass_turn(e, c);
    p.jobs
        .iter()
        .find(|s| s.asked.is_some())
        .cloned()
        .unwrap_or_else(|| first_moved(&p))
}

/// One turn of the loop, as [`turn`] takes it: what the whole pass came to.
fn pass_turn(e: &mut Engine, c: &api::Control) -> Passed {
    for (old, new) in e.replaced() {
        c.replaced(old, new);
    }
    for w in c.take_new_sessions() {
        e.sessions().ask_new(w);
    }
    let asked = c.asked();
    let (done, architects) = e
        .release_started(&asked.releasing, &asked.architects_releasing)
        .unwrap();
    c.released(&done, &architects);
    let asked = c.asked();
    let go = Go {
        pause: asked.paused.then(|| GlobalPause {
            warm: asked.keep_warm,
            runs: asked.started.clone(),
            architects: asked.architects_started.clone(),
        }),
        steps: asked.steps.clone(),
        architects: asked.architects.clone(),
    };
    let p = e.pass(&go).unwrap();
    for (old, new) in e.replaced() {
        c.replaced(old, new);
    }
    for s in &p.jobs {
        match &s.asked {
            Some(Ask::Work(w)) => c.stepped(*w),
            Some(Ask::Architect(project)) => c.stepped_architect(project),
            None => {}
        }
    }
    p
}

/// Takes turns of the loop and gives the works whose steps moved, in order: in each turn, in the order of their ids.
fn turns(e: &mut Engine, c: &api::Control, n: usize) -> Vec<i64> {
    let mut moved = Vec::new();
    for _ in 0..n {
        let p = pass_turn(e, c);
        moved.extend(p.jobs.iter().filter(|s| !s.rounds.is_empty()).filter_map(|s| s.work));
    }
    moved
}

/// A setup whose one machine has two slots, and an engine over it as [`start_with`] makes it, with two items in the
/// queue: the first and the second item.
fn two_works_with(
    name: &str,
    scripts: BTreeMap<&'static str, Script>,
    hooks: Hooks,
) -> (Setup, Engine, Arc<Mutex<Vec<String>>>, i64, i64) {
    let s = setup(name);
    let install = std::fs::read_to_string(s.data.join("install.toml")).unwrap();
    write(&s.data, "install.toml", &install.replace("slots = 1", "slots = 2"));
    git(&s.data, &["commit", "-q", "-am", "two slots"]);
    let (mut e, said, first) = start_with(&s, scripts, hooks);
    let second =
        e.db.write(Actor::Owner, |tx| {
            tx.execute(
                "INSERT INTO items (project, title, problem, why, deliver, level) VALUES ('p', 'Add another', 'x', 'y', 'change', 'l')",
                [],
            )?;
            let id = tx.last_insert_rowid();
            tx.execute("INSERT INTO dispatch_queue (item, position) VALUES (?1, 2.0)", params![id])?;
            Ok(id)
        })
        .unwrap();
    (s, e, said, first, second)
}

/// What a round hands in: its picks and what it sets.
type Picked = (Vec<(&'static str, &'static str)>, Vec<(&'static str, Value)>);

/// An ask of `to`, saying what it asks, with no kind.
fn an_ask(to: &'static str) -> Picked {
    (
        vec![("outcome", "ask"), ("outcome.ask", to)],
        vec![("outcome.ask.points[].what", json!(format!("A point for the {to}.")))],
    )
}

/// A text with what differs between two runs of the same step masked: the ids drawn at random, which have the form of
/// a UUID, and the times in milliseconds since the epoch, which have thirteen digits.
fn masked(text: &str) -> String {
    let uuid = |b: &[u8]| {
        b.len() >= 36
            && b[..36].iter().enumerate().all(|(i, c)| match i {
                8 | 13 | 18 | 23 => *c == b'-',
                _ => c.is_ascii_hexdigit(),
            })
    };
    let b = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < b.len() {
        if uuid(&b[i..]) {
            out.push_str("<id>");
            i += 36;
        } else if b[i].is_ascii_digit() {
            let n = b[i..].iter().take_while(|c| c.is_ascii_digit()).count();
            out.push_str(if n == 13 { "<ms>" } else { &text[i..i + n] });
            i += n;
        } else {
            let c = text[i..].chars().next().unwrap_or_default();
            out.push(c);
            i += c.len_utf8();
        }
    }
    out
}

/// Every round and every change in the database, one to a line, without the number and time of each change's
/// transaction or when each round started and ended, and masked as [`masked`] masks them.
fn written(e: &Engine) -> Vec<String> {
    let rows = |sql: &str| engine::query_json(e.db.conn(), sql, &[], usize::MAX).unwrap();
    let mut out = rows(
        "SELECT work, n, cell, session, input, output, transition, rejected, config_commit, model, usage, compactions
         FROM rounds ORDER BY work, n",
    );
    out.extend(rows(
        "SELECT id, actor, table_name, row_key, old_row, new_row FROM changes ORDER BY id",
    ));
    out.iter().map(|r| masked(&r.to_string())).collect()
}

// Works side by side: the loop dispatches each work's step on a thread of its own.

/// Takes turns of the engine's loop as `wf-agents run` takes them, waiting for no job: each reaps the jobs that ended
/// and dispatches what `go` lets go on, every work looked at afresh, until `done` holds or a minute passed; gives
/// whether it held.
fn turns_until(e: &mut Engine, go: &Go, done: impl Fn(&mut Engine) -> bool) -> bool {
    let until = Instant::now() + Duration::from_secs(60);
    while Instant::now() < until {
        e.reap().unwrap();
        if done(e) {
            return true;
        }
        e.rouse();
        e.dispatch(go).unwrap();
        std::thread::sleep(Duration::from_millis(10));
    }
    done(e)
}

/// How many rounds of a work there are, and how many of them ended after `since`.
fn rounds_after(e: &Engine, work: i64, since: i64) -> (i64, i64) {
    e.db.conn()
        .query_row(
            "SELECT count(*), count(*) FILTER (WHERE ended_at > ?2) FROM rounds WHERE work = ?1",
            params![work, since],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap()
}

/// When the round of a tool's cell `X1` a work has open started; none when it has none open.
fn tool_round_open(e: &Engine, work: i64) -> Option<i64> {
    e.db.conn()
        .query_row(
            "SELECT started_at FROM rounds WHERE work = ?1 AND cell = 'X1' AND ended_at IS NULL",
            [work],
            |r| r.get(0),
        )
        .ok()
}

/// Cancels an item, as the owner's command line or an architect's batch does. What came of it the tests read from what
/// stands afterwards, so that they say the same of the code before the requests as of the code with them.
fn cancel(e: &mut Engine, item: i64, by: &str) {
    let projects = e.config.config.projects.clone();
    let actor = if by == "owner" { Actor::Owner } else { Actor::Architect };
    e.db.write(actor, |tx| {
        engine::ending::cancel_item(tx, item, "no longer needed", by, &projects)
            .map(drop)
            .map_err(db::Error::Abort)
    })
    .unwrap();
}

/// The works that go on with a request to stop waiting in the outbox, as the owner's page shows them stopping.
fn stopping(e: &Engine) -> Vec<i64> {
    let mut st =
        e.db.conn()
            .prepare(
                "SELECT w.id FROM works w WHERE w.ended IS NULL AND EXISTS (SELECT 1 FROM outbox o
                     WHERE (o.kind = 'start_over' AND o.work = w.id)
                        OR (o.kind = 'cancel_item' AND json_extract(o.args, '$.item') = w.item))
                 ORDER BY w.id",
            )
            .unwrap();
    st.query_map([], |r| r.get(0)).unwrap().map(Result::unwrap).collect()
}

/// Why an item was cancelled; none while it is not.
fn cancelled_why(e: &Engine, item: i64) -> Option<String> {
    e.db.conn()
        .query_row("SELECT cancelled FROM items WHERE id = ?1", [item], |r| r.get(0))
        .unwrap()
}

/// Why a work ended; none while it goes on.
fn ended_why(e: &Engine, work: i64) -> Option<String> {
    e.db.conn()
        .query_row("SELECT ended FROM works WHERE id = ?1", [work], |r| r.get(0))
        .unwrap()
}

/// How many rounds of a work are open.
fn open_rounds(e: &Engine, work: i64) -> i64 {
    e.db.conn()
        .query_row(
            "SELECT count(*) FROM rounds WHERE work = ?1 AND ended_at IS NULL",
            [work],
            |r| r.get(0),
        )
        .unwrap()
}

/// The transition each of a work's rounds at a cell took, in their order; none for a round that took none.
fn transitions_at(e: &Engine, work: i64, cell: &str) -> Vec<Option<String>> {
    let mut st =
        e.db.conn()
            .prepare("SELECT transition FROM rounds WHERE work = ?1 AND cell = ?2 ORDER BY n")
            .unwrap();
    st.query_map(params![work, cell], |r| r.get(0))
        .unwrap()
        .map(|x| x.unwrap())
        .collect()
}

/// How many notices the owner has that say a word.
fn notices(e: &Engine, word: &str) -> i64 {
    e.db.conn()
        .query_row(
            "SELECT count(*) FROM engine_notices WHERE instr(text, ?1) > 0",
            [word],
            |r| r.get(0),
        )
        .unwrap()
}

/// Rounds of a cell as if they had run and ended, in a work of their own that is over, that a cell's lines are computed
/// from: for the round `i` of `n`, from 1, its tokens added, its calls, and its jump for `given` characters.
fn history(e: &mut Engine, cell: &str, n: i64, of: &dyn Fn(i64) -> (i64, i64, i64, i64)) {
    e.db.write(Actor::Engine, |tx| {
        let version: i64 = tx.query_row("SELECT max(id) FROM flow_versions", [], |r| r.get(0))?;
        tx.execute(
            "INSERT INTO works (flow_version, graph, at, ended) VALUES (?1, 'engineer', 'E1', 'history')",
            [version],
        )?;
        let work = tx.last_insert_rowid();
        for i in 1..=n {
            let (added, calls, jump, given) = of(i);
            tx.execute(
                "INSERT INTO rounds (work, n, cell, input, config_commit, started_at, ended_at)
                 VALUES (?1, ?2, ?3, '{}', 'c', 1, 2)",
                params![work, i, cell],
            )?;
            tx.execute(
                "INSERT INTO round_measures (work, round, graph, cell, calls, added, jump, given, repeats, compactions,
                                             ledger)
                 VALUES (?1, ?2, 'engineer', ?3, ?4, ?5, ?6, ?7, 0, 0, 1)",
                params![work, i, cell, calls, added, jump, given],
            )?;
        }
        Ok(())
    })
    .unwrap();
}

/// One number the database gives.
fn number(e: &Engine, sql: &str) -> i64 {
    e.db.conn().query_row(sql, [], |r| r.get(0)).unwrap()
}

/// How many times a work took the rows that count a counter, as the view `work_counters` says.
fn counted(e: &Engine, work: i64, counter: &str) -> i64 {
    e.db.conn()
        .query_row(
            "SELECT coalesce(sum(times), 0) FROM work_counters WHERE work = ?1 AND counter = ?2",
            params![work, counter],
            |r| r.get(0),
        )
        .unwrap()
}

/// The row each round of a work took at its cell, oldest first, as the view `round_rows` says.
fn rows_taken(e: &Engine, work: i64) -> Vec<(String, Option<String>)> {
    let mut st =
        e.db.conn()
            .prepare("SELECT cell, took FROM round_rows WHERE work = ?1 ORDER BY round")
            .unwrap();
    st.query_map([work], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(|x| x.unwrap())
        .collect()
}

/// Scripts that keep the request of every round of the cells named, before the script the cell had runs.
fn keep_requests(agents: &mut BTreeMap<&'static str, Script>, cells: &[&'static str]) -> Arc<Mutex<Vec<Request>>> {
    let seen = Arc::new(Mutex::new(Vec::<Request>::new()));
    for &cell in cells {
        let kept = seen.clone();
        let then = agents.remove(cell);
        agents.insert(
            cell,
            Box::new(move |r: &Request, n: usize| {
                kept.lock().unwrap().push(r.clone());
                then.as_ref().map(|f| f(r, n)).unwrap_or_default()
            }),
        );
    }
    seen
}

/// The sessions of a work, oldest first: each one's line, why it started, whether it forks another session (and the
/// CLI's id of that one), whether it is a matter's, and whether it ended.
fn sessions_of(e: &Engine, work: i64) -> Vec<(String, String, Option<String>, bool, bool)> {
    let mut st =
        e.db.conn()
            .prepare(
                "SELECT s.line, s.started, b.cli_session, s.matter IS NOT NULL, s.ended IS NOT NULL
             FROM sessions s LEFT JOIN sessions b ON b.id = s.fork_of WHERE s.work = ?1 ORDER BY s.id",
            )
            .unwrap();
    st.query_map([work], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))
        .unwrap()
        .map(|x| x.unwrap())
        .collect()
}

/// The CLI's id of the latest session of the base, the architect's own line, in a work.
fn base_session(e: &Engine, work: i64) -> String {
    e.db.conn()
        .query_row(
            "SELECT cli_session FROM sessions WHERE work = ?1 AND line = 'architect' ORDER BY id DESC LIMIT 1",
            [work],
            |r| r.get(0),
        )
        .unwrap()
}

/// Item changes as an agent drafts them on the items as the database at `path` has them now: each change that rewrites
/// an item's text or acceptance given the version it is drafted on, unless it names one: the item's latest change in
/// the log of changes, or 0 for an item the changes create, named by its key.
fn versioned(path: &Path, mut changes: Value) -> Value {
    let c = db::open_read(path).unwrap();
    for ch in changes.as_array_mut().into_iter().flatten() {
        let rewrites = matches!(ch["kind"].as_str(), Some("edit_item" | "edit_acceptance"));
        if !rewrites || ch.get("version").is_some() {
            continue;
        }
        // An item the same changes create, named by its key, has no version yet.
        let version: i64 = match ch["item"].as_str().and_then(|i| i.parse::<i64>().ok()) {
            Some(item) => c
                .query_row(
                    "SELECT coalesce(max(id), 0) FROM changes
                     WHERE table_name = 'items' AND row_key = json_object('id', ?1)",
                    [item],
                    |r| r.get(0),
                )
                .unwrap(),
            None => 0,
        };
        ch["version"] = json!(version);
    }
    changes
}

/// A check of project p's policy waits in its queue, as an answer to the owner's words queues one.
fn a_check_of_the_policy_is_queued(e: &mut Engine) {
    e.db.write(Actor::Engine, |tx| {
        let m = db::Matter {
            project: "p".into(),
            kind: "check_policy".into(),
            what: "Check whether the policy of p still holds: a test asks for it".into(),
            body: json!({ "asked_by": [] }),
            ..db::Matter::default()
        };
        db::queue_matter(tx, &m).map(|_| ())
    })
    .unwrap();
}

/// A policy takes effect in project p, as a work of its architect over before the test set it: a new base is due.
fn a_new_policy_takes_effect(e: &mut Engine) {
    let v = e.flow_now;
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO works (item, flow_version, graph, at, ended, project)
             VALUES (NULL, ?1, 'architect', 'R1', 'over before the test', 'p')",
            [v],
        )?;
        let arch = tx.last_insert_rowid();
        let policy = json!({ "outcome": { "policy": { "direction": "Small steps first." } } });
        record_round(tx, arch, 1, "P2", &policy, "written")?;
        record_round(tx, arch, 2, "P3", &json!({}), "next")?;
        Ok(())
    })
    .unwrap();
}
