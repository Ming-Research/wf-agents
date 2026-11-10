//! The limits within a run, through the loop: the owner is called, and once the owner lets the run go on what was
//! held is taken up and the count starts anew.

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

#[test]
fn calls_are_made_up_to_the_limit_then_the_owner_is_called_and_once_let_go_on_the_calls_asked_for_are_made() {
    let (mut w, write, _) = W::requested();
    let first = calls_of(&w.returns(write, Came::Asks(vec![tool("a"), tool("b")])));
    w.done(first[0].0);
    let effects = w.done(first[1].0);
    assert_eq!(
        effects,
        vec![round_effect(
            write,
            "write",
            2,
            Given::Results(vec![first[0].0, first[1].0])
        )]
    );
    let effects = w.returns(write, Came::Asks(vec![tool("c"), tool("d")]));
    let call = at_limit(&effects, write, Limit::Calls { made: 2 });
    let made = calls_of(&w.done(call));
    assert_eq!(
        made.iter()
            .map(|(_, run, what)| (*run, what.clone()))
            .collect::<Vec<_>>(),
        vec![(write, Call::Asked(tool("c"))), (write, Call::Asked(tool("d")))]
    );
}

#[test]
fn a_cell_gone_round_to_its_limit_waits_for_the_owner_and_runs_once_let_go_on() {
    let (mut w, mut write, _) = W::requested();
    // The writer hands in, the judge writes an opinion for it, and so round: three ways round are allowed.
    for _ in 0..4 {
        let judge = round(&w.returns(write, hands_in(&[("text", None)]))).0;
        let effects = w.returns(judge, hands_in(&[("opinions", None)]));
        if effects.is_empty() {
            break;
        }
        write = round(&effects).0;
    }
    let ways_round = (w.work.records.started.iter())
        .filter(|s| s.cell.as_str() == "write" && s.because.iter().all(|c| !c.forward()))
        .count();
    assert_eq!(ways_round, 3);
    let effects = w.on(Event::LetGo { cell: "write".into() });
    let (_, cell, number, given) = round(&effects);
    assert_eq!((cell.as_str(), number), ("write", 1));
    assert!(matches!(given, Given::Task { because, .. } if because.iter().all(|c| !c.forward())));
}
