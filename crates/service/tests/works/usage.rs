//! What the agents' service says of each account's usage, as the loop keeps it: under the account a call was handed out
//! with, whichever is in use as it is told; and the looks the owner's opening of the accounts takes, under each account
//! not heard of lately, the accounts said changed as each is taken and ends.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use decide::run::Failure;
use driver::{Found, LOOK_AFTER_MS, Window};
use service::{Changed, Heard, Message, Outside, Token};
use store::AccountAct;
use tokio::sync::{broadcast, oneshot};
use work::Time;

use crate::Trial;
use crate::trial_script;

/// The week's window, so much of it used.
fn week(utilization: f64) -> Window {
    Window {
        name: "seven_day".to_string(),
        utilization: Some(utilization),
        resets: Some(Time(1_795_399_200_000)),
        allowed: true,
    }
}

/// An outside whose short request names the week's window under the account `work`, and does not reach the service
/// under any other; it keeps the account each check was taken under.
#[derive(Default)]
struct Looks {
    under: Mutex<Vec<Option<String>>>,
}

impl Outside for Looks {
    fn check(&self, account: Option<String>) -> Pin<Box<dyn Future<Output = Heard> + Send + '_>> {
        self.under.lock().unwrap().push(account.clone());
        Box::pin(async move {
            match account.as_deref() {
                Some("work") => Heard {
                    found: Found::Well,
                    usage: vec![week(0.04)],
                },
                _ => Found::Down {
                    failure: Failure::Offline,
                    why: "The network does not reach the agents' service.".to_string(),
                    resets: None,
                }
                .into(),
            }
        })
    }

    fn check_token(&self, _: Token) -> Pin<Box<dyn Future<Output = Heard> + Send + '_>> {
        Box::pin(async { Found::Well.into() })
    }
}

/// The owner adds, or takes, the account of this name.
fn account(trial: &mut Trial, name: &str, what: AccountAct) {
    let (reply, mut answer) = oneshot::channel();
    trial.take(Message::Account {
        name: name.to_string(),
        what,
        reply,
    });
    (answer.try_recv())
        .expect("the loop answers at once")
        .expect("it is done");
}

/// The owner opens the accounts: gives the names looked under.
fn look(trial: &mut Trial) -> Vec<String> {
    let (reply, mut answer) = oneshot::channel();
    trial.take(Message::Look { reply });
    (answer.try_recv())
        .expect("the loop answers at once")
        .expect("it is looked")
}

/// Whether whoever listens was told the accounts changed since it last read, and nothing else.
fn told_accounts(changes: &mut broadcast::Receiver<Changed>) -> bool {
    let mut told = false;
    while let Ok(changed) = changes.try_recv() {
        told |= changed.accounts;
    }
    told
}

// A look is taken under each account not heard of lately, all at once and with none in use, none twice at a time; the
// accounts are said changed as the looks are taken and as each ends; a look that heard the week's window keeps it, and
// one that could not reach the service says why; neither is taken again until five minutes have passed.
#[tokio::test(start_paused = true)]
async fn a_look_is_taken_under_each_account_not_heard_of_lately_and_the_accounts_are_said_changed() {
    let looks = Arc::new(Looks::default());
    let mut trial = Trial::with_outside(trial_script(), looks.clone());
    account(&mut trial, "work", AccountAct::Added);
    account(&mut trial, "away", AccountAct::Added);
    let mut changes = trial.service.handle().changes();
    assert_eq!(
        look(&mut trial),
        ["work", "away"],
        "none is in use, and both are looked under"
    );
    assert!(told_accounts(&mut changes), "looking");
    assert!(trial.service.driver().usage().of("work").is_some_and(|u| u.looking));
    assert_eq!(look(&mut trial), Vec::<String>::new(), "none twice at a time");
    for _ in 0..2 {
        assert!(trial.step().await.is_some(), "a look ends");
    }
    let mut under = looks.under.lock().unwrap().clone();
    under.sort();
    assert_eq!(
        under,
        [Some("away".to_string()), Some("work".to_string())],
        "each under its account"
    );
    assert!(told_accounts(&mut changes), "looked");
    let usage = trial.service.driver().usage();
    let work = usage.of("work").expect("work's usage");
    assert_eq!(
        work.seen.as_ref().map(|(_, windows)| windows.clone()),
        Some(vec![week(0.04)])
    );
    assert!(!work.looking);
    let away = usage.of("away").expect("away's usage");
    assert_eq!(
        (away.seen.clone(), away.failed.as_deref()),
        (None, Some("The network does not reach the agents' service."))
    );
    assert_eq!(look(&mut trial), Vec::<String>::new(), "both were heard of just now");
    tokio::time::advance(Duration::from_millis(LOOK_AFTER_MS + 1)).await;
    assert_eq!(
        look(&mut trial),
        ["work", "away"],
        "five minutes on, both are looked under again"
    );
}

// The usage a call names is kept under the account it was handed out with, whichever is in use as it is told, and the
// accounts are said changed; one that names an account not added is passed over.
#[tokio::test(start_paused = true)]
async fn the_usage_a_call_names_is_kept_under_the_account_it_was_handed_out_with() {
    let mut trial = Trial::with_outside(trial_script(), Arc::new(Looks::default()));
    for name in ["work", "home"] {
        account(&mut trial, name, AccountAct::Added);
    }
    account(&mut trial, "work", AccountAct::Taken);
    account(&mut trial, "home", AccountAct::Taken);
    let mut changes = trial.service.handle().changes();
    trial.take(Message::Usage {
        account: "work".to_string(),
        windows: vec![week(0.67)],
    });
    assert!(told_accounts(&mut changes));
    let usage = trial.service.driver().usage();
    assert_eq!(
        usage
            .of("work")
            .and_then(|u| u.seen.clone())
            .map(|(_, windows)| windows),
        Some(vec![week(0.67)]),
        "under the call's account, not the one in use"
    );
    assert_eq!(usage.of("home").and_then(|u| u.seen.clone()), None);
    trial.take(Message::Usage {
        account: "gone".to_string(),
        windows: vec![week(0.5)],
    });
    assert!(!told_accounts(&mut changes), "no such account");
}
