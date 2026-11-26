//! A round that failed: nothing came back, for a kind of cause. A failure in passing waits, as a call the engine
//! makes, before the round is tried again, and so many in a row pause the work; what could not be read is sent back;
//! any other failure has the round again, given what the failed one was given, which a pause holds back as it holds
//! back any round. Only a return that could not be read counts against a limit. Whether the engine pauses the work as
//! a round fails is said by `pausing`.

use decide::run::{FIRST_WAIT, LONGEST_WAIT, PASSING, Pausing, pausing, wait};
use decide::{Hindrance, Obstacle, Retry};

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
    assert_eq!(step(vec![wanting(), failed()], &spec()), Step::Retry);
    for kind in [
        Failure::Flagged,
        Failure::UsageLimit,
        Failure::CannotRun,
        Failure::Offline,
        Failure::Unavailable,
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
    // However many rounds failed, none was a return sent back.
    let s = limited(1);
    let failures = || vec![failed(), failed(), failed(), failed()];
    assert_eq!(step([failures(), vec![failed()]].concat(), &s), Step::Retry);
    assert_eq!(
        step([failures(), vec![wanting()]].concat(), &s),
        send_back(wanting_unmet())
    );
    assert_eq!(step([failures(), vec![asks(2)]].concat(), &s), finish_asking());
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
            ms: FIRST_WAIT,
            not_before: None
        }
    );
    let third = [passing(2), vec![failed_as(Failure::Passing)]].concat();
    assert_eq!(
        step(third, &s),
        Step::Wait {
            failed: 3,
            ms: 4 * FIRST_WAIT,
            not_before: None
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
            ms: FIRST_WAIT,
            not_before: None
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
        not_before: None,
    };
    // A round that did not fail ends the row.
    let after_a_return = [passing(3), vec![wanting(), failed_as(Failure::Passing)]].concat();
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
            ms: 2 * FIRST_WAIT,
            not_before: None
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
    let s = limited(1);
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
    assert_eq!(pausing(&let_on(vec![flagged(), wanting(), flagged()], 0)), None);
    assert_eq!(pausing(&let_on(vec![flagged(), flagged()], 1)), None);
}

#[test]
fn a_round_the_run_guard_stopped_is_tried_again_once_and_stopped_again_pauses_the_work() {
    let s = spec();
    let guarded = || failed_as(Failure::Guarded);
    assert_eq!(pausing(&let_on(vec![guarded()], 0)), None);
    assert_eq!(step(vec![guarded()], &s), Step::Retry);
    assert_eq!(
        pausing(&let_on(vec![guarded(), guarded()], 0)),
        Some(Pausing::GuardedAgain)
    );
    // A flag between is another cause, which does not count as the guard stopping it again.
    assert_eq!(pausing(&let_on(vec![guarded(), failed_as(Failure::Flagged)], 0)), None);
    assert_eq!(pausing(&let_on(vec![guarded(), guarded()], 1)), None, "let on between");
}

#[test]
fn a_failure_running_again_cannot_help_pauses_the_work_and_one_every_work_shares_does_not() {
    for kind in [Failure::PromptTooLong, Failure::Other] {
        assert_eq!(pausing(&let_on(vec![failed_as(kind)], 0)), Some(Pausing::Failed(kind)));
    }
    for kind in [
        Failure::UsageLimit,
        Failure::CannotRun,
        Failure::Offline,
        Failure::Unavailable,
    ] {
        assert!(kind.shared(), "{kind:?}");
        assert_eq!(pausing(&let_on(vec![failed_as(kind)], 0)), None, "{kind:?}");
    }
    assert!(!Failure::Passing.shared() && !Failure::Flagged.shared() && !Failure::Other.shared());
    // A log whose last round did not fail pauses nothing.
    assert_eq!(pausing(&let_on(vec![wanting()], 0)), None);
    assert_eq!(pausing(&let_on(vec![], 0)), None);
}

// A round after one that failed for a cause outside every work begins only once the engine found the outside well: so
// many of them in a row say the rounds meet what the check does not find, and the next pauses the work. Kinds of such
// causes count together, a failure in passing between does not break the row, and the work let on starts it anew.
#[test]
fn so_many_failures_in_a_row_for_a_cause_outside_every_work_pause_the_work() {
    let s = spec();
    let outside = |n: usize| -> Vec<Round> {
        let kinds = [
            Failure::Unavailable,
            Failure::UsageLimit,
            Failure::CannotRun,
            Failure::Offline,
        ];
        (0..n).map(|i| failed_as(kinds[i % kinds.len()])).collect()
    };
    let within = outside(PASSING as usize);
    assert_eq!(pausing(&let_on(within.clone(), 0)), None);
    assert_eq!(step(within.clone(), &s), Step::Retry);
    let past = outside(PASSING as usize + 1);
    assert_eq!(
        pausing(&let_on(past.clone(), 0)),
        Some(Pausing::Outside { failed: PASSING + 1 })
    );
    let between = [outside(PASSING as usize), passing(1), outside(1)].concat();
    assert_eq!(
        pausing(&let_on(between, 0)),
        Some(Pausing::Outside { failed: PASSING + 1 })
    );
    let ended = past.len();
    let again = [past.clone(), outside(1)].concat();
    assert_eq!(pausing(&let_on(again, ended)), None, "let on between");
    // A round that came back between starts the row anew.
    let back = [outside(PASSING as usize), vec![wanting()], outside(1)].concat();
    assert_eq!(pausing(&let_on(back, 0)), None);
}

/// What keeps the host from an act: its limit on requests, naming when it may be asked again as given.
fn limit(retry: Option<Retry>) -> Hindrance {
    Hindrance {
        obstacle: Obstacle::Limit,
        retry,
    }
}

/// So many rounds in a row that the host kept from running for now, each tried again after its wait.
fn held(n: usize, kept: Hindrance) -> Vec<Round> {
    (0..n).map(|i| kept_by(kept, &[(i as u64 + 101, true)])).collect()
}

#[test]
fn a_round_the_host_kept_from_running_waits_for_the_host_however_many_times_and_never_pauses_the_work() {
    let s = spec();
    let hour = limit(Some(Retry::In(3600)));
    for n in [1, 2, PASSING as usize, PASSING as usize + 1, 20, 50] {
        let rounds = [held(n - 1, hour), vec![kept_by(hour, &[])]].concat();
        assert_eq!(pausing(&let_on(rounds.clone(), 0)), None, "{n} in a row");
        assert_eq!(
            step(rounds, &s),
            Step::Wait {
                failed: n as u32,
                ms: 3_601_000_u64.max(wait(n as u32)),
                not_before: None
            },
            "{n} in a row: the host's hour, and a second more, is longer than any backoff"
        );
    }
    // A limit that names no time waits as a round that failed in passing does.
    let silent = limit(None);
    let third = [held(2, silent), vec![kept_by(silent, &[])]].concat();
    assert_eq!(
        step(third, &s),
        Step::Wait {
            failed: 3,
            ms: 4 * FIRST_WAIT,
            not_before: None
        }
    );
    // A round of the kind whose record names no hindrance waits as a limit that names no time does.
    assert_eq!(
        step(vec![failed_with(Failure::Held, None, &[])], &s),
        Step::Wait {
            failed: 1,
            ms: FIRST_WAIT,
            not_before: None
        }
    );
    // A limit that names the time it resets waits not before it, and a second more.
    let at = limit(Some(Retry::At(1_764_000_000)));
    assert_eq!(
        step(vec![kept_by(at, &[])], &s),
        Step::Wait {
            failed: 1,
            ms: FIRST_WAIT,
            not_before: Some(1_764_000_001_000)
        }
    );
}

#[test]
fn a_round_the_host_kept_from_running_rests_at_its_wait_and_is_tried_again_once_it_is_over() {
    let s = spec();
    let kept = limit(Some(Retry::In(60)));
    assert_eq!(
        step(vec![kept_by(kept, &[(7, false)])], &s),
        Step::Rest { calls: vec![CallId(7)] }
    );
    let over = kept_by(kept, &[(7, true)]);
    assert_eq!(step(vec![over.clone()], &s), Step::Retry);
    // The wait is no round: a pause holds back the round after it, and not the wait.
    assert!(matches!(
        step_paused(vec![kept_by(kept, &[])], &s),
        Step::Wait { failed: 1, .. }
    ));
    assert_eq!(step_paused(vec![over.clone()], &s), Step::Held);
    // The work let on after it takes the round again at once.
    assert_eq!(next(&let_on(vec![over], 1), s.requires(), Hold::Nothing), Step::Retry);
}

#[test]
fn a_round_the_host_kept_from_running_is_in_the_row_of_failures_in_passing_and_counts_toward_none() {
    let s = spec();
    let kept = limit(Some(Retry::In(60)));
    let passing_failure = || failed_as(Failure::Passing);
    // Five failures in passing, three rounds the host kept from running, three failures in passing: eight count, and
    // the work still waits.
    let rounds = [passing(5), held(3, kept), passing(2), vec![passing_failure()]].concat();
    assert_eq!(pausing(&let_on(rounds.clone(), 0)), None);
    assert_eq!(
        step(rounds, &s),
        Step::Wait {
            failed: 8,
            ms: LONGEST_WAIT,
            not_before: None
        }
    );
    // The ninth of them pauses the work, though three more rounds were kept from running between them.
    let ninth = [passing(5), held(3, kept), passing(3), vec![passing_failure()]].concat();
    assert_eq!(
        pausing(&let_on(ninth.clone(), 0)),
        Some(Pausing::Passing { failed: PASSING + 1 })
    );
    assert_eq!(step(ninth, &s), Step::Retry);
    // The round after those kept from running that fails in passing is only the first.
    let after = [held(PASSING as usize + 2, kept), vec![passing_failure()]].concat();
    assert_eq!(pausing(&let_on(after.clone(), 0)), None);
    assert_eq!(
        step(after, &s),
        Step::Wait {
            failed: 1,
            ms: FIRST_WAIT,
            not_before: None
        }
    );
}
