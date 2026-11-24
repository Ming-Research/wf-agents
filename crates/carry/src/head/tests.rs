//! The work's head against a work's branch in a repository made for each test: which merges on top of it bring only
//! main's code, and what a round is given between two versions. Each test makes a repository of a few commits and
//! runs a few dozen git commands, in well under a second.

use decide::RunId;
use store::WorkNumber;

use super::*;
use crate::catch_up::{self, Outcome};
use crate::git::tests::{Repo, commit, run};

const WORK: WorkNumber = WorkNumber(1);

/// The item and the work whose commits these are.
const WHOSE: crate::commit_lines::Whose = crate::commit_lines::Whose {
    item: store::ItemNumber(36),
    work: WORK,
};

/// The reference that holds main's head in the tests' repository, its own main branch.
const MAIN: &str = "refs/heads/main";

/// A started work whose branch has a commit of its own, and that commit.
async fn built() -> (Repo, Place, String) {
    let repo = Repo::new("true");
    let started = git::start(&repo.project, &repo.place(WORK), "main", "main")
        .await
        .unwrap();
    let built = commit(&started.place.dir, "farewell.txt", "Bye.\n");
    (repo, started.place, built)
}

/// The work's head as the branch now has it.
async fn work_head(place: &Place) -> String {
    at(place, MAIN).await.expect("the head is read").0
}

/// The worktree's head, and its subject and author's address.
fn head_of(place: &Place) -> (String, String, String) {
    let line = run(&place.dir, &["log", "-1", "--format=%H%x00%s%x00%ae"]);
    let fields: Vec<&str> = line.split('\0').collect();
    (fields[0].to_string(), fields[1].to_string(), fields[2].to_string())
}

/// Merges main into the worktree as an agent would, by its own address and words.
fn agent_merges(place: &Place) -> std::process::Output {
    std::process::Command::new("git")
        .arg("-C")
        .arg(&place.dir)
        .args(["merge", "--no-ff", "-m", "Take main in", "main"])
        .output()
        .expect("git runs")
}

/// Resolves the merge under way in the worktree, the file in conflict given this text, and commits it.
fn resolve(place: &Place, file: &str, text: &str) {
    std::fs::write(place.dir.join(file), text).unwrap();
    run(&place.dir, &["add", file]);
    run(&place.dir, &["commit", "-q", "--no-edit"]);
}

// A branch as it starts holds no change of its own: its head is main's, and no version of the work's head is made
// until the work commits.
#[tokio::test]
async fn a_branch_with_no_commit_of_its_own_makes_no_version_until_the_work_commits() {
    let repo = Repo::new("true");
    let started = git::start(&repo.project, &repo.place(WORK), "main", "main")
        .await
        .unwrap();
    let place = started.place;
    assert_eq!(read(&place, MAIN, None).await.unwrap(), None);
    let built = commit(&place.dir, "farewell.txt", "Bye.\n");
    assert_eq!(read(&place, MAIN, None).await.unwrap(), Some(built.clone()));
    assert_eq!(
        read(&place, MAIN, Some(&built)).await.unwrap(),
        None,
        "nothing moved since"
    );
}

// The engine's merge of main as a run starts brings only main's code: the work's head stays its own commit, and main
// moving again and being taken in again keeps it there.
#[tokio::test]
async fn the_engines_merge_of_main_is_no_move_of_the_head() {
    let (repo, place, built) = built().await;
    for (file, round) in [("other.txt", 7), ("weather.txt", 8)] {
        commit(&repo.project.repo, file, "Good day.\n");
        let caught = catch_up::take_in(&repo.project, &place, MAIN, RunId(round), WHOSE).await;
        assert!(matches!(caught.outcome, Outcome::TookIn { .. }), "{caught:?}");
        assert_eq!(work_head(&place).await, built);
        assert_eq!(read(&place, MAIN, Some(&built)).await.unwrap(), None);
    }
}

// An agent that amends the engine's merge of main keeps its address and its words, and slips its own change in as
// main's: known by its content, the merge is no longer one that brings only main's code, and the head moves to it.
#[tokio::test]
async fn an_amended_merge_of_the_engines_is_a_move_of_the_head() {
    let (repo, place, built) = built().await;
    commit(&repo.project.repo, "other.txt", "Good day.\n");
    let caught = catch_up::take_in(&repo.project, &place, MAIN, RunId(7), WHOSE).await;
    let Outcome::TookIn { merge, .. } = caught.outcome else {
        panic!("{caught:?}");
    };
    let (_, words, address) = head_of(&place);
    std::fs::write(place.dir.join("other.txt"), "Good night.\n").unwrap();
    run(&place.dir, &["commit", "-q", "-a", "--amend", "--no-edit"]);
    let (amended, amended_words, amended_address) = head_of(&place);
    assert_ne!(amended, merge);
    assert_eq!(
        (amended_words, amended_address),
        (words, address),
        "the address and the words are kept"
    );
    assert_eq!(work_head(&place).await, amended);
    assert_eq!(read(&place, MAIN, Some(&built)).await.unwrap(), Some(amended));
}

// An agent's clean merge of main, by its own address and words, brings only main's code all the same.
#[tokio::test]
async fn an_agents_clean_merge_of_main_is_no_move_of_the_head() {
    let (repo, place, built) = built().await;
    commit(&repo.project.repo, "other.txt", "Good day.\n");
    assert!(agent_merges(&place).status.success());
    let (merge, words, _) = head_of(&place);
    assert_eq!(words, "Take main in");
    assert_ne!(merge, built);
    assert_eq!(work_head(&place).await, built);
}

// An agent's merge of main that resolved a conflict holds a change of the work's own: the head moves to it.
#[tokio::test]
async fn an_agents_merge_that_resolved_a_conflict_is_a_move_of_the_head() {
    let (repo, place, _) = built().await;
    commit(&place.dir, "greeting.txt", "Hello, you.\n");
    commit(&repo.project.repo, "greeting.txt", "Hello from main.\n");
    assert!(!agent_merges(&place).status.success(), "the merge meets the conflict");
    resolve(&place, "greeting.txt", "Hello, you, from main.\n");
    let (resolved, _, _) = head_of(&place);
    assert_eq!(work_head(&place).await, resolved);
}

// Between two heads that hold the same main, the round is given their diff; where the later holds a later main, the
// diff from the clean merge of the earlier with that main, which shows the work's own change alone; and where the
// earlier does not merge with that main, the whole change against main.
#[tokio::test]
async fn between_two_heads_the_round_is_given_what_the_work_itself_changed() {
    let (repo, place, first) = built().await;
    let second = commit(&place.dir, "farewell.txt", "Goodbye.\n");
    let command = between(&place.dir, MAIN, &first, &second).await.unwrap();
    assert_eq!(command, format!("`git diff {first} {second}`"));
    // Main moves, is taken in, and the work commits again: what main brought is left out.
    commit(&repo.project.repo, "other.txt", "Good day.\n");
    catch_up::take_in(&repo.project, &place, MAIN, RunId(7), WHOSE).await;
    let third = commit(&place.dir, "weather.txt", "Sunny.\n");
    let command = between(&place.dir, MAIN, &second, &third).await.unwrap();
    let from = command
        .strip_prefix("`git diff ")
        .and_then(|rest| rest.strip_suffix(&format!(" {third}`")))
        .unwrap_or_else(|| panic!("{command}"));
    let shown = run(&place.dir, &["diff", "--name-only", from, &third]);
    assert_eq!(shown, "weather.txt", "{command}");
    // The work changes what main changes, and main changes it again: the earlier head does not merge with main, and
    // the whole change against main is given.
    commit(&repo.project.repo, "farewell.txt", "Farewell, from main.\n");
    assert!(!agent_merges(&place).status.success(), "the merge meets the conflict");
    resolve(&place, "farewell.txt", "Farewell, from main, and bye.\n");
    let (fourth, _, _) = head_of(&place);
    let main = run(&repo.project.repo, &["rev-parse", "main"]);
    let command = between(&place.dir, MAIN, &third, &fourth).await.unwrap();
    assert_eq!(command, format!("`git diff {main} {fourth}`"));
}
