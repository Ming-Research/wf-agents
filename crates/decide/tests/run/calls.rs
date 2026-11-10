//! Calls: which are made, which are rested at, and the limit of how many a run makes.

use super::*;

#[test]
fn a_run_rests_at_its_calls_in_the_order_they_were_made() {
    let rounds = vec![wanting(), made(&[(9, false), (3, false)])];
    let rest = Step::Rest {
        calls: vec![CallId(9), CallId(3)],
    };
    assert_eq!(step(rounds, &spec()), rest);
}

#[test]
fn a_run_rests_at_its_calls_whatever_its_count_of_returns_sent_back() {
    // Three returns were not met: the third reached the limit of two, and the owner did not let the run go on.
    let rounds = vec![wanting(), wanting(), wanting(), made(&[(1, false)])];
    let rest = Step::Rest { calls: vec![CallId(1)] };
    assert_eq!(step(rounds, &spec()), rest);
}

#[test]
fn calls_are_made_up_to_the_limit_and_a_round_asking_past_it_reaches_the_limit() {
    // The cell may make three calls.
    let s = spec();
    assert_eq!(step(vec![asks(3)], &s), Step::Make);
    assert_eq!(step(vec![asks(4)], &s), calls_limit(0));
    assert_eq!(step(vec![asked_done(1, 2), asks(1)], &s), Step::Make);
    assert_eq!(step(vec![asked_done(1, 2), asks(2)], &s), calls_limit(2));
}

#[test]
fn calls_are_counted_from_where_the_owner_last_let_the_run_go_on() {
    let s = spec();
    let rounds = || vec![asked_done(1, 3), asked_done(4, 1), asks(2)];
    assert_eq!(step_let_go(rounds(), 1, &s), Step::Make);
    let rounds = vec![asked_done(1, 3), asked_done(4, 1), asks(3)];
    assert_eq!(step_let_go(rounds, 1, &s), calls_limit(1));
}

#[test]
fn letting_the_run_go_on_at_the_round_that_reached_the_limit_of_calls_makes_its_calls() {
    let s = spec();
    let rounds = || vec![asked_done(1, 3), asks(1)];
    assert_eq!(step(rounds(), &s), calls_limit(3));
    assert_eq!(step_let_go(rounds(), 2, &s), Step::Make);
}

#[test]
fn a_round_asking_for_more_calls_than_the_whole_limit_reaches_it_and_once_let_go_on_its_calls_are_made() {
    let s = spec();
    assert_eq!(step(vec![asks(5)], &s), calls_limit(0));
    assert_eq!(step_let_go(vec![asks(5)], 1, &s), Step::Make);
    // The count starts anew after the round at which the owner let the run go on.
    assert_eq!(step_let_go(vec![asked_done(1, 5), asks(3)], 1, &s), Step::Make);
    assert_eq!(step_let_go(vec![asked_done(1, 5), asks(4)], 1, &s), calls_limit(0));
}

#[test]
fn a_limit_of_no_calls_allows_none() {
    let s = limited(2, 0);
    assert_eq!(step(vec![asks(1)], &s), calls_limit(0));
}
