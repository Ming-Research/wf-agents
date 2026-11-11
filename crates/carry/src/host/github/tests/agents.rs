//! The agents' tools `check` and `check_before` on the GitHub host: the CI of the work's last commit, read through
//! its pull request, and the CI of the code before the change, read on a branch of its own.

use super::*;
use crate::host::Named;

/// Whether the bare repository on GitHub has this object.
fn holds(hub: &Hub, object: &str) -> bool {
    std::process::Command::new("git")
        .arg("-C")
        .arg(&hub.github().bare)
        .args(["cat-file", "-e", object])
        .output()
        .expect("git runs")
        .status
        .success()
}

/// Whether the bare repository on GitHub has this branch.
fn on_github(hub: &Hub, reference: &str) -> bool {
    std::process::Command::new("git")
        .arg("-C")
        .arg(&hub.github().bare)
        .args(["rev-parse", "--verify", "--quiet", reference])
        .output()
        .expect("git runs")
        .status
        .success()
}

impl Hub {
    async fn agents_check(&self) -> Result<Option<String>, String> {
        let place = self.place();
        self.host
            .check(&branch(&place), &Named::default(), &mut Stop::never())
            .await
    }

    /// The commit of the code before the change with these files of the work's last commit put on it.
    async fn before(&self, files: &[&str]) -> Before {
        let files: Vec<String> = files.iter().map(|f| f.to_string()).collect();
        let place = self.place();
        let made = git::before(&self.repo.project, &place, &self.host.kept_main(), &files).await;
        made.expect("git works").expect("the files are committed")
    }
}

#[tokio::test]
async fn the_agents_check_pushes_the_last_commit_and_says_the_ci_of_its_pull_request() {
    let hub = Hub::new();
    let head = hub.committed().await;
    hub.github().checks(&head, vec![vec![passed("lint"), passed("test")]]);
    let said = hub.agents_check().await.unwrap().expect("nothing stopped it");
    assert_eq!(
        said,
        format!(
            "The CI passed on the commit {head}, through the pull request https://github.example/pull/1: lint, test."
        )
    );
    assert_eq!(hub.pushed(), head);
    assert_eq!(hub.opened(), 1);
    // Asked again on the same commit, it reads the same CI and opens nothing more.
    assert_eq!(hub.agents_check().await.unwrap().as_deref(), Some(said.as_str()));
    assert_eq!(hub.opened(), 1);
}

#[tokio::test]
async fn a_red_check_is_said_with_what_its_log_says_of_the_failure() {
    let hub = Hub::new();
    let head = hub.committed().await;
    let mut test = check("test", Outcome::Failed("failure".into()));
    test.job = Some(42);
    hub.github().checks(&head, vec![vec![passed("lint"), test]]);
    hub.github().log(
        42,
        "2026-11-10T04:23:09.0000000Z ##[group]Run cargo test --workspace --locked\n\
         2026-11-10T04:23:09.0000001Z shell: /usr/bin/bash -e {0}\n\
         2026-11-10T04:23:09.0000002Z ##[endgroup]\n\
         2026-11-10T04:23:10.0000000Z test tools::tests::a ... ok\n\
         2026-11-10T04:23:10.1000000Z test tools::tests::b ... FAILED\n\
         2026-11-10T04:23:10.2000000Z failures:\n\
         2026-11-10T04:23:10.3000000Z ---- tools::tests::b stdout ----\n\
         2026-11-10T04:23:10.4000000Z assertion failed: said.is_empty()\n\
         2026-11-10T04:23:11.0000000Z ##[error]Process completed with exit code 101.\n\
         2026-11-10T04:23:12.0000000Z Post job cleanup.\n",
    );
    let said = hub.agents_check().await.unwrap().expect("nothing stopped it");
    assert_eq!(
        said,
        format!(
            "The CI of the commit {head}, through the pull request https://github.example/pull/1, did not pass. It \
             passed lint.\n\n\
             The CI failed on this commit, in 1 of its checks.\n\n\
             test (failure) https://github.example/checks/test\n\
             What its log says of the failure, as the CI printed it:\n\
             ```\n\
             Run cargo test --workspace --locked\n\
             test tools::tests::b ... FAILED\n\
             failures:\n\
             ---- tools::tests::b stdout ----\n\
             assertion failed: said.is_empty()\n\
             ##[error]Process completed with exit code 101.\n\
             ```"
        )
    );
}

#[tokio::test]
async fn changes_not_committed_are_said_and_nothing_is_pushed() {
    let hub = Hub::new();
    hub.committed().await;
    std::fs::write(hub.place().dir.join("farewell.txt"), "Goodbye.\n").unwrap();
    let said = hub.agents_check().await.unwrap().expect("said");
    assert!(
        said.starts_with("The worktree has changes that are not committed") && said.contains("Nothing was pushed."),
        "{said}"
    );
    assert!(said.contains("farewell.txt"), "{said}");
    assert!(!on_github(&hub, &hub.place().reference()), "nothing was pushed");
    assert!(hub.github().asked().is_empty(), "GitHub was asked nothing");
}

#[tokio::test]
async fn a_branch_with_no_commit_of_its_own_is_said_and_nothing_is_pushed() {
    let hub = Hub::new();
    crate::acts::start(&hub.repo.project, &hub.host, &hub.place(), &Mutex::new(()))
        .await
        .expect("the work starts");
    let said = hub.agents_check().await.unwrap().expect("said");
    assert!(
        said.starts_with("The work's branch has no commit of its own beyond main")
            && said.contains("Nothing was pushed."),
        "{said}"
    );
    assert!(!on_github(&hub, &hub.place().reference()), "nothing was pushed");
}

#[tokio::test]
async fn the_agents_check_refuses_what_the_check_before_the_merge_refuses() {
    for (key, value) in [("filter.crypt.clean", "cat"), ("http.proxy", "x")] {
        let hub = Hub::new();
        hub.committed().await;
        run(&hub.repo.project.repo, &["config", key, value]);
        let refused = hub.agents_check().await.expect_err(key);
        assert!(refused.contains(key), "{key}: {refused}");
        assert!(!on_github(&hub, &hub.place().reference()), "{key}: nothing was pushed");
    }
}

#[tokio::test]
async fn the_check_before_the_change_reads_the_ci_on_a_branch_of_its_own_and_takes_the_branch_away() {
    let hub = Hub::new();
    hub.committed().await;
    let before = hub.before(&["farewell.txt"]).await;
    let mut test = check("test", Outcome::Failed("failure".into()));
    test.job = Some(7);
    // No run yet when it is first asked: the commit is pushed, and its run is waited for.
    hub.github()
        .checks(&before.commit, vec![Vec::new(), vec![passed("lint"), test]]);
    hub.github().log(
        7,
        "2026-11-10T04:23:11.0000000Z ##[error]Process completed with exit code 101.\n",
    );
    let place = hub.place();
    let said = (hub
        .host
        .check_before(&branch(&place), &before, &Named::default(), &mut Stop::never())
        .await)
        .unwrap()
        .expect("nothing stopped it");
    let reference = format!("refs/heads/wf-agents-before/{}-{}", place.branch, &before.commit[..12]);
    let name = reference.trim_start_matches("refs/heads/");
    assert!(
        said.starts_with(&format!(
            "The CI of the commit {}, on the branch {name}, did not pass. It passed lint.",
            before.commit
        )),
        "{said}"
    );
    assert!(
        said.ends_with("##[error]Process completed with exit code 101.\n```"),
        "{said}"
    );
    assert!(holds(&hub, &before.commit), "the commit was pushed");
    assert!(!on_github(&hub, &reference), "the branch is taken away");
    assert!(!on_github(&hub, &place.reference()), "the work's branch is not pushed");
    assert_eq!(hub.opened(), 0, "no pull request is opened for it");
}

#[tokio::test]
async fn two_checks_at_once_go_through_one_pull_request() {
    let hub = Hub::new();
    let head = hub.committed().await;
    hub.github().checks(&head, vec![vec![passed("lint"), passed("test")]]);
    hub.agents_check().await.unwrap().expect("checked");
    // The other check read the list of pull requests before this one's was opened, and opens its own.
    hub.github().hide_pulls_once();
    let said = hub
        .agents_check()
        .await
        .expect("checked through the one opened")
        .unwrap();
    assert!(said.contains("https://github.example/pull/1"), "{said}");
    assert_eq!(hub.opened(), 1);
}

// GitHub answers a question about the checks of a commit it has not been given with a refusal (`HTTP 422: No commit
// found for SHA`), as the fake does: such a commit has no checks, and is pushed. Asked again, the commit is GitHub's,
// and a CI of it that is complete is answered from, with nothing pushed or waited for.
#[tokio::test]
async fn a_commit_github_does_not_know_is_pushed_and_one_whose_ci_is_complete_is_answered_from() {
    let hub = Hub::new();
    hub.committed().await;
    let before = hub.before(&["farewell.txt"]).await;
    hub.github()
        .checks(&before.commit, vec![vec![passed("lint"), passed("test")]]);
    let place = hub.place();
    let first = (hub.host)
        .check_before(&branch(&place), &before, &Named::default(), &mut Stop::never())
        .await
        .expect("a commit GitHub does not know has no checks yet")
        .expect("nothing stopped it");
    assert!(
        first.starts_with(&format!("The CI passed on the commit {}", before.commit)),
        "{first}"
    );
    assert!(holds(&hub, &before.commit), "it was pushed");
    let looks = |hub: &Hub| {
        (hub.github().asked().iter())
            .filter(|a| matches!(a, Asked::Checks(_)))
            .count()
    };
    let earlier = looks(&hub);
    let again = (hub.host)
        .check_before(&branch(&place), &before, &Named::default(), &mut Stop::never())
        .await
        .unwrap()
        .expect("nothing stopped it");
    assert_eq!(again, first);
    assert_eq!(
        looks(&hub),
        earlier + 1,
        "answered from one look, with no wait after a push"
    );
}

#[tokio::test]
async fn what_an_earlier_check_before_the_change_of_this_work_left_on_github_is_taken_away() {
    let hub = Hub::new();
    hub.committed().await;
    let place = hub.place();
    let bare = hub.github().bare.display().to_string();
    let left = format!("refs/heads/wf-agents-before/{}-deadbeef0000", place.branch);
    let others = "refs/heads/wf-agents-before/work-2-0a1b2c3d-deadbeef0000".to_string();
    for reference in [&left, &others] {
        run(
            &hub.repo.project.repo,
            &["push", "-q", &bare, &format!("main:{reference}")],
        );
    }
    let before = hub.before(&["farewell.txt"]).await;
    hub.github()
        .checks(&before.commit, vec![vec![passed("lint"), passed("test")]]);
    (hub.host
        .check_before(&branch(&place), &before, &Named::default(), &mut Stop::never())
        .await)
        .unwrap()
        .expect("nothing stopped it");
    assert!(!on_github(&hub, &left), "this work's leftover is taken away");
    assert!(on_github(&hub, &others), "another work's is let be");
}

#[tokio::test]
async fn a_project_whose_ci_does_not_run_on_the_branches_before_the_change_is_refused_before_anything_is_pushed() {
    let hub = Hub::new().with(|config| config.before_branches = false);
    hub.committed().await;
    let before = hub.before(&["farewell.txt"]).await;
    let place = hub.place();
    let refused = (hub
        .host
        .check_before(&branch(&place), &before, &Named::default(), &mut Stop::never())
        .await)
        .expect_err("refused");
    assert!(refused.contains("runs_on_before_branches"), "{refused}");
    assert!(!holds(&hub, &before.commit), "nothing was pushed");
}
