//! Calls: a return that asks ends its run, one call made for each thing asked; the cell is entered again, as a new run
//! given the results, only once all have them.

use super::*;

#[test]
fn each_thing_asked_for_is_one_call_that_waits_on_nobody() {
    let (mut w, write, _) = W::requested();
    let effects = w.returns(write, Came::asks(vec![tool("search"), tool("read")]));
    let calls = calls_of(&effects);
    assert_eq!(
        calls
            .iter()
            .map(|(_, run, what)| (*run, what.clone()))
            .collect::<Vec<_>>(),
        vec![(write, Call::Asked(tool("search"))), (write, Call::Asked(tool("read")))]
    );
    let on: Vec<_> = (w.work.records.called.iter())
        .map(|c| (c.call, c.run, c.on.clone()))
        .collect();
    assert_eq!(on, vec![(calls[0].0, write, None), (calls[1].0, write, None)]);
    assert_eq!(w.work.calls.get(&calls[0].0), Some(&Call::Asked(tool("search"))));
}

// No carrier asks a person by a call: the owner is asked by the record of the work's questions. A return that asks
// one is refused, as nothing that carries a round out says it, and the work is left as it was (decision
// no-call-is-made-on-a-person). Milliseconds alone, in memory; no other test holds the loop to making no call on a
// person.
#[test]
fn a_return_that_asks_a_person_by_a_call_is_refused() {
    let (mut w, write, _) = W::requested();
    let before = w.work.clone();
    let now = w.tick();
    let came = Came::asks(vec![tool("search"), question(Who::Owner)]);
    let taken = handle(
        &w.flow,
        &mut w.work,
        now,
        Event::Returned {
            run: write,
            came,
            spent: spent(1),
        },
    );
    assert!(taken.is_err(), "{taken:?}");
    assert_eq!(w.work.records, before.records);
    assert!(w.work.calls.is_empty(), "a call was made: {:?}", w.work.calls);
}

#[test]
fn a_return_that_asks_ends_its_run_having_dealt_with_nothing_and_its_cell_waits_for_every_result() {
    let (mut w, write, request) = W::requested();
    let effects = w.returns(write, Came::asks(vec![tool("search"), tool("read")]));
    let calls: Vec<CallId> = calls_of(&effects).iter().map(|(c, ..)| *c).collect();
    let finished = w.work.records.finished.last().expect("the run finished");
    assert_eq!((finished.run, finished.writes.clone()), (write, vec![]));
    assert_eq!(finished.dealt_with, Some(vec![]));
    // The calls were made before the finish.
    assert!(w.work.records.called.iter().all(|c| c.at < finished.at));
    assert_eq!(w.done(calls[0]), vec![]);
    let effects = w.done(calls[1]);
    let again = w.work.records.started.last().expect("a run").run;
    assert_ne!(again, write);
    assert_eq!(
        effects,
        vec![round_effect(again, "write", 1, Given::Results(calls.clone()))]
    );
    // It is entered for the results, and for the request it has not dealt with yet.
    let because = vec![changed("request", None, false), Cause::Results { calls }];
    assert_eq!(start_of(&w, again).0, because);
    let _ = request;
}

#[test]
fn a_return_that_asks_and_also_does_the_cells_work_is_sent_back() {
    let (mut w, write, _) = W::requested();
    let mixed = Came {
        dealt_with: None,
        writes: vec![handed("text", None)],
        asks: vec![tool("search")],
        ..Came::default()
    };
    let effects = w.returns(write, mixed);
    assert_eq!(
        effects,
        vec![round_effect(
            write,
            "write",
            2,
            Given::SentBack(vec![Unmet::AsksAndWorks])
        )]
    );
    assert!(w.work.records.called.is_empty());
}

#[test]
fn a_calls_result_is_kept() {
    let (mut w, write, _) = W::requested();
    let effects = w.returns(write, Came::asks(vec![tool("search")]));
    let call = calls_of(&effects)[0].0;
    w.on(Event::Done {
        call,
        result: "found it".to_string(),
    });
    assert_eq!(w.work.results, BTreeMap::from([(call, "found it".to_string())]));
    assert_eq!(
        w.work.records.done.iter().map(|d| d.call).collect::<Vec<_>>(),
        vec![call]
    );
}

#[test]
fn a_run_kept_resting_at_its_calls_from_before_a_return_that_asks_ended_its_run_finishes_making_none_again() {
    let (mut w, write, request) = W::requested();
    let asks = vec![tool("search"), tool("read")];
    let calls: Vec<CallId> = calls_of(&w.returns(write, Came::asks(asks.clone())))
        .iter()
        .map(|(c, ..)| *c)
        .collect();
    // The work as an engine kept it before: the return that asked was taken without ending its run, its calls made
    // after its round ended, and the run rests at them.
    w.work.records.finished.retain(|f| f.run != write);
    w.work.back.insert((write, 1), engine::Back::Asked(asks));
    assert_eq!(w.done(calls[0]), vec![]);
    let effects = w.done(calls[1]);
    // The run finishes with no call made again, and the cell is entered for the results of the calls it made.
    assert_eq!(w.work.records.called.len(), 2);
    let finished = (w.work.records.finished.iter())
        .find(|f| f.run == write)
        .expect("the run finished");
    assert_eq!(finished.dealt_with, Some(vec![]));
    let again = w.work.records.started.last().expect("a run").run;
    assert_ne!(again, write);
    assert_eq!(
        effects,
        vec![round_effect(again, "write", 1, Given::Results(calls.clone()))]
    );
    let because = vec![changed("request", None, false), Cause::Results { calls }];
    assert_eq!(start_of(&w, again).0, because);
    let _ = request;
}

#[test]
fn a_run_kept_resting_at_the_owners_word_at_the_old_limit_of_calls_makes_what_it_asked_for_once_let_go() {
    let (mut w, write, request) = W::requested();
    let asks = vec![tool("search"), tool("read")];
    w.returns(write, Came::asks(asks.clone()));
    // The work as an engine kept it before: the return that asked was taken without ending its run, and the owner's
    // word at the limit of calls was asked for before any of its calls was made.
    let r = &mut w.work.records;
    r.finished.retain(|f| f.run != write);
    r.called.retain(|c| c.run != write);
    w.work.calls.clear();
    w.work.back.insert((write, 1), engine::Back::Asked(asks.clone()));
    let limit = CallId(w.work.name());
    let at = w.work.tick();
    w.work.records.called.push(decide::Called {
        call: limit,
        run: write,
        on: Some(Who::Owner),
        at,
    });
    w.work.calls.insert(limit, Call::AtLimit(Limit::Calls { made: 0 }));
    // Let go, the run finishes, making the calls it asked for.
    let made = calls_of(&w.done(limit));
    assert_eq!(
        made.iter()
            .map(|(_, run, what)| (*run, what.clone()))
            .collect::<Vec<_>>(),
        vec![
            (write, Call::Asked(asks[0].clone())),
            (write, Call::Asked(asks[1].clone()))
        ]
    );
    let finished = (w.work.records.finished.iter())
        .find(|f| f.run == write)
        .expect("the run finished");
    assert_eq!(finished.dealt_with, Some(vec![]));
    // Once both have their results, the cell is entered for them, after the owner's word.
    assert_eq!(w.done(made[0].0), vec![]);
    w.done(made[1].0);
    let again = w.work.records.started.last().expect("a run").run;
    assert_ne!(again, write);
    let calls = vec![limit, made[0].0, made[1].0];
    let because = vec![changed("request", None, false), Cause::Results { calls }];
    assert_eq!(start_of(&w, again).0, because);
    let _ = request;
}
