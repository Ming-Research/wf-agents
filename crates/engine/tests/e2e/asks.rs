//! Asks of the owner and the owner's words: where an answer goes back to, and what waits for it.

use super::*;

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
        relayed: None,
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
