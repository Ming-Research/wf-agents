//! The GitHub host against a fake of GitHub: a bare repository on disk and pull requests and checks in memory.
//!
//! - `ends`: the pull request's text and draft, the checks that hold no work, main red, and the end of a work.

mod ends;

use std::time::Duration;

use store::WorkNumber;
use tokio::sync::Mutex;

use super::api::{Check, Outcome};
use super::fake::{Asked, Fake};
use super::*;
use crate::git::Place;
use crate::git::tests::{Repo, commit, run};
use crate::held::Held;

mod agents;
mod fast;

const WORK: WorkNumber = WorkNumber(1);

/// A local repository, its bare repository on GitHub holding its main branch, and the GitHub host over both.
struct Hub {
    repo: Repo,
    host: GitHub<Fake>,
}

/// The words of `run_tests`, as this repository's file beside the flows gives them.
fn tool() -> flow::agents::Tool {
    let text = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../flow/agents/tools/run_tests.toml"
    ));
    toml::from_str(text).expect("the file of run_tests reads")
}

/// The project's two workflows as the tests' project declares them: `fast.yml`, which runs the lint by name, and
/// `ci.yml`, the full one, both reporting in `test-results`.
fn ci() -> flow::agents::Ci {
    use flow::agents::{Ci, CiFast, CiFull};
    Ci {
        fast: Some(CiFast {
            workflow: "fast.yml".to_string(),
            runner: "github".to_string(),
            commit_input: "commit".to_string(),
            call_input: "call".to_string(),
            tests_input: "tests".to_string(),
            checks_input: "checks".to_string(),
            checks: vec!["lint".to_string()],
            results: "test-results".to_string(),
        }),
        full: Some(CiFull {
            workflows: vec!["ci.yml".to_string()],
            runner: "github".to_string(),
            too_long: "1m".to_string(),
            commit_input: "commit".to_string(),
            call_input: "call".to_string(),
            results: "test-results".to_string(),
        }),
    }
}

/// How the tests' host judges the CI: the checks `lint` and `test`, and short waits.
fn config(bare: &std::path::Path) -> Config {
    Config {
        url: bare.display().to_string(),
        remote: "origin".to_string(),
        start: Duration::from_millis(300),
        timeout: Duration::from_secs(2),
        merge_method: "squash".to_string(),
        delete_branch: false,
        ci: Some(ci()),
        runners: vec!["github".to_string()],
        tool: Some(tool()),
        fast_poll: Duration::from_millis(10),
        token: None,
        withheld: Vec::new(),
    }
}

impl Hub {
    fn new() -> Hub {
        let repo = Repo::new("false");
        let bare = repo.data.join("github.git");
        run(&repo.data, &["init", "-q", "--bare", "-b", "main", "github.git"]);
        run(&repo.project.repo, &["push", "-q", &bare.display().to_string(), "main"]);
        let mut config = config(&bare);
        config.withheld = withheld(&repo.data, &repo.project.repo, None);
        let host = GitHub::new(Fake::new(bare.clone()), repo.project.clone(), config);
        Hub { repo, host }
    }

    fn with(mut self, change: impl FnOnce(&mut Config)) -> Hub {
        change(&mut self.host.config);
        self
    }

    fn place(&self) -> Place {
        self.repo.place(WORK)
    }

    fn github(&self) -> &Fake {
        &self.host.api
    }

    /// Starts the work and commits a file on its branch, giving the commit.
    async fn committed(&self) -> String {
        let place = self.place();
        let started = crate::acts::start(&self.repo.project, &self.host, &branch(&place), &Mutex::new(()))
            .await
            .expect("the work starts");
        commit(&started.place.dir, "farewell.txt", "Bye.\n")
    }

    async fn verdict(&self, commit: &str) -> Result<Option<Verdict>, String> {
        let place = self.place();
        self.host.verdict(&branch(&place), commit, &mut Stop::never()).await
    }

    /// Merges the commit the check passed; a refusal but for a moved main in the host's words.
    async fn merge(&self, checked: &str) -> Result<String, String> {
        self.merging(checked).await.map_err(|not| match not {
            NotMerged::Failed(why) => why,
            moved => panic!("refused otherwise: {moved:?}"),
        })
    }

    /// Merges the commit the check passed.
    async fn merging(&self, checked: &str) -> Result<String, NotMerged> {
        let place = self.place();
        self.host.merge(&branch(&place), Some(checked)).await
    }

    /// The commit the branch is at on GitHub.
    fn pushed(&self) -> String {
        let reference = self.place().reference();
        run(&self.github().bare, &["rev-parse", &reference])
    }

    /// The commit's full workflows pass: every job green, and the one test it reports passed.
    fn green(&self, commit: &str) {
        let jobs = vec![passed("lint"), passed("test")];
        self.github()
            .fast(commit, jobs, Some(("test-results", fast::junit_green())));
    }

    /// The work's head, checked once, its pull request open.
    async fn pushed_and_checked(&self) -> String {
        let head = run(&self.place().dir, &["rev-parse", "HEAD"]);
        self.green(&head);
        self.verdict(&head).await.unwrap().unwrap();
        head
    }

    /// The commit, checked once, its pull request open.
    async fn pushed_and_checked_after(&self, commit: String) -> String {
        self.green(&commit);
        self.verdict(&commit).await.unwrap().unwrap();
        commit
    }

    /// How many pull requests were opened.
    fn opened(&self) -> usize {
        (self.github().asked().iter())
            .filter(|a| matches!(a, Asked::Open { .. }))
            .count()
    }
}

fn branch(place: &Place) -> Branch<'_> {
    Branch {
        work: WORK,
        title: "Greet the user",
        about: "Greets the user by name.",
        place,
    }
}

/// A check, at its own link; one that ran to a conclusion took a minute, as GitHub's job would say, and any other has
/// no time.
fn check(name: &str, outcome: Outcome) -> Check {
    let took = matches!(outcome, Outcome::Passed | Outcome::Failed(_)).then_some(60);
    Check {
        name: name.to_string(),
        outcome,
        url: Some(format!("https://github.example/checks/{name}")),
        took,
    }
}

fn passed(name: &str) -> Check {
    check(name, Outcome::Passed)
}

#[tokio::test]
async fn main_is_fetched_from_github() {
    let hub = Hub::new();
    let local = run(&hub.repo.project.repo, &["rev-parse", "main"]);
    // Main moves on GitHub, and not in the local repository.
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
    let tracked = run(&hub.repo.project.repo, &["rev-parse", "refs/remotes/origin/main"]);
    assert_eq!(tracked, moved);
    assert_eq!(
        run(&hub.repo.project.repo, &["rev-parse", "main"]),
        local,
        "the local main is let be"
    );
}

#[tokio::test]
async fn a_repository_whose_configuration_sends_connections_elsewhere_is_not_pushed_from() {
    for (key, value) in [
        ("url.https://elsewhere.example/.insteadOf", "https://github.com/"),
        ("http.proxy", "x"),
    ] {
        let hub = Hub::new();
        run(&hub.repo.project.repo, &["config", key, value]);
        let refused = hub.host.main().await.expect_err(key);
        assert!(refused.contains(&key.to_ascii_lowercase()), "{key}: {refused}");
    }
}

#[tokio::test]
async fn keys_of_the_clone_for_githubs_address_itself_are_refused() {
    for name in ["url", "pushurl", "receivepack", "uploadpack", "vcs", "proxy"] {
        let hub = Hub::new();
        let key = format!("remote.{}.{name}", hub.host.config.url);
        run(&hub.repo.project.repo, &["config", &key, "elsewhere"]);
        let refused = hub.host.main().await.expect_err(&key);
        assert!(refused.contains(&format!(".{name} (local)")), "{key}: {refused}");
    }
}

#[tokio::test]
async fn what_an_included_file_of_the_clone_says_of_connections_is_refused() {
    let hub = Hub::new();
    let included = hub.repo.data.join("included.config");
    std::fs::write(
        &included,
        "[url \"https://elsewhere.example/\"]\n\tinsteadOf = https://github.com/\n",
    )
    .unwrap();
    run(
        &hub.repo.project.repo,
        &["config", "include.path", &included.display().to_string()],
    );
    let refused = hub.host.main().await.expect_err("refused");
    assert!(
        refused.contains("url.https://elsewhere.example/.insteadof"),
        "{refused}"
    );
}

#[tokio::test]
async fn the_check_pushes_the_head_opens_a_pull_request_into_main_and_passes_once_the_full_workflows_have() {
    let hub = Hub::new();
    let head = hub.committed().await;
    hub.green(&head);
    let verdict = hub.verdict(&head).await.unwrap().expect("nothing stopped it");
    assert_eq!(
        verdict,
        Verdict {
            failed: None,
            by: Some("the full workflows of the commit (https://github.example/actions/runs/1000)".to_string()),
            notes: vec![
                "The jobs of the full workflows, each with how it ended and its wall time:\n  \
                 - lint: passed in 60 s https://github.example/checks/lint\n  \
                 - test: passed in 60 s https://github.example/checks/test"
                    .to_string()
            ],
        }
    );
    assert_eq!(hub.pushed(), head);
    let opened: Vec<Asked> = (hub.github().asked().into_iter())
        .filter(|a| matches!(a, Asked::Open { .. }))
        .collect();
    assert_eq!(
        opened,
        [Asked::Open {
            branch: hub.place().branch,
            base: "main".to_string(),
            title: "Greet the user".to_string(),
        }]
    );
}

#[tokio::test]
async fn a_check_given_again_pushes_nothing_new_and_opens_no_second_pull_request() {
    let hub = Hub::new();
    let head = hub.committed().await;
    hub.green(&head);
    hub.verdict(&head).await.unwrap().unwrap();
    // As after the engine started again in the middle of the check.
    let again = hub.verdict(&head).await.unwrap().unwrap();
    assert_eq!(again.failed, None);
    assert_eq!(hub.opened(), 1);
    assert_eq!(hub.pushed(), head);
}

#[tokio::test]
async fn the_full_workflows_are_run_once_on_the_commit_checked_by_mains_file() {
    let hub = Hub::new();
    let head = hub.committed().await;
    hub.green(&head);
    hub.verdict(&head).await.unwrap().unwrap();
    let dispatched: Vec<Asked> = (hub.github().asked().into_iter())
        .filter(|a| matches!(a, Asked::Dispatch { .. }))
        .collect();
    let [
        Asked::Dispatch {
            workflow,
            branch,
            inputs,
        },
    ] = &dispatched[..]
    else {
        panic!("{dispatched:?}");
    };
    assert_eq!((workflow.as_str(), branch.as_str()), ("ci.yml", "main"));
    assert_eq!(inputs[0], ("commit".to_string(), head));
}

#[tokio::test]
async fn a_pull_request_into_another_branch_fails_the_round() {
    let hub = Hub::new();
    let head = hub.committed().await;
    hub.green(&head);
    hub.verdict(&head).await.unwrap().unwrap();
    hub.github().set_base(1, "elsewhere");
    let failed = hub.verdict(&head).await.expect_err("the check fails");
    assert!(failed.contains("goes into elsewhere, not into main"), "{failed}");
    let failed = hub.merge(&head).await.expect_err("the merge fails");
    assert!(failed.contains("goes into elsewhere, not into main"), "{failed}");
}

#[tokio::test]
async fn a_pull_request_merged_into_another_branch_is_not_taken_for_the_works_merge() {
    let hub = Hub::new();
    let checked = hub.committed().await;
    hub.green(&checked);
    hub.verdict(&checked).await.unwrap().unwrap();
    hub.github().set_base(1, "elsewhere");
    hub.github().merged_by_a_person(1, &checked);
    let failed = hub.merge(&checked).await.expect_err("nothing is on main");
    assert!(failed.contains("has no open pull request"), "{failed}");
}

#[tokio::test]
async fn no_fetch_or_push_of_the_engine_goes_into_a_submodule() {
    let hub = Hub::new();
    let repo = &hub.repo.project.repo;
    // A library of the project's, a submodule of its clone, on main on GitHub.
    let lib = hub.repo.data.join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    run(&lib, &["init", "-q", "-b", "main"]);
    run(&lib, &["config", "user.name", "Tester"]);
    run(&lib, &["config", "user.email", "tester@example.com"]);
    commit(&lib, "lib.txt", "A library.\n");
    let lib = lib.display().to_string();
    run(
        repo,
        &[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            "-q",
            &lib,
            "lib",
        ],
    );
    run(repo, &["commit", "-q", "-m", "Add the library"]);
    run(repo, &["push", "-q", &hub.github().bare.display().to_string(), "main"]);
    let head = hub.committed().await;
    // The submodule's own configuration, which none of the engine's checks read, names a program for its transfers,
    // and the clone's says to go into submodules on every fetch and push.
    let marked = hub.repo.data.join("submodule-ran");
    let marker = hub.repo.data.join("marker");
    std::fs::write(&marker, format!("#!/bin/sh\ntouch '{}'\nexit 1\n", marked.display())).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&marker, std::fs::Permissions::from_mode(0o755)).unwrap();
    let submodule = repo.join("lib");
    let marker = marker.display().to_string();
    for (key, value) in [
        ("remote.origin.uploadpack", marker.as_str()),
        ("remote.origin.receivepack", marker.as_str()),
        ("protocol.file.allow", "always"),
    ] {
        run(&submodule, &["config", key, value]);
    }
    for (key, value) in [
        ("submodule.recurse", "true"),
        ("fetch.recurseSubmodules", "yes"),
        ("push.recurseSubmodules", "on-demand"),
    ] {
        run(repo, &["config", key, value]);
    }
    hub.green(&head);
    hub.verdict(&head).await.unwrap().expect("checked");
    hub.host.main().await.expect("main is fetched");
    assert!(!marked.exists(), "a program of the submodule's was run");
}

#[tokio::test]
async fn a_pull_request_a_person_merged_at_another_head_is_not_taken_for_the_works_merge() {
    let hub = Hub::new();
    let checked = hub.committed().await;
    hub.green(&checked);
    hub.verdict(&checked).await.unwrap().unwrap();
    hub.github().merged_by_a_person(1, "c0ffee");
    let failed = hub.merge(&checked).await.expect_err("refused");
    assert!(failed.contains("was merged at c0ffee, not at"), "{failed}");
}

#[tokio::test]
async fn a_red_test_and_a_red_job_hold_the_work_and_are_said_with_what_the_test_said_and_no_log_read() {
    let hub = Hub::new();
    let head = hub.committed().await;
    let jobs = vec![passed("lint"), check("test", Outcome::Failed("failure".into()))];
    hub.github().fast(&head, jobs, Some(("test-results", fast::junit())));
    let verdict = hub.verdict(&head).await.unwrap().unwrap();
    let failed = verdict.failed.expect("it failed");
    // The job that did not pass is said where it failed, and the job that passed in the note: no job is told twice.
    assert_eq!(
        verdict.notes,
        [
            "The jobs of the full workflows, each with how it ended and its wall time:\n  \
             - lint: passed in 60 s https://github.example/checks/lint"
        ]
    );
    assert!(!failed.contains("- lint:"), "{failed}");
    assert!(
        failed.starts_with(&format!(
            "The full workflows ran on the commit {head} (https://github.example/actions/runs/1000): 1 of the 2 tests \
             passed; 1 of the 2 checks passed."
        )),
        "{failed}"
    );
    assert!(
        failed.contains("- tools::tests::b: failed in 0.3 s\n  What it said:\n```\nthread 'tools::tests::b' panicked"),
        "{failed}"
    );
    assert!(!failed.contains("tools::tests::a"), "what passed is left out: {failed}");
    assert!(
        failed.ends_with("The jobs:\n- test: failed (failure) in 60 s https://github.example/checks/test"),
        "{failed}"
    );
}

#[tokio::test]
async fn a_skipped_job_fails_nothing_and_every_job_of_a_passing_check_is_given_with_its_time() {
    let hub = Hub::new();
    let head = hub.committed().await;
    let jobs = vec![
        Check {
            took: Some(192),
            ..passed("page")
        },
        check("docs", Outcome::Skipped),
        Check {
            took: None,
            ..passed("lint")
        },
    ];
    hub.github()
        .fast(&head, jobs, Some(("test-results", fast::junit_green())));
    let verdict = hub.verdict(&head).await.unwrap().unwrap();
    assert_eq!(verdict.failed, None);
    assert_eq!(
        verdict.notes,
        [
            "The jobs of the full workflows, each with how it ended and its wall time:\n  \
             - page: passed in 192 s https://github.example/checks/page\n  \
             - docs: did not run https://github.example/checks/docs\n  \
             - lint: passed (GitHub gives no time for it) https://github.example/checks/lint"
        ]
    );
}

#[tokio::test]
async fn a_run_that_uploads_no_junit_holds_the_work_and_a_job_that_ended_without_judging_does_too() {
    let hub = Hub::new();
    let head = hub.committed().await;
    hub.github().fast(&head, vec![passed("lint")], None);
    let failed = hub.verdict(&head).await.unwrap().unwrap().failed.expect("it failed");
    assert!(
        failed.contains("The run uploaded no JUnit results the engine can read"),
        "{failed}"
    );
    let hub = Hub::new();
    let head = hub.committed().await;
    let jobs = vec![check("test", Outcome::Broken("cancelled".into()))];
    hub.github()
        .fast(&head, jobs, Some(("test-results", fast::junit_green())));
    let failed = hub.verdict(&head).await.unwrap().unwrap().failed.expect("it failed");
    assert!(
        failed.contains("- test: ended without judging the code (cancelled) https://github.example/checks/test"),
        "a job that did not finish says so in place of a time: {failed}"
    );
}

#[tokio::test]
async fn a_full_run_no_runner_took_fails_the_round_and_a_project_with_no_full_workflows_has_no_check() {
    let hub = Hub::new();
    let head = hub.committed().await;
    hub.github().no_runner();
    let failed = hub.verdict(&head).await.expect_err("the round fails");
    assert!(
        failed.starts_with(&format!(
            "the run of ci.yml asked for on {head} never started: no runner took it after"
        )),
        "{failed}"
    );
    let hub = Hub::new().with(|config| config.ci.as_mut().expect("a declaration").full = None);
    let head = hub.committed().await;
    let failed = hub.verdict(&head).await.expect_err("the round fails");
    assert_eq!(
        failed,
        "Nothing was run: the project declares no full workflows; name the tests to run."
    );
}

#[tokio::test]
async fn a_push_github_refuses_fails_the_round_and_overwrites_nothing() {
    let hub = Hub::new();
    let head = hub.committed().await;
    // Someone else's commit on the work's branch on GitHub.
    let other = hub.repo.data.join("other");
    let bare = hub.github().bare.display().to_string();
    run(&hub.repo.data, &["clone", "-q", &bare, "other"]);
    run(&other, &["config", "user.name", "Someone"]);
    run(&other, &["config", "user.email", "someone@example.com"]);
    let theirs = commit(&other, "theirs.txt", "Theirs.\n");
    run(
        &other,
        &["push", "-q", "origin", &format!("HEAD:{}", hub.place().reference())],
    );
    let failed = hub.verdict(&head).await.expect_err("the round fails");
    assert!(
        failed.starts_with(&format!("the push of {head} to the branch")),
        "{failed}"
    );
    assert!(failed.contains("refused"), "{failed}");
    assert_eq!(hub.pushed(), theirs);
    assert_eq!(hub.opened(), 0);
}

#[tokio::test]
async fn a_pull_request_closed_on_github_fails_the_round() {
    let hub = Hub::new();
    let head = hub.committed().await;
    hub.green(&head);
    hub.verdict(&head).await.unwrap().unwrap();
    hub.github().close(1);
    let failed = hub.verdict(&head).await.expect_err("the round fails");
    assert!(
        failed.contains("https://github.example/pull/1") && failed.contains("was closed"),
        "{failed}"
    );
    assert_eq!(hub.opened(), 1, "no second pull request is opened");
}

#[tokio::test]
async fn a_stopped_wait_gives_nothing() {
    let hub = Hub::new();
    let head = hub.committed().await;
    hub.github().no_runner();
    let held: Held<u8, ()> = Held::new();
    let mut stop = held.hold(1, ()).expect("held");
    let place = hub.place();
    let stopping = async {
        tokio::time::sleep(Duration::from_millis(100)).await;
        held.stop(1, ());
    };
    let branch = branch(&place);
    let (verdict, ()) = tokio::join!(hub.host.verdict(&branch, &head, &mut stop), stopping);
    assert_eq!(verdict, Ok(None));
}

#[tokio::test]
async fn the_merge_merges_the_pull_request_only_at_the_commit_the_check_passed_and_once() {
    let hub = Hub::new();
    let checked = hub.committed().await;
    hub.green(&checked);
    hub.verdict(&checked).await.unwrap().unwrap();
    // The branch on GitHub moved on from what was checked.
    let later = commit(&hub.place().dir, "later.txt", "Later.\n");
    let target = format!("{later}:{}", hub.place().reference());
    run(
        &hub.repo.project.repo,
        &["push", "-q", &hub.github().bare.display().to_string(), &target],
    );
    let refused = hub.merge(&checked).await.expect_err("refused");
    assert!(
        refused.contains("not at") && refused.contains("checked again"),
        "{refused}"
    );
    let merges = |hub: &Hub| {
        (hub.github().asked().into_iter())
            .filter(|a| matches!(a, Asked::Merge { .. }))
            .collect::<Vec<_>>()
    };
    assert!(merges(&hub).is_empty(), "nothing was merged");
    // At what was checked, it merges, by the method set; merged already, it is not merged again.
    let merged = hub.merge(&later).await.expect("it merges");
    assert_eq!(
        merged,
        run(&hub.github().bare, &["rev-parse", "main"]),
        "main is the squash"
    );
    assert_eq!(hub.merge(&later).await, Ok(merged));
    assert_eq!(
        merges(&hub),
        [Asked::Merge {
            number: 1,
            head: later,
            method: "squash".to_string(),
        }]
    );
    assert!(
        !hub.github().asked().iter().any(|a| matches!(a, Asked::Delete(_))),
        "no branch is deleted unless the settings say so"
    );
}

#[tokio::test]
async fn a_merge_given_again_through_the_acts_finds_the_squash_github_made() {
    let hub = Hub::new();
    let checked = hub.committed().await;
    hub.green(&checked);
    hub.verdict(&checked).await.unwrap().unwrap();
    let place = hub.place();
    let lock = Mutex::new(());
    let merged = crate::acts::merge(&hub.host, &branch(&place), Some(&checked), &lock)
        .await
        .expect("it merges");
    // As when the engine stopped after GitHub merged and before the merge was recorded: main is now the squash,
    // which the checked commit does not hold.
    let again = crate::acts::merge(&hub.host, &branch(&place), Some(&checked), &lock).await;
    assert_eq!(again, Ok(merged));
}

#[tokio::test]
async fn the_merge_refuses_a_main_that_moved_on_github_since_the_check() {
    let hub = Hub::new();
    let checked = hub.committed().await;
    hub.green(&checked);
    hub.verdict(&checked).await.unwrap().unwrap();
    let other = hub.repo.data.join("other");
    run(
        &hub.repo.data,
        &["clone", "-q", &hub.github().bare.display().to_string(), "other"],
    );
    run(&other, &["config", "user.name", "Someone"]);
    run(&other, &["config", "user.email", "someone@example.com"]);
    let moved = commit(&other, "weather.txt", "Sunny.\n");
    run(&other, &["push", "-q", "origin", "main"]);
    let refused = hub.merging(&checked).await.expect_err("refused");
    assert_eq!(refused, NotMerged::Moved { main: moved, checked });
    assert!(
        !hub.github().asked().iter().any(|a| matches!(a, Asked::Merge { .. })),
        "nothing was merged"
    );
}

#[tokio::test]
async fn a_merge_github_refuses_fails_the_round_in_githubs_words() {
    let hub = Hub::new();
    let checked = hub.committed().await;
    hub.green(&checked);
    hub.verdict(&checked).await.unwrap().unwrap();
    hub.github()
        .refuse_merges("HTTP 405: At least 1 approving review is required");
    let refused = hub.merge(&checked).await.expect_err("refused");
    assert_eq!(
        refused,
        "GitHub did not merge the pull request https://github.example/pull/1: HTTP 405: At least 1 approving review \
         is required"
    );
}

// GitHub refusing the merge because main moved as it was to merge is a moved main, as the engine's own look finds.
#[tokio::test]
async fn a_merge_github_refuses_for_a_base_that_moved_is_a_moved_main() {
    let hub = Hub::new();
    let checked = hub.committed().await;
    hub.green(&checked);
    hub.verdict(&checked).await.unwrap().unwrap();
    hub.github()
        .refuse_merges("HTTP 405: Base branch was modified. Review and try the merge again.");
    let refused = hub.merging(&checked).await.expect_err("refused");
    let main = run(&hub.repo.project.repo, &["rev-parse", &hub.host.kept_main()]);
    assert_eq!(
        refused,
        NotMerged::Moved {
            main,
            checked: checked.clone()
        }
    );
}

#[tokio::test]
async fn the_branch_is_deleted_after_its_merge_where_the_settings_say_so() {
    let hub = Hub::new().with(|config| config.delete_branch = true);
    let checked = hub.committed().await;
    hub.green(&checked);
    hub.verdict(&checked).await.unwrap().unwrap();
    hub.merge(&checked).await.unwrap();
    let deleted: Vec<Asked> = (hub.github().asked().into_iter())
        .filter(|a| matches!(a, Asked::Delete(_)))
        .collect();
    assert_eq!(deleted, [Asked::Delete(hub.place().branch)]);
}

#[tokio::test]
async fn a_merge_with_no_pull_request_or_no_check_fails_the_round() {
    let hub = Hub::new();
    let checked = hub.committed().await;
    let failed = hub.merge(&checked).await.expect_err("no pull request");
    assert!(failed.contains("has no open pull request"), "{failed}");
    let place = hub.place();
    let failed = hub.host.merge(&branch(&place), None).await.expect_err("no check");
    assert!(
        matches!(&failed, NotMerged::Failed(why) if why.contains("the flow has no check")),
        "{failed:?}"
    );
}

/// What git's credential helpers give, with what the host gives git, for this protocol and host; offline.
fn filled(with: &With, protocol: &str, host: &str) -> String {
    let settings = with.settings.iter().flat_map(|s| ["-c", s.as_str()]);
    let mut child = std::process::Command::new("git")
        .args(settings)
        .args(["credential", "fill"])
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("GIT_TERMINAL_PROMPT", "0")
        .envs(with.env.iter().cloned())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("git runs");
    use std::io::Write;
    let asked = format!("protocol={protocol}\nhost={host}\n\n");
    child.stdin.take().unwrap().write_all(asked.as_bytes()).unwrap();
    let output = child.wait_with_output().unwrap();
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn the_token_goes_to_git_in_its_environment_for_github_over_https_alone() {
    let hub = Hub::new().with(|config| config.token = Some("the-token".to_string()));
    let with = hub.host.credentials();
    assert_eq!(
        with.settings[0], "credential.helper=",
        "the owner's own helpers are not asked"
    );
    assert!(
        with.settings.iter().any(|s| s == "http.lowSpeedLimit=1000")
            && with.settings.iter().any(|s| s == "http.lowSpeedTime=60"),
        "a connection that stalls is given up: {:?}",
        with.settings
    );
    assert!(
        with.settings.iter().all(|s| !s.contains("the-token")),
        "{:?}",
        with.settings
    );
    assert!(with.env.contains(&(TOKEN.to_string(), "the-token".to_string())));
    assert!(
        with.env
            .contains(&("GIT_CONFIG_GLOBAL".to_string(), "/dev/null".to_string()))
    );
    assert!(filled(&with, "https", "github.com").contains("password=the-token\n"));
    assert!(!filled(&with, "https", "elsewhere.example").contains("the-token"));
    assert!(!filled(&with, "http", "github.com").contains("the-token"));
    let none = Hub::new().host.credentials();
    assert!(none.env.iter().all(|(name, _)| name != TOKEN));
    assert!(
        !format!("{:?}", hub.host.config).contains("the-token"),
        "the token is not shown"
    );
}

#[test]
fn a_length_of_time_is_said_in_minutes_or_seconds() {
    assert_eq!(span(Duration::from_secs(600)), "10 minutes");
    assert_eq!(span(Duration::from_millis(300)), "0 seconds");
}

#[test]
fn github_as_the_settings_say_needs_the_engines_token() {
    let repo = Repo::new("true");
    let text = "repository = \"owner/name\"\n";
    let github: settings::GitHub = toml::from_str(text).unwrap();
    let none = GitHub::from_settings(&github, &repo.project, &repo.data, |_| None, None, None);
    let refused = none.err().expect("refused");
    assert!(refused.contains("no WF_GITHUB_TOKEN"), "{refused}");
    let given = |name: &str| (name == "WF_GITHUB_TOKEN").then(|| "the-token".to_string());
    let host = GitHub::from_settings(&github, &repo.project, &repo.data, given, Some(ci()), None).expect("made");
    let config = &host.config;
    assert_eq!(config.url, "https://github.com/owner/name.git");
    assert_eq!(
        (config.start, config.timeout),
        (Duration::from_secs(600), Duration::from_secs(7200))
    );
    assert_eq!(
        config.runners,
        ["github"],
        "an installation has GitHub's own machines unless it says otherwise"
    );
    assert_eq!(
        config.ci,
        Some(ci()),
        "the project's workflows are the project's declaration"
    );
    assert_eq!(config.token.as_deref(), Some("the-token"));
    assert!(
        repo.data.join("gh").is_dir(),
        "gh keeps its configuration in the data directory"
    );
}
