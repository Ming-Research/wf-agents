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

// The agents' commits usually end with a `Co-Authored-By` line: git puts the two lines in that block, with no blank
// line between, and the squash message is built from the message git made, not from one written by hand.
#[test]
fn a_commit_that_already_ends_in_a_trailer_gets_the_lines_in_the_same_block_and_the_squash_reads_it() {
    let (repo, given) = hooked(WHOSE);
    let dir = &repo.project.repo;
    staged(dir, "a.txt");
    let co_author = "Co-Authored-By: Claude <noreply@anthropic.com>";
    run_with(
        dir,
        &["commit", "-q", "-m", "First", "-m", "What was done.", "-m", co_author],
        &given,
    );
    let made = message(dir);
    assert_eq!(
        made,
        format!("First\n\nWhat was done.\n\n{co_author}\nItem: 36\nWork: W12")
    );
    let said = squash(&[made.as_str()], &as_it_is);
    assert_eq!(
        said,
        "What was done.\n\nCo-authored-by: Claude <noreply@anthropic.com>\nItem: 36\nWork: W12"
    );
}

// A commit leads back to one item: where its message already ends with the lines of another work, because it was taken
// from that work's branch or the agent copied them, they are replaced and not added to, so that `--grep='^Item: 20$'`
// does not find a commit of this work. Cherry-pick runs the hook, as `commit` does.
#[test]
fn a_commit_taken_from_another_work_or_written_with_its_lines_ends_with_this_works_lines_alone() {
    let (repo, given) = hooked(WHOSE);
    let dir = &repo.project.repo;
    let other = environment(
        &repo.data,
        &words(),
        Whose {
            item: ItemNumber(20),
            work: WorkNumber(3),
        },
    );
    run(dir, &["checkout", "-q", "-b", "other"]);
    staged(dir, "a.txt");
    run_with(dir, &["commit", "-q", "-m", "From another", "-m", "Why."], &other);
    assert_eq!(message(dir), "From another\n\nWhy.\n\nItem: 20\nWork: W3");
    run(dir, &["checkout", "-q", "main"]);
    run_with(dir, &["cherry-pick", "other"], &given);
    assert_eq!(message(dir), "From another\n\nWhy.\n\nItem: 36\nWork: W12");

    staged(dir, "b.txt");
    let said = "Co-Authored-By: Claude <noreply@anthropic.com>\nItem: 20\nWork: W3";
    run_with(dir, &["commit", "-q", "-m", "Copied", "-m", "Why.", "-m", said], &given);
    let ends = "Co-Authored-By: Claude <noreply@anthropic.com>\nItem: 36\nWork: W12";
    assert_eq!(message(dir), format!("Copied\n\nWhy.\n\n{ends}"));
    run_with(dir, &["commit", "-q", "--amend", "--no-edit"], &given);
    assert_eq!(message(dir), format!("Copied\n\nWhy.\n\n{ends}"));
    assert_eq!(run(dir, &["log", "--grep=^Item: 20$", "--format=%s", "main"]), "");
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

// Reference: the numbers as the engine writes them (`ItemNumber` bare in a commit's line, `WorkNumber` as `W12`): a line
// of a commit's text that has the form of one of the lines but not such a number is the agents' own text, and is kept.
#[test]
fn the_squash_keeps_a_line_that_has_the_form_of_one_of_the_lines_but_no_number_of_its_kind() {
    let said = squash(
        &["Make it so\n\nItem: W3\nWork: 3\nItem: #3\nItem: x\nWork: W3x"],
        &as_it_is,
    );
    assert_eq!(
        said,
        "Item: W3\nWork: 3\nItem: #3\nItem: x\nWork: W3x\n\nItem: 36\nWork: W12"
    );
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

/// Whether the message holds each of the two lines once, whatever else it holds.
fn names_once(message: &str) -> bool {
    let count = |line: &str| message.lines().filter(|said| said.trim_end() == line).count();
    count("Item: 36") == 1 && count("Work: W12") == 1
}

// Reference: git-interpret-trailers(1) for where the lines go and git-commit(1) for the ways a message is made. The
// agents also write a first line that looks like a trailer, read a message from a file or an editor, revert, and
// resolve a conflicted merge by `git commit`; each of those messages holds the lines once, and `git log --grep` finds it.
#[test]
fn a_message_made_from_a_file_an_editor_a_revert_or_a_resolved_conflict_holds_the_lines_once() {
    let (repo, mut given) = hooked(WHOSE);
    let dir = &repo.project.repo;
    let grep = |pattern: &str| {
        run(dir, &["log", &format!("--grep={pattern}"), "--format=%H"])
            .lines()
            .count()
    };

    // A first line of the form `key: value` is the subject, not a trailer: the lines are set apart from it.
    staged(dir, "a.txt");
    run_with(dir, &["commit", "-q", "-m", "Fix: the greeting"], &given);
    assert_eq!(message(dir), "Fix: the greeting\n\nItem: 36\nWork: W12");

    // A message read from a file, whose last paragraph reads as a trailer.
    staged(dir, "b.txt");
    let file = dir.join("message.txt");
    std::fs::write(&file, "Subject\n\nBody.\n\nNote: this matters\n").unwrap();
    let path = file.to_string_lossy().into_owned();
    run_with(dir, &["commit", "-q", "-F", &path], &given);
    assert!(names_once(&message(dir)), "{}", message(dir));
    assert!(message(dir).ends_with("Item: 36\nWork: W12"), "{}", message(dir));

    // The same message edited by an editor, which leaves it as it is.
    staged(dir, "c.txt");
    given.push(("GIT_EDITOR".to_string(), "true".to_string()));
    run_with(dir, &["commit", "-q", "-e", "-F", &path], &given);
    assert!(names_once(&message(dir)), "{}", message(dir));
    assert!(message(dir).ends_with("Item: 36\nWork: W12"), "{}", message(dir));

    // A revert writes a message of its own, with a line of git's after the subject.
    run_with(dir, &["revert", "--no-edit", "HEAD"], &given);
    assert!(names_once(&message(dir)), "{}", message(dir));

    // A merge that met a conflict, resolved and committed with the message git prepared, whose end is git's own list of
    // the conflicts.
    run(dir, &["checkout", "-q", "-b", "side", "HEAD~4"]);
    std::fs::write(dir.join("greeting.txt"), "Side.\n").unwrap();
    run(dir, &["commit", "-qam", "Side"]);
    run(dir, &["checkout", "-q", "main"]);
    std::fs::write(dir.join("greeting.txt"), "Main.\n").unwrap();
    run(dir, &["commit", "-qam", "Main"]);
    let merging = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["merge", "side"])
        .envs(given.iter().cloned())
        .output()
        .unwrap();
    assert!(!merging.status.success(), "the merge meets the conflict");
    std::fs::write(dir.join("greeting.txt"), "Both.\n").unwrap();
    run(dir, &["add", "greeting.txt"]);
    run_with(dir, &["commit", "-q", "--no-edit"], &given);
    assert!(names_once(&message(dir)), "{}", message(dir));

    // Each of the five commits made with the lines is found by the item's line, and no other is.
    assert_eq!(grep("^Item: 36$"), 5);
}

// The data directory of an installation may have a space in its path, which the hook's directory is given through
// git's variables with, and a work is done in a linked worktree made by the engine, not in the repository itself.
#[tokio::test]
async fn a_commit_in_a_works_linked_worktree_ends_with_the_lines_where_the_data_directory_has_a_space() {
    let repo = Repo::new("true");
    let spaced = repo.data.parent().unwrap().join("a data directory");
    std::fs::create_dir_all(&spaced).unwrap();
    install(&spaced).unwrap();
    let started = crate::git::start(&repo.project, &repo.place(WorkNumber(12)), "main", "main")
        .await
        .unwrap();
    let dir = &started.place.dir;
    assert_ne!(dir, &repo.project.repo, "the work is in a linked worktree");
    let given = environment(&spaced, &words(), WHOSE);
    staged(dir, "a.txt");
    run_with(dir, &["commit", "-q", "-m", "In the worktree"], &given);
    assert_eq!(message(dir), "In the worktree\n\nItem: 36\nWork: W12");
}

/// The text with each `#` and `@` broken as `neutral` breaks them, which is how the squash makes what an agent wrote fit
/// to publish.
fn published(text: &str) -> String {
    crate::host::github::neutral(text, &[])
}

// Reference: the contract of `neutral` ("no mention notifies anyone, no reference names an issue or a pull request" in
// what an agent wrote and the repository publishes) and the design principle agent-writing-is-data. A `Co-authored-by`
// line is what an agent wrote: a name that is a mention or a reference must not reach the squash message as one, while
// a real co-author, whose email has an `@`, is still read by the host.
#[test]
fn the_squash_publishes_no_mention_or_reference_that_an_agent_put_in_a_co_author_line() {
    let said = squash(
        &[
            "First\n\nWhat was done.\n\nCo-authored-by: Claude <noreply@anthropic.com>\n\
             Co-authored-by: Mallory <m@example.org> thanks @victim, see #7",
        ],
        &published,
    );
    assert!(
        said.contains("Co-authored-by: Claude <noreply@anthropic.com>"),
        "{said}"
    );
    assert!(!said.contains("@victim"), "a mention reaches the host as one: {said}");
    assert!(!said.contains("#7"), "a reference reaches the host as one: {said}");
    assert!(said.ends_with("Item: 36\nWork: W12"), "{said}");
}

// Reference: the item's own requirement, that a commit on main leads back to one item, found by `git log --grep`: a
// commit of the work that carries the lines of another work (one copied from another work's branch or from main, which
// ends with that work's lines) must not make the work's squash say two items.
#[test]
fn the_squash_says_one_item_and_one_work_where_a_commit_of_the_work_carries_the_lines_of_another() {
    let said = squash(
        &[
            "Copied\n\nWhy.\n\nItem: 20\nWork: W3\nItem: 36\nWork: W12",
            "Own\n\nItem: 36\nWork: W12",
        ],
        &as_it_is,
    );
    let count = |key: &str| said.lines().filter(|line| line.starts_with(key)).count();
    assert_eq!(count("Item: "), 1, "{said}");
    assert_eq!(count("Work: "), 1, "{said}");
    assert!(said.ends_with("Item: 36\nWork: W12"), "{said}");
}

// Reference: the form the host reads a co-author in, `Co-authored-by: Name <email>` (GitHub's "Creating a commit with
// multiple authors"). Each line below has one fault of that form, so each is told apart from a co-author by its own
// condition: it is no co-author, stays in the commit's text through `tidy`, and the end of the message holds none.
#[test]
fn a_line_that_is_not_exactly_a_name_and_an_email_stays_in_the_text_and_is_no_co_author() {
    let faulty = [
        "Co-authored-by: Ada <a#1@example.org>",
        "Co-authored-by: Ada <ada.example.org>",
        "Co-authored-by: Ada <a@b@example.org>",
        "Co-authored-by: Ada <@example.org>",
        "Co-authored-by: Ada <ada@>",
        "Co-authored-by: <ada@example.org>",
        "Co-authored-by: A>da <ada@example.org>",
        "Co-authored-by: Ada <ada@example.org> thanks",
        "Co-authored-by: Ada Lovelace",
    ];
    for line in faulty {
        let message = format!("First\n\nBody.\n\n{line}");
        let said = squash(&[message.as_str()], &published);
        let text = published(&format!("Body.\n\n{line}"));
        assert_eq!(said, format!("{text}\n\nItem: 36\nWork: W12"), "{line}");
    }
}

// A name that `tidy` cuts short is followed by a line of its own, which no co-author line has.
#[test]
fn a_name_that_tidy_cuts_short_makes_the_line_no_co_author() {
    let cutting = |text: &str| format!("{text}\n(cut)");
    let said = squash(&["First\n\nBody.\n\nCo-authored-by: Ada <ada@example.org>"], &cutting);
    assert_eq!(
        said,
        "Body.\n\nCo-authored-by: Ada <ada@example.org>\n(cut)\n\nItem: 36\nWork: W12"
    );
}

// A real co-author whose name holds a mention or a closing word still reaches the host as one, with the name broken and
// the email, which the host reads by its `@`, as it was written.
#[test]
fn a_co_author_whose_name_holds_a_mention_is_kept_with_the_name_made_neutral_and_the_email_as_it_is() {
    let said = squash(
        &["First\n\nBody.\n\nCo-authored-by: @victim Fixes <v@example.org>"],
        &published,
    );
    let name = published("@victim Fixes");
    assert_ne!(name, "@victim Fixes");
    assert_eq!(
        said,
        format!("Body.\n\nCo-authored-by: {name} <v@example.org>\nItem: 36\nWork: W12")
    );
}
