//! The limit of how often what a run returns is sent back.

use super::*;

#[test]
fn a_return_is_sent_back_as_often_as_the_limit_allows_and_the_next_unmet_one_reaches_it() {
    let s = spec();
    assert_eq!(step(vec![wanting()], &s), send_back(wanting_unmet()));
    assert_eq!(step(vec![wanting(), wanting()], &s), send_back(wanting_unmet()));
    assert_eq!(step(vec![wanting(), wanting(), wanting()], &s), returns_limit(2));
}

#[test]
fn a_return_that_meets_everything_finishes_the_run_though_the_limit_of_returns_is_reached() {
    let rounds = vec![wanting(), wanting(), handed_in(vec![prose("plan")])];
    assert_eq!(
        step(rounds, &spec()),
        Step::Finish {
            writes: vec![prose("plan")]
        }
    );
}

#[test]
fn rounds_that_asked_for_calls_between_two_returns_are_not_counted_as_sent_back() {
    let rounds = vec![wanting(), asked_done(1, 1), asked_done(2, 1), wanting()];
    assert_eq!(step(rounds, &spec()), send_back(wanting_unmet()));
    let rounds = vec![wanting(), asked_done(1, 1), wanting(), asked_done(2, 1), wanting()];
    assert_eq!(step(rounds, &spec()), returns_limit(2));
}

#[test]
fn rounds_that_ask_for_nothing_are_sent_back_and_reach_the_limit_of_returns() {
    let s = spec();
    assert_eq!(
        step(vec![asks_nothing(), asks_nothing()], &s),
        send_back(vec![Unmet::AsksNothing])
    );
    assert_eq!(
        step(vec![asks_nothing(), asks_nothing(), asks_nothing()], &s),
        returns_limit(2)
    );
}

#[test]
fn a_return_handed_in_unmet_and_a_round_asking_nothing_are_counted_together_as_sent_back() {
    let s = spec();
    assert_eq!(
        step(vec![wanting(), asks_nothing()], &s),
        send_back(vec![Unmet::AsksNothing])
    );
    assert_eq!(step(vec![wanting(), asks_nothing(), wanting()], &s), returns_limit(2));
    assert_eq!(
        step(vec![asks_nothing(), wanting(), asks_nothing()], &s),
        returns_limit(2)
    );
}

#[test]
fn a_limit_of_no_returns_sends_none_back() {
    let s = limited(0, 3);
    assert_eq!(step(vec![wanting()], &s), returns_limit(0));
    assert_eq!(step(vec![asks_nothing()], &s), returns_limit(0));
}

#[test]
fn letting_the_run_go_on_at_its_limit_of_returns_sends_the_return_back_and_starts_the_count_anew() {
    let s = spec();
    // The owner lets it go on at the very return that reached the limit: three rounds had ended.
    let mut log = Log {
        rounds: vec![wanting(), wanting(), wanting()],
        let_go: 3,
    };
    assert_eq!(next(&log, s.requires(), Hold::Nothing), send_back(wanting_unmet()));
    log.rounds.push(wanting());
    assert_eq!(next(&log, s.requires(), Hold::Nothing), send_back(wanting_unmet()));
    log.rounds.push(wanting());
    assert_eq!(next(&log, s.requires(), Hold::Nothing), send_back(wanting_unmet()));
    log.rounds.push(wanting());
    assert_eq!(next(&log, s.requires(), Hold::Nothing), returns_limit(2));
}

#[test]
fn letting_the_run_go_on_at_a_round_asking_nothing_that_reached_the_limit_sends_it_back() {
    let rounds = vec![asks_nothing(), asks_nothing(), asks_nothing()];
    assert_eq!(step_let_go(rounds, 3, &spec()), send_back(vec![Unmet::AsksNothing]));
}
