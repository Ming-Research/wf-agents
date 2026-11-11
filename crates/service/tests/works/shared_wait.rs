//! A wait every work shares, as the loop carries it through with its own timer: it looks at the network, lets a round
//! try or lets every work on at the reset when the time comes, whatever else the loop takes meanwhile, and after a
//! restart at any moment of the wait as well.

use std::time::Duration;

use decide::{Decision, End};
use service::Act;

use crate::Trial;
use crate::failing::failing_as;

/// The trial work with the plan's second round failing for this kind of cause, let on, the owner answering the plan's
/// question, the failure taken: the wait stands.
async fn waiting(failure: &str, resets: Option<u64>) -> (Trial, store::WorkNumber) {
    let mut trial = Trial::with(failing_as(failure, resets));
    let (_, work) = trial.add("Greet the user", &[]);
    trial.act(work, Act::Resume).unwrap();
    trial.step().await.unwrap();
    trial.answer(work, "Yes, by name.");
    trial.step().await.unwrap();
    assert!(trial.service.driver().outage().is_some(), "the wait stands");
    (trial, work)
}

/// The wait of this kind, the process stopped and started again once this long has passed since the failure: the work
/// goes on to its end by the loop's own timer.
async fn restarted_after(failure: &str, resets: Option<u64>, after: Duration) {
    let (mut trial, work) = waiting(failure, resets).await;
    tokio::time::advance(after).await;
    trial.restart_and_lift();
    assert_eq!(
        trial.drive(work).await,
        Decision::Ended(End::Merged),
        "{failure} after {after:?}"
    );
    assert_eq!(trial.service.driver().outage(), None);
}

#[tokio::test(start_paused = true)]
async fn without_the_network_a_restart_at_any_time_of_the_wait_looks_again_and_goes_on() {
    for seconds in [10, 31, 60, 299, 301, 3600] {
        restarted_after("offline", None, Duration::from_secs(seconds)).await;
    }
}

#[tokio::test(start_paused = true)]
async fn where_the_command_line_cannot_run_a_restart_at_any_time_of_the_wait_lets_a_round_try() {
    for seconds in [10, 31, 60, 3600] {
        restarted_after("cannot_run", None, Duration::from_secs(seconds)).await;
    }
}

#[tokio::test(start_paused = true)]
async fn at_the_usage_limit_a_restart_before_or_after_the_reset_goes_on_at_the_reset() {
    let hour = 3_600_000;
    for seconds in [60, 3599, 3601, 7200] {
        restarted_after("usage_limit", Some(hour), Duration::from_secs(seconds)).await;
    }
}

#[tokio::test(start_paused = true)]
async fn two_works_held_by_the_wait_both_go_on_by_the_loops_timer_whatever_comes_between() {
    for failure in ["offline", "cannot_run"] {
        let (mut trial, held) = waiting(failure, None).await;
        let (_, busy) = trial.add("Greet the owner", &[]);
        trial.act(busy, Act::Resume).unwrap();
        // The other work meets the wait as well, and the owner's answers come while the wait's time comes.
        let ended = |trial: &Trial| {
            [held, busy]
                .iter()
                .all(|w| matches!(trial.decision(*w), Decision::Ended(_)))
        };
        while !ended(&trial) {
            for work in [held, busy] {
                if trial.owners_call(work).is_some() {
                    trial.answer(work, "Yes, by name.");
                }
            }
            assert!(trial.step().await.is_some(), "{failure}: nothing more comes");
        }
        assert!(trial.service.driver().outage().is_none());
    }
}
