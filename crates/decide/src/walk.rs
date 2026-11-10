//! The walk: what a work does next, from the flow and its records.
//!
//! The engine keeps no place where a work stands. Each time something happens it walks the cells from the first, in
//! their order: a cell off the path is skipped; at the first whose run waits for a person, that depends on a refused
//! product no cell can answer for, whose preconditions do not hold, or that has reached its limit, the work waits; the
//! first that needs to run is run; the rest are passed. A work has ended when its last cell has a record or its item
//! is cancelled.

use std::collections::BTreeMap;

use crate::flow::{Cell, CellId, Condition, Flow, ProductId, TestName};
use crate::records::{
    Asked, Built, CallId, Finished, Outcome, Records, Refusal, RefusalId, Refuser, RunId, Seq, Started, Tests, Version,
    Who, Writer,
};

/// What a work does next, with the reason as data (principle decide-purely).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decision {
    /// Nothing more is to be done.
    Ended(End),
    /// The work is paused: nothing is decided until it is let on.
    Paused,
    /// A cell is running: nothing is decided until it stops. `deliver` are the refusals made since that its run
    /// answers for and was not given: the engine puts them into the run's session and records them as delivered.
    Busy {
        cell: CellId,
        run: RunId,
        deliver: Vec<RefusalId>,
    },
    /// The cell runs: why, what the run is built on, and the refusals it is given. The engine records the run as
    /// started with exactly these.
    Run {
        cell: CellId,
        because: Vec<Cause>,
        basis: BTreeMap<ProductId, Built>,
        given: Vec<RefusalId>,
    },
    /// The run that waits for a person no longer stands: it is abandoned and its call withdrawn.
    Abandon { cell: CellId, run: RunId, because: Stale },
    /// The work waits before this cell, or in its run, for each of these.
    Wait { at: CellId, on: Vec<Waited> },
}

/// How a work ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum End {
    /// Its last cell has a record.
    Passed,
    /// The item was cancelled.
    Cancelled,
}

/// Why a cell needs to run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Cause {
    /// It has no record.
    NoRecord,
    /// A product it wrote no longer exists.
    Gone { product: ProductId },
    /// Something it depends on changed in meaning since its record. `built` is none when the record was built on no
    /// version of it, `now` when no version of it is seen.
    Changed {
        product: ProductId,
        built: Option<u32>,
        now: Option<u32>,
    },
    /// What it refused was written anew by a run that was given the refusal, or by a person, so it judges again.
    Answered { product: ProductId },
    /// A refusal that has force stands against a product it wrote.
    Refused {
        refusal: RefusalId,
        product: ProductId,
        by: Refuser,
        why: String,
    },
}

/// Why a run that waits for a person no longer stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Stale {
    /// Its cell is no longer in the flow, or no longer on the path.
    NoLongerApplies,
    /// Something it depends on changed in meaning since it started.
    Outdated { product: ProductId },
}

/// One thing a work waits for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Waited {
    /// A precondition of the cell does not hold.
    Precondition { test: TestName, what: String, who: Who },
    /// The cell's run waits at a call for a person.
    Call { call: CallId, what: String, who: Who },
    /// The cell has run this often without anything before it having changed: the owner lets it go on, or cancels.
    Limit { runs: u32 },
    /// A refusal stands against a product the cell depends on, and no cell on the path can answer for it.
    Unanswered {
        refusal: RefusalId,
        product: ProductId,
        by: Refuser,
    },
}

impl Waited {
    /// Who can give what is waited for.
    pub fn who(&self) -> Who {
        match self {
            Waited::Precondition { who, .. } | Waited::Call { who, .. } => who.clone(),
            Waited::Limit { .. } | Waited::Unanswered { .. } => Who::Owner,
        }
    }
}

/// What the work does next.
pub fn decide(flow: &Flow, records: &Records, tests: &dyn Tests) -> Decision {
    let walk = Walk::new(flow, records, tests);
    let cells = flow.cells();
    if records.cancelled.is_some() {
        return Decision::Ended(End::Cancelled);
    }
    if walk.record(&cells[cells.len() - 1].id).is_some() {
        return Decision::Ended(End::Passed);
    }
    if records.pauses.iter().max_by_key(|p| p.at).is_some_and(|p| p.paused) {
        return Decision::Paused;
    }
    // One cell of a work at a time. A run that waits for a person is not running.
    if let Some(run) = walk.runs.values().find(|r| walk.running(r)) {
        let cell = run.started.cell.clone();
        let deliver = walk
            .records
            .refusals
            .iter()
            .filter(|r| walk.given(run, r.id).is_none() && walk.stands(r) && walk.has_force(r))
            .filter(|r| walk.answers_for(&r.product).iter().any(|c| c.id == cell))
            .map(|r| r.id)
            .collect();
        return Decision::Busy {
            cell,
            run: run.started.run,
            deliver,
        };
    }
    // A waiting run that no longer stands is abandoned before anything else: its call would be answered for nothing.
    for run in walk.runs.values().filter(|r| walk.asked(r).is_some()) {
        let cell = &run.started.cell;
        let stale = match flow.place(cell) {
            Some(i) if walk.on_path[i] => cells[i].follows.iter().find_map(|p| {
                let now = walk.seen(p).map(|v| v.meaning);
                let built = run.started.basis.get(p).map(|b| b.meaning);
                (built != now).then(|| Stale::Outdated { product: p.clone() })
            }),
            _ => Some(Stale::NoLongerApplies),
        };
        if let Some(because) = stale {
            return Decision::Abandon {
                cell: cell.clone(),
                run: run.started.run,
                because,
            };
        }
    }
    for (i, cell) in cells.iter().enumerate() {
        if !walk.on_path[i] {
            continue;
        }
        let at = cell.id.clone();
        if let Some(asked) = walk
            .runs
            .values()
            .filter(|r| r.started.cell == cell.id)
            .find_map(|r| walk.asked(r))
        {
            let on = vec![Waited::Call {
                call: asked.call,
                what: asked.what.clone(),
                who: asked.who.clone(),
            }];
            return Decision::Wait { at, on };
        }
        let unanswered = walk.unanswered(cell);
        if !unanswered.is_empty() {
            return Decision::Wait { at, on: unanswered };
        }
        let unmet: Vec<Waited> = cell
            .requires
            .iter()
            .filter_map(|t| match tests.test(t, records) {
                Outcome::Holds => None,
                Outcome::Waits { what, who } => Some(Waited::Precondition {
                    test: t.clone(),
                    what,
                    who,
                }),
            })
            .collect();
        if !unmet.is_empty() {
            return Decision::Wait { at, on: unmet };
        }
        let because = walk.needs(i);
        if because.is_empty() {
            continue;
        }
        if let Some(limit) = walk.at_limit(i) {
            return Decision::Wait { at, on: vec![limit] };
        }
        let basis = cell
            .follows
            .iter()
            .filter_map(|p| {
                walk.seen(p).map(|v| {
                    (
                        p.clone(),
                        Built {
                            content: v.content,
                            meaning: v.meaning,
                        },
                    )
                })
            })
            .collect();
        let given = because
            .iter()
            .filter_map(|c| match c {
                Cause::Refused { refusal, .. } => Some(*refusal),
                _ => None,
            })
            .collect();
        return Decision::Run {
            cell: at,
            because,
            basis,
            given,
        };
    }
    // The last cell of a flow has no condition (`Flow::new`), so it is on the path; it has no record, or the work had
    // ended above; and a cell on the path with no record is waited at or run.
    unreachable!("the walk passed the last cell of a work that has not ended")
}

/// A run with what became of it.
struct RunView<'a> {
    started: &'a Started,
    finished: Option<&'a Finished>,
    abandoned: bool,
}

/// One walk's view of a work.
struct Walk<'a> {
    flow: &'a Flow,
    records: &'a Records,
    runs: BTreeMap<RunId, RunView<'a>>,
    /// For each cell of the flow, whether it is on the work's path.
    on_path: Vec<bool>,
}

impl<'a> Walk<'a> {
    fn new(flow: &'a Flow, records: &'a Records, tests: &dyn Tests) -> Self {
        let mut runs: BTreeMap<RunId, RunView<'a>> = records
            .started
            .iter()
            .map(|s| {
                (
                    s.run,
                    RunView {
                        started: s,
                        finished: None,
                        abandoned: false,
                    },
                )
            })
            .collect();
        for f in &records.finished {
            if let Some(r) = runs.get_mut(&f.run) {
                r.finished = Some(f);
            }
        }
        for a in &records.abandoned {
            if let Some(r) = runs.get_mut(&a.run) {
                r.abandoned = true;
            }
        }
        let mut walk = Self {
            flow,
            records,
            runs,
            on_path: Vec::new(),
        };
        // A condition reads only cells before its own, so the path is found in the cells' order.
        for cell in flow.cells() {
            let on = match &cell.applies {
                None => true,
                Some(Condition::Answered { by, output, is }) => {
                    walk.cell_on_path(by)
                        && walk
                            .record(by)
                            .and_then(|r| r.finished)
                            .and_then(|f| f.answers.get(output))
                            == Some(is)
                }
                Some(Condition::Test(name)) => tests.test(name, records) == Outcome::Holds,
            };
            walk.on_path.push(on);
        }
        walk
    }

    /// Whether the cell of this name is in the flow and on the path, as far as the path has been found.
    fn cell_on_path(&self, cell: &CellId) -> bool {
        self.flow
            .place(cell)
            .is_some_and(|i| self.on_path.get(i).copied().unwrap_or(false))
    }

    /// A cell's record: its latest finished run.
    fn record(&self, cell: &CellId) -> Option<&RunView<'a>> {
        self.runs
            .values()
            .filter(|r| &r.started.cell == cell && r.finished.is_some())
            .max_by_key(|r| r.finished.map(|f| f.at))
    }

    /// The call the run waits at for a person, when it waits at one.
    fn asked(&self, run: &RunView<'a>) -> Option<&'a Asked> {
        if run.finished.is_some() || run.abandoned {
            return None;
        }
        self.records
            .asked
            .iter()
            .filter(|a| a.run == run.started.run)
            .find(|a| !self.records.answered.iter().any(|n| n.call == a.call))
    }

    /// Whether the run is under way: neither finished nor abandoned, and not waiting for a person.
    fn running(&self, run: &RunView<'a>) -> bool {
        run.finished.is_none() && !run.abandoned && self.asked(run).is_none()
    }

    /// Whether the version counts as written: it is no mark of a loss, and a person, the outside or a finished run of
    /// a cell on the path wrote it.
    fn written(&self, v: &Version) -> bool {
        !v.gone
            && match v.by {
                Writer::Person | Writer::Outside => true,
                Writer::Run(id) => self
                    .runs
                    .get(&id)
                    .is_some_and(|r| r.finished.is_some() && self.cell_on_path(&r.started.cell)),
            }
    }

    /// The product as the cells see it: its latest version that counts as written. What a cell off the path handed in
    /// is absent.
    fn seen(&self, product: &ProductId) -> Option<&'a Version> {
        self.records
            .versions
            .iter()
            .filter(|v| &v.product == product && self.written(v))
            .max_by_key(|v| v.at)
    }

    /// Whether the product was lost and not written since.
    fn lost(&self, product: &ProductId) -> bool {
        self.records
            .versions
            .iter()
            .filter(|v| &v.product == product)
            .max_by_key(|v| v.at)
            .is_some_and(|v| v.gone)
    }

    /// Whether a finished run of the cell wrote a version of the product.
    fn wrote(&self, cell: &CellId, product: &ProductId) -> bool {
        self.records.versions.iter().any(|v| {
            &v.product == product
                && !v.gone
                && matches!(v.by, Writer::Run(id) if self.runs.get(&id).is_some_and(|r| r.finished.is_some() && &r.started.cell == cell))
        })
    }

    /// The meaning of the product the record was built on: that of its own last write of it, when it wrote it.
    fn built(&self, record: &RunView<'a>, product: &ProductId) -> Option<u32> {
        let own = self
            .records
            .versions
            .iter()
            .filter(|v| &v.product == product && !v.gone && v.by == Writer::Run(record.started.run))
            .max_by_key(|v| v.at);
        own.map(|v| v.meaning)
            .or_else(|| record.started.basis.get(product).map(|b| b.meaning))
    }

    /// When the run was given the refusal: as it started, or when the refusal was put into its session while it was
    /// under way.
    fn given(&self, run: &RunView<'a>, refusal: RefusalId) -> Option<Seq> {
        if run.started.given.contains(&refusal) {
            return Some(run.started.at);
        }
        let delivered = self.records.delivered.iter();
        delivered
            .filter(|d| d.run == run.started.run && d.refusal == refusal)
            .map(|d| d.at)
            .min()
    }

    /// Whether the refusal still stands: the product has not been written since by a person or from outside, nor by a
    /// finished run that had been given the refusal when it wrote.
    fn stands(&self, refusal: &Refusal) -> bool {
        !self.records.versions.iter().any(|v| {
            v.product == refusal.product
                && !v.gone
                && v.at > refusal.at
                && match v.by {
                    Writer::Person | Writer::Outside => true,
                    Writer::Run(id) => self
                        .runs
                        .get(&id)
                        .is_some_and(|r| r.finished.is_some() && self.given(r, refusal.id).is_some_and(|at| at < v.at)),
                }
        })
    }

    /// Whether the refusal has force: a cell's only while the run that made it is the cell's record and the cell is on
    /// the path, and none against a version that a cell off the path wrote.
    fn has_force(&self, refusal: &Refusal) -> bool {
        let by = match &refusal.by {
            Refuser::Person(_) => true,
            Refuser::Run(id) => self.runs.get(id).is_some_and(|by| {
                self.cell_on_path(&by.started.cell)
                    && self
                        .record(&by.started.cell)
                        .is_some_and(|r| r.started.run == by.started.run)
            }),
        };
        let against = self
            .records
            .versions
            .iter()
            .find(|v| v.product == refusal.product && v.content == refusal.content && !v.gone)
            .is_none_or(|v| self.written(v));
        by && against
    }

    /// The cells that answer for a product: on the path, declaring that they write it, and having written it.
    fn answers_for(&self, product: &ProductId) -> Vec<&'a Cell> {
        let flow: &'a Flow = self.flow;
        flow.cells()
            .iter()
            .enumerate()
            .filter(|(i, c)| self.on_path[*i] && c.writes.contains(product) && self.wrote(&c.id, product))
            .map(|(_, c)| c)
            .collect()
    }

    /// The refusals that stand, with force, against products the cell depends on and that no cell answers for.
    fn unanswered(&self, cell: &Cell) -> Vec<Waited> {
        self.records
            .refusals
            .iter()
            .filter(|r| cell.follows.contains(&r.product) && self.stands(r) && self.has_force(r))
            .filter(|r| self.answers_for(&r.product).is_empty())
            .map(|r| Waited::Unanswered {
                refusal: r.id,
                product: r.product.clone(),
                by: r.by.clone(),
            })
            .collect()
    }

    /// Why the cell needs to run; empty when its record still holds.
    fn needs(&self, i: usize) -> Vec<Cause> {
        let cell = &self.flow.cells()[i];
        let Some(record) = self.record(&cell.id) else {
            return vec![Cause::NoRecord];
        };
        let mut because = Vec::new();
        for product in &cell.writes {
            if self.lost(product) && self.wrote(&cell.id, product) {
                because.push(Cause::Gone {
                    product: product.clone(),
                });
            }
        }
        for product in &cell.follows {
            let built = self.built(record, product);
            let now = self.seen(product).map(|v| v.meaning);
            if built != now {
                because.push(Cause::Changed {
                    product: product.clone(),
                    built,
                    now,
                });
            } else if self
                .records
                .refusals
                .iter()
                .any(|r| r.by == Refuser::Run(record.started.run) && &r.product == product && !self.stands(r))
            {
                // An answer to a refusal reaches the one who refused, whatever its writer said of its meaning.
                because.push(Cause::Answered {
                    product: product.clone(),
                });
            }
        }
        for refusal in &self.records.refusals {
            if self.stands(refusal)
                && self.has_force(refusal)
                && self.answers_for(&refusal.product).iter().any(|c| c.id == cell.id)
            {
                because.push(Cause::Refused {
                    refusal: refusal.id,
                    product: refusal.product.clone(),
                    by: refusal.by.clone(),
                    why: refusal.why.clone(),
                });
            }
        }
        because
    }

    /// The wait for the owner when the cell has run, since anything before it last changed, as often as its limit
    /// allows beyond the run that change called for; or, since the owner last let it go on, as often as its limit.
    fn at_limit(&self, i: usize) -> Option<Waited> {
        let cell = &self.flow.cells()[i];
        // What is before the cell: what people, the outside or the cells before it wrote of what it depends on.
        let before = self
            .records
            .versions
            .iter()
            .filter(|v| !v.gone && cell.follows.contains(&v.product))
            .filter(|v| match v.by {
                Writer::Person | Writer::Outside => true,
                Writer::Run(id) => self
                    .runs
                    .get(&id)
                    .and_then(|r| self.flow.place(&r.started.cell))
                    .is_some_and(|place| place < i),
            })
            .map(|v| v.at)
            .max()
            .unwrap_or_default();
        let let_go = self
            .records
            .let_go
            .iter()
            .filter(|l| l.cell == cell.id)
            .map(|l| l.at)
            .max();
        // The run a change before the cell calls for is no way round; after the owner let it go on, every run is.
        let (since, free): (Seq, u32) = match let_go {
            Some(at) if at > before => (at, 0),
            _ => (before, 1),
        };
        let runs = self
            .runs
            .values()
            .filter(|r| r.started.cell == cell.id && r.finished.is_some() && r.started.at > since)
            .count() as u32;
        (runs >= cell.limit + free).then_some(Waited::Limit { runs })
    }
}
