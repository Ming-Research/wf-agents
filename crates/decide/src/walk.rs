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

use std::collections::{BTreeMap, BTreeSet};

use crate::flow::{BlockId, Cell, CellId, Condition, Flow, TestName};
use crate::records::{
    CallId, Cause, Finished, Outcome, Outsider, RecordFault, Records, RunId, Seq, Started, Tests, Who,
};

/// What a work does next, with the reason as data (principle decide-purely).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decision {
    /// The records cannot be those of a work: nothing is decided on them.
    Fault(RecordFault),
    /// The work has ended.
    Ended(End),
    /// A round of this run is under way: nothing is decided until it ends, unless the work is cancelled.
    Busy { cell: CellId, run: RunId },
    /// The work is paused, and what it would do next is run a cell: no run starts until it is let on. What does not
    /// begin a round is decided as if the work were not paused: a run at rest finishes, rests or is abandoned.
    Paused,
    /// The cell runs: why, and the version of each block it reads or may write that the run starts from. The engine
    /// records the run as started with exactly these.
    Run {
        cell: CellId,
        because: Vec<Cause>,
        basis: BTreeMap<BlockId, Option<Seq>>,
    },
    /// The run, which is at rest and waits for nothing, goes on: what its last round returned is taken up, and it
    /// has its next round, which it holds while the work is paused.
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
    /// A precondition of the cell does not hold: this person can make it hold.
    Precondition { test: TestName, who: Who },
    /// The cell's last runs, this many in a row, were each for what a cell after it wrote, and so would the next be:
    /// the owner lets it go on or cancels the work.
    Limit { runs: u32 },
}

/// What the work does next.
pub fn decide(flow: &Flow, records: &Records, tests: &dyn Tests) -> Decision {
    if let Err(fault) = check(records) {
        return Decision::Fault(fault);
    }
    let walk = Walk::new(flow, records, tests);
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
        if !walk.applies(cell) {
            continue;
        }
        let rest = walk.runs.values().find(|r| r.at_rest() && r.started.cell == cell.id);
        let mut on = Vec::new();
        if let Some(run) = rest {
            on.extend(walk.open_calls(run));
        }
        on.extend(walk.unmet(cell));
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
        // A pause lets no round begin, and so no run: everything before this is decided whether or not the work is
        // paused.
        if paused(records) {
            return Decision::Paused;
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
pub fn stale(flow: &Flow, records: &Records, tests: &dyn Tests, run: RunId) -> Option<Abandoning> {
    let walk = Walk::new(flow, records, tests);
    let run = walk.runs.get(&run).filter(|r| r.at_rest())?;
    walk.stale(run)
}

/// Whether the work is paused: the latest of its pauses and lettings on is a pause.
pub fn paused(records: &Records) -> bool {
    (records.pauses.iter()).max_by_key(|p| p.at).is_some_and(|p| p.paused)
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
        .chain(records.cancelled);
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
    began: usize,
    ended: usize,
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
    tests: &'a dyn Tests,
    pub(crate) runs: BTreeMap<RunId, RunView<'a>>,
    /// The versions of each block, in the order they were written.
    pub(crate) versions: BTreeMap<&'a BlockId, Vec<Version<'a>>>,
}

impl<'a> Walk<'a> {
    pub(crate) fn new(flow: &'a Flow, records: &'a Records, tests: &'a dyn Tests) -> Self {
        let runs = read_runs(records);
        let versions = read_versions(records, &runs);
        Self {
            flow,
            records,
            tests,
            runs,
            versions,
        }
    }

    /// Whether the cell's condition is met: with none it always is.
    pub(crate) fn applies(&self, cell: &Cell) -> bool {
        match &cell.applies {
            None => true,
            Some(Condition::Test(name)) => self.tests.test(name) == Outcome::Holds,
            Some(Condition::Is { block, value }) => self.current(block).and_then(|v| v.value) == Some(value.as_str()),
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

    /// Why the run at rest is stale, if it is.
    fn stale(&self, run: &RunView<'a>) -> Option<Abandoning> {
        let cell = (self.flow.place(&run.started.cell)).map(|i| &self.flow.cells()[i]);
        if !cell.is_some_and(|c| self.applies(c)) {
            return Some(Abandoning::NoLongerApplies);
        }
        (run.started.basis.iter())
            .find(|(block, version)| self.meaning_of(block, **version) != self.meaning_now(block))
            .map(|(block, _)| Abandoning::Outdated { block: block.clone() })
    }

    /// The calls the run rests at that have no result yet.
    fn open_calls(&self, run: &RunView<'a>) -> Vec<Waited> {
        let records = self.records;
        (records.called.iter())
            .filter(|c| c.run == run.started.run && !records.done.iter().any(|d| d.call == c.call))
            .map(|c| Waited::Call {
                call: c.call,
                on: c.on.clone(),
            })
            .collect()
    }

    /// The preconditions of the cell that do not hold.
    fn unmet(&self, cell: &Cell) -> Vec<Waited> {
        (cell.requires.iter())
            .filter_map(|test| match self.tests.test(test) {
                Outcome::Holds => None,
                Outcome::Waits { who } => Some(Waited::Precondition {
                    test: test.clone(),
                    who,
                }),
            })
            .collect()
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
                behind: self.written_behind(block, i),
            })
            .collect()
    }

    /// Whether the meaning the block has now was given it by a cell after the one at this place.
    fn written_behind(&self, block: &BlockId, i: usize) -> bool {
        let changed = self.meaning_now(block);
        (self.versions(block).iter())
            .find(|v| Some(v.at) == changed)
            .and_then(|v| v.by)
            .and_then(|run| self.runs.get(&run))
            .and_then(|r| self.flow.place(&r.started.cell))
            .is_some_and(|j| j > i)
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
