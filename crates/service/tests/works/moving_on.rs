//! The engine's moving to another account at a usage limit, as the loop carries it through with its own timer: where
//! the owner has let it, a round that meets the usage limit of the account in use has the loop check the outside under
//! another account, by the timer, and take it once the check finds it well; the work goes on under it. Nothing is
//! checked under another account, and nothing moves, where the owner has not.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use decide::{Decision, End};
use driver::Found;
use service::{Act, Heard, Outside};
use store::AccountAct;

use crate::Trial;
use crate::failing::failing_as;

/// The trial work with the plan's second round failing as the script says, let on, the owner answering the plan's
/// question once the work waits for it, the failure taken: the wait stands. A warm loop reports its rounds'
/// sessions as events of their own, so each stage is stepped to, not counted.
async fn waiting(trial: &mut Trial) -> store::WorkNumber {
    let (_, work) = trial.add("Greet the user", &[]);
    trial.act(work, Act::Resume).unwrap();
    for _ in 0..10 {
        if trial.owners_call(work).is_some() {
            break;
        }
        trial.step().await.expect("the loop goes on");
    }
    trial.let_go_on(work);
    for _ in 0..10 {
        if trial.service.driver().outage().is_some() {
            break;
        }
        trial.step().await.expect("the loop goes on");
    }
    assert!(trial.service.driver().outage().is_some(), "the wait stands");
    work
}

/// The outside of the agents' rounds, always well, which keeps the account each check was taken under.
#[derive(Default)]
struct Seen {
    under: Mutex<Vec<Option<String>>>,
}

impl Seen {
    fn under(&self) -> Vec<Option<String>> {
        self.under.lock().unwrap().clone()
    }
}

impl Outside for Seen {
    fn check_token(&self, _: service::Token) -> Pin<Box<dyn Future<Output = Heard> + Send + '_>> {
        Box::pin(async { Found::Well.into() })
    }

    fn check(&self, account: Option<String>) -> Pin<Box<dyn Future<Output = Heard> + Send + '_>> {
        Box::pin(async move {
            self.under.lock().unwrap().push(account);
            Found::Well.into()
        })
    }
}

// With the switch on, the loop's own timer checks the outside under the other account the moment the limit is met, and
// takes it on the check finding it well: no check is taken under the account in use, and the work goes on under the
// account taken to its end, the move recorded as the engine's. Under the paused clock, which passes at once.
#[tokio::test(start_paused = true)]
async fn at_a_usage_limit_the_loop_checks_another_account_by_its_own_timer_and_takes_it_where_the_owner_lets_it() {
    let seen = Arc::new(Seen::default());
    let mut trial = Trial::warm_with(failing_as("usage_limit", None), seen.clone());
    trial.account("home", AccountAct::Added);
    trial.set_moving_on(true);
    let work = waiting(&mut trial).await;
    let outage = trial.service.driver().outage().expect("the wait stands");
    assert!(outage.holds() && outage.may_move);
    for _ in 0..10 {
        if trial.service.driver().accounts().in_use() != Some("work") {
            break;
        }
        trial.step().await.expect("the loop's timer checks");
    }
    assert_eq!(seen.under(), [Some("home".to_string())]);
    assert_eq!(trial.service.driver().accounts().in_use(), Some("home"));
    let moves = trial.service.driver().moves();
    assert_eq!(moves.len(), 1);
    assert_eq!((moves[0].from.as_deref(), moves[0].to.as_str()), (Some("work"), "home"));
    assert_eq!(trial.drive(work).await, Decision::Ended(End::Merged));
    assert_eq!(trial.service.driver().outage(), None);
    let ran = trial.work(work).ran;
    assert_eq!(
        ran.last().and_then(|ran| ran.on.account.as_deref()),
        Some("home"),
        "the round that met the limit was run again under the account taken"
    );
}

// With the switch off, the same history checks nothing under the other account: the wait is waited out at the reset,
// by the checks under the account in use alone.
#[tokio::test(start_paused = true)]
async fn with_the_switch_off_nothing_is_checked_under_another_account_and_the_wait_is_waited_out() {
    let seen = Arc::new(Seen::default());
    let mut trial = Trial::warm_with(failing_as("usage_limit", None), seen.clone());
    trial.account("home", AccountAct::Added);
    let work = waiting(&mut trial).await;
    assert!(!trial.service.driver().outage().expect("the wait stands").may_move);
    assert_eq!(trial.drive(work).await, Decision::Ended(End::Merged));
    assert!(
        seen.under().iter().all(|under| under.as_deref() == Some("work")),
        "{:?}",
        seen.under()
    );
    assert_eq!(trial.service.driver().accounts().in_use(), Some("work"));
    assert_eq!(trial.service.driver().moves(), vec![]);
}
