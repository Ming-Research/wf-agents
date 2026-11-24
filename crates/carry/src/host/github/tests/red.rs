//! The check before a merge on the GitHub host where the full workflows failed tests and nothing else: the failed tests
//! are run again by the fast workflow on the commit, beside every check it runs by name, and what fails again there is
//! run on main's head. A test that passes again passes the commit and is observed as maybe flaky; a test that fails on
//! main's head as well holds the commit; one that fails on the commit alone, or that could not be run again, fails it,
//! and so does a run again that judged nothing. Each test here runs in well under a second alone, a repository on disk
//! and GitHub in memory, and is the only one that holds its outcome to the runs.

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

/// The JUnit of a run of `tools::tests::b` and `tools::tests::c`, each passed or failed as given, in this file: the full
/// workflows' is the file of the job `test`, and the fast workflow's is one file for the whole run.
fn b_and_c(file: &str, b: bool, c: bool) -> Vec<(String, Vec<u8>)> {
    let case = |name: &str, passed: bool| {
        let failure = if passed {
            String::new()
        } else {
            format!("<failure message=\"assertion failed\">thread '{name}' panicked</failure>")
        };
        format!(r#"<testcase name="{name}" time="0.3">{failure}</testcase>"#)
    };
    let xml = format!(
        r#"<testsuites><testsuite name="carry">{}{}</testsuite></testsuites>"#,
        case("tools::tests::b", b),
        case("tools::tests::c", c)
    );
    vec![(file.to_string(), xml.into_bytes())]
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

/// The project's full workflow as main holds it in these tests: the page, the tests and the docs, and `check`, which
/// needs every other job and so sums them up.
const CI: &str = "name: ci\njobs:\n  page:\n    runs-on: x\n  test:\n    runs-on: x\n  docs:\n    runs-on: x\n  \
                  check:\n    needs: [page, test, docs]\n    if: always()\n";

/// A file of results of the job, named by it.
fn file(job: &str, xml: &str) -> (String, Vec<u8>) {
    (format!("{job}.xml"), xml.as_bytes().to_vec())
}

impl Hub {
    /// Main holds this file of the full workflow, on GitHub as here: before the work starts, which takes main in.
    fn with_workflow(&self, text: &str) {
        let repo = &self.repo.project.repo;
        std::fs::create_dir_all(repo.join(".github/workflows")).unwrap();
        commit(repo, ".github/workflows/ci.yml", text);
        run(repo, &["push", "-q", &self.github().bare.display().to_string(), "main"]);
    }

    /// The work's commit, whose full workflows pass the lint and fail `tools::tests::b`, the job of the tests failing
    /// with it; and the head of main on GitHub.
    async fn red_by_b(&self) -> (String, String) {
        let head = self.committed().await;
        let jobs = vec![passed("lint"), check("test", red())];
        self.github()
            .of_workflow("ci.yml", &head, jobs, Some(("test-results", fast::junit())));
        (head, self.main_on_github())
    }

    /// The verdict of this round of the check on the commit, which holds main's head on GitHub: given again after the
    /// engine started again, a round is the same one.
    async fn verdict_of(&self, commit: &str, round: Asking) -> Option<Verdict> {
        let (place, main) = (self.place(), self.main_on_github());
        (self.host)
            .verdict(
                &branch(&place),
                commit,
                &main,
                round,
                &Mutex::new(()),
                &mut Stop::never(),
            )
            .await
            .map_err(String::from)
            .expect("the check judges")
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
        "These failed on the commit, failed again when they were run again, and also failed on the head of main, \
         {main}: `tools::tests::b`.\n\nThe full workflows ran on the commit {head}"
    );
    assert!(held.starts_with(&says), "{held}");
    for part in [
        "The fast workflow ran what failed again on the commit, together with every check it runs by name:",
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
        failed.ends_with("These failed again on the commit, but not on the head of main: `tools::tests::b`."),
        "{failed}"
    );
    // The full workflows ran `tools::tests::a` too, and it passed: only what did not pass is said of them.
    assert!(!failed.contains("tools::tests::a"), "{failed}");
}

// Where the full workflows failed several tests and nothing else, and each fails again on the commit, the commit is held
// only where every one of them fails on main's head as well; one that passes there is the change's, and the commit
// fails, saying it. Well under a second alone each way, a repository on disk and GitHub in memory; no other test holds
// a verdict to more than one test failed.
#[tokio::test]
async fn several_tests_that_fail_again_hold_the_commit_only_where_all_fail_on_mains_head() {
    for c_passes_on_main in [false, true] {
        let hub = Hub::new();
        let head = hub.committed().await;
        let jobs = || vec![passed("lint"), check("test", red())];
        hub.github().of_workflow(
            "ci.yml",
            &head,
            jobs(),
            Some(("test-results", b_and_c("test.xml", false, false))),
        );
        let main = hub.main_on_github();
        hub.github().of_workflow(
            "fast.yml",
            &head,
            jobs(),
            Some(("test-results", b_and_c("junit.xml", false, false))),
        );
        hub.github().of_workflow(
            "fast.yml",
            &main,
            jobs(),
            Some(("test-results", b_and_c("junit.xml", false, c_passes_on_main))),
        );
        let verdict = hub.verdict(&head).await.unwrap().expect("judged");
        if c_passes_on_main {
            assert_eq!(verdict.held, None, "{verdict:?}");
            let failed = verdict.failed.expect("failed");
            assert!(
                failed.ends_with("These failed again on the commit, but not on the head of main: `tools::tests::c`."),
                "{failed}"
            );
        } else {
            assert_eq!(verdict.failed, None, "{verdict:?}");
            let held = verdict.held.expect("held");
            assert!(
                held.starts_with(&format!(
                    "These failed on the commit, failed again when they were run again, and also failed on the head \
                     of main, {main}: `tools::tests::b`, `tools::tests::c`."
                )),
                "{held}"
            );
        }
    }
}

// The check given again, as after the engine started again, its runs having ended: it finds the full run, the run
// again on the commit and the run on main's head by the values it asked for them by, asks for none of them twice, and
// comes to the same verdict. Well under a second alone, a repository on disk and GitHub in memory; no other test gives
// a check that ran its failed tests again a second time.
#[tokio::test]
async fn a_check_given_again_finds_its_runs_again_and_asks_for_none_twice() {
    let hub = Hub::new();
    let (head, main) = hub.red_by_b().await;
    hub.fast_on(&head, false, Outcome::Passed);
    hub.fast_on(&main, false, Outcome::Passed);
    let round = hub.round();
    let first = hub.verdict_of(&head, round).await.expect("judged");
    assert!(first.held.is_some(), "{first:?}");
    let asked = runs(&hub);
    assert_eq!(
        asked.len(),
        3,
        "the full workflows, the run again and the run on main: {asked:?}"
    );
    let again = hub.verdict_of(&head, round).await.expect("judged");
    assert_eq!(runs(&hub), asked, "nothing is asked for twice");
    assert_eq!((&again.held, &again.failed), (&first.held, &first.failed));
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
    // What the run again found is said last: its jobs, the lint red again, with what its log says, here none to be
    // read, and the test's job passed.
    let again = failed
        .split("The fast workflow ran what failed again on the commit")
        .nth(1)
        .expect("the run again is said");
    assert!(
        again.contains(
            "- lint: failed (failure) in 60 s https://github.example/checks/lint\n  Its log could not be read: HTTP \
             404: Not Found\n- test: passed in 60 s https://github.example/checks/test"
        ),
        "{failed}"
    );
    assert_eq!(runs(&hub).len(), 2, "nothing is run on main");
}

// What failed beside the tests is not run again: a job that ended without judging the code, a unit not built. Nor is
// anything where the fast workflow runs where the installation has no runner, which is said.
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
            ci.fast.runner = "14900K".to_string();
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
            "What failed was not run again: Nothing was run: the workflow runs on 14900K, and this installation has \
             only github."
        ),
        "{failed}"
    );
    assert_eq!(runs(&hub).len(), 1);
}

// A job of tests that did not pass and reported no failed test of its own, in the file named by it, failed for a step
// before or beside its tests: the commit fails as the full workflows found it, and nothing is run again, though the
// failed test of another job would pass again; and a step the job says failed, as its file says it, is said with the
// job. Well under a second alone, a repository on disk and GitHub in memory; no other test holds a job to its own file.
#[tokio::test]
async fn a_job_red_with_no_failed_test_of_its_own_fails_the_commit_beside_a_test_that_would_pass_again() {
    let tests = r#"<testsuites><testsuite name="carry"><testcase name="tools::tests::b"><failure/></testcase></testsuite></testsuites>"#;
    let page = r#"<testsuites><testsuite name="src/page.test.ts"><testcase name="a page > shows the list"/></testsuite></testsuites>"#;
    let step = r#"<testsuites><testsuite name="page"><error message="the step Build the page of the job failed"/></testsuite></testsuites>"#;
    for (page, said) in [
        (page, None),
        (step, Some("page: the step Build the page of the job failed")),
    ] {
        let hub = Hub::new();
        hub.with_workflow(CI);
        let head = hub.committed().await;
        let jobs = vec![check("page", red()), check("test", red()), check("check", red())];
        let files = vec![file("page", page), file("test", tests)];
        hub.github()
            .of_workflow("ci.yml", &head, jobs, Some(("test-results", files)));
        hub.fast_on(&head, true, Outcome::Passed);
        let verdict = hub.verdict(&head).await.unwrap().expect("judged");
        assert_eq!((&verdict.held, verdict.observations.len()), (&None, 0), "{verdict:?}");
        let failed = verdict.failed.expect("failed");
        assert!(!failed.contains("run again"), "{failed}");
        assert!(failed.contains("- page: failed (failure)"), "{failed}");
        if let Some(said) = said {
            assert!(failed.contains(said), "{failed}");
        }
        assert_eq!(runs(&hub).len(), 1, "only the full workflows ran");
    }
}

// The job `tree` of a run on a change of only the design tree fails a step of its own: its results say so as a suite of
// its name with an error, a unit not built, not as a test that failed. The commit fails as the full workflows found it,
// with that step said, and nothing is run again, as would be a test of that name on the fast workflow. Well under a
// second alone, a repository on disk and GitHub in memory; no other test holds a job of no tests to its own file.
#[tokio::test]
async fn a_design_tree_check_that_failed_fails_the_commit_by_its_step_and_runs_no_test_again() {
    let step = r#"<testsuites><testsuite name="tree"><error message="the step Citations of the documents of the job failed"/></testsuite></testsuites>"#;
    let hub = Hub::new();
    hub.with_workflow(CI);
    let head = hub.committed().await;
    let jobs = vec![
        passed("scope"),
        check("tree", red()),
        check("page", Outcome::Skipped),
        check("test", Outcome::Skipped),
        check("check", red()),
    ];
    hub.github()
        .of_workflow("ci.yml", &head, jobs, Some(("test-results", vec![file("tree", step)])));
    hub.fast_on(&head, true, Outcome::Passed);
    let verdict = hub.verdict(&head).await.unwrap().expect("judged");
    assert_eq!((&verdict.held, verdict.observations.len()), (&None, 0), "{verdict:?}");
    let failed = verdict.failed.expect("failed");
    assert!(!failed.contains("run again"), "{failed}");
    assert!(failed.contains("- tree: failed (failure)"), "{failed}");
    assert!(
        failed.contains("tree: the step Citations of the documents of the job failed"),
        "{failed}"
    );
    assert_eq!(runs(&hub).len(), 1, "only the full workflows ran");
}

// A job that did not pass and left no results of its own, as one whose runner died before it named them, is let
// through only where the run again reproduces it, as a check the fast workflow runs by name, or where it only sums up
// the others, needing every other job of the full workflow as main holds it: beside a failed test of another job that
// would pass again, any other such job fails the commit as the full workflows found it, and nothing is run again.
// Well under a second alone, a repository on disk and GitHub in memory; no other test holds a red job with no results.
#[tokio::test]
async fn a_red_job_that_left_no_results_of_its_own_fails_the_commit_unless_the_run_again_or_the_sum_accounts_for_it() {
    let tests = r#"<testsuites><testsuite name="carry"><testcase name="tools::tests::b"><failure/></testcase></testsuite></testsuites>"#;
    let judged = |workflow: bool, jobs: Vec<Check>| async move {
        let hub = Hub::new();
        if workflow {
            hub.with_workflow(CI);
        }
        let head = hub.committed().await;
        let files = vec![file("test", tests)];
        hub.github()
            .of_workflow("ci.yml", &head, jobs, Some(("test-results", files)));
        hub.fast_on(&head, true, Outcome::Passed);
        let verdict = hub.verdict(&head).await.unwrap().expect("judged");
        (verdict, runs(&hub).len())
    };
    // The docs left nothing, and neither the run again nor the sum accounts for them.
    let (verdict, ran) = judged(
        true,
        vec![check("docs", red()), check("test", red()), check("check", red())],
    )
    .await;
    let failed = verdict.failed.expect("the docs fail it");
    assert!(
        failed.contains("- docs: failed (failure)") && !failed.contains("run again"),
        "{failed}"
    );
    assert_eq!((ran, verdict.observations.len()), (1, 0), "nothing is run again");
    // The lint is a check the fast workflow runs by name, which the run again passes; `check` sums up the others.
    let (verdict, ran) = judged(
        true,
        vec![check("lint", red()), check("test", red()), check("check", red())],
    )
    .await;
    assert_eq!(
        (verdict.failed, ran),
        (None, 2),
        "the run again accounts for the lint, the sum for check"
    );
    // With no file of the workflow on main, nothing says `check` only sums up the others.
    let (verdict, ran) = judged(false, vec![check("test", red()), check("check", red())]).await;
    assert!(
        verdict.failed.is_some_and(|f| f.contains("- check: failed (failure)")),
        "check fails it"
    );
    assert_eq!(ran, 1, "nothing is run again");
}

// A run again that judged nothing fails the commit, never holds it, said as the full workflows found it and as that run
// found it: on the commit, a run that uploaded no results, after which nothing is run on main; on main's head, a run
// whose job of the tests ended without judging the code, though the results it uploaded fail the test there as well.
// Well under a second alone each, a repository on disk and GitHub in memory; no other test holds a run again that
// judged nothing.
#[tokio::test]
async fn a_run_again_on_the_commit_or_on_mains_head_that_judged_nothing_fails_the_commit() {
    let hub = Hub::new();
    let (head, _) = hub.red_by_b().await;
    let jobs = vec![passed("lint"), passed("test")];
    hub.github().of_workflow("fast.yml", &head, jobs, None);
    let verdict = hub.verdict(&head).await.unwrap().expect("judged");
    assert_eq!(verdict.held, None, "{verdict:?}");
    let failed = verdict.failed.expect("failed");
    assert!(
        failed.starts_with(&format!("The full workflows ran on the commit {head}")),
        "{failed}"
    );
    let again = failed
        .split("The fast workflow ran what failed again on the commit")
        .nth(1)
        .expect("the run again is said");
    assert!(
        again.contains("The run uploaded no JUnit results the engine can read"),
        "{failed}"
    );
    assert_eq!(runs(&hub).len(), 2, "nothing is run on main");

    let hub = Hub::new();
    let (head, main) = hub.red_by_b().await;
    hub.fast_on(&head, false, Outcome::Passed);
    let broken = vec![check("test", Outcome::Broken("cancelled".into()))];
    hub.github()
        .of_workflow("fast.yml", &main, broken, Some(("test-results", b(false))));
    let verdict = hub.verdict(&head).await.unwrap().expect("judged");
    assert_eq!(verdict.held, None, "main's run judged nothing: {verdict:?}");
    let failed = verdict.failed.expect("failed");
    let on_main = failed
        .split(&format!("What failed again was run on the head of main, {main}:"))
        .nth(1)
        .expect("main's run is said");
    assert!(
        on_main.contains("- test: ended without judging the code (cancelled)"),
        "{failed}"
    );
    assert!(!failed.contains("These failed again on the commit"), "{failed}");
    assert_eq!(runs(&hub).len(), 3);
}

// A test the run again did not run could not be run again: the commit fails as the full workflows found it and as the
// run again found it, which says the test was not run, and never says it failed again; nothing is run on main,
// whatever main would make of it. Counted as failed again, a test run on neither read "Failed again on the commit, and
// not on the head of main", and one that fails on main's head held the work for main. Well under a second alone each
// way, a repository on disk and GitHub in memory; no other test holds a test the run again did not run.
#[tokio::test]
async fn a_test_the_run_again_did_not_run_fails_the_commit_and_is_never_said_to_have_failed_again() {
    for fails_on_main in [false, true] {
        let hub = Hub::new();
        let (head, main) = hub.red_by_b().await;
        // The run again holds no test of that name.
        let ran = || vec![passed("lint"), passed("test")];
        hub.github()
            .of_workflow("fast.yml", &head, ran(), Some(("test-results", fast::junit_green())));
        if fails_on_main {
            hub.fast_on(&main, false, Outcome::Passed);
        } else {
            hub.github()
                .of_workflow("fast.yml", &main, ran(), Some(("test-results", fast::junit_green())));
        }
        let verdict = hub.verdict(&head).await.unwrap().expect("judged");
        assert_eq!(verdict.held, None, "{verdict:?}");
        let failed = verdict.failed.expect("failed");
        let again = failed
            .split("The fast workflow ran what failed again on the commit")
            .nth(1)
            .expect("the run again is said");
        assert!(
            again.contains("- tools::tests::b: not run: no test in the run has this name"),
            "{failed}"
        );
        assert!(!failed.contains("These failed again on the commit"), "{failed}");
        assert_eq!(runs(&hub).len(), 2, "nothing is run on main");
    }
}
