//! The walk: what a work does next, from the flow and its records (design document `data-and-context`, 5 to 12).
//!
//! A work's state is its data, each with versions. A cell names the blocks that run it; one of them is new to the cell
//! while it has a version, changing its meaning, later than the one the cell consumed, that the cell did not write
//! itself. A run consumes, as its return is taken, what the return says it dealt with, up to the version its round was
//! told of; a round that fails, a return sent back and a run stopped consume nothing, and nothing else does. The engine
//! keeps no place where a work is. Each time a round ends or something else happens to the work, it walks the cells
//! from the first, in their order: a cell whose condition is not met is skipped, what it wrote before still counting;
//! at the first that has a run at rest the run goes on, unless it waits for something; a cell whose last runs asked for
//! calls waits there until every result is in, and is then entered again, given them; at the first whose preconditions
//! do not hold or that has reached a limit the work waits; the first that something new runs is entered; the rest are
//! passed. A work in which nothing new runs a cell, and nothing is under way or waited for, rests until someone writes
//! what runs one. One round of a work is under way at a time: a round of a cell after one the owner turned to, by words
//! or by an answer, is stopped, and its cell runs again later. A work has ended when its last cell has a record or its
//! item is cancelled.
//!
//! A pause of the work changes none of this order. It holds back the run the walk comes to, unless one of that run's
//! reasons is a note a person wrote, or the owner turning to the cell, since the cell last ran (since the run of its
//! record started, or with no record its first run): a paused work holds everything it would do by itself, still
//! answers the owner, and lets nothing else a person writes pass. Such a run has its rounds while the work is paused
//! ([`hold`]), and once it has finished what it wrote waits, as anything else does, for the work to be let on.
//!
//! The owner's pause of everything holds back what the work's own pause does, and the walk reads it, as it reads whether
//! a slot is free, from what the scheduler gives it ([`Room`]): where no slot is free, a cell that takes one waits for
//! it before it runs, a run that answers a note as much as any other.

use std::collections::{BTreeMap, BTreeSet};

use crate::flow::{BlockId, Cell, CellId, Condition, Flow, TestKind, TestName};
use crate::records::{
    CallId, Cause, Finished, Outsider, Pause, Pauser, PutIn, RecordFault, Records, RunId, Seq, Started, Who,
};
use crate::room::Room;
use crate::run::{Hold, Standing, Told};

/// What a work does next, with the reason as data (principle decide-purely).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decision {
    /// The records cannot be those of a work: nothing is decided on them.
    Fault(RecordFault),
    /// The work has ended.
    Ended(End),
    /// A round of this run is under way: nothing is decided until it ends, unless the work is cancelled.
    Busy { cell: CellId, run: RunId },
    /// The work is paused, and what it would do next is run a cell none of whose reasons is a change of a note a person
    /// made: that run waits for the work to be let on. A cell one of whose reasons is such a change runs as if the work
    /// were not paused, and what begins no run is decided so too: a run at rest goes on, rests or is abandoned.
    Paused,
    /// The cell runs: why, and the version of each block it reads or may write that the run starts from. The engine
    /// records the run as started with exactly these.
    Run {
        cell: CellId,
        because: Vec<Cause>,
        basis: BTreeMap<BlockId, Option<Seq>>,
    },
    /// The run, which is at rest and waits for nothing, goes on: what its last round returned is taken up, and it
    /// has its next round, which the pause of its work may hold back ([`hold`]). A run whose last round failed is at
    /// rest too, and its next round is that round again.
    GoOn { cell: CellId, run: RunId },
    /// The run is abandoned, consuming nothing, and the walk is taken again: it is at rest and its cell no longer
    /// applies, or its round under way is stopped for a cell before it that the owner turned to, or its work was
    /// cancelled, when it may have a round under way, which is stopped.
    Abandon {
        cell: CellId,
        run: RunId,
        because: Abandoning,
    },
    /// The work waits at this cell, for each of these.
    Wait { at: CellId, on: Vec<Waited> },
    /// Nothing new runs a cell the flow lets run, nothing of the work is under way or waited for, and it has not ended:
    /// it goes on once someone writes what runs a cell.
    Idle,
}

/// How a work ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    /// Its last cell, the merge, has a record.
    Merged,
    /// Its item was cancelled.
    Cancelled,
}

/// Why a run is abandoned.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Abandoning {
    /// Its work was cancelled.
    Cancelled,
    /// Its cell is skipped now, or is no longer in the flow.
    NoLongerApplies,
    /// The owner turned to this cell before it, by a new version of this block, and the cell is to run first: the
    /// round under way is stopped, and its cell runs again later (design document `data-and-context`, 41, 33).
    Stopped { cell: CellId, block: BlockId },
}

/// What a work waits for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Waited {
    /// The result of a call the cell's run rests at: from this person, or from the engine where `on` is `None`.
    Call { call: CallId, on: Option<Who> },
    /// A precondition of the cell, a named test of the flow, does not hold: each of these writes a block that keeps it
    /// from holding, and none other can make it hold.
    Precondition { test: TestName, by: Vec<Maker> },
    /// The cell's last runs, this many in a row, were each for what a cell after it wrote, and so would the next be:
    /// the owner lets it go on or cancels the work.
    Limit { runs: u32 },
    /// The cell's last runs, this many in a row, each asked for calls and dealt with nothing, and it would be entered
    /// again for their results: the owner lets it go on or cancels the work.
    Calls { runs: u32 },
    /// A free slot: the cell takes one, and as many rounds that take one are under way as the engine has slots. The
    /// engine gives it the first that comes free, in its order.
    Slot,
}

/// Who can make a precondition hold, by writing a block its test reads: the flow says who writes each block.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Maker {
    /// The owner, as people write the block.
    Owner,
    /// This source outside the work, such as the items the work depends on as they merge.
    Outside { source: String },
    /// The cell, by what its next run writes.
    Cell { cell: CellId },
}

/// What the work does next, were nothing outside its records to hold it back ([`Room::FREE`]).
pub fn decide(flow: &Flow, records: &Records) -> Decision {
    decide_in(flow, records, Room::FREE)
}

/// What the work does next, in the room the engine leaves it: under the owner's pause of everything it begins what it
/// would begin were it paused itself, and where no slot is free a cell that takes one waits for it.
pub fn decide_in(flow: &Flow, records: &Records, room: Room) -> Decision {
    if let Err(fault) = check(records) {
        return Decision::Fault(fault);
    }
    let walk = Walk::new(flow, records);
    let cells = flow.cells();
    if walk.record(&cells[cells.len() - 1].id).is_some() {
        return Decision::Ended(End::Merged);
    }
    if records.cancelled.is_some() {
        // Nothing of a cancelled work goes on: each run that is still open is abandoned, whatever it is doing.
        return match walk.runs.values().find(|r| r.open()) {
            Some(run) => Decision::Abandon {
                cell: run.started.cell.clone(),
                run: run.started.run,
                because: Abandoning::Cancelled,
            },
            None => Decision::Ended(End::Cancelled),
        };
    }
    // One round of a work at a time: a round under way is stopped only for a cell before it that the owner turned to.
    if let Some(run) = walk.runs.values().find(|r| r.under_way()) {
        if let Some((cell, block)) = walk.stopping(run, room) {
            return Decision::Abandon {
                cell: run.started.cell.clone(),
                run: run.started.run,
                because: Abandoning::Stopped { cell, block },
            };
        }
        return Decision::Busy {
            cell: run.started.cell.clone(),
            run: run.started.run,
        };
    }
    // A run at rest of a cell that no longer applies is abandoned before anything else: its next round, or the answer
    // to its call, would be for nothing.
    for run in walk.runs.values().filter(|r| r.at_rest()) {
        if (walk.flow.place(&run.started.cell)).is_none_or(|i| !walk.applies(i)) {
            return Decision::Abandon {
                cell: run.started.cell.clone(),
                run: run.started.run,
                because: Abandoning::NoLongerApplies,
            };
        }
    }
    for (i, cell) in cells.iter().enumerate() {
        if !walk.applies(i) {
            continue;
        }
        let wait = |on| Decision::Wait {
            at: cell.id.clone(),
            on,
        };
        if let Some(run) = walk.runs.values().find(|r| r.at_rest() && r.started.cell == cell.id) {
            let mut on = walk.open_calls(run);
            on.extend(walk.unmet(i));
            if !on.is_empty() {
                return wait(on);
            }
            return Decision::GoOn {
                cell: cell.id.clone(),
                run: run.started.run,
            };
        }
        let (because, mut on) = walk.entering(i);
        on.extend(walk.unmet(i));
        if because.is_empty() {
            if on.is_empty() {
                continue;
            }
            return wait(on);
        }
        on.extend(walk.at_limit(cell, &because));
        if !on.is_empty() {
            return wait(on);
        }
        // A pause holds back the run, unless it is for a note a person wrote or the owner turning to the cell:
        // everything before this is decided whether or not the work is paused.
        if (paused(records) || room.paused) && !walk.for_a_note(i, &because, None) {
            return Decision::Paused;
        }
        if cell.slot && !room.slot {
            return wait(vec![Waited::Slot]);
        }
        return Decision::Run {
            cell: cell.id.clone(),
            because,
            basis: walk.basis(cell),
        };
    }
    Decision::Idle
}

/// What the round of the run was told, and how the blocks its cell may write stand now: what its return may name as
/// dealt with, and what it would write over ([`Told`]). `None` where the run never started.
pub fn told(flow: &Flow, records: &Records, run: RunId) -> Option<Told> {
    let walk = Walk::new(flow, records);
    let r = walk.runs.get(&run)?;
    let cell = flow.place(&r.started.cell).map(|i| &flow.cells()[i]);
    let mut ran_by: Vec<BlockId> = Vec::new();
    for cause in &r.started.because {
        match cause {
            Cause::Changed { block, .. } => ran_by.push(block.clone()),
            // A run started before the walk chose by what is new was run by every block it began from that ran its
            // cell.
            Cause::NoRecord => ran_by.extend(
                (r.started.basis.keys())
                    .filter(|b| cell.is_some_and(|c| c.runs_on.contains(b)))
                    .cloned(),
            ),
            Cause::Results { .. } => {}
        }
    }
    for p in records.put_in.iter().filter(|p| p.run == run) {
        if !ran_by.contains(&p.block) {
            ran_by.push(p.block.clone());
        }
    }
    let outputs = (cell.into_iter().flat_map(|c| &c.outputs))
        .map(|block| {
            let latest = walk.current(block);
            let standing = Standing {
                told: walk.told(r, block),
                latest: latest.map(|v| v.at),
                value: latest.and_then(|v| v.value).map(str::to_string),
            };
            (block.clone(), standing)
        })
        .collect();
    Some(Told { ran_by, outputs })
}

/// What the pause of the work holds back of the run's rounds: nothing where the work is not paused; where it is, of a
/// run one of whose reasons was a change of a note a person made, only the round after one that failed for which the
/// engine paused the work, so that the owner sees why first, and nothing where the engine paused it for none of the
/// run's rounds since its last began; and every round of any other run.
pub fn hold(flow: &Flow, records: &Records, run: RunId) -> Hold {
    hold_in(flow, records, run, Room::FREE)
}

/// What holds back the run's rounds in the room the engine leaves the work: every round, where its cell takes a slot
/// and none is free; otherwise what the pause of the work would hold back ([`hold`]), where the work is paused or the
/// owner's pause of everything holds.
pub fn hold_in(flow: &Flow, records: &Records, run: RunId, room: Room) -> Hold {
    let walk = Walk::new(flow, records);
    let Some(r) = walk.runs.get(&run) else {
        return Hold::Rounds;
    };
    if !room.slot && (flow.place(&r.started.cell)).is_none_or(|i| flow.cells()[i].slot) {
        return Hold::Rounds;
    }
    if !paused(records) && !room.paused {
        return Hold::Nothing;
    }
    match flow.place(&r.started.cell) {
        Some(i) if walk.for_a_note(i, &r.started.because, Some(r.started.at)) => {
            if paused_by_its_failure(records, run) {
                Hold::Retry
            } else {
                Hold::Nothing
            }
        }
        _ => Hold::Rounds,
    }
}

/// Whether the engine paused the work since the run's last round, which has ended, began, and the work was not let on
/// since: as it does where the round's failure pauses it, or where the round took the work over its budget, which its
/// carrier says before the round's end.
fn paused_by_its_failure(records: &Records, run: RunId) -> bool {
    let began = (records.rounds.iter()).filter(|r| r.run == run).map(|r| r.at).max();
    let ended = records.returned.iter().filter(|r| r.run == run).count();
    let begun = records.rounds.iter().filter(|r| r.run == run).count();
    let let_on = (records.pauses.iter()).filter(|p| !p.paused).map(|p| p.at).max();
    let Some(began) = began.filter(|_| ended == begun) else {
        return false;
    };
    let after = let_on.map_or(began, |at| at.max(began));
    (records.pauses.iter()).any(|p| p.paused && p.by == Pauser::Engine && p.at > after)
}

/// Whether the work is paused: the latest of its pauses and lettings on is a pause.
pub fn paused(records: &Records) -> bool {
    pause(records).is_some()
}

/// The pause that holds the work, with who paused it and why: the latest of its pauses and lettings on, where it is a
/// pause.
pub fn pause(records: &Records) -> Option<&Pause> {
    (records.pauses.iter()).max_by_key(|p| p.at).filter(|p| p.paused)
}

/// Whether the records can be those of a work.
fn check(records: &Records) -> Result<(), RecordFault> {
    let mut runs = BTreeSet::new();
    for s in &records.started {
        if !runs.insert(s.run) {
            return Err(RecordFault::RunTwice(s.run));
        }
    }
    let named = (records.rounds.iter().map(|r| r.run))
        .chain(records.returned.iter().map(|r| r.run))
        .chain(records.called.iter().map(|c| c.run))
        .chain(records.finished.iter().map(|f| f.run))
        .chain(records.abandoned.iter().map(|a| a.run))
        .chain(records.put_in.iter().map(|d| d.run));
    for run in named {
        if !runs.contains(&run) {
            return Err(RecordFault::UnknownRun(run));
        }
    }
    let mut finished = BTreeSet::new();
    for f in &records.finished {
        if !finished.insert(f.run) {
            return Err(RecordFault::FinishedTwice(f.run));
        }
        let mut blocks = BTreeSet::new();
        if !f.writes.iter().all(|w| blocks.insert(&w.block)) {
            return Err(RecordFault::WroteTwice(f.run));
        }
    }
    if let Some(a) = records.abandoned.iter().find(|a| finished.contains(&a.run)) {
        return Err(RecordFault::FinishedAndAbandoned(a.run));
    }
    for run in &runs {
        let began = records.rounds.iter().filter(|r| r.run == *run).count();
        let ended = records.returned.iter().filter(|r| r.run == *run).count();
        if ended > began {
            return Err(RecordFault::ReturnedWithoutRound(*run));
        }
    }
    let mut done = BTreeSet::new();
    for d in &records.done {
        if !records.called.iter().any(|c| c.call == d.call) {
            return Err(RecordFault::UnknownCall(d.call));
        }
        if !done.insert(d.call) {
            return Err(RecordFault::DoneTwice(d.call));
        }
    }
    let places = (records.started.iter().map(|e| e.at))
        .chain(records.rounds.iter().map(|e| e.at))
        .chain(records.returned.iter().map(|e| e.at))
        .chain(records.called.iter().map(|e| e.at))
        .chain(records.done.iter().map(|e| e.at))
        .chain(records.finished.iter().map(|e| e.at))
        .chain(records.abandoned.iter().map(|e| e.at))
        .chain(records.put_in.iter().map(|e| e.at))
        .chain(records.written.iter().map(|e| e.at))
        .chain(records.let_go.iter().map(|e| e.at))
        .chain(records.pauses.iter().map(|e| e.at))
        .chain(records.cancelled)
        .chain(records.released);
    let mut seen = BTreeSet::new();
    for at in places {
        if !seen.insert(at) {
            return Err(RecordFault::PlaceTwice(at));
        }
    }
    for p in &records.put_in {
        put_in_fault(records, p)?;
    }
    for c in &records.called {
        let ended = (records.finished.iter().filter(|f| f.run == c.run).map(|f| f.at))
            .chain(records.abandoned.iter().filter(|a| a.run == c.run).map(|a| a.at))
            .any(|at| at < c.at);
        if ended {
            return Err(RecordFault::CalledAfterEnd(c.call));
        }
    }
    for f in &records.finished {
        let Some(dealt_with) = &f.dealt_with else {
            continue;
        };
        let basis = (records.started.iter().find(|s| s.run == f.run)).map(|s| &s.basis);
        let mut named = BTreeSet::new();
        for block in dealt_with {
            if !named.insert(block) {
                return Err(RecordFault::DealtWithTwice(f.run));
            }
            let began_with = basis.is_some_and(|b| b.contains_key(block));
            let put_in = (records.put_in.iter()).any(|p| p.run == f.run && &p.block == block);
            if !began_with && !put_in {
                return Err(RecordFault::DealtWithUnknown(f.run));
            }
        }
    }
    Ok(())
}

/// What is wrong with a version put into a round: it is no version of its block, it was written at the place it was
/// put in or after, or its run had no round under way there, between a round's beginning and its end, and before the
/// run finished or was abandoned.
fn put_in_fault(records: &Records, p: &PutIn) -> Result<(), RecordFault> {
    if !records.wrote(&p.block, p.version) {
        return Err(RecordFault::PutInNoVersion(p.at));
    }
    if p.version >= p.at {
        return Err(RecordFault::PutInBeforeWritten(p.at));
    }
    let mut began: Vec<Seq> = (records.rounds.iter())
        .filter(|r| r.run == p.run)
        .map(|r| r.at)
        .collect();
    let mut ended: Vec<Seq> = (records.returned.iter())
        .filter(|r| r.run == p.run)
        .map(|r| r.at)
        .collect();
    began.sort();
    ended.sort();
    let in_round = (began.iter().enumerate()).any(|(i, b)| *b < p.at && ended.get(i).is_none_or(|e| *e > p.at));
    let gone = (records.finished.iter().filter(|f| f.run == p.run).map(|f| f.at))
        .chain(records.abandoned.iter().filter(|a| a.run == p.run).map(|a| a.at))
        .any(|at| at <= p.at);
    if !in_round || gone {
        return Err(RecordFault::PutInNoRound(p.at));
    }
    Ok(())
}

/// A run, with what the records say became of it.
pub(crate) struct RunView<'a> {
    pub(crate) started: &'a Started,
    finished: Option<&'a Finished>,
    abandoned: bool,
    /// How many of its rounds began, and how many ended.
    pub(crate) began: usize,
    pub(crate) ended: usize,
}

impl RunView<'_> {
    /// Whether the run has neither finished nor been abandoned.
    pub(crate) fn open(&self) -> bool {
        self.finished.is_none() && !self.abandoned
    }

    /// Whether a round of the run is under way.
    pub(crate) fn under_way(&self) -> bool {
        self.open() && self.began > self.ended
    }

    /// Whether the run is at rest: open, with no round under way.
    fn at_rest(&self) -> bool {
        self.open() && self.began == self.ended
    }
}

/// A version of a block: the write that made it.
pub(crate) struct Version<'a> {
    /// The place of the entry that wrote it, which names it.
    pub(crate) at: Seq,
    /// The run that wrote it; `None` for a person or the outside.
    pub(crate) by: Option<RunId>,
    /// Which of a person and the outside wrote it, where no run did.
    pub(crate) outsider: Option<Outsider>,
    /// The version at which the meaning it has last changed.
    pub(crate) meaning: Seq,
    /// The block's value, where it has one.
    pub(crate) value: Option<&'a str>,
}

/// A write of a block, by a run as it finished or by a person or the outside.
struct Written<'a> {
    block: &'a BlockId,
    at: Seq,
    by: Option<RunId>,
    outsider: Option<Outsider>,
    matters: bool,
    value: Option<&'a str>,
}

/// One walk of a work: the flow, the records, and what is worked out of them once.
pub(crate) struct Walk<'a> {
    flow: &'a Flow,
    records: &'a Records,
    pub(crate) runs: BTreeMap<RunId, RunView<'a>>,
    /// The versions of each block, in the order they were written.
    pub(crate) versions: BTreeMap<&'a BlockId, Vec<Version<'a>>>,
}

impl<'a> Walk<'a> {
    pub(crate) fn new(flow: &'a Flow, records: &'a Records) -> Self {
        let runs = read_runs(records);
        let versions = read_versions(records, &runs);
        Self {
            flow,
            records,
            runs,
            versions,
        }
    }

    /// Whether the condition of the cell at this place is met: with none it always is.
    pub(crate) fn applies(&self, i: usize) -> bool {
        match &self.flow.cells()[i].applies {
            None => true,
            Some(Condition::Test(name)) => self.holds(name, i),
            Some(Condition::Is { block, value }) => self.current(block).and_then(|v| v.value) == Some(value.as_str()),
        }
    }

    /// Whether the named test holds, for the cell at this place: when no block it reads keeps it from holding.
    fn holds(&self, name: &TestName, i: usize) -> bool {
        self.holding(name, i).is_empty()
    }

    /// The blocks the named test reads that keep it from holding, for the cell at this place, by the value each has
    /// now: a write of any of them can change whether it holds. The flow has every test its cells name.
    fn holding(&self, name: &TestName, i: usize) -> Vec<&'a BlockId> {
        let test = &self.flow.tests()[name];
        let has = |block: &BlockId| {
            self.current(block).is_some_and(|v| {
                v.value == Some(test.is.as_str()) && (test.kind != TestKind::NoneStanding || self.stands(v, i))
            })
        };
        let reads = test.reads.iter();
        match test.kind {
            TestKind::All => reads.filter(|b| !has(b)).collect(),
            TestKind::Any if test.reads.iter().any(has) => Vec::new(),
            TestKind::Any => reads.collect(),
            TestKind::None | TestKind::NoneStanding => reads.filter(|b| has(b)).collect(),
        }
    }

    /// Whether the version stands, for the cell at this place: unless a run of a cell that is skipped now wrote it.
    /// What a person or the outside wrote always stands. A cell's test reads only blocks that cells before it write,
    /// so the writer stands before it, and whether it is skipped is decided first; a version written by a cell that
    /// does not stand before it (one taken out of the flow, or moved after it, as only a changed flow gives) stands.
    fn stands(&self, v: &Version<'a>, i: usize) -> bool {
        let writer = (v.by)
            .and_then(|run| self.runs.get(&run))
            .and_then(|r| self.flow.place(&r.started.cell));
        match writer {
            Some(j) if j < i => self.applies(j),
            _ => true,
        }
    }

    /// The cell's record: its latest finished run.
    pub(crate) fn record(&self, cell: &CellId) -> Option<&RunView<'a>> {
        (self.runs.values())
            .filter(|r| &r.started.cell == cell)
            .filter_map(|r| r.finished.map(|f| (f.at, r)))
            .max_by_key(|(at, _)| *at)
            .map(|(_, r)| r)
    }

    fn versions(&self, block: &BlockId) -> &[Version<'a>] {
        self.versions.get(block).map_or(&[], Vec::as_slice)
    }

    /// The current version of the block: the latest written.
    pub(crate) fn current(&self, block: &BlockId) -> Option<&Version<'a>> {
        self.versions(block).last()
    }

    /// The latest version of the block the run's round was told of: the one it started from, or one put in to it
    /// since.
    pub(crate) fn told(&self, run: &RunView<'a>, block: &BlockId) -> Option<Seq> {
        let began = run.started.basis.get(block).copied().flatten();
        let put_in = (self.records.put_in.iter())
            .filter(|p| p.run == run.started.run && &p.block == block)
            .map(|p| p.version);
        began.into_iter().chain(put_in).max()
    }

    /// Whether the finished run dealt with the block: its return named it, or, of a run whose return did not say, the
    /// run began from it.
    fn dealt_with(&self, run: &RunView<'a>, block: &BlockId) -> bool {
        match run.finished.and_then(|f| f.dealt_with.as_ref()) {
            Some(named) => named.contains(block),
            None => run.finished.is_some() && run.started.basis.contains_key(block),
        }
    }

    /// The version of the block the cell consumed: of its runs that finished dealing with the block, the latest
    /// version one of them was told of; `None` where none did (design document `data-and-context`, 9).
    pub(crate) fn consumed(&self, cell: &CellId, block: &BlockId) -> Option<Seq> {
        (self.runs.values())
            .filter(|r| &r.started.cell == cell && self.dealt_with(r, block))
            .filter_map(|r| self.told(r, block))
            .max()
    }

    /// The versions of the block that are new to the cell: later than the one it consumed, changing the block's
    /// meaning, and not written by a run of the cell itself (design document `data-and-context`, 6 and 10).
    fn new_to(&self, cell: &CellId, block: &BlockId) -> (Option<Seq>, Vec<&Version<'a>>) {
        let was = self.consumed(cell, block);
        let own = |v: &Version<'a>| (v.by.and_then(|run| self.runs.get(&run))).is_some_and(|r| &r.started.cell == cell);
        let new = (self.versions(block).iter())
            .filter(|v| Some(v.at) > was && v.meaning == v.at && !own(v))
            .collect();
        (was, new)
    }

    /// The version the cell's run starts from, of each block that runs it or that it may write: the latest.
    fn basis(&self, cell: &Cell) -> BTreeMap<BlockId, Option<Seq>> {
        (cell.runs_on.iter().chain(&cell.outputs))
            .map(|b| (b.clone(), self.current(b).map(|v| v.at)))
            .collect()
    }

    /// The calls the run rests at that have no result yet.
    fn open_calls(&self, run: &RunView<'a>) -> Vec<Waited> {
        let records = self.records;
        let mut open: Vec<_> = (records.called.iter())
            .filter(|c| c.run == run.started.run && !records.done.iter().any(|d| d.call == c.call))
            .collect();
        // In the order the calls were made, however the records list them.
        open.sort_by_key(|c| c.at);
        (open.into_iter())
            .map(|c| Waited::Call {
                call: c.call,
                on: c.on.clone(),
            })
            .collect()
    }

    /// The preconditions of the cell at this place that do not hold, each with who writes a block that keeps it from
    /// holding.
    fn unmet(&self, i: usize) -> Vec<Waited> {
        (self.flow.cells()[i].requires.iter())
            .filter_map(|test| {
                let holding = self.holding(test, i);
                (!holding.is_empty()).then(|| Waited::Precondition {
                    test: test.clone(),
                    by: self.makers(&holding),
                })
            })
            .collect()
    }

    /// Who writes any of these blocks, each once, in the order of [`Maker`]: the owner where people write one, the
    /// source outside the work where one does, and each cell that has one among its outputs.
    fn makers(&self, blocks: &[&BlockId]) -> Vec<Maker> {
        let mut makers = BTreeSet::new();
        for block in blocks {
            if let Some(outsiders) = self.flow.outsiders(block) {
                if outsiders.people {
                    makers.insert(Maker::Owner);
                }
                if let Some(source) = &outsiders.outside {
                    makers.insert(Maker::Outside { source: source.clone() });
                }
            }
            for cell in self.flow.writers(block) {
                makers.insert(Maker::Cell { cell: cell.id.clone() });
            }
        }
        makers.into_iter().collect()
    }

    /// What is new to the cell at this place: each block that runs it and has a version new to it, in the order the
    /// cell names them; empty where nothing is.
    pub(crate) fn needs(&self, i: usize) -> Vec<Cause> {
        let cell = &self.flow.cells()[i];
        (cell.runs_on.iter())
            .filter_map(|block| {
                let (was, new) = self.new_to(&cell.id, block);
                let behind = new.iter().all(|v| self.later(v, i));
                (!new.is_empty()).then(|| Cause::Changed {
                    block: block.clone(),
                    was,
                    behind,
                })
            })
            .collect()
    }

    /// Whether the version was written by a run of a cell after the one at this place.
    fn later(&self, v: &Version<'a>, i: usize) -> bool {
        (v.by.and_then(|run| self.runs.get(&run)))
            .and_then(|r| self.flow.place(&r.started.cell))
            .is_some_and(|j| j > i)
    }

    /// Why the cell at this place is entered, and what it waits for instead: what is new to it, and the results of the
    /// calls its runs asked for once all are in. While one of them has no result, the cell waits for it, and only the
    /// owner turning to it enters it meanwhile, its calls still waited for (design document `data-and-context`, 28,
    /// 40).
    fn entering(&self, i: usize) -> (Vec<Cause>, Vec<Waited>) {
        let mut because = self.needs(i);
        let (calls, open) = self.owed(i);
        if !open.is_empty() {
            because.retain(|cause| matches!(cause, Cause::Changed { block, .. } if self.flow.interrupts(block)));
            let on = if because.is_empty() { open } else { Vec::new() };
            return (because, on);
        }
        if !calls.is_empty() {
            because.push(Cause::Results { calls });
        }
        (because, Vec::new())
    }

    /// The calls the cell at this place is owed the results of, in the order they were made, and those of them that
    /// have no result yet. A call is owed where a run of the cell asked for it as its return was taken, after its last
    /// round ended and before it finished, and no run of the cell that finished since was given it.
    fn owed(&self, i: usize) -> (Vec<CallId>, Vec<Waited>) {
        let cell = &self.flow.cells()[i];
        let records = self.records;
        let given: BTreeSet<CallId> = (self.runs.values())
            .filter(|r| r.started.cell == cell.id && r.finished.is_some())
            .flat_map(|r| &r.started.because)
            .flat_map(|cause| match cause {
                Cause::Results { calls } => calls.as_slice(),
                _ => &[],
            })
            .copied()
            .collect();
        let mut owed: Vec<_> = (records.called.iter())
            .filter(|c| !given.contains(&c.call))
            .filter(|c| {
                self.runs.get(&c.run).is_some_and(|r| {
                    let ended = (records.returned.iter()).filter(|e| e.run == c.run).map(|e| e.at).max();
                    r.started.cell == cell.id && r.finished.is_some() && ended.is_none_or(|e| c.at > e)
                })
            })
            .collect();
        owed.sort_by_key(|c| c.at);
        let open = (owed.iter())
            .filter(|c| !records.done.iter().any(|d| d.call == c.call))
            .map(|c| Waited::Call {
                call: c.call,
                on: c.on.clone(),
            })
            .collect();
        (owed.into_iter().map(|c| c.call).collect(), open)
    }

    /// The cell before the run's that the owner turned to and that is to run, with the block by which the owner turned
    /// to it, where there is one: the round under way of the run is then stopped. Of the cells before the run's that
    /// apply, the first to which something that [interrupts](Flow::interrupts) is new, whose preconditions hold, that
    /// is at no limit, that the pause lets run, and that has no run at rest of its own.
    fn stopping(&self, run: &RunView<'a>, room: Room) -> Option<(CellId, BlockId)> {
        let place = self.flow.place(&run.started.cell)?;
        let paused = paused(self.records) || room.paused;
        (0..place).filter(|&j| self.applies(j)).find_map(|j| {
            let cell = &self.flow.cells()[j];
            if self.runs.values().any(|r| r.open() && r.started.cell == cell.id) {
                return None;
            }
            let (because, _) = self.entering(j);
            let block = because.iter().find_map(|cause| match cause {
                Cause::Changed { block, .. } if self.flow.interrupts(block) => Some(block.clone()),
                _ => None,
            })?;
            let held = !self.unmet(j).is_empty()
                || self.at_limit(cell, &because).is_some()
                || (paused && !self.for_a_note(j, &because, None));
            (!held).then(|| (cell.id.clone(), block))
        })
    }

    /// Whether one of these reasons of a run of the cell at this place, which starts at `before` (now where `None`), is
    /// the owner's since the cell last ran: a note a person wrote to the cell, or the owner turning to it, written
    /// after the start of the run of its record, or with no record after the start of its first run, after the
    /// version the cell consumed, and before `before`. A cell with no record and one with a record so both count it
    /// until a run of it finishes, whatever runs were abandoned since. A cell no run of which has started has not run,
    /// and answers no one: a work added paused begins nothing.
    fn for_a_note(&self, i: usize, because: &[Cause], before: Option<Seq>) -> bool {
        let cell = &self.flow.cells()[i];
        let earlier = |at: Seq| before.is_none_or(|before| at < before);
        let of_cell = || (self.runs.values()).filter(|r| r.started.cell == cell.id && earlier(r.started.at));
        let record = (of_cell())
            .filter_map(|r| r.finished.filter(|f| earlier(f.at)).map(|f| (f.at, r.started.at)))
            .max();
        let Some(since) = record
            .map(|(_, at)| at)
            .or_else(|| of_cell().map(|r| r.started.at).min())
        else {
            return false;
        };
        let owners = |block: &BlockId, was: Option<Seq>| {
            (self.versions(block).iter()).any(|v| {
                let theirs = (self.flow.is_note(block) && v.outsider == Some(Outsider::Person))
                    || (self.flow.interrupts(block) && v.by.is_none());
                theirs && v.meaning == v.at && v.at > since && Some(v.at) > was && earlier(v.at)
            })
        };
        because.iter().any(|cause| match cause {
            Cause::NoRecord => cell.runs_on.iter().any(|block| owners(block, None)),
            Cause::Changed { block, was, .. } => owners(block, *was),
            Cause::Results { .. } => false,
        })
    }

    /// The wait for the owner where the run these causes call for would be one too many: a way round, after as many of
    /// the cell's runs in a row were ways round as its limit allows (plan, D2); or an entering for the results of its
    /// calls, after as many of its runs in a row each asked for calls and dealt with nothing as its limit of calls
    /// allows (plan, D1). Both count since the owner last let the cell go on.
    fn at_limit(&self, cell: &Cell, because: &[Cause]) -> Option<Waited> {
        let let_go = (self.records.let_go.iter())
            .filter(|l| l.cell == cell.id)
            .map(|l| l.at)
            .max();
        let mut finished: Vec<(&Finished, &RunView<'a>)> = (self.runs.values())
            .filter(|r| r.started.cell == cell.id && let_go.is_none_or(|at| r.started.at > at))
            .filter_map(|r| r.finished.map(|f| (f, r)))
            .collect();
        finished.sort_by_key(|(f, _)| f.at);
        if way_round(because) {
            // A run is a way round when what was new to it was all written by cells after it; one with a reason of
            // someone else's ends the row, and one that dealt with nothing, or was entered for its results alone, is
            // passed over.
            let runs = (finished.iter().rev())
                .take_while(|(_, r)| !r.started.because.iter().any(Cause::forward))
                .filter(|(f, r)| way_round(&r.started.because) && f.dealt_with.as_ref().is_none_or(|d| !d.is_empty()))
                .count();
            if runs >= cell.limit as usize {
                return Some(Waited::Limit { runs: runs as u32 });
            }
        }
        if because.iter().any(|c| matches!(c, Cause::Results { .. })) {
            let asked = |r: &RunView<'a>, f: &Finished| {
                let ended = (self.records.returned.iter())
                    .filter(|e| e.run == r.started.run)
                    .map(|e| e.at)
                    .max();
                (self.records.called.iter())
                    .any(|c| c.run == r.started.run && ended.is_none_or(|e| c.at > e) && c.at < f.at)
            };
            let runs = (finished.iter().rev())
                .take_while(|(f, r)| f.dealt_with.as_ref().is_some_and(Vec::is_empty) && asked(r, f))
                .count();
            if runs >= cell.calls as usize {
                return Some(Waited::Calls { runs: runs as u32 });
            }
        }
        None
    }
}

/// Whether a run for these reasons is a way round: something new runs it, and every new version was written by a cell
/// after it.
fn way_round(because: &[Cause]) -> bool {
    because.iter().any(|c| matches!(c, Cause::Changed { .. })) && !because.iter().any(Cause::forward)
}

/// The runs of the records, each as the walk reads it.
pub(crate) fn read_runs(records: &Records) -> BTreeMap<RunId, RunView<'_>> {
    let mut runs = BTreeMap::new();
    for started in &records.started {
        let view = RunView {
            started,
            finished: records.finished.iter().find(|f| f.run == started.run),
            abandoned: records.abandoned.iter().any(|a| a.run == started.run),
            began: records.rounds.iter().filter(|r| r.run == started.run).count(),
            ended: records.returned.iter().filter(|r| r.run == started.run).count(),
        };
        runs.insert(started.run, view);
    }
    runs
}

/// Every version of every block, each with its meaning. The first version of a block has its own. A block that has
/// a value in this version or the one before, a choice, keeps the meaning of the version before when the two values
/// are the same, and has its own when they are not, whatever its writer says. Of any other block, a person's write
/// has its own meaning, and so does a write that matters; a write that does not matter keeps the meaning of the
/// version before it, provided its writer saw that meaning: the outside always did, and a run did when the version
/// it started from has it. A run that did not, because the block's meaning was changed after it started, has a
/// meaning of its own for what it wrote.
pub(crate) fn read_versions<'a>(
    records: &'a Records,
    runs: &BTreeMap<RunId, RunView<'a>>,
) -> BTreeMap<&'a BlockId, Vec<Version<'a>>> {
    let by_people = (records.written.iter()).map(|w| Written {
        block: &w.block,
        at: w.at,
        by: None,
        outsider: Some(w.by),
        matters: w.matters || w.by == Outsider::Person,
        value: w.value.as_deref(),
    });
    let by_runs = records.finished.iter().flat_map(|f| {
        f.writes.iter().map(|w| Written {
            block: &w.block,
            at: f.at,
            by: Some(f.run),
            outsider: None,
            matters: !w.meaning_unchanged,
            value: w.value.as_deref(),
        })
    });
    let mut writes: Vec<Written<'a>> = by_people.chain(by_runs).collect();
    writes.sort_by_key(|w| w.at);
    let mut versions: BTreeMap<&'a BlockId, Vec<Version<'a>>> = BTreeMap::new();
    for w in writes {
        let of_block = versions.entry(w.block).or_default();
        let over = of_block.last();
        let from = (w.by.and_then(|run| runs.get(&run))).and_then(|r| r.started.basis.get(w.block).copied().flatten());
        // A run saw what it writes over when the version it started from has the meaning of the version before.
        let started = of_block.iter().find(|v| Some(v.at) == from).map(|v| v.meaning);
        let saw = w.by.is_none() || (over.is_some() && started == over.map(|v| v.meaning));
        let meaning = match over {
            // A choice changes its meaning when its value does, whatever its writer says.
            Some(v) if w.value.is_some() || v.value.is_some() => {
                if w.value == v.value {
                    v.meaning
                } else {
                    w.at
                }
            }
            Some(v) if !w.matters && saw => v.meaning,
            _ => w.at,
        };
        of_block.push(Version {
            at: w.at,
            by: w.by,
            outsider: w.outsider,
            meaning,
            value: w.value,
        });
    }
    versions
}
