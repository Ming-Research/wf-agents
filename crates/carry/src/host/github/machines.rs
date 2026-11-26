//! The machines the projects' measurements run on, which every project's host shares. On a machine of the owner's own,
//! a self-hosted runner's label, one measurement runs at a time, of whatever project: a measurement that another one
//! ran beside would be disturbed by it and say nothing of it. GitHub's own machines give each run a machine of its own,
//! so a measurement there waits for none.
//!
//! A request waits for its turn on the machine in the order it came, saying so as it waits, and holds the turn until
//! the run it asks for has ended and been read, or what asked for it stopped waiting. The one whose turn it is waits
//! further while GitHub lists a run of a measurement's workflow on the machine that has not completed, of any project:
//! a run asked for before the engine started again, which the turns kept in memory do not know of, or one a person
//! started by hand. A request whose own run GitHub lists already, asked for before the engine started again, takes no
//! turn: its run is on the machine, and it waits for it, while the one whose turn it is waits for that run to end.
//!
//! The turns keep the requests in order in the engine, where the owner's page shows each wait. The machine's own gate
//! (`runner/measure-gate.sh`, docs/installing.md) keeps the machine's other jobs out while a measurement times, of
//! whatever workflow: a run of another workflow on a machine a measurement holds may wait in that gate, so its time is
//! not counted against its limit while the turn is held.

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, PoisonError, Weak};
use std::time::Duration;

use service::{Along, Clock};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::held::Stop;
use crate::host::Failed;

/// What lists the runs on a machine of the measurements one project declares that have not completed: the link of
/// each; or why GitHub could not say, of the kind the host tells.
pub trait Busy: Send + Sync {
    fn going<'a>(&'a self, machine: &'a str) -> Pin<Box<dyn Future<Output = Result<Vec<String>, Failed>> + Send + 'a>>;
}

/// The machines measurements run on, each by its runner's label as the declarations name it.
pub struct Machines {
    clock: Arc<dyn Clock>,
    of: Mutex<BTreeMap<String, Arc<Machine>>>,
}

/// One machine: whose turn it is, and what lists the runs of each project's measurements on it.
struct Machine {
    /// One turn, given in the order it was asked for: a semaphore of tokio's gives its permits first come, first
    /// served.
    turn: Arc<Semaphore>,
    /// The projects whose measurements run on it. They are not kept alive by it: a host that is gone lists nothing.
    sharing: Mutex<Vec<Weak<dyn Busy>>>,
}

/// A request's turn on a machine, held until it is dropped.
pub(crate) struct Turn {
    _permit: OwnedSemaphorePermit,
}

impl std::fmt::Debug for Machines {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let of = self.of.lock().unwrap_or_else(PoisonError::into_inner);
        f.debug_struct("Machines")
            .field("of", &of.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl Machines {
    /// No machine yet, the time a wait begins read from the clock given.
    pub fn new(clock: Arc<dyn Clock>) -> Machines {
        Machines {
            clock,
            of: Mutex::new(BTreeMap::new()),
        }
    }

    /// The machine of this label, made the first time it is named.
    fn machine(&self, label: &str) -> Arc<Machine> {
        let mut of = self.of.lock().unwrap_or_else(PoisonError::into_inner);
        (of.entry(label.to_string()).or_insert_with(|| {
            Arc::new(Machine {
                turn: Arc::new(Semaphore::new(1)),
                sharing: Mutex::new(Vec::new()),
            })
        }))
        .clone()
    }

    /// Whether a measurement holds the turn on the machine of this label: no machine is held that no measurement ever
    /// asked for.
    pub(crate) fn held(&self, label: &str) -> bool {
        let of = self.of.lock().unwrap_or_else(PoisonError::into_inner);
        of.get(label)
            .is_some_and(|machine| machine.turn.available_permits() == 0)
    }

    /// A project measures on the machine of this label, and `busy` lists its runs there.
    pub fn share(&self, label: &str, busy: Weak<dyn Busy>) {
        let machine = self.machine(label);
        machine
            .sharing
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(busy);
    }

    /// Waits for the turn on the machine of this label, saying that it waits for it and since when, then for every
    /// run of a measurement there that has not completed to end, looking each `poll`; the turn, or none where it was
    /// told to stop. An `Err` is why GitHub could not say whether the machine is free, or why the time could not be
    /// read.
    pub(crate) async fn turn(&self, label: &str, poll: Duration, stop: &mut Stop) -> Result<Option<Turn>, Failed> {
        let since = self
            .clock
            .now()
            .map_err(|why| format!("the time cannot be read: {why}"))?;
        stop.say(Some(Along::WaitsForMachine {
            machine: label.to_string(),
            since: since.0,
        }));
        let machine = self.machine(label);
        let permit = tokio::select! {
            permit = machine.turn.clone().acquire_owned() => permit.map_err(|_| "the turns on a machine were closed")?,
            () = stop.told() => return Ok(None),
        };
        loop {
            let sharing: Vec<Arc<dyn Busy>> = (machine.sharing.lock().unwrap_or_else(PoisonError::into_inner))
                .iter()
                .filter_map(Weak::upgrade)
                .collect();
            let mut going = Vec::new();
            for busy in sharing {
                going.extend(busy.going(label).await?);
            }
            if going.is_empty() {
                return Ok(Some(Turn { _permit: permit }));
            }
            tokio::select! {
                () = tokio::time::sleep(poll) => {}
                () = stop.told() => return Ok(None),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use crate::held::Held;
    use work::Time;

    /// A clock that always reads the same time.
    struct Still(u64);

    impl Clock for Still {
        fn now(&self) -> Result<Time, String> {
            Ok(Time(self.0))
        }
    }

    /// A project whose runs on a machine are as many as `going` says, each looked at counted.
    #[derive(Default)]
    struct Runs {
        going: AtomicUsize,
        looked: AtomicUsize,
    }

    impl Busy for Runs {
        fn going<'a>(
            &'a self,
            machine: &'a str,
        ) -> Pin<Box<dyn Future<Output = Result<Vec<String>, Failed>> + Send + 'a>> {
            self.looked.fetch_add(1, Ordering::SeqCst);
            let going = self.going.load(Ordering::SeqCst);
            Box::pin(async move { Ok((0..going).map(|n| format!("{machine}/{n}")).collect()) })
        }
    }

    const POLL: Duration = Duration::from_millis(5);

    // Three requests for one machine take their turns one at a time in the order they came, each turn given only once
    // the one before it is let go of; a request for another machine waits for none of them. Milliseconds alone; the one
    // test of the order of the turns.
    #[tokio::test]
    async fn requests_for_one_machine_take_their_turns_one_at_a_time_in_the_order_they_came() {
        let machines = Arc::new(Machines::new(Arc::new(Still(7))));
        let first = machines
            .turn("14900k", POLL, &mut Stop::never())
            .await
            .unwrap()
            .expect("a turn");
        let (to, mut came) = tokio::sync::mpsc::unbounded_channel();
        for n in 1..=2 {
            let (machines, to) = (machines.clone(), to.clone());
            tokio::spawn(async move {
                let turn = machines.turn("14900k", POLL, &mut Stop::never()).await;
                to.send((n, "turn")).expect("sent");
                tokio::time::sleep(Duration::from_millis(20)).await;
                to.send((n, "done")).expect("sent");
                drop(turn);
            });
            // The second asks after the first has begun to wait.
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let other = machines.turn("other", POLL, &mut Stop::never()).await.unwrap();
        assert!(other.is_some(), "another machine waits for none");
        tokio::time::sleep(Duration::from_millis(30)).await;
        assert!(came.try_recv().is_err(), "nobody has the turn while the first holds it");
        drop(first);
        let mut order = Vec::new();
        for _ in 0..4 {
            order.push(came.recv().await.expect("said"));
        }
        assert_eq!(order, [(1, "turn"), (1, "done"), (2, "turn"), (2, "done")]);
    }

    // The one whose turn it is waits while a project sharing the machine lists a run there that has not completed, of
    // whichever project, looking again until none is; a project that measures on another machine is not looked at.
    // Milliseconds alone; the one test that a run the turns do not know of holds the machine.
    #[tokio::test]
    async fn the_turn_waits_while_any_project_lists_a_run_on_the_machine_that_has_not_completed() {
        let machines = Machines::new(Arc::new(Still(7)));
        let (firn, whitefoot, elsewhere) = (
            Arc::new(Runs::default()),
            Arc::new(Runs::default()),
            Arc::new(Runs::default()),
        );
        let weak = |runs: &Arc<Runs>| Arc::downgrade(runs) as Weak<dyn Busy>;
        machines.share("14900k", weak(&firn));
        machines.share("14900k", weak(&whitefoot));
        machines.share("other", weak(&elsewhere));
        whitefoot.going.store(1, Ordering::SeqCst);
        let mut stop = Stop::never();
        let waiting = machines.turn("14900k", POLL, &mut stop);
        tokio::pin!(waiting);
        tokio::select! {
            _ = &mut waiting => panic!("the turn is given while a run of whitefoot's is on the machine"),
            () = tokio::time::sleep(Duration::from_millis(30)) => {}
        }
        assert!(whitefoot.looked.load(Ordering::SeqCst) >= 2, "it is looked at again");
        whitefoot.going.store(0, Ordering::SeqCst);
        assert!(waiting.await.unwrap().is_some());
        assert_eq!(elsewhere.looked.load(Ordering::SeqCst), 0);
    }

    // A request that waits says that it waits for the machine and since when; told to stop, it leaves the line and the
    // next request is given the turn in its place. Milliseconds alone; the one test of what a wait says and of a stop
    // while it waits.
    #[tokio::test]
    async fn a_request_says_it_waits_for_the_machine_and_leaves_the_line_when_told_to_stop() {
        let machines = Arc::new(Machines::new(Arc::new(Still(1_700_000_000_000))));
        let held: Held<u8, service::CallReply> = Held::new();
        let (to, mut came) = tokio::sync::mpsc::unbounded_channel();
        let reply = service::CallReply::new(store::WorkNumber(3), decide::CallId(4), to);
        let saying = reply.saying();
        let mut stop = held.hold(1, reply).expect("held").saying(saying);
        let first = machines.turn("14900k", POLL, &mut Stop::never()).await.unwrap();
        let waits = {
            let machines = machines.clone();
            tokio::spawn(async move { machines.turn("14900k", POLL, &mut stop).await.map(|t| t.is_some()) })
        };
        tokio::time::sleep(Duration::from_millis(10)).await;
        let Ok(service::Message::Along { along, .. }) = came.try_recv() else {
            panic!("where it stands is said");
        };
        assert_eq!(
            along,
            Some(Along::WaitsForMachine {
                machine: "14900k".to_string(),
                since: 1_700_000_000_000
            })
        );
        let (to, _) = tokio::sync::mpsc::unbounded_channel();
        held.stop(1, service::CallReply::new(store::WorkNumber(3), decide::CallId(4), to));
        assert!(!waits.await.expect("ends").unwrap(), "told to stop, it has no turn");
        drop(first);
        let next = machines.turn("14900k", POLL, &mut Stop::never()).await.unwrap();
        assert!(next.is_some(), "the line goes on without the one that left it");
    }
}
