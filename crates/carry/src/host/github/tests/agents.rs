//! The agents' tool `run_tests` on the GitHub host, before and as it pushes: where no commit is named it runs the last
//! commit of the work's branch, which it refuses while the worktree holds a change that is not in it; where one is
//! named it runs that commit, whatever the worktree holds. Either is pushed to a branch of its own for the call, never
//! to the work's branch, and no pull request is opened for it.

use super::fast::named;
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
    /// The call of `run_tests` that names the one test the fake's results hold, with no commit.
    async fn agents_check(&self) -> Result<Option<String>, String> {
        self.agents_named(&named(&["tools::tests::a"], &[])).await
    }

    async fn agents_named(&self, named: &Named) -> Result<Option<String>, String> {
        let place = self.place();
        self.host
            .run_tests(&branch(&place), named, self.call(), &mut Stop::never())
            .await
    }

    /// The commit each run GitHub was asked for was given, in order.
    fn run_on(&self) -> Vec<String> {
        (self.dispatched().into_iter())
            .filter_map(|asked| match asked {
                Asked::Dispatch { inputs, .. } => inputs.into_iter().find(|(name, _)| name == "commit"),
                _ => None,
            })
            .map(|(_, commit)| commit)
            .collect()
    }

    /// The branches GitHub was asked to delete.
    fn deleted(&self) -> Vec<String> {
        (self.github().asked().into_iter())
            .filter_map(|asked| match asked {
                Asked::Delete(name) => Some(name),
                _ => None,
            })
            .collect()
    }
}

#[tokio::test]
async fn the_agents_run_pushes_the_last_commit_to_a_branch_of_its_own_and_opens_no_pull_request() {
    let hub = Hub::new();
    let head = hub.committed().await;
    hub.github().fast(
        &head,
        vec![passed("lint"), passed("test")],
        Some(("test-results", fast::junit())),
    );
    let said = hub.agents_check().await.unwrap().expect("nothing stopped it");
    assert!(
        said.starts_with(&format!("The fast workflow ran on the commit {head}")),
        "{said}"
    );
    assert_eq!(hub.run_on(), [head]);
    assert!(
        !on_github(&hub, &hub.place().reference()),
        "the work's own branch is the check's to push"
    );
    assert_eq!(hub.opened(), 0);
    // The branch the commit was pushed to is gone once the result is read, and was named for the work and the call.
    assert!(hub.trials().is_empty());
    let branch = hub.place().branch;
    let deleted = hub.deleted();
    assert!(
        matches!(deleted.as_slice(), [one] if one.starts_with(&format!("wf-try/{branch}/call/"))),
        "{deleted:?}"
    );
}

#[tokio::test]
async fn the_agents_run_says_no_pull_request_for_the_work_to_keep() {
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
        .run_tests(
            &branch(&place),
            &named(&["tools::tests::a"], &[]),
            hub.call(),
            &mut stop,
        )
        .await)
        .expect("run")
        .expect("nothing stopped it");
    assert_eq!(super::ends::heard(&mut said), Vec::<engine::Event>::new());
}

// A commit no branch holds, as an agent makes one to show a test red on the code before its change, is the commit run:
// by its full name or by one git resolves in the worktree.
#[tokio::test]
async fn a_commit_named_is_the_one_run_and_the_works_branch_is_not_pushed() {
    let hub = Hub::new();
    let head = hub.committed().await;
    let dir = hub.place().dir;
    let tree = run(&dir, &["rev-parse", "HEAD^{tree}"]);
    let parent = run(&dir, &["rev-parse", "HEAD~1"]);
    let demonstration = run(&dir, &["commit-tree", &tree, "-p", &parent, "-m", "A demonstration"]);
    assert_ne!(demonstration, head);
    hub.github().fast(
        &demonstration,
        vec![passed("test")],
        Some(("test-results", fast::junit())),
    );
    let asked = Named {
        commit: Some(demonstration.clone()),
        ..named(&["tools::tests::a"], &[])
    };
    let said = hub.agents_named(&asked).await.unwrap().expect("run");
    assert!(
        said.starts_with(&format!("The fast workflow ran on the commit {demonstration}")),
        "{said}"
    );
    // A name given as git takes it is said and run as the commit it gives, in full.
    hub.github()
        .fast(&parent, vec![passed("test")], Some(("test-results", fast::junit())));
    let by_name = Named {
        commit: Some("HEAD~1".to_string()),
        ..named(&["tools::tests::a"], &[])
    };
    let said = hub.agents_named(&by_name).await.unwrap().expect("run");
    assert!(
        said.starts_with(&format!("The fast workflow ran on the commit {parent}")),
        "{said}"
    );
    assert_eq!(hub.run_on(), [demonstration, parent]);
    assert!(
        !on_github(&hub, &hub.place().reference()),
        "the work's own branch is the check's to push"
    );
    assert_eq!(hub.opened(), 0);
    assert!(hub.trials().is_empty());
}

// The agent says which commit to run, so a change not committed, a file git does not track and a merge not committed are
// no concern of the run, and the result says nothing of them.
#[tokio::test]
async fn a_commit_named_is_run_whatever_the_worktree_holds_and_the_result_says_nothing_of_it() {
    let hub = Hub::new();
    let head = hub.committed().await;
    hub.github().fast(
        &head,
        vec![passed("lint"), passed("test")],
        Some(("test-results", fast::junit())),
    );
    let dir = hub.place().dir;
    // A merge under way with no commit made, as the test of its refusal makes it, then a tracked file changed and a file
    // git does not track.
    let tree = run(&dir, &["rev-parse", "HEAD^{tree}"]);
    let parent = run(&dir, &["rev-parse", "HEAD~1"]);
    let side = run(&dir, &["commit-tree", &tree, "-p", &parent, "-m", "A side"]);
    run(&dir, &["merge", "--no-commit", "--no-ff", &side]);
    std::fs::write(dir.join("farewell.txt"), "Goodbye.\n").unwrap();
    std::fs::write(dir.join("forgotten_test.rs"), "A test.\n").unwrap();
    let asked = Named {
        commit: Some(head.clone()),
        ..named(&["tools::tests::a"], &[])
    };
    let said = hub.agents_named(&asked).await.unwrap().expect("nothing stopped it");
    assert!(
        said.starts_with(&format!("The fast workflow ran on the commit {head}")),
        "{said}"
    );
    assert!(
        !said.contains("not tracked by git") && !said.contains("not committed"),
        "{said}"
    );
}

// A name git does not resolve to a commit in the worktree, one that is not a commit, and one that git would read as an
// option, are said, and nothing is pushed or asked for.
#[tokio::test]
async fn a_commit_git_does_not_find_is_said_and_nothing_is_pushed() {
    let hub = Hub::new();
    hub.committed().await;
    for (name, why) in [
        ("no-such-commit", "no-such-commit"),
        ("HEAD:farewell.txt", "HEAD:farewell.txt is no commit"),
        ("-x", "-x: a name does not begin with a dash"),
    ] {
        let asked = Named {
            commit: Some(name.to_string()),
            ..named(&["tools::tests::a"], &[])
        };
        let said = hub.agents_named(&asked).await.unwrap().expect("said");
        assert!(
            said.starts_with("Nothing was run: git finds no commit in your worktree by the name you gave: ")
                && said.contains(why)
                && said.ends_with("Nothing was pushed."),
            "{name}: {said}"
        );
    }
    assert!(hub.github().asked().is_empty(), "GitHub was asked nothing");
    assert!(hub.trials().is_empty());
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
    assert!(hub.trials().is_empty(), "nothing was pushed");
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
    assert!(hub.trials().is_empty(), "nothing was pushed");
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
        result.starts_with(&format!("The fast workflow ran on the commit {head}")),
        "{said}"
    );
    assert_eq!(hub.run_on(), [head], "the last commit was run");
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
async fn the_agents_check_refuses_what_the_check_before_the_merge_refuses() {
    for (key, value) in [("filter.crypt.clean", "cat"), ("http.proxy", "x")] {
        let hub = Hub::new();
        hub.committed().await;
        run(&hub.repo.project.repo, &["config", key, value]);
        let refused = hub.agents_check().await.expect_err(key);
        assert!(refused.contains(key), "{key}: {refused}");
        assert!(hub.trials().is_empty(), "{key}: nothing was pushed");
    }
}

// Calls of one work asked together push to branches of their own, and none is the work's branch, so that one call's
// deleting its branch takes nothing from another's run.
#[test]
fn each_call_has_a_branch_of_its_own_that_is_not_the_works() {
    let hub = Hub::new();
    let place = hub.place();
    let (one, other) = (Asking::Call(CallId(1)), Asking::Call(CallId(2)));
    let (first, second) = (
        super::super::trial_branch(&branch(&place), one),
        super::super::trial_branch(&branch(&place), other),
    );
    assert_ne!(first, second);
    for name in [first, second] {
        assert!(name.starts_with(&format!("wf-try/{}/", place.branch)), "{name}");
        assert_ne!(name, place.branch);
    }
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
    assert!(hub.trials().is_empty(), "nothing was pushed");
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
