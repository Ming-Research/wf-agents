//! Rounds: the first, one under way, and what a round that asked for calls leads to.

use super::*;

#[test]
fn a_run_with_no_round_yet_has_its_first_round() {
    assert_eq!(step(vec![], &spec()), Step::Begin);
}

#[test]
fn while_a_round_is_under_way_nothing_is_decided() {
    assert_eq!(step(vec![under_way()], &spec()), Step::UnderWay);
    assert_eq!(step(vec![asked_done(1, 1), under_way()], &spec()), Step::UnderWay);
}

#[test]
fn the_calls_a_round_asked_for_are_made_when_none_was_made_yet() {
    assert_eq!(step(vec![asks(2)], &spec()), Step::Make);
}

#[test]
fn a_run_rests_at_exactly_the_calls_made_for_its_last_round_that_have_no_result() {
    let rounds = vec![made(&[(1, true), (2, false), (3, true), (4, false)])];
    let rest = Step::Rest {
        calls: vec![CallId(2), CallId(4)],
    };
    assert_eq!(step(rounds, &spec()), rest);
}

#[test]
fn a_run_whose_calls_all_have_their_results_has_its_next_round() {
    assert_eq!(step(vec![made(&[(1, true), (2, true)])], &spec()), Step::Resume);
}

#[test]
fn only_the_calls_of_the_last_round_are_rested_at() {
    let rounds = vec![made(&[(1, false)]), made(&[(2, true)])];
    assert_eq!(step(rounds, &spec()), Step::Resume);
}

#[test]
fn a_round_that_asked_for_no_calls_at_all_is_sent_back_as_asking_nothing() {
    assert_eq!(step(vec![asks_nothing()], &spec()), send_back(vec![Unmet::AsksNothing]));
}
