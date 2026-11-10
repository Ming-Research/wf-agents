//! The walk: what a work does next, from the flow and its records.
//!
//! The engine keeps no place where a work stands. Each time something happens it walks the cells from the first, in
//! their order: a cell that does not apply is skipped; at the first whose preconditions do not hold, whose run waits
//! for a person, or that has reached its limit, the work waits; the first that needs to run is run; the rest are
//! passed. A work has ended when its last cell has a record or its item is cancelled.

use crate::flow::{Cell, CellId, Condition, Flow, ProductId};
use crate::records::{Outcome, Records, Refusal, Refuser, RunId, RunState, Seq, Tests, Version, Who, Writer};

/// What a work does next, with the reason as data (principle decide-purely).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decision {
    /// Nothing more is to be done.
    Ended(End),
    /// The work is paused: nothing is decided until it is let on.
    Paused,
    /// A cell is running: nothing is decided until it stops.
    Busy { cell: CellId },
    /// The cell runs.
    Run { cell: CellId, because: Vec<Cause> },
    /// The run that waited for a person has what it waited for, and goes on.
    GoOn { cell: CellId, run: RunId },
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
    /// A product it writes no longer exists.
    Gone { product: ProductId },
    /// Something it depends on changed in meaning since its record. `built` is none when the record was built on no
    /// version of it, `now` when the cell that handed it in is no longer on the work's path.
    Changed {
        product: ProductId,
        built: Option<u32>,
        now: Option<u32>,
    },
    /// What it refused was written anew by a run that was given the refusal, so it judges again.
    Answered { product: ProductId },
    /// A refusal stands against a product it wrote.
    Refused {
        product: ProductId,
        by: Refuser,
        why: String,
    },
}

/// Why a run that waits for a person no longer stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Stale {
    /// Its cell is no longer in the flow, or no longer applies.
    NoLongerApplies,
    /// Something it depends on changed in meaning since it started.
    Outdated { product: ProductId },
}

/// One thing a work waits for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Waited {
    pub what: String,
    pub who: Who,
}

/// What the work does next.
pub fn decide(flow: &Flow, records: &Records, tests: &dyn Tests) -> Decision {
    let walk = Walk { flow, records, tests };
    if records.cancelled {
        return Decision::Ended(End::Cancelled);
    }
    if flow.cells.last().is_some_and(|last| records.record(&last.id).is_some()) {
        return Decision::Ended(End::Passed);
    }
    if records.paused {
        return Decision::Paused;
    }
    // One cell of a work at a time. A run that waits for a person is not running.
    if let Some(run) = records.runs.iter().find(|r| r.state == RunState::Running) {
        return Decision::Busy { cell: run.cell.clone() };
    }
    // A waiting run that no longer stands is abandoned before anything else: its call would be answered for nothing.
    for run in records
        .runs
        .iter()
        .filter(|r| matches!(r.state, RunState::Waiting { .. }))
    {
        let stale = match flow.cell(&run.cell) {
            Some(cell) if walk.applies(cell) => cell.follows.iter().find_map(|p| {
                let now = walk.seen(p).map(|v| v.meaning);
                let built = run.basis.get(p).map(|b| b.meaning);
                (built != now).then(|| Stale::Outdated { product: p.clone() })
            }),
            _ => Some(Stale::NoLongerApplies),
        };
        if let Some(because) = stale {
            return Decision::Abandon {
                cell: run.cell.clone(),
                run: run.id,
                because,
            };
        }
    }
    for cell in &flow.cells {
        if !walk.applies(cell) {
            continue;
        }
        if let Some(run) = records.waiting(&cell.id) {
            let RunState::Waiting { call, who, answered } = &run.state else {
                unreachable!()
            };
            if *answered {
                return Decision::GoOn {
                    cell: cell.id.clone(),
                    run: run.id,
                };
            }
            let on = vec![Waited {
                what: call.clone(),
                who: who.clone(),
            }];
            return Decision::Wait {
                at: cell.id.clone(),
                on,
            };
        }
        let unmet: Vec<Waited> = cell
            .requires
            .iter()
            .filter_map(|t| match tests.test(t, records) {
                Outcome::Holds => None,
                Outcome::Waits { what, who } => Some(Waited { what, who }),
            })
            .collect();
        if !unmet.is_empty() {
            return Decision::Wait {
                at: cell.id.clone(),
                on: unmet,
            };
        }
        let because = walk.needs(cell);
        if because.is_empty() {
            continue;
        }
        if let Some(waited) = walk.at_limit(cell, &because) {
            return Decision::Wait {
                at: cell.id.clone(),
                on: vec![waited],
            };
        }
        return Decision::Run {
            cell: cell.id.clone(),
            because,
        };
    }
    // Every cell that applies was passed and the last does not apply: nothing is left to do.
    Decision::Ended(End::Passed)
}

/// One walk's view of a work.
struct Walk<'a> {
    flow: &'a Flow,
    records: &'a Records,
    tests: &'a dyn Tests,
}

impl Walk<'_> {
    /// Whether the cell applies on these records: whether it is on the work's path.
    fn applies(&self, cell: &Cell) -> bool {
        match &cell.applies {
            None => true,
            Some(Condition::Answered { by, output, is }) => self
                .records
                .record(by)
                .and_then(|r| r.answers.get(output))
                .is_some_and(|a| a == is),
            Some(Condition::Test(name)) => self.tests.test(name, self.records) == Outcome::Holds,
        }
    }

    /// Whether the cell this run belongs to is in the flow and on the work's path.
    fn on_path(&self, run: RunId) -> bool {
        self.records
            .run(run)
            .and_then(|r| self.flow.cell(&r.cell))
            .is_some_and(|c| self.applies(c))
    }

    /// The product as the cells after it see it: its latest version, unless a cell that is no longer on the path
    /// handed it in, which counts as having no record.
    fn seen(&self, product: &ProductId) -> Option<&Version> {
        let now = self.records.current(product)?;
        match now.by {
            Writer::Run(id) if !self.on_path(id) => None,
            _ => Some(now),
        }
    }

    /// Whether the refusal stood at this place in the order: the product had not been written since by a run that was
    /// given the refusal, which is one that started after it, nor by a person.
    fn stood(&self, refusal: &Refusal, at: Seq) -> bool {
        !self.records.versions.iter().any(|v| {
            v.product == refusal.product
                && v.at > refusal.at
                && v.at < at
                && match v.by {
                    Writer::Run(id) => self.records.run(id).is_some_and(|r| r.started > refusal.at),
                    Writer::Person | Writer::Outside => true,
                }
        })
    }

    /// Whether the refusal still stands.
    fn stands(&self, refusal: &Refusal) -> bool {
        self.stood(refusal, Seq::MAX)
    }

    /// Whether a refusal has force: one by a cell only while that run is the cell's record and the cell is on the path.
    fn in_force(&self, refusal: &Refusal) -> bool {
        match &refusal.by {
            Refuser::Person(_) => true,
            Refuser::Run(id) => {
                self.on_path(*id)
                    && self
                        .records
                        .run(*id)
                        .is_some_and(|by| self.records.record(&by.cell).is_some_and(|r| r.id == by.id))
            }
        }
    }

    /// Why the cell needs to run; empty when its record still holds.
    fn needs(&self, cell: &Cell) -> Vec<Cause> {
        let Some(record) = self.records.record(&cell.id) else {
            return vec![Cause::NoRecord];
        };
        let mut because = Vec::new();
        for product in &cell.writes {
            if self.records.current(product).is_some_and(|v| v.gone) {
                because.push(Cause::Gone {
                    product: product.clone(),
                });
            }
        }
        for product in &cell.follows {
            let seen = self.seen(product);
            // What the cell wrote itself never makes it need to run.
            if seen.is_some_and(|v| v.by == Writer::Run(record.id)) {
                continue;
            }
            let built = record.basis.get(product).map(|b| b.meaning);
            let now = seen.map(|v| v.meaning);
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
                .any(|r| r.by == Refuser::Run(record.id) && &r.product == product && !self.stands(r))
            {
                // An answer to a refusal reaches the one who refused, whatever its writer said of its meaning.
                because.push(Cause::Answered {
                    product: product.clone(),
                });
            }
        }
        for refusal in &self.records.refusals {
            if self.records.wrote(&cell.id, &refusal.product) && self.stands(refusal) && self.in_force(refusal) {
                because.push(Cause::Refused {
                    product: refusal.product.clone(),
                    by: refusal.by.clone(),
                    why: refusal.why.clone(),
                });
            }
        }
        because
    }

    /// The cell or the person a refusal is by.
    fn refuser(&self, by: &Refuser) -> Option<Party> {
        match by {
            Refuser::Person(who) => Some(Party::Person(who.clone())),
            Refuser::Run(id) => self.records.run(*id).map(|r| Party::Cell(r.cell.clone())),
        }
    }

    /// The wait for the owner when the cell has been run again for the refusals of one cell or person as often as its
    /// limit allows, since the owner last let it go on.
    fn at_limit(&self, cell: &Cell, because: &[Cause]) -> Option<Waited> {
        // Only a refusal makes a way round: a rerun for a change before it is bounded by the reruns before it.
        if because.iter().any(|c| !matches!(c, Cause::Refused { .. })) {
            return None;
        }
        let let_go = self
            .records
            .let_go
            .iter()
            .filter(|l| l.cell == cell.id)
            .map(|l| l.at)
            .max()
            .unwrap_or(0);
        because.iter().find_map(|c| {
            let Cause::Refused { by, .. } = c else { return None };
            let party = self.refuser(by)?;
            // The cell's runs since then that started while a refusal by this party stood against what it wrote.
            let again = self
                .records
                .runs
                .iter()
                .filter(|run| {
                    run.cell == cell.id && run.started > let_go && matches!(run.state, RunState::Finished { .. })
                })
                .filter(|run| {
                    self.records.refusals.iter().any(|r| {
                        r.at < run.started
                            && self.records.wrote(&cell.id, &r.product)
                            && self.refuser(&r.by).as_ref() == Some(&party)
                            && self.stood(r, run.started)
                    })
                })
                .count();
            (again >= cell.limit as usize).then(|| Waited {
                what: format!(
                    "cell {} was run again {again} times for refusals by {party} and one still stands: let it go on, or cancel",
                    cell.id
                ),
                who: Who::Owner,
            })
        })
    }
}

/// Who refuses, whichever of its runs it was.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Party {
    Cell(CellId),
    Person(Who),
}

impl std::fmt::Display for Party {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Party::Cell(cell) => write!(f, "cell {cell}"),
            Party::Person(Who::Owner) => f.write_str("the owner"),
            Party::Person(Who::Architect) => f.write_str("the architect"),
        }
    }
}
