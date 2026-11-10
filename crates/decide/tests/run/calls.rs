//! Calls: what a run rests at, whichever round asked.

use super::*;

#[test]
fn a_run_rests_at_its_calls_in_the_order_they_were_asked_for() {
    let rounds = vec![wanting(), asked(&[(9, false), (3, false)])];
    let rest = Step::Rest {
        calls: vec![CallId(9), CallId(3)],
    };
    assert_eq!(step(rounds, &spec()), rest);
}

#[test]
fn a_run_rests_at_its_calls_whatever_its_count_of_returns_sent_back() {
    // Three returns were not met: the third reached the limit of two, and the owner did not let the run go on.
    let rounds = vec![wanting(), wanting(), wanting(), asked(&[(1, false)])];
    let rest = Step::Rest { calls: vec![CallId(1)] };
    assert_eq!(step(rounds, &spec()), rest);
}
