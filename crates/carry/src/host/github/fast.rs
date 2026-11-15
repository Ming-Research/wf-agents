//! The project's two workflows, which the engine runs by hand through the project's declaration (`ci` in
//! `flow/project.toml`), always by their files on main, given the commit to run on, each run found by a value its
//! name holds, which is worked out from what asked for it; and what each reports, read from the JUnit XML it uploads
//! (`results`) and never from a log.
//!
//! The value is the same each time the same entry in which a cell asked for tests, or the same round of the check,
//! asks for the same workflow on the same commit, and is had by no other: given again after the engine started again,
//! or after the round that asked was stopped, the request finds the run it asked for before and waits for it, or reads
//! it where it has ended, and asks for a run again only where it finds none but runs that were cancelled, which judged
//! nothing. A run of an entry a newer one replaced is cancelled, found by the same value on the branch's commits.
//!
//! - fast: runs only the tests and checks a request names, and builds only what they need; every entry that names
//!   tests or checks is run by it, and so is what the check before a merge runs again of what failed.
//! - full: the workflows that hold every job required to merge; an entry that names none is run by them, once, on the
//!   work's last commit, and so is the check before a merge, but where the full workflows of such an entry passed on
//!   the very commit it checks.
//!
//! What is said of a run: each test, with what became of it, how long it took and what a failed one said; and each
//! job of the workflow, with how it ended and, where it ran to a conclusion, the wall time GitHub records for it. A
//! run that uploaded no JUnit the engine can read is said so: no result is guessed from a log.

use std::time::Duration;

use flow::agents::{Ci, CiFast, CiFull, ResultWords, RunTests, fill};
use store::WorkNumber;
use uuid::Uuid;

use super::api::{Api, Check, Outcome, Run};
use super::results::{self, Became, Case};
use super::{GitHub, output};
use crate::git;
use crate::held::Stop;
use crate::host::{Asking, Failed, Named, Verdict};
use crate::process;
use service::Along;

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
}

/// What a run of a workflow came to.
struct Ran {
    run: Run,
    jobs: Vec<Check>,
    results: Result<results::Run, String>,
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
    let concluded = (one.jobs.iter()).all(|j| !matches!(j.outcome, Outcome::Broken(_) | Outcome::Pending));
    concluded.then(|| results::rows(tests, run))
}

/// A time such as `90m`, `2h`, `30s` or `1d`; or why it is none.
pub(super) fn span_of(text: &str) -> Result<Duration, String> {
    let text = text.trim();
    let (number, unit) = text.split_at(text.find(|c: char| !c.is_ascii_digit()).unwrap_or(text.len()));
    let number: u64 = number
        .parse()
        .map_err(|_| format!("{text:?} is no time such as 90m or 2h"))?;
    let seconds = match unit {
        "s" => 1,
        "m" => 60,
        "h" => 3600,
        "d" => 86_400,
        _ => return Err(format!("{text:?} is no time such as 90m or 2h")),
    };
    Ok(Duration::from_secs(number * seconds))
}

/// What a request is run by.
pub(super) enum Role<'a> {
    Fast(&'a CiFast),
    Full(&'a CiFull),
}

impl<A: Api> GitHub<A> {
    /// The words of the act that runs the tests, or why there are none.
    fn tool(&self) -> Result<&RunTests, String> {
        self.config
            .tool
            .as_ref()
            .ok_or_else(|| "the words of run_tests (agents/acts/run_tests.toml) are not given".to_string())
    }

    /// Why a request cannot be run by the project's workflows, in the words of the tool's file, or the workflow it is
    /// run by: the fast one where it names tests or checks, the full ones where it names none.
    pub(super) fn role<'a>(&'a self, ci: Option<&'a Ci>, named: &Named) -> Result<Result<Role<'a>, String>, String> {
        let words = &self.tool()?.github;
        let have = self.config.runners.join(", ");
        let runner_missing = |runner: &str| {
            (!self.config.runners.iter().any(|r| r == runner))
                .then(|| fill(&words.runner_missing, &[("runner", runner), ("have", &have)]))
        };
        if named.is_empty() {
            let Some(full) = ci.and_then(|ci| ci.full.as_ref()) else {
                return Ok(Err(words.no_full.clone()));
            };
            return Ok(runner_missing(&full.runner).map_or(Ok(Role::Full(full)), Err));
        }
        let Some(fast) = ci.and_then(|ci| ci.fast.as_ref()) else {
            return Ok(Err(words.no_fast.clone()));
        };
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
        Ok(runner_missing(&fast.runner).map_or(Ok(Role::Fast(fast)), Err))
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
        let words = &self.tool()?.result;
        Ok(Some(said(words, &role, request.commit, &ran, named, false)))
    }

    /// The verdict of the check before a merge on the commit where the full workflows the request asked for before
    /// ran on it and passed, each of them, found again by its value and nothing run for it; none where one of them did
    /// not run, is not complete or did not pass, or where it was told to stop.
    pub(super) async fn passed_full(&self, request: Request<'_>, stop: &mut Stop) -> Result<Option<Verdict>, Failed> {
        let Some(full) = self.config.ci.as_ref().and_then(|ci| ci.full.as_ref()) else {
            return Ok(None);
        };
        let mut ran = Vec::new();
        for workflow in &full.workflows {
            let plan = Plan {
                workflow,
                results: &full.results,
                inputs: Vec::new(),
                call: self.call(request, workflow),
                limit: Duration::ZERO,
            };
            let Some(runs) = self.runs_of(&plan, stop).await? else {
                return Ok(None);
            };
            let Some(run) = (runs.into_iter()).filter(|r| !r.cancelled).min_by_key(|r| r.id) else {
                return Ok(None);
            };
            if !run.completed {
                return Ok(None);
            }
            let jobs = self.api.jobs(run.id).await?;
            let results = match self.api.artifact(run.id, plan.results).await {
                Ok(Some(files)) => results::read(&files).map_err(|why| format!("the artifact {}: {why}", plan.results)),
                Ok(None) => Err(format!("the run uploaded no artifact {}", plan.results)),
                Err(why) => Err(format!("the artifact {} could not be fetched: {why}", plan.results)),
            };
            ran.push(Ran { run, jobs, results });
        }
        let red = ran.iter().any(|one| match &one.results {
            Ok(run) => run.cases.iter().any(|c| c.outcome == results::Outcome::Failed) || one.jobs.iter().any(red_job),
            Err(_) => true,
        });
        if red || ran.is_empty() {
            return Ok(None);
        }
        let words = &self.tool()?.result;
        let urls: Vec<&str> = ran.iter().map(|r| r.run.url.as_str()).collect();
        let jobs: Vec<String> = (ran.iter().flat_map(|r| &r.jobs)).map(|j| job_line(words, j)).collect();
        let notes = if jobs.is_empty() {
            Vec::new()
        } else {
            vec![fill(
                &words.verdict_jobs,
                &[(
                    "jobs",
                    &jobs.join(
                        "
  ",
                    ),
                )],
            )]
        };
        Ok(Some(Verdict {
            by: Some(fill(&words.verdict_by, &[("urls", &urls.join(", "))])),
            notes,
            ..Verdict::default()
        }))
    }

    /// Cancels each run of the project's workflows not complete that one of the askings asked for, of the work of this
    /// number, on one of the commits beside it: found by the values that ask for it, in one list of each workflow's
    /// runs. Gives why, for each list or run, it could not be listed or cancelled; the rest are cancelled all the same.
    pub(super) async fn cancel_runs(&self, work: WorkNumber, on: &[(Asking, Vec<String>)]) -> Vec<String> {
        let ci = self.config.ci.as_ref();
        let fast = (ci.and_then(|ci| ci.fast.as_ref())).map(|fast| fast.workflow.as_str());
        let full = (ci.and_then(|ci| ci.full.as_ref())).map_or(&[][..], |full| &full.workflows[..]);
        let mut failed = Vec::new();
        for workflow in fast.into_iter().chain(full.iter().map(String::as_str)) {
            let calls: Vec<String> = (on.iter())
                .flat_map(|(asking, commits)| {
                    (commits.iter()).map(move |commit| Request {
                        work,
                        commit,
                        asking: *asking,
                    })
                })
                .map(|request| self.call(request, workflow))
                .collect();
            if calls.is_empty() {
                continue;
            }
            let runs = match self.api.runs(workflow, &self.project.main).await {
                Ok(runs) => runs,
                Err(why) => {
                    failed.push(why.why);
                    continue;
                }
            };
            for run in (runs.iter()).filter(|r| !r.completed && calls.iter().any(|c| r.title.contains(c))) {
                if let Err(why) = self.api.cancel(run.id).await {
                    failed.push(why.why);
                }
            }
        }
        failed
    }

    /// The jobs of the full workflows that did not pass may be let through with no failed test of their own, as the
    /// project's own declaration and workflows say them, the files of them as main holds them at `main`, the head the
    /// commit checked holds: each check the fast workflow runs by name, which the run again runs and so finds red
    /// again; and each job of a full workflow that needs every other job of it, which only sums them up
    /// ([`super::workflow::summing`]). A workflow whose file cannot be read lets none through for summing up.
    pub(super) async fn accounted(&self, main: &str) -> Vec<String> {
        let ci = self.config.ci.as_ref();
        let mut accounted = (ci.and_then(|ci| ci.fast.as_ref())).map_or_else(Vec::new, |fast| fast.checks.clone());
        let workflows = (ci.and_then(|ci| ci.full.as_ref())).map_or(&[][..], |full| &full.workflows[..]);
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
                let plan = Plan {
                    workflow: &fast.workflow,
                    results: &fast.results,
                    inputs,
                    call,
                    limit: self.config.timeout,
                };
                match self.execute(&plan, commit, stop).await? {
                    Some(one) => ran.push(one),
                    None => return Ok(None),
                }
            }
            Role::Full(full) => {
                let limit = span_of(&full.too_long).map_err(|why| format!("ci.full.too_long: {why}"))?;
                for workflow in &full.workflows {
                    let call = self.call(request, workflow);
                    let inputs = vec![
                        (full.commit_input.clone(), commit.to_string()),
                        (full.call_input.clone(), call.clone()),
                    ];
                    let plan = Plan {
                        workflow,
                        results: &full.results,
                        inputs,
                        call,
                        limit,
                    };
                    match self.execute(&plan, commit, stop).await? {
                        Some(one) => ran.push(one),
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
        let role = match self.role(self.config.ci.as_ref(), &named)? {
            Ok(role) => role,
            Err(refused) => return Err(Failed::refused(refused)),
        };
        let Some(ran) = self.execute_role(&role, request, &named, stop).await? else {
            return Ok(None);
        };
        let words = &self.tool()?.result;
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
        let first = said(words, &role, commit, &ran, &named, true);
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
        let checks = (ci.and_then(|ci| ci.fast.as_ref())).map_or_else(Vec::new, |fast| fast.checks.clone());
        let named = Named {
            tests: tests.clone(),
            checks,
            commit: None,
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
            let run = said(words, &role, head, ran, named, false);
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
            checks: Vec::new(),
            commit: None,
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
        let only = filled(&check.only_here, &only_here);
        Ok(Some(Again {
            flaky,
            ..Again::failed(vec![first, again_on_commit, again_on_main, only])
        }))
    }

    /// Waits for the run of the workflow the plan asks for, and reads what it uploaded: the run its request asked for
    /// already, before the engine started again, where one was not cancelled; or, where none was, one the workflow is
    /// dispatched for now with the plan's inputs. Of two runs of the request, the first asked for is waited for, which
    /// began first. None where it was told to stop.
    ///
    /// A run that never started, GitHub listing none or no runner taking it in the time the CI may take to start, is
    /// cancelled where it is listed, so that it does not run later for nothing, and fails in passing: what stops it is
    /// no work's (GitHub, or a runner offline), and asked for again after a wait it may start. A run not complete in the
    /// time it may take is cancelled, and is what it found: its jobs not complete said so, and no test's result known.
    async fn execute(&self, plan: &Plan<'_>, commit: &str, stop: &mut Stop) -> Result<Option<Ran>, Failed> {
        let workflow = plan.workflow;
        let main = &self.project.main;
        let Some(asked) = self.runs_of(plan, stop).await? else {
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
        let run = loop {
            let Some(runs) = self.runs_of(plan, stop).await? else {
                return Ok(None);
            };
            let mine = (runs.into_iter())
                .filter(|r| !cancelled.contains(&r.id))
                .min_by_key(|r| r.id);
            // Once its run is found, the call or the act runs as it, at its link.
            if let Some(run) = &mine {
                stop.say(Some(Along::Runs { at: run.url.clone() }));
            }
            let waited = began.elapsed();
            match mine {
                Some(run) if run.completed => break run,
                // Listed at once as queued where no runner is online: given up as never started, not waited for
                // until the run would time out.
                Some(run) if !run.started && waited >= self.config.start => {
                    self.api.cancel(run.id).await?;
                    return Err(Failed::passing(format!(
                        "the run of {workflow} asked for on {commit} never started: no runner took it after {} ({}), \
                         and the engine cancelled it",
                        super::span(self.config.start),
                        run.url
                    )));
                }
                None if waited >= self.config.start => {
                    return Err(Failed::passing(format!(
                        "the run of {workflow} asked for on {commit} never started: GitHub listed none after {}",
                        super::span(self.config.start)
                    )));
                }
                // Its jobs are read before it is cancelled, so that those still under way are said not complete.
                Some(run) if run.started && waited >= plan.limit => {
                    let jobs = self.api.jobs(run.id).await?;
                    self.api.cancel(run.id).await?;
                    let results = Err(format!(
                        "the run did not complete within {}, the time it may take, and the engine cancelled it",
                        super::span(plan.limit)
                    ));
                    return Ok(Some(Ran { run, jobs, results }));
                }
                _ => {}
            }
            tokio::select! {
                () = tokio::time::sleep(self.config.fast_poll) => {}
                () = stop.told() => return Ok(None),
            }
        };
        let jobs = self.api.jobs(run.id).await?;
        let results = match self.api.artifact(run.id, plan.results).await {
            Ok(Some(files)) => results::read(&files).map_err(|why| format!("the artifact {}: {why}", plan.results)),
            Ok(None) => Err(format!("the run uploaded no artifact {}", plan.results)),
            Err(why) => Err(format!("the artifact {} could not be fetched: {why}", plan.results)),
        };
        Ok(Some(Ran { run, jobs, results }))
    }

    /// The runs by hand of the plan's workflow on main whose name holds the plan's value; none where it was told to
    /// stop.
    async fn runs_of(&self, plan: &Plan<'_>, stop: &mut Stop) -> Result<Option<Vec<Run>>, Failed> {
        let runs = tokio::select! {
            runs = self.api.runs(plan.workflow, &self.project.main) => runs?,
            () = stop.told() => return Ok(None),
        };
        Ok(Some(
            runs.into_iter().filter(|r| r.title.contains(&plan.call)).collect(),
        ))
    }
}

/// What became of the runs, in words for the agent: a line of what they came to, the tests that did not pass, each
/// job, and the tests that passed, so that what a cut result gives is what matters. A run whose results could not be
/// read says no test's result is known.
fn said(words: &ResultWords, role: &Role<'_>, commit: &str, ran: &[Ran], named: &Named, red_only: bool) -> String {
    let which = match role {
        Role::Fast(_) => &words.which_fast,
        Role::Full(_) => &words.which_full,
    };
    let urls: Vec<&str> = ran.iter().map(|r| r.run.url.as_str()).collect();
    let mut lines = Vec::new();
    let mut tests: Vec<String> = Vec::new();
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
                    let lines = test_lines(words, &row, run);
                    if row.became == Became::Passed {
                        green.extend(lines);
                    } else {
                        tests.extend(lines);
                    }
                }
            }
            Role::Full(_) => {
                for case in &run.cases {
                    total += 1;
                    if case.outcome == results::Outcome::Passed {
                        passed += 1;
                        if !red_only {
                            green.extend(case_lines(words, case));
                        }
                    } else {
                        tests.extend(case_lines(words, case));
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
        .filter(|j| matches!(role, Role::Full(_)) || named.checks.contains(&j.name))
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
    let missing: Vec<&String> = match role {
        Role::Fast(_) => (named.checks.iter())
            .filter(|c| !ran.iter().flat_map(|r| &r.jobs).any(|j| &j.name == *c))
            .collect(),
        Role::Full(_) => Vec::new(),
    };
    // Every job of the runs, with how it ended and how long it took; only those that did not pass where only what did
    // not pass is said, the others being the note of the check.
    let shown = jobs.iter().filter(|j| !red_only || red_job(j));
    let mut said_jobs: Vec<String> = shown.map(|j| job_line(words, j)).collect();
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
        Outcome::Pending => words.job_pending.clone(),
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
fn case_lines(words: &ResultWords, case: &Case) -> Vec<String> {
    let became = match case.outcome {
        results::Outcome::Passed => &words.passed,
        results::Outcome::Failed => &words.failed,
        results::Outcome::Skipped => &words.skipped,
    };
    let mut lines = vec![fill(
        &words.test,
        &[
            ("name", &case.test.name),
            ("became", became),
            ("time", &time(words, case)),
        ],
    )];
    if case.outcome == results::Outcome::Failed
        && let Some(said) = &case.said
    {
        let said = output::fenced(&output::plain(said, process::KEPT_WHOLE));
        lines.push(fill(&words.output, &[("said", &said)]));
    }
    lines
}

/// A test an agent named, as the lines of the tests of the run it names, or of why none is.
fn test_lines(words: &ResultWords, row: &results::Row<'_>, run: &results::Run) -> Vec<String> {
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
        Became::Passed | Became::Failed => {
            for case in &row.matched {
                lines.extend(case_lines(words, case));
            }
        }
    }
    lines
}
