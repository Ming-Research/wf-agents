//! The agents' `run_tests` on the GitHub host with the tests and checks they name, run by the project's fast workflow,
//! and with none named, run by nothing: a workflow run by hand on main's file with the commit given, whose JUnit
//! results say each test's own result and time, and whose jobs' logs say why a job failed that no test of its own says.

use driver::red_main::RedTest;

use std::collections::BTreeMap;

use super::super::api::{Check, Outcome};
use super::*;
use crate::host::{Measure, Named};
use decide::Obstacle;

pub(super) fn named(tests: &[&str], checks: &[&str]) -> Named {
    let all = |xs: &[&str]| xs.iter().map(|x| x.to_string()).collect();
    Named {
        tests: all(tests),
        checks: all(checks),
        ..Named::default()
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
    pub(super) async fn named_check(&self, named: &Named) -> Result<Option<String>, String> {
        self.named_failed(named).await.map_err(|failed| failed.why)
    }

    /// As [`Hub::named_check`], the failure as the host told it, with its kind and what holds what asked.
    pub(super) async fn named_failed(&self, named: &Named) -> Result<Option<String>, Failed> {
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

// The line the engine gives is an input of the run, in whole seconds rounded up; where the history gives none, or the
// project's workflow takes no such input, the run is asked for without it.
#[tokio::test]
async fn the_stop_the_engine_gives_is_an_input_of_the_run_in_whole_seconds_rounded_up() {
    let hub = Hub::new();
    let head = hub.committed().await;
    hub.github()
        .fast(&head, vec![passed("test")], Some(("test-results", junit_green())));
    let place = hub.place();
    let mut stop = Stop::never().test_time(Some(12_001.0));
    (hub.host
        .run_tests(
            &branch(&place),
            &named(&["tools::tests::a"], &[]),
            hub.call(),
            &mut stop,
        )
        .await)
        .expect("run")
        .expect("nothing stopped it");
    let mut want = inputs(&head, "tools::tests::a\n", None);
    want.push(("test_stop".to_string(), "13".to_string()));
    let asked = hub.dispatched_as_said();
    assert!(
        matches!(asked.as_slice(), [Asked::Dispatch { inputs, .. }] if *inputs == want),
        "{asked:?}"
    );
}

#[test]
fn a_stop_is_given_only_where_the_history_says_one_and_the_workflow_takes_one() {
    let said =
        |input: &str, millis: Option<f64>| super::super::fast::test_stop(input, &Stop::never().test_time(millis));
    assert_eq!(
        said("test_stop", Some(1.0)),
        Some(("test_stop".to_string(), "1".to_string()))
    );
    assert_eq!(
        said("test_stop", Some(5_000.0)),
        Some(("test_stop".to_string(), "5".to_string()))
    );
    assert_eq!(said("test_stop", None), None, "the history gives none");
    assert_eq!(said("test_stop", Some(0.0)), None, "no time is no stop");
    assert_eq!(said("", Some(5_000.0)), None, "the workflow takes none");
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
    assert!(
        !said.contains("also failed on"),
        "no test is known to be red on main: {said}"
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

// A test that failed and is red on the head of main as the engine knows it is said so beneath its own lines, with main's
// name and the head it failed on; one that failed and is not red there, and one that passed, are not. The one test of
// what a result says of main, and of which tests it says it for. Milliseconds alone, a repository on disk and GitHub in
// memory.
#[tokio::test]
async fn a_failed_test_that_is_red_on_main_is_said_to_have_failed_there_too_and_no_other_is() {
    let hub = Hub::new();
    let head = hub.committed().await;
    let xml = r#"<testsuites><testsuite name="carry">
  <testcase name="tools::tests::a" time="0.12"/>
  <testcase name="tools::tests::b" time="0.3"><failure message="assertion failed">b panicked</failure></testcase>
  <testcase name="tools::tests::c" time="0.2"><failure message="assertion failed">c panicked</failure></testcase>
</testsuite></testsuites>"#;
    let results = vec![("test.xml".to_string(), xml.as_bytes().to_vec())];
    hub.github().fast(
        &head,
        vec![job("test", Outcome::Failed("failure".into()), 5)],
        Some(("test-results", results)),
    );
    let red = |unit: &str, name: &str| RedTest {
        unit: unit.to_string(),
        name: name.to_string(),
        head: "9f8e7d6c5b4a".to_string(),
    };
    // A test that passed is never said to have failed, whatever the engine knows of main; and `tools::tests::c` is red
    // on main in another unit, so it is another test than the one of this result.
    let known = vec![
        red("carry", "tools::tests::b"),
        red("carry", "tools::tests::a"),
        red("db", "tools::tests::c"),
    ];
    let place = hub.place();
    let mut stop = Stop::never().red_on_main(known);
    let said = (hub.host)
        .run_tests(
            &branch(&place),
            &named(&["tools::tests::a", "tools::tests::b", "tools::tests::c"], &[]),
            hub.call(),
            &mut stop,
        )
        .await
        .expect("run")
        .expect("nothing stopped it");
    let also = "  This test also failed on main, at its head 9f8e7d6c5b4a.";
    assert_eq!(said.matches(also).count(), 1, "{said}");
    assert!(
        said.contains(&format!(
            "- tools::tests::b: failed in 0.3 s\n{also}\n  What it said:\n```\nb panicked\n```"
        )),
        "{said}"
    );
    let c = said.split("- tools::tests::c").nth(1).expect("the test is said");
    assert!(!c.contains("also failed on main"), "{said}");
}

// A run whose results hold more tests than the call named: the result lists each test named once with its time, and
// none of the others, passed or failed.
#[tokio::test]
async fn a_result_lists_only_the_tests_named_though_the_run_holds_many_more() {
    let hub = Hub::new();
    let head = hub.committed().await;
    let xml = r#"<testsuites><testsuite name="carry">
  <testcase name="tools::tests::a" time="0.12"/>
  <testcase name="tools::tests::b" time="0.3"/>
  <testcase name="tools::tests::c" time="0.4"><failure message="assertion failed">boom</failure></testcase>
  <testcase name="tools::tests::d" time="0.5"/>
</testsuite></testsuites>"#;
    let results = vec![("test.xml".to_string(), xml.as_bytes().to_vec())];
    hub.github().fast(&head, vec![], Some(("test-results", results)));
    let said = hub
        .named_check(&named(&["tools::tests::a"], &[]))
        .await
        .unwrap()
        .expect("nothing stopped it");
    assert!(said.contains(": 1 of the 1 tests passed."), "{said}");
    assert_eq!(said.matches("tools::tests::a: passed in 0.12 s").count(), 1, "{said}");
    for other in ["tools::tests::b", "tools::tests::c", "tools::tests::d", "boom"] {
        assert!(!said.contains(other), "{other} was not named\n\n{said}");
    }
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

// With no runner of its kind online, the run is listed at once, queued: once the start is waited for it is cancelled,
// and what asked is held, not failed, the timeout of a run not being waited for.
#[tokio::test]
async fn a_run_no_runner_took_is_cancelled_and_holds_what_asked() {
    let hub = super::queued::on_14900k(Hub::new());
    let head = hub.committed().await;
    hub.github().queue_runs();
    let began = std::time::Instant::now();
    let failed = hub.named_failed(&named(&["a"], &[])).await.expect_err("not run");
    assert!(
        failed.why.starts_with(&format!(
            "the run of fast.yml asked for on {head} never started: no runner took it after "
        )),
        "{failed}"
    );
    assert!(
        failed.why.ends_with(
            "(https://github.example/actions/runs/1000), none of 14900k is online, and the engine cancelled the run"
        ),
        "{failed}"
    );
    assert_eq!(
        failed.held,
        Some(crate::host::Held {
            obstacle: Obstacle::NoRunner,
            retry: None
        })
    );
    assert_eq!(failed.kind, Some(Failure::Passing));
    assert!(
        began.elapsed() < Duration::from_secs(2),
        "the timeout was not waited for"
    );
    assert_eq!(hub.github().runners_asked(), 1, "the runners were asked after once");
    assert_eq!(cancelled(&hub), [1000]);
}

// A run GitHub never lists, in the time the CI may take to start, is a fault of GitHub's for now: what asked is held, the
// run is not cancelled (there is none to cancel) and the timeout of a run is not waited for.
#[tokio::test]
async fn a_run_github_never_lists_holds_what_asked_as_a_fault_of_its_own() {
    let hub = super::queued::on_14900k(Hub::new());
    let head = hub.committed().await;
    hub.github().hide_runs();
    let began = std::time::Instant::now();
    let failed = hub.named_failed(&named(&["a"], &[])).await.expect_err("not run");
    assert!(
        failed.why.starts_with(&format!(
            "the run of fast.yml asked for on {head} never started: GitHub listed none after "
        )),
        "{failed}"
    );
    assert_eq!(
        failed.held,
        Some(crate::host::Held {
            obstacle: Obstacle::Fault,
            retry: None
        })
    );
    assert_eq!(failed.kind, Some(Failure::Passing));
    assert!(
        began.elapsed() < Duration::from_secs(2),
        "the timeout was not waited for"
    );
    assert!(cancelled(&hub).is_empty(), "no run was listed to cancel");
}

// GitHub's own faults and its limit on the requests it takes keep a request from being run for now, at each place the
// host asks it: they hold what asked, with when it may be asked again where GitHub says, and nothing of them is said as
// the result of the tests.
#[tokio::test]
async fn a_fault_of_githubs_or_its_limit_holds_what_asked_and_says_nothing_of_the_tests() {
    for (refused, obstacle, retry) in [
        ("HTTP 503: Service Unavailable", Obstacle::Fault, None),
        ("HTTP 502: Bad Gateway", Obstacle::Fault, None),
        (
            "HTTP 429: Too Many Requests",
            Obstacle::Limit,
            Some(decide::Retry::In(60)),
        ),
    ] {
        let hub = Hub::new();
        hub.committed().await;
        hub.github().refuse_dispatch(refused);
        let failed = hub.named_failed(&named(&["a"], &[])).await.expect_err("held");
        assert_eq!(failed.held, Some(crate::host::Held { obstacle, retry }), "{refused}");
        assert_eq!(failed.kind, Some(Failure::Passing), "{refused}");
    }
    // Any other refusal is the agent's to read, and holds nothing.
    let hub = Hub::new();
    hub.committed().await;
    hub.github().refuse_dispatch("HTTP 422: No ref found for: main");
    let failed = hub.named_failed(&named(&["a"], &[])).await.expect_err("refused");
    assert_eq!(failed.held, None);
}

// A run that ended, whose results GitHub cannot give for now, holds what asked and is not said to have no results;
// asked again, it reads the same run, which no second one was asked for.
#[tokio::test]
async fn results_githubs_fault_keeps_from_being_read_hold_what_asked_and_are_read_by_asking_again() {
    let hub = Hub::new();
    let head = hub.committed().await;
    hub.github().fast(&head, vec![], Some(("test-results", junit_green())));
    hub.github().refuse_artifact(Some("HTTP 502: Bad Gateway"));
    let place = hub.place();
    let (at, request, call) = (branch(&place), named(&["tools::tests::a"], &[]), hub.call());
    let first = hub.host.run_tests(&at, &request, call, &mut Stop::never()).await;
    let failed = first.expect_err("held");
    assert_eq!(
        failed.held,
        Some(crate::host::Held {
            obstacle: Obstacle::Fault,
            retry: None
        })
    );
    hub.github().refuse_artifact(None);
    let said = hub.host.run_tests(&at, &request, call, &mut Stop::never()).await;
    assert!(said.expect("read").expect("run").contains("tools::tests::a"));
    assert_eq!(hub.dispatched().len(), 1, "one run on {head}");
}

// The log of a job that failed with nothing in the results to say why is read to say it: where GitHub cannot give it
// for now, what asked is held and not told that the log could not be read; asked again, it reads the same run and the
// log.
#[tokio::test]
async fn a_log_githubs_fault_keeps_from_being_read_holds_what_asked_and_is_read_by_asking_again() {
    let hub = Hub::new();
    let head = hub.committed().await;
    hub.github()
        .fast(&head, vec![job("test", Outcome::Failed("failure".into()), 5)], None);
    hub.github().refuse_log(Some("HTTP 503: Service Unavailable"));
    let place = hub.place();
    let (at, request, call) = (branch(&place), named(&["a"], &[]), hub.call());
    let first = hub.host.run_tests(&at, &request, call, &mut Stop::never()).await;
    let failed = first.expect_err("held");
    assert_eq!(
        failed.held,
        Some(crate::host::Held {
            obstacle: Obstacle::Fault,
            retry: None
        })
    );
    hub.github().refuse_log(None);
    hub.github().logged(5, "the log of the job");
    let said = hub.host.run_tests(&at, &request, call, &mut Stop::never()).await;
    assert!(said.expect("read").expect("run").contains("the log of the job"));
    assert_eq!(hub.dispatched().len(), 1, "one run on {head}");
}

// `gh` says nothing of GitHub's status where the network keeps it from reaching GitHub: the log is then not read for
// the network's failure, which holds what asked as a failure of GitHub's does, and not told that the log could not be
// read; asked again, it reads the same run and the log.
#[tokio::test]
async fn a_log_the_network_keeps_from_being_read_holds_what_asked_and_is_read_by_asking_again() {
    let hub = Hub::new();
    let head = hub.committed().await;
    hub.github()
        .fast(&head, vec![job("test", Outcome::Failed("failure".into()), 5)], None);
    hub.github().refuse_log(Some(
        "gh api GET repos/o/r/actions/jobs/5/logs gave no answer (exit status: 1): error connecting to api.github.com\n\
         check your internet connection or https://githubstatus.com",
    ));
    let place = hub.place();
    let (at, request, call) = (branch(&place), named(&["a"], &[]), hub.call());
    let first = hub.host.run_tests(&at, &request, call, &mut Stop::never()).await;
    let failed = first.expect_err("held");
    assert_eq!(
        failed.held,
        Some(crate::host::Held {
            obstacle: Obstacle::Network,
            retry: None
        })
    );
    hub.github().refuse_log(None);
    hub.github().logged(5, "the log of the job");
    let said = hub.host.run_tests(&at, &request, call, &mut Stop::never()).await;
    assert!(said.expect("read").expect("run").contains("the log of the job"));
    assert_eq!(hub.dispatched().len(), 1, "one run on {head}");
}

// A run not complete in the time a run may take is cancelled, and said as what it found: each job, the one not
// complete said so, and no test's result known.
#[tokio::test]
async fn a_run_past_its_time_is_cancelled_and_said_with_its_jobs_not_complete() {
    let hub = Hub::new().with(|config| config.ci.as_mut().expect("a declaration").fast.too_long = "1s".to_string());
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

/// The measurement `compare`, as the tests' project declares it where it measures: `measure.yml` on the machine
/// `14900k`, which the installation has, given the commit, the commit the change began from, the call and both
/// commits in one input, letting an agent set `passes`, and reporting the requests a second and the 99th percentile of
/// each line in `bench.csv` of the artifact `measurement`.
pub(super) fn measuring(config: &mut Config) {
    let pairs = |pairs: &[(&str, &str)]| -> BTreeMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    };
    let compare = flow::agents::Measurement {
        about: "the commit against where its change began".to_string(),
        workflow: "measure.yml".to_string(),
        runner: "14900k".to_string(),
        too_long: "1m".to_string(),
        inputs: pairs(&[
            ("commit", "{commit}"),
            ("base", "{base}"),
            ("call", "{call}"),
            ("revisions", "base={base} head={commit}"),
        ]),
        open: vec!["passes".to_string()],
        artifact: "measurement".to_string(),
        file: "bench.csv".to_string(),
        metrics: pairs(&[("rps", "requests/s"), ("p99_ms", "ms")]),
    };
    let ci = config.ci.as_mut().expect("a declaration");
    ci.measure.insert("compare".to_string(), compare);
    config.runners.push("14900k".to_string());
}

/// A request that names these measurements, setting none of their inputs.
pub(super) fn measured(names: &[&str]) -> Named {
    Named {
        measure: (names.iter())
            .map(|name| Measure {
                name: name.to_string(),
                inputs: BTreeMap::new(),
            })
            .collect(),
        ..Named::default()
    }
}

/// What `bench.csv` holds of a run of the measurement `compare`: each line's requests a second and 99th percentile.
pub(super) const BENCH: &str = "line,test,rps,p99_ms\nbase,set,150000,0.31\nhead,set,161000.5,0.29\n";

/// What the run of `compare` uploads: the host's description beside `bench.csv`.
pub(super) fn timings() -> Vec<(String, Vec<u8>)> {
    vec![
        ("host.txt".to_string(), b"cpus: 32".to_vec()),
        ("bench.csv".to_string(), BENCH.as_bytes().to_vec()),
    ]
}

/// Main moves on GitHub, as other works merge, and the host fetches it: its head now.
async fn main_moves(hub: &Hub) -> String {
    let other = hub.repo.data.join("other");
    run(
        &hub.repo.data,
        &["clone", "-q", &hub.github().bare.display().to_string(), "other"],
    );
    run(&other, &["config", "user.name", "Someone"]);
    run(&other, &["config", "user.email", "someone@example.com"]);
    let moved = commit(&other, "weather.txt", "Sunny.\n");
    run(&other, &["push", "-q", "origin", "main"]);
    assert_eq!(hub.host.main().await, Ok(moved.clone()));
    moved
}

// A measurement named runs its workflow once, on main's file, with the inputs its declaration gives, the commit, the
// commit the work's change began from (not main's head once main has moved on) and the run's own value filled in,
// and the inputs the request sets beside them. What it said is its run, each job, and what it measured as the file
// holds it, with each metric's unit. Milliseconds alone, a repository on disk and GitHub in memory; the one test of a
// measurement's inputs and of how its result is said.
#[tokio::test]
async fn a_measurement_named_runs_its_workflow_with_the_inputs_declared_and_set_and_what_it_measured_is_said() {
    let hub = Hub::new().with(measuring);
    let head = hub.committed().await;
    let began = run(&hub.place().dir, &["rev-parse", "HEAD~1"]);
    let moved = main_moves(&hub).await;
    assert_ne!(moved, began, "main has moved past where the change began");
    hub.github().of_workflow(
        "measure.yml",
        &head,
        vec![passed("bench")],
        Some(("measurement", timings())),
    );
    let mut asked = measured(&["compare"]);
    asked.measure[0].inputs.insert("passes".to_string(), "6".to_string());
    let said = hub.named_check(&asked).await.unwrap().expect("nothing stopped it");
    assert_eq!(
        said,
        format!(
            "The measurement compare ran measure.yml on the commit {head}, whose change began from {began} \
             (https://github.example/actions/runs/1000): every job of it passed.\n\n\
             The jobs:\n- bench: passed in 60 s https://github.example/checks/bench\n\n\
             It wrote 2 rows to bench.csv, whose metrics are `p99_ms` in ms, `rps` in requests/s:\n```\n{}\n```",
            BENCH.trim_end()
        )
    );
    assert_eq!(
        hub.dispatched_as_said(),
        [Asked::Dispatch {
            workflow: "measure.yml".to_string(),
            branch: "main".to_string(),
            inputs: vec![
                ("base".to_string(), began.clone()),
                ("call".to_string(), "<call>".to_string()),
                ("commit".to_string(), head.clone()),
                ("revisions".to_string(), format!("base={began} head={head}")),
                ("passes".to_string(), "6".to_string()),
            ],
        }]
    );
    assert_eq!(hub.opened(), 0, "no pull request is opened for a run");
    assert!(hub.trials().is_empty(), "nothing stays pushed: {:?}", hub.trials());
}

// A request that names tests and two measurements has the fast workflow run, then each measurement in the order named,
// each with a value of its own, though both are of one workflow on one commit, and says all in one result, the fast
// workflow's first. Milliseconds alone; the one test of a request of several.
#[tokio::test]
async fn tests_and_measurements_named_together_run_the_fast_workflow_and_then_each_measurement() {
    let hub = Hub::new().with(measuring);
    let head = hub.committed().await;
    hub.github().fast(&head, vec![], Some(("test-results", junit())));
    hub.github().of_workflow(
        "measure.yml",
        &head,
        vec![passed("bench")],
        Some(("measurement", timings())),
    );
    let asked = Named {
        tests: vec!["tools::tests::a".to_string()],
        ..measured(&["compare", "compare"])
    };
    let said = hub.named_check(&asked).await.unwrap().expect("nothing stopped it");
    let fast = said
        .find("The fast workflow ran on the commit")
        .expect("the fast workflow is said");
    let measure = said
        .find("The measurement compare ran")
        .expect("the measurement is said");
    assert!(fast < measure, "{said}");
    assert_eq!(said.matches("The measurement compare ran").count(), 2, "{said}");
    assert!(said.contains("tools::tests::a: passed in 0.12 s"), "{said}");
    let workflows: Vec<String> = (hub.dispatched_as_said().into_iter())
        .map(|asked| match asked {
            Asked::Dispatch { workflow, .. } => workflow,
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(workflows, ["fast.yml", "measure.yml", "measure.yml"]);
    let calls: Vec<String> = (hub.dispatched().into_iter())
        .filter_map(|asked| match asked {
            Asked::Dispatch { inputs, .. } => inputs.into_iter().find(|(name, _)| name == "call"),
            _ => None,
        })
        .map(|(_, value)| value)
        .collect();
    assert_eq!(calls.len(), 3);
    assert!(calls[0] != calls[1] && calls[1] != calls[2], "{calls:?}");
    assert!(hub.trials().is_empty(), "nothing stays pushed: {:?}", hub.trials());
}

// A measurement the project cannot run is refused before anything is pushed or asked for, and the tests named beside
// it are not run either: where the project declares none; and, every reason at once, a measurement it does not
// declare, an input the measurement does not let an agent set, and a machine the installation lacks. Milliseconds
// alone; the one test of what refuses a measurement named.
#[tokio::test]
async fn a_measurement_the_project_cannot_run_is_refused_before_anything_is_pushed_and_runs_no_test_either() {
    let with_tests = |named: Named| Named {
        tests: vec!["tools::tests::a".to_string()],
        ..named
    };
    let hub = Hub::new();
    hub.committed().await;
    let said = hub
        .named_check(&with_tests(measured(&["compare"])))
        .await
        .unwrap()
        .expect("said");
    assert_eq!(
        said,
        "Nothing was run: the project declares no measurement, so none can be named. Nothing was pushed."
    );
    assert!(hub.github().asked().is_empty(), "nothing was pushed or asked");
    let hub = Hub::new().with(measuring);
    hub.committed().await;
    let mut asked = with_tests(measured(&["startup", "compare"]));
    asked.measure[1].inputs.insert("mode".to_string(), "scale".to_string());
    let said = hub.named_check(&asked).await.unwrap().expect("said");
    assert_eq!(
        said,
        "Nothing was run: the project declares no measurement startup; it declares `compare`. Nothing was pushed.\n\
         Nothing was run: the measurement compare does not let you set `mode`; it lets you set `passes`. Nothing was \
         pushed."
    );
    assert!(hub.github().asked().is_empty(), "nothing was pushed or asked");
    let hub = Hub::new()
        .with(measuring)
        .with(|config| config.runners = vec!["github".to_string()]);
    hub.committed().await;
    let said = hub
        .named_check(&with_tests(measured(&["compare"])))
        .await
        .unwrap()
        .expect("said");
    assert_eq!(
        said,
        "Nothing was run: the workflow runs on 14900k, and this installation has only github."
    );
    assert!(hub.github().asked().is_empty(), "nothing was pushed or asked");
    assert!(hub.trials().is_empty());
}

// A measurement's run is waited for as long as its own declaration says, not as long as the fast workflow's: one that
// goes on past it is cancelled, and said so, with no metric given, and so is one a person cancels. About a second
// alone, which is the wait itself; the one test of the limit of a measurement's run and of one cancelled.
#[tokio::test]
async fn a_measurement_cancelled_by_its_limit_or_by_a_person_is_said_so_and_gives_no_metric() {
    let hub = Hub::new().with(measuring).with(|config| {
        let ci = config.ci.as_mut().expect("a declaration");
        ci.fast.too_long = "1m".to_string();
        ci.measure.get_mut("compare").expect("declared").too_long = "1s".to_string();
    });
    let head = hub.committed().await;
    hub.github().runs_go_on();
    hub.github().of_workflow(
        "measure.yml",
        &head,
        vec![check("bench", Outcome::Pending)],
        Some(("measurement", timings())),
    );
    let said = hub
        .named_check(&measured(&["compare"]))
        .await
        .expect("said")
        .expect("run");
    assert!(said.contains("not every job of it passed"), "{said}");
    assert!(
        said.contains(
            "What it measured is not known, and the engine reads no log to guess it: the run did not complete within \
             1 second, the time it may take, and the engine cancelled it."
        ),
        "{said}"
    );
    assert!(!said.contains("It wrote"), "{said}");
    assert!(
        hub.github().asked().contains(&Asked::Cancel(1000)),
        "the run is cancelled"
    );
    // A person cancels the run of the next request while it goes on.
    let asked = measured(&["compare"]);
    let (said, ()) = tokio::join!(hub.named_check(&asked), async {
        while hub.dispatched().len() < 2 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        hub.github().cancel_runs();
    });
    let said = said.expect("said").expect("run");
    assert!(
        said.contains("- test: ended without judging the code (cancelled)"),
        "{said}"
    );
    assert!(
        said.contains(
            "What it measured is not known, and the engine reads no log to guess it: the run uploaded no \
                       artifact measurement."
        ),
        "{said}"
    );
}

// A measurement whose job failed is said so, with the end of what its failed step printed, and what it measured all
// the same where its file can be read; one whose file is not as its declaration says is said with why, and none of it
// is given. Milliseconds alone; the one test of a failed measurement and of a file refused.
#[tokio::test]
async fn a_failed_measurement_is_said_with_its_step_and_a_file_not_as_declared_is_said_with_why() {
    let hub = Hub::new().with(measuring);
    let head = hub.committed().await;
    hub.github().of_workflow(
        "measure.yml",
        &head,
        vec![job("bench", Outcome::Failed("failure".to_string()), 7)],
        Some(("measurement", timings())),
    );
    hub.github().logged(7, &clippy_log());
    let said = hub.named_check(&measured(&["compare"])).await.unwrap().expect("run");
    assert!(
        said.contains("not every job of it passed, as its jobs below say"),
        "{said}"
    );
    assert!(said.contains("- bench: failed (failure)"), "{said}");
    assert!(said.contains("The end of what its failed step printed:"), "{said}");
    assert!(said.contains("It wrote 2 rows to bench.csv"), "{said}");
    let hub = Hub::new().with(measuring);
    let head = hub.committed().await;
    let wrong = vec![("bench.csv".to_string(), b"line,rps\nbase,fast\n".to_vec())];
    hub.github().of_workflow(
        "measure.yml",
        &head,
        vec![passed("bench")],
        Some(("measurement", wrong)),
    );
    let said = hub.named_check(&measured(&["compare"])).await.unwrap().expect("run");
    assert!(
        said.contains(
            "What it measured is not known, and the engine reads no log to guess it: the artifact measurement: \
             bench.csv has no column p99_ms, which the measurement's declaration names as a metric."
        ),
        "{said}"
    );
    assert!(!said.contains("base,fast"), "{said}");
}

// Two projects that measure on one machine take turns: while one's run goes on there, the other's request asks GitHub
// for no run and says that it waits for the machine; once that run has ended and been read, the other's run is asked
// for. A run on GitHub's own machines waits for none. About a fifth of a second alone, which is the wait the test gives
// the second request to ask for a run it must not ask for; the one test of the turns across projects.
#[tokio::test]
async fn a_measurement_waits_while_another_project_measures_on_the_same_machine_and_runs_once_that_ends() {
    let machines = super::machines();
    let shared = |config: &mut Config| config.machines = machines.clone();
    let firn = Hub::new().with(measuring).with(shared).sharing();
    let whitefoot = Hub::new().with(measuring).with(shared).sharing();
    let firn_head = firn.committed().await;
    let whitefoot_head = whitefoot.committed().await;
    for (hub, head) in [(&firn, &firn_head), (&whitefoot, &whitefoot_head)] {
        hub.github().of_workflow(
            "measure.yml",
            head,
            vec![passed("bench")],
            Some(("measurement", timings())),
        );
    }
    firn.github().runs_go_on();
    let asked = measured(&["compare"]);
    let (to, mut came) = tokio::sync::mpsc::unbounded_channel();
    let reply = service::CallReply::new(WORK, CallId(77), to);
    let saying = reply.saying();
    let held: Held<u8, service::CallReply> = Held::new();
    let mut stop = held.hold(1, reply).expect("held").saying(saying);
    let place = whitefoot.place();
    let on = branch(&place);
    let whitefoot_asks = whitefoot.host.run_tests(&on, &asked, whitefoot.call(), &mut stop);
    let (firn_said, whitefoot_said) = tokio::join!(firn.named_check(&asked), async {
        while firn.dispatched().is_empty() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        tokio::pin!(whitefoot_asks);
        tokio::select! {
            _ = &mut whitefoot_asks => panic!("whitefoot measured beside firn"),
            () = tokio::time::sleep(Duration::from_millis(200)) => {}
        }
        assert!(
            whitefoot.dispatched().is_empty(),
            "no run of whitefoot's is asked for while firn's goes on"
        );
        firn.github().finish_runs();
        whitefoot_asks.await
    });
    assert!(firn_said.unwrap().expect("read").contains("every job of it passed"));
    assert!(
        whitefoot_said
            .unwrap()
            .expect("read")
            .contains("It wrote 2 rows to bench.csv")
    );
    assert_eq!(whitefoot.dispatched().len(), 1);
    let mut said = Vec::new();
    while let Ok(message) = came.try_recv() {
        if let service::Message::Along { along, .. } = message {
            said.push(along);
        }
    }
    assert!(
        matches!(said.first(), Some(Some(service::Along::WaitsForMachine { machine, .. })) if machine == "14900k"),
        "{said:?}"
    );
    assert!(
        matches!(said.last(), Some(Some(service::Along::Runs { .. }))),
        "it runs once its turn came: {said:?}"
    );
    // On GitHub's own machines a measurement waits for no other.
    let on_github = |config: &mut Config| {
        measuring(config);
        let ci = config.ci.as_mut().expect("a declaration");
        ci.measure.get_mut("compare").expect("declared").runner = "github".to_string();
    };
    let one = Hub::new().with(on_github).with(shared).sharing();
    let other = Hub::new().with(on_github).with(shared).sharing();
    let (one_head, other_head) = (one.committed().await, other.committed().await);
    for (hub, head) in [(&one, &one_head), (&other, &other_head)] {
        hub.github().of_workflow(
            "measure.yml",
            head,
            vec![passed("bench")],
            Some(("measurement", timings())),
        );
    }
    one.github().runs_go_on();
    let (_, other_said) = tokio::join!(
        async {
            tokio::select! {
                _ = one.named_check(&asked) => panic!("its run goes on"),
                () = tokio::time::sleep(Duration::from_millis(300)) => {}
            }
        },
        async {
            while one.dispatched().is_empty() {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            other.named_check(&asked).await
        }
    );
    assert!(other_said.unwrap().expect("read").contains("every job of it passed"));
}

// A fast run on a machine a measurement holds may wait in the machine's gate, and that wait is the measurement's: it is
// not timed against the run's limit, as a wait for a runner is not, and the run is cancelled for its limit only once the
// measurement has let the machine go. About two seconds alone, the waits themselves; the one test of a run's time while
// a measurement holds its machine.
#[tokio::test]
async fn a_run_is_not_timed_while_a_measurement_holds_its_machine() {
    let hub = Hub::new().with(measuring).with(|config| {
        let fast = &mut config.ci.as_mut().expect("a declaration").fast;
        fast.runner = "14900k".to_string();
        fast.too_long = "1s".to_string();
    });
    let head = hub.committed().await;
    hub.github().runs_go_on();
    hub.github().fast(
        &head,
        vec![check("test", Outcome::Pending)],
        Some(("test-results", junit())),
    );
    let machines = hub.host.config.machines.clone();
    let poll = Duration::from_millis(10);
    let turn = (machines.turn("14900k", WorkNumber(7), poll, &mut Stop::never()).await)
        .unwrap()
        .expect("a turn");
    let asked = named(&["tools::tests::a"], &[]);
    let (said, ()) = tokio::join!(hub.named_check(&asked), async {
        tokio::time::sleep(Duration::from_millis(1500)).await;
        let asked = hub.github().asked();
        assert_eq!(hub.dispatched().len(), 1, "{asked:?}");
        assert!(
            !asked.contains(&Asked::Cancel(1000)),
            "the run was cancelled while the machine was held"
        );
        drop(turn);
    });
    let said = said.unwrap().expect("run");
    assert!(said.contains("did not complete within 1 second"), "{said}");
    assert!(hub.github().asked().contains(&Asked::Cancel(1000)));
}

/// What a request that publishes the commit says first, as the words of `run_tests` say it.
fn published(commit: &str, hub: &Hub) -> String {
    format!(
        "The commit {commit} is on GitHub as the branch {} until the work ends; start a workflow on it with that branch \
         as its ref.",
        kept(commit, hub)
    )
}

/// The branch on GitHub that the commit is published on, for the work of the hub.
fn kept(commit: &str, hub: &Hub) -> String {
    format!("wf-try/{}/commit/{commit}", hub.place().branch)
}

// A request that asks only to publish the commit puts it on a branch of the work's own and runs nothing: no workflow is
// asked for, no branch for the call is pushed, and the result names the branch. With a commit named, that commit is the
// one left there; and with tests named as well they run, and the branch stays when the call returns while the call's own
// is gone. Milliseconds alone, a repository on disk and GitHub in memory; the one test of what `publish` leaves on GitHub
// and of what it says.
#[tokio::test]
async fn a_commit_published_is_left_on_a_branch_of_the_works_own_and_nothing_is_run_for_it() {
    let hub = Hub::new();
    let head = hub.committed().await;
    let parent = run(&hub.place().dir, &["rev-parse", "HEAD~1"]);
    let alone = Named {
        publish: true,
        ..Named::default()
    };
    let said = hub.named_check(&alone).await.unwrap().expect("said");
    assert_eq!(said, published(&head, &hub));
    assert_eq!(
        hub.trials(),
        [kept(&head, &hub)],
        "it is the only branch the work has there"
    );
    assert!(hub.dispatched().is_empty(), "no workflow was run");
    assert!(hub.deleted().is_empty(), "nothing was deleted");
    // A commit named is the one published, beside the one before it, and asking again for one published is no change.
    let older = Named {
        commit: Some(parent.clone()),
        ..alone.clone()
    };
    for _ in 0..2 {
        let said = hub.named_check(&older).await.unwrap().expect("said");
        assert_eq!(said, published(&parent, &hub));
    }
    let mut left = vec![kept(&head, &hub), kept(&parent, &hub)];
    left.sort();
    assert_eq!(hub.trials(), left);
    // With tests named too, they run on the commit, and only the branch of the call goes once the result is read.
    hub.github().fast(&head, vec![], Some(("test-results", junit())));
    let with_tests = Named {
        publish: true,
        ..named(&["tools::tests::a"], &[])
    };
    let said = hub.named_check(&with_tests).await.unwrap().expect("run");
    assert!(
        said.starts_with(&format!(
            "{}\n\nThe fast workflow ran on the commit {head}",
            published(&head, &hub)
        )),
        "{said}"
    );
    assert_eq!(hub.dispatched().len(), 1);
    assert_eq!(hub.trials(), left, "only the published branches stay");
    let calls: Vec<String> = hub
        .deleted()
        .into_iter()
        .filter(|name| name.contains("/call/"))
        .collect();
    assert_eq!(calls.len(), 1, "the call's own branch was deleted: {calls:?}");
}

/// The tests' project's fast workflow as one that takes the budget and the work's number.
fn takes_budget_and_work(config: &mut Config) {
    let fast = &mut config.ci.as_mut().expect("a declaration").fast;
    fast.budget_input = "budget_seconds".to_string();
    fast.work_input = "work".to_string();
}

/// A stop through which what the host says reaches the loop for this call, knowing these medians of the project's
/// earlier fast runs; and the way to hear what it said.
fn hosted(call: Asking, medians: &[(&str, f64)]) -> (Stop, tokio::sync::mpsc::UnboundedReceiver<service::Message>) {
    let Asking::Call(id) = call else {
        panic!("a call of run_tests is asked for");
    };
    let (to, said) = tokio::sync::mpsc::unbounded_channel();
    let reply = service::CallReply::new(WORK, id, to);
    let medians: std::collections::BTreeMap<String, f64> = (medians.iter())
        .map(|(name, millis)| (name.to_string(), *millis))
        .collect();
    let stop = Stop::never()
        .hosting(reply.hosting())
        .fast_medians(std::sync::Arc::new(medians));
    (stop, said)
}

// A call of run_tests gives the fast workflow its budget in whole seconds, the one run_tests.toml gives every project,
// and the work's number, each as the input the declaration names; a workflow that names neither is given neither, as
// the other tests of this file show.
#[tokio::test]
async fn a_call_gives_the_fast_workflow_its_budget_and_the_works_number_where_the_declaration_names_their_inputs() {
    let hub = Hub::new().with(takes_budget_and_work);
    let head = hub.committed().await;
    hub.github()
        .fast(&head, vec![passed("test")], Some(("test-results", junit_green())));
    (hub.named_check(&named(&["tools::tests::a"], &[])).await)
        .expect("run")
        .expect("nothing stopped it");
    let mut want = inputs(&head, "tools::tests::a\n", None);
    want.push(("budget_seconds".to_string(), "600".to_string()));
    want.push(("work".to_string(), WORK.0.to_string()));
    let asked = hub.dispatched_as_said();
    assert!(
        matches!(asked.as_slice(), [Asked::Dispatch { inputs, .. }] if *inputs == want),
        "{asked:?}"
    );
}

// The project's own budget takes the place of the one every project has; the check before a merge, which runs again
// what its full workflows failed, has none, so that what it runs again runs whole as the full workflows ran it.
#[test]
fn a_call_has_its_projects_budget_or_else_the_tools_and_a_round_of_the_check_has_none() {
    let hub = Hub::new();
    assert_eq!(hub.host.budget(hub.call()), Ok(Some(600)), "run_tests.toml's 10m");
    assert_eq!(hub.host.budget(hub.round()), Ok(None));
    let hub = hub.with(|config| config.ci.as_mut().expect("a declaration").fast.budget = Some("90s".to_string()));
    assert_eq!(hub.host.budget(hub.call()), Ok(Some(90)));
    assert_eq!(hub.host.budget(hub.round()), Ok(None));
}

// A request is refused where the medians of its tests' times in earlier fast runs, one alone or together, are over the
// budget; never at the budget, never for a test no fast run timed, and each name counts once, trimmed.
#[test]
fn a_request_is_refused_where_one_median_or_their_sum_is_over_the_budget_and_an_untimed_test_counts_for_nothing() {
    let medians: std::collections::BTreeMap<&str, f64> =
        std::collections::BTreeMap::from([("a", 30_000.0), ("b", 31_000.0), ("c", 61_000.0), ("whole", 60_000.0)]);
    let median = |test: &str| medians.get(test).copied();
    let refused = |tests: &[&str]| {
        let tests: Vec<String> = tests.iter().map(|t| t.to_string()).collect();
        super::super::fast::over_budget(&tests, 60, median).map(|refused| {
            (refused.iter())
                .map(|t| (t.name.to_string(), t.median))
                .collect::<Vec<_>>()
        })
    };
    assert_eq!(refused(&["a"]), None);
    assert_eq!(refused(&["whole"]), None, "a median at the budget is not over it");
    assert_eq!(
        refused(&["a", "never_timed"]),
        None,
        "a test no fast run timed counts for nothing"
    );
    assert_eq!(refused(&[]), None);
    assert_eq!(
        refused(&["c"]),
        Some(vec![("c".to_string(), Some(61_000.0))]),
        "one test over the budget alone"
    );
    assert_eq!(
        refused(&["a", " b ", "a", "never_timed"]),
        Some(vec![
            ("a".to_string(), Some(30_000.0)),
            ("b".to_string(), Some(31_000.0)),
            ("never_timed".to_string(), None),
        ]),
        "together over it, each named once and in order"
    );
}

// A request known to take longer than the budget runs nothing and pushes nothing: it says why for each test it named,
// and is said to the loop as short of time, each test with its median.
#[tokio::test]
async fn a_request_known_to_run_over_the_budget_runs_nothing_and_says_why_for_each_test() {
    let hub = Hub::new()
        .with(takes_budget_and_work)
        .with(|config| config.ci.as_mut().expect("a declaration").fast.budget = Some("60s".to_string()));
    hub.committed().await;
    let medians = [("long", 75_000.0), ("short", 20_000.0), ("shorter", 44_500.0)];
    let place = hub.place();
    let call = hub.call();
    let (mut stop, mut said) = hosted(call, &medians);
    let asked = named(&["long", "never_timed"], &[]);
    let result = (hub.host.run_tests(&branch(&place), &asked, call, &mut stop).await)
        .expect("answered")
        .expect("nothing stopped it");
    assert_eq!(
        result,
        "Nothing was run, and nothing was pushed: by their times in this project's earlier fast runs, the tests you named \
         would take longer than the fast check's budget of 60 s. Name fewer or shorter tests; the check before the merge \
         runs every test.\n\
         - long: refused: its median of 75 s is over the budget of 60 s\n\
         - never_timed: not run with the others, though not refused itself: no earlier fast run timed it"
    );
    assert!(hub.trials().is_empty(), "nothing was pushed");
    assert!(hub.github().asked().is_empty(), "GitHub was asked nothing");
    let Asking::Call(id) = call else { unreachable!() };
    let short = |name: &str, millis: Option<u64>| work::Short {
        name: name.to_string(),
        how: work::ShortHow::Refused,
        millis,
    };
    assert_eq!(
        super::ends::heard(&mut said),
        [engine::Event::ShortOfTime {
            call: id,
            budget: 60,
            tests: vec![short("long", Some(75_000)), short("never_timed", None)],
        }]
    );
    // Two tests each under the budget and over it together are refused, each saying what they come to.
    let call = hub.call();
    let (mut stop, _) = hosted(call, &medians);
    let asked = named(&["short", "shorter"], &[]);
    let result = (hub.host.run_tests(&branch(&place), &asked, call, &mut stop).await)
        .expect("answered")
        .expect("nothing stopped it");
    for line in [
        "- short: refused: its median of 20 s brings the tests named to 65 s, over the budget of 60 s",
        "- shorter: refused: its median of 45 s brings the tests named to 65 s, over the budget of 60 s",
    ] {
        assert!(result.contains(line), "{line}\n\n{result}");
    }
    assert!(hub.github().asked().is_empty(), "GitHub was asked nothing");
}

/// The JUnit of a run whose budget ran out: `tools::tests::a` passed, `slow` was stopped as the budget ran out, and
/// `later` was not started; in the file of the job `test`, which failed for it.
fn junit_short_of_time() -> Vec<(String, Vec<u8>)> {
    let xml = r#"<testsuites><testsuite name="carry">
  <testcase name="tools::tests::a" time="0.12"/>
  <testcase name="slow" time="58.5"><error message="stopped: the fast check's time ran out after 60 s"/></testcase>
  <testcase name="later"><skipped message="not run: the fast check's time was used up"/></testcase>
</testsuite></testsuites>"#;
    vec![("test.xml".to_string(), xml.as_bytes().to_vec())]
}

// What the budget kept from running to its end is said apart from what did not pass, in the budget's own words, and is
// no failure: the call has its result, its job's failure needs no log, and the loop is told the tests it kept short.
#[tokio::test]
async fn the_tests_a_run_stopped_or_did_not_start_for_its_budget_are_said_apart_from_failures_and_kept() {
    let hub = Hub::new().with(takes_budget_and_work);
    let head = hub.committed().await;
    let test = job("test", Outcome::Failed("failure".into()), 5);
    hub.github()
        .fast(&head, vec![test], Some(("test-results", junit_short_of_time())));
    let place = hub.place();
    let call = hub.call();
    let (mut stop, mut said) = hosted(call, &[]);
    let asked = named(&["tools::tests::a", "slow", "later"], &[]);
    let result = (hub.host.run_tests(&branch(&place), &asked, call, &mut stop).await)
        .expect("answered")
        .expect("nothing stopped it");
    assert!(result.contains("1 of the 3 tests passed."), "{result}");
    assert!(
        result.contains(
            "The tests the fast check's time did not let finish:\n\
             - slow: stopped: the fast check's time ran out in 58.5 s\n\
             - later: not run: the fast check's time was used up"
        ),
        "{result}"
    );
    assert!(!result.contains("The tests that did not pass"), "{result}");
    assert!(hub.github().logs_asked().is_empty(), "the stop says why its job failed");
    let Asking::Call(id) = call else { unreachable!() };
    let events = super::ends::heard(&mut said);
    let timed: Vec<&work::Reading> = (events.iter())
        .filter_map(|e| match e {
            engine::Event::Timed { reading } => Some(reading),
            _ => None,
        })
        .collect();
    let tests: Vec<&str> = timed[0].tests.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(tests, ["slow", "tools::tests::a"], "a test not started has no time");
    let short: Vec<&engine::Event> = (events.iter())
        .filter(|e| matches!(e, engine::Event::ShortOfTime { .. }))
        .collect();
    assert_eq!(
        short,
        [&engine::Event::ShortOfTime {
            call: id,
            budget: 600,
            tests: vec![
                work::Short {
                    name: "slow".to_string(),
                    how: work::ShortHow::Stopped,
                    millis: Some(58_500),
                },
                work::Short {
                    name: "later".to_string(),
                    how: work::ShortHow::NotStarted,
                    millis: None,
                },
            ],
        }]
    );
}
