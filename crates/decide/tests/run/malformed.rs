//! Logs that records cannot hold: what the run does on them anyway, without panicking.

use super::*;

#[test]
fn a_let_go_past_the_rounds_there_are_counts_nothing_and_does_not_panic() {
    let s = spec();
    assert_eq!(step_let_go(vec![wanting()], 10, &s), send_back(wanting_unmet()));
    assert_eq!(step_let_go(vec![asks(5)], 10, &s), Step::Make);
}

#[test]
fn an_earlier_round_with_no_return_does_not_count_as_sent_back_and_does_not_panic() {
    // A round begins only once the one before it has ended, so records never hold this.
    let rounds = vec![under_way(), wanting(), wanting()];
    assert_eq!(step(rounds, &spec()), send_back(wanting_unmet()));
}
