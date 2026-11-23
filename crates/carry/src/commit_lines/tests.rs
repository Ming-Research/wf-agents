//! The two lines every commit of a work ends with: the hook the agents' git runs, with the real git, and the message of
//! the squash that merges a work.

use std::path::Path;

use super::*;
use crate::git::tests::{Repo, run};

/// The item and the work the tests' commits name.
const WHOSE: Whose = Whose {
    item: ItemNumber(36),
    work: WorkNumber(12),
};

/// The words the engine writes, as the flows of this repository say them.
fn words() -> Commits {
    crate::acts::writes().commits
}

/// Runs git in the directory with the environment of a round given beside the test's own, and gives its output.
fn run_with(dir: &Path, args: &[&str], environment: &[(String, String)]) -> String {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .envs(environment.iter().cloned())
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim_end().to_string()
}

/// The message of the head, as git keeps it, without its last line break.
fn message(dir: &Path) -> String {
    run(dir, &["log", "-1", "--format=%B"])
}

/// A repository whose work has the hook of the engine in its data directory, and what a round of the work is given.
fn hooked(whose: Whose) -> (Repo, Vec<(String, String)>) {
    let repo = Repo::new("true");
    install(&repo.data).unwrap();
    let given = environment(&repo.data, &words(), whose);
    (repo, given)
}

/// Writes the file in the directory and stages it.
fn staged(dir: &Path, file: &str) {
    std::fs::write(dir.join(file), format!("{file}\n")).unwrap();
    run(dir, &["add", file]);
}

// The agents commit as they like, and the engine cannot ask them to write the lines: git runs the hook for each way a
// message is made, `--no-verify` and `--amend` included, and the lines are there once.
#[test]
fn a_commit_the_agents_git_makes_ends_with_the_two_lines_however_it_is_made_and_once() {
    let (repo, given) = hooked(WHOSE);
    let dir = &repo.project.repo;
    let ends = "\n\nItem: 36\nWork: W12";

    staged(dir, "a.txt");
    run_with(dir, &["commit", "-q", "-m", "First", "-m", "Its body."], &given);
    assert_eq!(message(dir), format!("First\n\nIts body.{ends}"));

    staged(dir, "b.txt");
    run_with(dir, &["commit", "-q", "--no-verify", "-m", "Second"], &given);
    assert_eq!(message(dir), format!("Second{ends}"));

    run_with(dir, &["commit", "-q", "--amend", "--no-edit"], &given);
    assert_eq!(message(dir), format!("Second{ends}"), "an amend adds nothing twice");

    run(dir, &["checkout", "-q", "-b", "side"]);
    staged(dir, "c.txt");
    run_with(dir, &["commit", "-q", "-m", "Third"], &given);
    run(dir, &["checkout", "-q", "main"]);
    staged(dir, "d.txt");
    run_with(dir, &["commit", "-q", "-m", "Fourth"], &given);
    run_with(dir, &["merge", "-q", "--no-ff", "-m", "Merge side", "side"], &given);
    assert_eq!(message(dir), format!("Merge side{ends}"));
}

// Without the lines in its environment the hook does nothing, so that a commit made by hand, with the hook's directory
// still configured, is not given lines that name nothing.
#[test]
fn the_hook_adds_nothing_where_it_is_given_no_lines() {
    let (repo, given) = hooked(WHOSE);
    let dir = &repo.project.repo;
    let configured: Vec<(String, String)> = given
        .into_iter()
        .filter(|(name, _)| name.starts_with("GIT_CONFIG"))
        .collect();
    assert_eq!(configured.len(), 3);
    staged(dir, "a.txt");
    run_with(dir, &["commit", "-q", "-m", "Alone"], &configured);
    assert_eq!(message(dir), "Alone");
}

// The two lines are found by `git log --grep='^Item: 36$'`, which must not find the commits of item 360.
#[test]
fn the_lines_find_the_commits_of_one_item_and_not_those_of_another_whose_number_begins_the_same() {
    let repo = Repo::new("true");
    install(&repo.data).unwrap();
    let dir = &repo.project.repo;
    for (file, item, work) in [("a.txt", 36, 12), ("b.txt", 360, 120), ("c.txt", 36, 12)] {
        let whose = Whose {
            item: ItemNumber(item),
            work: WorkNumber(work),
        };
        staged(dir, file);
        let given = environment(&repo.data, &words(), whose);
        run_with(dir, &["commit", "-q", "-m", &format!("Write {file}")], &given);
    }
    let found = |pattern: &str| run(dir, &["log", &format!("--grep={pattern}"), "--format=%s"]);
    assert_eq!(found("^Item: 36$"), "Write c.txt\nWrite a.txt");
    assert_eq!(found("^Item: 360$"), "Write b.txt");
    assert_eq!(found("^Work: W12$"), "Write c.txt\nWrite a.txt");
    // Without the anchors the shorter number finds the longer one's commit too: the lines are searched whole.
    assert!(found("Item: 36").contains("Write b.txt"));
}

#[test]
fn the_hook_is_written_whole_executable_and_over_one_an_older_engine_left() {
    let data = tempfile::tempdir().unwrap();
    let file = directory(data.path()).join(HOOK);
    install(data.path()).unwrap();
    assert_eq!(std::fs::read_to_string(&file).unwrap(), SCRIPT);
    assert!(executable(&file));
    assert!(!directory(data.path()).join("prepare-commit-msg.part").exists());

    // What an older engine left, or what was made unexecutable, is made again.
    std::fs::write(&file, "#!/bin/sh\nexit 0\n").unwrap();
    install(data.path()).unwrap();
    assert_eq!(std::fs::read_to_string(&file).unwrap(), SCRIPT);
    assert!(executable(&file));
}

#[test]
fn a_rounds_environment_gives_git_the_hook_directory_and_the_two_lines_filled_with_the_numbers() {
    let data = Path::new("/data");
    assert_eq!(
        environment(data, &words(), WHOSE),
        [
            ("GIT_CONFIG_COUNT", "1"),
            ("GIT_CONFIG_KEY_0", "core.hooksPath"),
            ("GIT_CONFIG_VALUE_0", "/data/hooks"),
            ("WF_ITEM_LINE", "Item: 36"),
            ("WF_WORK_LINE", "Work: W12"),
        ]
        .map(|(name, value)| (name.to_string(), value.to_string()))
    );
}

/// The squash message of these commits' messages, tidied by `tidy`.
fn squash(messages: &[&str], tidy: &dyn Fn(&str) -> String) -> String {
    let messages: Vec<String> = messages.iter().map(|m| m.to_string()).collect();
    squashed(&words(), WHOSE, &messages, tidy)
}

fn as_it_is(text: &str) -> String {
    text.to_string()
}

#[test]
fn the_squash_of_one_commit_says_its_body_and_ends_with_the_lines_once() {
    let said = squash(
        &["Make it so\n\nThe body.\nMore of it.\n\nItem: 36\nWork: W12"],
        &as_it_is,
    );
    assert_eq!(said, "The body.\nMore of it.\n\nItem: 36\nWork: W12");
}

#[test]
fn the_squash_of_a_commit_with_no_body_is_the_lines_alone() {
    assert_eq!(squash(&["Make it so"], &as_it_is), "Item: 36\nWork: W12");
    assert_eq!(squash(&[], &as_it_is), "Item: 36\nWork: W12");
}

#[test]
fn the_squash_of_several_commits_gives_each_its_first_line_after_a_star_and_then_its_body() {
    let said = squash(
        &[
            "First\n\nWhy it was first.\n\nItem: 36\nWork: W12",
            "Second\n\nItem: 36\nWork: W12",
            "Third\n\nAnd why.\n\nItem: 36\nWork: W12",
        ],
        &as_it_is,
    );
    assert_eq!(
        said,
        "* First\n\nWhy it was first.\n\n* Second\n\n* Third\n\nAnd why.\n\nItem: 36\nWork: W12"
    );
}

// The host reads the co-authors of a squash from the end of its message, once each, and the commits' own two lines are
// in the commits' bodies no more.
#[test]
fn the_squash_gathers_the_co_authors_once_before_the_lines_and_keeps_none_in_the_bodies() {
    let said = squash(
        &[
            "First\n\nWhat was done.\n\nCo-Authored-By: Claude <noreply@anthropic.com>\nItem: 36\nWork: W12",
            "Second\n\nMore.\n\nCo-authored-by: Claude <noreply@anthropic.com>\n\
             Co-authored-by: Ada <ada@example.org>\nItem: 36\nWork: W12",
        ],
        &as_it_is,
    );
    assert_eq!(
        said,
        "* First\n\nWhat was done.\n\n* Second\n\nMore.\n\n\
         Co-authored-by: Claude <noreply@anthropic.com>\nCo-authored-by: Ada <ada@example.org>\nItem: 36\nWork: W12"
    );
}

// What the commits' agents wrote is made fit to publish piece by piece, and the lines the engine adds are not touched.
#[test]
fn the_squash_gives_each_piece_the_agents_wrote_to_tidy_and_not_the_lines() {
    let shout = |text: &str| text.to_uppercase();
    let said = squash(&["first\n\nbody", "second"], &shout);
    assert_eq!(said, "* FIRST\n\nBODY\n\n* SECOND\n\nItem: 36\nWork: W12");
    let one = squash(&["first\n\nbody"], &shout);
    assert_eq!(one, "BODY\n\nItem: 36\nWork: W12");
}
