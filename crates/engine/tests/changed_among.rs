//! Which of the test files a run of `ci_before` put on the code before the change differ from the head now, against a
//! real repository: git's own account of two commits is the reference. A name is taken as it is written, whatever
//! characters it holds, and told as it was written.

use std::path::{Path, PathBuf};
use std::process::Command;

use engine::git::changed_among;

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

/// An empty repository of its own, named for the test that makes it.
fn repo(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("wf-agents-{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    git(&dir, &["init", "-q"]);
    dir
}

/// Writes the files, commits them, and gives the commit.
fn commit(dir: &Path, files: &[(&str, &str)], message: &str) -> String {
    for (name, text) in files {
        let p = dir.join(name);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", message]);
    git(dir, &["rev-parse", "HEAD"])
}

fn names(xs: &[&str]) -> Vec<String> {
    xs.iter().map(|s| s.to_string()).collect()
}

// A file whose name holds a character outside ASCII (the name of a test in another language), a space or a leading dash
// is told by the name it was given, which git writes in quotes with escapes for such characters unless it is told not
// to: the engine tells the agent which file to run `ci_before` again for, and a name in escapes is not a file.
#[test]
fn a_changed_file_is_told_by_the_name_it_was_written_with_whatever_its_characters() {
    let dir = repo("changed-among-names");
    let wide = format!("tests/{}.rs", char::from_u32(0x4e2d).unwrap());
    let accented = format!("tests/caf{}.rs", char::from_u32(0xe9).unwrap());
    let odd = ["tests/with space.rs", "-dash.rs", "tests/tab\tname.rs"];
    let mut files: Vec<(&str, &str)> = vec![(wide.as_str(), "one\n"), (accented.as_str(), "one\n")];
    files.extend(odd.iter().map(|n| (*n, "one\n")));
    let start = commit(&dir, &files, "Start");
    let files: Vec<(&str, &str)> = files.iter().map(|(n, _)| (*n, "two\n")).collect();
    commit(&dir, &files, "Change");
    let mut asked: Vec<String> = names(&odd);
    asked.push(wide.clone());
    asked.push(accented.clone());
    let mut told = changed_among(&dir, &start, "HEAD", &asked).unwrap();
    told.sort();
    let mut want = asked.clone();
    want.sort();
    assert_eq!(told, want);
    let _ = std::fs::remove_dir_all(&dir);
}

// A file edited after the run and edited back to what it was is the file the run used, so it is not told as changed;
// a file the run used that the head does not have any more, and one the head has that the run's commit did not, are.
#[test]
fn a_file_edited_and_edited_back_is_not_changed_and_one_added_or_taken_away_is() {
    let dir = repo("changed-among-back");
    let start = commit(&dir, &[("t/a.rs", "same\n"), ("t/b.rs", "b\n")], "Start");
    commit(&dir, &[("t/a.rs", "other\n")], "Edit");
    commit(&dir, &[("t/a.rs", "same\n"), ("t/new.rs", "new\n")], "Back and add");
    std::fs::remove_file(dir.join("t/b.rs")).unwrap();
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "Remove"]);
    let among = |paths: &[&str]| changed_among(&dir, &start, "HEAD", &names(paths)).unwrap();
    assert!(among(&["t/a.rs"]).is_empty());
    assert_eq!(among(&["t/a.rs", "t/b.rs", "t/new.rs"]), ["t/b.rs", "t/new.rs"]);
    // The commit in the middle is the run's commit: only what changed after it counts.
    let middle = git(&dir, &["rev-parse", "HEAD~2"]);
    assert_eq!(
        changed_among(&dir, &middle, "HEAD", &names(&["t/a.rs", "t/b.rs"])).unwrap(),
        ["t/a.rs", "t/b.rs"]
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// The files among no files are none: a list with no path is not "every path", which is what git itself reads it as.
// And a commit the worktree does not have is an error to be told, not a list: the caller counts that as every file
// changed, and may not mistake an empty answer for nothing changed.
#[test]
fn no_paths_name_no_files_and_a_missing_commit_is_an_error() {
    let dir = repo("changed-among-none");
    let start = commit(&dir, &[("t/a.rs", "one\n")], "Start");
    commit(&dir, &[("t/a.rs", "two\n"), ("src/lib.rs", "two\n")], "Change");
    assert!(
        changed_among(&dir, &start, "HEAD", &[]).unwrap().is_empty(),
        "a list of no paths gave files"
    );
    let gone = "0123456789abcdef0123456789abcdef01234567";
    assert!(changed_among(&dir, gone, "HEAD", &names(&["t/a.rs"])).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}
