//! The agents' `run_tests` on the GitHub host with the tests and checks they name, run by the project's fast workflow,
//! and with none named, run by nothing: a workflow run by hand on main's file with the commit given, whose JUnit
//! results say each test's own result and time, and whose jobs' logs say why a job failed that no test of its own says.

use super::super::api::{Check, Outcome};
use super::*;
use crate::host::Named;

pub(super) fn named(tests: &[&str], checks: &[&str]) -> Named {
    let all = |xs: &[&str]| xs.iter().map(|x| x.to_string()).collect();
    Named {
        tests: all(tests),
        checks: all(checks),
        commit: None,
    }
}

/// The JUnit a run uploads, in the file of the job `test`: `tools::tests::a` passed, `tools::tests::b` failed, saying why.
pub(super) fn junit() -> Vec<(String, Vec<u8>)> {
    let xml = r#"<testsuites><testsuite name="carry">
  <testcase name="tools::tests::a" time="0.12"/>
  <testcase name="tools::tests::b" time="0.3"><failure message="assertion failed">thread 'tools::tests::b' panicked at crates/carry/src/tools.rs:9:5:
assertion failed: said.is_empty()</failure></testcase>
</testsuite></testsuites>"#;
    vec![("test.xml".to_string(), xml.as_bytes().to_vec())]
}

/// The JUnit of a run in which the one test it holds passed, in the file of the job `test`.
pub(super) fn junit_green() -> Vec<(String, Vec<u8>)> {
    let xml = r#"<testsuites><testsuite name="carry"><testcase name="tools::tests::a" time="0.12"/></testsuite></testsuites>"#;
    vec![("test.xml".to_string(), xml.as_bytes().to_vec())]
}

/// A check of this id.
pub(super) fn job(name: &str, outcome: Outcome, id: u64) -> Check {
    Check {
        id,
        ..check(name, outcome)
    }
}

/// The log of a lint job that failed on one error of clippy's, as GitHub keeps it: each line after the time it was
/// written, the first after the mark of UTF-8 that begins the file; the runner's set-up, the checkout, the format,
/// which passed, then clippy, coloured as cargo colours it, and the clean-up after it.
pub(super) fn clippy_log() -> String {
    let lines = [
        "Current runner version: '2.329.0'",
        "##[group]Run actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1",
        "with:",
        "  persist-credentials: false",
        "##[endgroup]",
        "##[group]Run cargo fmt --all --check",
        "cargo fmt --all --check",
        "shell: /usr/bin/bash -e {0}",
        "##[endgroup]",
        "##[group]Run cargo clippy --workspace --all-targets --locked -- -D warnings",
        "cargo clippy --workspace --all-targets --locked -- -D warnings",
        "shell: /usr/bin/bash -e {0}",
        "##[endgroup]",
        "\x1b[1m\x1b[92m    Checking\x1b[0m driver v0.1.0 (/runner/work/r/r/crates/driver)",
        "\x1b[0m\x1b[1m\x1b[91merror\x1b[0m\x1b[0m\x1b[1m: this `if` statement can be collapsed\x1b[0m",
        "\x1b[0m    \x1b[0m\x1b[1m\x1b[94m--> \x1b[0mcrates/driver/src/lib.rs:1533:9",
        "\x1b[1m\x1b[94m     |\x1b[0m",
        "\x1b[1m\x1b[94m1533\x1b[0m \x1b[1m\x1b[94m|\x1b[0m \x1b[1m\x1b[91m/\x1b[0m         if held {",
        "\x1b[1m\x1b[94m1534\x1b[0m \x1b[1m\x1b[94m|\x1b[0m \x1b[1m\x1b[91m|\x1b[0m             if ended {",
        "\x1b[1m\x1b[94m     |\x1b[0m \x1b[1m\x1b[91m|_________^\x1b[0m",
        "     = note: `-D clippy::collapsible-if` implied by `-D warnings`",
        "\x1b[1m\x1b[96mhelp\x1b[0m: collapse nested if block",
        "1533 ~         if held",
        "1534 ~             && ended {",
        "",
        "\x1b[1m\x1b[91merror\x1b[0m\x1b[1m:\x1b[0m could not compile `driver` (lib) due to 1 previous error",
        "##[error]Process completed with exit code 101.",
        "Post job cleanup.",
        "[command]/usr/bin/git version",
        "git version 2.43.0",
        "Cleaning up orphan processes",
    ];
    let stamped: String = (lines.iter().enumerate())
        .map(|(n, line)| {
            let fraction = n * 13_717;
            format!("2026-11-21T09:48:{n:02}.{fraction:07}Z {line}\n")
        })
        .collect();
    format!("\u{feff}{stamped}")
}

/// The end of the failed step of [`clippy_log`], as a reader of what a job said is given it: clippy's step, from its
/// command to the error that ended it, without its colours and times.
pub(super) const CLIPPY_STEP: &str = "##[group]Run cargo clippy --workspace --all-targets --locked -- -D warnings
cargo clippy --workspace --all-targets --locked -- -D warnings
shell: /usr/bin/bash -e {0}
##[endgroup]
    Checking driver v0.1.0 (/runner/work/r/r/crates/driver)
error: this `if` statement can be collapsed
    --> crates/driver/src/lib.rs:1533:9
     |
1533 | /         if held {
1534 | |             if ended {
     | |_________^
     = note: `-D clippy::collapsible-if` implied by `-D warnings`
help: collapse nested if block
1533 ~         if held
1534 ~             && ended {

error: could not compile `driver` (lib) due to 1 previous error
##[error]Process completed with exit code 101.";

impl Hub {
    async fn named_check(&self, named: &Named) -> Result<Option<String>, String> {
        let place = self.place();
        self.host
            .run_tests(&branch(&place), named, self.call(), &mut Stop::never())
            .await
    }

    /// The runs by hand GitHub was asked for.
    pub(super) fn dispatched(&self) -> Vec<Asked> {
        (self.github().asked().into_iter())
            .filter(|a| matches!(a, Asked::Dispatch { .. }))
            .collect()
    }

    /// The runs by hand GitHub was asked for, each request's own value, which differs at each, said as `<call>`.
    fn dispatched_as_said(&self) -> Vec<Asked> {
        let mut calls = Vec::new();
        let said = (self.dispatched().into_iter())
            .map(|asked| match asked {
                Asked::Dispatch {
                    workflow,
                    branch,
                    inputs,
                } => {
                    let inputs = (inputs.into_iter())
                        .map(|(name, value)| {
                            if name != "call" {
                                return (name, value);
                            }
                            assert!(
                                value.len() == 32 && value.chars().all(|c| c.is_ascii_hexdigit()),
                                "{value}"
                            );
                            calls.push(value);
                            (name, "<call>".to_string())
                        })
                        .collect();
                    Asked::Dispatch {
                        workflow,
                        branch,
                        inputs,
                    }
                }
                other => other,
            })
            .collect();
        let mut apart = calls.clone();
        apart.dedup();
        assert_eq!(apart, calls, "each request has a value of its own");
        said
    }
}

/// The inputs of a run asked for on the commit with these tests, and these checks where some are named.
fn inputs(commit: &str, tests: &str, checks: Option<&str>) -> Vec<(String, String)> {
    let mut inputs = vec![
        ("commit".to_string(), commit.to_string()),
        ("call".to_string(), "<call>".to_string()),
        ("tests".to_string(), tests.to_string()),
    ];
    inputs.extend(checks.map(|c| ("checks".to_string(), c.to_string())));
    inputs
}

#[tokio::test]
async fn the_tests_named_run_by_the_fast_workflow_and_each_is_said_with_its_result_and_time() {
    let hub = Hub::new();
    let head = hub.committed().await;
    let test = job("test", Outcome::Failed("failure".into()), 5);
    hub.github().fast(&head, vec![test], Some(("test-results", junit())));
    let asked = named(&["tools::tests::a", "tools::tests::b", "a_name_of_no_test"], &[]);
    let said = hub.named_check(&asked).await.unwrap().expect("nothing stopped it");
    assert!(
        said.starts_with(&format!(
            "The fast workflow ran on the commit {head} (https://github.example/actions/runs/1000): 1 of the 3 tests \
             passed."
        )),
        "{said}"
    );
    for line in [
        "The tests that passed:\n- tools::tests::a: passed in 0.12 s",
        "The tests that did not pass:\n- tools::tests::b: failed in 0.3 s\n  What it said:\n```\nthread 'tools::tests::b' panicked at \
         crates/carry/src/tools.rs:9:5:\nassertion failed: said.is_empty()\n```",
        "- a_name_of_no_test: not run: no test in the run has this name",
    ] {
        assert!(said.contains(line), "{line}\n\n{said}");
    }
    assert_eq!(
        hub.dispatched_as_said(),
        [Asked::Dispatch {
            workflow: "fast.yml".to_string(),
            branch: "main".to_string(),
            inputs: inputs(&head, "tools::tests::a\ntools::tests::b\na_name_of_no_test\n", None),
        }]
    );
    assert_eq!(hub.opened(), 0, "no pull request is opened for a run");
}

#[tokio::test]
async fn a_name_is_the_name_its_results_give_and_no_part_of_it_names_a_test() {
    let hub = Hub::new();
    let head = hub.committed().await;
    hub.github().fast(&head, vec![], Some(("test-results", junit())));
    let said = hub
        .named_check(&named(&["a", "tests::a", "carry tools::tests::a"], &[]))
        .await
        .unwrap()
        .expect("run");
    assert!(said.contains("): 0 of the 3 tests passed."), "{said}");
    assert_eq!(
        said.matches(": not run: no test in the run has this name").count(),
        3,
        "{said}"
    );
}

// A check that reports no test, as the lint, failed with nothing in the results to say why: it is said with the end of
// what the step that failed it printed, read from its log, without the colours and the times GitHub's log holds, and
// without the steps before it and the clean-up after it. Well under a second alone, a repository on disk and GitHub in
// memory; the one test of a job's log on `run_tests`.
#[tokio::test]
async fn a_check_named_runs_beside_the_tests_and_a_red_one_is_said_with_the_end_of_its_failed_step() {
    let hub = Hub::new();
    let head = hub.committed().await;
    let lint = job("lint", Outcome::Failed("failure".into()), 9);
    hub.github().fast(&head, vec![lint], None);
    hub.github().logged(9, &clippy_log());
    let said = hub.named_check(&named(&[], &["lint"])).await.unwrap().expect("run");
    assert!(said.contains("): 0 of the 1 checks passed."), "{said}");
    let lint = format!(
        "The jobs:\n- lint: failed (failure) in 60 s https://github.example/checks/lint\n  The end of what its failed \
         step printed:\n```\n{CLIPPY_STEP}\n```"
    );
    assert!(said.ends_with(&lint), "{said}");
    assert!(
        !said.contains('\u{1b}') && !said.contains("2026-11-21T09:48:"),
        "{said}"
    );
    assert_eq!(hub.github().logs_asked(), [9]);
    let asked = match hub.dispatched_as_said().as_slice() {
        [Asked::Dispatch { inputs, .. }] => inputs.clone(),
        other => panic!("{other:?}"),
    };
    assert_eq!(asked, inputs(&head, "", Some("[\"lint\"]")));
}

#[tokio::test]
async fn a_check_the_fast_workflow_does_not_run_by_name_is_refused_before_anything_is_pushed() {
    let hub = Hub::new();
    hub.committed().await;
    let said = hub
        .named_check(&named(&["a"], &["clippy"]))
        .await
        .unwrap()
        .expect("said");
    assert_eq!(
        said,
        "Nothing was run: the fast workflow runs only these checks by name: lint; not clippy."
    );
    assert!(hub.github().asked().is_empty(), "GitHub was asked nothing");
}

// A run that uploaded no results is said so, and a job of it that failed has its log read; a log GitHub does not give
// is said in one line, and the rest of the result is given. Well under a second alone, a repository on disk and GitHub
// in memory; the one test of a log that cannot be read on `run_tests`.
#[tokio::test]
async fn a_run_that_uploads_no_junit_is_said_so_and_a_log_that_cannot_be_read_is_said_in_one_line() {
    let hub = Hub::new();
    let head = hub.committed().await;
    hub.github()
        .fast(&head, vec![job("test", Outcome::Failed("failure".into()), 5)], None);
    let said = hub.named_check(&named(&["a"], &[])).await.unwrap().expect("run");
    assert!(
        said.contains(
            "The run uploaded no JUnit results the engine can read, so no test's result is known, and the engine reads \
             no log to guess one: the run uploaded no artifact test-results."
        ),
        "{said}"
    );
    assert!(
        said.ends_with(
            "The jobs:\n- test: failed (failure) in 60 s https://github.example/checks/test\n  Its log could not be \
             read: HTTP 404: Not Found"
        ),
        "{said}"
    );
    assert_eq!(hub.github().logs_asked(), [5]);
}

// What did not pass comes first, then the jobs, then the tests that passed, each with its time: a result cut short loses
// what passed first, which is what the agent least needs. Where the names are few, nothing is left out of it.
#[tokio::test]
async fn what_did_not_pass_and_the_jobs_are_said_before_the_tests_that_passed_each_with_its_time() {
    let hub = Hub::new();
    let head = hub.committed().await;
    let lint = Check {
        took: Some(192),
        ..passed("lint")
    };
    let jobs = vec![lint, job("test", Outcome::Failed("failure".into()), 5)];
    hub.github().fast(&head, jobs, Some(("test-results", junit())));
    let asked = named(&["tools::tests::a", "tools::tests::b"], &["lint"]);
    let said = hub.named_check(&asked).await.unwrap().expect("run");
    assert!(
        said.starts_with(&format!(
            "The fast workflow ran on the commit {head} (https://github.example/actions/runs/1000): 1 of the 2 tests \
             passed; 1 of the 1 checks passed."
        )),
        "{said}"
    );
    let b = said.find("- tools::tests::b: failed in 0.3 s").expect("b");
    let a = said.find("- tools::tests::a: passed in 0.12 s").expect("a");
    assert!(b < a, "{said}");
    assert!(said.contains("assertion failed: said.is_empty()"), "{said}");
    let jobs = "The jobs:\n- lint: passed in 192 s https://github.example/checks/lint\n- test: failed (failure) in 60 \
                s https://github.example/checks/test";
    let at = said.find(jobs).expect("the jobs");
    assert!(b < at && at < a, "{said}");
    assert!(
        said.ends_with("The tests that passed:\n- tools::tests::a: passed in 0.12 s"),
        "{said}"
    );
    assert_eq!(
        hub.dispatched_as_said(),
        [Asked::Dispatch {
            workflow: "fast.yml".to_string(),
            branch: "main".to_string(),
            inputs: inputs(&head, "tools::tests::a\ntools::tests::b\n", Some("[\"lint\"]")),
        }]
    );
}

// A job says how it ended and, where it ran to a conclusion, its wall time; one that did not run, did not finish or was
// cancelled says so in place of a time, and a job that did not run is no check that came to anything.
#[tokio::test]
async fn each_job_is_said_with_how_it_ended_and_its_wall_time_or_why_it_has_none() {
    let hub = Hub::new().with(|config| {
        let ci = config.ci.as_mut().expect("a declaration");
        ci.fast.checks = ["lint", "page", "docs", "deploy", "test"].map(String::from).to_vec();
    });
    let head = hub.committed().await;
    let jobs = vec![
        Check {
            took: Some(192),
            ..passed("lint")
        },
        check("page", Outcome::Broken("cancelled".into())),
        check("docs", Outcome::Skipped),
        check("deploy", Outcome::Pending),
        Check {
            took: None,
            ..passed("test")
        },
    ];
    hub.github().fast(&head, jobs, Some(("test-results", junit_green())));
    let every = named(&["tools::tests::a"], &["lint", "page", "docs", "deploy", "test"]);
    let said = hub.named_check(&every).await.unwrap().expect("run");
    assert!(said.contains("; 2 of the 4 checks passed."), "{said}");
    assert!(
        said.contains(
            "The jobs:\n\
             - lint: passed in 192 s https://github.example/checks/lint\n\
             - page: ended without judging the code (cancelled) https://github.example/checks/page\n\
             - docs: did not run https://github.example/checks/docs\n\
             - deploy: not complete https://github.example/checks/deploy\n\
             - test: passed (GitHub gives no time for it) https://github.example/checks/test"
        ),
        "{said}"
    );
}

// The fast workflow's run says every job it ran, the one named and the one that did not run alike.
#[tokio::test]
async fn the_fast_workflows_run_says_every_job_of_it_and_a_check_named_that_it_has_none_of() {
    let hub = Hub::new();
    let head = hub.committed().await;
    let jobs = vec![
        Check {
            took: Some(95),
            ..passed("test")
        },
        check("lint", Outcome::Skipped),
    ];
    hub.github().fast(&head, jobs, Some(("test-results", junit_green())));
    let said = hub
        .named_check(&named(&["tools::tests::a"], &[]))
        .await
        .unwrap()
        .expect("run");
    assert!(
        said.contains(
            "The jobs:\n- test: passed in 95 s https://github.example/checks/test\n\
             - lint: did not run https://github.example/checks/lint"
        ),
        "{said}"
    );
    let hub = Hub::new();
    let head = hub.committed().await;
    hub.github().fast(&head, vec![passed("test")], None);
    let said = hub.named_check(&named(&[], &["lint"])).await.unwrap().expect("run");
    assert!(
        said.ends_with(
            "- test: passed in 60 s https://github.example/checks/test\n- lint: the run has no job of that name"
        ),
        "{said}"
    );
}

#[tokio::test]
async fn nothing_can_be_run_where_the_project_declares_no_workflows() {
    let hub = Hub::new().with(|config| config.ci = None);
    hub.committed().await;
    let said = hub.named_check(&named(&["a"], &[])).await.unwrap().expect("said");
    assert_eq!(
        said,
        "Nothing was run: the project declares no workflows, so nothing can be run through run_tests."
    );
    assert!(hub.github().asked().is_empty(), "nothing was pushed or asked");
    assert!(hub.trials().is_empty());
}

// Only what is named is run: a request that names neither a test nor a check, a commit alone included, runs nothing, and
// pushes nothing, whatever the project's workflows are.
#[tokio::test]
async fn a_request_that_names_neither_a_test_nor_a_check_runs_nothing_and_pushes_nothing() {
    let hub = Hub::new();
    let head = hub.committed().await;
    let commit_alone = Named {
        commit: Some(head),
        ..Named::default()
    };
    for nothing in [Named::default(), commit_alone] {
        let said = hub.named_check(&nothing).await.unwrap().expect("said");
        assert_eq!(
            said,
            "Nothing was run: you named no test and no check, and only what is named is run. Name what to run, and ask \
             again. Nothing was pushed."
        );
    }
    assert!(hub.github().asked().is_empty(), "nothing was pushed or asked");
    assert!(hub.trials().is_empty());
}

#[tokio::test]
async fn a_workflow_that_runs_where_the_installation_has_no_runner_is_not_run() {
    let hub = Hub::new().with(|config| {
        let ci = config.ci.as_mut().expect("a declaration");
        ci.fast.runner = "14900K".to_string();
    });
    hub.committed().await;
    let said = hub.named_check(&named(&["a"], &[])).await.unwrap().expect("said");
    assert_eq!(
        said,
        "Nothing was run: the workflow runs on 14900K, and this installation has only github."
    );
    assert!(hub.dispatched().is_empty());
    let hub = Hub::new().with(|config| config.runners = vec!["github".to_string(), "14900K".to_string()]);
    let head = hub.committed().await;
    hub.github().fast(&head, vec![], Some(("test-results", junit())));
    assert!(
        hub.named_check(&named(&["tools::tests::a"], &[]))
            .await
            .unwrap()
            .is_some()
    );
}

// The work's branch may hold no file of the workflow, or one of its own making: the run is always asked for on main,
// by main's file, with the commit to test given, and the commit is pushed to a branch of its own that is gone once the
// result is read: nothing stays on GitHub but main.
#[tokio::test]
async fn the_workflow_is_run_by_mains_file_with_the_commit_given_and_nothing_stays_pushed() {
    let hub = Hub::new();
    let head = hub.committed().await;
    hub.github().fast(&head, vec![], Some(("test-results", junit())));
    let said = hub
        .named_check(&named(&["tools::tests::a"], &[]))
        .await
        .unwrap()
        .expect("run");
    assert!(said.contains("- tools::tests::a: passed in 0.12 s"), "{said}");
    let branches = run(
        &hub.github().bare,
        &["for-each-ref", "--format=%(refname:short)", "refs/heads"],
    );
    assert_eq!(branches, "main", "the branch pushed for the run is deleted");
    assert!(
        hub.github()
            .asked()
            .iter()
            .any(|a| matches!(a, Asked::Delete(name) if name.starts_with("wf-try/"))),
        "it was deleted by asking GitHub"
    );
}

// With no runner online, the run is listed at once, queued: it is given up as never started once the start is waited
// for, and not waited for until the run would time out.
#[tokio::test]
async fn a_run_no_runner_took_is_given_up_as_never_started() {
    let hub = Hub::new();
    let head = hub.committed().await;
    hub.github().no_runner();
    let began = std::time::Instant::now();
    let why = hub.named_check(&named(&["a"], &[])).await.expect_err("not run");
    assert!(
        why.starts_with(&format!(
            "the run of fast.yml asked for on {head} never started: no runner took it after "
        )),
        "{why}"
    );
    assert!(
        why.ends_with("(https://github.example/actions/runs/1000), and the engine cancelled it"),
        "{why}"
    );
    assert!(
        began.elapsed() < Duration::from_secs(2),
        "the timeout was not waited for"
    );
    assert!(
        hub.github().asked().contains(&Asked::Cancel(1000)),
        "the run is cancelled"
    );
}

// A run not complete in the time a run may take is cancelled, and said as what it found: each job, the one not
// complete said so, and no test's result known.
#[tokio::test]
async fn a_run_past_its_time_is_cancelled_and_said_with_its_jobs_not_complete() {
    let hub = Hub::new().with(|config| config.timeout = Duration::from_secs(1));
    let head = hub.committed().await;
    hub.github().runs_go_on();
    let jobs = vec![check("lint", Outcome::Pending)];
    hub.github().fast(&head, jobs, Some(("test-results", junit())));
    let said = hub
        .named_check(&named(&["a"], &["lint"]))
        .await
        .expect("said")
        .expect("run");
    assert!(
        said.contains("the run did not complete within 1 second, the time it may take, and the engine cancelled it"),
        "{said}"
    );
    assert!(said.contains("- lint: not complete"), "{said}");
    assert!(
        hub.github().asked().contains(&Asked::Cancel(1000)),
        "the run is cancelled"
    );
}

// GitHub's refusal of the run, whatever its words, "No ref found" or that the workflow takes no such request, is
// said once as GitHub's, with nothing else pushed or run in its place.
#[tokio::test]
async fn a_refusal_to_run_a_workflow_is_said_once_as_githubs() {
    let hub = Hub::new();
    let head = hub.committed().await;
    hub.github().refuse_dispatch("HTTP 422: No ref found for: main");
    let why = hub.named_check(&named(&["a"], &[])).await.expect_err("not run");
    assert_eq!(
        why,
        format!("GitHub refused to run fast.yml on main for {head}: HTTP 422: No ref found for: main")
    );
    assert_eq!(hub.dispatched().len(), 1, "asked once");
}

// Another request of the same work asks for a run of the same commit just after this one: each reads the run it asked
// for, told by its own value in the run's name, never the newest.
#[tokio::test]
async fn a_request_reads_the_run_it_asked_for_where_another_asked_for_one_after_it() {
    let hub = Hub::new();
    let head = hub.committed().await;
    hub.github().fast(&head, vec![], Some(("test-results", junit())));
    hub.github().another_call_after_each();
    let said = hub
        .named_check(&named(&["tools::tests::a"], &[]))
        .await
        .unwrap()
        .expect("run");
    assert!(
        said.starts_with(&format!(
            "The fast workflow ran on the commit {head} (https://github.example/actions/runs/1000): 1 of the 1 tests \
             passed."
        )),
        "{said}"
    );
    assert!(!said.contains(super::super::fake::ANOTHER_CALLS), "{said}");
}

// What a test said is kept whole, however long.
#[tokio::test]
async fn what_a_failed_test_said_is_kept_whole() {
    let hub = Hub::new();
    let head = hub.committed().await;
    let said: String = (0..70).map(|n| format!("line {n:02} {}\n", "x".repeat(90))).collect();
    let xml = format!(
        r#"<testsuites><testsuite name="carry"><testcase name="tools::tests::b" time="0.3"><failure message="m">{said}</failure></testcase></testsuite></testsuites>"#
    );
    let files = vec![("junit.xml".to_string(), xml.into_bytes())];
    hub.github().fast(&head, Vec::new(), Some(("test-results", files)));
    let told = hub
        .named_check(&named(&["tools::tests::b"], &[]))
        .await
        .unwrap()
        .expect("run");
    assert!(told.len() > crate::process::TAIL, "{}", told.len());
    assert!(told.contains("line 00 ") && told.contains("line 69 "), "{told}");
    assert!(!told.contains("left out"), "{told}");
}

#[test]
fn a_time_is_read_in_seconds_minutes_hours_or_days() {
    use super::super::fast::span_of;
    assert_eq!(span_of("30s").unwrap(), Duration::from_secs(30));
    assert_eq!(span_of("90m").unwrap(), Duration::from_secs(5400));
    assert_eq!(span_of("2h").unwrap(), Duration::from_secs(7200));
    assert_eq!(span_of("1d").unwrap(), Duration::from_secs(86_400));
    for wrong in ["", "h", "2", "2 weeks", "-1h"] {
        assert!(span_of(wrong).is_err(), "{wrong:?}");
    }
}
