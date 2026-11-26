//! The project's workflows, which the engine runs by hand through the project's declaration (`ci` in
//! `wf-agents.toml`), always by their files on main, given the commit to run on, each run found by a value its
//! name holds, which is worked out from what asked for it; and what each reports, read from the JUnit XML it uploads
//! (`results`), never guessed from a log.
//!
//! The value is the same each time the same call of `run_tests` or the same round of the check asks for the same
//! workflow on the same commit, and is had by no other: given again after the engine started again, the request
//! finds the run it asked for before and waits for it, or reads it where it has ended, and asks for a run again only
//! where it finds none but runs that were cancelled, which judged nothing.
//!
//! - fast: runs only the tests and checks a request names, and builds only what they need; every `run_tests` that names
//!   a test or a check is run by it, and so is what the check before a merge runs again of what failed.
//! - a measurement: the workflow a measurement the project declares names (`ci.measure.<name>`), run for each
//!   measurement a request names, on the commit, with the commit the work's change began from among its inputs where
//!   the declaration puts it there; it reports what it measured as a CSV file, which the engine gives as the file holds
//!   it, after checking that each metric the declaration names is a number in every row, and works out nothing of its
//!   own from it. On a machine of the project's own, it waits for its turn (`machines`).
//! - full: the workflows that hold every job required to merge; the check before a merge runs them, once, on the
//!   commit to merge, and nothing else does.
//!
//! A request of `run_tests` has a budget: the time the tests it names may take together, the project's `ci.fast.budget`
//! or else the one `run_tests.toml` gives every project. Where the medians of their times in the project's earlier fast
//! runs say that one of them, or all of them together, would take longer, nothing is run ([`over_budget`]); otherwise
//! the fast workflow is given the budget, where it takes it, and stops the tests when it is used up. A test with no
//! time in an earlier fast run is never refused. What the budget kept from running to its end is said apart from what
//! failed, and is no failure the check before a merge runs again: that check's runs are given no budget.
//!
//! What is said of a run: each test, with what became of it, how long it took and what a failed one said; and each
//! job of the workflow, with how it ended and, where it ran to a conclusion, the wall time GitHub records for it. A
//! run that uploaded no JUnit the engine can read is said so: no result is guessed from a log. A job that failed with
//! no failed test of its own to say why, as a lint, is said with the end of what the step that failed it printed, read
//! from its log on GitHub with the engine's token (`log`); a log that cannot be read is said so, and nothing else.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use driver::red_main::RedTest;
use flow::agents::{Ci, CiFast, CiFull, Measurement, MeasurementWords, ResultWords, Tool, fill};
use store::WorkNumber;
use uuid::Uuid;

use super::api::{Api, Check, File, Outcome, Run};
use super::metrics::{self, Table};
use super::results::{self, Became, Case};
use super::{GitHub, log, output};
use crate::git;
use crate::held::Stop;
use crate::host::{Asking, Failed, Measure, Named, Verdict};
use crate::process;
use decide::Obstacle;
use service::Along;
use work::timed::{Job, Reading, Source, Test};

/// A request for runs of the workflows: the commit they run on, and which of the work's asks it is, by which its runs
/// are found again.
#[derive(Clone, Copy, Debug)]
pub(super) struct Request<'a> {
    pub(super) work: WorkNumber,
    pub(super) commit: &'a str,
    pub(super) asking: Asking,
}

/// The workflow to run, the artifact its results are in, and what it is run with.
struct Plan<'a> {
    workflow: &'a str,
    results: &'a str,
    inputs: Vec<(String, String)>,
    call: String,
    limit: Duration,
    /// The runner the workflow runs on, as the project's declaration names it.
    runner: &'a str,
    /// Whether the run is a measurement's, which holds the turn on its machine itself; a run that is not waits, and is
    /// not timed, while a measurement holds the turn on its machine, as its jobs wait in the machine's gate then.
    measures: bool,
}

/// The runner that is GitHub's own machines, which a workflow's `runner` names `github`: they are never offline, and
/// each run has a machine of its own.
pub(super) const HOSTED: &str = "github";

/// The fewest runs a workflow's usual time is worked out of: with fewer, the median is one or two runs' time, and the
/// workflow has none.
const USUAL_OF: usize = 5;

/// How long the workflow's runs usually take: the median of the times of those of these runs that completed and were
/// not cancelled, the upper of the two middle ones where they are an even number; none where fewer than [`USUAL_OF`]
/// have a time.
pub(super) fn usual(runs: &[Run]) -> Option<Duration> {
    let mut took: Vec<u64> = runs.iter().filter_map(|run| run.took).collect();
    if took.len() < USUAL_OF {
        return None;
    }
    took.sort_unstable();
    Some(Duration::from_secs(took[took.len() / 2]))
}

/// Whether a runner of a kind is online.
enum Online {
    Yes,
    No,
    /// It could not be told: no layer of runners lists one, and GitHub's words for why a layer could not be read.
    Unknown(String),
}

/// Whether a run that has started waits for a runner: none of its jobs runs, and one waits for a runner to take it. A
/// run whose jobs wait on one another, as one that needs the others, is not told apart by this: a job runs first.
fn waits_for_a_runner(jobs: &[Check]) -> bool {
    !jobs.iter().any(|j| j.outcome == Outcome::Pending) && jobs.iter().any(|j| j.outcome == Outcome::Queued)
}

/// How long a run has waited for a runner and how long it has not, as the waits for it saw it at each look.
pub(super) struct Waiting {
    /// The time the run may take counts: the time it did not wait for a runner, and the time it waited where none of its
    /// kind was online.
    pub(super) ran_for: Duration,
    /// When it was last looked at, and whether it waited for a runner then, which holds until the next look.
    last: tokio::time::Instant,
    waiting: bool,
    /// Whether the time it waits is not counted: so until the runners are asked after and none of its kind is online,
    /// and again once one is or it stops waiting.
    excused: bool,
    /// Since when it has waited for a runner without a break; none while it does not.
    since: Option<tokio::time::Instant>,
    /// Before when the runners are not asked after again; none where they were not asked after.
    next: Option<tokio::time::Instant>,
}

impl Waiting {
    pub(super) fn new(now: tokio::time::Instant) -> Waiting {
        Waiting {
            ran_for: Duration::ZERO,
            last: now,
            waiting: false,
            excused: true,
            since: None,
            next: None,
        }
    }

    /// The run was looked at now, and does or does not wait for a runner: what came since the last look is counted
    /// where it did not wait then, or waited where none of its kind was online.
    pub(super) fn tick(&mut self, now: tokio::time::Instant, waiting: bool) {
        if !self.waiting || !self.excused {
            self.ran_for += now.saturating_duration_since(self.last);
        }
        self.last = now;
        self.waiting = waiting;
        if waiting {
            self.since.get_or_insert(now);
        } else {
            self.since = None;
            self.next = None;
            self.excused = true;
        }
    }

    /// How long it has waited for a runner without a break.
    pub(super) fn waited(&self, now: tokio::time::Instant) -> Duration {
        self.since
            .map_or(Duration::ZERO, |since| now.saturating_duration_since(since))
    }

    /// Whether the runners are to be asked after: it has waited for a window, and, where they were asked after, the
    /// window since has passed.
    pub(super) fn due(&self, now: tokio::time::Instant, window: Duration) -> bool {
        self.waited(now) >= window && self.next.is_none_or(|next| now >= next)
    }

    /// The runners were asked after now, and one of its kind was online or not: they are asked after again a window
    /// from now, and until then the time it waits is not counted where one was, and is where none was.
    pub(super) fn asked(&mut self, now: tokio::time::Instant, window: Duration, online: bool) {
        self.next = Some(now + window);
        self.excused = online;
    }
}

/// What a run of a workflow came to: its jobs, its results as the reader of what it uploaded read them, and what the
/// log of each job that failed with nothing in the results to say why says of it ([`Reader::unexplained`]).
struct Ran<T = results::Run> {
    run: Run,
    jobs: Vec<Check>,
    results: Result<T, String>,
    ends: Vec<StepEnd>,
}

/// How what a run uploaded in its artifact is read.
trait Reader: Sync {
    type Read: Send;

    /// What the files of the artifact say, or why they say nothing.
    fn read(&self, files: &[File]) -> Result<Self::Read, String>;

    /// The ids of the jobs that failed with nothing in what was read to say why, whose logs are read.
    fn unexplained(&self, jobs: &[Check], read: &Result<Self::Read, String>) -> Vec<u64>;
}

/// JUnit XML, as the fast and the full workflows report each test.
struct Junit;

impl Reader for Junit {
    type Read = results::Run;

    fn read(&self, files: &[File]) -> Result<results::Run, String> {
        results::read(files)
    }

    fn unexplained(&self, jobs: &[Check], read: &Result<results::Run, String>) -> Vec<u64> {
        unexplained(jobs, read).map(|job| job.id).collect()
    }
}

/// The CSV file a measurement's run uploads what it measured in, as its declaration names it.
struct Metrics<'a>(&'a Measurement);

impl Reader for Metrics<'_> {
    type Read = Table;

    fn read(&self, files: &[File]) -> Result<Table, String> {
        metrics::read(files, self.0)
    }

    /// Every job that failed: a measurement reports no test of its own to say why one did.
    fn unexplained(&self, jobs: &[Check], _: &Result<Table, String>) -> Vec<u64> {
        (jobs.iter())
            .filter(|job| matches!(job.outcome, Outcome::Failed(_)))
            .map(|job| job.id)
            .collect()
    }
}

/// What the log of a job says of the step that failed it: the end of what the step printed ([`log::failed_step`]), or
/// why the log could not be read.
struct StepEnd {
    job: u64,
    said: Result<String, String>,
}

/// The jobs of a run that failed with nothing in its results to say why: each that judged the code and found it
/// wanting and reported no failed test of its own, in results of its own as the names of their files tell
/// ([`results::job_of`]), as a lint that reports no test, or a job whose own step failed before its tests; and each
/// that failed, where the results could not be read.
fn unexplained<'a>(jobs: &'a [Check], read: &'a Result<results::Run, String>) -> impl Iterator<Item = &'a Check> {
    // A test the budget stopped or did not start says why its job failed as a failed test does.
    let ended = |outcome: results::Outcome| {
        matches!(
            outcome,
            results::Outcome::Failed | results::Outcome::Stopped | results::Outcome::NotStarted
        )
    };
    let own_failed = move |job: &Check| {
        (read.as_ref()).is_ok_and(|run| (run.cases.iter()).any(|c| c.job == job.name && ended(c.outcome)))
    };
    (jobs.iter()).filter(move |&job| matches!(job.outcome, Outcome::Failed(_)) && !own_failed(job))
}

/// The full workflows red with failed tests alone: what they found, said as the check says it, the names of the tests
/// they failed, and their runs' links.
struct Failing {
    first: String,
    tests: Vec<String>,
    urls: String,
}

/// What the tests the full workflows failed came to when they were run again: what fails the commit, or what holds it,
/// in words, and neither where it passes; and each test that failed and then passed, in words.
#[derive(Default)]
struct Again {
    failed: Option<String>,
    held: Option<String>,
    flaky: Vec<String>,
}

impl Again {
    /// The commit failed, as these say, each a paragraph of its own.
    fn failed(said: Vec<String>) -> Again {
        Again {
            failed: Some(said.join("\n\n")),
            ..Again::default()
        }
    }
}

/// The names of the tests the runs failed, each once, in the order the runs give them, where nothing but tests did
/// not pass: each run's results were read, none has a unit not built, each job that did not pass judged the code and
/// found it wanting, and each such job reported a failed test of its own in results of its own, or is among the jobs
/// `accounted` for without ([`GitHub::accounted`]). None where anything else did not pass, as a job of tests red for a
/// step before or beside them, or one that left no results, or where no test failed.
fn failed_alone(ran: &[Ran], accounted: &[String]) -> Option<Vec<String>> {
    let mut tests: Vec<String> = Vec::new();
    for one in ran {
        let run = one.results.as_ref().ok()?;
        let judged = (one.jobs.iter())
            .filter(|j| red_job(j))
            .all(|j| matches!(j.outcome, Outcome::Failed(_)));
        // A job that did not pass and reported no failed test of its own failed for something else, unless the run
        // again reproduces it or it only sums up the others.
        let own = (one.jobs.iter())
            .filter(|j| red_job(j))
            .all(|j| match run.reported.contains(&j.name) {
                true => (run.cases.iter()).any(|c| c.job == j.name && c.outcome == results::Outcome::Failed),
                false => accounted.contains(&j.name),
            });
        if !run.unbuilt.is_empty() || !judged || !own {
            return None;
        }
        for case in (run.cases.iter()).filter(|c| c.outcome == results::Outcome::Failed) {
            if !tests.contains(&case.test.name) {
                tests.push(case.test.name.clone());
            }
        }
    }
    (!tests.is_empty()).then_some(tests)
}

/// What became of each test named on the one run of the fast workflow, where the run judged them: its results were read
/// and every job of it ran to a conclusion. None where it judged nothing.
fn judged<'a>(ran: &'a [Ran], tests: &'a [String]) -> Option<Vec<results::Row<'a>>> {
    let [one] = ran else {
        return None;
    };
    let run = one.results.as_ref().ok()?;
    let concluded =
        (one.jobs.iter()).all(|j| !matches!(j.outcome, Outcome::Broken(_) | Outcome::Pending | Outcome::Queued));
    concluded.then(|| results::rows(tests, run))
}

/// How long the jobs and the tests of a run of the workflow took, as the work's records keep it: each job that ran to a
/// conclusion GitHub gave a time for, and each test the run ran and gave a time for. A run whose results could not be
/// read has its jobs only.
fn reading(ran: &Ran, source: Source, workflow: &str, request: Request<'_>) -> Reading {
    let jobs = (ran.jobs.iter())
        .filter_map(|job| {
            job.took.map(|seconds| Job {
                name: job.name.clone(),
                millis: seconds * 1000,
            })
        })
        .collect();
    let tests = match &ran.results {
        Ok(run) => (run.cases.iter())
            .filter(|case| !matches!(case.outcome, results::Outcome::Skipped | results::Outcome::NotStarted))
            .filter_map(|case| {
                case.millis.map(|millis| Test {
                    unit: case.test.unit.clone(),
                    name: case.test.name.clone(),
                    passed: case.outcome == results::Outcome::Passed,
                    millis,
                })
            })
            .collect(),
        Err(_) => Vec::new(),
    };
    Reading {
        asking: request.asking.name(),
        source,
        commit: request.commit.to_string(),
        workflow: workflow.to_string(),
        jobs,
        tests,
    }
    .normalised()
}

/// The input that says how long one test may run before it is stopped, in whole seconds, where the project's workflows
/// take one and the history of its merged checks says it.
pub(super) fn test_stop(input: &str, stop: &Stop) -> Option<(String, String)> {
    if input.is_empty() {
        return None;
    }
    Some((input.to_string(), stop.test_seconds()?.to_string()))
}

/// A test a request names whose median, by the project's earlier fast runs, is over the budget, or which a request is
/// refused with: its name, and its median in milliseconds, none where no fast run timed it.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Refused<'a> {
    pub(super) name: &'a str,
    pub(super) median: Option<f64>,
}

/// The tests of a request known to take longer than the budget, in whole seconds, together or one alone, by the median
/// of each one's times in the project's earlier fast runs: each test named, once, trimmed, in the order named, where
/// the request is refused; none where it is not. A test no fast run timed counts for nothing, and is never refused for
/// itself.
pub(super) fn over_budget<'a>(
    tests: &'a [String],
    budget: u64,
    median: impl Fn(&str) -> Option<f64>,
) -> Option<Vec<Refused<'a>>> {
    let mut seen = BTreeSet::new();
    let named: Vec<Refused<'a>> = (tests.iter())
        .map(|t| t.trim())
        .filter(|t| seen.insert(*t))
        .map(|name| Refused {
            name,
            median: median(name),
        })
        .collect();
    let limit = budget as f64 * 1000.0;
    let sum: f64 = named.iter().filter_map(|t| t.median).sum();
    (sum > limit).then_some(named)
}

/// Whole seconds of a time in milliseconds, rounded up: a time over a budget is said over it.
fn seconds_up(millis: f64) -> u64 {
    (millis / 1000.0).ceil() as u64
}

/// What a request refused for its budget is told, in the words of the tool's file: each test named with why.
fn refused_words(
    github: &flow::agents::GithubWords,
    result: &ResultWords,
    refused: &[Refused<'_>],
    budget: u64,
) -> String {
    let budget_s = budget.to_string();
    let sum = seconds_up(refused.iter().filter_map(|t| t.median).sum()).to_string();
    let lines: Vec<String> = (refused.iter())
        .map(|t| {
            let why = match t.median {
                Some(median) => {
                    let median_s = seconds_up(median).to_string();
                    let word = if median > budget as f64 * 1000.0 {
                        &github.refused_over
                    } else {
                        &github.refused_sum
                    };
                    fill(word, &[("median", &median_s), ("sum", &sum), ("budget", &budget_s)])
                }
                None => github.refused_untimed.clone(),
            };
            fill(&result.test, &[("name", t.name), ("became", &why), ("time", "")])
        })
        .collect();
    fill(
        &github.over_budget,
        &[("budget", &budget_s), ("tests", &lines.join("\n"))],
    )
}

/// The record of what a request refused for its budget kept from running: every test named.
fn refused_record(refused: &[Refused<'_>]) -> Vec<work::Short> {
    (refused.iter())
        .map(|t| work::Short {
            name: t.name.to_string(),
            how: work::ShortHow::Refused,
            millis: t.median.map(|m| m.round() as u64),
        })
        .collect()
}

/// The record of what the budget of a run kept from running to its end: each test named, once, that it stopped or did
/// not start, with how long a stopped one ran, its cases' times together.
fn short_record(tests: &[String], run: &results::Run) -> Vec<work::Short> {
    let mut seen = BTreeSet::new();
    (results::rows(tests, run).into_iter())
        .filter(|row| seen.insert(row.named.trim()))
        .filter_map(|row| {
            let how = match row.became {
                Became::Stopped => work::ShortHow::Stopped,
                Became::NotStarted => work::ShortHow::NotStarted,
                _ => return None,
            };
            let times: Vec<u64> = (row.matched.iter())
                .filter(|c| c.outcome == results::Outcome::Stopped)
                .filter_map(|c| c.millis)
                .collect();
            Some(work::Short {
                name: row.named.trim().to_string(),
                how,
                millis: (!times.is_empty()).then(|| times.iter().sum()),
            })
        })
        .collect()
}

/// What a request is run by.
pub(super) enum Role<'a> {
    Fast(&'a CiFast),
    Full(&'a CiFull),
}

/// A measurement a request names, as the project declares it, with the inputs the request sets, and its place among
/// the measurements the request names, which tells its run from that of another of them.
pub(super) struct Measuring<'a> {
    pub(super) name: &'a str,
    pub(super) declared: &'a Measurement,
    pub(super) set: &'a BTreeMap<String, String>,
    pub(super) place: usize,
}

impl<A: Api> GitHub<A> {
    /// The words of the tool, or why there are none.
    fn tool(&self) -> Result<&Tool, String> {
        self.config
            .tool
            .as_ref()
            .ok_or_else(|| "the words of run_tests (agents/tools/run_tests.toml) are not given".to_string())
    }

    /// The budget of what asked, in whole seconds: for a call of `run_tests`, the project's `ci.fast.budget`, or else
    /// the one the tool's file gives every project; none for a round of the check before a merge, whose runs again of
    /// what failed run whole, as the full workflows did.
    pub(super) fn budget(&self, asking: Asking) -> Result<Option<u64>, String> {
        if matches!(asking, Asking::Round(..)) {
            return Ok(None);
        }
        let own = (self.config.ci.as_ref()).and_then(|ci| ci.fast.budget.as_deref());
        let (key, text) = match own {
            Some(own) => ("ci.fast.budget", own),
            None => {
                let fast = (self.tool()?.fast.as_ref())
                    .ok_or("the words of run_tests (agents/tools/run_tests.toml) have no [fast]")?;
                ("fast.budget of run_tests", fast.budget.as_str())
            }
        };
        flow::agents::budget_of(text)
            .map(Some)
            .map_err(|why| format!("{key}: {why}"))
    }

    /// What a request of `run_tests` whose tests are known by the project's earlier fast runs to take longer than its
    /// budget is told, where it is ([`over_budget`]), said as kept from running; none where it may run, or where it
    /// names no test. Nothing has run or been pushed.
    pub(super) fn refused_for_time(
        &self,
        named: &Named,
        asking: Asking,
        stop: &Stop,
    ) -> Result<Option<String>, String> {
        let Some(budget) = self.budget(asking)? else {
            return Ok(None);
        };
        let Some(refused) = over_budget(&named.tests, budget, |test| stop.fast_median(test)) else {
            return Ok(None);
        };
        let tool = self.tool()?;
        let github = (tool.github.as_ref()).ok_or("the words of run_tests have no [github]")?;
        let result = (tool.result.as_ref()).ok_or("the words of run_tests have no [result]")?;
        stop.short_of_time(asking, budget, refused_record(&refused));
        Ok(Some(refused_words(github, result, &refused, budget)))
    }

    /// Why a runner is not one this installation has, in the words of the tool's file; none where it is.
    fn runner_missing(&self, runner: &str) -> Result<Option<String>, String> {
        let words = self
            .tool()?
            .github
            .as_ref()
            .ok_or("the words of run_tests have no [github]")?;
        let have = self.config.runners.join(", ");
        Ok((!self.config.runners.iter().any(|r| r == runner))
            .then(|| fill(&words.runner_missing, &[("runner", runner), ("have", &have)])))
    }

    /// Why a request that names tests or checks cannot be run by the project's fast workflow, in the words of the tool's
    /// file, or that workflow as its role. A request that names neither is refused by its caller, before this is asked.
    pub(super) fn role<'a>(&'a self, ci: Option<&'a Ci>, named: &Named) -> Result<Result<Role<'a>, String>, String> {
        let words = self
            .tool()?
            .github
            .as_ref()
            .ok_or("the words of run_tests have no [github]")?;
        let Some(ci) = ci else {
            return Ok(Err(words.no_fast.clone()));
        };
        let fast = &ci.fast;
        let unknown: Vec<&str> = (named.checks.iter())
            .filter(|c| !fast.checks.contains(c))
            .map(String::as_str)
            .collect();
        if !unknown.is_empty() {
            let (runs, unknown) = (fast.checks.join(", "), unknown.join(", "));
            return Ok(Err(fill(
                &words.unknown_checks,
                &[("runs", &runs), ("unknown", &unknown)],
            )));
        }
        Ok(self.runner_missing(&fast.runner)?.map_or(Ok(Role::Fast(fast)), Err))
    }

    /// The measurements a request names as the project declares them, or why the request cannot be run, in the words of
    /// the tool's file, every reason at once, each once: the project declares no measurement; it declares none of a
    /// name named; a measurement named is set an input it does not let an agent set; or it runs on a machine the
    /// installation lacks. A request that names none does not ask this.
    pub(super) fn measurements<'a>(
        &'a self,
        ci: Option<&'a Ci>,
        named: &'a [Measure],
    ) -> Result<Result<Vec<Measuring<'a>>, String>, String> {
        let words = self
            .tool()?
            .github
            .as_ref()
            .ok_or("the words of run_tests have no [github]")?;
        let declared = match ci {
            Some(ci) if !ci.measure.is_empty() => &ci.measure,
            _ => return Ok(Err(words.no_measure.clone())),
        };
        let mut refused: Vec<String> = Vec::new();
        let mut refuse = |why: String| {
            if !refused.contains(&why) {
                refused.push(why);
            }
        };
        let mut measuring = Vec::new();
        for (place, asked) in named.iter().enumerate() {
            let Some((name, measurement)) = declared.get_key_value(&asked.name) else {
                let names: Vec<String> = declared.keys().map(|name| format!("`{name}`")).collect();
                refuse(fill(
                    &words.unknown_measure,
                    &[("name", &asked.name), ("declared", &names.join(", "))],
                ));
                continue;
            };
            let closed: Vec<&str> = (asked.inputs.keys())
                .filter(|input| !measurement.open.contains(*input))
                .map(String::as_str)
                .collect();
            if !closed.is_empty() {
                let open: Vec<String> = measurement.open.iter().map(|input| format!("`{input}`")).collect();
                let open = if open.is_empty() {
                    words.none_open.clone()
                } else {
                    open.join(", ")
                };
                let closed: Vec<String> = closed.iter().map(|input| format!("`{input}`")).collect();
                refuse(fill(
                    &words.not_open,
                    &[("name", name), ("inputs", &closed.join(", ")), ("open", &open)],
                ));
                continue;
            }
            if let Some(missing) = self.runner_missing(&measurement.runner)? {
                refuse(missing);
                continue;
            }
            measuring.push(Measuring {
                name,
                declared: measurement,
                set: &asked.inputs,
                place,
            });
        }
        Ok(match refused.is_empty() {
            true => Ok(measuring),
            false => Err(refused.join("\n")),
        })
    }

    /// The machines this project's measurements run on that one measurement at a time runs on, each once: every runner
    /// they name but GitHub's own machines.
    pub(super) fn measured_on(&self) -> Vec<String> {
        let ci = self.config.ci.as_ref();
        let mut machines: Vec<String> = (ci.into_iter().flat_map(|ci| ci.measure.values()))
            .map(|measurement| measurement.runner.clone())
            .filter(|runner| runner != HOSTED)
            .collect();
        machines.sort();
        machines.dedup();
        machines
    }

    /// The runs of this project's measurements on the machine that have not completed, by their links: each run by hand
    /// on main of a workflow a measurement on it names, as the look every wait on that workflow shares found them.
    pub(super) async fn measuring_on(&self, machine: &str) -> Result<Vec<String>, Failed> {
        let ci = self.config.ci.as_ref();
        let mut workflows: Vec<&str> = (ci.into_iter().flat_map(|ci| ci.measure.values()))
            .filter(|measurement| measurement.runner == machine)
            .map(|measurement| measurement.workflow.as_str())
            .collect();
        workflows.sort_unstable();
        workflows.dedup();
        let mut going = Vec::new();
        for workflow in workflows {
            let found = (self.looks)
                .runs(&self.api, workflow, &self.project.main, self.config.fast_poll)
                .await?;
            going.extend((found.runs.into_iter()).filter(|run| !run.completed).map(|run| run.url));
        }
        Ok(going)
    }

    /// Runs the measurement on the commit, pushed already, and says what became of it; none where it was told to stop.
    /// Its workflow is run by its file on main, given the inputs its declaration gives, the commit, the commit the
    /// work's change began from and the call filled in, and those the request sets. On a machine of the project's own
    /// the run waits for its turn first, and holds it until its result is read; a run the request asked for before,
    /// which GitHub lists, is on the machine already and takes no turn.
    pub(super) async fn run_measurement(
        &self,
        measuring: &Measuring<'_>,
        base: &str,
        request: Request<'_>,
        stop: &mut Stop,
    ) -> Result<Option<String>, Failed> {
        let declared = measuring.declared;
        let tool = self.tool()?;
        let words = (tool.result.as_ref()).ok_or("the words of run_tests have no [result]")?;
        let said = (tool.measurement.as_ref()).ok_or("the words of run_tests have no [measurement]")?;
        let key = format!("ci.measure.{}.too_long", measuring.name);
        let limit = flow::agents::span(&declared.too_long).map_err(|why| format!("{key}: {why}"))?;
        let asking = format!(
            "{}/measurement/{}/{}",
            declared.workflow, measuring.place, measuring.name
        );
        let call = self.call(request, &asking);
        let plan = Plan {
            workflow: &declared.workflow,
            results: &declared.artifact,
            inputs: declared.inputs_of(request.commit, base, &call, measuring.set),
            call,
            limit,
            runner: &declared.runner,
            measures: true,
        };
        let turn = match declared.runner == HOSTED {
            true => None,
            false => {
                let Some((asked, _, _)) = self.runs_of(&plan, Duration::ZERO, stop).await? else {
                    return Ok(None);
                };
                match asked.iter().any(|run| !run.cancelled) {
                    true => None,
                    false => match (self.config.machines)
                        .turn(&declared.runner, request.work, self.config.fast_poll, stop)
                        .await?
                    {
                        Some(turn) => Some(turn),
                        None => return Ok(None),
                    },
                }
            }
        };
        let Some(ran) = self.execute(&plan, request.commit, stop, &Metrics(declared)).await? else {
            return Ok(None);
        };
        drop(turn);
        Ok(Some(measured(words, said, measuring, request.commit, base, &ran)))
    }

    /// Why the check before a merge cannot run the project's full workflows, in the words of the check's file, or those
    /// workflows as their role.
    pub(super) fn full_role<'a>(&'a self, ci: Option<&'a Ci>) -> Result<Result<Role<'a>, String>, String> {
        let check = (self.config.check.as_ref())
            .ok_or("the words of the check before a merge (agents/acts/check.toml) are not given")?;
        let Some(ci) = ci else {
            return Ok(Err(check.no_full.clone()));
        };
        Ok(self
            .runner_missing(&ci.full.runner)?
            .map_or(Ok(Role::Full(&ci.full)), Err))
    }

    /// Runs the workflows of the role on the commit, pushed already, and says what became of them; none where it was
    /// told to stop. The workflows are run by their files on main, so that no work chooses what runs on the project's
    /// machines, and GitHub always has those files and that branch.
    pub(super) async fn run_role(
        &self,
        role: Role<'_>,
        request: Request<'_>,
        named: &Named,
        stop: &mut Stop,
    ) -> Result<Option<String>, Failed> {
        let Some(ran) = self.execute_role(&role, request, named, stop).await? else {
            return Ok(None);
        };
        let words = &self
            .tool()?
            .result
            .as_ref()
            .ok_or("the words of run_tests have no [result]")?;
        let on_main = OnMain {
            main: &self.project.main,
            red: stop.red_tests(),
        };
        Ok(Some(said(words, &role, request.commit, &ran, named, Some(&on_main))))
    }

    /// The jobs of the full workflows that did not pass may be let through with no failed test of their own, as the
    /// project's own declaration and workflows say them, the files of them as main holds them at `main`, the head the
    /// commit checked holds: each check the fast workflow runs by name, which the run again runs and so finds red
    /// again; and each job of a full workflow that needs every other job of it, which only sums them up
    /// ([`super::workflow::summing`]). A workflow whose file cannot be read lets none through for summing up.
    pub(super) async fn accounted(&self, main: &str) -> Vec<String> {
        let ci = self.config.ci.as_ref();
        let mut accounted = ci.map_or_else(Vec::new, |ci| ci.fast.checks.clone());
        let workflows = ci.map_or(&[][..], |ci| &ci.full.workflows[..]);
        for workflow in workflows {
            let file = format!("{main}:.github/workflows/{workflow}");
            if let Ok(said) = git::said(&self.project.repo, &["show", &file]).await
                && said.ok
            {
                accounted.extend(super::workflow::summing(&said.out));
            }
        }
        accounted
    }

    /// The value the run of the workflow asked for by the request holds in its name: a name-based UUID (version 5) of
    /// the work, the request, the workflow and the commit, in the namespace of this installation, so that the request
    /// given again works out the same, and no other request, of this installation or another, has it.
    pub(super) fn call(&self, request: Request<'_>, workflow: &str) -> String {
        let name = format!(
            "work/{}/{}/workflow/{workflow}/commit/{}",
            request.work.0,
            request.asking.name(),
            request.commit
        );
        let call = Uuid::new_v5(&self.config.installation, name.as_bytes());
        call.simple().to_string()
    }

    /// Runs the workflows of the role, one after the other, and gives what each came to; none where it was told to
    /// stop.
    async fn execute_role(
        &self,
        role: &Role<'_>,
        request: Request<'_>,
        named: &Named,
        stop: &mut Stop,
    ) -> Result<Option<Vec<Ran>>, Failed> {
        let commit = request.commit;
        let mut ran = Vec::new();
        match role {
            Role::Fast(fast) => {
                let call = self.call(request, &fast.workflow);
                let tests: String = (named.tests.iter()).map(|t| format!("{}\n", t.trim())).collect();
                let mut inputs = vec![
                    (fast.commit_input.clone(), commit.to_string()),
                    (fast.call_input.clone(), call.clone()),
                    (fast.tests_input.clone(), tests),
                ];
                if !named.checks.is_empty() {
                    let checks = serde_json::to_string(&named.checks).unwrap_or_default();
                    inputs.push((fast.checks_input.clone(), checks));
                }
                inputs.extend(test_stop(&fast.test_stop_input, stop));
                let budget = self.budget(request.asking)?;
                if let Some(budget) = budget
                    && !fast.budget_input.is_empty()
                {
                    inputs.push((fast.budget_input.clone(), budget.to_string()));
                }
                if !fast.work_input.is_empty() {
                    inputs.push((fast.work_input.clone(), request.work.0.to_string()));
                }
                let limit = flow::agents::span(&fast.too_long).map_err(|why| format!("ci.fast.too_long: {why}"))?;
                let plan = Plan {
                    workflow: &fast.workflow,
                    results: &fast.results,
                    inputs,
                    call,
                    limit,
                    runner: &fast.runner,
                    measures: false,
                };
                match self.execute(&plan, commit, stop, &Junit).await? {
                    Some(one) => {
                        stop.timed(reading(&one, Source::Fast, &fast.workflow, request));
                        if let (Some(budget), Ok(run)) = (budget, &one.results) {
                            stop.short_of_time(request.asking, budget, short_record(&named.tests, run));
                        }
                        ran.push(one);
                    }
                    None => return Ok(None),
                }
            }
            Role::Full(full) => {
                let limit = flow::agents::span(&full.too_long).map_err(|why| format!("ci.full.too_long: {why}"))?;
                for workflow in &full.workflows {
                    let call = self.call(request, workflow);
                    let mut inputs = vec![
                        (full.commit_input.clone(), commit.to_string()),
                        (full.call_input.clone(), call.clone()),
                    ];
                    inputs.extend(test_stop(&full.test_stop_input, stop));
                    let plan = Plan {
                        workflow,
                        results: &full.results,
                        inputs,
                        call,
                        limit,
                        runner: &full.runner,
                        measures: false,
                    };
                    match self.execute(&plan, commit, stop, &Junit).await? {
                        Some(one) => {
                            stop.timed(reading(&one, Source::Full, workflow, request));
                            ran.push(one);
                        }
                        None => return Ok(None),
                    }
                }
            }
        }
        Ok(Some(ran))
    }

    /// The verdict of the check before a merge on the commit, pushed already, which holds the head of main `main`: the
    /// project's full workflows run on it, once, and judged by what they uploaded and by their jobs: it failed where a
    /// test failed, where a job did not pass, or where a run uploaded no JUnit the engine can read, said as `run_tests`
    /// says what a run found, of what did not pass; but where tests failed and nothing else did, they are run again
    /// ([`GitHub::again`]). None where it was told to stop.
    pub(super) async fn judge_full(
        &self,
        request: Request<'_>,
        main: &str,
        stop: &mut Stop,
    ) -> Result<Option<Verdict>, Failed> {
        let (commit, named) = (request.commit, Named::default());
        let role = match self.full_role(self.config.ci.as_ref())? {
            Ok(role) => role,
            Err(refused) => return Err(Failed::refused(refused)),
        };
        let Some(ran) = self.execute_role(&role, request, &named, stop).await? else {
            return Ok(None);
        };
        let words = &self
            .tool()?
            .result
            .as_ref()
            .ok_or("the words of run_tests have no [result]")?;
        let red = ran.iter().any(|one| match &one.results {
            Ok(run) => run.cases.iter().any(|c| c.outcome == results::Outcome::Failed) || one.jobs.iter().any(red_job),
            Err(_) => true,
        });
        let urls: Vec<&str> = ran.iter().map(|r| r.run.url.as_str()).collect();
        // What failed is said above with its jobs that did not pass; the note gives each other job, so that no job is
        // told twice and, where the check passed, every job is.
        let others: Vec<String> = (ran.iter().flat_map(|r| &r.jobs))
            .filter(|j| !red_job(j))
            .map(|j| job_line(words, j))
            .collect();
        let notes = if others.is_empty() {
            Vec::new()
        } else {
            vec![fill(&words.verdict_jobs, &[("jobs", &others.join("\n  "))])]
        };
        let mut verdict = Verdict {
            by: Some(fill(&words.verdict_by, &[("urls", &urls.join(", "))])),
            notes,
            ..Verdict::default()
        };
        if !red {
            return Ok(Some(verdict));
        }
        let first = said(words, &role, commit, &ran, &named, None);
        let accounted = self.accounted(main).await;
        let Some(tests) = failed_alone(&ran, &accounted) else {
            verdict.failed = Some(first);
            return Ok(Some(verdict));
        };
        let failed = Failing {
            first,
            tests,
            urls: urls.join(", "),
        };
        let Some(again) = self.again(request, main, failed, words, stop).await? else {
            return Ok(None);
        };
        verdict.failed = again.failed;
        verdict.held = again.held;
        verdict.notes.extend(again.flaky.iter().cloned());
        verdict.observations = again.flaky;
        Ok(Some(verdict))
    }

    /// Runs again by the fast workflow the tests the full workflows failed, they alone having failed: on the commit,
    /// beside every check the fast workflow runs by name, so that what else the full workflows found red is found red
    /// again; and what failed again there, on the head of main `main`. A test that passed on the commit again may be
    /// flaky, and is said so. Where every test passed there and every check did, the commit passes; where every test
    /// that failed again failed on main as well, the commit is held, its change not being what fails it; and otherwise
    /// it failed, said as the full workflows found it, then as each run again found it. A run again that judged
    /// nothing, as one whose results cannot be read, and tests that could not be run again, not run, skipped or not
    /// built on the commit, fail the commit as the full workflows found it and as that run found it, which says why;
    /// such a test is not said to have failed again. None where it was told to stop.
    async fn again(
        &self,
        request: Request<'_>,
        main: &str,
        failing: Failing,
        words: &ResultWords,
        stop: &mut Stop,
    ) -> Result<Option<Again>, Failed> {
        let Failing { first, tests, urls } = failing;
        let check = (self.config.check.as_ref())
            .ok_or("the words of the check before a merge (agents/acts/check.toml) are not given")?;
        let ci = self.config.ci.as_ref();
        let checks = ci.map_or_else(Vec::new, |ci| ci.fast.checks.clone());
        let named = Named {
            tests: tests.clone(),
            checks,
            ..Named::default()
        };
        let role = match self.role(ci, &named)? {
            Ok(role) => role,
            Err(why) => {
                return Ok(Some(Again::failed(vec![
                    first,
                    fill(&check.again_not_run, &[("why", &why)]),
                ])));
            }
        };
        let commit = request.commit;
        let Some(on_commit) = self.execute_role(&role, request, &named, stop).await? else {
            return Ok(None);
        };
        let section = |word: &str, head: &str, ran: &[Ran], named: &Named| {
            let run = said(words, &role, head, ran, named, None);
            let slots = [("main", self.project.main.as_str()), ("head", head), ("run", &run)];
            fill(word, &slots)
        };
        let again_on_commit = section(&check.again_on_commit, commit, &on_commit, &named);
        let Some(rows) = judged(&on_commit, &tests) else {
            return Ok(Some(Again::failed(vec![first, again_on_commit])));
        };
        let again_url: Vec<&str> = on_commit.iter().map(|r| r.run.url.as_str()).collect();
        let again_url = again_url.join(", ");
        let flaky: Vec<String> = (rows.iter())
            .filter(|row| row.became == Became::Passed)
            .map(|row| {
                let slots = [
                    ("test", row.named),
                    ("commit", commit),
                    ("failed", &urls),
                    ("again", &again_url),
                ];
                fill(&check.flaky, &slots)
            })
            .collect();
        let red_again: Vec<String> = (rows.iter())
            .filter(|row| row.became == Became::Failed)
            .map(|row| row.named.to_string())
            .collect();
        // A test the run again did not run, skipped or could not build was not run again, and has not failed again: the
        // commit fails as the run again found it, which says what became of the test, and nothing is run on main.
        let not_run_again = (rows.iter()).any(|row| !matches!(row.became, Became::Passed | Became::Failed));
        let checks_red = (named.checks.iter()).any(|check| {
            let jobs: Vec<&Check> = (on_commit.iter().flat_map(|r| &r.jobs))
                .filter(|j| &j.name == check)
                .collect();
            jobs.is_empty() || jobs.into_iter().any(red_job)
        });
        if checks_red || not_run_again {
            return Ok(Some(Again {
                flaky,
                ..Again::failed(vec![first, again_on_commit])
            }));
        }
        if red_again.is_empty() {
            return Ok(Some(Again {
                flaky,
                ..Again::default()
            }));
        }
        let at_main = Request {
            work: request.work,
            commit: main,
            asking: request.asking,
        };
        let named_main = Named {
            tests: red_again.clone(),
            ..Named::default()
        };
        let Some(on_main) = self.execute_role(&role, at_main, &named_main, stop).await? else {
            return Ok(None);
        };
        let again_on_main = section(&check.again_on_main, main, &on_main, &named_main);
        let Some(rows) = judged(&on_main, &red_again) else {
            return Ok(Some(Again {
                flaky,
                ..Again::failed(vec![first, again_on_commit, again_on_main])
            }));
        };
        let filled = |word: &str, tests: &[&str]| {
            let named: Vec<String> = (tests.iter())
                .map(|t| fill(&check.test_named, &[("test", t)]))
                .collect();
            let tests = named.join(&check.tests_between);
            fill(word, &[("main", &self.project.main), ("head", main), ("tests", &tests)])
        };
        // A test that failed again and did not fail on main, passing there or not run there, as one the change adds, is
        // no red of main's: it fails the commit.
        let only_here: Vec<&str> = (rows.iter())
            .filter(|row| row.became != Became::Failed)
            .map(|row| row.named)
            .collect();
        if only_here.is_empty() {
            let red: Vec<&str> = red_again.iter().map(String::as_str).collect();
            let held = [filled(&check.on_main_too, &red), first, again_on_commit, again_on_main];
            return Ok(Some(Again {
                held: Some(held.join("\n\n")),
                flaky,
                ..Again::default()
            }));
        }
        let mut failures = vec![
            first,
            again_on_commit,
            again_on_main,
            filled(&check.only_here, &only_here),
        ];
        // What failed on main as well is said so, that what the change did not make is not looked for in the change.
        let also_on_main: Vec<&str> = (rows.iter())
            .filter(|row| row.became == Became::Failed)
            .map(|row| row.named)
            .collect();
        if !also_on_main.is_empty() {
            failures.push(filled(&check.on_main_too, &also_on_main));
        }
        Ok(Some(Again {
            flaky,
            ..Again::failed(failures)
        }))
    }

    /// Waits for the run of the workflow the plan asks for, and reads what it uploaded: the run its request asked for
    /// already, before the engine started again, where one was not cancelled; or, where none was, one the workflow is
    /// dispatched for now with the plan's inputs. Of two runs of the request, the first asked for is waited for, which
    /// began first. None where it was told to stop.
    ///
    /// A run waits for a runner while GitHub says it is queued, or while it is under way and none of its jobs runs and
    /// one waits ([`waits_for_a_runner`]). A run is under way once a job of it has started, which its jobs tell: GitHub
    /// says a run queued again when a job has run and the next waits for a runner, so the jobs of a run it says is
    /// queued are read where that decides what is done, once it has waited a window, and a run seen under way once is
    /// under way after. Once it has waited as long as the CI may take to start, the engine asks GitHub whether a
    /// runner of the workflow's kind is online ([`GitHub::online`]), and asks again after each further such time.
    /// Where one is, busy or not, the run waits on, said to wait for that runner, and its wait is no time it is judged
    /// by. Where none is, or it cannot be told, a run no job of which has started is cancelled where it is listed, so
    /// that it does not run later for nothing, and holds what asked ([`Failed::held`]): what stops it is no work's
    /// (GitHub, or a runner offline), and asked for again after a wait another run is asked for, which may start. A run
    /// under way is never so cancelled, since what its jobs did would be lost: it waits on, and the time it waits until
    /// a runner of its kind is online again is counted as time it ran. A run that is not complete in the time it may
    /// take is cancelled, and is what it found: its jobs not complete said so, and no test's result known.
    ///
    /// The runs are looked at by the look every wait on the workflow shares ([`super::looks`]): the first time by one
    /// sent now, so that a run the request asked for moments before is found; then by the last one, where it is younger
    /// than [`Config::fast_poll`](super::Config::fast_poll), and the wait sleeps until that one is that old, so that the
    /// waits on a workflow look at it together, once each time. The jobs of a run that has started are looked at in the
    /// same way. Once the run is found, it is looked at so only near the time it is expected to end, from half the time
    /// the workflow's runs listed with it usually take ([`usual`]) after the wait began; before that, every
    /// [`Config::slow_poll`](super::Config::slow_poll). A workflow with no usual time is looked at often throughout. A
    /// run asked for before the engine started again is timed from the wait, which began later than the run.
    async fn execute<R: Reader>(
        &self,
        plan: &Plan<'_>,
        commit: &str,
        stop: &mut Stop,
        reader: &R,
    ) -> Result<Option<Ran<R::Read>>, Failed> {
        let mut run = None;
        let ran = self.wait_for(plan, commit, stop, &mut run, reader).await;
        if let Some(run) = run {
            self.looks.forget(run);
        }
        ran
    }

    /// [`GitHub::execute`], which leaves in `run` the number of the run it found, so that the look at its jobs is let go
    /// however the wait ends.
    async fn wait_for<R: Reader>(
        &self,
        plan: &Plan<'_>,
        commit: &str,
        stop: &mut Stop,
        found: &mut Option<u64>,
        reader: &R,
    ) -> Result<Option<Ran<R::Read>>, Failed> {
        let workflow = plan.workflow;
        let main = &self.project.main;
        let (fast, slow) = (self.config.fast_poll, self.config.slow_poll);
        let start = self.config.start;
        let Some((asked, _, _)) = self.runs_of(plan, Duration::ZERO, stop).await? else {
            return Ok(None);
        };
        // A run cancelled judged nothing, and is passed over: the request asks for another.
        let cancelled: Vec<u64> = (asked.iter()).filter(|r| r.cancelled).map(|r| r.id).collect();
        if asked.len() == cancelled.len() {
            (self.api.dispatch(workflow, main, &plan.inputs).await).map_err(|failed| {
                failed.said(|why| format!("GitHub refused to run {workflow} on {main} for {commit}: {why}"))
            })?;
        }
        let began = tokio::time::Instant::now();
        let mut waiting = Waiting::new(began);
        // Whether a job of the run has started, as GitHub's word for the run or, where that is queued, its jobs said.
        let mut under_way = false;
        // From when the run is near the time it is expected to end; none until it is found, or where its workflow has
        // no usual time.
        let mut near: Option<tokio::time::Instant> = None;
        let run = loop {
            let poll = match near {
                Some(near) if tokio::time::Instant::now() < near => slow,
                _ => fast,
            };
            let Some((runs, at, usual)) = self.runs_of(plan, poll, stop).await? else {
                return Ok(None);
            };
            let mine = (runs.into_iter())
                .filter(|r| !cancelled.contains(&r.id))
                .min_by_key(|r| r.id);
            if let Some(run) = &mine {
                *found = Some(run.id);
            }
            under_way |= mine.as_ref().is_some_and(|run| run.started);
            // GitHub says a run queued only while none of its jobs runs, so that alone says it waits for a runner.
            let queued = match &mine {
                None => false,
                Some(run) if run.completed => false,
                Some(run) if !run.started => true,
                Some(run) => {
                    let Some(jobs) = self.jobs_of(run.id, poll, stop).await? else {
                        return Ok(None);
                    };
                    waits_for_a_runner(&jobs)
                }
            };
            let now = tokio::time::Instant::now();
            // A run on a machine a measurement holds may wait in the machine's gate, and that wait is the
            // measurement's, not the run's: it is not timed, as a wait for a runner is not.
            let measured_beside = !plan.measures && self.config.machines.held(plan.runner);
            waiting.tick(now, queued || measured_beside);
            // Once its run is found, the call or the act runs as it, at its link, and says the runner it waits for.
            if let Some(run) = &mine {
                stop.say(Some(Along::Runs {
                    at: run.url.clone(),
                    queued_for: queued.then(|| plan.runner.to_string()),
                }));
            }
            // Once the run is found, it is near its end from half the workflow's usual time after the wait began.
            let near_from = mine.as_ref().and(usual).map(|usual| began + usual / 2);
            match mine {
                Some(run) if run.completed => break run,
                None if began.elapsed() >= start => {
                    return Err(Failed::held(
                        format!(
                            "the run of {workflow} asked for on {commit} never started: GitHub listed none after {}",
                            super::span(start)
                        ),
                        Obstacle::Fault,
                        None,
                    ));
                }
                Some(run) if queued && waiting.due(now, start) => {
                    if !under_way {
                        let Some(jobs) = self.jobs_of(run.id, poll, stop).await? else {
                            return Ok(None);
                        };
                        under_way = jobs.iter().any(|job| job.outcome.started());
                    }
                    let online = self.online(plan.runner).await;
                    waiting.asked(now, start, matches!(online, Online::Yes));
                    let (asked_of, waited) = (plan.runner, super::span(waiting.waited(now)));
                    let never = format!("the run of {workflow} asked for on {commit} never started");
                    // Listed at once as queued where no runner is online: given up as never started, not waited for
                    // until the run would time out.
                    let said = match online {
                        Online::Yes => None,
                        _ if under_way => None,
                        Online::No => Some(format!(
                            "{never}: no runner took it after {waited} ({}), none of {asked_of} is online, and the \
                             engine cancelled the run",
                            run.url
                        )),
                        Online::Unknown(why) => Some(format!(
                            "{never}: no runner took it after {waited} ({}), the engine could not tell whether a \
                             runner of {asked_of} is online ({why}), and cancelled the run",
                            run.url
                        )),
                    };
                    if let Some(said) = said {
                        self.api.cancel(run.id).await?;
                        return Err(Failed::held(said, Obstacle::NoRunner, None));
                    }
                }
                // Its jobs are read before it is cancelled, so that those still under way are said not complete.
                Some(run) if waiting.ran_for >= plan.limit => {
                    let Some(jobs) = self.jobs_of(run.id, Duration::ZERO, stop).await? else {
                        return Ok(None);
                    };
                    self.api.cancel(run.id).await?;
                    let results = Err(format!(
                        "the run did not complete within {}, the time it may take, and the engine cancelled it",
                        super::span(plan.limit)
                    ));
                    let ends = self.step_ends(&jobs, &reader.unexplained(&jobs, &results)).await?;
                    return Ok(Some(Ran {
                        run,
                        jobs,
                        results,
                        ends,
                    }));
                }
                _ => {}
            }
            near = near_from;
            let next = match near {
                Some(near) if now < near => (at + slow).min(near),
                _ => at + fast,
            };
            tokio::select! {
                () = tokio::time::sleep_until(next) => {}
                () = stop.told() => return Ok(None),
            }
        };
        let Some(jobs) = self.jobs_of(run.id, Duration::ZERO, stop).await? else {
            return Ok(None);
        };
        let results = match self.api.artifact(run.id, plan.results).await {
            Ok(Some(files)) => (reader.read(&files)).map_err(|why| format!("the artifact {}: {why}", plan.results)),
            Ok(None) => Err(format!("the run uploaded no artifact {}", plan.results)),
            // GitHub's limit, or its own fault, keeps the results from being read for now: not a run without results.
            Err(failed) if failed.hindrance.is_some() => return Err(failed),
            Err(why) => Err(format!("the artifact {} could not be fetched: {why}", plan.results)),
        };
        let ends = self.step_ends(&jobs, &reader.unexplained(&jobs, &results)).await?;
        Ok(Some(Ran {
            run,
            jobs,
            results,
            ends,
        }))
    }

    /// Whether a runner of this kind is online, as the repository's runners and its organisation's say: GitHub's own
    /// machines always are; any other is, where a layer lists an online runner, busy or not, with a label equal to it,
    /// ignoring the case; it cannot be told where no layer lists one and a layer could not be read, as a token that may
    /// not read runners is refused.
    async fn online(&self, runner: &str) -> Online {
        if runner == HOSTED {
            return Online::Yes;
        }
        let listed = self.api.runners().await;
        let mut unread = Vec::new();
        let mut online = false;
        for layer in [Some(listed.repository), listed.organisation].into_iter().flatten() {
            match layer {
                Ok(runners) => {
                    online |= (runners.iter())
                        .any(|r| r.online && r.labels.iter().any(|label| label.eq_ignore_ascii_case(runner)));
                }
                Err(why) => unread.push(why),
            }
        }
        match (online, unread.is_empty()) {
            (true, _) => Online::Yes,
            (false, true) => Online::No,
            (false, false) => Online::Unknown(unread.join("; ")),
        }
    }

    /// What the log of each job of a run that failed with nothing in what it uploaded to say why, by its id, says of
    /// the step that failed it ([`Reader::unexplained`]); a log that cannot be read is said so, and fails nothing else.
    async fn step_ends(&self, jobs: &[Check], unexplained: &[u64]) -> Result<Vec<StepEnd>, Failed> {
        let mut ends = Vec::new();
        for job in jobs.iter().filter(|job| unexplained.contains(&job.id)) {
            let said = match self.api.log(job.id).await {
                Ok(text) => Ok(log::failed_step(&text)),
                // GitHub's limit, its own fault or the network keeps the log from being read for now: the run is read
                // again.
                Err(failed) => match crate::acts::failing::told(failed) {
                    failed if failed.hindrance.is_some() => return Err(failed),
                    failed => Err(failed.why),
                },
            };
            ends.push(StepEnd { job: job.id, said });
        }
        Ok(ends)
    }

    /// The jobs of the run, as the look every wait on it shares found them, sent less than `within` ago, or else as one
    /// sent now finds them; none where it was told to stop.
    async fn jobs_of(&self, run: u64, within: Duration, stop: &mut Stop) -> Result<Option<Vec<Check>>, Failed> {
        tokio::select! {
            jobs = self.looks.jobs(&self.api, run, within) => Ok(Some(jobs?)),
            () = stop.told() => Ok(None),
        }
    }

    /// The runs by hand of the plan's workflow on main whose name holds the plan's value, as the shared look found them
    /// that was sent less than `within` ago, or else one sent now; when that look was sent; and how long the runs it
    /// lists usually take ([`usual`]). None where it was told to stop.
    async fn runs_of(
        &self,
        plan: &Plan<'_>,
        within: Duration,
        stop: &mut Stop,
    ) -> Result<Option<(Vec<Run>, tokio::time::Instant, Option<Duration>)>, Failed> {
        let found = tokio::select! {
            found = self.looks.runs(&self.api, plan.workflow, &self.project.main, within) => found?,
            () = stop.told() => return Ok(None),
        };
        let usual = usual(&found.runs);
        let mine = (found.runs.into_iter())
            .filter(|r| r.title.contains(&plan.call))
            .collect();
        Ok(Some((mine, found.at, usual)))
    }
}

/// The tests that are red on the head of main, as the engine knows them from the checks it ran there, so that a test of a
/// result that failed and is among them is said to have failed on main too.
struct OnMain<'a> {
    /// The main branch's name.
    main: &'a str,
    red: &'a [RedTest],
}

impl OnMain<'_> {
    /// What is said beneath the failed case where its test is red on main: that the test failed there, at the head it
    /// last failed on. None for a case that did not fail, and for a test main is not known to fail.
    fn said(&self, words: &ResultWords, case: &Case) -> Option<String> {
        if case.outcome != results::Outcome::Failed {
            return None;
        }
        let red = (self.red.iter()).find(|red| red.unit == case.test.unit && red.name == case.test.name)?;
        Some(fill(&words.also_on_main, &[("main", self.main), ("head", &red.head)]))
    }
}

/// What became of the runs, in words for the agent: a line of what they came to, the tests that did not pass, each
/// job, and the tests that passed, so that what a cut result gives is what matters. Of the fast workflow it says each
/// test the request named, and each job; of the full workflows, which name no test, only what did not pass: the tests
/// that failed, and the jobs that did not pass. A run whose results could not be read says no test's result is known.
///
/// A test that failed and is red on the main of the project as well, which `on_main` gives, is said so beneath its line.
fn said(
    words: &ResultWords,
    role: &Role<'_>,
    commit: &str,
    ran: &[Ran],
    named: &Named,
    on_main: Option<&OnMain<'_>>,
) -> String {
    let which = match role {
        Role::Fast(_) => &words.which_fast,
        Role::Full(_) => &words.which_full,
    };
    let urls: Vec<&str> = ran.iter().map(|r| r.run.url.as_str()).collect();
    let mut lines = Vec::new();
    let mut tests: Vec<String> = Vec::new();
    let mut short: Vec<String> = Vec::new();
    let mut green: Vec<String> = Vec::new();
    let (mut total, mut passed) = (0, 0);
    let mut unread: Vec<String> = Vec::new();
    for one in ran {
        let run = match &one.results {
            Ok(run) => run,
            Err(why) => {
                unread.push(fill(&words.no_junit, &[("why", why)]));
                continue;
            }
        };
        match role {
            Role::Fast(_) => {
                for row in results::rows(&named.tests, run) {
                    total += 1;
                    passed += usize::from(row.became == Became::Passed);
                    let lines = test_lines(words, &row, run, on_main);
                    match row.became {
                        Became::Passed => green.extend(lines),
                        Became::Stopped | Became::NotStarted => short.extend(lines),
                        _ => tests.extend(lines),
                    }
                }
            }
            Role::Full(_) => {
                for case in &run.cases {
                    total += 1;
                    if case.outcome == results::Outcome::Passed {
                        passed += 1;
                    } else {
                        tests.extend(case_lines(words, case, None));
                    }
                }
                tests.extend(run.unbuilt.iter().map(|u| {
                    let why = fill(&words.unit_not_built, &[("unit", &u.unit), ("why", &u.why)]);
                    fill(
                        &words.test,
                        &[
                            ("name", &u.unit),
                            ("became", &fill(&words.not_built, &[("why", &why)])),
                            ("time", ""),
                        ],
                    )
                }));
            }
        }
    }
    let jobs: Vec<&Check> = ran.iter().flat_map(|r| &r.jobs).collect();
    // The checks that came to something: the jobs named in the fast workflow, every job in the full ones, but for a
    // job that did not run.
    let checks: Vec<&Check> = (jobs.iter().copied())
        .filter(|j| j.outcome != Outcome::Skipped)
        .filter(|j| match role {
            Role::Full(_) => true,
            Role::Fast(_) => named.checks.contains(&j.name),
        })
        .collect();
    let mut came = Vec::new();
    if total > 0 || matches!(role, Role::Full(_)) && unread.is_empty() {
        let (passed, total) = (passed.to_string(), total.to_string());
        came.push(fill(&words.came_tests, &[("passed", &passed), ("total", &total)]));
    }
    if !checks.is_empty() {
        let green = checks.iter().filter(|j| !red_job(j)).count().to_string();
        let all = checks.len().to_string();
        came.push(fill(&words.came_checks, &[("passed", &green), ("total", &all)]));
    }
    let head = fill(
        &words.head,
        &[
            ("which", which),
            ("commit", commit),
            ("url", &urls.join(", ")),
            ("came", &came.join(&words.came_join)),
        ],
    );
    lines.push(head);
    lines.extend(unread);
    if !tests.is_empty() {
        lines.push(format!("{}\n{}", words.tests_title, tests.join("\n")));
    }
    if !short.is_empty() {
        lines.push(format!("{}\n{}", words.short_title, short.join("\n")));
    }
    let missing: Vec<&String> = match role {
        Role::Fast(_) => (named.checks.iter())
            .filter(|c| !ran.iter().flat_map(|r| &r.jobs).any(|j| &j.name == *c))
            .collect(),
        Role::Full(_) => Vec::new(),
    };
    // Every job of the runs, with how it ended and how long it took, and beneath one whose log was read what it says;
    // only those that did not pass where only what did not pass is said, the others being the note of the check.
    let ends: Vec<&StepEnd> = ran.iter().flat_map(|r| &r.ends).collect();
    let shown = jobs.iter().filter(|j| matches!(role, Role::Fast(_)) || red_job(j));
    let mut said_jobs: Vec<String> = shown.map(|j| job_said(words, j, &ends)).collect();
    said_jobs.extend(missing.iter().map(|c| {
        fill(
            &words.job,
            &[("name", c), ("how", &words.job_missing), ("time", ""), ("link", "")],
        )
    }));
    if !said_jobs.is_empty() {
        lines.push(format!("{}\n{}", words.jobs_title, said_jobs.join("\n")));
    }
    if !green.is_empty() {
        lines.push(format!("{}\n{}", words.passed_title, green.join("\n")));
    }
    lines.join("\n\n")
}

/// What became of a measurement's run, in words for the agent: a line of its run, saying whether every job of it
/// passed; each job, with how it ended, and beneath one that failed the end of what its failed step printed; and what
/// it measured, its file's lines as the file holds them, with each metric's unit, or why what it measured is not known.
fn measured(
    words: &ResultWords,
    said: &MeasurementWords,
    measuring: &Measuring<'_>,
    commit: &str,
    base: &str,
    ran: &Ran<Table>,
) -> String {
    let declared = measuring.declared;
    let came = match ran.jobs.iter().any(red_job) {
        true => &said.came_failed,
        false => &said.came_passed,
    };
    let head = fill(
        &said.head,
        &[
            ("name", measuring.name),
            ("workflow", &declared.workflow),
            ("commit", commit),
            ("base", base),
            ("url", &ran.run.url),
            ("came", came),
        ],
    );
    let mut lines = vec![head];
    let ends: Vec<&StepEnd> = ran.ends.iter().collect();
    let jobs: Vec<String> = ran.jobs.iter().map(|job| job_said(words, job, &ends)).collect();
    if !jobs.is_empty() {
        lines.push(format!("{}\n{}", words.jobs_title, jobs.join("\n")));
    }
    lines.push(match &ran.results {
        Ok(table) => {
            let units: Vec<String> = (declared.metrics.iter())
                .map(|(metric, unit)| fill(&said.unit, &[("metric", metric), ("unit", unit)]))
                .collect();
            fill(
                &said.measured,
                &[
                    ("rows", &table.rows.to_string()),
                    ("file", &declared.file),
                    ("units", &units.join(&said.units_join)),
                    ("table", &output::fenced(&table.text)),
                ],
            )
        }
        Err(why) => fill(&said.unread, &[("why", why)]),
    });
    lines.join("\n\n")
}

/// A job's name and what came of it, with how long it took where it ran to a conclusion, and its link. A job that
/// ran to a conclusion and has no time says GitHub gave none.
fn job_line(words: &ResultWords, job: &Check) -> String {
    let link = job
        .url
        .as_ref()
        .map_or(String::new(), |url| fill(&words.link, &[("url", url)]));
    let how = match &job.outcome {
        Outcome::Passed => words.job_passed.clone(),
        Outcome::Failed(how) => fill(&words.job_failed, &[("how", how)]),
        Outcome::Broken(how) => fill(&words.job_broken, &[("how", how)]),
        Outcome::Pending | Outcome::Queued => words.job_pending.clone(),
        Outcome::Skipped => words.job_skipped.clone(),
    };
    let time = match (job.took, &job.outcome) {
        (Some(took), _) => fill(&words.time, &[("time", &took.to_string())]),
        (None, Outcome::Passed | Outcome::Failed(_)) => words.job_untimed.clone(),
        (None, _) => String::new(),
    };
    fill(
        &words.job,
        &[("name", &job.name), ("how", &how), ("time", &time), ("link", &link)],
    )
}

/// A job's line ([`job_line`]), and beneath a job whose log was read, the end of what the step that failed it printed,
/// or why its log could not be read, in one line.
fn job_said(words: &ResultWords, job: &Check, ends: &[&StepEnd]) -> String {
    let line = job_line(words, job);
    let Some(end) = ends.iter().find(|end| end.job == job.id) else {
        return line;
    };
    let below = match &end.said {
        Ok(said) => fill(&words.step_end, &[("said", &output::fenced(said))]),
        Err(why) => {
            let why: Vec<&str> = why.split_whitespace().collect();
            fill(&words.log_unread, &[("why", &why.join(" "))])
        }
    };
    format!("{line}\n{below}")
}

/// Whether a job did not pass: one that did not run, its condition false, passed nothing and failed nothing.
fn red_job(job: &Check) -> bool {
    !matches!(job.outcome, Outcome::Passed | Outcome::Skipped)
}

/// How long a test took, as the results give it, in the words of the file; nothing where they give none.
fn time(words: &ResultWords, case: &Case) -> String {
    case.time
        .as_ref()
        .map_or(String::new(), |time| fill(&words.time, &[("time", time)]))
}

/// A test as a line of what became of it and how long it took, and what a failed one said beneath it.
fn case_lines(words: &ResultWords, case: &Case, also: Option<String>) -> Vec<String> {
    let became = match case.outcome {
        results::Outcome::Passed => &words.passed,
        results::Outcome::Failed => &words.failed,
        results::Outcome::Skipped => &words.skipped,
        results::Outcome::Stopped => &words.stopped,
        results::Outcome::NotStarted => &words.not_started,
    };
    let mut lines = vec![fill(
        &words.test,
        &[
            ("name", &case.test.name),
            ("became", became),
            ("time", &time(words, case)),
        ],
    )];
    lines.extend(also);
    if case.outcome == results::Outcome::Failed
        && let Some(said) = &case.said
    {
        let said = output::fenced(&output::plain(said, process::KEPT_WHOLE));
        lines.push(fill(&words.output, &[("said", &said)]));
    }
    lines
}

/// A test an agent named, as the lines of the tests of the run it names, or of why none is.
fn test_lines(
    words: &ResultWords,
    row: &results::Row<'_>,
    run: &results::Run,
    on_main: Option<&OnMain<'_>>,
) -> Vec<String> {
    let mut lines = Vec::new();
    match row.became {
        Became::NotRun | Became::NotBuilt => {
            let why = if row.became == Became::NotRun {
                words.no_such_test.clone()
            } else {
                let units: Vec<String> = (run.unbuilt.iter())
                    .map(|u| fill(&words.unit_not_built, &[("unit", &u.unit), ("why", &u.why)]))
                    .collect();
                fill(&words.no_such_test_unbuilt, &[("units", &units.join("; "))])
            };
            let became = fill(
                if row.became == Became::NotRun {
                    &words.not_run
                } else {
                    &words.not_built
                },
                &[("why", &why)],
            );
            lines.push(fill(
                &words.test,
                &[("name", row.named), ("became", &became), ("time", "")],
            ));
        }
        Became::Skipped => {
            let became = fill(&words.not_run, &[("why", &words.all_skipped)]);
            lines.push(fill(
                &words.test,
                &[("name", row.named), ("became", &became), ("time", "")],
            ));
        }
        // The budget's own words say why a test did not start, which is no failure and has no output to give.
        Became::NotStarted => {
            lines.push(fill(
                &words.test,
                &[("name", row.named), ("became", &words.not_started), ("time", "")],
            ));
        }
        Became::Passed | Became::Failed | Became::Stopped => {
            for case in &row.matched {
                lines.extend(case_lines(words, case, on_main.and_then(|red| red.said(words, case))));
            }
        }
    }
    lines
}
