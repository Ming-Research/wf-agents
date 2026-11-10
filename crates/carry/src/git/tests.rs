//! The acts' git against a repository made for each test, with a file committed on its main branch.

use std::path::{Path, PathBuf};

use store::WorkNumber;

use super::*;
use crate::held::Stop;

/// A repository with one commit on `main`, a data directory beside it, and the project they make.
pub(crate) struct Repo {
    _dir: tempfile::TempDir,
    pub project: Project,
    pub data: PathBuf,
}

/// Runs git in the directory, which must pass, and gives its output.
pub(crate) fn run(dir: &Path, args: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim_end().to_string()
}

/// Writes the file in the directory and commits it there.
pub(crate) fn commit(dir: &Path, file: &str, text: &str) -> String {
    std::fs::write(dir.join(file), text).expect("the file is written");
    run(dir, &["add", file]);
    run(dir, &["commit", "-q", "-m", &format!("Write {file}")]);
    run(dir, &["rev-parse", "HEAD"])
}

impl Repo {
    /// A new repository, whose check is this command.
    pub(crate) fn new(check: &str) -> Repo {
        let dir = tempfile::tempdir().expect("a directory");
        let repo = dir.path().join("repo");
        let data = dir.path().join("data");
        std::fs::create_dir_all(&repo).unwrap();
        std::fs::create_dir_all(&data).unwrap();
        run(&repo, &["init", "-q", "-b", "main"]);
        run(&repo, &["config", "user.name", "Tester"]);
        run(&repo, &["config", "user.email", "tester@example.com"]);
        run(&repo, &["config", "commit.gpgsign", "false"]);
        commit(&repo, "greeting.txt", "Hello.\n");
        Repo {
            _dir: dir,
            project: Project {
                repo,
                main: "main".to_string(),
                check: check.to_string(),
            },
            data,
        }
    }
}

const WORK: WorkNumber = WorkNumber(1);

async fn checked(repo: &Repo) -> Checked {
    check(&repo.project, &repo.data, WORK, &mut Stop::never())
        .await
        .expect("the work is checked")
}

#[tokio::test]
async fn start_twice_gives_one_worktree() {
    let repo = Repo::new("true");
    let main = run(&repo.project.repo, &["rev-parse", "main"]);
    let first = start(&repo.project, &repo.data, WORK).await.expect("the work starts");
    assert_eq!(first.place.branch, "work-1");
    assert_eq!(first.from, main);
    assert!(first.place.dir.join("greeting.txt").is_file());
    let again = start(&repo.project, &repo.data, WORK)
        .await
        .expect("the work starts again");
    assert_eq!(again, first);
    let worktrees = run(&repo.project.repo, &["worktree", "list", "--porcelain"]);
    assert_eq!(worktrees.lines().filter(|l| l.starts_with("worktree ")).count(), 2);
}

#[tokio::test]
async fn a_work_with_no_worktree_is_not_checked() {
    let repo = Repo::new("true");
    let refused = check(&repo.project, &repo.data, WORK, &mut Stop::never()).await;
    assert!(refused.is_err());
}

#[tokio::test]
async fn the_check_passes_a_committed_work_that_merges_and_passes_the_command() {
    let repo = Repo::new("test -f greeting.txt");
    let started = start(&repo.project, &repo.data, WORK).await.unwrap();
    let head = commit(&started.place.dir, "farewell.txt", "Bye.\n");
    let checked = checked(&repo).await;
    assert_eq!(checked.failed, None);
    assert_eq!(checked.commit, head);
    assert_eq!(checked.main, run(&repo.project.repo, &["rev-parse", "main"]));
}

#[tokio::test]
async fn the_check_fails_a_failing_command_with_the_end_of_what_it_said() {
    let repo = Repo::new("echo the greeting is wrong >&2; exit 3");
    start(&repo.project, &repo.data, WORK).await.unwrap();
    let failed = checked(&repo).await.failed.expect("the check fails");
    assert!(failed.contains("the greeting is wrong"), "{failed}");
}

#[tokio::test]
async fn the_check_fails_uncommitted_changes() {
    let repo = Repo::new("true");
    let started = start(&repo.project, &repo.data, WORK).await.unwrap();
    std::fs::write(started.place.dir.join("draft.txt"), "Not yet.\n").unwrap();
    let failed = checked(&repo).await.failed.expect("the check fails");
    assert!(failed.contains("not committed"), "{failed}");
}

#[tokio::test]
async fn the_check_fails_a_conflict_with_main() {
    let repo = Repo::new("true");
    let started = start(&repo.project, &repo.data, WORK).await.unwrap();
    commit(&started.place.dir, "greeting.txt", "Hello, you.\n");
    commit(&repo.project.repo, "greeting.txt", "Good day.\n");
    let failed = checked(&repo).await.failed.expect("the check fails");
    assert!(failed.contains("conflict"), "{failed}");
    // Nothing was merged.
    assert_eq!(
        std::fs::read_to_string(repo.project.repo.join("greeting.txt")).unwrap(),
        "Good day.\n"
    );
}

#[tokio::test]
async fn the_merge_puts_the_work_on_main_once() {
    let repo = Repo::new("true");
    let started = start(&repo.project, &repo.data, WORK).await.unwrap();
    let head = commit(&started.place.dir, "farewell.txt", "Bye.\n");
    let merged = merge(&repo.project, &repo.data, WORK).await.expect("the work merges");
    assert_eq!(merged, run(&repo.project.repo, &["rev-parse", "main"]));
    let parents = run(&repo.project.repo, &["rev-list", "--parents", "-n", "1", "main"]);
    assert_eq!(
        parents.split(' ').nth(2),
        Some(head.as_str()),
        "a merge commit of the work's head"
    );
    assert!(repo.project.repo.join("farewell.txt").is_file());
    let again = merge(&repo.project, &repo.data, WORK).await.expect("merged again");
    assert_eq!(again, merged);
}

#[tokio::test]
async fn the_merge_needs_the_checkout_on_main() {
    let repo = Repo::new("true");
    start(&repo.project, &repo.data, WORK).await.unwrap();
    run(&repo.project.repo, &["switch", "-q", "-c", "elsewhere"]);
    let refused = merge(&repo.project, &repo.data, WORK)
        .await
        .expect_err("the merge is refused");
    assert!(refused.contains("not on the main branch"), "{refused}");
}
