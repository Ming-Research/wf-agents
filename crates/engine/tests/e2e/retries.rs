//! Bounds on what runs again: steps and rounds run again, and returns sent back, counted in a row.

use super::*;

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
    assert!(
        why.starts_with("3 returns in a row were sent back, the last at cell R1"),
        "{why}"
    );
    assert!(why.contains("outcome must be an object, and is a string"), "{why}");
    // The text the owner is paused with is the reason alone, without the problems the column keeps for counting.
    assert!(!why.contains("problems"), "{why}");
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

// A work that took as many steps in a row with no agent's round among them as the flow's case allows, going round cells
// that run no agent as work 234 of the trials went round M1, T6 and T7, takes the case at its next step: it pauses where
// it stands, saying at which cells the steps were, and the first time its project's architect is asked to look at why,
// by a matter to set straight. Let go on, it counts afresh, and the next time it is the owner's to look at, the
// architect asked no more. The owner's round asked of a work past the bound is not held to it. The architect works the
// matter would start are held, so that the test says when each step comes.
#[test]
fn a_work_past_the_bound_on_steps_without_an_agent_pauses_and_has_the_architect_look_first() {
    let s = setup("loop");
    let (mut e, said, item) = start(&s, scripts());
    e.tick().unwrap();
    let log = || said.lock().unwrap().join("\n");
    let (work, _, paused) = state(&e, item).unwrap_or_else(|| panic!("no work started:\n{}", log()));
    assert_eq!(paused, None, "{}", log());
    let most = {
        let f = e.flow(e.flow_now).unwrap();
        let rounds = f.graph(engine::ENGINEER).unwrap().rounds.clone().unwrap();
        rounds.engine.iter().find_map(|k| k.steps).unwrap()
    };
    // The steps of work 234's loop, each in a round no agent ran.
    let turn = |e: &mut Engine| {
        for i in 0..most as usize {
            e.db.write(Actor::Engine, |tx| {
                tx.execute(
                    "INSERT INTO rounds (work, n, cell, input, output, config_commit, started_at, ended_at)
                     VALUES (?1, ?2, ?3, '{}', '{}', 'c', ?4, ?4)",
                    params![work, next_round(tx, work)?, ["M1", "T6", "T7"][i % 3], db::now()],
                )?;
                Ok(())
            })
            .unwrap();
        }
    };
    let rounds = |e: &Engine| -> i64 {
        e.db.conn()
            .query_row("SELECT count(*) FROM rounds WHERE work = ?1", [work], |r| r.get(0))
            .unwrap()
    };
    let looks = |e: &Engine| -> Vec<String> {
        let mut st =
            e.db.conn()
                .prepare(
                    "SELECT what FROM architect_matters WHERE kind = 'set_straight'
                   AND json_extract(body, '$.work') = ?1 AND json_extract(body, '$.steps_without_agent') IS NOT NULL",
                )
                .unwrap();
        st.query_map([work], |r| r.get(0))
            .unwrap()
            .map(|x| x.unwrap())
            .collect()
    };
    // Lets the work go on, and takes its steps until one is recorded.
    let go_on = |e: &mut Engine| {
        api::resume(&mut e.db, work).unwrap();
        let before = rounds(e);
        for _ in 0..5 {
            if rounds(e) > before {
                break;
            }
            e.advance(work).unwrap();
        }
        assert!(rounds(e) > before, "{}", log());
    };
    // The owner's round asked of a work past the bound runs.
    turn(&mut e);
    let before = rounds(&e);
    e.step_work(work).unwrap();
    assert!(rounds(&e) > before, "{}", log());
    assert_eq!(paused_by(&e, work).as_deref(), Some("owner"), "{}", log());
    assert!(looks(&e).is_empty(), "{}", log());
    go_on(&mut e);
    // Past the bound the first time: the work pauses where it stands, and its architect is asked to look.
    turn(&mut e);
    let (_, at, _) = state(&e, item).unwrap();
    let before = rounds(&e);
    e.advance(work).unwrap();
    let took = format!(
        "it took {most} steps in a row with no agent's round among them, at cells M1, T6 and T7, and stands at cell \
         {at} (the flow's steps_without_agent)"
    );
    let then = format!("once it is let go on, it may take {most} such steps again");
    let (_, stays, paused) = state(&e, item).unwrap();
    assert_eq!(
        paused.as_deref(),
        Some(format!("{took}; its project's architect is asked to look at why, and {then}").as_str()),
        "{}",
        log()
    );
    assert_eq!((stays.as_str(), rounds(&e)), (at.as_str(), before), "{}", log());
    assert_eq!(paused_by(&e, work).as_deref(), Some("engine"), "{}", log());
    let paused = paused.unwrap();
    assert!(
        notices_of(&e, work).iter().any(|n| n.ends_with(&paused)),
        "{:?}",
        notices_of(&e, work)
    );
    assert_eq!(
        looks(&e),
        [format!(
            "Work {work} on item {item} (Add the feature) took {most} steps in a row with no agent's round among them, \
             at cells M1, T6 and T7, and waits paused at cell {at} for the owner's Start: why it took them, and what to \
             change"
        )],
        "{}",
        log()
    );
    // Let go on, it counts afresh; past the bound again, it is the owner's.
    go_on(&mut e);
    turn(&mut e);
    let (_, at, _) = state(&e, item).unwrap();
    e.advance(work).unwrap();
    let (_, _, paused) = state(&e, item).unwrap();
    let took = format!(
        "it took {most} steps in a row with no agent's round among them, at cells M1, T6 and T7, and stands at cell \
         {at} (the flow's steps_without_agent)"
    );
    assert_eq!(
        paused,
        Some(format!(
            "{took}; its project's architect was asked to look at why the time before, so it is the owner's to look at \
             now, and {then}"
        )),
        "{}",
        log()
    );
    assert_eq!(looks(&e).len(), 1, "{}", log());
    let _ = std::fs::remove_dir_all(&s.root);
}

// How many returns sent back pause a work is the number the flow the work runs on says. The return is sent back for not
// fitting the schema in two places, which `rounds.rejected` keeps as the text it is sent back with under `format` and as
// two problems of check `format`, each with the path of its field; the text the owner is paused with, and the one the
// agent is told, are that text alone, word for word as they were before the problems were kept.
#[test]
fn returns_sent_back_pause_the_work_after_as_many_as_its_flow_says() {
    let s = setup("pause");
    let mut scripts = scripts();
    scripts.insert(
        "R1",
        Box::new(|_: &Request, _: usize| {
            (
                Vec::new(),
                vec![("outcome", json!("not an object")), ("progress", json!(7))],
            )
        }),
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
            .is_some_and(|p| p.starts_with("2 returns in a row were sent back, the last at cell R1")),
        "{paused:?}\n{log}"
    );
    // Each return sent back is counted, and none is a retry.
    assert_eq!((counted(&e, work, "sent_back"), counted(&e, work, "retries")), (2, 0));
    // The column holds the text under `format`, then each place under `problems`, the check named `format`, no check of
    // the flow being named so; the text is the sentences of the places, each on a line, and the fields are the paths.
    let stored: String = e
        .db
        .conn()
        .query_row(
            "SELECT rejected FROM rounds WHERE work = ?1 AND cell = 'R1' AND rejected IS NOT NULL ORDER BY n LIMIT 1",
            [work],
            |r| r.get(0),
        )
        .unwrap();
    let rejected: Value = serde_json::from_str(&stored).unwrap();
    assert_eq!(
        rejected.as_object().unwrap().keys().collect::<Vec<_>>(),
        ["format", "problems"],
        "{stored}"
    );
    let problems = rejected["problems"].as_array().unwrap();
    assert_eq!(problems.len(), 2, "{stored}");
    assert!(problems.iter().all(|p| p["check"] == "format"), "{stored}");
    let mut fields: Vec<&str> = problems.iter().map(|p| p["field"].as_str().unwrap()).collect();
    fields.sort();
    assert_eq!(fields, ["outcome", "progress"], "{stored}");
    let text = rejected["format"].as_str().unwrap();
    assert_eq!(
        text,
        format!(
            "the return does not fit the schema of R1 in 2 places:\n{}\n{}",
            problems[0]["text"].as_str().unwrap(),
            problems[1]["text"].as_str().unwrap()
        ),
        "{stored}"
    );
    assert!(text.contains("outcome must be an object, and is a string"), "{text}");
    assert!(text.contains("progress must be a string, and is a number"), "{text}");
    // The work is paused with that text alone, in the form the column had before the problems were kept.
    assert_eq!(
        paused.as_deref(),
        Some(
            format!(
                "2 returns in a row were sent back, the last at cell R1: {}",
                json!({ "format": text })
            )
            .as_str()
        ),
        "{log}"
    );
    // After a return sent back the message only says why, and that the return is to be rewritten.
    let rounds = agent_rounds(&e, item);
    let (id, _, again) = &rounds[1];
    let m = again["message"].as_str().unwrap();
    assert_eq!(
        m,
        format!("# {id} · cell R1\n\nYour return was sent back: {text}\nRewrite it (format:R1).\n")
    );
    assert_eq!(again["conditional"], json!([]));
    let _ = std::fs::remove_dir_all(&s.root);
}

/// The architect's work at `U1`, with its project's remote at `url` from now on, and no wait for a step that failed to
/// be taken again.
fn architect_with_remote(s: &Setup, url: &str) -> (Engine, Arc<Mutex<Vec<String>>>, i64) {
    no_reply_wait(s);
    let (mut e, said) = engine(s, scripts());
    quiet(&mut e);
    e.backoff_unit = Duration::ZERO;
    let arch = architect_work_at(&mut e, "U1");
    git(&s.root.join("checkout"), &["remote", "set-url", "origin", url]);
    (e, said, arch)
}

/// A remote that refuses every connection, as a network that fails in passing does.
const REFUSED: &str = "http://127.0.0.1:1/p.git";

/// Limits the project sets on counters of the flow, committed to the configuration.
fn limit_on(s: &Setup, limits: &[(&str, u64)]) {
    let project = std::fs::read_to_string(s.data.join("projects/p.toml")).unwrap();
    let limits: String = limits.iter().map(|(c, n)| format!("{c} = {n}\n")).collect();
    write(&s.data, "projects/p.toml", &format!("{project}\n[limits]\n{limits}"));
    git(&s.data, &["commit", "-q", "-am", "limits on counters"]);
}

// The rows the engine itself takes for a round or a step that failed in passing count `retries`, and a return sent back
// counts `sent_back`: a project's limit on either is told to the owner once, as the count passes it, and the work goes
// on. Here a failed step of the architect's work passes a limit of 1 on its second time.
#[test]
fn a_step_taken_again_past_its_projects_limit_tells_the_owner_once() {
    let s = setup("limit-step");
    limit_on(&s, &[("retries", 1)]);
    let (mut e, said, arch) = architect_with_remote(&s, REFUSED);
    for _ in 0..3 {
        e.advance(arch).unwrap();
    }
    let log = said.lock().unwrap().join("\n");
    assert_eq!(counted(&e, arch, "retries"), 3, "{log}");
    assert_eq!(paused_by(&e, arch), None, "{log}");
    assert_eq!(told(&s, "passed the limit its project sets on retries"), 1, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The same for the agent's rounds: two runs that end in passing failures pass a limit of 1 on `retries` at the second,
// and two returns sent back pass a limit of 1 on `sent_back` at the second; each is told once.
#[test]
fn a_round_run_again_or_sent_back_past_its_projects_limit_tells_the_owner_once() {
    let s = setup("limit-round");
    limit_on(&s, &[("retries", 1), ("sent_back", 1)]);
    let mut scripts = scripts();
    scripts.insert(
        "R1",
        Box::new(|_: &Request, n: usize| match n {
            1 | 2 => (Vec::new(), vec![("@error", json!("API Error: 529 Overloaded"))]),
            3 | 4 => (Vec::new(), vec![("outcome", json!("not an object"))]),
            _ => (Vec::new(), Vec::new()),
        }),
    );
    let (mut e, said, item) = start(&s, scripts);
    for _ in 0..12 {
        e.tick().unwrap();
        let work = state(&e, item).unwrap().0;
        if counted(&e, work, "retries") >= 2 && counted(&e, work, "sent_back") >= 2 {
            break;
        }
    }
    let log = said.lock().unwrap().join("\n");
    let work = state(&e, item).unwrap().0;
    assert_eq!(
        (counted(&e, work, "retries"), counted(&e, work, "sent_back")),
        (2, 2),
        "{log}"
    );
    assert_eq!(told(&s, "passed the limit its project sets on retries"), 1, "{log}");
    assert_eq!(told(&s, "passed the limit its project sets on sent_back"), 1, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A step of the engine's before the architect's round that fails the way the network does by itself does not pause the
// work: the failure is recorded on the cell with its text, counted as a retry, and the step is taken again, after which
// the round runs as if nothing had happened, and is told nothing of the failed step.
#[test]
fn a_step_that_fails_like_the_network_is_taken_again() {
    let s = setup("step-network");
    let (mut e, said, arch) = architect_with_remote(&s, REFUSED);
    e.advance(arch).unwrap();
    let log = said.lock().unwrap().join("\n");
    // First, that the work did not pause: this is what the code before the change fails, where there is no view
    // `round_rows` to read the rows from yet.
    assert_eq!(paused_by(&e, arch), None, "{log}");
    assert_eq!(
        rows_taken(&e, arch),
        [("U1".to_string(), Some("step_network".to_string()))],
        "{log}"
    );
    assert_eq!(counted(&e, arch, "retries"), 1, "{log}");
    let (input, rejected): (String, String) =
        e.db.conn()
            .query_row(
                "SELECT input, rejected FROM rounds WHERE work = ?1 AND n = 1",
                [arch],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
    assert!(input.contains("127.0.0.1"), "{input}");
    assert!(
        rejected.contains("step_network") && !rejected.contains("over"),
        "{rejected}"
    );
    // The remote answers again: the step is taken again and the round runs.
    git(
        &s.root.join("checkout"),
        &["remote", "set-url", "origin", &s.origin.to_string_lossy()],
    );
    for _ in 0..3 {
        e.advance(arch).unwrap();
    }
    let log = said.lock().unwrap().join("\n");
    let rows = rows_taken(&e, arch);
    assert_eq!(rows[0], ("U1".to_string(), Some("step_network".to_string())), "{log}");
    assert!(
        rows.len() > 1 && rows[1].1.as_deref() != Some("step_network"),
        "{rows:?}\n{log}"
    );
    assert_eq!(paused_by(&e, arch), None, "{log}");
    assert_eq!(counted(&e, arch, "retries"), 1, "{log}");
    // The round that ran is the cell's last; its message says nothing of the step that failed before it.
    let message = message_at(&e, arch, "U1", true);
    assert!(
        message.contains("cell U1") && !message.contains("step_network"),
        "{message}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// A step that fails for any cause but the network's is not taken again: the work pauses at once, with the reason, and
// no round records a retry.
#[test]
fn a_step_that_fails_for_another_cause_pauses_the_work_at_once() {
    let s = setup("step-other");
    let (mut e, said, arch) = architect_with_remote(&s, "/nonexistent/p.git");
    e.advance(arch).unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(paused_by(&e, arch).as_deref(), Some("engine"), "{log}");
    let why: String =
        e.db.conn()
            .query_row("SELECT paused FROM works WHERE id = ?1", [arch], |r| r.get(0))
            .unwrap();
    assert!(why.contains("could not be brought to main"), "{why}");
    assert!(rows_taken(&e, arch).is_empty(), "{log}");
    assert_eq!(counted(&e, arch, "retries"), 0, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A step that keeps failing like the network pauses the work and tells the owner once the flow's bound of failures in a
// row is passed, the architect's own work as much as any; the owner's Start counts afresh.
#[test]
fn a_step_that_keeps_failing_like_the_network_pauses_the_work_past_its_bound() {
    let s = setup("step-bound");
    let (mut e, said, arch) = architect_with_remote(&s, REFUSED);
    let mut advances = 0;
    for _ in 0..12 {
        e.advance(arch).unwrap();
        advances += 1;
        if paused_by(&e, arch).is_some() {
            break;
        }
    }
    let log = said.lock().unwrap().join("\n");
    // Five failures are taken again and the sixth pauses; the code before the change pauses at the first. This comes
    // before any read of the view `round_rows`, which the code before the change does not have.
    assert_eq!(advances, 6, "{log}");
    let rows: Vec<Option<String>> = rows_taken(&e, arch).into_iter().map(|r| r.1).collect();
    let mut want = vec![Some("step_network".to_string()); 5];
    want.push(Some("step_network_too_often".to_string()));
    assert_eq!(rows, want, "{log}");
    assert_eq!(paused_by(&e, arch).as_deref(), Some("engine"), "{log}");
    assert_eq!(counted(&e, arch, "retries"), 5, "{log}");
    assert!(told(&s, "6 times in a row") > 0, "{log}");
    // Once the owner lets the work go on, it counts afresh: the next failure is retried, not paused.
    api::resume(&mut e.db, arch).unwrap();
    e.advance(arch).unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(paused_by(&e, arch), None, "{log}");
    assert_eq!(
        rows_taken(&e, arch).last().and_then(|r| r.1.clone()).as_deref(),
        Some("step_network"),
        "{log}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// A passing failure of the agent's run is run again only up to the flow's bound of failures in a row: past it, the work
// pauses and the owner is told. Each run again is counted as a retry, and the run past the bound as none.
#[test]
fn a_passing_failure_pauses_the_work_past_its_bound() {
    let (s, mut e, said, item) = failing("transient-bound", "API Error: 529 Overloaded".to_string(), 100);
    for _ in 0..30 {
        e.tick().unwrap();
        if state(&e, item).unwrap().2.is_some() {
            break;
        }
    }
    let log = said.lock().unwrap().join("\n");
    let (work, at, paused) = state(&e, item).unwrap();
    assert_eq!(at, "R0", "{log}");
    assert!(
        paused
            .as_deref()
            .is_some_and(|p| p.contains("once more after 8 times in a row")),
        "{paused:?}\n{log}"
    );
    assert_eq!(failed_kinds(&e), vec!["transient"; 9], "{log}");
    assert_eq!(counted(&e, work, "retries"), 8, "{log}");
    assert_eq!(
        rows_taken(&e, work).last().and_then(|r| r.1.clone()).as_deref(),
        Some("transient_too_often"),
        "{log}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// Failures are in a row only until a row of an agent is taken: eight failures, a return, and one more failure are not
// nine in a row, and the ninth is not past the bound.
#[test]
fn failures_with_a_return_between_are_not_in_a_row() {
    let s = setup("far-apart");
    no_reply_wait(&s);
    let mut scripts = scripts();
    scripts.insert(
        "U1",
        Box::new(|_: &Request, _: usize| (Vec::new(), vec![("@error", json!("API Error: 529 Overloaded"))])),
    );
    let (mut e, said) = engine(&s, scripts);
    quiet(&mut e);
    let arch = architect_work_at(&mut e, "U1");
    // Eight failures, as many as the flow runs again, and a return of an agent after them.
    let transient = Some(json!({ "failed": { "kind": "transient", "what": "529 Overloaded" } }));
    let mut past: Vec<(&str, Option<&str>, Option<Value>)> = (0..8).map(|_| ("U1", None, transient.clone())).collect();
    past.push(("H9", Some("done"), None));
    past_rounds(&mut e, arch, &past);
    // The next failure is the first in its row: it is run again, not paused (it would be past the bound if the eight
    // before the return were counted).
    e.advance(arch).unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(paused_by(&e, arch), None, "{log}");
    assert_eq!(
        rows_after(&e, arch, past.len()),
        [Some("transient".to_string())],
        "{log}"
    );
    assert_eq!(counted(&e, arch, "retries"), 9, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

/// Rounds already in a work's history, oldest first, each as `(cell, row taken, rejected)`; they began an hour before
/// now, so that an owner's Start after them ends the row they are in.
fn past_rounds(e: &mut Engine, work: i64, rounds: &[(&str, Option<&str>, Option<Value>)]) {
    let first = db::now() - 3_600_000;
    e.db.write(Actor::Engine, |tx| {
        for (i, (cell, row, rejected)) in rounds.iter().enumerate() {
            let at = first + i as i64;
            tx.execute(
                "INSERT INTO rounds (work, n, cell, input, output, transition, rejected, config_commit, started_at, ended_at)
                 VALUES (?1, ?2, ?3, '{}', ?4, ?5, ?6, 'c', ?7, ?7)",
                params![
                    work,
                    i as i64 + 1,
                    cell,
                    row.map(|_| "{}"),
                    row,
                    rejected.as_ref().map(Value::to_string),
                    at
                ],
            )?;
            // The counter of the row the round took, as the engine keeps it on a round it ended itself: read from the
            // flow the work is on, by the row the view `round_rows` says the round took.
            tx.execute(
                "UPDATE rounds SET counts = (
                   SELECT json_extract(row.value, '$.counts')
                   FROM round_rows x
                   JOIN works w ON w.id = x.work
                   JOIN flow_versions v ON v.id = w.flow_version
                   JOIN json_each(v.compiled, '$.graphs') g ON json_extract(g.value, '$.name') = w.graph
                   JOIN json_each(g.value, '$.cells') c ON json_extract(c.value, '$.id') = x.cell
                   JOIN json_each(c.value, '$.rows') row ON json_extract(row.value, '$.name') = x.took
                   WHERE x.work = ?1 AND x.round = ?2)
                 WHERE work = ?1 AND n = ?2",
                params![work, i as i64 + 1],
            )?;
        }
        Ok(())
    })
    .unwrap();
}

/// The rejection a round has that a step of the engine's before it failed like the network.
fn step_failed() -> Option<Value> {
    Some(json!({ "failed": { "kind": "step_network", "what": "channel 0: open failed: administratively prohibited" } }))
}

/// The rows a work took after its first `skip` rounds.
fn rows_after(e: &Engine, work: i64, skip: usize) -> Vec<Option<String>> {
    rows_taken(e, work).into_iter().skip(skip).map(|r| r.1).collect()
}

/// Advances a work until it is paused, at most `most` times.
fn advance_to_pause(e: &mut Engine, work: i64, most: usize) {
    for _ in 0..most {
        e.advance(work).unwrap();
        if paused_by(e, work).is_some() {
            break;
        }
    }
}

// Five failures of the step and then a return of an agent: the return is a row from outside the engine, so the
// failures after it are counted afresh, and all five of them are run again before the sixth pauses (a count that kept
// the five before the return would pause at the first).
#[test]
fn step_failures_before_a_return_of_an_agent_are_not_in_a_row_with_those_after_it() {
    let s = setup("step-reset");
    let (mut e, said, arch) = architect_with_remote(&s, REFUSED);
    let mut past: Vec<(&str, Option<&str>, Option<Value>)> = (0..5).map(|_| ("U1", None, step_failed())).collect();
    past.push(("H9", Some("done"), None));
    past_rounds(&mut e, arch, &past);
    advance_to_pause(&mut e, arch, 12);
    let log = said.lock().unwrap().join("\n");
    let mut want = vec![Some("step_network".to_string()); 5];
    want.push(Some("step_network_too_often".to_string()));
    assert_eq!(rows_after(&e, arch, 6), want, "{log}");
    assert_eq!(paused_by(&e, arch).as_deref(), Some("engine"), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// Rows of the engine's between two failures of the step neither count nor start the count afresh: a failed run, a
// round cut off and the engine's own cell between five failures leave the sixth past the bound.
#[test]
fn step_failures_with_only_rows_of_the_engine_between_are_in_a_row() {
    let s = setup("step-engine-rows");
    let (mut e, said, arch) = architect_with_remote(&s, REFUSED);
    let transient = Some(json!({ "failed": { "kind": "transient", "what": "529 Overloaded" } }));
    let cut_off = Some(json!({ "interrupted": "the engine stopped before this round ended; it runs again" }));
    let past: Vec<(&str, Option<&str>, Option<Value>)> = vec![
        ("U1", None, step_failed()),
        ("U1", None, step_failed()),
        ("U1", None, step_failed()),
        ("Q0", Some("waits"), None),
        ("U1", None, transient),
        ("U1", None, cut_off),
        ("U1", None, step_failed()),
        ("U1", None, step_failed()),
    ];
    past_rounds(&mut e, arch, &past);
    advance_to_pause(&mut e, arch, 12);
    let log = said.lock().unwrap().join("\n");
    assert_eq!(
        rows_after(&e, arch, past.len()),
        [Some("step_network_too_often".to_string())],
        "{log}"
    );
    assert_eq!(paused_by(&e, arch).as_deref(), Some("engine"), "{log}");
    assert!(told(&s, "6 times in a row") > 0, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A record of a step that failed before the cell's round is no run of the cell: after a run the service refused and a
// step that failed like the network, a run refused again pauses the work at once, since the run before the step's
// record was refused too. (A reading that took the step's record for the round before would find the first refusal
// of its kind and not pause the work yet.)
#[test]
fn a_run_refused_after_a_step_that_failed_pauses_the_work_at_once() {
    let s = setup("refused-at-once");
    no_reply_wait(&s);
    let mut scripts = scripts();
    scripts.insert(
        "U1",
        Box::new(|_: &Request, _: usize| {
            let flagged = "API Error: Opus 5.5's safeguards flagged this message (https://www.anthropic.com/legal/aup)";
            (Vec::new(), vec![("@error", json!(flagged))])
        }),
    );
    let (mut e, said) = engine(&s, scripts);
    quiet(&mut e);
    let arch = architect_work_at(&mut e, "U1");
    let flagged = Some(json!({ "failed": { "kind": "flagged", "what": "flagged" } }));
    past_rounds(&mut e, arch, &[("U1", None, flagged), ("U1", None, step_failed())]);
    e.advance(arch).unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(paused_by(&e, arch).as_deref(), Some("engine"), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A step that failed waits as long as the flow's backoff says for the time it failed (5, 15, then 45 of its seconds, a
// second here being an hour), and not for a shorter or a longer one: the round that runs it again is not begun before
// the wait is over, and is begun once it is.
#[test]
fn a_step_that_failed_is_taken_again_only_after_the_waits_the_flow_gives() {
    let s = setup("step-backoff");
    let (mut e, said, arch) = architect_with_remote(&s, REFUSED);
    e.backoff_unit = Duration::from_secs(3600);
    let hour = 3_600_000;
    // The round that failed ended `ago` ago.
    let ended_ago = |e: &Engine, round: i64, ago: i64| {
        e.db.conn()
            .execute(
                "UPDATE rounds SET ended_at = ?3 WHERE work = ?1 AND n = ?2",
                params![arch, round, db::now() - ago],
            )
            .unwrap();
    };
    for (round, hours) in [(1_i64, 5_i64), (2, 15), (3, 45)] {
        e.advance(arch).unwrap();
        let log = said.lock().unwrap().join("\n");
        assert_eq!(rows_taken(&e, arch).len() as i64, round, "{log}");
        // A minute short of the wait: nothing runs.
        ended_ago(&e, round, hours * hour - 60_000);
        e.advance(arch).unwrap();
        e.advance(arch).unwrap();
        assert_eq!(rows_taken(&e, arch).len() as i64, round, "{round}: {log}");
        assert_eq!(paused_by(&e, arch), None, "{log}");
        // A minute past it: the step is taken again, and fails again.
        ended_ago(&e, round, hours * hour + 60_000);
    }
    e.advance(arch).unwrap();
    assert_eq!(rows_taken(&e, arch).len(), 4, "{}", said.lock().unwrap().join("\n"));
    let _ = std::fs::remove_dir_all(&s.root);
}

// The owner's Start counts the failed runs of an agent afresh as it does the steps': once the work paused past the
// bound is let go on, the next failure is run again, not paused.
#[test]
fn a_passing_failure_after_the_owner_s_start_is_run_again() {
    let (s, mut e, said, item) = failing("transient-start", "API Error: 529 Overloaded".to_string(), 100);
    for _ in 0..30 {
        e.tick().unwrap();
        if state(&e, item).unwrap().2.is_some() {
            break;
        }
    }
    let (work, _, paused) = state(&e, item).unwrap();
    assert!(paused.is_some(), "{}", said.lock().unwrap().join("\n"));
    let before = rows_taken(&e, work).len();
    api::resume(&mut e.db, work).unwrap();
    for _ in 0..2 {
        e.tick().unwrap();
    }
    let log = said.lock().unwrap().join("\n");
    assert_eq!(state(&e, item).unwrap().2, None, "{log}");
    let rows = rows_after(&e, work, before);
    assert_eq!(rows.first(), Some(&Some("transient".to_string())), "{rows:?}\n{log}");
    assert!(
        rows.iter().all(|r| r.as_deref() == Some("transient")),
        "{rows:?}\n{log}"
    );
    assert_eq!(counted(&e, work, "retries"), 8 + rows.len() as i64, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// Returns sent back pause the work after the flow's own number, 3, and each is counted.
#[test]
fn three_returns_sent_back_in_a_row_pause_the_work() {
    let s = setup("sent-back-three");
    let mut scripts = scripts();
    scripts.insert(
        "R1",
        Box::new(|_: &Request, _: usize| (Vec::new(), vec![("outcome", json!("not an object"))])),
    );
    let (mut e, said, item) = start(&s, scripts);
    for _ in 0..12 {
        e.tick().unwrap();
        if state(&e, item).unwrap().2.is_some() {
            break;
        }
    }
    let log = said.lock().unwrap().join("\n");
    let (work, at, paused) = state(&e, item).unwrap();
    assert_eq!(at, "R1", "{log}");
    assert!(
        paused
            .as_deref()
            .is_some_and(|p| p.starts_with("3 returns in a row were sent back, the last at cell R1")),
        "{paused:?}\n{log}"
    );
    assert_eq!(sent_back(&e), 3, "{log}");
    assert_eq!(
        (counted(&e, work, "sent_back"), counted(&e, work, "retries")),
        (3, 0),
        "{log}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// A return of an agent taken between two cells' returns sent back starts the count afresh: two sent back at one cell,
// its return taken, and two at the next are four, and pause nothing (they would at three).
#[test]
fn returns_sent_back_either_side_of_a_return_taken_are_not_in_a_row() {
    let s = setup("sent-back-apart");
    let mut scripts = scripts();
    for cell in ["R1", "R2"] {
        scripts.insert(
            cell,
            Box::new(|_: &Request, n: usize| {
                let bad = vec![("outcome", json!("not an object"))];
                (Vec::new(), if n <= 2 { bad } else { Vec::new() })
            }),
        );
    }
    let (mut e, said, item) = start(&s, scripts);
    for _ in 0..20 {
        e.tick().unwrap();
        let (_, at, paused) = state(&e, item).unwrap();
        if paused.is_some() || !matches!(at.as_str(), "R1" | "R2") {
            break;
        }
    }
    let log = said.lock().unwrap().join("\n");
    let (work, at, paused) = state(&e, item).unwrap();
    assert_eq!(paused, None, "{log}");
    assert!(!matches!(at.as_str(), "R1" | "R2"), "{at}\n{log}");
    assert_eq!(sent_back(&e), 4, "{log}");
    assert_eq!(
        (counted(&e, work, "sent_back"), counted(&e, work, "retries")),
        (4, 0),
        "{log}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// `retries` is one counter for a step that failed and a run that failed: with a limit of 1, one step taken again and
// then one run taken again pass it at the run, which is told once, and the next run taken again is told no more.
#[test]
fn a_limit_on_retries_is_told_once_whichever_row_takes_the_count_past_it() {
    let s = setup("limit-mixed");
    limit_on(&s, &[("retries", 1)]);
    no_reply_wait(&s);
    let mut scripts = scripts();
    scripts.insert(
        "U1",
        Box::new(|_: &Request, n: usize| {
            let fail = vec![("@error", json!("API Error: 529 Overloaded"))];
            (Vec::new(), if n <= 2 { fail } else { Vec::new() })
        }),
    );
    let (mut e, said) = engine(&s, scripts);
    quiet(&mut e);
    let arch = architect_work_at(&mut e, "U1");
    // One step taken again already: the count is at the limit, not past it.
    past_rounds(&mut e, arch, &[("U1", None, step_failed())]);
    let phrase = "passed the limit its project sets on retries";
    for _ in 0..6 {
        e.advance(arch).unwrap();
        if counted(&e, arch, "retries") >= 3 {
            break;
        }
    }
    let log = said.lock().unwrap().join("\n");
    assert_eq!(counted(&e, arch, "retries"), 3, "{log}");
    assert_eq!(paused_by(&e, arch), None, "{log}");
    assert_eq!(told(&s, phrase), 1, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}
