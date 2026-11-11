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
//! one waits for good, its block saying which: the owner cancels it, takes that dependency away, or leaves it.
//!
//! Whoever plans the work changes an item after it was added: its title, its level, and, while its work has started no
//! run, what it depends on ([`Driver::change`], module [`changes`]). Each change is kept as a record of the item's own;
//! a change of what it depends on writes the work's block of [`DEPENDENCIES`] anew, as an add does.
//!
//! The driver writes one more block of a work: the block of a flow whose source is [`SESSION`], which a cell reads that
//! loads into a line's session what the line's other cells work from. As a line of a work begins a new session, the
//! driver records it and writes the block, so that the cell runs again before the line goes on; when a line begins one
//! is the installation's [`Policy`], worked out of the work's records after each thing that happens and before the walk
//! (module [`sessions`]).
//!
//! What another source outside the works says, such as the head of the project's main branch, the driver is told
//! ([`Driver::outside`]), and writes into the block of that source of each work that has not ended, where the block
//! does not say it already.
//!
//! The driver is also the scheduler: it alone sees every work, and so it works out what holds back every work at once
//! and gives each work's loop that room ([`Room`]). The owner's pause of everything is kept in the store, so that it
//! holds across a stop. The slots bound how many rounds of cells that take one go on at once: a work is given a slot
//! where fewer are taken by the other works than there are; a work whose round ends keeps the slot it held for its next
//! round, if it begins one at once, as its session is then warm; and a slot that comes free otherwise is given to the
//! works that wait for one in the engine's order, those that have begun a round before those that have not, each in the
//! order of their numbers, so that what was begun is carried on before something new begins. No work waits for a slot
//! while one is free: each call gives away what came free before it returns. A work is added paused, and goes on once
//! the owner lets it on, so that whoever plans orders what runs.
//!
//! What happens to one work also reaches every other where a round of an agent failed for a cause every work shares,
//! such as the account's usage limit: no agent's round of any work begins while that wait stands ([`outage`]). It is
//! worked out from the records of every work, and given to the engine's loop for each work as a [`Gate`]; once it
//! changes, every work is looked at again. When it is over by itself, at a time, is the driver's to say ([`Driver::wake`])
//! and its caller's, which reads the clock, to tell it ([`Driver::tick`]), with what a look at the network found where
//! the driver asks for one.
//!
//! What the driver holds of a work in memory is what the store holds of it: a work is saved each time it grows, and
//! what is to be done is given out only once the store holds the entries that say so. Stopped at any moment and
//! [opened](Driver::open) again, the driver gives again everything that was to be done and is not known as done
//! (principle `effects-known-as-done`). When the store fails the driver stops: it gives the fault to every later call
//! and shows nothing more of the works, which it may hold otherwise than the store does, and is to be opened again
//! (principle `nothing-passes-for-normal`).
//!
//! What needs whoever plans the work since a mark they keep, read from the records of every work, is [`attention`].

pub mod attention;

pub mod changes;

pub mod sessions;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

pub mod outage;

use decide::{BlockId, Decision, End, Holder, Outsider, Pauser, Room, View, begun, decide, taking_slot};
use engine::{Effect, Event, Gate, Refused, handle_held, handle_then, held};
use flow::Compiled;
pub use outage::{Outage, Until};
use store::{Change, First, Item, ItemChange, ItemNumber, Storage, WorkHead, WorkNumber};
use work::{Time, Work};

pub use changes::{Gone, Unchangeable};
pub use sessions::{Policy, SESSION};

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
    /// How many rounds of cells that take a slot may go on at once.
    slots: usize,
    /// When the owner's pause of everything began, while it holds, as the store keeps it.
    paused: Option<Time>,
    /// What stopped the driver, once something has.
    fault: Option<Fault>,
    /// When a line begins a new session; with none, only what others record of sessions is followed.
    policy: Option<Policy>,
    /// The latest look at the network, while the agents' rounds wait for it: when, and whether it was reached.
    looked: Option<(Time, bool)>,
    /// When the driver was last told the time it asked for: a wake at or before it has been acted on. Lost in a stop,
    /// when a wake whose time has come is acted on again.
    ticked: Option<Time>,
}

/// When the driver is to be told the time again: the agents' rounds may begin then, or, where `look`, the network is
/// to be looked at then, and what the look found told with the time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Wake {
    pub at: Time,
    pub look: bool,
}

/// The slots, as they stand.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slots {
    /// How many rounds of cells that take a slot may go on at once.
    pub slots: usize,
    /// How many such rounds are under way: one for each work that has one, a round being stopped among them until it
    /// has ended. More may be under way than there are slots, where the slots were fewer when the engine started again.
    pub taken: usize,
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
enum Halt<R = Refused> {
    /// The engine's loop refused what happened, and nothing was changed.
    Refused(R),
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
    /// An item was to be added, or given a title, with a title of no words. Nothing was changed.
    BlankTitle,
    /// The item may not be changed so, for this reason. Nothing was changed.
    Unchangeable { item: ItemNumber, why: Unchangeable },
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
    /// Opens the driver on a store, with the flows the works run on, this many slots and the policy of when a line
    /// begins a new session, and gives everything that is to be done: what was to be done before and is not known as
    /// done, and what follows from looking at every work now, the free slots given in the engine's order.
    pub fn open(
        store: Box<dyn Storage + Send>,
        flows: Vec<Compiled>,
        slots: usize,
        policy: Option<Policy>,
        now: Time,
    ) -> Result<(Driver, Vec<Todo>), Fault> {
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
        let paused = store.everything_paused().map_err(Fault::Store)?;
        let mut driver = Driver {
            store,
            flows: by_name,
            works: BTreeMap::new(),
            of_item: BTreeMap::new(),
            slots,
            paused,
            fault: None,
            policy,
            looked: None,
            ticked: None,
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
        // to; then each work that has not ended has its block written where it says otherwise, and is looked at, given
        // no slot, so that the free slots go to the works in the engine's order once all have been looked at.
        let wanted: Vec<(WorkNumber, Option<Event>)> = (driver.works.iter())
            .map(|(number, held)| (*number, driver.wanted(held)))
            .collect();
        for (number, write) in wanted {
            for event in write.into_iter().chain([Event::Look]) {
                let (room, gate) = (driver.room(Some(number), driver.trial(now)), driver.gate(now));
                let Driver {
                    store, works, policy, ..
                } = &mut driver;
                let Some(held) = works.get_mut(&number).filter(|held| held.standing == Standing::Going) else {
                    continue;
                };
                match step(store.as_mut(), held, policy.as_ref(), now, event, room, gate) {
                    Ok(effects) => todos.extend(effects.into_iter().map(|effect| held.todo(effect))),
                    Err(Halt::Store(fault)) => return Err(Fault::Store(fault)),
                    Err(Halt::Refused(refused)) => return Err(Fault::Refused { work: number, refused }),
                }
            }
        }
        match driver.fill(now) {
            Ok(given) => todos.extend(given),
            Err(Halt::Store(fault)) => return Err(Fault::Store(fault)),
            Err(Halt::Refused(refused)) => return Err(refused),
        }
        Ok((driver, todos))
    }

    /// Adds an item together with its one work and with what people write for it first, all of it or none (entry
    /// `item-added-with-its-work`), and gives their numbers with what is to be done. The work is added paused, so that
    /// it begins no round until the owner lets it on. What is kept of it from its start is, in this order, its pause,
    /// what the driver says of the item's dependencies, what people wrote, and what follows from looking at it then.
    ///
    /// The driver keeps no key of the caller's for an add: a caller that was stopped before it had the numbers finds
    /// the item among those the driver shows, or adds it again as another item.
    pub fn add(&mut self, now: Time, new: NewItem) -> Result<(ItemNumber, WorkNumber, Vec<Todo>), Failed> {
        self.going()?;
        if new.title.trim().is_empty() {
            return Err(Failed::BlankTitle);
        }
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
        // What is written as the item is added is taken in a room that begins nothing, so that no run starts before the
        // last of it is in: under a pause a run starts only for a note to a cell that ran before, and none has. The
        // work is then looked at in the room the engine leaves it, a slot free being one no work waits for.
        let mut effects = Vec::new();
        let taking = Room {
            paused: true,
            slot: false,
        };
        let gate = self.gate(now);
        for event in first {
            effects.extend(handle_held(&flow, &mut work, now, event, taking, gate).map_err(Failed::Refused)?);
        }
        let room = self.room(None, true);
        effects.extend(handle_held(&flow, &mut work, now, Event::Look, room, gate).map_err(Failed::Refused)?);
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
            item: Item::new(
                item,
                now,
                First {
                    title: new.title,
                    level: Some(new.level),
                    depends_on,
                },
                Vec::new(),
            ),
            standing: standing_of(&flow, &work),
            flow,
            work,
        };
        let todos = effects.into_iter().map(|effect| held.todo(effect)).collect();
        self.works.insert(number, held);
        self.of_item.insert(item, number);
        Ok((item, number, todos))
    }

    /// Changes an item, as whoever plans the work asks, and says whether that changed it, with what is to be done: a
    /// change that leaves the item as it is keeps nothing. A change is refused where [`changes::judge`] refuses it,
    /// and a level is one whoever asks has checked against the installation's levels. The change is kept as the item's
    /// own record before anything follows from it. Where the item comes to depend on another or no longer does, its
    /// work's block of [`DEPENDENCIES`] is written to say what it now waits for, and the work is looked at; a level or
    /// a title decides nothing now: the work's rounds handed out from now on run on the new level, and where it gives a
    /// line another model, the line begins a new session as the next thing that happens to the work is taken.
    pub fn change(&mut self, now: Time, item: ItemNumber, change: ItemChange) -> Result<(bool, Vec<Todo>), Failed> {
        self.going()?;
        let Some(work) = self.of_item.get(&item).copied() else {
            return Err(Failed::NoSuchItem(item));
        };
        let held = self.works.get(&work).ok_or(Failed::NoSuchItem(item))?;
        let gone = match held.standing {
            Standing::Ended(end) => Gone::Ended(end),
            _ if held.work.records.started.is_empty() => Gone::NotStarted,
            _ => Gone::Started,
        };
        let depends_on = |on: ItemNumber| {
            let work = self.of_item.get(&on)?;
            self.works.get(work).map(|held| held.item.depends_on.clone())
        };
        let changes = changes::judge(&held.item, gone, &change, depends_on).map_err(|refused| match refused {
            changes::Refused::NoSuchItem(on) => Failed::NoSuchItem(on),
            changes::Refused::BlankTitle => Failed::BlankTitle,
            changes::Refused::Unchangeable(why) => Failed::Unchangeable { item, why },
        })?;
        if !changes {
            return Ok((false, Vec::new()));
        }
        if let Err(fault) = self.store.change(item, &change, now) {
            return Err(self.stop(Fault::Store(fault), false));
        }
        let dependencies = matches!(change, ItemChange::DependsOn(_) | ItemChange::NoLongerDependsOn(_));
        let Some(held) = self.works.get_mut(&work) else {
            return Err(Failed::NoSuchItem(item));
        };
        held.item.changed(Change { at: now, what: change });
        let mut todos = Vec::new();
        if dependencies && let Some(event) = self.wanted(&self.works[&work]) {
            // The change is kept by now, whatever becomes of what follows from it.
            if let Err(fault) = self.step_one(now, work, event, false, &mut todos) {
                return Err(self.stop(fault, true));
            }
        }
        self.filled_after(now, todos).map(|todos| (true, todos))
    }

    /// Takes one thing that happened to a work, at the time the caller's clock gives, and gives what is to be done:
    /// for that work, and, where the work ended with it, for the works whose items depend on its item; and gives the
    /// slots that came free to the works that wait for one.
    pub fn happened(&mut self, now: Time, work: WorkNumber, event: Event) -> Result<Vec<Todo>, Failed> {
        self.going()?;
        if !self.works.contains_key(&work) {
            return Err(Failed::NoSuchWork(work));
        }
        // A work whose round ends keeps the slot it held for the round it begins next.
        let room = self.room(Some(work), true);
        // A round that fails for a cause every work shares begins the wait as it is taken: its own work's next round
        // is held by it as every other's.
        let begins_a_wait = match &event {
            Event::Failed { run, failure, .. } if failure.shared() => {
                let r = &self.works[&work].work.records;
                let open = r.started.iter().any(|s| s.run == *run)
                    && !r.finished.iter().any(|f| f.run == *run)
                    && !r.abandoned.iter().any(|a| a.run == *run);
                open.then_some(*failure)
            }
            _ => None,
        };
        let outage = self.outage();
        let gate = match begins_a_wait {
            Some(failure) => held_by(failure),
            None => self.gate(now),
        };
        let Some(held) = self.works.get_mut(&work) else {
            return Err(Failed::NoSuchWork(work));
        };
        let was = held.standing;
        let item = held.head.item;
        let mut todos = Vec::new();
        match step(self.store.as_mut(), held, self.policy.as_ref(), now, event, room, gate) {
            Ok(effects) => todos.extend(effects.into_iter().map(|effect| held.todo(effect))),
            Err(Halt::Refused(refused)) => return Err(Failed::Refused(refused)),
            Err(Halt::Store(fault)) => return Err(self.stop(Fault::Store(fault), false)),
        }
        // A wait every work shares began, ended or moved with it: every work is looked at again. What happened is
        // kept by now, whatever becomes of what follows from it.
        if self.outage() != outage {
            match self.look(now) {
                Ok(more) => todos.extend(more),
                Err(fault) => return Err(self.stop(fault, true)),
            }
        } else if begins_a_wait.is_some() {
            // The wait it would have begun did not begin, or stood already as it was: the work is looked at again as
            // things stand, so that nothing of it is left held for nothing.
            if let Err(fault) = self.step_one(now, work, Event::Look, true, &mut todos) {
                return Err(self.stop(fault, true));
            }
        }
        let held = &self.works[&work];
        if was != Standing::Going || !matches!(held.standing, Standing::Ended(_)) {
            return self.filled_after(now, todos);
        }
        // The item has merged or was cancelled: that is written to each work whose item depends on it. What happened
        // is kept by now, whatever becomes of what follows from it.
        let mut wanted: BTreeMap<WorkNumber, Event> = (self.works.iter())
            .filter(|(_, held)| held.standing == Standing::Going && held.item.depends_on.contains(&item))
            .filter_map(|(number, held)| self.wanted(held).map(|event| (*number, event)))
            .collect();
        let numbers: Vec<WorkNumber> = self.works.keys().copied().collect();
        for number in numbers {
            let Some(event) = wanted.remove(&number) else {
                continue;
            };
            if let Err(fault) = self.step_one(now, number, event, false, &mut todos) {
                return Err(self.stop(fault, true));
            }
        }
        self.filled_after(now, todos)
    }

    /// Sets the owner's pause of everything, or lifts it, at the time the caller's clock gives, and gives what is to be
    /// done. While it holds no work begins a round but those of a run that answers a note, as a work's own pause holds
    /// them back, and a round under way goes on; it is kept in the store, and holds when the driver is opened again.
    /// Lifted, every work goes on that its own pause does not hold, the free slots given in the engine's order. Setting
    /// it while it holds, or lifting it while it does not, changes nothing.
    pub fn pause_everything(&mut self, now: Time, paused: bool) -> Result<Vec<Todo>, Failed> {
        self.going()?;
        if self.paused.is_some() == paused {
            return Ok(Vec::new());
        }
        if let Err(fault) = self.store.pause_everything(paused, now) {
            return Err(self.stop(Fault::Store(fault), false));
        }
        self.paused = paused.then_some(now);
        if paused {
            return Ok(Vec::new());
        }
        match self.look(now) {
            Ok(todos) => self.filled_after(now, todos),
            Err(fault) => Err(self.stop(fault, true)),
        }
    }

    /// The wait every work shares, if one stands: worked out from the records of every work ([`outage`]).
    pub fn outage(&self) -> Option<Outage> {
        outage::outage((self.works.values()).map(|held| (held.head.number, &*held.flow, &held.work)))
    }

    /// When the driver is to be told the time again, where the wait every work shares is over then by itself, or the
    /// network is to be looked at; none where nothing waits for a time. A wake whose time has come is given until the
    /// driver is told that time, and none at or before the time it was last told: the works were looked at then, and
    /// what still waits waits for a round to end.
    pub fn wake(&self) -> Option<Wake> {
        let ticked = self.ticked;
        self.next_wake().filter(|wake| ticked.is_none_or(|at| wake.at > at))
    }

    /// When the driver is to be told the time next, as the wait every work shares says, whether or not it has come.
    fn next_wake(&self) -> Option<Wake> {
        let outage = self.outage()?;
        match outage.until() {
            Until::At(at) => Some(Wake { at, look: false }),
            Until::Tried => None,
            Until::Trial { at, look: false, .. } => Some(Wake { at, look: false }),
            Until::Trial {
                at, look: true, latest, ..
            } => {
                let from = outage.last.unwrap_or(outage.since);
                let next = match self.looked {
                    Some((looked, _)) if looked >= from => Time(looked.0.saturating_add(outage::PROBE)),
                    _ => at,
                };
                Some(Wake {
                    at: next.min(latest),
                    look: next < latest,
                })
            }
        }
    }

    /// Takes the time, at which the driver asked to be told it ([`Driver::wake`]), and, where it asked for a look at
    /// the network, whether the look reached it: every work is looked at again, and what is to be done is given.
    pub fn tick(&mut self, now: Time, reached: Option<bool>) -> Result<Vec<Todo>, Fault> {
        if let Some(fault) = &self.fault {
            return Err(fault.clone());
        }
        if let Some(reached) = reached {
            self.looked = Some((now, reached));
        }
        self.ticked = Some(self.ticked.map_or(now, |at| at.max(now)));
        match self.look(now) {
            Ok(todos) => self.filled(now, todos),
            Err(fault) => {
                self.stop(fault.clone(), true);
                Err(fault)
            }
        }
    }

    /// Whether the wait every work shares holds rounds now.
    pub fn holds(&self, now: Time) -> bool {
        self.gate(now) != Gate::Open
    }

    /// Which rounds may begin now, as the wait every work shares says.
    fn gate(&self, now: Time) -> Gate {
        let reached = self.looked.filter(|(_, reached)| *reached).map(|(at, _)| at);
        match self.outage() {
            Some(outage) if outage.holds(now, reached) => held_by(outage.failure),
            _ => Gate::Open,
        }
    }

    /// Looks at every work that has not ended, in the order of their numbers, each with the gate as it is then and
    /// given no slot, so that the free slots go to the works in the engine's order once all have been looked at; but of
    /// a wait that lets one round try, the first work to begin one takes it ([`Driver::trial`]).
    fn look(&mut self, now: Time) -> Result<Vec<Todo>, Fault> {
        let mut todos = Vec::new();
        let numbers: Vec<WorkNumber> = (self.works.iter())
            .filter(|(_, held)| held.standing == Standing::Going)
            .map(|(number, _)| *number)
            .collect();
        for number in numbers {
            let trial = self.trial(now);
            self.step_one(now, number, Event::Look, trial, &mut todos)?;
        }
        Ok(todos)
    }

    /// Whether the wait every work shares lets one round try now and none has taken it: the first work looked at that
    /// begins a round takes it, with a slot where its round takes one and one is free, so that a work that needs none,
    /// or comes after, does not take it in that work's stead.
    fn trial(&self, now: Time) -> bool {
        self.outage()
            .is_some_and(|outage| matches!(outage.until(), Until::Trial { .. }))
            && !self.holds(now)
    }

    /// Gives one thing that happened to a work that has not ended to the engine's loop, with the gate as it is now, in
    /// the room the engine leaves it: where `slot` says a slot may be given at all, the work keeps the one it holds.
    fn step_one(
        &mut self,
        now: Time,
        number: WorkNumber,
        event: Event,
        slot: bool,
        todos: &mut Vec<Todo>,
    ) -> Result<(), Fault> {
        let (room, gate) = (self.room(Some(number), slot), self.gate(now));
        let Some(held) = self
            .works
            .get_mut(&number)
            .filter(|held| held.standing == Standing::Going)
        else {
            return Ok(());
        };
        match step(self.store.as_mut(), held, self.policy.as_ref(), now, event, room, gate) {
            Ok(effects) => {
                todos.extend(effects.into_iter().map(|effect| held.todo(effect)));
                Ok(())
            }
            Err(Halt::Store(fault)) => Err(Fault::Store(fault)),
            Err(Halt::Refused(refused)) => Err(Fault::Refused { work: number, refused }),
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
        let (room, gate) = (self.room(None, false), self.gate(now));
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
            match step(self.store.as_mut(), held, self.policy.as_ref(), now, event, room, gate) {
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
            None => self.filled(now, todos),
        }
    }

    /// What was to be done for the works and is not yet known as done, in the order of the works.
    pub fn pending(&self) -> Vec<Todo> {
        self.shown()
            .flat_map(|held| (engine::pending(&held.flow, &held.work).into_iter()).map(|effect| held.todo(effect)))
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

    /// The work as it is now, in the room the engine leaves it: what it does next, each cell's state, every version of
    /// every block.
    pub fn view(&self, number: WorkNumber) -> Option<View> {
        let kept = self.work(number)?;
        Some(decide::view_in(
            &kept.flow.flow,
            &kept.work.records,
            self.room(Some(number), true),
        ))
    }

    /// What holds back the round the work would begin next: its own pause, the owner's pause of everything, or a slot;
    /// none where nothing does, the work would begin no round, or there is no such work.
    pub fn held(&self, number: WorkNumber) -> Option<Holder> {
        let kept = self.work(number)?;
        held(kept.flow, kept.work, self.room(Some(number), true))
    }

    /// The slots, as they stand.
    pub fn slots(&self) -> Slots {
        Slots {
            slots: self.slots,
            taken: self.taken(None),
        }
    }

    /// When the owner's pause of everything began, while it holds.
    pub fn everything_paused(&self) -> Option<Time> {
        self.paused
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

    /// How many works have a round under way that takes a slot, but this one. A work at fault takes none: nothing more
    /// is decided of it, so a slot it held would be held for good.
    fn taken(&self, but: Option<WorkNumber>) -> usize {
        (self.works.values())
            .filter(|held| Some(held.head.number) != but && held.standing != Standing::AtFault)
            .filter(|held| taking_slot(&held.flow.flow, &held.work.records))
            .count()
    }

    /// The room the engine leaves a work: the owner's pause of everything, as it stands; and, where `slot` says a slot
    /// may be given at all, whether fewer are taken by the other works than there are.
    fn room(&self, work: Option<WorkNumber>, slot: bool) -> Room {
        Room {
            paused: self.paused.is_some(),
            slot: slot && self.taken(work) < self.slots,
        }
    }

    /// Gives the free slots away ([`Driver::fill`]) after something that was kept, adding what is to be done to these
    /// todos; where that fails, the driver stops, what was asked having been kept, and the fault is given.
    fn filled(&mut self, now: Time, mut todos: Vec<Todo>) -> Result<Vec<Todo>, Fault> {
        let fault = match self.fill(now) {
            Ok(given) => {
                todos.extend(given);
                return Ok(todos);
            }
            Err(Halt::Store(fault)) => Fault::Store(fault),
            Err(Halt::Refused(fault)) => fault,
        };
        self.stop(fault.clone(), true);
        Err(fault)
    }

    /// The free slots given away after what happened was kept ([`Driver::filled`]), as the driver's calls that take
    /// something that happened answer.
    fn filled_after(&mut self, now: Time, todos: Vec<Todo>) -> Result<Vec<Todo>, Failed> {
        self.filled(now, todos)
            .map_err(|fault| Failed::Stopped { fault, kept: true })
    }

    /// Gives each free slot to the first work in the engine's order that waits for one, and gives what is to be done:
    /// the works that have begun a round before those that have not, each in the order of their numbers. A work given
    /// a slot is looked at; each is looked at once, so that one that turns out to begin no round does not take the
    /// turn of the others.
    fn fill(&mut self, now: Time) -> Result<Vec<Todo>, Halt<Fault>> {
        let mut todos = Vec::new();
        let mut looked = BTreeSet::new();
        while self.taken(None) < self.slots {
            let waiting = self.room(None, false);
            let next = (self.works.values())
                .filter(|held| held.standing == Standing::Going && !looked.contains(&held.head.number))
                .filter(|held| engine::held(&held.flow, &held.work, waiting) == Some(Holder::Slot))
                .min_by_key(|held| (!begun(&held.flow.flow, &held.work.records), held.head.number))
                .map(|held| held.head.number);
            let Some(number) = next else {
                break;
            };
            looked.insert(number);
            let (room, gate) = (self.room(Some(number), true), self.gate(now));
            let Some(held) = self.works.get_mut(&number) else {
                break;
            };
            match step(
                self.store.as_mut(),
                held,
                self.policy.as_ref(),
                now,
                Event::Look,
                room,
                gate,
            ) {
                Ok(effects) => todos.extend(effects.into_iter().map(|effect| held.todo(effect))),
                Err(Halt::Store(fault)) => return Err(Halt::Store(fault)),
                Err(Halt::Refused(refused)) => return Err(Halt::Refused(Fault::Refused { work: number, refused })),
            }
        }
        Ok(todos)
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

/// Gives one thing that happened to the engine's loop for the work, in the room the engine leaves it and with the gate
/// of the wait every work shares, with what follows from it for the sessions of its lines, and keeps what the loop
/// added before giving back what is to be done.
fn step(
    store: &mut dyn Storage,
    held: &mut Held,
    policy: Option<&Policy>,
    now: Time,
    event: Event,
    room: Room,
    gate: Gate,
) -> Result<Vec<Effect>, Halt> {
    let before = held.work.tick();
    let item = &held.item;
    let then = |flow: &Compiled, work: &Work| sessions::follow(flow, work, item, policy);
    let effects = handle_then(&held.flow, &mut held.work, now, event, room, gate, &then).map_err(Halt::Refused)?;
    if held.work.tick() != before {
        store.save(held.head.number, &held.work).map_err(Halt::Store)?;
        held.standing = standing_of(&held.flow, &held.work);
    }
    Ok(effects)
}

/// The rounds a wait for a cause of this kind holds: without the network every round, the engine's own acts' too, which
/// may reach it; otherwise the agents'.
fn held_by(failure: decide::run::Failure) -> Gate {
    match failure {
        decide::run::Failure::Offline => Gate::Every,
        _ => Gate::Agents,
    }
}

/// How the work stands.
fn standing_of(flow: &Compiled, work: &Work) -> Standing {
    match decide(&flow.flow, &work.records) {
        Decision::Ended(end) => Standing::Ended(end),
        Decision::Fault(_) => Standing::AtFault,
        _ => Standing::Going,
    }
}
