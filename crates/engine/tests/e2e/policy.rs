//! The project's policy: its checks, the writing of a new one, and the signals it is given.

use super::*;

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

/// Project p, whose constitution is `now`, after a check of its policy that the owner's words asked for, handled where
/// the policy it set took effect, in a work of its architect over before the test, which kept the hash of `kept` as
/// the constitution it was queued under; when `newer`, another policy took effect after it. A new session then
/// understands the project. Gives the matters at the head of the queue once it has.
fn head_after_a_new_session(name: &str, kept: &str, now: &str, newer: bool) -> Vec<AtHead> {
    let s = setup(name);
    names_a_constitution(&s);
    // The constitution is on main only: no file of the checkout holds it, and the checkout has not fetched main since.
    main_moves_on(&s.root, &[("CONSTITUTION.md", now)], "The constitution");
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
