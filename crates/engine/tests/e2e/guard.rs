//! The run guard: a round judged against the lines of the rounds before it, stopped, and run again.

use super::*;

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

// A cancel asked while a round runs that the run guard then stops: the cancel is looked at first, so the cell does not
// run again in a new session, nothing counts as a failed run, and the loop ends the work.
#[test]
fn a_round_the_run_guard_stops_under_a_cancel_is_not_run_again() {
    let s = setup("guard-under-cancel");
    let hold = Gate::default();
    let h = hold.clone();
    let mut agents = scripts();
    agents.insert(
        "R1",
        Box::new(move |_: &Request, _: usize| {
            h.pass();
            (Vec::new(), vec![("@stream", calls_of(&[10_000, 30_000]))])
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    history(&mut e, "R1", 19, &usual);
    hold.shut();
    let came = turns_until(&mut e, &Go::default(), |_| hold.came() > 0);
    let log = said.lock().unwrap().join("\n");
    assert!(came, "{log}");
    let work = state(&e, item).unwrap().0;
    cancel(&mut e, item, "owner");
    hold.open();
    let ended = turns_until(&mut e, &Go::default(), |e| ended_why(e, work).is_some());
    e.drain().unwrap();
    let log = said.lock().unwrap().join("\n");
    assert!(ended, "{log}");
    assert_eq!(
        guard_events(&e),
        [("growth".to_string(), "stopped".to_string())],
        "{log}"
    );
    assert_eq!(hold.came(), 1, "{log}");
    assert_eq!(failed_kinds(&e), Vec::<String>::new(), "{log}");
    assert_eq!(paused_by(&e, work), None, "{log}");
    assert_eq!(cancelled_why(&e, item).as_deref(), Some("no longer needed"), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
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
