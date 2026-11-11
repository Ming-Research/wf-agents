//! `GET /api/v1/attention`: what needs whoever plans the work since a mark ([`driver::attention`]), for the command
//! `wf-agents wait`, and later for an architect's cell, as one answer to one request.
//!
//! `since` is the mark the last answer gave; with none, what stands now is given. `wait` is how many seconds at most
//! the request is held open while there is nothing: it is answered as soon as there is something, or with no line and
//! the mark to give next time once that time has passed; with none, it is answered at once. While it is held, what the
//! driver holds is read again each time something happens, and the mark it was asked with is kept, so that what
//! happens while it is held is in its answer.
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
use tokio::sync::broadcast::error::RecvError;
use tokio::time::Instant;

use crate::interface::{fail, refused};
use crate::works::{Broken, Now};

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
    /// The work's records cannot be those of a work.
    Fault { of: Of, since: u64, now: Now },
    /// The work rests for a person: what it waits for, or why the engine paused it, is what it does now.
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
    /// The answer as the pages read it, each line with its work as it does now.
    pub fn of(driver: &Driver, attention: &of::Attention) -> Result<Attention, Broken> {
        let lines = (attention.lines.iter())
            .map(|line| {
                let work = match &line.heed {
                    Heed::Fault { work }
                    | Heed::ForAPerson { work }
                    | Heed::Idle { work, .. }
                    | Heed::Ended { work, .. } => *work,
                };
                let kept = (driver.work(work)).ok_or_else(|| Broken(format!("there is no work {}", work.0)))?;
                let view = (driver.view(work)).ok_or_else(|| Broken(format!("work {} has no view", work.0)))?;
                let (of, since, now) = (
                    Of {
                        work: work.0,
                        item: kept.item.number.0,
                        title: kept.item.title.clone(),
                    },
                    line.since.0,
                    Now::of(kept, &view.decision)?,
                );
                Ok(match &line.heed {
                    Heed::Fault { .. } => Line::Fault { of, since, now },
                    Heed::ForAPerson { .. } => Line::ForAPerson { of, since, now },
                    Heed::Idle { items, .. } => Line::Idle {
                        of,
                        since,
                        now,
                        items: (items.iter())
                            .map(|idle| Idle {
                                item: idle.item.0,
                                why: Stuck::of(idle.why),
                            })
                            .collect(),
                    },
                    Heed::Ended { .. } => Line::Ended { of, since, now },
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
    let mark = match asked.since.as_deref().map(Mark::read).transpose() {
        Ok(mark) => mark,
        Err(unreadable) => return fail(StatusCode::BAD_REQUEST, format!("{}.", unreadable.0)),
    };
    let until = asked.wait.map(|seconds| Instant::now() + Duration::from_secs(seconds));
    // Listened to before the works are read, so that nothing that happens between is missed.
    let mut changes = handle.changes();
    loop {
        let since = mark.clone();
        let read = handle
            .read(move |driver| (driver.attention(since.as_ref())).map(|attention| Attention::of(driver, &attention)));
        let answer = match read.await {
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
