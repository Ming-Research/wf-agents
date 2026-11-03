//! The text `engine::git::own_lines` gives for a commit, tried against changes of the work's own that a text with no
//! line numbers and no blob ids could fail to tell apart. Each case builds a small repository: `main` with the
//! fetched `origin/main` at it, and a branch `work` forked from it.

use std::path::{Path, PathBuf};

use engine::git::{git, own_lines};

fn run(dir: &Path, args: &[&str]) -> String {
    let who = [
        "-c",
        "user.name=t",
        "-c",
        "user.email=t@t",
        "-c",
        "init.defaultBranch=main",
    ];
    git(dir, &[&who[..], args].concat()).unwrap().trim().to_string()
}

/// Writes `files`, commits all, and gives the commit.
fn commit(dir: &Path, files: &[(&str, &str)], message: &str) -> String {
    for (f, text) in files {
        std::fs::write(dir.join(f), text).unwrap();
    }
    run(dir, &["add", "-A"]);
    run(dir, &["commit", "-q", "-m", message]);
    run(dir, &["rev-parse", "HEAD"])
}

/// A repository with `files` on main, `origin/main` at it, and the branch `work` checked out.
fn forked(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("wf-agents-adversarial-lines-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    run(&dir, &["init", "-q"]);
    commit(&dir, files, "Start");
    run(&dir, &["update-ref", "refs/remotes/origin/main", "main"]);
    run(&dir, &["checkout", "-q", "-b", "work"]);
    dir
}

/// Fails unless the work's own lines at `before` and at HEAD differ, and the work had some at `before`.
fn must_differ(case: &str, dir: &Path, before: &str) {
    let was = own_lines(dir, "main", before).unwrap();
    let now = own_lines(dir, "main", "HEAD").unwrap();
    assert!(!was.is_empty(), "{case}: the work has no lines of its own at the start");
    assert_ne!(
        String::from_utf8_lossy(&was),
        String::from_utf8_lossy(&now),
        "{case}: the work's own change is another, but its own lines read the same"
    );
    let _ = std::fs::remove_dir_all(dir);
}

// The work's change is where it is made, not only what is written: the same line added in another function, or the
// same line removed from the other of two identical places, is another change, though the text without line numbers
// and function names has the same lines. The reference is the file itself: `git diff` between the two commits is not
// empty and the two versions of the file differ.
#[test]
fn the_same_line_written_or_removed_in_another_place_is_another_change() {
    let two = |a: &str, b: &str| format!("fn a() {{\n  x;\n{a}}}\nfn b() {{\n  y;\n{b}}}\n");
    // Added in `a`, then in `b`.
    let dir = forked("moved-add", &[("f.rs", two("", "").as_str())]);
    let before = commit(&dir, &[("f.rs", two("  return 1;\n", "").as_str())], "The work");
    commit(&dir, &[("f.rs", two("", "  return 1;\n").as_str())], "Moves it");
    assert!(!run(&dir, &["diff", &before, "HEAD"]).is_empty());
    must_differ("a line added in another function", &dir, &before);

    // Removed from `a`, then from `b`.
    let dir = forked("moved-remove", &[("f.rs", two("  log();\n", "  log();\n").as_str())]);
    let before = commit(&dir, &[("f.rs", two("", "  log();\n").as_str())], "The work");
    commit(&dir, &[("f.rs", two("  log();\n", "").as_str())], "Removes the other");
    assert!(!run(&dir, &["diff", &before, "HEAD"]).is_empty());
    must_differ("a line removed from the other of two equal places", &dir, &before);
}

// Changes of the work that are not a line of text: a file's mode, whether its last line ends with a newline, a file
// the work deletes, and an empty file it adds. Each is a change of what the work adds or removes.
#[test]
fn a_mode_a_missing_newline_a_deleted_file_and_an_empty_file_are_the_works_own_changes() {
    // A file made executable.
    let dir = forked("mode", &[("a.txt", "a\n"), ("run.sh", "echo\n")]);
    let before = commit(&dir, &[("a.txt", "a\nb\n")], "The work");
    run(&dir, &["update-index", "--chmod=+x", "run.sh"]);
    run(&dir, &["commit", "-q", "-m", "Executable"]);
    must_differ("mode", &dir, &before);

    // The last line given its newline.
    let dir = forked("newline", &[("a.txt", "a\n")]);
    let before = commit(&dir, &[("a.txt", "a\nb")], "The work");
    commit(&dir, &[("a.txt", "a\nb\n")], "Newline");
    must_differ("newline at the end", &dir, &before);

    // A file of main's deleted, besides the work's other change.
    let dir = forked("deleted", &[("a.txt", "a\n"), ("b.txt", "b\n")]);
    let before = commit(&dir, &[("a.txt", "a\nb\n")], "The work");
    run(&dir, &["rm", "-q", "b.txt"]);
    run(&dir, &["commit", "-q", "-m", "Deletes b"]);
    must_differ("file deleted", &dir, &before);

    // An empty file added: no line, only the file.
    let dir = forked("empty", &[("a.txt", "a\n")]);
    let before = commit(&dir, &[("a.txt", "a\nb\n")], "The work");
    commit(&dir, &[("empty.txt", "")], "Adds an empty file");
    must_differ("empty file added", &dir, &before);
}
