//! Calls held beside one another: each held call has its own wait and is asked for again by that wait alone, a call
//! that is not held goes on beside it, and the cell is entered once, for every call it asked for and for none of the
//! engine's waits. The reference is the design tree (decision `a-call-the-host-cannot-carry-out-for-now-is-held-and-asked-again`
//! and `a-tools-result-is-no-datum`): a call is asked again once its own wait has its result, and what the cell is
//! given is the results of the calls it asked for.

use decide::run::FIRST_WAIT;
use decide::{Obstacle, Retry};

use super::*;

fn held(call: CallId, obstacle: Obstacle, retry: Option<Retry>) -> Event {
    Event::Held { call, obstacle, retry }
}

/// A work whose run asked for two tools in one return and finished, so that its cell waits for both results.
fn asked_twice() -> (W, RunId, CallId, CallId) {
    let (mut w, write, _) = W::requested();
    let effects = w.returns(write, Came::asks(vec![tool("run_tests"), tool("run_tests")]));
    let calls = calls_of(&effects);
    assert_eq!(calls.len(), 2, "{effects:?}");
    (w, write, calls[0].0, calls[1].0)
}

fn wait_made(effects: &[Effect]) -> (CallId, Time) {
    match calls_of(effects)[..] {
        [(call, _, Call::Wait { until, .. })] => (call, until),
        ref other => panic!("not one wait: {other:?}"),
    }
}

#[test]
fn two_calls_held_together_are_each_asked_for_again_by_their_own_wait_alone() {
    let (mut w, write, first, second) = asked_twice();
    let (wait_first, _) = wait_made(&w.on(held(first, Obstacle::Limit, Some(Retry::In(60)))));
    let (wait_second, _) = wait_made(&w.on(held(second, Obstacle::Fault, None)));
    assert_ne!(wait_first, wait_second);
    assert!(w.work.records.is_held(first) && w.work.records.is_held(second));
    // Neither call is asked for while its wait is open, and a restart sees the two waits and nothing else.
    let waits = pending(&w.flow, &w.work);
    assert!(
        waits.iter().all(|e| matches!(
            e,
            Effect::Call {
                what: Call::Wait { .. },
                ..
            }
        )) && waits.len() == 2,
        "{waits:?}"
    );
    // The second's wait ends: the second alone is asked for again, the first stays held.
    let effects = w.done(wait_second);
    assert_eq!(
        effects,
        vec![Effect::Call {
            call: second,
            run: write,
            what: Call::Asked(tool("run_tests")),
        }]
    );
    assert!(w.work.records.is_held(first) && !w.work.records.is_held(second));
    // What a restart now gives: the first's wait and the second call, no more.
    let left = pending(&w.flow, &w.work);
    assert_eq!(left.len(), 2, "{left:?}");
    assert!(left.contains(&effects[0]), "{left:?}");
    // The second has its result while the first is still held: the cell is not entered.
    assert_eq!(w.done(second), vec![]);
    // The first's wait ends, the first is asked for again, its result enters the cell once, given both results and none
    // of the waits.
    let effects = w.done(wait_first);
    assert_eq!(
        effects,
        vec![Effect::Call {
            call: first,
            run: write,
            what: Call::Asked(tool("run_tests")),
        }]
    );
    let effects = w.done(first);
    let (_, cell, _, given) = round(&effects);
    assert_eq!(cell, "write");
    match given {
        Given::Results(mut calls) => {
            calls.sort();
            let mut both = vec![first, second];
            both.sort();
            assert_eq!(calls, both);
        }
        other => panic!("given {other:?}"),
    }
}

#[test]
fn a_reset_time_already_past_waits_as_long_as_the_backoff_and_a_word_of_forever_does_not_overflow() {
    let (mut w, _, call, other) = asked_twice();
    let at = w.clock;
    // A reset the host named that was a minute ago is not a wait in the past: the call waits the first backoff.
    let past = (at - 60_000) / 1000;
    let (wait, until) = wait_made(&w.on(held(call, Obstacle::Limit, Some(Retry::At(past)))));
    assert_eq!(until, Time(at + FIRST_WAIT));
    w.done(wait);
    // A host that says to wait for the largest number of seconds there is is taken in, and waits for a very long time.
    let (_, until) = wait_made(&w.on(held(other, Obstacle::Limit, Some(Retry::In(u64::MAX)))));
    assert!(until.0 > at + 365 * 24 * 3600 * 1000, "{until:?}");
}
