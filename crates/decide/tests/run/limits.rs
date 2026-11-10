//! The limits within a run: how often a return is sent back, and how many calls it makes.

use super::*;

#[test]
fn a_return_is_sent_back_as_often_as_the_limit_allows_and_the_next_unmet_one_reaches_it() {
    let s = spec();
    assert_eq!(step(vec![wanting()], &s), send_back(wanting_unmet()));
    assert_eq!(step(vec![wanting(), wanting()], &s), send_back(wanting_unmet()));
    let limit = Step::AtLimit(Limit::Returns { sent_back: 2 });
    assert_eq!(step(vec![wanting(), wanting(), wanting()], &s), limit);
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
    assert_eq!(step(rounds, &spec()), Step::AtLimit(Limit::Returns { sent_back: 2 }));
}

#[test]
fn a_limit_of_no_returns_sends_none_back() {
    assert_eq!(
        step(vec![wanting()], &limited(0, 3)),
        Step::AtLimit(Limit::Returns { sent_back: 0 })
    );
}

#[test]
fn letting_the_run_go_on_at_its_limit_of_returns_sends_the_return_back_and_starts_the_count_anew() {
    let s = spec();
    // The owner lets it go on at the very return that reached the limit: three rounds had ended.
    let mut log = Log {
        rounds: vec![wanting(), wanting(), wanting()],
        let_go: 3,
    };
    assert_eq!(next(&log, s.requires()), send_back(wanting_unmet()));
    log.rounds.push(wanting());
    assert_eq!(next(&log, s.requires()), send_back(wanting_unmet()));
    log.rounds.push(wanting());
    assert_eq!(next(&log, s.requires()), send_back(wanting_unmet()));
    log.rounds.push(wanting());
    assert_eq!(next(&log, s.requires()), Step::AtLimit(Limit::Returns { sent_back: 2 }));
}

#[test]
fn calls_may_be_made_up_to_the_limit_and_no_further() {
    let s = spec();
    let log = Log {
        rounds: vec![asked_done(1, 2), under_way()],
        let_go: 0,
    };
    assert_eq!(may_call(&log, 1, s.requires()), Ok(()));
    assert_eq!(may_call(&log, 2, s.requires()), Err(Limit::Calls { made: 2 }));
    assert_eq!(may_call(&Log::default(), 3, s.requires()), Ok(()));
    assert_eq!(
        may_call(&Log::default(), 4, s.requires()),
        Err(Limit::Calls { made: 0 })
    );
}

#[test]
fn asking_for_no_calls_is_always_allowed() {
    let s = spec();
    let log = Log {
        rounds: vec![asked_done(1, 3), under_way()],
        let_go: 0,
    };
    assert_eq!(may_call(&log, 0, s.requires()), Ok(()));
}

#[test]
fn calls_are_counted_from_where_the_owner_last_let_the_run_go_on() {
    let s = spec();
    let log = Log {
        rounds: vec![asked_done(1, 3), asked_done(4, 1), under_way()],
        let_go: 1,
    };
    assert_eq!(may_call(&log, 2, s.requires()), Ok(()));
    assert_eq!(may_call(&log, 3, s.requires()), Err(Limit::Calls { made: 1 }));
}

#[test]
fn a_limit_of_no_calls_allows_none() {
    let s = limited(2, 0);
    assert_eq!(
        may_call(&Log::default(), 1, s.requires()),
        Err(Limit::Calls { made: 0 })
    );
    assert_eq!(may_call(&Log::default(), 0, s.requires()), Ok(()));
}

#[test]
#[ignore = "next never reaches the limit of calls: it rests at calls past the limit, which only may_call refuses"]
fn a_round_that_asked_for_more_calls_than_the_limit_allows_reaches_the_limit() {
    // The limit is three calls: the first round made two and the last asks for two more.
    let rounds = vec![asked_done(1, 2), asked(&[(3, false), (4, false)])];
    assert_eq!(step(rounds, &spec()), Step::AtLimit(Limit::Calls { made: 2 }));
}

#[test]
#[ignore = "a round that asks for no calls resumes the run and is counted by no limit, so such rounds repeat without end"]
fn rounds_that_ask_for_nothing_again_and_again_reach_a_limit() {
    // Each thing a run lets repeat has a limit (entry every-way-round-has-a-limit).
    let rounds = vec![asked(&[]); 50];
    assert!(
        matches!(step(rounds, &limited(1, 1)), Step::AtLimit(_)),
        "a run went round fifty times unbounded"
    );
}
