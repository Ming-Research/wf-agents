//! The check before a merge on the GitHub host where the full workflows failed tests and nothing else: the failed tests
//! are run again by the fast workflow on the commit, beside every check it runs by name, and what fails again there is
//! run on main's head. A test that passes again passes the commit and is observed as maybe flaky; a test that fails on
//! main's head as well holds the commit; one that fails on the commit alone fails it. Each test here runs in well under
//! a second alone, a repository on disk and GitHub in memory, and is the only one that holds its outcome to the runs.

use super::super::api::Outcome;
use super::*;

/// The JUnit of a run in which `tools::tests::b`, the test the full workflows failed, passed or failed.
fn b(passed: bool) -> Vec<(String, Vec<u8>)> {
    let outcome = if passed {
        ""
    } else {
        "<failure message=\"assertion failed\">thread 'tools::tests::b' panicked</failure>"
    };
    let xml = format!(
        r#"<testsuites><testsuite name="carry"><testcase name="tools::tests::b" time="0.3">{outcome}</testcase></testsuite></testsuites>"#
    );
    vec![("junit.xml".to_string(), xml.into_bytes())]
}

fn red() -> Outcome {
    Outcome::Failed("failure".into())
}

/// Each run GitHub was asked for: its workflow, the commit it runs on, the tests and the checks it names.
fn runs(hub: &Hub) -> Vec<(String, String, String, String)> {
    (hub.dispatched().into_iter())
        .map(|asked| match asked {
            Asked::Dispatch { workflow, inputs, .. } => {
                let input = |name: &str| {
                    (inputs.iter())
                        .find(|(n, _)| n == name)
                        .map_or(String::new(), |(_, v)| v.clone())
                };
                (workflow, input("commit"), input("tests"), input("checks"))
            }
            other => panic!("{other:?}"),
        })
        .collect()
}

impl Hub {
    /// The work's commit, whose full workflows pass the lint and fail `tools::tests::b`, the job of the tests failing
    /// with it; and the head of main on GitHub.
    async fn red_by_b(&self) -> (String, String) {
        let head = self.committed().await;
        let jobs = vec![passed("lint"), check("test", red())];
        self.github()
            .of_workflow("ci.yml", &head, jobs, Some(("test-results", fast::junit())));
        (head, self.main_on_github())
    }

    /// The fast workflow on the commit passes the lint, and passes or fails `tools::tests::b`.
    fn fast_on(&self, commit: &str, passes: bool, lint: Outcome) {
        let test = if passes { passed("test") } else { check("test", red()) };
        let jobs = vec![check("lint", lint), test];
        self.github()
            .of_workflow("fast.yml", commit, jobs, Some(("test-results", b(passes))));
    }
}

// The test fails on the full workflows and passes on the commit again: the commit passes, the test is said maybe
// flaky in the notes and observed in the same words, and nothing is run on main.
#[tokio::test]
async fn a_failed_test_that_passes_when_the_commit_is_run_again_passes_it_and_is_observed() {
    let hub = Hub::new();
    let (head, _) = hub.red_by_b().await;
    hub.fast_on(&head, true, Outcome::Passed);
    let verdict = hub.verdict(&head).await.unwrap().expect("judged");
    assert_eq!((&verdict.failed, &verdict.held), (&None, &None), "{verdict:?}");
    let flaky = format!(
        "The test `tools::tests::b` failed on the full workflows of the commit {head} \
         (https://github.example/actions/runs/1000) and passed when the fast workflow ran it again on that commit \
         (https://github.example/actions/runs/1001): it may be flaky."
    );
    assert_eq!(verdict.notes.last(), Some(&flaky));
    assert_eq!(verdict.observations, [flaky]);
    let ran = [
        ("ci.yml", head.as_str(), "", ""),
        ("fast.yml", head.as_str(), "tools::tests::b\n", "[\"lint\"]"),
    ];
    let ran: Vec<_> = (ran.iter())
        .map(|(w, c, t, k)| (w.to_string(), c.to_string(), t.to_string(), k.to_string()))
        .collect();
    assert_eq!(runs(&hub), ran);
}

// The test fails again on the commit, and on main's head too: the commit is held, not failed, saying which test fails
// on main as well, with what each run found.
#[tokio::test]
async fn a_test_that_fails_again_and_on_mains_head_holds_the_commit() {
    let hub = Hub::new();
    let (head, main) = hub.red_by_b().await;
    hub.fast_on(&head, false, Outcome::Passed);
    hub.fast_on(&main, false, Outcome::Passed);
    let verdict = hub.verdict(&head).await.unwrap().expect("judged");
    assert_eq!(verdict.failed, None);
    assert!(verdict.observations.is_empty(), "{verdict:?}");
    let held = verdict.held.expect("held");
    let says = format!(
        "Failed on the commit, again when it was run again, and on the head of main, {main}, as well: \
         `tools::tests::b`.\n\nThe full workflows ran on the commit {head}"
    );
    assert!(held.starts_with(&says), "{held}");
    for part in [
        "What failed was run again on the commit by the fast workflow, beside every check it runs by name:",
        &format!(
            "What failed again was run on the head of main, {main}:\n\nThe fast workflow ran on the commit {main}"
        ),
    ] {
        assert!(held.contains(part), "{part}\n\n{held}");
    }
    let main_run = (main.clone(), "tools::tests::b\n".to_string(), String::new());
    let on_main: Vec<_> = (runs(&hub).into_iter())
        .filter(|(_, commit, _, _)| *commit == main)
        .map(|(_, c, t, k)| (c, t, k))
        .collect();
    assert_eq!(on_main, [main_run], "main's head is run with the test alone");
}

// The test fails again on the commit and passes on main's head: the change fails it, and the commit fails, said as
// the full workflows found it, then as each run again found it.
#[tokio::test]
async fn a_test_that_fails_again_and_passes_on_mains_head_fails_the_commit() {
    let hub = Hub::new();
    let (head, main) = hub.red_by_b().await;
    hub.fast_on(&head, false, Outcome::Passed);
    hub.fast_on(&main, true, Outcome::Passed);
    let verdict = hub.verdict(&head).await.unwrap().expect("judged");
    assert_eq!(verdict.held, None);
    let failed = verdict.failed.expect("failed");
    assert!(
        failed.starts_with(&format!("The full workflows ran on the commit {head}")),
        "{failed}"
    );
    assert!(
        failed.ends_with("Failed again on the commit, and not on the head of main: `tools::tests::b`."),
        "{failed}"
    );
}

// A check the full workflows found red beside a failed test is red again on the commit: the commit fails, whatever
// became of the test, which passed and is observed; nothing is run on main.
#[tokio::test]
async fn a_red_lint_beside_a_failed_test_fails_the_commit_when_it_is_red_again() {
    let hub = Hub::new();
    let head = hub.committed().await;
    let jobs = vec![check("lint", red()), check("test", red())];
    hub.github()
        .of_workflow("ci.yml", &head, jobs, Some(("test-results", fast::junit())));
    hub.fast_on(&head, true, red());
    let verdict = hub.verdict(&head).await.unwrap().expect("judged");
    assert_eq!(verdict.held, None);
    assert_eq!(verdict.observations.len(), 1, "{verdict:?}");
    let failed = verdict.failed.expect("the lint fails it");
    // What the run again found is said last: its jobs, the lint red again and the test's job passed.
    let again = failed
        .split("What failed was run again on the commit")
        .nth(1)
        .expect("the run again is said");
    assert!(
        again.contains(
            "- lint: failed (failure) in 60 s https://github.example/checks/lint\n- test: passed in 60 s \
             https://github.example/checks/test"
        ),
        "{failed}"
    );
    assert_eq!(runs(&hub).len(), 2, "nothing is run on main");
}

// What failed beside the tests is not run again: a job that ended without judging the code, a unit not built. Nor is
// anything where the project has no fast workflow, which is said.
#[tokio::test]
async fn nothing_is_run_again_where_more_than_tests_failed_or_nothing_can_run_them() {
    let unbuilt = r#"<testsuites><testsuite name="carry"><testcase name="tools::tests::b"><failure/></testcase></testsuite>
<testsuite name="db::rounds"><error message="build failed"/></testsuite></testsuites>"#;
    let unbuilt = vec![("junit.xml".to_string(), unbuilt.as_bytes().to_vec())];
    for (jobs, junit) in [
        (
            vec![passed("lint"), check("test", Outcome::Broken("cancelled".into()))],
            fast::junit(),
        ),
        (vec![passed("lint"), check("test", red())], unbuilt),
    ] {
        let hub = Hub::new();
        let head = hub.committed().await;
        hub.github()
            .of_workflow("ci.yml", &head, jobs, Some(("test-results", junit)));
        let verdict = hub.verdict(&head).await.unwrap().expect("judged");
        let failed = verdict.failed.expect("failed");
        assert!(!failed.contains("run again"), "{failed}");
        assert_eq!(runs(&hub).len(), 1, "only the full workflows ran");
    }
    let hub = Hub::new().with(|config| {
        if let Some(ci) = &mut config.ci {
            ci.fast = None;
        }
    });
    let (head, _) = hub.red_by_b().await;
    let failed = hub
        .verdict(&head)
        .await
        .unwrap()
        .expect("judged")
        .failed
        .expect("failed");
    assert!(
        failed.ends_with(
            "What failed was not run again: Nothing was run: tests or checks are named, and the project declares no \
             fast workflow, which alone runs only what is named; name none to run the full workflows."
        ),
        "{failed}"
    );
    assert_eq!(runs(&hub).len(), 1);
}
