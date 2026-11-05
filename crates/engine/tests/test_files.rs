//! The test files the work changed, which adversarial testing is given as `test_files` of the tests the engineer
//! changed: the files `engine::git::own_changes` gives that are `engine::git::under` the project's test paths, tried
//! against real git repositories. Their reference is git itself, what `git diff --name-status --no-renames` prints
//! for the same commits, and the rule `under` documents: a directory written with a trailing slash holds what is below
//! it, `*` matches any run of characters within one segment of a path, and a segment `**` any number of segments.

use std::path::{Path, PathBuf};
use std::process::Command;

use engine::git::{fork_point, own_changes, under};

/// A new repository whose first branch is `main`, in a directory of its own to the test.
fn fresh(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("wf-agents-test-files-{name}-{}", std::process::id()));
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

/// Writes `files`, commits all, and gives the commit.
fn commit(dir: &Path, files: &[(&str, &str)], message: &str) -> String {
    for (file, text) in files {
        let at = dir.join(file);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(at, text).unwrap();
    }
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", message]);
    git(dir, &["rev-parse", "HEAD"])
}

/// Main as the work's clone has it fetched (`origin/main`), at the commit main is at now.
fn fetched_main(dir: &Path) {
    git(dir, &["update-ref", "refs/remotes/origin/main", "main"]);
}

/// The files of `own_changes` from the fork point to the head that are under the test paths, as the engine gives them.
fn test_files(dir: &Path, test_paths: &[&str]) -> Vec<String> {
    let paths: Vec<String> = test_paths.iter().map(|p| p.to_string()).collect();
    let fork = fork_point(dir, "main").unwrap();
    let own = own_changes(dir, "main", &fork, "HEAD").unwrap();
    own.into_iter().filter(|f| under(f, &paths)).collect()
}

// The test paths this project's configuration writes, tried against the paths of its own tests: `**` stands for no
// segment as well as for several, `*` for one segment of a name only, and a directory holds what is below it and not
// itself. A path that holds no test is not under them, however much it looks like one.
#[test]
fn the_test_paths_of_this_project_pick_the_files_of_its_tests() {
    let paths: Vec<String> = [
        "crates/*/tests/",
        "crates/*/src/**/tests.rs",
        "crates/*/src/**/tests/",
        "client/src/**/*.test.ts",
        "client/src/**/*.test.tsx",
    ]
    .map(String::from)
    .to_vec();
    for yes in [
        "crates/engine/tests/test_files.rs",
        "crates/engine/tests/e2e/main.rs",
        "crates/engine/src/tests.rs",
        "crates/engine/src/a/tests.rs",
        "crates/engine/src/a/b/c/tests.rs",
        "crates/engine/src/tests/x.rs",
        "crates/engine/src/a/b/tests/x/y.rs",
        "client/src/a.test.ts",
        "client/src/a/b/c.test.tsx",
        "client/src/.test.ts",
    ] {
        assert!(under(yes, &paths), "{yes} holds tests");
    }
    for no in [
        "crates/engine/src/inputs.rs",
        "crates/engine/src/test_tables.rs",
        "crates/engine/tests",
        "crates/engine/src/tests",
        "crates/engine/src/tests.rs.orig",
        "crates/engine/src/a/mytests.rs",
        "crates/engine/src/contests/x.rs",
        "crates/a/b/tests/x.rs",
        "tests/x.rs",
        "client/src/a.test.js",
        "client/src/test.ts",
        "client/src/a.test.tsx.snap",
        "client/a.test.ts",
        "crates/engine/Cargo.toml",
    ] {
        assert!(!under(no, &paths), "{no} holds no test");
    }
    // Nothing named, nothing under it.
    assert!(!under("tests/x.rs", &[]));
}

// A test file the work moved or took away is a file the work changed, at the name it had: a test moved out of the test
// paths, where the reader would otherwise never see the file go, and one moved within them are told with their old
// names, as `changed_among` tells them. The reference is `git diff --name-status --no-renames`, which lists every one
// of these paths.
#[test]
fn a_test_file_the_work_moved_or_took_away_is_told_at_the_name_it_had() {
    let dir = fresh("moved");
    commit(
        &dir,
        &[
            ("tests/moved_out.rs", "fn moved_out() { assert!(true); }\n"),
            ("tests/moved_within.rs", "fn moved_within() { assert!(true); }\n"),
            ("tests/taken_away.rs", "fn taken_away() { assert!(true); }\n"),
            ("src/lib.rs", "fn lib() {}\n"),
        ],
        "Start",
    );
    fetched_main(&dir);
    git(&dir, &["checkout", "-q", "-b", "work"]);
    git(&dir, &["mv", "tests/moved_out.rs", "src/moved_out.rs"]);
    git(&dir, &["mv", "tests/moved_within.rs", "tests/moved_again.rs"]);
    git(&dir, &["rm", "-q", "tests/taken_away.rs"]);
    git(&dir, &["commit", "-q", "-m", "Move and take away"]);
    let expected = [
        "tests/moved_again.rs",
        "tests/moved_out.rs",
        "tests/moved_within.rs",
        "tests/taken_away.rs",
    ];
    // The reference: every path git names when it is not allowed to see a rename.
    let fork = fork_point(&dir, "main").unwrap();
    let reference = git(
        &dir,
        &[
            "diff",
            "--name-only",
            "--no-renames",
            &format!("{fork}..HEAD"),
            "--",
            "tests/",
        ],
    );
    assert_eq!(reference.lines().collect::<Vec<_>>(), expected);
    assert_eq!(test_files(&dir, &["tests/"]), expected);
    let _ = std::fs::remove_dir_all(&dir);
}

// Paths git must not mangle, and what main brought: the work's files are told whole whatever is in their names (a
// space, a quote, a newline, letters beyond ASCII), a file both the work and main changed stays, and a file only main
// changed does not come in, whether the branch took it in by a merge or main went on after that merge without it.
#[test]
fn the_files_of_the_work_are_told_whole_and_those_of_main_are_left_out() {
    let dir = fresh("odd");
    let many: String = (1..=30).map(|n| format!("line {n}\n")).collect();
    commit(
        &dir,
        &[
            ("tests/shared.rs", many.as_str()),
            ("tests/main_only.rs", "fn main_only() {}\n"),
            ("src/lib.rs", "fn lib() {}\n"),
        ],
        "Start",
    );
    fetched_main(&dir);
    git(&dir, &["checkout", "-q", "-b", "work"]);
    // A letter beyond ASCII, made from its code (the repository holds no such letter in its files).
    let accented = format!("tests/{}n.rs", char::from(0xfc_u8));
    let odd = [
        "tests/with space.rs",
        "tests/q\"uote.rs",
        "tests/new\nline.rs",
        accented.as_str(),
        "tests/back\\slash.rs",
    ];
    let mut files: Vec<(&str, &str)> = odd.iter().map(|f| (*f, "fn t() {}\n")).collect();
    let shared_by_work = many.replacen("line 1\n", "line 1 by the work\n", 1);
    files.push(("tests/shared.rs", shared_by_work.as_str()));
    files.push(("src/lib.rs", "fn lib() { /* changed */ }\n"));
    commit(&dir, &files, "The work's change");
    // Main changes the other end of the shared file and a file of its own, and the branch takes main in.
    git(&dir, &["checkout", "-q", "main"]);
    let shared_by_main = many.replacen("line 30\n", "line 30 by main\n", 1);
    commit(
        &dir,
        &[
            ("tests/shared.rs", shared_by_main.as_str()),
            ("tests/main_only.rs", "fn main_only() { 1; }\n"),
        ],
        "Main goes on",
    );
    fetched_main(&dir);
    git(&dir, &["checkout", "-q", "work"]);
    git(&dir, &["merge", "-q", "--no-edit", "origin/main"]);
    // Main goes on after the merge, and the branch does not take that in.
    git(&dir, &["checkout", "-q", "main"]);
    commit(&dir, &[("tests/later.rs", "fn later() {}\n")], "Main goes on again");
    fetched_main(&dir);
    git(&dir, &["checkout", "-q", "work"]);

    let mut expected: Vec<&str> = odd.to_vec();
    expected.push("tests/shared.rs");
    // git lists paths in byte order.
    expected.sort();
    assert_eq!(test_files(&dir, &["tests/"]), expected);
    let _ = std::fs::remove_dir_all(&dir);
}
