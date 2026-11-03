//! The run guard from outside the session manager: a real process stopped, what the database keeps and gives back, and
//! the judgement over rounds a migration put in, the parts its own tests give it as values.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use db::rusqlite::{Connection, params};
use db::{Actor, Db};
use serde_json::{Value, json};
use sessions::cli::{Agents, Call, Claude, Next, Reply, Request, Seen, Stream};
use sessions::guard::{self, Action, Line, Lines, Run, Sample, Scope, Signal};
use sessions::{Round, RoundId};

const OPUS: &str = "claude-opus-5-5";

/// How many migrations bring a database to version 65, the last one before the ledger.
const BEFORE_THE_LEDGER: usize = 64;

/// A directory of a test's own, empty.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("wf-agents-guard-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A line of the stream telling a call of the agent.
fn a_call_line() -> String {
    json!({ "type": "assistant", "parent_tool_use_id": null, "message": { "id": "a", "model": "m", "usage": {
        "input_tokens": 2, "cache_read_input_tokens": 1000, "cache_creation_input_tokens": 0, "output_tokens": 1,
    }, "content": [] } })
    .to_string()
}

/// Runs a shell script as the agents' command line, stopped at its first call after `wait`.
fn stopped_at_its_first_call(dir: &Path, script: &str, wait: Duration) -> Reply {
    use std::os::unix::fs::PermissionsExt;
    let program = dir.join("cli");
    std::fs::write(&program, script).unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
    let request = Request {
        work: 1,
        cell: "I1".into(),
        line: "engineer".into(),
        cli: config::Cli {
            kind: config::CliKind::Claude,
            command: program.display().to_string(),
            args: Vec::new(),
            config_dir: None,
        },
        model: "m".into(),
        effort: None,
        worktree: dir.to_path_buf(),
        config_dir: None,
        resume: None,
        system: None,
        new_session: "s1".into(),
        prompt: "p".into(),
        images: Vec::new(),
        schema: Value::Null,
        envelope: Value::Null,
        allowed: Vec::new(),
        refused: Vec::new(),
        readable: Vec::new(),
        replay: None,
        regive: None,
        deliver: None,
        bare: true,
    };
    Claude.run(&request, &|seen| match seen {
        Seen::Call(_) => {
            std::thread::sleep(wait);
            Next::Stop("past its line".into())
        }
        _ => Next::Go,
    })
}

/// Whether a process goes away within five seconds.
fn gone(pid: &str) -> bool {
    let alive = || {
        Command::new("kill")
            .args(["-0", pid])
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    };
    (0..100).any(|_| {
        if alive() {
            std::thread::sleep(Duration::from_millis(50));
            return false;
        }
        true
    })
}

// The design's stop is TERM, a short wait, then KILL: a run that takes the polite signal gets to clean up, and ends the
// stop at once instead of holding its slot for the whole wait.
#[test]
fn a_run_that_takes_the_polite_signal_ends_by_it_and_at_once() {
    let dir = scratch("term");
    let asked = dir.join("asked");
    let script = format!(
        "#!/bin/sh\ntrap 'echo asked > {}; exit 0' TERM\nsleep 120 &\nprintf '%s\\n' '{}'\nwait\n",
        asked.display(),
        a_call_line()
    );
    let began = Instant::now();
    let reply = stopped_at_its_first_call(&dir, &script, Duration::ZERO);
    let took = began.elapsed();
    let cleaned_up = asked.exists();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(reply.stopped.as_deref(), Some("past its line"), "{reply:?}");
    assert!(cleaned_up, "the run never had the polite signal");
    assert!(took < Duration::from_millis(2500), "{took:?}");
}

// A stop and the run's own end at once: the command line wrote its return and left, and all that is left of it is what
// it started, which ignores the polite signal and closed the stream, so nothing waits for it. The group still answers
// to the id of the leader, which is not reaped yet; what the run wrote after the call that was stopped is not a return;
// and what the stream said up to there is kept.
#[test]
fn a_stop_after_the_run_left_by_itself_still_takes_what_it_started() {
    let dir = scratch("left");
    let child = dir.join("child");
    let init = r#"{"type":"system","subtype":"init","model":"m","session_id":"s9"}"#;
    let result = r#"{"type":"result","subtype":"success","result":"{\"a\":1}","session_id":"s9","total_cost_usd":0.5}"#;
    let script = format!(
        "#!/bin/sh\n(trap '' TERM; sleep 120) > /dev/null 2>&1 < /dev/null &\necho $! > {}\nprintf '%s\\n' '{init}' '{}' \
         '{result}'\nexit 0\n",
        child.display(),
        a_call_line()
    );
    let reply = stopped_at_its_first_call(&dir, &script, Duration::from_millis(300));
    let pid = std::fs::read_to_string(&child).unwrap().trim().to_string();
    let went = gone(&pid);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(went, "process {pid} outlived the stop");
    assert_eq!(reply.stopped.as_deref(), Some("past its line"), "{reply:?}");
    assert_eq!(reply.error.as_deref(), Some("past its line"), "{reply:?}");
    assert!(reply.output.is_none(), "{reply:?}");
    assert_eq!(reply.model.as_deref(), Some("m"), "{reply:?}");
    assert_eq!(reply.session, "s9", "{reply:?}");
    assert_eq!(reply.usage["first_call"]["cache_read_input_tokens"], 1000, "{reply:?}");
    assert!(reply.usage.get("session_cost_usd").is_none(), "{reply:?}");
}

/// Where [`a_killed_engine_takes_its_run_with_it`] tells the process it kills to run.
const ENGINE_DIR: &str = "WF_AGENTS_GUARD_ENGINE_DIR";

// The process `a_killed_engine_takes_its_run_with_it` kills: it runs an agent's command line that writes down what it
// started and then waits, as an engine does while a round runs.
#[test]
#[ignore = "run only by a_killed_engine_takes_its_run_with_it, as the engine it kills"]
fn an_engine_for_the_test_to_kill() {
    let dir = PathBuf::from(std::env::var(ENGINE_DIR).unwrap());
    let script = format!(
        "#!/bin/sh\nsleep 120 &\necho $! > {}\nwait\n",
        dir.join("child").display()
    );
    stopped_at_its_first_call(&dir, &script, Duration::ZERO);
}

// The README: however the engine ends, the agents it was running are stopped with it. A process that holds a real run
// is killed, with no chance to clean up, and what the run started goes too. The overseer's own test closes its pipe by
// hand; this one has the end of a process close it, through the path a round takes.
#[test]
fn a_killed_engine_takes_its_run_with_it() {
    let dir = scratch("killed");
    let mut engine = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "an_engine_for_the_test_to_kill", "--ignored"])
        .env(ENGINE_DIR, &dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let child = dir.join("child");
    let started = (0..100).any(|_| {
        let written = std::fs::read_to_string(&child).is_ok_and(|t| !t.trim().is_empty());
        if !written {
            std::thread::sleep(Duration::from_millis(50));
        }
        written
    });
    // The overseer starts just after the run does.
    std::thread::sleep(Duration::from_millis(200));
    engine.kill().unwrap();
    engine.wait().unwrap();
    let pid = std::fs::read_to_string(&child).unwrap_or_default().trim().to_string();
    let went = gone(&pid);
    let _ = Command::new("kill")
        .args(["-s", "KILL", &pid])
        .stderr(Stdio::null())
        .status();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(started, "the run never started");
    assert!(went, "process {pid} outlived the engine");
}

// A subagent writes while the agent does: the partial message that ends a call says its output for the call of whoever
// wrote it, as `parent_tool_use_id` tells.
#[test]
fn a_calls_end_goes_to_the_call_of_whoever_wrote_it() {
    let line = |event: Value, parent: Value| {
        json!({ "type": "stream_event", "event": event, "parent_tool_use_id": parent }).to_string()
    };
    let start = |id: &str, parent: Value| {
        line(
            json!({ "type": "message_start", "message": { "id": id, "model": "m", "usage": {
                "input_tokens": 2, "cache_read_input_tokens": 1000, "cache_creation_input_tokens": 0,
                "output_tokens": 1,
            } } }),
            parent,
        )
    };
    let end = |output: i64, parent: Value| {
        line(
            json!({ "type": "message_delta", "usage": { "output_tokens": output } }),
            parent,
        )
    };
    let sub = || json!("toolu_1");
    let lines = [
        start("a", Value::Null),
        start("b", sub()),
        end(9, sub()),
        end(400, Value::Null),
    ];
    let mut stream = Stream::default();
    let told: Vec<(String, i64, bool)> = lines
        .iter()
        .flat_map(|l| stream.line(l))
        .filter_map(|s| match s {
            Seen::Call(c) => Some((c.id, c.output, c.subagent)),
            _ => None,
        })
        .collect();
    let want: [(String, i64, bool); 4] =
        [("a", 1, false), ("b", 1, true), ("b", 9, true), ("a", 400, false)].map(|(i, o, s)| (i.to_string(), o, s));
    assert_eq!(told, want);
}

/// A database with one work whose first `rounds` rounds ran at `I1`.
fn with_rounds(rounds: i64) -> Db {
    let mut db = Db::in_memory().unwrap();
    db.write(Actor::Engine, |tx| {
        tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
        tx.execute(
            "INSERT INTO works (flow_version, graph, at) VALUES (1, 'engineer', 'I1')",
            [],
        )?;
        for n in 1..=rounds {
            tx.execute(
                "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES (1, ?1, 'I1', '{}', 'c', 1)",
                [n],
            )?;
        }
        Ok(())
    })
    .unwrap();
    db
}

/// A call of the agent or of a subagent, of a prompt and an output.
fn told(id: &str, prompt: i64, output: i64, subagent: bool) -> Call {
    Call {
        id: id.into(),
        read: prompt,
        output,
        subagent,
        ..Call::default()
    }
}

// The stream tells a call again as its output grows, and out of order with other calls: the ledger has one row for it,
// in the place it was first told, with the most output told.
#[test]
fn a_call_told_many_times_is_one_row_of_the_ledger_with_the_most_output_told() {
    let mut db = with_rounds(1);
    let round = RoundId::new(1, 1);
    let calls = [
        told("a", 1000, 1, false),
        told("s", 90, 5, true),
        told("a", 1000, 435, false),
        told("a", 1000, 7, false),
        told("b", 2000, 3, false),
    ];
    db.write(Actor::Engine, |tx| {
        for c in &calls {
            guard::keep_call(tx, round, c, 1)?;
        }
        Ok(())
    })
    .unwrap();
    let kept: Vec<(String, i64, i64, bool)> = guard::calls_of(db.conn(), round)
        .unwrap()
        .into_iter()
        .map(|c| (c.id.clone(), c.prompt(), c.output, c.subagent))
        .collect();
    let want: [(String, i64, i64, bool); 3] = [("a", 1000, 435, false), ("s", 90, 5, true), ("b", 2000, 3, false)]
        .map(|(i, p, o, s)| (i.to_string(), p, o, s));
    assert_eq!(kept, want);
}

/// A round of a session as the guard reads it.
fn round(n: i64, output: Option<Value>, usage: Option<Value>) -> Round {
    Round {
        id: RoundId::new(1, n),
        cell: Some("I1".into()),
        session: Some(1),
        input: json!({}),
        output,
        transition: None,
        rejected: None,
        compactions: 0,
        started_at: n,
        ended_at: Some(n),
        usage,
    }
}

// A jump is measured from the session's context, which a subagent's call is not in; a round that made no call says
// nothing of it; a failed round that made calls does, since what it wrote stays in the session; and rounds from before
// the ledger tell it by their record.
#[test]
fn a_session_s_last_prompt_is_its_own_last_call_kept() {
    let mut db = with_rounds(3);
    db.write(Actor::Engine, |tx| {
        guard::keep_call(tx, RoundId::new(1, 1), &told("a", 1000, 1, false), 1)?;
        guard::keep_call(tx, RoundId::new(1, 1), &told("s", 900_000, 1, true), 1)?;
        guard::keep_call(tx, RoundId::new(1, 3), &told("b", 7000, 1, false), 1)?;
        Ok(())
    })
    .unwrap();
    let old = round(
        0,
        None,
        Some(json!({ "usage": { "iterations": [{ "input_tokens": 500 }] } })),
    );
    let (first, failed) = (round(1, None, None), round(2, None, None));
    let failed_with_calls = Round {
        rejected: Some(json!({ "failed": { "kind": "transient", "what": "529" } })),
        ..round(3, None, None)
    };
    let last = |rounds: &[Round]| guard::last_prompt(db.conn(), rounds).unwrap();
    assert_eq!(last(&[old.clone(), first.clone(), failed.clone()]), Some(1000));
    assert_eq!(last(&[first, failed_with_calls]), Some(7000));
    assert_eq!(last(&[old, failed.clone()]), Some(500));
    assert_eq!(last(&[failed]), None);
}

fn at(value: f64) -> Option<Line> {
    Some(Line {
        value,
        samples: 19,
        scope: Scope::Cell,
    })
}

fn lines() -> Lines {
    Lines {
        added: at(1000.0),
        rate: at(100.0),
        jump: at(5000.0),
        ratio: at(2.0),
        cost: at(1.0),
        calls: at(50.0),
        repeats: at(2.0),
        compactions: at(1.0),
    }
}

fn call(id: &str, prompt: i64) -> Seen {
    Seen::Call(told(id, prompt, 0, false))
}

// What was given counts in characters, as the migration's `length` counts them, and the return before the round counts
// though rounds after it left none. A jump as large is a large message when the message explains it, and a stop when it
// does not; the characters here reach the judgement as the engine hands them over.
#[test]
fn a_jump_is_judged_against_the_characters_the_message_gave() {
    let session = [round(1, Some(json!({ "a": 1 })), None), round(2, None, None)];
    // Three bytes, one character.
    assert_eq!(guard::given("wait…", &[]), 5);
    assert_eq!(guard::given("ab", &session), 2 + 7);
    let judged = |message: &str| {
        let given = guard::given(message, &session);
        let mut r = Run::new(lines(), Action::Stopped, OPUS, given, Some(10_000));
        r.see(&call("a", 30_000));
        r.stop_record().is_some()
    };
    assert!(judged(&"x".repeat(5000)));
    assert!(!judged(&"x".repeat(40_000)));
}

// A round's cost counts its subagents' calls, which its context does not: past its line it is noted, once, and nothing
// stops the round. The test beside this one in the module covers repeats and compactions.
#[test]
fn a_cost_past_its_line_is_noted_once_and_the_round_goes_on() {
    let mut r = Run::new(lines(), Action::Stopped, OPUS, 1000, None);
    let spending = |id: &str, subagent: bool| {
        Seen::Call(Call {
            id: id.into(),
            model: Some(OPUS.into()),
            read: 3_000_000,
            subagent,
            ..Call::default()
        })
    };
    let mut past = r.see(&spending("a", false));
    past.extend(r.see(&spending("s", true)));
    past.extend(r.see(&spending("b", false)));
    let noted: Vec<(Signal, Action)> = past
        .iter()
        .filter(|c| c.signal == Signal::Cost)
        .map(|c| (c.signal, c.action))
        .collect();
    assert_eq!(noted, [(Signal::Cost, Action::Noted)], "{past:?}");
    assert!(r.stop_record().is_none());
}

// The flow: "a distribution with no scale gives no line". A new installation's rounds that never jumped have no scale
// at the graph's level either, and the first token of a jump must not stop a round.
#[test]
fn a_graph_whose_rounds_never_jumped_gives_no_line_of_a_jump() {
    let rows: Vec<Sample> = (0..19)
        .map(|_| Sample {
            cell: "I1".into(),
            calls: 5,
            added: 400,
            jump: Some(0),
            given: Some(1000),
            ledger: true,
            ..Sample::default()
        })
        .collect();
    let judged = Lines::of(&rows, "engineer", "I1", OPUS, 19);
    let mut r = Run::new(judged, Action::Stopped, OPUS, 1000, Some(10_000));
    let past = r.see(&call("a", 10_100));
    assert!(past.iter().all(|c| c.signal != Signal::Jump), "{past:?}");
    assert!(r.stop_record().is_none());
}

// Acceptance: a cell without a line of its own is judged by every round of its graph, and only a graph of fewer than 19
// rounds is left unjudged. Most rounds of the architect's graph add nothing, so its median is 0 though it has a scale:
// of its 868 rounds measured on the real history, 753 added 0 tokens, and the most added was 39,549 at 9,280 a call. The
// rows here have that shape, and a round that adds ten times the most at ten times the pace is what the guard is for.
#[test]
fn a_graph_whose_median_is_nothing_but_whose_rounds_grew_still_stops_growth_past_its_lines() {
    let rows: Vec<Sample> = (1..=100)
        .map(|i| Sample {
            cell: "B1".into(),
            calls: 5,
            added: (i - 87).max(0) * 3000,
            ..Sample::default()
        })
        .collect();
    let judged = Lines::of(&rows, "architect", "B1", OPUS, 19);
    let mut r = Run::new(judged, Action::Stopped, OPUS, 1000, None);
    r.see(&call("a", 10_000));
    r.see(&call("b", 410_000));
    assert!(r.stop_record().is_some(), "{:?}", r.lines());
}

// Acceptance: the lines come only from history, no setting holds them, and changing history changes the verdict. The
// verdict is read here from what the database keeps, not from values handed to the judgement.
#[test]
fn the_verdict_follows_the_rounds_kept_and_the_flow_sets_no_line() {
    let mut db = with_rounds(19);
    db.write(Actor::Engine, |tx| {
        for n in 1..=19 {
            let kept = Sample {
                cell: "I1".into(),
                model: Some(OPUS.into()),
                calls: 11,
                added: n * 100,
                jump: Some(n * 100),
                given: Some(1000),
                repeats: Some(0),
                ledger: true,
                ..Sample::default()
            };
            guard::keep_sample(tx, RoundId::new(1, n), "engineer", &kept)?;
        }
        Ok(())
    })
    .unwrap();
    let stopped = |db: &Db| {
        let all = guard::samples(db.conn(), "engineer").unwrap();
        let judged = Lines::of(&all, "engineer", "I1", OPUS, 19);
        let mut r = Run::new(judged, Action::Stopped, OPUS, 1000, None);
        r.see(&call("a", 10_000));
        r.see(&call("b", 30_000));
        r.stop_record().is_some()
    };
    assert!(stopped(&db));
    db.write(Actor::Engine, |tx| {
        tx.execute(
            "UPDATE round_measures SET added = 50000, calls = 2 WHERE round = 19",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    assert!(!stopped(&db));

    let flow: Value = serde_json::from_str(include_str!("../../../flow/flow.json")).unwrap();
    for f in flow["flows"].as_array().unwrap() {
        let rule = f["rounds"]["guard"].as_object().unwrap();
        let fields: Vec<&str> = rule.keys().map(String::as_str).collect();
        assert_eq!(fields, ["desc", "samples"], "the rule may set no line");
    }
}

/// Makes a database at the version before the ledger at `path`, filled by `fill`, with the triggers only the engine's
/// connection can run left out, as a version 65 database needs none of them to be read.
fn before_the_ledger(path: &Path, fill: impl FnOnce(&Connection)) {
    let conn = Connection::open(path).unwrap();
    conn.pragma_update(None, "foreign_keys", false).unwrap();
    conn.execute_batch("BEGIN").unwrap();
    conn.execute_batch(include_str!("../../../db/migrations/0001.sql"))
        .unwrap();
    for m in &db::MIGRATIONS[..BEFORE_THE_LEDGER] {
        conn.execute_batch(m).unwrap();
    }
    conn.execute_batch("COMMIT").unwrap();
    conn.pragma_update(None, "user_version", BEFORE_THE_LEDGER as i64 + 1)
        .unwrap();
    fill(&conn);
}

/// What the command line reports of a round, for its first and last prompts.
fn usage(first: i64, last: i64, turns: i64, cost: f64) -> String {
    let prompt = |p: i64| json!({ "input_tokens": 2, "cache_read_input_tokens": p - 2 });
    json!({
        "first_call": prompt(first), "usage": { "iterations": [prompt(1), prompt(last)] },
        "turns": turns, "cost_usd": cost,
    })
    .to_string()
}

/// Work 38's round 22 as its record has it: 237 calls from a prompt of 169,675 tokens to one of 446,567.
const LONGEST: (i64, i64, i64) = (169_675, 446_567, 237);

/// A round of that shape, its context growing evenly; what the guard saw come of it.
fn replayed(judged: Lines) -> Run {
    let (first, last, calls) = LONGEST;
    let mut r = Run::new(judged, Action::Stopped, OPUS, 1000, None);
    for i in 0..calls {
        r.see(&call(&format!("c{i}"), first + (last - first) * i / (calls - 1)));
    }
    r
}

// Acceptance: a replay on the data as migrated does not stop work 38's round 22. The rounds here go through the real
// migration, and the replay is judged both with the round among them, as now, and without it, as it was when it ran,
// when it stands past the line of tokens added and only its pace keeps it from being stopped.
#[test]
fn a_replay_on_the_rounds_as_migrated_does_not_stop_the_longest_that_ran() {
    let dir = scratch("replay");
    let path = dir.join("db");
    before_the_ledger(&path, |conn| {
        conn.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])
            .unwrap();
        for work in [1, 38] {
            conn.execute(
                "INSERT INTO works (id, flow_version, graph, at) VALUES (?1, 1, 'engineer', 'I1')",
                [work],
            )
            .unwrap();
        }
        let put = |work: i64, n: i64, session: i64, usage: &str| {
            conn.execute(
                "INSERT INTO sessions (id, work, line, cli, cli_session, ended) VALUES (?1, ?2, 'engineer', 'c', ?3, 1)",
                params![session, work, format!("s{session}")],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO rounds (work, n, cell, session, input, model, config_commit, usage, started_at)
                 VALUES (?1, ?2, 'I1', ?3, '{\"message\":\"ab\"}', ?4, 'c', ?5, ?2)",
                params![work, n, session, OPUS, usage],
            )
            .unwrap();
        };
        for i in 1..=30 {
            put(1, i, i, &usage(100_000, 100_000 + i * 3000, 31, 1.0));
        }
        let (first, last, calls) = LONGEST;
        put(38, 22, 99, &usage(first, last, calls, 14.6));
    });
    let db = Db::open(&path).unwrap();
    let mut all = guard::samples(db.conn(), "engineer").unwrap();
    let longest = all.iter().find(|s| s.calls == 237).cloned();

    let judged = Lines::of(&all, "engineer", "I1", OPUS, 19);
    let added = judged.added.unwrap();
    let now = replayed(judged);

    all.retain(|s| s.calls != 237);
    let judged = Lines::of(&all, "engineer", "I1", OPUS, 19);
    let line_then = judged.added.unwrap().value;
    let then = replayed(judged);
    let _ = std::fs::remove_dir_all(&dir);

    let longest = longest.unwrap();
    assert_eq!((longest.added, longest.ledger), (276_892, false));
    assert_eq!((added.scope, added.samples), (Scope::Graph, 31));
    assert!(now.stop_record().is_none());
    assert_eq!(then.totals().added, 276_892);
    assert!(then.totals().added as f64 > line_then, "{line_then}");
    assert!(then.stop_record().is_none());
}
