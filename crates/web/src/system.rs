//! `GET /api/v1/system`: how the whole system went over a span of time, for the System view: what was produced (works
//! merged and cancelled, items added), what was spent and on what (by week, by cell, by model, by work), and what was
//! wasted or went wrong (rounds sent back, rounds failed). What goes on now the view reads from the items and the
//! engine, as the bar and the works do.
//!
//! The span is the weeks the page gives, `weeks`: when each begins, in milliseconds since 1970, the earliest first. The
//! page cuts them at Monday's midnight in the owner's time zone, which knows when its clocks change and the engine does
//! not. The span begins with the first and lasts until now; with no week given, it is all time, not cut into weeks. A
//! round counts in the span, and in the week, where it ended: what it took is known only then. Everything is worked out
//! from the works' records as the driver holds them, on the engine's loop, which alone reads the clock.

use std::collections::BTreeMap;

use axum::extract::rejection::QueryRejection;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use decide::{Decision, End};
use driver::{Driver, Kept};
use serde::{Deserialize, Serialize};
use service::Handle;
use work::Time;

use crate::interface::{fail, refused};
use crate::works::{Back, Broken, Failure, Given, Run, RunEnd, Spent, runs, time};

/// What the view asks for: when each week of the span begins, joined by commas; none for all time.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Span {
    pub weeks: Option<String>,
}

impl Span {
    /// When each week begins, the earliest first; or why the weeks cannot be read.
    pub fn starts(&self) -> Result<Vec<u64>, String> {
        let Some(weeks) = &self.weeks else {
            return Ok(Vec::new());
        };
        let mut starts = (weeks.split(','))
            .map(|start| (start.trim().parse::<u64>()).map_err(|_| format!("{start:?} is no time in milliseconds")))
            .collect::<Result<Vec<u64>, String>>()?;
        starts.sort_unstable();
        starts.dedup();
        Ok(starts)
    }
}

/// How the system went over the span.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct System {
    /// The engine's time as it worked this out.
    pub now: u64,
    /// When the span begins: its first week; none for all time.
    pub from: Option<u64>,
    /// What every round that ended in the span took.
    pub spent: Spent,
    /// Week by week, the latest first, each week the span was given; a week with nothing in it is there with nothing.
    pub weeks: Vec<Week>,
    /// What the span's rounds took, by the cell of the flow they ran, the costliest first.
    pub by_cell: Vec<CellSpent>,
    /// By the model a round ran on, the costliest first: an agent's round's model, the engine's own acts, which run on
    /// none, and an agent's round whose model was not recorded.
    pub by_model: Vec<ModelSpent>,
    /// By work, the costliest first: each work a round of which ended in the span.
    pub by_work: Vec<WorkSpent>,
    /// The works merged in the span, the latest first.
    pub merged: Vec<Merged>,
    /// The works whose items were cancelled in the span, the latest first.
    pub cancelled: Vec<Cancelled>,
    /// The items added in the span, the latest first.
    pub added: Vec<Added>,
    /// The rounds of the span given back what the round before handed in, the latest first.
    pub sent_back: Vec<RoundAt>,
    /// The rounds of the span that failed, the latest first.
    pub failed: Vec<FailedRound>,
}

/// A week: what its rounds took, and how many of each thing that is counted happened in it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Week {
    /// When it begins, as the page gave it.
    pub start: u64,
    pub spent: Spent,
    pub merged: usize,
    pub cancelled: usize,
    pub added: usize,
    pub sent_back: usize,
    pub failed: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CellSpent {
    pub flow: String,
    pub cell: String,
    pub spent: Spent,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelSpent {
    pub on: RanOn,
    pub spent: Spent,
}

/// What a round ran on.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum RanOn {
    /// An agent's round, on this model, as its last try recorded it.
    Model { model: String },
    /// A round of the engine's own act, which runs on no model.
    Act,
    /// An agent's round kept before the tries of a round were recorded, whose model is not known.
    NotRecorded,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkSpent {
    pub work: u64,
    pub item: u64,
    pub spent: Spent,
}

/// A work merged: when, when its first round began, and what all its rounds took.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Merged {
    pub work: u64,
    pub item: u64,
    pub at: u64,
    pub from: Option<u64>,
    pub spent: Spent,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Cancelled {
    pub work: u64,
    pub item: u64,
    pub at: u64,
    pub why: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Added {
    pub work: u64,
    pub item: u64,
    pub at: u64,
}

/// A round of a work: of this run of this cell, which began then; what it took, none while it is under way.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoundAt {
    pub work: u64,
    pub item: u64,
    pub cell: String,
    pub run: u64,
    pub round: usize,
    pub began: u64,
    pub spent: Option<Spent>,
}

/// A round that failed: when it ended, of what kind of cause, and why, as what carried it out said.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FailedRound {
    pub work: u64,
    pub item: u64,
    pub cell: String,
    pub run: u64,
    pub round: usize,
    pub ended: u64,
    pub failure: Failure,
    pub why: String,
    pub spent: Spent,
}

impl System {
    /// How the system went over these weeks, the earliest first, until now; over all time where there are none.
    pub fn of(driver: &Driver, now: Time, weeks: &[u64]) -> Result<System, Broken> {
        let from = weeks.first().copied();
        let within = |at: u64| from.is_none_or(|from| at >= from);
        let mut system = System {
            now: now.0,
            from,
            spent: Spent::default(),
            weeks: Vec::new(),
            by_cell: Vec::new(),
            by_model: Vec::new(),
            by_work: Vec::new(),
            merged: Vec::new(),
            cancelled: Vec::new(),
            added: Vec::new(),
            sent_back: Vec::new(),
            failed: Vec::new(),
        };
        let mut counted: BTreeMap<u64, Week> = (weeks.iter())
            .map(|&start| {
                (
                    start,
                    Week {
                        start,
                        ..Week::default()
                    },
                )
            })
            .collect();
        let mut by_cell: BTreeMap<(String, String), Spent> = BTreeMap::new();
        let mut by_model: BTreeMap<RanOn, Spent> = BTreeMap::new();
        for kept in driver.works() {
            let (work, item) = (kept.head.number.0, kept.item.number.0);
            let runs = runs(kept)?;
            let models = models(kept);
            if within(kept.item.added.0) {
                system.added.push(Added {
                    work,
                    item,
                    at: kept.item.added.0,
                });
                if let Some(week) = week_of(&mut counted, kept.item.added.0) {
                    week.added += 1;
                }
            }
            let mut of_work = Spent::default();
            for run in &runs {
                for round in &run.rounds {
                    if let Given::SentBack { .. } = round.given
                        && within(round.began)
                    {
                        system.sent_back.push(RoundAt {
                            work,
                            item,
                            cell: run.cell.clone(),
                            run: run.run,
                            round: round.number,
                            began: round.began,
                            spent: round.spent,
                        });
                        if let Some(week) = week_of(&mut counted, round.began) {
                            week.sent_back += 1;
                        }
                    }
                    let (Some(ended), Some(spent)) = (round.ended, round.spent) else {
                        continue;
                    };
                    if !within(ended) {
                        continue;
                    }
                    system.spent = system.spent.with(spent);
                    of_work = of_work.with(spent);
                    if let Some(week) = week_of(&mut counted, ended) {
                        week.spent = week.spent.with(spent);
                    }
                    let cell = by_cell.entry((kept.head.flow.clone(), run.cell.clone())).or_default();
                    *cell = cell.with(spent);
                    let act = (kept.flow.cells.iter())
                        .find(|spec| spec.id.as_str() == run.cell)
                        .is_some_and(|spec| spec.act.is_some());
                    let on = match models.get(&(run.run, round.number)) {
                        Some(model) => RanOn::Model { model: model.clone() },
                        None if act => RanOn::Act,
                        None => RanOn::NotRecorded,
                    };
                    let model = by_model.entry(on).or_default();
                    *model = model.with(spent);
                    if let Some(Back::Failed { why, failure, .. }) = &round.back {
                        system.failed.push(FailedRound {
                            work,
                            item,
                            cell: run.cell.clone(),
                            run: run.run,
                            round: round.number,
                            ended,
                            failure: *failure,
                            why: why.clone(),
                            spent,
                        });
                        if let Some(week) = week_of(&mut counted, ended) {
                            week.failed += 1;
                        }
                    }
                }
            }
            if of_work.rounds > 0 {
                system.by_work.push(WorkSpent {
                    work,
                    item,
                    spent: of_work,
                });
            }
            match ended(driver, kept, &runs)? {
                Some(Ending::Merged(at)) if within(at) => {
                    system.merged.push(Merged {
                        work,
                        item,
                        at,
                        from: runs.iter().flat_map(|run| &run.rounds).map(|round| round.began).min(),
                        spent: (runs.iter()).fold(Spent::default(), |sum, run| sum.with(run.spent)),
                    });
                    if let Some(week) = week_of(&mut counted, at) {
                        week.merged += 1;
                    }
                }
                Some(Ending::Cancelled(at)) if within(at) => {
                    system.cancelled.push(Cancelled {
                        work,
                        item,
                        at,
                        why: kept.work.why_cancelled.clone(),
                    });
                    if let Some(week) = week_of(&mut counted, at) {
                        week.cancelled += 1;
                    }
                }
                _ => {}
            }
        }
        system.weeks = counted.into_values().rev().collect();
        system.by_cell = (by_cell.into_iter())
            .map(|((flow, cell), spent)| CellSpent { flow, cell, spent })
            .collect();
        system.by_cell.sort_by_key(|c| std::cmp::Reverse(c.spent.usd_micros));
        system.by_model = (by_model.into_iter())
            .map(|(on, spent)| ModelSpent { on, spent })
            .collect();
        system.by_model.sort_by_key(|m| std::cmp::Reverse(m.spent.usd_micros));
        system.by_work.sort_by_key(|w| std::cmp::Reverse(w.spent.usd_micros));
        system.merged.sort_by_key(|x| std::cmp::Reverse(x.at));
        system.cancelled.sort_by_key(|x| std::cmp::Reverse(x.at));
        system.added.sort_by_key(|x| std::cmp::Reverse(x.at));
        system.sent_back.sort_by_key(|x| std::cmp::Reverse(x.began));
        system.failed.sort_by_key(|x| std::cmp::Reverse(x.ended));
        Ok(system)
    }
}

/// The week a time falls in, where the span is cut into weeks: the latest that begins at it or before.
fn week_of(weeks: &mut BTreeMap<u64, Week>, at: u64) -> Option<&mut Week> {
    weeks.range_mut(..=at).next_back().map(|(_, week)| week)
}

/// How a work ended, and when.
enum Ending {
    Merged(u64),
    Cancelled(u64),
}

/// How the work ended, where it has: merged when its last cell, the merge, finished, the latest run to finish; or
/// cancelled when its item was.
fn ended(driver: &Driver, kept: Kept<'_>, runs: &[Run]) -> Result<Option<Ending>, Broken> {
    let Some(view) = driver.view(kept.head.number) else {
        return Err(Broken(format!(
            "the driver shows work {} and has no view of it",
            kept.head.number.0
        )));
    };
    Ok(match view.decision {
        Decision::Ended(End::Merged) => (runs.iter())
            .filter_map(|run| match run.end {
                Some(RunEnd::Finished { time, .. }) => Some(time),
                _ => None,
            })
            .max()
            .map(Ending::Merged),
        Decision::Ended(End::Cancelled) => match kept.work.records.cancelled {
            Some(at) => Some(Ending::Cancelled(time(kept.work, at)?)),
            None => None,
        },
        _ => None,
    })
}

/// The model each round of an agent's ran on: its last try's.
fn models(kept: Kept<'_>) -> BTreeMap<(u64, usize), String> {
    (kept.work.ran.iter())
        .map(|ran| ((ran.run.0, ran.round), ran.on.model.clone()))
        .collect()
}

/// Answers `GET /api/v1/system`.
pub(crate) async fn system(State(handle): State<Handle>, span: Result<Query<Span>, QueryRejection>) -> Response {
    let weeks = match span
        .map_err(|rejection| rejection.body_text())
        .and_then(|Query(span)| span.starts())
    {
        Ok(weeks) => weeks,
        Err(why) => return fail(StatusCode::BAD_REQUEST, format!("The span cannot be read: {why}.")),
    };
    match handle.read_at(move |driver, now| System::of(driver, now, &weeks)).await {
        Ok(Ok(system)) => Json(system).into_response(),
        Ok(Err(broken)) => fail(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("The system cannot be shown: {}.", broken.0),
        ),
        Err(refusal) => refused(&refusal),
    }
}
