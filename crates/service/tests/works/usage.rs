//! What the agents' service says of each account's usage, as the loop keeps it: under the account a call was handed out
//! with, whichever is in use as it is told; and the looks the owner's opening of the accounts takes, under each account
//! not heard of lately, the accounts said changed as each is taken and ends, and as the accounts or the usage limits
//! that stand change.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use decide::run::Failure;
use driver::{Found, LOOK_AFTER_MS, Said, Window};
use service::{Act, Changed, Heard, Message, Outside, Token};
use store::AccountAct;
use tokio::sync::{Semaphore, broadcast, oneshot};
use work::Time;

use crate::failing::failing_as;
use crate::{Pings, Trial, trial_script};

/// The week's window, so much of it used.
fn week(utilization: f64) -> Said {
    Said::Windows(vec![Window {
        name: "seven_day".to_string(),
        utilization: Some(utilization),
        resets: Some(Time(1_795_399_200_000)),
        allowed: true,
    }])
}

/// An outside whose short request says a tenth of the week used under the account `first`, a fifth under any other
/// account, and does not reach the service under `away`; it keeps the account each check was taken under, and each
/// check waits for a permit of `gate` before it answers, where one is given.
#[derive(Default)]
struct Looks {
    under: Mutex<Vec<Option<String>>>,
    gate: Option<Semaphore>,
}

impl Outside for Looks {
    fn check(&self, account: Option<String>) -> Pin<Box<dyn Future<Output = Heard> + Send + '_>> {
        self.under.lock().unwrap().push(account.clone());
        Box::pin(async move {
            if let Some(gate) = &self.gate {
                gate.acquire().await.expect("the gate is open").forget();
            }
            match account.as_deref() {
                Some("away") | None => Found::Down {
                    failure: Failure::Offline,
                    why: "The network does not reach the agents' service.".to_string(),
                    resets: None,
                }
                .into(),
                Some(name) => Heard {
                    found: Found::Well,
                    usage: Some(week(if name == "first" { 0.1 } else { 0.2 })),
                },
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

/// Whether whoever listens was told the accounts changed since it last read.
fn told_accounts(changes: &mut broadcast::Receiver<Changed>) -> bool {
    let mut told = false;
    while let Ok(changed) = changes.try_recv() {
        told |= changed.accounts;
    }
    told
}

/// What the loop knows the service last said of the usage of the account of this name.
fn said(trial: &Trial, name: &str) -> Option<Said> {
    let usage = trial.service.driver().usage().of(name)?;
    usage.seen.as_ref().map(|(_, said)| said.clone())
}

// A look is taken under each account not heard of lately, all at once and with none in use, none twice at a time; the
// accounts are said changed as the looks are taken and as each ends; a look that heard the week's window keeps it, and
// one that could not reach the service says why; neither is taken again until five minutes have passed. Milliseconds
// of the paused clock, nothing run: the one test of when the loop spends a look.
#[tokio::test(start_paused = true)]
async fn a_look_is_taken_under_each_account_not_heard_of_lately_and_the_accounts_are_said_changed() {
    let looks = Arc::new(Looks::default());
    let mut trial = Trial::with_outside(trial_script(), looks.clone());
    account(&mut trial, "first", AccountAct::Added);
    account(&mut trial, "away", AccountAct::Added);
    let mut changes = trial.service.handle().changes();
    assert_eq!(
        look(&mut trial),
        ["first", "away"],
        "none is in use, and both are looked under"
    );
    assert!(told_accounts(&mut changes), "looking");
    assert!(trial.service.driver().usage().of("first").is_some_and(|u| u.looking));
    assert_eq!(look(&mut trial), Vec::<String>::new(), "none twice at a time");
    for _ in 0..2 {
        assert!(trial.step().await.is_some(), "a look ends");
    }
    let mut under = looks.under.lock().unwrap().clone();
    under.sort();
    assert_eq!(
        under,
        [Some("away".to_string()), Some("first".to_string())],
        "each under its account"
    );
    assert!(told_accounts(&mut changes), "looked");
    assert_eq!(said(&trial, "first"), Some(week(0.1)));
    let away = trial.service.driver().usage().of("away").expect("away's usage").clone();
    assert_eq!((away.seen, away.looking), (None, false));
    assert_eq!(
        away.failed.as_deref(),
        Some("The network does not reach the agents' service.")
    );
    assert_eq!(look(&mut trial), Vec::<String>::new(), "both were heard of just now");
    tokio::time::advance(Duration::from_millis(LOOK_AFTER_MS + 1)).await;
    assert_eq!(
        look(&mut trial),
        ["first", "away"],
        "five minutes on, both are looked under again"
    );
}

// The usage a call says is kept under the account it was handed out with, whichever is in use as it is told, and the
// accounts are said changed, but not where it says again what it said; one that names an account not added is passed
// over. Each act on an account says the accounts changed. Milliseconds, nothing run: the loop's keeping of the word.
#[tokio::test(start_paused = true)]
async fn the_usage_a_call_says_is_kept_under_the_account_it_was_handed_out_with() {
    let mut trial = Trial::with_outside(trial_script(), Arc::new(Looks::default()));
    let mut changes = trial.service.handle().changes();
    for name in ["first", "second"] {
        account(&mut trial, name, AccountAct::Added);
        assert!(told_accounts(&mut changes), "{name} added");
    }
    account(&mut trial, "first", AccountAct::Taken);
    account(&mut trial, "second", AccountAct::Taken);
    assert!(told_accounts(&mut changes), "taken");
    let usage = |account: &str, said: Said| Message::Usage {
        account: account.to_string(),
        said,
    };
    trial.take(usage("first", week(0.67)));
    assert!(told_accounts(&mut changes));
    assert_eq!(said(&trial, "first"), Some(week(0.67)), "under the call's account");
    assert_eq!(said(&trial, "second"), None, "not under the one in use");
    trial.take(usage("first", week(0.67)));
    assert!(!told_accounts(&mut changes), "the same said again is no news");
    trial.take(usage("gone", week(0.5)));
    assert!(!told_accounts(&mut changes), "no such account");
}

// A check of the outside taken under one account that comes back after the owner took another is kept under the
// account it was taken under. Milliseconds of the paused clock, nothing run: the one test of the check's word reaching
// the right account across a switch.
#[tokio::test(start_paused = true)]
async fn a_check_that_comes_back_after_another_account_was_taken_is_kept_under_its_own() {
    let looks = Arc::new(Looks {
        gate: Some(Semaphore::new(0)),
        ..Looks::default()
    });
    let mut trial = Trial::with_outside(failing_as("unavailable", None), looks.clone());
    for name in ["first", "second"] {
        account(&mut trial, name, AccountAct::Added);
    }
    let (_, work) = trial.add("Greet the user", &[]);
    trial.act(work, Act::Resume).unwrap();
    trial.to_the_owners_call(work).await;
    trial.let_go_on(work);
    trial.step().await.unwrap();
    assert!(trial.service.driver().outage().is_some(), "the wait stands");
    // The check under `first` is under way, held at the gate, as the owner takes `second`.
    account(&mut trial, "first", AccountAct::Taken);
    tokio::task::yield_now().await;
    account(&mut trial, "second", AccountAct::Taken);
    assert_eq!(trial.service.driver().accounts().in_use(), Some("second"));
    looks.gate.as_ref().expect("a gate").add_permits(10);
    while said(&trial, "first").is_none() {
        assert!(trial.step().await.is_some(), "the check comes back");
    }
    assert_eq!(
        said(&trial, "first"),
        Some(week(0.1)),
        "kept under the account it was taken under"
    );
    assert_ne!(said(&trial, "second"), Some(week(0.1)));
}

// A round that meets the usage limit under an account changes the limits that stand, and whoever listens is told the
// accounts changed with it. Milliseconds of the paused clock, the scripted rounds: the one test of the limits' notice.
#[tokio::test(start_paused = true)]
async fn a_round_that_meets_the_usage_limit_says_the_accounts_changed() {
    let mut trial = Trial::opened(failing_as("usage_limit", Some(3_600_000)), Some(Pings::default()), None);
    account(&mut trial, "first", AccountAct::Added);
    account(&mut trial, "first", AccountAct::Taken);
    let (_, work) = trial.add("Greet the user", &[]);
    trial.act(work, Act::Resume).unwrap();
    while trial.owners_call(work).is_none() {
        assert!(trial.step().await.is_some(), "the plan calls the owner");
    }
    trial.let_go_on(work);
    let mut changes = trial.service.handle().changes();
    while !trial.service.driver().limits().contains_key("first") {
        assert!(trial.step().await.is_some(), "the plan's round meets the limit");
    }
    let mut told = Vec::new();
    while let Ok(changed) = changes.try_recv() {
        told.push(changed.accounts);
    }
    assert!(told.contains(&true), "{told:?}");
}
