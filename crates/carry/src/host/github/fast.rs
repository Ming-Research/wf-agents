//! The project's fast CI: a workflow the engine runs by hand, which runs only the tests and the checks an agent named
//! and reports what became of each test (`results`). Brought over from the old engine's `ci` and `ci_before`
//! (`crates/engine/src/tools.rs` on `old-main`): the tests and checks named, the run asked for and waited for, and each
//! test's own result and time read from the run's JUnit. Unlike there, the workflow is always run by its file on main,
//! given the commit to test, and its run is found by a value only that request knows, which the run's name holds.
//!
//! What is said of a run: each test named, with what became of it and how long it ran, and what a failed one said;
//! each check named; and, for a job that failed with nothing named to say why (a unit that did not compile, say), what
//! its log says of the failure. What a test or a log said shares the bytes a check's output is given.

use super::api::{Api, Check, Outcome, Run};
use super::results::{self, Became, Case};
use super::{GitHub, log};
use crate::held::Stop;
use crate::host::Named;
use crate::process;
use crate::settings::Fast;

/// Why what the agent named cannot be run by the fast CI, in words for it: a check the workflow does not run by name.
pub(super) fn refused(fast: &Fast, named: &Named) -> Option<String> {
    let unknown: Vec<&str> = (named.checks.iter())
        .filter(|c| !fast.named_checks.contains(c))
        .map(String::as_str)
        .collect();
    if unknown.is_empty() {
        return None;
    }
    let runs = if fast.named_checks.is_empty() {
        "no check by name".to_string()
    } else {
        format!("by name only {}", fast.named_checks.join(", "))
    };
    Some(format!(
        "Nothing was checked: the project's fast CI runs {runs}, and not {}.",
        unknown.join(", ")
    ))
}

/// The inputs the workflow is run with: the commit, the request's own value, the tests, each followed by a line break,
/// and the checks as a JSON array, given only when one is named, so that a workflow made before that input takes its
/// default, which runs none.
fn inputs(fast: &Fast, commit: &str, call: &str, named: &Named) -> Vec<(String, String)> {
    let tests: String = (named.tests.iter()).map(|t| format!("{}\n", t.trim())).collect();
    let mut inputs = vec![
        (fast.commit_input.clone(), commit.to_string()),
        (fast.call_input.clone(), call.to_string()),
        (fast.tests_input.clone(), tests),
    ];
    if !named.checks.is_empty() {
        let checks = serde_json::to_string(&named.checks).unwrap_or_default();
        inputs.push((fast.checks_input.clone(), checks));
    }
    inputs
}

/// What a run of the fast CI came to.
struct Ran {
    run: Run,
    jobs: Vec<Check>,
    results: Result<results::Run, String>,
}

impl<A: Api> GitHub<A> {
    /// Runs the fast workflow on the commit, pushed already, with what the agent named, and says what became of it;
    /// none where it was told to stop. The workflow is run by its file on main, so that no work chooses what runs on
    /// the project's machine, and GitHub always has that file and that branch.
    pub(super) async fn fast(
        &self,
        fast: &Fast,
        commit: &str,
        named: &Named,
        stop: &mut Stop,
    ) -> Result<Option<String>, String> {
        let workflow = &fast.workflow;
        let main = &self.project.main;
        // A value no other request has, which the run's name holds: by it this request's run is told from another's
        // on the same commit, as another check of the same work asks for at once.
        let call = uuid::Uuid::new_v4().simple().to_string();
        (self
            .api
            .dispatch(workflow, main, &inputs(fast, commit, &call, named))
            .await)
            .map_err(|why| format!("GitHub refused to run {workflow} on {main} for {commit}: {why}"))?;
        let began = tokio::time::Instant::now();
        let run = loop {
            let runs = tokio::select! {
                runs = self.api.runs(workflow, main) => runs?,
                () = stop.told() => return Ok(None),
            };
            let mine = runs.into_iter().find(|r| r.title.contains(&call));
            let waited = began.elapsed();
            match mine {
                Some(run) if run.completed => break run,
                // Listed at once as queued where no runner is online: given up as never started, not waited for
                // until the run would time out.
                Some(run) if !run.started && waited >= self.config.start => {
                    return Err(format!(
                        "the run of {workflow} asked for on {commit} never started: no runner took it after {} ({})",
                        super::span(self.config.start),
                        run.url
                    ));
                }
                None if waited >= self.config.start => {
                    return Err(format!(
                        "the run of {workflow} asked for on {commit} never started: GitHub listed none after {}",
                        super::span(self.config.start)
                    ));
                }
                _ if waited >= self.config.timeout => {
                    return Err(format!(
                        "the run of {workflow} on {commit} is not complete after {}",
                        super::span(self.config.timeout)
                    ));
                }
                _ => {}
            }
            tokio::select! {
                () = tokio::time::sleep(self.config.fast_poll) => {}
                () = stop.told() => return Ok(None),
            }
        };
        let jobs = self.api.jobs(run.id).await?;
        let results = if named.tests.is_empty() {
            Err("no test was named".to_string())
        } else {
            match self.api.artifact(run.id, &fast.results).await {
                Ok(Some(files)) => results::read(&files).map_err(|why| format!("the artifact {}: {why}", fast.results)),
                Ok(None) => Err(format!("the run uploaded no artifact {}", fast.results)),
                Err(why) => Err(format!("the artifact {} could not be fetched: {why}", fast.results)),
            }
        };
        let ran = Ran { run, jobs, results };
        Ok(Some(self.said(fast, commit, &ran, named).await))
    }

    /// What became of the run, in words for the agent: a line of what it came to, each test named, each check named,
    /// and what the log of a job that failed says where nothing named says why.
    async fn said(&self, fast: &Fast, commit: &str, ran: &Ran, named: &Named) -> String {
        let rows = results::rows(
            &named.tests,
            ran.results.as_ref().map_err(String::as_str),
            &fast.separators,
        );
        let failed_tests = rows.iter().filter(|r| r.became == Became::Failed).count();
        let checks: Vec<&Check> = (ran.jobs.iter()).filter(|j| named.checks.contains(&j.name)).collect();
        // A job that failed and is no check named, where no test named failed, as the job of the tests when a unit did
        // not compile: its log says why.
        let unexplained: Vec<&Check> = (ran.jobs.iter())
            .filter(|j| red(j) && !named.checks.contains(&j.name) && failed_tests == 0)
            .collect();
        let excerpts = failed_tests + checks.iter().filter(|j| red(j)).count() + unexplained.len();
        let budget = process::TAIL / excerpts.max(1);
        let passed = rows.iter().filter(|r| r.became == Became::Passed).count();
        let mut came = Vec::new();
        if !rows.is_empty() {
            came.push(format!("{passed} of the {} tests named passed", rows.len()));
        }
        if !checks.is_empty() {
            let green = checks.iter().filter(|j| !red(j)).count();
            came.push(format!("{green} of the {} checks named passed", checks.len()));
        }
        let mut said = vec![format!(
            "The fast CI ran on the commit {commit} ({}): {}.",
            ran.run.url,
            came.join("; ")
        )];
        if !rows.is_empty() {
            let mut lines = vec!["The tests named:".to_string()];
            for row in &rows {
                lines.push(test_line(row, budget));
            }
            said.push(lines.join("\n"));
        }
        let missing: Vec<&str> = (named.checks.iter())
            .filter(|c| !ran.jobs.iter().any(|j| &j.name == *c))
            .map(String::as_str)
            .collect();
        if !checks.is_empty() || !missing.is_empty() {
            let mut lines = vec!["The checks named:".to_string()];
            lines.extend(checks.iter().map(|j| format!("- {}", self.job_line(j))));
            lines.extend(
                missing
                    .iter()
                    .map(|c| format!("- {c}: the run has no job of that name")),
            );
            said.push(lines.join("\n"));
        }
        let mut said = said.join("\n\n");
        for job in checks.iter().filter(|j| red(j)).chain(&unexplained) {
            said.push_str(&format!("\n\n{}", self.job_line(job)));
            let Some(id) = job.job else {
                continue;
            };
            match self.api.log(id).await {
                Ok(text) => said.push_str(&format!(
                    "\nWhat its log says of the failure, as the CI printed it:\n{}",
                    log::fenced(&log::what_failed(&text, budget))
                )),
                Err(why) => said.push_str(&format!("\nIts log could not be read: {why}")),
            }
        }
        said
    }

    /// A job's name and what came of it, with its link.
    fn job_line(&self, job: &Check) -> String {
        let at = job.url.as_ref().map_or(String::new(), |url| format!(" {url}"));
        match &job.outcome {
            Outcome::Passed => format!("{}: passed", job.name),
            Outcome::Failed(how) => format!("{}: failed ({how}){at}", job.name),
            Outcome::Broken(how) => format!("{}: ended without judging the code ({how}){at}", job.name),
            Outcome::Pending => format!("{}: not complete{at}", job.name),
        }
    }
}

/// Whether a job did not pass.
fn red(job: &Check) -> bool {
    !matches!(job.outcome, Outcome::Passed)
}

/// A test named, as a line of what became of it: with one test it names, that test's result and time; with several,
/// each of them on a line of its own; what a failed one said beneath it.
fn test_line(row: &results::Row<'_>, budget: usize) -> String {
    let named = row.named;
    let mut lines = vec![match (row.became, row.matched.as_slice()) {
        (Became::Passed | Became::Failed, [one]) => format!("- {named}: {}", case(one)),
        (Became::Passed, _) => format!("- {named}: passed, as each test it names"),
        (Became::Failed, _) => format!("- {named}: failed, as a test it names"),
        (Became::NotRun, _) => format!("- {named}: not run: {}", row.why.as_deref().unwrap_or_default()),
        (Became::NotBuilt, _) => format!("- {named}: not built: {}", row.why.as_deref().unwrap_or_default()),
        (Became::Unread, _) => format!("- {named}: unread: {}", row.why.as_deref().unwrap_or_default()),
    }];
    if row.matched.len() > 1 {
        lines.extend(
            row.matched
                .iter()
                .map(|c| format!("  - {} {}: {}", c.test.unit, c.test.name, case(c))),
        );
    }
    for c in row.matched.iter().filter(|c| c.outcome == results::Outcome::Failed) {
        if let Some(said) = &c.said {
            lines.push(format!("  What {} said:", c.test.name));
            lines.push(log::fenced(&log::plain(said, budget)));
        }
    }
    lines.join("\n")
}

/// What became of a test and how long it ran.
fn case(c: &Case) -> String {
    let became = c.outcome.as_str();
    match &c.time {
        Some(time) => format!("{became} in {time} s"),
        None => became.to_string(),
    }
}
