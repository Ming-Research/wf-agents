//! `GET /api/v1/system`: how the whole system went over a span of time, for the System view: what was produced (works
//! merged and cancelled, items added), what was spent and on what (by week, by project, by cell, by model, by work),
//! what was wasted or went wrong (rounds sent back, rounds failed), and how well it ran: the rounds and the calls of
//! tools that went past the lines of their history, what is under way now against its own (crate `driver`, module
//! `judged`), and how often the budget of each project's fast workflow kept tests from their end, day by day. What else
//! goes on now the view reads from the items and the engine, as the bar and the works do.
//!
//! What the works waited for is in `waited`: each stretch a work stopped for, by the cause the engine keeps what a work's
//! time went to by (the owner, the items its item depends on, a slot, the turn to check and merge, a pause), how often
//! and how long over the span and in each week; how long the calls and rounds that waited for a run of the project's CI
//! took, and how long its jobs waited for a runner; and how many works held a slot and how many ran, against how many
//! slots there were. A work that ended is read from what it kept as it ended, one going on from the flow it runs on,
//! with what holds every work at once. No threshold is applied: what is past its own line is for the judgments to say.
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
use crate::works::HeldAll;
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
    /// What the works waited for over the span, how long CI took them, and how many slots they held.
    #[serde(default)]
    pub waited: WaitedFor,
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
    /// What the works waited for in it, how long CI took them, and how many slots they held.
    #[serde(default)]
    pub waited: WeekWaited,
}

/// What the works waited for, as the System view counts it: why a work stopped for a time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WaitReason {
    /// For the owner, as the one definition of what waits on the owner reads it.
    Owner,
    /// For the items its item depends on.
    Items,
    /// For a slot: the walk would have begun a round of an agent's cell, and no round began.
    Slot,
    /// For the turn of its project to check and merge.
    Turn,
    /// For a pause: the work's own, the pause of everything, or a stop of the engine.
    Paused,
}

impl WaitReason {
    /// Every cause, in the order the view lists them.
    const ALL: [WaitReason; 5] = [
        WaitReason::Owner,
        WaitReason::Items,
        WaitReason::Slot,
        WaitReason::Turn,
        WaitReason::Paused,
    ];
}

/// What the works waited for over the span ([`System::waited`]).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WaitedFor {
    /// By each cause, in the order of [`WaitReason`]: how many stops of the works ended in the span, and how long they
    /// took, those still going on counted up to now.
    pub by: Vec<WaitedBy>,
    /// The waits that works which ended before a wait for a slot was told apart kept as anything else: which of them
    /// were for a slot is not known, so they are in none of `by`.
    pub unseparated: Stopped,
    /// How long the calls and rounds that waited for a run of the project's CI took, and how long its jobs waited for a
    /// runner.
    pub ci: CiWaited,
    /// How many works held a slot and how many ran, on average and at most, against how many slots there were.
    pub occupancy: Occupancy,
    /// How many works the waits and the slots are not known of: those that ended on a version of their flow that is no
    /// longer installed and kept nothing of what they waited for, or that are on no version kept.
    pub uncounted: usize,
}

/// How many stops of one cause ended in a stretch of time, and how long they took.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WaitedBy {
    pub reason: WaitReason,
    #[serde(flatten)]
    pub stopped: Stopped,
}

/// How often and how long works stopped: the stops that ended in the stretch, and the time they took within it, a stop
/// still going on counted up to now but not as one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stopped {
    pub stops: usize,
    pub total_ms: u64,
    /// The median and the longest of the stops that ended, in full; none where none ended.
    pub median_ms: Option<u64>,
    pub longest_ms: Option<u64>,
}

/// How long the works waited for the project's CI.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CiWaited {
    /// The calls and rounds that waited for a run and ended in the stretch, and the time they took within it.
    #[serde(flatten)]
    pub waited: Stopped,
    /// How many jobs of those runs said how long they waited for a runner, and the median and the longest of it; none
    /// where none said.
    pub jobs: usize,
    pub queued_median_ms: Option<u64>,
    pub queued_longest_ms: Option<u64>,
}

/// How many works held a slot and how many ran, over a stretch of time: the average, in thousandths of a work, weighted
/// by time, and the most at once; against the slots there were.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Occupancy {
    pub taking_avg: u64,
    pub taking_peak: usize,
    pub running_avg: u64,
    pub running_peak: usize,
    /// The most slots there were in the stretch, as the engine kept them; none where it kept none in the stretch.
    pub slots: Option<usize>,
    /// Whether the engine kept the slots from before the stretch began: where it did not, `slots` is what is known of it.
    pub recorded: bool,
}

/// What the works waited for in a week.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WeekWaited {
    /// How long the works stopped in the week for each cause, in the order of [`WaitReason`].
    pub total_ms: [u64; 5],
    /// How long the calls and rounds that waited for CI took in the week.
    pub ci_ms: u64,
    pub occupancy: Occupancy,
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
    /// What it stopped for, in order; none where that is not known ([`stops_of`]).
    stops: Option<Vec<WaitSpan>>,
    /// Whether `stops` are read from the flow its work is on and what holds every work at once, rather than from what the
    /// work kept as it ended: they are read again as that changes.
    live: bool,
    /// What the work waited for at the places of its records, as read from the flow installed, kept for the next time its
    /// part is worked out ([`driver::time::Decided`]); none for a work whose stops are not read from it.
    decided: driver::time::Decided,
    /// Each call and round that waited for a run of the project's CI and ended, from when it was made to when it ended.
    ci: Vec<(u64, u64)>,
    /// How long each job of a run of the project's CI waited for a runner, with when the run was read.
    queued: Vec<(u64, u64)>,
    /// Each stretch it held a slot, as the version its work is on says which cells take one, and each stretch a round or
    /// a call of a tool of it was under way, merged; the end none while it is under way. None where that version is
    /// not known.
    taking: Spans,
    running: Spans,
}

/// Stretches of time from a time to its end (none while under way), or none where they are not known.
type Spans = Option<Vec<(u64, Option<u64>)>>;

/// A stretch of time a work stopped for one cause.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct WaitSpan {
    from: u64,
    /// Where it has not ended, the time it was worked out to.
    until: u64,
    cause: Cause,
    /// Whether it has not ended: its end is now, whenever it is read.
    open: bool,
}

/// What a work stopped for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Cause {
    Wait(WaitReason),
    /// What a work that ended before a wait for a slot was told apart kept as anything else.
    Unseparated,
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
    fn of(
        driver: &Driver,
        kept: Kept<'_>,
        at_once: &driver::AtOnce,
        held: &HeldAll,
        now: Time,
        mut decided: driver::time::Decided,
    ) -> Result<Part, Broken> {
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
        let (stops, live) = stops_of(kept, held, ending.is_none(), now, &mut decided);
        let (taking, running) = held_slots(kept, &runs);
        let ci = (driver::time::ci_waits(kept.work, now).into_iter())
            .filter(|(_, ended)| *ended)
            .map(|(stretch, _)| (stretch.from.0, stretch.until.0))
            .collect();
        let queued = (kept.work.timed.iter())
            .filter_map(|timed| {
                let at = kept.work.time_of(timed.at)?.0;
                Some(
                    timed
                        .reading
                        .jobs
                        .iter()
                        .filter_map(move |job| Some((at, job.queued_millis?))),
                )
            })
            .flatten()
            .collect();
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
            stops,
            live,
            decided: if live { decided } else { Default::default() },
            ci,
            queued,
            taking,
            running,
        })
    }
}

/// What a work stopped for, in order, from what it kept as it ended where it kept it since it was last reopened, else
/// from the flow installed where the work is on it, with what holds every work at once ([`driver::time::stretches_to`]):
/// a work that has not ended is read up to `now`. Each stop is a stretch of one cause, where the stretches that follow
/// one another of the same cause are one. A stretch of what the work ran in is no stop. None where the work ended on a
/// version no longer installed and kept nothing: what it waited for is not known. Whether it was read from the flow
/// installed is the second. What an earlier reading of the work decided is in `decided`, and what this one decides is
/// left in it.
fn stops_of(
    kept: Kept<'_>,
    held: &HeldAll,
    going: bool,
    now: Time,
    decided: &mut driver::time::Decided,
) -> (Option<Vec<WaitSpan>>, bool) {
    let work = kept.work;
    let kept_time = (work.kept_time.last()).filter(|k| !work.records.reopens.iter().any(|reopen| *reopen > k.at));
    let (stretches, separated, live) = match kept_time {
        Some(kept_time) => (kept_time.stretches.clone(), kept_time.separated, false),
        None if kept.on.installed() => {
            let stretches = driver::time::stretches_to(
                &kept,
                &held.everything,
                &held.stops,
                &held.cannot_run,
                &held.down,
                held.turns_of_others(&kept),
                now,
                going.then_some(now),
                decided,
            );
            (stretches, true, true)
        }
        None => return (None, false),
    };
    let mut waits: Vec<(u64, u64, Cause)> = (stretches.iter())
        .filter_map(|s| {
            let cause = match s.went {
                work::Went::Model | work::Went::Tools => return None,
                work::Went::Owner => Cause::Wait(WaitReason::Owner),
                work::Went::Items => Cause::Wait(WaitReason::Items),
                work::Went::Slot => Cause::Wait(WaitReason::Slot),
                work::Went::Turn => Cause::Wait(WaitReason::Turn),
                work::Went::Paused | work::Went::Stopping => Cause::Wait(WaitReason::Paused),
                // What nothing says: no cause the view counts, but where it was kept before a wait for a slot was told
                // apart, it holds such waits.
                work::Went::Other if separated => return None,
                work::Went::Other => Cause::Unseparated,
            };
            (s.until > s.from).then_some((s.from.0, s.until.0, cause))
        })
        .collect();
    waits.sort();
    let mut stops: Vec<WaitSpan> = Vec::new();
    for (from, until, cause) in waits {
        match stops.last_mut() {
            Some(last) if last.until == from && last.cause == cause => last.until = until,
            _ => stops.push(WaitSpan {
                from,
                until,
                cause,
                open: false,
            }),
        }
    }
    if live
        && going
        && let Some(last) = stops.last_mut()
    {
        last.open = last.until >= now.0;
    }
    (Some(stops), live)
}

/// Each stretch the work held a slot, and each stretch a round or a call of a tool of it was under way, as the version
/// the work is on says which of its cells an agent runs, and so takes a slot; none of both for a work on none kept, whose
/// cells that version does not tell.
fn held_slots(kept: Kept<'_>, runs: &[Run]) -> (Spans, Spans) {
    if kept.on.outline().is_none() {
        return (None, None);
    }
    let mut taking: Vec<(u64, Option<u64>)> = Vec::new();
    let mut running: Vec<(u64, Option<u64>)> = Vec::new();
    for run in runs {
        let agent = kept
            .cell(&CellId::from(run.cell.as_str()))
            .is_some_and(|cell| cell.agent);
        for round in &run.rounds {
            running.push((round.began, round.ended));
            if agent {
                taking.push((round.began, round.ended));
            }
        }
        for call in &run.calls {
            if matches!(call.what, Call::Tool { .. }) {
                running.push((call.made, call.done));
            }
        }
    }
    (Some(joined(taking)), Some(joined(running)))
}

/// The stretches, joined where they overlap or touch, in order; an end of none is a stretch still going on.
fn joined(mut stretches: Vec<(u64, Option<u64>)>) -> Vec<(u64, Option<u64>)> {
    stretches.sort_by_key(|(from, until)| (*from, until.unwrap_or(u64::MAX)));
    let mut out: Vec<(u64, Option<u64>)> = Vec::new();
    for (from, until) in stretches {
        match out.last_mut() {
            Some(last) if last.1.is_none_or(|end| from <= end) => {
                last.1 = match (last.1, until) {
                    (Some(a), Some(b)) => Some(a.max(b)),
                    _ => None,
                };
            }
            _ => out.push((from, until)),
        }
    }
    out
}

/// What each work adds to the System view, kept between reads with the revision of the work it was worked out from
/// ([`Driver::revision`]): a read works out again only the parts of the works that changed since the last, so that it
/// reads no other work's records, and puts the parts together for its span.
#[derive(Debug, Default)]
pub struct Parts {
    /// Each work's part, with the revision of the work and the generation of what held every work at once that it was
    /// worked out from: the latter matters only to a part read from the flow installed ([`Part::live`]).
    kept: BTreeMap<WorkNumber, (u64, u64, Part)>,
    /// What held every work at once as the latest read that asked for it found it, and how many times it changed since the
    /// first: the stops of a work that has not ended, which it is read by, are worked out again as it changes.
    held: Option<HeldAll>,
    generation: u64,
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
    fn bring_up(&mut self, driver: &Driver, now: Time) -> Result<(), Broken> {
        let works: Vec<_> = driver
            .works()
            .map(|work| (work, driver.revision(work.head.number)))
            .collect();
        // What holds every work at once is read only where a part is to be worked out, or one of a work that has not ended
        // is to be told still current: a read that finds every part kept of works that ended asks the driver for nothing
        // beyond their revisions.
        let asks = works
            .iter()
            .any(|(work, revision)| match self.kept.get(&work.head.number) {
                Some((was, _, part)) => Some(*was) != *revision || part.live,
                None => true,
            });
        if asks {
            let held = HeldAll::of(driver, &driver.agents_at_once());
            if self.held.as_ref() != Some(&held) {
                self.held = Some(held);
                self.generation += 1;
            }
        }
        let mut kept = BTreeMap::new();
        let mut at_once = None;
        self.worked_out = 0;
        for (work, revision) in works {
            let number = work.head.number;
            match self.kept.remove(&number) {
                Some((was, generation, part))
                    if Some(was) == revision && (!part.live || generation == self.generation) =>
                {
                    kept.insert(number, (was, generation, part));
                }
                before => {
                    let at_once = at_once.get_or_insert_with(|| driver.at_once());
                    let held = self
                        .held
                        .as_ref()
                        .expect("what held every work is kept where a part is worked out");
                    // What the part before it decided at the places of the records stays decided: the records are only
                    // added to.
                    let decided = before.map(|(_, _, part)| part.decided).unwrap_or_default();
                    let part = Part::of(driver, work, at_once, held, now, decided)?;
                    self.worked_out += 1;
                    // A work with no revision is worked out again at every read: no revision given is nought.
                    kept.insert(number, (revision.unwrap_or_default(), self.generation, part));
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
        parts.bring_up(driver, now)?;
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
            waited: WaitedFor::default(),
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
        for (number, (_, _, part)) in &parts.kept {
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
        // What the works waited for, over the span and in each week: each week from when it begins to when the next does,
        // the latest to now.
        let held = || parts.kept.values().map(|(_, _, part)| part);
        let slots = driver.slots_history();
        let crowds = Crowds::of(held(), now.0);
        system.waited = WaitedFor::of(held(), from, now.0, &crowds, slots);
        let starts: Vec<u64> = weeks.to_vec();
        for week in &mut system.weeks {
            let next = starts.iter().find(|start| **start > week.start).copied();
            week.waited = WeekWaited::of(held(), (week.start, next.unwrap_or(now.0)), now.0, &crowds, slots);
        }
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

/// How many milliseconds of the stretch from `from` up to `until` fall in the window.
fn within_window(from: u64, until: u64, window: (u64, u64)) -> u64 {
    until.min(window.1).saturating_sub(from.max(window.0))
}

/// The median of the values, the mean of the two in the middle where there are two; none where there are none.
fn median_of(mut values: Vec<u64>) -> Option<u64> {
    values.sort_unstable();
    let n = values.len();
    match n {
        0 => None,
        _ if n % 2 == 1 => Some(values[n / 2]),
        _ => Some((values[n / 2 - 1] + values[n / 2]) / 2),
    }
}

/// What came of the stops of one cause in a window: those that ended in it, each in full, and how much of every stop's
/// time, those going on counted to `now`, lies in it.
#[derive(Default)]
struct Tally {
    total: u64,
    ended: Vec<u64>,
}

impl Tally {
    fn add(&mut self, from: u64, until: u64, open: bool, window: (u64, u64)) {
        self.total += within_window(from, until, window);
        if !open && until >= window.0 && until < window.1 {
            self.ended.push(until - from);
        }
    }

    fn stopped(self) -> Stopped {
        Stopped {
            stops: self.ended.len(),
            total_ms: self.total,
            longest_ms: self.ended.iter().max().copied(),
            median_ms: median_of(self.ended),
        }
    }
}

/// What every work stopped for, by cause, in the window: where a stop is still going on its end is `now`. The stops
/// that works which ended before a wait for a slot was told apart kept as anything else are told apart as `Cause`.
fn tallies<'p>(parts: impl Iterator<Item = &'p Part>, window: (u64, u64), now: u64) -> BTreeMap<Cause, Tally> {
    let mut by: BTreeMap<Cause, Tally> = BTreeMap::new();
    for stop in parts.filter_map(|part| part.stops.as_ref()).flatten() {
        let until = if stop.open { now.max(stop.until) } else { stop.until };
        by.entry(stop.cause)
            .or_default()
            .add(stop.from, until, stop.open, window);
    }
    by
}

/// How long the calls and rounds that waited for a run of the project's CI took in the window, as `tallies` counts a
/// stop: those that ended in it, and the time they took within it.
fn ci_tally<'p>(parts: impl Iterator<Item = &'p Part>, window: (u64, u64)) -> Tally {
    let mut tally = Tally::default();
    for &(from, until) in parts.flat_map(|part| &part.ci) {
        tally.add(from, until, false, window);
    }
    tally
}

/// How many slots there were in the window at most, and whether the engine kept them from before it began.
fn slots_in(history: &[store::SlotsRecord], window: (u64, u64)) -> (Option<usize>, bool) {
    let mut most = None;
    for (i, record) in history.iter().enumerate() {
        let until = history.get(i + 1).map_or(u64::MAX, |next| next.at.0);
        if record.at.0 < window.1 && until > window.0 {
            most = most.max(Some(record.slots));
        }
    }
    (most, history.first().is_some_and(|first| first.at.0 <= window.0))
}

/// How many stretches there were at each time, from stretches each from a time to its end (none while under way, which is
/// `now`): the times at which one began, counted up, and one ended, counted down, in order, where one that ends at a time
/// counts before one that begins at it, so that nothing is counted twice at once. Sorted once, it gives the crowd of
/// each window in a pass.
struct Crowd {
    moves: Vec<(u64, i64)>,
}

impl Crowd {
    fn of(stretches: impl Iterator<Item = (u64, Option<u64>)>, now: u64) -> Crowd {
        let mut moves: Vec<(u64, i64)> = Vec::new();
        for (from, until) in stretches {
            let until = until.unwrap_or(now);
            if until > from {
                moves.push((from, 1));
                moves.push((until, -1));
            }
        }
        moves.sort_unstable();
        Crowd { moves }
    }

    /// How many of them overlapped at a time in the window: the average by time, in thousandths of a stretch, and the
    /// most at once.
    fn within(&self, window: (u64, u64)) -> (u64, usize) {
        if window.1 <= window.0 {
            return (0, 0);
        }
        let begun = self.moves.partition_point(|(time, _)| *time <= window.0);
        let mut level: i64 = self.moves[..begun].iter().map(|(_, step)| step).sum();
        let (mut peak, mut area, mut at) = (level, 0_u128, window.0);
        for &(time, step) in self.moves[begun..].iter().take_while(|(time, _)| *time < window.1) {
            area += (level as u128) * u128::from(time - at);
            (level, at) = (level + step, time);
            peak = peak.max(level);
        }
        area += (level as u128) * u128::from(window.1 - at);
        let average = (area * 1000) / u128::from(window.1 - window.0);
        (average as u64, peak as usize)
    }
}

/// How many works held a slot, and how many ran, at each time: from the stretches of every work that kept them.
struct Crowds {
    taking: Crowd,
    running: Crowd,
}

impl Crowds {
    fn of<'p>(parts: impl Iterator<Item = &'p Part> + Clone, now: u64) -> Crowds {
        let stretches = |pick: fn(&Part) -> &Spans| {
            Crowd::of(
                parts.clone().flat_map(move |part| pick(part).iter().flatten().copied()),
                now,
            )
        };
        Crowds {
            taking: stretches(|part| &part.taking),
            running: stretches(|part| &part.running),
        }
    }
}

impl Occupancy {
    /// How many works held a slot and how many ran in the window, against the slots there were.
    fn of(crowds: &Crowds, window: (u64, u64), history: &[store::SlotsRecord]) -> Occupancy {
        let (taking_avg, taking_peak) = crowds.taking.within(window);
        let (running_avg, running_peak) = crowds.running.within(window);
        let (slots, recorded) = slots_in(history, window);
        Occupancy {
            taking_avg,
            taking_peak,
            running_avg,
            running_peak,
            slots,
            recorded,
        }
    }
}

impl WaitedFor {
    /// What the works waited for from the span's first week, or from the earliest of what they did where it has none,
    /// up to now.
    fn of<'p>(
        parts: impl Iterator<Item = &'p Part> + Clone,
        from: Option<u64>,
        now: u64,
        crowds: &Crowds,
        history: &[store::SlotsRecord],
    ) -> WaitedFor {
        let begins = (parts.clone())
            .flat_map(|part| part.stops.iter().flatten().map(|stop| stop.from))
            .chain(
                parts
                    .clone()
                    .flat_map(|part| part.running.iter().flatten().map(|(from, _)| *from)),
            )
            .min();
        let window = (from.or(begins).unwrap_or(now), now);
        let mut by = tallies(parts.clone(), window, now);
        let ci_parts = ci_tally(parts.clone(), window).stopped();
        let queued: Vec<u64> = (parts.clone())
            .flat_map(|part| part.queued.iter())
            .filter(|(at, _)| *at >= window.0 && *at < window.1.saturating_add(1))
            .map(|(_, millis)| *millis)
            .collect();
        WaitedFor {
            by: WaitReason::ALL
                .iter()
                .map(|reason| WaitedBy {
                    reason: *reason,
                    stopped: by.remove(&Cause::Wait(*reason)).unwrap_or_default().stopped(),
                })
                .collect(),
            unseparated: by.remove(&Cause::Unseparated).unwrap_or_default().stopped(),
            ci: CiWaited {
                waited: ci_parts,
                jobs: queued.len(),
                queued_longest_ms: queued.iter().max().copied(),
                queued_median_ms: median_of(queued),
            },
            occupancy: Occupancy::of(crowds, window, history),
            uncounted: parts
                .filter(|part| part.stops.is_none() || part.taking.is_none())
                .count(),
        }
    }
}

impl WeekWaited {
    /// What the works waited for in the week, from when it begins to when the next does, or to now.
    fn of<'p>(
        parts: impl Iterator<Item = &'p Part> + Clone,
        window: (u64, u64),
        now: u64,
        crowds: &Crowds,
        history: &[store::SlotsRecord],
    ) -> WeekWaited {
        let mut by = tallies(parts.clone(), window, now);
        let mut total_ms = [0; 5];
        for (i, reason) in WaitReason::ALL.iter().enumerate() {
            total_ms[i] = by.remove(&Cause::Wait(*reason)).map_or(0, |tally| tally.total);
        }
        WeekWaited {
            total_ms,
            ci_ms: ci_tally(parts.clone(), window).total,
            occupancy: Occupancy::of(crowds, window, history),
        }
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
            unpriced: false,
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

    /// A part holding only what the waits and the slots are counted from.
    fn part(stops: Option<Vec<WaitSpan>>, taking: Spans, running: Spans) -> Part {
        Part {
            work: 1,
            item: 1,
            project: None,
            flow: "trial".to_string(),
            added: 0,
            beside: Vec::new(),
            left_behind: Vec::new(),
            short_of_time: Vec::new(),
            runs: Vec::new(),
            ending: None,
            stops,
            live: false,
            decided: Default::default(),
            ci: Vec::new(),
            queued: Vec::new(),
            taking,
            running,
        }
    }

    fn stop(from: u64, until: u64, cause: Cause, open: bool) -> WaitSpan {
        WaitSpan {
            from,
            until,
            cause,
            open,
        }
    }

    const OWNER: Cause = Cause::Wait(WaitReason::Owner);
    const SLOT: Cause = Cause::Wait(WaitReason::Slot);
    const TURN: Cause = Cause::Wait(WaitReason::Turn);

    fn stretch(from: u64, until: u64, went: work::Went) -> work::Stretch {
        work::Stretch {
            from: Time(from),
            until: Time(until),
            went,
        }
    }

    // What a work stopped for is what it kept as it ended: each stretch it waited in, the stretches of one cause that
    // follow one another one stop, a pause and a stop of the engine both a pause, an empty stretch none, and what ran
    // no stop. What was kept before a wait for a slot was told apart holds such waits in its `Other`, which are then a
    // cause of their own, never a slot's; where it tells them apart, `Other` is no cause the view counts. A work on no
    // version kept that kept nothing is not known. Milliseconds.
    #[test]
    fn what_a_work_stopped_for_is_what_it_kept_by_cause_with_the_stretches_of_one_cause_made_one_stop() {
        use work::Went;
        let stretches = vec![
            stretch(0, 1_000, Went::Model),
            stretch(1_000, 2_000, Went::Owner),
            stretch(2_000, 3_000, Went::Owner),
            stretch(3_000, 4_000, Went::Tools),
            stretch(4_000, 4_500, Went::Slot),
            stretch(5_000, 6_000, Went::Stopping),
            stretch(6_000, 7_000, Went::Paused),
            stretch(8_000, 9_000, Went::Other),
            stretch(9_000, 9_000, Went::Turn),
            stretch(10_000, 11_000, Went::Items),
        ];
        let installed = flow();
        let none = driver::On::NotKept { end: End::Merged };
        let stored = Stored::new(&installed.name);
        let held = HeldAll {
            everything: Vec::new(),
            stops: Vec::new(),
            cannot_run: Vec::new(),
            down: Vec::new(),
            turns: Vec::new(),
        };
        let paused = Cause::Wait(WaitReason::Paused);
        let items = Cause::Wait(WaitReason::Items);
        for separated in [true, false] {
            let mut work = Work::default();
            work.kept_time.push(work::KeptTime {
                at: decide::Seq(9),
                stretches: stretches.clone(),
                turn: None,
                separated,
            });
            let (stops, live) = stops_of(
                stored.kept(&installed, &none, &work),
                &held,
                false,
                Time(20_000),
                &mut Default::default(),
            );
            let mut expected = vec![
                stop(1_000, 3_000, OWNER, false),
                stop(4_000, 4_500, SLOT, false),
                stop(5_000, 7_000, paused, false),
                stop(10_000, 11_000, items, false),
            ];
            if !separated {
                expected.insert(3, stop(8_000, 9_000, Cause::Unseparated, false));
            }
            assert_eq!(stops, Some(expected), "separated: {separated}");
            assert!(!live, "what was kept is not worked out again");
        }
        let work = Work::default();
        let (stops, live) = stops_of(
            stored.kept(&installed, &none, &work),
            &held,
            false,
            Time(20_000),
            &mut Default::default(),
        );
        assert_eq!((stops, live), (None, false), "a work that kept nothing, on no version");
    }

    // The stops of each cause that ended in the span are counted once, each in full, however many weeks they cross; the
    // time of every stop in the span, one still going on counted to now, is added up; a work whose waits are not
    // known is counted in `uncounted`, never as one that did not wait. The stops: the owner 0..10 s, 30..60 s and
    // 5..25 s, a slot 10..12 s, the turn from 90 s to now (100 s, the part kept to 95 s), and 40..50 s the work kept
    // before a wait for a slot was told apart. Milliseconds, worked out by hand.
    #[test]
    fn the_stops_of_each_cause_are_counted_where_they_ended_and_their_time_where_it_lies() {
        let mut first = part(
            Some(vec![
                stop(0, 10_000, OWNER, false),
                stop(10_000, 12_000, SLOT, false),
                stop(30_000, 60_000, OWNER, false),
                stop(90_000, 95_000, TURN, true),
            ]),
            Some(vec![(0, Some(40_000))]),
            Some(vec![(0, Some(50_000))]),
        );
        first.ci = vec![(1_000, 5_000), (50_000, 51_000)];
        first.queued = vec![(3_000, 5_000), (60_000, 15_000)];
        let mut second = part(
            Some(vec![
                stop(5_000, 25_000, OWNER, false),
                stop(40_000, 50_000, Cause::Unseparated, false),
            ]),
            Some(vec![(20_000, Some(60_000))]),
            Some(vec![(0, None)]),
        );
        second.ci = vec![(2_000, 8_000), (45_000, 55_000)];
        second.queued = vec![(70_000, 9_000), (150_000, 99_000)];
        let unknown = part(None, None, None);
        let parts = [first, second, unknown];
        let history = [
            store::SlotsRecord { slots: 4, at: Time(0) },
            store::SlotsRecord {
                slots: 6,
                at: Time(70_000),
            },
        ];
        let stopped = |stops, total_ms, median_ms, longest_ms| Stopped {
            stops,
            total_ms,
            median_ms,
            longest_ms,
        };
        let none = stopped(0, 0, None, None);
        let crowds = Crowds::of(parts.iter(), 100_000);
        let waited = WaitedFor::of(parts.iter(), Some(0), 100_000, &crowds, &history);
        assert_eq!(
            waited.by,
            [
                WaitedBy {
                    reason: WaitReason::Owner,
                    stopped: stopped(3, 60_000, Some(20_000), Some(30_000)),
                },
                WaitedBy {
                    reason: WaitReason::Items,
                    stopped: none,
                },
                WaitedBy {
                    reason: WaitReason::Slot,
                    stopped: stopped(1, 2_000, Some(2_000), Some(2_000)),
                },
                WaitedBy {
                    reason: WaitReason::Turn,
                    // Going on: its time counts to now, and it is no stop that ended.
                    stopped: stopped(0, 10_000, None, None),
                },
                WaitedBy {
                    reason: WaitReason::Paused,
                    stopped: none,
                },
            ]
        );
        assert_eq!(waited.unseparated, stopped(1, 10_000, Some(10_000), Some(10_000)));
        assert_eq!(waited.uncounted, 1, "the work whose waits are not known");
        // Four calls and rounds that waited for CI took 4, 1, 6 and 10 s; three jobs said how long they waited for a
        // runner, the one read after the span's end not counted.
        assert_eq!(
            waited.ci,
            CiWaited {
                waited: stopped(4, 21_000, Some(5_000), Some(10_000)),
                jobs: 3,
                queued_median_ms: Some(9_000),
                queued_longest_ms: Some(15_000),
            }
        );
        // The span with no first week begins where the earliest of what the works did does.
        assert_eq!(WaitedFor::of(parts.iter(), None, 100_000, &crowds, &history), waited);
        // Slots held: one from 0 to 20 s, two to 40 s, one to 60 s; ran: two to 50 s, then one, the round under way
        // counted to now.
        assert_eq!(
            waited.occupancy,
            Occupancy {
                taking_avg: 800,
                taking_peak: 2,
                running_avg: 1_500,
                running_peak: 2,
                slots: Some(6),
                recorded: true,
            }
        );
        // The week from 20 to 50 s has only the time of each stop that lies in it, and a stop is counted where it
        // ended: the owner's 30..60 s stop is none of this week's though most of it is.
        let week = (20_000, 50_000);
        assert_eq!(
            WeekWaited::of(parts.iter(), week, 100_000, &crowds, &history),
            WeekWaited {
                total_ms: [25_000, 0, 0, 0, 0],
                ci_ms: 5_000,
                occupancy: Occupancy {
                    taking_avg: 1_666,
                    taking_peak: 2,
                    running_avg: 2_000,
                    running_peak: 2,
                    slots: Some(4),
                    recorded: true,
                },
            }
        );
        let ended = tallies(parts.iter(), week, 100_000);
        let count = |cause: Cause| ended.get(&cause).map_or(0, |tally| tally.ended.len());
        assert_eq!(
            [count(OWNER), count(SLOT), count(TURN), count(Cause::Unseparated)],
            [1, 0, 0, 0],
            "only the owner's stop that ended at 25 s ended in the week; the one that ended at its end is the next week's"
        );
    }

    // The slots are known only from when the engine first kept them, a stretch's end that touches the next stretch's
    // start counts the ends first so that nothing is counted twice, and no slots kept is none, not a number.
    #[test]
    fn the_slots_there_were_and_how_many_ran_at_once_are_read_from_what_was_kept_and_from_each_stretch() {
        let kept = |slots: usize, at: u64| store::SlotsRecord { slots, at: Time(at) };
        assert_eq!(slots_in(&[], (0, 100)), (None, false));
        assert_eq!(
            slots_in(&[kept(4, 30)], (0, 100)),
            (Some(4), false),
            "kept from part of the window"
        );
        assert_eq!(slots_in(&[kept(4, 0)], (10, 100)), (Some(4), true));
        assert_eq!(
            slots_in(&[kept(4, 0), kept(6, 70)], (80, 100)),
            (Some(6), true),
            "the latest alone"
        );
        assert_eq!(
            slots_in(&[kept(4, 0), kept(6, 70)], (0, 60)),
            (Some(4), true),
            "none yet set to 6"
        );
        assert_eq!(
            slots_in(&[kept(4, 200)], (0, 100)),
            (None, false),
            "kept only after the window"
        );
        let touching = [(0, Some(10_000)), (10_000, Some(20_000))];
        assert_eq!(Crowd::of(touching.into_iter(), 20_000).within((0, 20_000)), (1_000, 1));
        assert_eq!(Crowd::of([].into_iter(), 20_000).within((0, 20_000)), (0, 0));
        assert_eq!(
            Crowd::of([(0, None)].into_iter(), 0).within((0, 0)),
            (0, 0),
            "a window with no length"
        );
        // Three stretches, the last going on to now (60): over 20..45 two held from the start of it to 30, one to 40, and
        // two to its end, for 20, 10 and 10 of its 25 more, the most at once two.
        let crowd = Crowd::of([(0, Some(30)), (10, Some(50)), (40, None)].into_iter(), 60);
        assert_eq!(crowd.within((20, 45)), (1_600, 2));
        assert_eq!(crowd.within((50, 60)), (1_000, 1), "the one going on alone, to now");
        assert_eq!(
            crowd.within((60, 90)),
            (0, 0),
            "nothing begins at the time the others end"
        );
    }

    /// A run of a cell with these rounds and tool calls, each from a time to its end, as the page reads one.
    fn run_of(cell: &str, rounds: &[(u64, Option<u64>)], calls: &[(u64, Option<u64>)]) -> Run {
        let rounds: Vec<_> = (rounds.iter().enumerate())
            .map(|(i, (began, ended))| {
                serde_json::json!({
                    "number": i + 1, "began": began, "ended": ended, "given": { "kind": "task" },
                    "compactions": 0, "tries": [],
                })
            })
            .collect();
        let calls: Vec<_> = (calls.iter().enumerate())
            .map(|(i, (made, done))| {
                serde_json::json!({
                    "call": i + 1, "what": { "kind": "tool", "name": "ci", "input": "" }, "made": made, "done": done,
                })
            })
            .collect();
        serde_json::from_value(serde_json::json!({
            "run": 1, "cell": cell, "at": 1, "time": 0, "because": [], "basis": [], "rounds": rounds, "calls": calls,
            "end": null, "spent": serde_json::to_value(Spent::default()).expect("nothing spent is said"),
        }))
        .expect("a run is read")
    }

    // A round holds a slot where the version its work is on says its cell is an agent's, whichever flow is installed
    // now, and a round or a call of a tool is running whichever its cell is. The cell `old` is an agent's in the
    // version the work is on and in none installed; `merge` is an act in both. Read by the version installed, `old`
    // would take no slot; a work on none kept has no outline to read, so it is none, not nothing.
    #[test]
    fn a_work_held_a_slot_in_the_rounds_of_the_cells_its_own_version_says_an_agent_runs() {
        let installed = flow();
        let mut outline = flow::Outline::of(&installed);
        let merge = (outline.cells.iter_mut())
            .find(|c| c.id.as_str() == "merge")
            .expect("the merge");
        merge.agent = false;
        let mut old = merge.clone();
        old.id = CellId::from("old");
        old.agent = true;
        outline.cells.push(old);
        let runs = [
            run_of(
                "old",
                &[(1_000, Some(5_000)), (4_000, Some(9_000))],
                &[(9_000, Some(12_000))],
            ),
            run_of("merge", &[(2_000, Some(3_000))], &[]),
            run_of("old", &[(20_000, None)], &[]),
        ];
        let work = Work::default();
        let stored = Stored::new(&installed.name);
        let on = earlier(outline);
        assert_eq!(
            held_slots(stored.kept(&installed, &on, &work), &runs),
            (
                Some(vec![(1_000, Some(9_000)), (20_000, None)]),
                Some(vec![(1_000, Some(12_000)), (20_000, None)])
            )
        );
        let now = driver::On::installed_flow(&installed);
        assert_eq!(
            held_slots(stored.kept(&installed, &now, &work), &runs),
            (Some(Vec::new()), Some(vec![(1_000, Some(12_000)), (20_000, None)])),
            "read by the version installed, where `old` is no cell"
        );
        let none = driver::On::NotKept { end: End::Merged };
        assert_eq!(held_slots(stored.kept(&installed, &none, &work), &runs), (None, None));
    }
}
