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
            .run_tests(&branch(&place), &Named::default(), self.call(), &mut Stop::never())
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
async fn the_agents_run_says_the_pull_request_it_checks_the_work_through_for_the_work_to_keep() {
    let hub = Hub::new();
    let head = hub.committed().await;
    hub.github().fast(
        &head,
        vec![passed("lint"), passed("test")],
        Some(("test-results", fast::junit())),
    );
    let (to, mut said) = tokio::sync::mpsc::unbounded_channel();
    let reply = service::CallReply::new(WORK, CallId(1), to);
    let mut stop = Stop::never().hosting(reply.hosting());
    let place = hub.place();
    (hub.host
        .run_tests(&branch(&place), &Named::default(), hub.call(), &mut stop)
        .await)
        .expect("run")
        .expect("nothing stopped it");
    let pull = hub.github().pull_requests()[0].clone();
    let pulled = engine::Event::Pulled {
        number: pull.number,
        url: pull.url,
    };
    assert_eq!(super::ends::heard(&mut said), [pulled]);
}

#[tokio::test]
async fn changes_not_committed_to_a_tracked_file_are_said_and_nothing_is_pushed() {
    let hub = Hub::new();
    hub.committed().await;
    std::fs::write(hub.place().dir.join("farewell.txt"), "Goodbye.\n").unwrap();
    let said = hub.agents_check().await.unwrap().expect("said");
    assert!(
        said.starts_with(
            "The worktree has changes to tracked files that are not committed, and only what is committed is run"
        ) && said.contains("Nothing was pushed."),
        "{said}"
    );
    assert!(said.contains("farewell.txt"), "{said}");
    assert!(!on_github(&hub, &hub.place().reference()), "nothing was pushed");
    assert!(hub.github().asked().is_empty(), "GitHub was asked nothing");
}

#[tokio::test]
async fn a_merge_not_committed_is_said_and_nothing_is_pushed() {
    let hub = Hub::new();
    hub.committed().await;
    let dir = hub.place().dir;
    // A commit of the same tree whose parent is not in the branch's history, merged with no commit made.
    let tree = run(&dir, &["rev-parse", "HEAD^{tree}"]);
    let parent = run(&dir, &["rev-parse", "HEAD~1"]);
    let side = run(&dir, &["commit-tree", &tree, "-p", &parent, "-m", "A side"]);
    run(&dir, &["merge", "--no-commit", "--no-ff", &side]);
    let said = hub.agents_check().await.unwrap().expect("said");
    assert!(
        said.starts_with("A merge is under way in the worktree and is not committed")
            && said.contains("Nothing was pushed."),
        "{said}"
    );
    assert!(!on_github(&hub, &hub.place().reference()), "nothing was pushed");
    assert!(hub.github().asked().is_empty(), "GitHub was asked nothing");
}

// A file git does not track is in no commit whoever left it, so it holds nothing back; but it is not in the run, and a
// test the agent forgot to commit must not be missing from the result unseen: the result says so before it begins.
#[tokio::test]
async fn files_git_does_not_track_do_not_hold_the_run_back_and_are_said_with_its_result() {
    let hub = Hub::new();
    let head = hub.committed().await;
    hub.github().fast(
        &head,
        vec![passed("lint"), passed("test")],
        Some(("test-results", fast::junit())),
    );
    let dir = hub.place().dir;
    std::fs::write(dir.join("forgotten_test.rs"), "A test.\n").unwrap();
    // A directory of files holds one line, not one for each file.
    std::fs::create_dir_all(dir.join("cache/deep")).unwrap();
    std::fs::write(dir.join("cache/one"), "1\n").unwrap();
    std::fs::write(dir.join("cache/deep/two"), "2\n").unwrap();
    let said = hub.agents_check().await.unwrap().expect("nothing stopped it");
    let heading = "These files are not tracked by git, so they are not in the run, which runs only what is committed.";
    let (untracked, result) = said.split_once("\n\n").expect(&said);
    assert!(untracked.starts_with(heading), "{said}");
    assert_eq!(
        untracked[heading.len()..].lines().skip(1).collect::<Vec<_>>(),
        ["cache/", "forgotten_test.rs"],
        "{said}"
    );
    assert!(
        result.starts_with(&format!("The full workflows ran on the commit {head}")),
        "{said}"
    );
    assert_eq!(hub.pushed(), head, "the last commit was run");
}

// Nothing is said of what is not there: a worktree with only what is committed has a result of the run alone.
#[tokio::test]
async fn a_result_says_nothing_of_untracked_files_where_there_are_none() {
    let hub = Hub::new();
    let head = hub.committed().await;
    hub.github().fast(
        &head,
        vec![passed("lint"), passed("test")],
        Some(("test-results", fast::junit())),
    );
    let said = hub.agents_check().await.unwrap().expect("nothing stopped it");
    assert!(!said.contains("not tracked by git"), "{said}");
}

#[tokio::test]
async fn a_branch_with_no_commit_of_its_own_is_said_and_nothing_is_pushed() {
    let hub = Hub::new();
    let place = hub.place();
    crate::acts::start(&hub.repo.project, &hub.host, &super::branch(&place), &Mutex::new(()))
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

// A change to a tracked file refuses the run as before, and what it says is the tracked change alone: the file git does
// not track is no part of what is refused for, and the run that was refused says nothing of files it never looked at.
#[tokio::test]
async fn a_tracked_change_refuses_the_run_and_names_it_alone_where_files_git_does_not_track_are_there_too() {
    let hub = Hub::new();
    hub.committed().await;
    let dir = hub.place().dir;
    std::fs::write(dir.join("farewell.txt"), "Goodbye.\n").unwrap();
    std::fs::write(dir.join("cache.txt"), "Cache.\n").unwrap();
    let said = hub.agents_check().await.unwrap().expect("said");
    assert!(said.contains("Nothing was pushed."), "{said}");
    assert!(said.contains("farewell.txt"), "{said}");
    assert!(!said.contains("cache.txt"), "{said}");
    assert!(!on_github(&hub, &hub.place().reference()), "nothing was pushed");
    assert!(hub.github().asked().is_empty(), "GitHub was asked nothing");
}

// A file the repository's ignore rules name is no file to commit: it is not said as one the run lacks.
#[tokio::test]
async fn a_file_the_ignore_rules_name_is_not_said_as_one_the_run_lacks() {
    let hub = Hub::new();
    hub.committed().await;
    let dir = hub.place().dir;
    let head = commit(&dir, ".gitignore", "target/\n");
    hub.github().fast(
        &head,
        vec![passed("lint"), passed("test")],
        Some(("test-results", fast::junit())),
    );
    std::fs::create_dir_all(dir.join("target")).unwrap();
    std::fs::write(dir.join("target/out"), "built\n").unwrap();
    let said = hub.agents_check().await.unwrap().expect("nothing stopped it");
    assert!(!said.contains("not tracked by git"), "{said}");
}
