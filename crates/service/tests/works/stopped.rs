//! The engine stopped by its store as the owner acts: the refusal says whether what was asked was kept.

use std::sync::{Arc, Mutex};

use driver::{Asked, Failed, Fault};
use service::{Act, Message, Refusal, Scripted, Service, TokioClock};
use store::{Item, ItemChange, ItemNumber, Storage, Store, WorkHead, WorkNumber};
use tokio::sync::oneshot;
use work::{Time, Work};

use crate::{START, act_on, add_to, new_item, trial_flow};

/// The store of these tests: a store on a file, whose saves fail once a set number of them have been made.
struct Failing {
    store: Store,
    /// How many saves are made before every later one fails; none where none fails.
    saves: Arc<Mutex<Option<usize>>>,
}

fn full() -> store::Fault {
    store::Fault::Database("the disk is full".to_string())
}

impl Storage for Failing {
    fn add(
        &mut self,
        item: &store::First,
        flow: &str,
        at: Time,
        first: &Work,
    ) -> Result<(ItemNumber, WorkNumber), store::Fault> {
        self.store.add(item, flow, at, first)
    }

    fn change(
        &mut self,
        at: Time,
        why: Option<&str>,
        changes: &[(ItemNumber, ItemChange)],
        works: &[(WorkNumber, &Work)],
    ) -> Result<(), store::Fault> {
        self.store.change(at, why, changes, works)
    }

    fn items(&self) -> Result<Vec<Item>, store::Fault> {
        self.store.items()
    }

    fn works(&self) -> Result<Vec<WorkHead>, store::Fault> {
        self.store.works()
    }

    fn load(&self, work: WorkNumber) -> Result<Work, store::Fault> {
        self.store.load(work)
    }

    fn everything_paused(&self) -> Result<Option<store::EverythingPaused>, store::Fault> {
        self.store.everything_paused()
    }

    fn pause_everything(&mut self, paused: bool, by: decide::Pauser, at: Time) -> Result<(), store::Fault> {
        self.store.pause_everything(paused, by, at)
    }

    fn save(&mut self, work: WorkNumber, given: &Work) -> Result<(), store::Fault> {
        let mut saves = self.saves.lock().expect("the count of saves");
        match saves.as_mut() {
            Some(0) => return Err(full()),
            Some(left) => *left -= 1,
            None => {}
        }
        self.store.save(work, given)
    }
}

/// The loop on a store whose saves fail once `saves` holds a number, and the way to set it.
fn failing() -> (Service, Arc<Mutex<Option<usize>>>, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("a directory for the store");
    let saves = Arc::new(Mutex::new(None));
    let store = Failing {
        store: Store::open(dir.path().join("store.sqlite")).expect("the store opens"),
        saves: saves.clone(),
    };
    let carriers = Scripted::new(crate::trial_script()).carriers();
    let clock = Arc::new(TokioClock::new(START));
    let (service, _inbox) =
        Service::open(Box::new(store), vec![trial_flow()], carriers, clock).expect("the loop opens");
    (service, saves, dir)
}

fn stopped(kept: bool) -> Refusal {
    Refusal::Failed(Failed::Stopped {
        fault: Fault::Store(full()),
        kept,
    })
}

#[tokio::test(start_paused = true)]
async fn an_act_the_engine_stopped_before_keeping_is_to_be_given_again() {
    let (mut service, saves, _dir) = failing();
    let (_, work) = add_to(&mut service, new_item("Greet the user", &[]));
    *saves.lock().unwrap() = Some(0);
    let (answer, taken) = act_on(&mut service, work, Act::Resume);
    assert_eq!(answer, Err(stopped(false)));
    assert_eq!(taken, Err(service::Fault::Driver(Fault::Store(full()))));
    assert_eq!(
        stopped(false).to_string(),
        "The engine stopped: the store failed: the database failed: the disk is full. What you asked was not kept: give \
         it again once the engine runs."
    );
}

#[tokio::test(start_paused = true)]
async fn a_change_kept_before_the_engine_stopped_is_not_to_be_given_again() {
    let (mut service, saves, _dir) = failing();
    let (first, _) = add_to(&mut service, new_item("Greet the user", &[]));
    let (second, _) = add_to(&mut service, new_item("Say goodbye", &[]));
    // The change is kept; what the driver writes of what the item that comes to wait now waits for is not.
    *saves.lock().unwrap() = Some(0);
    let (reply, mut answer) = oneshot::channel();
    let changes = vec![(second, Asked::Item(ItemChange::DependsOn(first)))];
    let why = "It greets first.".to_string();
    let taken = service.take(Message::Change { why, changes, reply });
    let answer = answer.try_recv().expect("the loop answers at once");
    assert_eq!(answer, Err(stopped(true)));
    assert_eq!(taken, Err(service::Fault::Driver(Fault::Store(full()))));
    assert_eq!(
        stopped(true).to_string(),
        "The engine stopped: the store failed: the database failed: the disk is full. What you asked was kept: do not \
         give it again."
    );
}

// The loop alone reads the clock, so whoever else needs the time asks the loop for it.
#[tokio::test(start_paused = true)]
async fn the_time_is_asked_of_the_loop_and_is_its_clocks() {
    let dir = tempfile::tempdir().expect("a directory for the store");
    let store = Store::open(dir.path().join("store.sqlite")).expect("the store opens");
    let carriers = Scripted::new(crate::trial_script()).carriers();
    let clock = Arc::new(TokioClock::new(START));
    let (mut service, mut inbox) =
        Service::open(Box::new(store), vec![trial_flow()], carriers, clock).expect("the loop opens");
    let handle = service.handle();
    let asked = tokio::spawn(async move { handle.now().await });
    let message = inbox.recv().await.expect("the loop is asked");
    service.take(message).expect("the loop takes it");
    assert_eq!(asked.await.expect("the task ends"), Ok(START));
    tokio::time::advance(std::time::Duration::from_secs(5)).await;
    let handle = service.handle();
    let asked = tokio::spawn(async move { handle.now().await });
    let message = inbox.recv().await.expect("the loop is asked");
    service.take(message).expect("the loop takes it");
    assert_eq!(asked.await.expect("the task ends"), Ok(Time(START.0 + 5_000)));
}
