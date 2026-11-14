//! The loop of the engine, for every work.
//!
//! The engine's own loop takes one thing that happened to one work and holds nothing (crate `engine`). The driver is
//! what takes the things that happen, in order, for all the works: it gives each to the
//! engine's loop for the work it happened to, keeps what the loop added in the store before anything is done about it,
//! and gives back what is to be done outside, as data. Whatever carries those out (the agents, the engine's own acts,
//! the tools) says what came of each by calling the driver again; the driver calls nothing outside but the store, and
//! reads no clock (principle `outside-behind-interfaces`).
//!
//! What happens to one work reaches another only as a write of a block of the other. The driver is itself what writes
//! one such block: the block of a flow whose source is [`DEPENDENCIES`] says whether the items the work's item
//! depends on have merged. The driver writes it as the item is added, whenever one of those items merges, is cancelled
//! or is reopened, and, on opening, wherever the block and what it stands for have come apart; it writes no block of a
//! work that has ended. A work whose flow has no such block waits for no item. No item is cancelled while an item
//! that has not ended depends on it, and none comes to depend on a cancelled one; an item that depends on a cancelled
//! one, as one kept before that held, waits for good, its block saying which.
//!
//! Whoever plans the work changes items after they were added, several at once all or none, each change saying why:
//! an item's title, level and tags, and, while its work has started no run, what it depends on; its cancel; and the
//! reopening of a cancelled item ([`Driver::change`], module [`changes`]). A change of an item is kept as a record of
//! the item's own, a cancel and a reopen as the work's; a change of what an item depends on writes the work's block of
//! [`DEPENDENCIES`] anew, as an add does.
//!
//! The driver writes the block of a flow whose source is [`START`] as the owner starts the work: as the work, added
//! paused, is first let on with nothing of it started, or, reopened, is first let on since, so that the cell it runs,
//! the work's start, is entered and makes the place the work is done in again.
//!
//! The driver writes one more kind of block of a work: a block of a flow whose source is [`SESSION`], which a cell reads
//! that loads into a line's session what the line's other cells work from, each line its own. As a line of a work
//! begins a new session, the driver records it and writes the block its cells read, so that the cell runs again before
//! the line goes on; when a line begins one is the installation's [`Policy`], worked out of the work's records after
//! each thing that happens and before the walk (module [`sessions`]).
//!
//! What another source outside the works says of a project, such as the head of the project's main branch, the driver
//! is told ([`Driver::outside`]), and writes into the block of that source of each work of that project's items that
//! has not ended, where the block does not say it already. An item belongs to one project, and may depend on an item
//! of another: the block of [`DEPENDENCIES`] waits for it as for any other.
//!
//! The driver is also the scheduler: it alone sees every work, and so it works out what holds back every work at once
//! and gives each work's loop that room ([`Room`]). The pause of everything is kept in the store, so that it holds
//! across a stop, with who set it: the owner, or the driver itself as it opens on works going on, so that after a stop
//! nothing begins a round that the owner has not let on since they could see how things stand ([`Driver::open`]). The
//! slots bound how many rounds of cells that take one go on at once: a work is given a slot
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
//! changes, every work is looked at again. When it is over by itself, at a time, is the driver's to say
//! ([`Driver::wake`]) and its caller's, which reads the clock, to tell it ([`Driver::tick`]), with what a look at the
//! network found where the driver asks for one.
//!
//! What the driver holds of a work in memory is what the store holds of it: a work is saved each time it grows, and
//! what is to be done is given out only once the store holds the entries that say so. Stopped at any moment and
//! [opened](Driver::open) again, the driver gives again everything that was to be done and is not known as done
//! (principle `effects-known-as-done`). When the store fails the driver stops: it gives the fault to every later call
//! and shows nothing more of the works, which it may hold otherwise than the store does, and is to be opened again
//! (principle `nothing-passes-for-normal`).
//!
//! What needs whoever plans the work since a mark they keep, read from the records of every work, is [`attention`].
//!
//! The driver alone sees every work, so it is also what judges by history (principle `judged-by-own-history`): it
//! gathers what the rounds of every work on a flow came to ([`history`]), and gives each round the lines it is judged
//! by as it runs, and each line of a work the context past which its session begins anew. It gives the keep-warm pings
//! of the sessions of lines that wait as their time comes, the time being among what it asks to be told ([`warm`]);
//! and it tells a round that missed its session's prompt cache from the records ([`cache`]).

pub mod attention;

pub mod cache;

pub mod changes;

pub mod history;

pub mod sessions;

pub mod warm;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

pub mod outage;

use decide::run::Failure;
use decide::{BlockId, Decision, End, Holder, Outsider, Pauser, Room, View, begun, decide, taking_slot};
use engine::{Effect, Event, Gate, Refused, handle_held, handle_then, held};
use flow::Compiled;
use flow::agents::fill;
pub use outage::{Outage, Service, Until};
use store::{Change, EverythingPaused, First, Item, ItemChange, ItemNumber, Storage, WorkHead, WorkNumber};
use work::{Time, Work};

pub use changes::{Asked, Gone, NotMade, Planned, Unchangeable};
pub use sessions::{Limits, Policy, SESSION, Warm};

/// The source of the blocks the driver writes: whether the items the work's item depends on have merged. Such a block
/// is a choice of [`WAITING`] and [`MERGED`].
pub const DEPENDENCIES: &str = "dependencies";
/// The value of a [`DEPENDENCIES`] block while an item the work's item depends on has not merged.
pub const WAITING: &str = "waiting";
/// The value of a [`DEPENDENCIES`] block once every item the work's item depends on has merged, or it depends on none.
pub const MERGED: &str = "merged";

/// The source of the block the driver writes as the owner starts a work: the work, added paused, is let on with nothing
/// of it started. The block has no content to speak of; its version is what runs the cell that starts the work.
pub const START: &str = "start";

/// What is wrong with the flow for what the engine writes, each in words with the file that says what it writes into
/// the block: a block of [`DEPENDENCIES`], of [`SESSION`], of [`START`] or of [`engine::asked::ASKED`] gives in its
/// `writes` each text the engine fills, so that none is written empty. A flow with any of these is not run.
pub fn faults(flow: &Compiled) -> Vec<String> {
    use engine::asked::{ANSWERED, ASKED, WAITING as UNANSWERED};
    let session: Vec<&str> = ["text"].into_iter().chain(work::Why::KINDS).collect();
    let needs: [(&str, &[&str]); 4] = [
        (DEPENDENCIES, &[MERGED, WAITING, "cancelled", "between"]),
        (SESSION, &session),
        (START, &["text"]),
        (ASKED, &[ANSWERED, UNANSWERED, "between"]),
    ];
    let mut faults = Vec::new();
    for (name, block) in &flow.blocks {
        let Some((source, keys)) = needs
            .iter()
            .find(|(source, _)| block.writers.outside.as_deref() == Some(*source))
        else {
            continue;
        };
        let missing: Vec<&str> = keys
            .iter()
            .copied()
            .filter(|key| block.writes.get(key).is_none())
            .collect();
        if !missing.is_empty() {
            faults.push(format!(
                "the flow {}, {}: the block {name} (from {source}): its writes must give {}",
                flow.name,
                block.file,
                missing.join(", ")
            ));
        }
    }
    faults
}

/// Drives the engine for every work the store holds.
pub struct Driver {
    store: Box<dyn Storage + Send>,
    flows: BTreeMap<String, Arc<Compiled>>,
    works: BTreeMap<WorkNumber, Held>,
    /// The work of each item.
    of_item: BTreeMap<ItemNumber, WorkNumber>,
    /// How many rounds of cells that take a slot may go on at once.
    slots: usize,
    /// The pause of everything, while it holds, as the store keeps it.
    paused: Option<EverythingPaused>,
    /// What stopped the driver, once something has.
    fault: Option<Fault>,
    /// When a line begins a new session; with none, only what others record of sessions is followed.
    policy: Option<Policy>,
    /// The latest look at the network, while the agents' rounds wait for it: when, and whether it was reached.
    looked: Option<(Time, bool)>,
    /// The latest look at the acts' host, while the acts' rounds wait for it: when, and whether it was reached.
    looked_at_host: Option<(Time, bool)>,
    /// When the driver was last told the time it asked for: a wake at or before it has been acted on. Lost in a stop,
    /// when a wake whose time has come is acted on again.
    ticked: Option<Time>,
    /// Each round, of a work and run by its number, that the agents' service answered while it was under way. What
    /// the outside says at a moment, kept only in memory: lost in a stop, the round is given again and answers again.
    answered: BTreeSet<(WorkNumber, decide::RunId, usize)>,
    /// The keep-warm pings given and not yet said to have ended, by work and line ([`warm`]): while one of a work is,
    /// no round of the work begins. Lost in a stop, when a ping still due is given again.
    pinging: BTreeSet<(WorkNumber, String)>,
}

/// When the driver is to be told the time again: the rounds a wait holds may begin then, or, where `look`, the
/// network to the agents' service is to be looked at then, and where `look_at_host`, the network to the acts' host;
/// what each look found is told with the time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Wake {
    pub at: Time,
    pub look: bool,
    pub look_at_host: bool,
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
#[derive(Clone)]
struct Held {
    head: WorkHead,
    item: Item,
    flow: Arc<Compiled>,
    work: Work,
    /// How the work stands. Worked out again each time the work grows.
    standing: Standing,
    /// The work's rounds that are history, which later rounds of every work on its flow are judged by. Worked out
    /// again each time the work grows.
    samples: Vec<history::Sample>,
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
    /// The name of the project it belongs to, which whoever adds it has checked against the installation's projects.
    pub project: String,
    /// The items it depends on, of its own project or another.
    pub depends_on: Vec<ItemNumber>,
    /// The tags it carries.
    pub tags: Vec<String>,
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
    /// An item was to be added with a title of no words. Nothing was changed.
    BlankTitle,
    /// An item was to be added carrying a tag that is no name an item may carry. Nothing was changed.
    BadTag(String),
    /// An item was to be added depending on these items, which are cancelled: it would wait for them for good.
    /// Nothing was changed.
    OnCancelled(Vec<ItemNumber>),
    /// Changes asked together were refused, each of these for its reason. None of them was made.
    NotChanged { asked: usize, refused: Vec<NotMade> },
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
    ///
    /// Where a work is going on, one that has not ended and that its own pause does not hold, and nothing pauses
    /// everything, the driver pauses everything itself before it looks at any work, and keeps that it did: nothing
    /// begins a round then but what the owner's own pause of everything lets begin, until the owner lifts it as they
    /// lift their own. What was to be done is given all the same, a round under way at the stop among it: the records
    /// count that round as begun, and holding it back in its carrier until the lift would be the path the decision
    /// slots-in-the-scheduler turned down.
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
            looked_at_host: None,
            ticked: None,
            answered: BTreeSet::new(),
            pinging: BTreeSet::new(),
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
            let samples = history::samples(&flow, &work);
            let held = Held {
                head,
                item,
                flow,
                work,
                standing,
                samples,
            };
            driver.works.insert(held.head.number, held);
        }
        if let Some(item) = items.iter().find(|item| !driver.of_item.contains_key(&item.number)) {
            return Err(Fault::NotOneWork(item.number));
        }
        // A work going on would begin its rounds again unseen: everything is paused, and kept so, before anything is
        // looked at, so that a stop before the looks leaves it paused all the same. Where everything is paused, by the
        // owner or by an opening before, it is left as it is.
        let going_on =
            (driver.works.values()).any(|held| held.standing == Standing::Going && !decide::paused(&held.work.records));
        if going_on && driver.paused.is_none() {
            (driver.store)
                .pause_everything(true, Pauser::Engine, now)
                .map_err(Fault::Store)?;
            driver.paused = Some(EverythingPaused {
                since: now,
                by: Pauser::Engine,
            });
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
                let (room, gate) = (
                    driver.room(Some(number), driver.trial(now)),
                    driver.gate_of(now, number),
                );
                let limits = driver.limits(number);
                let Driver {
                    store, works, policy, ..
                } = &mut driver;
                let Some(held) = works.get_mut(&number).filter(|held| held.standing == Standing::Going) else {
                    continue;
                };
                match step(store.as_mut(), held, policy.as_ref(), &limits, now, event, room, gate) {
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
        if let Some(tag) = new.tags.iter().find(|tag| changes::tag_fault(tag).is_some()) {
            return Err(Failed::BadTag(tag.clone()));
        }
        let Some(flow) = self.flows.get(&new.flow).cloned() else {
            return Err(Failed::NoSuchFlow(new.flow));
        };
        let mut depends_on = new.depends_on;
        depends_on.sort();
        depends_on.dedup();
        let cancelled: Vec<ItemNumber> = (depends_on.iter().copied())
            .filter(|on| self.standing_of_item(*on) == Some(Standing::Ended(End::Cancelled)))
            .collect();
        if !cancelled.is_empty() {
            return Err(Failed::OnCancelled(cancelled));
        }
        let mut tags = new.tags;
        tags.sort();
        tags.dedup();
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
        let first = First {
            title: new.title,
            level: Some(new.level),
            project: Some(new.project),
            depends_on,
            tags,
        };
        let (item, number) = (self.store)
            .add(&first, &new.flow, now, &work)
            .map_err(|fault| self.stop(Fault::Store(fault), false))?;
        let held = Held {
            head: WorkHead {
                number,
                item,
                flow: new.flow,
                added: now,
            },
            item: Item::new(item, now, first, Vec::new()),
            standing: standing_of(&flow, &work),
            samples: history::samples(&flow, &work),
            flow,
            work,
        };
        let todos = effects.into_iter().map(|effect| held.todo(effect)).collect();
        self.works.insert(number, held);
        self.of_item.insert(item, number);
        Ok((item, number, todos))
    }

    /// Makes changes of items asked together, for one reason, as whoever plans the work asks, all of them or none, and
    /// says whether each changed its item, with what is to be done: a change that leaves its item as it is keeps
    /// nothing. The changes are refused where [`changes::judge`] refuses any, and a level is one whoever asks has
    /// checked against the installation's levels. Everything the changes keep is kept in one transaction before
    /// anything follows from it: the changes of the items, as their own records, and each cancel and reopen, as the
    /// records of the item's work, the work cancelled ending and abandoning what it had under way, the work reopened
    /// being paused by the owner, as a work added is. Then each work whose item came to depend on another or no longer
    /// does, or that was reopened, and each that waits for an item that was cancelled or reopened, has its block of
    /// [`DEPENDENCIES`] written to say what it now waits for, and is looked at; a level, a title or a tag decides
    /// nothing now: the work's rounds handed out from now on run on the new level, and where it gives a line another
    /// model, the line begins a new session as the next thing that happens to the work is taken.
    pub fn change(
        &mut self,
        now: Time,
        why: Option<&str>,
        asked: Vec<(ItemNumber, Asked)>,
    ) -> Result<(Vec<bool>, Vec<Todo>), Failed> {
        self.going()?;
        let planned: BTreeMap<ItemNumber, Planned> = (self.works.values())
            .map(|held| {
                let planned = Planned {
                    item: held.item.clone(),
                    started: !held.work.records.started.is_empty(),
                    ended: match held.standing {
                        Standing::Ended(end) => Some(end),
                        _ => None,
                    },
                    letting_go: engine::letting_go(&held.flow, &held.work),
                };
                (held.item.number, planned)
            })
            .collect();
        let changed = (changes::judge(&planned, &asked)).map_err(|refused| Failed::NotChanged {
            asked: asked.len(),
            refused,
        })?;
        // What the changes make, worked out on copies of what is held before anything is kept: the changes of the
        // items, and each work as its cancels and reopens leave it, with what is to be done for it.
        let mut items: Vec<(ItemNumber, ItemChange)> = Vec::new();
        let mut works: BTreeMap<WorkNumber, Held> = BTreeMap::new();
        let mut effects: Vec<Todo> = Vec::new();
        for ((item, asked), changes) in asked.into_iter().zip(&changed) {
            if !changes {
                continue;
            }
            let work = self.of_item[&item];
            let event = match asked {
                Asked::Item(change) => {
                    items.push((item, change));
                    continue;
                }
                Asked::Cancel => Event::Cancelled {
                    why: why.map(str::to_string),
                },
                Asked::Reopen => Event::Reopened {
                    why: why.unwrap_or_default().to_string(),
                },
            };
            let (room, gate) = (self.room(Some(work), true), self.gate_of(now, work));
            let limits = self.limits(work);
            let held = works.entry(work).or_insert_with(|| self.works[&work].clone());
            let (item, policy) = (&held.item, self.policy.as_ref());
            let then = |flow: &Compiled, work: &Work| {
                let mut events = sessions::follow(flow, work, item, policy, &limits);
                events.extend(start(flow, work));
                events
            };
            let done =
                handle_then(&held.flow, &mut held.work, now, event, room, gate, &then).map_err(Failed::Refused)?;
            held.standing = standing_of(&held.flow, &held.work);
            held.samples = history::samples(&held.flow, &held.work);
            effects.extend(done.into_iter().map(|effect| held.todo(effect)));
        }
        if items.is_empty() && works.is_empty() {
            return Ok((changed, Vec::new()));
        }
        let saved: Vec<(WorkNumber, &Work)> = works.iter().map(|(number, held)| (*number, &held.work)).collect();
        if let Err(fault) = self.store.change(now, why, &items, &saved) {
            return Err(self.stop(Fault::Store(fault), false));
        }
        // Kept: what is held is made what was kept.
        let mut touched: BTreeSet<ItemNumber> = BTreeSet::new();
        let mut dependencies: BTreeSet<WorkNumber> = BTreeSet::new();
        for (number, held) in works {
            touched.insert(held.item.number);
            dependencies.insert(number);
            self.works.insert(number, held);
        }
        for (item, change) in items {
            let work = self.of_item[&item];
            if matches!(change, ItemChange::DependsOn(_) | ItemChange::NoLongerDependsOn(_)) {
                dependencies.insert(work);
            }
            let why = why.map(str::to_string);
            if let Some(held) = self.works.get_mut(&work) {
                held.item.changed(Change {
                    at: now,
                    what: change,
                    why,
                });
            }
        }
        // What follows, the changes being kept by now whatever becomes of it: each work whose item depends on an item
        // cancelled or reopened, and each whose own dependencies changed or that was reopened, has its block written
        // where it says otherwise.
        let mut todos = effects;
        let waiting: Vec<WorkNumber> = (self.works.iter())
            .filter(|(number, held)| {
                dependencies.contains(number) || held.item.depends_on.iter().any(|on| touched.contains(on))
            })
            .map(|(number, _)| *number)
            .collect();
        for number in waiting {
            let Some(event) = self.works.get(&number).and_then(|held| self.wanted(held)) else {
                continue;
            };
            if let Err(fault) = self.step_one(now, number, event, false, &mut todos) {
                return Err(self.stop(fault, true));
            }
        }
        self.filled_after(now, todos).map(|todos| (changed, todos))
    }

    /// How the work of this item stands, where the driver has the item.
    fn standing_of_item(&self, item: ItemNumber) -> Option<Standing> {
        let work = self.of_item.get(&item)?;
        self.works.get(work).map(|held| held.standing)
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
                let held = &self.works[&work];
                let r = &held.work.records;
                let open = r.started.iter().any(|s| s.run == *run)
                    && !r.finished.iter().any(|f| f.run == *run)
                    && !r.abandoned.iter().any(|a| a.run == *run);
                let cell = r.started.iter().find(|s| s.run == *run).map(|s| &s.cell);
                let act = cell.is_some_and(|cell| {
                    (held.flow.cells.iter()).any(|spec| spec.id == *cell && spec.kind == flow::Kind::Engine)
                });
                open.then_some(if act { Gate::Acts } else { Gate::Agents })
            }
            _ => None,
        };
        // A round that fails for a model the service does not have stops every round on that model: what its try ran
        // on, as the try recorded it, with why.
        let no_model = match &event {
            Event::Failed {
                run,
                why,
                failure: Failure::NoModel,
                ..
            } => (self.works[&work].work.ran.iter().rev())
                .find(|ran| ran.run == *run)
                .map(|ran| (ran.on.clone(), why.clone())),
            _ => None,
        };
        let outages = self.outages();
        if let Event::Pinged { line, .. } = &event {
            self.pinging.remove(&(work, line.clone()));
        }
        let gate = match begins_a_wait {
            Some(held) => self.gate_of(now, work).and(held),
            None => self.gate_of(now, work),
        };
        let limits = self.limits(work);
        let Some(held) = self.works.get_mut(&work) else {
            return Err(Failed::NoSuchWork(work));
        };
        let was = held.standing;
        let item = held.head.item;
        let mut todos = Vec::new();
        match step(
            self.store.as_mut(),
            held,
            self.policy.as_ref(),
            &limits,
            now,
            event,
            room,
            gate,
        ) {
            Ok(effects) => todos.extend(effects.into_iter().map(|effect| held.todo(effect))),
            Err(Halt::Refused(refused)) => return Err(Failed::Refused(refused)),
            Err(Halt::Store(fault)) => return Err(self.stop(Fault::Store(fault), false)),
        }
        // A wait every work shares began, ended or moved with it: every work is looked at again. What happened is
        // kept by now, whatever becomes of what follows from it.
        if self.outages() != outages {
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
        // The round's own work is paused by its failure; every other that runs a round on the model is paused with it.
        if let Some((on, why)) = no_model {
            let why = format!(
                "the service has no model {}, which the level {} runs on, as a round of work {} found: {why}",
                on.model, on.level, work.0
            );
            let numbers: Vec<WorkNumber> = (self.works.iter())
                .filter(|(number, held)| **number != work && runs_on(held, self.policy.as_ref(), &on.model))
                .map(|(number, _)| *number)
                .collect();
            for number in numbers {
                let paused = Event::Paused {
                    by: Pauser::Engine,
                    why: Some(why.clone()),
                };
                if let Err(fault) = self.step_one(now, number, paused, false, &mut todos) {
                    return Err(self.stop(fault, true));
                }
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

    /// Sets the owner's pause of everything, or lifts the pause of everything, the owner's or the driver's own, at the
    /// time the caller's clock gives, and gives what is to be done. While it holds no work begins a round but those of
    /// a run that answers a note, as a work's own pause holds them back, and a round under way goes on; it is kept in
    /// the store, and holds when the driver is opened again. Lifted, every work goes on that its own pause does not
    /// hold, the free slots given in the engine's order. Set while the driver's own holds, it is the owner's from then
    /// on. Setting it while the owner's holds, or lifting it while none does, changes nothing.
    pub fn pause_everything(&mut self, now: Time, paused: bool) -> Result<Vec<Todo>, Failed> {
        self.going()?;
        let theirs = self.paused.is_some_and(|p| p.by == Pauser::Owner);
        if (paused && theirs) || (!paused && self.paused.is_none()) {
            return Ok(Vec::new());
        }
        if let Err(fault) = self.store.pause_everything(paused, Pauser::Owner, now) {
            return Err(self.stop(Fault::Store(fault), false));
        }
        self.paused = paused.then_some(EverythingPaused {
            since: now,
            by: Pauser::Owner,
        });
        if paused {
            return Ok(Vec::new());
        }
        match self.look(now) {
            Ok(todos) => self.filled_after(now, todos),
            Err(fault) => Err(self.stop(fault, true)),
        }
    }

    /// The history of the rounds of every work on the flow of this name ([`history`]).
    pub fn history(&self, flow: &str) -> Vec<history::Sample> {
        (self.works.values())
            .filter(|held| held.head.flow == flow)
            .flat_map(|held| held.samples.iter().cloned())
            .collect()
    }

    /// The rounds of the work that missed their session's prompt cache ([`cache`]); none for a work the driver does not
    /// hold, or where the agents' program keeps no cache.
    pub fn misses(&self, work: WorkNumber) -> Vec<cache::Miss> {
        let life = (self.policy.as_ref())
            .and_then(|policy| policy.warm.as_ref())
            .map(|warm| warm.life_ms);
        match (life, self.works.get(&work)) {
            (Some(life), Some(held)) => cache::misses(&held.flow, &held.work, life),
            _ => Vec::new(),
        }
    }

    /// The lines a round of the cell of the work is judged by while it runs: from the history of the rounds of every
    /// work on its flow, its cost among those on the model it runs on now ([`history::Lines`]). None are given without
    /// a policy, as for rounds carried out in no session.
    pub fn lines(&self, work: WorkNumber, cell: &flow::CellSpec) -> history::Lines {
        let (Some(policy), Some(held)) = (&self.policy, self.works.get(&work)) else {
            return history::Lines::default();
        };
        let model = (policy.on)(&held.item, cell).map(|on| on.model).unwrap_or_default();
        let mut lines = history::Lines::of(&self.history(&held.head.flow), &cell.id, &model, policy.samples);
        // What the flow's other works have come to in all, which a work is judged by as each of its rounds ends.
        let others: Vec<(f64, f64)> = (self.works.iter())
            .filter(|(number, other)| **number != work && other.head.flow == held.head.flow)
            .map(|(_, other)| history::totals(&other.work))
            .filter(|(spent, _)| *spent > 0.0)
            .collect();
        let spent: Vec<f64> = others.iter().map(|(spent, _)| *spent).collect();
        let time: Vec<f64> = others.iter().map(|(_, ms)| *ms).collect();
        lines.work_spent = history::works_line(&spent, policy.samples);
        lines.work_time = history::works_line(&time, policy.samples);
        lines
    }

    /// The line of the context each line of the work's flow ends its rounds with, where the history gives one: past it,
    /// the line's session begins anew ([`sessions`]). The rounds of the session judged, the line's in the work now, are
    /// no part of the history it is judged by, so that a session growing as it goes does not set its own line. None
    /// for a work the driver does not hold.
    fn limits(&self, work: WorkNumber) -> Limits {
        let Some(held) = self.works.get(&work) else {
            return Limits::new();
        };
        let (Some(policy), Some(compiled)) = (&self.policy, self.flows.get(&held.head.flow)) else {
            return Limits::new();
        };
        let mut limits = Limits::new();
        for line in compiled.cells.iter().filter_map(|cell| cell.line.as_deref()) {
            let judged = work::sessions::number_at(&held.work, line, held.work.tick());
            let others = (self.works.iter())
                .filter(|(_, other)| other.head.flow == held.head.flow)
                .flat_map(|(number, other)| {
                    let own = *number == work;
                    (other.samples.iter()).filter(move |s| !(own && s.session == Some(judged)))
                });
            if let Some(found) = history::context_line(others, line, policy.samples) {
                limits.insert(line.to_string(), found);
            }
        }
        limits
    }

    /// The wait every work shares for the agents' service, if one stands: worked out from the records of every work
    /// and the rounds the service answered while under way ([`outage`]).
    pub fn outage(&self) -> Option<Outage> {
        self.outage_of(Service::Agents)
    }

    /// The wait every work shares for the acts' host, if one stands: worked out from the records of every work.
    pub fn host_outage(&self) -> Option<Outage> {
        self.outage_of(Service::Host)
    }

    /// The wait of the service every work shares, if one stands.
    fn outage_of(&self, service: Service) -> Option<Outage> {
        let works = (self.works.values()).map(|held| (held.head.number, &*held.flow, &held.work));
        outage::outage(service, works, &self.answered)
    }

    /// The waits of every service, as they stand.
    fn outages(&self) -> (Option<Outage>, Option<Outage>) {
        (self.outage(), self.host_outage())
    }

    /// Takes, at the time the caller's clock gives, that the agents' service answered the round under way of the run:
    /// where a wait every work shares lets that round try, it is over, and every work is looked at again; what is to
    /// be done is given. A round no longer under way is passed over.
    pub fn answered(&mut self, now: Time, work: WorkNumber, run: decide::RunId) -> Result<Vec<Todo>, Fault> {
        if let Some(fault) = &self.fault {
            return Err(fault.clone());
        }
        let Some(held) = self.works.get(&work) else {
            return Ok(Vec::new());
        };
        let records = &held.work.records;
        let began = records.rounds.iter().filter(|r| r.run == run).count();
        let ended = records.returned.iter().filter(|r| r.run == run).count();
        if began == 0 || ended >= began {
            return Ok(Vec::new());
        }
        let outage = self.outage();
        self.answered.insert((work, run, began));
        if self.outage() == outage {
            return Ok(Vec::new());
        }
        match self.look(now) {
            Ok(todos) => self.filled(now, todos),
            Err(fault) => {
                self.stop(fault.clone(), true);
                Err(fault)
            }
        }
    }

    /// When the driver is to be told the time again, where the wait every work shares is over then by itself, or the
    /// network is to be looked at; none where nothing waits for a time. A wake whose time has come is given until the
    /// driver is told that time, and none at or before the time it was last told: the works were looked at then, and
    /// what still waits waits for a round to end.
    pub fn wake(&self) -> Option<Wake> {
        let ticked = self.ticked;
        self.next_wake().filter(|wake| ticked.is_none_or(|at| wake.at > at))
    }

    /// When the driver is to be told the time next, as the waits every work shares say, whether or not it has come:
    /// the earliest of each wait's and of the next keep-warm ping's, with each look that is due then.
    fn next_wake(&self) -> Option<Wake> {
        let agents = self.outage().and_then(|outage| wake_of(&outage, self.looked));
        let host = self
            .host_outage()
            .and_then(|outage| wake_of(&outage, self.looked_at_host));
        // A keep-warm ping waits as the agents' rounds do: while their wait stands, its own wakes serve.
        let ping = match self.outage() {
            None => self.next_ping(),
            Some(_) => None,
        };
        let at = [agents.map(|(at, _)| at), host.map(|(at, _)| at), ping]
            .into_iter()
            .flatten()
            .min()?;
        Some(Wake {
            at,
            look: agents.is_some_and(|(a, look)| look && a == at),
            look_at_host: host.is_some_and(|(h, look)| look && h == at),
        })
    }

    /// Takes the time, at which the driver asked to be told it ([`Driver::wake`]), and, where it asked for a look at
    /// the network to the agents' service or to the acts' host, whether the look reached it: every work is looked at
    /// again, and what is to be done is given.
    pub fn tick(&mut self, now: Time, reached: Option<bool>, host_reached: Option<bool>) -> Result<Vec<Todo>, Fault> {
        if let Some(fault) = &self.fault {
            return Err(fault.clone());
        }
        if let Some(reached) = reached {
            self.looked = Some((now, reached));
        }
        if let Some(reached) = host_reached {
            self.looked_at_host = Some((now, reached));
        }
        self.ticked = Some(self.ticked.map_or(now, |at| at.max(now)));
        match self.look(now) {
            Ok(mut todos) => {
                todos.extend(self.pings(now));
                self.filled(now, todos)
            }
            Err(fault) => {
                self.stop(fault.clone(), true);
                Err(fault)
            }
        }
    }

    /// The keep-warm pings due at `now` and not under way, each given once: none while the wait every work shares
    /// holds the agents' rounds, whose pings would fail as they do, nor while the engine holds everything as it
    /// opened, until the owner has seen how things stand ([`warm`]).
    fn pings(&mut self, now: Time) -> Vec<Todo> {
        let Some(policy) = (self.policy.as_ref()).filter(|_| !self.gate(now).holds_agents() && self.warming()) else {
            return Vec::new();
        };
        let mut todos = Vec::new();
        for (number, held) in &self.works {
            if held.standing != Standing::Going {
                continue;
            }
            let limits = self.limits(*number);
            for due in warm::due(&held.flow, &held.work, &held.item, policy, &limits, now) {
                if self.pinging.contains(&(*number, due.line.clone())) {
                    continue;
                }
                todos.push(held.todo(Effect::Ping {
                    line: due.line,
                    session: due.session,
                }));
            }
        }
        for todo in &todos {
            if let Effect::Ping { line, .. } = &todo.effect {
                self.pinging.insert((todo.work, line.clone()));
            }
        }
        todos
    }

    /// Whether sessions are kept warm now: not while the engine holds everything as it opened on works going on, which
    /// spends nothing before the owner has seen how things stand (principle `no-unseen-spending-loop`); while the owner
    /// holds everything, as the settings say (`sessions.warm_while_paused`).
    fn warming(&self) -> bool {
        let keeps = (self.policy.as_ref())
            .and_then(|p| p.warm.as_ref())
            .is_some_and(|w| w.while_paused);
        match &self.paused {
            None => true,
            Some(paused) => paused.by == Pauser::Owner && keeps,
        }
    }

    /// When the next keep-warm ping not under way is due, of every work that has not ended, as of the time the driver
    /// was last told: a ping due then and not given waits for no later time than the next moment, and one whose
    /// session's cache will have let it go by then is due at no time, so that the driver is never woken for a ping it
    /// will not give.
    fn next_ping(&self) -> Option<Time> {
        let policy = self.policy.as_ref().filter(|_| self.warming())?;
        let now = self.ticked.unwrap_or(Time(0));
        let soonest = Time(now.0.saturating_add(1));
        (self.works.iter())
            .filter(|(_, held)| held.standing == Standing::Going)
            .flat_map(|(number, held)| {
                let limits = self.limits(*number);
                (warm::kept(&held.flow, &held.work, &held.item, policy, &limits, now).into_iter())
                    .filter(|kept| !self.pinging.contains(&(*number, kept.due.line.clone())))
                    .map(|kept| (kept.at.max(soonest), kept.life))
                    .filter(|(at, life)| at < life)
                    .map(|(at, _)| at)
                    .collect::<Vec<_>>()
            })
            .min()
    }

    /// The gate a work is stepped with: the wait every work shares, and while a keep-warm ping of the work is under
    /// way, every round of it, so that its next round begins once the ping has ended, after whatever the ping did to
    /// its sessions, and is filled as they then stand.
    pub fn gate_of(&self, now: Time, work: WorkNumber) -> Gate {
        match self.pinging.iter().any(|(w, _)| *w == work) {
            true => Gate::Every,
            false => self.gate(now),
        }
    }

    /// Whether a wait every work shares holds rounds now.
    pub fn holds(&self, now: Time) -> bool {
        self.gate(now) != Gate::Open
    }

    /// Which rounds may begin now, as the waits every work shares say: the agents' wait holds the agents' rounds, and
    /// the acts' wait the acts'.
    pub fn gate(&self, now: Time) -> Gate {
        let reached = |looked: Option<(Time, bool)>| looked.filter(|(_, reached)| *reached).map(|(at, _)| at);
        let agents = match self.outage() {
            Some(outage) if outage.holds(now, reached(self.looked)) => Gate::Agents,
            _ => Gate::Open,
        };
        let acts = match self.host_outage() {
            Some(outage) if outage.holds(now, reached(self.looked_at_host)) => Gate::Acts,
            _ => Gate::Open,
        };
        agents.and(acts)
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
            && !matches!(self.gate(now), Gate::Agents | Gate::Every)
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
        let (room, gate) = (self.room(Some(number), slot), self.gate_of(now, number));
        let limits = self.limits(number);
        let Some(held) = self
            .works
            .get_mut(&number)
            .filter(|held| held.standing == Standing::Going)
        else {
            return Ok(());
        };
        match step(
            self.store.as_mut(),
            held,
            self.policy.as_ref(),
            &limits,
            now,
            event,
            room,
            gate,
        ) {
            Ok(effects) => {
                todos.extend(effects.into_iter().map(|effect| held.todo(effect)));
                Ok(())
            }
            Err(Halt::Store(fault)) => Err(Fault::Store(fault)),
            Err(Halt::Refused(refused)) => Err(Fault::Refused { work: number, refused }),
        }
    }

    /// Takes what a source outside the works says now of the project of this name, such as the head of its main
    /// branch, at the time the caller's clock gives: it is written into the block of that source of each work of an
    /// item of that project that has not ended, where the block does not say it already, as a change of its meaning;
    /// and gives what is to be done. A work of another project's item, or of an item of none, is written nothing, nor
    /// is a work whose flow has no block of the source; [`DEPENDENCIES`] and [`START`], the driver's own sources, are
    /// written by the driver alone. Fails only where the driver is stopped, or stops on it.
    pub fn outside(&mut self, now: Time, project: &str, source: &str, content: &str) -> Result<Vec<Todo>, Fault> {
        if let Some(fault) = &self.fault {
            return Err(fault.clone());
        }
        if source == DEPENDENCIES || source == START {
            return Ok(Vec::new());
        }
        let mut todos = Vec::new();
        let (mut failed, mut written) = (None, false);
        let room = self.room(None, false);
        let gates: BTreeMap<WorkNumber, Gate> = (self.works.keys()).map(|n| (*n, self.gate_of(now, *n))).collect();
        let limits: BTreeMap<WorkNumber, Limits> = (self.works.keys()).map(|n| (*n, self.limits(*n))).collect();
        for (number, held) in &mut self.works {
            let gate = gates.get(number).copied().unwrap_or_default();
            if held.standing != Standing::Going || held.item.project.as_deref() != Some(project) {
                continue;
            }
            let limits = limits.get(number).cloned().unwrap_or_default();
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
            match step(
                self.store.as_mut(),
                held,
                self.policy.as_ref(),
                &limits,
                now,
                event,
                room,
                gate,
            ) {
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
        let mut view = decide::view_in(&kept.flow.flow, &kept.work.records, self.room(Some(number), true));
        // A work held at fault by the driver, though the walk finds its records sound, is one the deciding code goes
        // round on without end ([`step`]): it shows as that fault.
        let at_fault = self
            .works
            .get(&number)
            .is_some_and(|held| held.standing == Standing::AtFault);
        if at_fault && !matches!(view.decision, Decision::Fault(_)) {
            view.decision = Decision::Fault(decide::RecordFault::WithoutEnd);
        }
        Some(view)
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

    /// The pause of everything, while it holds: when it began, and who set it.
    pub fn everything_paused(&self) -> Option<EverythingPaused> {
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

    /// The room the engine leaves a work: the pause of everything, as it stands; and, where `slot` says a slot
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
            let (room, gate) = (self.room(Some(number), true), self.gate_of(now, number));
            let limits = self.limits(number);
            let Some(held) = self.works.get_mut(&number) else {
                break;
            };
            match step(
                self.store.as_mut(),
                held,
                self.policy.as_ref(),
                &limits,
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
    /// say whether they have all merged, and which it waits for, in the words its definition gives (`writes`); none
    /// where the flow has no such block, or the block says so already.
    fn block(&self, flow: &Compiled, depends_on: &[ItemNumber], work: &Work) -> Option<Event> {
        let (block, declared) =
            (flow.blocks.iter()).find(|(_, b)| b.writers.outside.as_deref() == Some(DEPENDENCIES))?;
        let says = |name: &str| declared.writes.get(name).unwrap_or_default();
        let standing = |on: &ItemNumber| {
            let work = self.of_item.get(on).and_then(|work| self.works.get(work));
            work.map_or(Standing::Going, |held| held.standing)
        };
        let waited: Vec<String> = (depends_on.iter())
            .filter_map(|on| match standing(on) {
                Standing::Ended(End::Merged) => None,
                Standing::Ended(End::Cancelled) => Some(fill(says("cancelled"), &[("item", &on.0.to_string())])),
                Standing::Going | Standing::AtFault => Some(on.0.to_string()),
            })
            .collect();
        let (value, content) = if waited.is_empty() {
            (MERGED, says(MERGED).to_string())
        } else {
            (
                WAITING,
                fill(says(WAITING), &[("items", &waited.join(says("between")))]),
            )
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
#[allow(clippy::too_many_arguments)]
fn step(
    store: &mut dyn Storage,
    held: &mut Held,
    policy: Option<&Policy>,
    limits: &Limits,
    now: Time,
    event: Event,
    room: Room,
    gate: Gate,
) -> Result<Vec<Effect>, Halt> {
    let before = held.work.tick();
    let item = &held.item;
    let then = |flow: &Compiled, work: &Work| {
        let mut events = sessions::follow(flow, work, item, policy, limits);
        events.extend(start(flow, work));
        events
    };
    let effects = match handle_then(&held.flow, &mut held.work, now, event, room, gate, &then) {
        Ok(effects) => effects,
        // The deciding code goes round without end on the work's records: what was done to the work for this is let
        // go, the work is held at fault as the store keeps it, nothing more is decided of it, and the rest go on.
        Err(engine::Refused::WithoutEnd(_)) => {
            held.work = store.load(held.head.number).map_err(Halt::Store)?;
            held.standing = Standing::AtFault;
            return Ok(Vec::new());
        }
        Err(refused) => return Err(Halt::Refused(refused)),
    };
    if held.work.tick() != before {
        store.save(held.head.number, &held.work).map_err(Halt::Store)?;
        held.standing = standing_of(&held.flow, &held.work);
        held.samples = history::samples(&held.flow, &held.work);
    }
    Ok(effects)
}

/// Whether the work, going and not paused, has an agent's cell whose round runs on the model, as the policy says.
fn runs_on(held: &Held, policy: Option<&Policy>, model: &str) -> bool {
    let Some(policy) = policy else {
        return false;
    };
    held.standing == Standing::Going
        && !decide::paused(&held.work.records)
        && (held.flow.cells.iter())
            .filter(|cell| cell.kind != flow::Kind::Engine)
            .any(|cell| (policy.on)(&held.item, cell).is_some_and(|on| on.model == model))
}

/// The write of the work's block of [`START`], where its flow has one and the owner has just started the work: the work
/// is not paused, was let on since it was added paused, and holds no place to be done in: it has started no run and
/// has no version of the block, or it was reopened since the block's latest version, its cancel having let go of
/// that place. What it writes is the block's `text`, as the flow says it ([`faults`]).
fn start(flow: &Compiled, work: &Work) -> Option<Event> {
    let (block, declared) = (flow.blocks.iter()).find(|(_, b)| b.writers.outside.as_deref() == Some(START))?;
    let r = &work.records;
    let started = !decide::paused(r) && r.pauses.iter().any(|p| !p.paused);
    let last = (r.written.iter()).filter(|w| w.block == *block).map(|w| w.at).max();
    let fresh = match last {
        None => r.started.is_empty(),
        Some(last) => r.reopens.iter().any(|at| *at > last),
    };
    (started && fresh).then(|| Event::Wrote {
        block: block.clone(),
        by: Outsider::Outside,
        content: declared.writes.get("text").unwrap_or_default().to_string(),
        matters: true,
        value: None,
    })
}

/// When the driver is to be told the time for a wait, as its kind says, with whether a look is then due: none where
/// the wait waits for a round to end. Where it waits for a look, the next is half a minute after the last since it
/// began.
fn wake_of(outage: &Outage, looked: Option<(Time, bool)>) -> Option<(Time, bool)> {
    let next_look = |at: Time| {
        let from = outage.last.unwrap_or(outage.since);
        match looked {
            Some((looked, _)) if looked >= from => Time(looked.0.saturating_add(outage::PROBE)),
            _ => at,
        }
    };
    match outage.until() {
        Until::At(at) | Until::Trial { at, look: false } => Some((at, false)),
        Until::Tried => None,
        Until::Trial { at, look: true } | Until::Looked { at } => Some((next_look(at), true)),
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
