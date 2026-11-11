//! `GET /api/v1/attention`: what needs whoever plans the work since a mark ([`driver::attention`]), for the command
//! `wf-agents wait`, and later for an architect's cell, as one answer to one request.
//!
//! `since` is the mark the last answer gave; with none, what stands now is given. `wait` is how many seconds at most
//! the request is held open while there is nothing: it is answered as soon as there is something, or with no line and
//! the mark to give next time once that time has passed; with none, it is answered at once. While it is held, what the
//! driver holds is read again each time something happens, from the mark the read before gave, so that what happens
//! while it is held is in its answer.
//!
//! A mark that cannot be read, or is no mark of these records, is refused with 400 and 422; a driver that stopped, and
//! an engine whose loop has ended, answer 503; a work that cannot be shown, which the engine never makes, 500.

use std::time::Duration;

use axum::extract::rejection::QueryRejection;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use driver::Driver;
use driver::attention::{self as of, Heed, Mark};
use serde::{Deserialize, Serialize};
use service::Handle;
use store::WorkNumber;
use tokio::sync::broadcast::error::RecvError;
use tokio::time::Instant;
use work::Time;

use crate::interface::{fail, refused};
use crate::works::{Broken, Now, Outage};

/// What needs the planner since the mark it gave, most pressing first.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attention {
    /// At most a fixed number of them.
    pub lines: Vec<Line>,
    /// How many more there are, which come again at the next request with `mark`.
    pub left_out: usize,
    /// The mark to give next time.
    pub mark: String,
}

/// Something for the planner: a work, as it does now, and since when it stands so, in milliseconds since the start of
/// 1970.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Line {
    /// No agent's round of any work begins: a round failed for a cause every work shares. It is about no work.
    Agents { since: u64, outage: Outage },
    /// The work's records cannot be those of a work.
    Fault { of: Of, since: u64, now: Now },
    /// The work rests for a person: what it waits for, or why the engine paused it, is what it does now; for a work
    /// that ended and could not let go of what it held, the engine's pause.
    ForAPerson { of: Of, since: u64, now: Now },
    /// The work waits for these items, none of which is going on.
    Idle {
        of: Of,
        since: u64,
        now: Now,
        items: Vec<Idle>,
    },
    /// The work ended, merged or cancelled.
    Ended { of: Of, since: u64, now: Now },
}

/// The work a line is of, with its item.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Of {
    pub work: u64,
    pub item: u64,
    pub title: String,
}

/// An item waited for that is not going on, and why.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Idle {
    pub item: u64,
    pub why: Stuck,
}

/// Why an item waited for is not going on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Stuck {
    /// The owner paused its work, or it was added paused and not let on.
    Paused,
    /// Its work rests for a person.
    ForAPerson,
    /// Its work waits in turn for items none of which is going on.
    Idle,
    Cancelled,
    /// Its work's records cannot be those of a work.
    Fault,
}

/// What the request asks.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Asked {
    pub since: Option<String>,
    /// How many seconds at most the request is held open while there is nothing.
    pub wait: Option<u64>,
}

impl Attention {
    /// The answer as the pages read it at this time, each line with its work as it does now.
    pub fn of(driver: &Driver, attention: &of::Attention, at: Time) -> Result<Attention, Broken> {
        let outage = driver.outage().map(|outage| Outage::of(&outage, driver.holds(at)));
        // The work a line is of, with its item, and what it does now.
        let of_work = |work: WorkNumber, person: bool| -> Result<(Of, Now), Broken> {
            let kept = (driver.work(work)).ok_or_else(|| Broken(format!("there is no work {}", work.0)))?;
            let view = (driver.view(work)).ok_or_else(|| Broken(format!("work {} has no view", work.0)))?;
            let of = Of {
                work: work.0,
                item: kept.item.number.0,
                title: kept.item.title.clone(),
            };
            // A work resting for the engine's pause is shown by the pause, which says why, whatever else it waits for.
            let now = if person && of::rests_for_the_engines_pause(&kept) {
                Now::paused(kept.work)?
            } else {
                Now::of(kept, &view.decision, outage.as_ref())?
            };
            Ok((of, now))
        };
        let lines = (attention.lines.iter())
            .map(|line| {
                let since = line.since.0;
                Ok(match &line.heed {
                    Heed::Agents { outage } => Line::Agents {
                        since,
                        outage: Outage::of(outage, driver.holds(at)),
                    },
                    Heed::Fault { work } => {
                        let (of, now) = of_work(*work, false)?;
                        Line::Fault { of, since, now }
                    }
                    Heed::ForAPerson { work } => {
                        let (of, now) = of_work(*work, true)?;
                        Line::ForAPerson { of, since, now }
                    }
                    Heed::Idle { work, items } => {
                        let (of, now) = of_work(*work, false)?;
                        let items = (items.iter())
                            .map(|idle| Idle {
                                item: idle.item.0,
                                why: Stuck::of(idle.why),
                            })
                            .collect();
                        Line::Idle { of, since, now, items }
                    }
                    Heed::Ended { work, .. } => {
                        let (of, now) = of_work(*work, false)?;
                        Line::Ended { of, since, now }
                    }
                })
            })
            .collect::<Result<_, Broken>>()?;
        Ok(Attention {
            lines,
            left_out: attention.left_out,
            mark: attention.mark.to_string(),
        })
    }
}

impl Stuck {
    fn of(why: of::Stuck) -> Stuck {
        match why {
            of::Stuck::Paused => Stuck::Paused,
            of::Stuck::ForAPerson => Stuck::ForAPerson,
            of::Stuck::Idle => Stuck::Idle,
            of::Stuck::Cancelled => Stuck::Cancelled,
            of::Stuck::Fault => Stuck::Fault,
        }
    }
}

/// What needs the planner since the mark, held open as the request asks while there is nothing.
pub(crate) async fn attention(State(handle): State<Handle>, asked: Result<Query<Asked>, QueryRejection>) -> Response {
    let asked = match asked {
        Ok(Query(asked)) => asked,
        Err(rejection) => {
            return fail(
                StatusCode::BAD_REQUEST,
                format!("The request cannot be read: {}.", rejection.body_text()),
            );
        }
    };
    let mut mark = match asked.since.as_deref().map(Mark::read).transpose() {
        Ok(mark) => mark,
        Err(unreadable) => return fail(StatusCode::BAD_REQUEST, format!("{}.", unreadable.0)),
    };
    let until = match asked
        .wait
        .map(|seconds| Instant::now().checked_add(Duration::from_secs(seconds)))
    {
        None => None,
        Some(Some(until)) => Some(until),
        Some(None) => {
            return fail(
                StatusCode::BAD_REQUEST,
                "The time to wait is longer than this machine's clock can count.".to_string(),
            );
        }
    };
    // Listened to before the works are read, so that nothing that happens between is missed.
    let mut changes = handle.changes();
    loop {
        let since = mark.clone();
        let read = handle.read_at(move |driver, at| {
            (driver.attention(since.as_ref()))
                .map(|attention| Attention::of(driver, &attention, at).map(|answer| (answer, attention.mark)))
        });
        let (answer, read_to) = match read.await {
            Ok(Ok(Ok(answer))) => answer,
            Ok(Ok(Err(broken))) => {
                return fail(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("It cannot be shown: {}.", broken.0),
                );
            }
            Ok(Err(refusal)) => return not_answered(&refusal),
            Err(refusal) => return refused(&refusal),
        };
        let Some(until) = until.filter(|_| answer.lines.is_empty()) else {
            return Json(answer).into_response();
        };
        // Read again from where this read left off: with no mark, a work that ends while the request is held would
        // otherwise be taken as read, as every work that ended is where no mark is given.
        mark = Some(read_to);
        match tokio::time::timeout_at(until, changes.recv()).await {
            Err(_) => return Json(answer).into_response(),
            // Something happened, or more than the listener keeps: either way the works are read again.
            Ok(Ok(_) | Err(RecvError::Lagged(_))) => {}
            Ok(Err(RecvError::Closed)) => return refused(&service::Refusal::Gone),
        }
    }
}

/// The answer to a mark that is not of these records, or to a driver that stopped.
fn not_answered(refusal: &of::Refused) -> Response {
    match refusal {
        of::Refused::NotOfTheseRecords { work } => fail(
            StatusCode::UNPROCESSABLE_ENTITY,
            format!(
                "The mark is not of these records: it names work {} as it has never been. Give no mark to read what \
                 stands now.",
                work.0
            ),
        ),
        of::Refused::Stopped(fault) => fail(
            StatusCode::SERVICE_UNAVAILABLE,
            format!("The engine stopped: {}.", service::Fault::Driver(fault.clone())),
        ),
    }
}
