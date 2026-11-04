//! An engineer's questions: the architect's answer, gaps in what the system can do, and what only the owner can
//! do or choose.

use super::*;

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

/// The cell that handed in the round of a work that words said to it were handed in with; none while they wait. The
/// words are found by their text.
fn delivered_at(e: &Engine, work: i64, text: &str) -> Option<String> {
    e.db.conn()
        .query_row(
            "SELECT r.cell FROM interjections j JOIN rounds r ON r.work = j.work AND r.n = j.delivered_round
             WHERE j.work = ?1 AND j.text = ?2",
            params![work, text],
            |r| r.get(0),
        )
        .ok()
}

/// The matters that are an engineer's question, each with whether it is in the architect's queue and whether it was
/// handled.
fn questions_of_engineers(e: &Engine) -> Vec<(bool, bool)> {
    matters(e)
        .into_iter()
        .filter(|m| m.0 == "engineer_question")
        .map(|m| (m.1, m.2))
        .collect()
}

/// The engine of a test whose item's work asks its architect from adversarial testing (`T1`), a cell of the adversarial
/// line, while words wait for the engineer's line: the design chooses adversarial testing, so the work goes on to it
/// after its implementation; when it stands at `T1`, `words` says the words to it, and the engine takes steps until the
/// question's cell (`A2`) has had its first turn. Gives the engine, what it said and the work.
fn t1_asks_with_words_waiting(
    s: &Setup,
    words: impl FnOnce(&mut Engine, i64),
) -> (Engine, Arc<Mutex<Vec<String>>>, i64) {
    let mut agents = scripts();
    agents.insert(
        "D11",
        Box::new(|_: &Request, _: usize| (vec![("outcome.adversarial", "needed")], Vec::new())),
    );
    agents.insert("T1", asks_about_a_gap("The test paths do not name the page's tests."));
    let reply = json!({ "text": "Leave the page's tests out.", "why": "They are another work's." });
    agents.insert(
        "H3",
        Box::new(move |_: &Request, _: usize| (Vec::new(), vec![("outcome.reply", reply.clone())])),
    );
    let (mut e, said, item) = start(s, agents);
    step_to(&mut e, item, "T1");
    let (work, _, _) = state(&e, item).unwrap();
    words(&mut e, work);
    step_until(&mut e, item, &said, "the question to the architect", |e| {
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM rounds WHERE work = ?1 AND cell = 'A2'",
                [work],
                |r| r.get::<_, i64>(0),
            )
            .unwrap()
            > 0
    });
    (e, said, work)
}

// A question asked from a line other than the engineer's is not withdrawn for words that wait for the engineer's line:
// the cell the work goes back to when it is withdrawn, adversarial testing here, hears only its own line's words, so
// withdrawing it would leave the words where they were and the question unanswered. The question stays with the
// architect, who replies, the work goes back to the cell that asked, and the words are still waiting, which the
// engineer's line hears at its next round. Both kinds of words wait here, a word and the notice that an item the work
// depended on was cancelled, which the question is withdrawn for by a row of its own, so that one wrong count of either
// withdraws it.
#[test]
fn a_question_asked_from_another_line_is_not_withdrawn_for_words_waiting_for_the_engineer() {
    let s = setup("ask-off-line");
    let text = "The design's open question is answered: keep the page's tests out.";
    let notice = "An item it depended on was cancelled.";
    let (mut e, said, work) = t1_asks_with_words_waiting(&s, |e, work| {
        e.db.write(Actor::Engine, |tx| {
            // The architect's answer to an open question of the design, as it reaches the engineer's line: not the
            // owner's word, which would be a matter of its own for the architect.
            tx.execute(
                "INSERT INTO interjections (work, line, speaker, text) VALUES (?1, 'engineer', 'architect', ?2)",
                params![work, text],
            )?;
            // The item the notice names as cancelled, on which no work starts.
            tx.execute(
                "INSERT INTO items (project, title, problem, why, deliver, level, cancelled)
                 VALUES ('p', 'Parse it', 'No parser.', 'Needed.', 'change', 'l', 'Not needed.')",
                [],
            )?;
            let cancelled = tx.last_insert_rowid();
            tx.execute(
                "INSERT INTO interjections (work, line, speaker, text, cancelled_item)
                 VALUES (?1, 'engineer', 'architect', ?2, ?3)",
                params![work, notice, cancelled],
            )?;
            Ok(())
        })
        .unwrap();
    });
    let log = said.lock().unwrap().join("\n");
    assert_eq!(question_took(&e, work), None, "{log}");
    assert_eq!(questions_of_engineers(&e), [(true, false)], "{log}");
    let replied = steps_until(&mut e, 80, |e| question_took(e, work).is_some());
    let log = said.lock().unwrap().join("\n");
    assert!(replied, "{log}");
    assert_eq!(question_took(&e, work).as_deref(), Some("answered"), "{log}");
    let path = path_of(&e, work);
    let at = path.iter().rposition(|c| c == "A2").unwrap();
    assert!(path[at + 1..].iter().all(|c| c == "T1"), "{path:?}\n{log}");
    for words in [text, notice] {
        assert_eq!(delivered_at(&e, work, words), None, "{words}\n{log}");
    }
    // The work goes on at the engineer's line, whose next round is given the words.
    move_to(&mut e, work, "A4");
    e.step_work(work).unwrap();
    for words in [text, notice] {
        assert_eq!(delivered_at(&e, work, words).as_deref(), Some("A4"), "{words}\n{log}");
    }
    let _ = std::fs::remove_dir_all(&s.root);
}

// A question asked from another line, kept for words that wait for the engineer's line, is still withdrawn by the rows
// that come after the words in `held`: here its architect is gone, which says nothing of the line. The work goes back to
// the cell that asked, and the words are not taken by it.
#[test]
fn a_question_asked_from_another_line_is_withdrawn_when_its_architect_is_gone_though_words_wait() {
    let s = setup("ask-off-line-gone");
    let text = "Keep the page's tests out of this change.";
    let (mut e, said, work) = t1_asks_with_words_waiting(&s, |e, work| {
        e.db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO interjections (work, line, speaker, text) VALUES (?1, 'engineer', 'architect', ?2)",
                params![work, text],
            )?;
            Ok(())
        })
        .unwrap();
    });
    assert_eq!(question_took(&e, work), None);
    // The architect of a project the engine does not know, the cell's own work alone going on.
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "UPDATE blockers SET project = 'gone' WHERE work = ?1 AND ended IS NULL",
            [work],
        )?;
        Ok(())
    })
    .unwrap();
    advance_until(&mut e, work, |e| question_took(e, work).is_some());
    let log = said.lock().unwrap().join("\n");
    assert_eq!(question_took(&e, work).as_deref(), Some("holder_gone"), "{log}");
    assert_eq!(questions_of_engineers(&e), [(false, false)], "{log}");
    assert_eq!(e.work(work).unwrap().pos.at, "T1", "{log}");
    assert_eq!(delivered_at(&e, work, text), None, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A question asked from the engineer's line is still withdrawn for words that come while it waits: the cell the work
// goes back to is on the engineer's line, hears them, and chooses the way on.
#[test]
fn a_question_asked_from_the_engineer_s_line_is_withdrawn_for_words_that_come_while_it_waits() {
    let s = setup("ask-on-line");
    let mut agents = scripts();
    agents.insert("R1", asks_about_a_gap("The work holds two separate changes."));
    let (mut e, said, item) = start(&s, agents);
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
    let (work, _, _) = state(&e, item).unwrap();
    assert_eq!(question_took(&e, work), None);
    let text = "Halt every work on a red check.";
    owner_says(&mut e, work, text);
    let withdrawn = steps_until(&mut e, 10, |e| question_took(e, work).is_some());
    let log = said.lock().unwrap().join("\n");
    assert!(withdrawn, "{log}");
    assert_eq!(question_took(&e, work).as_deref(), Some("interjection"), "{log}");
    assert_eq!(questions_of_engineers(&e), [(false, false)], "{log}");
    let heard = steps_until(&mut e, 10, |e| delivered_at(e, work, text).is_some());
    let log = said.lock().unwrap().join("\n");
    assert!(heard, "{log}");
    assert_eq!(delivered_at(&e, work, text).as_deref(), Some("A4"), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A question asked by a tool cell (the full CI, the merge queue, the watch on main) with no agent below it is not
// withdrawn for words that wait for the engineer's line either: the cell it would go back to is the tool itself, which
// runs again and hears no words, so withdrawing it would leave the words where they were, the question unanswered and
// the tool, failing again, asking again. The work is placed at the question's cell (`A2`) with the full CI (`T2`) as
// its asker, as the tool's `not_run` row leaves it, and a word waiting for the engineer's line.
#[test]
fn a_question_asked_by_a_tool_cell_is_not_withdrawn_for_words_waiting_for_the_engineer() {
    let s = setup("ask-from-tool");
    let (mut e, said, item) = start(&s, scripts());
    step_to(&mut e, item, "I1");
    let (work, _, _) = state(&e, item).unwrap();
    let text = "Halt every work on a red check.";
    let project: String =
        e.db.conn()
            .query_row("SELECT project FROM items WHERE id = ?1", [item], |r| r.get(0))
            .unwrap();
    e.db.write(Actor::Engine, |tx| {
        tx.execute("UPDATE works SET at = 'A2', waiting = '[\"T2\"]' WHERE id = ?1", [work])?;
        tx.execute(
            "INSERT INTO blockers (work, cell, kind, what, holder, project, at, since)
             VALUES (?1, 'T2', 'question', 'The full CI did not run.', 'architect', ?2, ?3, ?3)",
            params![work, project, db::now()],
        )?;
        tx.execute(
            "INSERT INTO interjections (work, line, speaker, text) VALUES (?1, 'engineer', 'architect', ?2)",
            params![work, text],
        )?;
        Ok(())
    })
    .unwrap();
    // Several steps, and a round of the question's cell among them: a question that no step looked at would be kept
    // whatever the words, and this test would pass before the change too.
    for _ in 0..6 {
        e.step_work(work).unwrap();
    }
    let log = said.lock().unwrap().join("\n");
    let turns: i64 =
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM rounds WHERE work = ?1 AND cell = 'A2'",
                [work],
                |r| r.get(0),
            )
            .unwrap();
    assert!(turns > 0, "{log}");
    assert_eq!(question_took(&e, work), None, "{log}");
    assert_eq!(e.work(work).unwrap().pos.at, "A2", "{log}");
    assert_eq!(delivered_at(&e, work, text), None, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
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
