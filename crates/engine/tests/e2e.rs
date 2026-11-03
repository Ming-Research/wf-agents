//! One item through the whole engineer graph, from the queue to main, and the architect's works beside it: a scripted
//! agent stands in for claude and a scripted GitHub for gh, over real git repositories in a temporary directory. Every
//! return the agent hands in is built from the cell's own schema, so a change to the flow that the engine does not
//! follow fails here.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicI64, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use db::Actor;
use db::rusqlite::params;
use engine::agent::{Agents, Call, Next, Reply, Request, Seen};
use engine::hub::{CheckRun, Failure, GitHub};
use engine::{Advance, Ask, Engine, GlobalPause, Go, Hooks, Passed, Stepped};
use serde_json::{Value, json};

/// The engine's loop as these tests take it: each call is one pass of it ([`Engine::pass`]), which dispatches what the
/// owner lets go on, each step on a thread of its own, and waits for every job to end, as `wf-agents run --once` does.
trait Passes {
    /// One pass while every work goes on: what its first step that moved came to. A work it leaves gathering the
    /// history (`G1`, `G2`) has that round taken too, in passes of their own: these tests are about the cells either
    /// side of it, and those about gathering look at its rounds.
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
                    "SELECT count(*) FROM works WHERE at IN ('G1', 'G2') AND ended IS NULL AND paused IS NULL",
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
        let mut pick: BTreeMap<String, String> =
            pick.into_iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        // The answer to the owner's words ends the talk, so that the architect waits for no more words, unless the
        // script says otherwise.
        if r.cell == "H8" {
            pick.entry("outcome.answer.talk".into())
                .or_insert_with(|| "over".into());
        }
        let mut set: BTreeMap<String, Value> = set.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
        // The understanding and the plan rewrite their sections of the notebook, unless the script says otherwise.
        let sections: &[&str] = match r.cell.as_str() {
            "U1" => &["project_now", "unclear"],
            "N1" => &["plan"],
            _ => &[],
        };
        if !sections.is_empty() {
            let edits: Vec<Value> = sections
                .iter()
                .map(|s| json!({ "section": s, "text": format!("The {s} at round {n}.") }))
                .collect();
            set.entry("outcome.notebook".into()).or_insert(json!(edits));
        }
        let session = r.resume.clone().unwrap_or_else(|| r.new_session.clone());
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
        Reply {
            output: Some(output),
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

/// A pull request on the fake GitHub: the branch it is from and the branch it merges into.
#[derive(Clone)]
struct Pr {
    branch: String,
    base: String,
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

/// GitHub as the engine sees it, over a bare repository: CI is green, once `ci` lets each check through, a pull
/// request keeps the base it was opened with until it is set, and a merge squashes the branch onto the pull request's
/// base.
struct FakeHub {
    origin: PathBuf,
    scratch: PathBuf,
    prs: Prs,
    ci: Gate,
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
        let Pr { branch, base } = self
            .prs
            .lock()
            .unwrap()
            .get(&pr)
            .cloned()
            .ok_or_else(|| Failure::other("no such PR"))?;
        let dir = self.scratch.join(format!("merge-{pr}"));
        git(
            &self.scratch,
            &[
                "clone",
                "-q",
                "-b",
                &base,
                &self.origin.to_string_lossy(),
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

/// The ids of the columns the review table lists in a prompt that gives it whole.
fn columns(prompt: &str) -> Vec<String> {
    columns_known(prompt, &mut Vec::new())
}

/// The ids of the columns of the review's table as a session that read its prompts knows them, `known` holding those
/// it knew before: the table given whole, under its address; what changed in it since, its lines removed and added; or
/// the table referred to, as given before.
fn columns_known(prompt: &str, known: &mut Vec<String>) -> Vec<String> {
    let Some(start) = prompt.find("## Review table") else {
        return Vec::new();
    };
    let body: Vec<&str> = prompt[start..]
        .lines()
        .skip(1)
        .skip_while(|l| l.trim().is_empty())
        .collect();
    if !body.first().is_some_and(|l| l.starts_with("### ")) {
        return known.clone();
    }
    let lines: Vec<&str> = body[1..]
        .iter()
        .skip_while(|l| l.trim().is_empty())
        .take_while(|l| !l.trim().is_empty())
        .copied()
        .collect();
    let id = |l: &str| l.split_once(". ").map(|(id, _)| id.to_string());
    if lines.first().is_some_and(|l| l.starts_with("What changed since")) {
        for l in &lines[1..] {
            if let Some(gone) = l.strip_prefix('-').and_then(id) {
                known.retain(|k| *k != gone);
            } else if let Some(added) = l.strip_prefix('+').and_then(id) {
                known.push(added);
            }
        }
        known.sort_by_key(|k| k.parse::<u32>().unwrap_or(u32::MAX));
        known.dedup();
    } else {
        *known = lines.iter().filter_map(|l| id(l)).collect();
    }
    known.clone()
}

struct Setup {
    root: PathBuf,
    data: PathBuf,
    origin: PathBuf,
    /// The pull requests on the fake GitHub, kept across the engines a test starts, as GitHub keeps them.
    prs: Prs,
    /// Where the fake GitHub's checks wait while a test keeps CI from coming back.
    ci: Gate,
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
    }
}

fn scripts() -> BTreeMap<&'static str, Script> {
    let mut s: BTreeMap<&'static str, Script> = BTreeMap::new();
    s.insert(
        "D2",
        Box::new(|_: &Request, _: usize| {
            (
                vec![("outcome.measure", "not_measured")],
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
        "D11",
        Box::new(|_: &Request, _: usize| (vec![("outcome.adversarial", "not_needed")], Vec::new())),
    );
    // The owner's words of a batch settle nothing to record, a row for each of the matters it was given.
    s.insert(
        "B5",
        Box::new(|r: &Request, _: usize| {
            let given = given_value(&r.prompt, "·engine·batch_words").unwrap_or(json!({}));
            let rows: Vec<Value> = given
                .as_object()
                .into_iter()
                .flat_map(|o| o.keys())
                .filter_map(|k| k.trim_start_matches('#').parse::<i64>().ok())
                .map(|m| json!({ "matter": m, "verdict": "nothing", "why": "Nothing to record." }))
                .collect();
            (Vec::new(), vec![("outcome.looked", json!(rows))])
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
                Vec::new(),
                vec![
                    ("outcome.head", json!(head)),
                    ("outcome.conditions", json!([{ "how": "tested", "id": "A1", "tests": ["feature"] }])),
                ],
            )
        }),
    );
    // The table review answers every column of its table.
    s.insert("T4", answers_columns(vec![("outcome.no_adversarial", "stands")]));
    s
}

/// A review that answers every column of its table as passing, which its session holds from its earlier prompts, with
/// the choices `also` beside the table.
fn answers_columns(also: Vec<(&'static str, &'static str)>) -> Script {
    let known: Mutex<BTreeMap<String, Vec<String>>> = Mutex::default();
    Box::new(move |r: &Request, _: usize| {
        let session = r.resume.clone().unwrap_or_else(|| r.new_session.clone());
        let ids = columns_known(&r.prompt, known.lock().unwrap().entry(session).or_default());
        let cols: Vec<Value> = ids
            .into_iter()
            .map(|id| json!({ "id": id, "verdict": "pass", "evidence": { "files": [], "ran": [], "nodes": [] } }))
            .collect();
        (also.clone(), vec![("outcome.columns", json!(cols))])
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
                said.lock().unwrap().join("\n")
            );
            if e.advance(*id).unwrap() == Advance::Stopped {
                done = true;
                break;
            }
        }
    }
    let log = said.lock().unwrap().join("\n");
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
    // The table review is given its columns under their address, which a later review of its session refers to or
    // is given what changed in, and the CI on the work as one object, whose entries a later review is given as they
    // change.
    let review = input_of("T4");
    let m = review["message"].as_str().unwrap_or_default();
    assert!(
        m.contains("## Review table: answer each column by its number\n\n### columns:T4\n\n1. "),
        "{m}"
    );
    let ci = given_value(m, "·engine·work_ci").expect(m);
    assert!(ci.is_object() && ci.get("fast_ci").is_some(), "{ci}");
    // The review line's session is given the worktree once, at its first round, the blind review.
    let blind = input_of("T3");
    let message = blind["message"].as_str().unwrap_or_default();
    assert!(
        message.contains("you only read it, and change and commit nothing there"),
        "{message}"
    );
    // The review is given what it reviews by commits the worktree has: main as fetched, the commit the branch was cut
    // from, the head under review and the command that reads the change between them.
    let base = given_value(message, "·engine·review_base").expect(message);
    let fork = base["fork_point"].as_str().unwrap_or_default();
    assert_eq!(base["main"], "origin/main", "{base}");
    assert!(
        fork.len() == 40 && base["head"].as_str().is_some_and(|h| h.len() == 40),
        "{base}"
    );
    assert_eq!(
        base["change"],
        format!("git diff {fork}..{}", base["head"].as_str().unwrap())
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
    let log = said.lock().unwrap().join("\n");
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
        rounds: Mutex::new(BTreeMap::new()),
    };
    let hub = FakeHub {
        origin: s.origin.clone(),
        scratch: s.root.clone(),
        prs: s.prs.clone(),
        ci: s.ci.clone(),
    };
    let committed = config::committed(&s.data).unwrap();
    let mut r = Engine::reading(&s.data, committed, Box::new(agents), Box::new(hub)).unwrap();
    let m = engine::next_message(&mut r, work).unwrap();
    assert!(m.text.contains("cell R2"), "{}", m.text);
    drop(r);
    assert_eq!(state_of(), before);
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
    let log = said.lock().unwrap().join("\n");
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

// The re-review after a fix is given the table the fix answered, as the fix was given it: a red check with its run and
// the end of its log, beside the fix's answer to it; before any fix it is given none.
#[test]
fn the_review_after_a_fix_is_given_the_table_the_fix_answered() {
    let s = setup("fix-table");
    let (mut e, _, item) = start(&s, scripts());
    for _ in 0..3 {
        e.step(None).unwrap();
    }
    let (work, _, _) = state(&e, item).unwrap();
    move_to(&mut e, work, "T4");
    let m = engine::next_message(&mut e, work).unwrap().text;
    assert_eq!(given_value(&m, "·engine·fix_table"), None, "{m}");
    let row = json!({ "row": "F1", "from": "T2·result[red]", "kind": "red", "name": "lint", "result": "red",
                      "run": "https://example.com/runs/7", "log_tail": "error: an unused import" });
    let input = json!({ "engine": { "table": [row] } });
    let output = json!({ "outcome": { "rows": [{ "answer": "fixed", "id": "F1", "commit": "abc" }], "head": "abc" },
                         "progress": "p", "tell_owner": [], "side_reports": [], "flow_feedback": "none" });
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO rounds (work, n, cell, input, output, transition, config_commit, started_at, ended_at)
             VALUES (?1, ?2, 'T6', ?3, ?4, 'done', 'c', 1, 2)",
            params![work, next_round(tx, work)?, input.to_string(), output.to_string()],
        )?;
        Ok(())
    })
    .unwrap();
    move_to(&mut e, work, "T4");
    let m = engine::next_message(&mut e, work).unwrap().text;
    let table = given_value(&m, "·engine·fix_table").expect(&m);
    assert_eq!(table, json!([row]), "{m}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The judgment whether a work may enter the merge queue reads the newest review, as the fix table does: a column the
// first review could not verify sends the work to the fix; the re-review that looks only at disputes is given that
// column again, and only it; and once it hands in no table, the first review's column no longer sends the work to a
// fix with nothing to answer, as in the sixth trial.
#[test]
fn the_merge_judgment_reads_the_newest_review_as_the_fix_does() {
    let s = setup("newest-review");
    let (mut e, said, item) = start(&s, scripts());
    for _ in 0..3 {
        e.step(None).unwrap();
    }
    let (work, _, _) = state(&e, item).unwrap();
    let add = |e: &mut Engine, cell: &str, output: &Value, row: &str, at: &str| {
        e.db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO rounds (work, n, cell, input, output, transition, config_commit, started_at, ended_at)
                 VALUES (?1, ?2, ?3, '{}', ?4, ?5, 'c', 1, 2)",
                params![work, next_round(tx, work)?, cell, output.to_string(), row],
            )?;
            tx.execute("UPDATE works SET at = ?2 WHERE id = ?1", params![work, at])?;
            Ok(())
        })
        .unwrap();
    };
    let took = |e: &Engine| -> String {
        e.db.conn()
            .query_row(
                "SELECT transition FROM rounds WHERE work = ?1 AND cell = 'M1' ORDER BY n DESC LIMIT 1",
                [work],
                |r| r.get(0),
            )
            .unwrap()
    };
    let first = returned(json!({ "columns": [
        { "id": "1", "verdict": "pass", "evidence": { "files": [], "ran": [], "nodes": [] } },
        { "id": "4", "verdict": "cannot_verify", "reason": "Nothing was measured." },
    ] }));
    add(&mut e, "T4", &first, "done", "M1");
    e.step_work(work).unwrap();
    assert_eq!(took(&e), "fix", "{}", said.lock().unwrap().join("\n"));
    let disputes = json!({ "plan": { "review": "disputes", "adversarial": "skipped" } });
    add(&mut e, "T7", &disputes, "review_first", "T4");
    let m = engine::next_message(&mut e, work).unwrap().text;
    assert_eq!(columns(&m), ["4"], "{m}");
    let again = returned(json!({ "previous": [{ "fid": "F1", "verdict": "withdraw" }] }));
    add(&mut e, "T4", &again, "done", "M1");
    e.step_work(work).unwrap();
    let log = said.lock().unwrap().join("\n");
    assert!(!took(&e).starts_with("fix"), "{}\n{log}", took(&e));
    let _ = std::fs::remove_dir_all(&s.root);
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

/// A table review that gives the last column of its table a finding, R1, and answers the others as passing; at each
/// later round it hands in only `later`, its answers to its earlier findings, one list for each round.
fn review_with_a_finding(later: Vec<Value>) -> Script {
    let known: Mutex<BTreeMap<String, Vec<String>>> = Mutex::default();
    Box::new(move |r: &Request, n: usize| {
        if n > 1 {
            return (Vec::new(), vec![("outcome.previous", later[n - 2].clone())]);
        }
        let session = r.resume.clone().unwrap_or_else(|| r.new_session.clone());
        let ids = columns_known(&r.prompt, known.lock().unwrap().entry(session).or_default());
        let evidence = json!({ "files": [], "ran": [], "nodes": [] });
        let cols: Vec<Value> = ids
            .iter()
            .enumerate()
            .map(|(i, id)| {
                if i + 1 < ids.len() {
                    json!({ "id": id, "verdict": "pass", "evidence": evidence })
                } else {
                    json!({ "id": id, "verdict": "findings", "evidence": evidence, "findings": [
                        { "fid": "R1", "where": "the pull request", "what": "It says too little.", "fix": "Say more." }
                    ] })
                }
            })
            .collect();
        (
            vec![("outcome.no_adversarial", "stands")],
            vec![("outcome.columns", json!(cols))],
        )
    })
}

/// A fix that answers its one row in each round with `rows`, one list for each round, and makes no commit: the head it
/// hands in is the one the work pushed already. The string `HEAD` in a row stands for that head.
fn fix_answering(rows: Vec<Value>) -> Script {
    Box::new(move |r: &Request, n: usize| {
        let head = git(&r.worktree, &["rev-parse", "HEAD"]);
        let rows = rows[n - 1].to_string().replace("HEAD", &head);
        (
            Vec::new(),
            vec![
                ("outcome.head", json!(head)),
                ("outcome.rows", serde_json::from_str(&rows).unwrap()),
            ],
        )
    })
}

// A fix that is in no commit, such as the pull request's description, answers the review's finding fixed_outside_commit,
// and the review then looks at that finding again, and only it. In the fourth trial a work whose description lacked two
// files turned from the merge judgment to the fix, which had no row to answer, to choosing what runs next, which
// chose nothing, ninety times and more: the answer left the change as it was, and nothing sent the review back.
#[test]
fn a_fix_in_no_commit_sends_the_review_back_to_the_finding_it_answered() {
    let s = setup("fix-no-commit");
    let mut agents = scripts();
    agents.insert(
        "T4",
        review_with_a_finding(vec![json!([{ "fid": "R1", "verdict": "closed" }])]),
    );
    let answered = "The pull request description now names the two test files; pushed with ci.";
    agents.insert(
        "T6",
        Box::new(move |r: &Request, n: usize| {
            let head = git(&r.worktree, &["rev-parse", "HEAD"]);
            if n == 1 {
                // The description is corrected with ci and no tests: there is nothing to run.
                let pr = json!({ "title": "Add the feature", "body": "It adds the feature and its test." });
                let tools = json!([{ "tool": "ci", "head": head, "pr": pr }]);
                return (vec![("outcome", "tools")], vec![("outcome.tools", tools)]);
            }
            let row = json!([{ "answer": "fixed_outside_commit", "id": "F1", "where": answered }]);
            (Vec::new(), vec![("outcome.head", json!(head)), ("outcome.rows", row)])
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    step_until(&mut e, item, &said, "the review clearing the work", |e| {
        transitions(e, item, "M1").iter().any(|t| t == "clear")
    });
    let (work, _, _) = state(&e, item).unwrap();
    let m1 = transitions(&e, item, "M1");
    assert_eq!(m1.iter().filter(|t| *t == "fix").count(), 1, "{m1:?}");
    let t6 = transitions(&e, item, "T6");
    assert!(!t6.iter().any(|t| t == "nothing_to_answer"), "{t6:?}");
    assert_eq!(transitions(&e, item, "T4"), ["done", "done"]);
    let plans: Vec<String> = e
        .db
        .conn()
        .prepare("SELECT json_extract(output, '$.plan.review') FROM rounds WHERE work = ?1 AND cell = 'T7' ORDER BY n")
        .unwrap()
        .query_map([work], |r| r.get(0))
        .unwrap()
        .map(|x| x.unwrap())
        .collect();
    assert!(plans.iter().any(|p| p == "disputes"), "{plans:?}");
    // The re-review is given the row and where the fix is.
    let m = message_at(&e, work, "T4", true);
    assert!(m.contains("fixed_outside_commit") && m.contains(answered), "{m}");
    let _ = std::fs::remove_dir_all(&s.root);
}

/// A work whose review raises one finding, R1, run through the review's later answers and the fix's rows (see
/// [`review_with_a_finding`] and [`fix_answering`]) until the review clears it. When the second round of the fix has
/// answered, `second` replaces the rows it handed in, as a fix that asked the advisor would have answered them. The
/// review looks at the finding again after every fix, and no round of the fix has an empty table.
fn finding_is_cleared(name: &str, review: Vec<Value>, fix: Vec<Value>, second: Option<Value>) {
    let s = setup(name);
    let mut agents = scripts();
    let fixes = review.len();
    agents.insert("T4", review_with_a_finding(review));
    agents.insert("T6", fix_answering(fix));
    let (mut e, said, item) = start(&s, agents);
    let done =
        |e: &Engine, cell: &str, n: usize| transitions(e, item, cell).iter().filter(|t| *t == "done").count() == n;
    if let Some(rows) = second {
        step_until(&mut e, item, &said, "the second fix answering", |e| done(e, "T6", 2));
        let (work, _, _) = state(&e, item).unwrap();
        e.db.write(Actor::Engine, |tx| {
            tx.execute(
                "UPDATE rounds SET output = json_set(output, '$.outcome.rows', json(?2))
                 WHERE work = ?1 AND cell = 'T6' AND n = (SELECT max(n) FROM rounds WHERE work = ?1 AND cell = 'T6'
                                                          AND transition = 'done')",
                params![work, rows.to_string()],
            )?;
            Ok(())
        })
        .unwrap();
    }
    step_until(&mut e, item, &said, "the review clearing the finding", |e| {
        transitions(e, item, "M1").iter().any(|t| t == "clear")
    });
    let t6 = transitions(&e, item, "T6");
    assert!(!t6.iter().any(|t| t == "nothing_to_answer"), "{t6:?}");
    assert_eq!(transitions(&e, item, "T4").len(), fixes + 1);
    let m1 = transitions(&e, item, "M1");
    assert_eq!(m1.iter().filter(|t| *t == "fix").count(), fixes, "{m1:?}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The review insisted on a finding, the engineer asked the advisor and answered advisor_ruled, which changes nothing in
// the code: the review looks at the finding again, withdraws it by the ruling, and the work goes on.
#[test]
fn a_ruling_that_changes_no_code_sends_the_review_back_to_the_finding() {
    finding_is_cleared(
        "ruled-no-code",
        vec![
            json!([{ "fid": "R1", "verdict": "insist", "reason": "It is still wrong.", "basis": { "kind": "judgment" } }]),
            json!([{ "fid": "R1", "verdict": "withdraw" }]),
        ],
        vec![
            json!([{ "answer": "disagree", "id": "F1", "reason": "It is right as it is.", "basis": { "kind": "judgment" } }]),
            json!([{ "answer": "cannot_verify", "id": "F1", "reason": "Replaced below." }]),
        ],
        Some(
            json!([{ "answer": "advisor_ruled", "id": "F1", "ruling": "The advisor ruled that no change is needed." }]),
        ),
    );
}

// The review said the finding was not fixed, and the engineer answers fixed again with the head it had, no new commit:
// the review looks at it again, not the merge judgment, the fix and the choice of what runs next in turn.
#[test]
fn an_answer_fixed_again_with_no_new_commit_sends_the_review_back_to_the_finding() {
    finding_is_cleared(
        "fixed-again-no-commit",
        vec![
            json!([{ "fid": "R1", "verdict": "not_fixed", "reason": "It is not." }]),
            json!([{ "fid": "R1", "verdict": "closed" }]),
        ],
        vec![
            json!([{ "answer": "fixed_outside_commit", "id": "F1", "where": "The pull request description." }]),
            json!([{ "answer": "fixed", "id": "F1", "commit": "HEAD" }]),
        ],
        None,
    );
}

// Whatever the fix answers to a row a side raised, and the code does not change, that side looks at it again: every
// answer the flow gives is tried, so an answer added later cannot be left out. The review looks at the rows it raised and
// adversarial testing, which has run in this work, stays skipped; adversarial testing looks at the rows it raised, and
// the review goes with it as it does when adversarial testing runs; a row another cell raised, as a red check of the full
// CI, sends neither anywhere.
#[test]
fn every_answer_to_a_row_a_side_raised_makes_that_side_look_again() {
    let s = setup("every-answer");
    let mut agents = scripts();
    agents.insert("T4", review_with_a_finding(Vec::new()));
    agents.insert(
        "T6",
        fix_answering(vec![
            json!([{ "answer": "cannot_verify", "id": "F1", "reason": "Replaced below." }]),
        ]),
    );
    let (mut e, said, item) = start(&s, agents);
    step_until(&mut e, item, &said, "the fix taking the finding", |e| {
        state(e, item).is_some_and(|(_, at, _)| at == "T6")
    });
    let (work, _, _) = state(&e, item).unwrap();
    // Adversarial testing has run on the change the review saw: a round of it, before the fix.
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO rounds (work, n, cell, input, output, transition, config_commit, started_at, ended_at)
             SELECT ?1, (SELECT max(n) + 1 FROM rounds WHERE work = ?1), 'T1',
                    json_object('engine', json(json_extract(input, '$.engine'))), '{\"outcome\":{}}', 'done', 'c', 1, 2
             FROM rounds WHERE work = ?1 AND cell = 'T4' ORDER BY n DESC LIMIT 1",
            [work],
        )?;
        Ok(())
    })
    .unwrap();
    step_until(&mut e, item, &said, "the fix answering", |e| {
        transitions(e, item, "T6").iter().any(|t| t == "done")
    });
    let plan = |e: &mut Engine, rows: Value| -> (String, String) {
        e.db.write(Actor::Engine, |tx| {
            tx.execute(
                "UPDATE rounds SET output = json_set(output, '$.outcome.rows', json(?2)) WHERE work = ?1 AND cell = 'T6'",
                params![work, rows.to_string()],
            )?;
            Ok(())
        })
        .unwrap();
        move_to(e, work, "T7");
        e.step_work(work).unwrap();
        e.db.conn()
            .query_row(
                "SELECT json_extract(output, '$.plan.review'), json_extract(output, '$.plan.adversarial') FROM rounds
                 WHERE work = ?1 AND cell = 'T7' ORDER BY n DESC LIMIT 1",
                [work],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap()
    };
    let answers = flow::answers("T6", "rows").unwrap();
    assert!(answers.iter().any(|a| a == "fixed_outside_commit"), "{answers:?}");
    let raised_by = |e: &mut Engine, cell: &str| {
        e.db.write(Actor::Engine, |tx| {
            tx.execute(
                "UPDATE rounds SET input = json_set(input, '$.engine.table[0].from', ?2)
                 WHERE work = ?1 AND cell = 'T6'",
                params![work, cell],
            )?;
            Ok(())
        })
        .unwrap();
    };
    let expect = |e: &mut Engine, review: &str, adversarial: &str| {
        for answer in &answers {
            let got = plan(e, json!([{ "answer": answer, "id": "F1" }]));
            assert_eq!(got, (review.to_string(), adversarial.to_string()), "{answer}");
        }
    };
    // The row is the review's finding.
    expect(&mut e, "disputes", "skipped");
    // The row is a test adversarial testing wrote that went wrong.
    raised_by(&mut e, "T1·new_tests");
    expect(&mut e, "changed", "disputes");
    // The row is a red check of the full CI.
    raised_by(&mut e, "T2·result[red]");
    expect(&mut e, "none", "skipped");
    // Adversarial testing, sent back to a row it raised, is given the fix's table with that row and the answer to it,
    // whatever the answer was: the row's number alone would say nothing of which test it is.
    raised_by(&mut e, "T1·new_tests");
    let where_it_is = "Only in the pull request description, pushed with ci.";
    let fixed = json!([{ "answer": "fixed_outside_commit", "id": "F1", "where": where_it_is }]);
    assert_eq!(plan(&mut e, fixed).1, "disputes");
    e.step_work(work).unwrap();
    let m = message_at(&e, work, "T1", true);
    assert!(
        m.contains("It says too little.") && m.contains("fixed_outside_commit") && m.contains(where_it_is),
        "{m}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

/// Records a round of `cell` that was given `input` and handed in `output`, and leaves the work at `at`, where the row
/// `transition` of the cell leads.
fn seed_round(e: &mut Engine, work: i64, cell: &str, input: &Value, output: &Value, transition: &str, at: &str) {
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO rounds (work, n, cell, input, output, transition, config_commit, started_at, ended_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'c', 1, 2)",
            params![
                work,
                next_round(tx, work)?,
                cell,
                input.to_string(),
                output.to_string(),
                transition
            ],
        )?;
        tx.execute("UPDATE works SET at = ?2 WHERE id = ?1", params![work, at])?;
        Ok(())
    })
    .unwrap();
}

/// A return of an agent's cell with `outcome`, which tells the owner nothing.
fn returned(outcome: Value) -> Value {
    json!({ "outcome": outcome, "progress": "p", "side_reports": [], "flow_feedback": "none", "tell_owner": [] })
}

/// The input of a round of the review that was given the change as the blind review was: what the review is given
/// again while no commit follows, as when a fix changes only the PR description.
fn input_of_the_change_reviewed(e: &Engine, work: i64) -> Value {
    let change: Option<String> =
        e.db.conn()
            .query_row(
                "SELECT json_extract(input, '$.engine.change') FROM rounds WHERE work = ?1 AND cell = 'T3'
             ORDER BY n DESC LIMIT 1",
                [work],
                |r| r.get(0),
            )
            .unwrap();
    json!({ "engine": { "change": change } })
}

/// What the latest round of choosing what runs next planned for the review, and for adversarial testing.
fn planned(e: &Engine, work: i64) -> (String, String) {
    e.db.conn()
        .query_row(
            "SELECT json_extract(output, '$.plan.review'), json_extract(output, '$.plan.adversarial') FROM rounds
             WHERE work = ?1 AND cell = 'T7' ORDER BY n DESC LIMIT 1",
            [work],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap()
}

/// The row the latest round of the merge judgment took.
fn judged(e: &Engine, work: i64) -> String {
    e.db.conn()
        .query_row(
            "SELECT transition FROM rounds WHERE work = ?1 AND cell = 'M1' ORDER BY n DESC LIMIT 1",
            [work],
            |r| r.get(0),
        )
        .unwrap()
}

// A fix that answers a row the review left itself (a finding, a finding it did not accept as fixed, or one it insisted
// on) with no commit after it, as when it changes only the PR description or rests on the advisor's ruling, sends the
// work back to that review, whatever the answer: only the review can close the row. Without it, the merge judgment
// sends the work to a fix that has no row to answer, and the work goes round the merge judgment, the fix and the
// choice of what runs next with no end, as in the sixth trial. Each of the three ways back to the review is taken
// once, then the review closes its row and the work goes on.
#[test]
fn a_fix_that_answered_the_reviews_row_without_a_commit_goes_back_to_the_review() {
    let s = setup("fix-no-commit");
    let (mut e, said, item) = start(&s, scripts());
    step_to(&mut e, item, "T4");
    let (work, _, _) = state(&e, item).unwrap();
    let reviewed = input_of_the_change_reviewed(&e, work);
    let log = || said.lock().unwrap().join("\n");
    // The review hands in its return and the merge judgment reads it.
    let review = |e: &mut Engine, outcome: Value| {
        seed_round(e, work, "T4", &reviewed, &returned(outcome), "done", "M1");
        e.step_work(work).unwrap();
    };
    // The fix answers the one row it was given, which came from `from`, and the work stands where the choice of what
    // runs next is made.
    let fix = |e: &mut Engine, row: Value, answer: Value| {
        let input = json!({ "engine": { "table": [row] } });
        let output = returned(json!({ "rows": [answer], "head": "abc" }));
        seed_round(e, work, "T6", &input, &output, "done", "T7");
    };
    // The choice of what runs next, and where the work goes after it.
    let choose = |e: &mut Engine| {
        e.step_work(work).unwrap();
        (planned(e, work), state(e, item).unwrap().1)
    };
    let disputes = (("disputes".to_string(), "skipped".to_string()), "T4".to_string());
    let finding = json!({ "id": "1", "verdict": "findings", "evidence": { "files": [], "ran": [], "nodes": [] },
        "findings": [{ "fid": "R1", "where": "PR description", "what": "It omits the ruling.", "fix": "Add it." }] });

    // A finding of a column, answered fixed: the PR description was changed.
    review(&mut e, json!({ "columns": [finding] }));
    assert_eq!(judged(&e, work), "fix", "{}", log());
    let row = json!({ "row": "F1", "from": "T4·columns[findings]", "column": "1", "fid": "R1",
                      "where": "PR description", "what": "It omits the ruling.", "fix": "Add it." });
    fix(&mut e, row, json!({ "id": "F1", "answer": "fixed", "commit": "abc" }));
    assert_eq!(choose(&mut e), disputes, "{}", log());
    // The review closes it: the merge judgment sends the work to no fix, and the same answer is not read again.
    review(&mut e, json!({ "previous": [{ "fid": "R1", "verdict": "closed" }] }));
    assert!(!judged(&e, work).starts_with("fix"), "{}\n{}", judged(&e, work), log());
    move_to(&mut e, work, "T7");
    assert_eq!(choose(&mut e).0.0, "none", "{}", log());

    // The finding not accepted as fixed, answered fixed again.
    review(
        &mut e,
        json!({ "previous": [{ "fid": "R1", "verdict": "not_fixed", "reason": "The ruling is still missing." }] }),
    );
    assert_eq!(judged(&e, work), "fix", "{}", log());
    let message = engine::next_message(&mut e, work).unwrap().text;
    assert!(message.contains("The ruling is still missing."), "{message}");
    let row = json!({ "row": "F1", "from": "T4·previous[not_fixed]", "fid": "R1", "verdict": "not_fixed",
                      "reason": "The ruling is still missing." });
    fix(&mut e, row, json!({ "id": "F1", "answer": "fixed", "commit": "abc" }));
    assert_eq!(choose(&mut e), disputes, "{}", log());
    review(&mut e, json!({ "previous": [{ "fid": "R1", "verdict": "closed" }] }));
    assert!(!judged(&e, work).starts_with("fix"), "{}\n{}", judged(&e, work), log());

    // The finding the review insisted on, answered advisor_ruled: the review withdraws it by the ruling.
    review(
        &mut e,
        json!({ "previous": [{ "fid": "R1", "verdict": "insist", "reason": "The ruling is still missing.",
                               "basis": { "kind": "read", "check": "the PR description" } }] }),
    );
    assert_eq!(judged(&e, work), "fix", "{}", log());
    let row = json!({ "row": "F1", "from": "T4·previous[insist]", "fid": "R1", "verdict": "insist",
                      "reason": "The ruling is still missing." });
    fix(
        &mut e,
        row,
        json!({ "id": "F1", "answer": "advisor_ruled", "ruling": "No change is needed." }),
    );
    assert_eq!(choose(&mut e), disputes, "{}", log());
    review(&mut e, json!({ "previous": [{ "fid": "R1", "verdict": "withdraw" }] }));
    assert!(!judged(&e, work).starts_with("fix"), "{}\n{}", judged(&e, work), log());
    let _ = std::fs::remove_dir_all(&s.root);
}

// A fix that answers a row another cell left (a red check of the CI, say) with no commit after it is no reason to
// review again: the review has no row of its own to close, and what the fix changed is what the checks look at.
#[test]
fn a_fix_that_answered_a_row_of_another_cell_without_a_commit_is_not_reviewed_again() {
    let s = setup("fix-no-commit-other");
    let (mut e, said, item) = start(&s, scripts());
    step_to(&mut e, item, "T4");
    let (work, _, _) = state(&e, item).unwrap();
    let pass = json!({ "id": "1", "verdict": "pass", "evidence": { "files": [], "ran": [], "nodes": [] } });
    let reviewed = input_of_the_change_reviewed(&e, work);
    seed_round(
        &mut e,
        work,
        "T4",
        &reviewed,
        &returned(json!({ "columns": [pass] })),
        "done",
        "M1",
    );
    let row = json!({ "row": "F1", "from": "T2·result[red]", "kind": "red", "name": "lint", "result": "red",
                      "run": "https://example.com/runs/7" });
    let input = json!({ "engine": { "table": [row] } });
    let output = returned(json!({ "rows": [{ "id": "F1", "answer": "fixed", "commit": "abc" }], "head": "abc" }));
    seed_round(&mut e, work, "T6", &input, &output, "done", "T7");
    e.step_work(work).unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(planned(&e, work), ("none".to_string(), "skipped".to_string()), "{log}");
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
                "SELECT count(*) FROM rounds r JOIN works w ON w.id = r.work
                 WHERE w.graph = 'engineer' AND r.cell NOT IN ('G1', 'G2')",
                [],
                |r| r.get(0),
            )
            .unwrap()
    };
    let first = e.step(None).unwrap();
    let log = said.lock().unwrap().join("\n");
    let (work, at, _) = state(&e, item).unwrap_or_else(|| panic!("no work started:\n{log}"));
    assert_eq!(first.work, Some(work), "{log}");
    assert_eq!(first.started_item, Some(item));
    assert_eq!(first.cell.as_deref(), Some("S1"));
    assert_eq!(rounds(&e), 1, "{log}");
    assert_ne!(at, "S1", "{log}");
    let mut last = first.work;
    for n in 2..=4 {
        let next = e.step(last).unwrap();
        let log = said.lock().unwrap().join("\n");
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
    let log = said.lock().unwrap().join("\n");
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

/// Puts works on a flow version before the one the engine runs: a copy of that one, its engineer's graph changed as
/// `change` says. Gives the version.
fn older(e: &mut Engine, works: &[i64], change: impl FnOnce(&mut flow::CompiledGraph)) -> i64 {
    let now = e.flow_now;
    let mut f = flow::Compiled::clone(&e.flow(now).unwrap());
    change(f.graphs.iter_mut().find(|g| g.name == engine::ENGINEER).unwrap());
    let text = f.json();
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO flow_versions (digest, compiled) VALUES ('older', ?1)",
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

/// The work an item had first, at its second cell `R2` on a flow version before the one the engine runs, changed as
/// `change` says, with the engine started again over it; gives the engine, what it says, the item and the work.
fn at_r2_on_an_older_version(
    s: &Setup,
    change: impl FnOnce(&mut flow::CompiledGraph),
) -> (Engine, Arc<Mutex<Vec<String>>>, i64, i64) {
    let (mut e, said, item) = start(s, scripts());
    for _ in 0..2 {
        e.step(None).unwrap();
    }
    let log = said.lock().unwrap().join("\n");
    let (work, at, _) = state(&e, item).unwrap();
    assert_eq!(at, "R2", "{log}");
    older(&mut e, &[work], change);
    drop(e);
    let (e, said) = engine(s, scripts());
    (e, said, item, work)
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

// A work on an older flow version that goes on moves to the version the engine runs when the cell it stands at is
// unchanged there and every return it handed in still fits its cell there, whatever else changed: the same work goes
// on at its cell in the same session, and says which version it moved from.
#[test]
fn a_work_at_an_unchanged_cell_moves_and_keeps_its_session() {
    let s = setup("moves");
    let (mut e, said, item, work) = at_r2_on_an_older_version(&s, |g| renamed(g, "D4"));
    let v: i64 =
        e.db.conn()
            .query_row("SELECT flow_version FROM works WHERE id = ?1", [work], |r| r.get(0))
            .unwrap();
    let stepped = e.step(None).unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(stepped.work, Some(work), "{log}");
    assert_eq!(stepped.cell.as_deref(), Some("R2"), "{log}");
    let works = works_of(&e, item);
    assert_eq!(works.len(), 1, "{works:?}\n{log}");
    assert_eq!(works[0].5, None);
    let (now, from): (i64, Option<i64>) =
        e.db.conn()
            .query_row(
                "SELECT flow_version, moved_from FROM works WHERE id = ?1",
                [work],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
    assert_eq!((now, from), (e.flow_now, Some(v)), "{log}");
    assert_eq!(session_of(&e, work, "R2"), session_of(&e, work, "R1"), "{log}");
    assert!(
        log.contains(&format!("work {work} moved from flow version {v} to {now}, at cell R2")),
        "{log}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

/// A change of a flow version at cell `R1`: its return may hold a field `before`, which the version the engine runs
/// does not have.
fn with_before(g: &mut flow::CompiledGraph) {
    let r1 = g.cells.iter_mut().find(|c| c.id == "R1").unwrap();
    if let Some(Value::Object(props)) = r1.schema.as_mut().and_then(|s| s.get_mut("properties")) {
        props.insert("before".into(), json!({ "type": "string" }));
    }
}

// A work on an older flow version whose earlier return's cell has another schema in the version the engine runs, which
// still takes the return as the round stored it, moves: a field the return does not hold, taken out.
#[test]
fn a_work_whose_earlier_returns_still_fit_moves() {
    let s = setup("refit");
    let (mut e, said, item, work) = at_r2_on_an_older_version(&s, with_before);
    let stepped = e.step(None).unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(stepped.work, Some(work), "{log}");
    assert_eq!(stepped.cell.as_deref(), Some("R2"), "{log}");
    assert_eq!(works_of(&e, item).len(), 1, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A work on an older flow version whose cell is unchanged, but one of whose earlier returns its cell's schema in the
// version the engine runs does not take, is taken over when it goes on: it ends, saying the round and where the return
// does not fit, and a new work takes its item over.
#[test]
fn a_work_whose_earlier_return_no_longer_fits_is_taken_over() {
    let s = setup("reshaped");
    let (mut e, said, item, work) = at_r2_on_an_older_version(&s, with_before);
    let n: i64 =
        e.db.write(Actor::Engine, |tx| {
            let n: i64 = tx.query_row(
                "SELECT n FROM rounds WHERE work = ?1 AND cell = 'R1' AND transition IS NOT NULL",
                [work],
                |r| r.get(0),
            )?;
            tx.execute(
                "UPDATE rounds SET output = json_set(output, '$.before', 'kept') WHERE work = ?1 AND n = ?2",
                params![work, n],
            )?;
            Ok(n)
        })
        .unwrap();
    let stepped = e.step(None).unwrap();
    let log = said.lock().unwrap().join("\n");
    let works = works_of(&e, item);
    assert_eq!(works.len(), 2, "{works:?}\n{log}");
    assert_eq!(works[0].0, work);
    let now = e.flow_now;
    let ended = works[0].5.clone().unwrap_or_default();
    assert!(
        ended.starts_with(&format!(
            "stopped: the flow moved to version {now}, where the return of cell R1 in round R{work}-{n} no longer \
             fits it (the return has before, which is not one of its fields: "
        )),
        "{ended}\n{log}"
    );
    assert!(
        ended.ends_with("), and a new work on it takes the item over"),
        "{ended}"
    );
    assert_eq!(
        (stepped.work, stepped.started_item),
        (Some(works[1].0), Some(item)),
        "{log}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
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
    let log = said.lock().unwrap().join("\n");
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
    let log = said.lock().unwrap().join("\n");
    let (old, at, worktree, branch, pr, _) = works_of(&e, item)[0].clone();
    assert!(pr.is_some() && worktree.is_some() && branch.is_some(), "{log}");
    // The work ran on a flow version the engine no longer has built in, where the cell it stands at was another.
    older(&mut e, &[old], |g| renamed(g, &at));
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
        speaker: "owner".into(),
        why: None,
        reply: None,
        to: None,
        images: Vec::new(),
    };
    api::answer(&mut e.db, asks[0], &answer).unwrap();
    let first = e.step(None).unwrap();
    let log = said.lock().unwrap().join("\n");
    let works = works_of(&e, item);
    assert_eq!(works.len(), 2, "{works:?}\n{log}");
    assert!(
        works[0]
            .5
            .as_deref()
            .is_some_and(|w| w.starts_with("stopped: the flow moved to version")
                && w.contains(&format!("where cell {at} changed (its rows)"))),
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
    let log = said.lock().unwrap().join("\n");
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
    let log = said.lock().unwrap().join("\n");
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
    let log = said.lock().unwrap().join("\n");
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

// Which way the design stage goes is the engine's cells' rows, never a switch hidden in a gate: a project with nothing
// under test and no other project goes past those gates (`D4` `no`, `D8` `no`), each a round of the engine's own; the
// cells on the ways not taken never run.
#[test]
fn the_design_stage_goes_its_way_by_the_engines_rows() {
    let s = setup("design-way");
    let (mut e, said, item) = start(&s, scripts());
    step_to(&mut e, item, "I1");
    let log = said.lock().unwrap().join("\n");
    for (cell, row) in [("D4", "no"), ("D8", "no")] {
        assert_eq!(
            ended_at(&e, item, cell),
            [(Some(row.to_string()), None)],
            "{cell}\n{log}"
        );
    }
    for cell in ["D5", "D9"] {
        assert!(ended_at(&e, item, cell).is_empty(), "{cell}\n{log}");
    }
    let _ = std::fs::remove_dir_all(&s.root);
}

/// The check of the test's records, standing in for the reasoning system's: it says what it was given, and is red until
/// the records hold a log.
const CHECK_PY: &str = "import os, sys\nprint('given: ' + ' '.join(sys.argv[1:]))\nsys.exit(0 if os.path.exists('design/log.md') else 1)\n";

/// A setup whose project keeps records, in `design/`, kept by a copy of the reasoning system in the test's directory
/// that the installation names: its rules, each saying which it is, and its check ([`CHECK_PY`]).
fn setup_records(name: &str) -> Setup {
    let s = setup(name);
    let system = s.root.join("reasoning-system");
    for rule in ["reading", "writing", "dcr", "extension-work", "extension-cards"] {
        write(&system, &format!("rules/{rule}.md"), &format!("The rule {rule}.\n"));
    }
    write(&system, "check.py", CHECK_PY);
    let install = std::fs::read_to_string(s.data.join("install.toml")).unwrap();
    write(
        &s.data,
        "install.toml",
        &format!("{install}\n[reasoning]\nsystem = \"{}\"\n", system.display()),
    );
    let p = std::fs::read_to_string(s.data.join("projects/p.toml")).unwrap();
    write(
        &s.data,
        "projects/p.toml",
        &p.replace(
            "rules = [\"ground\"]",
            "rules = [\"ground\"]\nrecords = { root = \"design\", ascii = true }",
        ),
    );
    git(
        &s.data,
        &["commit", "-q", "-am", "records kept by the reasoning system"],
    );
    s
}

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

// An interjection that comes while the work stands at one of the engine's steps and asks only for a change to the code
// is the way on `fix`: the change is a row of the fix's table, which the fix answers like any other, and the work goes
// on as after any fix, with no new design.
#[test]
fn a_change_to_the_code_an_interjection_asks_is_a_row_of_the_fix() {
    let s = setup("way-on-fix");
    let mut agents = scripts();
    agents.insert(
        "A4",
        Box::new(|_: &Request, _: usize| {
            (
                vec![("outcome.next", "fix")],
                vec![
                    (
                        "outcome.next.change.what",
                        json!("Halt every work on a red check, with a test."),
                    ),
                    ("outcome.next.change.asked_in", json!("The owner: halt every work.")),
                ],
            )
        }),
    );
    // The fix makes the change and asks for ci, then answers its table's one row.
    agents.insert(
        "T6",
        Box::new(|r: &Request, n: usize| {
            let wt = &r.worktree;
            if n == 1 {
                write(wt, "src/lib.txt", "old\nfeature\nhalt\n");
                git(wt, &["add", "-A"]);
                git(wt, &["commit", "-q", "-m", "Halt every work"]);
                let head = git(wt, &["rev-parse", "HEAD"]);
                let tools = json!([{ "tool": "ci", "head": head, "tests": ["feature"] }]);
                return (vec![("outcome", "tools")], vec![("outcome.tools", tools)]);
            }
            let head = git(wt, &["rev-parse", "HEAD"]);
            (
                Vec::new(),
                vec![
                    ("outcome.head", json!(head)),
                    (
                        "outcome.rows",
                        json!([{ "answer": "fixed", "id": "F1", "commit": head }]),
                    ),
                ],
            )
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    step_to(&mut e, item, "T7");
    let (work, _, _) = state(&e, item).unwrap();
    owner_says(&mut e, work, "Halt every work on a red check; nothing else changes.");
    for _ in 0..12 {
        if ended_at(&e, item, "T6")
            .iter()
            .any(|(t, _)| t.as_deref() == Some("done"))
        {
            break;
        }
        e.step(None).unwrap();
    }
    let log = said.lock().unwrap().join("\n");
    assert_eq!(ended_at(&e, item, "A4"), [(Some("fix".to_string()), None)], "{log}");
    let fixes = ended_at(&e, item, "T6");
    assert_eq!(
        fixes.last(),
        Some(&(Some("done".to_string()), None)),
        "{fixes:?}\n{log}"
    );
    let table = message_at(&e, work, "T6", false);
    assert!(
        table.contains("Halt every work on a red check, with a test."),
        "{table}"
    );
    assert!(ended_at(&e, item, "D1").len() == 1, "no new design\n{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// On a redesign, a check after design is given what the design changed since it last answered, sentence by sentence,
// with its answer then; the first time it judges the whole design and is given no change.
#[test]
fn a_check_after_a_redesign_is_given_what_the_design_changed() {
    let s = setup("gates-change");
    let mut agents = scripts();
    agents.insert(
        "D2",
        Box::new(|_: &Request, n: usize| {
            let solution = if n == 1 {
                "Read the input. Parse it into tokens."
            } else {
                "Read the input. Parse it into tokens. Keep the tokens' positions."
            };
            (
                vec![("outcome.measure", "not_measured")],
                vec![
                    (
                        "outcome.acceptance",
                        json!([{ "by": "test", "id": "A1", "condition": "The feature is there." }]),
                    ),
                    ("outcome.design.solution", json!(solution)),
                    (
                        "outcome.design.where",
                        json!("The parser module and its tests, and nothing else of the program."),
                    ),
                ],
            )
        }),
    );
    agents.insert(
        "A4",
        Box::new(|_: &Request, _: usize| (vec![("outcome.next", "redesign")], Vec::new())),
    );
    let (mut e, said, item) = start(&s, agents);
    step_to(&mut e, item, "T7");
    let (work, _, _) = state(&e, item).unwrap();
    owner_says(&mut e, work, "Keep the tokens' positions too.");
    for _ in 0..30 {
        if ended_at(&e, item, "D6").len() == 2 {
            break;
        }
        e.step(None).unwrap();
    }
    let log = said.lock().unwrap().join("\n");
    assert_eq!(ended_at(&e, item, "D6").len(), 2, "{log}");
    let first = message_at(&e, work, "D6", false);
    assert!(!first.contains("engine·design_change"), "{first}");
    let again = message_at(&e, work, "D6", true);
    assert!(again.contains("+Keep the tokens' positions."), "{again}");
    assert!(again.contains("your_last_answer"), "{again}");
    assert!(!again.contains("-Read the input."), "{again}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A new work that takes over from one the new flow version stopped takes a slot only when one is free; until then its
// item waits at the head of the queue, ahead of the items queued before, in the order of the works it replaces.
#[test]
fn a_takeover_waits_for_a_free_slot_at_the_head_of_the_queue() {
    let s = setup("takeover-slots");
    let (mut e, _, queued) = start(&s, scripts());
    // Two items had works running on a flow version the engine no longer has built in, where their cell was another;
    // the machine has one slot.
    let v = older(&mut e, &[], |g| renamed(g, "R1"));
    let items: Vec<i64> =
        e.db.write(Actor::Owner, |tx| {
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
    let log = said.lock().unwrap().join("\n");
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
    let log = said.lock().unwrap().join("\n");
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
    let log = said.lock().unwrap().join("\n");
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

// A pull request keeps the base it was opened with, and a work merges it into the project's main and nowhere else: the
// pull request an imported item takes over, opened against another branch, is set to merge into main as the work
// first pushes to it, and, its base another branch again by the time it merges, is set again before the merge; the
// branch it was opened against gets nothing.
#[test]
fn a_pull_request_opened_against_another_branch_merges_into_main_only() {
    let s = setup("base");
    let checkout = s.root.join("checkout");
    let origin = s.origin.to_string_lossy().to_string();
    git(&checkout, &["push", "-q", "origin", "HEAD:first-version"]);
    let first_version = git(&s.root, &["--git-dir", &origin, "rev-parse", "first-version"]);
    git(&checkout, &["checkout", "-q", "-b", "outside/feature"]);
    write(&checkout, "src/lib.txt", "half done\n");
    git(&checkout, &["commit", "-q", "-am", "Half of the feature"]);
    git(&checkout, &["push", "-q", "origin", "outside/feature"]);
    git(&checkout, &["checkout", "-q", "main"]);
    git(&checkout, &["branch", "-q", "-D", "outside/feature"]);
    let elsewhere = Pr {
        branch: "outside/feature".into(),
        base: "first-version".into(),
    };
    s.prs.lock().unwrap().insert(9, elsewhere.clone());
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
    let base_of = |n: i64| s.prs.lock().unwrap()[&n].base.clone();
    let mut pushed = false;
    for _ in 0..100 {
        if state(&e, item).is_some_and(|(_, at, _)| at == "M2") {
            break;
        }
        e.step(None).unwrap();
        pushed |= !ended_at(&e, item, "X1").is_empty();
        if !pushed {
            assert_eq!(base_of(9), "first-version");
        }
    }
    let log = said.lock().unwrap().join("\n");
    assert_eq!(state(&e, item).map(|(_, at, _)| at).as_deref(), Some("M2"), "{log}");
    assert!(pushed, "{log}");
    assert_eq!(base_of(9), "main", "{log}");
    // Its base is set again before it merges, whatever changed it since.
    s.prs.lock().unwrap().insert(9, elsewhere);
    for _ in 0..20 {
        if !ended_at(&e, item, "M2").is_empty() {
            break;
        }
        e.step(None).unwrap();
    }
    let log = said.lock().unwrap().join("\n");
    assert_eq!(ended_at(&e, item, "M2"), [(Some("merged".to_string()), None)], "{log}");
    assert_eq!(base_of(9), "main", "{log}");
    assert_eq!(
        git(&s.root, &["--git-dir", &origin, "show", "main:src/lib.txt"]),
        "old\nfeature"
    );
    assert_eq!(
        git(&s.root, &["--git-dir", &origin, "rev-parse", "first-version"]),
        first_version
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// Three returns of a cell sent back in a row pause the work; once the owner lets it go on, it counts afresh.
#[test]
fn a_work_let_go_on_after_returns_were_sent_back_counts_afresh() {
    let s = setup("resume");
    let mut scripts = scripts();
    // As many returns as the flow sends back before it pauses, read once the engine has the flow.
    let max = Arc::new(AtomicUsize::new(0));
    let bad_until = max.clone();
    scripts.insert(
        "R1",
        Box::new(move |_: &Request, n: usize| {
            let bad = vec![("outcome", json!("the JSON text of an object"))];
            (
                Vec::new(),
                if n <= bad_until.load(Ordering::Relaxed) {
                    bad
                } else {
                    Vec::new()
                },
            )
        }),
    );
    let (mut e, said, item) = start(&s, scripts);
    max.store(pause_after(&mut e), Ordering::Relaxed);
    let mut paused = None;
    for _ in 0..20 {
        e.tick().unwrap();
        if let Some((id, _, Some(why))) = state(&e, item) {
            paused = Some((id, why));
            break;
        }
    }
    let log = said.lock().unwrap().join("\n");
    let (id, why) = paused.unwrap_or_else(|| panic!("the work did not pause:\n{log}"));
    assert!(why.starts_with("3 returns of cell R1 in a row were sent back"), "{why}");
    assert!(why.contains("outcome must be an object, and is a string"), "{why}");
    api::resume(&mut e.db, id).unwrap();
    for _ in 0..3 {
        e.tick().unwrap();
    }
    let log = said.lock().unwrap().join("\n");
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
         WHERE w.item = ?1 AND r.session IS NOT NULL AND r.cell NOT IN ('G1', 'G2') ORDER BY r.work, r.n",
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
    let log = said.lock().unwrap().join("\n");
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
    for a in ["format:R1", "inputs:R1", workspace.as_str()] {
        assert_eq!(given(first, a), Some(true), "{a}");
    }
    // What every session of the line is given alike is in the session's system prompt, which its messages refer to.
    let system = system_text(first).expect("the system prompt of the engineer's first session");
    for a in ["format:common", "rule:ground", "rule:general", "rule:engineering"] {
        assert_eq!(given(first, a), Some(false), "{a}");
        assert!(in_system(first, a), "{a}: {}", first["system"]);
        assert!(system.contains(&format!("\n### {a}\n\n")), "{a}: {system}");
    }
    assert!(!in_system(first, "format:R1") && !in_system(first, &workspace));
    for a in ["format:common", "rule:ground", workspace.as_str()] {
        assert_eq!(given(second, a), Some(false), "{a}");
    }
    // A session going on carries the system prompt it started with, recorded once; every session of the line starts
    // with the same, byte for byte, and another line's with its own.
    assert_eq!(system_text(second), Some(system.clone()));
    assert!(second.get("system").is_none(), "{second}");
    assert_eq!(system_text(&at("I1")), Some(system.clone()));
    let review = system_text(&at("T3")).unwrap();
    assert_ne!(review, system);
    assert!(review.contains("\n### format:common\n\n") && !review.contains("\n### rule:engineering\n\n"));
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
    assert!(at_guide < m.find("### format:R1").unwrap(), "{m}");
    assert_eq!(guide(second).map(|g| g.1), Some(false));
    assert_eq!(guide(&at("T3")).map(|g| g.1), Some(true));
    let m = second["message"].as_str().unwrap();
    assert!(!m.contains("### rule:ground") && m.contains("rule:ground"), "{m}");
    let m = first["message"].as_str().unwrap();
    // The index of the database is in the query tool's description, which the return formats give.
    assert!(!m.contains("db:tables"), "{m}");
    assert!(given(first, "db:tables").is_none());
    // The formats, in the system prompt and the message, name the owner's language; the engine writes the configured
    // one beside it.
    let formats = format!("{system}\n{m}");
    assert!(formats.contains("`owner.language` (English)"), "{formats}");
    assert!(
        formats.contains("SELECT sql FROM sqlite_master WHERE name = '<name>'"),
        "{formats}"
    );
    assert!(formats.contains("- items (id, project, title, "), "{formats}");
    assert!(!formats.contains("CREATE TABLE"), "{formats}");
    let design = &rounds.iter().rev().find(|r| r.1 == "D2").unwrap().0;
    let acceptance = format!("{design}·D2·acceptance");
    assert_eq!(given(&at("D11"), &acceptance), Some(false), "{}", at("D11"));
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
    // The record keeps each large text once: the shared formats and the structured-output schema of each line (the
    // engineer's, the reader of the history's and the review's) are in contents once, however many rounds and sessions
    // were given them, and a round's record refers to them.
    let kept = |like: &str| -> i64 {
        e.db.conn()
            .query_row("SELECT count(*) FROM contents WHERE text LIKE ?1", [like], |r| r.get(0))
            .unwrap()
    };
    assert_eq!(kept("The forms every agent's return shares.%"), 3);
    assert_eq!(kept("{\"type\":\"object\"%"), 3);
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

// A rule that changed since the session got it in its system prompt is given as what changed in it, under its address;
// after a compaction the session holds nothing but its system prompt, and the next round is given what it needs again
// whole, the session's own earlier returns included, and what changed since its system prompt again as what changed.
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
    let log = said.lock().unwrap().join("\n");
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
    assert_eq!((changes(b), changes(c)), (Some(true), Some(true)));
    assert!(in_system(a, "rule:ground"), "{a}");
    assert_eq!(
        given_text(b, "rule:ground").unwrap().trim_end(),
        format!(
            "What changed since rule:ground, given in {}, as a unified diff of its lines:\n@@ -1,1 +1,1 @@\n\
             -Facts first.\n+Facts first, then the rest.",
            rounds[0].0
        )
    );
    assert_eq!(given(b, "format:common"), Some(false));
    assert_eq!(given(c, "format:common"), Some(false));
    assert_eq!(given(c, "rule:ground"), Some(true));
    assert_eq!(given_text(c, "rule:ground"), given_text(b, "rule:ground"));
    assert_eq!(hash_of(b, "rule:ground"), hash_of(c, "rule:ground"));
    assert_eq!(given(c, "format:D1"), Some(true));
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
    let end = [rest.find("\n### "), rest.find("\n## ")]
        .into_iter()
        .flatten()
        .min()
        .unwrap_or(rest.len());
    Some(rest[..end].to_string())
}

// The project guide is read from main as the work's branch merged it: the work's own edits to its records and to
// AGENTS.md, committed or not, never make it a new one, so its session is not given it again.
#[test]
fn a_works_own_edits_to_its_records_do_not_give_the_guide_again() {
    let s = setup_records("own-guide");
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
    let log = said.lock().unwrap().join("\n");
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
    let s = setup_records("main-guide");
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
    let log = said.lock().unwrap().join("\n");
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
            .ends_with(r##"{"records":{"design/later.md":"# Later"}}"##),
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
             WHERE work = ?1 AND n = 3",
            [work],
        )?;
        Ok(())
    })
    .unwrap();
    let before = rounds_count(&e);
    e.step(None).unwrap();
    let log = said.lock().unwrap().join("\n");
    let (_, at, paused) = state(&e, item).unwrap();
    assert_eq!(at, "R2", "{log}");
    let paused = paused.unwrap_or_else(|| panic!("the work did not pause:\n{log}"));
    assert!(
        paused.starts_with(&format!(
            "cell R2: the message of R{work}-4 was not sent, since it would repeat what its session holds: \
             rule:ground (12 characters) would be given whole again, though the session got the same content in \
             R{work}-3 as rule:elsewhere"
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
    let mut f = flow::Compiled::clone(&e.flow(now).unwrap());
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
    let log = said.lock().unwrap().join("\n");
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
fn failing(name: &str, report: String, times: usize) -> (Setup, Engine, Arc<Mutex<Vec<String>>>, i64) {
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
        .query_row("SELECT count(*) FROM rounds WHERE cell NOT IN ('G1', 'G2')", [], |r| {
            r.get(0)
        })
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
    // The first pass starts the work; the second runs its first agent's round, which fails.
    e.tick().unwrap();
    e.tick().unwrap();
    let log = said.lock().unwrap().join("\n");
    let h = e.halt().unwrap_or_else(|| panic!("no wait:\n{log}"));
    assert_eq!((h.kind.as_str(), h.until), ("usage_limit", reset * 1000));
    let before = rounds_count(&e);
    e.tick().unwrap();
    e.tick().unwrap();
    assert_eq!(rounds_count(&e), before);
    assert_eq!(told(&s, "usage_limit"), 1);
    e.set_halt(e.halt().map(|h| engine::Halt { until: 0, ..h }));
    e.tick().unwrap();
    let log = said.lock().unwrap().join("\n");
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
    // The first pass starts the work; the second runs its first agent's round, which fails.
    e.tick().unwrap();
    e.tick().unwrap();
    let log = said.lock().unwrap().join("\n");
    let h = e.halt().unwrap_or_else(|| panic!("no wait:\n{log}"));
    assert_eq!(h.kind, "offline");
    assert!(h.until > db::now());
    let before = rounds_count(&e);
    e.tick().unwrap();
    assert_eq!(rounds_count(&e), before);
    e.set_halt(e.halt().map(|h| engine::Halt { until: 0, ..h }));
    e.tick().unwrap();
    let (_, at, paused) = state(&e, item).unwrap();
    assert_eq!(paused, None);
    assert_ne!(at, "R1");
    assert_eq!(failed_kinds(&e), ["offline"]);
    assert_eq!(sent_back(&e), 0);
    assert_eq!(told(&s, "offline"), 1);
    let _ = std::fs::remove_dir_all(&s.root);
}

// With a look at the network of its own, the engine runs no round while the network is down, looking again when each
// wait is over; once it is up, the work goes on from its cell. No round's message goes into a session meanwhile.
#[test]
fn no_network_is_looked_at_without_running_a_round() {
    let report = "API Error: Connection error. (getaddrinfo ENOTFOUND api.anthropic.com)".to_string();
    let s = setup("offline-look");
    let mut scripts = scripts();
    scripts.insert(
        "R1",
        Box::new(move |_: &Request, n: usize| {
            let fail = vec![("@error", json!(report))];
            (Vec::new(), if n <= 1 { fail } else { Vec::new() })
        }),
    );
    let up = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let seen = up.clone();
    let hooks = Hooks {
        network: Some(Arc::new(move || seen.load(std::sync::atomic::Ordering::SeqCst))),
        ..Hooks::default()
    };
    let (mut e, said, item) = start_with(&s, scripts, hooks);
    // The first pass starts the work; the second runs its first agent's round, which fails.
    e.tick().unwrap();
    e.tick().unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(e.halt().map(|h| h.kind).as_deref(), Some("offline"), "{log}");
    let before = rounds_count(&e);
    // The wait is over and the network still down: no round runs, and the engine waits again.
    e.set_halt(e.halt().map(|h| engine::Halt { until: 0, ..h }));
    e.tick().unwrap();
    assert_eq!(rounds_count(&e), before);
    assert!(e.halt().is_some_and(|h| h.until > db::now()));
    // Up again: the work goes on from its cell.
    up.store(true, std::sync::atomic::Ordering::SeqCst);
    e.set_halt(e.halt().map(|h| engine::Halt { until: 0, ..h }));
    e.tick().unwrap();
    let log = said.lock().unwrap().join("\n");
    let (_, at, paused) = state(&e, item).unwrap();
    assert_eq!(paused, None, "{log}");
    assert_ne!(at, "R1", "{log}");
    assert_eq!(failed_kinds(&e), ["offline"]);
    let _ = std::fs::remove_dir_all(&s.root);
}

// A tool that fails for want of the network, while the engine finds the network down, is no failure of the work's:
// no question goes to the architect, every work waits for the network, and the cell runs its tools again once it is
// back.
#[test]
fn a_tool_without_the_network_waits_for_it() {
    let s = setup("tool-offline");
    let up = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let seen = up.clone();
    let hooks = Hooks {
        network: Some(Arc::new(move || seen.load(std::sync::atomic::Ordering::SeqCst))),
        ..Hooks::default()
    };
    let (mut e, said, item) = start_with(&s, scripts(), hooks);
    for _ in 0..40 {
        if state(&e, item).is_some_and(|x| x.1 == "X1") {
            break;
        }
        e.tick().unwrap();
    }
    let log = said.lock().unwrap().join("\n");
    assert_eq!(state(&e, item).map(|x| x.1).as_deref(), Some("X1"), "{log}");
    // The network goes down: the push cannot reach the remote.
    let away = s.root.join("origin-away");
    std::fs::rename(&s.origin, &away).unwrap();
    up.store(false, std::sync::atomic::Ordering::SeqCst);
    e.tick().unwrap();
    let log = said.lock().unwrap().join("\n");
    let (_, at, paused) = state(&e, item).unwrap();
    assert_eq!(at, "X1", "{log}");
    assert_eq!(paused, None, "{log}");
    assert_eq!(e.halt().map(|h| h.kind).as_deref(), Some("offline"), "{log}");
    let asked: i64 =
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM architect_matters WHERE work IS NOT NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(asked, 0, "{log}");
    // Back: the cell runs its tools again and the work goes on.
    std::fs::rename(&away, &s.origin).unwrap();
    up.store(true, std::sync::atomic::Ordering::SeqCst);
    e.set_halt(e.halt().map(|h| engine::Halt { until: 0, ..h }));
    e.tick().unwrap();
    let log = said.lock().unwrap().join("\n");
    let (_, at, paused) = state(&e, item).unwrap();
    assert_eq!(paused, None, "{log}");
    assert_ne!(at, "X1", "{log}");
    assert!(failed_kinds(&e).contains(&"offline".to_string()), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A message the service refused is not run again in the session that holds what it refused: the line's session ends
// and the round runs once more in a new one.
#[test]
fn a_refused_run_goes_on_in_a_new_session_and_pauses_when_refused_again() {
    let report =
        "API Error: Opus 5.5's safeguards flagged this message (https://www.anthropic.com/legal/aup).".to_string();
    let (s, mut e, said, item) = failing("flagged-once", report.clone(), 1);
    for _ in 0..6 {
        e.tick().unwrap();
        if state(&e, item).unwrap().1 != "R1" {
            break;
        }
    }
    let log = said.lock().unwrap().join("\n");
    let (work, at, paused) = state(&e, item).unwrap();
    assert_eq!(paused, None, "{log}");
    assert_ne!(at, "R1", "{log}");
    assert_eq!(failed_kinds(&e), ["flagged"]);
    let sessions: Vec<(i64, Option<i64>)> = {
        let conn = e.db.conn();
        let mut st = conn
            .prepare(
                "SELECT r.session, s.ended FROM rounds r JOIN sessions s ON s.id = r.session
                 WHERE r.work = ?1 AND r.cell = 'R1' ORDER BY r.n",
            )
            .unwrap();
        st.query_map([work], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    };
    assert_eq!(sessions.len(), 2, "{sessions:?}");
    assert_ne!(sessions[0].0, sessions[1].0, "{sessions:?}");
    assert!(sessions[0].1.is_some(), "{sessions:?}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// Refused again in the new session, the work pauses and the owner is told, naming both rounds: what the agent is given
// needs a look.
#[test]
fn a_run_refused_twice_pauses_its_work() {
    let report =
        "API Error: Opus 5.5's safeguards flagged this message (https://www.anthropic.com/legal/aup).".to_string();
    let (s, mut e, said, item) = failing("flagged-twice", report, 2);
    for _ in 0..6 {
        e.tick().unwrap();
    }
    let log = said.lock().unwrap().join("\n");
    let (_, at, paused) = state(&e, item).unwrap();
    assert_eq!(at, "R1", "{log}");
    assert!(
        paused.as_deref().is_some_and(|p| p.contains("refused the agent's run")),
        "{paused:?}\n{log}"
    );
    assert_eq!(failed_kinds(&e), ["flagged", "flagged"]);
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
    let log = said.lock().unwrap().join("\n");
    let (_, at, paused) = state(&e, item).unwrap();
    assert_ne!(at, "R1", "{log}");
    assert_eq!(paused, None, "{log}");
    assert_eq!(failed_kinds(&e), ["transient", "transient", "transient"]);
    assert_eq!(sent_back(&e), 0);
    // The round after a failed run is told its kind only, never what the command line reported, which may quote the
    // run's own return. The report is looked for whole: the test's directory holds its process id, which may hold 529.
    let (work, _, _) = state(&e, item).unwrap();
    let again = message_at(&e, work, "R1", true);
    assert!(
        again.contains("ended without a return (transient)") && !again.contains("529 Overloaded"),
        "{again}"
    );
    // The session holds the message of the run that failed, which the command line wrote there: the round run again
    // is not given it once more.
    let first = message_at(&e, work, "R1", false);
    assert!(
        again.len() * 2 < first.len(),
        "{} of {}\n{again}",
        again.len(),
        first.len()
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
) -> (Setup, Engine, Arc<Mutex<Vec<String>>>, i64) {
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
    let log = said.lock().unwrap().join("\n");
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
    let log = said.lock().unwrap().join("\n");
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
    let log = said.lock().unwrap().join("\n");
    let (work, at, paused) = state(&e, item).unwrap();
    assert_eq!((at.as_str(), paused), ("R2", None), "{log}");
    assert_eq!(rows_at(&e, work, "R1"), ["done"], "{log}");
    assert!(failed_kinds(&e).is_empty(), "{log}");
    let (record, _) = repair_record(&e, work);
    assert_eq!(record["as_it_is"], true, "{record}");
    assert!(
        record.get("repair").is_none() && record.get("again").is_none(),
        "{record}"
    );
    assert_eq!(taken_from_text(&e, work).as_deref(), Some("outcome"), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// In the engineer's trial (R18-5, R18-10 and R18-11, cell D2) every attempt the command line refused for its outcome as
// a string held the JSON of the outcome with a closing brace too many after it: the model wrote the form as an object
// whose JSON did not parse, which reached the command line as text. A run the command line gave up on so is taken from
// its last attempt within its round, with the outcome that JSON holds, and the round records that its outcome was taken
// from the text, as for a string that holds the JSON alone.
#[test]
fn an_attempt_refused_for_its_outcome_as_json_text_with_a_brace_too_many_is_taken_within_its_round() {
    let (s, mut e, said, item) = unparsed_at_r1("unparsed-outcome-brace", "outcome_text_brace", None, None);
    past_r1(&mut e, item);
    let log = said.lock().unwrap().join("\n");
    let (work, at, paused) = state(&e, item).unwrap();
    assert_eq!((at.as_str(), paused), ("R2", None), "{log}");
    assert_eq!(rows_at(&e, work, "R1"), ["done"], "{log}");
    assert_eq!(sent_back(&e), 0, "{log}");
    let (record, _) = repair_record(&e, work);
    assert_eq!(record["as_it_is"], true, "{record}");
    assert_eq!(taken_from_text(&e, work).as_deref(), Some("outcome"), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A repair is checked as the return it gives is taken: a repaired return whose outcome is a string holding its JSON is
// taken with that outcome, and the round records that it was taken from the text, instead of the repair being refused
// for an outcome that is no object and the agent asked to hand its return in again.
#[test]
fn a_repair_whose_outcome_is_its_json_text_is_taken_as_any_return() {
    let escapes: Script = Box::new(|r: &Request, _: usize| {
        let text = r.prompt.split("The text:\n").nth(1).unwrap_or_default().trim();
        let fixed = text.replacen(" \"at\" ", " \\\"at\\\" ", 1);
        (Vec::new(), vec![("@result", json!(fixed))])
    });
    let (s, mut e, said, item) = unparsed_at_r1(
        "unparsed-repair-text",
        "unescaped_quote_outcome_text",
        Some(escapes),
        None,
    );
    past_r1(&mut e, item);
    let log = said.lock().unwrap().join("\n");
    let (work, at, paused) = state(&e, item).unwrap();
    assert_eq!((at.as_str(), paused), ("R2", None), "{log}");
    assert_eq!(sent_back(&e), 0, "{log}");
    let (record, _) = repair_record(&e, work);
    assert_eq!(record["repair"]["taken"], true, "{record}");
    assert!(record.get("again").is_none(), "{record}");
    assert_eq!(taken_from_text(&e, work).as_deref(), Some("outcome"), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The attempts the command line refused in a run that handed a return in at last are kept on its round, each by why,
// and the round takes its row as any: nothing is sent back.
#[test]
fn the_attempts_the_command_line_refused_before_it_took_a_return_are_recorded() {
    let s = setup("refused-before-taken");
    let mut agents = scripts();
    let why = "Output does not match required schema: /outcome: must NOT have additional properties ('tell_owner' is \
               not allowed)";
    agents.insert(
        "R1",
        Box::new(move |_: &Request, n: usize| match n {
            1 => (Vec::new(), vec![("@refused", json!([why]))]),
            _ => (Vec::new(), Vec::new()),
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    past_r1(&mut e, item);
    let log = said.lock().unwrap().join("\n");
    let (work, at, paused) = state(&e, item).unwrap();
    assert_eq!((at.as_str(), paused), ("R2", None), "{log}");
    assert_eq!(sent_back(&e), 0, "{log}");
    let sql = "SELECT json_extract(input, '$.engine.refused') FROM rounds WHERE work = ?1 AND cell = 'R1' ORDER BY n";
    let refused: Option<String> = e.db.conn().query_row(sql, [work], |r| r.get(0)).unwrap();
    let refused = refused.map(|r| serde_json::from_str::<Value>(&r).unwrap());
    assert_eq!(refused, Some(json!([why])), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A run the command line gave up on because its last attempt held the fields every return holds inside `outcome`,
// which it refused for one missing beside it, is taken from that attempt as it is within its round, with those fields
// moved up beside `outcome`: the round takes its row, and nothing is repaired, handed in again or sent back.
#[test]
fn an_attempt_refused_for_its_shape_is_taken_as_it_is_within_its_round() {
    let (s, mut e, said, item) = unparsed_at_r1("unparsed-inside", "inside", None, None);
    past_r1(&mut e, item);
    let log = said.lock().unwrap().join("\n");
    let (work, at, paused) = state(&e, item).unwrap();
    assert_eq!((at.as_str(), paused), ("R2", None), "{log}");
    assert_eq!(rows_at(&e, work, "R1"), ["done"], "{log}");
    assert!(failed_kinds(&e).is_empty(), "{log}");
    assert_eq!(sent_back(&e), 0, "{log}");
    let (record, _) = repair_record(&e, work);
    assert_eq!(record["as_it_is"], true, "{record}");
    assert!(
        record.get("repair").is_none() && record.get("again").is_none(),
        "{record}"
    );
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
    let log = said.lock().unwrap().join("\n");
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
    let log = said.lock().unwrap().join("\n");
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
    let log = said.lock().unwrap().join("\n");
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
    // The first pass starts the work; the second runs its first agent's round, which fails.
    e.tick().unwrap();
    e.tick().unwrap();
    let log = said.lock().unwrap().join("\n");
    let h = e.halt().unwrap_or_else(|| panic!("no wait:\n{log}"));
    assert_eq!(h.kind, "cannot_run");
    assert!(h.until > db::now());
    let before = rounds_count(&e);
    e.tick().unwrap();
    assert_eq!(rounds_count(&e), before);
    assert_eq!(told(&s, "cannot_run"), 1);
    e.set_halt(e.halt().map(|h| engine::Halt { until: 0, ..h }));
    e.tick().unwrap();
    let log = said.lock().unwrap().join("\n");
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
    let log = said.lock().unwrap().join("\n");
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
    let log = said.lock().unwrap().join("\n");
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
    let busy = Arc::new(Mutex::new(Vec::new()));
    let seen = busy.clone();
    let hooks = Hooks {
        busy: Arc::new(move |w: &[i64]| seen.lock().unwrap().push(w.to_vec())),
        ..Hooks::default()
    };
    let (mut e, said, item) = start_with(&s, scripts, hooks);
    let first = e.step(None).unwrap();
    e.step(first.work).unwrap();
    let log = said.lock().unwrap().join("\n");
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
    assert_eq!(
        *busy.lock().unwrap(),
        [vec![work], vec![], vec![work], vec![], vec![work], vec![]]
    );
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
    let log = said.lock().unwrap().join("\n");
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
    let log = said.lock().unwrap().join("\n");
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
    let log = said.lock().unwrap().join("\n");
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
    let at = state(&e, item).unwrap().1;
    older(&mut e, &[old], |g| renamed(g, &at));
    drop(e);
    let (mut e, _) = engine(&s, scripts());
    assert_eq!(e.step(None).unwrap().work, None);
    assert_eq!(works_of(&e, item).len(), 1);
    let stepped = e.step_work(old).unwrap();
    let log = said.lock().unwrap().join("\n");
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

// The owner starts a work over while a round of it runs, after pausing it: what the round hands in is not taken, and the
// takeover cuts the round off with the owner's reason, what it cost kept on it. The work ends as taken over, saying the
// owner started it over, as the group's notice says too; its question to the architect leaves the queue, its ask the
// owner answered keeps the owner's answer and the other needs none, and its decision card is replaced with the owner's
// ruling kept. A new work on the item starts at the first cell, paused as the owner had paused the old one: it takes
// over the branch, the worktree with what the cut-off round left uncommitted in it, and the pull request, and its cell
// `R1` is told what exists, the old work's decision cards and asks with their rulings and answers included. The work
// over and a work of the architect cannot be started over.
#[test]
fn the_owner_starts_a_work_over_in_the_middle_of_a_round() {
    let s = setup("start-over");
    let mut agents = scripts();
    let implement = agents.remove("I1").unwrap();
    let path = config::database(&s.data);
    // The owner pauses the work and starts it over while its second round at cell `I1` runs, which leaves a file it
    // does not commit.
    agents.insert(
        "I1",
        Box::new(move |r: &Request, n: usize| {
            if n == 2 {
                write(&r.worktree, "notes.txt", "half done\n");
                let mut db = db::Db::open(&path).unwrap();
                let work: i64 = db
                    .conn()
                    .query_row(
                        "SELECT id FROM works WHERE graph = 'engineer' AND ended IS NULL",
                        [],
                        |x| x.get(0),
                    )
                    .unwrap();
                api::pause(&mut db, work, None).unwrap();
                assert_eq!(
                    api::start_over(&mut db, work).unwrap(),
                    format!("work {work} is started over")
                );
            }
            implement(r, n)
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    for _ in 0..80 {
        e.step(None).unwrap();
        if works_of(&e, item)[0].4.is_some() {
            break;
        }
    }
    let log = said.lock().unwrap().join("\n");
    let (old, _, worktree, branch, pr, _) = works_of(&e, item)[0].clone();
    assert!(pr.is_some() && worktree.is_some() && branch.is_some(), "{log}");
    // It put two asks to the owner, a decision card and a question to the architect; the owner answers one ask and rules
    // on the card.
    let (asks, card) =
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
            tx.execute(
                "INSERT INTO design_cards (work, round, key, body, status) VALUES (?1, ?2, 'name', ?3, 'open')",
                params![
                    old,
                    round,
                    json!({ "key": "name", "question": "Which name?", "options": [{ "name": "Old" }, { "name": "New" }],
                            "recommended": "Old" })
                    .to_string()
                ],
            )?;
            let card = tx.last_insert_rowid();
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
            Ok((asks, card))
        })
        .unwrap();
    let answer = api::Answer {
        text: "Keep the old name.".into(),
        speaker: "owner".into(),
        why: None,
        reply: None,
        to: None,
        images: Vec::new(),
    };
    api::answer(&mut e.db, asks[0], &answer).unwrap();
    let ruling = api::Ruling {
        verdict: "disagree".into(),
        option: None,
        text: Some("Name it after the feature.".into()),
    };
    api::rule(&mut e.db, card, &ruling).unwrap();
    // The owner's step of the work runs its round, in which the owner starts it over.
    let stepped = e.step_work(old).unwrap();
    let log = said.lock().unwrap().join("\n");
    let works = works_of(&e, item);
    assert_eq!(works.len(), 2, "{works:?}\n{log}");
    let why = "stopped: the owner started it over, and a new work on it takes the item over";
    assert_eq!(works[0].5.as_deref(), Some(why), "{log}");
    let (n, cut, usage): (i64, String, Option<String>) =
        e.db.conn()
            .query_row(
                "SELECT n, json_extract(rejected, '$.interrupted'), usage FROM rounds
                 WHERE work = ?1 AND cell = 'I1' AND transition IS NULL",
                [old],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
    assert_eq!(stepped.rounds, [engine::RoundId::new(old, n)], "{log}");
    assert_eq!(cut, why);
    assert!(usage.is_some_and(|u| u.contains("cost_usd")));
    let notice: String =
        e.db.conn()
            .query_row(
                "SELECT text FROM engine_notices WHERE work = ?1 ORDER BY id DESC LIMIT 1",
                [old],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(
        notice,
        format!("Work {old} stopped: the owner started it over, and a new work on it takes the item over.")
    );
    let withdrawn: String =
        e.db.conn()
            .query_row("SELECT withdrawn FROM architect_matters WHERE work = ?1", [old], |r| {
                r.get(0)
            })
            .unwrap();
    assert_eq!(withdrawn, format!("the work that raised it ended ({why})"));
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
    let (status, rulings): (String, String) =
        e.db.conn()
            .query_row(
                "SELECT status, rulings FROM item_design_cards WHERE work = ?1",
                [old],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
    assert_eq!(status, "replaced");
    assert!(rulings.contains("Name it after the feature."), "{rulings}");
    // The new work is at the first cell, paused as the old one was.
    let new = works[1].0;
    assert_eq!(works[1].1, "S1");
    assert_eq!(paused_by(&e, new).as_deref(), Some("owner"));
    // Stepped, it takes over the branch, the worktree with what was left in it, and the pull request, and is told what
    // exists.
    e.step_work(new).unwrap();
    e.step_work(new).unwrap();
    e.step_work(new).unwrap();
    let log = said.lock().unwrap().join("\n");
    let works = works_of(&e, item);
    assert_eq!(
        (&works[1].2, &works[1].3, works[1].4),
        (&worktree, &branch, pr),
        "{log}"
    );
    let left = std::fs::read_to_string(PathBuf::from(worktree.unwrap()).join("notes.txt")).unwrap();
    assert_eq!(left, "half done\n");
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
        format!("when it ended: {why}."),
        format!("the views `item_design_cards` and `item_asks` with `work = {old}`"),
        "what is not committed is `git status`".to_string(),
        format!("pr view {} --comments", pr.unwrap()),
        format!("- Work {old} asked the architect, and had no answer when it ended, so the question was withdrawn"),
    ] {
        assert!(m.contains(&want), "{want}\n{m}");
    }
    // The work over cannot be started over again, nor can a work of the architect.
    assert_eq!(api::start_over(&mut e.db, old), Err(format!("work {old} has ended")));
    let v = e.flow_now;
    let arch: i64 =
        e.db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO works (flow_version, graph, at, project) VALUES (?1, 'architect', 'Q0', 'p')",
                [v],
            )?;
            Ok(tx.last_insert_rowid())
        })
        .unwrap();
    assert_eq!(
        api::start_over(&mut e.db, arch),
        Err(format!(
            "work {arch} is the architect's: only a work on an item can be started over"
        ))
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// While every work is paused, the owner's start over of a work the owner started is carried out at once, and the
// owner's Start goes on in the new work, which goes on as the old one did.
#[test]
fn the_owner_s_start_goes_on_in_the_work_that_starts_a_work_over() {
    let s = setup("start-over-started");
    let (mut e, said, item) = start(&s, scripts());
    let c = api::Control::default();
    let work = e.create_work(item).unwrap();
    c.start(api::Fresh::Work(work), false);
    assert_eq!(
        turns(&mut e, &c, 2),
        [work, work],
        "{}",
        said.lock().unwrap().join("\n")
    );
    api::start_over(&mut e.db, work).unwrap();
    let moved = turns(&mut e, &c, 1);
    let log = said.lock().unwrap().join("\n");
    let works = works_of(&e, item);
    assert_eq!(works.len(), 2, "{works:?}\n{log}");
    let new = works[1].0;
    assert!(works[0].5.is_some(), "{log}");
    assert_eq!(moved, [new], "{log}");
    let asked = c.asked();
    assert!(
        asked.started.contains(&new) && !asked.started.contains(&work),
        "{asked:?}"
    );
    assert_eq!(paused_by(&e, new), None);
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

// The owner's words start a work of project p's architect, which goes once through the whole graph: it starts, finds
// its new session has not understood the project, understands it, which queues the check of the policy and the plan of
// what to do next at the head of the queue, and takes the owner's words first, in one round: it replies, in the owner's
// one-to-one, and hands in an item change, which is collected; then it checks the policy and plans what to do next;
// the batch is done, its review finds the change sound, and the engine makes it as the architect; the rethink goes on,
// nothing is left, and the work ends. A field the path cell may leave out and wrote as null is taken as left out.
#[test]
fn an_architect_work_goes_through_understanding_a_matter_and_its_review() {
    let s = setup("architect-work");
    let mut agents = scripts();
    let change = json!({ "kind": "edit_item", "item": "1", "part": "title", "text": "Parse the input first",
                         "reason": "Clearer." });
    let given = change.clone();
    agents.insert(
        "H8",
        Box::new(move |_: &Request, _: usize| {
            (
                vec![("outcome.answer", "done")],
                vec![
                    ("outcome.answer.reply", json!("I retitle it as an action.")),
                    (
                        "outcome.effects",
                        json!({ "item_changes": [given.clone()], "observation_changes": null }),
                    ),
                ],
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
    db::say_to_architect(
        &mut e.db,
        "p",
        None,
        None,
        "Make the parser's item clearer.",
        &[],
        db::Speaker::Owner,
    )
    .unwrap();
    let started = e.step(None).unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(started.cell.as_deref(), Some("S1"), "{log}");
    let arch = started.work.unwrap();
    let project: Option<String> =
        e.db.conn()
            .query_row("SELECT project FROM works WHERE id = ?1", [arch], |r| r.get(0))
            .unwrap();
    assert_eq!(project.as_deref(), Some("p"));
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    assert_eq!(
        path_of(&e, arch),
        [
            "S1", "U1", "Q0", "Q2", "H8", "B1", "Q0", "Q2", "P1", "B1", "Q0", "Q2", "N1", "B1", "B5", "B2", "B4", "Q0",
            "E2"
        ],
        "{log}"
    );
    let rows: Vec<Option<String>> = cells_of(&e, arch).into_iter().map(|c| c.1).collect();
    assert_eq!(rows[0].as_deref(), Some("not_understood"));
    assert_eq!(rows[2].as_deref(), Some("owner_words"));
    assert_eq!(rows[4].as_deref(), Some("done"));
    assert_eq!(
        rows[6..9].iter().map(|r| r.as_deref()).collect::<Vec<_>>(),
        [Some("head"), Some("to_policy_check"), Some("holds")]
    );
    assert_eq!(
        rows[10..13].iter().map(|r| r.as_deref()).collect::<Vec<_>>(),
        [Some("head"), Some("to_plan"), Some("fits")]
    );
    // The batch's end goes to the recording of the owner's decisions, which has none and sends the change collected to
    // its review.
    assert_eq!(rows[13].as_deref(), Some("decisions"));
    assert_eq!(rows[14].as_deref(), Some("changes"));
    assert_eq!(rows[15].as_deref(), Some("ok"));
    // Its reply is its message in the owner's one-to-one.
    let replied: String =
        e.db.conn()
            .query_row(
                "SELECT text FROM conversation_entries WHERE conversation = 'architect:p' AND source = 'message'
                 AND sender = 'architect' ORDER BY id DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(replied, "I retitle it as an action.");
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
    // The review is given the owner's words the change came from, whole, which in the sixth trial it had only the
    // first line of and could not check a change's reason against.
    let looked = message_at(&e, arch, "B2", false);
    let words = given_value(&looked, "·engine·words_behind_changes").expect(&looked);
    let first = words
        .as_object()
        .and_then(|o| o.values().next())
        .cloned()
        .expect(&looked);
    assert_eq!(first["words"], "Make the parser's item clearer.", "{words}");
    // It is given each item a change touches in full, as its creator wrote it, so that it checks a change against the
    // facts it rests on, and the levels an item may be at, each with its model and the work it is for.
    let touched = given_value(&looked, "·engine·changes_touch").expect(&looked);
    assert_eq!(touched["#1"]["item"]["problem"], "No parser.", "{touched}");
    let levels = given_value(&looked, "·engine·levels").expect(&looked);
    assert_eq!(levels, json!([{ "level": "l", "model": "m", "for": "all work" }]));
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
    // The engineer's graph has five lines, the architect's three.
    assert_eq!(lines, 8);
    let _ = std::fs::remove_dir_all(&s.root);
}

// A batch that handled no matter and changed no item, as when its one matter was parked for the owner's reply, ends with
// no round of the rethink: in the sixth trial the rethink ran an agent's round on such a batch, which changed nothing.
#[test]
fn a_batch_that_changed_nothing_ends_with_no_rethink() {
    let s = setup("nothing-to-rethink");
    no_reply_wait(&s);
    let mut agents = scripts();
    agents.insert(
        "H8",
        Box::new(|_: &Request, n: usize| {
            if n == 2 {
                return asks_the_owner("Which parser goes first?");
            }
            (Vec::new(), Vec::new())
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    say(&mut e, "Look at the queue.");
    let first = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, first)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    say(&mut e, "Ask me which parser goes first.");
    let second = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 30, |e| architect_work_done(e, second)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    assert_eq!(
        path_of(&e, second),
        ["S1", "Q0", "Q2", "H8", "A3", "B1", "Q0", "E2"],
        "{log}"
    );
    assert_eq!(rows_at(&e, second, "B1"), ["nothing"], "{log}");
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
        said.lock().unwrap().join("\n")
    );
    say(&mut e, "Look again.");
    let second = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 30, |e| architect_work_done(e, second)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    assert_eq!(
        path_of(&e, second),
        ["S1", "Q0", "Q2", "H8", "B1", "B5", "B4", "Q0", "E2"],
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
/// no round of it begins in the millisecond they were said in. Gives the works' ids, oldest first.
fn architect_works_in(
    name: &str,
    agents: BTreeMap<&'static str, Script>,
    talks: &[(i64, &str)],
) -> (Setup, Engine, Arc<Mutex<Vec<String>>>, Vec<i64>) {
    let s = setup(name);
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    let mut works = Vec::new();
    for (i, (age, words)) in talks.iter().enumerate() {
        if i > 0 {
            age_architect(&mut e, *age);
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

/// Two works of project p's architect, the owner's `first` words starting the first and `second` words the second,
/// said `age` milliseconds after the first work ended (the first work and what it was told made that much older).
fn two_architect_works(
    name: &str,
    age: i64,
    first: &str,
    second: &str,
) -> (Setup, Engine, Arc<Mutex<Vec<String>>>, i64, i64) {
    let (s, e, said, works) = architect_works_in(name, scripts(), &[(0, first), (age, second)]);
    (s, e, said, works[0], works[1])
}

/// The scripts of a test of the understanding's rounds: the project's first understanding answers at once; its second
/// round of the understanding in the test, the first round of a later session's, asks for tools, and the round after
/// it answers.
fn understanding_asks_for_tools_once() -> BTreeMap<&'static str, Script> {
    let mut agents = scripts();
    agents.insert(
        "U1",
        Box::new(|_: &Request, n: usize| {
            if n == 2 {
                let tools = json!([{ "tool": "query", "sql": "SELECT id FROM items" }]);
                return (vec![("outcome", "tools")], vec![("outcome.tools", tools)]);
            }
            (Vec::new(), Vec::new())
        }),
    );
    agents
}

/// The hash and whether it was given of the talk (`owner_talk`) a round's input records, if it records one.
fn talk_recorded(input: &Value) -> Option<(String, bool)> {
    input["conditional"]
        .as_array()?
        .iter()
        .find(|m| m["address"].as_str().is_some_and(|a| a.ends_with("·engine·owner_talk")))
        .map(|m| {
            (
                m["hash"].as_str().unwrap_or_default().to_string(),
                m["given"].as_bool().unwrap_or(false),
            )
        })
}

/// The number of each round of a work at a cell, oldest first.
fn rounds_numbered(e: &Engine, work: i64, cell: &str) -> Vec<i64> {
    let mut st =
        e.db.conn()
            .prepare("SELECT n FROM rounds WHERE work = ?1 AND cell = ?2 ORDER BY n")
            .unwrap();
    st.query_map(params![work, cell], |r| r.get(0))
        .unwrap()
        .map(|x| x.unwrap())
        .collect()
}

/// The input a work's round of a given number was sent.
fn input_numbered(e: &Engine, work: i64, n: i64) -> Value {
    let input: String =
        e.db.conn()
            .query_row(
                "SELECT input FROM rounds WHERE work = ?1 AND n = ?2",
                params![work, n],
                |r| r.get(0),
            )
            .unwrap();
    db::put_back(e.db.conn(), &serde_json::from_str(&input).unwrap())
}

// A work of the architect whose project's previous architect work ended 90 minutes ago, a session whose last call the
// CLI's cache has let go, understands the project in a new session, started as expired, and is told what was said
// since the session before began and not what was said before; the same work 30 minutes after the other goes on in
// the session it had, as before.
#[test]
fn a_work_after_the_cache_expired_understands_the_project_in_a_new_session() {
    // The end of each of the words is its own, which the first sentence a matter is named by does not carry.
    let (s, e, said, first, second) = two_architect_works(
        "expired-between",
        90 * 60_000,
        "Alpha talk. ALPHAEND",
        "Beta talk. BETAEND",
    );
    let log = said.lock().unwrap().join("\n");
    let path = path_of(&e, second);
    assert_eq!(&path[..2], ["S1", "U1"], "{path:?}\n{log}");
    let sessions = architect_sessions(&e, first, second);
    assert_eq!(sessions.len(), 2, "{sessions:?}\n{log}");
    assert_eq!(sessions[1].0, "expired", "{sessions:?}");
    assert_ne!(sessions[0].1, sessions[1].1);
    // The new session is told the talk since the session before began: the second words, not the first, which were
    // said before it began.
    let u1 = input_at(&e, second, "U1", false);
    assert_eq!(given_ending(&u1, "·engine·owner_talk"), Some(true), "{u1}");
    let m = u1["message"].as_str().unwrap();
    assert!(m.contains("BETAEND"), "{m}");
    assert!(!m.contains("ALPHAEND"), "{m}");
    let _ = std::fs::remove_dir_all(&s.root);

    let (s, e, said, first, second) = two_architect_works(
        "fresh-between",
        30 * 60_000,
        "Alpha talk. ALPHAEND",
        "Beta talk. BETAEND",
    );
    let log = said.lock().unwrap().join("\n");
    let path = path_of(&e, second);
    assert_eq!(&path[..2], ["S1", "Q0"], "{path:?}\n{log}");
    let sessions = architect_sessions(&e, first, second);
    assert_eq!(sessions[1].0, "carried", "{sessions:?}");
    assert_eq!(sessions[0].1, sessions[1].1);
    let _ = std::fs::remove_dir_all(&s.root);
}

// The first understanding of a project, with no earlier session to cut the talk at, is given the latest
// entries of the one-to-one; over the size limit it is given the latest whole entries and one line saying how many
// earlier ones there are.
#[test]
fn the_first_understanding_is_given_the_latest_whole_entries_of_the_talk() {
    let s = setup("owner-talk-limit");
    let (mut e, said) = engine(&s, scripts());
    quiet(&mut e);
    an_item(&mut e);
    // The end of each long entry is its own, which the first sentence a matter is named by does not carry.
    say(&mut e, "Alpha talk.");
    say(&mut e, &format!("{} GAMMAEND", "g".repeat(35_000)));
    say(&mut e, &format!("{} DELTAEND", "d".repeat(35_000)));
    say(&mut e, "Epsilon talk. EPSILONEND");
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 30, |e| path_of(e, arch).iter().any(|c| c == "U1")),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let u1 = input_at(&e, arch, "U1", false);
    assert_eq!(given_ending(&u1, "·engine·owner_talk"), Some(true), "{u1}");
    let m = u1["message"].as_str().unwrap();
    assert!(
        m.contains("DELTAEND") && m.contains("EPSILONEND"),
        "the latest entries, whole"
    );
    assert!(!m.contains("GAMMAEND"), "an entry that does not fit is left out whole");
    assert!(
        m.contains("and 2 earlier entries before these"),
        "{}",
        &m[m.len().saturating_sub(3_000)..]
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// The cache of the architect's session expiring in the middle of a work, with no keep-warm ping to hold it, has the
// next round take the row for a new session, naming the expired cache: the matter in hand goes back to the queue and
// the work understands the project in a new session, started as expired, before it takes the matter again.
#[test]
fn the_cache_expiring_in_the_middle_of_an_architect_work_starts_a_new_session() {
    let s = setup("expired-midway");
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
        said.lock().unwrap().join("\n")
    );
    age_architect(&mut e, 90 * 60_000);
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let path = path_of(&e, arch);
    let back = path.iter().rposition(|c| c == "S1").expect(&log);
    assert_eq!(&path[back - 1..back + 3], ["H8", "S1", "U1", "Q0"], "{path:?}\n{log}");
    let why: String =
        e.db.conn()
            .query_row(
                "SELECT json_extract(input, '$.engine.new_session') FROM rounds
                 WHERE work = ?1 AND transition = 'session_anew'",
                [arch],
                |r| r.get(0),
            )
            .unwrap();
    assert!(why.contains("prompt cache expired"), "{why}");
    let lines = line_sessions(&e, arch, "architect");
    assert_eq!(lines.len(), 2, "{lines:?}\n{log}");
    assert_eq!(lines[1].0, "expired", "{lines:?}");
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

// An engineer's question to its project's architect is a matter of its queue, which starts a work of the architect; the
// engineer's work waits at its question, holding no slot. The architect finds the crux, the question going back to the
// queue between, weighs the ways and replies, and the reply takes the work on from there unchanged.
#[test]
fn an_engineer_s_question_is_answered_through_its_crux_and_ways() {
    let s = setup("question");
    let mut agents = scripts();
    agents.insert(
        "R1",
        Box::new(|_: &Request, n: usize| {
            if n == 1 {
                return (
                    vec![("outcome", "ask"), ("outcome.ask", "architect")],
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
    let log = said.lock().unwrap().join("\n");
    assert_eq!(matters(&e), [("engineer_question".to_string(), true, false)], "{log}");
    let (detail, project, what): (Option<String>, String, String) =
        e.db.conn()
            .query_row(
                "SELECT detail, project, what FROM architect_matters WHERE kind = 'engineer_question'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
    // A question carries no kind: the matter says what it asks, and its detail is none.
    assert_eq!((detail, project.as_str()), (None, "p"));
    assert!(what.ends_with("asks: The work holds two separate changes."), "{what}");
    assert_eq!(e.free_slots().unwrap(), 1, "{log}");
    let started = e.step(None).unwrap();
    let arch = started.work.unwrap();
    let replied = steps_until(&mut e, 60, |e| {
        question(e).is_some_and(|q| q.1.is_some()) && architect_work_done(e, arch)
    });
    let log = said.lock().unwrap().join("\n");
    assert!(replied, "{log}");
    let path = path_of(&e, arch);
    let at = path.iter().position(|c| c == "H2").expect(&log);
    // The crux found, the question goes back to the queue and on to the ways, which reply.
    assert_eq!(&path[at - 1..at + 5], ["Q2", "H2", "Q0", "Q2", "H3", "B1"], "{log}");
    // The ways were given the crux by its address in the session, not again.
    let ways = message_at(&e, arch, "H3", false);
    assert!(ways.contains("·H2·crux"), "{ways}");
    assert!(
        ways.contains("What the question needs, judge from what it says"),
        "{ways}"
    );
    assert!(!ways.contains("What is asked is whether to split the work"), "{ways}");
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
/// architect's ways (`H3`) and any other cell scripted as given, run until the work of the architect it started ended
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

// An engineer that finds the system lacks what its work needs, a tool that is not on the machine, asks its architect:
// the question is a capability gap, which the ways weigh by their case of it, unblocking the work first; here by a way
// around within the rules, said in the reply, which takes the work on at once, the gap itself recorded as an
// observation in the same round. The work is told in one line to its engineer, which needs no answer, what became of
// the gap, the observation by its number and title, so that it does not report it again.
#[test]
fn a_capability_gap_is_unblocked_by_a_way_around_and_its_work_told_what_became_of_it() {
    let s = setup("gap-around");
    let ways: Script = Box::new(|_: &Request, _: usize| {
        let new = json!({ "kind": "new", "title": "No jq on the machine", "what": "The machine has no jq.",
                          "tags": [], "source": "A work's question.", "refs": [], "reason": "Worth keeping." });
        (
            Vec::new(),
            vec![
                (
                    "outcome.reply",
                    json!({ "text": "Use `python3 -m json.tool` instead.", "why": "It is on the machine." }),
                ),
                ("outcome.effects", json!({ "observation_changes": [new] })),
            ],
        )
    });
    let ask = asks_about_a_gap("The `jq` command is not on the machine.");
    let (e, said, _, work, arch) = a_gap_asked(&s, ask, vec![("H3", ways)]);
    let log = said.lock().unwrap().join("\n");
    let ways = message_at(&e, arch, "H3", false);
    assert!(
        ways.contains("A capability gap, the system lacking what the work needs"),
        "{ways}"
    );
    assert_eq!(question_took(&e, work).as_deref(), Some("answered"), "{log}");
    let id: i64 =
        e.db.conn()
            .query_row(
                "SELECT id FROM observations WHERE title = 'No jq on the machine'",
                [],
                |r| r.get(0),
            )
            .unwrap();
    let told = lines_told(&e, work);
    assert_eq!(told.len(), 1, "{told:?}\n{log}");
    assert!(
        told[0].starts_with("About your question at cell R1 (\"The `jq` command is not on the machine\"): ")
            && told[0].ends_with(&format!(
                ": recorded as observation O{id} \"No jq on the machine\". You need not report it again."
            )),
        "{told:?}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// An action only the owner can do, a credential the data directory lacks, the architect puts to the owner by pausing
// the work, saying what the owner must do and where, and replying that it goes on once the owner has done it: the work
// holds at its question with the reply waiting, paused by the architect, and nothing is asked of the owner; the ask
// carries no kind on its way. Once the owner starts it, which tells the architect, the work takes the reply and goes on
// to choose its way on.
#[test]
fn an_action_only_the_owner_can_do_pauses_the_work_until_the_owner_starts_it() {
    let s = setup("gap-owner");
    let ways: Script = Box::new(|_: &Request, _: usize| {
        let pause = json!({ "kind": "pause_work", "item": 1,
                            "reason": "Put the registry token in the data directory, then start the work." });
        let reply = json!({
            "text": "The work goes on once the owner has put the registry token in the data directory and started it.",
            "why": "A credential is the owner's to give.",
        });
        (
            Vec::new(),
            vec![
                ("outcome.reply", reply),
                ("outcome.effects", json!({ "actions": [pause] })),
            ],
        )
    });
    let ask = asks_about_a_gap("There is no token to read the registry with.");
    let (mut e, said, item, work, _) = a_gap_asked(&s, ask, vec![("H3", ways)]);
    assert_eq!(item, 1);
    let log = said.lock().unwrap().join("\n");
    let paused: (Option<String>, Option<String>) =
        e.db.conn()
            .query_row("SELECT paused, paused_by FROM works WHERE id = ?1", [work], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
    assert_eq!(paused.1.as_deref(), Some("engine"), "{log}");
    assert!(
        paused
            .0
            .as_deref()
            .is_some_and(|p| p.starts_with(db::STOPPED_BY_ARCHITECT)
                && p.ends_with("Put the registry token in the data directory, then start the work.")),
        "{paused:?}"
    );
    assert_eq!(question_took(&e, work), None, "{log}");
    let asks: i64 =
        e.db.conn()
            .query_row("SELECT count(*) FROM owner_asks WHERE work = ?1", [work], |r| r.get(0))
            .unwrap();
    assert_eq!(asks, 0, "{log}");
    let detail: Option<String> =
        e.db.conn()
            .query_row(
                "SELECT detail FROM architect_matters WHERE kind = 'engineer_question'",
                [],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(detail, None, "{log}");
    // The owner has done it, and starts the work.
    let c = api::Control::default();
    c.set_paused(true);
    c.start(api::Fresh::Work(work), false);
    for _ in 0..10 {
        if question_took(&e, work).is_some() {
            break;
        }
        turn(&mut e, &c);
    }
    let log = said.lock().unwrap().join("\n");
    assert_eq!(question_took(&e, work).as_deref(), Some("answered"), "{log}");
    assert_eq!(paused_by(&e, work), None, "{log}");
    let told: String =
        e.db.conn()
            .query_row(
                "SELECT body ->> 'text' FROM architect_matters WHERE kind = 'owner_words'",
                [],
                |r| r.get(0),
            )
            .unwrap();
    assert!(
        told.starts_with(&format!("I started work {work}, which you had stopped")),
        "{told}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// A choice only the owner can make about a work, accepting a measured slowdown, the architect has put as an open
// question of the work's design: its reply has the engineer say so there, with the options and the one it recommends,
// and go on with that option, the merge waiting for the answer. Nothing is asked of the owner yet, and the work is not
// paused: it takes the reply at once.
#[test]
fn a_choice_only_the_owner_can_make_is_an_open_question_and_the_work_goes_on() {
    let s = setup("owner-choice");
    let ways: Script = Box::new(|_: &Request, _: usize| {
        let reply = json!({
            "text": "Only the owner can accept the slowdown: put it to the owner as an open question of your design, with \
                     the options and the one you recommend, and go on with that option; the merge waits for the answer.",
            "why": "Accepting a slowdown is the owner's choice.",
        });
        (Vec::new(), vec![("outcome.reply", reply)])
    });
    let ask = asks_about_a_gap("The change makes parsing 3% slower, measured over 30 pairs.");
    let (e, said, _, work, arch) = a_gap_asked(&s, ask, vec![("H3", ways)]);
    let log = said.lock().unwrap().join("\n");
    assert_eq!(question_took(&e, work).as_deref(), Some("answered"), "{log}");
    assert_eq!(paused_by(&e, work), None, "{log}");
    let asks: i64 =
        e.db.conn()
            .query_row("SELECT count(*) FROM owner_asks WHERE work = ?1", [work], |r| r.get(0))
            .unwrap();
    assert_eq!(asks, 0, "{log}");
    let ways = message_at(&e, arch, "H3", false);
    assert!(
        ways.contains("put to the owner as an open question of its design"),
        "{ways}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// A way on chosen that does not fit the situation is a flow anomaly, which goes to the architect as a gap of the flow:
// the task's blocker, raised where the way was chosen, is held by the architect at the question's cell, which queues the
// anomaly for it, saying what the engine found; its reply brings the work back to choose again, and nothing is asked of
// the owner.
#[test]
fn a_flow_anomaly_at_choosing_the_way_on_reaches_the_architect() {
    let s = setup("anomaly");
    let mut agents = scripts();
    agents.insert(
        "R1",
        Box::new(|_: &Request, n: usize| {
            if n == 1 {
                an_ask("architect")
            } else {
                (Vec::new(), Vec::new())
            }
        }),
    );
    agents.insert(
        "H3",
        Box::new(|_: &Request, n: usize| {
            let text = if n == 1 {
                "Go on as you planned."
            } else {
                "There is no design yet: carry on researching."
            };
            (
                Vec::new(),
                vec![("outcome.reply", json!({ "text": text, "why": "It holds." }))],
            )
        }),
    );
    // Still researching, the first way on chosen is to change a design there is not yet.
    agents.insert(
        "A4",
        Box::new(|_: &Request, n: usize| {
            let next = if n == 1 { "redesign" } else { "carry_on" };
            (vec![("outcome.next", next)], Vec::new())
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    let questions = |e: &Engine| -> Vec<String> {
        engine::query_json(
            e.db.conn(),
            "SELECT what FROM architect_matters WHERE kind = 'engineer_question' AND item = ?1 ORDER BY id",
            &[&item],
            10,
        )
        .unwrap()
        .iter()
        .map(|r| r["what"].as_str().unwrap().to_string())
        .collect()
    };
    let chose_again = |e: &mut Engine| -> bool {
        state(e, item)
            .is_some_and(|(w, _, _)| cells_of(e, w).iter().filter(|c| c.0 == "A4" && c.1.is_some()).count() >= 2)
    };
    assert!(
        steps_until(&mut e, 150, chose_again),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let work = state(&e, item).unwrap().0;
    let cells = cells_of(&e, work);
    let at = cells
        .iter()
        .position(|c| c == &("A4".to_string(), Some("redesign_in_research".to_string())))
        .expect(&log);
    assert_eq!(cells[at + 1].0, "A2", "{cells:?}");
    let asked = questions(&e);
    assert_eq!(asked.len(), 2, "{asked:?}\n{log}");
    assert!(asked[1].contains("met a flow anomaly at cell A4"), "{asked:?}");
    let (cell, holder): (String, String) =
        e.db.conn()
            .query_row(
                "SELECT cell, holder FROM blockers WHERE work = ?1 ORDER BY id DESC LIMIT 1",
                [work],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
    assert_eq!((cell.as_str(), holder.as_str()), ("A4", "architect"), "{log}");
    let asks: i64 =
        e.db.conn()
            .query_row("SELECT count(*) FROM owner_asks WHERE work = ?1", [work], |r| r.get(0))
            .unwrap();
    assert_eq!(asks, 0, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A work standing at the owner's cell of an older flow version, waiting for its ask of the owner, cannot move to the
// version the engine runs, which has no such cell: a new work takes its item over when it next goes on, and the old
// work's ending closes its open ask, saying no answer is needed.
#[test]
fn a_work_at_the_owner_s_cell_of_an_older_version_is_taken_over_and_its_ask_closed() {
    let s = setup("owner-cell-gone");
    let (mut e, said, item, work) = at_r2_on_an_older_version(&s, |g| {
        // The older version still had the owner's cell.
        let mut owner = g.cell("A2").unwrap().clone();
        owner.id = "A3".into();
        owner.kind = "owner".into();
        owner.title = "Owner answers".into();
        owner.step = Some("ask_owner".into());
        g.cells.push(owner);
    });
    let ask =
        e.db.write(Actor::Engine, |tx| {
            tx.execute("UPDATE works SET at = 'A3', resume_at = 'R2' WHERE id = ?1", [work])?;
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
    e.step(None).unwrap();
    let log = said.lock().unwrap().join("\n");
    let works = works_of(&e, item);
    assert_eq!(works.len(), 2, "{works:?}\n{log}");
    assert!(
        works[0].5.as_deref().is_some_and(|w| w.contains("cell A3 is gone")),
        "{works:?}"
    );
    let by: Option<String> =
        e.db.conn()
            .query_row(
                "SELECT json_extract(answer, '$.by') FROM owner_asks WHERE id = ?1",
                [ask],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(by.as_deref(), Some("engine"), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A capability gap with no way around that the owner need not fill is fixed by an item: the ways create it, have the
// work's item depend on it, blocking, and name it in the reply's waits. Once the review of the batch lets it be made,
// the reply goes with the item's number and the work stops to wait for it to merge; the reply says what became of the
// gap, so that the work is told nothing beside it.
#[test]
fn a_capability_gap_with_no_way_around_has_its_work_wait_for_the_item_that_fixes_it() {
    let s = setup("gap-fix");
    let ways: Script = Box::new(|_: &Request, _: usize| {
        let changes = json!([
            { "kind": "create_item", "key": "add-jq", "project": "p", "title": "Install jq",
              "problem": "There is no jq on the machine.", "why": "Works need it.", "acceptance": ["jq runs."],
              "level": "l", "deliver": "change", "reason": "The gap has no way around." },
            { "kind": "add_dependency", "item": "1", "on": "add-jq", "dependency": "blocking",
              "reason": "The work cannot go on without jq." },
        ]);
        let reply = json!({
            "text": "Wait for the item that installs jq.", "why": "There is no way around it.",
            "waits": [{ "key": "add-jq", "title": "Install jq", "delivers": "jq on the machine" }],
        });
        (
            Vec::new(),
            vec![
                ("outcome.reply", reply),
                ("outcome.effects", json!({ "item_changes": changes })),
            ],
        )
    });
    let review: Script = Box::new(|_: &Request, _: usize| {
        let ok = |n: i64| json!({ "change": n, "verdict": "ok", "why": "Sound." });
        (Vec::new(), vec![("outcome.review", json!([ok(1), ok(2)]))])
    });
    let way_on: Script = Box::new(|_: &Request, _: usize| (vec![("outcome.next", "wait_for_items")], Vec::new()));
    let ask = asks_about_a_gap("The `jq` command is not on the machine.");
    let (mut e, said, item, work, _) = a_gap_asked(&s, ask, vec![("H3", ways), ("B2", review), ("A4", way_on)]);
    assert_eq!(item, 1);
    let log = said.lock().unwrap().join("\n");
    assert_eq!(question_took(&e, work).as_deref(), Some("answered"), "{log}");
    let fix: i64 =
        e.db.conn()
            .query_row("SELECT id FROM items WHERE title = 'Install jq'", [], |r| r.get(0))
            .unwrap();
    let waits: String =
        e.db.conn()
            .query_row(
                "SELECT json_extract(output, '$.reply.waits[0].key') FROM rounds
                 WHERE work = ?1 AND cell = 'A2' AND transition IS NOT NULL",
                [work],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(waits, fix.to_string(), "{log}");
    assert!(steps_until(&mut e, 20, |e| state(e, item).is_some_and(|w| w.1 == "A6")));
    let kind: String =
        e.db.conn()
            .query_row(
                "SELECT kind FROM item_links WHERE item = ?1 AND depends_on = ?2",
                params![item, fix],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(kind, "blocking", "{log}");
    assert_eq!(lines_told(&e, work), Vec::<String>::new(), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A side report that the system lacks something is taken in, and the work that handed it in is told what became of it
// in one line to its engineer: in the work's group, as the architect's word to the engineer, and given to the
// engineer's next round, which needs to answer nothing.
#[test]
fn the_work_that_handed_a_side_report_in_is_told_what_became_of_it() {
    let s = setup("side-report-answered");
    let mut agents = scripts();
    agents.insert(
        "R1",
        Box::new(|_: &Request, n: usize| {
            let reports = if n == 1 {
                json!([{ "kind": "other", "what": "There is no `jq` on the machine.", "urgent": false }])
            } else {
                json!([])
            };
            (Vec::new(), vec![("side_reports", reports)])
        }),
    );
    agents.insert(
        "H5",
        Box::new(|_: &Request, _: usize| {
            let new = json!({ "kind": "new", "title": "No jq on the machine", "what": "The machine has no jq.",
                              "tags": [], "source": "A side report.", "refs": [], "reason": "Worth keeping." });
            (
                Vec::new(),
                vec![
                    (
                        "outcome.take",
                        json!({ "way": "new_observation", "why": "No item does it." }),
                    ),
                    ("outcome.effects", json!({ "observation_changes": [new] })),
                ],
            )
        }),
    );
    let (mut e, said, _) = start(&s, agents);
    let work = e.step(None).unwrap().work;
    e.step(work).unwrap();
    let work = work.unwrap();
    // The work stays where it is while the architect takes the report in.
    pause_by_owner(&mut e, work, "Held while the architect looks.");
    let arch = e.step(Some(work)).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let id: i64 =
        e.db.conn()
            .query_row("SELECT id FROM observations", [], |r| r.get(0))
            .unwrap();
    let line = format!(
        "About your side report at cell R1 (\"There is no `jq` on the machine\"): recorded as observation O{id} \
         \"No jq on the machine\". You need not report it again."
    );
    assert_eq!(lines_told(&e, work), std::slice::from_ref(&line), "{log}");
    let group: Vec<(String, String, String)> = engine::query_json(
        e.db.conn(),
        "SELECT sender, recipient, text FROM conversation_entries WHERE conversation = 'work:' || ?1
           AND source = 'interjection'",
        &[&work],
        10,
    )
    .unwrap()
    .iter()
    .map(|r| {
        let t = |k: &str| r[k].as_str().unwrap().to_string();
        (t("sender"), t("recipient"), t("text"))
    })
    .collect();
    assert_eq!(group, [("architect".into(), "engineer".into(), line)], "{log}");
    let_go_on(&mut e, work);
    let delivered = |e: &Engine| -> Option<i64> {
        e.db.conn()
            .query_row(
                "SELECT delivered_round FROM interjections WHERE work = ?1 AND speaker = 'architect'",
                [work],
                |r| r.get(0),
            )
            .unwrap()
    };
    assert!(steps_until(&mut e, 20, |e| delivered(e).is_some()), "{log}");
    let input: String =
        e.db.conn()
            .query_row(
                "SELECT input FROM rounds WHERE work = ?1 AND n = ?2",
                params![work, delivered(&e).unwrap()],
                |r| r.get(0),
            )
            .unwrap();
    let given = db::put_back(e.db.conn(), &serde_json::from_str(&input).unwrap()).to_string();
    assert!(
        given.contains("From the architect: About your side report at cell R1"),
        "{given}"
    );
    // A word that needs no answer turns nothing: the work is not taken to choose its way on.
    assert!(!path_of(&e, work).contains(&"A4".to_string()), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A gap reported a third time is work: a merge that makes an observation no item took in yet reported three times is
// sent back while no change of the batch folds it into an item, and again while that item is not moved ahead; handed in
// folded into a new item moved first, it is taken, and once the batch is made the observation is the item's, the item
// starts first, and the work that reported it is told it was reported again and taken into that item.
#[test]
fn a_gap_reported_a_third_time_is_taken_into_an_item_moved_ahead() {
    let s = setup("gap-recurs");
    let mut agents = scripts();
    agents.insert(
        "R1",
        Box::new(|_: &Request, n: usize| {
            let reports = if n == 1 {
                json!([{ "kind": "other", "what": "There is no `jq` on the machine.", "urgent": false }])
            } else {
                json!([])
            };
            (Vec::new(), vec![("side_reports", reports)])
        }),
    );
    agents.insert(
        "H5",
        Box::new(|_: &Request, n: usize| {
            let merge = json!({ "kind": "merge", "into": 1, "seen": "", "refs": [], "reason": "The same gap." });
            let mut changes = vec![
                json!({ "kind": "create_item", "key": "jq", "project": "p", "title": "Install jq",
                        "problem": "There is no jq on the machine.", "why": "Works keep meeting it.",
                        "acceptance": ["jq runs."], "level": "l", "deliver": "change", "reason": "It recurs." }),
                json!({ "kind": "fold_observations", "item": "jq", "observations": [1], "reason": "It is the gap." }),
            ];
            if n > 2 {
                changes
                    .push(json!({ "kind": "move", "item": "jq", "to": { "place": "first" }, "reason": "It recurs." }));
            }
            let effects = if n == 1 {
                json!({ "observation_changes": [merge] })
            } else {
                json!({ "observation_changes": [merge], "item_changes": changes })
            };
            (
                Vec::new(),
                vec![
                    ("outcome.take", json!({ "way": "merged", "why": "Reported before." })),
                    ("outcome.effects", effects),
                ],
            )
        }),
    );
    agents.insert(
        "B2",
        Box::new(|_: &Request, _: usize| {
            let ok = |n: i64| json!({ "change": n, "verdict": "ok", "why": "Sound." });
            (Vec::new(), vec![("outcome.review", json!([ok(1), ok(2), ok(3)]))])
        }),
    );
    let (mut e, said, _) = start(&s, agents);
    e.db.write(Actor::Architect, |tx| {
        tx.execute(
            "INSERT INTO observations (project, title, what, source, times_reported)
             VALUES ('p', 'No jq', 'The machine has no jq.', 'Two works.', 2)",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    let work = e.step(None).unwrap().work;
    e.step(work).unwrap();
    let work = work.unwrap();
    pause_by_owner(&mut e, work, "Held while the architect looks.");
    let arch = e.step(Some(work)).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 60, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let rejected: Vec<String> = engine::query_json(
        e.db.conn(),
        "SELECT rejected FROM rounds WHERE work = ?1 AND cell = 'H5' AND rejected IS NOT NULL ORDER BY n",
        &[&arch],
        10,
    )
    .unwrap()
    .iter()
    .map(|r| r["rejected"].to_string())
    .collect();
    assert_eq!(rejected.len(), 2, "{rejected:?}\n{log}");
    assert!(
        rejected[0].contains("it makes observation 1 reported 3 times, and an observation reported 3 times is work"),
        "{rejected:?}"
    );
    assert!(
        rejected[1].contains("item jq, which takes it in, is not moved ahead"),
        "{rejected:?}"
    );
    let fix: i64 =
        e.db.conn()
            .query_row("SELECT id FROM items WHERE title = 'Install jq'", [], |r| r.get(0))
            .unwrap();
    let (times, folded): (i64, Option<i64>) =
        e.db.conn()
            .query_row(
                "SELECT times_reported, folded_into FROM observations WHERE id = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
    assert_eq!((times, folded), (3, Some(fix)), "{log}");
    let rank: Option<i64> =
        e.db.conn()
            .query_row("SELECT rank FROM items WHERE id = ?1", [fix], |r| r.get(0))
            .unwrap();
    assert_eq!(rank, Some(1), "{log}");
    assert_eq!(
        lines_told(&e, work),
        [format!(
            "About your side report at cell R1 (\"There is no `jq` on the machine\"): reported once more on \
             observation O1 \"No jq\", 3 times now; taken into new item #{fix} \"Install jq\". You need not report \
             it again."
        )],
        "{log}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// The review of a batch finds an item change to fix: the revision hands in the whole set again, the review looks at it
// again, and the revised change is made; what was handed in first is kept as revised, not made. The revision closes an
// observation the review found done already, at once, as an answer's effects would: in the sixth trial it could hand in
// item changes only, and the review's advice to close two observations went into its notebook, with no cell to do it.
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
                        "what": "The title is no action.", "how": "Write it as one.", "cause": "rule_missing" })
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
            let close = json!({ "kind": "close", "id": 1, "why": "outdated", "text": "Main reads the token already.",
                                "reason": "The review found it done." });
            (
                Vec::new(),
                vec![
                    ("outcome.changes", json!([{ "matter": matter, "change": change }])),
                    ("outcome.observation_changes", json!([close])),
                ],
            )
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO observations (project, what, source) VALUES ('p', 'The lexer drops a token.', 'A report')",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    say(&mut e, "Name the parser's item.");
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
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
    let closed: (String, String) =
        e.db.conn()
            .query_row("SELECT closed, closed_why FROM observations WHERE id = 1", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
    assert_eq!(
        closed,
        ("outdated".to_string(), "Main reads the token already.".to_string()),
        "{log}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
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

/// Sets the installation's wait for the owner's reply to none, so that a question the architect puts waits at the back
/// of its queue at once.
fn no_reply_wait(s: &Setup) {
    let install = std::fs::read_to_string(s.data.join("install.toml")).unwrap();
    write(
        &s.data,
        "install.toml",
        &format!("{install}\n[agents]\nowner_reply_wait = 0\n"),
    );
    git(&s.data, &["commit", "-q", "-am", "no wait for the owner's reply"]);
}

// A question to the owner from a cell on a matter's path parks only that matter: once the wait for the owner's reply
// ran out, the architect's work goes on with the next and ends; the owner's reply puts the matter back in the queue,
// which starts a work of the architect, and it goes back to the cell that asked, which is given the reply.
#[test]
fn an_ask_holds_only_its_matter_and_the_answer_goes_back_to_the_cell_that_asked() {
    let s = setup("ask-holds");
    no_reply_wait(&s);
    let mut agents = scripts();
    picks_first(&s, &mut agents);
    agents.insert(
        "H9",
        Box::new(|_: &Request, n: usize| {
            // Its first round, on the matter picked first, asks the owner.
            if n == 1 {
                return asks_the_owner("Should the release wait for the parser?");
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
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
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
    // The question is pinned for the owner, as the question the cell asked.
    let pinned: (String, String) =
        e.db.conn()
            .query_row(
                "SELECT json_extract(body, '$.kind'), json_extract(body, '$.question') FROM owner_asks WHERE id = ?1",
                [ask],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
    assert_eq!(
        pinned,
        (
            "question".to_string(),
            "Should the release wait for the parser?".to_string()
        )
    );
    // The owner replies with the question ticked: the matter is back, an architect work starts, and the matter goes
    // back to the cell that asked.
    api::reply_to_question(&mut e.db, "p", ask, "Yes, hold it.", &[], db::Speaker::Owner).unwrap();
    assert!(api::reply_to_question(&mut e.db, "p", ask, "Again.", &[], db::Speaker::Owner).is_err());
    let again = e.step(None).unwrap().work.unwrap();
    assert_ne!(again, arch);
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, again)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
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

// A matter back with the owner's reply is taken first, after the owner's words, before the other matters waiting in
// the queue and without a pick: the look at the queue takes it, and the router sends it back to the cell that asked.
#[test]
fn a_matter_back_with_the_owner_s_answer_is_taken_before_the_others_waiting() {
    let s = setup("answered-first");
    no_reply_wait(&s);
    let mut agents = scripts();
    picks_first(&s, &mut agents);
    agents.insert(
        "H9",
        Box::new(|_: &Request, n: usize| {
            // Its first round, on the only matter queued then, asks the owner.
            if n == 1 {
                return asks_the_owner("Should the release wait for the parser?");
            }
            (Vec::new(), Vec::new())
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    let queue = |e: &mut Engine, text: &str| {
        let m = db::Matter {
            project: "p".into(),
            kind: "other_architect".into(),
            detail: Some("message".into()),
            what: format!("The architect of q says: {text}"),
            body: json!({ "text": text }),
            ..db::Matter::default()
        };
        e.db.write(Actor::Engine, |tx| db::queue_matter(tx, &m)).unwrap()
    };
    let held = queue(&mut e, "Hold the release?");
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let ask: i64 =
        e.db.conn()
            .query_row("SELECT asked FROM architect_matters WHERE id = ?1", [held], |r| {
                r.get(0)
            })
            .expect(&log);
    // Two other matters wait in the queue when the owner answers.
    queue(&mut e, "Lift the freeze?");
    queue(&mut e, "Look at the queue.");
    let yes = api::Answer {
        text: "Yes, hold it.".into(),
        speaker: "owner".into(),
        why: None,
        reply: None,
        to: None,
        images: Vec::new(),
    };
    api::answer(&mut e.db, ask, &yes).unwrap();
    let again = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 60, |e| architect_work_done(e, again)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let cells = cells_of(&e, again);
    let at = cells.iter().position(|c| c.0 == "Q0").expect(&log);
    assert_eq!(cells[at].1.as_deref(), Some("answered"), "{cells:?}\n{log}");
    assert_eq!(
        cells[at + 1..at + 3].iter().map(|c| c.0.as_str()).collect::<Vec<_>>(),
        ["Q2", "H9"],
        "{cells:?}\n{log}"
    );
    assert!(message_at(&e, again, "H9", false).contains("Yes, hold it."), "{log}");
    assert_eq!(
        matters(&e),
        vec![("other_architect".to_string(), false, true); 3],
        "{log}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
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

/// The question a matter is parked by.
fn parked_by(e: &Engine, matter: i64) -> i64 {
    e.db.conn()
        .query_row("SELECT asked FROM architect_matters WHERE id = ?1", [matter], |r| {
            r.get(0)
        })
        .unwrap()
}

// A question just put waits at the look at the queue for the owner's reply, before any other matter, and taking the
// wait again records nothing new; the reply, given with the question ticked, comes back at once to the cell that
// asked, in the same work.
#[test]
fn a_question_just_put_waits_at_the_queue_for_the_owner_s_reply() {
    let s = setup("question-waits");
    let mut agents = scripts();
    picks_first(&s, &mut agents);
    agents.insert(
        "H9",
        Box::new(|r: &Request, _: usize| {
            if r.prompt.contains("Hold the release?") && !r.prompt.contains("Yes, hold it.") {
                return asks_the_owner("Should the release wait for the parser?");
            }
            (Vec::new(), Vec::new())
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    let held = from_q(&mut e, "Hold the release?");
    from_q(&mut e, "Look at the queue.");
    let arch = e.step(None).unwrap().work.unwrap();
    let waits = |e: &mut Engine| {
        cells_of(e, arch)
            .last()
            .is_some_and(|c| c == &("Q0".into(), Some("waits".into())))
    };
    assert!(steps_until(&mut e, 40, waits), "{}", said.lock().unwrap().join("\n"));
    let rounds = cells_of(&e, arch).len();
    assert!(e.step(None).unwrap().rounds.is_empty());
    assert_eq!(cells_of(&e, arch).len(), rounds);
    // The other matter waits behind the question.
    assert_eq!(
        matters(&e),
        [
            ("other_architect".to_string(), false, false),
            ("other_architect".to_string(), true, false)
        ]
    );
    let ask = parked_by(&e, held);
    api::reply_to_question(&mut e.db, "p", ask, "Yes, hold it.", &[], db::Speaker::Owner).unwrap();
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let cells = cells_of(&e, arch);
    let waited = cells.iter().position(|c| c.1.as_deref() == Some("waits")).expect(&log);
    assert_eq!(
        cells[waited + 1..waited + 4]
            .iter()
            .map(|c| (c.0.as_str(), c.1.as_deref()))
            .collect::<Vec<_>>(),
        [
            ("Q0", Some("answered")),
            ("Q2", Some("to_answer_architect")),
            ("H9", Some("done"))
        ],
        "{cells:?}\n{log}"
    );
    assert_eq!(
        matters(&e),
        vec![("other_architect".to_string(), false, true); 2],
        "{log}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// The owner's words said while the architect waits for the reply to a question end the wait: the words are taken, the
// question waits at the back of the queue, blocking nothing, and the work ends with it pending. A reply to a question
// whose matter is gone comes to the architect as the owner's words, with the question.
#[test]
fn the_owner_s_words_end_the_wait_and_a_reply_to_a_closed_question_is_the_owner_s_words() {
    let s = setup("question-words");
    let mut agents = scripts();
    picks_first(&s, &mut agents);
    agents.insert(
        "H9",
        Box::new(|r: &Request, _: usize| {
            if r.prompt.contains("Hold the release?") {
                return asks_the_owner("Should the release wait for the parser?");
            }
            (Vec::new(), Vec::new())
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    let held = from_q(&mut e, "Hold the release?");
    let arch = e.step(None).unwrap().work.unwrap();
    let waits = |e: &mut Engine| {
        cells_of(e, arch)
            .last()
            .is_some_and(|c| c == &("Q0".into(), Some("waits".into())))
    };
    assert!(steps_until(&mut e, 40, waits), "{}", said.lock().unwrap().join("\n"));
    say(&mut e, "What about the parser?");
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let cells = cells_of(&e, arch);
    let waited = cells.iter().position(|c| c.1.as_deref() == Some("waits")).expect(&log);
    assert_eq!(
        cells[waited + 1],
        ("Q0".to_string(), Some("owner_words".to_string())),
        "{log}"
    );
    assert_eq!(cells.last().map(|c| c.0.as_str()), Some("E2"), "{log}");
    // The question still waits, its matter parked.
    let ask = parked_by(&e, held);
    let pending = |e: &Engine| -> i64 {
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM owner_asks a JOIN architect_matters m ON m.asked = a.id
                 WHERE a.answer IS NULL AND m.handled_round IS NULL AND m.withdrawn IS NULL",
                [],
                |r| r.get(0),
            )
            .unwrap()
    };
    assert_eq!(pending(&e), 1);
    // Its matter is withdrawn: a reply to it is the owner's words, which tell the architect the question.
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "UPDATE architect_matters SET withdrawn = 'its item was cancelled' WHERE id = ?1",
            [held],
        )?;
        Ok(())
    })
    .unwrap();
    assert_eq!(pending(&e), 0);
    api::reply_to_question(&mut e.db, "p", ask, "Yes, hold it.", &[], db::Speaker::Owner).unwrap();
    let words: (String, String) =
        e.db.conn()
            .query_row(
                "SELECT detail, json_extract(body, '$.question') FROM architect_matters
                 WHERE kind = 'owner_words' ORDER BY id DESC LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
    assert_eq!(
        words,
        (
            "answer".to_string(),
            "Should the release wait for the parser?".to_string()
        )
    );
    let _ = std::fs::remove_dir_all(&s.root);
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

/// A question project p's architect put to the owner just now, in a work of it over before the test, which parks a
/// message of q's architect at the cell that answers such a message (`H9`); gives the matter and the question.
fn a_question(e: &mut Engine, question: &str) -> (i64, i64) {
    let v = e.flow_now;
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO works (item, flow_version, graph, at, ended, project)
             VALUES (NULL, ?1, 'architect', 'E2', 'over before the test', 'p')",
            [v],
        )?;
        let work = tx.last_insert_rowid();
        record_round(tx, work, 1, "A3", &json!({}), "held")?;
        let m = db::Matter {
            project: "p".into(),
            kind: "other_architect".into(),
            detail: Some("message".into()),
            what: "The architect of q says: Hold the release?".into(),
            body: json!({ "text": "Hold the release?" }),
            ..db::Matter::default()
        };
        let matter = db::queue_matter(tx, &m)?;
        let body = json!({ "from": "architect", "project": "p", "work": work, "cell": "A3", "kind": "question",
                           "question": question, "holds": [matter] });
        tx.execute(
            "INSERT INTO owner_asks (work, round, body) VALUES (?1, 1, ?2)",
            params![work, body.to_string()],
        )?;
        let ask = tx.last_insert_rowid();
        tx.execute(
            "UPDATE architect_matters SET asked = ?2, resume_cell = 'H9' WHERE id = ?1",
            params![matter, ask],
        )?;
        Ok((matter, ask))
    })
    .unwrap()
}

/// Whether a work of the architect's last round waits at the look at the queue for the owner's reply.
fn waits_at_queue(e: &Engine, work: i64) -> bool {
    cells_of(e, work)
        .last()
        .is_some_and(|c| c == &("Q0".into(), Some("waits".into())))
}

/// The question to the owner put last that waits for the reply: its number and its text.
fn question_now(e: &Engine) -> (i64, String) {
    e.db.conn()
        .query_row(
            "SELECT id, json_extract(body, '$.question') FROM owner_asks WHERE answer IS NULL ORDER BY id DESC LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap()
}

// A new session understands the project and queues at the head of its queue the check of the policy, then the plan of
// what to do next, each asked for by the understanding with why; the look at the queue waits first for the owner's
// reply to a question just put, takes the matter back with the reply, and then the two matters at the head of the queue
// in their order, without a pick, before the other matters: the check finds the policy holds and is closed, and the
// plan is made once.
#[test]
fn a_new_session_queues_the_check_and_the_plan_and_takes_them_after_the_wait() {
    let s = setup("head-after-wait");
    let (mut e, said) = engine(&s, scripts());
    quiet(&mut e);
    an_item(&mut e);
    let (_, ask) = a_question(&mut e, "Should the release wait for the parser?");
    from_q(&mut e, "Look at the queue.");
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 20, |e| waits_at_queue(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let understood = "A new session understood the project".to_string();
    assert_eq!(
        at_head(&e),
        [
            (
                "check_policy".to_string(),
                false,
                vec![("U1".to_string(), understood.clone())]
            ),
            ("make_plan".to_string(), false, vec![("U1".to_string(), understood)])
        ]
    );
    api::reply_to_question(&mut e.db, "p", ask, "Yes, hold it.", &[], db::Speaker::Owner).unwrap();
    assert!(
        steps_until(&mut e, 60, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let cells = cells_of(&e, arch);
    let taken: Vec<(&str, Option<&str>)> = cells[..cells.len() - 1]
        .iter()
        .map(|c| (c.0.as_str(), c.1.as_deref()))
        .collect();
    assert_eq!(
        taken,
        [
            ("S1", Some("not_understood")),
            ("U1", Some("understood")),
            ("Q0", Some("waits")),
            ("Q0", Some("answered")),
            ("Q2", Some("to_answer_architect")),
            ("H9", Some("done")),
            ("B1", Some("more")),
            ("Q0", Some("head")),
            ("Q2", Some("to_policy_check")),
            ("P1", Some("holds")),
            ("B1", Some("more")),
            ("Q0", Some("head")),
            ("Q2", Some("to_plan")),
            ("N1", Some("fits")),
            ("B1", Some("more")),
            ("Q0", Some("one")),
            ("Q2", Some("to_answer_architect")),
            ("H9", Some("done")),
            ("B1", Some("done")),
            ("B4", Some("go_on")),
            ("Q0", Some("none"))
        ],
        "{log}"
    );
    assert_eq!(cells.last().map(|c| c.0.as_str()), Some("E2"), "{log}");
    let handled: Vec<(String, bool)> = at_head(&e).into_iter().map(|m| (m.0, m.1)).collect();
    assert_eq!(
        handled,
        [("check_policy".to_string(), true), ("make_plan".to_string(), true)],
        "{log}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

/// Project p, whose constitution is `now`, after a check of its policy that the owner's words asked for, handled where
/// the policy it set took effect, in a work of its architect over before the test, which kept the hash of `kept` as
/// the constitution it was queued under; when `newer`, another policy took effect after it. A new session then
/// understands the project. Gives the matters at the head of the queue once it has.
fn head_after_a_new_session(name: &str, kept: &str, now: &str, newer: bool) -> Vec<AtHead> {
    let s = setup(name);
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
    write(&s.root.join("checkout"), "CONSTITUTION.md", now);
    let (mut e, said) = engine(&s, scripts());
    quiet(&mut e);
    let v = e.flow_now;
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO works (item, flow_version, graph, at, ended, project)
             VALUES (NULL, ?1, 'architect', 'E2', 'over before the test', 'p')",
            [v],
        )?;
        let arch = tx.last_insert_rowid();
        let policy = json!({ "outcome": { "policy": { "direction": "Small steps first." } } });
        record_round(tx, arch, 1, "P2", &policy, "written")?;
        record_round(tx, arch, 2, "P12", &json!({}), "next")?;
        let body = json!({
            "asked_by": [{ "cell": "H8", "round": format!("R{arch}-1"), "why": "The owner asked for it." }],
            "constitution": db::content_hash(kept),
        });
        tx.execute(
            "INSERT INTO architect_matters (project, kind, what, body, at, handled_work, handled_round)
             VALUES ('p', 'check_policy', 'Check whether the policy of p still holds', ?1, ?2, ?3, 2)",
            params![body.to_string(), db::now(), arch],
        )?;
        if newer {
            record_round(tx, arch, 3, "P2", &policy, "written")?;
            record_round(tx, arch, 4, "P12", &json!({}), "next")?;
        }
        Ok(())
    })
    .unwrap();
    from_q(&mut e, "Look at the queue.");
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 20, |e| cells_of(e, arch)
            .iter()
            .any(|c| c.0 == "U1" && c.1.is_some())),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let head = at_head(&e);
    let _ = std::fs::remove_dir_all(&s.root);
    head
}

// A new session's check of the policy is not queued while the policy in force and the constitution are those the last
// check handled had, since it would only come to the same end; its plan of what to do next is. Once the constitution
// changed, or another policy took effect since, the check is queued as before.
#[test]
fn a_new_session_checks_the_policy_only_once_the_policy_or_the_constitution_changed() {
    let constitution = "## Article 1. Small\n\nKeep it small.\n";
    let understood = "A new session understood the project".to_string();
    let last = (
        "check_policy".to_string(),
        true,
        vec![("H8".to_string(), "The owner asked for it.".to_string())],
    );
    let check = (
        "check_policy".to_string(),
        false,
        vec![("U1".to_string(), understood.clone())],
    );
    let plan = ("make_plan".to_string(), false, vec![("U1".to_string(), understood)]);
    assert_eq!(
        head_after_a_new_session("check-same", constitution, constitution, false),
        [last.clone(), plan.clone()]
    );
    let changed = "## Article 1. Small\n\nKeep it smaller.\n";
    assert_eq!(
        head_after_a_new_session("check-constitution", constitution, changed, false),
        [last.clone(), check.clone(), plan.clone()]
    );
    assert_eq!(
        head_after_a_new_session("check-policy", constitution, constitution, true),
        [last, check, plan]
    );
}

// The check of the policy in force, in doubt, asks the owner whether to research a new policy, which parks the check at
// it while the architect waits for the reply; the reply brings it back, the check cannot hand in that the policy holds
// while none is in force, and it goes on to write the new policy by the owner's stance. The engine asks the owner to approve the draft,
// which parks the check at the writing: a reply other than approved brings it back there, to be written again by it,
// and approved, exactly, has it take effect: it is the policy in force, no draft waits any longer, the check is closed,
// and the plan of what to do next, which the new session's understanding queued and the new policy asked for again, is
// made once, under it.
#[test]
fn a_new_policy_is_written_by_the_owner_s_reply_and_takes_effect_once_approved() {
    let s = setup("policy-approved");
    let mut agents = scripts();
    agents.insert(
        "P1",
        Box::new(|_: &Request, n: usize| match n {
            1 => asks_the_owner("Research a new policy? The parser no longer leads the work."),
            2 => (vec![("outcome.verdict", "holds")], Vec::new()),
            _ => (
                vec![("outcome.verdict", "write_policy")],
                vec![("outcome.verdict.stance", json!("Research one led by the parser."))],
            ),
        }),
    );
    // Each draft names the same new measures, one of them twice however spaced, and one the queue holds already.
    let measures = json!([
        { "measure": "How long a merge waits for CI", "signal": "Speed" },
        { "measure": "how long a  merge waits for CI ", "signal": "Speed" },
        { "measure": "Rounds sent back by cell", "signal": "Speed" }
    ]);
    agents.insert("P2", writes_a_policy_with(measures));
    picks_first(&s, &mut agents);
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    from_q(&mut e, "Look at the queue.");
    let arch = e.step(None).unwrap().work.unwrap();
    let waited = |e: &Engine| rows_at(e, arch, "Q0").iter().filter(|r| *r == "waits").count();
    assert!(
        steps_until(&mut e, 30, |e| waits_at_queue(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let (research, _) = question_now(&e);
    api::reply_to_question(&mut e.db, "p", research, "Yes, research it.", &[], db::Speaker::Owner).unwrap();
    assert!(
        steps_until(&mut e, 30, |e| waited(e) == 2 && waits_at_queue(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let (approval, text) = question_now(&e);
    assert_eq!(text, "Approve this policy? Reply exactly: approved");
    let pinned: Value =
        e.db.conn()
            .query_row("SELECT body FROM owner_asks WHERE id = ?1", [approval], |r| {
                r.get::<_, String>(0)
            })
            .map(|b| serde_json::from_str(&b).unwrap())
            .unwrap();
    assert_eq!((&pinned["kind"], &pinned["cell"]), (&json!("question"), &json!("P2")));
    assert!(pinned.get("policy").is_none(), "{pinned}");
    api::reply_to_question(
        &mut e.db,
        "p",
        approval,
        "Make the why shorter.",
        &[],
        db::Speaker::Owner,
    )
    .unwrap();
    assert!(
        steps_until(&mut e, 30, |e| waited(e) == 3 && waits_at_queue(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let (again, _) = question_now(&e);
    assert_ne!(again, approval);
    // No draft queued its new measures: a draft the owner has not approved queues nothing.
    assert!(measures_queued(&e).is_empty(), "{}", said.lock().unwrap().join("\n"));
    e.db.write(Actor::Engine, |tx| {
        let fix = json!({ "where": "The numbers the engine measures", "items": [], "works": [], "kind": "missing",
                          "wrong": "w", "steps": ["Make an item that has the engine measure it"] });
        db::queue_matter(
            tx,
            &db::Matter {
                project: "p".into(),
                kind: "set_straight".into(),
                detail: Some("missing".into()),
                what: "A new measure a signal of the policy needs: rounds sent back by cell".into(),
                body: json!({ "fix": fix, "measure": { "measure": "Rounds sent back by cell", "signal": "Speed" } }),
                ..db::Matter::default()
            },
        )?;
        Ok(())
    })
    .unwrap();
    api::reply_to_question(&mut e.db, "p", again, " approved ", &[], db::Speaker::Owner).unwrap();
    assert!(
        steps_until(&mut e, 90, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    // The draft that took effect queued its new measures, each once, but the one the queue held already.
    assert_eq!(
        measures_queued(&e),
        ["Rounds sent back by cell", "How long a merge waits for CI"],
        "{log}"
    );
    assert_eq!(rows_at(&e, arch, "P1"), ["ask_owner", "write_policy"], "{log}");
    let refused: String =
        e.db.conn()
            .query_row(
                "SELECT rejected FROM rounds WHERE work = ?1 AND cell = 'P1' AND rejected IS NOT NULL",
                [arch],
                |r| r.get(0),
            )
            .expect(&log);
    assert!(
        refused.contains("the owner replied and no policy is in force"),
        "{refused}"
    );
    assert_eq!(rows_at(&e, arch, "P2"), ["written", "written"], "{log}");
    // The writing is given the owner's reply that brought it back; the first, which the session was given at the
    // check, by its address only.
    assert!(
        message_at(&e, arch, "P2", true).contains("Make the why shorter."),
        "{log}"
    );
    let routes = rows_at(&e, arch, "Q2");
    assert!(routes.contains(&"back_to_set_policy".to_string()), "{routes:?}");
    assert_eq!(rows_at(&e, arch, "P12"), ["next"], "{log}");
    let direction: String =
        e.db.conn()
            .query_row(
                "SELECT json_extract(policy, '$.direction') FROM policy_current WHERE project = 'p'",
                [],
                |r| r.get(0),
            )
            .expect(&log);
    assert_eq!(direction, "Ship the parser.");
    let drafts: i64 =
        e.db.conn()
            .query_row("SELECT count(*) FROM policy_draft", [], |r| r.get(0))
            .unwrap();
    assert_eq!(drafts, 0);
    let path = path_of(&e, arch);
    let took_effect = path.iter().position(|c| c == "P12").expect(&log);
    assert_eq!(rows_at(&e, arch, "N1").len(), 1, "{path:?}\n{log}");
    assert!(
        path.iter().position(|c| c == "N1").expect(&log) > took_effect,
        "{path:?}"
    );
    assert_eq!(
        at_head(&e),
        [
            (
                "check_policy".to_string(),
                true,
                vec![("U1".to_string(), "A new session understood the project".to_string())]
            ),
            (
                "make_plan".to_string(),
                true,
                vec![
                    ("U1".to_string(), "A new session understood the project".to_string()),
                    ("P12".to_string(), "A new policy took effect".to_string())
                ]
            )
        ],
        "{log}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// The owner's words to the architect in a work's group are a matter of the architect's, which it answers in one round,
// its reply its message in that group and not in the one-to-one; under the words, the line says where they stand there
// as in the one-to-one.
#[test]
fn the_reply_to_the_owner_s_words_in_a_work_s_group_is_in_that_group() {
    let s = setup("words-in-group");
    let mut agents = scripts();
    agents.insert(
        "H8",
        Box::new(|_: &Request, _: usize| {
            (
                vec![("outcome.answer", "done")],
                vec![("outcome.answer.reply", json!("The tests run on every push."))],
            )
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    let item = an_item(&mut e);
    let v = e.flow_now;
    let work =
        e.db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO works (item, flow_version, graph, at, paused, paused_by) VALUES (?1, ?2, 'engineer', 'R1', 'later', 'owner')",
                params![item, v],
            )?;
            Ok(tx.last_insert_rowid())
        })
        .unwrap();
    db::say_to_architect(
        &mut e.db,
        "p",
        Some(item),
        Some(work),
        "Do the tests run on every push?",
        &[],
        db::Speaker::Owner,
    )
    .unwrap();
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    assert_eq!(rows_at(&e, arch, "H8"), ["done"], "{log}");
    let reply_in = |conversation: &str| -> i64 {
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM conversation_entries WHERE conversation = ?1 AND source = 'message'
                 AND sender = 'architect' AND text = 'The tests run on every push.'",
                [conversation],
                |r| r.get(0),
            )
            .unwrap()
    };
    assert_eq!(
        (reply_in(&format!("work:{work}")), reply_in("architect:p")),
        (1, 0),
        "{log}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// What was said in a work's group, other than to the architect, is a matter of its own: the engine queues it as the
// architect waits to start, and it starts no work of the architect; a work something else started picks it, as it
// came before that work, and its cell is given what was said. What the engineer told the owner there queues it as the
// owner's words do, since every message to the owner reaches the architect too, and what is said after joins it.
#[test]
fn what_was_said_in_the_groups_waits_for_a_work_and_its_cell_takes_it() {
    let s = setup("group-talk");
    let mut agents = scripts();
    picks_first(&s, &mut agents);
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    let item = an_item(&mut e);
    let v = e.flow_now;
    let work =
        e.db.write(Actor::Owner, |tx| {
            tx.execute(
                "INSERT INTO works (item, flow_version, graph, at, paused, paused_by) VALUES (?1, ?2, 'engineer', 'R1', 'later', 'owner')",
                params![item, v],
            )?;
            let work = tx.last_insert_rowid();
            tx.execute(
                "INSERT INTO rounds (work, n, cell, input, output, transition, config_commit, started_at, ended_at)
                 VALUES (?1, 1, 'R1', '{}', '{}', 'next', 'c', ?2, ?2)",
                params![work, db::now()],
            )?;
            tx.execute(
                "INSERT INTO owner_messages (work, round, text) VALUES (?1, 1, 'The parser needs a token of its own.')",
                [work],
            )?;
            Ok(work)
        })
        .unwrap();
    assert_eq!(e.step(None).unwrap().work, None);
    let talks = |e: &Engine| -> i64 {
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM architect_matters WHERE kind = 'group_talk'",
                [],
                |r| r.get(0),
            )
            .unwrap()
    };
    assert_eq!(talks(&e), 1);
    e.db.write(Actor::Owner, |tx| {
        tx.execute(
            "INSERT INTO interjections (work, line, speaker, text) VALUES (?1, 'engineer', 'owner', 'Keep the parser small.')",
            [work],
        )?;
        Ok(())
    })
    .unwrap();
    assert_eq!(e.step(None).unwrap().work, None);
    assert_eq!(talks(&e), 1);
    // In the queue, not handled; and no work of the architect started for it.
    let talk = |e: &Engine| -> (bool, bool) {
        e.db.conn()
            .query_row(
                "SELECT id IN (SELECT matter FROM architect_queue), handled_round IS NOT NULL
                 FROM architect_matters WHERE kind = 'group_talk'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap()
    };
    assert_eq!(talk(&e), (true, false));
    assert!(architect_works(&e).is_empty());
    // It came before the work something else starts.
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "UPDATE architect_matters SET at = at - 1000 WHERE kind = 'group_talk'",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    from_q(&mut e, "Look at the queue.");
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 60, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    assert_eq!(rows_at(&e, arch, "H12"), ["done"], "{log}");
    let heard = message_at(&e, arch, "H12", false);
    assert!(heard.contains("Keep the parser small."), "{log}");
    assert!(heard.contains("The parser needs a token of its own."), "{log}");
    assert!(
        !message_at(&e, arch, "Q1", false).contains("Keep the parser small."),
        "{log}"
    );
    assert_eq!(talk(&e), (false, true), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The owner's words come before the matters at the head of the queue: words answered with planning again queue the
// plan, and words answered with a new policy queue the check, each with what the owner asks; a new session's
// understanding queued both already, so each is queued once and takes the owner's why beside the understanding's, and
// each is taken once, after the owner's words.
#[test]
fn a_matter_at_the_head_asked_for_twice_is_queued_once_with_both_whys() {
    let s = setup("head-twice");
    let mut agents = scripts();
    agents.insert(
        "H8",
        Box::new(|r: &Request, _: usize| {
            let (way, why) = if r.prompt.contains("Plan again.") {
                ("re_plan", "The owner wants the parser first.")
            } else {
                ("new_policy", "The owner doubts the direction.")
            };
            (
                vec![("outcome.answer", way)],
                vec![
                    ("outcome.answer.reply", json!("I see.")),
                    ("outcome.answer.why", json!(why)),
                ],
            )
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    say(&mut e, "Plan again.");
    say(&mut e, "Check the policy.");
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 60, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    assert_eq!(
        path_of(&e, arch),
        [
            "S1", "U1", "Q0", "Q2", "H8", "B1", "Q0", "Q2", "H8", "B1", "Q0", "Q2", "P1", "B1", "Q0", "Q2", "N1", "B1",
            "B5", "B4", "Q0", "E2"
        ],
        "{log}"
    );
    assert_eq!(rows_at(&e, arch, "H8"), ["re_plan", "new_policy"], "{log}");
    let understood = "A new session understood the project".to_string();
    assert_eq!(
        at_head(&e),
        [
            (
                "check_policy".to_string(),
                true,
                vec![
                    ("U1".to_string(), understood.clone()),
                    ("H8".to_string(), "The owner doubts the direction.".to_string())
                ]
            ),
            (
                "make_plan".to_string(),
                true,
                vec![
                    ("U1".to_string(), understood),
                    ("H8".to_string(), "The owner wants the parser first.".to_string())
                ]
            )
        ],
        "{log}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

/// A work of the architect on a side report, whose cell `H5` has the project's open observations searched, the
/// observer handing back `candidates`, and then merges the report into observation 1, run to its end. Gives the engine,
/// the architect's work, what the engine said and the setup.
fn searched(name: &str, candidates: Value) -> (Engine, i64, String, Setup) {
    let s = setup(name);
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
        Box::new(move |_: &Request, _: usize| (Vec::new(), vec![("outcome.candidates", candidates.clone())])),
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
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    (e, arch, log, s)
}

// A cell on a matter's path has the project's open observations searched, by the observer at its own level and line,
// and goes on with the candidates: here a report taken in is merged into the observation found, reported once more.
#[test]
fn the_search_of_observations_hands_back_candidates() {
    let (e, arch, log, s) = searched("search", json!([{ "id": 1, "why": "It records the same slowness." }]));
    let path = path_of(&e, arch);
    let at = path.iter().position(|c| c == "X2").expect(&log);
    assert_eq!(
        &path[at - 4..at + 3],
        ["H4", "Q0", "Q2", "H5", "X2", "H5", "B1"],
        "{log}"
    );
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
    // The observer writes what the owner reads in the owner's language, as every line does: its forms, in the system
    // prompt its session starts with or in its message, say which.
    let input = input_at(&e, arch, "X2", false);
    let search = format!(
        "{}\n{}",
        system_text(&input).unwrap_or_default(),
        input["message"].as_str().unwrap_or_default()
    );
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

// A search that found nothing comes back saying so: the cell that asked has the search's results listed as none,
// where before the search it had them not listed at all, and does not take them for not given.
#[test]
fn a_search_that_found_nothing_comes_back_as_none() {
    let (mut e, arch, log, s) = searched("search-none", json!([]));
    let w = e.work(arch).unwrap();
    let g = e.graph_of(&w).unwrap();
    let number = g
        .cell("H5")
        .unwrap()
        .inputs
        .iter()
        .filter(|x| !x.from.iter().all(|s| *s == flow::format::Src::Itself))
        .position(|x| x.name.as_deref() == Some("Search results"))
        .unwrap()
        + 1;
    let listed = |m: &str| work_from(m).into_iter().find(|l| l.starts_with(&format!("{number}. ")));
    let (asked, back) = (message_at(&e, arch, "H5", false), message_at(&e, arch, "H5", true));
    assert_eq!(listed(&asked), None, "{asked}");
    assert_eq!(listed(&back), Some(format!("{number}. none")), "{back}\n{log}");
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
fn keep_what_they_saw(agents: &mut BTreeMap<&'static str, Script>, cells: &[&'static str]) -> Arc<Mutex<Vec<Saw>>> {
    let seen = Arc::new(Mutex::new(Vec::<Saw>::new()));
    for &cell in cells {
        let kept = seen.clone();
        let then = agents.remove(cell);
        agents.insert(
            cell,
            Box::new(move |r: &Request, n: usize| {
                let wt = &r.worktree;
                kept.lock().unwrap().push(Saw {
                    r: r.clone(),
                    lib: std::fs::read_to_string(wt.join("src/lib.txt")).unwrap_or_default(),
                    head: if wt.join(".git").exists() {
                        git(wt, &["rev-parse", "HEAD"])
                    } else {
                        String::new()
                    },
                    left: wt.join("left.txt").exists(),
                });
                if r.cell == "P1" && n == 1 {
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
// the latest main before each round, whatever was changed there dropped: the check of the policy too, which edits
// nothing and changes no repository; gh for a work on no item runs in that worktree, where gh knows the repository.
#[test]
fn an_architect_s_lines_read_the_latest_main_in_a_worktree_of_its_own() {
    let s = setup("architect-worktrees");
    no_reply_wait(&s);
    let mut agents = scripts();
    let seen = keep_what_they_saw(&mut agents, &["U1", "P1"]);
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    let arch = architect_work_at(&mut e, "U1");
    let worktrees = s.data.join("worktrees");
    let architect = worktrees.join("architect").join("p");
    main_moves_on(&s.root, &[("src/lib.txt", "one\n")], "One");
    for _ in 0..5 {
        if seen.lock().unwrap().len() >= 2 {
            break;
        }
        e.advance(arch).unwrap();
    }
    let main = git(&s.origin, &["rev-parse", "main"]);
    {
        let seen = seen.lock().unwrap();
        let log = said.lock().unwrap().join("\n");
        assert_eq!(seen.len(), 2, "{log}");
        let (u1, p1) = (&seen[0], &seen[1]);
        assert_eq!((u1.r.cell.as_str(), p1.r.cell.as_str()), ("U1", "P1"), "{log}");
        for saw in [u1, p1] {
            assert_eq!(saw.r.worktree, architect);
            assert_eq!((saw.lib.as_str(), saw.head.as_str()), ("one\n", main.as_str()));
            // The project's checkout is no longer a directory added beside where it runs.
            assert_eq!(saw.r.readable.len(), 2, "{:?}", saw.r.readable);
        }
        for t in config::EDIT_TOOLS {
            assert!(p1.r.refused.iter().any(|x| x == t), "{t}: {:?}", p1.r.refused);
        }
        for g in config::GIT_CHANGES {
            assert!(p1.r.refused.contains(&format!("Bash(git {g} *)")), "git {g}");
        }
        // The session is told where it works in the system prompt it starts with, the same in every work of the
        // project, and the check of the policy, in the same session, is pointed to it.
        let system = u1.r.system.clone().unwrap_or_default();
        assert!(
            system.contains(&format!(
                "\n### work:architect\n\nYou are the architect of the project p. Your working directory is {},",
                u1.r.worktree.display()
            )),
            "{system}"
        );
        assert_eq!(p1.r.system.as_deref(), Some(system.as_str()));
        assert!(p1.r.prompt.contains("Worktree: work:architect"), "{}", p1.r.prompt);
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
        if seen.lock().unwrap().len() > 2 {
            break;
        }
        e.advance(arch).unwrap();
    }
    let seen = seen.lock().unwrap();
    let log = said.lock().unwrap().join("\n");
    let next = seen.get(2).expect(&log);
    assert_eq!(next.r.worktree, architect, "{log}");
    assert_eq!(
        (next.lib.as_str(), next.head.as_str(), next.left),
        ("two\n", git(&s.origin, &["rev-parse", "main"]).as_str(), false),
        "{log}"
    );
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
    assert!(
        steps_until(&mut e, 120, done_all),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
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
    let log = said.lock().unwrap().join("\n");
    assert_eq!(matters(&e), [("report".to_string(), true, false)], "{log}");
    let (detail, urgent): (String, i64) =
        e.db.conn()
            .query_row("SELECT detail, urgent FROM architect_queue", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
    assert_eq!((detail.as_str(), urgent), ("duplicate", 1));
    // A work of the architect starts for the report, its first step beside the work's own.
    let pass = e.pass(&Go::default()).unwrap();
    let log = said.lock().unwrap().join("\n");
    let started = pass.jobs.iter().find(|s| s.work != work).expect(&log);
    assert_eq!(started.cell.as_deref(), Some("S1"), "{log}");
    let arch = started.work.unwrap();
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
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
    let log = said.lock().unwrap().join("\n");
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
    assert!(steps_until(&mut e, 80, p_done), "{}", said.lock().unwrap().join("\n"));
    for _ in 0..5 {
        e.step(None).unwrap();
    }
    let log = said.lock().unwrap().join("\n");
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
    let log = said.lock().unwrap().join("\n");
    assert_eq!(ended_at(&e, item, "R1"), [(Some("done".to_string()), None)], "{log}");
    assert!(matters(&e).is_empty(), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

/// A design that is not sure of one thing: it hands in an open question, with the options and the one it recommends.
fn designs_with_an_open_question() -> Script {
    Box::new(|_: &Request, _: usize| {
        (
            vec![("outcome.measure", "not_measured")],
            vec![
                (
                    "outcome.acceptance",
                    json!([{ "by": "test", "id": "A1", "condition": "The feature is there." }]),
                ),
                (
                    "outcome.open_questions",
                    json!([{ "question": "Which table?", "options": "One or two.", "recommended": "One." }]),
                ),
            ],
        )
    })
}

/// The work's open questions, by the view the cells read: each with its answer.
fn open_questions(e: &Engine, work: i64) -> Vec<(i64, String, Option<String>)> {
    let mut st =
        e.db.conn()
            .prepare("SELECT matter, question, answer FROM work_open_questions WHERE work = ?1 ORDER BY matter")
            .unwrap();
    st.query_map([work], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .map(|x| x.unwrap())
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

// An open question of the design goes to the project's architect as a matter of its own once the checks after design
// pass, and the work goes on by what the design recommends, once and not again; the merge waits for its answer, and the
// owner is told which question holds it. The architect answers by the path an engineer's question takes: the answer is
// kept in the matter, reaches the engineer as an interjection, which takes a work standing at the merge's judge to
// choose the way on, and the merge then goes on. The matter keeps the time the answer came, which a decision quoting
// the answer is dated by: none before it, and not earlier than the question after it.
#[test]
fn an_open_question_goes_to_the_architect_holds_the_merge_and_its_answer_reaches_the_engineer() {
    let s = setup("open-question");
    let mut agents = scripts();
    agents.insert("D2", designs_with_an_open_question());
    agents.insert(
        "H3",
        Box::new(|_: &Request, _: usize| {
            let reply = json!({ "text": "One table.", "why": "The items are few." });
            (Vec::new(), vec![("outcome.reply", reply)])
        }),
    );
    agents.insert(
        "A4",
        Box::new(|_: &Request, _: usize| (vec![("outcome.next", "carry_on")], Vec::new())),
    );
    let (mut e, said, item) = start(&s, agents);
    step_to(&mut e, item, "I1");
    let (work, _, _) = state(&e, item).unwrap();
    let asked = open_questions(&e, work);
    assert_eq!(asked.len(), 1, "{}", said.lock().unwrap().join("\n"));
    let matter = asked[0].0;
    assert_eq!((asked[0].1.as_str(), asked[0].2.as_deref()), ("Which table?", None));
    // A question not answered has no answer time, which a decision quoting the answer is dated by.
    assert_eq!(matter_body(&e, matter).get("answered_at"), None);
    let raised: i64 =
        e.db.conn()
            .query_row("SELECT at FROM architect_matters WHERE id = ?1", [matter], |r| r.get(0))
            .unwrap();
    let body: String =
        e.db.conn()
            .query_row("SELECT body FROM architect_matters WHERE id = ?1", [matter], |r| {
                r.get(0)
            })
            .unwrap();
    let body: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(
        (&body["options"], &body["recommended"]),
        (&json!("One or two."), &json!("One."))
    );
    assert!(
        body["design_link"]
            .as_str()
            .is_some_and(|l| l.starts_with(&format!("/works/{work}?round=R{work}-"))),
        "{body}"
    );
    // The work goes on, its architect held, to the merge's judge, which waits for the answer.
    advance_until(&mut e, work, |e| !ended_at(e, item, "M1").is_empty());
    let log = said.lock().unwrap().join("\n");
    assert_eq!(
        ended_at(&e, item, "M1").first().map(|x| x.0.clone()),
        Some(Some("wait_questions".to_string())),
        "{log}"
    );
    let holding = format!("is ready to merge, but open question {matter} waits for its answer.");
    e.tick().unwrap();
    assert!(
        told(&s, &holding) > 0,
        "{}",
        std::fs::read_to_string(s.data.join("notifications.log")).unwrap_or_default()
    );
    // The architect's work takes the question by its crux and its ways, and the answer goes to the engineer.
    let answered = |e: &mut Engine| open_questions(e, work)[0].2.is_some();
    assert!(steps_until(&mut e, 60, answered), "{}", said.lock().unwrap().join("\n"));
    assert_eq!(
        open_questions(&e, work)[0].2.as_deref(),
        Some("One table.\n\nThe items are few.")
    );
    // The architect answered it itself: the record says so, and holds no ask of the owner.
    let body = matter_body(&e, matter);
    assert_eq!(body["answered_by"], "architect", "{body}");
    assert!(body.get("owner_ask").is_none(), "{body}");
    // The time of the answer is the one it came at, set when the architect replied, not the one the question was raised.
    let came = body["answered_at"].as_i64().expect("the answer's time is kept");
    assert!(came >= raised, "{came} {raised}");
    let cleared = |e: &mut Engine| ended_at(e, item, "M1").iter().any(|x| x.0.as_deref() == Some("clear"));
    assert!(steps_until(&mut e, 60, cleared), "{}", said.lock().unwrap().join("\n"));
    let way_on = message_at(&e, work, "A4", true);
    assert!(
        way_on.contains("The answer to the open question of your design \"Which table?\": One table."),
        "{way_on}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

/// A matter's body.
fn matter_body(e: &Engine, matter: i64) -> Value {
    let body: String =
        e.db.conn()
            .query_row("SELECT body FROM architect_matters WHERE id = ?1", [matter], |r| {
                r.get(0)
            })
            .unwrap();
    serde_json::from_str(&body).unwrap()
}

/// The ask of the owner that parks a matter, with the cell the matter goes back to with the owner's reply; none while
/// no ask parks it.
fn parked_for(e: &Engine, matter: i64) -> Option<(i64, String)> {
    let (ask, cell): (Option<i64>, Option<String>) =
        e.db.conn()
            .query_row(
                "SELECT asked, resume_cell FROM architect_matters WHERE id = ?1",
                [matter],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
    ask.zip(cell)
}

/// The work of the architect whose ways (`H3`) took a round, the first or the last.
fn ways_work(e: &Engine, last: bool) -> i64 {
    let order = if last { "DESC" } else { "" };
    e.db.conn()
        .query_row(
            &format!("SELECT work FROM rounds WHERE cell = 'H3' ORDER BY work {order}, n {order} LIMIT 1"),
            [],
            |r| r.get(0),
        )
        .unwrap()
}

/// The ways for an open question only the owner can settle: they pin it in the owner's one-to-one, the question linking
/// to the design as the matter holds the link (the architect's session was given the matter at the crux already, so the
/// message does not repeat it), and reply once the owner's words are in the message, in words of their own.
fn ways_pin_for_the_owner(s: &Setup) -> Script {
    let path = config::database(&s.data);
    Box::new(move |r: &Request, _: usize| {
        if r.prompt.contains("Take one table.") {
            let reply = json!({ "text": "Go on with one table.", "why": "The owner decided it." });
            return (Vec::new(), vec![("outcome.reply", reply)]);
        }
        let c = db::open_read(&path).unwrap();
        let link: String = c
            .query_row(
                "SELECT body ->> '$.design_link' FROM architect_matters WHERE kind = 'open_question' ORDER BY id LIMIT 1",
                [],
                |x| x.get(0),
            )
            .unwrap();
        asks_the_owner(&format!("Is one table enough? See [the design]({link})."))
    })
}

// An open question the architect pins for the owner has no answer until the owner has answered and the architect has
// replied by it, so the merge keeps waiting; the architect does not wait for the reply, its work goes on and ends; the
// question is pinned in the owner's one-to-one with the link to the design; and the owner's reply brings the matter back
// to the ways, which are given it, and whose reply is kept after the owner's words as they were said, the record saying
// the owner answered; the engineer is told, and the merge goes on.
#[test]
fn an_open_question_pinned_for_the_owner_holds_the_merge_until_the_owner_has_answered() {
    let s = setup("open-question-pinned");
    let mut agents = scripts();
    agents.insert("D2", designs_with_an_open_question());
    agents.insert("H3", ways_pin_for_the_owner(&s));
    agents.insert(
        "A4",
        Box::new(|_: &Request, _: usize| (vec![("outcome.next", "carry_on")], Vec::new())),
    );
    let (mut e, said, item) = start(&s, agents);
    step_to(&mut e, item, "I1");
    let (work, _, _) = state(&e, item).unwrap();
    let matter = open_questions(&e, work)[0].0;
    advance_until(&mut e, work, |e| !ended_at(e, item, "M1").is_empty());
    let pinned = |e: &mut Engine| parked_for(e, matter).is_some();
    assert!(steps_until(&mut e, 60, pinned), "{}", said.lock().unwrap().join("\n"));
    let log = said.lock().unwrap().join("\n");
    // The matter is parked at the ways for the ask, which holds the question with the link to the design.
    let (ask, cell) = parked_for(&e, matter).unwrap();
    assert_eq!(cell, "H3", "{log}");
    let pinned_question: String =
        e.db.conn()
            .query_row(
                "SELECT body ->> '$.question' FROM owner_asks WHERE id = ?1",
                [ask],
                |r| r.get(0),
            )
            .unwrap();
    let link = format!("(/works/{work}?round=R{work}-");
    assert!(pinned_question.contains(&link), "{pinned_question}");
    // It has no answer, so the merge goes on waiting for it, and the one-to-one lists it as pending.
    assert_eq!(open_questions(&e, work)[0].2, None, "{log}");
    assert_eq!(matter_body(&e, matter).get("answer"), None);
    let pending: Vec<i64> = {
        let mut st = e.db.conn().prepare(db::QUESTIONS_PENDING).unwrap();
        st.query_map(["p"], |r| r.get("question"))
            .unwrap()
            .map(|x| x.unwrap())
            .collect()
    };
    assert_eq!(pending, [ask]);
    // The architect does not wait for the reply to it: its work ends with no wait at the look at the queue.
    let arch = ways_work(&e, false);
    assert!(
        steps_until(&mut e, 60, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    assert!(
        cells_of(&e, arch).iter().all(|c| c.1.as_deref() != Some("waits")),
        "{:?}\n{log}",
        cells_of(&e, arch)
    );
    for _ in 0..10 {
        e.advance(work).unwrap();
    }
    assert_eq!(open_questions(&e, work)[0].2, None, "{log}");
    assert!(
        ended_at(&e, item, "M1")
            .iter()
            .all(|x| x.0.as_deref() == Some("wait_questions")),
        "{:?}\n{log}",
        ended_at(&e, item, "M1")
    );
    // The owner replies with the question ticked: the matter is back at the ways with the owner's reply.
    api::reply_to_question(&mut e.db, "p", ask, "Take one table.", &[], db::Speaker::Owner).unwrap();
    let answered = |e: &mut Engine| open_questions(e, work)[0].2.is_some();
    assert!(steps_until(&mut e, 80, answered), "{}", said.lock().unwrap().join("\n"));
    let log = said.lock().unwrap().join("\n");
    let again = ways_work(&e, true);
    assert_ne!(again, arch, "{log}");
    let cells = cells_of(&e, again);
    let back = cells.iter().position(|c| c.0 == "Q2").expect(&log);
    assert_eq!(
        (cells[back].1.as_deref(), cells[back + 1].0.as_str()),
        (Some("back_with_answer_to_ways"), "H3"),
        "{cells:?}\n{log}"
    );
    assert!(message_at(&e, again, "H3", true).contains("Take one table."), "{log}");
    // The answer begins with the owner's words as they were said, and the record says who answered and where.
    let words = "Take one table.\n\nGo on with one table.\n\nThe owner decided it.";
    assert_eq!(open_questions(&e, work)[0].2.as_deref(), Some(words), "{log}");
    let body = matter_body(&e, matter);
    assert_eq!(
        (&body["answer"], &body["answered_by"], &body["owner_ask"]),
        (&json!(words), &json!("owner"), &json!(ask)),
        "{body}"
    );
    // The engineer is told it, and the merge goes on.
    let cleared = |e: &mut Engine| ended_at(e, item, "M1").iter().any(|x| x.0.as_deref() == Some("clear"));
    assert!(steps_until(&mut e, 60, cleared), "{}", said.lock().unwrap().join("\n"));
    let way_on = message_at(&e, work, "A4", true);
    // The engineer is told which part is the owner's and which the architect's.
    assert!(
        way_on.contains(
            "The answer to the open question of your design \"Which table?\": The owner answered: Take one \
             table.\n\nThe architect added: Go on with one table.\n\nThe owner decided it."
        ),
        "{way_on}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// The ways ask the owner only about an open question: an engineer waits for the reply to its question, and the ask of the
// owner is sent back, nothing is pinned, and the reply that follows reaches the engineer.
#[test]
fn an_engineer_s_question_cannot_be_pinned_for_the_owner() {
    let s = setup("pin-engineer-question");
    let ways: Script = Box::new(|_: &Request, n: usize| {
        if n == 1 {
            return asks_the_owner("Is a 3% slowdown acceptable? See [the design](/works/1?round=R1-1).");
        }
        let reply = json!({
            "text": "Put the slowdown to the owner as an open question of your design, and go on.",
            "why": "Accepting a slowdown is the owner's choice.",
        });
        (Vec::new(), vec![("outcome.reply", reply)])
    });
    let ask = asks_about_a_gap("The change makes parsing 3% slower, measured over 30 pairs.");
    let (e, said, _, work, arch) = a_gap_asked(&s, ask, vec![("H3", ways)]);
    let log = said.lock().unwrap().join("\n");
    let sent_back = why_sent_back(&e, arch, "H3");
    assert_eq!(sent_back.len(), 1, "{sent_back:?}\n{log}");
    assert!(sent_back[0].contains("cannot be parked for the owner"), "{sent_back:?}");
    let asks: i64 =
        e.db.conn()
            .query_row("SELECT count(*) FROM owner_asks", [], |r| r.get(0))
            .unwrap();
    assert_eq!(asks, 0, "{log}");
    assert_eq!(question_took(&e, work).as_deref(), Some("answered"), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The question pinned for the owner links to the design it is about, which the owner reads it away from: an ask without
// the link is sent back, and so is one that links to another round of the same work, whose address only begins with the
// design's; nothing is pinned, and the architect that answers the question itself instead has it recorded as its own
// answer.
#[test]
fn a_question_pinned_for_the_owner_must_link_to_its_design() {
    let s = setup("pin-without-link");
    let path = config::database(&s.data);
    let mut agents = scripts();
    agents.insert("D2", designs_with_an_open_question());
    agents.insert(
        "H3",
        Box::new(move |_: &Request, n: usize| {
            if n == 1 {
                return asks_the_owner("Is one table enough?");
            }
            if n == 2 {
                // The address of the design's round with one more digit: the round of another number.
                let link = format!("{}0", design_link_of(&path));
                return asks_the_owner(&format!("Is one table enough? See [the design]({link})."));
            }
            let reply = json!({ "text": "One table.", "why": "The items are few." });
            (Vec::new(), vec![("outcome.reply", reply)])
        }),
    );
    agents.insert(
        "A4",
        Box::new(|_: &Request, _: usize| (vec![("outcome.next", "carry_on")], Vec::new())),
    );
    let (mut e, said, item) = start(&s, agents);
    step_to(&mut e, item, "I1");
    let (work, _, _) = state(&e, item).unwrap();
    let matter = open_questions(&e, work)[0].0;
    let answered = |e: &mut Engine| open_questions(e, work)[0].2.is_some();
    assert!(steps_until(&mut e, 80, answered), "{}", said.lock().unwrap().join("\n"));
    let log = said.lock().unwrap().join("\n");
    let sent_back = why_sent_back(&e, ways_work(&e, false), "H3");
    assert_eq!(sent_back.len(), 2, "{sent_back:?}\n{log}");
    for why in &sent_back {
        assert!(why.contains("link to the design"), "{sent_back:?}");
        assert!(why.contains(&format!("/works/{work}?round=R{work}-")), "{sent_back:?}");
    }
    assert_eq!(parked_for(&e, matter), None, "{log}");
    let asks: i64 =
        e.db.conn()
            .query_row("SELECT count(*) FROM owner_asks", [], |r| r.get(0))
            .unwrap();
    assert_eq!(asks, 0, "{log}");
    assert_eq!(matter_body(&e, matter)["answered_by"], "architect", "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

/// The link to its design that the first open question's matter holds, as the ways read it from the database.
fn design_link_of(path: &std::path::Path) -> String {
    db::open_read(path)
        .unwrap()
        .query_row(
            "SELECT body ->> '$.design_link' FROM architect_matters WHERE kind = 'open_question' ORDER BY id LIMIT 1",
            [],
            |x| x.get(0),
        )
        .unwrap()
}

/// An engine whose work has the open question "Which table?" and whose ways (`H3`) are `ways`, with any other cell
/// scripted as given, run until the architect pinned the question for the owner and its work ended: the engine, what it
/// said, the item, its work, the matter of the question, and the ask that parks it.
fn pinned(
    s: &Setup,
    ways: Script,
    scripted: Vec<(&'static str, Script)>,
) -> (Engine, Arc<Mutex<Vec<String>>>, i64, i64, i64, i64) {
    let mut agents = scripts();
    agents.insert("D2", designs_with_an_open_question());
    agents.insert("H3", ways);
    agents.insert(
        "A4",
        Box::new(|_: &Request, _: usize| (vec![("outcome.next", "carry_on")], Vec::new())),
    );
    for (cell, script) in scripted {
        agents.insert(cell, script);
    }
    let (mut e, said, item) = start(s, agents);
    step_to(&mut e, item, "I1");
    let (work, _, _) = state(&e, item).unwrap();
    let matter = open_questions(&e, work)[0].0;
    advance_until(&mut e, work, |e| !ended_at(e, item, "M1").is_empty());
    assert!(
        steps_until(&mut e, 60, |e| parked_for(e, matter).is_some()),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let ask = parked_for(&e, matter).unwrap().0;
    let arch = ways_work(&e, false);
    assert!(
        steps_until(&mut e, 60, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    (e, said, item, work, matter, ask)
}

// The owner may be asked about the same open question again: the ways, back with the owner's reply, ask once more, and
// the matter is parked by the new ask. The answer is then made of the last words only, with the image that came with
// them named as an agent is given words, the record points to the last ask, and the first one, answered, stays as it was
// said and is pending no more.
#[test]
fn an_open_question_pinned_twice_is_answered_by_the_owner_s_last_words_and_their_image() {
    let s = setup("open-question-pinned-twice");
    let path = config::database(&s.data);
    let ways: Script = Box::new(move |r: &Request, _: usize| {
        if r.prompt.contains("Two tables after all.") {
            let reply = json!({ "text": "Go on with two tables.", "why": "The owner changed it." });
            return (Vec::new(), vec![("outcome.reply", reply)]);
        }
        let link = design_link_of(&path);
        let question = if r.prompt.contains("Take one table.") {
            "Is a second table needed?"
        } else {
            "Is one table enough?"
        };
        asks_the_owner(&format!("{question} See [the design]({link})."))
    });
    let (mut e, said, item, work, matter, first) = pinned(&s, ways, Vec::new());
    api::reply_to_question(&mut e.db, "p", first, "Take one table.", &[], db::Speaker::Owner).unwrap();
    // The ways ask again: the matter is parked by another ask, while the merge still has no answer.
    let again = |e: &mut Engine| parked_for(e, matter).is_some_and(|p| p.0 != first);
    assert!(steps_until(&mut e, 80, again), "{}", said.lock().unwrap().join("\n"));
    let second = parked_for(&e, matter).unwrap().0;
    assert_eq!(open_questions(&e, work)[0].2, None);
    assert_eq!(matter_body(&e, matter).get("answer"), None);
    let pending = |e: &Engine| -> Vec<i64> {
        let mut st = e.db.conn().prepare(db::QUESTIONS_PENDING).unwrap();
        st.query_map(["p"], |r| r.get("question"))
            .unwrap()
            .map(|x| x.unwrap())
            .collect()
    };
    assert_eq!(pending(&e), [second]);
    let image = api::images(&two_images()[..1]).unwrap();
    api::reply_to_question(
        &mut e.db,
        "p",
        second,
        "Two tables after all.",
        &image,
        db::Speaker::Owner,
    )
    .unwrap();
    let answered = |e: &mut Engine| open_questions(e, work)[0].2.is_some();
    assert!(steps_until(&mut e, 80, answered), "{}", said.lock().unwrap().join("\n"));
    let log = said.lock().unwrap().join("\n");
    let ids = db::attachments::of(e.db.conn(), db::attachments::Words::Ask(second)).unwrap();
    assert_eq!(ids.len(), 1, "{log}");
    let words = db::attachments::with_named("Two tables after all.", &ids);
    let expected = format!("{words}\n\nGo on with two tables.\n\nThe owner changed it.");
    let body = matter_body(&e, matter);
    assert_eq!(
        (&body["answer"], &body["answered_by"], &body["owner_ask"]),
        (&json!(expected), &json!("owner"), &json!(second)),
        "{body}"
    );
    assert!(!expected.contains("Take one table."));
    assert!(pending(&e).is_empty(), "{log}");
    let said_first: String =
        e.db.conn()
            .query_row(
                "SELECT answer ->> '$.text' FROM owner_asks WHERE id = ?1",
                [first],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(said_first, "Take one table.");
    let cleared = |e: &mut Engine| ended_at(e, item, "M1").iter().any(|x| x.0.as_deref() == Some("clear"));
    assert!(steps_until(&mut e, 60, cleared), "{}", said.lock().unwrap().join("\n"));
    let _ = std::fs::remove_dir_all(&s.root);
}

// A work that ends while its open question is pinned for the owner takes the matter out of the queue: the question is
// pending no more and the architect never waits for it; the owner's reply to it afterwards is taken and puts no answer
// in the matter of a work that is gone.
#[test]
fn a_question_pinned_for_a_work_that_ended_is_pending_no_more() {
    let s = setup("pinned-work-ended");
    let path = config::database(&s.data);
    let ways: Script = Box::new(move |_: &Request, _: usize| {
        let link = design_link_of(&path);
        asks_the_owner(&format!("Is one table enough? See [the design]({link})."))
    });
    let (mut e, said, item, work, matter, ask) = pinned(&s, ways, Vec::new());
    let projects = e.config.config.projects.clone();
    e.db.write(Actor::Owner, |tx| {
        engine::ending::cancel_item(tx, item, "no longer needed", "owner", &projects).map_err(db::Error::Abort)
    })
    .unwrap();
    let ended = |e: &Engine| -> bool {
        e.db.conn()
            .query_row("SELECT ended IS NOT NULL FROM works WHERE id = ?1", [work], |r| {
                r.get(0)
            })
            .unwrap()
    };
    for _ in 0..20 {
        if ended(&e) {
            break;
        }
        e.step_work(work).unwrap();
    }
    let log = said.lock().unwrap().join("\n");
    assert!(ended(&e), "{log}");
    let withdrawn: Option<String> =
        e.db.conn()
            .query_row("SELECT withdrawn FROM architect_matters WHERE id = ?1", [matter], |r| {
                r.get(0)
            })
            .unwrap();
    assert!(withdrawn.is_some(), "{log}");
    let pending: i64 =
        e.db.conn()
            .query_row(
                &format!("SELECT count(*) FROM ({})", db::QUESTIONS_PENDING),
                ["p"],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(pending, 0, "{log}");
    let now = db::now();
    assert_eq!(db::question_waited_on(e.db.conn(), "p", 300_000, now).unwrap(), None);
    api::reply_to_question(&mut e.db, "p", ask, "Take one table.", &[], db::Speaker::Owner).unwrap();
    assert_eq!(matter_body(&e, matter).get("answer"), None, "{log}");
    assert_eq!(matter_body(&e, matter).get("answered_by"), None, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A reply the batch's review holds back, because it comes with changes to items, is written once the batch is made, and
// then too it begins with the owner's words as they were said and records that the owner answered; the engineer is
// told it, and the merge goes on.
#[test]
fn a_reply_held_for_the_batch_review_keeps_the_owner_s_words() {
    let s = setup("pinned-reply-held");
    let path = config::database(&s.data);
    let ways: Script = Box::new(move |r: &Request, _: usize| {
        if r.prompt.contains("Take one table.") {
            let reply = json!({ "text": "Go on with one table.", "why": "The owner decided it." });
            let change = json!({ "kind": "create_item", "key": "one-table", "project": "p", "title": "Merge the tables",
                "problem": "There are two.", "why": "The owner chose one.", "acceptance": ["One table."],
                "level": "l", "deliver": "change", "reason": "The owner chose one table." });
            return (
                Vec::new(),
                vec![
                    ("outcome.reply", reply),
                    ("outcome.effects", json!({ "item_changes": [change] })),
                ],
            );
        }
        let link = design_link_of(&path);
        asks_the_owner(&format!("Is one table enough? See [the design]({link})."))
    });
    let review: Script = Box::new(|_: &Request, _: usize| {
        let ok = json!({ "change": 1, "verdict": "ok", "why": "Sound." });
        (Vec::new(), vec![("outcome.review", json!([ok]))])
    });
    let (mut e, said, item, work, matter, ask) = pinned(&s, ways, vec![("B2", review)]);
    api::reply_to_question(&mut e.db, "p", ask, "Take one table.", &[], db::Speaker::Owner).unwrap();
    let answered = |e: &mut Engine| open_questions(e, work)[0].2.is_some();
    assert!(steps_until(&mut e, 80, answered), "{}", said.lock().unwrap().join("\n"));
    let log = said.lock().unwrap().join("\n");
    let again = ways_work(&e, true);
    assert!(path_of(&e, again).contains(&"B2".to_string()), "{log}");
    let words = "Take one table.\n\nGo on with one table.\n\nThe owner decided it.";
    let body = matter_body(&e, matter);
    assert_eq!(
        (&body["answer"], &body["answered_by"], &body["owner_ask"]),
        (&json!(words), &json!("owner"), &json!(ask)),
        "{body}"
    );
    let held: Option<String> =
        e.db.conn()
            .query_row(
                "SELECT reply_held FROM architect_matters WHERE id = ?1",
                [matter],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(held, None, "{log}");
    let made: i64 =
        e.db.conn()
            .query_row("SELECT count(*) FROM items WHERE title = 'Merge the tables'", [], |r| {
                r.get(0)
            })
            .unwrap();
    assert_eq!(made, 1, "{log}");
    let cleared = |e: &mut Engine| ended_at(e, item, "M1").iter().any(|x| x.0.as_deref() == Some("clear"));
    assert!(steps_until(&mut e, 60, cleared), "{}", said.lock().unwrap().join("\n"));
    let way_on = message_at(&e, work, "A4", true);
    assert!(way_on.contains("The owner answered: Take one table."), "{way_on}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A reply that comes with item changes the batch review drops at its third look is held, and the owner is asked what to
// do about the dropped changes in another ask that parks the same matter. The owner's words in that ask are not the
// answer to the open question: the answer is made of the owner's words to the question pinned, which stay on the record,
// and the engineer is not told the words about the changes as the owner's decision.
#[test]
fn the_owner_s_words_about_dropped_changes_are_no_answer_to_a_pinned_open_question() {
    let s = setup("pinned-reply-dropped");
    let path = config::database(&s.data);
    let change = json!({ "kind": "create_item", "key": "one-table", "project": "p", "title": "Merge the tables",
        "problem": "There are two.", "why": "The owner chose one.", "acceptance": ["One table."],
        "level": "l", "deliver": "change", "reason": "The owner chose one table." });
    let with_change = change.clone();
    let ways: Script = Box::new(move |r: &Request, _: usize| {
        let reply = json!({ "text": "Go on with one table.", "why": "The owner decided it." });
        if r.prompt.contains("Leave them out.") {
            return (Vec::new(), vec![("outcome.reply", reply)]);
        }
        if r.prompt.contains("Take one table.") {
            return (
                Vec::new(),
                vec![
                    ("outcome.reply", reply),
                    ("outcome.effects", json!({ "item_changes": [with_change.clone()] })),
                ],
            );
        }
        let link = design_link_of(&path);
        asks_the_owner(&format!("Is one table enough? See [the design]({link})."))
    });
    // The review finds the change wrong at every look, and gives up at the third.
    let review: Script = Box::new(|_: &Request, _: usize| {
        let row = json!({ "change": 1, "verdict": "fix", "about": "constitution_and_policy",
                          "what": "The title is no action.", "how": "Write it as one.", "cause": "rule_missing" });
        (Vec::new(), vec![("outcome.review", json!([row]))])
    });
    let path = config::database(&s.data);
    let revise: Script = Box::new(move |_: &Request, _: usize| {
        let c = db::open_read(&path).unwrap();
        let matter: i64 = c
            .query_row(
                "SELECT matter FROM architect_item_changes WHERE state = 'collected'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        (
            Vec::new(),
            vec![(
                "outcome.changes",
                json!([{ "matter": matter, "change": change.clone() }]),
            )],
        )
    });
    let (mut e, said, item, work, matter, first) = pinned(&s, ways, vec![("B2", review), ("B3", revise)]);
    api::reply_to_question(&mut e.db, "p", first, "Take one table.", &[], db::Speaker::Owner).unwrap();
    // The review drops the changes and asks the owner about them in another ask, which parks the matter.
    let dropped = |e: &mut Engine| parked_for(e, matter).is_some_and(|p| p.0 != first);
    assert!(steps_until(&mut e, 120, dropped), "{}", said.lock().unwrap().join("\n"));
    let second = parked_for(&e, matter).unwrap().0;
    assert_eq!(open_questions(&e, work)[0].2, None);
    api::reply_to_question(&mut e.db, "p", second, "Leave them out.", &[], db::Speaker::Owner).unwrap();
    let answered = |e: &mut Engine| open_questions(e, work)[0].2.is_some();
    assert!(
        steps_until(&mut e, 120, answered),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let words = "Take one table.\n\nGo on with one table.\n\nThe owner decided it.";
    let body = matter_body(&e, matter);
    assert_eq!(
        (&body["answer"], &body["answered_by"], &body["owner_ask"]),
        (&json!(words), &json!("owner"), &json!(first)),
        "{log}"
    );
    let cleared = |e: &mut Engine| ended_at(e, item, "M1").iter().any(|x| x.0.as_deref() == Some("clear"));
    assert!(steps_until(&mut e, 60, cleared), "{}", said.lock().unwrap().join("\n"));
    let way_on = message_at(&e, work, "A4", true);
    assert!(way_on.contains("The owner answered: Take one table."), "{way_on}");
    assert!(!way_on.contains("Leave them out."), "{way_on}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A design written again that hands the same open question in does not put it again.
#[test]
fn an_open_question_handed_in_again_is_put_once() {
    let s = setup("open-question-again");
    let mut agents = scripts();
    agents.insert("D2", designs_with_an_open_question());
    agents.insert(
        "A4",
        Box::new(|_: &Request, _: usize| (vec![("outcome.next", "redesign")], Vec::new())),
    );
    let (mut e, said, item) = start(&s, agents);
    step_to(&mut e, item, "T7");
    let (work, _, _) = state(&e, item).unwrap();
    owner_says(&mut e, work, "Design it again.");
    advance_until(&mut e, work, |e| ended_at(e, item, "D11").len() == 2);
    let log = said.lock().unwrap().join("\n");
    assert_eq!(ended_at(&e, item, "D11").len(), 2, "{log}");
    assert_eq!(open_questions(&e, work).len(), 1, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// What the work learned goes into its item's approach once it merged, with the commit it merged, through the one write
// path, so that the changes keep it.
#[test]
fn what_a_work_learned_goes_into_its_item_with_its_commits() {
    let s = setup("learned");
    let mut agents = scripts();
    agents.insert(
        "L2",
        Box::new(|_: &Request, _: usize| {
            (
                Vec::new(),
                vec![(
                    "outcome.learned",
                    json!("Fact (2026-11-02): One table is enough (R1-2)"),
                )],
            )
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    let done = |e: &mut Engine| !ended_at(e, item, "L2").is_empty();
    assert!(steps_until(&mut e, 200, done), "{}", said.lock().unwrap().join("\n"));
    let log = said.lock().unwrap().join("\n");
    let approach: String =
        e.db.conn()
            .query_row("SELECT approach FROM items WHERE id = ?1", [item], |r| r.get(0))
            .unwrap();
    let merged: String =
        e.db.conn()
            .query_row(
                "SELECT json_extract(r.output, '$.result.merge_commit') FROM rounds r JOIN works w ON w.id = r.work
                 WHERE w.item = ?1 AND r.cell = 'M2' AND r.transition = 'merged'",
                [item],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(
        approach,
        format!("Learned:\nFact (2026-11-02): One table is enough (R1-2)\nCommits: {merged}"),
        "{log}"
    );
    let kept: i64 =
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM changes WHERE table_name = 'items' AND json_extract(new_row, '$.approach') = ?1",
                [&approach],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(kept, 1, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The history is gathered before the research and before the design, each in a round of the reader's line at the lowest
// level, and given to the cell after it; a look-up goes to the reader and back to the cell that asked, in the same
// session, and is given to it.
#[test]
fn the_history_is_gathered_and_looked_up_by_the_reader_of_the_line_librarian() {
    let s = setup("history");
    let mut agents = scripts();
    agents.insert(
        "G1",
        Box::new(|_: &Request, _: usize| (Vec::new(), vec![("outcome.brief", json!("The problem's history."))])),
    );
    agents.insert(
        "G2",
        Box::new(|_: &Request, _: usize| (Vec::new(), vec![("outcome.brief", json!("The code's history."))])),
    );
    agents.insert(
        "X5",
        Box::new(|_: &Request, _: usize| (Vec::new(), vec![("outcome.brief", json!("What was looked up."))])),
    );
    agents.insert(
        "R1",
        Box::new(|_: &Request, n: usize| {
            if n == 1 {
                (
                    vec![("outcome", "history")],
                    vec![("outcome.history", json!("Why is it so?"))],
                )
            } else {
                (Vec::new(), Vec::new())
            }
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    step_to(&mut e, item, "D2");
    let (work, _, _) = state(&e, item).unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(ended_at(&e, item, "X5").len(), 1, "{log}");
    assert!(
        message_at(&e, work, "R1", false).contains("The problem's history."),
        "{log}"
    );
    assert!(
        message_at(&e, work, "R1", true).contains("What was looked up."),
        "{log}"
    );
    assert!(
        message_at(&e, work, "D1", false).contains("The code's history."),
        "{log}"
    );
    assert!(message_at(&e, work, "X5", false).contains("Why is it so?"), "{log}");
    let session = |cell: &str| session_of(&e, work, cell);
    assert_eq!(session("G1"), session("G2"));
    assert_eq!(session("G1"), session("X5"));
    assert_ne!(session("G1"), session("R1"));
    let line: String =
        e.db.conn()
            .query_row("SELECT line FROM sessions WHERE id = ?1", [session("G1")], |r| r.get(0))
            .unwrap();
    assert_eq!(line, "librarian");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The gatherer reads the live database by the address it is given: the database is in WAL mode, with the engine's
// connection open, and what was committed to its log and not yet to the file is read too.
#[test]
fn the_history_s_address_reads_the_live_database_in_wal_mode() {
    let s = setup("history-wal");
    let (mut e, said, item) = start(&s, scripts());
    step_to(&mut e, item, "R1");
    let (work, _, _) = state(&e, item).unwrap();
    let log = said.lock().unwrap().join("\n");
    let sources = given_value(&message_at(&e, work, "G1", false), "·engine·history_sources").expect(&log);
    let database = sources["database"].as_str().unwrap().to_string();
    assert_eq!(database, config::database(&s.data).display().to_string());
    assert!(
        sources["read"]
            .as_str()
            .unwrap()
            .starts_with("sqlite3 -safe -readonly '"),
        "{sources}"
    );
    assert!(sources["checkout"].is_string(), "{sources}");
    e.db.write(Actor::Owner, |tx| {
        tx.execute("UPDATE items SET title = 'Only in the log' WHERE id = ?1", [item])?;
        Ok(())
    })
    .unwrap();
    let wal = std::fs::metadata(format!("{database}-wal")).unwrap();
    assert!(wal.len() > 0);
    let out = Command::new("sqlite3")
        .args([
            "-safe",
            "-readonly",
            &database,
            &format!("SELECT title FROM items WHERE id = {item}"),
        ])
        .output()
        .expect("sqlite3 runs");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "Only in the log");
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
    // A work of the architect starts for what was said, its first step beside the work's own.
    let pass = e.pass(&Go::default()).unwrap();
    let log = said.lock().unwrap().join("\n");
    let started = pass.jobs.iter().find(|s| s.work != Some(work)).expect(&log);
    assert_eq!(started.cell.as_deref(), Some("S1"), "{log}");
    let arch = started.work.unwrap();
    let stopped = |e: &mut Engine| state(e, item).is_some_and(|x| x.2.is_some());
    assert!(steps_until(&mut e, 40, stopped), "{}", said.lock().unwrap().join("\n"));
    let why = format!("{}: It waits for the parser.", db::STOPPED_BY_ARCHITECT);
    assert_eq!(state(&e, item).unwrap().2, Some(why.clone()));
    assert_eq!(paused_by(&e, work).as_deref(), Some("engine"));
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let n = rounds_count(&e);
    let log = said.lock().unwrap().join("\n");
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

// The architect starts a work over in its effects: the engine takes it over as it takes over a work the owner started
// over, saying the architect started it over and why, in the work's end and in the group's notice, and a new work on
// the item takes it over. An item with no work running cannot be started over, and the return naming one is sent back;
// nor can a work of the architect.
#[test]
fn the_architect_starts_a_work_over() {
    let s = setup("architect-start-over");
    let mut agents = scripts();
    agents.insert(
        "H8",
        Box::new(|_: &Request, n: usize| {
            let item = if n == 1 { 2 } else { 1 };
            let over = json!({ "kind": "start_work_over", "item": item, "reason": "Its design went the wrong way." });
            (Vec::new(), vec![("outcome.effects", json!({ "actions": [over] }))])
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    let gone =
        e.db.write(Actor::Owner, |tx| {
            tx.execute(
                "INSERT INTO items (project, title, problem, why, deliver, level, cancelled)
                 VALUES ('p', 'Gone', 'x', 'y', 'change', 'l', 'not needed')",
                [],
            )?;
            Ok(tx.last_insert_rowid())
        })
        .unwrap();
    assert_eq!(gone, 2);
    let work = e.step(None).unwrap().work.unwrap();
    say(&mut e, "Begin the feature again, and keep what it built.");
    // A work of the architect starts for what was said, its first step beside the work's own.
    let pass = e.pass(&Go::default()).unwrap();
    let log = said.lock().unwrap().join("\n");
    let started = pass.jobs.iter().find(|s| s.work != Some(work)).expect(&log);
    assert_eq!(started.cell.as_deref(), Some("S1"), "{log}");
    let arch = started.work.unwrap();
    let over = |e: &mut Engine| works_of(e, item).len() == 2;
    assert!(steps_until(&mut e, 40, over), "{}", said.lock().unwrap().join("\n"));
    let why = "stopped: the architect started it over, and a new work on it takes the item over: Its design went the \
               wrong way";
    let works = works_of(&e, item);
    assert_eq!((works[0].0, works[0].5.as_deref()), (work, Some(why)));
    let notice: String =
        e.db.conn()
            .query_row(
                "SELECT text FROM engine_notices WHERE work = ?1 ORDER BY id DESC LIMIT 1",
                [work],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(notice, format!("Work {work} stopped: {}.", &why["stopped: ".len()..]));
    let sent_back: String =
        e.db.conn()
            .query_row(
                "SELECT rejected FROM rounds WHERE work = ?1 AND cell = 'H8' AND rejected IS NOT NULL",
                [arch],
                |r| r.get(0),
            )
            .unwrap();
    assert!(
        sent_back.contains("action 1: item 2 has no work running"),
        "{sent_back}"
    );
    assert_eq!(
        api::start_over(&mut e.db, arch),
        Err(format!(
            "work {arch} is the architect's: only a work on an item can be started over"
        ))
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// The architect runs at the level the installation names for it, and so does the review of its item changes: their
// rounds run with the named level's model, not the top level's, at the effort their cell declares whatever the model,
// and record the level in their call.
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
        said.lock().unwrap().join("\n")
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
        said.lock().unwrap().join("\n")
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// While slots are free and fewer of a project's items are ready to start than slots are free, here none, the engine
// queues a matter of slots free for its architect, with the slots free and the items ready, which starts a work of it;
// once one is handled, no other comes for an hour, nor after it while the slots free and the items ready are those the
// last held, since the architect would only come to the same end.
#[test]
fn slots_free_start_a_work_of_the_architect_once_an_hour_at_most_and_only_on_new_facts() {
    let s = setup("slots-free");
    let (mut e, said) = engine(&s, scripts());
    let started = e.step(None).unwrap();
    let log = said.lock().unwrap().join("\n");
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
    assert_eq!(body, r#"{"free":1,"ready":[]}"#);
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
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
    // Past the wait, with the same slot free and still nothing ready, none comes.
    assert_eq!(e.step(None).unwrap().work, None);
    assert_eq!(count(&e), 1);
    // Once the facts differ from those the last held, here as if an item had been ready then, one comes.
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "UPDATE architect_matters SET body = '{\"free\":1,\"ready\":[7]}' WHERE kind = 'slots_free'",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    let again = e.step(None).unwrap();
    let log = said.lock().unwrap().join("\n");
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
    assert_eq!(started.started_item, Some(first), "{}", said.lock().unwrap().join("\n"));
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
    let log = said.lock().unwrap().join("\n");
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
    let log = said.lock().unwrap().join("\n");
    assert_eq!((first.cell.as_deref(), first.rounds.len()), (Some("S1"), 1), "{log}");
    let arch = first.work.unwrap();
    assert_eq!(paused_by(&e, arch).as_deref(), Some("owner"));
    assert_eq!(e.step(None).unwrap().work, None);
    let second = e.step_work(arch).unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!((second.cell.as_deref(), second.rounds.len()), (Some("U1"), 1), "{log}");
    let third = e.step_architect("p").unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(
        (third.work, third.cell.as_deref(), third.rounds.len()),
        (Some(arch), Some("Q0"), 1),
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
    let log = said.lock().unwrap().join("\n");
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
    assert_eq!(followups(&e).len(), 1, "{}", said.lock().unwrap().join("\n"));
    let _ = std::fs::remove_dir_all(&s.root);
}

/// What a keep-warm ping was given: the command line it ran, and its message.
type Replayed = (Vec<String>, String);

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
    assert_eq!(lines.len(), 2, "{lines:?}\n{}", said.lock().unwrap().join("\n"));
    assert_eq!((lines[0].0.as_str(), lines[0].1), ("line", true), "{lines:?}");
    assert!(lines[0].2.starts_with("R1,") && !lines[0].2.contains("I1"), "{lines:?}");
    assert_eq!(lines[1], ("cell".to_string(), false, "I1,I1".to_string()));
    let rounds = agent_rounds(&e, item);
    let implements: Vec<&Value> = rounds.iter().filter(|r| r.1 == "I1").map(|r| &r.2).collect();
    // The new session starts with the line's system prompt, and is given its cell's format.
    assert!(in_system(implements[0], "format:common"), "{}", implements[0]);
    assert_eq!(given(implements[0], "format:I1"), Some(true));
    assert!(
        implements[0]["call"]["session"]["started"].is_string(),
        "{}",
        implements[0]["call"]
    );
    assert_eq!(given(implements[1], "format:I1"), Some(false));
    assert!(!in_system(implements[1], "format:common"));
    // The cell's task is given once in a session, under its address, and referred to after.
    assert_eq!(given(implements[0], "task:I1"), Some(true));
    let m = implements[0]["message"].as_str().unwrap();
    assert!(m.contains("## Task\n\n### task:I1\n\n"), "{m}");
    assert_eq!(given(implements[1], "task:I1"), Some(false));
    let m = implements[1]["message"].as_str().unwrap();
    assert!(m.contains("## Task\n\ntask:I1, given to you before in R"), "{m}");
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
        "·D10·answer",
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
    // The fix has a row to answer, a check the full CI on the draft found red, else it would run no round.
    e.db.write(Actor::Engine, |tx| {
        let n: i64 = tx.query_row("SELECT max(n) + 1 FROM rounds WHERE work = ?1", [work], |r| r.get(0))?;
        let red = json!({ "result": { "kind": "red", "checks": [{ "name": "test", "result": "red", "run": "r" }] } });
        record_round(tx, work, n, "T2", &red, "for_engineer")
    })
    .unwrap();
    move_to(&mut e, work, "T6");
    e.step(None).unwrap();
    let log = said.lock().unwrap().join("\n");
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
    assert_eq!(lines.len(), 3, "{lines:?}\n{}", said.lock().unwrap().join("\n"));
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
    let s = setup_priced("limit");
    let mut scripts = scripts();
    scripts.insert(
        "R2",
        Box::new(|_: &Request, _: usize| (Vec::new(), vec![("@usage", used(1.0, 499_997))])),
    );
    let (mut e, said, item) = start(&s, scripts);
    agent_rounds_to(&mut e, item, 2);
    let (work, at, _) = state(&e, item).unwrap();
    assert_eq!(at, "D1", "{}", said.lock().unwrap().join("\n"));
    pause_by_owner(&mut e, work, "to look at it");
    let from = last_call(&e, work);
    assert_eq!(warm_now(&mut e, from + 55 * 60_000 + 1_000, None).unwrap(), 0);
    let_go_on(&mut e, work);
    e.step(None).unwrap();
    let log = said.lock().unwrap().join("\n");
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
    assert_eq!(given(two, "format:D1"), Some(true));
    for a in ["format:common", "rule:ground"] {
        assert!(in_system(two, a), "{a}: {two}");
    }
    assert!(two["call"]["session"]["started"].is_string(), "{}", two["call"]);
    let _ = std::fs::remove_dir_all(&s.root);
}

// A work that waits has the session its next round goes on in pinged 55 minutes after the session's last call, while
// the cache still holds it, and every 55 minutes after, at most 39 times in a wait on Sonnet 5.5, whose write to the
// cache costs forty reads. Each ping runs the last round's command line again, resuming the session, records where the
// work waited and for what, and costs what the session grew by, which the next round's cost leaves out.
#[test]
fn a_waiting_session_is_kept_warm_until_the_wait_has_cost_one_rewrite() {
    let s = setup_priced("warm");
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
    let seen: Arc<Mutex<Vec<Replayed>>> = Arc::default();
    let pinged = seen.clone();
    scripts.insert(
        "@ping",
        Box::new(move |r: &Request, n: usize| {
            pinged
                .lock().unwrap()
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
    assert_eq!(at, "D1", "{}", said.lock().unwrap().join("\n"));
    pause_by_owner(&mut e, work, "to look at it");
    let from = last_call(&e, work);
    // Not before 55 minutes, and not once the cache has let the session go.
    assert_eq!(warm_now(&mut e, from + 54 * 60_000, None).unwrap(), 0);
    assert_eq!(warm_now(&mut e, from + 60 * 60_000, None).unwrap(), 0);
    let mut now = from + 55 * 60_000;
    assert_eq!(warm_now(&mut e, now, None).unwrap(), 1);
    assert_eq!(warm_now(&mut e, now + 60_000, None).unwrap(), 0);
    for _ in 0..40 {
        now += 55 * 60_000 + 1_000;
        warm_now(&mut e, now, None).unwrap();
    }
    let log = said.lock().unwrap().join("\n");
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
    let (replayed, prompt) = seen.lock().unwrap()[0].clone();
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

// While a work goes on without a line, the line's session is kept warm as long as the flow can come back to it: the
// review's, while the work stands at fixing, is pinged for the work going on; the engineer's implementing session is
// not, since fixing starts the engineer a new session, but is once the work stands where a way leads back to it
// without passing fixing, as choosing the way on after an interjection.
#[test]
fn the_lines_a_work_can_come_back_to_are_kept_warm_while_it_goes_on() {
    let s = setup_priced("warm-lines");
    let (mut e, said, item) = start(&s, scripts());
    step_to(&mut e, item, "T4");
    let (work, _, _) = state(&e, item).unwrap();
    // Each round recorded what it used, as the command line reports it.
    let usage = used(1.0, 9_000).to_string();
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "UPDATE rounds SET usage = ?2 WHERE work = ?1 AND session IS NOT NULL",
            params![work, usage],
        )?;
        Ok(())
    })
    .unwrap();
    let from: i64 =
        e.db.conn()
            .query_row(
                "SELECT max(ended_at) FROM rounds WHERE work = ?1 AND session IS NOT NULL",
                [work],
                |r| r.get(0),
            )
            .unwrap();
    let pinged = |e: &Engine| -> Vec<(String, Value)> {
        engine::query_json(
            e.db.conn(),
            "SELECT s.line, p.waited FROM pings p JOIN sessions s ON s.id = p.session WHERE s.work = ?1 ORDER BY p.id",
            &[&work],
            100,
        )
        .unwrap()
        .into_iter()
        .map(|r| (r["line"].as_str().unwrap().to_string(), r["waited"].clone()))
        .collect()
    };
    let now = from + 55 * 60_000 + 1_000;
    move_to(&mut e, work, "T6");
    let log = said.lock().unwrap().join("\n");
    // The review comes back after the fix, and the reader of the history at a look-up the fix may ask for.
    assert_eq!(warm_now(&mut e, now, None).unwrap(), 2, "{log}");
    let going_on = json!({ "cell": "T6", "on": "work", "what": "the work goes on" });
    let mut pings = pinged(&e);
    pings.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(
        pings,
        [
            ("librarian".to_string(), going_on.clone()),
            ("review".to_string(), going_on)
        ]
    );
    move_to(&mut e, work, "M1");
    assert_eq!(warm_now(&mut e, now + 2_000, None).unwrap(), 1, "{log}");
    let pings = pinged(&e);
    assert_eq!(pings.len(), 3, "{pings:?}");
    assert_eq!((pings[2].0.as_str(), &pings[2].1["cell"]), ("engineer", &json!("M1")));
    let _ = std::fs::remove_dir_all(&s.root);
}

// No ping goes to a session whose next round starts a new one, at a cell the line starts one at, nor beside a round of
// the session that runs.
#[test]
fn no_ping_before_a_new_session_or_beside_a_round() {
    let s = setup_priced("no-warm");
    let mut scripts = scripts();
    scripts.insert(
        "D11",
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
    assert_eq!(warm_now(&mut e, from + 55 * 60_000 + 1_000, None).unwrap(), 0);
    // At a cell that goes on in the session, a round of the session that runs keeps the ping off.
    move_to(&mut e, work, "D11");
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
                 VALUES (?1, ?2, 'D11', ?3, '{}', 'c', ?4)",
                params![work, n, session, from],
            )?;
            Ok(n)
        })
        .unwrap();
    assert_eq!(warm_now(&mut e, from + 55 * 60_000 + 1_000, None).unwrap(), 0);
    e.db.write(Actor::Engine, |tx| {
        tx.execute("DELETE FROM rounds WHERE work = ?1 AND n = ?2", [work, open])?;
        Ok(())
    })
    .unwrap();
    assert_eq!(
        warm_now(&mut e, from + 55 * 60_000 + 1_000, None).unwrap(),
        1,
        "{}",
        said.lock().unwrap().join("\n")
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// A work waiting for the architect's reply has the session of the line that asked kept warm, and the ping says the work
// waited at the architect's cell for the architect's reply.
#[test]
fn a_ping_says_where_the_work_waited_and_for_what() {
    let s = setup_priced("warm-architect");
    let mut scripts = scripts();
    scripts.insert(
        "R2",
        Box::new(|_: &Request, _: usize| (Vec::new(), vec![("@usage", used(1.0, 9_000))])),
    );
    let (mut e, said, item) = start(&s, scripts);
    agent_rounds_to(&mut e, item, 2);
    let (work, at, _) = state(&e, item).unwrap();
    assert_eq!(at, "D1", "{}", said.lock().unwrap().join("\n"));
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "UPDATE works SET at = 'A2', resume_at = 'D1', waiting = '[\"D1\"]' WHERE id = ?1",
            [work],
        )?;
        let round = next_round(tx, work)?;
        tx.execute(
            "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES (?1, ?2, 'A2', '{}', 'c', 1)",
            [work, round],
        )?;
        Ok(())
    })
    .unwrap();
    let from = last_call(&e, work);
    assert_eq!(
        warm_now(&mut e, from + 55 * 60_000 + 1_000, None).unwrap(),
        1,
        "{}",
        said.lock().unwrap().join("\n")
    );
    let pings = pings_of(&e, work);
    assert_eq!(
        pings[0].0,
        json!({ "cell": "A2", "on": "architect", "what": "the architect's reply" })
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
    let read: Arc<Mutex<Option<GivenAgain>>> = Arc::default();
    let seen = read.clone();
    scripts.insert(
        "R2",
        Box::new(move |r: &Request, _: usize| {
            let file = r.regive.clone().unwrap();
            let text = std::fs::read_to_string(&file).unwrap_or_default();
            *seen.lock().unwrap() = Some((file, text));
            (Vec::new(), Vec::new())
        }),
    );
    let (mut e, said, item) = start(&s, scripts);
    agent_rounds_to(&mut e, item, 2);
    let rounds = agent_rounds(&e, item);
    let (id, cell, input) = &rounds[1];
    assert_eq!(cell, "R2", "{}", said.lock().unwrap().join("\n"));
    let (file, text) = read.lock().unwrap().clone().expect("the file the hook gives again");
    // The session holds the project guide, so the message only refers to it; the file gives it whole, and none of
    // what the session's system prompt holds, which a compaction leaves.
    let guide = |i: &Value| guide_in(i).map(|g| g.0).unwrap();
    assert_eq!(guide_in(input).map(|g| g.1), Some(false));
    assert!(
        text.starts_with(&format!("Your session was compacted in the middle of {id}"))
            && text.contains(&format!("# {id} · cell R2"))
            && text.contains(&format!("### {}", guide(input)))
            && text.contains("### format:R2")
            && !text.contains("### format:common"),
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
    let log = said.lock().unwrap().join("\n");
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

/// What scripted rounds were sent, in order: each one's cell, its message's text, and each image given after the text,
/// by its address, its media type and its bytes.
type Sent = Arc<Mutex<Vec<(String, String, Vec<(String, String, Vec<u8>)>)>>>;

/// Keeps what a scripted round was sent.
fn keep_sent(sent: &Sent, r: &Request) {
    let images = r
        .images
        .iter()
        .map(|i| (i.address.clone(), i.media_type.clone(), i.bytes.clone()))
        .collect();
    sent.lock().unwrap().push((r.cell.clone(), r.prompt.clone(), images));
}

/// The addresses of images, as agents are given them.
fn image_addresses(ids: &[i64]) -> Vec<String> {
    ids.iter().map(|i| db::attachments::address(*i)).collect()
}

// What the owner says to the engineer with two images while a round runs is not handed in by the hook, which adds text
// only: the line's next round is given it, the words ending with the line that names the images and the images given
// after the message's text, each after its address, in the order the owner added them. The round's record says which
// images were given, by their attachments, with the words that named them.
#[test]
fn the_owner_s_images_reach_the_engineer_s_next_round_with_the_words() {
    let s = setup("images-engineer");
    let mut scripts = scripts();
    let data = s.data.clone();
    scripts.insert(
        "R1",
        Box::new(move |r: &Request, _: usize| {
            let mut db = engine::database::open(&data).unwrap();
            let said = api::WorkSaid {
                text: "Look at these two.".into(),
                speaker: "owner".into(),
                to: None,
                question: None,
                design_card: None,
                images: two_images(),
            };
            api::say_to_work(&mut db, r.work, &said).unwrap();
            let n: i64 = db
                .conn()
                .query_row("SELECT max(n) FROM rounds WHERE work = ?1", [r.work], |x| x.get(0))
                .unwrap();
            assert_eq!(
                sessions::interject::deliver(&mut db, r.work, n, "engineer").unwrap(),
                None
            );
            (Vec::new(), Vec::new())
        }),
    );
    let sent: Sent = Arc::default();
    let seen = sent.clone();
    scripts.insert(
        "R2",
        Box::new(move |r: &Request, _: usize| {
            keep_sent(&seen, r);
            (
                vec![("outcome", "answers")],
                vec![("outcome.answers", json!([{ "to": "owner", "text": "I see both." }]))],
            )
        }),
    );
    let (mut e, said, item) = start(&s, scripts);
    agent_rounds_to(&mut e, item, 2);
    let log = said.lock().unwrap().join("\n");
    let j: i64 =
        e.db.conn()
            .query_row(
                "SELECT id FROM interjections WHERE text = 'Look at these two.'",
                [],
                |r| r.get(0),
            )
            .unwrap();
    let ids = db::attachments::of(e.db.conn(), db::attachments::Words::Interjection(j)).unwrap();
    let a = image_addresses(&ids);
    assert_eq!(a.len(), 2);
    let sent = sent.lock().unwrap();
    let (cell, prompt, images) = sent.first().unwrap_or_else(|| panic!("no round at R2:\n{log}"));
    assert_eq!(cell, "R2");
    assert!(
        prompt.contains(&format!(
            "From the owner: Look at these two.\n\nImages: {}, {}",
            a[0], a[1]
        )),
        "{prompt}"
    );
    assert!(
        prompt.contains(&format!(
            "- {} (PNG, 1 KB), with ij:{j}: follows this message's text",
            a[0]
        )) && prompt.contains(&format!(
            "- {} (JPEG, 1 KB), with ij:{j}: follows this message's text",
            a[1]
        )),
        "{prompt}"
    );
    let given_images: Vec<(&str, &str)> = images.iter().map(|i| (i.0.as_str(), i.1.as_str())).collect();
    assert_eq!(
        given_images,
        [(a[0].as_str(), "image/png"), (a[1].as_str(), "image/jpeg")]
    );
    assert_eq!(images[0].2, b"\x89PNG\r\n\x1a\n a screenshot");
    assert_eq!(images[1].2, b"\xff\xd8\xff\xe0 a photo");
    let rounds = agent_rounds(&e, item);
    let input = &rounds[1].2;
    assert_eq!(
        input["images"],
        json!([
            { "attachment": ids[0], "with": format!("ij:{j}"), "given": true },
            { "attachment": ids[1], "with": format!("ij:{j}"), "given": true }
        ]),
        "{input}"
    );
    assert_eq!(given(input, &a[0]), Some(true), "{input}");
    // The round that ran when the owner said it was given no image.
    assert!(rounds[0].2.get("images").is_none(), "{}", rounds[0].2);
    let _ = std::fs::remove_dir_all(&s.root);
}

// The owner's words to the architect with two images come to its queue as a matter that names them: the round that
// takes the words is given the images after its message's text. When the matter comes back to that cell in the same
// session with the owner's reply to what it asked, and the image sent with the reply, the words are only referred to and
// their images not given again, while the reply's image is given.
#[test]
fn the_owner_s_images_reach_the_architect_once_in_a_session() {
    let s = setup("images-architect");
    no_reply_wait(&s);
    let mut agents = scripts();
    picks_first(&s, &mut agents);
    let sent: Sent = Arc::default();
    let seen = sent.clone();
    agents.insert(
        "H8",
        Box::new(move |r: &Request, n: usize| {
            keep_sent(&seen, r);
            if n == 1 {
                return asks_the_owner("Which of the two do you mean?");
            }
            (
                vec![("outcome.answer", "done")],
                vec![("outcome.answer.reply", json!("The left one, then."))],
            )
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    let images = api::images(&two_images()).unwrap();
    db::say_to_architect(
        &mut e.db,
        "p",
        None,
        None,
        "Look at these two.",
        &images,
        db::Speaker::Owner,
    )
    .unwrap();
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let (matter, ask): (i64, i64) =
        e.db.conn()
            .query_row(
                "SELECT id, asked FROM architect_matters WHERE kind = 'owner_words'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
    let words = image_addresses(&db::attachments::of(e.db.conn(), db::attachments::Words::Matter(matter)).unwrap());
    {
        let sent = sent.lock().unwrap();
        let (_, prompt, images) = &sent[0];
        assert!(
            prompt.contains(&format!("\"images\":[\"{}\",\"{}\"]", words[0], words[1])),
            "{prompt}"
        );
        let blocks: Vec<&str> = images.iter().map(|i| i.0.as_str()).collect();
        assert_eq!(blocks, [words[0].as_str(), words[1].as_str()]);
    }
    let reply = vec![api::ImageSent {
        media_type: "image/gif".into(),
        data: db::attachments::base64(b"GIF89a the left one"),
    }];
    api::reply_to_question(
        &mut e.db,
        "p",
        ask,
        "The left one.",
        &api::images(&reply).unwrap(),
        db::Speaker::Owner,
    )
    .unwrap();
    let again = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, again)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let answered = image_addresses(&db::attachments::of(e.db.conn(), db::attachments::Words::Ask(ask)).unwrap());
    let sent = sent.lock().unwrap();
    assert_eq!(sent.len(), 2, "{log}");
    let (_, prompt, images) = &sent[1];
    assert!(
        prompt.contains("The left one.") && prompt.contains(&answered[0]),
        "{prompt}"
    );
    let blocks: Vec<&str> = images.iter().map(|i| i.0.as_str()).collect();
    assert_eq!(blocks, [answered[0].as_str()], "{prompt}");
    let input = input_at(&e, again, "H8", false);
    assert_eq!(given_ending(&input, "·engine·matter"), Some(false), "{input}");
    assert_eq!(given(&input, &words[0]), None, "{input}");
    assert_eq!(given(&input, &answered[0]), Some(true), "{input}");
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
    let log = said.lock().unwrap().join("\n");
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

// When the agents' login account changes, as when the owner switches it on the Settings page, the line's next round
// starts a new session under the new one: the CLI keeps a session in the configuration directory it ran in, which
// another account's is not, so the session could not be resumed there.
#[test]
fn a_new_login_account_starts_a_new_session_at_the_lines_next_round() {
    let s = setup("account");
    let (mut e, said, item) = start(&s, scripts());
    agent_rounds_to(&mut e, item, 2);
    let (work, _, _) = state(&e, item).unwrap();
    let install = std::fs::read_to_string(s.data.join("install.toml")).unwrap();
    let switched = install.replace(
        "command = \"claude\"\n",
        "command = \"claude\"\nconfig_dir = \"agents/claude-other\"\n",
    );
    assert_ne!(switched, install);
    write(&s.data, "install.toml", &switched);
    git(&s.data, &["commit", "-q", "-am", "another login account"]);
    e.step(None).unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(
        line_sessions(&e, work, "engineer"),
        [
            ("line".to_string(), true, "R1,R2".to_string()),
            ("account".to_string(), false, "D1".to_string())
        ],
        "{log}"
    );
    let home: String = e
        .db
        .conn()
        .query_row(
            "SELECT json_extract(input, '$.call.env.CLAUDE_CONFIG_DIR') FROM rounds WHERE work = ?1 AND cell = 'D1'",
            [work],
            |r| r.get(0),
        )
        .unwrap();
    assert!(home.ends_with("agents/claude-other"), "{home}");
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

// At a project's first architect work its architect is told what happened in the week before it, as at a later work
// since the last; its board holds every item not done: one whose last work stopped, queued again, with why; one whose
// work asks the owner, at the stage of the cell it stopped at, its pause said once, with who paused it; and one whose
// work runs tools, at the stage of the cell that waits for them; and the owner's start queue, the stopped item first in
// it, which starts ahead of the ranks: in the sixth trial a plan put an item first while the owner's queue started
// another, which no cell was given.
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
                 VALUES (?1, ?2, 'engineer', 'A2', 'I1', 'later', 'owner')",
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
            // The owner put the stopped item first to start, by hand.
            tx.execute("INSERT INTO dispatch_queue (item, position) VALUES (?1, 1.0)", [stopped])?;
            Ok((stopped, asking, tools))
        })
        .unwrap();
    say(&mut e, "Look at the items.");
    // A work of the architect starts for what was said, beside the step of the work that merged.
    e.tick().unwrap();
    let arch: i64 =
        e.db.conn()
            .query_row("SELECT id FROM works WHERE graph = 'architect'", [], |r| r.get(0))
            .unwrap();
    assert!(
        steps_until(&mut e, 10, |e| path_of(e, arch).iter().any(|c| c == "U1")),
        "{}",
        said.lock().unwrap().join("\n")
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
    assert_eq!(
        row(asking)["paused"],
        json!({ "by": "owner", "why": "later" }),
        "{board}"
    );
    assert_eq!(row(asking)["waits"], "the architect's reply to its question", "{board}");
    assert_eq!(row(tools)["stage"], "implement", "{board}");
    assert_eq!(board["owner_start_queue"], json!([format!("#{stopped}")]), "{board}");
    assert_eq!(row(stopped)["in_owner_queue"], 1, "{board}");
    assert_eq!(row(asking).get("in_owner_queue"), None, "{board}");
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
        steps_until(&mut e, 20, |e| path_of(e, arch).iter().any(|c| c == "N1")),
        "{}",
        said.lock().unwrap().join("\n")
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

// A round of the understanding that asks for tools writes nothing in the notebook: until the cell answers, the project's
// notebook is empty, as at its architect's first work, and the cell's next round is still told it is the first work.
// The cell's own inputs that have nothing are listed as none; the tool results, which every cell may have, only once
// there are some.
#[test]
fn a_round_of_the_understanding_that_asks_for_tools_changes_neither_the_notebook_nor_the_task() {
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
            .query_row("SELECT count(*) FROM notebook WHERE project = 'p'", [], |r| r.get(0))
            .unwrap()
    };
    assert!(
        steps_until(&mut e, 10, took("tools")),
        "{}",
        said.lock().unwrap().join("\n")
    );
    assert_eq!(states(&e), 0, "{}", said.lock().unwrap().join("\n"));
    assert!(
        steps_until(&mut e, 10, took("understood")),
        "{}",
        said.lock().unwrap().join("\n")
    );
    assert_eq!(states(&e), 2);
    let (first, second) = (message_at(&e, arch, "U1", false), message_at(&e, arch, "U1", true));
    // The first round is given the task of a first work; the round after its tools is told it holds it.
    assert!(first.contains("This is your first work on the project"), "{first}");
    assert!(!second.contains("This is your first work on the project"), "{second}");
    assert!(
        second.contains("task:U1·entry=first, given to you before in R"),
        "{second}"
    );
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
    let before = number(&|x| x.from == [flow::format::Src::Engine("notebook_unclear".into())]);
    let tools = number(&|x| x.name.as_deref() == Some("Tool results"));
    let listed = |m: &str, n: usize| work_from(m).into_iter().find(|l| l.starts_with(&format!("{n}. ")));
    assert_eq!(listed(&first, before), Some(format!("{before}. none")), "{first}");
    assert_eq!(listed(&first, tools), None, "{first}");
    assert!(listed(&second, tools).is_some_and(|l| l.contains("·X1·")), "{second}");
    let _ = std::fs::remove_dir_all(&s.root);
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

// The understanding rewrites the notebook's sections on what the project is now and on what is unclear, and the plan
// its plan: a return of the understanding that rewrites only one of its two, and one of the plan that leaves the plan
// as it was, are sent back saying which section to write; handed in again whole, they are the notebook's.
#[test]
fn a_return_of_the_understanding_or_the_plan_without_its_sections_is_sent_back() {
    let s = setup("notebook-written");
    let mut agents = scripts();
    agents.insert(
        "U1",
        Box::new(|_: &Request, n: usize| {
            if n > 1 {
                return (Vec::new(), Vec::new());
            }
            let edits = json!([{ "section": "project_now", "text": "One line of work: the parser." }]);
            (Vec::new(), vec![("outcome.notebook", edits)])
        }),
    );
    agents.insert(
        "N1",
        Box::new(|_: &Request, n: usize| {
            if n > 1 {
                return (Vec::new(), Vec::new());
            }
            let edits = json!([{ "section": "other", "text": "Nothing else." }]);
            (Vec::new(), vec![("outcome.notebook", edits)])
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    say(&mut e, "Look.");
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 60, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let understanding = why_sent_back(&e, arch, "U1");
    assert_eq!(understanding.len(), 1, "{understanding:?}\n{log}");
    assert!(
        understanding[0].contains("rewrite the notebook's section unclear whole in notebook"),
        "{understanding:?}"
    );
    let plan = why_sent_back(&e, arch, "N1");
    assert_eq!(plan.len(), 1, "{plan:?}\n{log}");
    assert!(
        plan[0].contains("rewrite the notebook's section plan whole in notebook"),
        "{plan:?}"
    );
    let sections: Vec<(String, String)> = notebook(&e).into_iter().map(|x| (x.0, x.2)).collect();
    assert_eq!(
        sections,
        [
            ("plan".to_string(), "N1".to_string()),
            ("project_now".to_string(), "U1".to_string()),
            ("unclear".to_string(), "U1".to_string())
        ],
        "{log}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// A section of the notebook holds 4,000 characters at most and the four 12,000 in all, the sections a round leaves as
// they are counted as they stand, and a return names each section once: a return past them is sent back saying which
// section and how many characters, and nothing of it is written.
#[test]
fn a_notebook_past_its_size_is_sent_back_saying_which_section() {
    let s = setup("notebook-size");
    let mut agents = scripts();
    agents.insert(
        "H8",
        Box::new(|_: &Request, n: usize| {
            let long = |k: usize| "x".repeat(k);
            let edits = match n {
                1 => json!([{ "section": "plan", "text": long(4_001) }, { "section": "plan", "text": "Short." }]),
                2 => json!([
                    { "section": "plan", "text": long(4_000) },
                    { "section": "unclear", "text": long(4_000) },
                    { "section": "other", "text": long(4_000) }
                ]),
                _ => json!([{ "section": "other", "text": "Kept." }]),
            };
            (Vec::new(), vec![("outcome.notebook", edits)])
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    say(&mut e, "Look.");
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 60, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let back = why_sent_back(&e, arch, "H8");
    assert_eq!(back.len(), 2, "{back:?}\n{log}");
    assert!(
        back[0].contains("the notebook's section plan is 4001 characters, and a section holds 4000 at most"),
        "{back:?}"
    );
    assert!(
        back[0].contains("the notebook's section plan is rewritten twice"),
        "{back:?}"
    );
    // What the project is now, as the understanding wrote it, counts as it stands.
    let now = "The project_now at round 1.".chars().count();
    assert!(
        back[1].contains(&format!(
            "the notebook would be {} characters in all, and it holds 12000 at most",
            12_000 + now
        )),
        "{back:?}"
    );
    let kept: i64 =
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM notebook_edits WHERE project = 'p' AND length(text) >= 4000",
                [],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(kept, 0);
    assert!(
        notebook(&e).contains(&("other".to_string(), "Kept.".to_string(), "H8".to_string())),
        "{log}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// A section of the notebook a cell rewrites is the notebook's latest version of it at once, every version kept; each
// section is an input of its own of the architect's cells, so that the next cell's session is given a section that
// changed whole and one that did not only by reference. The answer to the owner's words keeps something in the other
// section, which the check of the policy after it is given whole while it is referred to what the project is now; the
// plan, made again after a batch, is given the plan it made before, and the plan is given whole to the first cell of
// the batch's end that reads it.
#[test]
fn the_notebook_is_written_where_it_is_learned_and_given_whole_once_changed() {
    let s = setup("notebook-given");
    let mut agents = scripts();
    agents.insert(
        "H8",
        Box::new(|_: &Request, _: usize| {
            let edits = json!([{ "section": "other", "text": "The owner looks at the parser first." }]);
            (Vec::new(), vec![("outcome.notebook", edits)])
        }),
    );
    agents.insert(
        "B4",
        Box::new(|_: &Request, n: usize| {
            if n == 1 {
                (vec![("outcome.rethink", "re_plan")], Vec::new())
            } else {
                (Vec::new(), Vec::new())
            }
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    say(&mut e, "Look at the parser first.");
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 80, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let path = path_of(&e, arch);
    assert_eq!(path.iter().filter(|c| *c == "N1").count(), 2, "{path:?}\n{log}");
    assert_eq!(
        notebook(&e),
        [
            (
                "other".to_string(),
                "The owner looks at the parser first.".to_string(),
                "H8".to_string()
            ),
            ("plan".to_string(), "The plan at round 2.".to_string(), "N1".to_string()),
            (
                "project_now".to_string(),
                "The project_now at round 1.".to_string(),
                "U1".to_string()
            ),
            (
                "unclear".to_string(),
                "The unclear at round 1.".to_string(),
                "U1".to_string()
            )
        ],
        "{log}"
    );
    let plans: i64 =
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM notebook_edits WHERE project = 'p' AND section = 'plan'",
                [],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(plans, 2);
    // The understanding wrote what the project is now: the answer to the owner's words, the next cell, is given it
    // whole, and has nothing in the other section yet.
    let answer = input_at(&e, arch, "H8", false);
    assert_eq!(
        given_ending(&answer, "·engine·notebook_project_now"),
        Some(true),
        "{answer}"
    );
    assert_eq!(given_ending(&answer, "·engine·notebook_other"), None, "{answer}");
    // The check of the policy after it is referred to what did not change and given the other section whole.
    let check = input_at(&e, arch, "P1", false);
    assert_eq!(
        given_ending(&check, "·engine·notebook_project_now"),
        Some(false),
        "{check}"
    );
    assert_eq!(given_ending(&check, "·engine·notebook_unclear"), Some(false), "{check}");
    assert_eq!(given_ending(&check, "·engine·notebook_other"), Some(true), "{check}");
    assert!(
        check["message"]
            .as_str()
            .unwrap()
            .contains("The owner looks at the parser first."),
        "{check}"
    );
    // The plan made again is given the plan before, which the first cell after the batch's end that reads it, the
    // recording of the decisions, was given whole, and the rethink after that only by reference.
    let (first, again) = (input_at(&e, arch, "N1", false), input_at(&e, arch, "N1", true));
    assert_eq!(given_ending(&first, "·engine·notebook_plan"), None, "{first}");
    let record = input_at(&e, arch, "B5", false);
    assert_eq!(given_ending(&record, "·engine·notebook_plan"), Some(true), "{record}");
    assert!(
        record["message"].as_str().unwrap().contains("The plan at round 1."),
        "{record}"
    );
    let rethink = input_at(&e, arch, "B4", false);
    assert_eq!(
        given_ending(&rethink, "·engine·notebook_plan"),
        Some(false),
        "{rethink}"
    );
    assert_eq!(given_ending(&again, "·engine·notebook_plan"), Some(false), "{again}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The understanding of a new session understands the project afresh: it is not given the notebook's section on what
// the project is now, which it writes anew, and is given what is unclear, as questions to settle.
#[test]
fn a_new_session_s_understanding_is_given_what_is_unclear_and_not_what_the_project_is_now() {
    let s = setup("notebook-new-session");
    let (mut e, said) = engine(&s, scripts());
    quiet(&mut e);
    an_item(&mut e);
    say(&mut e, "Look.");
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 20, |e| e.work(arch).unwrap().pos.at == "H8"),
        "{}",
        said.lock().unwrap().join("\n")
    );
    e.sessions().ask_new(api::Fresh::Architect("p".into()));
    assert!(
        steps_until(&mut e, 60, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    assert_eq!(path_of(&e, arch).iter().filter(|c| *c == "U1").count(), 2, "{log}");
    let again = input_at(&e, arch, "U1", true);
    assert_eq!(given_ending(&again, "·engine·notebook_project_now"), None, "{again}");
    assert_eq!(given_ending(&again, "·engine·notebook_unclear"), Some(true), "{again}");
    let m = again["message"].as_str().unwrap();
    assert!(m.contains("The unclear at round 1."), "{m}");
    assert!(!m.contains("The project_now at round 1."), "{m}");
    let now = notebook(&e).into_iter().find(|x| x.0 == "project_now").unwrap();
    assert_eq!(now.1, "The project_now at round 2.");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The check of the policy writes a new policy only once the owner said yes: a check a new session's understanding
// queued that hands in write_policy without the owner's reply to the question whether to research one is sent back
// saying to ask first.
#[test]
fn a_check_of_the_policy_writes_a_new_one_only_once_the_owner_said_yes() {
    let s = setup("policy-unasked");
    let mut agents = scripts();
    agents.insert(
        "P1",
        Box::new(|_: &Request, n: usize| {
            if n == 1 {
                (vec![("outcome.verdict", "write_policy")], Vec::new())
            } else {
                (Vec::new(), Vec::new())
            }
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    say(&mut e, "Look.");
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 60, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let back = why_sent_back(&e, arch, "P1");
    assert_eq!(back.len(), 1, "{back:?}\n{log}");
    assert!(
        back[0].contains("the owner has not said yes to a new policy: ask the owner first"),
        "{back:?}"
    );
    assert_eq!(rows_at(&e, arch, "P1"), ["holds"], "{log}");
    assert!(!path_of(&e, arch).contains(&"P2".to_string()), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A check of the policy the owner's own words asked for, queued by the answer to them, has the owner's yes already:
// it writes a new policy without asking.
#[test]
fn a_check_the_owner_s_words_asked_for_writes_a_new_policy_without_asking() {
    let s = setup("policy-asked-by-words");
    let mut agents = scripts();
    agents.insert(
        "H8",
        Box::new(|_: &Request, _: usize| {
            (
                vec![("outcome.answer", "new_policy")],
                vec![
                    ("outcome.answer.reply", json!("I will write a new policy.")),
                    ("outcome.answer.why", json!("The owner wants the parser first.")),
                ],
            )
        }),
    );
    agents.insert(
        "P1",
        Box::new(|_: &Request, _: usize| (vec![("outcome.verdict", "write_policy")], Vec::new())),
    );
    agents.insert("P2", writes_a_policy());
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    say(&mut e, "Write a new policy: the parser first.");
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 60, |e| path_of(e, arch).contains(&"P2".to_string())),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    assert_eq!(rows_at(&e, arch, "P1"), ["write_policy"], "{log}");
    assert!(why_sent_back(&e, arch, "P1").is_empty(), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// What was said in a work's group while a work of the architect runs is queued at the end of its batch, for that work,
// which takes it at its cell in its next batch; no other work of the architect starts for it.
#[test]
fn what_was_said_in_the_groups_while_a_work_runs_is_taken_after_its_batch() {
    let s = setup("group-talk-batch");
    let (mut e, said) = engine(&s, scripts());
    quiet(&mut e);
    let item = an_item(&mut e);
    say(&mut e, "Look.");
    let arch = e.step(None).unwrap().work.unwrap();
    let v = e.flow_now;
    e.db.write(Actor::Owner, |tx| {
        tx.execute(
            "INSERT INTO works (item, flow_version, graph, at, paused, paused_by) VALUES (?1, ?2, 'engineer', 'R1', 'later', 'owner')",
            params![item, v],
        )?;
        tx.execute(
            "INSERT INTO interjections (work, line, speaker, text) VALUES (?1, 'engineer', 'owner', 'Keep the parser small.')",
            [tx.last_insert_rowid()],
        )?;
        Ok(())
    })
    .unwrap();
    assert!(
        steps_until(&mut e, 80, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    assert_eq!(rows_at(&e, arch, "H12"), ["done"], "{log}");
    let path = path_of(&e, arch);
    let first_end = path.iter().position(|c| c == "B4").expect(&log);
    let heard = path.iter().position(|c| c == "H12").expect(&log);
    assert!(first_end < heard, "{path:?}");
    assert!(
        message_at(&e, arch, "H12", false).contains("Keep the parser small."),
        "{log}"
    );
    assert_eq!(architect_works(&e), [arch], "{log}");
    let open: i64 =
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM architect_matters WHERE kind = 'group_talk' AND handled_round IS NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(open, 0, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// While new starts are held to set the project straight, the owner's words are still taken first: the hold keeps back
// only the matters from the head of the queue on, a place to set straight coming before the check of the policy and
// the plan.
#[test]
fn the_owner_s_words_come_first_while_new_starts_are_held_to_set_things_straight() {
    let s = setup("straight-words");
    let (mut e, said) = engine(&s, scripts());
    quiet(&mut e);
    an_item(&mut e);
    e.db.write(Actor::Architect, |tx| {
        tx.execute(
            "INSERT INTO dispatch_holds (project, reason, since) VALUES ('p', 'set_straight', 1)",
            [],
        )?;
        let fix = json!({ "where": "item 1", "items": [1], "works": [], "kind": "dependencies",
                          "wrong": "It waits for nothing it needs.", "steps": ["Check its links."] });
        db::queue_matter(
            tx,
            &db::Matter {
                project: "p".into(),
                kind: "set_straight".into(),
                what: "To set straight, item 1: Check its links".into(),
                body: json!({ "fix": fix }),
                ..db::Matter::default()
            },
        )?;
        Ok(())
    })
    .unwrap();
    say(&mut e, "Look.");
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 80, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let path = path_of(&e, arch);
    let at = |cell: &str| {
        path.iter()
            .position(|c| c == cell)
            .unwrap_or_else(|| panic!("{cell}: {path:?}\n{log}"))
    };
    assert!(at("H8") < at("H1") && at("H1") < at("P1"), "{path:?}");
    assert_eq!(rows_at(&e, arch, "Q0")[0], "owner_words", "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

/// What the cell that writes a new policy hands in: the policy, and no new measure.
fn writes_a_policy() -> Script {
    writes_a_policy_with(json!([]))
}

/// What the cell that writes a new policy hands in: a draft written, the policy, and the new measures its signals need.
fn writes_a_policy_with(measures: Value) -> Script {
    Box::new(move |_: &Request, _: usize| {
        let policy = json!({ "direction": "Ship the parser.", "why": "Users wait for it.",
                             "principles": [{ "no": 1, "text": "Parse before checking." }],
                             "signals": [{ "what": "Speed", "how": "merged" }] });
        (
            Vec::new(),
            vec![
                ("outcome.draft", json!({ "draft": "written" })),
                ("outcome.policy", policy),
                ("outcome.new_measures", measures.clone()),
            ],
        )
    })
}

/// The new measures queued for project p's architect, handled or not, each by its words, in the order they came.
fn measures_queued(e: &Engine) -> Vec<String> {
    let mut st =
        e.db.conn()
            .prepare(
                "SELECT json_extract(body, '$.measure.measure') FROM architect_matters
                 WHERE kind = 'set_straight' AND json_extract(body, '$.measure.measure') IS NOT NULL ORDER BY id",
            )
            .unwrap();
    st.query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect()
}

/// The rows a work's rounds at a cell took, oldest first; a round that took none is left out.
fn rows_at(e: &Engine, work: i64, cell: &str) -> Vec<String> {
    cells_of(e, work)
        .into_iter()
        .filter(|c| c.0 == cell)
        .filter_map(|c| c.1)
        .collect()
}

// The writing of a new policy is given the project's design goal, the owner's own words on where it goes, beside the
// constitution, as a work of the architect is given its project's settings.
#[test]
fn the_writing_of_a_new_policy_is_given_the_design_goal() {
    let s = setup("study-goal");
    no_reply_wait(&s);
    let (mut e, _) = engine(&s, scripts());
    quiet(&mut e);
    let (writing, _) = check_at(&mut e, "P2");
    let m = engine::next_message(&mut e, writing).unwrap().text;
    let goal = work_from(&m).into_iter().next().unwrap_or_default();
    assert!(goal.starts_with("1. ") && !goal.ends_with("none"), "{goal}\n{m}");
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

/// A work of project p's architect at a cell of the check of its policy, with no round there yet: the work took the
/// check, a matter at the head of the queue, at its look at the queue. Gives the work and the matter.
fn check_at(e: &mut Engine, cell: &str) -> (i64, i64) {
    let v = e.flow_now;
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO works (item, flow_version, graph, at, project) VALUES (NULL, ?1, 'architect', ?2, 'p')",
            params![v, cell],
        )?;
        let work = tx.last_insert_rowid();
        let m = db::Matter {
            project: "p".into(),
            kind: "check_policy".into(),
            what: "Check whether the policy of p still holds: a test asks for it".into(),
            body: json!({ "asked_by": [] }),
            ..db::Matter::default()
        };
        let matter = db::queue_matter(tx, &m)?;
        record_round(
            tx,
            work,
            1,
            "Q0",
            &json!({ "queue": { "verdict": "head", "matter": matter } }),
            "head",
        )?;
        Ok((work, matter))
    })
    .unwrap()
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

/// The line of the next message of a work that lists its input by number.
fn next_listed(e: &mut Engine, work: i64, number: usize) -> String {
    let m = engine::next_message(e, work).unwrap().text;
    work_from(&m)
        .into_iter()
        .find(|l| l.starts_with(&format!("{number}. ")))
        .unwrap_or_else(|| panic!("{m}"))
}

// `P1` is given only the signals of the policy in force that the engine measures, each with its numbers:
// a signal that names a measure of the numbers by its name. When the policy has none such, the input is none.
#[test]
fn the_policy_s_signals_come_only_when_the_engine_measures_them_with_their_numbers() {
    let s = setup("signals");
    no_reply_wait(&s);
    let (mut e, _) = engine(&s, scripts());
    quiet(&mut e);
    let set = |e: &mut Engine, signals: Value| {
        let v = e.flow_now;
        let policy = json!({ "direction": "d", "why": "w", "principles": [{ "no": 1, "text": "t" }],
                             "signals": signals });
        let written =
            json!({ "outcome": { "policy": policy, "new_measures": [] }, "progress": "Written.", "tell_owner": [] });
        e.db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO works (item, flow_version, graph, at, ended, project)
                 VALUES (NULL, ?1, 'architect', 'S1', 'over', 'p')",
                [v],
            )?;
            let set = tx.last_insert_rowid();
            record_round(tx, set, 1, "P2", &written, "written")?;
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
    set(
        &mut e,
        json!([{ "what": "The owner is unhappy", "how": "something the owner says" }]),
    );
    assert_eq!(next_listed(&mut e, arch, 4), "4. none");
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
    assert_eq!(held.started_item, None, "{}", said.lock().unwrap().join("\n"));
    e.db.write(Actor::Engine, |tx| {
        tx.execute("UPDATE architect_item_changes SET state = 'made'", [])?;
        Ok(())
    })
    .unwrap();
    let started = e.step(None).unwrap();
    assert_eq!(started.started_item, Some(item), "{}", said.lock().unwrap().join("\n"));
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
    assert_eq!(
        started.started_item,
        Some(library),
        "{}",
        said.lock().unwrap().join("\n")
    );
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
    assert_eq!(at(&e), "A6", "{}", said.lock().unwrap().join("\n"));
    let projects = e.config.config.projects.clone();
    e.db.write(Actor::Owner, |tx| {
        engine::ending::cancel_item(tx, lexer, "no longer needed", "owner", &projects).map_err(db::Error::Abort)
    })
    .unwrap();
    e.step_work(work).unwrap();
    assert_eq!(at(&e), "A4", "{}", said.lock().unwrap().join("\n"));
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
    for _ in 0..12 {
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
        said.lock().unwrap().join("\n")
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
        said.lock().unwrap().join("\n")
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
    let log = said.lock().unwrap().join("\n");
    assert_eq!(order.len(), 2, "{log}");
    assert_eq!(order[0].1, "Measure the spend", "{log}");
    assert_eq!(order[1].0, first, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// An item's title has 20 characters at most, spaces not counted: an answer that creates an item, or retitles one, with
// a longer title is sent back saying how long each is, and handed in again with shorter titles, its changes are made.
#[test]
fn an_item_title_over_twenty_characters_is_sent_back() {
    let s = setup("title-too-long");
    let mut agents = scripts();
    agents.insert(
        "H8",
        Box::new(|_: &Request, n: usize| {
            if n > 2 {
                return (Vec::new(), Vec::new());
            }
            // 23 characters and 22, spaces not counted; then 20 and 18.
            let (title, retitle) = if n == 1 {
                ("Let match take integers too", "Parse all the inputs first")
            } else {
                ("Let match take integers", "Parse the input first")
            };
            let changes = json!([
                { "kind": "create_item", "key": "match", "project": "p", "title": title, "problem": "No match.",
                  "why": "Needed.", "acceptance": ["It matches."], "level": "l", "deliver": "change", "reason": "r" },
                { "kind": "edit_item", "item": "1", "part": "title", "text": retitle, "reason": "r" },
            ]);
            (
                Vec::new(),
                vec![("outcome.effects", json!({ "item_changes": changes }))],
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
    an_item(&mut e);
    say(&mut e, "Let match take integers.");
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let rejected = why_sent_back(&e, arch, "H8");
    assert_eq!(rejected.len(), 1, "{rejected:?}");
    assert!(
        rejected[0].contains("item change 1 (create_item): the title it gives has 23 characters")
            && rejected[0].contains("item change 2 (edit_item): the title it gives has 22 characters"),
        "{rejected:?}"
    );
    let titles: Vec<String> = engine::query_json(e.db.conn(), "SELECT title FROM items ORDER BY id", &[], 10)
        .unwrap()
        .iter()
        .map(|r| r["title"].as_str().unwrap_or_default().to_string())
        .collect();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(titles, ["Parse the input first", "Let match take integers"], "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A cancel of an item the batch creates, named by its key, cancels the create out: the item is never made, and the
// create, the changes that name its key and the cancel are dropped as they are collected, saying so, so that the review
// is not given them; in the sixth trial a batch held a create, a move and a cancel of one item, which the review could
// only say in words should go.
#[test]
fn a_batch_that_cancels_an_item_it_creates_never_makes_it() {
    let s = setup("cancel-own");
    let mut agents = scripts();
    let changes = json!([
        { "kind": "create_item", "key": "spend", "project": "p", "title": "Measure the spend", "problem": "Unknown.",
          "why": "Cost.", "acceptance": ["It is measured."], "level": "l", "deliver": "change", "reason": "r" },
        { "kind": "move", "item": "spend", "to": { "place": "first" }, "reason": "r" },
        { "kind": "cancel", "item": "spend", "goes": "Not needed: the owner measures it by hand.", "reason": "r" },
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
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    say(&mut e, "Measure the spend, or not.");
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    assert!(!path_of(&e, arch).contains(&"B2".to_string()), "{log}");
    let rows = engine::query_json(
        e.db.conn(),
        "SELECT state, json_extract(outcome, '$.what') AS why FROM architect_item_changes ORDER BY id",
        &[],
        10,
    )
    .unwrap();
    assert_eq!(rows.len(), 3, "{log}");
    for r in &rows {
        assert_eq!(r["state"], "dropped", "{rows:?}");
        assert!(
            r["why"].as_str().unwrap().contains("cancelled in the same batch"),
            "{rows:?}"
        );
    }
    let made: i64 =
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM items WHERE title = 'Measure the spend'",
                [],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(made, 0, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The review drops a change that should not be made at all, by a row of its own, saying why: the change is left out,
// and when nothing is left to fix the rest is made at once, with no revision; in the sixth trial the review could say
// only in words that changes should go, and the revision had to read it there.
#[test]
fn the_review_drops_a_change_and_the_rest_is_made_without_a_revision() {
    let s = setup("review-drops");
    let mut agents = scripts();
    agents.insert(
        "H8",
        Box::new(|_: &Request, _: usize| {
            let title = json!({ "kind": "edit_item", "item": "1", "part": "title", "text": "Write the parser",
                                "reason": "r" });
            let hold = json!({ "kind": "hold", "item": "1", "reason": "r" });
            (
                Vec::new(),
                vec![("outcome.effects", json!({ "item_changes": [title, hold] }))],
            )
        }),
    );
    agents.insert(
        "B2",
        Box::new(|_: &Request, _: usize| {
            let rows = json!([
                { "change": 1, "verdict": "ok", "why": "Now it is an action." },
                { "change": 2, "verdict": "drop", "why": "Nothing it waits for is missing." }
            ]);
            (Vec::new(), vec![("outcome.review", rows)])
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    say(&mut e, "Name the parser's item, and hold it.");
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let path = path_of(&e, arch);
    let at = path.iter().position(|c| c == "B2").expect(&log);
    assert_eq!(&path[at..at + 2], ["B2", "B4"], "{log}");
    let rows = engine::query_json(
        e.db.conn(),
        "SELECT state, json_extract(outcome, '$.why') AS why FROM architect_item_changes ORDER BY id",
        &[],
        10,
    )
    .unwrap();
    assert_eq!(rows[0]["state"], "made", "{rows:?}");
    assert_eq!(
        (&rows[1]["state"], &rows[1]["why"]),
        (&json!("dropped"), &json!("Nothing it waits for is missing.")),
        "{rows:?}"
    );
    let (title, held): (String, Option<String>) =
        e.db.conn()
            .query_row("SELECT title, held FROM items WHERE id = 1", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
    assert_eq!((title.as_str(), held), ("Write the parser", None), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The side reports handed in before the architect ran, which a migration put in one matter, are given by work and
// cell, each text once, its spaces collapsed, with how many times and in which rounds it was handed in, and how the
// work it came from stands; a report that holds nothing, such as the text of an empty list, is left out. In the sixth
// trial the matter gave 65 reports from five works as one list, most handed in again round after round.
#[test]
fn the_side_reports_handed_in_before_are_given_once_by_work_and_cell() {
    let s = setup("reports-once");
    let (mut e, _) = engine(&s, scripts());
    quiet(&mut e);
    let item = an_item(&mut e);
    let v = e.flow_now;
    let arch = architect_work_at(&mut e, "H4");
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO works (item, flow_version, graph, at, ended) VALUES (?1, ?2, 'engineer', 'I1', 'stopped: by hand')",
            params![item, v],
        )?;
        let work = tx.last_insert_rowid();
        let report = |round: i64, cell: &str, what: &str| {
            json!({ "work": work, "round": round, "cell": cell, "index": 0,
                    "report": { "kind": "other", "what": what, "urgent": false } })
        };
        let reports = json!([
            report(2, "R1", "The lexer drops a token."),
            report(3, "R1", " The lexer  drops a token.\n"),
            { "work": work, "round": 4, "cell": "R1", "index": null, "report": "[]" },
            report(5, "D1", "The lexer drops a token."),
        ]);
        let m = db::queue_matter(
            tx,
            &db::Matter {
                project: "p".into(),
                kind: "report".into(),
                detail: Some("other".into()),
                what: "Look through the 4 side reports handed in before you ran".into(),
                body: json!({ "side_reports": reports }),
                ..db::Matter::default()
            },
        )?;
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
    let given = given_value(&m, "·engine·matter").expect(&m);
    assert_eq!(given["matter"]["body"].get("side_reports"), None, "{given}");
    let groups = given["material"]["side_reports"].as_array().expect(&m).clone();
    let cells: Vec<&str> = groups.iter().map(|g| g["cell"].as_str().unwrap()).collect();
    assert_eq!(cells, ["R1", "D1"], "{given}");
    assert_eq!(groups[0]["work_now"], "it ended: stopped: by hand", "{given}");
    assert_eq!(
        groups[0]["reports"],
        json!([{ "kind": "other", "urgent": false, "what": "The lexer drops a token.", "times": 2, "rounds": [2, 3] }]),
        "{given}"
    );
    assert_eq!(groups[1]["reports"][0]["rounds"], json!([5]), "{given}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The cells that judge what runs and what starts, and the rethink after a batch, are told, with the slots, whether the
// engine runs works or the owner paused every work, and which run despite it; and every cell that hands in item
// changes, and the plan, is told, with the batch, the
// changes collected so far, each whole with the key of an item it creates, not made until the review: in the sixth
// trial `N1` and `H10` were told two slots were free while three works stood mid-flow and could not tell whether the
// engine ran, `H12` could not cancel an item made earlier in the batch, for want of its key, and `N1`, given no batch,
// planned as if the changes it had seen earlier in the batch were sure to be made.
#[test]
fn the_slots_say_whether_works_run_and_the_batch_its_changes_with_their_keys() {
    let s = setup("engine-state");
    let (mut e, _) = engine(&s, scripts());
    quiet(&mut e);
    let arch = architect_work_at(&mut e, "H10");
    e.db.write(Actor::Engine, |tx| {
        let m = db::queue_matter(
            tx,
            &db::Matter {
                project: "p".into(),
                kind: "slots_free".into(),
                what: "2 slots are free, and no item of p is ready to start".into(),
                body: json!({ "free": 2, "ready": 0 }),
                ..db::Matter::default()
            },
        )?;
        record_round(
            tx,
            arch,
            1,
            "Q0",
            &json!({ "queue": { "verdict": "one", "matter": m } }),
            "one",
        )?;
        let create = json!({ "kind": "create_item", "key": "cover", "project": "p", "title": "Cover the lexer",
                             "problem": "p", "why": "w", "acceptance": ["It is covered."], "level": "l",
                             "deliver": "change", "reason": "r" });
        tx.execute(
            "INSERT INTO architect_item_changes (work, round, matter, change, state)
             VALUES (?1, 1, ?2, ?3, 'collected')",
            params![arch, m, create.to_string()],
        )?;
        Ok(())
    })
    .unwrap();
    e.set_global_pause(Some(engine::GlobalPause {
        warm: true,
        runs: [7].into(),
        architects: ["p".to_string()].into(),
    }));
    let m = engine::next_message(&mut e, arch).unwrap().text;
    let slots = given_value(&m, "·engine·slots").expect(&m);
    assert_eq!(slots["engine"]["runs"], "paused", "{slots}");
    assert_eq!(slots["engine"]["started"], json!([7]), "{slots}");
    let batch = given_value(&m, "·engine·batch").expect(&m);
    let change = &batch["item_changes"]["1"];
    assert_eq!(
        (&change["state"], &change["change"]["key"]),
        (&json!("collected"), &json!("cover")),
        "{batch}"
    );
    e.set_global_pause(None);
    let m = engine::next_message(&mut e, arch).unwrap().text;
    let slots = given_value(&m, "·engine·slots").expect(&m);
    assert_eq!(slots["engine"], json!({ "runs": "running" }), "{slots}");
    // The rethink after a batch, which judges whether that calls for planning again, is told the same.
    e.db.write(Actor::Engine, |tx| {
        tx.execute("UPDATE works SET at = 'B4' WHERE id = ?1", [arch])?;
        Ok(())
    })
    .unwrap();
    let m = engine::next_message(&mut e, arch).unwrap().text;
    let slots = given_value(&m, "·engine·slots").expect(&m);
    assert_eq!(slots["engine"], json!({ "runs": "running" }), "{slots}");
    // The plan is told the batch's changes so far with their state, as every cell that hands in item changes is.
    e.db.write(Actor::Engine, |tx| {
        tx.execute("UPDATE works SET at = 'N1' WHERE id = ?1", [arch])?;
        Ok(())
    })
    .unwrap();
    let m = engine::next_message(&mut e, arch).unwrap().text;
    let batch = given_value(&m, "·engine·batch").expect(&m);
    let change = &batch["item_changes"]["1"];
    assert_eq!(
        (&change["state"], &change["change"]["key"]),
        (&json!("collected"), &json!("cover")),
        "{batch}"
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
    let log = said.lock().unwrap().join("\n");
    assert_eq!(
        matters(&e),
        [("report".to_string(), true, false), ("report".to_string(), true, false)],
        "{log}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// A new session of the architect's line that would start in the middle of an architect work, here at a matter's path,
// remembers nothing but the notebook: the work goes back to understanding the project first, in the new session, and the matter
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
        said.lock().unwrap().join("\n")
    );
    e.sessions().ask_new(api::Fresh::Architect("p".into()));
    assert!(
        steps_until(&mut e, 40, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let path = path_of(&e, arch);
    // The work was at `H8`, the matter picked, when the new session was asked for: it took the row for that there.
    let back = path.iter().rposition(|c| c == "S1").expect(&log);
    assert_eq!(&path[back - 1..back + 3], ["H8", "S1", "U1", "Q0"], "{path:?}\n{log}");
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
        said.lock().unwrap().join("\n")
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

// A new observation of a project that has tags carries one of them at least, and a title as short as an item's: one
// with no tag and a title too long is sent back, saying which tags there are and how long the title is, and the
// return handed in again with a tag and a short title records it, its title with it.
#[test]
fn an_observation_with_no_tag_or_a_long_title_is_sent_back() {
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
            let (tags, title) = if n == 1 {
                (json!([]), "Parsing of large files is slow")
            } else {
                (json!(["perf"]), "Slow large parses")
            };
            let new = json!({ "kind": "new", "title": title, "what": "Parsing large files is slow.", "tags": tags,
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
        said.lock().unwrap().join("\n")
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
    assert!(
        rejected[0].contains(
            "observation change 1: the title it gives has 25 characters, spaces not counted, and a title has 20 at most"
        ),
        "{rejected:?}"
    );
    let recorded: Vec<(Value, Value)> =
        engine::query_json(e.db.conn(), "SELECT title, tags FROM observations", &[], 10)
            .unwrap()
            .iter()
            .map(|r| (r["title"].clone(), r["tags"].clone()))
            .collect();
    assert_eq!(
        recorded,
        [(json!("Slow large parses"), json!(["perf"]))],
        "{recorded:?}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
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

/// The owner's Pause of a work, as the page sends it: the work's own pause written as the owner's, its Start over.
fn owner_pauses(e: &mut Engine, c: &api::Control, work: i64) {
    c.pause(&api::Fresh::Work(work));
    api::pause(&mut e.db, work, None).unwrap();
}

/// A setup whose one machine has two slots, and an engine over it with two items in the queue.
fn two_works(name: &str) -> (Setup, Engine, Arc<Mutex<Vec<String>>>) {
    let (s, e, said, _, _) = two_works_with(name, scripts(), Hooks::default());
    (s, e, said)
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

// A work that waits for the architect's reply holds no slot, and another work starts in the slot it left; once the
// reply came, the first goes on only when a slot is free again, so that the machine never runs more works than its
// slots, and then before any new work starts on a ready item; meanwhile the engine names it as waiting for a free slot,
// which the owner's page shows in place of ready. In the sixth trial a work answered went on at once, and the slots
// read three used of two. The architect's replies are written here by hand, each matter taken off the queue as it comes,
// so that no work of the architect starts and the test says when each reply comes.
#[test]
fn a_work_back_from_waiting_goes_on_in_a_free_slot_before_any_new_start() {
    let s = setup("back-to-a-slot");
    let mut agents = scripts();
    // The first work to research asks the architect; the first to weigh whether the problem stands, the second work,
    // too.
    for cell in ["R1", "R2"] {
        agents.insert(
            cell,
            Box::new(|_: &Request, n: usize| {
                if n == 1 {
                    an_ask("architect")
                } else {
                    (Vec::new(), Vec::new())
                }
            }),
        );
    }
    let (mut e, said, first) = start(&s, agents);
    let queue = |e: &mut Engine, title: &str, position: f64| -> i64 {
        e.db.write(Actor::Owner, |tx| {
            tx.execute(
                "INSERT INTO items (project, title, problem, why, deliver, level) VALUES ('p', ?1, 'x', 'y', 'change', 'l')",
                [title],
            )?;
            let id = tx.last_insert_rowid();
            tx.execute("INSERT INTO dispatch_queue (item, position) VALUES (?1, ?2)", params![id, position])?;
            Ok(id)
        })
        .unwrap()
    };
    let second = queue(&mut e, "Add another", 2.0);
    let third = queue(&mut e, "Add a third", 3.0);
    // The question a work's item put to the architect, taken off the queue as if a work of the architect had it in hand.
    let held = |e: &mut Engine, item: i64| -> Option<i64> {
        let m: Option<i64> =
            e.db.conn()
                .query_row(
                    "SELECT id FROM architect_matters WHERE kind = 'engineer_question' AND item = ?1",
                    [item],
                    |r| r.get(0),
                )
                .ok();
        if let Some(m) = m {
            e.db.write(Actor::Engine, |tx| {
                tx.execute(
                    "UPDATE architect_matters SET handled_work = (SELECT min(id) FROM works WHERE graph = 'architect'),
                       handled_round = 1 WHERE id = ?1 AND handled_round IS NULL",
                    [m],
                )?;
                Ok(())
            })
            .unwrap();
        }
        m
    };
    assert!(
        steps_until(&mut e, 20, |e| held(e, first).is_some() && state(e, second).is_some()),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let asked = held(&mut e, first).unwrap();
    let (a, _, _) = state(&e, first).unwrap();
    // The architect replies while the second work holds the one slot: the first stays where it waited.
    e.db.write(Actor::Architect, |tx| {
        tx.execute(
            "UPDATE architect_matters SET reply = ?2 WHERE id = ?1",
            params![asked, json!({ "text": "Go ahead.", "why": "It holds." }).to_string()],
        )?;
        Ok(())
    })
    .unwrap();
    let mut after = None;
    // While the second holds the one slot the first stays where it waited, and the engine names it, for the owner's
    // page, as waiting for a free slot; the second's question is taken off the queue as it comes.
    let mut named = false;
    for _ in 0..20 {
        if state(&e, first).is_some_and(|x| x.1 != "A2") {
            break;
        }
        let log = said.lock().unwrap().join("\n");
        assert!(e.free_slots().unwrap() >= 0, "{log}");
        let waiting = e.waiting_for_slot().unwrap();
        assert!(waiting.iter().all(|&w| w == a), "{waiting:?}\n{log}");
        named |= waiting == [a];
        after = e.step(after).unwrap().work;
        held(&mut e, second);
    }
    let log = said.lock().unwrap().join("\n");
    assert!(named, "{log}");
    // The first went on only once the second asked the architect in turn, holding no slot, and went on in the slot the
    // second left, to choose the way on with the reply, before a work started on the third item.
    assert_eq!(state(&e, first).map(|x| x.1).as_deref(), Some("A4"), "{log}");
    assert_eq!(state(&e, second).map(|x| x.1).as_deref(), Some("A2"), "{log}");
    assert_eq!(e.waiting_for_slot().unwrap(), Vec::<i64>::new(), "{log}");
    assert_eq!(state(&e, third), None, "{log}");
    assert_eq!(e.free_slots().unwrap(), 0, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A work at the fix whose table to answer has no row runs no round there, which could answer nothing: the engine takes
// the fix's row for it, recorded with why, on to choose what runs next. In the sixth trial such a work went from the
// fix to choosing the way on, whose carry on led back to the fix, round after round.
#[test]
fn a_fix_with_nothing_to_answer_goes_on_without_a_round() {
    let s = setup("nothing-to-answer");
    let (mut e, said, item) = start(&s, scripts());
    for _ in 0..2 {
        e.step(None).unwrap();
    }
    let (work, at, _) = state(&e, item).unwrap();
    assert_eq!(at, "R2", "{}", said.lock().unwrap().join("\n"));
    e.db.write(Actor::Engine, |tx| {
        tx.execute("UPDATE works SET at = 'T6' WHERE id = ?1", [work])?;
        Ok(())
    })
    .unwrap();
    assert_eq!(e.advance(work).unwrap(), Advance::Moved);
    let log = said.lock().unwrap().join("\n");
    assert_eq!(state(&e, item).map(|x| x.1).as_deref(), Some("T7"), "{log}");
    let (transition, session, why): (Option<String>, Option<i64>, Option<String>) =
        e.db.conn()
            .query_row(
                "SELECT transition, session, json_extract(input, '$.engine.nothing_to_answer') FROM rounds
                 WHERE work = ?1 AND cell = 'T6'",
                [work],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
    assert_eq!(transition.as_deref(), Some("nothing_to_answer"), "{log}");
    assert_eq!(session, None, "{log}");
    assert!(why.is_some_and(|w| w.contains("could answer nothing")), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
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
    // Started all, one turn starts both works, side by side.
    c.set_paused(false);
    let moved = turns(&mut e, &c, 1);
    let log = said.lock().unwrap().join("\n");
    assert_eq!(moved.len(), 2, "{log}");
    let (one, two) = (moved[0], moved[1]);
    assert_ne!(one, two, "{log}");
    // Paused again: nothing moves, until the owner starts one work, which alone goes on.
    c.set_paused(true);
    assert_eq!(turns(&mut e, &c, 2), Vec::<i64>::new());
    c.start(api::Fresh::Work(one), false);
    let log = said.lock().unwrap().join("\n");
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
    let log = said.lock().unwrap().join("\n");
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
    let stepped = turn(&mut e, &c);
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
    let first = turn(&mut e, &c).work.unwrap();
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
    let second = turn(&mut e, &c).work.unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_ne!(second, first, "{log}");
    c.set_paused(true);
    c.start(api::Fresh::Work(first), false);
    assert_eq!(turns(&mut e, &c, 2), Vec::<i64>::new());
    assert_eq!(paused_by(&e, first).as_deref(), Some("owner"));
    assert!(c.asked().releasing.contains(&first));
    // The other work paused frees its slot: the first goes on.
    owner_pauses(&mut e, &c, second);
    let moved = turns(&mut e, &c, 2);
    let log = said.lock().unwrap().join("\n");
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
    let work = turn(&mut e, &c).work.unwrap();
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
    let log = said.lock().unwrap().join("\n");
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
    let log = said.lock().unwrap().join("\n");
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
    let stepped = turn(&mut e, &c);
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
    assert_eq!(at, "D1", "{}", said.lock().unwrap().join("\n"));
    let c = api::Control::default();
    c.ask_step(work, true).unwrap();
    turn(&mut e, &c);
    let log = said.lock().unwrap().join("\n");
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
    assert_eq!(given(two, "format:D1"), Some(true));
    for a in ["format:common", "rule:ground"] {
        assert!(in_system(two, a), "{a}: {two}");
    }
    assert!(two["call"]["session"]["started"].is_string(), "{}", two["call"]);
    // Started as it is, the work's next round goes on in that session; started in a new session, the next starts one.
    c.start(api::Fresh::Work(work), false);
    let n = agent_rounds(&e, item).len();
    for _ in 0..10 {
        if agent_rounds(&e, item).len() > n {
            break;
        }
        turn(&mut e, &c);
    }
    let lines = line_sessions(&e, work, "engineer");
    assert_eq!(lines.len(), 2, "{lines:?}\n{}", said.lock().unwrap().join("\n"));
    owner_pauses(&mut e, &c, work);
    c.start(api::Fresh::Work(work), true);
    let n = agent_rounds(&e, item).len();
    for _ in 0..10 {
        if agent_rounds(&e, item).len() > n {
            break;
        }
        turn(&mut e, &c);
    }
    let lines = line_sessions(&e, work, "engineer");
    let log = said.lock().unwrap().join("\n");
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
    let log = said.lock().unwrap().join("\n");
    let lines = line_sessions(&e, arch, "architect");
    assert_eq!(lines.len(), 1, "{lines:?}\n{log}");
    c.pause(&api::Fresh::Architect("p".into()));
    c.ask_step_architect("p", true).unwrap();
    turn(&mut e, &c);
    for _ in 0..5 {
        c.ask_step_architect("p", false).unwrap();
        turn(&mut e, &c);
    }
    let log = said.lock().unwrap().join("\n");
    let path = path_of(&e, arch);
    assert_eq!(path[..7], ["S1", "U1", "Q0", "Q2", "H8", "S1", "U1"], "{log}");
    let lines = line_sessions(&e, arch, "architect");
    assert_eq!(lines.len(), 2, "{lines:?}\n{log}");
    assert!(lines[0].1, "{lines:?}");
    assert_eq!((lines[1].0.as_str(), lines[1].1), ("owner", false), "{lines:?}");
    assert!(lines[1].2.starts_with("U1"), "{lines:?}\n{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// While every work is paused, a work the pause holds waits on it: its session is kept warm as the session manager's
// rules say, unless the owner switched keeping warm off, which holds across restarts; a work the owner started runs,
// and is kept warm as any work that goes on is, for its going on.
#[test]
fn sessions_are_kept_warm_while_every_work_is_paused_unless_the_owner_switches_it_off() {
    let s = setup_priced("warm-paused");
    let mut scripts = scripts();
    scripts.insert(
        "R2",
        Box::new(|_: &Request, _: usize| (Vec::new(), vec![("@usage", used(1.0, 9_000))])),
    );
    let (mut e, said, item) = start(&s, scripts);
    agent_rounds_to(&mut e, item, 2);
    let (work, at, _) = state(&e, item).unwrap();
    assert_eq!(at, "D1", "{}", said.lock().unwrap().join("\n"));
    let from = last_call(&e, work);
    let due = from + 55 * 60_000 + 1_000;
    let paused = |warm: bool, runs: &[i64]| engine::GlobalPause {
        warm,
        runs: runs.iter().copied().collect(),
        architects: Default::default(),
    };
    assert_eq!(warm_now(&mut e, due, Some(&paused(false, &[]))).unwrap(), 0);
    assert_eq!(
        warm_now(&mut e, due, Some(&paused(true, &[]))).unwrap(),
        1,
        "{}",
        said.lock().unwrap().join("\n")
    );
    assert_eq!(
        pings_of(&e, work)[0].0,
        json!({ "cell": "D1", "on": "pause", "what": "the owner paused every work" })
    );
    // A work the owner started runs on, as every work does with no pause: the line it comes back to is kept warm
    // for its going on.
    let later = due + 55 * 60_000 + 1_000;
    assert_eq!(warm_now(&mut e, later, Some(&paused(true, &[work]))).unwrap(), 1);
    let going_on = json!({ "cell": "D1", "on": "work", "what": "the work goes on" });
    assert_eq!(pings_of(&e, work)[1].0, going_on);
    assert_eq!(warm_now(&mut e, later + 55 * 60_000 + 1_000, None).unwrap(), 1);
    assert_eq!(pings_of(&e, work)[2].0, going_on);
    // The owner's switch is kept in the database, as the owner's, across a restart.
    assert!(api::keep_warm_paused(e.db.conn()).unwrap());
    api::set_keep_warm_paused(&mut e.db, false).unwrap();
    drop(e);
    let (e, _) = engine(&s, BTreeMap::new());
    assert!(!api::keep_warm_paused(e.db.conn()).unwrap());
    let _ = std::fs::remove_dir_all(&s.root);
}

/// The same sequence of numbers for the same seed: the random tests' dice.
struct Dice(AtomicU64);

impl Dice {
    fn new(seed: u64) -> Dice {
        Dice(AtomicU64::new(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1))
    }

    /// A number below `n`.
    fn roll(&self, n: u64) -> u64 {
        let mut x = self.0.load(Ordering::Relaxed);
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0.store(x, Ordering::Relaxed);
        x % n.max(1)
    }
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

/// What a round hands in, at random: an ask of the advisor or the architect, or its own answer.
fn random_ask(dice: &Dice) -> Picked {
    match dice.roll(10) {
        0 | 1 => an_ask("advisor"),
        2..=4 => an_ask("architect"),
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

/// One random sequence of events over three items of project p, beside project q, related to it: steps of the engine,
/// whose rounds ask the advisor and the architect, hand questions on (the advisor to the architect, an architect to the
/// other project's architect), and reply, sometimes with an item to wait for, of either project, a way on chosen wrong
/// going to the architect as a flow anomaly; the architects' hours, past their limit; interjections; cancels; and
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
    let dice = Arc::new(Dice::new(seed));
    let mut agents = scripts();
    for cell in ["R1", "D1", "T3"] {
        let d = dice.clone();
        agents.insert(cell, Box::new(move |_: &Request, _: usize| random_ask(&d)));
    }
    // The advisor answers, or hands the question to the architect.
    let d = dice.clone();
    agents.insert(
        "A1",
        Box::new(move |_: &Request, _: usize| match d.roll(4) {
            0 | 1 => an_ask("architect"),
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
                0 | 1 => reply["hand_over"] = json!({ "to": "architect", "project": other, "why": "It is theirs." }),
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
    // which goes to the architect as a flow anomaly.
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
            let v = ["wait_on", "withdraw"][d.roll(2) as usize];
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
            0..=19 => match a_work(&e, &dice) {
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
            said.lock().unwrap().join("\n")
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
        said.lock().unwrap().join("\n")
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

// Random sequences of events (asks of the advisor and the architect, hand-overs between them and between two related
// projects' architects, the architects' replies, flow anomalies, interjections, cancels, ends and takeovers by a new
// flow version) leave every chain of blockers closed after each event: every stopped task has
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

// A step taken on another thread, on a context forked from the engine with a connection of its own, writes the same
// rounds and changes as the same step taken on the engine's own thread from the same database, the numbers and times
// of the transactions and the ids drawn at random aside; and the one commit listener the engine was given hears of
// what that connection wrote, as the owner's page hears of every change.
#[test]
fn a_step_on_another_thread_writes_what_it_writes_on_the_engines_own() {
    let s = setup("fork_step");
    let (mut e, said, item) = start(&s, scripts());
    // The first step starts the work and sets up its worktree; the next is the round of its first agent's cell.
    e.step(None).unwrap();
    let at = state(&e, item).unwrap().1;
    let before = s.root.join("before.db");
    e.db.conn()
        .execute("VACUUM INTO ?1", [before.to_string_lossy().into_owned()])
        .unwrap();
    let base = written(&e);
    let on_main = e.step(None).unwrap();
    let main = written(&e);
    let log = said.lock().unwrap().join("\n");
    assert_eq!(on_main.cell.as_deref(), Some(at.as_str()), "{on_main:?}\n{log}");
    assert!(main.len() > base.len(), "{log}");
    drop(e);
    // The database as it was before that step, and the same step taken on another thread.
    let path = config::database(&s.data);
    for end in ["-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{end}", path.display()));
    }
    std::fs::copy(&before, &path).unwrap();
    let heard = Arc::new(Mutex::new(Vec::<db::Changed>::new()));
    let h = heard.clone();
    let hooks = Hooks {
        commit: Some(Arc::new(move |_: &db::rusqlite::Connection, rows: &[db::Changed]| {
            h.lock().unwrap().extend(rows.iter().cloned());
        })),
        ..Hooks::default()
    };
    let (e, said) = engine_with(&s, scripts(), hooks);
    let mut t = e.fork().unwrap();
    let on_thread = std::thread::spawn(move || t.step(None)).join().unwrap().unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(on_thread, on_main, "{log}");
    assert_eq!(written(&e), main, "{log}");
    let round = on_thread.rounds.first().unwrap_or_else(|| panic!("no round:\n{log}"));
    let rowid: i64 =
        e.db.conn()
            .query_row(
                "SELECT rowid FROM rounds WHERE work = ?1 AND n = ?2",
                [round.work, round.n],
                |r| r.get(0),
            )
            .unwrap();
    let heard = heard.lock().unwrap().clone();
    let told = db::Changed {
        table: "rounds".into(),
        rowid,
    };
    assert!(heard.contains(&told), "{heard:?}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A step reports the rounds its own connection wrote: the round of the work it took, and a round of another work it
// cut off, as taking over a work on an older flow version cuts off the round the work was in; never a round another
// connection wrote meanwhile, as another thread's step would.
#[test]
fn a_step_reports_the_rounds_its_own_connection_wrote() {
    let s = setup("own_rounds");
    let path = config::database(&s.data);
    let mut scripts = scripts();
    // While the round of cell `R1` runs, another connection writes a round of the architect's work over before.
    scripts.insert(
        "R1",
        Box::new(move |_: &Request, _: usize| {
            let mut db = db::Db::open(&path).unwrap();
            db.write(Actor::Engine, |tx| {
                tx.execute(
                    "INSERT INTO rounds (work, n, cell, input, output, transition, config_commit, started_at, ended_at)
                     SELECT w.id, (SELECT max(r.n) FROM rounds r WHERE r.work = w.id) + 1, 'H10', '{}', '{}', 'done',
                            'c', ?1, ?1
                     FROM works w WHERE w.graph = 'architect'",
                    [db::now()],
                )?;
                Ok(())
            })
            .unwrap();
            (Vec::new(), Vec::new())
        }),
    );
    let (mut e, said, item) = start(&s, scripts);
    let first = e.step(None).unwrap();
    let work = first.work.unwrap();
    let r1 = e.step(first.work).unwrap();
    let log = said.lock().unwrap().join("\n");
    let n: i64 =
        e.db.conn()
            .query_row("SELECT max(n) FROM rounds WHERE work = ?1", [work], |r| r.get(0))
            .unwrap();
    let architects: i64 =
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM rounds r JOIN works w ON w.id = r.work WHERE w.graph = 'architect'",
                [],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(r1.cell.as_deref(), Some("R1"), "{r1:?}\n{log}");
    assert_eq!(architects, 2, "{log}");
    assert_eq!(r1.rounds, [engine::RoundId::new(work, n)], "{log}");
    // A round of the work under way as the next step takes the work over, on an older flow version where the cell it
    // stands at is another: the step cuts it off, and reports it with the round of the new work it starts.
    let at = state(&e, item).unwrap().1;
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES (?1, ?2, ?3, '{}', 'c', ?4)",
            params![work, n + 1, at, db::now()],
        )?;
        Ok(())
    })
    .unwrap();
    older(&mut e, &[work], |g| renamed(g, &at));
    let over = e.step(Some(work)).unwrap();
    let log = said.lock().unwrap().join("\n");
    let works = works_of(&e, item);
    assert_eq!(works.len(), 2, "{works:?}\n{log}");
    let new = works[1].0;
    assert_eq!(over.work, Some(new), "{over:?}\n{log}");
    assert!(
        over.rounds.contains(&engine::RoundId::new(work, n + 1)),
        "{over:?}\n{log}"
    );
    assert!(over.rounds.iter().any(|r| r.work == new), "{over:?}\n{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// What lasts from one step to the next is the engine's, whichever thread's context takes the step: a work taken over on
// a forked context's thread is among those the engine's own context reads as replaced, which is how the owner's Start
// of the old work goes on in the new one; a configuration one context reads is the one every context goes on with,
// told once; and a halt one context sets stops a step on another.
#[test]
fn a_forked_context_shares_what_lasts_between_steps() {
    let s = setup("fork_shared");
    let (mut e, said, item) = start(&s, scripts());
    let first = e.step(None).unwrap();
    let work = first.work.unwrap();
    e.step(first.work).unwrap();
    let at = state(&e, item).unwrap().1;
    older(&mut e, &[work], |g| renamed(g, &at));
    let mut t = e.fork().unwrap();
    let (mut t, over) = std::thread::spawn(move || {
        let over = t.step(Some(work));
        (t, over)
    })
    .join()
    .unwrap();
    let over = over.unwrap();
    let log = said.lock().unwrap().join("\n");
    let works = works_of(&e, item);
    assert_eq!(works.len(), 2, "{works:?}\n{over:?}\n{log}");
    let new = works[1].0;
    assert_eq!(e.replaced(), [(work, new)], "{log}");
    assert_eq!(t.replaced(), Vec::<(i64, i64)>::new(), "{log}");
    // A new configuration, read on the engine's own context, is the forked context's at its next read.
    let install = std::fs::read_to_string(s.data.join("install.toml")).unwrap();
    write(&s.data, "install.toml", &format!("{install}\n# read once\n"));
    git(&s.data, &["commit", "-q", "-am", "a configuration read once"]);
    e.reload_config();
    let commit = e.config.commit.clone();
    t.reload_config();
    assert_eq!(t.config.commit, commit);
    let log = said.lock().unwrap().join("\n");
    let in_use = format!("configuration {commit} in use");
    assert_eq!(log.matches(&in_use).count(), 1, "{log}");
    // A halt set on the engine's own context stops the forked context's step.
    e.set_halt(Some(engine::Halt {
        kind: "usage_limit".into(),
        until: db::now() + 3_600_000,
        why: "a usage limit".into(),
    }));
    let rounds: i64 =
        e.db.conn()
            .query_row("SELECT count(*) FROM rounds", [], |r| r.get(0))
            .unwrap();
    let halted = std::thread::spawn(move || t.step(Some(new))).join().unwrap().unwrap();
    assert_eq!(halted, engine::Stepped::default(), "{log}");
    let after: i64 =
        e.db.conn()
            .query_row("SELECT count(*) FROM rounds", [], |r| r.get(0))
            .unwrap();
    assert_eq!(after, rounds);
    let _ = std::fs::remove_dir_all(&s.root);
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

// Two works on two ready items of one project, with two slots free on the machine, run their agents' rounds side by
// side: each round of `R1` waits until the other's has come, or ten seconds, so that rounds taken one after the other
// do not overlap. While both run, the engine says both works' rounds run, which the owner's page shows as running.
#[test]
fn two_works_run_their_agents_rounds_side_by_side() {
    let met = Gate::default();
    let at = met.clone();
    let mut agents = scripts();
    agents.insert(
        "R1",
        Box::new(move |_: &Request, _: usize| {
            at.meet(2);
            (Vec::new(), Vec::new())
        }),
    );
    let busy = Arc::new(Mutex::new(Vec::new()));
    let seen = busy.clone();
    let hooks = Hooks {
        busy: Arc::new(move |w: &[i64]| seen.lock().unwrap().push(w.to_vec())),
        ..Hooks::default()
    };
    let (s, mut e, said, first, second) = two_works_with("side-by-side", agents, hooks);
    // The first pass starts both works; the second runs the round of `R1` of each.
    e.tick().unwrap();
    e.tick().unwrap();
    let log = said.lock().unwrap().join("\n");
    let span = |item: i64| -> (i64, i64) {
        e.db.conn()
            .query_row(
                "SELECT r.started_at, r.ended_at FROM rounds r JOIN works w ON w.id = r.work
                 WHERE w.item = ?1 AND r.cell = 'R1'",
                [item],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap_or_else(|x| panic!("{x}\n{log}"))
    };
    let (a, b) = (span(first), span(second));
    assert!(a.0 < b.1 && b.0 < a.1, "{a:?} {b:?}\n{log}");
    let works = vec![state(&e, first).unwrap().0, state(&e, second).unwrap().0];
    let told = busy.lock().unwrap().clone();
    assert!(told.contains(&works), "{told:?}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// While one work's CI has not come back, another work of the same project goes on: the fake GitHub holds every check
// until the test lets it through, and the owner's global pause holds the second work until the first's CI is under
// way.
#[test]
fn a_work_goes_on_while_another_waits_for_its_ci() {
    let (s, mut e, said, first, second) = two_works_with("beside-ci", scripts(), Hooks::default());
    s.ci.shut();
    e.tick().unwrap();
    let (a, b) = (state(&e, first).unwrap().0, state(&e, second).unwrap().0);
    let only_first = Go {
        pause: held_but(&[a], &[]),
        ..Go::default()
    };
    let ci = turns_until(&mut e, &only_first, |_| s.ci.came() > 0);
    let log = said.lock().unwrap().join("\n");
    assert!(ci, "{log}");
    let since = tool_round_open(&e, a).expect(&log);
    let went_on = turns_until(&mut e, &Go::default(), |e| rounds_after(e, b, since).1 > 0);
    let log = said.lock().unwrap().join("\n");
    assert!(went_on, "{log}");
    // The first's CI is still under way: its step in flight, its round open.
    assert!(e.in_flight().contains(&a), "{log}");
    assert_eq!(tool_round_open(&e, a), Some(since), "{log}");
    s.ci.open();
    e.drain().unwrap();
    let _ = std::fs::remove_dir_all(&s.root);
}

// Two works that run side by side each merge into main through the flow, in the order they came into the project's
// merge queue. Every change to their state is one transaction recorded in `changes`, the rows of a transaction written
// at one time by one writer, one after the other; each work's rounds are numbered from one, none twice or missing,
// and none is left open.
#[test]
fn two_works_side_by_side_merge_in_turn_and_record_every_change_once() {
    let mut agents = scripts();
    // Each work changes a file of its own, so that the second has something to merge after the first.
    agents.insert(
        "I1",
        Box::new(|r: &Request, _: usize| {
            let wt = &r.worktree;
            let file = format!("src/work-{}.txt", r.work);
            if !wt.join(&file).exists() {
                write(wt, &file, "feature\n");
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
    let (s, mut e, said, first, second) = two_works_with("both-merge", agents, Hooks::default());
    let over = |e: &Engine, item: i64| state(e, item).is_some_and(|w| w.1 == "E1");
    // The works in the order they came into the merge queue.
    let mut queued: Vec<i64> = Vec::new();
    for _ in 0..300 {
        e.tick().unwrap();
        let log = said.lock().unwrap().join("\n");
        let mut st =
            e.db.conn()
                .prepare("SELECT work FROM merge_queue WHERE project = 'p'")
                .unwrap();
        let queue: Vec<i64> = st.query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect();
        drop(st);
        for w in queue {
            if !queued.contains(&w) {
                queued.push(w);
            }
        }
        for item in [first, second] {
            let paused = state(&e, item).and_then(|w| w.2);
            assert!(paused.is_none(), "{paused:?}\n{log}");
        }
        if over(&e, first) && over(&e, second) {
            break;
        }
    }
    let log = said.lock().unwrap().join("\n");
    assert!(over(&e, first) && over(&e, second), "{log}");
    let (a, b) = (state(&e, first).unwrap().0, state(&e, second).unwrap().0);
    // Merged onto main in the queue's order: the merges, oldest first, by the works whose pull requests they merged.
    let subjects = git(
        &s.root,
        &["--git-dir", &s.origin.to_string_lossy(), "log", "--format=%s", "main"],
    );
    let merged: Vec<i64> = subjects
        .lines()
        .rev()
        .filter_map(|l| l.strip_prefix("Merge PR "))
        .map(|pr| {
            let pr: i64 = pr.parse().unwrap();
            e.db.conn()
                .query_row("SELECT id FROM works WHERE pr = ?1", [pr], |r| r.get(0))
                .unwrap()
        })
        .collect();
    assert_eq!(merged.len(), 2, "{subjects}\n{log}");
    assert_eq!(merged, queued, "{log}");
    // Each work's rounds numbered from one, none twice or missing, none open.
    for w in [a, b] {
        let mut st =
            e.db.conn()
                .prepare("SELECT n, ended_at IS NULL FROM rounds WHERE work = ?1 ORDER BY n")
                .unwrap();
        let rounds: Vec<(i64, bool)> = st
            .query_map([w], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(|x| x.unwrap())
            .collect();
        let numbers: Vec<i64> = rounds.iter().map(|r| r.0).collect();
        assert_eq!(numbers, (1..=numbers.len() as i64).collect::<Vec<_>>(), "work {w}");
        assert!(rounds.iter().all(|r| !r.1), "work {w}: {rounds:?}");
    }
    // No transaction's number taken twice: the rows that share one were written at one time, by one writer, one
    // after the other.
    let shared: i64 =
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM (SELECT tx FROM changes GROUP BY tx
                 HAVING count(DISTINCT at) > 1 OR count(DISTINCT actor) > 1 OR max(id) - min(id) + 1 <> count(*))",
                [],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(shared, 0, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The owner pauses one of two works side by side while its round runs: that round ends as usual and is recorded, the
// work then takes no step, and the other goes on.
#[test]
fn a_work_paused_beside_another_stops_after_its_round_and_the_other_goes_on() {
    let pausing = Arc::new(AtomicI64::new(0));
    let hold = Gate::default();
    let (p, h) = (pausing.clone(), hold.clone());
    let mut agents = scripts();
    // The round of `R1` of the work to pause waits at the gate while the test pauses the work.
    agents.insert(
        "R1",
        Box::new(move |r: &Request, _: usize| {
            if r.work == p.load(Ordering::SeqCst) {
                h.pass();
            }
            (Vec::new(), Vec::new())
        }),
    );
    let (s, mut e, said, first, second) = two_works_with("pause-beside", agents, Hooks::default());
    e.tick().unwrap();
    let (a, b) = (state(&e, first).unwrap().0, state(&e, second).unwrap().0);
    pausing.store(a, Ordering::SeqCst);
    hold.shut();
    let came = turns_until(&mut e, &Go::default(), |_| hold.came() > 0);
    let log = said.lock().unwrap().join("\n");
    assert!(came, "{log}");
    api::pause(&mut e.db, a, None).unwrap();
    hold.open();
    let ended = turns_until(&mut e, &Go::default(), |e| !e.in_flight().contains(&a));
    let log = said.lock().unwrap().join("\n");
    assert!(ended, "{log}");
    let took: Option<String> =
        e.db.conn()
            .query_row(
                "SELECT transition FROM rounds WHERE work = ?1 AND cell = 'R1'",
                [a],
                |r| r.get(0),
            )
            .unwrap();
    assert!(took.is_some(), "{log}");
    assert_eq!(paused_by(&e, a).as_deref(), Some("owner"), "{log}");
    let (mine, _) = rounds_after(&e, a, 0);
    let (theirs, _) = rounds_after(&e, b, 0);
    let went_on = turns_until(&mut e, &Go::default(), |e| rounds_after(e, b, 0).0 >= theirs + 2);
    e.drain().unwrap();
    let log = said.lock().unwrap().join("\n");
    assert!(went_on, "{log}");
    assert_eq!(rounds_after(&e, a, 0).0, mine, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// Two items, the second depending on the first by a blocking dependency, run one after the other though two slots are
// free: no work starts on the second until the first has merged into main, and then one does.
#[test]
fn a_blocking_dependency_starts_the_second_item_only_once_the_first_merged() {
    let (s, mut e, said, first, second) = two_works_with("after-the-first", scripts(), Hooks::default());
    e.db.write(Actor::Owner, |tx| {
        tx.execute(
            "INSERT INTO item_links (item, depends_on, needs, kind) VALUES (?1, ?2, 'the feature', 'blocking')",
            params![second, first],
        )?;
        Ok(())
    })
    .unwrap();
    let merged = |s: &Setup| {
        git(
            &s.root,
            &["--git-dir", &s.origin.to_string_lossy(), "show", "main:src/lib.txt"],
        ) == "old\nfeature"
    };
    // Whether main held the first's change when the pass that started the second began.
    let mut after_merge = None;
    for _ in 0..300 {
        let was = merged(&s);
        e.tick().unwrap();
        if state(&e, second).is_some() {
            after_merge = Some(was);
            break;
        }
    }
    let log = said.lock().unwrap().join("\n");
    assert_eq!(after_merge, Some(true), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// With one slot free and two works whose wait for the architect's reply is over, only the first goes on, granted the
// slot; the other waits on for a free slot, named so for the owner's page, until a slot frees, here when the owner
// pauses the work that holds the other slot. The architect's replies are written by hand, each question taken off the
// queue as it comes, so that no work of the architect starts.
#[test]
fn of_two_works_back_from_waiting_one_takes_the_one_free_slot() {
    let asked: Arc<Mutex<BTreeSet<i64>>> = Arc::default();
    let who = asked.clone();
    let mut agents = scripts();
    // The first two works to research ask the architect, once each.
    agents.insert(
        "R1",
        Box::new(move |r: &Request, _: usize| {
            let mut who = who.lock().unwrap();
            if who.len() < 2 && who.insert(r.work) {
                an_ask("architect")
            } else {
                (Vec::new(), Vec::new())
            }
        }),
    );
    let (s, mut e, said, first, second) = two_works_with("one-free-slot", agents, Hooks::default());
    let third =
        e.db.write(Actor::Owner, |tx| {
            tx.execute(
                "INSERT INTO items (project, title, problem, why, deliver, level) VALUES ('p', 'Add a third', 'x', 'y', 'change', 'l')",
                [],
            )?;
            let id = tx.last_insert_rowid();
            tx.execute("INSERT INTO dispatch_queue (item, position) VALUES (?1, 3.0)", params![id])?;
            Ok(id)
        })
        .unwrap();
    // The questions put to the architect, taken off the queue as if a work of the architect had them in hand.
    let take = |e: &mut Engine| {
        e.db.write(Actor::Engine, |tx| {
            tx.execute(
                "UPDATE architect_matters SET handled_work = (SELECT min(id) FROM works WHERE graph = 'architect'),
                   handled_round = 1 WHERE kind = 'engineer_question' AND handled_round IS NULL",
                [],
            )?;
            Ok(())
        })
        .unwrap();
    };
    let at = |e: &Engine, item: i64| state(e, item).map(|w| w.1);
    for _ in 0..10 {
        if at(&e, first).as_deref() == Some("A2") && at(&e, second).as_deref() == Some("A2") && at(&e, third).is_some()
        {
            break;
        }
        e.tick().unwrap();
        take(&mut e);
    }
    let log = said.lock().unwrap().join("\n");
    assert_eq!(at(&e, first).as_deref(), Some("A2"), "{log}");
    assert_eq!(at(&e, second).as_deref(), Some("A2"), "{log}");
    let (b, c) = (state(&e, second).unwrap().0, state(&e, third).expect(&log).0);
    // The architect replies to both at once, while the third work holds one of the two slots.
    e.db.write(Actor::Architect, |tx| {
        tx.execute(
            "UPDATE architect_matters SET reply = ?1 WHERE kind = 'engineer_question'",
            [json!({ "text": "Go ahead.", "why": "It holds." }).to_string()],
        )?;
        Ok(())
    })
    .unwrap();
    // A pass finds both replies come, granting no slot to a work before its step found its wait over; the next grants
    // the one free slot to the first.
    e.tick().unwrap();
    assert_eq!(at(&e, first).as_deref(), Some("A2"), "{log}");
    e.tick().unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_ne!(at(&e, first).as_deref(), Some("A2"), "{log}");
    assert_eq!(at(&e, second).as_deref(), Some("A2"), "{log}");
    assert_eq!(e.waiting_for_slot().unwrap(), vec![b], "{log}");
    assert_eq!(e.free_slots().unwrap(), 0, "{log}");
    // The owner pauses the third, which frees its slot: the second goes on in it.
    api::pause(&mut e.db, c, None).unwrap();
    e.tick().unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_ne!(at(&e, second).as_deref(), Some("A2"), "{log}");
    assert_eq!(e.waiting_for_slot().unwrap(), Vec::<i64>::new(), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// No keep-warm ping goes to a work's sessions while a step of it runs: here its CI has not come back while the owner
// has paused it, and its engineer's session, due a ping, gets one only once the step has ended. The session has no
// round open meanwhile, so that only the step in flight keeps the ping off.
#[test]
fn no_ping_goes_to_a_work_while_its_step_runs() {
    let s = setup_priced("no-ping-in-a-step");
    s.ci.shut();
    let mut agents = scripts();
    let implement = agents.remove("I1").unwrap();
    // The engineer's round that asks for CI says what it used, as the CLI does, which a ping needs to run it again.
    agents.insert(
        "I1",
        Box::new(move |r: &Request, n: usize| {
            let (pick, mut set) = implement(r, n);
            set.push(("@usage", used(1.0, 9_000)));
            (pick, set)
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    let ci = turns_until(&mut e, &Go::default(), |_| s.ci.came() > 0);
    let log = said.lock().unwrap().join("\n");
    assert!(ci, "{log}");
    let work = state(&e, item).unwrap().0;
    pause_by_owner(&mut e, work, "to look at it");
    let now = last_call(&e, work) + 55 * 60_000 + 1_000;
    assert_eq!(e.keep_warm(now, None).unwrap(), 0, "{log}");
    s.ci.open();
    e.drain().unwrap();
    let log = said.lock().unwrap().join("\n");
    assert!(warm_now(&mut e, now, None).unwrap() > 0, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A failed run whose cause stops every work, here a usage limit, keeps the loop from dispatching any step until the
// wait is over, while the step of the other work under way ends as usual and is recorded; then both go on.
#[test]
fn a_halt_dispatches_no_step_and_lets_the_step_under_way_end() {
    let reset = db::now() / 1000 + 3600;
    let failing = Arc::new(AtomicI64::new(0));
    let hold = Gate::default();
    let (f, h) = (failing.clone(), hold.clone());
    let mut agents = scripts();
    // The round of `R1` of the work to fail fails once, when the other's round has come to the gate, where it waits.
    agents.insert(
        "R1",
        Box::new(move |r: &Request, _: usize| {
            if f.compare_exchange(r.work, 0, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                let until = Instant::now() + Duration::from_secs(10);
                while h.came() == 0 && Instant::now() < until {
                    std::thread::sleep(Duration::from_millis(5));
                }
                let report = format!("Claude AI usage limit reached|{reset}");
                return (Vec::new(), vec![("@error", json!(report))]);
            }
            h.pass();
            (Vec::new(), Vec::new())
        }),
    );
    let (s, mut e, said, first, second) = two_works_with("halt-beside", agents, Hooks::default());
    e.tick().unwrap();
    let (a, b) = (state(&e, first).unwrap().0, state(&e, second).unwrap().0);
    failing.store(a, Ordering::SeqCst);
    hold.shut();
    let halted = turns_until(&mut e, &Go::default(), |e| {
        e.halt().is_some() && !e.in_flight().contains(&a) && hold.came() > 0
    });
    let log = said.lock().unwrap().join("\n");
    assert!(halted, "{log}");
    assert_eq!(e.in_flight(), BTreeSet::from([b]), "{log}");
    // While the halt holds, a turn of the loop dispatches nothing.
    let n = rounds_count(&e);
    e.dispatch(&Go::default()).unwrap();
    assert_eq!(e.in_flight(), BTreeSet::from([b]), "{log}");
    hold.open();
    e.drain().unwrap();
    let log = said.lock().unwrap().join("\n");
    let took: Option<String> =
        e.db.conn()
            .query_row(
                "SELECT transition FROM rounds WHERE work = ?1 AND cell = 'R1'",
                [b],
                |r| r.get(0),
            )
            .unwrap();
    assert!(took.is_some(), "{log}");
    assert_eq!(rounds_count(&e), n, "{log}");
    e.dispatch(&Go::default()).unwrap();
    assert!(e.in_flight().is_empty(), "{log}");
    assert_eq!(rounds_count(&e), n, "{log}");
    // Once the wait is over, both go on.
    e.set_halt(e.halt().map(|h| engine::Halt { until: 0, ..h }));
    let p = e.pass(&Go::default()).unwrap();
    let log = said.lock().unwrap().join("\n");
    let moved: BTreeSet<i64> = p
        .jobs
        .iter()
        .filter(|j| !j.rounds.is_empty())
        .filter_map(|j| j.work)
        .collect();
    assert_eq!(moved, BTreeSet::from([a, b]), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The owner cancels a work's item while its CI runs: the cancel ends the work and cuts its round off at once. The step
// under way, at its end, finds its round ended: its transaction rolls back, the work and its rounds stay as the cancel
// left them, and the loop does not pause the work.
#[test]
fn a_work_whose_item_is_cancelled_during_its_ci_stays_as_the_cancel_left_it() {
    let s = setup("cancel-in-ci");
    s.ci.shut();
    let (mut e, said, item) = start(&s, scripts());
    let ci = turns_until(&mut e, &Go::default(), |_| s.ci.came() > 0);
    let log = said.lock().unwrap().join("\n");
    assert!(ci, "{log}");
    let work = state(&e, item).unwrap().0;
    assert!(tool_round_open(&e, work).is_some(), "{log}");
    let projects = e.config.config.projects.clone();
    e.db.write(Actor::Owner, |tx| {
        engine::ending::cancel_item(tx, item, "no longer needed", "owner", &projects).map_err(db::Error::Abort)
    })
    .unwrap();
    let rows = |e: &Engine| -> Vec<Value> {
        let mut out = engine::query_json(
            e.db.conn(),
            "SELECT at, ended, paused, paused_by FROM works WHERE id = ?1",
            &[&work],
            10,
        )
        .unwrap();
        out.extend(
            engine::query_json(
                e.db.conn(),
                "SELECT n, cell, ended_at, transition, rejected FROM rounds WHERE work = ?1 ORDER BY n",
                &[&work],
                1000,
            )
            .unwrap(),
        );
        out
    };
    let cancelled = rows(&e);
    assert!(!cancelled[0]["ended"].is_null(), "{cancelled:?}");
    assert!(tool_round_open(&e, work).is_none(), "{log}");
    s.ci.open();
    e.drain().unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(rows(&e), cancelled, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A start over asked while a step of the work runs, here its CI, waits until that step has ended: the loop's turn
// meanwhile leaves it in the outbox and the step's round ends as usual; the next turn carries it out.
#[test]
fn a_start_over_asked_during_a_step_waits_until_the_step_ends() {
    let s = setup("start-over-in-ci");
    s.ci.shut();
    let (mut e, said, item) = start(&s, scripts());
    let ci = turns_until(&mut e, &Go::default(), |_| s.ci.came() > 0);
    let log = said.lock().unwrap().join("\n");
    assert!(ci, "{log}");
    let work = state(&e, item).unwrap().0;
    api::start_over(&mut e.db, work).unwrap();
    e.dispatch(&Go::default()).unwrap();
    assert_eq!(works_of(&e, item).len(), 1, "{log}");
    assert!(tool_round_open(&e, work).is_some(), "{log}");
    s.ci.open();
    e.drain().unwrap();
    let log = said.lock().unwrap().join("\n");
    let took: Option<String> =
        e.db.conn()
            .query_row(
                "SELECT transition FROM rounds WHERE work = ?1 AND cell = 'X1'",
                [work],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(took.as_deref(), Some("ran"), "{log}");
    e.dispatch(&Go::default()).unwrap();
    e.drain().unwrap();
    let log = said.lock().unwrap().join("\n");
    let works = works_of(&e, item);
    assert_eq!(works.len(), 2, "{works:?}\n{log}");
    assert!(works[0].5.is_some(), "{works:?}\n{log}");
    let _ = std::fs::remove_dir_all(&s.root);
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

// A step whose thread panics, here in its agent's round, leaves neither a job in flight nor a round open: the loop
// pauses its work with why, so that the slot it held is free, while the other work's round beside it ends as usual.
// Once the owner lets it go on, the work is dispatched again: a job left in flight would keep it from any step for good.
#[test]
fn a_step_whose_thread_panics_pauses_its_work_and_lets_it_go_on_later() {
    let panicking = Arc::new(AtomicI64::new(0));
    let p = panicking.clone();
    let mut agents = scripts();
    agents.insert(
        "R1",
        Box::new(move |r: &Request, _: usize| {
            if r.work == p.load(Ordering::SeqCst) {
                panic!("a scripted fault in the agent's round");
            }
            (Vec::new(), Vec::new())
        }),
    );
    let (s, mut e, said, first, second) = two_works_with("panic-in-a-step", agents, Hooks::default());
    e.tick().unwrap();
    let (a, b) = (state(&e, first).unwrap().0, state(&e, second).unwrap().0);
    panicking.store(a, Ordering::SeqCst);
    let paused = turns_until(&mut e, &Go::default(), |e| {
        paused_by(e, a).is_some()
            && !e.in_flight().contains(&a)
            && transitions_at(e, b, "R1").iter().any(Option::is_some)
    });
    e.drain().unwrap();
    let log = said.lock().unwrap().join("\n");
    assert!(paused, "{log}");
    let why: Option<String> =
        e.db.conn()
            .query_row("SELECT paused FROM works WHERE id = ?1", [a], |r| r.get(0))
            .unwrap();
    assert_eq!(paused_by(&e, a).as_deref(), Some("engine"), "{log}");
    assert!(
        why.as_deref()
            .is_some_and(|w| w.contains("a scripted fault in the agent's round")),
        "{why:?}\n{log}"
    );
    assert_eq!(open_rounds(&e, a), 0, "{log}");
    assert!(e.in_flight().is_empty(), "{log}");
    // The paused work holds no slot: the other holds one of the two at most.
    assert!(e.free_slots().unwrap() >= 1, "{log}");
    // The owner lets it go on: its next round of `R1` runs and ends.
    panicking.store(0, Ordering::SeqCst);
    api::resume(&mut e.db, a).unwrap();
    let again = turns_until(&mut e, &Go::default(), |e| {
        transitions_at(e, a, "R1").iter().any(Option::is_some)
    });
    e.drain().unwrap();
    let log = said.lock().unwrap().join("\n");
    assert!(again, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The owner's Step of a work whose step is under way waits until that step has ended: the loop's turn meanwhile
// dispatches no second step of the work and leaves the Step asked, so that the step under way ends as a step of its
// own and the work is not paused by it; the turn after takes the owner's round, after which the owner holds the work
// paused.
#[test]
fn the_owner_s_step_of_a_work_whose_step_runs_waits_until_that_step_ends() {
    let s = setup("step-beside-a-step");
    let hold = Gate::default();
    let h = hold.clone();
    let mut agents = scripts();
    agents.insert(
        "R1",
        Box::new(move |_: &Request, n: usize| {
            if n == 1 {
                h.pass();
            }
            (Vec::new(), Vec::new())
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    hold.shut();
    let came = turns_until(&mut e, &Go::default(), |_| hold.came() > 0);
    let log = said.lock().unwrap().join("\n");
    assert!(came, "{log}");
    let work = state(&e, item).unwrap().0;
    // The round of `R1` under way is among them.
    let rounds = rounds_after(&e, work, 0).0;
    let step = Go {
        pause: held_but(&[], &[]),
        steps: vec![work],
        ..Go::default()
    };
    let taken = e.dispatch(&step).unwrap();
    assert!(taken.is_empty(), "{taken:?}\n{log}");
    assert!(e.in_flight().contains(&work), "{log}");
    hold.open();
    let reaped = e.drain().unwrap();
    let log = said.lock().unwrap().join("\n");
    let mine: Vec<&Stepped> = reaped.iter().filter(|s| s.work == Some(work)).collect();
    assert_eq!(mine.len(), 1, "{reaped:?}\n{log}");
    assert_eq!(mine[0].asked, None, "{reaped:?}\n{log}");
    assert_eq!(transitions_at(&e, work, "R1").len(), 1, "{log}");
    assert_eq!(rounds_after(&e, work, 0).0, rounds, "{log}");
    assert_eq!(paused_by(&e, work), None, "{log}");
    // The Step, still asked, is the next turn's: one round, and the work is the owner's, paused.
    let p = e.pass(&step).unwrap();
    let log = said.lock().unwrap().join("\n");
    let asked = asked_in(&p, &Ask::Work(work));
    assert_eq!(asked.work, Some(work), "{p:?}\n{log}");
    assert_eq!(rounds_after(&e, work, 0).0, rounds + 1, "{p:?}\n{log}");
    assert_eq!(paused_by(&e, work).as_deref(), Some("owner"), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The owner cancels a work's item while its agent's round runs: the cancel ends the work and cuts the round off at
// once. The step under way, as the round's call returns, takes nothing of what it handed in and keeps only what it
// cost on the round; the work and its rounds otherwise stay as the cancel left them, and the loop neither pauses the
// work nor dispatches it again.
#[test]
fn a_work_whose_item_is_cancelled_during_its_agent_s_round_takes_nothing_of_the_round() {
    let s = setup("cancel-in-a-round");
    let hold = Gate::default();
    let h = hold.clone();
    let mut agents = scripts();
    agents.insert(
        "R1",
        Box::new(move |_: &Request, n: usize| {
            if n == 1 {
                h.pass();
            }
            (Vec::new(), Vec::new())
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    hold.shut();
    let came = turns_until(&mut e, &Go::default(), |_| hold.came() > 0);
    let log = said.lock().unwrap().join("\n");
    assert!(came, "{log}");
    let work = state(&e, item).unwrap().0;
    assert_eq!(open_rounds(&e, work), 1, "{log}");
    let projects = e.config.config.projects.clone();
    e.db.write(Actor::Owner, |tx| {
        engine::ending::cancel_item(tx, item, "no longer needed", "owner", &projects).map_err(db::Error::Abort)
    })
    .unwrap();
    let rows = |e: &Engine| -> Vec<Value> {
        let mut out = engine::query_json(
            e.db.conn(),
            "SELECT at, ended, paused, paused_by FROM works WHERE id = ?1",
            &[&work],
            10,
        )
        .unwrap();
        out.extend(
            engine::query_json(
                e.db.conn(),
                "SELECT n, cell, ended_at, output, transition, rejected FROM rounds WHERE work = ?1 ORDER BY n",
                &[&work],
                1000,
            )
            .unwrap(),
        );
        out
    };
    let cancelled = rows(&e);
    assert!(!cancelled[0]["ended"].is_null(), "{cancelled:?}");
    assert_eq!(open_rounds(&e, work), 0, "{log}");
    hold.open();
    e.drain().unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(rows(&e), cancelled, "{log}");
    // What the round cost is kept on it.
    let cost: Option<String> =
        e.db.conn()
            .query_row(
                "SELECT usage FROM rounds WHERE work = ?1 AND cell = 'R1'",
                [work],
                |r| r.get(0),
            )
            .unwrap();
    assert!(cost.is_some(), "{log}");
    e.dispatch(&Go::default()).unwrap();
    assert!(!e.in_flight().contains(&work), "{log}");
    e.drain().unwrap();
    assert_eq!(rows(&e), cancelled, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// While the loop looks at the network as a wait for no network ends, a step on another thread may stop every work for
// another cause, here a usage limit that resets in an hour: the look ends only the wait it looked at, and the one set
// meanwhile still stops every work, so that no step is dispatched before the limit resets.
#[test]
fn a_halt_a_step_sets_while_the_loop_looks_at_the_network_holds() {
    let s = setup("halt-during-a-look");
    let look = Gate::default();
    let l = look.clone();
    // The network is up, once the test lets the look through.
    let hooks = Hooks {
        network: Some(Arc::new(move || {
            l.pass();
            true
        })),
        ..Hooks::default()
    };
    let (e, said) = engine_with(&s, scripts(), hooks);
    e.set_halt(Some(engine::Halt {
        kind: "offline".into(),
        until: 0,
        why: "no network".into(),
    }));
    look.shut();
    let t = e.fork().unwrap();
    let looking = std::thread::spawn(move || t.halted());
    let until = Instant::now() + Duration::from_secs(10);
    while look.came() == 0 && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(look.came(), 1);
    // A step's run meets a usage limit while the look is under way.
    let limit = engine::Halt {
        kind: "usage_limit".into(),
        until: db::now() + 3_600_000,
        why: "a usage limit".into(),
    };
    e.set_halt(Some(limit.clone()));
    look.open();
    looking.join().unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(e.halt(), Some(limit), "{log}");
    assert!(e.halted(), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The rounds of two works side by side fail together, for two causes that stop every work: whichever of the two waits
// ends later stands, in either order, so that a wait of half a minute for the network that came after a usage limit
// resetting in an hour does not take its place, and no step is dispatched while the limit holds; the owner is told of
// each cause once.
#[test]
fn of_two_waits_set_together_the_one_that_ends_later_holds() {
    let reset = db::now() / 1000 + 3600;
    let limit = format!("Claude AI usage limit reached|{reset}");
    let offline = "API Error: Connection error. (getaddrinfo ENOTFOUND api.anthropic.com)".to_string();
    two_waits_set_together("later-wait-second", limit.clone(), offline.clone(), reset);
    two_waits_set_together("later-wait-first", offline, limit, reset);
}

/// Two works whose rounds of `R1` run side by side and fail, the first to come with `first` and the other, once that
/// wait stands, with `second`; the wait for the usage limit that resets at `reset`, in seconds, is the one that holds.
fn two_waits_set_together(name: &str, first: String, second: String, reset: i64) {
    let at = Gate::default();
    let then = Gate::default();
    let after = then.clone();
    let mut agents = scripts();
    agents.insert(
        "R1",
        Box::new(move |_: &Request, n: usize| {
            let report = match n {
                1 => first.clone(),
                2 => second.clone(),
                _ => return (Vec::new(), Vec::new()),
            };
            at.meet(2);
            // The second fails only once the test saw the first's wait set.
            if n == 2 {
                after.pass();
            }
            (Vec::new(), vec![("@error", json!(report))])
        }),
    );
    let (s, mut e, said, a, b) = two_works_with(name, agents, Hooks::default());
    // The first pass starts both works; the next dispatches the round of `R1` of each.
    e.tick().unwrap();
    then.shut();
    e.dispatch(&Go::default()).unwrap();
    let until = Instant::now() + Duration::from_secs(10);
    while e.halt().is_none() && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(5));
    }
    let log = said.lock().unwrap().join("\n");
    assert!(e.halt().is_some(), "{log}");
    then.open();
    e.drain().unwrap();
    let log = said.lock().unwrap().join("\n");
    let h = e.halt().unwrap_or_else(|| panic!("no wait:\n{log}"));
    assert_eq!((h.kind.as_str(), h.until), ("usage_limit", reset * 1000), "{log}");
    let mut kinds = failed_kinds(&e);
    kinds.sort();
    assert_eq!(kinds, ["offline", "usage_limit"], "{log}");
    let before = rounds_count(&e);
    e.tick().unwrap();
    assert_eq!(rounds_count(&e), before, "{log}");
    assert_eq!((told(&s, "usage_limit"), told(&s, "offline")), (1, 1), "{log}");
    // Once the limit resets, both works go on from their cell.
    e.set_halt(e.halt().map(|h| engine::Halt { until: 0, ..h }));
    e.tick().unwrap();
    let log = said.lock().unwrap().join("\n");
    for item in [a, b] {
        let (_, at, paused) = state(&e, item).unwrap();
        assert_eq!(paused, None, "{log}");
        assert_ne!(at, "R1", "{log}");
    }
    let _ = std::fs::remove_dir_all(&s.root);
}

// The rounds of three works side by side fail one after another while the first wait stands: a usage limit, then no
// network, then the usage limit again. The owner is told of each cause once, as the first time it stops every work:
// the third round's limit is the one the owner was told of first, and the wait for it still stands, so no second
// notice of it comes because a notice of another cause came between.
#[test]
fn a_cause_told_once_is_not_told_again_after_another_cause_between() {
    let reset = db::now() / 1000 + 3600;
    let limit = format!("Claude AI usage limit reached|{reset}");
    let offline = "API Error: Connection error. (getaddrinfo ENOTFOUND api.anthropic.com)".to_string();
    let met = Gate::default();
    let (second, third) = (Gate::default(), Gate::default());
    let (g2, g3) = (second.clone(), third.clone());
    let mut agents = scripts();
    agents.insert(
        "R1",
        Box::new(move |_: &Request, n: usize| {
            let report = match n {
                1 | 3 => limit.clone(),
                2 => offline.clone(),
                _ => return (Vec::new(), Vec::new()),
            };
            met.meet(3);
            // The second and the third fail only once the test let each through, one after the other.
            match n {
                2 => g2.pass(),
                3 => g3.pass(),
                _ => {}
            }
            (Vec::new(), vec![("@error", json!(report))])
        }),
    );
    let s = setup("told-once-between");
    let install = std::fs::read_to_string(s.data.join("install.toml")).unwrap();
    write(&s.data, "install.toml", &install.replace("slots = 1", "slots = 3"));
    git(&s.data, &["commit", "-q", "-am", "three slots"]);
    let (mut e, said, _) = start_with(&s, agents, Hooks::default());
    e.db.write(Actor::Owner, |tx| {
        for (title, position) in [("Add another", 2.0), ("Add a third", 3.0)] {
            tx.execute(
                "INSERT INTO items (project, title, problem, why, deliver, level) VALUES ('p', ?1, 'x', 'y', 'change', 'l')",
                params![title],
            )?;
            let id = tx.last_insert_rowid();
            tx.execute(
                "INSERT INTO dispatch_queue (item, position) VALUES (?1, ?2)",
                params![id, position],
            )?;
        }
        Ok(())
    })
    .unwrap();
    // The first pass starts the three works; the next dispatches the round of `R1` of each.
    e.tick().unwrap();
    second.shut();
    third.shut();
    e.dispatch(&Go::default()).unwrap();
    let failed = |e: &Engine, n: usize| {
        let until = Instant::now() + Duration::from_secs(10);
        while failed_kinds(e).len() < n && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(5));
        }
        failed_kinds(e).len()
    };
    let log = said.lock().unwrap().join("\n");
    assert_eq!(failed(&e, 1), 1, "{log}");
    second.open();
    assert_eq!(failed(&e, 2), 2, "{log}");
    third.open();
    e.drain().unwrap();
    // The notices the steps queued are written out by the loop's next turn, which dispatches nothing while the wait
    // stands.
    e.tick().unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(failed_kinds(&e).len(), 3, "{log}");
    let h = e.halt().unwrap_or_else(|| panic!("no wait:\n{log}"));
    assert_eq!((h.kind.as_str(), h.until), ("usage_limit", reset * 1000), "{log}");
    assert_eq!((told(&s, "usage_limit"), told(&s, "offline")), (1, 1), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A failed run's cause that another work's round meets after the loop dispatched a step stops that step's round too,
// as the owner is told no agent round starts: the second work's step is held at its first write, the session it
// opens, before its round starts, until the first work's round failed for a usage limit; its round does not start, and
// it goes on from its cell once the wait is over.
#[test]
fn a_round_whose_step_was_dispatched_before_a_halt_does_not_start() {
    let reset = db::now() / 1000 + 3600;
    let mut agents = scripts();
    agents.insert(
        "R1",
        Box::new(move |_: &Request, n: usize| {
            if n == 1 {
                let report = format!("Claude AI usage limit reached|{reset}");
                return (Vec::new(), vec![("@error", json!(report))]);
            }
            (Vec::new(), Vec::new())
        }),
    );
    let stop = Gate::default();
    let held = Arc::new(AtomicI64::new(0));
    let (g, which) = (stop.clone(), held.clone());
    // The first write on the thread of the work held waits at the gate, once.
    let hooks = Hooks {
        commit: Some(Arc::new(move |_: &db::rusqlite::Connection, _: &[db::Changed]| {
            let w = which.load(Ordering::SeqCst);
            if w != 0 && std::thread::current().name() == Some(format!("work {w}").as_str()) {
                which.store(0, Ordering::SeqCst);
                g.pass();
            }
        })),
        ..Hooks::default()
    };
    let (s, mut e, said, first, second) = two_works_with("halt-before-a-round", agents, hooks);
    // The first pass starts both works; the next dispatches the round of `R1` of each.
    e.tick().unwrap();
    let b = state(&e, second).unwrap().0;
    stop.shut();
    held.store(b, Ordering::SeqCst);
    // The second's round starts a new session, as the owner may ask, whose opening is its step's first write.
    e.sessions().ask_new(api::Fresh::Work(b));
    e.dispatch(&Go::default()).unwrap();
    let until = Instant::now() + Duration::from_secs(10);
    while !(stop.came() > 0 && e.halt().is_some()) && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(5));
    }
    let log = said.lock().unwrap().join("\n");
    assert!(stop.came() > 0 && e.halt().is_some(), "{log}");
    stop.open();
    e.drain().unwrap();
    let log = said.lock().unwrap().join("\n");
    let rounds_at = |e: &Engine, work: i64| -> i64 {
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM rounds WHERE work = ?1 AND cell = 'R1'",
                [work],
                |r| r.get(0),
            )
            .unwrap()
    };
    assert_eq!(rounds_at(&e, b), 0, "{log}");
    let (_, at, paused) = state(&e, second).unwrap();
    assert_eq!((at.as_str(), paused), ("R1", None), "{log}");
    assert_eq!(failed_kinds(&e), ["usage_limit"], "{log}");
    // Once the limit resets, both works go on from their cell.
    e.set_halt(e.halt().map(|h| engine::Halt { until: 0, ..h }));
    e.tick().unwrap();
    let log = said.lock().unwrap().join("\n");
    for item in [first, second] {
        let (_, at, paused) = state(&e, item).unwrap();
        assert_eq!(paused, None, "{log}");
        assert_ne!(at, "R1", "{log}");
    }
    let _ = std::fs::remove_dir_all(&s.root);
}

// Of two works that wait, holding no slot, with one slot free, the one whose wait is over goes on, though the other,
// whose wait is not over, has the lower id: the loop grants slots only to works whose step found their wait over, so
// that a work still waiting never takes the slot a work back from waiting needs. Here the second work asked the
// architect and its reply came, while the first still waits for its own reply.
#[test]
fn a_work_back_from_waiting_takes_the_free_slot_before_one_still_waiting_with_a_lower_id() {
    let s = setup("still-waiting-lower-id");
    let mut agents = scripts();
    agents.insert(
        "R1",
        Box::new(|_: &Request, n: usize| {
            if n <= 2 {
                an_ask("architect")
            } else {
                (Vec::new(), Vec::new())
            }
        }),
    );
    let (mut e, said, first) = start(&s, agents);
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
    // The question each work put to the architect, taken off the queue as if a work of the architect had it in hand.
    let question = |e: &mut Engine, item: i64| -> Option<i64> {
        let m: Option<i64> =
            e.db.conn()
                .query_row(
                    "SELECT id FROM architect_matters WHERE kind = 'engineer_question' AND item = ?1",
                    [item],
                    |r| r.get(0),
                )
                .ok();
        if let Some(m) = m {
            e.db.write(Actor::Engine, |tx| {
                tx.execute(
                    "UPDATE architect_matters SET handled_work = (SELECT min(id) FROM works WHERE graph = 'architect'),
                       handled_round = 1 WHERE id = ?1 AND handled_round IS NULL",
                    [m],
                )?;
                Ok(())
            })
            .unwrap();
        }
        m
    };
    // With one slot, the first work asks and waits, holding none, and the second starts in it and asks in turn.
    assert!(
        steps_until(&mut e, 30, |e| question(e, first).is_some()
            && question(e, second).is_some()),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let asked = question(&mut e, second).unwrap();
    let (a, _, _) = state(&e, first).unwrap();
    let (b, _, _) = state(&e, second).unwrap();
    assert!(a < b);
    e.db.write(Actor::Architect, |tx| {
        tx.execute(
            "UPDATE architect_matters SET reply = ?2 WHERE id = ?1",
            params![asked, json!({ "text": "Go ahead.", "why": "It holds." }).to_string()],
        )?;
        Ok(())
    })
    .unwrap();
    assert!(
        steps_until(&mut e, 10, |e| state(e, second).is_some_and(|x| x.1 == "A4")),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    assert_eq!(state(&e, first).map(|x| x.1).as_deref(), Some("A2"), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The owner's Step of a work whose round a failed run's cause stopped before it started, the cause met by another
// work's round after the loop dispatched the Step, is not taken: the work is not paused and the ask stands; once the
// wait is over, the Step runs exactly one round and pauses the work after it. Every work is held by the owner's global
// pause but the one that fails; the stepped work's step is held at its first write, the new session it opens.
#[test]
fn the_owner_s_step_stopped_by_a_halt_before_its_round_stands_until_the_wait_is_over() {
    let reset = db::now() / 1000 + 3600;
    let mut agents = scripts();
    agents.insert(
        "R1",
        Box::new(move |_: &Request, n: usize| {
            if n == 1 {
                let report = format!("Claude AI usage limit reached|{reset}");
                return (Vec::new(), vec![("@error", json!(report))]);
            }
            (Vec::new(), Vec::new())
        }),
    );
    let stop = Gate::default();
    let held = Arc::new(AtomicI64::new(0));
    let (g, which) = (stop.clone(), held.clone());
    // The first write on the thread of the work held waits at the gate, once.
    let hooks = Hooks {
        commit: Some(Arc::new(move |_: &db::rusqlite::Connection, _: &[db::Changed]| {
            let w = which.load(Ordering::SeqCst);
            if w != 0 && std::thread::current().name() == Some(format!("work {w}").as_str()) {
                which.store(0, Ordering::SeqCst);
                g.pass();
            }
        })),
        ..Hooks::default()
    };
    let (s, mut e, said, first, second) = two_works_with("step-before-a-halt", agents, hooks);
    // The first pass starts both works; then the first goes on and the owner steps the second.
    e.tick().unwrap();
    let (a, b) = (state(&e, first).unwrap().0, state(&e, second).unwrap().0);
    let go = Go {
        pause: held_but(&[a], &[]),
        steps: vec![b],
        architects: Vec::new(),
    };
    stop.shut();
    held.store(b, Ordering::SeqCst);
    e.sessions().ask_new(api::Fresh::Work(b));
    e.dispatch(&go).unwrap();
    let until = Instant::now() + Duration::from_secs(10);
    while !(stop.came() > 0 && e.halt().is_some()) && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(5));
    }
    let log = said.lock().unwrap().join("\n");
    assert!(stop.came() > 0 && e.halt().is_some(), "{log}");
    stop.open();
    let jobs = e.drain().unwrap();
    let log = said.lock().unwrap().join("\n");
    let ask = Ask::Work(b);
    assert!(jobs.iter().all(|j| j.asked.as_ref() != Some(&ask)), "{jobs:?}\n{log}");
    let rounds_at = |e: &Engine| -> i64 {
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM rounds WHERE work = ?1 AND cell = 'R1'",
                [b],
                |r| r.get(0),
            )
            .unwrap()
    };
    assert_eq!(rounds_at(&e), 0, "{log}");
    assert_eq!(state(&e, second).unwrap().2, None, "{log}");
    // Once the limit resets, the ask that stood runs one round, and the work is paused after it.
    e.set_halt(e.halt().map(|h| engine::Halt { until: 0, ..h }));
    let p = e.pass(&go).unwrap();
    let log = said.lock().unwrap().join("\n");
    let stepped = asked_in(&p, &ask);
    assert_eq!(
        stepped.rounds.iter().filter(|r| r.work == b).count(),
        1,
        "{stepped:?}\n{log}"
    );
    assert_eq!(rounds_at(&e), 1, "{log}");
    assert!(state(&e, second).unwrap().2.is_some(), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

/// A decision as the cell that records decisions hands it in: what it settles, and the words it rests on named by their
/// record and the part that decides. Without an item it is one no item holds, with the title and tags of the observation
/// it is kept as.
fn decision_of(item: Option<&str>, what: &str, quotes: &[(i64, &str)]) -> Value {
    let mut d = json!({
        "what": what,
        "situation": "The parser's item is being written",
        "reason": "It keeps the item small",
        "reason_by": "said",
        "replaces": "none",
        "words": quotes
            .iter()
            .map(|(m, q)| json!({ "from": "matter", "id": m, "quote": q }))
            .collect::<Vec<_>>(),
    });
    match item {
        Some(i) => d["item"] = json!(i),
        None => {
            d["title"] = json!("Name stays");
            d["tags"] = json!([]);
        }
    }
    d
}

/// The number of the matter that holds the owner's words with `part` in them, read from the database at `path`.
fn matter_with(path: &Path, part: &str) -> i64 {
    let c = db::open_read(path).unwrap();
    c.query_row(
        "SELECT id FROM architect_matters WHERE kind = 'owner_words' AND json_extract(body, '$.text') LIKE ?1",
        [format!("%{part}%")],
        |r| r.get(0),
    )
    .unwrap()
}

/// The day a matter was queued, `YYYY-MM-DD`, as SQLite tells it from the matter's time.
fn day_of(e: &Engine, matter: i64) -> String {
    e.db.conn()
        .query_row(
            "SELECT date(at / 1000, 'unixepoch') FROM architect_matters WHERE id = ?1",
            [matter],
            |r| r.get(0),
        )
        .unwrap()
}

/// One line of a decision about the parser's item as the engine writes it, from the parts the test hands in.
fn decision_line(date: &str, who: &str, what: &str, words: &str) -> String {
    format!(
        "Decision ({date}, {who}): {what}. Situation: The parser's item is being written. Reason: It keeps the \
         item small. Replaces: none. Words: {words}"
    )
}

// The decisions the owner's words of a batch make are handed in by the cell that records them, as the records they quote
// and the part that decides, and are reviewed with the batch's other changes: the review is given the words whole and
// the line the engine made, the review drops one and the others are written under the head `Decisions:` of the item,
// after the item's text the batch changed and before `Learned:`, as the engine makes them (the date, who decided and the
// words from the records, not from what the cell typed), in the order handed in. Words said by the owner are the owner's,
// those of a delegate its own, and a decision on several records takes the least certain. One that names no item is kept
// as an observation, whose text is the line. The change the batch made to the item's text first is not written over.
#[test]
fn the_owner_s_decisions_are_written_by_the_engine_from_the_records_once_the_review_finds_them_sound() {
    let s = setup("decisions-written");
    let path = config::database(&s.data);
    let mut agents = scripts();
    agents.insert(
        "H8",
        Box::new(|_: &Request, n: usize| {
            if n > 1 {
                return (Vec::new(), Vec::new());
            }
            let edit = json!({ "kind": "edit_item", "item": "1", "part": "approach",
                               "text": "Approach:\n- Parse it first.\n\nLearned:\nFact: parse first",
                               "reason": "Say it first." });
            (
                vec![("outcome.answer", "done")],
                vec![
                    ("outcome.answer.reply", json!("Noted.")),
                    ("outcome.effects", json!({ "item_changes": [edit] })),
                ],
            )
        }),
    );
    let at = path.clone();
    agents.insert(
        "B5",
        Box::new(move |_: &Request, _: usize| {
            let (a, b) = (matter_with(&at, "integers"), matter_with(&at, "old name"));
            let looked: Vec<Value> = [a, b]
                .iter()
                .map(|m| json!({ "matter": m, "verdict": "decision", "why": "It settles a choice." }))
                .collect();
            let decisions = json!([
                decision_of(
                    Some("1"),
                    "Integers come first",
                    &[(a, "the parser takes integers first")]
                ),
                decision_of(Some("1"), "The old name stays", &[(b, "keep the old name")]),
                decision_of(
                    Some("1"),
                    "No browser check at all",
                    &[(a, "No browser check"), (b, "Fine")]
                ),
                decision_of(None, "The name stays for good", &[(b, "Fine, keep")]),
            ]);
            (
                Vec::new(),
                vec![("outcome.looked", json!(looked)), ("outcome.decisions", decisions)],
            )
        }),
    );
    agents.insert(
        "B2",
        Box::new(|_: &Request, _: usize| {
            let row = |n: i64, v: &str| json!({ "change": n, "verdict": v, "why": "Judged against the words." });
            (
                Vec::new(),
                vec![(
                    "outcome.review",
                    json!([row(1, "ok"), row(2, "ok"), row(3, "drop"), row(4, "ok"), row(5, "ok")]),
                )],
            )
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    e.db.write(Actor::Owner, |tx| {
        tx.execute("UPDATE items SET approach = 'Approach:\n- Parse it.' WHERE id = 1", [])?;
        Ok(())
    })
    .unwrap();
    db::say_to_architect(
        &mut e.db,
        "p",
        None,
        None,
        "Yes, the parser takes integers first. No browser check.",
        &[],
        db::Speaker::Owner,
    )
    .unwrap();
    db::say_to_architect(
        &mut e.db,
        "p",
        None,
        None,
        "Fine, keep the old name.",
        &[],
        db::Speaker::Delegate,
    )
    .unwrap();
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 80, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let path_taken = path_of(&e, arch);
    let at = path_taken.iter().position(|c| c == "B5").expect(&log);
    assert_eq!(&path_taken[at..at + 3], ["B5", "B2", "B4"], "{log}");
    assert_eq!(rows_at(&e, arch, "B5"), ["decisions"], "{log}");
    let (a, b) = (matter_with(&path, "integers"), matter_with(&path, "old name"));
    let (da, dayb) = (day_of(&e, a), day_of(&e, b));
    let latest = std::cmp::max(da.clone(), dayb.clone());
    let first = decision_line(
        &da,
        "owner",
        "Integers come first",
        &format!("\"the parser takes integers first\" (matter {a}, {da})"),
    );
    let third = decision_line(
        &latest,
        "delegate",
        "No browser check at all",
        &format!("\"No browser check\" (matter {a}, {da}); \"Fine\" (matter {b}, {dayb})"),
    );
    let approach: String =
        e.db.conn()
            .query_row("SELECT approach FROM items WHERE id = 1", [], |r| r.get(0))
            .unwrap();
    assert_eq!(
        approach,
        format!("Approach:\n- Parse it first.\nDecisions:\n{first}\n{third}\n\nLearned:\nFact: parse first"),
        "{log}"
    );
    let states: Vec<String> = engine::query_json(
        e.db.conn(),
        "SELECT state FROM architect_item_changes ORDER BY id",
        &[],
        10,
    )
    .unwrap()
    .iter()
    .map(|r| r["state"].as_str().unwrap_or_default().to_string())
    .collect();
    assert_eq!(states, ["made", "made", "dropped", "made", "made"], "{log}");
    // The one that names no item is an observation, its text the line.
    let (title, what): (String, String) =
        e.db.conn()
            .query_row("SELECT title, what FROM observations", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
    assert_eq!(title, "Name stays");
    assert_eq!(
        what,
        decision_line(
            &dayb,
            "delegate",
            "The name stays for good",
            &format!("\"Fine, keep\" (matter {b}, {dayb})")
        ),
        "{log}"
    );
    // The review was given the owner's words whole and the line to be written.
    let review = message_at(&e, arch, "B2", false);
    assert!(
        review.contains("Yes, the parser takes integers first. No browser check."),
        "{review}"
    );
    assert!(review.contains(&format!("(matter {a}, {da})")), "{review}");
    assert!(review.contains("delegate"), "{review}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A return of the cell that records decisions is sent back for each of these: a row of `looked` missing for some of the
// owner's words and another twice; a quotation that is not the record's letter for letter (the record's words come with
// the reason), words of another project's record, and a row that says decision with no decision quoting its words. Handed
// in sound, the decision goes to the review, which asks for a fix; the revision hands the decision in again, and the line
// written is the revised one, once, with the words from the record.
#[test]
fn a_decision_the_records_do_not_bear_out_is_sent_back_and_a_revised_one_is_written_once() {
    let s = setup("decisions-sent-back");
    let path = config::database(&s.data);
    let mut agents = scripts();
    let foreign = Arc::new(AtomicI64::new(0));
    let (at, other) = (path.clone(), foreign.clone());
    agents.insert(
        "B5",
        Box::new(move |_: &Request, n: usize| {
            let (a, b) = (matter_with(&at, "integers"), matter_with(&at, "old name"));
            let row = |m: i64, v: &str| json!({ "matter": m, "verdict": v, "why": "It is read." });
            match n {
                // One row for the words of `a` twice and none for those of `b`.
                1 => (
                    Vec::new(),
                    vec![("outcome.looked", json!([row(a, "nothing"), row(a, "nothing")]))],
                ),
                // A quotation in other letters, a record of another project, and a row for `b` no decision answers.
                2 => {
                    let wrong = decision_of(Some("1"), "Integers come first", &[(a, "THE PARSER TAKES")]);
                    let elsewhere = decision_of(
                        Some("1"),
                        "Elsewhere",
                        &[(a, "integers"), (other.load(Ordering::Relaxed), "Elsewhere.")],
                    );
                    (
                        Vec::new(),
                        vec![
                            ("outcome.looked", json!([row(a, "decision"), row(b, "decision")])),
                            ("outcome.decisions", json!([wrong, elsewhere])),
                        ],
                    )
                }
                _ => {
                    let sound = decision_of(
                        Some("1"),
                        "Integers come first",
                        &[(a, "the parser takes integers first")],
                    );
                    (
                        Vec::new(),
                        vec![
                            ("outcome.looked", json!([row(a, "decision"), row(b, "nothing")])),
                            ("outcome.decisions", json!([sound])),
                        ],
                    )
                }
            }
        }),
    );
    agents.insert(
        "B2",
        Box::new(|_: &Request, n: usize| {
            let row = if n == 1 {
                json!({ "change": 1, "verdict": "fix", "about": "constitution_and_policy",
                        "what": "It says what was done, not what is settled.", "how": "Say what stands.",
                        "cause": "rule_missing" })
            } else {
                json!({ "change": 1, "verdict": "ok", "why": "Now it says what stands." })
            };
            (Vec::new(), vec![("outcome.review", json!([row]))])
        }),
    );
    let at = path.clone();
    agents.insert(
        "B3",
        Box::new(move |_: &Request, _: usize| {
            let a = matter_with(&at, "integers");
            let revised = decision_of(
                Some("1"),
                "Integers come first, always",
                &[(a, "the parser takes integers first")],
            );
            (
                Vec::new(),
                vec![("outcome.changes", json!([{ "matter": a, "decision": revised }]))],
            )
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    e.db.write(Actor::Owner, |tx| {
        tx.execute("UPDATE items SET approach = 'Approach:\n- Parse it.' WHERE id = 1", [])?;
        Ok(())
    })
    .unwrap();
    // Another project's words, handled already, which no batch of p's holds.
    let elsewhere =
        e.db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO architect_matters (project, kind, detail, what, body, at, handled_work, handled_round)
                 VALUES ('q', 'owner_words', 'said', 'w',
                         '{\"text\":\"Elsewhere.\",\"where\":\"page\",\"speaker\":\"owner\"}', 1,
                         (SELECT id FROM works WHERE ended = 'over before the test'), 1)",
                [],
            )?;
            Ok(tx.last_insert_rowid())
        })
        .unwrap();
    foreign.store(elsewhere, Ordering::Relaxed);
    say(&mut e, "Yes, the parser takes integers first. No browser check.");
    say(&mut e, "Fine, keep the old name.");
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 80, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let sent_back = why_sent_back(&e, arch, "B5");
    assert_eq!(sent_back.len(), 2, "{sent_back:?}\n{log}");
    assert!(
        sent_back[0].contains("there is one row for each of the owner's words this batch handled"),
        "{sent_back:?}"
    );
    let (a, b) = (matter_with(&path, "integers"), matter_with(&path, "old name"));
    for part in [
        "is not found as it is in the words of matter".to_string(),
        "Yes, the parser takes integers first. No browser check.".to_string(),
        format!("matter {elsewhere} is of q, and the decision is of p"),
        format!("matter {b}: its row says it holds a decision, and no decision quotes it"),
    ] {
        assert!(sent_back[1].contains(&part), "{part}\n{sent_back:?}");
    }
    let path_taken = path_of(&e, arch);
    let at = path_taken.iter().rposition(|c| c == "B5").expect(&log);
    assert_eq!(&path_taken[at..at + 5], ["B5", "B2", "B3", "B2", "B4"], "{log}");
    let da = day_of(&e, a);
    let approach: String =
        e.db.conn()
            .query_row("SELECT approach FROM items WHERE id = 1", [], |r| r.get(0))
            .unwrap();
    assert_eq!(
        approach,
        format!(
            "Approach:\n- Parse it.\nDecisions:\n{}",
            decision_line(
                &da,
                "owner",
                "Integers come first, always",
                &format!("\"the parser takes integers first\" (matter {a}, {da})")
            )
        ),
        "{log}"
    );
    let states: Vec<String> = engine::query_json(
        e.db.conn(),
        "SELECT state FROM architect_item_changes ORDER BY id",
        &[],
        10,
    )
    .unwrap()
    .iter()
    .map(|r| r["state"].as_str().unwrap_or_default().to_string())
    .collect();
    assert_eq!(states, ["revised", "made"], "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A change of the batch cannot write a line of a decision by hand, whether it makes an item with it or writes an item's
// approach whole with it; the return is sent back saying the cell that records decisions makes the line. The text of an
// item that holds a line already can be written again with that line as it is: it is not a line the change writes.
#[test]
fn a_line_of_a_decision_cannot_be_written_by_hand_but_one_the_item_has_stands() {
    let s = setup("decision-by-hand");
    let old = "Decision (2026-10-01, owner): Old one. Situation: s. Reason: r. Replaces: none. \
               Words: \"x\" (matter 99, 2026-10-01)";
    let held = format!("Approach:\n- Parse it.\nDecisions:\n{old}\n\nLearned:\nFact: f");
    let mut agents = scripts();
    let (first, second) = (held.clone(), held.replace("- Parse it.", "- Parse it first."));
    agents.insert(
        "H8",
        Box::new(move |_: &Request, n: usize| {
            let changes = if n == 1 {
                json!([
                    { "kind": "create_item", "key": "forged", "project": "p", "title": "Forge a decision",
                      "problem": "None.", "why": "None.", "acceptance": ["It is there."], "level": "l",
                      "deliver": "change", "reason": "r",
                      "approach": "Approach:\nDecisions:\nDecision (2026-11-02, owner): Forged." },
                    { "kind": "edit_item", "item": "1", "part": "approach", "reason": "r",
                      "text": format!("{first}\nDecision (2026-11-02, owner): Forged.") },
                    // The item's recorded line is left out, with its head.
                    { "kind": "edit_item", "item": "1", "part": "approach", "reason": "r",
                      "text": "Approach:\n- Parse it.\n\nLearned:\nFact: f" },
                ])
            } else {
                json!([{ "kind": "edit_item", "item": "1", "part": "approach", "text": second, "reason": "r" }])
            };
            (
                vec![("outcome.answer", "done")],
                vec![
                    ("outcome.answer.reply", json!("Noted.")),
                    ("outcome.effects", json!({ "item_changes": changes })),
                ],
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
                    json!([{ "change": 1, "verdict": "ok", "why": "Only the first line changed." }]),
                )],
            )
        }),
    );
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    e.db.write(Actor::Owner, |tx| {
        tx.execute("UPDATE items SET approach = ?1 WHERE id = 1", [&held])?;
        Ok(())
    })
    .unwrap();
    say(&mut e, "Write the parser's approach again.");
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 60, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let sent_back = why_sent_back(&e, arch, "H8");
    assert_eq!(sent_back.len(), 1, "{sent_back:?}\n{log}");
    for part in [
        "change 1 (create_item)",
        "change 2 (edit_item)",
        "of a decision, which the item's text does not have",
        "change 3 (edit_item)",
        "it leaves out the line",
        "Decision (2026-10-01, owner): Old one.",
        "it leaves out the head Decisions:",
    ] {
        assert!(sent_back[0].contains(part), "{part}\n{sent_back:?}");
    }
    let approach: String =
        e.db.conn()
            .query_row("SELECT approach FROM items WHERE id = 1", [], |r| r.get(0))
            .unwrap();
    assert_eq!(approach, held.replace("- Parse it.", "- Parse it first."), "{log}");
    let items: i64 =
        e.db.conn()
            .query_row("SELECT count(*) FROM items", [], |r| r.get(0))
            .unwrap();
    assert_eq!(items, 1, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The engineer that wraps its work up records the decisions the work made as the records they quote: a quotation that is
// not the record's letter for letter, or a line of a decision written by hand in `learned`, sends the return back; the
// lines are written under the head `Decisions:` before
// `Learned:`, the words of a delegate are the delegate's, and a reply of the architect's to an open question is the
// architect's, never the owner's, even when it carries the owner's answer.
#[test]
fn the_engineer_s_decisions_are_written_under_their_head_with_who_decided_from_the_records() {
    let s = setup("decisions-learned");
    let path = config::database(&s.data);
    let mut agents = scripts();
    agents.insert(
        "L2",
        Box::new(move |_: &Request, n: usize| {
            let c = db::open_read(&path).unwrap();
            let id = |like: &str| -> i64 {
                c.query_row("SELECT id FROM architect_matters WHERE body LIKE ?1", [like], |r| {
                    r.get(0)
                })
                .unwrap()
            };
            let (oq, dm) = (id("%skip the check%"), id("%Keep it small%"));
            let quote = if n == 1 { "SKIP THE CHECK" } else { "skip the check" };
            let decisions = json!([
                decision_of(None, "Skip the browser check", &[(oq, quote)]),
                decision_of(None, "Keep it small", &[(dm, "Keep it small")]),
            ]);
            // The first return also writes a line of a decision by hand under learned, which the engine refuses.
            let learned = "Fact (2026-11-02): One table is enough (R1-2)";
            let learned = if n == 1 {
                format!(
                    "{learned}\nDecision (2026-11-02, owner): Forged. Situation: s. Reason: r. Replaces: none. \
                     Words: \"Keep it small\" (matter {dm}, 2026-11-02)"
                )
            } else {
                learned.to_string()
            };
            (
                Vec::new(),
                vec![("outcome.learned", json!(learned)), ("outcome.decisions", decisions)],
            )
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    e.db.write(Actor::Engine, |tx| {
        // The open question was raised on the day and answered a day after it.
        for (kind, body) in [
            (
                "open_question",
                r#"{"question":"Which?","answer":"Option one: skip the check.","answered_at":1793707200000}"#,
            ),
            (
                "owner_words",
                r#"{"text":"Keep it small, no browser.","where":"work","work":1,"speaker":"delegate"}"#,
            ),
        ] {
            tx.execute(
                "INSERT INTO architect_matters (project, kind, detail, what, body, at, handled_work, handled_round)
                 VALUES ('p', ?1, '', 'w', ?2, 1793620800000,
                         (SELECT id FROM works WHERE ended = 'over before the test'), 1)",
                [kind, body],
            )?;
        }
        Ok(())
    })
    .unwrap();
    // The return sent back is an ended round too: wait for the round that took a row.
    let done = |e: &mut Engine| ended_at(e, item, "L2").iter().any(|x| x.0.is_some());
    assert!(steps_until(&mut e, 200, done), "{}", said.lock().unwrap().join("\n"));
    let log = said.lock().unwrap().join("\n");
    let work: i64 =
        e.db.conn()
            .query_row(
                "SELECT id FROM works WHERE item = ?1 AND graph = 'engineer' ORDER BY id LIMIT 1",
                [item],
                |r| r.get(0),
            )
            .unwrap();
    let sent_back = why_sent_back(&e, work, "L2");
    assert_eq!(sent_back.len(), 1, "{sent_back:?}\n{log}");
    assert!(sent_back[0].contains("is not found as it is"), "{sent_back:?}");
    assert!(
        sent_back[0].contains("learned holds 1 line(s) that start a decision"),
        "{sent_back:?}"
    );
    let day: String =
        e.db.conn()
            .query_row("SELECT date(1793620800, 'unixepoch')", [], |r| r.get(0))
            .unwrap();
    let day_after: String =
        e.db.conn()
            .query_row("SELECT date(1793707200, 'unixepoch')", [], |r| r.get(0))
            .unwrap();
    assert_ne!(day, day_after);
    let oq: i64 =
        e.db.conn()
            .query_row(
                "SELECT id FROM architect_matters WHERE kind = 'open_question'",
                [],
                |r| r.get(0),
            )
            .unwrap();
    let dm: i64 =
        e.db.conn()
            .query_row(
                "SELECT id FROM architect_matters WHERE body LIKE '%Keep it small%'",
                [],
                |r| r.get(0),
            )
            .unwrap();
    let approach: String =
        e.db.conn()
            .query_row("SELECT approach FROM items WHERE id = ?1", [item], |r| r.get(0))
            .unwrap();
    let lines = format!(
        "Decisions:\n{}\n{}\n",
        decision_line(
            &day_after,
            "architect",
            "Skip the browser check",
            &format!("\"skip the check\" (matter {oq}, {day_after})")
        ),
        decision_line(
            &day,
            "delegate",
            "Keep it small",
            &format!("\"Keep it small\" (matter {dm}, {day})")
        ),
    );
    assert!(approach.starts_with(&lines), "{approach}\n{log}");
    let learned = approach.find("Learned:\nFact (2026-11-02): One table is enough (R1-2)\nCommits: ");
    assert!(learned.is_some_and(|at| at >= lines.len()), "{approach}");
    assert!(!approach.contains("Forged"), "{approach}");
    let _ = std::fs::remove_dir_all(&s.root);
}

/// The message the first round of the understanding of project p's architect was sent, when the owner had said each of
/// `texts` to it, oldest first, and the project's notebook holds nothing yet.
fn first_understanding_after(name: &str, texts: &[String]) -> String {
    let s = setup(name);
    let (mut e, said) = engine(&s, scripts());
    quiet(&mut e);
    an_item(&mut e);
    for t in texts {
        say(&mut e, t);
    }
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 30, |e| path_of(e, arch).iter().any(|c| c == "U1")),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let u1 = input_at(&e, arch, "U1", false);
    assert_eq!(given_ending(&u1, "·engine·owner_talk"), Some(true), "{u1}");
    let m = u1["message"].as_str().unwrap().to_string();
    let _ = std::fs::remove_dir_all(&s.root);
    m
}

/// `n` characters of `fill` ending in `end`.
fn long_words(n: usize, fill: char, end: &str) -> String {
    format!("{}{end}", fill.to_string().repeat(n - end.len()))
}

/// The tail of a message, for a failure to show.
fn tail(m: &str) -> &str {
    &m[m.len().saturating_sub(2_000)..]
}

// The talk given to a new session is cut to the character: entries that together make 60,000 characters are all given,
// one character more and the earlier of them is left out whole, and an entry that is alone longer than that is given
// whole. The reference is the limit the design states (60,000 characters, whole entries, latest first, a line after
// them saying how many earlier ones there are).
#[test]
fn the_talk_given_is_cut_at_exactly_the_limit_and_never_inside_an_entry() {
    // Exactly the limit: both long entries are given, the short one before them is the one earlier entry.
    let m = first_understanding_after(
        "owner-talk-exact",
        &[
            "Alpha talk. ALPHAEND".to_string(),
            long_words(30_000, 'g', "GAMMAEND"),
            long_words(30_000, 'd', "DELTAEND"),
        ],
    );
    assert!(
        m.contains("GAMMAEND") && m.contains("DELTAEND"),
        "both fit the limit exactly: {}",
        tail(&m)
    );
    assert!(!m.contains("ALPHAEND"));
    assert!(m.contains("and 1 earlier entry before these"), "{}", tail(&m));

    // One character over: the earlier long entry is left out whole, with the short one.
    let m = first_understanding_after(
        "owner-talk-over",
        &[
            "Alpha talk. ALPHAEND".to_string(),
            long_words(30_001, 'g', "GAMMAEND"),
            long_words(30_000, 'd', "DELTAEND"),
        ],
    );
    assert!(m.contains("DELTAEND"), "{}", tail(&m));
    assert!(
        !m.contains("GAMMAEND") && !m.contains("ALPHAEND"),
        "an entry is left out whole"
    );
    assert!(m.contains("and 2 earlier entries before these"), "{}", tail(&m));

    // The very latest entry is given whole though it alone is over the limit.
    let m = first_understanding_after(
        "owner-talk-alone",
        &[
            "Alpha talk. ALPHAEND".to_string(),
            long_words(70_000, 'e', "EPSILONEND"),
        ],
    );
    assert!(m.contains("EPSILONEND"), "the latest entry is given whole");
    assert!(m.contains("and 1 earlier entry before these"), "{}", tail(&m));
    assert!(!m.contains("ALPHAEND"));
}

/// The owner says words to project p's architect with one image.
fn say_with_an_image(e: &mut Engine, text: &str) {
    say(e, text);
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO attachments (matter, position, media_type, size, hash, bytes)
             VALUES ((SELECT max(id) FROM architect_matters), 1, 'image/png', 4, 'h', zeroblob(4))",
            [],
        )?;
        Ok(())
    })
    .unwrap();
}

// A new session is told the talk of the one-to-one from the first round of the session before it, the one the project's
// latest answered understanding ran in, on: the moment itself counts, and what was said before it is not told. The
// owner's asks are told as the page shows them: an ask asked before that moment and answered at or after it with its
// question and answer, one asked at it, one asked and answered after it, one asked and answered before it not at all,
// one asked after it and not yet answered with its question; and how many images came with words or with an answer,
// not the images. The reference is the owner's page: the entries of the conversation `architect:p` from that moment.
#[test]
fn a_new_session_is_told_the_talk_from_the_start_of_the_session_before_with_its_asks_and_images() {
    let s = setup("owner-talk-window");
    let (mut e, said) = engine(&s, scripts());
    quiet(&mut e);
    an_item(&mut e);
    say(&mut e, "Alpha talk. ALPHAEND");
    let one = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 60, |e| architect_work_done(e, one)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    // The work's session began 100 minutes ago: its first round, the understanding's, started at `t - 30 seconds`, the
    // later rounds after it, and what was said before that earlier still. The notebook the work wrote plays no part.
    let t = db::now() - 100 * 60_000;
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "UPDATE rounds SET started_at = ?1 - 30000,
                    ended_at = ?1 - CASE WHEN cell = 'U1' THEN 20000 ELSE 0 END WHERE work = ?2",
            params![t, one],
        )?;
        tx.execute("UPDATE architect_matters SET at = ?1 - 100000", [t])?;
        Ok(())
    })
    .unwrap();
    let ask = |e: &mut Engine, question: &str, asked: i64, answer: Option<(&str, i64)>, images: i64| {
        e.db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO owner_asks (work, round, body, answer) VALUES (?1, 1, ?2, ?3)",
                params![
                    one,
                    json!({ "question": question }).to_string(),
                    answer.map(|(text, at)| json!({ "by": "owner", "text": text, "at": at }).to_string())
                ],
            )?;
            let id = tx.last_insert_rowid();
            // An ask is asked when its notice says so.
            tx.execute(
                "INSERT INTO engine_notices (work, text, at, ask) VALUES (?1, 'An ask.', ?2, ?3)",
                params![one, asked, id],
            )?;
            for position in 1..=images {
                tx.execute(
                    "INSERT INTO attachments (ask, position, media_type, size, hash, bytes)
                     VALUES (?1, ?2, 'image/png', 4, 'h', zeroblob(4))",
                    params![id, position],
                )?;
            }
            Ok(())
        })
        .unwrap();
    };
    // The cut is `t - 30 seconds`. Asked and answered before it: said before the session began. One answered at it, and
    // one asked at it and not answered: the moment itself counts. One asked before it and answered after it, and one
    // asked and answered after it, the answer with two images, and one asked after it and not answered.
    ask(&mut e, "QOLD?", t - 50_000, Some(("AOLD", t - 40_000)), 0);
    ask(&mut e, "QATCUT?", t - 30_000, None, 0);
    ask(&mut e, "QEDGE?", t - 31_000, Some(("AEDGE", t - 30_000)), 0);
    ask(&mut e, "QMID?", t - 15_000, Some(("AMID", t - 12_000)), 0);
    ask(&mut e, "QSTRADDLE?", t - 35_000, Some(("ASTRADDLE", t + 3_000)), 0);
    ask(&mut e, "QOPEN?", t + 5_000, None, 0);
    ask(&mut e, "QNEW?", t + 4_000, Some(("ANEW", t + 6_000)), 2);
    // The owner's words now, with one image.
    say_with_an_image(&mut e, "Beta talk. BETAEND");
    let two = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 30, |e| path_of(e, two).iter().any(|c| c == "U1")),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let sessions = architect_sessions(&e, one, two);
    assert_eq!(sessions[1].0, "expired", "{sessions:?}\n{log}");
    let u1 = input_at(&e, two, "U1", false);
    assert_eq!(given_ending(&u1, "·engine·owner_talk"), Some(true), "{u1}");
    let m = u1["message"].as_str().unwrap();
    for gone in ["ALPHAEND", "QOLD", "AOLD"] {
        assert!(
            !m.contains(gone),
            "{gone} was said before the session before began: {m}"
        );
    }
    for told in [
        "QATCUT",
        "QEDGE",
        "AEDGE",
        "QMID",
        "AMID",
        "QSTRADDLE",
        "ASTRADDLE",
        "QOPEN",
        "QNEW",
        "ANEW",
        "BETAEND",
    ] {
        assert!(m.contains(told), "{told} is told: {m}");
    }
    assert!(m.contains("2 images attached"), "{m}");
    assert!(m.contains("1 image attached"), "{m}");
    // In the order of the times an entry was made, an ask when it was answered if it was: the two at the cut, the one
    // answered 18 seconds after it, the one answered 33 seconds after it, the one asked at 35, the one answered at 36,
    // then the owner's words, which came now.
    let at = |x: &str| m.find(x).unwrap();
    let order = ["QATCUT", "QEDGE", "QMID", "QSTRADDLE", "QOPEN", "QNEW", "BETAEND"];
    assert!(order.windows(2).all(|w| at(w[0]) < at(w[1])), "{m}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// Every round of a new session's understanding is told the same talk. The understanding that asks for tools first is
// given its inputs again at its next round, after the first round has begun the session and its row exists: the cut
// must not move to the session's own first round, which would leave that round with almost nothing and the talk it was
// first given held in a session that no longer matches it. The reference is the talk of the first round: the second
// round records the same content (the same hash) and refers to it, not gives it again, and the first is told the
// second words, not the first.
#[test]
fn the_rounds_of_a_new_sessions_understanding_are_all_told_the_same_talk() {
    let (s, e, said, works) = architect_works_in(
        "owner-talk-rounds",
        understanding_asks_for_tools_once(),
        &[(0, "Alpha talk. ALPHAEND"), (90 * 60_000, "Beta talk. BETAEND")],
    );
    let log = said.lock().unwrap().join("\n");
    let rounds = rounds_numbered(&e, works[1], "U1");
    assert_eq!(rounds.len(), 2, "tools, then the answer: {rounds:?}\n{log}");
    let (first, second) = (
        input_numbered(&e, works[1], rounds[0]),
        input_numbered(&e, works[1], rounds[1]),
    );
    let m = first["message"].as_str().unwrap();
    assert!(m.contains("BETAEND") && !m.contains("ALPHAEND"), "{m}");
    let (before, after) = (talk_recorded(&first).expect(&log), talk_recorded(&second).expect(&log));
    assert_eq!(
        (before.1, after.1),
        (true, false),
        "given once, then referred to: {first}\n{second}"
    );
    assert_eq!(before.0, after.0, "the same talk at both rounds");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A session counts from its first round across the works it ran in, and a session replaced before its understanding
// answered is no session before: a session first began in the first of three works (the later two carried it on), the
// next work's session was replaced at the understanding's second round for its cache (an hour and a half later), and
// that round's session is told the talk from the start of the first, not of the third work, and not from the start of
// the replaced session; its first round, in the replaced session, was told the same.
#[test]
fn the_talk_is_cut_at_the_first_round_of_a_session_across_the_works_it_ran_in() {
    let (s, mut e, said, _) = architect_works_in(
        "owner-talk-carried",
        understanding_asks_for_tools_once(),
        &[
            (0, "Alpha talk. ALPHAEND"),
            (30 * 60_000, "Beta talk. BETAEND"),
            (30 * 60_000, "Gamma talk. GAMMAEND"),
        ],
    );
    let log = said.lock().unwrap().join("\n");
    let started: Vec<String> = {
        let mut st =
            e.db.conn()
                .prepare("SELECT started FROM sessions WHERE line = 'architect' ORDER BY id")
                .unwrap();
        st.query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect()
    };
    assert_eq!(started, ["line", "carried", "carried"], "{started:?}\n{log}");
    // The fourth work begins in a new session an hour and a half after, asks for tools, and the cache of that session
    // is let go before its next round.
    age_architect(&mut e, 90 * 60_000);
    say(&mut e, "Delta talk. DELTAEND");
    let four = e.step(None).unwrap().work.unwrap();
    let took_tools = |e: &mut Engine| {
        cells_of(e, four)
            .iter()
            .any(|c| c.0 == "U1" && c.1.as_deref() == Some("tools"))
    };
    assert!(
        steps_until(&mut e, 20, took_tools),
        "{}",
        said.lock().unwrap().join("\n")
    );
    age_architect(&mut e, 90 * 60_000);
    assert!(
        steps_until(&mut e, 60, |e| architect_work_done(e, four)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let started: Vec<String> = {
        let mut st =
            e.db.conn()
                .prepare("SELECT started FROM sessions WHERE line = 'architect' AND work = ?1 ORDER BY id")
                .unwrap();
        st.query_map([four], |r| r.get(0))
            .unwrap()
            .map(|x| x.unwrap())
            .collect()
    };
    assert_eq!(started, ["expired", "expired"], "{started:?}\n{log}");
    let rounds = rounds_numbered(&e, four, "U1");
    assert_eq!(rounds.len(), 2, "{rounds:?}\n{log}");
    let (first, second) = (input_numbered(&e, four, rounds[0]), input_numbered(&e, four, rounds[1]));
    // From the start of the first work on: the words of the later two works, which the first session ran in too, and
    // the fourth's, are told, and the first work's own words, said before its first round, are not.
    for round in [&first, &second] {
        let m = round["message"].as_str().unwrap();
        for told in ["BETAEND", "GAMMAEND", "DELTAEND"] {
            assert!(m.contains(told), "{told} is told: {m}");
        }
        assert!(!m.contains("ALPHAEND"), "{m}");
    }
    // The session the second round began is given the talk whole.
    assert!(talk_recorded(&second).expect(&log).1, "{second}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// With no previous session there is no lower bound, at any round of the project's first understanding: the round after
// tools, which runs when the session's own row exists, is told the very talk the first round was, the owner's words
// from before the session began included, and refers to it. The reference is the design: no answered understanding of
// the project, no cut.
#[test]
fn the_first_understanding_is_told_the_same_talk_at_every_round_with_no_lower_bound() {
    // The first round of the understanding, the project's first, asks for tools; the round after answers.
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
    let (s, e, said, works) = architect_works_in("owner-talk-first-rounds", agents, &[(0, "Alpha talk. ALPHAEND")]);
    let log = said.lock().unwrap().join("\n");
    let rounds = rounds_numbered(&e, works[0], "U1");
    assert_eq!(rounds.len(), 2, "tools, then the answer: {rounds:?}\n{log}");
    let (first, second) = (
        input_numbered(&e, works[0], rounds[0]),
        input_numbered(&e, works[0], rounds[1]),
    );
    let m = first["message"].as_str().unwrap();
    assert!(m.contains("ALPHAEND"), "said before the session began, and told: {m}");
    let (before, after) = (talk_recorded(&first).expect(&log), talk_recorded(&second).expect(&log));
    assert_eq!(
        (before.1, after.1),
        (true, false),
        "given once, then referred to: {first}\n{second}"
    );
    assert_eq!(before.0, after.0, "the same talk at both rounds");
    let _ = std::fs::remove_dir_all(&s.root);
}

// Every kind of entry of the one-to-one is told from the first round of the session before on, and none from before it:
// the owner's interjection with its images counted, and an entry of a talk, which the page shows in the conversation.
// The reference is the owner's page, which shows these entries in the conversation `architect:p`, and the design: at or
// after the session before began.
#[test]
fn a_new_session_is_told_an_interjection_and_a_talk_entry_made_since_the_session_before_began() {
    let s = setup("owner-talk-kinds");
    let (mut e, said) = engine(&s, scripts());
    quiet(&mut e);
    an_item(&mut e);
    say(&mut e, "Alpha talk. ALPHAEND");
    // Said clearly before the work's first round.
    e.db.write(Actor::Engine, |tx| {
        tx.execute("UPDATE architect_matters SET at = at - 5000", [])?;
        Ok(())
    })
    .unwrap();
    let one = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 60, |e| architect_work_done(e, one)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    age_architect(&mut e, 90 * 60_000);
    // The session began with the first round of the work, whatever it was.
    let cut: i64 =
        e.db.conn()
            .query_row(
                "SELECT min(started_at) FROM rounds WHERE work = ?1 AND session IS NOT NULL",
                [one],
                |r| r.get(0),
            )
            .unwrap();
    e.db.write(Actor::Engine, |tx| {
        let matter: i64 = tx.query_row("SELECT min(id) FROM architect_matters", [], |r| r.get(0))?;
        for (text, at) in [("TALKOLD", cut - 1_000), ("TALKNEW", cut + 1_000)] {
            tx.execute(
                "INSERT INTO talk_entries (matter, text, at) VALUES (?1, ?2, ?3)",
                params![matter, text, at],
            )?;
        }
        tx.execute(
            "INSERT INTO interjections (work, line, speaker, text, delivered_round)
             VALUES (?1, 'architect', 'owner', 'INTERJECTEND', 1)",
            [one],
        )?;
        let id = tx.last_insert_rowid();
        for position in 1..=3 {
            tx.execute(
                "INSERT INTO attachments (interjection, position, media_type, size, hash, bytes)
                 VALUES (?1, ?2, 'image/png', 4, 'h', zeroblob(4))",
                params![id, position],
            )?;
        }
        Ok(())
    })
    .unwrap();
    say(&mut e, "Beta talk. BETAEND");
    let two = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 30, |e| path_of(e, two).iter().any(|c| c == "U1")),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let u1 = input_at(&e, two, "U1", false);
    assert_eq!(given_ending(&u1, "·engine·owner_talk"), Some(true), "{u1}");
    let m = u1["message"].as_str().unwrap();
    assert!(!m.contains("TALKOLD"), "said before the session before began: {m}");
    assert!(!m.contains("ALPHAEND"), "{m}");
    for told in ["TALKNEW", "INTERJECTEND", "BETAEND"] {
        assert!(m.contains(told), "{told} is told: {m}");
    }
    assert!(m.contains("3 images attached"), "{m}");
    let _ = std::fs::remove_dir_all(&s.root);
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

/// The usual shape of a cell's rounds: their line of tokens added is 2,800, of tokens a call 280, and of calls 11; of a
/// jump 2,800 and of a jump for each character 2.8.
fn usual(i: i64) -> (i64, i64, i64, i64) {
    (i * 100, 11, i * 100, 1000)
}

/// A stream of calls, each with its prompt.
fn calls_of(prompts: &[i64]) -> Value {
    let entries: Vec<Value> = prompts
        .iter()
        .enumerate()
        .map(|(i, p)| json!({ "call": format!("m{i}"), "prompt": p }))
        .collect();
    Value::Array(entries)
}

/// One number the database gives.
fn number(e: &Engine, sql: &str) -> i64 {
    e.db.conn().query_row(sql, [], |r| r.get(0)).unwrap()
}

/// What the run guard recorded, in order: each signal with what came of it.
fn guard_events(e: &Engine) -> Vec<(String, String)> {
    let conn = e.db.conn();
    let mut st = conn
        .prepare("SELECT signal, action FROM guard_events ORDER BY id")
        .unwrap();
    st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

/// An item whose `R1` goes past both lines of growth in its first `times` rounds, then runs as usual.
fn growing(name: &str, times: usize) -> (Setup, Engine, Arc<Mutex<Vec<String>>>, i64) {
    let s = setup(name);
    let mut scripts = scripts();
    scripts.insert(
        "R1",
        Box::new(move |_: &Request, n: usize| {
            let stream = vec![("@stream", calls_of(&[10_000, 30_000]))];
            (Vec::new(), if n <= times { stream } else { Vec::new() })
        }),
    );
    let (mut e, said, item) = start(&s, scripts);
    history(&mut e, "R1", 19, &usual);
    (s, e, said, item)
}

// A round whose context grows past both of its cell's lines is stopped at the call that passes them: it is the kind
// `guarded`, and not a run that merely failed; the line's session ends, the cell runs again in a new one told why, and
// the owner hears of it. What the calls cost stands for the report the run never reached, and the round stopped joins
// no history.
#[test]
fn a_round_past_both_lines_is_stopped_and_runs_again_in_a_new_session() {
    let (s, mut e, said, item) = growing("guard-once", 1);
    for _ in 0..8 {
        e.tick().unwrap();
        if state(&e, item).unwrap().1 != "R1" {
            break;
        }
    }
    let log = said.lock().unwrap().join("\n");
    let (work, at, paused) = state(&e, item).unwrap();
    assert_eq!(paused, None, "{log}");
    assert_ne!(at, "R1", "{log}");
    assert_eq!(failed_kinds(&e), ["guarded"], "{log}");
    assert_eq!(guard_events(&e), [("growth".to_string(), "stopped".to_string())]);
    let sessions: Vec<(i64, Option<i64>)> = {
        let conn = e.db.conn();
        let mut st = conn
            .prepare(
                "SELECT r.session, s.ended FROM rounds r JOIN sessions s ON s.id = r.session
                 WHERE r.work = ?1 AND r.cell = 'R1' ORDER BY r.n",
            )
            .unwrap();
        st.query_map([work], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    };
    assert_eq!(sessions.len(), 2, "{sessions:?}");
    assert_ne!(sessions[0].0, sessions[1].0, "{sessions:?}");
    assert!(sessions[0].1.is_some(), "{sessions:?}");
    // Told why, with the signal, its value and its line, and to look at the worktree first.
    let again = input_at(&e, work, "R1", true);
    let told = again["message"].as_str().unwrap_or_default();
    assert!(
        told.contains("The run guard stopped") && told.contains("look at the worktree"),
        "{told}"
    );
    assert!(
        told.contains(r#""signal":"growth""#)
            && told.contains(r#""value":20000.0"#)
            && told.contains(r#""line":2800.0"#),
        "{told}"
    );
    // The round's record keeps the lines it was judged by, where the owner's page reads them.
    let judged = input_at(&e, work, "R1", false);
    assert_eq!(judged["engine"]["guard"]["added"]["value"], 2800.0, "{judged}");
    // The owner is told the first time too.
    let notices = number(
        &e,
        "SELECT count(*) FROM engine_notices WHERE text LIKE '%the run guard stopped%'",
    );
    assert_eq!(notices, 1, "{log}");
    // Its usage comes from its calls, marked so; it is in no history, and the round after it is.
    let (first, estimated): (i64, bool) =
        e.db.conn()
            .query_row(
                "SELECT n, json_extract(usage, '$.estimated') FROM rounds
                 WHERE work = ?1 AND json_extract(rejected, '$.failed.kind') = 'guarded'",
                [work],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
    assert!(estimated);
    let kept = number(
        &e,
        &format!("SELECT count(*) FROM round_measures WHERE work = {work} AND round = {first}"),
    );
    assert_eq!(kept, 0);
    let calls = number(
        &e,
        &format!("SELECT count(*) FROM round_calls WHERE work = {work} AND round = {first}"),
    );
    assert_eq!(calls, 2);
    let _ = std::fs::remove_dir_all(&s.root);
}

// A round that adds much at the pace of its cell's rounds is a long round, as the longest that ever ran was: it runs to
// its return, and what it came to joins the history. Its calls go past their line, which is only recorded, once.
#[test]
fn a_long_round_at_the_usual_pace_is_not_stopped() {
    let s = setup("guard-long");
    let mut scripts = scripts();
    scripts.insert(
        "R1",
        Box::new(|_: &Request, _: usize| {
            let prompts: Vec<i64> = (0..12).map(|i| 10_000 + i * 270).collect();
            (Vec::new(), vec![("@stream", calls_of(&prompts))])
        }),
    );
    let (mut e, said, item) = start(&s, scripts);
    history(&mut e, "R1", 19, &|i| (i * 100, 11, 0, 1000));
    for _ in 0..4 {
        e.tick().unwrap();
        if state(&e, item).unwrap().1 != "R1" {
            break;
        }
    }
    let log = said.lock().unwrap().join("\n");
    assert!(failed_kinds(&e).is_empty(), "{log}");
    assert_eq!(guard_events(&e), [("calls".to_string(), "noted".to_string())], "{log}");
    let (work, at, _) = state(&e, item).unwrap();
    assert_ne!(at, "R1", "{log}");
    let (added, calls): (i64, i64) =
        e.db.conn()
            .query_row(
                "SELECT added, calls FROM round_measures WHERE work = ?1 AND cell = 'R1' AND ledger = 1",
                [work],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
    assert_eq!((added, calls), (2970, 12), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The first prompt of a round past the session's last, by more than its cell's line and by more for each character the
// engine gave, stops the round at its first call; what a failed round left in the session between them counts, since
// the ledger keeps every call.
#[test]
fn a_jump_nothing_given_explains_stops_the_round_at_its_first_call() {
    let s = setup("guard-jump");
    let mut scripts = scripts();
    scripts.insert(
        "R1",
        Box::new(|_: &Request, _: usize| (Vec::new(), vec![("@stream", calls_of(&[10_000]))])),
    );
    scripts.insert(
        "R2",
        Box::new(|_: &Request, n: usize| {
            let failing = vec![("@error", json!("API Error: 529 Overloaded"))];
            let jumping = vec![("@stream", calls_of(&[9_000_000, 9_000_100]))];
            (
                Vec::new(),
                match n {
                    1 => failing,
                    2 => jumping,
                    _ => Vec::new(),
                },
            )
        }),
    );
    let (mut e, said, item) = start(&s, scripts);
    history(&mut e, "R2", 19, &usual);
    for _ in 0..12 {
        e.tick().unwrap();
        if !guard_events(&e).is_empty() {
            break;
        }
    }
    let log = said.lock().unwrap().join("\n");
    assert_eq!(failed_kinds(&e), ["transient", "guarded"], "{log}");
    let (work, _, _) = state(&e, item).unwrap();
    let calls = number(
        &e,
        &format!(
            "SELECT count(*) FROM round_calls c JOIN rounds r ON r.work = c.work AND r.n = c.round
             WHERE c.work = {work} AND json_extract(r.rejected, '$.failed.kind') = 'guarded'"
        ),
    );
    assert_eq!(calls, 1, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// With too few rounds behind it there is no line, and nothing a round does stops it.
#[test]
fn with_too_few_rounds_nothing_is_judged() {
    let s = setup("guard-few");
    let mut scripts = scripts();
    scripts.insert(
        "R1",
        Box::new(|_: &Request, _: usize| (Vec::new(), vec![("@stream", calls_of(&[10_000, 900_000]))])),
    );
    let (mut e, said, item) = start(&s, scripts);
    history(&mut e, "R1", 18, &usual);
    for _ in 0..4 {
        e.tick().unwrap();
        if state(&e, item).unwrap().1 != "R1" {
            break;
        }
    }
    let log = said.lock().unwrap().join("\n");
    assert!(guard_events(&e).is_empty(), "{log}");
    assert!(failed_kinds(&e).is_empty(), "{log}");
    assert_ne!(state(&e, item).unwrap().1, "R1", "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The round run again is given its cell's whole input besides why, and why names the last tools the stopped round used,
// oldest first, with what each was used on, and the second condition of the signal.
#[test]
fn a_round_run_again_is_given_its_cell_whole_and_the_last_tools_of_the_one_stopped() {
    let s = setup("guard-tools");
    let mut scripts = scripts();
    scripts.insert(
        "R1",
        Box::new(|_: &Request, n: usize| {
            let stream = json!([
                { "tool": "Bash", "on": "cargo build", "digest": 1 },
                { "tool": "Read", "on": "src/a.rs", "digest": 2 },
                { "call": "a", "prompt": 10_000 },
                { "call": "b", "prompt": 30_000 },
            ]);
            (Vec::new(), if n == 1 { vec![("@stream", stream)] } else { Vec::new() })
        }),
    );
    let (mut e, said, item) = start(&s, scripts);
    history(&mut e, "R1", 19, &usual);
    for _ in 0..8 {
        e.tick().unwrap();
        if state(&e, item).unwrap().1 != "R1" {
            break;
        }
    }
    let log = said.lock().unwrap().join("\n");
    assert_eq!(failed_kinds(&e), ["guarded"], "{log}");
    let (work, _, _) = state(&e, item).unwrap();
    let again = input_at(&e, work, "R1", true);
    let told = again["message"].as_str().unwrap_or_default();
    assert!(
        told.contains(r#""last_tools":[{"tool":"Bash","on":"cargo build"},{"tool":"Read","on":"src/a.rs"}]"#),
        "{told}"
    );
    assert!(told.contains(r#""also":{"value":20000.0,"line":280.0}"#), "{told}");
    let first = input_at(&e, work, "R1", false);
    let task = first["message"]
        .as_str()
        .and_then(|m| m.split("## Task\n").nth(1))
        .and_then(|m| m.split("## Work from").next())
        .unwrap_or_default();
    assert!(!task.trim().is_empty(), "{first}");
    assert!(told.contains(task), "{told}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// Acceptance: a round run again after a stop that is stopped again pauses the work. A failure that passes between the
// two is a retry of that same round, as the owner's waiver reads it too: it must not start the count again.
#[test]
fn a_round_run_again_and_stopped_after_a_passing_failure_still_pauses_the_work() {
    let s = setup("guard-retry");
    let mut scripts = scripts();
    scripts.insert(
        "R1",
        Box::new(|_: &Request, n: usize| {
            (
                Vec::new(),
                match n {
                    1 | 3 => vec![("@stream", calls_of(&[10_000, 30_000]))],
                    2 => vec![("@error", json!("API Error: 529 Overloaded"))],
                    _ => Vec::new(),
                },
            )
        }),
    );
    let (mut e, said, item) = start(&s, scripts);
    history(&mut e, "R1", 19, &usual);
    for _ in 0..12 {
        e.tick().unwrap();
        let (_, at, paused) = state(&e, item).unwrap();
        if paused.is_some() || at != "R1" {
            break;
        }
    }
    let log = said.lock().unwrap().join("\n");
    assert_eq!(failed_kinds(&e), ["guarded", "transient", "guarded"], "{log}");
    let (_, at, paused) = state(&e, item).unwrap();
    assert_eq!(at, "R1", "{log}");
    let why = paused.unwrap_or_default();
    assert!(why.contains("the run guard stopped"), "{why}\n{log}");
    let told = number(&e, "SELECT count(*) FROM engine_notices WHERE text LIKE '%is paused%'");
    assert_eq!(told, 1, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// Acceptance: the owner's Start lets the cell's round run past its line, however many times a passing failure makes it
// run, and the rounds after it are judged as ever: the next cell's round past its own line is stopped, so the Start did
// not turn the guard off for the work.
#[test]
fn after_the_round_the_owner_let_go_on_the_next_is_judged_as_ever() {
    let s = setup("guard-after");
    let mut scripts = scripts();
    scripts.insert(
        "R1",
        Box::new(|_: &Request, n: usize| {
            let past = || ("@stream", calls_of(&[10_000, 30_000]));
            (
                Vec::new(),
                match n {
                    1 | 2 => vec![past()],
                    3 => vec![past(), ("@error", json!("API Error: 529 Overloaded"))],
                    // The round let go on joined no history, so the same growth is past the same lines again.
                    4 => vec![past()],
                    _ => Vec::new(),
                },
            )
        }),
    );
    scripts.insert(
        "R2",
        Box::new(|_: &Request, n: usize| {
            let past = vec![("@stream", calls_of(&[10_000, 30_000]))];
            (Vec::new(), if n == 1 { past } else { Vec::new() })
        }),
    );
    let (mut e, said, item) = start(&s, scripts);
    history(&mut e, "R1", 19, &usual);
    history(&mut e, "R2", 19, &usual);
    for _ in 0..8 {
        e.tick().unwrap();
        if state(&e, item).unwrap().2.is_some() {
            break;
        }
    }
    let (work, _, paused) = state(&e, item).unwrap();
    let log = said.lock().unwrap().join("\n");
    let why = paused.unwrap_or_default();
    assert!(why.contains("the run guard stopped the agent's run in"), "{why}\n{log}");
    api::resume(&mut e.db, work).unwrap();
    for _ in 0..12 {
        e.tick().unwrap();
        if guard_events(&e).len() >= 5 {
            break;
        }
    }
    let log = said.lock().unwrap().join("\n");
    let actions: Vec<String> = guard_events(&e)
        .into_iter()
        .filter(|(signal, _)| signal == "growth")
        .map(|(_, a)| a)
        .collect();
    assert_eq!(actions, ["stopped", "stopped", "waived", "waived", "stopped"], "{log}");
    // R1 stopped twice, its failure in passing, and R2's stop: no third stop of R1 after the Start.
    let kinds = ["guarded", "guarded", "transient", "guarded"];
    assert_eq!(failed_kinds(&e), kinds, "{log}");
    assert_eq!(state(&e, item).unwrap().2, None, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// What the hook handed in during a round the guard stopped went with the session that ended: the round will never
// answer it, so it waits again, and the new session's whole message gives it.
#[test]
fn what_was_said_during_a_stopped_round_reaches_the_round_run_again() {
    let s = setup("guard-said");
    let mut scripts = scripts();
    let path = config::database(&s.data);
    scripts.insert(
        "R1",
        Box::new(move |_: &Request, n: usize| {
            if n > 1 {
                return (Vec::new(), Vec::new());
            }
            let mut db = db::Db::open(&path).unwrap();
            let (work, round): (i64, i64) = db
                .conn()
                .query_row(
                    "SELECT work, n FROM rounds WHERE cell = 'R1' AND ended_at IS NULL",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .unwrap();
            db.write(Actor::Owner, |tx| {
                tx.execute(
                    "INSERT INTO interjections (work, line, speaker, text)
                     VALUES (?1, 'engineer', 'owner', 'Look at the log.')",
                    [work],
                )?;
                Ok(())
            })
            .unwrap();
            sessions::interject::deliver(&mut db, work, round, "engineer")
                .unwrap()
                .unwrap();
            (Vec::new(), vec![("@stream", calls_of(&[10_000, 30_000]))])
        }),
    );
    let (mut e, said, item) = start(&s, scripts);
    history(&mut e, "R1", 19, &usual);
    for _ in 0..8 {
        e.tick().unwrap();
        if state(&e, item).unwrap().1 != "R1" {
            break;
        }
    }
    let log = said.lock().unwrap().join("\n");
    assert_eq!(failed_kinds(&e), ["guarded"], "{log}");
    let (work, at, _) = state(&e, item).unwrap();
    assert_ne!(at, "R1", "{log}");
    let again = input_at(&e, work, "R1", true);
    let told = again["message"].as_str().unwrap_or_default();
    assert!(told.contains("Look at the log."), "{told}");
    let stopped = input_at(&e, work, "R1", false);
    assert_eq!(stopped["engine"]["interjections"], json!([]), "{stopped}");
    let taken: i64 = number(&e, "SELECT count(*) FROM interjections WHERE delivered_round IS NULL");
    assert_eq!(taken, 0, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A login that expired right after a stop writes nothing in the session, so the round after it is the first the new
// session is told anything by, and it is still told why the round before was stopped.
#[test]
fn a_run_that_never_wrote_its_message_leaves_the_guards_reason_to_the_next() {
    let s = setup("guard-login");
    let mut scripts = scripts();
    scripts.insert(
        "R1",
        Box::new(|_: &Request, n: usize| {
            (
                Vec::new(),
                match n {
                    1 => vec![("@stream", calls_of(&[10_000, 30_000]))],
                    2 => vec![("@error", json!("Not logged in · Please run /login"))],
                    _ => Vec::new(),
                },
            )
        }),
    );
    let (mut e, said, item) = start(&s, scripts);
    history(&mut e, "R1", 19, &usual);
    for _ in 0..8 {
        e.tick().unwrap();
        if let Some(h) = e.halt() {
            e.set_halt(Some(engine::Halt { until: 0, ..h }));
        }
        if state(&e, item).unwrap().1 != "R1" {
            break;
        }
    }
    let log = said.lock().unwrap().join("\n");
    assert_eq!(failed_kinds(&e), ["guarded", "cannot_run"], "{log}");
    let (work, at, _) = state(&e, item).unwrap();
    assert_ne!(at, "R1", "{log}");
    let again = input_at(&e, work, "R1", true);
    let told = again["message"].as_str().unwrap_or_default();
    assert!(told.contains("The run guard stopped"), "{told}");
    assert!(told.contains(r#""also":{"value":20000.0,"line":280.0}"#), "{told}");
    assert!(told.contains(r#""last_tools":"#), "{told}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The loop asks for a work's graph many times a pass: a copy of it each time, for every work not ended, kept a core of
// the machine busy with nothing to do.
#[test]
fn a_works_graph_is_made_once_and_shared() {
    let s = setup("graph-once");
    let (mut e, _, item) = start(&s, scripts());
    e.tick().unwrap();
    let (work, _, _) = state(&e, item).unwrap();
    let w = e.work(work).unwrap();
    let first = e.graph_of(&w).unwrap();
    let again = e.graph_of(&w).unwrap();
    assert!(std::sync::Arc::ptr_eq(&first, &again));
    let _ = std::fs::remove_dir_all(&s.root);
}
