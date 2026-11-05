//! The corners of the pause after three returns sent back in a row: the owner's Start beside the architect's
//! `resume_work`, the count that starts afresh at each Start, what the architect takes first, and what it cannot let go
//! on. The tests run the flow as built.

use super::sent_back_looks::*;
use super::*;

// The owner's Start lifts the pause as it lifts any other: the cell runs again in the same session, the pause is lifted
// by the owner and by no one else, and the matters are still there for the architect, who is not told as the owner's
// words that a work it never stopped was started. The count of returns in a row starts afresh at the Start: two more
// returns sent back, which are fewer than three, pause the work no more. The architect is held, so that the owner's
// Start is the one that lifts the pause.
#[test]
fn the_owner_s_start_lifts_the_pause_and_the_count_of_returns_in_a_row_starts_afresh() {
    let s = setup("look-owner-start");
    let (mut e, said, item) = start(
        &s,
        returns_sent_back(vec![
            both_wrong(),
            outcome_wrong(),
            progress_wrong(),
            outcome_wrong(),
            progress_wrong(),
        ]),
    );
    let work = e.step(None).unwrap().work.unwrap();
    let held = Go {
        pause: held_but(&[work], &[]),
        ..Go::default()
    };
    let paused = turns_until(&mut e, &held, |e| paused_by(e, work).is_some());
    let log = said.lock().unwrap().join("\n");
    assert!(paused, "{log}");
    assert_eq!(state(&e, item).unwrap().1, "R1", "{log}");
    assert_eq!(looks(&e).len(), 3, "{log}");
    api::resume(&mut e.db, work).unwrap();
    // Two more returns are sent back, and the sixth is taken: no second pause.
    let on = turns_until(&mut e, &held, |e| state(e, item).is_some_and(|w| w.1 != "R1"));
    let log = said.lock().unwrap().join("\n");
    assert!(on, "{log}");
    assert_eq!(lifted_by(&e, work), ["owner"], "{log}");
    assert_eq!(pauses(&e, work).len(), 1, "{log}");
    assert_eq!(paused_by(&e, work), None, "{log}");
    // Five matters, one for each return sent back, none handled and none the owner's words.
    let all = looks(&e);
    assert_eq!(all.len(), 5, "{log}");
    assert!(all.iter().all(|l| !l.1), "{log}");
    assert_eq!(
        number(&e, "SELECT count(*) FROM architect_matters WHERE kind = 'owner_words'"),
        0,
        "{log}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// A work paused for three returns in a row is waited for by the architect's queue: the look at the queue that follows
// the architect's first plan takes the first report of that work's returns as one a work waits on (`waiting`), not as
// one of the rest of the queue (`queued`). The plan that came first withdraws the places to set straight of the plan
// before that no session took, as it always does, and leaves the reports: a return sent back is a report, which no plan
// replaces, so that its cause is still looked for.
#[test]
fn the_architect_takes_the_reports_of_a_paused_work_as_ones_a_work_waits_on_and_the_plan_leaves_them() {
    let s = setup("look-first");
    let (mut e, said, item) = start(
        &s,
        returns_sent_back(vec![both_wrong(), outcome_wrong(), progress_wrong()]),
    );
    let work = e.step(None).unwrap().work.unwrap();
    // A step of an earlier plan, which nothing waits for and no session took.
    let step =
        e.db.write(Actor::Engine, |tx| {
            db::queue_matter(
                tx,
                &db::Matter {
                    project: "p".into(),
                    kind: "set_straight".into(),
                    what: "Put the parser before the lexer.".into(),
                    body: json!({ "fix": { "where": "Put the parser before the lexer.", "steps": [] } }),
                    ..db::Matter::default()
                },
            )
        })
        .unwrap();
    let held = Go {
        pause: held_but(&[work], &[]),
        ..Go::default()
    };
    let paused = turns_until(&mut e, &held, |e| paused_by(e, work).is_some());
    let log = said.lock().unwrap().join("\n");
    assert!(paused, "{log}");
    assert_eq!(state(&e, item).unwrap().1, "R1", "{log}");
    let first: i64 = number(
        &e,
        "SELECT min(id) FROM architect_matters WHERE kind = 'report' AND detail = 'sent_back'",
    );
    // The architect's work starts at its look at the queue, which calls for a plan first: the project has no base yet.
    let started = e.step(None).unwrap();
    let log = said.lock().unwrap().join("\n");
    assert_eq!(started.cell.as_deref(), Some("N0"), "{log}");
    let arch = started.work.unwrap();
    assert!(
        steps_until(&mut e, 40, |e| rows_at(e, arch, "N0").len() >= 2),
        "{}",
        said.lock().unwrap().join("\n")
    );
    let log = said.lock().unwrap().join("\n");
    assert_eq!(rows_at(&e, arch, "N0")[..2], ["plan", "waiting"], "{log}");
    let taken: i64 =
        e.db.conn()
            .query_row(
                "SELECT json_extract(output, '$.next.matter') FROM rounds
                 WHERE work = ?1 AND cell = 'N0' AND transition IS NOT NULL ORDER BY n LIMIT 1 OFFSET 1",
                [arch],
                |r| r.get(0),
            )
            .unwrap();
    assert_eq!(taken, first, "{log}");
    // The plan withdrew the step of the plan before and none of the three reports.
    let withdrawn = |sql: &str| number(&e, sql);
    assert_eq!(
        withdrawn(&format!(
            "SELECT count(*) FROM architect_matters WHERE id = {step} AND withdrawn IS NOT NULL"
        )),
        1,
        "{log}"
    );
    assert_eq!(
        withdrawn(
            "SELECT count(*) FROM architect_matters
             WHERE kind = 'report' AND detail = 'sent_back' AND withdrawn IS NULL AND handled_round IS NULL"
        ),
        3,
        "{log}"
    );
    let _ = std::fs::remove_dir_all(&s.root);
}

// The architect's `resume_work` does not reach a pause the owner made, whatever the work had been sent back for: the
// check of its effects sends the round back, saying the work is not one it may let go on, and the work stays paused by
// the owner. The reference is the words of the item: the owner's pause is not the architect's to lift.
#[test]
fn the_architect_cannot_let_go_on_a_work_the_owner_paused() {
    let s = setup("look-owner-paused");
    let mut agents = scripts();
    agents.insert(
        "H1",
        Box::new(|_: &Request, _: usize| {
            let go = json!({ "kind": "resume_work", "item": 1, "reason": "It looks well." });
            (Vec::new(), vec![("outcome.effects", json!({ "actions": [go] }))])
        }),
    );
    let (mut e, said, item) = start(&s, agents);
    let work = e.step(None).unwrap().work.unwrap();
    api::pause(&mut e.db, work, None).unwrap();
    say(&mut e, "Let the feature go on.");
    let refused = steps_until(&mut e, 60, |e| {
        number(
            e,
            "SELECT count(*) FROM rounds WHERE cell = 'H1' AND rejected LIKE '%nor for a return sent back%'",
        ) > 0
    });
    let log = said.lock().unwrap().join("\n");
    assert!(refused, "{log}");
    assert_eq!(state(&e, item).unwrap().0, work, "{log}");
    assert_eq!(paused_by(&e, work).as_deref(), Some("owner"), "{log}");
    assert!(lifted_by(&e, work).is_empty(), "{log}");
    let _ = std::fs::remove_dir_all(&s.root);
}
