//! The agents' tool `run_tests` on the GitHub host with none named, which the project's full workflows run on the work's
//! last commit, pushed and put in its pull request first; and what it refuses before it pushes.

use super::*;
use crate::host::Named;

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
            .run_tests(&branch(&place), &Named::default(), &mut Stop::never())
            .await
    }
}

#[tokio::test]
async fn the_agents_run_pushes_the_last_commit_opens_its_pull_request_and_runs_the_full_workflows_on_it() {
    let hub = Hub::new();
    let head = hub.committed().await;
    hub.github().fast(
        &head,
        vec![passed("lint"), passed("test")],
        Some(("test-results", fast::junit())),
    );
    let said = hub.agents_check().await.unwrap().expect("nothing stopped it");
    assert!(
        said.starts_with(&format!("The full workflows ran on the commit {head}")),
        "{said}"
    );
    assert_eq!(hub.pushed(), head);
    assert_eq!(hub.opened(), 1);
    // Asked again on the same commit, it opens nothing more.
    hub.agents_check().await.unwrap().expect("again");
    assert_eq!(hub.opened(), 1);
}

#[tokio::test]
async fn changes_not_committed_are_said_and_nothing_is_pushed() {
    let hub = Hub::new();
    hub.committed().await;
    std::fs::write(hub.place().dir.join("farewell.txt"), "Goodbye.\n").unwrap();
    let said = hub.agents_check().await.unwrap().expect("said");
    assert!(
        said.starts_with("The worktree has changes that are not committed, and only what is committed is run")
            && said.contains("Nothing was pushed."),
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
async fn two_checks_at_once_go_through_one_pull_request() {
    let hub = Hub::new();
    let head = hub.committed().await;
    hub.github().fast(&head, vec![], Some(("test-results", fast::junit())));
    hub.agents_check().await.unwrap().expect("checked");
    // The other check read the list of pull requests before this one's was opened, and opens its own.
    hub.github().hide_pulls_once();
    let said = hub
        .agents_check()
        .await
        .expect("checked through the one opened")
        .unwrap();
    assert!(said.starts_with("The full workflows ran on the commit"), "{said}");
    assert_eq!(hub.opened(), 1);
}
