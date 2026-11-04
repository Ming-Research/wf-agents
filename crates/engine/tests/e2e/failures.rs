//! Runs that fail: a repeat in a session, usage limits, the network, refused runs, a passing failure, an expired login
//! and a prompt too long.

use super::*;

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
    // Each run again is a retry; none is a return sent back.
    assert_eq!((counted(&e, work, "retries"), counted(&e, work, "sent_back")), (3, 0));
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
