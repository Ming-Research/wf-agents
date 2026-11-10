//! The walk: what a work does next, from the flow and its records.
//!
//! The engine keeps no place where a work stands. Each time a round ends or something else happens to the work, it
//! walks the cells from the first, in their order: a cell off the path is skipped; at the first that has a run at rest
//! the run goes on, unless it waits for something; at the first that depends on a refused product no cell answers for,
//! whose preconditions do not hold, or that has reached its limit, the work waits; the first that needs to run is run;
//! the rest are passed. A run is running only while one of its rounds is under way: between rounds it is at rest, a
//! cell before it may run, and it is abandoned once it no longer holds. A work has ended when its last cell has a
//! record or its item is cancelled.

use std::collections::{BTreeMap, BTreeSet};

use crate::flow::{Cell, CellId, Condition, Flow, ProductId, TestName};
use crate::records::{
    CallId, Cause, Finished, Outcome, RecordFault, Records, Refusal, RefusalId, Refuser, RunId, Seq, Started, Tests,
    Who,
};

/// What a work does next, with the reason as data (principle decide-purely).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decision {
    /// The records cannot be those of a work: nothing is decided on them.
    Fault(RecordFault),
    /// The work has ended.
    Ended(End),
    /// A round of this run is under way: nothing is decided until it ends.
    Busy { cell: CellId, run: RunId },
    /// The work is paused: nothing starts until it is let on.
    Paused,
    /// The cell runs: why, the version of each thing it depends on that the run starts from, the version of each
    /// product it may write that the run writes over, and the refusals it is given. The engine records the run as
    /// started with exactly these.
    Run {
        cell: CellId,
        because: Vec<Cause>,
        basis: BTreeMap<ProductId, Option<Seq>>,
        over: BTreeMap<ProductId, Option<Seq>>,
        given: Vec<RefusalId>,
    },
    /// The run, which is at rest and waits for nothing, has its next round.
    GoOn { cell: CellId, run: RunId },
    /// The run, which is at rest, no longer holds: it is abandoned, and the walk is taken again.
    Abandon { cell: CellId, run: RunId, because: Stale },
    /// The work waits at this cell, for each of these.
    Wait { at: CellId, on: Vec<Waited> },
}

/// How a work ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    /// Its last cell has a record.
    Passed,
    /// Its item was cancelled.
    Cancelled,
}

/// Why a run at rest no longer holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Stale {
    /// Its cell is no longer on the path, or no longer in the flow.
    NoLongerApplies,
    /// This product has changed in meaning since the run started from it.
    Outdated { product: ProductId },
    /// This refusal, which the run's cell is the one to answer, was made after the run started.
    Refused { refusal: RefusalId },
}

/// What a work waits for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Waited {
    /// The result of a call the cell's run rests at: from this person, or from the engine where `on` is `None`.
    Call { call: CallId, on: Option<Who> },
    /// A product is refused and no cell on the path answers for it: the owner decides.
    Unanswered {
        refusal: RefusalId,
        product: ProductId,
        by: Refuser,
    },
    /// A precondition of the cell does not hold: this person can make it hold.
    Precondition { test: TestName, who: Who },
    /// The cell's last runs, this many in a row, were each a way round, and so would the next be: the owner lets it
    /// go on or cancels the work.
    Limit { runs: u32 },
}

/// What the work does next.
pub fn decide(flow: &Flow, records: &Records, tests: &dyn Tests) -> Decision {
    if let Err(fault) = check(records) {
        return Decision::Fault(fault);
    }
    if records.cancelled.is_some() {
        return Decision::Ended(End::Cancelled);
    }
    let walk = Walk::new(flow, records, tests);
    let cells = flow.cells();
    let last = cells.len() - 1;
    if walk.record(last).is_some() {
        return Decision::Ended(End::Passed);
    }
    // One round of a work at a time.
    if let Some(run) = walk.runs.values().find(|r| r.under_way()) {
        return Decision::Busy {
            cell: run.started.cell.clone(),
            run: run.started.run,
        };
    }
    if walk.paused() {
        return Decision::Paused;
    }
    // A run at rest that no longer holds is abandoned before anything else: its next round, or the answer to its call,
    // would be for nothing.
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
        if !walk.on_path[i] {
            continue;
        }
        let rest = walk.runs.values().find(|r| r.at_rest() && r.started.cell == cell.id);
        let mut on = Vec::new();
        if let Some(run) = rest {
            on.extend(walk.open_calls(run));
        }
        on.extend(walk.unanswered(i));
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
        on.extend(walk.at_limit(i, &because));
        if !on.is_empty() {
            return wait(on);
        }
        return Decision::Run {
            cell: cell.id.clone(),
            because,
            basis: walk.basis(i),
            over: walk.over(i),
            given: walk.given(cell),
        };
    }
    unreachable!("the last cell is always on the path, and with no record it runs, goes on or holds the work")
}

/// The version of the product that is current: the latest written by a person, from outside, or by a finished run of
/// a cell on the path. A refusal by a person is of this version.
pub fn current(flow: &Flow, records: &Records, tests: &dyn Tests, product: &ProductId) -> Option<Seq> {
    Walk::new(flow, records, tests).current(product).map(|v| v.at)
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
        .chain(records.refusals.iter().filter_map(|r| match r.by {
            Refuser::Run(run) => Some(run),
            Refuser::Person(_) => None,
        }));
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
    if let Some(d) = (records.done.iter()).find(|d| !records.called.iter().any(|c| c.call == d.call)) {
        return Err(RecordFault::UnknownCall(d.call));
    }
    let places = (records.started.iter().map(|e| e.at))
        .chain(records.rounds.iter().map(|e| e.at))
        .chain(records.returned.iter().map(|e| e.at))
        .chain(records.called.iter().map(|e| e.at))
        .chain(records.done.iter().map(|e| e.at))
        .chain(records.finished.iter().map(|e| e.at))
        .chain(records.abandoned.iter().map(|e| e.at))
        .chain(records.written.iter().map(|e| e.at))
        .chain(records.lost.iter().map(|e| e.at))
        .chain(records.refusals.iter().map(|e| e.at))
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
struct RunView<'a> {
    started: &'a Started,
    finished: Option<&'a Finished>,
    abandoned: bool,
    /// How many of its rounds began, and how many ended.
    began: usize,
    ended: usize,
}

impl RunView<'_> {
    /// Whether the run has neither finished nor been abandoned.
    fn open(&self) -> bool {
        self.finished.is_none() && !self.abandoned
    }

    /// Whether a round of the run is under way.
    fn under_way(&self) -> bool {
        self.open() && self.began > self.ended
    }

    /// Whether the run is at rest: open, with no round under way.
    fn at_rest(&self) -> bool {
        self.open() && self.began == self.ended
    }

    /// Whether the run, as it finished, wrote the product.
    fn wrote(&self, product: &ProductId) -> bool {
        self.finished
            .is_some_and(|f| f.writes.iter().any(|w| &w.product == product))
    }
}

/// How far along the flow a reading of a product goes. A cell depends only on what was written before it: a write by a
/// cell after it changes nothing for it, and the way back is a refusal.
#[derive(Clone, Copy)]
enum Reach {
    /// The whole path: the product's current version.
    All,
    /// A person, the outside and the cells before this place: what the cell at this place depends on.
    Before(usize),
    /// The same and the cell at this place itself: what the cell at this place writes over.
    Through(usize),
}

/// A version of a product: the write that made it.
struct Version {
    /// The place of the entry that wrote it, which names it.
    at: Seq,
    /// The run that wrote it; `None` for a person or the outside.
    by: Option<RunId>,
    /// The version at which the meaning it has last changed.
    meaning: Seq,
}

/// One walk of a work: the flow, the records, and what is worked out of them once.
struct Walk<'a> {
    flow: &'a Flow,
    records: &'a Records,
    tests: &'a dyn Tests,
    runs: BTreeMap<RunId, RunView<'a>>,
    /// Whether each cell, by its place, is on the path.
    on_path: Vec<bool>,
    /// The versions of each product, in the order they were written.
    versions: BTreeMap<&'a ProductId, Vec<Version>>,
}

impl<'a> Walk<'a> {
    fn new(flow: &'a Flow, records: &'a Records, tests: &'a dyn Tests) -> Self {
        let mut runs: BTreeMap<RunId, RunView<'a>> = BTreeMap::new();
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
        let mut walk = Self {
            flow,
            records,
            tests,
            runs,
            on_path: Vec::new(),
            versions: BTreeMap::new(),
        };
        for cell in flow.cells() {
            let on = walk.applies(cell);
            walk.on_path.push(on);
        }
        walk.versions = walk.read_versions();
        walk
    }

    /// Whether the cell's condition holds; the cells before it have been judged already.
    fn applies(&self, cell: &Cell) -> bool {
        match &cell.applies {
            None => true,
            Some(Condition::Test(name)) => self.tests.test(name, self.records) == Outcome::Holds,
            Some(Condition::Answered { by, output, is }) => self
                .flow
                .place(by)
                .filter(|&j| self.on_path.get(j).copied().unwrap_or(false))
                .and_then(|_| self.latest(by))
                .and_then(|r| r.finished)
                .is_some_and(|f| f.answers.get(output) == Some(is)),
        }
    }

    /// Every version of every product, each with its meaning: its own where a person or the outside wrote it or its
    /// writer said the change matters, and otherwise that of the version its run wrote over.
    fn read_versions(&self) -> BTreeMap<&'a ProductId, Vec<Version>> {
        let records = self.records;
        let mut writes: Vec<(&'a ProductId, Seq, Option<RunId>, bool)> = Vec::new();
        for w in &records.written {
            writes.push((&w.product, w.at, None, true));
        }
        for f in &records.finished {
            for w in &f.writes {
                writes.push((&w.product, f.at, Some(f.run), w.matters));
            }
        }
        writes.sort_by_key(|w| w.1);
        let mut versions: BTreeMap<&'a ProductId, Vec<Version>> = BTreeMap::new();
        for (product, at, by, matters) in writes {
            let of_product = versions.entry(product).or_default();
            let from = by
                .filter(|_| !matters)
                .and_then(|run| self.runs.get(&run))
                .and_then(|r| r.started.over.get(product).copied().flatten());
            let meaning = from
                .and_then(|v| of_product.iter().find(|x| x.at == v))
                .map_or(at, |v| v.meaning);
            of_product.push(Version { at, by, meaning });
        }
        versions
    }

    /// The cell's latest finished run, whether or not the cell is on the path.
    fn latest(&self, cell: &CellId) -> Option<&RunView<'a>> {
        self.runs
            .values()
            .filter(|r| &r.started.cell == cell)
            .filter_map(|r| r.finished.map(|f| (f.at, r)))
            .max_by_key(|(at, _)| *at)
            .map(|(_, r)| r)
    }

    /// The cell's record: its latest finished run. A cell off the path has none.
    fn record(&self, i: usize) -> Option<&RunView<'a>> {
        if !self.on_path[i] {
            return None;
        }
        self.latest(&self.flow.cells()[i].id)
    }

    /// Whether the work is paused.
    fn paused(&self) -> bool {
        self.records
            .pauses
            .iter()
            .max_by_key(|p| p.at)
            .is_some_and(|p| p.paused)
    }

    /// Whether the version counts within the reach: a person or the outside wrote it, or a run of a cell on the path
    /// that the reach takes in.
    fn counts(&self, version: &Version, reach: Reach) -> bool {
        let Some(run) = version.by else {
            return true;
        };
        let place = (self.runs.get(&run))
            .and_then(|r| self.flow.place(&r.started.cell))
            .filter(|&j| self.on_path[j]);
        match (place, reach) {
            (None, _) => false,
            (Some(_), Reach::All) => true,
            (Some(j), Reach::Before(i)) => j < i,
            (Some(j), Reach::Through(i)) => j <= i,
        }
    }

    fn versions(&self, product: &ProductId) -> &[Version] {
        self.versions.get(product).map_or(&[], Vec::as_slice)
    }

    /// The latest version of the product that counts within the reach.
    fn seen(&self, product: &ProductId, reach: Reach) -> Option<&Version> {
        self.versions(product).iter().rev().find(|v| self.counts(v, reach))
    }

    /// The current version of the product: the latest that counts on the whole path.
    fn current(&self, product: &ProductId) -> Option<&Version> {
        self.seen(product, Reach::All)
    }

    /// The meaning the product has within the reach.
    fn meaning_seen(&self, product: &ProductId, reach: Reach) -> Option<Seq> {
        self.seen(product, reach).map(|v| v.meaning)
    }

    /// The meaning of the version of the product written at this place.
    fn meaning_of(&self, product: &ProductId, version: Option<Seq>) -> Option<Seq> {
        let at = version?;
        self.versions(product).iter().find(|v| v.at == at).map(|v| v.meaning)
    }

    /// The meaning of the product the run was built on: that of the version it started from.
    fn built(&self, run: &RunView<'a>, product: &ProductId) -> Option<Seq> {
        self.meaning_of(product, run.started.basis.get(product).copied().flatten())
    }

    /// Whether the product, which the record wrote, was lost after the record finished.
    fn gone(&self, record: &RunView<'a>, product: &ProductId) -> bool {
        record.finished.is_some_and(|f| {
            record.wrote(product) && self.records.lost.iter().any(|l| &l.product == product && l.at > f.at)
        })
    }

    /// Whether the refusal still stands: the product has not been written since by a person or from outside, nor by
    /// a run that was given the refusal.
    fn stands(&self, refusal: &Refusal) -> bool {
        !self.versions(&refusal.product).iter().any(|v| {
            v.at > refusal.at
                && self.counts(v, Reach::All)
                && v.by.is_none_or(|run| {
                    self.runs
                        .get(&run)
                        .is_some_and(|r| r.started.given.contains(&refusal.id))
                })
        })
    }

    /// Whether the refusal has force: a cell's only while the run that made it is the cell's record, and none
    /// against a version that does not count.
    fn has_force(&self, refusal: &Refusal) -> bool {
        let by = match &refusal.by {
            Refuser::Person(_) => true,
            Refuser::Run(id) => self.runs.get(id).is_some_and(|r| {
                self.flow
                    .place(&r.started.cell)
                    .and_then(|i| self.record(i))
                    .is_some_and(|record| record.started.run == *id)
            }),
        };
        by && self
            .versions(&refusal.product)
            .iter()
            .any(|v| v.at == refusal.version && self.counts(v, Reach::All))
    }

    /// The refusals in force: those that stand and have force.
    fn in_force(&self) -> impl Iterator<Item = &'a Refusal> + '_ {
        self.records
            .refusals
            .iter()
            .filter(|r| self.stands(r) && self.has_force(r))
    }

    /// The places of the cells that answer for the product, in their order: on the path, declaring that they may
    /// write it, and either having no record yet or having written it in their record.
    fn answerers(&self, product: &ProductId) -> Vec<usize> {
        let cells = self.flow.cells();
        (0..cells.len())
            .filter(|&i| self.on_path[i] && cells[i].writes.contains(product))
            .filter(|&i| self.record(i).is_none_or(|r| r.wrote(product)))
            .collect()
    }

    /// The version the run of the cell at this place starts from, of each product the cell depends on: the latest
    /// written by a person, from outside or by a cell before it.
    fn basis(&self, i: usize) -> BTreeMap<ProductId, Option<Seq>> {
        let follows = self.flow.cells()[i].follows.iter();
        follows
            .map(|p| (p.clone(), self.seen(p, Reach::Before(i)).map(|v| v.at)))
            .collect()
    }

    /// The version the run of the cell at this place writes over, of each product the cell may write: the latest
    /// written by a person, from outside, by a cell before it or by the cell itself.
    fn over(&self, i: usize) -> BTreeMap<ProductId, Option<Seq>> {
        let writes = self.flow.cells()[i].writes.iter();
        writes
            .map(|p| (p.clone(), self.seen(p, Reach::Through(i)).map(|v| v.at)))
            .collect()
    }

    /// The refusals the cell's run is given: those in force against a product the cell may write.
    fn given(&self, cell: &Cell) -> Vec<RefusalId> {
        self.in_force()
            .filter(|r| cell.writes.contains(&r.product))
            .map(|r| r.id)
            .collect()
    }

    /// Why the run at rest no longer holds, if it does not.
    fn stale(&self, run: &RunView<'a>) -> Option<Stale> {
        let Some(i) = self.flow.place(&run.started.cell).filter(|&i| self.on_path[i]) else {
            return Some(Stale::NoLongerApplies);
        };
        let from = (run.started.basis.iter()).map(|(p, v)| (p, v, Reach::Before(i)));
        let over = (run.started.over.iter()).map(|(p, v)| (p, v, Reach::Through(i)));
        for (product, version, reach) in from.chain(over) {
            if self.meaning_of(product, *version) != self.meaning_seen(product, reach) {
                return Some(Stale::Outdated {
                    product: product.clone(),
                });
            }
        }
        self.in_force()
            .filter(|r| !run.started.given.contains(&r.id))
            .find(|r| self.answerers(&r.product).first() == Some(&i))
            .map(|r| Stale::Refused { refusal: r.id })
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

    /// The refusals in force that no cell answers for and that hold the work before this cell: those of a product
    /// the cell depends on, and before the last cell every one.
    fn unanswered(&self, i: usize) -> Vec<Waited> {
        let cells = self.flow.cells();
        self.in_force()
            .filter(|r| i == cells.len() - 1 || cells[i].follows.contains(&r.product))
            .filter(|r| self.answerers(&r.product).is_empty())
            .map(|r| Waited::Unanswered {
                refusal: r.id,
                product: r.product.clone(),
                by: r.by.clone(),
            })
            .collect()
    }

    /// The preconditions of the cell that do not hold.
    fn unmet(&self, cell: &Cell) -> Vec<Waited> {
        (cell.requires.iter())
            .filter_map(|test| match self.tests.test(test, self.records) {
                Outcome::Holds => None,
                Outcome::Waits { who } => Some(Waited::Precondition {
                    test: test.clone(),
                    who,
                }),
            })
            .collect()
    }

    /// Why the cell needs to run; empty when its record still holds.
    fn needs(&self, i: usize) -> Vec<Cause> {
        let cell = &self.flow.cells()[i];
        let Some(record) = self.record(i) else {
            return vec![Cause::NoRecord];
        };
        let mut because = Vec::new();
        for product in &cell.writes {
            if self.gone(record, product) {
                because.push(Cause::Gone {
                    product: product.clone(),
                });
            }
        }
        for product in &cell.follows {
            if self.built(record, product) != self.meaning_seen(product, Reach::Before(i)) {
                because.push(Cause::Changed {
                    product: product.clone(),
                });
            }
        }
        for refusal in &self.records.refusals {
            if refusal.by == Refuser::Run(record.started.run) && !self.stands(refusal) {
                because.push(Cause::Answered {
                    refusal: refusal.id,
                    product: refusal.product.clone(),
                });
            }
        }
        for refusal in self.in_force() {
            if self.answerers(&refusal.product).first() == Some(&i) {
                because.push(Cause::Refused {
                    refusal: refusal.id,
                    product: refusal.product.clone(),
                    by: refusal.by.clone(),
                });
            }
        }
        because
    }

    /// The wait for the owner when the run these causes call for would be a way round, and so was each of the cell's
    /// last runs, as many in a row as its limit, since the owner last let the cell go on.
    fn at_limit(&self, i: usize, because: &[Cause]) -> Option<Waited> {
        if because.iter().any(Cause::forward) {
            return None;
        }
        let cell = &self.flow.cells()[i];
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
