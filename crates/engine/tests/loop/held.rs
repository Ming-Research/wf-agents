//! A call the host could not carry out for now: it is held, the engine makes a wait of its own for how long the host
//! and the number of times in a row say, and the call is asked for again once the wait has its result.

use decide::run::{FIRST_WAIT, LONGEST_WAIT};
use decide::{Obstacle, Retry};

use super::*;

/// The host could not carry the call out for now, for this reason.
fn held(call: CallId, obstacle: Obstacle, retry: Option<Retry>) -> Event {
    Event::Held { call, obstacle, retry }
}

/// The one call given to be made, the engine's wait, with when it is over.
fn wait_of(effects: &[Effect]) -> (CallId, Time, u32) {
    match calls_of(effects)[..] {
        [(call, _, Call::Wait { until, failed })] => (call, until, failed),
        ref other => panic!("not one wait: {other:?}"),
    }
}

/// A work whose run asked for one tool and finished, so that its cell waits for the call's result.
fn asked() -> (W, RunId, CallId) {
    let (mut w, write, _) = W::requested();
    let effects = w.returns(write, Came::asks(vec![tool("run_tests")]));
    let call = calls_of(&effects)[0].0;
    (w, write, call)
}

#[test]
fn a_call_held_is_not_given_a_result_but_waited_for_and_then_asked_for_again() {
    let (mut w, write, call) = asked();
    let at = w.clock;
    let effects = w.on(held(call, Obstacle::Fault, None));
    let (wait, until, failed) = wait_of(&effects);
    assert_eq!((until, failed), (Time(at + FIRST_WAIT), 1));
    let r = &w.work.records;
    assert!(r.done.is_empty(), "the call has no result");
    assert_eq!(r.held.len(), 1);
    assert_eq!(
        (r.held[0].call, r.held[0].wait, r.held[0].obstacle),
        (call, wait, Obstacle::Fault)
    );
    assert!(r.is_held(call));
    // What was left to be done is the wait alone: the call itself is not asked for until the wait has its result, even
    // by an engine that starts again.
    assert_eq!(pending(&w.flow, &w.work), effects);
    // Once the wait has its result the same call is asked for again, and it is not held now.
    let effects = w.done(wait);
    assert_eq!(
        effects,
        vec![Effect::Call {
            call,
            run: write,
            what: Call::Asked(tool("run_tests")),
        }]
    );
    assert!(!w.work.records.is_held(call));
    assert_eq!(pending(&w.flow, &w.work), effects);
    // The call has its result as any does, and its cell is entered for it, given the result of the call it asked for
    // and none of the wait the engine made.
    let effects = w.done(call);
    assert_eq!(w.work.results.get(&call), Some(&format!("the result of {}", call.0)));
    let again = w.work.records.started.last().expect("a run").run;
    assert_ne!(again, write);
    assert_eq!(
        effects,
        vec![round_effect(again, "write", 1, Given::Results(vec![call]))]
    );
}

#[test]
fn a_call_held_again_waits_twice_as_long_and_never_longer_than_a_round_that_failed_in_passing() {
    let (mut w, _, call) = asked();
    let mut waits = Vec::new();
    for n in 1..=8u32 {
        let at = w.clock;
        let (wait, until, failed) = wait_of(&w.on(held(call, Obstacle::NoRunner, None)));
        assert_eq!(failed, n);
        waits.push(until.0 - at);
        w.done(wait);
    }
    assert_eq!(
        waits,
        [
            FIRST_WAIT,
            2 * FIRST_WAIT,
            4 * FIRST_WAIT,
            8 * FIRST_WAIT,
            16 * FIRST_WAIT,
            LONGEST_WAIT,
            LONGEST_WAIT,
            LONGEST_WAIT
        ]
    );
}

#[test]
fn a_call_held_for_a_limit_waits_until_the_time_the_host_gave() {
    let (mut w, _, call) = asked();
    // In so many seconds: longer than the first wait, so it is the wait.
    let at = w.clock;
    let (wait, until, _) = wait_of(&w.on(held(call, Obstacle::Limit, Some(Retry::In(3600)))));
    assert_eq!(until, Time(at + 3_601_000));
    w.done(wait);
    // At a time in seconds since 1970, which the wait does not end before, and a second after it.
    let at = w.clock;
    let reset = (at + 7_200_000) / 1000;
    let (_, until, failed) = wait_of(&w.on(held(call, Obstacle::Limit, Some(Retry::At(reset)))));
    assert_eq!((until, failed), (Time(reset * 1000 + 1000), 2));
    assert!(until.0 > at + 2 * FIRST_WAIT);
}

#[test]
fn a_call_said_to_be_held_again_while_it_is_held_is_held_once() {
    let (mut w, _, call) = asked();
    w.on(held(call, Obstacle::Fault, None));
    assert_eq!(w.on(held(call, Obstacle::Fault, None)), vec![]);
    assert_eq!(w.work.records.held.len(), 1);
    assert_eq!(w.work.records.called.len(), 2, "the call and its wait");
}

#[test]
fn a_call_that_has_its_result_or_that_is_none_cannot_be_held() {
    let (mut w, _, call) = asked();
    assert_eq!(
        w.refused(held(CallId(99), Obstacle::Fault, None)),
        Refused::NoSuchCall(CallId(99))
    );
    w.done(call);
    assert_eq!(w.refused(held(call, Obstacle::Fault, None)), Refused::AlreadyDone(call));
}

#[test]
fn a_call_held_of_a_work_that_was_cancelled_is_withdrawn_not_asked_for_again() {
    let (mut w, _, call) = asked();
    let (wait, ..) = wait_of(&w.on(held(call, Obstacle::Limit, Some(Retry::In(60)))));
    let effects = w.on(Event::Cancelled { why: None });
    assert!(
        effects
            .iter()
            .all(|e| !matches!(e, Effect::Call { call: c, .. } if *c == call)),
        "{effects:?}"
    );
    // The engine's wait is withdrawn with the call, and a wait that has its result asks for nothing.
    assert!(effects.contains(&Effect::Withdraw { call }), "{effects:?}");
    let effects = w.done(wait);
    assert!(effects.iter().all(|e| !matches!(e, Effect::Call { .. })), "{effects:?}");
}
