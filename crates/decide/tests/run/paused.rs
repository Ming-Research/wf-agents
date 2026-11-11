//! A pause of the work: what would begin a round is held, and nothing else is.

use super::*;

#[test]
fn a_paused_run_with_no_round_yet_is_held_from_its_first_round() {
    assert_eq!(step_paused(vec![], &spec()), Step::Held);
}

#[test]
fn a_paused_run_whose_calls_all_have_their_results_is_held_from_its_next_round() {
    assert_eq!(step_paused(vec![asked_done(1, 2)], &spec()), Step::Held);
}

#[test]
fn a_paused_run_whose_return_is_sent_back_is_held_from_the_round_that_would_say_why() {
    assert_eq!(step_paused(vec![wanting()], &spec()), Step::Held);
    assert_eq!(step_paused(vec![asks_nothing()], &spec()), Step::Held);
}

#[test]
fn a_pause_holds_back_nothing_that_begins_no_round() {
    let s = spec();
    assert_eq!(step_paused(vec![under_way()], &s), Step::UnderWay);
    assert_eq!(step_paused(vec![asks(2)], &s), Step::Make);
    let rest = Step::Rest { calls: vec![CallId(1)] };
    assert_eq!(step_paused(vec![made(&[(1, false)])], &s), rest);
    let finish = Step::Finish {
        writes: vec![prose("plan")],
    };
    assert_eq!(step_paused(vec![handed_in(vec![prose("plan")])], &s), finish);
    assert_eq!(step_paused(vec![asks(4)], &s), calls_limit(0));
    assert_eq!(step_paused(vec![wanting(), wanting(), wanting()], &s), returns_limit(2));
}

#[test]
fn a_paused_run_let_go_on_at_its_limit_is_held_from_the_round_that_sends_its_return_back() {
    let log = Log {
        rounds: vec![wanting(), wanting(), wanting()],
        let_go: 3,
        let_on: 0,
    };
    assert_eq!(next(&log, spec().requires(), Hold::Rounds), Step::Held);
    let log = Log {
        rounds: vec![asked_done(1, 3), asks(1)],
        let_go: 2,
        let_on: 0,
    };
    assert_eq!(next(&log, spec().requires(), Hold::Rounds), Step::Make);
}
