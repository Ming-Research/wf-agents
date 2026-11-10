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
fn a_run_rests_at_exactly_the_calls_its_last_round_asked_for_that_have_no_result() {
    let rounds = vec![asked(&[(1, true), (2, false), (3, true), (4, false)])];
    let rest = Step::Rest {
        calls: vec![CallId(2), CallId(4)],
    };
    assert_eq!(step(rounds, &spec()), rest);
}

#[test]
fn a_run_whose_calls_all_have_their_results_has_its_next_round() {
    assert_eq!(step(vec![asked(&[(1, true), (2, true)])], &spec()), Step::Resume);
}

#[test]
fn only_the_calls_of_the_last_round_are_rested_at() {
    let rounds = vec![asked(&[(1, false)]), asked(&[(2, true)])];
    assert_eq!(step(rounds, &spec()), Step::Resume);
}

#[test]
fn a_round_that_asked_for_no_calls_at_all_leads_to_the_next_round() {
    // Every call it asked for, of which there are none, has its result.
    assert_eq!(step(vec![asked(&[])], &spec()), Step::Resume);
}
