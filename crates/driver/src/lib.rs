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
//! one such block: the block of a flow whose source is [`DEPENDENCIES`] says whether the work still waits for the
//! items its item depends on: it waits for each until it has merged or was cancelled, a dependency on a cancelled item
//! holding as one on a merged item does (decision `a-cancelled-item-keeps-its-place`). The driver writes it as the
//! item is added, whenever one of those items merges, is cancelled or is reopened, and, on opening, wherever the block
//! and what it stands for have come apart; it writes no block of a work that has ended. A work whose flow has no such
//! block waits for no item.
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
//! The driver writes one more kind of block of a work: a block of a flow whose source is [`SESSION`], which a cell
//! reads that loads into a line's session what the line's other cells work from, each line its own. As a line of a work
//! begins a new session, the driver records it and writes the block its cells read, so that the cell runs again before
//! the line goes on; when a line begins one is the installation's [`Policy`], worked out of the work's records, and of
//! what the carriers say a new session of the line would begin with, after each thing that happens and before the walk
//! (module [`sessions`]).
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
//! What happens to one work also reaches every other where a round of an agent failed for a cause outside every work,
//! such as the account's usage limit: no agent's round of any work begins while that wait stands ([`outage`]). It is
//! worked out from the records of every work, and given to the engine's loop for each work as a [`Gate`]; once it
//! changes, every work is looked at again. When the outside is to be checked, and the acts' host looked at, is the
//! driver's to say ([`Driver::wake`]) and its caller's, which reads the clock and reaches the outside, to tell it with
//! what the check or the look found ([`Driver::tick`]).
//!
//! The accounts the agents' program may run under are the engine's own state, as the pause of everything is, kept in
//! the store as the owner adds, removes and takes them ([`accounts`]). Every round is handed out with the account in
//! use as it is handed out, none where none is, a keep-warm ping runs under it ([`warm`]), and a check of the outside
//! is taken under it; a usage limit is its account's own ([`outage`]), and taking another account drops what the checks
//! found and has the outside checked at once. What the agents' service last said of each account's usage is kept beside
//! them in memory, out of everything that decides ([`usage`]).
//!
//! What the driver holds of a work in memory is what the store holds of it: a work is saved each time it grows, and
//! what is to be done is given out only once the store holds the entries that say so. Stopped at any moment and
//! [opened](Driver::open) again, the driver gives again everything that was to be done and is not known as done
//! (principle `effects-known-as-done`). When the store fails the driver stops: it gives the fault to every later call
//! and shows nothing more of the works, which it may hold otherwise than the store does, and is to be opened again
//! (principle `nothing-passes-for-normal`).
//!
//! What needs whoever plans the work since a mark they keep, read from the records of every work, is [`attention`];
//! what a work waits on the owner for, defined once for everything that counts or shows it, is [`owner`].
//!
//! The driver alone sees every work, so it is also what judges by history (principle `judged-by-own-history`): it
//! gathers what the rounds of every work on a flow came to ([`history`]), and gives each round the lines it is judged
//! by as it runs. It gives the keep-warm pings of the sessions of lines that wait as their time comes, the time being
//! among what it asks to be told ([`warm`]); it tells a round that missed its session's prompt cache from the records
//! ([`cache`]); and it tells how each round and each call of a tool went against the lines of its history, which stops
//! nothing ([`judged`]).
//!
//! It keeps, while a round or a call is under way, where it stands as what carries it out says as it goes ([`along`]).

pub mod accounts;

pub mod along;

pub mod attention;

pub mod budget;

pub mod cache;

pub mod changes;

pub mod history;

pub mod judged;

pub mod owner;

pub mod sessions;

pub mod usage;

pub mod warm;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

pub mod outage;

pub use accounts::{Accounts, Instead, Live};
pub use along::{Along, Under};
use decide::run::Failure;
use decide::{BlockId, Decision, End, Holder, Outsider, Pauser, Room, View, begun, decide, taking_slot};
use engine::{Effect, Event, Gate, Refused, handle_held, handle_then, held};
use flow::Compiled;
use flow::agents::fill;
pub use outage::{Checked, Found, Limit, Outage, Service, Until};
use store::{
    AccountAct, AccountRecord, Change, EverythingPaused, First, Item, ItemChange, ItemNumber, Storage, WorkHead,
    WorkNumber,
};
use work::{Time, Work};

pub use budget::Budget;
pub use changes::{Asked, Gone, NotMade, Planned, Unchangeable};
pub use sessions::{Policy, SESSION, Warm};
pub use usage::{LOOK_AFTER_MS, Said, Usage, Usages, Window};

/// The source of the blocks the driver writes: whether the items the work's item depends on have merged or were
/// cancelled. Such a block is a choice of [`WAITING`] and [`MERGED`].
pub const DEPENDENCIES: &str = "dependencies";
/// The value of a [`DEPENDENCIES`] block while an item the work's item depends on has neither merged nor been
/// cancelled.
pub const WAITING: &str = "waiting";
/// The value of a [`DEPENDENCIES`] block once every item the work's item depends on has merged or was cancelled, or it
/// depends on none.
pub const MERGED: &str = "merged";

/// The source of the block the driver writes as the owner starts a work: the work, added paused, is let on with nothing
/// of it started. The block has no content to speak of; its version is what runs the cell that starts the work.
pub const START: &str = "start";

/// What is wrong with the flow for what the engine writes, each in words with the file that says what it writes into
/// the block: a block of [`DEPENDENCIES`], of [`SESSION`], of [`START`], of [`engine::asked::ASKED`] or of
/// [`engine::handed_in::HANDED_IN`], and each cell's own words and questions, which the sources of the owner's words
/// and of the answers write and the cell writes too, gives in its `writes` each text the engine fills, so that none is
/// written empty; and the words beside the flows ([`flow::waits_file`]) give every reason the engine says to the owner.
/// A flow with any of these is not run.
pub fn faults(flow: &Compiled) -> Vec<String> {
    use engine::asked::{ANSWERED, ASKED, WAITING as UNANSWERED};
    use engine::handed_in::HANDED_IN;
    let session: Vec<&str> = ["text"].into_iter().chain(work::Why::KINDS).collect();
    let needs: [(&str, &[&str]); 5] = [
        (DEPENDENCIES, &[MERGED, WAITING, "between"]),
        (SESSION, &session),
        (START, &["text"]),
        (ASKED, &[ANSWERED, UNANSWERED, "between"]),
        (HANDED_IN, &["text"]),
    ];
    // The dialogues the sources of the owner's turning write versions of, each a cell's own: its words, the owner's
    // versions and the cell's answers; its questions, each kind of version.
    let records: [(&str, &[&str]); 2] = [
        (flow::WORDS, &[engine::spoken::OWNER, engine::spoken::REPLY]),
        (flow::ANSWER, &engine::asked::ENTRIES),
    ];
    let mut faults = Vec::new();
    for (name, block) in &flow.blocks {
        let found = needs
            .iter()
            .find(|(source, _)| block.writers.outside.as_deref() == Some(*source));
        let written = !block.writers.cells.is_empty();
        let record = (records.iter()).find(|(source, _)| block.writers.outside.as_deref() == Some(*source) && written);
        let Some((source, keys)) = found.or(record) else {
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
    // What the engine says to the owner of why a work waits on them, and why it paused one, is beside the flows: a flow
    // run without it would show every such reason empty (principle nothing-passes-for-normal).
    let waits = &flow.waits;
    let words = [
        ("paused.too_long", &waits.paused.too_long),
        ("paused.passing", &waits.paused.passing),
        ("paused.outside", &waits.paused.outside),
        ("paused.flagged_again", &waits.paused.flagged_again),
        ("paused.let_go", &waits.paused.let_go),
        ("owner.limit", &waits.owner.limit),
        ("owner.calls", &waits.owner.calls),
        ("owner.returns", &waits.owner.returns),
        ("owner.made", &waits.owner.made),
        ("owner.ruling", &waits.owner.ruling),
        ("owner.paused", &waits.owner.paused),
        ("owner.no_reason", &waits.owner.no_reason),
        ("owner.held", &waits.owner.held),
        ("owner.agents", &waits.owner.agents),
        ("owner.mend", &waits.owner.mend),
        ("stalled.waits", &waits.stalled.waits),
        ("stalled.last", &waits.stalled.last),
        ("stalled.only", &waits.stalled.only),
        ("stalled.none", &waits.stalled.none),
        ("stalled.unrun", &waits.stalled.unrun),
        ("stalled.skipped", &waits.stalled.skipped),
        ("stalled.nothing_new", &waits.stalled.nothing_new),
        ("stalled.held", &waits.stalled.held),
        ("stalled.name", &waits.stalled.name),
        ("stalled.and", &waits.stalled.and),
        ("stalled.or", &waits.stalled.or),
        ("stalled.then", &waits.stalled.then),
    ];
    let empty: Vec<&str> = (words.iter())
        .filter(|(_, text)| text.trim().is_empty())
        .map(|(key, _)| *key)
        .collect();
    if !empty.is_empty() {
        faults.push(format!(
            "the flow {}, {}: the words beside the flows must give {}",
            flow.name,
            flow::waits_file(),
            empty.join(", ")
        ));
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
    /// Whether the sessions of works that wait are kept warm, as the store keeps it: the engine's own state, which the
    /// owner switches, and no setting, so that a switch takes effect at once ([`warm`]).
    keep_warm: bool,
    /// The accounts the agents' program may run under, as the store keeps them: the engine's own state, which the owner
    /// changes ([`accounts`]).
    accounts: Accounts,
    /// Every stretch the pause of everything held, in order, as the store keeps them.
    spans: Vec<Span>,
    /// What stopped the driver, once something has.
    fault: Option<Fault>,
    /// When a line begins a new session; with none, only what others record of sessions is followed.
    policy: Option<Policy>,
    /// The checks of the outside taken while the agents' rounds wait for it, by the account each was taken under, none
    /// where none was in use, with when the wait they were taken in began ([`Outage::since`]): a check speaks only of
    /// its own account, and checks of a wait that ended are of no other.
    checked: BTreeMap<Option<String>, (Time, Checked)>,
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
    /// Where each round or call under way stands, of a work, as what carries it out last said ([`along`]). What the
    /// outside says at a moment, kept only in memory: lost in a stop, it is said again as what is given again goes on.
    along: BTreeMap<(WorkNumber, Under), Along>,
    /// What the agents' service last said of each account's usage, and the looks under way ([`usage`]). What the
    /// outside says at a moment, kept only in memory and read by nothing that decides: lost in a stop, it is said again
    /// at the next call under each account, or the next look.
    usage: Usages,
}

/// A stretch the pause of everything held: from when it was set and by whom, to when it was lifted; none while it
/// holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub from: Time,
    pub until: Option<Time>,
    pub by: Pauser,
}

/// The stretches the pause of everything held, as the store keeps each pause set and lifted, in order.
fn spans_of(pauses: &[store::PauseOfEverything]) -> Vec<Span> {
    let mut spans: Vec<Span> = Vec::new();
    for pause in pauses {
        add_span(&mut spans, pause.paused, pause.by, pause.at);
    }
    spans
}

/// The stretches with a pause of everything set, or lifted, by this pauser at this time: one set while another holds
/// ends it and holds from then, by its own pauser; one lifted while none holds changes nothing.
fn add_span(spans: &mut Vec<Span>, paused: bool, by: Pauser, at: Time) {
    let open = spans.last_mut().filter(|span| span.until.is_none());
    match (paused, open) {
        (false, Some(open)) => open.until = Some(at),
        (false, None) => {}
        (true, Some(open)) if open.by == by => {}
        (true, open) => {
            if let Some(open) = open {
                open.until = Some(at);
            }
            spans.push(Span {
                from: at,
                until: None,
                by,
            });
        }
    }
}

/// When the driver is to be told the time again: where `check`, the outside of the agents' rounds is to be checked
/// then, and where `look_at_host`, the network to the acts' host looked at; what each found is told with the time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Wake {
    pub at: Time,
    pub check: bool,
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

/// What holds every work at once as the driver stands: the owner's pause of everything, the wait of each service the
/// works reach, and the works whose round under way takes a slot. The waits and the slots taken are worked out from the
/// records of every work, so a reader of many works works them out once ([`Driver::at_once`]) and hands them to its
/// reading of each ([`Driver::view_at`], [`Driver::held_at`], [`Driver::waits_on_owner_at`]); the forms that read one
/// work alone work them out for it.
#[derive(Clone, Debug)]
pub struct AtOnce {
    /// The pause of everything, while it holds.
    pub everything: Option<EverythingPaused>,
    /// The wait of the agents' service, if one stands.
    pub agents: Option<Outage>,
    /// The wait of the acts' host, if one stands.
    pub host: Option<Outage>,
    /// The works whose round under way takes a slot.
    taking: BTreeSet<WorkNumber>,
}

impl AtOnce {
    /// The agents' wait where it holds the rounds now and their command line cannot run: since when, and what it said.
    pub fn cannot_run(&self) -> Option<(Time, &str)> {
        let outage = self.agents.as_ref().filter(|outage| outage.holds())?;
        let (failure, why) = outage.down();
        (failure == Failure::CannotRun).then_some((outage.since, why))
    }

    /// Which rounds may begin now, as these waits say ([`Driver::gate`]).
    pub fn gate(&self) -> Gate {
        gate_of_waits(self.agents.as_ref(), self.host.as_ref())
    }

    /// What every work's waits on the owner read alike ([`owner::Shared`]).
    fn shared(&self) -> owner::Shared<'_> {
        owner::Shared {
            everything: self.everything.map(|paused| paused.by),
            cannot_run: self.cannot_run().map(|(_, why)| why),
        }
    }
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
    /// An account was to be added under a name that is no account's name, in these words. Nothing was changed.
    AccountName(String),
    /// An account was to be added under the name of one that is added and not removed. Nothing was changed.
    NameInUse(String),
    /// There is no account of this name added and not removed. Nothing was changed.
    NoSuchAccount(String),
    /// The account in use was to be removed: another is taken first. Nothing was changed.
    RemovingInUse(String),
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
        let keep_warm = store.keeping_warm().map_err(Fault::Store)?;
        let accounts = Accounts::new(store.accounts().map_err(Fault::Store)?);
        let spans = spans_of(&store.everything_pauses().map_err(Fault::Store)?);
        let mut driver = Driver {
            store,
            flows: by_name,
            works: BTreeMap::new(),
            of_item: BTreeMap::new(),
            slots,
            paused,
            keep_warm,
            accounts,
            spans,
            fault: None,
            policy,
            checked: BTreeMap::new(),
            looked_at_host: None,
            ticked: None,
            answered: BTreeSet::new(),
            pinging: BTreeSet::new(),
            along: BTreeMap::new(),
            usage: Usages::default(),
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
            add_span(&mut driver.spans, true, Pauser::Engine, now);
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
                let (room, gate) = (driver.room(Some(number), false), driver.gate_of(number));
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

    /// Adds an item together with its one work and with what people write for it first, all of it or none, and gives
    /// their numbers with what is to be done. The work is added paused, so that
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
        let gate = self.gate();
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
            let (room, gate) = (self.room(Some(work), true), self.gate_of(work));
            let held = works.entry(work).or_insert_with(|| self.works[&work].clone());
            let (number, item, policy) = (work, &held.item, self.policy.as_ref());
            let then = |flow: &Compiled, work: &Work| {
                let mut events = sessions::follow(flow, number, work, item, policy);
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
            Some(held) => self.gate_of(work).and(held),
            None => self.gate_of(work),
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
        add_span(&mut self.spans, paused, Pauser::Owner, now);
        if paused {
            return Ok(Vec::new());
        }
        match self.look(now) {
            Ok(todos) => self.filled_after(now, todos),
            Err(fault) => Err(self.stop(fault, true)),
        }
    }

    /// Whether the sessions of works that wait are kept warm, as the owner last switched it ([`warm`]).
    pub fn keeping_warm(&self) -> bool {
        self.keep_warm
    }

    /// Keeps the sessions of works that wait warm from the time the caller's clock gives, or stops keeping them. It is
    /// kept in the store and holds when the driver is opened again, and switching it as it stands changes nothing. No
    /// ping is given here: one due is given as the driver is told the time next, which it asks for at once
    /// ([`Driver::wake`]); a ping under way runs to its end whatever the switch says.
    pub fn keep_warm(&mut self, now: Time, warm: bool) -> Result<(), Failed> {
        self.going()?;
        if warm == self.keep_warm {
            return Ok(());
        }
        if let Err(fault) = self.store.keep_warm(warm, now) {
            return Err(self.stop(Fault::Store(fault), false));
        }
        self.keep_warm = warm;
        Ok(())
    }

    /// The accounts the agents' program may run under, as the owner added, removed and took them.
    pub fn accounts(&self) -> &Accounts {
        &self.accounts
    }

    /// Adds an account under this name, at the time the caller's clock gives, whose token the caller has kept: it is
    /// refused where the name is no account's name or is one's that is added and not removed. An account added is not
    /// taken: the agents run under it once the owner takes it.
    pub fn add_account(&mut self, now: Time, name: &str) -> Result<(), Failed> {
        self.going()?;
        if let Some(why) = accounts::name_fault(name) {
            return Err(Failed::AccountName(why));
        }
        if self.accounts.is_live(name) {
            return Err(Failed::NameInUse(name.to_string()));
        }
        self.keep_account(now, name, AccountAct::Added)
    }

    /// Removes the account of this name, at the time the caller's clock gives, which is then the caller's to let go of
    /// the token of: refused where there is no such account, or it is the one in use.
    pub fn remove_account(&mut self, now: Time, name: &str) -> Result<(), Failed> {
        self.going()?;
        if !self.accounts.is_live(name) {
            return Err(Failed::NoSuchAccount(name.to_string()));
        }
        if self.accounts.in_use() == Some(name) {
            return Err(Failed::RemovingInUse(name.to_string()));
        }
        self.keep_account(now, name, AccountAct::Removed)?;
        self.usage.keep_only(&self.accounts);
        Ok(())
    }

    /// Takes the account of this name as the one the agents run under, at the time the caller's clock gives, and gives
    /// what is to be done: every round handed out from now on runs under it, a round handed out before under the
    /// account it was handed out with. What the checks of the outside found under another account is no check of this
    /// one, which is checked at once; every work is looked at again, a wait that held them for another account's usage
    /// limit ending. Taking the account in use changes nothing; there being no such account, it is refused.
    pub fn take_account(&mut self, now: Time, name: &str) -> Result<Vec<Todo>, Failed> {
        self.going()?;
        if !self.accounts.is_live(name) {
            return Err(Failed::NoSuchAccount(name.to_string()));
        }
        if self.accounts.in_use() == Some(name) {
            return Ok(Vec::new());
        }
        self.keep_account(now, name, AccountAct::Taken)?;
        // The check under this account is asked for now: a time told at or after now, as in the same moment, would pass
        // it over as acted on.
        self.ticked = self.ticked.filter(|at| *at < now);
        match self.look(now) {
            Ok(todos) => self.filled_after(now, todos),
            Err(fault) => Err(self.stop(fault, true)),
        }
    }

    /// Keeps what was done to the account, in the store and then here.
    fn keep_account(&mut self, now: Time, name: &str, what: AccountAct) -> Result<(), Failed> {
        let record = AccountRecord {
            name: name.to_string(),
            what,
            at: now,
        };
        if let Err(fault) = self.store.keep_account(&record) {
            return Err(self.stop(Fault::Store(fault), false));
        }
        self.accounts.push(record);
        Ok(())
    }

    /// What the agents' service last said of each account's usage, and the looks under way ([`usage`]).
    pub fn usage(&self) -> &Usages {
        &self.usage
    }

    /// Takes what the agents' service said of the usage of the account of this name, at the time the caller's clock
    /// gives: the account a call was handed out with, whichever is in use now. Gives whether what it says changed.
    pub fn heard_usage(&mut self, now: Time, account: &str, said: Said) -> bool {
        self.usage.heard(&self.accounts, now, account, said)
    }

    /// The accounts a look is to be taken under, as the owner opens them, at the time the caller's clock gives, each
    /// marked as being looked at ([`usage`]).
    pub fn looks(&mut self, now: Time) -> Vec<String> {
        self.usage.looks(&self.accounts, now)
    }

    /// Takes what a look under the account of this name found, at the time the caller's clock gives, with what the
    /// service said of its usage, where it said anything; gives whether what is known changed.
    pub fn looked(&mut self, now: Time, account: &str, found: &Found, said: Option<Said>) -> bool {
        self.usage.looked(&self.accounts, now, account, found, said)
    }

    /// The usage limit of each account that stands, by the account's name, whether or not it is in use ([`outage`]).
    pub fn limits(&self) -> BTreeMap<String, Limit> {
        let works = (self.works.values()).map(|held| (held.head.number, &*held.flow, &held.work));
        outage::limits(works, &self.answered)
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

    /// How the rounds and the calls of tools of the work, or of every work where none is named, went against the lines
    /// of their history, what is under way as of `now` ([`judged`]): nothing without a policy, which says how many
    /// values a line is worked out from, as no round is judged while it runs without one.
    pub fn judged(&self, work: Option<WorkNumber>, now: Time) -> judged::Judged {
        let Some(policy) = &self.policy else {
            return judged::Judged::default();
        };
        let works: Vec<judged::Of<'_>> = (self.shown())
            .map(|held| judged::Of {
                number: held.head.number,
                flow: &held.head.flow,
                compiled: &held.flow,
                work: &held.work,
                samples: &held.samples,
            })
            .collect();
        judged::judged(&works, |number| work.is_none_or(|w| w == number), policy.samples, now)
    }

    /// The lines a round of the cell of the work is judged by while it runs: from the history of the rounds of every
    /// work on its flow, its cost among those on the model it runs on now ([`history::Lines`]). None are given without
    /// a policy, as for rounds carried out in no session.
    pub fn lines(&self, work: WorkNumber, cell: &flow::CellSpec) -> history::Lines {
        let (Some(policy), Some(held)) = (&self.policy, self.works.get(&work)) else {
            return history::Lines::default();
        };
        let model = (policy.model)(&held.item, cell).unwrap_or_default();
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

    /// The wait every work shares for the agents' service, if one stands: worked out from the records of every work
    /// and the rounds the service answered while under way ([`outage`]), with the checks of the outside taken in it.
    pub fn outage(&self) -> Option<Outage> {
        self.outage_of(Service::Agents)
    }

    /// The wait every work shares for the acts' host, if one stands: worked out from the records of every work, with
    /// when a look at the host last reached it.
    pub fn host_outage(&self) -> Option<Outage> {
        self.outage_of(Service::Host)
    }

    /// The wait of the service every work shares, if one stands, with what the outside said of it since it began.
    fn outage_of(&self, service: Service) -> Option<Outage> {
        let in_use = self.accounts.in_use();
        let works = (self.works.values()).map(|held| (held.head.number, &*held.flow, &held.work));
        let mut outage = outage::outage(service, works, &self.answered, in_use)?;
        match service {
            Service::Agents => {
                outage.checked = (self.checked.get(&in_use.map(str::to_string)))
                    .filter(|(of, _)| *of == outage.since)
                    .map(|(_, checked)| checked.clone());
                outage.switched = self.accounts.taken_at().filter(|at| *at >= outage.met);
            }
            Service::Host => {
                outage.reached = (self.looked_at_host)
                    .filter(|(at, reached)| *reached && *at >= outage.met)
                    .map(|(at, _)| at);
            }
        }
        Some(outage)
    }

    /// The waits of every service, as they stand.
    fn outages(&self) -> (Option<Outage>, Option<Outage>) {
        (self.outage(), self.host_outage())
    }

    /// What carries out a round or a call of the work says where it stands now, or, with none, that it no longer says
    /// so; taken while it is under way, and dropped otherwise. Gives whether what is kept changed. What it kept of the
    /// work's rounds and calls no longer under way is let go.
    pub fn along(&mut self, work: WorkNumber, under: Under, along: Option<Along>) -> bool {
        let Some(held) = self.works.get(&work) else {
            return false;
        };
        let records = &held.work.records;
        self.along
            .retain(|(of, under), _| *of != work || under.under_way(records));
        let key = (work, under);
        let was = self.along.get(&key).cloned();
        match along {
            Some(along) if under.under_way(records) => {
                self.along.insert(key, along);
            }
            _ => {
                self.along.remove(&key);
            }
        }
        self.along.get(&key) != was.as_ref()
    }

    /// Where each round or call of the work under way stands, as what carries it out last said.
    pub fn along_of(&self, work: WorkNumber) -> Vec<(Under, &Along)> {
        let Some(held) = self.works.get(&work) else {
            return Vec::new();
        };
        (self.along.range(
            (
                work,
                Under::Round {
                    run: decide::RunId(0),
                    round: 0,
                },
            )..,
        ))
        .take_while(|((of, _), _)| *of == work)
        .filter(|((_, under), _)| under.under_way(&held.work.records))
        .map(|((_, under), along)| (*under, along))
        .collect()
    }

    /// Takes, at the time the caller's clock gives, that the agents' service answered the round under way of the run:
    /// where the round began after the latest that failed for a cause outside every work, the wait is over, and every
    /// work is looked at again; what is to be done is given. A round no longer under way is passed over.
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

    /// When the driver is to be told the time again, where the outside of the agents' rounds is to be checked, the
    /// acts' host looked at, or a session kept warm; none where nothing waits for a time. A wake whose time has come is
    /// given until the driver is told that time, and none at or before the time it was last told: the works were
    /// looked at then, and what still waits waits for a round to end.
    pub fn wake(&self) -> Option<Wake> {
        let ticked = self.ticked;
        self.next_wake().filter(|wake| ticked.is_none_or(|at| wake.at > at))
    }

    /// When the driver is to be told the time next, as the waits every work shares say, whether or not it has come:
    /// the earliest of each wait's and of the next keep-warm ping's, with each check and look that is due then.
    fn next_wake(&self) -> Option<Wake> {
        let agents = self.outage().and_then(|outage| wake_of(&outage, None));
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
            check: agents.is_some_and(|(a, _)| a == at),
            look_at_host: host.is_some_and(|(h, look)| look && h == at),
        })
    }

    /// Takes the time, at which the driver asked to be told it ([`Driver::wake`]), and, where it asked for a check of
    /// the outside of the agents' rounds, the account the check was taken under, none where none was in use, and
    /// what it found, and for a look at the acts' host, whether the look reached it: every work is looked at again, and
    /// what is to be done is given. A check speaks only of the account it was taken under, whichever is in use as it is
    /// told; one taken while no wait of that account's rounds stands is of none, and is passed over.
    #[allow(clippy::type_complexity)]
    pub fn tick(
        &mut self,
        now: Time,
        found: Option<(Option<String>, Found)>,
        host_reached: Option<bool>,
    ) -> Result<Vec<Todo>, Fault> {
        if let Some(fault) = &self.fault {
            return Err(fault.clone());
        }
        if let Some((under, found)) = found {
            let works = (self.works.values()).map(|held| (held.head.number, &*held.flow, &held.work));
            if let Some(outage) = outage::outage(Service::Agents, works, &self.answered, under.as_deref()) {
                let before = (self.checked.get(&under)).filter(|(of, _)| *of == outage.since);
                let taken = before.map_or(0, |(_, checked)| checked.taken) + 1;
                self.checked
                    .insert(under, (outage.since, Checked { taken, at: now, found }));
            }
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

    /// The keep-warm pings due at `now` and not under way, each given once: of every work that has not ended, whoever
    /// holds it, and none while the wait every work shares holds the agents' rounds, whose pings would fail as their
    /// rounds do ([`warm`]).
    fn pings(&mut self, now: Time) -> Vec<Todo> {
        let Some(policy) = (self.policy.as_ref()).filter(|_| !self.gate().holds_agents()) else {
            return Vec::new();
        };
        let mut todos = Vec::new();
        for (number, held) in &self.works {
            if held.standing != Standing::Going {
                continue;
            }
            for due in warm::due(
                &held.flow,
                *number,
                &held.work,
                &held.item,
                policy,
                self.keep_warm,
                &self.accounts,
                now,
            ) {
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

    /// When the next keep-warm ping not under way is due, of every work that has not ended, as of the time the driver
    /// was last told: a ping due then and not given waits for no later time than the next moment, and one whose
    /// session's cache will have let it go by then is due at no time, so that the driver is never woken for a ping it
    /// will not give.
    fn next_ping(&self) -> Option<Time> {
        let policy = self.policy.as_ref()?;
        let now = self.ticked.unwrap_or(Time(0));
        let soonest = Time(now.0.saturating_add(1));
        (self.works.iter())
            .filter(|(_, held)| held.standing == Standing::Going)
            .flat_map(|(number, held)| {
                (warm::kept(
                    &held.flow,
                    *number,
                    &held.work,
                    &held.item,
                    policy,
                    self.keep_warm,
                    &self.accounts,
                    now,
                )
                .into_iter())
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
    pub fn gate_of(&self, work: WorkNumber) -> Gate {
        match self.pinging.iter().any(|(w, _)| *w == work) {
            true => Gate::Every,
            false => self.gate(),
        }
    }

    /// Whether a wait every work shares holds rounds now.
    pub fn holds(&self) -> bool {
        self.gate() != Gate::Open
    }

    /// Which rounds may begin now, as the waits every work shares say: the agents' wait holds the agents' rounds until
    /// a check of the outside finds it well, and the acts' wait the acts' until a look at their host reaches it.
    pub fn gate(&self) -> Gate {
        gate_of_waits(self.outage().as_ref(), self.host_outage().as_ref())
    }

    /// Looks at every work that has not ended, in the order of their numbers, each with the gate as it is then and
    /// given no slot, so that the free slots go to the works in the engine's order once all have been looked at.
    fn look(&mut self, now: Time) -> Result<Vec<Todo>, Fault> {
        let mut todos = Vec::new();
        let numbers: Vec<WorkNumber> = (self.works.iter())
            .filter(|(_, held)| held.standing == Standing::Going)
            .map(|(number, _)| *number)
            .collect();
        for number in numbers {
            self.step_one(now, number, Event::Look, false, &mut todos)?;
        }
        Ok(todos)
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
        let (room, gate) = (self.room(Some(number), slot), self.gate_of(number));
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
        let gates: BTreeMap<WorkNumber, Gate> = (self.works.keys()).map(|n| (*n, self.gate_of(*n))).collect();
        for (number, held) in &mut self.works {
            let gate = gates.get(number).copied().unwrap_or_default();
            if held.standing != Standing::Going || held.item.project.as_deref() != Some(project) {
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
        (self.works.get(&number))
            .filter(|_| self.fault.is_none())
            .map(Held::kept)
    }

    /// The flows the driver was given, in the order of their names.
    pub fn flows(&self) -> impl Iterator<Item = &Compiled> {
        self.flows.values().map(|flow| &**flow)
    }

    /// The work as it is now, in the room the engine leaves it: what it does next, each cell's state, every version of
    /// every block.
    /// This is the form for a reader of one work: what holds every work at once is worked out for it.
    pub fn view(&self, number: WorkNumber) -> Option<View> {
        self.view_at(number, &self.at_once())
    }

    /// The work as it is now ([`Driver::view`]), in the room what holds every work at once leaves it.
    pub fn view_at(&self, number: WorkNumber, at_once: &AtOnce) -> Option<View> {
        let kept = self.work(number)?;
        let mut view = decide::view_in(&kept.flow.flow, &kept.work.records, self.room_at(number, at_once));
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
    /// This is the form for a reader of one work: what holds every work at once is worked out for it.
    pub fn held(&self, number: WorkNumber) -> Option<Holder> {
        self.held_at(number, &self.at_once())
    }

    /// What holds back the round the work would begin next ([`Driver::held`]), in the room what holds every work at
    /// once leaves it.
    pub fn held_at(&self, number: WorkNumber, at_once: &AtOnce) -> Option<Holder> {
        let kept = self.work(number)?;
        held(kept.flow, kept.work, self.room_at(number, at_once))
    }

    /// What the work waits on the owner for now, as the one definition of it says ([`owner::waits`]), of its view and
    /// what holds back its next round as the reader has them, with what holds every work at once: the pause of
    /// everything, and the agents' wait where their command line cannot run. None where there is no such work.
    pub fn waits_on_owner_at(
        &self,
        number: WorkNumber,
        view: &View,
        held: Option<Holder>,
        at_once: &AtOnce,
    ) -> Vec<owner::Waiting> {
        let Some(kept) = self.work(number) else {
            return Vec::new();
        };
        owner::waits(&kept, &kept.work.records, &view.decision, held, at_once.shared())
    }

    /// What holds every work at once as it stands ([`AtOnce`]), worked out from every work's records.
    pub fn at_once(&self) -> AtOnce {
        AtOnce {
            everything: self.paused,
            agents: self.outage(),
            host: self.host_outage(),
            taking: self.taking(),
        }
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

    /// Every stretch the pause of everything held, in order, the one that holds now last.
    pub fn everything_spans(&self) -> &[Span] {
        &self.spans
    }

    /// The items the work's item depends on that its work waits for, those whose works have neither merged nor been
    /// cancelled, in the order the item names them, as the block of [`DEPENDENCIES`] says they are waited for; none
    /// where there is no such work.
    pub fn items_waited_for(&self, number: WorkNumber) -> Vec<ItemNumber> {
        let Some(held) = self.works.get(&number) else {
            return Vec::new();
        };
        self.waited_for(&held.item.depends_on)
    }

    /// Of these items, each whose work has not ended, merged or cancelled: the one reading of what a block of
    /// [`DEPENDENCIES`] waits for, which writes the block and says which items a work waits for. An item with no work
    /// yet goes on.
    fn waited_for(&self, depends_on: &[ItemNumber]) -> Vec<ItemNumber> {
        (depends_on.iter().copied())
            .filter(|on| {
                let work = self.of_item.get(on).and_then(|work| self.works.get(work));
                !work.is_some_and(|held| matches!(held.standing, Standing::Ended(_)))
            })
            .collect()
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

    /// How many works have a round under way that takes a slot ([`Driver::taking`]), but this one.
    fn taken(&self, but: Option<WorkNumber>) -> usize {
        taken_among(&self.taking(), but)
    }

    /// The works that have a round under way that takes a slot. A work at fault takes none: nothing more is decided of
    /// it, so a slot it held would be held for good.
    fn taking(&self) -> BTreeSet<WorkNumber> {
        (self.works.values())
            .filter(|held| held.standing != Standing::AtFault)
            .filter(|held| taking_slot(&held.flow.flow, &held.work.records))
            .map(|held| held.head.number)
            .collect()
    }

    /// The room the engine leaves a work: the pause of everything, as it stands; and, where `slot` says a slot
    /// may be given at all, whether fewer are taken by the other works than there are.
    fn room(&self, work: Option<WorkNumber>, slot: bool) -> Room {
        Room {
            paused: self.paused.is_some(),
            slot: slot && self.taken(work) < self.slots,
        }
    }

    /// The room the engine leaves a work ([`Driver::room`], a slot given where one is free), as what holds every work
    /// at once says.
    fn room_at(&self, work: WorkNumber, at_once: &AtOnce) -> Room {
        Room {
            paused: at_once.everything.is_some(),
            slot: taken_among(&at_once.taking, Some(work)) < self.slots,
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
            let (room, gate) = (self.room(Some(number), true), self.gate_of(number));
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
    /// say whether they have all merged or were cancelled, and which it waits for, in the words its definition gives
    /// (`writes`); none where the flow has no such block, or the block says so already.
    fn block(&self, flow: &Compiled, depends_on: &[ItemNumber], work: &Work) -> Option<Event> {
        let (block, declared) =
            (flow.blocks.iter()).find(|(_, b)| b.writers.outside.as_deref() == Some(DEPENDENCIES))?;
        let says = |name: &str| declared.writes.get(name).unwrap_or_default();
        let waited: Vec<String> = (self.waited_for(depends_on).into_iter())
            .map(|on| on.0.to_string())
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
    let (number, item) = (held.head.number, &held.item);
    let then = |flow: &Compiled, work: &Work| {
        let mut events = sessions::follow(flow, number, work, item, policy);
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
            .any(|cell| (policy.model)(&held.item, cell).is_some_and(|on| on == model))
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

/// How many of the works that take a slot are works but this one.
fn taken_among(taking: &BTreeSet<WorkNumber>, but: Option<WorkNumber>) -> usize {
    taking.iter().filter(|work| Some(**work) != but).count()
}

/// Which rounds may begin, as the agents' wait and the acts' wait say: each holds its own rounds while it holds.
fn gate_of_waits(agents: Option<&Outage>, host: Option<&Outage>) -> Gate {
    let agents = match agents {
        Some(outage) if outage.holds() => Gate::Agents,
        _ => Gate::Open,
    };
    let acts = match host {
        Some(outage) if outage.holds() => Gate::Acts,
        _ => Gate::Open,
    };
    agents.and(acts)
}

/// When the driver is to be told the time for a wait, as its kind says, with whether a look at the acts' host is then
/// due: the next check of the outside of the agents' rounds, none once one found it well; the next look at the acts'
/// host, half a minute after the last since the latest act that failed so.
fn wake_of(outage: &Outage, looked: Option<(Time, bool)>) -> Option<(Time, bool)> {
    match outage.until() {
        Until::Checked { at } => Some((at, false)),
        Until::Well { .. } => None,
        Until::Looked { at } => match looked {
            Some((looked, _)) if looked >= outage.met => Some((Time(looked.0.saturating_add(outage::PROBE)), true)),
            _ => Some((at, true)),
        },
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
