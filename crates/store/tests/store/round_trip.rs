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
    assert!(back.iter().any(|b| matches!(b, Back::Asked(_))), "nothing asked");
    assert!(back.iter().any(|b| matches!(b, Back::HandedIn(_))), "nothing handed in");
    assert!(back.iter().any(|b| matches!(b, Back::Failed { .. })), "nothing failed");
    assert!(
        (r.pauses.iter()).any(|p| p.by == Pauser::Engine && p.why.is_some()),
        "no pause of the engine's with its reason"
    );
    assert!(
        (r.pauses.iter()).any(|p| p.paused && p.by == Pauser::Owner && p.why.is_none()),
        "no pause of the owner's"
    );

    let calls: Vec<&Call> = w.calls.values().collect();
    let kinds = [
        calls.iter().any(|c| matches!(c, Call::Asked(Ask::Tool { .. }))),
        calls.iter().any(|c| matches!(c, Call::Asked(Ask::Person { .. }))),
        calls.contains(&&Call::AtLimit(Limit::Returns { sent_back: 2 })),
        calls.contains(&&Call::AtLimit(Limit::Calls { made: 2 })),
    ];
    assert_eq!(kinds, [true; 4], "a kind of call is missing: {calls:?}");
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
    assert!(r.abandoned.len() >= 3, "too few runs abandoned: {:?}", r.abandoned);
    assert_eq!(r.let_go.len(), 1);
    assert_eq!(
        r.pauses.iter().map(|p| p.paused).collect::<Vec<_>>(),
        [true, false, true, false]
    );
    assert!(r.cancelled.is_some());
    assert_eq!(w.spent.len(), d.ended as usize, "a round's cost was not kept");
    assert_eq!(r.returned.len(), d.ended as usize);
    // Each round that began was given something, and each that ended while its run was open came back with
    // something: only the round stopped at the cancel did not.
    assert_eq!(given.len(), r.rounds.len());
    assert_eq!(back.len() + 1, r.returned.len());
    assert!(w.contents.len() >= r.written.len() + r.finished.len());
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
