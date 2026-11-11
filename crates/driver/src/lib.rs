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
//! one such block: the block of a flow whose source is [`DEPENDENCIES`] says whether the items the work's item
//! depends on have merged. The driver writes it as the item is added, whenever one of those items merges or is
//! cancelled, and, on opening, wherever the block and what it stands for have come apart; it writes no block of a
//! work that has ended. A work whose flow has no such block waits for no item. An item that depends on a cancelled
//! one waits for good, its block saying which: the owner cancels it or leaves it.
//!
//! What another source outside the works says, such as the head of the project's main branch, the driver is told
//! ([`Driver::outside`]), and writes into the block of that source of each work that has not ended, where the block
//! does not say it already.
//!
//! What the driver holds of a work in memory is what the store holds of it: a work is saved each time it grows, and
//! what is to be done is given out only once the store holds the entries that say so. Stopped at any moment and
//! [opened](Driver::open) again, the driver gives again everything that was to be done and is not known as done
//! (principle `effects-known-as-done`). When the store fails the driver stops: it gives the fault to every later call
//! and shows nothing more of the works, which it may hold otherwise than the store does, and is to be opened again
//! (principle `nothing-passes-for-normal`).

use std::collections::BTreeMap;
use std::sync::Arc;

use decide::{BlockId, Decision, End, Outsider, Pauser, View, decide};
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
    /// How the work stands. Worked out again each time the work grows.
    standing: Standing,
}

/// How a work stands, for the driver: whether it is still to be written to and looked at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Standing {
    /// It has not ended.
    Going,
    /// It has ended, so.
    Ended(End),
    /// Its records cannot be those of a work: nothing is decided on them, and the driver writes it nothing.
    AtFault,
}

/// Why a step of a work did not go through.
enum Halt {
    /// The engine's loop refused what happened, and nothing was changed.
    Refused(Refused),
    /// The store failed, and the work is ahead of it in memory.
    Store(store::Fault),
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
    /// The name of the level it is given, which whoever adds it has checked against the installation's levels.
    pub level: String,
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
    /// The block of the flow that names [`DEPENDENCIES`] as its source is not what the driver writes: a choice that
    /// has the values [`WAITING`] and [`MERGED`], which neither people nor a cell write.
    NotDependencies { flow: String, block: BlockId },
    /// More than one block of the flow names [`DEPENDENCIES`] as its source: the driver writes one.
    DependenciesTwice { flow: String },
    /// A work of the store runs on a flow the driver was not given.
    NoSuchFlow { work: WorkNumber, flow: String },
    /// The store holds an item that has not exactly one work, or a work whose item it does not hold.
    NotOneWork(ItemNumber),
    /// The engine's loop refused what the driver itself gave it for a work: a look, or the write of its own block.
    Refused { work: WorkNumber, refused: Refused },
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
            let mut ours = (flow.blocks.iter()).filter(|(_, b)| b.writers.outside.as_deref() == Some(DEPENDENCIES));
            if let Some((block, declared)) = ours.next() {
                if ours.next().is_some() {
                    return Err(Fault::DependenciesTwice {
                        flow: flow.name.clone(),
                    });
                }
                let values = declared.values.as_deref().unwrap_or_default();
                let both = values.iter().any(|v| v == WAITING) && values.iter().any(|v| v == MERGED);
                if !both || declared.writers.people || !declared.writers.cells.is_empty() {
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
            let standing = standing_of(&flow, &work);
            let held = Held {
                head,
                item,
                flow,
                work,
                standing,
            };
            driver.works.insert(held.head.number, held);
        }
        if let Some(item) = items.iter().find(|item| !driver.of_item.contains_key(&item.number)) {
            return Err(Fault::NotOneWork(item.number));
        }
        // What was to be done is worked out before the works are looked at: looking adds to it.
        let mut todos = driver.pending();
        // What each work's block is to say is worked out of the works as the store gave them, before any is written
        // to; then each work that has not ended has its block written where it says otherwise, and is looked at.
        let wanted: Vec<Option<Event>> = driver.works.values().map(|held| driver.wanted(held)).collect();
        let Driver { store, works, .. } = &mut driver;
        for (held, write) in works.values_mut().zip(wanted) {
            if held.standing != Standing::Going {
                continue;
            }
            for event in write.into_iter().chain([Event::Look]) {
                match step(store.as_mut(), held, now, event) {
                    Ok(effects) => todos.extend(effects.into_iter().map(|effect| held.todo(effect))),
                    Err(Halt::Store(fault)) => return Err(Fault::Store(fault)),
                    Err(Halt::Refused(refused)) => {
                        return Err(Fault::Refused {
                            work: held.head.number,
                            refused,
                        });
                    }
                }
            }
        }
        Ok((driver, todos))
    }

    /// Adds an item together with its one work and with what people write for it first, all of it or none (entry
    /// `item-added-with-its-work`), and gives their numbers with what is to be done. The work is added paused, so
    /// that it begins no round until it is let on. What is kept of it from its start is, in this order, the pause,
    /// what the driver says of the item's dependencies, and what people wrote.
    ///
    /// The driver keeps no key of the caller's for an add: a caller that was stopped before it had the numbers finds
    /// the item among those the driver shows, or adds it again as another item.
    pub fn add(&mut self, now: Time, new: NewItem) -> Result<(ItemNumber, WorkNumber, Vec<Todo>), Failed> {
        self.going()?;
        if let Some(on) = new.depends_on.iter().find(|on| !self.of_item.contains_key(*on)) {
            return Err(Failed::NoSuchItem(*on));
        }
        let Some(flow) = self.flows.get(&new.flow).cloned() else {
            return Err(Failed::NoSuchFlow(new.flow));
        };
        let mut depends_on = new.depends_on;
        depends_on.sort();
        depends_on.dedup();
        let mut work = Work::default();
        let mut first = vec![Event::Paused {
            by: Pauser::Owner,
            why: None,
        }];
        first.extend(self.block(&flow, &depends_on, &work));
        first.extend(new.writes.into_iter().map(|write| Event::Wrote {
            block: write.block,
            by: Outsider::Person,
            content: write.content,
            matters: true,
            value: write.value,
        }));
        let mut effects = Vec::new();
        for event in first {
            effects.extend(handle(&flow, &mut work, now, event).map_err(Failed::Refused)?);
        }
        let (item, number) = (self.store)
            .add(&new.title, &new.level, &depends_on, &new.flow, now, &work)
            .map_err(|fault| self.stop(Fault::Store(fault), false))?;
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
                level: Some(new.level),
                added: now,
                depends_on,
            },
            standing: standing_of(&flow, &work),
            flow,
            work,
        };
        let todos = effects.into_iter().map(|effect| held.todo(effect)).collect();
        self.works.insert(number, held);
        self.of_item.insert(item, number);
        Ok((item, number, todos))
    }

    /// Puts an item at another level, which whoever asks has checked against the installation's levels, and says whether
    /// that changed it: an item at the level already is left as it is. Its work's rounds handed out from now on run on
    /// the new level; a round under way goes on as it was given. Nothing is to be done for it: a level decides nothing.
    pub fn set_level(&mut self, now: Time, item: ItemNumber, level: &str) -> Result<bool, Failed> {
        self.going()?;
        let Some(work) = self.of_item.get(&item).copied() else {
            return Err(Failed::NoSuchItem(item));
        };
        let held = self.works.get(&work).ok_or(Failed::NoSuchItem(item))?;
        if held.item.level.as_deref() == Some(level) {
            return Ok(false);
        }
        if let Err(fault) = self.store.set_level(item, level, now) {
            return Err(self.stop(Fault::Store(fault), false));
        }
        Ok(true)
    }

    /// Takes one thing that happened to a work, at the time the caller's clock gives, and gives what is to be done:
    /// for that work, and, where the work ended with it, for the works whose items depend on its item.
    pub fn happened(&mut self, now: Time, work: WorkNumber, event: Event) -> Result<Vec<Todo>, Failed> {
        self.going()?;
        let Some(held) = self.works.get_mut(&work) else {
            return Err(Failed::NoSuchWork(work));
        };
        let was = held.standing;
        let item = held.head.item;
        let mut todos = Vec::new();
        match step(self.store.as_mut(), held, now, event) {
            Ok(effects) => todos.extend(effects.into_iter().map(|effect| held.todo(effect))),
            Err(Halt::Refused(refused)) => return Err(Failed::Refused(refused)),
            Err(Halt::Store(fault)) => return Err(self.stop(Fault::Store(fault), false)),
        }
        if was != Standing::Going || !matches!(held.standing, Standing::Ended(_)) {
            return Ok(todos);
        }
        // The item has merged or was cancelled: that is written to each work whose item depends on it. What happened
        // is kept by now, whatever becomes of what follows from it.
        let mut wanted: BTreeMap<WorkNumber, Event> = (self.works.iter())
            .filter(|(_, held)| held.standing == Standing::Going && held.item.depends_on.contains(&item))
            .filter_map(|(number, held)| self.wanted(held).map(|event| (*number, event)))
            .collect();
        let mut failed = None;
        for (number, held) in &mut self.works {
            let Some(event) = wanted.remove(number) else {
                continue;
            };
            match step(self.store.as_mut(), held, now, event) {
                Ok(effects) => todos.extend(effects.into_iter().map(|effect| held.todo(effect))),
                Err(Halt::Store(fault)) => failed = Some(Fault::Store(fault)),
                Err(Halt::Refused(refused)) => failed = Some(Fault::Refused { work: *number, refused }),
            }
            if failed.is_some() {
                break;
            }
        }
        match failed {
            Some(fault) => Err(self.stop(fault, true)),
            None => Ok(todos),
        }
    }

    /// Takes what a source outside the works says now, at the time the caller's clock gives: it is written into the
    /// block of that source of each work that has not ended, where the block does not say it already, as a change of
    /// its meaning; and gives what is to be done. A work whose flow has no block of the source is written nothing, and
    /// [`DEPENDENCIES`], the driver's own source, is written by the driver alone. Fails only where the driver is
    /// stopped, or stops on it.
    pub fn outside(&mut self, now: Time, source: &str, content: &str) -> Result<Vec<Todo>, Fault> {
        if let Some(fault) = &self.fault {
            return Err(fault.clone());
        }
        if source == DEPENDENCIES {
            return Ok(Vec::new());
        }
        let mut todos = Vec::new();
        let (mut failed, mut written) = (None, false);
        for (number, held) in &mut self.works {
            if held.standing != Standing::Going {
                continue;
            }
            let ours = (held.flow.blocks.iter()).find(|(_, b)| b.writers.outside.as_deref() == Some(source));
            let Some((block, _)) = ours else {
                continue;
            };
            let block = block.clone();
            let versions = decide::versions(&held.work.records);
            let current = versions.get(&block).and_then(|versions| versions.last());
            let says = current.and_then(|version| held.work.contents.get(&(block.clone(), version.at)));
            if says.map(String::as_str) == Some(content) {
                continue;
            }
            let event = Event::Wrote {
                block,
                by: Outsider::Outside,
                content: content.to_string(),
                matters: true,
                value: None,
            };
            match step(self.store.as_mut(), held, now, event) {
                Ok(effects) => {
                    written = true;
                    todos.extend(effects.into_iter().map(|effect| held.todo(effect)));
                }
                Err(Halt::Store(fault)) => failed = Some(Fault::Store(fault)),
                Err(Halt::Refused(refused)) => failed = Some(Fault::Refused { work: *number, refused }),
            }
            if failed.is_some() {
                break;
            }
        }
        match failed {
            // What was written before the fault is kept; being said again, it is written nowhere twice.
            Some(fault) => {
                self.stop(fault.clone(), written);
                Err(fault)
            }
            None => Ok(todos),
        }
    }

    /// What was to be done for the works and is not yet known as done, in the order of the works.
    pub fn pending(&self) -> Vec<Todo> {
        self.shown()
            .flat_map(|held| (engine::pending(&held.work).into_iter()).map(|effect| held.todo(effect)))
            .collect()
    }

    /// Every work with its item, in the order of the works' numbers.
    pub fn works(&self) -> impl Iterator<Item = Kept<'_>> {
        self.shown().map(Held::kept)
    }

    /// The work of this number.
    pub fn work(&self, number: WorkNumber) -> Option<Kept<'_>> {
        self.shown().find(|held| held.head.number == number).map(Held::kept)
    }

    /// The flows the driver was given, in the order of their names.
    pub fn flows(&self) -> impl Iterator<Item = &Compiled> {
        self.flows.values().map(|flow| &**flow)
    }

    /// The work as it is now: what it does next, each cell's state, every version of every block.
    pub fn view(&self, number: WorkNumber) -> Option<View> {
        let kept = self.work(number)?;
        Some(decide::view(&kept.flow.flow, &kept.work.records))
    }

    /// What stopped the driver, once something has.
    pub fn fault(&self) -> Option<&Fault> {
        self.fault.as_ref()
    }

    /// The works the driver shows: all of them, and none once it is stopped, when what it holds may be ahead of
    /// what the store holds.
    fn shown(&self) -> impl Iterator<Item = &Held> {
        self.works.values().filter(|_| self.fault.is_none())
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

    /// The write that brings the work's [`DEPENDENCIES`] block to say what it is to say, where it says otherwise.
    fn wanted(&self, held: &Held) -> Option<Event> {
        self.block(&held.flow, &held.item.depends_on, &held.work)
    }

    /// The write of the flow's [`DEPENDENCIES`] block, for a work whose item depends on these items, that makes it
    /// say whether they have all merged, and which it waits for; none where the flow has no such block, or the block
    /// says so already.
    fn block(&self, flow: &Compiled, depends_on: &[ItemNumber], work: &Work) -> Option<Event> {
        let (block, _) = (flow.blocks.iter()).find(|(_, b)| b.writers.outside.as_deref() == Some(DEPENDENCIES))?;
        let standing = |on: &ItemNumber| {
            let work = self.of_item.get(on).and_then(|work| self.works.get(work));
            work.map_or(Standing::Going, |held| held.standing)
        };
        let waited: Vec<String> = (depends_on.iter())
            .filter_map(|on| match standing(on) {
                Standing::Ended(End::Merged) => None,
                Standing::Ended(End::Cancelled) => Some(format!("{} (cancelled)", on.0)),
                Standing::Going | Standing::AtFault => Some(on.0.to_string()),
            })
            .collect();
        let (value, content) = if waited.is_empty() {
            (MERGED, "Every item this one depends on has merged.".to_string())
        } else {
            (WAITING, format!("Waits for the items {} to merge.", waited.join(", ")))
        };
        let versions = decide::versions(&work.records);
        let current = versions.get(block).and_then(|versions| versions.last());
        let says = current.map(|version| {
            (
                version.value.as_deref(),
                work.contents.get(&(block.clone(), version.at)),
            )
        });
        if says == Some((Some(value), Some(&content))) {
            return None;
        }
        Some(Event::Wrote {
            block: block.clone(),
            by: Outsider::Outside,
            content,
            matters: true,
            value: Some(value.to_string()),
        })
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

    /// Something to be done for this work.
    fn todo(&self, effect: Effect) -> Todo {
        Todo {
            work: self.head.number,
            effect,
        }
    }
}

/// Gives one thing that happened to the engine's loop for the work, and keeps what the loop added before giving back
/// what is to be done.
fn step(store: &mut dyn Storage, held: &mut Held, now: Time, event: Event) -> Result<Vec<Effect>, Halt> {
    let before = held.work.tick();
    let effects = handle(&held.flow, &mut held.work, now, event).map_err(Halt::Refused)?;
    if held.work.tick() != before {
        store.save(held.head.number, &held.work).map_err(Halt::Store)?;
        held.standing = standing_of(&held.flow, &held.work);
    }
    Ok(effects)
}

/// How the work stands.
fn standing_of(flow: &Compiled, work: &Work) -> Standing {
    match decide(&flow.flow, &work.records) {
        Decision::Ended(end) => Standing::Ended(end),
        Decision::Fault(_) => Standing::AtFault,
        _ => Standing::Going,
    }
}
