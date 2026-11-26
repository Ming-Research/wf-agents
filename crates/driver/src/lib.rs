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
//! an item's title, level, tags and what it depends on, until its work ends; its cancel; and the reopening of a
//! cancelled item ([`Driver::change`], module [`changes`]). A change of an item is kept as a record of the item's own, a
//! cancel and a reopen as the work's; a change of what an item depends on writes the work's block of [`DEPENDENCIES`]
//! anew, as an add does, and a work that has started waits where it stands for the items it depends on that have not
//! ended (decision `dependencies-hold-a-started-work-where-it-would-run`).
//!
//! The driver writes the block of a flow whose source is [`START`] as the owner starts the work: as the work, added
//! paused, is first let on with nothing of it started, or, reopened, is first let on since, so that the cell it runs,
//! the work's start, is entered and makes the place the work is done in again.
//!
//! As a line of a work begins a new session, the driver records it, and the line goes on in it; when a line begins one
//! is the installation's [`Policy`], worked out of the work's records, and of what the carriers say a new session of the
//! line would begin with, after each thing that happens and before the walk (module [`sessions`]).
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
//! while one is free: each call gives away what came free before it returns. The driver opens with its slots and is
//! given them again whenever the owner changes them ([`Driver::set_slots`]): slots added go at once to the works that
//! wait for one, in the engine's order; with fewer, no round under way is stopped, and none begins until fewer are
//! under way than there are slots. A work is added paused, and goes on once the owner lets it on, so that whoever
//! plans orders what runs.
//!
//! The works of a project take turns to check and merge, one at a time ([`turn`]). Whose turn it is in a project is
//! worked out from the records of its works: the work that holds the turn, which it does from a round of a cell that
//! takes it while its walk stays at such cells; else the work that has wanted it longest, the lower number first where
//! two began to want it at one time. A work whose turn it is not waits for it in its walk, as for a slot; the work
//! whose turn it is, where it has no round under way, is looked at each time something was kept, as the free slots are
//! given, so that it begins its round the moment the work before it lets go.
//!
//! What happens to one work also reaches every other where a round of an agent failed for a cause outside every work,
//! such as the account's usage limit: no agent's round of any work begins while that wait stands ([`outage`]). It is
//! worked out from the records of every work, and given to the engine's loop for each work as a [`Gate`] in its
//! [`Hold`]; once it changes, every work is looked at again. When the outside is to be checked, and the acts' host
//! looked at, is the driver's to say ([`Driver::wake`]) and its caller's, which reads the clock and reaches the
//! outside, to tell it with what the check or the look found ([`Driver::tick`]).
//!
//! The accounts the agents' program may run under are the engine's own state, as the pause of everything is, kept in
//! the store as the owner adds, removes and takes them ([`accounts`]). Every round is handed out with the account in
//! use as it is handed out, none where none is, a keep-warm ping runs under it ([`warm`]), and a check of the outside
//! is taken under it; a usage limit is its account's own ([`outage`]), and taking another account drops what the checks
//! found and has the outside checked at once. Where the owner lets it, the engine takes another account itself at a usage
//! limit, once a check under that account found it well, and records that it did ([`outage::move_on`]). What the agents'
//! service last said of each account's usage is kept beside them in memory, out of everything that decides ([`usage`]).
//!
//! What the driver holds of a work in memory is what the store holds of it: a work is saved each time it grows, and
//! what is to be done is given out only once the store holds the entries that say so. Stopped at any moment and
//! [opened](Driver::open) again, the driver gives again everything that was to be done and is not known as done
//! (principle `effects-known-as-done`). When the store fails the driver stops: it gives the fault to every later call
//! and shows nothing more of the works, which it may hold otherwise than the store does, and is to be opened again
//! (principle `nothing-passes-for-normal`).
//!
//! What needs whoever plans the work, read from the records of every work, each since when it stands, is [`attention`];
//! what a work waits on the owner for, defined once for everything that counts or shows it, is [`owner`].
//!
//! The driver alone sees every work, so it is also what judges by history (principle `judged-by-own-history`): it
//! gathers what the rounds of every work on a flow came to ([`history`]), and gives each round the lines it is judged
//! by as it runs. It gives the keep-warm pings of the sessions as their time comes, each by its own last call and none
//! while a round of its own line is under way, as many as the prices of the session's model make worth it, which the
//! work's own reports of what its calls cost tell ([`prices`]), the time being among what it asks to be told
//! ([`warm`]); it tells a
//! round that missed its session's prompt cache from the records ([`cache`]); and it tells how each round and each
//! call of a tool went against the lines of its history, which stops nothing ([`judged`]).
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

pub mod prices;

pub mod red_main;

pub mod sessions;

pub mod time;
/// What the times of the project's tests and jobs say, by the history of their own kind.
pub mod timed;

mod turn;

pub mod usage;

pub mod versions;

pub mod warm;

use std::cell::{RefCell, RefMut};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

pub mod outage;

pub use accounts::{Accounts, Instead, Live, Moved};
pub use along::{Along, Figure, Going, Under};
use decide::run::Failure;
use decide::{BlockId, CellId, Decision, End, Holder, Outsider, Pauser, Room, View, begun, decide, taking_slot};
use engine::{Effect, Event, Gate, Hold, Refused, handle_held, handle_then, held};
use flow::Compiled;
use flow::agents::{ResultGiven, fill};
pub use outage::{Candidate, CannotRun, Checked, Found, Limit, Move, Outage, Service, Until};
use store::{
    AccountAct, AccountRecord, AccountTaker, Change, Continues, EverythingPaused, First, Item, ItemChange, ItemNumber,
    StopAct, StopRecord, Storage, WorkHead, WorkNumber,
};
use turn::Turned;
use versions::Installed;
pub use versions::{Misread, On};
use work::{Ask, Call, KeptTime, Moment, Time, TurnSpan, Work};

pub use budget::Budget;
pub use changes::{Asked, NotMade, Planned, Unchangeable};
pub use history::HISTORY_SAMPLES;
pub use sessions::{Policy, Warm};
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

/// The source of the blocks the engine writes as the head of a project's main branch changes: each work of the
/// project's items that has not ended is told the head, and the check before its merge runs again on it.
pub const MAIN: &str = "main";

/// What is wrong with the flow for what the engine writes, each in words with the file that says what it writes into
/// the block: a block of [`DEPENDENCIES`], of [`START`], of [`timed::TEST_TIMES`], of [`timed::TEST_RUNS`], of
/// [`engine::asked::ASKED`] or of [`engine::handed_in::HANDED_IN`], and each cell's own words and questions, which the
/// sources of the owner's words and of the answers write and the cell writes too, gives in its `writes` each text the
/// engine fills, so that none is written empty; and the words beside the flows ([`flow::waits_file`]) give every reason
/// the engine says to the owner. A flow with any of these is not run.
pub fn faults(flow: &Compiled) -> Vec<String> {
    use engine::asked::{ANSWERED, ASKED, WAITING as UNANSWERED};
    use engine::handed_in::HANDED_IN;
    let needs: [(&str, &[&str]); 6] = [
        (DEPENDENCIES, &[MERGED, WAITING, "between"]),
        (START, &["text"]),
        (ASKED, &[ANSWERED, UNANSWERED, "between"]),
        (HANDED_IN, &["text"]),
        (timed::TEST_TIMES, &timed::WORDS),
        (timed::TEST_RUNS, &timed::RUNS_WORDS),
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
        ("paused.failed", &waits.paused.failed),
        ("paused.no_model", &waits.paused.no_model),
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
    /// The version installed of each flow, by its name.
    flows: BTreeMap<String, Installed>,
    works: BTreeMap<WorkNumber, Held>,
    /// The work of each item.
    of_item: BTreeMap<ItemNumber, WorkNumber>,
    /// How many rounds of cells that take a slot may go on at once, as the driver opened with them or was last given
    /// them ([`Driver::set_slots`]).
    slots: usize,
    /// The pause of everything, while it holds, as the store keeps it.
    paused: Option<EverythingPaused>,
    /// Whether the sessions of works that wait are kept warm, as the store keeps it: the engine's own state, which the
    /// owner switches, and no setting, so that a switch takes effect at once ([`warm`]).
    keep_warm: bool,
    /// Whether the engine may move to another account at the usage limit of the one in use, as the store keeps it: the
    /// engine's own state, which the owner switches, and no setting, so that a switch takes effect at once
    /// ([`outage::move_on`]).
    moving_on: bool,
    /// The accounts the agents' program may run under, as the store keeps them: the engine's own state, which the owner
    /// changes ([`accounts`]).
    accounts: Accounts,
    /// Every stretch the pause of everything held, in order, as the store keeps them.
    spans: Vec<Span>,
    /// Every stop of the engine asked for, and every start after one, in order, as the store keeps them.
    stops: Vec<StopRecord>,
    /// What stopped the driver, once something has.
    fault: Option<Fault>,
    /// When a line begins a new session; with none, only what others record of sessions is followed.
    policy: Option<Policy>,
    /// How much of a result of `run_tests` a round is given, which the datum `test_runs` gives of each call too; with
    /// none, a result is given whole.
    tests_given: Option<ResultGiven>,
    /// The checks of the outside taken while the agents' rounds wait for it, by the account each was taken under, none
    /// where none was in use, with when the wait they were taken in began ([`Outage::since`]): a check speaks only of
    /// its own account, and checks of a wait that ended are of no other. A check under an account that is not the one
    /// in use, taken to move to it, is of the wait at the in-use account's usage limit, and has that wait's `since`.
    checked: BTreeMap<Option<String>, (Time, Checked)>,
    /// The latest look at the acts' host, while the acts' rounds wait for it: when, and whether it was reached.
    looked_at_host: Option<(Time, bool)>,
    /// When the driver was last told the time it asked for: a wake at or before it has been acted on. Lost in a stop,
    /// when a wake whose time has come is acted on again.
    ticked: Option<Time>,
    /// Each round, of a work and run by its number, that the service it reaches answered while it was under way: the
    /// agents' service an agent's round, the host an act. What the outside says at a moment, kept only in memory: lost
    /// in a stop, the round is given again and answers again.
    answered: BTreeSet<(WorkNumber, decide::RunId, usize)>,
    /// The keep-warm pings given and not yet said to have ended, by work and line ([`warm`]): while one of a line is,
    /// no round of that line of the work begins. Lost in a stop, when a ping still due is given again.
    pinging: BTreeSet<(WorkNumber, String)>,
    /// Where each round or call under way stands, of a work, as what carries it out last said ([`along`]). What the
    /// outside says at a moment, kept only in memory: lost in a stop, it is said again as what is given again goes on.
    along: BTreeMap<(WorkNumber, Under), Along>,
    /// Since when each round or call under way has waited for a runner, as what carries it out first said so after it
    /// last did not: the time by the loop's clock, with the runner. Kept only in memory with [`Driver::along`]: lost in
    /// a stop, it is taken again from the first time it is said after.
    queued: BTreeMap<(WorkNumber, Under), (String, Time)>,
    /// What the agents' service last said of each account's usage, and the looks under way ([`usage`]). What the
    /// outside says at a moment, kept only in memory and read by nothing that decides: lost in a stop, it is said again
    /// at the next call under each account, or the next look.
    usage: Usages,
    /// The merged checks of each project as they were last worked out from the works' records, by the project's name
    /// ([`Driver::checks`]): worked out again only once a work merged, or a merged work changed, since.
    checks: RefCell<BTreeMap<Option<String>, Checks>>,
}

/// The merged checks of a project as they were last worked out ([`timed::history`]), with what they say once it was
/// asked for.
#[derive(Default)]
struct Checks {
    /// Each work of the project that merged, with its revision as it was then ([`Held::revision`]).
    of: Vec<(WorkNumber, u64)>,
    history: Arc<Vec<timed::Merge>>,
    /// What they say ([`timed::tested`]), with how many values a history was judged from at least.
    tested: Option<(usize, timed::Tested)>,
}

#[cfg(test)]
thread_local! {
    /// How many times the merged checks of a project were worked out from the works' records on this thread
    /// ([`Driver::checks`]): what a test of what a read of the tests' times costs reads.
    static CHECKS_WORKED_OUT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// The next revision of a work as the driver holds it ([`Held::revision`]): one never given before in this process.
fn next_revision() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
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

/// A stop of the engine that stands ([`Driver::ask_to_stop`]): since when, and whether it was asked to stop at once,
/// cutting off what is under way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stopping {
    pub since: Time,
    pub at_once: bool,
}

/// A stretch a stop of the engine stood: from when it was asked for to when the engine started again; none while it
/// stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StopSpan {
    pub from: Time,
    pub until: Option<Time>,
}

/// The stretches the stops of the engine stood, as the store keeps each stop and each start after one, in order: a stop
/// asked for again while one stands goes on from the first, and a start after none changes nothing.
fn stop_spans(stops: &[StopRecord]) -> Vec<StopSpan> {
    let mut spans: Vec<StopSpan> = Vec::new();
    for stop in stops {
        let open = spans.last_mut().filter(|span| span.until.is_none());
        match (stop.what, open) {
            (StopAct::Over, Some(open)) => open.until = Some(stop.at),
            (StopAct::Over, None) | (StopAct::Asked | StopAct::Now, Some(_)) => {}
            (StopAct::Asked | StopAct::Now, None) => spans.push(StopSpan {
                from: stop.at,
                until: None,
            }),
        }
    }
    spans
}

/// When the driver is to be told the time again: where `check`, the outside of the agents' rounds is to be checked
/// then, and where `look_at_host`, the network to the acts' host looked at; what each found is told with the time.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Wake {
    pub at: Time,
    pub check: bool,
    /// Where `check` is a check to move to this account, the account to take it under; none where it is the check of
    /// the wait under the account in use.
    pub candidate: Option<String>,
    pub look_at_host: bool,
}

/// The slots, as they stand.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slots {
    /// How many rounds of cells that take a slot may go on at once.
    pub slots: usize,
    /// How many such rounds are under way: one for each work that has one, a round being stopped among them until it
    /// has ended. More may be under way than there are slots, where the slots were made fewer while they were under
    /// way, or were fewer when the engine started again.
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
    /// The stop of the engine, while it stands.
    pub stopping: Option<Stopping>,
    /// The wait of the agents' service, if one stands.
    pub agents: Option<Outage>,
    /// The wait of the acts' host, if one stands.
    pub host: Option<Outage>,
    /// The works whose round under way takes a slot.
    taking: BTreeSet<WorkNumber>,
    /// Whose turn it is to check and merge in each project that has a work that holds or wants it, by the project's name:
    /// none where the items of no project, as items kept before projects.
    turn: BTreeMap<Option<String>, WorkNumber>,
}

impl AtOnce {
    /// Whose turn it is to check and merge in the project, where a work of it holds or wants the turn.
    pub fn turn_of(&self, project: Option<&str>) -> Option<WorkNumber> {
        self.turn.get(&project.map(str::to_string)).copied()
    }

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

/// A work as the driver holds it: what the store holds of it, read through its moves, with its item, the version of its
/// flow it is on and the flow installed of that name at hand.
#[derive(Clone)]
struct Held {
    head: WorkHead,
    item: Item,
    /// The flow installed of the work's flow's name: what decides the work where it is on that version, and what says
    /// the engine's words beside the flows whatever version it is on.
    flow: Arc<Compiled>,
    /// The version of its flow the work is on.
    on: On,
    /// The outline of each version kept that the work was on before the one it is on, in the order it was on them:
    /// what a cell of it taken out since is read by.
    passed: Vec<Arc<flow::Outline>>,
    work: Work,
    /// How the work stands. Worked out again each time the work grows.
    standing: Standing,
    /// The work's rounds that are history, which later rounds of every work on its flow are judged by. Worked out
    /// again each time the work grows.
    samples: Vec<history::Sample>,
    /// How the work stands as to the turn of its project to check and merge. Worked out again each time the work grows.
    turned: Turned,
    /// The work's rounds as the waits every work shares read them, on the version it is on ([`outage::Rounds`]). Worked
    /// out again each time the work grows.
    rounds: outage::Rounds,
    /// Each line's session the work keeps warm as its records, its item and the policy say ([`warm::of_records`]), which
    /// a wake takes its next ping from. Worked out again each time the work grows or its item changes.
    warm: Vec<warm::Kept>,
    /// What the work adds to the judgments of the rounds and calls of its flow ([`judged::Part`]). Worked out again each
    /// time the work grows.
    part: judged::Part,
    /// A number that changes each time anything the driver holds of the work changes, its records, its item or the
    /// version of its flow it is on, and is never given twice in the process: what a reader that keeps what it worked
    /// out of the work knows it by ([`Driver::revision`]).
    revision: u64,
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
    /// The flow installed of the name the work runs on: what decides the work where it is on that version
    /// ([`On::installed`]), and the engine's words beside the flows whatever version it is on. A work that ended on
    /// another version is read from `on` and its records alone.
    pub flow: &'a Compiled,
    /// The version of its flow the work is on, with its outline.
    pub on: &'a On,
    /// The outline of each version kept that the work was on before, in the order it was on them.
    pub passed: &'a [Arc<flow::Outline>],
    pub work: &'a Work,
}

impl Kept<'_> {
    /// The cell of this id as the version the work is on has it: what a cell of the work is, whichever version is
    /// installed. None where that version has no such cell, or where the work is on none kept.
    pub fn cell(&self, id: &CellId) -> Option<&flow::outline::OutlinedCell> {
        self.on.outline()?.cell(id)
    }

    /// Where the datum comes from, as the version the work is on says of a datum written from outside the work: the
    /// owner, or the source that writes it. None where that version writes it from no outside, or where the work is on
    /// none kept.
    pub fn from(&self, block: &BlockId) -> Option<&str> {
        self.on.outline()?.from.get(block).map(String::as_str)
    }

    /// The number of the version of the cell's questions written with this entry of the records, as the version the
    /// work is on names the cell's questions ([`engine::asked::number_in`]). None where none was written with it, or
    /// where the work is on none kept.
    pub fn question_version(&self, cell: &CellId, at: decide::Seq) -> Option<u64> {
        engine::asked::number_in(&self.work.records, self.cell(cell)?.questions.as_ref()?, at)
    }

    /// The cell a run of the work ran at that reads now as this id, a cell taken out since
    /// ([`flow::outline::taken_out`]), as the latest version kept that the work was on before and that had it has it.
    pub fn came_from(&self, id: &CellId) -> Option<&flow::outline::OutlinedCell> {
        let now = self.on.outline()?;
        (self.passed.iter().rev()).find_map(|was| {
            let reading = flow::reading(now, was.earlier_ids.len());
            (was.cells.iter()).find(|cell| reading.cell(&cell.id) == *id)
        })
    }

    /// What the work does next as its version reads it: what the flow installed decides, for a work on it; how it ended,
    /// for one that ended on another version or on none kept.
    pub fn decision(&self) -> Decision {
        match self.on.end() {
            Some(end) => Decision::Ended(end),
            None => decide(&self.flow.flow, &self.work.records),
        }
    }
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
    /// What it continues, where its work starts from an existing branch or pull request and not from the head of main.
    pub continues: Option<Continues>,
    /// The name of the flow its work runs on.
    pub flow: String,
    /// What people write for it as it is added, in order.
    pub writes: Vec<FirstWrite>,
    /// Why its work is paused as it is added, where whoever adds it says: it is added paused either way.
    pub why: Option<String>,
    /// The items its work's pause waits for, to merge or be cancelled, where whoever adds it says.
    pub waits_for: Vec<ItemNumber>,
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
    /// The store holds the outline of this version of a flow as what does not read as one, for this reason.
    Outline { version: String, why: String },
    /// A work was put on a version of its flow the store does not keep.
    NoSuchVersion { work: WorkNumber, version: String },
    /// The table of earlier ids of the flow installed would misread this version of it, which these works are on, so
    /// (decision `a-table-that-misreads-a-kept-version-is-refused`): the table is mended in the flow. What it misreads
    /// is boxed, so that the fault, which every refusal of the driver may carry, stays small.
    EarlierIds {
        flow: String,
        version: String,
        works: Vec<WorkNumber>,
        why: Box<Misread>,
    },
}

/// Why the driver did not do what it was asked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failed {
    /// The driver has no such item. Nothing was changed.
    NoSuchItem(ItemNumber),
    /// An item was to be added with a title of no words. Nothing was changed.
    BlankTitle,
    /// A pause was to wait for the item of its own work, which it would never end. Nothing was changed.
    WaitsForItself(ItemNumber),
    /// An item was to be added carrying a tag that is no name an item may carry. Nothing was changed.
    BadTag(String),
    /// An item was to be added continuing what is no branch name or pull request number an item may name. Nothing was
    /// changed.
    BadContinues(Continues),
    /// An item was to be added continuing something, on a flow that has no cell that starts a work, which would read
    /// it. Nothing was changed.
    NoStart(String),
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
    /// Opens the driver on a store, with the flows the works run on, this many slots, the policy of when a line
    /// begins a new session and how much of a result of `run_tests` is given, and gives everything that is to be done: what was to be done before and is not known as
    /// done, and what follows from looking at every work now, the free slots given in the engine's order.
    ///
    /// Where a work is going on, one that has not ended and that its own pause does not hold, and nothing pauses
    /// everything, the driver pauses everything itself before it looks at any work, and keeps that it did: nothing
    /// begins a round then but what the owner's own pause of everything lets begin, until the owner lifts it as they
    /// lift their own. What was to be done is given all the same, a round under way at the stop among it: the records
    /// count that round as begun, and holding it back in its carrier until the lift would be the path the decision
    /// slots-in-the-scheduler turned down.
    pub fn open(
        mut store: Box<dyn Storage + Send>,
        flows: Vec<Compiled>,
        slots: usize,
        policy: Option<Policy>,
        tests_given: Option<ResultGiven>,
        now: Time,
    ) -> Result<(Driver, Vec<Todo>), Fault> {
        let mut given = BTreeMap::new();
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
            if given.insert(name.clone(), flow).is_some() {
                return Err(Fault::FlowTwice(name));
            }
        }
        // Each flow given is the version installed of it, kept as the driver opens with it.
        let mut by_name = BTreeMap::new();
        for (name, flow) in given {
            let installed = versions::keep(store.as_mut(), flow).map_err(Fault::Store)?;
            by_name.insert(name, installed);
        }
        let paused = store.everything_paused().map_err(Fault::Store)?;
        let keep_warm = store.keeping_warm().map_err(Fault::Store)?;
        let moving_on = store.moving_on().map_err(Fault::Store)?;
        let accounts = Accounts::new(store.accounts().map_err(Fault::Store)?);
        let spans = spans_of(&store.everything_pauses().map_err(Fault::Store)?);
        // A stop that stood as the engine ended no longer stands once it starts again: that is kept before anything is
        // looked at, so that the stretch it stood ends here.
        let mut stops = store.stops().map_err(Fault::Store)?;
        if (stops.last()).is_some_and(|last| last.what != StopAct::Over) {
            let over = StopRecord {
                what: StopAct::Over,
                at: now,
            };
            store.keep_stop(&over).map_err(Fault::Store)?;
            stops.push(over);
        }
        let mut driver = Driver {
            store,
            flows: by_name,
            works: BTreeMap::new(),
            of_item: BTreeMap::new(),
            slots,
            paused,
            keep_warm,
            moving_on,
            accounts,
            spans,
            stops,
            fault: None,
            policy,
            tests_given,
            checked: BTreeMap::new(),
            looked_at_host: None,
            ticked: None,
            answered: BTreeSet::new(),
            pinging: BTreeSet::new(),
            along: BTreeMap::new(),
            queued: BTreeMap::new(),
            usage: Usages::default(),
            checks: RefCell::default(),
        };
        let items = driver.store.items().map_err(Fault::Store)?;
        let heads = driver.store.works().map_err(Fault::Store)?;
        let outlines = versions::outlines(driver.store.as_ref())?;
        let mut loaded = Vec::new();
        for head in heads {
            let Some(installed) = driver.flows.get(&head.flow).cloned() else {
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
            let mut work = driver.store.load(head.number).map_err(Fault::Store)?;
            versions::read(&mut work, head.number, &outlines)?;
            loaded.push((head, item, installed, work));
        }
        if let Some(item) = items.iter().find(|item| !driver.of_item.contains_key(&item.number)) {
            return Err(Fault::NotOneWork(item.number));
        }
        // No version a work is on may be misread by the table installed, which every work is read through as it is
        // moved, judged and reopened; each is checked before any work is moved.
        let mut misread: BTreeMap<(String, String), (Misread, Vec<WorkNumber>)> = BTreeMap::new();
        for (head, _, installed, work) in &loaded {
            let Some(version) = work.version().and_then(|on| on.version.clone()) else {
                continue;
            };
            if version == installed.version {
                continue;
            }
            let outline = versions::outline_of(&outlines, head.number, &version)?;
            if let Some(why) = versions::misread(&installed.outline, &outline) {
                let key = (head.flow.clone(), version);
                misread.entry(key).or_insert((why, Vec::new())).1.push(head.number);
            }
        }
        if let Some(((flow, version), (why, works))) = misread.into_iter().next() {
            return Err(Fault::EarlierIds {
                flow,
                version,
                works,
                why: Box::new(why),
            });
        }
        // Each work is held on its version. One that has not ended on an earlier version is put on the installed one,
        // and one kept before versions were is put on one; each such move is kept before anything is looked at.
        for (head, item, installed, mut work) in loaded {
            let before = work.tick();
            let on = match work.version().map(|on| on.version.clone()) {
                // Kept before versions were, the work is read once by the flow installed: ended there, it ran on a
                // version not kept; else it goes on, on the installed one.
                None => match standing_of(&installed.flow, &work) {
                    Standing::Ended(end) => {
                        work.onto(now, None, decide::Reading::default());
                        On::NotKept { end }
                    }
                    Standing::Going | Standing::AtFault => {
                        work.onto(now, Some(installed.version.clone()), decide::Reading::default());
                        installed.on()
                    }
                },
                Some(None) => On::NotKept {
                    end: versions::ended_unkept(&work.records),
                },
                Some(Some(version)) if version == installed.version => installed.on(),
                Some(Some(version)) => {
                    let outline = versions::outline_of(&outlines, head.number, &version)?;
                    match versions::ended(&outline, &work.records) {
                        Some(end) => {
                            let reading = flow::reading(&installed.outline, outline.earlier_ids.len());
                            On::Earlier {
                                version,
                                outline,
                                end,
                                reading,
                            }
                        }
                        None => {
                            versions::put_on(&mut work, now, Some(&*outline), &installed);
                            installed.on()
                        }
                    }
                }
            };
            if work.tick() != before {
                driver.store.save(head.number, &work).map_err(Fault::Store)?;
            }
            let passed = versions::passed(&work, &outlines);
            let held = Held::new(
                head,
                item,
                installed.flow.clone(),
                on,
                passed,
                work,
                driver.policy.as_ref(),
            );
            driver.works.insert(held.head.number, held);
        }
        driver.keep_times(now)?;
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
                    Ok((effects, _)) => todos.extend(effects.into_iter().map(|effect| held.todo(effect))),
                    Err(Halt::Store(fault)) => return Err(Fault::Store(fault)),
                    Err(Halt::Refused(refused)) => return Err(Fault::Refused { work: number, refused }),
                }
            }
        }
        match driver.give_away(now) {
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
        let Some(installed) = self.flows.get(&new.flow).cloned() else {
            return Err(Failed::NoSuchFlow(new.flow));
        };
        let flow = installed.flow.clone();
        let waits_for = self.pause_waits(None, new.waits_for)?;
        if let Some(continues) = &new.continues {
            if changes::continues_fault(continues).is_some() {
                return Err(Failed::BadContinues(continues.clone()));
            }
            if !changes::starts(&flow) {
                return Err(Failed::NoStart(new.flow));
            }
        }
        let mut depends_on = new.depends_on;
        depends_on.sort();
        depends_on.dedup();
        let mut tags = new.tags;
        tags.sort();
        tags.dedup();
        // The version the work is added on is its first entry (decision `a-move-is-read-through-the-renumberings`).
        let mut work = Work::default();
        work.added_on(installed.version.clone());
        let mut first = vec![Event::Paused {
            by: Pauser::Owner,
            why: new.why,
            waits_for,
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
            turn: true,
        };
        let gate = self.holding();
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
            continues: new.continues,
        };
        let (item, number) = (self.store)
            .add(&first, &new.flow, now, &work)
            .map_err(|fault| self.stop(Fault::Store(fault), false))?;
        let head = WorkHead {
            number,
            item,
            flow: new.flow,
            added: now,
        };
        let held = Held::new(
            head,
            Item::new(item, now, first, Vec::new()),
            flow,
            installed.on(),
            Vec::new(),
            work,
            self.policy.as_ref(),
        );
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
                    merging: changes::merging(&held.flow, &held.work),
                    ended: match held.standing {
                        Standing::Ended(end) => Some(end),
                        _ => None,
                    },
                    letting_go: held.letting_go(),
                    placed: changes::placed(&held.flow, &held.work),
                    starts: changes::starts(&held.flow),
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
            // A work reopened is put on the version installed as it is reopened, from the one it ended on.
            if matches!(event, Event::Reopened { .. })
                && !held.on.installed()
                && let Some(installed) = self.flows.get(&held.head.flow)
            {
                let from = match &held.on {
                    On::Earlier { outline, .. } => Some(outline.clone()),
                    On::Installed { .. } | On::NotKept { .. } => None,
                };
                versions::put_on(&mut held.work, now, from.as_deref(), installed);
                held.passed.extend(from);
                held.on = installed.on();
            }
            let (number, item, policy) = (work, &held.item, self.policy.as_ref());
            let then = |flow: &Compiled, work: &Work| {
                let mut events = sessions::follow(flow, number, work, item, policy);
                events.extend(start(flow, work));
                events
            };
            let done =
                handle_then(&held.flow, &mut held.work, now, event, room, gate, &then).map_err(Failed::Refused)?;
            held.update(policy);
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
        if let Err(fault) = self.keep_times(now) {
            return Err(self.stop(fault, true));
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
                // A level can give the line's rounds another model, which its session is not kept warm for.
                held.warm_again(self.policy.as_ref());
                held.revision = next_revision();
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

    /// Whether the call of the work is a call of the tool that runs the project's tests.
    fn runs_tests(&self, work: WorkNumber, call: decide::CallId) -> bool {
        self.works.get(&work).is_some_and(|held| {
            matches!(
                held.work.calls.get(&call),
                Some(Call::Asked(Ask::Tool { name, .. })) if name == timed::RUN_TESTS
            )
        })
    }

    /// The merged checks of the works of the project that merged, in the order they were read.
    fn merged_checks(&self, project: Option<&str>) -> Arc<Vec<timed::Merge>> {
        self.checks(project).history.clone()
    }

    /// The merged checks of the project as they were last worked out, worked out again from the records of the works of
    /// the project that merged only where one of them changed since, or another merged: a read of them where nothing
    /// changed reads no work's records.
    fn checks(&self, project: Option<&str>) -> RefMut<'_, Checks> {
        let of: Vec<(WorkNumber, u64)> = (self.works.iter())
            .filter(|(_, held)| {
                held.standing == Standing::Ended(End::Merged) && held.item.project.as_deref() == project
            })
            .map(|(number, held)| (*number, held.revision))
            .collect();
        RefMut::map(self.checks.borrow_mut(), |checks| {
            let kept = checks.entry(project.map(str::to_string)).or_default();
            if kept.of != of {
                #[cfg(test)]
                CHECKS_WORKED_OUT.with(|worked_out| worked_out.set(worked_out.get() + 1));
                let merged =
                    (of.iter()).filter_map(|(number, _)| self.works.get(number).map(|held| (*number, &held.work)));
                *kept = Checks {
                    history: Arc::new(timed::history(merged)),
                    of,
                    tested: None,
                };
            }
            kept
        })
    }

    /// The revision of the work as the driver holds it ([`Held::revision`]): it changes each time anything held of the
    /// work changes, so a reader that keeps what it worked out of the work needs to work it out again only where its
    /// revision changed since. None where the driver shows no such work.
    pub fn revision(&self, number: WorkNumber) -> Option<u64> {
        (self.works.get(&number))
            .filter(|_| self.fault.is_none())
            .map(|held| held.revision)
    }

    /// How many values a history is judged from at least, as the installation says; none where it says nothing.
    fn samples(&self) -> usize {
        self.policy.as_ref().map_or(0, |policy| policy.samples)
    }

    /// What the merged checks of each project say of the tests' and jobs' times, judged by the history of their own
    /// kind where the installation says how many values it takes, for whoever shows how the system ran.
    pub fn tested(&self) -> Vec<timed::Tested> {
        let samples = self.samples();
        let projects: BTreeSet<Option<String>> = (self.works.values())
            .filter(|held| held.standing == Standing::Ended(End::Merged))
            .map(|held| held.item.project.clone())
            .collect();
        (projects.into_iter())
            .map(|project| {
                let mut checks = self.checks(project.as_deref());
                match &checks.tested {
                    Some((judged_by, tested)) if *judged_by == samples => tested.clone(),
                    _ => {
                        let tested = timed::tested(project, &checks.history, samples);
                        checks.tested = Some((samples, tested.clone()));
                        tested
                    }
                }
            })
            .filter(|tested| tested.checks > 0)
            .collect()
    }

    /// Writes the work's datum of [`timed::TEST_TIMES`] as the facts of its runs and of the project's history now put it,
    /// where the flow has one and that is not what it says already.
    fn write_test_times(&mut self, now: Time, number: WorkNumber, todos: &mut Vec<Todo>) -> Result<(), Fault> {
        let Some(held) = self.works.get(&number).filter(|held| held.standing == Standing::Going) else {
            return Ok(());
        };
        let ours = (held.flow.blocks.iter()).find(|(_, b)| b.writers.outside.as_deref() == Some(timed::TEST_TIMES));
        let Some((block, declared)) = ours else {
            return Ok(());
        };
        let history = self.merged_checks(held.item.project.as_deref());
        let facts = timed::facts(&held.work, &history, self.samples());
        let content = timed::render(&facts, &declared.writes);
        let block = block.clone();
        let versions = decide::versions(&held.work.records);
        let current = versions.get(&block).and_then(|versions| versions.last());
        if current.and_then(|version| held.work.contents.get(&(block.clone(), version.at))) == Some(&content) {
            return Ok(());
        }
        let event = Event::Wrote {
            block,
            by: Outsider::Outside,
            content,
            matters: true,
            value: None,
        };
        self.step_one(now, number, event, false, todos)
    }

    /// Writes the work's datum of [`timed::TEST_RUNS`]: a version for each call of `run_tests` that ended and has none yet,
    /// in the order the calls ended, where the flow has one. The versions the datum already has are the first calls, so
    /// that a call is written once however often the engine starts again.
    fn write_test_runs(&mut self, now: Time, number: WorkNumber, todos: &mut Vec<Todo>) -> Result<(), Fault> {
        let Some(held) = self.works.get(&number).filter(|held| held.standing == Standing::Going) else {
            return Ok(());
        };
        let ours = (held.flow.blocks.iter()).find(|(_, b)| b.writers.outside.as_deref() == Some(timed::TEST_RUNS));
        let Some((block, declared)) = ours else {
            return Ok(());
        };
        let block = block.clone();
        let had = decide::versions(&held.work.records).get(&block).map_or(0, Vec::len);
        let missing: Vec<String> = timed::runs(&held.work, &declared.writes, self.tests_given.as_ref())
            .into_iter()
            .skip(had)
            .collect();
        for content in missing {
            let event = Event::Wrote {
                block: block.clone(),
                by: Outsider::Outside,
                content,
                matters: true,
                value: None,
            };
            self.step_one(now, number, event, false, todos)?;
        }
        Ok(())
    }

    /// Keeps what the time of each work that ended on the installed version went to, and each stretch it held the turn
    /// of its project, where it has kept none since it last ended, at the time the caller's clock gives (decision
    /// `a-work-keeps-what-its-time-went-to`): as the work ends, while that version, which alone can read what it waited
    /// for, is installed.
    fn keep_times(&mut self, now: Time) -> Result<(), Fault> {
        let numbers: Vec<WorkNumber> = (self.works.iter())
            .filter(|(_, held)| held.keeps_time())
            .map(|(number, _)| *number)
            .collect();
        if numbers.is_empty() {
            return Ok(());
        }
        let cannot_run = self.cannot_runs(&self.at_once());
        let stops = self.stop_spans();
        for number in numbers {
            let Some(held) = self.works.get(&number) else {
                continue;
            };
            let turns = self.turns_of_others(number);
            let stretches = time::stretches(&held.kept(), &self.spans, &stops, &cannot_run, turns, now);
            let turn = held.turned.spans.clone();
            let Some(held) = self.works.get_mut(&number) else {
                continue;
            };
            let at = held.work.tick();
            held.work.kept_time.push(KeptTime {
                at,
                stretches,
                turn: Some(turn),
            });
            held.work.moments.push(Moment { from: at, at: now });
            held.revision = next_revision();
            self.store.save(number, &held.work).map_err(Fault::Store)?;
        }
        Ok(())
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
        // A call of the tool that runs the project's tests ended: what the work's runs said of the tests' times, and
        // what the call asked for and gave, are told to the cells that read them, once the call's own record is kept.
        let tested = matches!(&event, Event::Done { call, .. } if self.runs_tests(work, *call));
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
            Ok((effects, _)) => todos.extend(effects.into_iter().map(|effect| held.todo(effect))),
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
        if tested && let Err(fault) = self.write_test_times(now, work, &mut todos) {
            return Err(self.stop(fault, true));
        }
        if tested && let Err(fault) = self.write_test_runs(now, work, &mut todos) {
            return Err(self.stop(fault, true));
        }
        // The round's own work is paused by its failure; every other that runs a round on the model is paused with it.
        if let Some((on, why)) = no_model {
            let (number, cause) = (work.to_string(), flow::agents::sentence(&why));
            let found = [
                ("model", on.model.as_str()),
                ("level", on.level.as_str()),
                ("work", number.as_str()),
                ("why", cause.as_str()),
            ];
            let why = flow::agents::fill(&self.works[&work].flow.waits.paused.no_model, &found);
            let numbers: Vec<WorkNumber> = (self.works.iter())
                .filter(|(number, held)| **number != work && runs_on(held, self.policy.as_ref(), &on.model))
                .map(|(number, _)| *number)
                .collect();
            for number in numbers {
                let paused = Event::Paused {
                    by: Pauser::Engine,
                    why: Some(why.clone()),
                    waits_for: Vec::new(),
                };
                if let Err(fault) = self.step_one(now, number, paused, false, &mut todos) {
                    return Err(self.stop(fault, true));
                }
            }
        }
        if let Err(fault) = self.keep_times(now) {
            return Err(self.stop(fault, true));
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
        if let Err(fault) = self.move_if_due(now) {
            return Err(self.stop(fault, false));
        }
        match self.look(now) {
            Ok(todos) => self.filled_after(now, todos),
            Err(fault) => Err(self.stop(fault, true)),
        }
    }

    /// The engine is asked to stop, at the time the caller's clock gives: once what is under way has ended, or, where
    /// `at_once`, at once, cutting it off. From then no round of any work begins, not even one of a run that answers a
    /// note; calls are still given their results and keep-warm pings still sent, and what is under way goes on, its end
    /// kept as any is. It is kept in the store, and stands until the driver is opened again. Asked again while a stop
    /// stands changes nothing, but that one asked at once while one waits for what is under way stands from then.
    pub fn ask_to_stop(&mut self, now: Time, at_once: bool) -> Result<(), Failed> {
        self.going()?;
        if self.stopping().is_some_and(|stop| stop.at_once || !at_once) {
            return Ok(());
        }
        let record = StopRecord {
            what: if at_once { StopAct::Now } else { StopAct::Asked },
            at: now,
        };
        if let Err(fault) = self.store.keep_stop(&record) {
            return Err(self.stop(Fault::Store(fault), false));
        }
        self.stops.push(record);
        Ok(())
    }

    /// The stop of the engine, while one stands: since it was first asked for, and whether at once.
    pub fn stopping(&self) -> Option<Stopping> {
        let latest = self.stops.last().filter(|latest| latest.what != StopAct::Over)?;
        let since = stop_spans(&self.stops).last().map_or(latest.at, |span| span.from);
        Some(Stopping {
            since,
            at_once: latest.what == StopAct::Now,
        })
    }

    /// Every stretch a stop of the engine stood, in order, the one that stands among them.
    pub fn stop_spans(&self) -> Vec<StopSpan> {
        stop_spans(&self.stops)
    }

    /// Whether the engine may end as the stop that stands asks: it was asked to stop at once, or nothing is under way,
    /// no round of any work, an act's among them, and no keep-warm ping. False where no stop stands.
    pub fn may_end(&self) -> bool {
        match self.stopping() {
            Some(stop) => stop.at_once || !self.under_way(),
            None => false,
        }
    }

    /// Whether a round of any work is under way, a round being stopped among them until it has ended, or a keep-warm
    /// ping.
    fn under_way(&self) -> bool {
        !self.pinging.is_empty()
            || (self.works.values()).any(|held| held.work.records.rounds.len() > held.work.records.returned.len())
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

    /// Whether the engine may move to another account at the usage limit of the one in use, as the owner last switched
    /// it; it may not until he switches it on.
    pub fn moving_on(&self) -> bool {
        self.moving_on
    }

    /// Lets the engine move to another account at a usage limit from the time the caller's clock gives, or does not,
    /// and gives what is to be done. It is kept in the store and holds when the driver is opened again, and switching
    /// it as it stands changes nothing. No check is taken here: one that is due is asked for at the wake the driver
    /// gives ([`Driver::wake`]); but where an account was found well in the wait before the switch was off, it is taken
    /// now.
    pub fn set_moving_on(&mut self, now: Time, moving: bool) -> Result<Vec<Todo>, Failed> {
        self.going()?;
        if moving == self.moving_on {
            return Ok(Vec::new());
        }
        if let Err(fault) = self.store.set_moving_on(moving, now) {
            return Err(self.stop(Fault::Store(fault), false));
        }
        self.moving_on = moving;
        match self.move_if_due(now) {
            Ok(false) => Ok(Vec::new()),
            Ok(true) => match self.look(now) {
                Ok(todos) => self.filled_after(now, todos),
                Err(fault) => Err(self.stop(fault, true)),
            },
            Err(fault) => Err(self.stop(fault, false)),
        }
    }

    /// Sets how many rounds of cells that take a slot may go on at once, at the time the caller's clock gives, and gives
    /// what is to be done: with more, the slots free now go to the works that wait for one, in the engine's order; with
    /// fewer, no round under way is stopped, and none begins until fewer are under way than there are slots. The store
    /// keeps no slots: whoever sets them keeps them, so that the driver is opened with them again. Setting them as they
    /// stand changes nothing.
    pub fn set_slots(&mut self, now: Time, slots: usize) -> Result<Vec<Todo>, Failed> {
        self.going()?;
        if slots == self.slots {
            return Ok(Vec::new());
        }
        self.slots = slots;
        self.filled_after(now, Vec::new())
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

    /// Keeps what the owner did to the account, in the store and then here.
    fn keep_account(&mut self, now: Time, name: &str, what: AccountAct) -> Result<(), Failed> {
        let record = AccountRecord {
            name: name.to_string(),
            what,
            at: now,
            taker: AccountTaker::Owner,
            left_resets: None,
        };
        self.record_account(record).map_err(|fault| self.stop(fault, false))
    }

    /// Keeps a record of what was done to an account, in the store and then here.
    fn record_account(&mut self, record: AccountRecord) -> Result<(), Fault> {
        self.store.keep_account(&record).map_err(Fault::Store)?;
        self.accounts.push(record);
        Ok(())
    }

    /// Takes the account that moving on says to take, where the owner has let the engine move, nothing is paused and a
    /// check under another account found it well in the wait at the in-use account's usage limit: the engine takes it
    /// as the owner does, and the outside is checked at once under it where the wait that stands is another's. Gives
    /// whether an account was taken.
    fn move_if_due(&mut self, now: Time) -> Result<bool, Fault> {
        if !self.moving_on || self.paused.is_some() {
            return Ok(false);
        }
        let Some(outage) = self.agents_wait() else {
            return Ok(false);
        };
        let moved = outage::move_on(&outage, &self.accounts, || self.limits(), &self.checked, now);
        let Move::Take { name, left_resets } = moved else {
            return Ok(false);
        };
        self.record_account(AccountRecord {
            name,
            what: AccountAct::Taken,
            at: now,
            taker: AccountTaker::Engine,
            left_resets,
        })?;
        self.ticked = self.ticked.filter(|at| *at < now);
        Ok(true)
    }

    /// The accounts the engine took at a usage limit, newest first, each with the account it left.
    pub fn moves(&self) -> Vec<Moved> {
        self.accounts.moves()
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
        outage::limits(self.rounds(), &self.answered)
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
            (Some(life), Some(held)) => cache::misses_by(&held.line_of(), &held.work, life),
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
                part: &held.part,
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
        // The engine's own rounds run the project's tests, which a test past its own history's line is stopped in.
        if cell.kind == flow::Kind::Engine {
            lines.test_time = self.test_time(work);
        }
        lines
    }

    /// How long one test of the work's project may run in the project's CI before it is stopped, in milliseconds, as
    /// the history of the project's merged checks says it ([`timed::test_time`]); none where the installation says no
    /// number of values to judge by or the history is too short.
    pub fn test_time(&self, work: WorkNumber) -> Option<f64> {
        let held = self.works.get(&work)?;
        timed::test_time(&self.merged_checks(held.item.project.as_deref()), self.samples())
    }

    /// The median of each test's times in the earlier fast runs of the work's project, every work of it counted, by the
    /// name a request of `run_tests` asks for it by ([`timed::fast_medians`]); empty where the work is not held.
    pub fn fast_medians(&self, work: WorkNumber) -> BTreeMap<String, f64> {
        let Some(held) = self.works.get(&work) else {
            return BTreeMap::new();
        };
        let project = held.item.project.as_deref();
        let readings = (self.works.values())
            .filter(|other| other.item.project.as_deref() == project)
            .flat_map(|other| other.work.timed.iter().map(|t| &t.reading));
        timed::fast_medians(readings)
    }

    /// The wait every work shares for the agents' service, if one stands: worked out from the records of every work
    /// and the rounds the service answered while under way ([`outage`]), with the checks of the outside taken in it.
    pub fn outage(&self) -> Option<Outage> {
        let mut outage = self.agents_wait()?;
        outage.may_move = self.moving_on
            && outage::may_move(
                &outage,
                &self.accounts,
                || self.limits(),
                &self.checked,
                self.ticked.unwrap_or(outage.met),
            );
        Some(outage)
    }

    /// The wait every work shares for the agents' service as [`Driver::outage`] gives it, but for whether the engine may
    /// yet move to another account in it: what the driver decides by, which asks nothing of the accounts' limits, whose
    /// worth is a scan of every round of every work.
    fn agents_wait(&self) -> Option<Outage> {
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
        let mut outage = outage::outage(service, self.rounds(), &self.answered, in_use)?;
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

    /// Each work's rounds as the waits every work shares read them ([`outage::Rounds`]): what a wait is worked out
    /// from, each round served as the version its work is on has its cell, so that a work that met a wait and ended
    /// since, on whatever version, still says it. A work on none kept has none: which of its cells an agent ran its
    /// version does not say, and it ended before versions were kept.
    fn rounds(&self) -> impl Iterator<Item = &outage::Rounds> + Clone {
        self.works.values().map(|held| &held.rounds)
    }

    /// The waits of every service, as they stand.
    fn outages(&self) -> (Option<Outage>, Option<Outage>) {
        (self.agents_wait(), self.host_outage())
    }

    /// What carries out a round or a call of the work says where it stands now, or, with none, that it no longer says
    /// so, at this time; taken while it is under way, and dropped otherwise. Gives whether what is kept changed. What
    /// it kept of the work's rounds and calls no longer under way is let go. Where it says it waits for a runner, it
    /// has waited since the first time it said so for that runner after it last did not.
    pub fn along(&mut self, at: Time, work: WorkNumber, under: Under, along: Option<Along>) -> bool {
        let Some(held) = self.works.get(&work) else {
            return false;
        };
        let records = &held.work.records;
        self.along
            .retain(|(of, under), _| *of != work || under.under_way(records));
        self.queued
            .retain(|(of, under), _| *of != work || under.under_way(records));
        let key = (work, under);
        let was = self.along.get(&key).cloned();
        match along {
            Some(along) if under.under_way(records) => {
                match &along {
                    Along::Runs {
                        queued_for: Some(runner),
                        ..
                    } => {
                        let since = (self.queued.get(&key))
                            .filter(|(was, _)| was == runner)
                            .map_or(at, |(_, since)| *since);
                        self.queued.insert(key, (runner.clone(), since));
                    }
                    _ => {
                        self.queued.remove(&key);
                    }
                }
                self.along.insert(key, along);
            }
            _ => {
                self.along.remove(&key);
                self.queued.remove(&key);
            }
        }
        self.along.get(&key) != was.as_ref()
    }

    /// Each round or call of the work under way that waits for a runner, with the runner and since when it has waited
    /// for it, as what carries it out has said since the engine started.
    pub fn queued_of(&self, work: WorkNumber) -> Vec<(Under, &str, Time)> {
        let Some(held) = self.works.get(&work) else {
            return Vec::new();
        };
        (self.queued.iter())
            .filter(|((of, under), _)| *of == work && under.under_way(&held.work.records))
            .map(|((_, under), (runner, since))| (*under, runner.as_str(), *since))
            .collect()
    }

    /// What the agents' rounds under way have come to so far, as what carries them out last said, each with its work,
    /// run and round: a round that has made no call has told nothing and is not there.
    pub fn rounds_come_to(&self) -> Vec<(WorkNumber, decide::RunId, usize, Going)> {
        (self.along.iter())
            .filter_map(|((work, under), along)| {
                let (Along::Going(going), Under::Round { run, round }) = (along, under) else {
                    return None;
                };
                let held = self.works.get(work)?;
                under
                    .under_way(&held.work.records)
                    .then_some((*work, *run, *round, *going))
            })
            .collect()
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

    /// Takes, at the time the caller's clock gives, that the service the round under way of the run reaches answered
    /// it: the agents' service an agent's round, the host an act. Where the round began after the latest that failed
    /// for a cause outside every work, that service's wait is over, and every work is looked at again; what is to be
    /// done is given. A round no longer under way is passed over.
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
        let waits = self.outages();
        self.answered.insert((work, run, began));
        if self.outages() == waits {
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
        let (agents, host) = self.outages();
        let other = self
            .next_wake(agents.as_ref(), host.as_ref())
            .filter(|wake| ticked.is_none_or(|at| wake.at > at));
        // A check to move to an account is asked for by what has been checked, never by the time it was last told: a
        // check that is taken is kept, and is not asked for again. It goes before another wake no later than it.
        match (self.next_move_check(agents.as_ref()), other) {
            (Some((at, name)), other) if other.as_ref().is_none_or(|wake| at <= wake.at) => Some(Wake {
                at,
                check: true,
                candidate: Some(name),
                look_at_host: false,
            }),
            (_, other) => other,
        }
    }

    /// The check to move to an account that is due first, and when, where the owner has let the engine move, nothing is
    /// paused and the agents' wait is at the in-use account's usage limit ([`outage::move_on`]).
    fn next_move_check(&self, agents: Option<&Outage>) -> Option<(Time, String)> {
        if !self.moving_on || self.paused.is_some() {
            return None;
        }
        let outage = agents?;
        let now = self.ticked.unwrap_or(outage.met);
        match outage::move_on(outage, &self.accounts, || self.limits(), &self.checked, now) {
            Move::Check { name, at } => Some((at, name)),
            Move::Take { .. } | Move::Stay => None,
        }
    }

    /// When the driver is to be told the time next, as the waits every work shares say, whether or not it has come:
    /// the earliest of each wait's and of the next keep-warm ping's, with each check and look that is due then.
    fn next_wake(&self, agents: Option<&Outage>, host: Option<&Outage>) -> Option<Wake> {
        let standing = agents.is_some();
        let agents = agents.and_then(|outage| wake_of(outage, None));
        let host = host.and_then(|outage| wake_of(outage, self.looked_at_host));
        // A keep-warm ping waits as the agents' rounds do: while their wait stands, its own wakes serve.
        let ping = if standing { None } else { self.next_ping() };
        let at = [agents.map(|(at, _)| at), host.map(|(at, _)| at), ping]
            .into_iter()
            .flatten()
            .min()?;
        Some(Wake {
            at,
            check: agents.is_some_and(|(a, _)| a == at),
            candidate: None,
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
        if let Some((under, found)) = found
            && let Some(outage) = outage::outage(Service::Agents, self.rounds(), &self.answered, under.as_deref())
        {
            let before = (self.checked.get(&under)).filter(|(of, _)| *of == outage.since);
            let taken = before.map_or(0, |(_, checked)| checked.taken) + 1;
            self.checked
                .insert(under, (outage.since, Checked { taken, at: now, found }));
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

    /// Takes what a check under an account other than the one in use found, at the time the caller's clock gives, the
    /// check having been asked for to move to that account ([`Wake::candidate`]), and takes the account where the
    /// owner has let the engine move and the check found it well; gives what is to be done. The check is kept with the
    /// `since` of the wait at the in-use account's usage limit; one taken when no such wait stands is passed over. It
    /// is no time the driver was told ([`Driver::tick`]): what is asked for is worked out from what was checked.
    pub fn tick_candidate(&mut self, now: Time, account: &str, found: Found) -> Result<Vec<Todo>, Fault> {
        if let Some(fault) = &self.fault {
            return Err(fault.clone());
        }
        if let Some(since) = self.agents_wait().map(|outage| outage.since) {
            let key = Some(account.to_string());
            let before = (self.checked.get(&key)).filter(|(of, _)| *of == since);
            let taken = before.map_or(0, |(_, checked)| checked.taken) + 1;
            self.checked.insert(key, (since, Checked { taken, at: now, found }));
        }
        let taken = match self.move_if_due(now) {
            Ok(taken) => taken,
            Err(fault) => {
                self.stop(fault.clone(), false);
                return Err(fault);
            }
        };
        if !taken {
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

    /// The keep-warm pings due at `now` and not under way, each given once: of every work that has not ended, whoever
    /// holds it, and none while the wait every work shares holds the agents' rounds, whose pings would fail as their
    /// rounds do ([`warm`]). They are taken from the sessions each work keeps warm ([`Held::warm`]), which are as the
    /// work stands: the look at every work just before ([`Driver::tick`]) records each new session a line is to begin.
    fn pings(&mut self, now: Time) -> Vec<Todo> {
        if self.policy.is_none() || self.gate().holds_agents() {
            return Vec::new();
        }
        let mut todos = Vec::new();
        for (number, held) in &self.works {
            if held.standing != Standing::Going {
                continue;
            }
            for kept in warm::as_of(&held.warm, self.keep_warm, &self.accounts, now).filter(|kept| kept.at <= now) {
                if self.pinging.contains(&(*number, kept.due.line.clone())) {
                    continue;
                }
                todos.push(held.todo(Effect::Ping {
                    line: kept.due.line.clone(),
                    session: kept.due.session,
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
    /// will not give. It is taken from the sessions each work keeps warm as its records say ([`Held::warm`]), and goes
    /// through no work's sessions again.
    fn next_ping(&self) -> Option<Time> {
        self.policy.as_ref()?;
        let now = self.ticked.unwrap_or(Time(0));
        let soonest = Time(now.0.saturating_add(1));
        (self.works.iter())
            .filter(|(_, held)| held.standing == Standing::Going)
            .flat_map(|(number, held)| {
                (warm::as_of(&held.warm, self.keep_warm, &self.accounts, now))
                    .filter(|kept| !self.pinging.contains(&(*number, kept.due.line.clone())))
                    .map(|kept| (kept.at.max(soonest), kept.life))
                    .filter(|(at, life)| at < life)
                    .map(|(at, _)| at)
            })
            .min()
    }

    /// The rounds a work is stepped with held back: those the wait every work shares holds, and, for each line of the
    /// work whose session a keep-warm ping is under way for, those of its cells, so that the line's next round begins
    /// once the ping has ended, after whatever the ping did to the session, and is filled as it then stands. The
    /// rounds of the work's other lines begin as they would without the ping.
    pub fn gate_of(&self, work: WorkNumber) -> Hold {
        self.hold_with(work, self.holding())
    }

    /// The rounds a work is stepped with held back ([`Driver::gate_of`]), the waits every work shares and the stop of the
    /// engine holding those `holding` says.
    fn hold_with(&self, work: WorkNumber, holding: Gate) -> Hold {
        Hold {
            gate: holding,
            lines: (self.pinging.iter())
                .filter(|(w, _)| *w == work)
                .map(|(_, line)| line.clone())
                .collect(),
        }
    }

    /// Which rounds may begin now, as the waits every work shares and the stop of the engine say: while a stop stands,
    /// none.
    fn holding(&self) -> Gate {
        match self.stopping() {
            Some(_) => Gate::Every,
            None => self.gate(),
        }
    }

    /// Whether a wait every work shares holds rounds now.
    pub fn holds(&self) -> bool {
        self.gate() != Gate::Open
    }

    /// Which rounds may begin now, as the waits every work shares say: the agents' wait holds the agents' rounds until
    /// a check of the outside finds it well, and the acts' wait the acts' until a look at their host reaches it.
    pub fn gate(&self) -> Gate {
        gate_of_waits(self.agents_wait().as_ref(), self.host_outage().as_ref())
    }

    /// Looks at every work that has not ended, in the order of their numbers, each with the gate as it is then and
    /// given no slot, so that the free slots go to the works in the engine's order once all have been looked at. Where
    /// a wait every work shares lets one round go first, the work whose round met it first is looked at first.
    ///
    /// The gate is worked out once, and again after each work that changed as it was looked at: the waits every work
    /// shares are worked out from the rounds of every work, which a look changes only in the work it changes, and from
    /// nothing else a look changes. So a look at many works that change nothing works them out once, not once a work.
    fn look(&mut self, now: Time) -> Result<Vec<Todo>, Fault> {
        let mut todos = Vec::new();
        let first = first_turns(self.agents_wait().as_ref(), self.host_outage().as_ref());
        let mut numbers: Vec<WorkNumber> = (self.works.iter())
            .filter(|(_, held)| held.standing == Standing::Going)
            .map(|(number, _)| *number)
            .collect();
        numbers.sort_by_key(|number| (!first.contains(number), *number));
        let mut holding = self.holding();
        for number in numbers {
            if self.step_with(now, number, Event::Look, false, holding, &mut todos)? {
                holding = self.holding();
            }
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
        let holding = self.holding();
        self.step_with(now, number, event, slot, holding, todos).map(|_| ())
    }

    /// Gives one thing that happened to a work that has not ended to the engine's loop, as [`Driver::step_one`] does,
    /// the waits every work shares and the stop of the engine holding those `holding` says. Gives whether the work
    /// changed as the driver holds it.
    fn step_with(
        &mut self,
        now: Time,
        number: WorkNumber,
        event: Event,
        slot: bool,
        holding: Gate,
        todos: &mut Vec<Todo>,
    ) -> Result<bool, Fault> {
        let (room, gate) = (self.room(Some(number), slot), self.hold_with(number, holding));
        let Some(held) = self
            .works
            .get_mut(&number)
            .filter(|held| held.standing == Standing::Going)
        else {
            return Ok(false);
        };
        match step(self.store.as_mut(), held, self.policy.as_ref(), now, event, room, gate) {
            Ok((effects, changed)) => {
                todos.extend(effects.into_iter().map(|effect| held.todo(effect)));
                Ok(changed)
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
        let holding = self.holding();
        let gates: BTreeMap<WorkNumber, Hold> =
            (self.works.keys()).map(|n| (*n, self.hold_with(*n, holding))).collect();
        let numbers: Vec<WorkNumber> = self.works.keys().copied().collect();
        for number in &numbers {
            // The turn is the work's as the works written before it left it, so that where several are due to check at
            // once, one begins and the others wait.
            let room = self.room(Some(*number), false);
            let Some(held) = self.works.get_mut(number) else {
                continue;
            };
            let gate = gates.get(number).cloned().unwrap_or_default();
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
                Ok((effects, _)) => {
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
            .flat_map(|held| (held.pending().into_iter()).map(|effect| held.todo(effect)))
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
        self.flows.values().map(|installed| &*installed.flow)
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
        // A work no flow decides now has ended, and its cells are no flow's to walk: its blocks' versions are read
        // from its records alone.
        if let Some(end) = kept.on.end() {
            return Some(View {
                decision: Decision::Ended(end),
                cells: Vec::new(),
                blocks: decide::versions(&kept.work.records),
            });
        }
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
        if !kept.on.installed() {
            return None;
        }
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
            stopping: self.stopping(),
            agents: self.outage(),
            host: self.host_outage(),
            taking: self.taking(),
            turn: self.turns(),
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

    /// Each wait in which the agents' command line could not run, in order, as a bar reads them: those that are over,
    /// worked out from the records of every work, and the one that stands now, where one does, from what every work
    /// reads alike ([`AtOnce::cannot_run`]).
    pub fn cannot_runs(&self, at_once: &AtOnce) -> Vec<CannotRun> {
        let mut waits = outage::cannot_run_spans(self.rounds());
        waits.extend(at_once.cannot_run().map(|(since, why)| CannotRun {
            from: since,
            until: None,
            why: why.to_string(),
        }));
        waits
    }

    /// Each stretch another work of the work's project held the turn to check and merge, which what the work waited for
    /// is read by: each as the work kept it as it ended, or as it is worked out.
    fn turns_of_others(&self, work: WorkNumber) -> Vec<TurnSpan> {
        let Some(project) = self.works.get(&work).map(|held| held.item.project.as_deref()) else {
            return Vec::new();
        };
        (self.works.values())
            .filter(|held| held.head.number != work && held.item.project.as_deref() == project)
            .flat_map(Held::turn_spans)
            .collect()
    }

    /// The stretches each work held the turn of its project, with the project's name, for a reader of many works, which
    /// reads the others' stretches of each ([`Driver::turns_of_others`]) from them.
    pub fn turn_spans(&self) -> Vec<(Option<String>, WorkNumber, Vec<TurnSpan>)> {
        (self.works.values())
            .map(|held| (held.item.project.clone(), held.head.number, held.turn_spans()))
            .collect()
    }

    /// Whose turn it is to check and merge, where the work waits for it, and since when the work has stood at the cells
    /// that take it without the turn: since it wanted it, or, of a work that holds it where another that holds it too
    /// began first, as records kept before one work alone could hold it can say, since its hold began. None where it
    /// waits for no turn, or it is the work's own.
    pub fn turn_waited_at(&self, work: WorkNumber, at_once: &AtOnce) -> Option<(WorkNumber, Time)> {
        let held = self.works.get(&work)?;
        let whose = at_once
            .turn_of(held.item.project.as_deref())
            .filter(|whose| *whose != work)?;
        let holding = (held.turned.spans.last())
            .filter(|span| held.turned.holds && span.until.is_none())
            .map(|span| span.from);
        Some((whose, held.turned.wants.or(holding)?))
    }

    /// The items a pause waits for, as the pause keeps them, in the order of their numbers, each once: where it is made
    /// for the work of `own`, none of them is that item, which would never end, and each of them is an item the driver
    /// has.
    pub fn pause_waits(&self, own: Option<ItemNumber>, items: Vec<ItemNumber>) -> Result<Vec<u64>, Failed> {
        let mut waits: Vec<u64> = Vec::with_capacity(items.len());
        for item in items {
            if Some(item) == own {
                return Err(Failed::WaitsForItself(item));
            }
            if !self.of_item.contains_key(&item) {
                return Err(Failed::NoSuchItem(item));
            }
            waits.push(item.0);
        }
        waits.sort_unstable();
        waits.dedup();
        Ok(waits)
    }

    /// The items the work's pause waits for that have neither merged nor been cancelled, in the order of their
    /// numbers: none where the work is not paused, or its pause waits for no item, or each has ended. The owner's wait
    /// for them is over then, and the work is let on by the owner alone.
    pub fn items_paused_for(&self, number: WorkNumber) -> Vec<ItemNumber> {
        let Some(held) = self.works.get(&number) else {
            return Vec::new();
        };
        let Some(pause) = decide::pause(&held.work.records) else {
            return Vec::new();
        };
        let named: Vec<ItemNumber> = pause.waits_for.iter().map(|item| ItemNumber(*item)).collect();
        self.waited_for(&named)
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
    /// it, so a slot it held would be held for good; nor does one no flow decides now, which has ended.
    fn taking(&self) -> BTreeSet<WorkNumber> {
        (self.works.values())
            .filter(|held| held.standing != Standing::AtFault)
            .filter(|held| {
                held.deciding()
                    .is_some_and(|flow| taking_slot(&flow.flow, &held.work.records))
            })
            .map(|held| held.head.number)
            .collect()
    }

    /// The room the engine leaves a work: the pause of everything, as it stands; and, where `slot` says a slot
    /// may be given at all, whether fewer are taken by the other works than there are.
    /// And whether it is the work's turn to check and merge, which no work given is not asking for.
    fn room(&self, work: Option<WorkNumber>, slot: bool) -> Room {
        let project = work
            .and_then(|work| self.works.get(&work))
            .map(|held| held.item.project.as_deref());
        Room {
            paused: self.paused.is_some(),
            slot: slot && self.taken(work) < self.slots,
            turn: match (work, project) {
                (Some(work), Some(project)) => self.turn_in(project).is_none_or(|turn| turn == work),
                _ => true,
            },
        }
    }

    /// Whose turn it is to check and merge in the project: the work that holds it, else the one that has wanted it
    /// longest, among the works of the project ([`turn::whose`]); none where no work holds or wants it.
    fn turn_in(&self, project: Option<&str>) -> Option<WorkNumber> {
        turn::whose(
            (self.works.values())
                .filter(|held| held.item.project.as_deref() == project)
                .map(|held| (held.head.number, &held.turned)),
        )
    }

    /// Each project that has a work, with whose turn it is to check and merge in it where a work of it holds or wants
    /// the turn.
    fn turns(&self) -> BTreeMap<Option<String>, WorkNumber> {
        let projects: BTreeSet<Option<String>> = self.works.values().map(|held| held.item.project.clone()).collect();
        (projects.into_iter())
            .filter_map(|project| self.turn_in(project.as_deref()).map(|work| (project, work)))
            .collect()
    }

    /// The room the engine leaves a work ([`Driver::room`], a slot given where one is free), as what holds every work
    /// at once says.
    pub fn room_at(&self, work: WorkNumber, at_once: &AtOnce) -> Room {
        let project = self.works.get(&work).and_then(|held| held.item.project.as_deref());
        Room {
            paused: at_once.everything.is_some(),
            slot: taken_among(&at_once.taking, Some(work)) < self.slots,
            turn: at_once.turn_of(project).is_none_or(|turn| turn == work),
        }
    }

    /// Gives the turn and the free slots away ([`Driver::give_away`]) after something that was kept, adding what is to be
    /// done to these todos; where that fails, the driver stops, what was asked having been kept, and the fault is given.
    fn filled(&mut self, now: Time, mut todos: Vec<Todo>) -> Result<Vec<Todo>, Fault> {
        let fault = match self.give_away(now) {
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

    /// Gives each project's turn to check and merge to the work whose turn it is, where it wants it and does not hold it
    /// yet, and then the free slots ([`Driver::fill`]), and gives what is to be done. The turn is given first, for a work
    /// that is given it may begin a round that takes a slot.
    fn give_away(&mut self, now: Time) -> Result<Vec<Todo>, Halt<Fault>> {
        let mut todos = self.give_turns(now)?;
        todos.extend(self.fill(now)?);
        Ok(todos)
    }

    /// Looks at each work whose turn it is to check and merge in its project, that has no round under way, in the room
    /// the engine leaves it, which begins its round if nothing else holds it back: one that wants the turn, and one that
    /// holds it where another that held it too began first. Each is looked at once, so that one that begins no round
    /// does not keep another from being looked at.
    fn give_turns(&mut self, now: Time) -> Result<Vec<Todo>, Halt<Fault>> {
        let mut todos = Vec::new();
        let mut looked = BTreeSet::new();
        loop {
            let next = (self.turns().into_values())
                .filter(|number| !looked.contains(number))
                .find(|number| {
                    (self.works.get(number))
                        .is_some_and(|held| held.work.records.rounds.len() == held.work.records.returned.len())
                });
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
                Ok((effects, _)) => todos.extend(effects.into_iter().map(|effect| held.todo(effect))),
                Err(Halt::Store(fault)) => return Err(Halt::Store(fault)),
                Err(Halt::Refused(refused)) => return Err(Halt::Refused(Fault::Refused { work: number, refused })),
            }
        }
        Ok(todos)
    }

    /// Gives each free slot to the first work in the engine's order that waits for one, and gives what is to be done:
    /// the works that have begun a round before those that have not, each in the order of their numbers, and before
    /// them all the work whose round met the agents' wait first, where the wait lets one round go first. A work given
    /// a slot is looked at; each is looked at once, so that one that turns out to begin no round does not take the
    /// turn of the others.
    fn fill(&mut self, now: Time) -> Result<Vec<Todo>, Halt<Fault>> {
        let mut todos = Vec::new();
        let mut looked = BTreeSet::new();
        let first = first_turns(self.agents_wait().as_ref(), None);
        while self.taken(None) < self.slots {
            let waiting = self.room(None, false);
            let next = (self.works.values())
                .filter(|held| held.standing == Standing::Going && !looked.contains(&held.head.number))
                .filter(|held| engine::held(&held.flow, &held.work, waiting) == Some(Holder::Slot))
                .min_by_key(|held| {
                    let number = held.head.number;
                    (
                        !first.contains(&number),
                        !begun(&held.flow.flow, &held.work.records),
                        number,
                    )
                })
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
                Ok((effects, _)) => todos.extend(effects.into_iter().map(|effect| held.todo(effect))),
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
            .map(|on| on.to_string())
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
    /// The work held on this version, with how it stands and its rounds that are history worked out.
    fn new(
        head: WorkHead,
        item: Item,
        flow: Arc<Compiled>,
        on: On,
        passed: Vec<Arc<flow::Outline>>,
        work: Work,
        policy: Option<&Policy>,
    ) -> Held {
        let mut held = Held {
            head,
            item,
            flow,
            on,
            passed,
            work,
            standing: Standing::Going,
            samples: Vec::new(),
            turned: Turned::default(),
            rounds: outage::Rounds::default(),
            warm: Vec::new(),
            part: judged::Part::default(),
            revision: 0,
        };
        held.update(policy);
        held
    }

    /// The work as the store keeps it, held at fault by the driver: nothing more is decided of it, so it no longer holds
    /// the turn of its project and wants none, and the works that wait for the turn go on. Its rounds are read as the
    /// store keeps them.
    fn hold_at_fault(&mut self, kept: Work) {
        self.work = kept;
        self.standing = Standing::AtFault;
        self.turned.clear();
        self.rounds = outage::Rounds::of(self.head.number, self.on.outline(), &self.work);
        self.warm.clear();
        self.part = self.part_of();
        self.revision = next_revision();
    }

    /// What the work adds to the judgments of its flow, as it is held now ([`judged::Part::of`]).
    fn part_of(&self) -> judged::Part {
        let compiled = (!matches!(self.on, On::NotKept { .. })).then_some(&*self.flow);
        judged::Part::of(self.head.number, compiled, self.on.reading(), &self.work, &self.samples)
    }

    /// Works out again how the work stands, its rounds that are history, its rounds as the waits every work shares
    /// read them, the sessions it keeps warm and what it adds to the judgments of its flow, as it grew, and gives it a
    /// new revision: how it stands by its end alone where no flow decides it now.
    fn update(&mut self, policy: Option<&Policy>) {
        self.standing = match self.on.end() {
            Some(end) => Standing::Ended(end),
            None => standing_of(&self.flow, &self.work),
        };
        self.samples = samples_of(&self.flow, &self.on, &self.work);
        self.rounds = outage::Rounds::of(self.head.number, self.on.outline(), &self.work);
        if self.on.installed() {
            self.turned
                .update(&self.flow.flow, &self.work, self.standing == Standing::Going);
        } else {
            self.turned.clear();
        }
        self.warm_again(policy);
        self.part = self.part_of();
        self.revision = next_revision();
    }

    /// Works out again the sessions the work keeps warm as its records, its item and the policy say: none of a work
    /// that is not going, or where no policy keeps sessions warm.
    fn warm_again(&mut self, policy: Option<&Policy>) {
        self.warm = match policy {
            Some(policy) if self.standing == Standing::Going => {
                warm::of_records(&self.flow, self.head.number, &self.work, &self.item, policy)
            }
            _ => Vec::new(),
        };
    }

    /// Whether what the work holds outside is being let go ([`engine::letting_go`]).
    fn letting_go(&self) -> bool {
        match self.on.end() {
            Some(end) => engine::letting_go_after(Some(end), &self.work),
            None => engine::letting_go(&self.flow, &self.work),
        }
    }

    /// What was to be done for the work and is not yet known as done ([`engine::pending`]).
    fn pending(&self) -> Vec<Effect> {
        match self.on.end() {
            Some(end) => engine::pending_after(Some(end), &self.work),
            None => engine::pending(&self.flow, &self.work),
        }
    }

    /// Whether what the work's time went to is to be kept now: it ended on the installed version, nothing of it is
    /// under way any longer, as when what it holds outside is let go ([`engine::at_rest`]), so that a round of a run
    /// abandoned as it was cancelled is kept as it came back, and it has kept none since it was last reopened.
    fn keeps_time(&self) -> bool {
        let r = &self.work.records;
        self.on.installed()
            && matches!(self.standing, Standing::Ended(_))
            && engine::at_rest(&self.work)
            && (self.work.kept_time.last()).is_none_or(|kept| r.reopens.iter().any(|reopen| *reopen > kept.at))
    }

    /// Each stretch the work held the turn of its project, in order: as it kept them as it ended, where it has kept its
    /// time since it was last reopened, none where it kept none, so that a work that ended before they were kept is read
    /// as one that never held it; else as they are worked out.
    fn turn_spans(&self) -> Vec<TurnSpan> {
        let work = &self.work;
        let kept = (work.kept_time.last()).filter(|kept| !work.records.reopens.iter().any(|reopen| *reopen > kept.at));
        match kept {
            Some(kept) => kept.turn.clone().unwrap_or_default(),
            None => self.turned.spans.clone(),
        }
    }

    /// The flow that decides the work: the installed one, where the work is on it.
    fn deciding(&self) -> Option<&Compiled> {
        self.on.installed().then_some(&*self.flow)
    }

    /// The line of each cell of the work, as the version it is on has it: none of a work on no version kept.
    fn line_of(&self) -> impl Fn(&CellId) -> Option<String> + '_ {
        move |cell: &CellId| {
            let outline = self.on.outline()?;
            outline.cell(cell).and_then(|c| c.line.clone())
        }
    }

    fn kept(&self) -> Kept<'_> {
        Kept {
            head: &self.head,
            item: &self.item,
            flow: &self.flow,
            on: &self.on,
            passed: &self.passed,
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
/// added before giving back what is to be done, with whether the work changed as the driver holds it: it grew, or is
/// held at fault.
fn step(
    store: &mut dyn Storage,
    held: &mut Held,
    policy: Option<&Policy>,
    now: Time,
    event: Event,
    room: Room,
    gate: Hold,
) -> Result<(Vec<Effect>, bool), Halt> {
    let before = held.work.tick();
    // A work that ended on a version not installed, or on none kept, takes what no flow decides, by its end alone.
    if let Some(end) = held.on.end() {
        let let_go = &held.flow.waits.paused.let_go;
        let effects = engine::handle_ended(end, let_go, &mut held.work, now, event).map_err(Halt::Refused)?;
        let grew = held.work.tick() != before;
        if grew {
            store.save(held.head.number, &held.work).map_err(Halt::Store)?;
            held.update(policy);
        }
        return Ok((effects, grew));
    }
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
            let mut kept = store.load(held.head.number).map_err(Halt::Store)?;
            kept.read_through(&held.work.readings);
            held.hold_at_fault(kept);
            return Ok((Vec::new(), true));
        }
        Err(refused) => return Err(Halt::Refused(refused)),
    };
    let grew = held.work.tick() != before;
    if grew {
        store.save(held.head.number, &held.work).map_err(Halt::Store)?;
        held.update(policy);
    }
    Ok((effects, grew))
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
        Some(outage) if outage.holds_now() => Gate::Agents,
        _ => Gate::Open,
    };
    let acts = match host {
        Some(outage) if outage.holds_now() => Gate::Acts,
        _ => Gate::Open,
    };
    agents.and(acts)
}

/// The works given the first turn to go first, each where its wait lets one round go and none has yet: the work whose
/// round met the failure first in the wait of the acts' host, then in the agents' wait.
fn first_turns(agents: Option<&Outage>, host: Option<&Outage>) -> Vec<WorkNumber> {
    [host, agents]
        .into_iter()
        .flatten()
        .filter(|outage| !outage.holds_now())
        .map(|outage| outage.first)
        .collect()
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

/// The rounds of the work that are history ([`history::samples`]), as the version it is on tells each round's line, and
/// each round's cell as it is on the flow installed: through the renumberings since, for a work on an earlier version,
/// and none, for a work on no version kept, whose rounds are history of the flow's alone.
fn samples_of(flow: &Compiled, on: &On, work: &Work) -> Vec<history::Sample> {
    match on {
        On::Installed { .. } => history::samples(flow, work),
        On::Earlier { outline, reading, .. } => {
            let line_of = |cell: &CellId| outline.cell(cell).and_then(|c| c.line.clone());
            let now = |cell: &CellId| {
                let now = reading.cell(cell);
                flow.flow.place(&now).map(|_| now)
            };
            history::samples_read(work, &line_of, &now)
        }
        On::NotKept { .. } => history::samples_read(work, &|_| None, &|_| None),
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

#[cfg(test)]
mod tests {
    //! What a tick of the driver, and a read of what it keeps of every work, costs as the works grow in number.

    use flow::{Has, Source, compile};
    use store::Store;

    use super::*;

    /// A writer that the owner's start runs, and the merge.
    fn flow() -> Compiled {
        let text = r#"
            name = "small"
            order = ["write", "merge"]
            [limits]
            runs = 2
            returns = 2
            calls = 3
            [lines.main]
            [blocks.text]
            [blocks.merged]
            shown = true
            [blocks.start]
            outside = "start"
            writes = { text = "The owner started the work." }
        "#;
        let write = "title = \"Write\"\nkind = \"agent\"\neffort = \"high\"\nline = \"main\"\ntask = \"Write.\"\n\
                     watches = [\"start\"]\noutputs = [\"text\"]\n[kinds]\nstart = \"last_and_latest\"\n";
        let merge =
            "title = \"Merge\"\nkind = \"engine\"\nact = \"merge\"\nwatches = [\"text\"]\noutputs = [\"merged\"]\n";
        let source = Source {
            flow: text.to_string(),
            cells: [("write", write), ("merge", merge)]
                .iter()
                .map(|(id, text)| (id.to_string(), text.to_string()))
                .collect(),
            files: [(
                flow::waits_file(),
                include_str!("../../../flow/agents/waits.toml").to_string(),
            )]
            .into(),
            ..Source::default()
        };
        let has = Has {
            sources: [START.to_string()].into(),
            acts: ["merge".to_string()].into(),
            ..Has::default()
        };
        compile(&source, &has).expect("the definition compiles")
    }

    /// How many times a tick works out a wait every work shares, with this many works let on, all but the first
    /// waiting for the one slot, as a tick before it left them.
    fn worked_out_in_a_tick(works: usize) -> usize {
        let dir = tempfile::tempdir().expect("a directory for the store");
        let store = Store::open(dir.path().join("store.db")).expect("the store opens");
        let mut now = Time(1_800_000_000_000);
        let (mut driver, _) =
            Driver::open(Box::new(store), vec![flow()], 1, None, None, now).expect("the driver opens");
        for i in 0..works {
            now.0 += 1_000;
            let new = NewItem {
                title: format!("Work {i}"),
                level: "1".to_string(),
                project: "small".to_string(),
                depends_on: Vec::new(),
                tags: Vec::new(),
                continues: None,
                flow: "small".to_string(),
                writes: Vec::new(),
                why: None,
                waits_for: Vec::new(),
            };
            let (_, work, _) = driver.add(now, new).expect("the item is added");
            (driver.happened(now, work, Event::Resumed)).expect("the work is let on");
        }
        now.0 += 1_000;
        driver.tick(now, None, None).expect("the tick is taken");
        now.0 += 1_000;
        outage::WORKED_OUT.with(|worked_out| worked_out.set(0));
        driver.tick(now, None, None).expect("the tick is taken");
        outage::WORKED_OUT.with(|worked_out| worked_out.get())
    }

    // The store's writes for thirty-three works, nothing run: a tick looks at every work, and works out the waits every
    // work shares as often with thirty works as with three. Fails where the step of each work works them out again.
    #[test]
    fn a_tick_works_out_the_shared_waits_as_often_for_many_works_as_for_few() {
        assert_eq!(worked_out_in_a_tick(30), worked_out_in_a_tick(3));
    }

    /// The round of the work given to be done.
    fn round_of(driver: &Driver, work: WorkNumber) -> decide::RunId {
        (driver.pending().into_iter())
            .find_map(|todo| match todo.effect {
                Effect::Round { run, .. } if todo.work == work => Some(run),
                _ => None,
            })
            .expect("a round of the work is given")
    }

    /// The round of the work under way hands in the block, a second after the last thing happened: the writer's, whose
    /// try ran on a model, or the merge's, an act's.
    fn hands_in(driver: &mut Driver, now: &mut Time, work: WorkNumber, block: &str) {
        now.0 += 1_000;
        let run = round_of(driver, work);
        if block == "text" {
            let on = engine::RanOn {
                level: "1".to_string(),
                model: "opus".to_string(),
                effort: None,
                directory: None,
                account: None,
            };
            let ran = Event::Ran { run, attempt: 1, on };
            (driver.happened(*now, work, ran)).expect("the try is taken");
        }
        let handed = engine::Handed {
            block: BlockId::from(block),
            content: format!("The {block}."),
            matters: true,
            value: None,
        };
        let returned = Event::Returned {
            run,
            came: engine::Came::hands_in(vec![handed]),
            spent: engine::Spent::default(),
        };
        (driver.happened(*now, work, returned)).expect("the return is taken");
    }

    /// What the work's CI read of its one test in a run of this source, asked for so.
    fn reading(work: WorkNumber, asking: &str, source: work::Source) -> Event {
        let reading = work::timed::Reading {
            asking: asking.to_string(),
            source,
            commit: "c0ffee".to_string(),
            workflow: "ci.yml".to_string(),
            jobs: Vec::new(),
            tests: vec![work::timed::Test {
                unit: "unit".to_string(),
                name: "a".to_string(),
                passed: true,
                millis: 100 + work.0,
            }],
        };
        Event::Timed { reading }
    }

    /// A work added and let on whose writer hands in its text, the CI's full run on it having read its one test.
    fn written(driver: &mut Driver, now: &mut Time) -> WorkNumber {
        now.0 += 1_000;
        let new = NewItem {
            title: "Work".to_string(),
            level: "1".to_string(),
            project: "small".to_string(),
            depends_on: Vec::new(),
            tags: Vec::new(),
            continues: None,
            flow: "small".to_string(),
            writes: Vec::new(),
            why: None,
            waits_for: Vec::new(),
        };
        let (_, work, _) = driver.add(*now, new).expect("the item is added");
        (driver.happened(*now, work, Event::Resumed)).expect("the work is let on");
        hands_in(driver, now, work, "text");
        let full = reading(work, &format!("run/{}", work.0), work::Source::Full);
        (driver.happened(*now, work, full)).expect("the reading is taken");
        work
    }

    /// How many times what a work adds to the judgments, and the merged checks of a project, were worked out since this
    /// was last asked.
    fn worked_out() -> (usize, usize) {
        let parts = judged::WORKED_OUT.with(|worked_out| worked_out.replace(0));
        let checks = CHECKS_WORKED_OUT.with(|worked_out| worked_out.replace(0));
        (parts, checks)
    }

    // A read of the judgments of every work, or of the tests' times of the merged checks, reads what the driver keeps of
    // each work and works out nothing of it again where nothing changed: what a work adds to the judgments is worked
    // out as it grows, that work's alone, and a project's merged checks once a work of it merged or a merged work
    // changed. Fails where a read works out each work's part again, or the checks of a project none of whose works
    // changed, as each read of the System view did with every work in the store.
    #[test]
    fn a_read_of_the_judgments_and_the_tests_times_works_out_nothing_again_where_nothing_changed() {
        let dir = tempfile::tempdir().expect("a directory for the store");
        let store = Store::open(dir.path().join("store.db")).expect("the store opens");
        let mut now = Time(1_800_000_000_000);
        let policy = Policy {
            samples: 2,
            model: Box::new(|_, _| Some("opus".to_string())),
            changed: None,
            warm: None,
        };
        let (mut driver, _) =
            Driver::open(Box::new(store), vec![flow()], 10, Some(policy), None, now).expect("the driver opens");
        let works: Vec<WorkNumber> = (0..3).map(|_| written(&mut driver, &mut now)).collect();
        for work in &works {
            hands_in(&mut driver, &mut now, *work, "merged");
        }
        let going = written(&mut driver, &mut now);
        worked_out();
        let (judged, tested) = (driver.judged(None, now), driver.tested());
        assert_eq!(worked_out().0, 0, "no work's part is worked out by a read");
        assert_eq!(tested.len(), 1);
        assert_eq!(tested[0].checks, 3, "the three merged checks: {tested:?}");
        assert_eq!((driver.judged(None, now), driver.tested()), (judged, tested.clone()));
        assert_eq!(worked_out(), (0, 0), "a read again works out nothing");
        // The going work grows: its part alone is worked out as it does, and the checks of its project, none of whose
        // merged works changed, are not worked out again.
        now.0 += 1_000;
        let revisions = |driver: &Driver| works.iter().map(|work| driver.revision(*work)).collect::<Vec<_>>();
        let (merged, was) = (revisions(&driver), driver.revision(going));
        let fast = reading(going, "call/1", work::Source::Fast);
        (driver.happened(now, going, fast)).expect("the reading is taken");
        assert!(worked_out().0 > 0, "the work that grew has its part worked out");
        assert_ne!(driver.revision(going), was, "the work that grew has a new revision");
        assert_eq!(
            revisions(&driver),
            merged,
            "the works that did not grow are held as they were"
        );
        driver.judged(None, now);
        assert_eq!(driver.tested(), tested);
        assert_eq!(
            worked_out(),
            (0, 0),
            "a read after one work grew works out nothing of the others"
        );
        // It merges: the next read of the tests' times works out its project's checks once, with its own.
        hands_in(&mut driver, &mut now, going, "merged");
        worked_out();
        assert_eq!(driver.tested()[0].checks, 4);
        driver.tested();
        assert_eq!(
            worked_out(),
            (0, 1),
            "the project's checks are worked out once after a work of it merged"
        );
    }
}
