//! A work saved after each thing that happens loads back, in a store opened anew on the same file, as it was.

use decide::Pauser;
use decide::run::Limit;
use engine::{Back, Call, Given};

use super::*;

#[test]
fn a_work_saved_after_each_thing_that_happens_loads_back_as_it_was_with_an_entry_of_every_kind() {
    let mut d = Driver::saved();
    // The driver saves and loads back after each event, and compares.
    history(&mut d);
    let w = &d.work;
    let r = &w.records;

    let given: Vec<&Given> = w.given.values().collect();
    assert!(given.iter().any(|g| matches!(g, Given::Task)), "no task");
    assert!(given.iter().any(|g| matches!(g, Given::Results(_))), "no results");
    assert!(
        given.iter().any(|g| matches!(g, Given::SentBack(_))),
        "nothing sent back"
    );
    let back: Vec<&Back> = w.back.values().collect();
    assert!(
        back.iter()
            .any(|b| matches!(b, Back::Returned { asks, .. } if !asks.is_empty())),
        "nothing asked"
    );
    assert!(
        back.iter()
            .any(|b| matches!(b, Back::Returned { handed, .. } if !handed.is_empty())),
        "nothing handed in"
    );
    assert!(back.iter().any(|b| matches!(b, Back::Failed { .. })), "nothing failed");
    assert!(
        (r.pauses.iter()).any(|p| p.by == Pauser::Engine && p.why.is_some()),
        "no pause of the engine's with its reason"
    );
    assert!(
        (r.pauses.iter()).any(|p| p.paused && p.by == Pauser::Owner && p.why.is_none()),
        "no pause of the owner's"
    );

    assert_eq!(w.sessions.len(), 2, "the sessions begun");
    assert_eq!(w.ran.len(), 2, "the tries and what they ran on");
    assert!(
        w.spent
            .values()
            .any(|s| s.context_tokens.is_some() && s.compactions > 0),
        "no context kept"
    );
    assert!(
        w.spent.values().any(|s| s.context_tokens.is_none()),
        "no round of no known context"
    );

    let calls: Vec<&Call> = w.calls.values().collect();
    let kinds = [
        calls.iter().any(|c| matches!(c, Call::Asked(Ask::Tool { .. }))),
        calls.iter().any(|c| matches!(c, Call::Asked(Ask::Person { .. }))),
        calls.contains(&&Call::AtLimit(Limit::Returns { sent_back: 2 })),
    ];
    assert_eq!(kinds, [true; 3], "a kind of call is missing: {calls:?}");
    assert!(r.called.iter().any(|c| c.on.is_none()), "no call the engine makes");
    assert!(
        r.called.iter().any(|c| c.on == Some(Who::Architect)),
        "no call on the architect"
    );
    assert_eq!(r.done.len(), w.results.len());
    assert!(w.results.values().any(|v| v == "withdrawn"), "no call withdrawn");

    let every = [
        (Outsider::Person, false),
        (Outsider::Person, true),
        (Outsider::Outside, false),
        (Outsider::Outside, true),
    ];
    for (by, valued) in every {
        assert!(
            (r.written.iter()).any(|e| e.by == by && e.value.is_some() == valued),
            "no write by {by:?} with a value: {valued}"
        );
    }

    assert!(r.finished.len() >= 3, "too few runs finished: {:?}", r.finished);
    assert!(
        (r.finished.iter()).any(|f| f.writes.iter().any(|w| w.value.is_some())),
        "no run wrote a choice"
    );
    // A run is abandoned only as its cell is skipped, a round is stopped, or the work is cancelled: here once.
    assert!(!r.abandoned.is_empty(), "no run abandoned: {:?}", r.abandoned);
    assert_eq!(r.let_go.len(), 1);
    assert_eq!(
        r.pauses.iter().map(|p| p.paused).collect::<Vec<_>>(),
        [true, false, true, false]
    );
    assert!(r.cancelled.is_some());
    assert_eq!(
        (r.questions.len(), r.answers.len()),
        (1, 1),
        "the question and its answer"
    );
    assert_eq!(w.spent.len(), d.ended as usize, "a round's cost was not kept");
    assert_eq!(r.returned.len(), d.ended as usize);
    // Each round that began was given something, and each that ended came back with something: no round is under way
    // at the cancel, a run that asks having ended with its round.
    assert_eq!(given.len(), r.rounds.len());
    assert_eq!(back.len(), r.returned.len());
    // A content for each write, and for each finish that wrote something: a run that only asked writes nothing.
    let wrote = r.finished.iter().filter(|f| !f.writes.is_empty()).count();
    assert!(w.contents.len() >= r.written.len() + wrote);
    assert!(w.moments.len() > 20);
}

#[test]
fn a_new_work_loads_with_nothing_kept_of_it() {
    let mut store = Store::in_memory().expect("the store opens");
    let work = first_work(&mut store);
    assert_eq!(store.load(work), Ok(Work::default()));
    store.save(work, &Work::default()).expect("nothing is saved");
    assert_eq!(store.load(work), Ok(Work::default()));
}

#[test]
fn runs_of_the_new_form_load_back_with_what_they_dealt_with_what_was_put_in_and_what_filled_their_sessions() {
    let new = of_the_new_form();
    // They are records a work can have.
    let flow = judged();
    let decision = decide::decide(&flow.flow, &new.records);
    assert!(!matches!(decision, decide::Decision::Fault(_)), "{decision:?}");
    assert_eq!(work::filled::check(&new), Ok(()));
    let mut d = Driver::new();
    history(&mut d);
    let db = Db::new();
    let mut store = db.open();
    let before = first_work(&mut store);
    let (_, work) = store
        .add("second", "2", "one", &[], "judged", Time(START), &Work::default())
        .expect("the item is added");
    store.save(before, &d.work).expect("the work is saved");
    store.save(work, &new).expect("the work is saved");
    let opened = db.open();
    assert_eq!(opened.load(work), Ok(new.clone()));
    assert_eq!(opened.load(before), Ok(d.work.clone()));

    // In the file, every run the engine finished says what it dealt with, and so does each of the new form, some naming
    // nothing.
    let rows = dump(&db.path);
    let of = |n: WorkNumber| move |row: &&Vec<String>| row[0] == format!("Integer({})", n.0);
    let dealt = |n: WorkNumber| -> Vec<String> {
        rows["finished"]
            .iter()
            .filter(of(n))
            .map(|row| row[3].clone())
            .collect()
    };
    assert!(dealt(before).iter().all(|v| v != "Null"));
    let mut said = dealt(work);
    said.sort();
    let mut expected = [
        r#"Text("[\"request\"]")"#,
        r#"Text("[]")"#,
        r#"Text("[]")"#,
        r#"Text("[\"request\",\"opinions\"]")"#,
    ];
    expected.sort();
    assert_eq!(said, expected);
    // A version handed in as changing nothing in meaning is kept as a change that does not matter, as one was before.
    let mattering: Vec<&str> = (rows["writes"].iter().filter(of(work)))
        .filter(|row| row[3] == r#"Text("text")"#)
        .map(|row| row[4].as_str())
        .collect();
    assert_eq!(mattering, ["Integer(1)", "Integer(0)"]);
    assert_eq!(rows["put_in"].len(), 1);
    // Each try that was filled, the round tried twice among them.
    assert_eq!(rows["filled"].len(), 5);
    assert!(new.filled.iter().any(|f| f.attempt == 2));
}
