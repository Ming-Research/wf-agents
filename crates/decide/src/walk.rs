//! The walk: what a work does next, from the flow and its records.
//!
//! A work's state is its blocks. A cell reads some and writes some, and the cells are joined by nothing else: a cell
//! runs again when a block it reads has changed in meaning since it last ran, whoever wrote the change. The engine
//! keeps no place where a work is. Each time a round ends or something else happens to the work, it walks the cells
//! from the first, in their order: a cell whose condition is not met is skipped, what it wrote before still counting;
//! at the first that has a run at rest the run goes on, unless it waits for something; at the first whose
//! preconditions do not hold or that has reached its limit the work waits; the first that needs to run is run; the
//! rest are passed. A run is running only while one of its rounds is under way: between rounds it is at rest, a cell
//! before it may run, and it is abandoned once a block it started from has changed. A work has ended when its last
//! cell has a record or its item is cancelled.
//!
//! A pause of the work changes none of this order. It holds back the run the walk comes to, unless one of that run's
//! reasons is a change a person made, since the cell last ran (since the run of its record started, or with no record
//! its first run), of a note the cell reads: a paused work holds everything it would do by itself, still answers a
//! note, a person's words to a cell, and lets nothing else a person writes pass. Such a run has its rounds while the
//! work is paused ([`hold`]), and once it has finished what it wrote waits, as anything else does, for the work to be
//! let on.
//!
//! The owner's pause of everything holds back what the work's own pause does, and the walk reads it, as it reads whether
//! a slot is free, from what the scheduler gives it ([`Room`]): where no slot is free, a cell that takes one waits for
//! it before it runs, a run that answers a note as much as any other.

use std::collections::{BTreeMap, BTreeSet};

use crate::flow::{BlockId, Cell, CellId, Condition, Flow, TestKind, TestName};
use crate::records::{
    CallId, Cause, Finished, Outsider, Pause, Pauser, RecordFault, Records, RunId, Seq, Started, Who,
};
use crate::room::Room;
use crate::run::Hold;

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
    /// The run is abandoned, and the walk is taken again: it is at rest and stale, or its work was cancelled, when
    /// it may have a round under way, which is stopped.
    Abandon {
        cell: CellId,
        run: RunId,
        because: Abandoning,
    },
    /// The work waits at this cell, for each of these.
    Wait { at: CellId, on: Vec<Waited> },
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
    /// This block has changed in meaning since the run started from it.
    Outdated { block: BlockId },
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
    // One round of a work at a time.
    if let Some(run) = walk.runs.values().find(|r| r.under_way()) {
        return Decision::Busy {
            cell: run.started.cell.clone(),
            run: run.started.run,
        };
    }
    // A stale run is abandoned before anything else: its next round, or the answer to its call, would be for nothing.
    for run in walk.runs.values().filter(|r| r.at_rest()) {
        if let Some(because) = walk.stale(run) {
            return Decision::Abandon {
                cell: run.started.cell.clone(),
                run: run.started.run,
                because,
            };
        }
    }
    for (i, cell) in cells.iter().enumerate() {
        if !walk.applies(i) {
            continue;
        }
        let rest = walk.runs.values().find(|r| r.at_rest() && r.started.cell == cell.id);
        let mut on = Vec::new();
        if let Some(run) = rest {
            on.extend(walk.open_calls(run));
        }
        on.extend(walk.unmet(i));
        let wait = |on| Decision::Wait {
            at: cell.id.clone(),
            on,
        };
        if let Some(run) = rest {
            if !on.is_empty() {
                return wait(on);
            }
            return Decision::GoOn {
                cell: cell.id.clone(),
                run: run.started.run,
            };
        }
        let because = walk.needs(i);
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
        // A pause holds back the run, unless it is for a note a person wrote: everything before this is decided whether
        // or not the work is paused.
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
    unreachable!("the last cell has no condition, and with no record it runs, goes on or holds the work")
}

/// Why the run, if it is at rest, is stale; `None` where it is not, or is not at rest. What a round handed in is taken
/// as the round ends only of a run that is not stale.
pub fn stale(flow: &Flow, records: &Records, run: RunId) -> Option<Abandoning> {
    let walk = Walk::new(flow, records);
    let run = walk.runs.get(&run).filter(|r| r.at_rest())?;
    walk.stale(run)
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
        .chain(records.abandoned.iter().map(|a| a.run));
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

    /// The meaning the block has now.
    pub(crate) fn meaning_now(&self, block: &BlockId) -> Option<Seq> {
        self.current(block).map(|v| v.meaning)
    }

    /// The meaning of the version of the block written at this place.
    pub(crate) fn meaning_of(&self, block: &BlockId, version: Option<Seq>) -> Option<Seq> {
        let at = version?;
        self.versions(block).iter().find(|v| v.at == at).map(|v| v.meaning)
    }

    /// The version of the block the run knows: the one the run itself wrote, if it did, and otherwise the one it
    /// started from.
    pub(crate) fn known(&self, run: &RunView<'a>, block: &BlockId) -> Option<Seq> {
        match run.finished {
            Some(f) if f.writes.iter().any(|w| &w.block == block) => Some(f.at),
            _ => run.started.basis.get(block).copied().flatten(),
        }
    }

    /// The version the cell's run starts from, of each block the cell reads or may write: the current one.
    fn basis(&self, cell: &Cell) -> BTreeMap<BlockId, Option<Seq>> {
        (cell.inputs.iter().chain(&cell.outputs))
            .map(|b| (b.clone(), self.current(b).map(|v| v.at)))
            .collect()
    }

    /// Why the run at rest is stale, if it is: its cell is skipped or no longer in the flow, or a block its cell reads
    /// or may write has changed in meaning since the run started from it. A block the flow has given the cell since
    /// the run started, which the run started from no version of, makes it stale once the block has a version.
    fn stale(&self, run: &RunView<'a>) -> Option<Abandoning> {
        let place = self.flow.place(&run.started.cell).filter(|&i| self.applies(i));
        let Some(cell) = place.map(|i| &self.flow.cells()[i]) else {
            return Some(Abandoning::NoLongerApplies);
        };
        let mut blocks: Vec<&BlockId> = cell.inputs.iter().chain(&cell.outputs).collect();
        blocks.sort();
        blocks.dedup();
        (blocks.into_iter())
            .find(|block| {
                let from = run.started.basis.get(*block).copied().flatten();
                self.meaning_of(block, from) != self.meaning_now(block)
            })
            .map(|block| Abandoning::Outdated { block: block.clone() })
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

    /// Why the cell at this place needs to run; empty when its record still holds.
    pub(crate) fn needs(&self, i: usize) -> Vec<Cause> {
        let cell = &self.flow.cells()[i];
        let Some(record) = self.record(&cell.id) else {
            return vec![Cause::NoRecord];
        };
        (cell.inputs.iter())
            .map(|block| (block, self.known(record, block)))
            .filter(|(block, was)| self.meaning_of(block, *was) != self.meaning_now(block))
            .map(|(block, was)| Cause::Changed {
                block: block.clone(),
                was,
                behind: self.written_behind(block, was, i),
            })
            .collect()
    }

    /// Whether every change of the block's meaning since the version at `was` was written by a cell after the one at
    /// this place. One change among them by a person, the outside or a cell before it makes the reason theirs, however
    /// many cells after it wrote since: the owner is not asked to let go on what the owner sent back.
    fn written_behind(&self, block: &BlockId, was: Option<Seq>, i: usize) -> bool {
        (self.versions(block).iter())
            .filter(|v| v.meaning == v.at && Some(v.at) > was)
            .all(|v| {
                (v.by.and_then(|run| self.runs.get(&run)))
                    .and_then(|r| self.flow.place(&r.started.cell))
                    .is_some_and(|j| j > i)
            })
    }

    /// Whether one of these reasons of a run of the cell at this place, which starts at `before` (now where `None`), is
    /// a change of meaning a person made since the cell last ran: of a note the cell reads, written after the start of
    /// the run of its record, or with no record after the start of its first run, after the version the record knew,
    /// and before `before`. A cell with no record and one with a record so both count a note until a run of it
    /// finishes, whatever runs were abandoned since. A cell no run of which has started has not run, and answers no
    /// one: a work added paused begins nothing.
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
        let by_person = |block: &BlockId, was: Option<Seq>| {
            self.flow.is_note(block)
                && (self.versions(block).iter()).any(|v| {
                    v.meaning == v.at
                        && v.outsider == Some(Outsider::Person)
                        && v.at > since
                        && Some(v.at) > was
                        && earlier(v.at)
                })
        };
        because.iter().any(|cause| match cause {
            Cause::NoRecord => cell.inputs.iter().any(|block| by_person(block, None)),
            Cause::Changed { block, was, .. } => by_person(block, *was),
        })
    }

    /// The wait for the owner when the run these causes call for would be a way round, and so was each of the cell's
    /// last runs, as many in a row as its limit, since the owner last let the cell go on.
    fn at_limit(&self, cell: &Cell, because: &[Cause]) -> Option<Waited> {
        if because.iter().any(Cause::forward) {
            return None;
        }
        let let_go = (self.records.let_go.iter())
            .filter(|l| l.cell == cell.id)
            .map(|l| l.at)
            .max();
        let mut finished: Vec<(Seq, &RunView<'a>)> = (self.runs.values())
            .filter(|r| r.started.cell == cell.id)
            .filter_map(|r| r.finished.map(|f| (f.at, r)))
            .collect();
        finished.sort_by_key(|(at, _)| *at);
        let runs = (finished.iter().rev())
            .take_while(|(_, r)| let_go.is_none_or(|at| r.started.at > at))
            .take_while(|(_, r)| !r.started.because.iter().any(Cause::forward))
            .count();
        (runs >= cell.limit as usize).then_some(Waited::Limit { runs: runs as u32 })
    }
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
            matters: w.matters,
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
