//! The sessions a line runs in: new ones, kept warm, compacted, and what reaches them during a round.

use super::*;

// When the engine stopped in the middle of a round, the same work goes on in the same session: the round is cut off,
// and the next one is only told so and where to look.
#[test]
fn a_round_cut_off_by_the_engine_goes_on_in_the_same_session() {
    let s = setup("cut");
    let (mut e, _, item) = start(&s, scripts());
    step_to(&mut e, item, "I1");
    let (work, _, _) = state(&e, item).unwrap();
    let session: i64 =
        e.db.conn()
            .query_row(
                "SELECT id FROM sessions WHERE work = ?1 AND line = 'engineer'",
                [work],
                |r| r.get(0),
            )
            .unwrap();
    // The engine stops while cell `I1` runs.
    let cut = e
        .db
        .write(Actor::Engine, |tx| {
            let n = next_round(tx, work)?;
            tx.execute(
                "INSERT INTO rounds (work, n, cell, session, input, config_commit, started_at) VALUES (?1, ?2, 'I1', ?3, '{}', 'c', 1)",
                params![work, n, session],
            )?;
            Ok(n)
        })
        .unwrap();
    e.step(None).unwrap();
    let rounds = agent_rounds(&e, item);
    let (id, cell, input) = rounds.last().unwrap();
    assert_eq!(cell, "I1");
    let m = input["message"].as_str().unwrap();
    assert!(
        m.starts_with(&format!(
            "# {id} · cell I1\n\nR{work}-{cut} was cut off midway. First look at the worktree"
        )),
        "{m}"
    );
    assert_eq!(input["conditional"], json!([]));
    assert!(input["call"]["session"]["resumed"].is_string(), "{input}");
    let _ = std::fs::remove_dir_all(&s.root);
}

/// What a keep-warm ping was given: the command line it ran, and its message.
type Replayed = (Vec<String>, String);

// A line's session whose context is past the configuration's limit is replaced at the line's next round by a new one,
// which is given everything its cell needs; the session before ends. While the work waits before that round, the
// session is not kept warm: the next round starts a new one anyway.
#[test]
fn a_session_past_the_context_limit_is_replaced_at_the_lines_next_round() {
    let s = setup_priced("limit");
    let mut scripts = scripts();
    scripts.insert("R1", adding("R1", |_| vec![("@usage", used(1.0, 499_997))]));
    let (mut e, said, item) = start(&s, scripts);
    agent_rounds_to(&mut e, item, 1);
    let (work, at, _) = state(&e, item).unwrap();
    assert_eq!(at, "I1", "{}", said.lock().unwrap().join("\n"));
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
            ("line".to_string(), true, "R1".to_string()),
            ("limit".to_string(), false, "I1".to_string())
        ],
        "{log}"
    );
    let rounds = agent_rounds(&e, item);
    let two = &rounds[1].2;
    assert_eq!(given(two, "format:I1"), Some(true));
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
    scripts.insert("R1", adding("R1", |_| vec![("@usage", used(1.5, 9_500))]));
    scripts.insert("I1", adding("I1", |_| vec![("@usage", used(3.0, 10_000))]));
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
    agent_rounds_to(&mut e, item, 1);
    let (work, at, _) = state(&e, item).unwrap();
    assert_eq!(at, "I1", "{}", said.lock().unwrap().join("\n"));
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
        assert_eq!(*waited, json!({ "cell": "I1", "on": "pause", "what": "to look at it" }));
    }
    // Each ping cost what the session grew by since the call before it.
    let costs: Vec<f64> = pings
        .iter()
        .map(|(_, u)| u.as_ref().unwrap()["cost_usd"].as_f64().unwrap())
        .collect();
    assert!(costs.iter().all(|c| (c - 0.02).abs() < 1e-9), "{costs:?}");
    // The ping ran the last round's command line, resuming its session, with only the message changed: that round, the
    // research's, started the session the ping resumes.
    let rounds = agent_rounds(&e, item);
    let recorded: Vec<String> = rounds[0].2["call"]["command"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| match a.as_str().unwrap() {
            "--session-id" => "--resume".to_string(),
            a => a.to_string(),
        })
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
                "SELECT json_extract(usage, '$.cost_usd') FROM rounds WHERE work = ?1 AND cell = 'I1'",
                [work],
                |r| r.get(0),
            )
            .unwrap();
    assert!((cost - (3.0 - 1.5 - 0.02 * 39.0)).abs() < 1e-9, "{cost}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// While a work goes on, each line's session is kept warm as long as the flow can come back to it: while the work stands
// at fixing, the engineer's, which the fix goes on in, the review's, which re-checks after it, and the reader's, which a
// look-up the fix may ask for goes to, are pinged for the work going on; standing at the merge gate a moment later,
// nothing is pinged again.
#[test]
fn the_lines_a_work_can_come_back_to_are_kept_warm_while_it_goes_on() {
    let s = setup_priced("warm-lines");
    let (mut e, said, item) = start(&s, scripts());
    step_to(&mut e, item, "T3");
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
    assert_eq!(warm_now(&mut e, now, None).unwrap(), 3, "{log}");
    let going_on = json!({ "cell": "T6", "on": "work", "what": "the work goes on" });
    let mut pings = pinged(&e);
    pings.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(
        pings,
        [
            ("engineer".to_string(), going_on.clone()),
            ("librarian".to_string(), going_on.clone()),
            ("review".to_string(), going_on)
        ]
    );
    move_to(&mut e, work, "M1");
    assert_eq!(warm_now(&mut e, now + 2_000, None).unwrap(), 0, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// No ping goes beside a round of the session that runs.
#[test]
fn no_ping_beside_a_round_of_the_session() {
    let s = setup_priced("no-warm");
    let mut scripts = scripts();
    scripts.insert("R1", adding("R1", |_| vec![("@usage", used(1.0, 9_000))]));
    let (mut e, said, item) = start(&s, scripts);
    step_to(&mut e, item, "I1");
    let (work, _, _) = state(&e, item).unwrap();
    pause_by_owner(&mut e, work, "to look at it");
    let from = last_call(&e, work);
    let line_pings = |e: &Engine| {
        number(
            e,
            &format!(
                "SELECT count(*) FROM pings p JOIN sessions s ON s.id = p.session WHERE s.work = {work} AND s.line = 'engineer'"
            ),
        )
    };
    // At a cell that goes on in the session, a round of the session that runs keeps the ping off.
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
                 VALUES (?1, ?2, 'I1', ?3, '{}', 'c', ?4)",
                params![work, n, session, from],
            )?;
            Ok(n)
        })
        .unwrap();
    warm_now(&mut e, from + 55 * 60_000 + 1_000, None).unwrap();
    assert_eq!(line_pings(&e), 0);
    e.db.write(Actor::Engine, |tx| {
        tx.execute("DELETE FROM rounds WHERE work = ?1 AND n = ?2", [work, open])?;
        Ok(())
    })
    .unwrap();
    warm_now(&mut e, from + 55 * 60_000 + 1_000, None).unwrap();
    assert_eq!(line_pings(&e), 1, "{}", said.lock().unwrap().join("\n"));
    let _ = std::fs::remove_dir_all(&s.root);
}

// A work waiting for the architect's reply has the session of the line that asked kept warm, and the ping says the work
// waited at the architect's cell for the architect's reply.
#[test]
fn a_ping_says_where_the_work_waited_and_for_what() {
    let s = setup_priced("warm-architect");
    let mut scripts = scripts();
    scripts.insert("R1", adding("R1", |_| vec![("@usage", used(1.0, 9_000))]));
    let (mut e, said, item) = start(&s, scripts);
    agent_rounds_to(&mut e, item, 1);
    let (work, at, _) = state(&e, item).unwrap();
    assert_eq!(at, "I1", "{}", said.lock().unwrap().join("\n"));
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "UPDATE works SET at = 'A2', resume_at = 'I1', waiting = '[\"I1\"]' WHERE id = ?1",
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
        "I1",
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
    assert_eq!(cell, "I1", "{}", said.lock().unwrap().join("\n"));
    let (file, text) = read.lock().unwrap().clone().expect("the file the hook gives again");
    // The session holds the project guide, so the message only refers to it; the file gives it whole, and none of
    // what the session's system prompt holds, which a compaction leaves.
    let guide = |i: &Value| guide_in(i).map(|g| g.0).unwrap();
    assert_eq!(guide_in(input).map(|g| g.1), Some(false));
    assert!(
        text.starts_with(&format!("Your session was compacted in the middle of {id}"))
            && text.contains(&format!("# {id} · cell I1"))
            && text.contains(&format!("### {}", guide(input)))
            && text.contains("### format:I1")
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
        "I1",
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
    assert_eq!(cell, "I1", "{log}");
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

// What the engineer is given of an interjection depends on whether it needs an answer (`informs`), in the round's
// message and in the hook alike: a word that needs none is told as such, apart from what may need one. What the message
// gave the hook does not give again; what comes while the round runs it does give. The round's record lists all four,
// and each is recorded as delivered in that round.
#[test]
fn an_interjection_is_given_once_and_asks_for_an_answer_only_where_one_may_be_needed() {
    let s = setup("informs");
    let mut scripts = scripts();
    let data = s.data.clone();
    let research = scripts.remove("R1").unwrap();
    // Said while the research runs: the line's next round, at I1, is given both by its message.
    scripts.insert(
        "R1",
        Box::new(move |r: &Request, n: usize| {
            let mut db = engine::database::open(&data).unwrap();
            db.write(Actor::Owner, |tx| {
                tx.execute(
                    "INSERT INTO interjections (work, line, speaker, text, informs)
                     VALUES (?1, 'engineer', 'owner', 'Why this size?', 0)",
                    [r.work],
                )?;
                Ok(())
            })
            .unwrap();
            db.write(Actor::Architect, |tx| {
                tx.execute(
                    "INSERT INTO interjections (work, line, speaker, text, informs)
                     VALUES (?1, 'engineer', 'architect', 'Your question became an item.', 1)",
                    [r.work],
                )?;
                Ok(())
            })
            .unwrap();
            research(r, n)
        }),
    );
    let sent: Sent = Arc::default();
    let seen = sent.clone();
    let hooked: Arc<Mutex<Vec<Option<String>>>> = Arc::default();
    let hooks = hooked.clone();
    let data = s.data.clone();
    scripts.insert(
        "I1",
        Box::new(move |r: &Request, _: usize| {
            keep_sent(&seen, r);
            // The hook after a tool use, first with nothing new and then with a word of each kind that came since.
            let mut db = engine::database::open(&data).unwrap();
            let n: i64 = db
                .conn()
                .query_row("SELECT max(n) FROM rounds WHERE work = ?1", [r.work], |x| x.get(0))
                .unwrap();
            let hook = |db: &mut db::Db| {
                let out = sessions::interject::deliver(db, r.work, n, "engineer").unwrap();
                hooks.lock().unwrap().push(out);
            };
            hook(&mut db);
            db.write(Actor::Owner, |tx| {
                tx.execute(
                    "INSERT INTO interjections (work, line, speaker, text, informs) VALUES
                     (?1, 'engineer', 'architect', 'Nothing changes for you.', 1),
                     (?1, 'engineer', 'owner', 'Look at the log.', 0)",
                    [r.work],
                )?;
                Ok(())
            })
            .unwrap();
            hook(&mut db);
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
    let id = |text: &str| -> i64 {
        e.db.conn()
            .query_row("SELECT id FROM interjections WHERE text = ?1", [text], |r| r.get(0))
            .unwrap()
    };
    let (asked, told, later_told, later_asked) = (
        id("Why this size?"),
        id("Your question became an item."),
        id("Nothing changes for you."),
        id("Look at the log."),
    );
    // The message tells the two kinds apart.
    let sent = sent.lock().unwrap();
    let (cell, prompt, _) = sent.first().unwrap_or_else(|| panic!("no round at I1:\n{log}"));
    assert_eq!(cell, "I1");
    assert!(
        prompt.contains(&format!(
            "Said to you: ij:{asked}. Answer in this round what needs an answer."
        )),
        "{prompt}"
    );
    assert!(
        prompt.contains(&format!("Told to you, needing no answer: ij:{told}.")),
        "{prompt}"
    );
    // The hook gave nothing the message had given, and each kind of what came since under its own lead.
    let hooked = hooked.lock().unwrap();
    assert_eq!(hooked[0], None, "{log}");
    let second = hooked[1]
        .as_ref()
        .unwrap_or_else(|| panic!("the hook gave nothing:\n{log}"));
    assert!(
        !second.contains(&format!("ij:{asked}")) && !second.contains(&format!("ij:{told}")),
        "{second}"
    );
    let at = |text: &str| second.find(text).unwrap_or_else(|| panic!("no {text} in {second}"));
    assert!(
        at("answer in this round's return") < at(&format!("### ij:{later_asked}"))
            && at(&format!("### ij:{later_asked}")) < at("none of it needs an answer")
            && at("none of it needs an answer") < at(&format!("### ij:{later_told}")),
        "{second}"
    );
    // The round records all four, and each as delivered in it.
    let rounds = agent_rounds(&e, item);
    let (round_id, _, input) = &rounds[1];
    let mut listed: Vec<i64> = input["engine"]["interjections"]
        .as_array()
        .unwrap_or_else(|| panic!("{input}"))
        .iter()
        .filter_map(Value::as_i64)
        .collect();
    listed.sort();
    assert_eq!(listed, [asked, told, later_told, later_asked], "{input}");
    let delivered: Vec<Option<i64>> = [asked, told, later_told, later_asked]
        .iter()
        .map(|j| {
            e.db.conn()
                .query_row("SELECT delivered_round FROM interjections WHERE id = ?1", [j], |r| {
                    r.get(0)
                })
                .unwrap()
        })
        .collect();
    let (work, _, _) = state(&e, item).unwrap();
    let round: i64 = round_id.rsplit('-').next().unwrap().parse().unwrap();
    assert_eq!(delivered, vec![Some(round); 4], "R{work}-{round}:\n{log}");
    let _ = std::fs::remove_dir_all(&s.root);
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
    let research = scripts.remove("R1").unwrap();
    scripts.insert(
        "R1",
        Box::new(move |r: &Request, n: usize| {
            let mut db = engine::database::open(&data).unwrap();
            let said = api::WorkSaid {
                text: "Look at these two.".into(),
                speaker: "owner".into(),
                relayed: None,
                to: None,
                question: None,
                design_card: None,
                condition: None,
                images: two_images(),
            };
            api::say_to_work(&mut db, r.work, &said).unwrap();
            let round: i64 = db
                .conn()
                .query_row("SELECT max(n) FROM rounds WHERE work = ?1", [r.work], |x| x.get(0))
                .unwrap();
            assert_eq!(
                sessions::interject::deliver(&mut db, r.work, round, "engineer").unwrap(),
                None
            );
            research(r, n)
        }),
    );
    let sent: Sent = Arc::default();
    let seen = sent.clone();
    scripts.insert(
        "I1",
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
    let (cell, prompt, images) = sent.first().unwrap_or_else(|| panic!("no round at I1:\n{log}"));
    assert_eq!(cell, "I1");
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
    let mut agents = scripts();
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
    agent_rounds_to(&mut e, item, 1);
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
            ("line".to_string(), true, "R1".to_string()),
            ("model".to_string(), false, "I1".to_string())
        ],
        "{log}"
    );
    let model: String =
        e.db.conn()
            .query_row(
                "SELECT model FROM rounds WHERE work = ?1 AND cell = 'I1'",
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
    agent_rounds_to(&mut e, item, 1);
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
            ("line".to_string(), true, "R1".to_string()),
            ("account".to_string(), false, "I1".to_string())
        ],
        "{log}"
    );
    let home: String = e
        .db
        .conn()
        .query_row(
            "SELECT json_extract(input, '$.call.env.CLAUDE_CONFIG_DIR') FROM rounds WHERE work = ?1 AND cell = 'I1'",
            [work],
            |r| r.get(0),
        )
        .unwrap();
    assert!(home.ends_with("agents/claude-other"), "{home}");
    let _ = std::fs::remove_dir_all(&s.root);
}
