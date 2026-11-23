//! A wait every work shares, as the loop carries it through with its own timer: it checks the outside of the agents'
//! rounds when the driver asks, under the account in use, begins no round while a check finds it down, and lets every
//! work on once one finds it well, whatever else the loop takes meanwhile, and after a restart at any moment of the
//! wait as well.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use decide::run::Failure;
use decide::{Decision, End};
use driver::Found;
use driver::outage::{PROBE, spacing};
use service::{Act, Heard, Message, Outside};
use store::AccountAct;
use tokio::sync::oneshot;

use crate::Trial;
use crate::failing::failing_as;

/// The trial work with the plan's round after the owner's word failing for this kind of cause, let on, the owner
/// letting the plan's run go on, the failure taken: the wait stands.
async fn waiting(trial: &mut Trial) -> store::WorkNumber {
    let (_, work) = trial.add("Greet the user", &[]);
    trial.act(work, Act::Resume).unwrap();
    trial.to_the_owners_call(work).await;
    trial.answer(work, "Yes, by name.");
    trial.step().await.unwrap();
    assert!(trial.service.driver().outage().is_some(), "the wait stands");
    work
}

/// The wait of this kind, the process stopped and started again once this long has passed since the failure: the work
/// goes on to its end by the loop's own timer.
async fn restarted_after(failure: &str, resets: Option<u64>, after: Duration) {
    let mut trial = Trial::with(failing_as(failure, resets));
    let work = waiting(&mut trial).await;
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
async fn a_restart_at_any_time_of_the_wait_checks_the_outside_again_and_goes_on() {
    for failure in ["offline", "cannot_run", "unavailable", "usage_limit"] {
        for seconds in [10, 31, 60, 299, 301, 3600] {
            restarted_after(failure, None, Duration::from_secs(seconds)).await;
        }
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
    for failure in ["offline", "cannot_run", "unavailable", "usage_limit"] {
        let mut trial = Trial::with(failing_as(failure, None));
        let held = waiting(&mut trial).await;
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

/// An outside a check finds down so many times, the service not serving, and then well; it keeps when each check was
/// taken, by the loop's clock.
struct Downs {
    left: Mutex<u32>,
    taken: Mutex<Vec<tokio::time::Instant>>,
    /// The account each check was taken under, in order.
    under: Mutex<Vec<Option<String>>>,
}

impl Downs {
    fn new(left: u32) -> Arc<Downs> {
        Arc::new(Downs {
            left: Mutex::new(left),
            taken: Mutex::new(Vec::new()),
            under: Mutex::new(Vec::new()),
        })
    }

    fn taken(&self) -> Vec<tokio::time::Instant> {
        self.taken.lock().unwrap().clone()
    }

    fn under(&self) -> Vec<Option<String>> {
        self.under.lock().unwrap().clone()
    }
}

impl Outside for Downs {
    fn check_token(&self, _: service::Token) -> Pin<Box<dyn Future<Output = Heard> + Send + '_>> {
        Box::pin(async { Found::Well.into() })
    }

    fn check(&self, account: Option<String>) -> Pin<Box<dyn Future<Output = Heard> + Send + '_>> {
        Box::pin(async move {
            self.taken.lock().unwrap().push(tokio::time::Instant::now());
            self.under.lock().unwrap().push(account);
            let mut left = self.left.lock().unwrap();
            if *left == 0 {
                return Found::Well.into();
            }
            *left -= 1;
            Found::Down {
                failure: Failure::Unavailable,
                why: "API Error: 529 Overloaded".to_string(),
                resets: None,
            }
            .into()
        })
    }
}

// The loop's timer checks the outside when the driver asks, each check longer after the one before while it finds it
// down, and begins no round meanwhile; once a check finds it well, the round that met the failure is run again and the
// work goes on to its end. Some six minutes of the paused clock, which passes at once.
#[tokio::test(start_paused = true)]
async fn while_the_checks_find_the_outside_down_no_round_begins_and_once_one_finds_it_well_the_work_goes_on() {
    let downs = Downs::new(3);
    let mut trial = Trial::with_outside(failing_as("unavailable", None), downs.clone());
    let work = waiting(&mut trial).await;
    let failed_at = tokio::time::Instant::now();
    let rounds = trial.work(work).records.rounds.len();
    while downs.taken().len() < 3 {
        assert!(trial.step().await.is_some(), "the loop's timer checks");
        assert_eq!(
            trial.work(work).records.rounds.len(),
            rounds,
            "a round began while the outside was down"
        );
        assert!(!decide::paused(&trial.work(work).records));
    }
    assert_eq!(trial.drive(work).await, Decision::Ended(End::Merged));
    let taken = downs.taken();
    assert_eq!(taken.len(), 4, "checked once more, and found well");
    let spaced: Vec<u64> = (std::iter::once(failed_at).chain(taken.iter().copied()))
        .collect::<Vec<_>>()
        .windows(2)
        .map(|w| (w[1] - w[0]).as_millis() as u64)
        .collect();
    assert_eq!(spaced, vec![PROBE, spacing(1), spacing(2), spacing(3)]);
    assert_eq!(trial.service.driver().outage(), None);
}

// Taking another account while the wait stands has the loop check the outside at once under the account taken, by its
// own timer, rather than at the spacing the checks under the account before had reached; and the work goes on once a
// check under it finds the outside well.
#[tokio::test(start_paused = true)]
async fn taking_another_account_while_the_wait_stands_checks_the_outside_at_once_under_it() {
    let downs = Downs::new(1);
    let mut trial = Trial::with_outside(failing_as("unavailable", None), downs.clone());
    let work = waiting(&mut trial).await;
    for what in [AccountAct::Added, AccountAct::Taken] {
        let (reply, _answer) = oneshot::channel();
        let account = Message::Account {
            name: "work".to_string(),
            what,
            reply,
        };
        trial.service.take(account).expect("the loop takes it");
    }
    let taken_at = tokio::time::Instant::now();
    assert!(trial.step().await.is_some(), "the loop's timer checks");
    assert_eq!(downs.under(), [Some("work".to_string())]);
    let after = downs.taken()[0].duration_since(taken_at);
    assert!(
        after < Duration::from_millis(PROBE),
        "checked {after:?} after the account was taken"
    );
    assert_eq!(trial.drive(work).await, Decision::Ended(End::Merged));
    assert!(downs.under().iter().all(|under| under.as_deref() == Some("work")));
}
