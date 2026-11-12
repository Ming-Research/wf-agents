//! A flow's definition as it was committed, for the owner's page to compare the flow's files against.
//!
//! The owner reviews every change to the flow on the page (Article 7), and a change to the flow is first an edit of its
//! files that is not committed yet. The page of a running engine therefore compares the files with what the commit
//! checked out in their repository has of them, so that what was edited since is listed there as what changed. A flow
//! whose directory is in no repository, or is not in that commit, is compared with nothing.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use flow::Source;

/// Reads the flow in this directory as the commit checked out has it, anew each time; or none, where the directory is
/// in no repository or the commit has no flow there.
pub fn reader(dir: PathBuf) -> Option<web::Read> {
    read(&dir).ok()?;
    Some(Box::new(move || read(&dir)))
}

/// The flow in this directory as the commit checked out in its repository has it, whatever its files say now.
fn read(dir: &Path) -> Result<Source, String> {
    // Each file and directory of the directory at that commit: its mode, its kind and its object, a tab, its name.
    let listed = git(dir, &["ls-tree", "--full-tree", "-z", "HEAD:./"])?;
    let mut files = Vec::new();
    for entry in listed.split('\0') {
        let Some((about, name)) = entry.split_once('\t') else {
            continue;
        };
        let mut about = about.split(' ');
        let (Some(_), Some("blob"), Some(object)) = (about.next(), about.next(), about.next()) else {
            continue;
        };
        // Only the files a flow is made of are read.
        if name.ends_with(".toml") {
            files.push((name.to_string(), git(dir, &["cat-file", "blob", object])?));
        }
    }
    Source::of_files(files)
        .ok_or_else(|| format!("{}: the commit checked out has no {}", dir.display(), flow::FLOW_FILE))
}

/// Runs git in the directory and gives what it wrote, as every git command of the engine's is run: with the settings
/// that keep a repository's configuration from running anything, and with only what a program needs of the engine's
/// environment. Says what git said where it fails.
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
    }

    #[test]
    fn a_directory_that_is_not_committed_is_compared_with_nothing() {
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("flow.toml"), "name = \"one\"\n").unwrap();
        assert!(reader(outside.path().to_path_buf()).is_none(), "in no repository");

        run(outside.path(), &["init", "--quiet"]);
        run(
            outside.path(),
            &["commit", "--quiet", "--allow-empty", "-m", "nothing yet"],
        );
        assert!(reader(outside.path().to_path_buf()).is_none(), "not in the commit");
    }
}
