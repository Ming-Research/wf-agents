//! Decisions: designs that depart from their item's, the rulings they wait for, and the decisions the engine
//! writes in items from the records.

use super::*;

/// The line of a decision the owner made, as an item written by hand lists it under its head, after a list mark.
const OWNER_DECIDED: &str = "Decision (2026-11-01, owner): Keep the light way on a path of its own. Situation: s. \
                             Reason: r. Replaces: none. Words: \"own path\" (matter 99, 2026-11-01)";

/// The line of a decision the architect made.
const ARCHITECT_DECIDED: &str = "Decision (2026-11-01, architect): Keep the light way on a path of its own. Situation: \
                                 s. Reason: r. Replaces: none. Words: \"own path\" (matter 98, 2026-11-01)";

/// What the designs of these tests do, in their solution: otherwise than the decisions above.
const ON_THE_MAIN_PATH: &str = "Keep the light way on the main path, skipping cells.";

/// The owner's ruling on the departure, which lets the design's way stand.
const LET_IT_STAND: &str = "Let the main path stand.";

/// Writes an item's approach, as the owner does.
fn set_approach(e: &mut Engine, item: i64, approach: &str) {
    e.db.write(Actor::Owner, |tx| {
        tx.execute("UPDATE items SET approach = ?2 WHERE id = ?1", params![item, approach])?;
        Ok(())
    })
    .unwrap();
}

/// An item's approach as it stands.
fn approach_of(e: &Engine, item: i64) -> String {
    e.db.conn()
        .query_row("SELECT coalesce(approach, '') FROM items WHERE id = ?1", [item], |r| {
            r.get(0)
        })
        .unwrap()
}

/// A design whose solution keeps the light way on the main path, answering the item's decisions by `rows`.
fn designs_answering(rows: Value) -> Script {
    Box::new(move |_: &Request, _: usize| {
        (
            vec![("outcome.measure", "not_measured")],
            vec![
                (
                    "outcome.acceptance",
                    json!([{ "by": "test", "id": "A1", "condition": "The feature is there." }]),
                ),
                (
                    "outcome.design",
                    json!({ "solution": ON_THE_MAIN_PATH, "where": "The flow.", "excluded": "Nothing.", "pitfalls": "None." }),
                ),
                ("outcome.item_decisions", rows.clone()),
            ],
        )
    })
}

/// A review of the design that finds the design's answer to each decision `ids` names stands against its solution.
fn tries_the_answers(ids: &'static [&'static str]) -> Script {
    Box::new(move |_: &Request, _: usize| {
        let rows: Vec<Value> = ids
            .iter()
            .map(|id| json!({ "decision": id, "verdict": "holds", "passage": ON_THE_MAIN_PATH, "why": "It is what it does." }))
            .collect();
        (
            vec![("outcome.better", "present_best")],
            vec![("outcome.claims", json!({ "articles": [], "decisions": rows }))],
        )
    })
}

/// A design that departs from the item's one decision, and the review that finds it does.
fn departing(agents: &mut BTreeMap<&'static str, Script>) {
    let row = json!({ "decision": "L1", "verdict": "departs", "passage": ON_THE_MAIN_PATH, "instead": "One path is simpler to keep." });
    agents.insert("D2", designs_answering(json!([row])));
    agents.insert("D3", tries_the_answers(&["L1"]));
    agents.insert(
        "A4",
        Box::new(|_: &Request, _: usize| (vec![("outcome.next", "carry_on")], Vec::new())),
    );
}

/// The architect ruling on a departure from its own decision each time it is asked, by `ruling`: `stands` or
/// `turned_down`.
fn rules_on_its_own(ruling: &'static str) -> Script {
    Box::new(move |_: &Request, _: usize| {
        let reply = json!({ "text": "As ruled.", "why": "My own decision.", "departure": ruling });
        (Vec::new(), vec![("outcome.reply", reply)])
    })
}

/// A work's blockers, oldest first: each with the cell that raised it, its holder, its kind and whether it ended.
fn blockers_of(e: &Engine, work: i64) -> Vec<(String, String, String, bool)> {
    let mut st =
        e.db.conn()
            .prepare("SELECT cell, holder, kind, ended IS NOT NULL FROM blockers WHERE work = ?1 ORDER BY id")
            .unwrap();
    st.query_map([work], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .unwrap()
        .map(|x| x.unwrap())
        .collect()
}

/// Why each return of a cell of any work was sent back, in order.
fn sent_back_at(e: &Engine, cell: &str) -> Vec<String> {
    let mut st =
        e.db.conn()
            .prepare("SELECT rejected FROM rounds WHERE cell = ?1 AND rejected IS NOT NULL ORDER BY work, n")
            .unwrap();
    st.query_map([cell], |r| r.get(0))
        .unwrap()
        .map(|x| x.unwrap())
        .collect()
}

// A design that departs from a decision the owner made stops the work once the checks after design passed, before it
// implements: it waits for the ruling, the owner holds its blocker, and the architect's queue has the open question
// that names the decision's line. The architect's reply on its own judgment is sent back, since only whoever made a
// decision rules on a departure from it; it pins the question for the owner, and replies once the owner answered. The
// engineer is told the ruling, carries on, comes back to the wait, which now lets it go, and implements, its blocker
// ended.
#[test]
fn a_design_departing_from_the_owner_s_decision_waits_for_the_owner_s_ruling_before_it_implements() {
    let s = setup("departs-owner");
    let path = config::database(&s.data);
    let mut agents = scripts();
    departing(&mut agents);
    let at = path.clone();
    agents.insert(
        "H3",
        Box::new(move |r: &Request, n: usize| {
            if r.prompt.contains(LET_IT_STAND) {
                let reply =
                    json!({ "text": "Keep it on the main path.", "why": "The owner let it stand.", "departure": "stands" });
                return (Vec::new(), vec![("outcome.reply", reply)]);
            }
            if n == 1 {
                let reply = json!({ "text": "Go on.", "why": "It is small." });
                return (Vec::new(), vec![("outcome.reply", reply)]);
            }
            let c = db::open_read(&at).unwrap();
            let link: String = c
                .query_row(
                    "SELECT body ->> '$.design_link' FROM architect_matters WHERE kind = 'open_question' ORDER BY id LIMIT 1",
                    [],
                    |x| x.get(0),
                )
                .unwrap();
            asks_the_owner(&format!("The design departs from your decision. See [the design]({link})."))
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    set_approach(
        &mut e,
        item,
        &format!("Approach:\n- Do it.\n\nDecisions:\n- {OWNER_DECIDED}"),
    );
    step_to(&mut e, item, "D12");
    let (work, _, _) = state(&e, item).unwrap();
    e.advance(work).unwrap();
    let log = |said: &Arc<Mutex<Vec<String>>>| said.lock().unwrap().join("\n");
    assert_eq!(
        ended_at(&e, item, "D12").first().map(|x| x.0.clone()),
        Some(Some("wait_owner".to_string())),
        "{}",
        log(&said)
    );
    assert!(ended_at(&e, item, "I1").is_empty(), "{}", log(&said));
    assert_eq!(
        blockers_of(&e, work),
        [("D12".to_string(), "owner".to_string(), "ruling".to_string(), false)],
        "{}",
        log(&said)
    );
    let asked = open_questions(&e, work);
    assert_eq!(asked.len(), 1, "{}", log(&said));
    assert_eq!(
        asked[0].1,
        format!("The design departs from this decision of the item: - {OWNER_DECIDED}")
    );
    let matter = asked[0].0;
    let departs = matter_body(&e, matter);
    assert_eq!(
        (&departs["departs"]["by"], &departs["passage"]),
        (&json!("owner"), &json!(ON_THE_MAIN_PATH)),
        "{departs}"
    );
    // The architect's own reply is sent back, and it pins the question for the owner instead.
    let pinned = |e: &mut Engine| parked_for(e, matter).is_some();
    assert!(steps_until(&mut e, 80, pinned), "{}", log(&said));
    let back = sent_back_at(&e, "H3");
    assert!(
        back.len() == 1 && back[0].contains("departure_ruled_by_owner"),
        "{back:?}"
    );
    // The work waits for the ruling meanwhile, and implements nothing.
    for _ in 0..3 {
        e.advance(work).unwrap();
    }
    assert!(ended_at(&e, item, "I1").is_empty(), "{}", log(&said));
    assert_eq!(open_questions(&e, work)[0].2, None);
    let (ask, _) = parked_for(&e, matter).unwrap();
    api::reply_to_question(&mut e.db, "p", ask, LET_IT_STAND, &[], db::Speaker::Owner).unwrap();
    let implements =
        |e: &mut Engine| state(e, item).is_some_and(|x| x.1 == "I1") || !ended_at(e, item, "I1").is_empty();
    assert!(steps_until(&mut e, 160, implements), "{}", log(&said));
    let rows = transitions(&e, item, "D12");
    assert!(
        rows.first().map(String::as_str) == Some("wait_owner")
            && rows.iter().any(|r| r == "interjection")
            && rows.last().map(String::as_str) == Some("clear"),
        "{rows:?}\n{}",
        log(&said)
    );
    assert!(
        transitions(&e, item, "A4").iter().any(|r| r == "carry_on"),
        "{}",
        log(&said)
    );
    assert!(blockers_of(&e, work).iter().all(|b| b.3), "{:?}", blockers_of(&e, work));
    let _ = std::fs::remove_dir_all(&s.root);
}

// A design that departs only from a decision the architect made waits for the architect's ruling: the architect holds
// its blocker, and its reply on its own judgment rules on it, with nothing asked of the owner; the engineer carries on
// and the work implements, its blocker ended. The architect's own answer is no word of the owner's: no batch records it.
#[test]
fn a_design_departing_from_the_architect_s_decision_waits_for_the_architect() {
    let s = setup("departs-architect");
    let mut agents = scripts();
    departing(&mut agents);
    agents.insert("H3", rules_on_its_own("stands"));
    let (mut e, said, item) = start(&s, agents);
    set_approach(&mut e, item, &format!("Decisions:\n{ARCHITECT_DECIDED}"));
    step_to(&mut e, item, "D12");
    let (work, _, _) = state(&e, item).unwrap();
    e.advance(work).unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(
        ended_at(&e, item, "D12").first().map(|x| x.0.clone()),
        Some(Some("wait_architect".to_string())),
        "{log}"
    );
    assert_eq!(
        blockers_of(&e, work),
        [("D12".to_string(), "architect".to_string(), "ruling".to_string(), false)],
        "{log}"
    );
    let implements =
        |e: &mut Engine| state(e, item).is_some_and(|x| x.1 == "I1") || !ended_at(e, item, "I1").is_empty();
    assert!(
        steps_until(&mut e, 120, implements),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    assert_eq!(
        transitions(&e, item, "D12").last().map(String::as_str),
        Some("clear"),
        "{log}"
    );
    assert!(blockers_of(&e, work).iter().all(|b| b.3), "{:?}", blockers_of(&e, work));
    assert!(sent_back_at(&e, "H3").is_empty(), "{log}");
    let asks: i64 =
        e.db.conn()
            .query_row("SELECT count(*) FROM owner_asks", [], |r| r.get(0))
            .unwrap();
    assert_eq!(asks, 0, "{log}");
    let b5: i64 =
        e.db.conn()
            .query_row("SELECT count(*) FROM rounds WHERE cell = 'B5'", [], |r| r.get(0))
            .unwrap();
    assert_eq!(b5, 0, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A design that follows each decision of its item, or is not concerned by it, goes on past the wait for a ruling at
// once, as a design of an item with no decision does: no blocker, and no question put.
#[test]
fn a_design_true_to_its_item_s_decisions_goes_on_past_the_wait_for_a_ruling() {
    let s = setup("departs-none");
    let mut agents = scripts();
    let rows = json!([
        { "decision": "L1", "verdict": "follows", "passage": ON_THE_MAIN_PATH },
        { "decision": "L2", "verdict": "not_concerned", "why": "It is about the page." },
    ]);
    agents.insert("D2", designs_answering(rows));
    agents.insert("D3", tries_the_answers(&["L1", "L2"]));
    let (mut e, said, item) = start(&s, agents);
    set_approach(
        &mut e,
        item,
        "Decisions:\n- Decision (2026-11-01, owner): Keep it on the main path.\nDecision (2026-11-02, owner): Blue \
         buttons.",
    );
    step_to(&mut e, item, "I1");
    let (work, _, _) = state(&e, item).unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(ended_at(&e, item, "D12"), [(Some("clear".to_string()), None)], "{log}");
    assert!(blockers_of(&e, work).is_empty(), "{log}");
    assert!(open_questions(&e, work).is_empty(), "{log}");
    // The design's round was given the two decisions, the line listed by hand among them, and keeps them as numbered.
    let given = input_at(&e, work, "D2", false)["engine"]["decisions"].clone();
    assert_eq!(
        given,
        json!([
            { "decision": "L1", "line": "- Decision (2026-11-01, owner): Keep it on the main path.", "by": "owner" },
            { "decision": "L2", "line": "Decision (2026-11-02, owner): Blue buttons.", "by": "owner" },
        ])
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

/// The design's cells as a flow version before item #247 had them: the design's articles in a text, with no answers to
/// the item's decisions, and its review with no claims tried.
fn design_before_247(g: &mut flow::CompiledGraph) {
    for (id, own, added) in [("D2", "design", "item_decisions"), ("D3", "better", "claims")] {
        let c = g.cells.iter_mut().find(|c| c.id == id).unwrap();
        c.carried = None;
        let forms = c.schema.as_mut().unwrap()["properties"]["outcome"]["anyOf"]
            .as_array_mut()
            .unwrap();
        let form = forms.iter_mut().find(|f| f["properties"].get(own).is_some()).unwrap();
        form["properties"].as_object_mut().unwrap().remove(added);
        form["required"].as_array_mut().unwrap().retain(|r| r != added);
        if id == "D2" {
            form["properties"]["articles"] = json!({ "type": "string" });
        }
    }
}

// Item #247 had the design answer the constitution's articles in rows and each decision of its item, which its review
// and the wait for a ruling read. A work under way when it came carries a design handed in before: it moves to the
// version the engine runs where it stands, at the review, and does not come to cell V1 to design again; the review
// tries the item's decision against the design as it is, and the wait finds no departure in it and lets the work go on
// to implement. This holds the flow's own marks of D2 and D3 (`earlier`), and is removed with them.
#[test]
fn a_work_carrying_a_design_from_before_its_rows_moves_and_goes_on_to_implement() {
    let s = setup("design-before-rows");
    let mut agents = scripts();
    let row = json!({ "decision": "L1", "verdict": "follows", "passage": ON_THE_MAIN_PATH });
    agents.insert("D2", designs_answering(json!([row])));
    agents.insert("D3", tries_the_answers(&["L1"]));
    let (mut e, said, item) = start(&s, agents);
    set_approach(
        &mut e,
        item,
        "Decisions:\n- Decision (2026-11-01, owner): Keep it on the main path.",
    );
    step_to(&mut e, item, "D3");
    let (work, _, _) = state(&e, item).unwrap();
    let v = older(&mut e, &[work], design_before_247);
    // The design as one handed in before: its articles in a text, and no decisions given or answered.
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "UPDATE rounds SET output = json_set(json_remove(output, '$.outcome.item_decisions'),
                                                 '$.outcome.articles', 'It keeps every article it touches.'),
                               input = json_remove(input, '$.engine.decisions')
             WHERE work = ?1 AND cell = 'D2' AND transition IS NOT NULL",
            [work],
        )?;
        Ok(())
    })
    .unwrap();
    step_to(&mut e, item, "I1");
    let log = said.lock().unwrap().join("\n");
    assert_eq!(versions_of(&e, work), (e.flow_now, Some(v)), "{log}");
    assert!(brought(&e, work).is_empty(), "{log}");
    assert_eq!(transitions(&e, item, "D2"), ["first"], "{log}");
    assert_eq!(
        ended_at(&e, item, "D3"),
        [(Some("present_best".to_string()), None)],
        "{log}"
    );
    assert_eq!(ended_at(&e, item, "D12"), [(Some("clear".to_string()), None)], "{log}");
    assert!(blockers_of(&e, work).is_empty(), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A work that delivers a report goes on from the checks after design by its own way, past the wait for a ruling, which
// lets it go at once when its design departs from nothing.
#[test]
fn a_report_goes_on_past_the_wait_for_a_ruling() {
    let s = setup("departs-report");
    let (mut e, said) = engine(&s, scripts());
    quiet(&mut e);
    let item =
        e.db.write(Actor::Owner, |tx| {
            tx.execute(
                "INSERT INTO items (project, title, problem, why, deliver, level)
                 VALUES ('p', 'Study the parser', 'x', 'y', 'report', 'l')",
                [],
            )?;
            let id = tx.last_insert_rowid();
            tx.execute(
                "INSERT INTO dispatch_queue (item, position) VALUES (?1, 1.0)",
                params![id],
            )?;
            Ok(id)
        })
        .unwrap();
    step_to(&mut e, item, "I1");
    let log = said.lock().unwrap().join("\n");
    assert_eq!(transitions(&e, item, "D10"), ["report"], "{log}");
    assert_eq!(transitions(&e, item, "D12"), ["clear"], "{log}");
    assert!(ended_at(&e, item, "D11").is_empty(), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// Each version of a design is through its review once: the review's better design is written and goes on past it,
// and a design written again after the work went back to design for what was said to it is reviewed again, not sent
// straight on to the checks after design.
#[test]
fn a_design_written_again_after_the_work_went_back_to_design_is_reviewed_again() {
    let s = setup("redesign-reviewed");
    let mut agents = scripts();
    agents.insert(
        "D3",
        Box::new(|_: &Request, n: usize| {
            let verdict = if n == 1 { "found" } else { "present_best" };
            (vec![("outcome.better", verdict)], Vec::new())
        }),
    );
    agents.insert(
        "A4",
        Box::new(|_: &Request, _: usize| (vec![("outcome.next", "redesign")], Vec::new())),
    );
    let (mut e, said, item) = start(&s, agents);
    step_to(&mut e, item, "T7");
    let (work, _, _) = state(&e, item).unwrap();
    owner_says(&mut e, work, "Design it again.");
    advance_until(&mut e, work, |e| transitions(e, item, "D3").len() == 2);
    let log = said.lock().unwrap().join("\n");
    assert_eq!(transitions(&e, item, "D2"), ["first", "revised", "first"], "{log}");
    assert_eq!(transitions(&e, item, "D3"), ["found", "present_best"], "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// The design (part 5) says that when whoever made a decision turns a departure down, the engineer redesigns from D1. A
// design written again that still departs from the same decision has had no ruling: the ruling given was on the earlier
// design, and it said no. So the work waits at the wait for a ruling again, and does not go on to implement. Here the
// architect turns down a departure from its own decision, the engineer redesigns, and the new design departs in the
// same way. Once that one is turned down too, the engineer's choice to carry on with it anyway is a flow anomaly, not a
// way to implement: the ruling the engine reads says the design must change.
#[test]
fn a_design_written_again_that_still_departs_after_its_departure_was_turned_down_waits_again() {
    let s = setup("departs-again");
    let mut agents = scripts();
    departing(&mut agents);
    // The first ruling sends the work back to design; any later one lets it carry on.
    agents.insert(
        "A4",
        Box::new(|_: &Request, n: usize| {
            let next = if n == 1 { "redesign" } else { "carry_on" };
            (vec![("outcome.next", next)], Vec::new())
        }),
    );
    // The architect turns the departure down each time it is asked.
    agents.insert("H3", rules_on_its_own("turned_down"));
    let (mut e, said, item) = start(&s, agents);
    set_approach(&mut e, item, &format!("Decisions:\n{ARCHITECT_DECIDED}"));
    step_to(&mut e, item, "D12");
    let (work, _, _) = state(&e, item).unwrap();
    e.advance(work).unwrap();
    // The wait, the ruling coming in as an interjection, and the wait for the design written again.
    let judged_again = |e: &mut Engine| transitions(e, item, "D12").len() >= 3;
    assert!(
        steps_until(&mut e, 200, judged_again),
        "{:?}\n{}",
        transitions(&e, item, "D12"),
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    assert_eq!(transitions(&e, item, "D2"), ["first", "first"], "{log}");
    let rows = transitions(&e, item, "D12");
    // The design written again departs from the decision the ruling kept, so it waits for a ruling of its own; going on
    // to implement here means a turned-down departure got past the wait.
    assert_eq!(
        rows[..3],
        ["wait_architect", "interjection", "wait_architect"],
        "{rows:?}\n{log}"
    );
    // The second design turned down, carrying on with it goes to the architect as a flow anomaly.
    let chose_again = |e: &mut Engine| transitions(e, item, "A4").len() >= 2;
    assert!(
        steps_until(&mut e, 200, chose_again),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    assert_eq!(
        transitions(&e, item, "A4")[..2],
        ["redesign", "carry_on_unfit"],
        "{log}"
    );
    assert!(ended_at(&e, item, "I1").is_empty(), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A ruling that lets a departure stand holds for the work: a design written again after it, for another reason, that
// departs from the same decision is not asked about again, and goes on past the wait for a ruling at once.
#[test]
fn a_departure_let_stand_is_not_asked_about_again_when_the_design_is_written_again() {
    let s = setup("departs-stands");
    let mut agents = scripts();
    departing(&mut agents);
    // After the ruling the engineer designs again; after that it carries on.
    agents.insert(
        "A4",
        Box::new(|_: &Request, n: usize| {
            let next = if n == 1 { "redesign" } else { "carry_on" };
            (vec![("outcome.next", next)], Vec::new())
        }),
    );
    agents.insert("H3", rules_on_its_own("stands"));
    let (mut e, said, item) = start(&s, agents);
    set_approach(&mut e, item, &format!("Decisions:\n{ARCHITECT_DECIDED}"));
    let implements =
        |e: &mut Engine| state(e, item).is_some_and(|x| x.1 == "I1") || !ended_at(e, item, "I1").is_empty();
    assert!(
        steps_until(&mut e, 250, implements),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let (work, _, _) = state(&e, item).unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(transitions(&e, item, "D2"), ["first", "first"], "{log}");
    assert_eq!(
        transitions(&e, item, "D12"),
        ["wait_architect", "interjection", "clear"],
        "{log}"
    );
    assert_eq!(open_questions(&e, work).len(), 1, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}

// A design written again while the owner has not ruled on where the earlier one departed withdraws that question: it is
// about a design that is no more. The ask that pinned it for the owner needs no answer, the merge does not wait for it,
// and the owner rules once, on the design written again. Here the owner asks for a new design while the work waits.
#[test]
fn a_design_written_again_withdraws_the_unanswered_question_on_the_earlier_one() {
    let s = setup("departs-superseded");
    let path = config::database(&s.data);
    let mut agents = scripts();
    departing(&mut agents);
    agents.insert(
        "A4",
        Box::new(|_: &Request, n: usize| {
            let next = if n == 1 { "redesign" } else { "carry_on" };
            (vec![("outcome.next", next)], Vec::new())
        }),
    );
    let at = path.clone();
    // The architect pins each departure for the owner, linking the design it is about, and rules once they answered.
    agents.insert(
        "H3",
        Box::new(move |r: &Request, _: usize| {
            if r.prompt.contains(LET_IT_STAND) {
                let reply = json!({ "text": "Keep it.", "why": "The owner let it stand.", "departure": "stands" });
                return (Vec::new(), vec![("outcome.reply", reply)]);
            }
            let c = db::open_read(&at).unwrap();
            let link: String = c
                .query_row(
                    "SELECT body ->> '$.design_link' FROM architect_matters WHERE kind = 'open_question'
                       AND withdrawn IS NULL AND body ->> '$.answer' IS NULL ORDER BY id DESC LIMIT 1",
                    [],
                    |x| x.get(0),
                )
                .unwrap();
            asks_the_owner(&format!(
                "The design departs from your decision. See [the design]({link})."
            ))
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    set_approach(&mut e, item, &format!("Decisions:\n- {OWNER_DECIDED}"));
    step_to(&mut e, item, "D12");
    let (work, _, _) = state(&e, item).unwrap();
    let log = |said: &Arc<Mutex<Vec<String>>>| said.lock().unwrap().join("\n");
    let first = open_questions(&e, work)[0].0;
    // The owner asks for a new design while the work waits for the ruling.
    owner_says(&mut e, work, "Design it again.");
    let asked_again = |e: &mut Engine| open_questions(e, work).len() == 2;
    assert!(steps_until(&mut e, 200, asked_again), "{}", log(&said));
    let withdrawn: Option<String> =
        e.db.conn()
            .query_row("SELECT withdrawn FROM architect_matters WHERE id = ?1", [first], |r| {
                r.get(0)
            })
            .unwrap();
    assert!(
        withdrawn.as_deref().is_some_and(|w| w.contains("written again")),
        "{withdrawn:?}\n{}",
        log(&said)
    );
    // The owner rules on the question the design written again put, once it is pinned for them.
    let second = open_questions(&e, work)[1].0;
    let pinned = |e: &mut Engine| parked_for(e, second).is_some();
    assert!(steps_until(&mut e, 120, pinned), "{}", log(&said));
    let (ask, _) = parked_for(&e, second).unwrap();
    api::reply_to_question(&mut e.db, "p", ask, LET_IT_STAND, &[], db::Speaker::Owner).unwrap();
    let implements =
        |e: &mut Engine| state(e, item).is_some_and(|x| x.1 == "I1") || !ended_at(e, item, "I1").is_empty();
    assert!(steps_until(&mut e, 200, implements), "{}", log(&said));
    // No ask is left for the owner, the owner answered one, and the merge waits for no question.
    let asks: Vec<(i64, Option<String>)> = {
        let mut st =
            e.db.conn()
                .prepare("SELECT id, answer ->> '$.by' FROM owner_asks WHERE answer IS NOT NULL ORDER BY id")
                .unwrap();
        st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(|x| x.unwrap())
            .collect()
    };
    let unanswered: i64 =
        e.db.conn()
            .query_row("SELECT count(*) FROM owner_asks WHERE answer IS NULL", [], |r| r.get(0))
            .unwrap();
    assert_eq!(unanswered, 0, "{asks:?}");
    let by_owner = asks.iter().filter(|a| a.1.as_deref() != Some("engine")).count();
    assert_eq!(by_owner, 1, "{asks:?}");
    let waiting: i64 =
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM work_open_questions q JOIN architect_matters m ON m.id = q.matter
                 WHERE q.work = ?1 AND q.answer IS NULL AND m.withdrawn IS NULL",
                [work],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(waiting, 0, "{}", log(&said));
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

/// One line of a decision about the parser's item as the engine writes it, from the parts the test hands in.
fn decision_line(date: &str, who: &str, what: &str, words: &str) -> String {
    format!(
        "Decision ({date}, {who}): {what}. Situation: The parser's item is being written. Reason: It keeps the \
         item small. Replaces: none. Words: {words}"
    )
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
    let path = config::database(&s.data);
    let mut agents = scripts();
    let (first, second) = (held.clone(), held.replace("- Parse it.", "- Parse it first."));
    agents.insert(
        "H8",
        Box::new(move |_: &Request, n: usize| {
            let changes = versioned(
                &path,
                if n == 1 {
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
                },
            );
            (
                vec![("outcome.answer", "done")],
                vec![
                    ("outcome.answer.reply", json!("Noted.")),
                    ("outcome.effects", json!({ "item_changes": changes })),
                ],
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

// The decisions of an item that a batch merges into another are the kept item's from then on, whatever else the batch
// does to the kept item's `approach`. A batch that merges item 2 into item 1 and then writes item 1's approach whole,
// from the text item 1 had before the merge, must not lose the lines the merge carried: either the return is sent back
// or the line stands in item 1. Oracle: the acceptance rule (A4) that a recorded line is never lost or changed but by
// the engine's own writes, and the merge's rule that the absorbed item's lines go to the kept item.
#[test]
fn a_text_that_replaces_the_approach_of_the_item_kept_in_a_merge_keeps_the_decisions_the_merge_carried() {
    let s = setup("merge-then-edit");
    let old = "Decision (2026-10-01, owner): Old one. Situation: s. Reason: r. Replaces: none. \
               Words: \"x\" (matter 99, 2026-10-01)";
    let mut agents = scripts();
    agents.insert(
        "H8",
        Box::new(|_: &Request, n: usize| {
            let merge = json!({ "kind": "merge", "keep": "1", "absorbed": ["2"], "reason": "The same work." });
            let edit = json!({ "kind": "edit_item", "item": "1", "part": "approach", "reason": "Say it first.",
                               "text": "Approach:\n- Parse it first." });
            let changes = match n {
                1 => json!([merge, edit]),
                2 => json!([merge]),
                _ => return (Vec::new(), Vec::new()),
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
    let (mut e, said) = engine(&s, agents);
    quiet(&mut e);
    an_item(&mut e);
    e.db.write(Actor::Owner, |tx| {
        tx.execute("UPDATE items SET approach = 'Approach:\n- Parse it.' WHERE id = 1", [])?;
        tx.execute(
            "INSERT INTO items (project, title, problem, why, approach, deliver, level)
             VALUES ('p', 'Parse more', 'x', 'y', ?1, 'change', 'l')",
            [format!("Approach:\n- Other.\nDecisions:\n{old}")],
        )?;
        Ok(())
    })
    .unwrap();
    say(
        &mut e,
        "Merge the second item into the first, and write the first again.",
    );
    let arch = e.step(None).unwrap().work.unwrap();
    assert!(
        steps_until(&mut e, 80, |e| architect_work_done(e, arch)),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    let approach: String =
        e.db.conn()
            .query_row("SELECT approach FROM items WHERE id = 1", [], |r| r.get(0))
            .unwrap();
    // The line is in the kept item, whether the batch was sent back or not.
    assert!(
        approach.lines().any(|l| l == old),
        "the merge carried the line, and the text written over it lost it:\n{approach}\n{log}"
    );
    let cancelled: i64 =
        e.db.conn()
            .query_row(
                "SELECT count(*) FROM items WHERE id = 2 AND cancelled IS NOT NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(cancelled, 1, "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}
