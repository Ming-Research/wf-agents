//! A design's open questions: to the architect or pinned for the owner, and how they hold the merge.

use super::*;

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
