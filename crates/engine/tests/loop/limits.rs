//! The limits, through the loop: of returns within a run, at which the owner is called, and once the owner lets the run
//! go on what was held is taken up and the count starts anew; of ways round and of runs that only ask, at which the
//! work waits at the cell until the owner lets it go on. A cell the work does not wait at past its limit is not let go
//! on: the let-go is refused, and the count of its limit runs on.

use super::*;

/// The owner's call at the limit, which the effects must be alone.
fn at_limit(effects: &[Effect], run: RunId, limit: Limit) -> CallId {
    match effects {
        [Effect::Call { call, run: r, what }] if *r == run && *what == Call::AtLimit(limit.clone()) => *call,
        other => panic!("not the owner's call at {limit:?}: {other:?}"),
    }
}

#[test]
fn returns_are_sent_back_up_to_the_limit_then_the_owner_is_called_and_once_let_go_on_the_return_is_sent_back() {
    let (mut w, write, _) = W::requested();
    assert_eq!(
        w.returns(write, wanting()),
        vec![round_effect(write, "write", 2, Given::SentBack(wanting_unmet()))]
    );
    assert_eq!(
        w.returns(write, wanting()),
        vec![round_effect(write, "write", 3, Given::SentBack(wanting_unmet()))]
    );
    let effects = w.returns(write, wanting());
    let call = at_limit(&effects, write, Limit::Returns { sent_back: 2 });
    let called = w.work.records.called.last().expect("a call").clone();
    assert_eq!((called.call, called.run, called.on), (call, write, Some(Who::Owner)));
    // The owner lets the run go on: the return that reached the limit is sent back, and the count starts anew.
    assert_eq!(
        w.done(call),
        vec![round_effect(write, "write", 4, Given::SentBack(wanting_unmet()))]
    );
    assert_eq!(
        w.returns(write, wanting()),
        vec![round_effect(write, "write", 5, Given::SentBack(wanting_unmet()))]
    );
    assert_eq!(
        w.returns(write, wanting()),
        vec![round_effect(write, "write", 6, Given::SentBack(wanting_unmet()))]
    );
    at_limit(&w.returns(write, wanting()), write, Limit::Returns { sent_back: 2 });
}

/// The writer, entered for its request, asks for a call in each of its runs, and is entered again once the call has its
/// result, until the work waits at it: how many runs it asked in, from this run.
fn asking_until_it_waits(w: &mut W, mut write: RunId) -> usize {
    let mut runs = 0;
    loop {
        let calls = calls_of(&w.returns(write, Came::asks(vec![tool("a")])));
        runs += 1;
        let effects = w.done(calls[0].0);
        if effects.is_empty() {
            return runs;
        }
        let (run, cell, number, given) = round(&effects);
        assert_eq!((cell.as_str(), number), ("write", 1));
        assert_eq!(given, Given::Results(vec![calls[0].0]));
        write = run;
    }
}

#[test]
fn a_cell_entered_again_and_again_only_to_ask_waits_for_the_owner_at_its_limit_of_calls_and_runs_once_let_go_on() {
    let (mut w, write, _) = W::requested();
    // The flow lets a cell be entered again for results after up to three runs in a row that only asked.
    assert_eq!(asking_until_it_waits(&mut w, write), 3);
    let waits = decide::Decision::Wait {
        at: "write".into(),
        on: vec![decide::Waited::Calls { runs: 3 }],
    };
    assert_eq!(w.decision(), waits);
    let effects = w.on(Event::LetGo { cell: "write".into() });
    let (run, cell, number, given) = round(&effects);
    assert_eq!((cell.as_str(), number), ("write", 1));
    assert!(matches!(given, Given::Results(_)));
    // The count starts anew from there.
    assert_eq!(asking_until_it_waits(&mut w, run), 3);
}

/// The writer hands in and the judge writes an opinion for it, so that the writer goes round, this many times or until
/// the work waits at the writer; the writer's run whose round is under way, where one is.
fn going_round(w: &mut W, mut write: RunId, times: usize) -> Option<RunId> {
    for _ in 0..times {
        let judge = round(&w.returns(write, hands_in(&[("text", None)]))).0;
        let effects = w.returns(judge, hands_in(&[("opinions", None)]));
        if effects.is_empty() {
            return None;
        }
        write = round(&effects).0;
    }
    Some(write)
}

/// How many of the writer's runs were ways round: none of their reasons a change that people, the outside or a cell
/// before it made.
fn ways_round(w: &W) -> usize {
    (w.work.records.started.iter())
        .filter(|s| s.cell.as_str() == "write" && s.because.iter().all(|c| !c.forward()))
        .count()
}

#[test]
fn a_cell_gone_round_to_its_limit_waits_for_the_owner_and_runs_once_let_go_on() {
    let (mut w, write, _) = W::requested();
    // Three ways round are allowed.
    assert_eq!(going_round(&mut w, write, 4), None);
    assert_eq!(ways_round(&w), 3);
    let waits = decide::Decision::Wait {
        at: "write".into(),
        on: vec![decide::Waited::Limit { runs: 3 }],
    };
    assert_eq!(w.decision(), waits);
    let effects = w.on(Event::LetGo { cell: "write".into() });
    let (run, cell, number, given) = round(&effects);
    assert_eq!((cell.as_str(), number, given), ("write", 1, Given::Task));
    assert!(start_of(&w, run).0.iter().all(|c| !c.forward()));
    let let_go: Vec<&str> = w.work.records.let_go.iter().map(|l| l.cell.as_str()).collect();
    assert_eq!(let_go, ["write"]);
}

#[test]
fn a_cell_not_at_its_limit_is_not_let_go_on_and_its_count_runs_on() {
    let (mut w, write, _) = W::requested();
    // Before the writer has run, and after it has gone round twice, one way round short of its limit: the let-go is
    // refused and leaves the work as it was.
    assert_eq!(
        w.refused(Event::LetGo { cell: "write".into() }),
        Refused::NotAtItsLimit("write".into())
    );
    let write = going_round(&mut w, write, 2).expect("the writer runs again");
    assert_eq!(ways_round(&w), 2);
    assert_eq!(
        w.refused(Event::LetGo { cell: "write".into() }),
        Refused::NotAtItsLimit("write".into())
    );
    // The count was not started anew: one more way round, and the work waits at the writer.
    assert_eq!(going_round(&mut w, write, 2), None);
    assert_eq!(ways_round(&w), 3);
    // Waiting there, a cell it does not wait at is not let go on either; the writer is.
    assert_eq!(
        w.refused(Event::LetGo { cell: "judge".into() }),
        Refused::NotAtItsLimit("judge".into())
    );
    assert_eq!(w.work.records.let_go, vec![]);
    let (_, cell, ..) = round(&w.on(Event::LetGo { cell: "write".into() }));
    assert_eq!(cell, "write");
}

#[test]
fn the_owners_call_at_a_limit_is_withdrawn_when_its_run_is_abandoned() {
    let (mut w, write, _) = W::requested();
    w.returns(write, wanting());
    w.returns(write, wanting());
    let call = calls_of(&w.returns(write, wanting()))[0].0;
    // A new request leaves the run at rest at the owner's call: what it was not told of stays new.
    let (effects, _) = w.person_writes("request");
    assert_eq!(effects, vec![]);
    assert_eq!(w.on(Event::Cancelled { why: None }), vec![Effect::Withdraw { call }]);
}
