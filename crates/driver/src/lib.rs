//! The loop of the engine, for every work.
//!
//! The engine's own loop takes one thing that happened to one work and holds nothing (crate `engine`). The driver is
//! what takes the things that happen, in order, for all the works (entry `one-loop-decides`): it gives each to the
//! engine's loop for the work it happened to, keeps what the loop added in the store before anything is done about it,
//! and gives back what is to be done outside, as data. Whatever carries those out (the agents, the engine's own acts,
//! the tools) says what came of each by calling the driver again; the driver calls nothing outside but the store, and
//! reads no clock (principle `outside-behind-interfaces`).
//!
//! What happens to one work reaches another only as a write of a block of the other. The driver is itself what writes
//! one such block: a block whose source is [`DEPENDENCIES`] says whether the items the work's item depends on have
//! merged, and the driver writes it as an item is added, when the last of those items merges, and, on opening, for
//! every work that has not ended, wherever the block and what it stands for have come apart. A work that has ended
//! is added to no more.
//!
//! What the driver holds of a work in memory is what the store holds of it: a work is saved each time it grows, and
//! what is to be done is given out only once the store holds the entries that say so. Stopped at any moment and
//! [opened](Driver::open) again, the driver gives again everything that was to be done and is not known as done
//! (principle `effects-known-as-done`). When the store fails the driver stops: it gives the fault to every later call
//! and shows nothing more of the works, which it may hold otherwise than the store does, and is to be opened again
//! (principle `nothing-passes-for-normal`).

use std::collections::BTreeMap;
use std::sync::Arc;

use decide::{BlockId, Decision, End, Outsider, View, decide};
use engine::{Effect, Event, Refused, handle};
use flow::Compiled;
use store::{Item, ItemNumber, Storage, WorkHead, WorkNumber};
use work::{Time, Work};

/// The source of the blocks the driver writes: whether the items the work's item depends on have merged. Such a block
/// is a choice of [`WAITING`] and [`MERGED`].
pub const DEPENDENCIES: &str = "dependencies";
/// The value of a [`DEPENDENCIES`] block while an item the work's item depends on has not merged.
pub const WAITING: &str = "waiting";
/// The value of a [`DEPENDENCIES`] block once every item the work's item depends on has merged, or it depends on none.
pub const MERGED: &str = "merged";

/// Drives the engine for every work the store holds.
pub struct Driver {
    store: Box<dyn Storage + Send>,
    flows: BTreeMap<String, Arc<Compiled>>,
    works: BTreeMap<WorkNumber, Held>,
    /// The work of each item.
    of_item: BTreeMap<ItemNumber, WorkNumber>,
    /// What stopped the driver, once something has.
    fault: Option<Fault>,
}

/// A work as the driver holds it: what the store holds of it, with its item and its flow at hand.
struct Held {
    head: WorkHead,
    item: Item,
    flow: Arc<Compiled>,
    work: Work,
    /// How the work ended, once it has. Worked out again each time the work grows.
    ended: Option<End>,
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

/// An item to be added, with what people write for it first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewItem {
    pub title: String,
    /// The items it depends on.
    pub depends_on: Vec<ItemNumber>,
    /// The name of the flow its work runs on.
    pub flow: String,
    /// What people write for it as it is added, in order.
    pub writes: Vec<FirstWrite>,
}

/// A block people write as an item is added.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FirstWrite {
    pub block: BlockId,
    pub content: String,
    /// The block's value, where it is a choice.
    pub value: Option<String>,
}

/// What stops the driver.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fault {
    /// The store failed, or holds what the driver cannot go on from.
    Store(store::Fault),
    /// Two of the flows the driver was given have this name.
    FlowTwice(String),
    /// A block of the flow names [`DEPENDENCIES`] as its source and is no choice of [`WAITING`] and [`MERGED`].
    NotDependencies { flow: String, block: BlockId },
    /// A work of the store runs on a flow the driver was not given.
    NoSuchFlow { work: WorkNumber, flow: String },
    /// The store holds an item that has not exactly one work.
    NotOneWork(ItemNumber),
    /// What the driver itself wrote for a work, a [`DEPENDENCIES`] block, was refused by the engine's loop.
    OwnWriteRefused { work: WorkNumber, refused: Refused },
}

/// Why the driver did not do what it was asked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failed {
    /// The driver has no such item. Nothing was changed.
    NoSuchItem(ItemNumber),
    /// The driver has no such work. Nothing was changed.
    NoSuchWork(WorkNumber),
    /// The driver was given no flow of this name. Nothing was changed.
    NoSuchFlow(String),
    /// What happened cannot have happened to the work, or what was to be written for a new item cannot be written.
    /// Nothing was changed.
    Refused(Refused),
    /// The driver is stopped, by this fault. `kept` says whether what it was asked was kept before the fault: where
    /// it was, it has happened and is not to be given again once the driver is opened again, and what follows from
    /// it is given then; where it was not, it has not happened.
    Stopped { fault: Fault, kept: bool },
}

impl Driver {
    /// Opens the driver on a store, with the flows the works run on, and gives everything that is to be done: what was
    /// to be done before and is not known as done, and what follows from looking at every work now.
    pub fn open(store: Box<dyn Storage + Send>, flows: Vec<Compiled>, now: Time) -> Result<(Driver, Vec<Todo>), Fault> {
        let mut by_name = BTreeMap::new();
        for flow in flows {
            for (block, declared) in &flow.blocks {
                let ours = declared.writers.outside.as_deref() == Some(DEPENDENCIES);
                let values = declared.values.as_deref().unwrap_or_default();
                if ours && !(values.iter().any(|v| v == WAITING) && values.iter().any(|v| v == MERGED)) {
                    return Err(Fault::NotDependencies {
                        flow: flow.name.clone(),
                        block: block.clone(),
                    });
                }
            }
            let name = flow.name.clone();
            if by_name.insert(name.clone(), Arc::new(flow)).is_some() {
                return Err(Fault::FlowTwice(name));
            }
        }
        let mut driver = Driver {
            store,
            flows: by_name,
            works: BTreeMap::new(),
            of_item: BTreeMap::new(),
            fault: None,
        };
        let items = driver.store.items().map_err(Fault::Store)?;
        let heads = driver.store.works().map_err(Fault::Store)?;
        for head in heads {
            let Some(flow) = driver.flows.get(&head.flow).cloned() else {
                return Err(Fault::NoSuchFlow {
                    work: head.number,
                    flow: head.flow,
                });
            };
            let Some(item) = items.iter().find(|item| item.number == head.item).cloned() else {
                return Err(Fault::NotOneWork(head.item));
            };
            if driver.of_item.insert(head.item, head.number).is_some() {
                return Err(Fault::NotOneWork(head.item));
            }
            let work = driver.store.load(head.number).map_err(Fault::Store)?;
            let ended = end_of(&flow, &work);
            let held = Held {
                head,
                item,
                flow,
                work,
                ended,
            };
            driver.works.insert(held.head.number, held);
        }
        if let Some(item) = items.iter().find(|item| !driver.of_item.contains_key(&item.number)) {
            return Err(Fault::NotOneWork(item.number));
        }
        // What was to be done is worked out before the works are looked at: looking adds to it.
        let mut todos = driver.pending();
        let numbers: Vec<WorkNumber> = driver.works.keys().copied().collect();
        for number in numbers {
            let look = (driver.agree(now, number, &mut todos))
                .and_then(|()| driver.turn(now, number, Event::Look, &mut todos));
            match look {
                Ok(_) => {}
                Err(Failed::Stopped { fault, .. }) => return Err(fault),
                // A look records nothing that could be refused.
                Err(Failed::Refused(refused)) => return Err(Fault::OwnWriteRefused { work: number, refused }),
                // The number is one the driver holds.
                Err(Failed::NoSuchWork(_) | Failed::NoSuchItem(_) | Failed::NoSuchFlow(_)) => {}
            }
        }
        Ok((driver, todos))
    }

    /// Adds an item together with its one work and with what people write for it first, all of it or none (entry
    /// `item-added-with-its-work`), and gives their numbers. The work is added paused: it begins no round until it
    /// is let on, so nothing is to be done yet.
    pub fn add(&mut self, now: Time, new: NewItem) -> Result<(ItemNumber, WorkNumber), Failed> {
        self.going()?;
        if let Some(on) = new.depends_on.iter().find(|on| !self.of_item.contains_key(*on)) {
            return Err(Failed::NoSuchItem(*on));
        }
        let Some(flow) = self.flows.get(&new.flow).cloned() else {
            return Err(Failed::NoSuchFlow(new.flow));
        };
        // What is kept of the work from its start: the pause, what the driver says of the item's dependencies, and
        // what people wrote, in that order. Being paused from its first entry, the work is given nothing to do.
        let mut work = Work::default();
        let mut first = vec![Event::Paused];
        first.extend(self.dependencies(&flow, &new.depends_on, &work));
        first.extend(new.writes.into_iter().map(|write| Event::Wrote {
            block: write.block,
            by: Outsider::Person,
            content: write.content,
            matters: true,
            value: write.value,
        }));
        for event in first {
            handle(&flow, &mut work, now, event).map_err(Failed::Refused)?;
        }
        let (item, number) = (self.store)
            .add(&new.title, &new.depends_on, &new.flow, now, &work)
            .map_err(|fault| self.stop(Fault::Store(fault), false))?;
        let mut depends_on = new.depends_on;
        depends_on.sort();
        depends_on.dedup();
        let held = Held {
            head: WorkHead {
                number,
                item,
                flow: new.flow,
                added: now,
            },
            item: Item {
                number: item,
                title: new.title,
                added: now,
                depends_on,
            },
            ended: end_of(&flow, &work),
            flow,
            work,
        };
        self.works.insert(number, held);
        self.of_item.insert(item, number);
        Ok((item, number))
    }

    /// Takes one thing that happened to a work, at the time the caller's clock gives, and gives what is to be done:
    /// for that work, and, where the work merged with it, for the works whose items waited for that.
    pub fn happened(&mut self, now: Time, work: WorkNumber, event: Event) -> Result<Vec<Todo>, Failed> {
        self.going()?;
        let mut todos = Vec::new();
        let was = self.works.get(&work).and_then(|held| held.ended);
        self.turn(now, work, event, &mut todos)?;
        let held = self.works.get(&work);
        if was.is_none() && held.and_then(|held| held.ended) == Some(End::Merged) {
            // The item has merged: that is written to each work whose item depended on it and now waits for none.
            let item = held.map(|held| held.head.item);
            let waiting: Vec<WorkNumber> = (self.works.iter())
                .filter(|(_, held)| item.is_some_and(|item| held.item.depends_on.contains(&item)))
                .map(|(number, _)| *number)
                .collect();
            for number in waiting {
                // The thing that happened is kept by now, whatever becomes of what follows from it.
                self.agree(now, number, &mut todos).map_err(kept)?;
            }
        }
        Ok(todos)
    }

    /// What was to be done for the works and is not yet known as done, in the order of the works.
    pub fn pending(&self) -> Vec<Todo> {
        self.shown()
            .flat_map(|(&work, held)| {
                (engine::pending(&held.work).into_iter()).map(move |effect| Todo { work, effect })
            })
            .collect()
    }

    /// Every work with its item, in the order of the works' numbers.
    pub fn works(&self) -> impl Iterator<Item = Kept<'_>> {
        self.shown().map(|(_, held)| held.kept())
    }

    /// The work of this number.
    pub fn work(&self, number: WorkNumber) -> Option<Kept<'_>> {
        match self.fault {
            None => self.works.get(&number).map(Held::kept),
            Some(_) => None,
        }
    }

    /// The flows the driver was given, in the order of their names.
    pub fn flows(&self) -> impl Iterator<Item = &Compiled> {
        self.flows.values().map(|flow| &**flow)
    }

    /// The work as it is now: what it does next, each cell's state, every version of every block.
    pub fn view(&self, number: WorkNumber) -> Option<View> {
        let kept = self.work(number)?;
        let tests = kept.flow.tests(&kept.work.records);
        Some(decide::view(&kept.flow.flow, &kept.work.records, &tests))
    }

    /// What stopped the driver, once something has.
    pub fn fault(&self) -> Option<&Fault> {
        self.fault.as_ref()
    }

    /// The works the driver shows: all of them, and none once it is stopped, when what it holds may be ahead of
    /// what the store holds.
    fn shown(&self) -> impl Iterator<Item = (&WorkNumber, &Held)> {
        self.works.iter().filter(|_| self.fault.is_none())
    }

    /// Refuses once the driver is stopped: nothing more is kept.
    fn going(&self) -> Result<(), Failed> {
        match &self.fault {
            Some(fault) => Err(Failed::Stopped {
                fault: fault.clone(),
                kept: false,
            }),
            None => Ok(()),
        }
    }

    /// Stops the driver.
    fn stop(&mut self, fault: Fault, kept: bool) -> Failed {
        self.fault = Some(fault.clone());
        Failed::Stopped { fault, kept }
    }

    /// Gives one thing that happened to the engine's loop for the work, and keeps what the loop added before giving
    /// out what is to be done. Says whether the work grew.
    fn turn(&mut self, now: Time, number: WorkNumber, event: Event, todos: &mut Vec<Todo>) -> Result<bool, Failed> {
        let Some(held) = self.works.get_mut(&number) else {
            return Err(Failed::NoSuchWork(number));
        };
        let before = held.work.tick();
        let effects = handle(&held.flow, &mut held.work, now, event).map_err(Failed::Refused)?;
        if held.work.tick() == before {
            return Ok(false);
        }
        if let Err(fault) = self.store.save(number, &held.work) {
            return Err(self.stop(Fault::Store(fault), false));
        }
        held.ended = end_of(&held.flow, &held.work);
        todos.extend(effects.into_iter().map(|effect| Todo { work: number, effect }));
        Ok(true)
    }

    /// Brings the work's [`DEPENDENCIES`] blocks to agree with what they stand for, writing those that do not.
    fn agree(&mut self, now: Time, number: WorkNumber, todos: &mut Vec<Todo>) -> Result<(), Failed> {
        let Some(held) = self.works.get(&number) else {
            return Err(Failed::NoSuchWork(number));
        };
        if held.ended.is_some() {
            return Ok(());
        }
        for event in self.dependencies(&held.flow, &held.item.depends_on, &held.work) {
            match self.turn(now, number, event, todos) {
                // The driver's own write was refused: the flow and the driver disagree about the block.
                Err(Failed::Refused(refused)) => {
                    return Err(self.stop(Fault::OwnWriteRefused { work: number, refused }, false));
                }
                other => other?,
            };
        }
        Ok(())
    }

    /// The writes that bring the flow's [`DEPENDENCIES`] blocks to say, for a work whose item depends on these items,
    /// whether they have all merged: one for each such block that does not say so now.
    fn dependencies(&self, flow: &Compiled, depends_on: &[ItemNumber], work: &Work) -> Vec<Event> {
        let waited: Vec<ItemNumber> = (depends_on.iter().copied())
            .filter(|on| {
                let ended = self
                    .of_item
                    .get(on)
                    .and_then(|work| self.works.get(work))
                    .and_then(|held| held.ended);
                ended != Some(End::Merged)
            })
            .collect();
        let (value, content) = if waited.is_empty() {
            (MERGED, "Every item this one depends on has merged.".to_string())
        } else {
            let numbers: Vec<String> = waited.iter().map(|on| on.0.to_string()).collect();
            (WAITING, format!("Waits for the items {} to merge.", numbers.join(", ")))
        };
        let versions = decide::versions(&work.records);
        (flow.blocks.iter())
            .filter(|(_, declared)| declared.writers.outside.as_deref() == Some(DEPENDENCIES))
            .filter(|(block, _)| {
                let now = versions
                    .get(*block)
                    .and_then(|v| v.last())
                    .and_then(|v| v.value.as_deref());
                now != Some(value)
            })
            .map(|(block, _)| Event::Wrote {
                block: block.clone(),
                by: Outsider::Outside,
                content: content.clone(),
                matters: true,
                value: Some(value.to_string()),
            })
            .collect()
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

/// How the work ended, if it has.
fn end_of(flow: &Compiled, work: &Work) -> Option<End> {
    match decide(&flow.flow, &work.records, &flow.tests(&work.records)) {
        Decision::Ended(end) => Some(end),
        _ => None,
    }
}

/// The failure of what follows from a thing that happened, once the thing itself is kept.
fn kept(failed: Failed) -> Failed {
    match failed {
        Failed::Stopped { fault, .. } => Failed::Stopped { fault, kept: true },
        other => other,
    }
}
