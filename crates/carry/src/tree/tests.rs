//! The tree's versions against a work's branch in a repository made for each test.

use std::path::Path;

use store::WorkNumber;

use super::*;
use crate::git::tests::{Repo, run};

const WORK: WorkNumber = WorkNumber(1);

/// Writes the file, in a directory of the worktree where it names one, commits it, and gives the commit.
fn write(dir: &Path, file: &str, text: &str) -> String {
    let path = dir.join(file);
    std::fs::create_dir_all(path.parent().expect("a directory")).expect("the directory is made");
    std::fs::write(&path, text).expect("the file is written");
    run(dir, &["add", file]);
    run(dir, &["commit", "-q", "-m", &format!("Write {file}")]);
    run(dir, &["rev-parse", "HEAD"])
}

/// Removes the path, commits that, and gives the commit.
fn remove(dir: &Path, path: &str) -> String {
    run(dir, &["rm", "-q", "-r", path]);
    run(dir, &["commit", "-q", "-m", &format!("Remove {path}")]);
    run(dir, &["rev-parse", "HEAD"])
}

/// A started work.
async fn started() -> (Repo, Place) {
    let repo = Repo::new("true");
    let started = git::start(&repo.project, &repo.place(WORK), "main").await.unwrap();
    (repo, started.place)
}

/// The looks at a work's branch for the tree's block, each against the latest version, as the looks before it left it.
#[derive(Default)]
struct Looks {
    last: Option<String>,
}

impl Looks {
    /// Looks at the branch, and gives the content of the new version it makes, where it makes one.
    async fn next(&mut self, place: &Place) -> Option<String> {
        let new = read(place, "design/", self.last.as_deref()).await.unwrap();
        if new.is_some() {
            self.last = new.clone();
        }
        new
    }
}

/// The content a version of the work's branch at its head has, where it has a design tree.
fn content(place: &Place) -> String {
    let branch = place.reference();
    let tree = run(&place.dir, &["rev-parse", &format!("{branch}:design")]);
    format!("{} {tree}", run(&place.dir, &["rev-parse", &branch]))
}

#[tokio::test]
async fn a_branch_with_no_design_tree_makes_no_version() {
    let (_repo, place) = started().await;
    assert_eq!(Looks::default().next(&place).await, None);
}

#[tokio::test]
async fn the_first_look_at_a_design_tree_makes_a_version_of_the_commit_and_the_tree_and_the_next_makes_none() {
    let (_repo, place) = started().await;
    let mut looks = Looks::default();
    write(&place.dir, "design/a.md", "One.\n");
    assert_eq!(looks.next(&place).await, Some(content(&place)));
    assert_eq!(looks.next(&place).await, None, "nothing changed since");
}

#[tokio::test]
async fn a_commit_that_leaves_the_design_tree_as_it_was_makes_no_version() {
    let (_repo, place) = started().await;
    let mut looks = Looks::default();
    write(&place.dir, "design/a.md", "One.\n");
    looks.next(&place).await;
    write(&place.dir, "other.txt", "Elsewhere.\n");
    assert_eq!(looks.next(&place).await, None);
    write(&place.dir, "design/a.md", "Two.\n");
    assert_eq!(looks.next(&place).await, Some(content(&place)));
}

#[tokio::test]
async fn a_tree_put_back_as_an_earlier_version_had_it_is_a_new_version_after_the_last() {
    let (_repo, place) = started().await;
    let mut looks = Looks::default();
    write(&place.dir, "design/a.md", "One.\n");
    looks.next(&place).await;
    write(&place.dir, "design/a.md", "Two.\n");
    looks.next(&place).await;
    write(&place.dir, "design/a.md", "One.\n");
    assert_eq!(looks.next(&place).await, Some(content(&place)));
}

#[tokio::test]
async fn a_branch_that_removes_its_design_tree_makes_a_version_and_one_that_never_had_it_makes_none() {
    let (_repo, place) = started().await;
    let mut looks = Looks::default();
    write(&place.dir, "design/a.md", "One.\n");
    looks.next(&place).await;
    let removed = remove(&place.dir, "design");
    assert_eq!(looks.next(&place).await, Some(format!("{removed} -")));
    assert_eq!(looks.next(&place).await, None, "it is gone still");
    write(&place.dir, "design/a.md", "One.\n");
    assert_eq!(looks.next(&place).await, Some(content(&place)));
}

#[tokio::test]
async fn the_look_is_at_the_work_s_branch_and_not_at_what_the_worktree_has_checked_out() {
    let (_repo, place) = started().await;
    let start = run(&place.dir, &["rev-parse", "HEAD"]);
    write(&place.dir, "design/a.md", "One.\n");
    let on_the_branch = content(&place);
    run(&place.dir, &["checkout", "-q", "--detach", &start]);
    assert_eq!(Looks::default().next(&place).await, Some(on_the_branch));
}

/// The content of the tree's version at each of these commits.
fn versions(dir: &Path, commits: &[&str]) -> Vec<String> {
    (commits.iter())
        .map(|commit| {
            let tree = run(dir, &["rev-parse", &format!("{commit}:design")]);
            format!("{commit} {tree}")
        })
        .collect()
}

#[tokio::test]
async fn the_files_that_changed_between_two_versions_are_those_the_diff_of_the_design_tree_gives_between_their_commits()
{
    let (_repo, place) = started().await;
    let dir = &place.dir;
    let first = write(dir, "design/a.md", "One.\n");
    write(dir, "other.txt", "Elsewhere.\n");
    let second = write(dir, "design/decisions/b.md", "Two.\n");
    let third = write(dir, "design/decisions/c.md", "Three.\n");
    let v = versions(dir, &[&first, &second, &third]);
    let files = changed(dir, "design/", &v[0], &v[2]).await.unwrap();
    assert_eq!(files, "design/decisions/b.md\ndesign/decisions/c.md");
    let given = run(dir, &["diff", "--name-only", &first, &third, "--", "design"]);
    assert_eq!(files, given);
    let later = changed(dir, "design/", &v[1], &v[2]).await.unwrap();
    assert_eq!(later, "design/decisions/c.md");
}

#[tokio::test]
async fn a_rename_and_a_deletion_are_named_by_every_path_they_touch() {
    let (_repo, place) = started().await;
    let dir = &place.dir;
    let first = write(dir, "design/a.md", "One, which is long enough to be known again.\n");
    write(dir, "design/b.md", "Two, which goes.\n");
    let second = write(dir, "design/keep.md", "Three.\n");
    run(dir, &["mv", "design/a.md", "design/c.md"]);
    run(dir, &["commit", "-q", "-m", "Rename a.md"]);
    let third = remove(dir, "design/b.md");
    let v = versions(dir, &[&first, &second, &third]);
    let files = changed(dir, "design/", &v[1], &v[2]).await.unwrap();
    assert_eq!(files, "design/a.md\ndesign/b.md\ndesign/c.md");
}

#[tokio::test]
async fn a_version_that_is_not_the_trees_has_no_files() {
    let (_repo, place) = started().await;
    let files = changed(&place.dir, "design/", "nothing", "not of the tree at all").await;
    assert_eq!(files.unwrap(), "");
}
