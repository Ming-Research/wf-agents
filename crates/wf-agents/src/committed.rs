//! A flow's definition as it was committed, for the owner's page to compare the flow's files against.
//!
//! The owner reviews every change to the flow on the page (Article 7), and a change to the flow is first an edit of its
//! files that is not committed yet. The page of a running engine therefore compares the files with what the commit
//! checked out in their repository has of them, so that what was edited since is listed there as what changed. A flow
//! whose directory is in no repository is compared with nothing.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use flow::Source;

/// Reads the flow in this directory as the commit checked out has it, anew each time; or none, where the directory is
/// in no repository. Whether the commit has a flow there is found at each reading, so that a flow committed while the
/// engine runs is compared from then on, and one not committed yet says so on the page.
pub fn reader(dir: PathBuf) -> Option<web::Read> {
    git(&dir, &["rev-parse", "--is-inside-work-tree"]).ok()?;
    Some(Box::new(move || read(&dir)))
}

/// The flow in this directory as the commit checked out in its repository has it, whatever its files say now.
fn read(dir: &Path) -> Result<Source, String> {
    // Each file of the directory at that commit, in it or below it: its mode, its kind and its object, a tab, its path
    // within the directory.
    let listed = git(dir, &["ls-tree", "-r", "--full-tree", "-z", "HEAD:./"])?;
    let mut files = Vec::new();
    for entry in listed.split('\0') {
        let Some((about, name)) = entry.split_once('\t') else {
            continue;
        };
        let mut about = about.split(' ');
        // Only a file as such: of a link the commit holds where it points, not what the file it points at says.
        let (Some("100644" | "100755"), Some("blob"), Some(object)) = (about.next(), about.next(), about.next()) else {
            continue;
        };
        // Only the files a flow is made of are read: its own, directly in the directory, and its sets of rules.
        let of_the_flow = match name.split_once('/') {
            None => name.ends_with(".toml"),
            Some((parent, rules)) => parent == flow::RULES_DIR && !rules.contains('/') && rules.ends_with(".md"),
        };
        if of_the_flow {
            files.push((name.to_string(), git(dir, &["cat-file", "blob", object])?));
        }
    }
    Source::of_files(files)
        .ok_or_else(|| format!("{}: the commit checked out has no {}", dir.display(), flow::FLOW_FILE))
}

/// Runs git in the directory and gives what it wrote; says what git said where it fails. It is run only to read what
/// the repository holds already: with the settings every git command of the engine's is given, so that nothing the
/// repository's configuration names is run by them; with only what a program needs of the engine's environment; and
/// told to fetch nothing and to ask nothing, so that a repository that holds only part of its history cannot make a
/// reading reach its remote, which could run a program that configuration names and wait without end.
fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let kept =
        std::env::vars().filter(|(name, _)| carry::cli::KEPT.contains(&name.as_str()) || name.starts_with("LC_"));
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(carry::git::SAFE)
        .args(args)
        .env_clear()
        .envs(kept)
        .env("GIT_NO_LAZY_FETCH", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("git cannot be run: {e}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim_end().to_string());
    }
    String::from_utf8(output.stdout).map_err(|_| "what git gave is not UTF-8".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs git in the directory for a test, as someone who may commit.
    fn run(dir: &Path, args: &[&str]) {
        let done = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["-c", "user.name=test", "-c", "user.email=test@example.invalid"])
            .args(args)
            .output()
            .expect("git runs");
        assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    }

    #[test]
    fn a_flow_is_read_as_committed_whatever_its_files_say_now() {
        let repository = tempfile::tempdir().unwrap();
        let dir = repository.path().join("flows").join("one");
        std::fs::create_dir_all(dir.join("notes")).unwrap();
        std::fs::write(dir.join("flow.toml"), "name = \"one\"\n").unwrap();
        std::fs::write(dir.join("a.toml"), "title = \"A\"\n").unwrap();
        std::fs::write(dir.join("README.md"), "not of the flow\n").unwrap();
        std::fs::write(dir.join("notes").join("b.toml"), "not directly in it\n").unwrap();
        std::fs::create_dir_all(dir.join("rules").join("old")).unwrap();
        std::fs::write(dir.join("rules").join("general.md"), "Say one thing per sentence.\n").unwrap();
        std::fs::write(dir.join("rules").join("notes.txt"), "not a set of rules\n").unwrap();
        std::fs::write(dir.join("rules").join("old").join("general.md"), "not directly in it\n").unwrap();
        run(repository.path(), &["init", "--quiet"]);
        run(repository.path(), &["add", "."]);
        run(repository.path(), &["commit", "--quiet", "-m", "a flow"]);

        // Edited and added to since the commit: the page shows these, and compares them with what was committed.
        std::fs::write(dir.join("a.toml"), "title = \"First\"\n").unwrap();
        std::fs::write(dir.join("c.toml"), "title = \"C\"\n").unwrap();

        let committed = (reader(dir.clone()).expect("the directory is in the commit"))().unwrap();
        assert_eq!(committed.flow, "name = \"one\"\n");
        assert_eq!(
            committed.cells.into_iter().collect::<Vec<_>>(),
            [("a".to_string(), "title = \"A\"\n".to_string())]
        );
        assert_eq!(
            committed.rules.into_iter().collect::<Vec<_>>(),
            [("general".to_string(), "Say one thing per sentence.\n".to_string())]
        );
    }

    #[test]
    fn a_directory_in_no_repository_is_compared_with_nothing_and_one_not_committed_says_so() {
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("flow.toml"), "name = \"one\"\n").unwrap();
        assert!(reader(outside.path().to_path_buf()).is_none(), "in no repository");

        run(outside.path(), &["init", "--quiet"]);
        run(
            outside.path(),
            &["commit", "--quiet", "--allow-empty", "-m", "nothing yet"],
        );
        let read = reader(outside.path().to_path_buf()).expect("in a repository");
        let why = read().expect_err("the commit has no flow");
        assert!(why.ends_with("the commit checked out has no flow.toml"), "{why}");

        // Committed while the reader stands, it is read from then on.
        run(outside.path(), &["add", "flow.toml"]);
        run(outside.path(), &["commit", "--quiet", "-m", "a flow"]);
        assert_eq!(read().unwrap().flow, "name = \"one\"\n");
    }

    #[cfg(unix)]
    #[test]
    fn a_link_in_the_commit_is_not_read_as_a_file_of_the_flow() {
        let repository = tempfile::tempdir().unwrap();
        std::fs::write(repository.path().join("flow.toml"), "name = \"one\"\n").unwrap();
        std::fs::write(repository.path().join("elsewhere.txt"), "title = \"A\"\n").unwrap();
        std::os::unix::fs::symlink("elsewhere.txt", repository.path().join("a.toml")).unwrap();
        run(repository.path(), &["init", "--quiet"]);
        run(repository.path(), &["add", "."]);
        run(repository.path(), &["commit", "--quiet", "-m", "a flow with a link"]);
        let committed = (reader(repository.path().to_path_buf()).expect("in a repository"))().unwrap();
        assert!(committed.cells.is_empty(), "{:?}", committed.cells);
    }
}
