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
use decide::{Holder, View};
use driver::Driver;
use driver::attention::{self as of, Heed, Mark};
use serde::{Deserialize, Serialize};
use service::{Changed, Handle};
use store::WorkNumber;
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;
use tokio::time::Instant;

use crate::interface::{fail, refused};
use crate::system::RedTest;
use crate::works::{Broken, Now, Others, Outage, Outages, Pauser, Wait};

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
    /// Everything was paused, at this time, by the owner or by the engine as it started with works going on: no work
    /// begins a round but to answer a person's words to its cell, until the owner lifts it. It is about no work.
    Everything { since: u64, by: Pauser },
    /// No agent's round of any work begins: a round failed for a cause every work shares. It is about no work. Where
    /// the cause is the agents' command line, which cannot run until the owner mends it, what the owner does, in the
    /// words beside the flow of the work whose round met it (`owner.mend`), said here alone.
    Agents {
        since: u64,
        outage: Outage,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mend: Option<String>,
    },
    /// The main branch of the project is red on these tests, which the check before a merge found failing on its head,
    /// since the time its red began; these works wait for it to pass. It is about no work.
    MainRed {
        since: u64,
        project: String,
        tests: Vec<RedTest>,
        works: Vec<Of>,
    },
    /// The work's records cannot be those of a work.
    Fault { of: Of, since: u64, now: Now },
    /// The work waits on the owner, for each of these, as the one definition of that says.
    ForAPerson {
        of: Of,
        since: u64,
        now: Now,
        waits: Vec<Wait>,
    },
    /// The work is stalled: no cell will write what it waits for, and nothing pauses it.
    Stalled { of: Of, since: u64, now: Now },
    /// The work waits for these items, none of which is going on.
    Idle {
        of: Of,
        since: u64,
        now: Now,
        items: Vec<Idle>,
    },
    /// The owner's pause of the work waited for these items to merge or be cancelled, and each has: the work is paused
    /// still, and only the owner lets it on.
    PauseOver { of: Of, since: u64, items: Vec<u64> },
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
    /// Its work waits on the owner.
    ForAPerson,
    /// Its work is stalled.
    Stalled,
    /// Its work waits in turn for items none of which is going on.
    Idle,
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
        let at_once = driver.at_once();
        let outages = Outages::of(&at_once);
        // The work a line is of, with its item, and what it does now; with its view and what holds back its next
        // round, which what it waits on the owner for reads.
        let of_work = |work: WorkNumber| -> Result<(Of, Now, View, Option<Holder>), Broken> {
            let kept = (driver.work(work)).ok_or_else(|| Broken(format!("there is no work {work}")))?;
            let view = (driver.view_at(work, &at_once)).ok_or_else(|| Broken(format!("{work} has no view")))?;
            let held = driver.held_at(work, &at_once);
            let of = Of {
                work: work.0,
                item: kept.item.number.0,
                title: kept.item.title.clone(),
            };
            let now = Now::of(
                kept,
                &view.decision,
                held,
                &outages,
                &driver.along_of(work),
                &Others::of(driver, work, &at_once),
            )?;
            Ok((of, now, view, held))
        };
        let lines = (attention.lines.iter())
            .map(|line| {
                let since = line.since.0;
                Ok(match &line.heed {
                    Heed::Everything { by } => Line::Everything {
                        since,
                        by: Pauser::of(*by),
                    },
                    Heed::Agents { outage } => Line::Agents {
                        since,
                        outage: Outage::of(outage, at_once.gate().holds_agents()),
                        mend: (outage.down().0 == decide::run::Failure::CannotRun)
                            .then(|| driver.work(outage.work))
                            .flatten()
                            .map(|kept| kept.flow.waits.owner.mend.clone()),
                    },
                    Heed::MainRed { project, tests, works } => {
                        let works = (works.iter())
                            .map(|work| {
                                let kept =
                                    (driver.work(*work)).ok_or_else(|| Broken(format!("there is no work {work}")))?;
                                Ok(Of {
                                    work: work.0,
                                    item: kept.item.number.0,
                                    title: kept.item.title.clone(),
                                })
                            })
                            .collect::<Result<Vec<_>, Broken>>()?;
                        Line::MainRed {
                            since,
                            project: project.clone(),
                            tests: tests.iter().map(RedTest::of).collect(),
                            works,
                        }
                    }
                    Heed::Fault { work } => {
                        let (of, now, ..) = of_work(*work)?;
                        Line::Fault { of, since, now }
                    }
                    Heed::ForAPerson { work } => {
                        let (of, now, view, held) = of_work(*work)?;
                        let waits = (driver.waits_on_owner_at(*work, &view, held, &at_once).into_iter())
                            .map(Wait::of)
                            .collect();
                        Line::ForAPerson { of, since, now, waits }
                    }
                    Heed::Stalled { work } => {
                        let (of, now, ..) = of_work(*work)?;
                        Line::Stalled { of, since, now }
                    }
                    Heed::Idle { work, items } => {
                        let (of, now, ..) = of_work(*work)?;
                        let items = (items.iter())
                            .map(|idle| Idle {
                                item: idle.item.0,
                                why: Stuck::of(idle.why),
                            })
                            .collect();
                        Line::Idle { of, since, now, items }
                    }
                    Heed::PauseOver { work, items } => {
                        let (of, ..) = of_work(*work)?;
                        let items = items.iter().map(|item| item.0).collect();
                        Line::PauseOver { of, since, items }
                    }
                    Heed::Ended { work, .. } => {
                        let (of, now, ..) = of_work(*work)?;
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
            of::Stuck::Stalled => Stuck::Stalled,
            of::Stuck::Idle => Stuck::Idle,
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
        let read = handle.read(move |driver| {
            (driver.attention(since.as_ref()))
                .map(|attention| Attention::of(driver, &attention).map(|answer| (answer, attention.mark)))
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
        match woken(&mut changes, until).await {
            Woken::TimedOut => return Json(answer).into_response(),
            Woken::Changed => {}
            Woken::Gone => return refused(&service::Refusal::Gone),
        }
    }
}

/// How the wait of a held request ended.
#[derive(Debug, PartialEq, Eq)]
enum Woken {
    /// The time asked for passed.
    TimedOut,
    /// Something that the works are read again for happened, or more than the listener keeps.
    Changed,
    /// The engine's loop ended.
    Gone,
}

/// Waits until `until` for a change the works are read again for. What the rounds under way have come to is no such
/// change: it comes at every call of every round, and nothing a request is answered with depends on it, so each such
/// notice would make the engine's loop read every work for nothing.
async fn woken(changes: &mut broadcast::Receiver<Changed>, until: Instant) -> Woken {
    loop {
        match tokio::time::timeout_at(until, changes.recv()).await {
            Err(_) => return Woken::TimedOut,
            Ok(Ok(changed)) if changed.only_going() => {}
            Ok(Ok(_) | Err(RecvError::Lagged(_))) => return Woken::Changed,
            Ok(Err(RecvError::Closed)) => return Woken::Gone,
        }
    }
}

/// The answer to a mark that is not of these records, or to a driver that stopped.
fn not_answered(refusal: &of::Refused) -> Response {
    match refusal {
        of::Refused::NotOfTheseRecords { work } => fail(
            StatusCode::UNPROCESSABLE_ENTITY,
            format!(
                "The mark is not of these records: it names {} as it has never been. Give no mark to read what \
                 stands now.",
                work
            ),
        ),
        of::Refused::Stopped(fault) => fail(
            StatusCode::SERVICE_UNAVAILABLE,
            format!("The engine stopped: {}.", service::Fault::Driver(fault.clone())),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn said(going: bool, works: Vec<WorkNumber>, items: bool) -> Changed {
        Changed {
            works,
            items,
            copies: false,
            accounts: false,
            observations: false,
            going,
        }
    }

    // A red main is a line of its own kind, about no work, which names the tests, the head and the works that wait, and is
    // read back as it was said. Milliseconds; the one test of this line's wire shape.
    #[test]
    fn a_red_main_is_a_line_of_its_own_kind_that_is_read_back_as_it_was_said() {
        let line = Line::MainRed {
            since: 1_700_000_000_000,
            project: "Firn-wf".to_string(),
            tests: vec![RedTest {
                unit: "carry".to_string(),
                name: "tools::tests::b".to_string(),
                head: "9f8e7d6c".to_string(),
            }],
            works: vec![Of {
                work: 184,
                item: 90,
                title: "Greet".to_string(),
            }],
        };
        let said = serde_json::to_value(&line).expect("it is said");
        assert_eq!(
            said,
            serde_json::json!({
                "kind": "mainRed",
                "since": 1_700_000_000_000u64,
                "project": "Firn-wf",
                "tests": [{ "unit": "carry", "name": "tools::tests::b", "head": "9f8e7d6c" }],
                "works": [{ "work": 184, "item": 90, "title": "Greet" }],
            })
        );
        assert_eq!(serde_json::from_value::<Line>(said).expect("it is read"), line);
    }

    // A held request is woken by a change that names a work or sets another flag, also where `going` is set with it,
    // and by none that says only what the rounds under way have come to, which comes at every call. Fails where such a
    // notice wakes the wait, which reads every work in the engine's loop for nothing. A paused clock, nothing run.
    #[tokio::test(start_paused = true)]
    async fn a_notice_of_only_what_the_rounds_under_way_have_come_to_wakes_no_held_request() {
        let wait = |seconds| Instant::now() + Duration::from_secs(seconds);

        let (send, mut changes) = broadcast::channel(16);
        for _ in 0..3 {
            send.send(said(true, Vec::new(), false)).unwrap();
        }
        assert_eq!(woken(&mut changes, wait(5)).await, Woken::TimedOut);

        // What comes after the notices of figures still wakes it, whatever it names.
        for (after, what) in [
            (said(false, vec![WorkNumber(1)], false), "a work"),
            (said(true, vec![WorkNumber(1)], false), "a work, with figures"),
            (said(false, Vec::new(), true), "an item"),
            (said(true, Vec::new(), true), "an item, with figures"),
        ] {
            send.send(said(true, Vec::new(), false)).unwrap();
            send.send(after).unwrap();
            assert_eq!(woken(&mut changes, wait(600)).await, Woken::Changed, "{what}");
        }

        drop(send);
        assert_eq!(woken(&mut changes, wait(600)).await, Woken::Gone);
    }
}
