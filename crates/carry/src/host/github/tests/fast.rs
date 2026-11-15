//! The agents' `run_tests` on the GitHub host with the tests and checks they name, run by the project's fast workflow,
//! and with none named, run by its full workflows: workflows run by hand on main's file with the commit given, whose
//! JUnit results say each test's own result and time.

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

pub(super) fn job(name: &str, outcome: Outcome, _: u64) -> Check {
    check(name, outcome)
}

impl Hub {
    async fn named_check(&self, named: &Named) -> Result<Option<String>, String> {
        let place = self.place();
        self.host
            .run_tests(&branch(&place), named, self.call(), &mut Stop::never())
            .await
            .map_err(String::from)
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

/// The inputs of a run of a full workflow on the commit.
fn full_inputs(commit: &str) -> Vec<(String, String)> {
    vec![
        ("commit".to_string(), commit.to_string()),
        ("call".to_string(), "<call>".to_string()),
    ]
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
        "- a_name_of_no_test: not run: no test of the run has this name",
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
    assert_eq!(hub.pushed(), head);
    assert_eq!(hub.opened(), 1, "the work's pull request is opened as for any run");
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
        said.matches(": not run: no test of the run has this name").count(),
        3,
        "{said}"
    );
}

#[tokio::test]
async fn a_check_named_runs_beside_the_tests_and_a_red_one_is_said_failed_with_no_log_read() {
    let hub = Hub::new();
    let head = hub.committed().await;
    let lint = job("lint", Outcome::Failed("failure".into()), 9);
    hub.github().fast(&head, vec![lint], None);
    let said = hub.named_check(&named(&[], &["lint"])).await.unwrap().expect("run");
    assert!(said.contains("): 0 of the 1 checks passed."), "{said}");
    assert!(
        said.ends_with("The jobs:\n- lint: failed (failure) in 60 s https://github.example/checks/lint"),
        "{said}"
    );
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
        "Nothing was run: the fast workflow runs by name only these checks: lint; not clippy."
    );
    assert!(hub.github().asked().is_empty(), "GitHub was asked nothing");
}

#[tokio::test]
async fn a_run_that_uploads_no_junit_is_said_so_and_its_log_is_not_read() {
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
        said.ends_with("The jobs:\n- test: failed (failure) in 60 s https://github.example/checks/test"),
        "a job that failed with nothing to say why is said failed: {said}"
    );
}

#[tokio::test]
async fn with_nothing_named_the_full_workflows_run_on_the_last_commit_and_each_test_and_check_is_said() {
    let hub = Hub::new();
    let head = hub.committed().await;
    let lint = Check {
        took: Some(192),
        ..passed("lint")
    };
    let jobs = vec![lint, job("test", Outcome::Failed("failure".into()), 5)];
    hub.github().fast(&head, jobs, Some(("test-results", junit())));
    let said = hub.named_check(&Named::default()).await.unwrap().expect("run");
    assert!(
        said.starts_with(&format!(
            "The full workflows ran on the commit {head} (https://github.example/actions/runs/1000): 1 of the 2 tests \
             passed; 1 of the 2 checks passed."
        )),
        "{said}"
    );
    // What did not pass comes first, each with its time.
    let b = said.find("- tools::tests::b: failed in 0.3 s").expect("b");
    let a = said.find("- tools::tests::a: passed in 0.12 s").expect("a");
    assert!(b < a, "{said}");
    assert!(said.contains("assertion failed: said.is_empty()"), "{said}");
    // The jobs come before what did not pass, and that before what passed, which a result cut short loses first.
    let jobs = "The jobs:\n- lint: passed in 192 s https://github.example/checks/lint\n- test: failed (failure) in 60 \
                s https://github.example/checks/test";
    let at = said.find(jobs).expect("the jobs");
    assert!(at < b && b < a, "{said}");
    assert!(
        said.ends_with("The tests that passed:\n- tools::tests::a: passed in 0.12 s"),
        "{said}"
    );
    assert_eq!(
        hub.dispatched_as_said(),
        [Asked::Dispatch {
            workflow: "ci.yml".to_string(),
            branch: "main".to_string(),
            inputs: full_inputs(&head),
        }]
    );
}

// Each job, a line, comes before every test and what a failed one said, however long: a result cut at the bound a round
// is given keeps every job, how it ended and its time, and what the cut takes away is tests. Well under a second
// alone, a bare repository for GitHub; no other test holds the jobs to the bound.
#[tokio::test]
async fn every_job_comes_before_what_the_tests_said_and_stays_within_the_bound_a_round_is_given() {
    let hub = Hub::new();
    let head = hub.committed().await;
    let bound = crate::acts::words().run_tests.given.bound;
    let long = "x".repeat(bound + 1000);
    let xml = format!(
        r#"<testsuites><testsuite name="carry"><testcase name="tools::tests::a" time="0.12"/><testcase name="tools::tests::b" time="0.3"><failure message="assertion failed">{long}</failure></testcase></testsuite></testsuites>"#
    );
    let lint = Check {
        took: Some(192),
        ..passed("lint")
    };
    let jobs = vec![lint, job("test", Outcome::Failed("failure".into()), 5)];
    let results = vec![("test.xml".to_string(), xml.into_bytes())];
    hub.github().fast(&head, jobs, Some(("test-results", results)));
    let said = hub.named_check(&Named::default()).await.unwrap().expect("run");
    assert!(said.len() > bound, "the result is longer than a round is given");
    let jobs = "The jobs:\n- lint: passed in 192 s https://github.example/checks/lint\n- test: failed (failure) in 60 \
                s https://github.example/checks/test";
    let at = said.find(jobs).expect("the jobs");
    assert!(at + jobs.len() <= bound, "the jobs end at byte {}", at + jobs.len());
    assert!(
        at < said.find("- tools::tests::b: failed").expect("b"),
        "{}",
        &said[..at]
    );
}

// A job says how it ended and, where it ran to a conclusion, its wall time; one that did not run, did not finish or was
// cancelled says so in place of a time, and a job that did not run is no check that came to anything.
#[tokio::test]
async fn each_job_is_said_with_how_it_ended_and_its_wall_time_or_why_it_has_none() {
    let hub = Hub::new();
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
    let said = hub.named_check(&Named::default()).await.unwrap().expect("run");
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
async fn names_given_where_the_project_has_no_fast_workflow_are_refused_and_so_is_none_where_it_has_no_full_ones() {
    let hub = Hub::new().with(|config| {
        let ci = config.ci.as_mut().expect("a declaration");
        ci.fast = None;
    });
    hub.committed().await;
    let said = hub.named_check(&named(&["a"], &[])).await.unwrap().expect("said");
    assert!(said.starts_with("Nothing was run: tests or checks are named, and the project declares no fast workflow"));
    let hub = Hub::new().with(|config| {
        let ci = config.ci.as_mut().expect("a declaration");
        ci.full = None;
    });
    hub.committed().await;
    let said = hub.named_check(&Named::default()).await.unwrap().expect("said");
    assert_eq!(
        said,
        "Nothing was run: the project declares no full workflows; name the tests to run."
    );
    assert!(hub.github().asked().is_empty(), "nothing was pushed or asked");
}

#[tokio::test]
async fn a_workflow_that_runs_where_the_installation_has_no_runner_is_not_run() {
    let hub = Hub::new().with(|config| {
        let ci = config.ci.as_mut().expect("a declaration");
        ci.fast.as_mut().expect("fast").runner = "14900K".to_string();
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
// by main's file, with the commit to test given, and nothing but the work's commit is pushed for it.
#[tokio::test]
async fn the_workflow_is_run_by_mains_file_with_the_commit_given_and_nothing_else_is_pushed() {
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
    let mut expected = vec!["main".to_string(), hub.place().branch];
    expected.sort();
    let mut branches: Vec<String> = branches.lines().map(str::to_string).collect();
    branches.sort();
    assert_eq!(branches, expected, "only the work's commit is pushed");
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

/// The commit each run by hand was asked for on.
fn dispatched_on(hub: &Hub) -> Vec<String> {
    (hub.dispatched().into_iter())
        .filter_map(|asked| match asked {
            Asked::Dispatch { inputs, .. } => inputs.into_iter().find(|(name, _)| name == "commit").map(|(_, c)| c),
            _ => None,
        })
        .collect()
}

// An entry that names a commit of the branch runs that commit, whatever the worktree holds, and opens no pull request:
// a run on a commit needs none, and GitHub opens none for a branch with nothing of its own, as a work's in research.
// One that names none is held to the worktree as before. Under a second alone, a bare repository for GitHub; no other
// test holds a named commit to its run on GitHub.
#[tokio::test]
async fn a_commit_named_is_run_whatever_the_worktree_holds_and_opens_no_pull_request() {
    let hub = Hub::new();
    let place = hub.place();
    crate::acts::start(&hub.repo.project, &hub.host, &branch(&place), &Mutex::new(()))
        .await
        .expect("the work starts");
    // The branch holds nothing of its own, and the worktree has a change not committed.
    let main = run(&place.dir, &["rev-parse", "HEAD"]);
    std::fs::write(place.dir.join("draft.txt"), "Not committed.\n").unwrap();
    let refused = hub.named_check(&named(&["a"], &[])).await.unwrap().expect("said");
    assert!(refused.contains("not committed"), "{refused}");
    assert_eq!(dispatched_on(&hub), Vec::<String>::new(), "nothing is run");
    hub.github()
        .fast(&main, vec![passed("test")], Some(("test-results", junit_green())));
    let mut asked = named(&["a"], &[]);
    asked.commit = Some(main.clone());
    let said = hub.named_check(&asked).await.unwrap().expect("run");
    assert!(
        said.starts_with(&format!("The fast workflow ran on the commit {main} (")),
        "{said}"
    );
    assert_eq!(dispatched_on(&hub), [main], "the commit named is run");
    assert_eq!(hub.opened(), 0, "no pull request is opened");
}

// A newer entry of a cell has the runs of the one it replaced cancelled on the commit that one named, though that
// commit is none of the branch's own since main, as the point the branch left main is. Well under a second alone, a
// bare repository for GitHub; no other test holds a cancel to the commit an entry named.
#[tokio::test]
async fn the_runs_of_a_replaced_entry_are_cancelled_on_the_commit_it_named() {
    let hub = Hub::new();
    let fork = run(&hub.repo.project.repo, &["rev-parse", "main"]);
    hub.committed().await;
    hub.github().runs_go_on();
    let replaced = Asking::Entry(decide::Seq(1));
    let request = super::super::fast::Request {
        work: WORK,
        commit: &fork,
        asking: replaced,
    };
    let call = hub.host.call(request, "fast.yml");
    let inputs = [("commit".to_string(), fork.clone()), ("call".to_string(), call)];
    super::super::api::Api::dispatch(hub.github(), "fast.yml", "main", &inputs)
        .await
        .unwrap();
    let place = hub.place();
    hub.host
        .cancel_tests(&branch(&place), &[(replaced, Some(fork.clone()))])
        .await
        .expect("cancelled");
    assert!(
        hub.github().asked().contains(&Asked::Cancel(1000)),
        "the run on the commit named is cancelled: {:?}",
        hub.github().asked()
    );
}

// The runs of several replaced entries, each named or not, are found in one list of each workflow's runs, and a run
// whose cancel GitHub refuses leaves the others cancelled all the same, the refusal said. Well under a second alone, a
// bare repository for GitHub; no other test holds a cancel to more than one entry.
#[tokio::test]
async fn the_runs_of_several_replaced_entries_are_listed_once_and_each_cancelled_whatever_one_refuses() {
    let hub = Hub::new();
    let fork = run(&hub.repo.project.repo, &["rev-parse", "main"]);
    let head = hub.committed().await;
    hub.github().runs_go_on();
    let replaced = [
        (Asking::Entry(decide::Seq(1)), Some(fork.clone())),
        (Asking::Entry(decide::Seq(2)), Some(head.clone())),
        (Asking::Entry(decide::Seq(3)), None),
    ];
    for (asking, on) in &replaced {
        let commit = on.as_deref().unwrap_or(&head);
        let request = super::super::fast::Request {
            work: WORK,
            commit,
            asking: *asking,
        };
        let call = hub.host.call(request, "fast.yml");
        let inputs = [("commit".to_string(), commit.to_string()), ("call".to_string(), call)];
        super::super::api::Api::dispatch(hub.github(), "fast.yml", "main", &inputs)
            .await
            .unwrap();
    }
    hub.github()
        .refuse_cancel(1000, "HTTP 409: Cannot cancel a workflow run that is completed");
    let place = hub.place();
    let refused = hub
        .host
        .cancel_tests(&branch(&place), &replaced)
        .await
        .expect_err("one cancel was refused");
    assert!(refused.why.contains("HTTP 409: Cannot cancel"), "{refused:?}");
    let asked = hub.github().asked();
    for run in [1000, 1001, 1002] {
        assert!(asked.contains(&Asked::Cancel(run)), "{run}: {asked:?}");
    }
    assert_eq!(hub.github().listed(), 2, "one list of each workflow's runs");
}
