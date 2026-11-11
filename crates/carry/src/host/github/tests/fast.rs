//! The agents' checks with the tests and checks they name, run by the project's fast CI: a workflow run by hand on
//! the work's branch, or on the branch of the code before the change, whose results say each test's own result.

use super::*;
use crate::host::Named;
use crate::settings::Fast;

/// The fast CI of the tests' project: `fast.yml`, which runs the lint by name, and reports in `test-results`.
fn fast() -> Fast {
    Fast {
        workflow: "fast.yml".to_string(),
        tests_input: "tests".to_string(),
        checks_input: "checks".to_string(),
        named_checks: vec!["lint".to_string()],
        results: "test-results".to_string(),
        separators: vec!["::".to_string(), " > ".to_string()],
    }
}

fn named(tests: &[&str], checks: &[&str]) -> Named {
    let all = |xs: &[&str]| xs.iter().map(|x| x.to_string()).collect();
    Named {
        tests: all(tests),
        checks: all(checks),
    }
}

/// The JUnit a run uploads: `tools::tests::a` passed, `tools::tests::b` failed, saying why.
fn junit() -> Vec<(String, Vec<u8>)> {
    let xml = r#"<testsuites><testsuite name="carry">
  <testcase name="tools::tests::a" time="0.12"/>
  <testcase name="tools::tests::b" time="0.3"><failure message="assertion failed">thread 'tools::tests::b' panicked at crates/carry/src/tools.rs:9:5:
assertion failed: said.is_empty()</failure></testcase>
</testsuite></testsuites>"#;
    vec![("junit.xml".to_string(), xml.as_bytes().to_vec())]
}

fn job(name: &str, outcome: Outcome, id: u64) -> Check {
    let mut job = check(name, outcome);
    job.job = Some(id);
    job
}

impl Hub {
    fn fast(self) -> Hub {
        self.with(|config| config.fast = Some(fast()))
    }

    async fn named_check(&self, named: &Named) -> Result<Option<String>, String> {
        let place = self.place();
        self.host.check(&branch(&place), named, &mut Stop::never()).await
    }

    /// The runs by hand GitHub was asked for.
    fn dispatched(&self) -> Vec<Asked> {
        (self.github().asked().into_iter())
            .filter(|a| matches!(a, Asked::Dispatch { .. }))
            .collect()
    }
}

#[tokio::test]
async fn the_tests_named_run_by_hand_on_the_works_branch_and_each_is_said_with_its_result_and_time() {
    let hub = Hub::new().fast();
    let head = hub.committed().await;
    let test = job("test", Outcome::Failed("failure".into()), 5);
    hub.github().fast(&head, vec![test], Some(("test-results", junit())));
    let asked = named(&["tools::tests::a", "b", "a_name_of_no_test"], &[]);
    let said = hub.named_check(&asked).await.unwrap().expect("nothing stopped it");
    assert!(
        said.starts_with(&format!(
            "The fast CI ran on the commit {head} (https://github.example/actions/runs/1000): 1 of the 3 tests named \
             passed."
        )),
        "{said}"
    );
    for line in [
        "The tests named:\n- tools::tests::a: passed in 0.12 s\n",
        "- b: failed in 0.3 s\n  What tools::tests::b said:\n```\nthread 'tools::tests::b' panicked at \
         crates/carry/src/tools.rs:9:5:\nassertion failed: said.is_empty()\n```",
        "- a_name_of_no_test: not run: no test the run reported is named so",
    ] {
        assert!(said.contains(line), "{line}\n\n{said}");
    }
    assert!(
        !said.contains("What its log says"),
        "a failed test says why itself: {said}"
    );
    assert_eq!(
        hub.dispatched(),
        [Asked::Dispatch {
            workflow: "fast.yml".to_string(),
            branch: hub.place().branch,
            inputs: vec![(
                "tests".to_string(),
                "tools::tests::a\nb\na_name_of_no_test\n".to_string()
            )],
        }]
    );
    assert_eq!(hub.pushed(), head);
    assert_eq!(hub.opened(), 1, "the work's pull request is opened as for any check");
    assert!(
        !hub.github().asked().iter().any(|a| matches!(a, Asked::Checks(_))),
        "the whole CI is not waited for"
    );
}

#[tokio::test]
async fn a_check_named_runs_beside_the_tests_and_a_red_one_says_what_its_log_says() {
    let hub = Hub::new().fast();
    let head = hub.committed().await;
    let lint = job("lint", Outcome::Failed("failure".into()), 9);
    hub.github().fast(&head, vec![lint], None);
    hub.github().log(
        9,
        "2026-11-11T10:00:00.0000000Z ##[group]Run cargo clippy --workspace --all-targets -- -D warnings\n\
         2026-11-11T10:00:00.1000000Z ##[endgroup]\n\
         2026-11-11T10:00:01.0000000Z error: unused variable: `x`\n\
         2026-11-11T10:00:02.0000000Z ##[error]Process completed with exit code 101.\n",
    );
    let said = hub.named_check(&named(&[], &["lint"])).await.unwrap().expect("checked");
    assert!(said.contains("): 0 of the 1 checks named passed."), "{said}");
    assert!(
        said.contains("The checks named:\n- lint: failed (failure) https://github.example/checks/lint"),
        "{said}"
    );
    assert!(
        said.ends_with(
            "What its log says of the failure, as the CI printed it:\n```\nRun cargo clippy --workspace --all-targets \
             -- -D warnings\nerror: unused variable: `x`\n##[error]Process completed with exit code 101.\n```"
        ),
        "{said}"
    );
    let inputs = match hub.dispatched().as_slice() {
        [Asked::Dispatch { inputs, .. }] => inputs.clone(),
        other => panic!("{other:?}"),
    };
    assert_eq!(
        inputs,
        [
            ("tests".to_string(), String::new()),
            ("checks".to_string(), "[\"lint\"]".to_string())
        ]
    );
}

#[tokio::test]
async fn a_check_the_fast_ci_does_not_run_by_name_is_refused_before_anything_is_pushed() {
    let hub = Hub::new().fast();
    hub.committed().await;
    let said = hub
        .named_check(&named(&["a"], &["clippy"]))
        .await
        .unwrap()
        .expect("said");
    assert_eq!(
        said,
        "Nothing was checked: the project's fast CI runs by name only lint, and not clippy."
    );
    assert!(hub.github().asked().is_empty(), "GitHub was asked nothing");
}

#[tokio::test]
async fn a_unit_that_did_not_compile_is_said_by_what_the_log_of_its_job_says() {
    let hub = Hub::new().fast();
    let head = hub.committed().await;
    hub.github()
        .fast(&head, vec![job("test", Outcome::Failed("failure".into()), 5)], None);
    hub.github().log(
        5,
        "2026-11-11T10:00:01.0000000Z error[E0425]: cannot find value `said` in this scope\n\
         2026-11-11T10:00:02.0000000Z ##[error]Process completed with exit code 101.\n",
    );
    let said = hub.named_check(&named(&["a"], &[])).await.unwrap().expect("checked");
    assert!(
        said.contains("- a: unread: the run's results could not be read: the run uploaded no artifact test-results"),
        "{said}"
    );
    assert!(
        said.ends_with(
            "test: failed (failure) https://github.example/checks/test\nWhat its log says of the failure, as the CI \
             printed it:\n```\nerror[E0425]: cannot find value `said` in this scope\n##[error]Process completed with \
             exit code 101.\n```"
        ),
        "{said}"
    );
}

#[tokio::test]
async fn with_nothing_named_the_check_is_the_whole_ci_of_its_pull_request() {
    let hub = Hub::new().fast();
    let head = hub.committed().await;
    hub.github().checks(&head, vec![vec![passed("lint"), passed("test")]]);
    let said = hub.named_check(&Named::default()).await.unwrap().expect("checked");
    assert!(said.starts_with("The CI passed on the commit"), "{said}");
    assert!(hub.dispatched().is_empty());
}

#[tokio::test]
async fn names_given_where_the_project_has_no_fast_ci_are_said_not_run_apart() {
    let hub = Hub::new();
    let head = hub.committed().await;
    hub.github().checks(&head, vec![vec![passed("lint"), passed("test")]]);
    let said = hub.named_check(&named(&["a"], &[])).await.unwrap().expect("checked");
    assert!(
        said.starts_with("This check runs whole: the tests and checks you named are not run apart.\n\nThe CI passed"),
        "{said}"
    );
}

#[tokio::test]
async fn a_branch_whose_workflow_does_not_take_the_request_is_run_with_mains_ci_files() {
    let hub = Hub::new().fast();
    hub.committed().await;
    // Main's workflow takes the request; the branch, made before it did, holds none.
    let other = hub.repo.data.join("other");
    run(
        &hub.repo.data,
        &["clone", "-q", &hub.github().bare.display().to_string(), "other"],
    );
    run(&other, &["config", "user.name", "Someone"]);
    run(&other, &["config", "user.email", "someone@example.com"]);
    std::fs::create_dir_all(other.join(".github/workflows")).unwrap();
    commit(&other, ".github/workflows/fast.yml", "name: fast\n");
    run(&other, &["push", "-q", "origin", "main"]);
    hub.host.main().await.expect("main is fetched");
    let place = hub.place();
    hub.github().refuse_dispatch_on(&place.branch);
    let said = hub.named_check(&named(&["a"], &[])).await.unwrap().expect("checked");
    assert!(said.contains("so it ran with main's: on the commit "), "{said}");
    let branches: Vec<String> = (hub.dispatched().into_iter())
        .filter_map(|a| match a {
            Asked::Dispatch { branch, .. } => Some(branch),
            _ => None,
        })
        .collect();
    assert_eq!(branches.len(), 2, "{branches:?}");
    assert_eq!(branches[0], place.branch);
    assert!(
        branches[1].starts_with(&format!("wf-agents-main-ci/{}-", place.branch)),
        "{branches:?}"
    );
    let reference = format!("refs/heads/{}", branches[1]);
    let left = std::process::Command::new("git")
        .arg("-C")
        .arg(&hub.github().bare)
        .args(["rev-parse", "--verify", "--quiet", &reference])
        .output()
        .unwrap();
    assert!(!left.status.success(), "its branch is taken away");
}

#[tokio::test]
async fn the_check_before_the_change_runs_the_tests_named_on_a_branch_of_its_own() {
    let hub = Hub::new().fast();
    hub.committed().await;
    let files = vec!["farewell.txt".to_string()];
    let place = hub.place();
    let before = git::before(&hub.repo.project, &place, &hub.host.kept_main(), &files)
        .await
        .unwrap()
        .unwrap();
    hub.github().fast(
        &before.commit,
        vec![job("test", Outcome::Failed("failure".into()), 5)],
        Some(("test-results", junit())),
    );
    let said = (hub.host)
        .check_before(&branch(&place), &before, &named(&["b"], &[]), &mut Stop::never())
        .await
        .unwrap()
        .expect("checked");
    assert!(
        said.starts_with(&format!("The fast CI ran on the commit {}", before.commit)),
        "{said}"
    );
    assert!(said.contains("- b: failed in 0.3 s"), "{said}");
    let name = format!("wf-agents-before/{}-{}", place.branch, &before.commit[..12]);
    assert_eq!(
        hub.dispatched(),
        [Asked::Dispatch {
            workflow: "fast.yml".to_string(),
            branch: name,
            inputs: vec![("tests".to_string(), "b\n".to_string())],
        }]
    );
    assert_eq!(hub.opened(), 0, "no pull request is opened for it");
    let refused = (hub.host)
        .check_before(&branch(&place), &before, &named(&["b"], &["lint"]), &mut Stop::never())
        .await
        .unwrap()
        .expect("said");
    assert!(
        refused.starts_with("Nothing was checked: the check before the change runs the tests"),
        "{refused}"
    );
}

// With a fast CI and no whole CI on the branches of the code before the change, a check before the change that names
// no test is told to name them, once, as its result; nothing is pushed or run.
#[tokio::test]
async fn a_check_before_the_change_naming_no_test_is_told_to_name_them() {
    let hub = Hub::new().fast().with(|config| config.before_branches = false);
    hub.committed().await;
    let files = vec!["farewell.txt".to_string()];
    let place = hub.place();
    let before = git::before(&hub.repo.project, &place, &hub.host.kept_main(), &files)
        .await
        .unwrap()
        .unwrap();
    let said = (hub.host)
        .check_before(&branch(&place), &before, &Named::default(), &mut Stop::never())
        .await
        .expect("a refusal the agent can mend is its result")
        .expect("said");
    assert_eq!(said, NO_TESTS_BEFORE);
    assert!(hub.github().asked().is_empty(), "GitHub was asked nothing");
}
