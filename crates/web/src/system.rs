//! `GET /api/v1/system`: how the whole system went over a span of time, for the System view: what was produced (works
//! merged and cancelled, items added), what was spent and on what (by week, by project, by cell, by model, by work),
//! what was wasted or went wrong (rounds sent back, rounds failed), and how well it ran: the rounds and the calls of
//! tools that went past the lines of their history, what is under way now against its own (crate `driver`, module
//! `judged`), and how often the budget of each project's fast workflow kept tests from their end, day by day. What else
//! goes on now the view reads from the items and the engine, as the bar and the works do.
//!
//! `GET /api/v1/system/under-way` gives, apart from it and light to read, what the calls of each agent's round under way
//! have come to so far against the lines the run guard judges the round by ([`Going`]): it changes with every call, which
//! no record of a work holds, and the page reads it each time the engine's stream of changes says `going`.
//!
//! The span is the weeks the page gives, `weeks`: when each begins, in milliseconds since 1970, the earliest first. The
//! page cuts them at Monday's midnight in the owner's time zone, which knows when its clocks change and the engine does
//! not. The span begins with the first and lasts until now; with no week given, it is all time, not cut into weeks. A
//! round counts in the span, and in the week, where it ended: what it took is known only then. Everything is worked out
//! from the works' records as the driver holds them, on the engine's loop, which alone reads the clock: what each work
//! adds is worked out as the work changes and kept between reads ([`Parts`]), and a read puts the kept parts together
//! for its span with what the driver keeps of the judgments and the merged checks.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};

use axum::extract::rejection::QueryRejection;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use decide::{CallId, CellId, Decision, End, RunId};
use driver::{Driver, Kept};
use serde::{Deserialize, Serialize};
use service::Handle;
use store::WorkNumber;
use work::Time;

use crate::interface::{fail, refused};
use crate::works::{
    Against, Back, Beside, Broken, Call, Failure, Given, Judged, Line, Run, RunEnd, Spent, runs, time, viewed_at,
};

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
    /// What every round that ended in the span took, and every keep-warm ping and fork: `rounds` counts the rounds alone.
    pub spent: Spent,
    /// What the keep-warm pings that ended in the span took, each counted as a round.
    #[serde(default)]
    pub pinged: Spent,
    /// Week by week, the latest first, each week the span was given; a week with nothing in it is there with nothing.
    pub weeks: Vec<Week>,
    /// By the project of the item whose work it was, the costliest first: what each work's rounds, keep-warm pings and
    /// forks that ended in the span took, so that the projects together took what the span did.
    #[serde(default)]
    pub by_project: Vec<ProjectSpent>,
    /// What the span's rounds took, by the cell of the flow they ran, the costliest first.
    pub by_cell: Vec<CellSpent>,
    /// By the model a round ran on, the costliest first: an agent's round's model, the engine's own acts, which run on
    /// none, and an agent's round whose model was not recorded.
    pub by_model: Vec<ModelSpent>,
    /// By work, the costliest first: each work a round or a keep-warm ping of which ended in the span.
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
    /// What ended in the span past the line of its history, the latest first: a round whose cost or time went past
    /// its line, and a call of a tool whose time or whose result's size did.
    #[serde(default)]
    pub past: Vec<Past>,
    /// The rounds and the calls of tools under way now, the oldest first, each with how long it has taken so far
    /// against its line.
    #[serde(default)]
    pub under_way: Vec<UnderWay>,
    /// What the merged checks of each project say of how long its tests and jobs took: the tests that got slower, the
    /// CI series that grew, and the latest merges each with what its check took and what it added.
    #[serde(default)]
    pub tests: Vec<TestTimes>,
    /// The tries of the span whose program left processes running in its group as it ended, which were stopped, the
    /// latest first: what they did no one read, and what they took is counted nowhere.
    #[serde(default)]
    pub left_behind: Vec<LeftBehind>,
    /// How often the budget of each project's fast workflow kept the tests of a call of `run_tests` from their end, by
    /// project and by day, the latest day first: only the days of the span on which it did.
    #[serde(default)]
    pub short_of_time: Vec<ShortDay>,
    /// The stop the engine was asked for, while it stands, with how long it is expected to take to end.
    #[serde(default)]
    pub stopping: Option<Stop>,
    /// Each project whose main is red on a test, as the checks the engine ran on main's head say, the projects in the
    /// order of their names.
    #[serde(default)]
    pub red_main: Vec<RedMain>,
}

/// The main branch of a project is red: its tests that fail on its head, since when, and the works that wait for it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedMain {
    pub project: String,
    /// When the earliest failure of the tests that are red now began, in milliseconds since the start of 1970.
    pub since: u64,
    pub tests: Vec<RedTest>,
    /// The works of the project that have not ended and wait for main to pass, by number.
    pub waiting: Vec<u64>,
}

/// A test that fails on the head of main: its unit and name, and the head it failed on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedTest {
    pub unit: String,
    pub name: String,
    pub head: String,
}

impl RedTest {
    pub(crate) fn of(test: &driver::red_main::RedTest) -> RedTest {
        RedTest {
            unit: test.unit.clone(),
            name: test.name.clone(),
            head: test.head.clone(),
        }
    }
}

impl RedMain {
    fn of(red: &driver::red_main::Red) -> RedMain {
        RedMain {
            project: red.project.clone(),
            since: red.since.0,
            tests: red.tests.iter().map(RedTest::of).collect(),
            waiting: red.waiting.iter().map(|work| work.0).collect(),
        }
    }
}

/// A stop of the engine that stands: since when it was asked for, whether at once, and how much longer the engine is
/// expected to run: the longest rest of the rounds under way, which it ends after; nothing where none is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stop {
    pub since: u64,
    pub at_once: bool,
    pub rest: Rest,
}

/// How much longer something under way is expected to take, by its history ([`driver::judged::Rest`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Rest {
    /// About this many milliseconds more.
    Expected { ms: u64 },
    /// It has taken longer than anything of its history.
    Longer {},
}

impl Rest {
    fn of(rest: driver::judged::Rest) -> Rest {
        match rest {
            driver::judged::Rest::Expected(ms) => Rest::Expected { ms: ms.round() as u64 },
            driver::judged::Rest::Longer => Rest::Longer {},
        }
    }

    /// The later of two rests: longer than any before where either is.
    fn latest(self, other: Rest) -> Rest {
        match (self, other) {
            (Rest::Expected { ms: a }, Rest::Expected { ms: b }) => Rest::Expected { ms: a.max(b) },
            _ => Rest::Longer {},
        }
    }
}

/// How much longer the rounds of what is under way are expected to take, all of them: the latest of their rests, a
/// round with none taken as longer than any before, and none to come where no round is under way. A stop that waits
/// for them ends then.
pub fn rest_of_rounds(under_way: &[UnderWay]) -> Rest {
    (under_way.iter())
        .filter_map(|under_way| match under_way {
            UnderWay::Round { rest, .. } => Some(rest.unwrap_or(Rest::Longer {})),
            UnderWay::Call { .. } => None,
        })
        .fold(Rest::Expected { ms: 0 }, Rest::latest)
}

/// What the merged checks of a project say of the time of its tests and jobs (crate `driver`, module `timed`): each
/// work named is the one whose merge the thing was measured at.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestTimes {
    pub project: Option<String>,
    /// How many merged checks the project has: a judgment needs [`driver::history::HISTORY_SAMPLES`] of them, and the
    /// growth of the CI twice as many.
    pub checks: usize,
    /// The tests that took longer than their own history says in the latest merged check and in the one before.
    pub slowed: Vec<SlowedTest>,
    /// The series of the CI's times that grew.
    pub grown: Vec<Grown>,
    /// The latest merges, the earliest first.
    pub merges: Vec<MergeTimes>,
}

/// A test that got slower.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlowedTest {
    pub work: u64,
    pub unit: String,
    pub name: String,
    pub millis: u64,
    /// The median of its earlier times and the line they give, in milliseconds.
    pub median: u64,
    pub line: u64,
}

/// A series of the checks' times that grew: `of` is `tests`, the sum of the tests' times, or the name of a job.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Grown {
    pub of: String,
    /// The work whose merge the series stepped at.
    pub work: u64,
    /// The median of the `n` values before the latest `n`, and of the latest `n`, in milliseconds.
    pub before: u64,
    pub after: u64,
    pub above: usize,
    pub n: usize,
    pub needed: usize,
}

/// What a merged check took: each job, the tests in all, and the tests it added to the check before it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeTimes {
    pub work: u64,
    pub jobs: Vec<JobTime>,
    pub tests_millis: u64,
    pub added: usize,
    pub added_millis: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobTime {
    pub name: String,
    pub millis: u64,
}

impl TestTimes {
    /// What the driver worked out of a project's merged checks, as the system says it.
    fn of(tested: driver::timed::Tested) -> TestTimes {
        TestTimes {
            project: tested.project,
            checks: tested.checks,
            slowed: (tested.slowed.into_iter())
                .map(|s| SlowedTest {
                    work: s.work.0,
                    unit: s.unit,
                    name: s.name,
                    millis: s.millis,
                    median: s.median.round() as u64,
                    line: s.line.round() as u64,
                })
                .collect(),
            grown: (tested.growth.into_iter())
                .map(|g| Grown {
                    of: g.of,
                    work: g.work.0,
                    before: g.before.round() as u64,
                    after: g.after.round() as u64,
                    above: g.above,
                    n: g.n,
                    needed: g.needed,
                })
                .collect(),
            merges: (tested.series.into_iter())
                .map(|row| MergeTimes {
                    work: row.work.0,
                    jobs: (row.jobs.into_iter())
                        .map(|(name, millis)| JobTime { name, millis })
                        .collect(),
                    tests_millis: row.tests_millis,
                    added: row.added,
                    added_millis: row.added_millis,
                })
                .collect(),
        }
    }
}

/// A week: what its rounds took, and how many of each thing that is counted happened in it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Week {
    /// When it begins, as the page gave it.
    pub start: u64,
    /// What its rounds, its keep-warm pings and its forks took: `rounds` counts the rounds alone.
    pub spent: Spent,
    /// What the keep-warm pings that ended in it took, each counted as a round.
    #[serde(default)]
    pub pinged: Spent,
    pub merged: usize,
    pub cancelled: usize,
    pub added: usize,
    pub sent_back: usize,
    pub failed: usize,
}

/// What the works of one project's items took: the project by the name the installation's settings give it, none for
/// the items kept before items had projects.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSpent {
    pub project: Option<String>,
    /// What their rounds, keep-warm pings and forks that ended in the span took: `rounds` counts the rounds alone.
    pub spent: Spent,
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
    /// A round with no model recorded, not known to be an act's: an agent's round kept before the tries of a round were
    /// recorded, or a round of a cell the version its work is on does not tell, of a work on none kept among them.
    NotRecorded,
}

impl RanOn {
    /// What a round of a run of this cell of the work ran on: the model its last try recorded, where one did; else the
    /// engine's act, where the version the work is on has the cell as an engine's. A cell that version has not, or any
    /// of a work on none kept, is not known to be an act's, and its round is not recorded.
    fn of(kept: Kept<'_>, cell: &str, model: Option<&String>) -> RanOn {
        match model {
            Some(model) => RanOn::Model { model: model.clone() },
            None if (kept.cell(&CellId::from(cell))).is_some_and(|cell| !cell.agent) => RanOn::Act,
            None => RanOn::NotRecorded,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkSpent {
    pub work: u64,
    pub item: u64,
    /// What its rounds and keep-warm pings that ended in the span took: `rounds` counts the rounds alone.
    pub spent: Spent,
}

/// A work merged: when, when its first round began, and what all its rounds and keep-warm pings took.
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

/// A program run for a work that left processes running in its group as it ended, which were stopped then: which
/// program, and how many.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LeftBehind {
    pub work: u64,
    pub item: u64,
    pub of: LeftOf,
    pub at: u64,
    pub processes: usize,
}

/// One project's calls of `run_tests` on one day whose tests the budget of its fast workflow kept from their end
/// ([`work::ShortOfTime`]): how many calls, and how many of their tests the runs stopped as the budget ran out, the
/// runs did not start as it was used up, and the engine refused before they ran. A day begins at the start of its week
/// of the span, as the page cut it, and every 24 hours after it; with no week given, at midnight UTC. A call counts on
/// the day it was kept.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortDay {
    pub project: Option<String>,
    /// When the day begins, in milliseconds since 1970.
    pub day: u64,
    pub calls: usize,
    pub stopped: usize,
    pub not_started: usize,
    pub refused: usize,
}

/// How long a day is, in milliseconds.
const DAY: u64 = 86_400_000;

/// The day a time falls in ([`ShortDay`]): counted in whole days from the start of the latest week given that begins at
/// it or before, or from 1970 where none is given.
fn day_of(weeks: &[u64], at: u64) -> u64 {
    let from = weeks.iter().copied().filter(|start| *start <= at).max().unwrap_or(0);
    from + (at - from) / DAY * DAY
}

/// What a program that left processes running was run for ([`work::Left`]), with the cell of the run it was run for.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum LeftOf {
    /// A try of a round of a run of an agent's cell.
    Try {
        cell: String,
        run: u64,
        round: usize,
        attempt: usize,
    },
    /// The project's check, run by a round of a run of an act's cell.
    Act { cell: String, run: u64, round: usize },
    /// The project's check, run for a call of the tool by a run of the cell.
    Call {
        cell: String,
        run: u64,
        call: u64,
        tool: String,
    },
    /// A keep-warm ping of the session of this number of the line.
    Ping { line: String, session: u32 },
    /// A fork of the session of a round of a run of an agent's cell.
    Fork { cell: String, run: u64, round: usize },
}

/// What ended past the line of its history.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Past {
    /// A round of a run of the cell, which ended then, its cost or its time past its line: each as judged.
    Round {
        work: u64,
        item: u64,
        cell: String,
        run: u64,
        round: usize,
        ended: u64,
        judged: Judged,
    },
    /// A call of the tool by a run of the cell, which had its result then, its time or the size of its result past its
    /// line: each as judged.
    Call {
        work: u64,
        item: u64,
        cell: String,
        run: u64,
        call: u64,
        tool: String,
        done: u64,
        time: Option<Against>,
        size: Option<Against>,
    },
}

impl Past {
    /// When it ended.
    fn ended(&self) -> u64 {
        match self {
            Past::Round { ended, .. } => *ended,
            Past::Call { done, .. } => *done,
        }
    }
}

/// What is under way, against the lines of its history.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum UnderWay {
    /// A round of a run of the cell, which began then: how long it has taken so far against its line, where enough
    /// rounds tell one, and how much longer it is expected to take.
    Round {
        work: u64,
        item: u64,
        cell: String,
        run: u64,
        round: usize,
        began: u64,
        time: Option<Against>,
        #[serde(default)]
        rest: Option<Rest>,
    },
    /// A call of the tool by a run of the cell, made then, with no result yet: how long it has taken so far against
    /// its line, where enough calls of the tool tell one.
    Call {
        work: u64,
        item: u64,
        cell: String,
        run: u64,
        call: u64,
        tool: String,
        made: u64,
        time: Option<Against>,
    },
}

impl UnderWay {
    /// When it began.
    fn began(&self) -> u64 {
        match self {
            UnderWay::Round { began, .. } => *began,
            UnderWay::Call { made, .. } => *made,
        }
    }
}

/// What the calls to the model of an agent's round under way have come to so far, each measure none before a call
/// tells it: what `GET /api/v1/system/under-way` gives for each round. The lines are those the run guard judges the
/// round by, as they were when it began.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Going {
    pub work: u64,
    pub run: u64,
    pub round: usize,
    /// The whole prompt of the latest call, in tokens.
    pub context: Option<u64>,
    /// The tokens the context has grown by over the round.
    pub added: Option<Figure>,
    /// The tokens the context has grown by for each call after the first, from the second call.
    pub per_call: Option<Figure>,
    /// What the calls cost so far, in millionths of a dollar, priced from the calls.
    pub cost: Option<Figure>,
}

/// A measure of a round under way, with the line it is judged by where the history gives one, and whether it is past
/// it, as the driver judges it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Figure {
    pub value: u64,
    pub line: Option<Line>,
    pub past: bool,
}

impl Figure {
    fn of(figure: &driver::Figure) -> Figure {
        Figure {
            value: figure.value.round() as u64,
            line: figure.line.as_ref().map(Line::of),
            past: figure.past(),
        }
    }
}

impl Going {
    /// What each agent's round under way has come to, by its work, run and round.
    pub fn of(driver: &Driver) -> Vec<Going> {
        let mut going: Vec<Going> = (driver.rounds_come_to().into_iter())
            .map(|(work, run, round, going)| Going {
                work: work.0,
                run: run.0,
                round,
                context: going.context,
                added: going.added.as_ref().map(Figure::of),
                per_call: going.per_call.as_ref().map(Figure::of),
                cost: going.cost.as_ref().map(Figure::of),
            })
            .collect();
        going.sort_by_key(|g| (g.work, g.run, g.round));
        going
    }
}

/// What one work adds to the System view, whatever the span: each thing of it that is counted, with when it happened,
/// and each round and call of a tool, by which the judgments of the time it is read are looked up. Worked out from the
/// work's records alone ([`Part::of`]).
#[derive(Clone, Debug)]
struct Part {
    work: u64,
    item: u64,
    project: Option<String>,
    flow: String,
    /// When its item was added.
    added: u64,
    /// Each of its calls beside its rounds, with when it ended and what it took ([`crate::works::beside_rounds`]).
    beside: Vec<(u64, work::BesideKind, Spent)>,
    /// Each program run for it that left processes running, the earliest first, with when it ended.
    left_behind: Vec<(u64, LeftOf, usize)>,
    /// Each call of `run_tests` whose tests the budget kept from their end, with when it was kept, and how many of its
    /// tests were stopped, not started and refused.
    short_of_time: Vec<(u64, [usize; 3])>,
    /// Its runs, in the order they started.
    runs: Vec<PartRun>,
    /// How it ended, where it has.
    ending: Option<Ended>,
}

/// A run of a work, as the System view reads it.
#[derive(Clone, Debug)]
struct PartRun {
    run: u64,
    cell: String,
    /// Its calls of tools, in the order they were made.
    calls: Vec<PartCall>,
    /// Its rounds, in order.
    rounds: Vec<PartRound>,
}

/// A call of a tool by a run.
#[derive(Clone, Debug)]
struct PartCall {
    call: u64,
    tool: String,
    made: u64,
    /// When it had its result; none while it has none.
    done: Option<u64>,
    /// Whether it is withdrawn with its run or its work, where it has no result ([`driver::judged::withdrawn`]).
    withdrawn: bool,
}

/// A round of a run.
#[derive(Clone, Debug)]
struct PartRound {
    number: usize,
    began: u64,
    ended: Option<u64>,
    spent: Option<Spent>,
    /// Whether it was given back what the round before handed in.
    sent_back: bool,
    /// Why it failed, and of what kind of cause, where it did.
    failed: Option<(String, Failure)>,
    /// What it ran on ([`RanOn::of`]).
    on: RanOn,
}

/// How a work ended, and when, with what the System view says of a merged work.
#[derive(Clone, Debug)]
enum Ended {
    Merged {
        at: u64,
        /// When its first round began.
        from: Option<u64>,
        /// What it took in all ([`spent_in_all`]).
        spent: Spent,
    },
    Cancelled {
        at: u64,
        why: Option<String>,
    },
}

impl Part {
    /// What the work adds to the System view, as the driver holds it, in the room what holds every work at once leaves
    /// it.
    fn of(driver: &Driver, kept: Kept<'_>, at_once: &driver::AtOnce) -> Result<Part, Broken> {
        let runs = runs(kept)?;
        let models = models(kept);
        let mut left_behind = Vec::new();
        for left in &kept.work.left_behind {
            let Some(at) = kept.work.time_of(left.at).map(|at| at.0) else {
                continue;
            };
            let cell_of = |of: RunId| runs.iter().find(|run| run.run == of.0).map(|run| run.cell.clone());
            let of = match &left.of {
                work::Left::Try { run, round, attempt } => cell_of(*run).map(|cell| LeftOf::Try {
                    cell,
                    run: run.0,
                    round: *round,
                    attempt: *attempt,
                }),
                work::Left::Act { run, round } => cell_of(*run).map(|cell| LeftOf::Act {
                    cell,
                    run: run.0,
                    round: *round,
                }),
                work::Left::Call { call } => (runs.iter()).find_map(|run| {
                    let made = run.calls.iter().find(|c| c.call == call.0)?;
                    let tool = match &made.what {
                        Call::Tool { name, .. } => name.clone(),
                        _ => return None,
                    };
                    Some(LeftOf::Call {
                        cell: run.cell.clone(),
                        run: run.run,
                        call: call.0,
                        tool,
                    })
                }),
                work::Left::Ping { line, session } => Some(LeftOf::Ping {
                    line: line.clone(),
                    session: *session,
                }),
                work::Left::Fork { run, round } => cell_of(*run).map(|cell| LeftOf::Fork {
                    cell,
                    run: run.0,
                    round: *round,
                }),
            };
            if let Some(of) = of {
                left_behind.push((at, of, left.processes));
            }
        }
        let short_of_time = (kept.work.short_of_time.iter())
            .filter_map(|short| {
                let at = kept.work.time_of(short.at)?.0;
                let count = |how: work::ShortHow| short.tests.iter().filter(|t| t.how == how).count();
                let counts = [
                    count(work::ShortHow::Stopped),
                    count(work::ShortHow::NotStarted),
                    count(work::ShortHow::Refused),
                ];
                Some((at, counts))
            })
            .collect();
        let ending = match ended(driver, kept, &runs, at_once)? {
            Some(Ending::Merged(at)) => Some(Ended::Merged {
                at,
                from: runs.iter().flat_map(|run| &run.rounds).map(|round| round.began).min(),
                spent: spent_in_all(&runs, kept.work)?,
            }),
            Some(Ending::Cancelled(at)) => Some(Ended::Cancelled {
                at,
                why: (kept.work.records.cancelled()).and_then(|at| kept.work.why_cancelled.get(&at).cloned()),
            }),
            None => None,
        };
        let runs = (runs.iter())
            .map(|run| PartRun {
                run: run.run,
                cell: run.cell.clone(),
                calls: (run.calls.iter())
                    .filter_map(|call| match &call.what {
                        Call::Tool { name, .. } => Some(PartCall {
                            call: call.call,
                            tool: name.clone(),
                            made: call.made,
                            done: call.done,
                            withdrawn: call.done.is_none() && driver::judged::withdrawn(kept.work, CallId(call.call)),
                        }),
                        _ => None,
                    })
                    .collect(),
                rounds: (run.rounds.iter())
                    .map(|round| PartRound {
                        number: round.number,
                        began: round.began,
                        ended: round.ended,
                        spent: round.spent,
                        sent_back: matches!(round.given, Given::SentBack { .. }),
                        failed: match &round.back {
                            Some(Back::Failed { why, failure, .. }) => Some((why.clone(), *failure)),
                            _ => None,
                        },
                        on: RanOn::of(kept, &run.cell, models.get(&(run.run, round.number))),
                    })
                    .collect(),
            })
            .collect();
        Ok(Part {
            work: kept.head.number.0,
            item: kept.item.number.0,
            project: kept.item.project.clone(),
            flow: kept.head.flow.clone(),
            added: kept.item.added.0,
            beside: crate::works::beside_rounds(kept.work, &|_| true)?,
            left_behind,
            short_of_time,
            runs,
            ending,
        })
    }
}

/// What each work adds to the System view, kept between reads with the revision of the work it was worked out from
/// ([`Driver::revision`]): a read works out again only the parts of the works that changed since the last, so that it
/// reads no other work's records, and puts the parts together for its span.
#[derive(Debug, Default)]
pub struct Parts {
    kept: BTreeMap<WorkNumber, (u64, Part)>,
    /// How many works' parts the latest read worked out again.
    worked_out: usize,
}

impl Parts {
    /// How many works' parts the latest read worked out again, the others' being as they were kept.
    pub fn worked_out(&self) -> usize {
        self.worked_out
    }

    /// Brings the parts to every work the driver shows as it holds them now: a part kept of a work whose revision is
    /// not the one it was worked out from is worked out again, and one of a work the driver no longer shows dropped.
    fn bring_up(&mut self, driver: &Driver) -> Result<(), Broken> {
        let mut kept = BTreeMap::new();
        let mut at_once = None;
        self.worked_out = 0;
        for work in driver.works() {
            let number = work.head.number;
            let revision = driver.revision(number);
            match self.kept.remove(&number) {
                Some((was, part)) if Some(was) == revision => {
                    kept.insert(number, (was, part));
                }
                _ => {
                    let at_once = at_once.get_or_insert_with(|| driver.at_once());
                    let part = Part::of(driver, work, at_once)?;
                    self.worked_out += 1;
                    // A work with no revision is worked out again at every read: no revision given is nought.
                    kept.insert(number, (revision.unwrap_or_default(), part));
                }
            }
        }
        self.kept = kept;
        Ok(())
    }
}

impl System {
    /// How the system went over these weeks, the earliest first, until now; over all time where there are none:
    /// worked out from every work's records.
    pub fn of(driver: &Driver, now: Time, weeks: &[u64]) -> Result<System, Broken> {
        System::kept(driver, now, weeks, &mut Parts::default())
    }

    /// How the system went over these weeks ([`System::of`]), from what each work adds to it as these parts keep it,
    /// each brought up to the work as the driver holds it now.
    pub fn kept(driver: &Driver, now: Time, weeks: &[u64], parts: &mut Parts) -> Result<System, Broken> {
        parts.bring_up(driver)?;
        let from = weeks.first().copied();
        let within = |at: u64| from.is_none_or(|from| at >= from);
        let mut system = System {
            now: now.0,
            from,
            spent: Spent::default(),
            pinged: Spent::default(),
            weeks: Vec::new(),
            by_project: Vec::new(),
            by_cell: Vec::new(),
            by_model: Vec::new(),
            by_work: Vec::new(),
            merged: Vec::new(),
            cancelled: Vec::new(),
            added: Vec::new(),
            sent_back: Vec::new(),
            failed: Vec::new(),
            past: Vec::new(),
            under_way: Vec::new(),
            tests: driver.tested().into_iter().map(TestTimes::of).collect(),
            left_behind: Vec::new(),
            short_of_time: Vec::new(),
            stopping: None,
            red_main: driver.red_main().iter().map(RedMain::of).collect(),
        };
        let judged = driver.judged(None, now);
        let mut short_days: BTreeMap<(u64, Option<String>), ShortDay> = BTreeMap::new();
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
        let mut by_project: BTreeMap<Option<String>, Spent> = BTreeMap::new();
        let mut by_cell: BTreeMap<(String, String), Spent> = BTreeMap::new();
        let mut by_model: BTreeMap<RanOn, Spent> = BTreeMap::new();
        for (number, (_, part)) in &parts.kept {
            let (number, work, item) = (*number, part.work, part.item);
            if within(part.added) {
                system.added.push(Added {
                    work,
                    item,
                    at: part.added,
                });
                if let Some(week) = week_of(&mut counted, part.added) {
                    week.added += 1;
                }
            }
            let mut beside = Beside::default();
            for &(at, kind, one) in part.beside.iter().filter(|(at, ..)| within(*at)) {
                beside = beside.with(kind, one);
                if let Some(week) = week_of(&mut counted, at) {
                    if kind == work::BesideKind::Ping {
                        week.pinged = week.pinged.with(one);
                    }
                    week.spent = week.spent.with(one.as_no_round());
                }
            }
            system.pinged = system.pinged.with(beside.pinged);
            system.spent = system.spent.with(beside.all().as_no_round());
            for (at, of, processes) in part.left_behind.iter().filter(|(at, ..)| within(*at)) {
                system.left_behind.push(LeftBehind {
                    work,
                    item,
                    of: of.clone(),
                    at: *at,
                    processes: *processes,
                });
            }
            for &(at, [stopped, not_started, refused]) in part.short_of_time.iter().filter(|(at, _)| within(*at)) {
                let day = day_of(weeks, at);
                let counted = (short_days.entry((day, part.project.clone()))).or_insert_with(|| ShortDay {
                    project: part.project.clone(),
                    day,
                    calls: 0,
                    stopped: 0,
                    not_started: 0,
                    refused: 0,
                });
                counted.calls += 1;
                counted.stopped += stopped;
                counted.not_started += not_started;
                counted.refused += refused;
            }
            let mut of_work = beside.all().as_no_round();
            for run in &part.runs {
                let cell = &run.cell;
                for call in &run.calls {
                    let against = judged.calls.get(&(number, CallId(call.call)));
                    let time = against.and_then(|against| against.time.as_ref()).map(Against::of);
                    let size = against.and_then(|against| against.size.as_ref()).map(Against::of);
                    match call.done {
                        // A call being withdrawn with its run or its work is not under way.
                        None if call.withdrawn => {}
                        None => system.under_way.push(UnderWay::Call {
                            work,
                            item,
                            cell: cell.clone(),
                            run: run.run,
                            call: call.call,
                            tool: call.tool.clone(),
                            made: call.made,
                            time,
                        }),
                        Some(done) if within(done) => {
                            let past = |against: Option<Against>| against.is_some_and(|a| a.past);
                            if past(time) || past(size) {
                                system.past.push(Past::Call {
                                    work,
                                    item,
                                    cell: cell.clone(),
                                    run: run.run,
                                    call: call.call,
                                    tool: call.tool.clone(),
                                    done,
                                    time,
                                    size,
                                });
                            }
                        }
                        Some(_) => {}
                    }
                }
                for round in &run.rounds {
                    let of_round = judged.rounds.get(&(number, RunId(run.run), round.number));
                    let shown = of_round.map_or_else(Judged::default, Judged::of);
                    match round.ended {
                        None => system.under_way.push(UnderWay::Round {
                            work,
                            item,
                            cell: cell.clone(),
                            run: run.run,
                            round: round.number,
                            began: round.began,
                            time: shown.time,
                            rest: of_round.and_then(|judged| judged.rest).map(Rest::of),
                        }),
                        Some(ended) if within(ended) => {
                            let past = |against: Option<Against>| against.is_some_and(|a| a.past);
                            if past(shown.cost) || past(shown.time) {
                                system.past.push(Past::Round {
                                    work,
                                    item,
                                    cell: cell.clone(),
                                    run: run.run,
                                    round: round.number,
                                    ended,
                                    judged: shown,
                                });
                            }
                        }
                        Some(_) => {}
                    }
                    if round.sent_back && within(round.began) {
                        system.sent_back.push(RoundAt {
                            work,
                            item,
                            cell: cell.clone(),
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
                    let of_cell = by_cell.entry((part.flow.clone(), cell.clone())).or_default();
                    *of_cell = of_cell.with(spent);
                    let model = by_model.entry(round.on.clone()).or_default();
                    *model = model.with(spent);
                    if let Some((why, failure)) = &round.failed {
                        system.failed.push(FailedRound {
                            work,
                            item,
                            cell: cell.clone(),
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
            if of_work.rounds > 0 || beside.all().rounds > 0 {
                system.by_work.push(WorkSpent {
                    work,
                    item,
                    spent: of_work,
                });
                let project = by_project.entry(part.project.clone()).or_default();
                *project = project.with(of_work);
            }
            match &part.ending {
                Some(Ended::Merged { at, from, spent }) if within(*at) => {
                    system.merged.push(Merged {
                        work,
                        item,
                        at: *at,
                        from: *from,
                        spent: *spent,
                    });
                    if let Some(week) = week_of(&mut counted, *at) {
                        week.merged += 1;
                    }
                }
                Some(Ended::Cancelled { at, why }) if within(*at) => {
                    system.cancelled.push(Cancelled {
                        work,
                        item,
                        at: *at,
                        why: why.clone(),
                    });
                    if let Some(week) = week_of(&mut counted, *at) {
                        week.cancelled += 1;
                    }
                }
                _ => {}
            }
        }
        system.weeks = counted.into_values().rev().collect();
        system.by_project = (by_project.into_iter())
            .map(|(project, spent)| ProjectSpent { project, spent })
            .collect();
        system.by_project.sort_by_key(|p| std::cmp::Reverse(p.spent.usd_micros));
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
        system.past.sort_by_key(|x| std::cmp::Reverse(x.ended()));
        system.under_way.sort_by_key(UnderWay::began);
        system.left_behind.sort_by_key(|x| std::cmp::Reverse(x.at));
        // The latest day first, and on one day the projects in the order of their names.
        system.short_of_time = short_days.into_values().collect();
        system
            .short_of_time
            .sort_by(|a, b| b.day.cmp(&a.day).then_with(|| a.project.cmp(&b.project)));
        // The engine ends once the last round under way has: as the latest of their rests says.
        system.stopping = driver.stopping().map(|stop| Stop {
            since: stop.since.0,
            at_once: stop.at_once,
            rest: rest_of_rounds(&system.under_way),
        });
        Ok(system)
    }
}

/// What a work took in all, whenever it was: what its runs' rounds took, and its calls beside its rounds, its keep-warm
/// pings and its forks, counted in no round.
fn spent_in_all(runs: &[Run], work: &work::Work) -> Result<Spent, Broken> {
    Ok((runs.iter().fold(Spent::default(), |sum, run| sum.with(run.spent)))
        .with(crate::works::spent_beside_rounds(work, &|_| true)?.all().as_no_round()))
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
fn ended(driver: &Driver, kept: Kept<'_>, runs: &[Run], at_once: &driver::AtOnce) -> Result<Option<Ending>, Broken> {
    let view = viewed_at(driver, kept.head.number, at_once)?;
    Ok(match view.decision {
        Decision::Ended(End::Merged) => (runs.iter())
            .filter_map(|run| match run.end {
                Some(RunEnd::Finished { time, .. }) => Some(time),
                _ => None,
            })
            .max()
            .map(Ending::Merged),
        Decision::Ended(End::Cancelled) => match kept.work.records.cancelled() {
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

/// Answers `GET /api/v1/system/under-way`: what the calls of each agent's round under way have come to so far. Read from
/// what the engine keeps in memory, so that a page reads it each time it is told it changed ([`service::Changed`]).
pub(crate) async fn under_way(State(handle): State<Handle>) -> Response {
    match handle.read(Going::of).await {
        Ok(going) => Json(going).into_response(),
        Err(refusal) => refused(&refusal),
    }
}

/// Answers `GET /api/v1/system`, from the parts of the works these keep between reads ([`Parts`]): the engine's loop
/// alone reads and brings them up, one read after another.
pub(crate) async fn system(
    State(handle): State<Handle>,
    parts: Arc<Mutex<Parts>>,
    span: Result<Query<Span>, QueryRejection>,
) -> Response {
    let weeks = match span
        .map_err(|rejection| rejection.body_text())
        .and_then(|Query(span)| span.starts())
    {
        Ok(weeks) => weeks,
        Err(why) => return fail(StatusCode::BAD_REQUEST, format!("The span cannot be read: {why}.")),
    };
    let read = move |driver: &Driver, now| {
        // Each part kept is that of a work at its revision, whatever a read before did, so a read goes on from them.
        let mut parts = parts.lock().unwrap_or_else(PoisonError::into_inner);
        System::kept(driver, now, &weeks, &mut parts)
    };
    match handle.read_at(read).await {
        Ok(Ok(system)) => Json(system).into_response(),
        Ok(Err(broken)) => fail(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("The system cannot be shown: {}.", broken.0),
        ),
        Err(refusal) => refused(&refusal),
    }
}

#[cfg(test)]
mod tests {
    //! What a round is said to have run on, as the version of the flow its work is on tells its cell.

    use work::Work;

    use super::*;
    use crate::works::tests::{Stored, earlier, flow};

    // A project whose main is red is said to the page and the command line with its tests, the head each failed on, when
    // it began and the works that wait, in the names the page reads. Milliseconds; the one test of this wire shape.
    #[test]
    fn a_red_main_is_said_as_the_page_and_the_command_line_read_it() {
        let red = driver::red_main::Red {
            project: "Firn-wf".to_string(),
            since: work::Time(1_700_000_000_000),
            tests: vec![driver::red_main::RedTest {
                unit: "carry".to_string(),
                name: "tools::tests::b".to_string(),
                head: "9f8e7d6c".to_string(),
            }],
            waiting: vec![store::WorkNumber(184)],
        };
        let said = serde_json::to_value(RedMain::of(&red)).expect("it is said");
        assert_eq!(
            said,
            serde_json::json!({
                "project": "Firn-wf",
                "since": 1_700_000_000_000u64,
                "tests": [{ "unit": "carry", "name": "tools::tests::b", "head": "9f8e7d6c" }],
                "waiting": [184],
            })
        );
        let back: RedMain = serde_json::from_value(said).expect("it is read");
        assert_eq!(back, RedMain::of(&red));
    }

    // A round with no model recorded is an act's where the version its work is on has its cell as an engine's, and is
    // not recorded otherwise. The flow installed has the merge as an act and no cell `check`; the earlier version has
    // the merge as an agent's and `check` as an act, so that a reading of the flow installed fails each case, and the
    // work on none kept too. Milliseconds.
    #[test]
    fn a_round_with_no_model_is_an_acts_as_the_version_its_work_is_on_has_its_cell() {
        let installed = flow();
        let mut outline = flow::Outline::of(&installed);
        let merge = (outline.cells.iter_mut())
            .find(|c| c.id.as_str() == "merge")
            .expect("the merge");
        let mut check = merge.clone();
        merge.agent = true;
        check.id = CellId::from("check");
        outline.cells.push(check);
        let on = earlier(outline);
        let work = Work::default();
        let stored = Stored::new(&installed.name);
        let kept = stored.kept(&installed, &on, &work);
        assert_eq!(RanOn::of(kept, "check", None), RanOn::Act);
        assert_eq!(RanOn::of(kept, "merge", None), RanOn::NotRecorded);
        let model = "a-model".to_string();
        assert_eq!(RanOn::of(kept, "merge", Some(&model)), RanOn::Model { model });
        let none = driver::On::NotKept { end: End::Merged };
        let kept = stored.kept(&installed, &none, &work);
        assert_eq!(RanOn::of(kept, "merge", None), RanOn::NotRecorded);
    }

    // What a merged work took in all is its runs' rounds and its calls beside them, its pings and its forks, in no
    // round. Milliseconds; the merged works of the System view are the only reader of this sum.
    #[test]
    fn what_a_work_took_in_all_counts_its_forks_beside_its_pings_and_in_no_round() {
        let mut work = Work::default();
        work.moments.push(work::Moment {
            from: decide::Seq(1),
            at: Time(1_000),
        });
        work.pinged.push(work::Pinged {
            at: decide::Seq(2),
            line: "engineer".to_string(),
            session: 0,
            spent: work::Spent {
                usd_micros: 20_000,
                uncached_tokens: 5,
                cache_written_tokens: Some(2),
                ..Default::default()
            },
            failed: None,
        });
        work.forked.push(work::Forked {
            at: decide::Seq(3),
            run: RunId(1),
            round: 1,
            usd_micros: 300_000,
            uncached_tokens: 120,
            cache_written_tokens: Some(100),
            cached_tokens: 18_100,
            output_tokens: 30,
        });
        let spent = spent_in_all(&[], &work).expect("the records are sound");
        assert_eq!(
            (
                spent.usd_micros,
                spent.input_tokens,
                spent.cache_written_tokens,
                spent.cached_tokens,
                spent.rounds
            ),
            (320_000, 23, 102, 18_100, 0)
        );
    }
}
