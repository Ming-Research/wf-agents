//! What a work changed, against a repository made for each test: what its head and its worktree hold over where its
//! branch left main, the same once its worktree and branch are gone and it merged, and one file's diff. Each test makes
//! a repository of a few commits and runs a few dozen git commands, in well under a second.

use super::*;
use crate::git::tests::{Repo, commit, run};

const WORK: WorkNumber = WorkNumber(1);

/// Main's head as the local host keeps it, which the tests' works branch from.
const MAIN: &str = "refs/heads/main";

/// A started work.
async fn started() -> (Repo, Place) {
    let repo = Repo::new("true");
    let started = git::start(&repo.project, &repo.place(WORK), "main", "main")
        .await
        .unwrap();
    (repo, started.place)
}

/// Each file changed: its path, lines added and removed, whether binary, and whether committed.
fn files(changed: &Changed) -> Vec<(&str, u64, u64, bool, bool)> {
    (changed.changes.files.iter())
        .map(|f| (f.path.as_str(), f.added, f.removed, f.binary, f.committed))
        .collect()
}

#[tokio::test]
async fn a_work_changed_what_its_head_and_its_worktree_hold_over_where_its_branch_left_main() {
    let (repo, place) = started().await;
    let r = &repo.project.repo;
    let known = Known::default();
    assert!(
        known
            .changed(r, MAIN, WorkNumber(2), &repo.place(WorkNumber(2)))
            .await
            .unwrap()
            .is_none(),
        "a work not started has no head to tell from"
    );
    let left = run(r, &["rev-parse", "main"]);
    // Main moves on after the branch left it: what it brought is none of the work's change.
    commit(r, "elsewhere.txt", "Not the work's.\n");
    let head = commit(&place.dir, "farewell.txt", "Bye.\nSee you.\n");
    // A tracked file changed and not committed, a file git does not track whose last line has no newline, and a
    // binary one.
    std::fs::write(place.dir.join("greeting.txt"), "Hello.\nHi.\n").unwrap();
    std::fs::write(place.dir.join("notes.txt"), "One\nTwo\nThree").unwrap();
    std::fs::write(place.dir.join("blob.bin"), [0u8, 1, 2]).unwrap();
    let changed = known.changed(r, MAIN, WORK, &place).await.unwrap().expect("a head");
    let changes = &changed.changes;
    assert_eq!(
        (changes.base.as_str(), changes.head.as_str(), changes.worktree),
        (left.as_str(), head.as_str(), true)
    );
    assert_eq!(
        files(&changed),
        [
            ("blob.bin", 0, 0, true, false),
            ("farewell.txt", 2, 0, false, true),
            ("greeting.txt", 1, 0, false, false),
            ("notes.txt", 3, 0, false, false),
        ]
    );
    let stat = Diffstat {
        files: 4,
        added: 6,
        removed: 0,
    };
    assert_eq!(changed.stat(), stat);
}

#[tokio::test]
async fn once_its_worktree_and_branch_are_gone_a_merged_work_changed_what_its_kept_head_holds() {
    let (repo, place) = started().await;
    let r = &repo.project.repo;
    let left = run(r, &["rev-parse", "main"]);
    commit(&place.dir, "greeting.txt", "Hello, you.\n");
    let head = commit(&place.dir, "farewell.txt", "Bye.\n");
    crate::head::keep(&place, &head).await.unwrap();
    // Merged by a commit of the merge, as the local repository merges, after which main moves on.
    git::merge(&repo.project, &place, "Say goodbye", Some(&head))
        .await
        .unwrap();
    commit(r, "later.txt", "Later.\n");
    git::remove(&repo.project, &place).await.unwrap();
    run(r, &["branch", "-D", &place.branch]);
    let changed = Known::default()
        .changed(r, MAIN, WORK, &place)
        .await
        .unwrap()
        .expect("the kept head");
    let changes = &changed.changes;
    assert_eq!(
        (changes.base.as_str(), changes.head.as_str(), changes.worktree),
        (left.as_str(), head.as_str(), false)
    );
    assert_eq!(
        files(&changed),
        [("farewell.txt", 1, 0, false, true), ("greeting.txt", 1, 1, false, true)]
    );
}

#[tokio::test]
async fn the_diff_of_a_file_the_work_changed_is_given_and_of_no_other_path() {
    let (repo, place) = started().await;
    let r = &repo.project.repo;
    commit(&place.dir, "farewell.txt", "Bye.\n");
    std::fs::write(place.dir.join("notes.txt"), "One\n").unwrap();
    let known = Known::default();
    let diff = |path: &'static str| known.diff(r, MAIN, (WORK, &place), path);
    let committed = diff("farewell.txt").await.unwrap().expect("changed");
    assert!(committed.text.contains("+Bye."), "{}", committed.text);
    assert_eq!((committed.path.as_str(), committed.cut), ("farewell.txt", None));
    let untracked = diff("notes.txt").await.unwrap().expect("changed");
    assert!(untracked.text.contains("+One"), "{}", untracked.text);
    assert_eq!(
        diff("greeting.txt").await.unwrap(),
        None,
        "a file the work did not change"
    );
    assert_eq!(
        diff("../repo/greeting.txt").await.unwrap(),
        None,
        "no path the work changed"
    );
}

#[test]
fn a_diff_longer_than_its_bound_is_cut_at_a_character_saying_its_length() {
    let long = format!("{}é", "a".repeat(DIFF - 1));
    let given = cut("big.txt", long.clone());
    assert_eq!(given.text, "a".repeat(DIFF - 1));
    assert_eq!(given.cut, Some(long.len() as u64));
}

#[test]
fn the_commit_a_merge_made_is_read_back_from_what_it_wrote_by_its_words() {
    let text = crate::acts::words().merge.text;
    let commit = "0123456789abcdef0123456789abcdef01234567";
    let slots = [("branch", "refs/heads/work-1-x"), ("main", "main"), ("head", commit)];
    let wrote = flow::agents::fill(&text, &slots);
    assert_eq!(merged_commit(&wrote, &text).as_deref(), Some(commit));
    assert_eq!(merged_commit("Something else.", &text), None);
}

#[tokio::test]
async fn the_pull_request_a_merge_s_commit_names_at_the_end_of_its_title_is_read() {
    let repo = Repo::new("true");
    let r = &repo.project.repo;
    run(r, &["commit", "-q", "--allow-empty", "-m", "Greet the user (#41)"]);
    let named = run(r, &["rev-parse", "HEAD"]);
    run(r, &["commit", "-q", "--allow-empty", "-m", "Greet the user"]);
    let unnamed = run(r, &["rev-parse", "HEAD"]);
    let known = Known::default();
    assert_eq!(known.pull_named(r, &named).await.unwrap(), Some(41));
    assert_eq!(known.pull_named(r, &unnamed).await.unwrap(), None);
}
