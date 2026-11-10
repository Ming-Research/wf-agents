//! The loop of the engine, for every work.
//!
//! The engine's own loop takes one thing that happened to one work and holds nothing (crate `engine`). The driver is
//! what takes the things that happen, in order, for all the works (entry `one-loop-decides`): it gives each to the
//! engine's loop, keeps what the loop added in the store before anything is done about it, then walks every other work
//! that has not ended, since what happened to one work may be what another waits for, and gives back what is to be done
//! outside, as data. Whatever carries those out (the agents, the engine's own acts, the tools) says what came of each
//! by calling the driver again; the driver calls nothing outside and reads no clock (principle
//! `outside-behind-interfaces`).
//!
//! What the driver holds of a work in memory is what the store holds of it: a work is saved each time it grows, and
//! what is to be done is given out only once the store holds the entries that say so. Stopped at any moment and
//! [opened](Driver::open) again, the driver gives again everything that was to be done and is not known as done
//! (principle `effects-known-as-done`). When the store fails, the driver stops: it gives the fault to every later
//! call, and is to be opened again (principle `nothing-passes-for-normal`).
//!
//! The named tests of the flows are functions the driver is given, each shown the work as it is at the walk that asks
//! and what the driver knows of the items ([`named`]).

pub mod named;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use decide::{Decision, End, Outcome, TestName, View, Who, decide};
use engine::{Effect, Event, NamedTests, Now, Refused, handle};
use flow::Compiled;
use store::{Item, ItemNumber, Store, WorkHead, WorkNumber};
use work::{Time, Work};

pub use named::{Seen, Test, Tests, dependencies_merged};

/// Drives the engine for every work the store holds.
pub struct Driver {
    store: Store,
    flows: BTreeMap<String, Arc<Compiled>>,
    tests: Tests,
    items: BTreeMap<ItemNumber, Item>,
    works: BTreeMap<WorkNumber, Held>,
    /// The items that have a work that merged.
    merged: BTreeSet<ItemNumber>,
    /// What stopped the driver, once something has.
    fault: Option<Fault>,
}

/// A work as the driver holds it: what the store holds of it, with its item and its flow at hand.
struct Held {
    head: WorkHead,
    item: Item,
    flow: Arc<Compiled>,
    work: Work,
    /// Whether the work has ended: merged, or cancelled with every run abandoned. Worked out again each time the
    /// work grows.
    ended: bool,
}

/// Something to be done outside the engine, for this work.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Todo {
    pub work: WorkNumber,
    pub effect: Effect,
}

/// A work as the driver holds it, for whoever shows it.
#[derive(Clone, Copy, Debug)]
pub struct Kept<'a> {
    pub head: &'a WorkHead,
    /// The item the work is done for.
    pub item: &'a Item,
    /// The flow the work runs on.
    pub flow: &'a Compiled,
    pub work: &'a Work,
}

/// What stops the driver.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fault {
    /// The store failed, or holds what the driver cannot go on from.
    Store(store::Fault),
    /// Two of the flows the driver was given have this name.
    FlowTwice(String),
    /// A flow uses a named test the driver was given no function for.
    NoSuchTest { flow: String, test: TestName },
    /// A work of the store runs on a flow the driver was not given.
    NoSuchFlow { work: WorkNumber, flow: String },
}

/// Why the driver did not do what it was asked. But for [`Failed::Stopped`], nothing was changed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failed {
    /// The driver has no such item.
    NoSuchItem(ItemNumber),
    /// The driver has no such work.
    NoSuchWork(WorkNumber),
    /// The driver was given no flow of this name.
    NoSuchFlow(String),
    /// The item has a work that has not ended: an item has one work going on at a time.
    WorkGoesOn { item: ItemNumber, work: WorkNumber },
    /// What happened cannot have happened to the work.
    Refused(Refused),
    /// The driver is stopped, by this fault. A work that grew before the fault and was saved keeps what it was
    /// given to do: opening the driver again gives it.
    Stopped(Fault),
}

impl Driver {
    /// Opens the driver on a store, with the flows the works run on and the named tests those flows use, and gives
    /// everything that is to be done: what was to be done before and is not known as done, and what follows from
    /// walking every work now.
    pub fn open(store: Store, flows: Vec<Compiled>, tests: Tests, now: Time) -> Result<(Driver, Vec<Todo>), Fault> {
        let mut by_name = BTreeMap::new();
        for flow in flows {
            if let Some(test) = flow.tests.keys().find(|test| !tests.contains_key(*test)) {
                return Err(Fault::NoSuchTest {
                    flow: flow.name.clone(),
                    test: test.clone(),
                });
            }
            let name = flow.name.clone();
            if by_name.insert(name.clone(), Arc::new(flow)).is_some() {
                return Err(Fault::FlowTwice(name));
            }
        }
        let items: BTreeMap<ItemNumber, Item> = (store.items().map_err(Fault::Store)?.into_iter())
            .map(|item| (item.number, item))
            .collect();
        let mut driver = Driver {
            store,
            flows: by_name,
            tests,
            items,
            works: BTreeMap::new(),
            merged: BTreeSet::new(),
            fault: None,
        };
        for head in driver.store.works().map_err(Fault::Store)? {
            let Some(flow) = driver.flows.get(&head.flow) else {
                return Err(Fault::NoSuchFlow {
                    work: head.number,
                    flow: head.flow,
                });
            };
            let Some(item) = driver.items.get(&head.item) else {
                return Err(Fault::Store(store::Fault::NoSuchItem(head.item)));
            };
            let work = driver.store.load(head.number).map_err(Fault::Store)?;
            let held = Held {
                item: item.clone(),
                flow: flow.clone(),
                work,
                ended: false,
                head,
            };
            driver.works.insert(held.head.number, held);
        }
        // Whether a work has ended is read from its records before any named test is asked, so the works may be
        // looked at in any order.
        let numbers: Vec<WorkNumber> = driver.works.keys().copied().collect();
        for number in numbers {
            driver.note_end(number);
        }
        // What was to be done is worked out before the works are walked: the walk adds to it.
        let mut todos = driver.pending();
        driver.settle(now, &mut todos)?;
        Ok((driver, todos))
    }

    /// Adds an item, depending on these items, and gives its number. The item has no work yet.
    pub fn add_item(&mut self, now: Time, title: &str, depends_on: &[ItemNumber]) -> Result<ItemNumber, Failed> {
        self.going()?;
        if let Some(on) = depends_on.iter().find(|on| !self.items.contains_key(*on)) {
            return Err(Failed::NoSuchItem(*on));
        }
        let number = (self.store.add_item(title, depends_on, now)).map_err(|fault| self.stop(Fault::Store(fault)))?;
        let items = self.store.items().map_err(|fault| self.stop(Fault::Store(fault)))?;
        self.items = items.into_iter().map(|item| (item.number, item)).collect();
        Ok(number)
    }

    /// Adds a work for the item, on the flow of this name, and gives its number with what is to be done. A work
    /// added paused begins no round until it is let on: what people write for it first can be written before it
    /// starts.
    pub fn add_work(
        &mut self,
        now: Time,
        item: ItemNumber,
        flow: &str,
        paused: bool,
    ) -> Result<(WorkNumber, Vec<Todo>), Failed> {
        self.going()?;
        let Some(of_item) = self.items.get(&item).cloned() else {
            return Err(Failed::NoSuchItem(item));
        };
        let Some(compiled) = self.flows.get(flow).cloned() else {
            return Err(Failed::NoSuchFlow(flow.to_string()));
        };
        if let Some(held) = self.works.values().find(|held| held.head.item == item && !held.ended) {
            return Err(Failed::WorkGoesOn {
                item,
                work: held.head.number,
            });
        }
        let number = (self.store.add_work(item, flow, now)).map_err(|fault| self.stop(Fault::Store(fault)))?;
        let held = Held {
            head: WorkHead {
                number,
                item,
                flow: flow.to_string(),
                added: now,
            },
            item: of_item,
            flow: compiled,
            work: Work::default(),
            ended: false,
        };
        self.works.insert(number, held);
        let mut todos = Vec::new();
        if paused {
            self.turn(now, number, Event::Paused, &mut todos)?;
        }
        self.settle(now, &mut todos).map_err(Failed::Stopped)?;
        Ok((number, todos))
    }

    /// Takes one thing that happened to a work, at the time the caller's clock gives, and gives what is to be done:
    /// for that work, and for every other work for which something follows from it.
    pub fn happened(&mut self, now: Time, work: WorkNumber, event: Event) -> Result<Vec<Todo>, Failed> {
        self.going()?;
        let mut todos = Vec::new();
        self.turn(now, work, event, &mut todos)?;
        self.settle(now, &mut todos).map_err(Failed::Stopped)?;
        Ok(todos)
    }

    /// What was to be done for the works and is not yet known as done, in the order of the works.
    pub fn pending(&self) -> Vec<Todo> {
        (self.works.iter())
            .flat_map(|(&work, held)| {
                (engine::pending(&held.work).into_iter()).map(move |effect| Todo { work, effect })
            })
            .collect()
    }

    /// Every item, in the order of their numbers.
    pub fn items(&self) -> impl Iterator<Item = &Item> {
        self.items.values()
    }

    /// Every work, in the order of their numbers.
    pub fn works(&self) -> impl Iterator<Item = Kept<'_>> {
        self.works.values().map(Held::kept)
    }

    /// The work of this number.
    pub fn work(&self, number: WorkNumber) -> Option<Kept<'_>> {
        self.works.get(&number).map(Held::kept)
    }

    /// The flows the driver was given, in the order of their names.
    pub fn flows(&self) -> impl Iterator<Item = &Compiled> {
        self.flows.values().map(|flow| &**flow)
    }

    /// The work as it is now: what it does next, each cell's state, every version of every block.
    pub fn view(&self, number: WorkNumber) -> Option<View> {
        let held = self.works.get(&number)?;
        let outside = self.outside(held);
        let tests = Now {
            tests: &outside,
            work: &held.work,
        };
        Some(decide::view(&held.flow.flow, &held.work.records, &tests))
    }

    /// What stopped the driver, once something has.
    pub fn fault(&self) -> Option<&Fault> {
        self.fault.as_ref()
    }

    /// Refuses once the driver is stopped.
    fn going(&self) -> Result<(), Failed> {
        match &self.fault {
            Some(fault) => Err(Failed::Stopped(fault.clone())),
            None => Ok(()),
        }
    }

    /// Stops the driver.
    fn stop(&mut self, fault: Fault) -> Failed {
        self.fault = Some(fault.clone());
        Failed::Stopped(fault)
    }

    /// The named tests for a work, bound to what the driver knows outside it.
    fn outside<'a>(&'a self, held: &'a Held) -> Outside<'a> {
        Outside {
            tests: &self.tests,
            flow: &held.flow,
            item: &held.item,
            merged: &self.merged,
        }
    }

    /// Gives one thing that happened to the engine's loop for the work, and keeps what the loop added before giving
    /// out what is to be done. Says whether the work grew.
    fn turn(&mut self, now: Time, number: WorkNumber, event: Event, todos: &mut Vec<Todo>) -> Result<bool, Failed> {
        let Some(held) = self.works.get_mut(&number) else {
            return Err(Failed::NoSuchWork(number));
        };
        let outside = Outside {
            tests: &self.tests,
            flow: &held.flow,
            item: &held.item,
            merged: &self.merged,
        };
        let before = held.work.tick();
        let effects = handle(&held.flow, &outside, &mut held.work, now, event).map_err(Failed::Refused)?;
        if held.work.tick() == before {
            return Ok(false);
        }
        if let Err(fault) = self.store.save(number, &held.work) {
            // What the loop added is not kept, so it did not happen: the work is again what the store holds of it,
            // where the store can still be read.
            if let Ok(stored) = self.store.load(number) {
                held.work = stored;
            }
            return Err(self.stop(Fault::Store(fault)));
        }
        self.note_end(number);
        todos.extend(effects.into_iter().map(|effect| Todo { work: number, effect }));
        Ok(true)
    }

    /// Notes whether the work has ended, and that its item has a merged work where it merged.
    fn note_end(&mut self, number: WorkNumber) {
        let Some(held) = self.works.get(&number) else {
            return;
        };
        let outside = self.outside(held);
        let tests = Now {
            tests: &outside,
            work: &held.work,
        };
        let end = match decide(&held.flow.flow, &held.work.records, &tests) {
            Decision::Ended(end) => Some(end),
            _ => None,
        };
        let item = held.head.item;
        if end == Some(End::Merged) {
            self.merged.insert(item);
        }
        if let Some(held) = self.works.get_mut(&number) {
            held.ended = end.is_some();
        }
    }

    /// Walks every work that has not ended, again and again until none of them grows: what happened to one work may
    /// be what another waits for. A work with a round under way is walked too, and nothing is decided for it.
    fn settle(&mut self, now: Time, todos: &mut Vec<Todo>) -> Result<(), Fault> {
        loop {
            let numbers: Vec<WorkNumber> = (self.works.iter())
                .filter(|(_, held)| !held.ended)
                .map(|(number, _)| *number)
                .collect();
            let mut grew = false;
            for number in numbers {
                match self.turn(now, number, Event::Look, todos) {
                    Ok(it_grew) => grew |= it_grew,
                    Err(Failed::Stopped(fault)) => return Err(fault),
                    // A look at a work the driver holds is never refused: it records nothing that could be.
                    Err(_) => {}
                }
            }
            if !grew {
                return Ok(());
            }
        }
    }
}

impl Held {
    fn kept(&self) -> Kept<'_> {
        Kept {
            head: &self.head,
            item: &self.item,
            flow: &self.flow,
            work: &self.work,
        }
    }
}

/// The named tests of a work's flow, each shown the work as it is at the walk that asks.
struct Outside<'a> {
    tests: &'a Tests,
    flow: &'a Compiled,
    item: &'a Item,
    merged: &'a BTreeSet<ItemNumber>,
}

impl NamedTests for Outside<'_> {
    fn test(&self, name: &TestName, work: &Work) -> Outcome {
        let seen = Seen {
            flow: self.flow,
            work,
            item: self.item,
            merged: self.merged,
        };
        match self.tests.get(name) {
            Some(test) => test(&seen),
            // Every flow was checked, as the driver opened, to use only tests the driver has; a test it lacks holds
            // the work for the owner and lets nothing through.
            None => Outcome::Waits { who: Who::Owner },
        }
    }
}
