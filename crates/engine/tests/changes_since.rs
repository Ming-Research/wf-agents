//! Adversarial tests of `engine::git::changes_since`, the one definition of "since" that the inputs of adversarial
//! testing and of both reviews rest on, against real git repositories. Their reference is git itself: what `git log`
//! and `git diff` print for the same commits, and git's own rule that a merge is a commit with more than one parent.

use std::path::{Path, PathBuf};
use std::process::Command;

use engine::git::changes_since;

/// A new repository whose first branch is `main`, in a directory of its own to the test.
fn fresh(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("wf-agents-since-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    git(&dir, &["init", "-q"]);
    dir
}

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=t", "-c", "user.email=t@t"])
        .args(["-c", "init.defaultBranch=main", "-c", "commit.gpgsign=false"])
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// Main as the work's clone has it fetched (`origin/main`), at the commit main is at now: the work's own files are told
/// from the files main brought by it.
fn fetched_main(dir: &Path) {
    git(dir, &["update-ref", "refs/remotes/origin/main", "main"]);
}

/// Writes the files, commits them with the message, and gives the new commit.
fn commit(dir: &Path, files: &[(&str, &str)], message: &str) -> String {
    for (file, text) in files {
        std::fs::write(dir.join(file), text).unwrap();
    }
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "--allow-empty-message", "-m", message]);
    git(dir, &["rev-parse", "HEAD"])
}

// A branch that takes main in twice, the second time as the web page's "update branch" does (a merge commit with the
// branch as its first parent and a message of its own): both merges are marked, newest first, in the commits from a
// base before them and from main itself. A file that only main changed is not the work's own, in either merge; one
// that main and the work both changed stays; and the commits the merges brought in are not listed.
#[test]
fn two_merges_of_main_are_marked_newest_first_and_only_what_the_work_changed_is_its_own() {
    let dir = fresh("two-merges");
    // A file of nine lines, which main changes at the top and the work at the bottom, so that they merge cleanly.
    let nine = |first: &str, last: &str| format!("{first}\nb\nc\nd\ne\nf\ng\nh\n{last}\n");
    commit(
        &dir,
        &[
            ("shared.txt", "s\n"),
            ("both.txt", &nine("a", "i")),
            ("main1.txt", "1\n"),
            ("main2.txt", "2\n"),
        ],
        "Start",
    );
    git(&dir, &["checkout", "-q", "-b", "work"]);
    let w1 = commit(&dir, &[("shared.txt", "work 1\n")], "First");
    git(&dir, &["checkout", "-q", "main"]);
    commit(
        &dir,
        &[("main1.txt", "changed\n"), ("both.txt", &nine("A", "i"))],
        "Main's first",
    );
    git(&dir, &["update-ref", "refs/remotes/origin/main", "main"]);
    git(&dir, &["checkout", "-q", "work"]);
    let w2 = commit(&dir, &[("both.txt", &nine("a", "I"))], "Second");
    git(
        &dir,
        &[
            "merge",
            "-q",
            "--no-ff",
            "-m",
            "Merge branch 'main' into work",
            "origin/main",
        ],
    );
    let merge1 = git(&dir, &["rev-parse", "HEAD"]);
    git(&dir, &["checkout", "-q", "main"]);
    commit(&dir, &[("main2.txt", "changed\n")], "Main's second");
    git(&dir, &["update-ref", "refs/remotes/origin/main", "main"]);
    git(&dir, &["checkout", "-q", "work"]);
    let w3 = commit(&dir, &[("own3.txt", "3\n")], "Third");
    git(
        &dir,
        &[
            "merge",
            "-q",
            "--no-ff",
            "-m",
            "Merge branch 'main' into work",
            "origin/main",
        ],
    );
    let merge2 = git(&dir, &["rev-parse", "HEAD"]);

    let since = changes_since(&dir, "main", &w1, "HEAD").unwrap();
    let ids: Vec<(&str, bool)> = since.commits.iter().map(|c| (c.id.as_str(), c.merge)).collect();
    assert_eq!(
        ids,
        [
            (merge2.as_str(), true),
            (w3.as_str(), false),
            (merge1.as_str(), true),
            (w2.as_str(), false)
        ],
        "{since:?}"
    );
    let merges: Vec<&str> = since.merges.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(merges, [merge2.as_str(), merge1.as_str()]);
    assert_eq!(since.merges[0].subject, "Merge branch 'main' into work");
    // main1.txt and main2.txt came in with the merges; both.txt the work changed too.
    assert_eq!(since.own_files, ["both.txt", "own3.txt"], "{since:?}");
    assert!(!since.diverged);

    // From main: the work's three commits and its two merges, none of main's own commits.
    let from_main = changes_since(&dir, "main", "origin/main", "HEAD").unwrap();
    let ids: Vec<&str> = from_main.commits.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids.len(), 5, "{from_main:?}");
    assert_eq!(ids[0], merge2);
    assert_eq!(from_main.merges.len(), 2);
    assert_eq!(
        from_main.own_files,
        ["both.txt", "own3.txt", "shared.txt"],
        "{from_main:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// The commits' messages are read as git prints them: a subject with a tab in it, one with a percent sign and one that
// is empty keep their words, and a file name with a space is one file. (The reference is `git log --format=%s`.)
#[test]
fn a_subject_with_a_tab_or_nothing_and_a_name_with_a_space_are_read_whole() {
    let dir = fresh("awkward");
    let start = commit(&dir, &[("a.txt", "a\n")], "Start");
    fetched_main(&dir);
    let tabbed = commit(&dir, &[("with space.txt", "1\n")], "Fix\tthe tab: 100% done");
    let empty = commit(&dir, &[("b.txt", "2\n")], "");
    let since = changes_since(&dir, "main", &start, "HEAD").unwrap();
    assert_eq!(since.commits.len(), 2, "{since:?}");
    assert_eq!(since.commits[0].id, empty);
    assert_eq!(
        since.commits[0].subject,
        git(&dir, &["log", "-1", "--format=%s", &empty])
    );
    assert_eq!(since.commits[0].subject, "");
    assert_eq!(since.commits[1].id, tabbed);
    assert_eq!(
        since.commits[1].subject,
        git(&dir, &["log", "-1", "--format=%s", &tabbed])
    );
    assert_eq!(since.commits[1].subject, "Fix\tthe tab: 100% done");
    assert_eq!(since.own_files, ["b.txt", "with space.txt"]);
    let _ = std::fs::remove_dir_all(&dir);
}

// A file whose name is not plain ASCII is given by its name, not by the quoted, escaped form git prints it in by
// default (`"caf\303\251.txt"`), which no agent could open or compare with a path.
#[test]
fn a_file_with_a_name_beyond_ascii_is_named_as_it_is() {
    let dir = fresh("unicode");
    let start = commit(&dir, &[("a.txt", "a\n")], "Start");
    fetched_main(&dir);
    // The names are written as escapes, because the repository is all ASCII (AGENTS.md): a Latin letter with an
    // accent, and three characters of Japanese.
    let accent = "caf\u{e9}.txt";
    let japanese = "\u{65e5}\u{672c}\u{8a9e}.txt";
    commit(&dir, &[(accent, "1\n"), (japanese, "2\n")], "Names");
    let since = changes_since(&dir, "main", &start, "HEAD").unwrap();
    assert_eq!(since.own_files, [accent, japanese], "{since:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

// A base that names no commit is an error, whatever it looks like, and never an empty answer that reads as "nothing
// changed": an empty name (which git's range `..HEAD` would read as the head itself), a name git reads as an option,
// and a branch or a tag name that does exist is a base like any commit.
#[test]
fn a_base_that_is_empty_or_names_nothing_is_an_error_and_a_tag_or_a_branch_is_a_base() {
    let dir = fresh("bases");
    let start = commit(&dir, &[("a.txt", "a\n")], "Start");
    fetched_main(&dir);
    git(&dir, &["tag", "-a", "-m", "release", "v1"]);
    git(&dir, &["branch", "old"]);
    let second = commit(&dir, &[("b.txt", "b\n")], "Second");
    for base in ["", "--all", "-1", "nothing", "HEAD~9"] {
        let got = changes_since(&dir, "main", base, "HEAD");
        assert!(got.is_err(), "base {base:?} gave {got:?}");
    }
    for base in ["v1", "old"] {
        let got = changes_since(&dir, "main", base, "HEAD").unwrap();
        assert_eq!(got.commits.len(), 1, "{base}: {got:?}");
        assert_eq!(got.commits[0].id, second);
        assert_eq!(got.own_files, ["b.txt"]);
        assert!(!got.diverged);
    }
    let from_start = changes_since(&dir, "main", &start, "HEAD").unwrap();
    assert_eq!(from_start.commits.len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}
