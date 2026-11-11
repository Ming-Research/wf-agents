//! A round that failed: nothing came back, for a kind of cause. A failure in passing waits, as a call the engine
//! makes, before the round is tried again, and so many in a row pause the work; what could not be read is sent back;
//! any other failure has the round again, given what the failed one was given, which a pause holds back as it holds
//! back any round. Only a return that could not be read counts against a limit. Whether the engine pauses the work as
//! a round fails is said by `pausing`.

use decide::run::{FIRST_WAIT, LONGEST_WAIT, PASSING, Pausing, pausing, wait};

use super::*;

/// A round that failed for a cause no kind tells apart, which pauses the work.
fn failed() -> Round {
    failed_as(Failure::Other)
}

/// A log of these rounds, the work having been let on when this many had ended.
fn let_on(rounds: Vec<Round>, let_on: usize) -> Log {
    Log {
        rounds,
        let_go: 0,
        let_on,
    }
}

/// So many rounds in a row that failed in passing, each tried again after its wait.
fn passing(n: usize) -> Vec<Round> {
    (0..n)
        .map(|i| waited_after(Failure::Passing, &[(i as u64 + 1, true)]))
        .collect()
}

#[test]
fn a_run_whose_round_failed_has_that_round_again() {
    assert_eq!(step(vec![failed()], &spec()), Step::Retry);
    assert_eq!(step(vec![asked_done(1, 2), failed()], &spec()), Step::Retry);
    for kind in [
        Failure::Flagged,
        Failure::UsageLimit,
        Failure::CannotRun,
        Failure::Offline,
        Failure::PromptTooLong,
    ] {
        assert_eq!(step(vec![failed_as(kind)], &spec()), Step::Retry, "{kind:?}");
    }
}

#[test]
fn a_run_whose_round_failed_is_held_while_its_work_is_paused() {
    assert_eq!(step_paused(vec![failed()], &spec()), Step::Held);
}

#[test]
fn failed_rounds_count_against_no_limit() {
    // However many rounds failed, none was a return sent back nor made a call.
    let s = limited(1, 2);
    let failures = || vec![failed(), failed(), failed(), failed()];
    assert_eq!(step([failures(), vec![failed()]].concat(), &s), Step::Retry);
    assert_eq!(
        step([failures(), vec![wanting()]].concat(), &s),
        send_back(wanting_unmet())
    );
    assert_eq!(step([failures(), vec![asks(2)]].concat(), &s), Step::Make);
    let between = vec![wanting(), failed(), failed(), wanting()];
    assert_eq!(step(between, &s), returns_limit(1));
}

#[test]
fn a_round_that_failed_in_passing_waits_longer_each_time_before_it_is_tried_again() {
    let s = spec();
    assert_eq!(
        step(vec![failed_as(Failure::Passing)], &s),
        Step::Wait {
            failed: 1,
            ms: FIRST_WAIT
        }
    );
    let third = [passing(2), vec![failed_as(Failure::Passing)]].concat();
    assert_eq!(
        step(third, &s),
        Step::Wait {
            failed: 3,
            ms: 4 * FIRST_WAIT
        }
    );
    assert_eq!(wait(1), 10_000);
    assert_eq!(wait(2), 20_000);
    assert_eq!(wait(5), 160_000);
    assert_eq!(wait(6), LONGEST_WAIT);
    assert_eq!(wait(PASSING), LONGEST_WAIT);
}

#[test]
fn a_round_that_failed_in_passing_rests_at_its_wait_and_is_tried_again_once_it_is_over() {
    let s = spec();
    let resting = waited_after(Failure::Passing, &[(7, false)]);
    assert_eq!(step(vec![resting], &s), Step::Rest { calls: vec![CallId(7)] });
    let over = waited_after(Failure::Passing, &[(7, true)]);
    assert_eq!(step(vec![over.clone()], &s), Step::Retry);
    // The wait is no round: a pause holds back the round after it, and not the wait.
    assert_eq!(
        step_paused(vec![failed_as(Failure::Passing)], &s),
        Step::Wait {
            failed: 1,
            ms: FIRST_WAIT
        }
    );
    assert_eq!(step_paused(vec![over], &s), Step::Held);
}

#[test]
fn a_failure_in_passing_counts_only_those_in_a_row_since_the_work_was_let_on() {
    let s = spec();
    let first = Step::Wait {
        failed: 1,
        ms: FIRST_WAIT,
    };
    // A round that did not fail ends the row.
    let after_a_return = [passing(3), vec![asked_done(9, 1), failed_as(Failure::Passing)]].concat();
    assert_eq!(step(after_a_return, &s), first);
    // A failure of another kind between is in the row, and counts nothing.
    let other_between = [
        passing(1),
        vec![failed_as(Failure::UsageLimit), failed_as(Failure::Passing)],
    ]
    .concat();
    assert_eq!(
        step(other_between, &s),
        Step::Wait {
            failed: 2,
            ms: 2 * FIRST_WAIT
        }
    );
    // The work let on after the round tries it again at once, and counts anew from there.
    let rounds = [passing(PASSING as usize), vec![failed_as(Failure::Passing)]].concat();
    let ended = rounds.len();
    assert_eq!(
        next(&let_on(rounds.clone(), ended), s.requires(), Hold::Nothing),
        Step::Retry
    );
    let again = [rounds, vec![failed_as(Failure::Passing)]].concat();
    assert_eq!(next(&let_on(again, ended), s.requires(), Hold::Nothing), first);
}

#[test]
fn so_many_failures_in_passing_in_a_row_pause_the_work_and_the_round_waits_no_more() {
    let s = spec();
    let within = [passing(PASSING as usize - 1), vec![failed_as(Failure::Passing)]].concat();
    assert_eq!(pausing(&let_on(within.clone(), 0)), None);
    assert!(matches!(step(within, &s), Step::Wait { failed, .. } if failed == PASSING));
    let past = [passing(PASSING as usize), vec![failed_as(Failure::Passing)]].concat();
    assert_eq!(
        pausing(&let_on(past.clone(), 0)),
        Some(Pausing::Passing { failed: PASSING + 1 })
    );
    // The pause holds the round back; once let on, it is tried at once.
    assert_eq!(step_paused(past.clone(), &s), Step::Held);
    assert_eq!(step(past, &s), Step::Retry);
}

#[test]
fn what_could_not_be_read_is_sent_back_and_counts_as_a_return_sent_back() {
    let s = limited(1, 3);
    let unread = || failed_as(Failure::Unread);
    let unmet = vec![Unmet::Unreadable {
        why: "it failed".to_string(),
    }];
    assert_eq!(step(vec![unread()], &s), send_back(unmet));
    assert_eq!(step(vec![wanting(), unread()], &s), returns_limit(1));
    assert_eq!(step(vec![unread(), wanting()], &s), returns_limit(1));
    assert_eq!(pausing(&let_on(vec![unread()], 0)), None);
    assert_eq!(step_paused(vec![unread()], &s), Step::Held);
}

#[test]
fn a_flagged_round_is_tried_again_once_and_flagged_again_pauses_the_work() {
    let s = spec();
    let flagged = || failed_as(Failure::Flagged);
    assert_eq!(pausing(&let_on(vec![flagged()], 0)), None);
    assert_eq!(step(vec![flagged()], &s), Step::Retry);
    assert_eq!(
        pausing(&let_on(vec![flagged(), flagged()], 0)),
        Some(Pausing::FlaggedAgain)
    );
    // Whatever failed between, in passing or for a cause every work shares.
    let between = [
        vec![flagged()],
        passing(2),
        vec![failed_as(Failure::Offline), flagged()],
    ]
    .concat();
    assert_eq!(pausing(&let_on(between, 0)), Some(Pausing::FlaggedAgain));
    // A round that came back between, or the work let on between, starts anew.
    assert_eq!(pausing(&let_on(vec![flagged(), asked_done(1, 1), flagged()], 0)), None);
    assert_eq!(pausing(&let_on(vec![flagged(), flagged()], 1)), None);
}

#[test]
fn a_failure_running_again_cannot_help_pauses_the_work_and_one_every_work_shares_does_not() {
    for kind in [Failure::PromptTooLong, Failure::Other] {
        assert_eq!(pausing(&let_on(vec![failed_as(kind)], 0)), Some(Pausing::Failed(kind)));
    }
    for kind in [Failure::UsageLimit, Failure::CannotRun, Failure::Offline] {
        assert!(kind.shared(), "{kind:?}");
        assert_eq!(pausing(&let_on(vec![failed_as(kind)], 0)), None, "{kind:?}");
    }
    assert!(!Failure::Passing.shared() && !Failure::Flagged.shared() && !Failure::Other.shared());
    // A log whose last round did not fail pauses nothing.
    assert_eq!(pausing(&let_on(vec![asked_done(1, 1)], 0)), None);
    assert_eq!(pausing(&let_on(vec![], 0)), None);
}
