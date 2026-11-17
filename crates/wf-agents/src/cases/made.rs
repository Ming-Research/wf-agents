//! The made project the cases run in, as its repositories are made from what is kept beside the flows: once a run, a
//! bare repository imported from the project's `git fast-export` text, so that every commit has the id it has in the
//! file; and, for each try of a case, a repository of the project's own, cloned from that one sharing its objects (no
//! object copied and no file linked twice, which an indexer of the machine would chase), its main at the commit the
//! case's item starts from. What a run makes is removed once its results are kept.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The directory beside the flows' cases that holds the made projects, each as `<name>.fast-export`.
pub const PROJECTS: &str = "projects";

/// The branch of the project's own repository that the works merge into.
pub const MAIN: &str = "main";

/// Runs git in the directory with these arguments, giving what it printed, or why it failed.
pub fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("git {}: {e}", args.join(" ")))?;
    match out.status.success() {
        true => Ok(String::from_utf8_lossy(&out.stdout).trim_end().to_string()),
        false => Err(format!(
            "git {} in {}: {}",
            args.join(" "),
            dir.display(),
            String::from_utf8_lossy(&out.stderr).trim_end()
        )),
    }
}

/// Imports the made project's export into a new bare repository at `into`.
pub fn import(export: &Path, into: &Path) -> Result<PathBuf, String> {
    let text = std::fs::File::open(export).map_err(|e| format!("{}: {e}", export.display()))?;
    git(Path::new("."), &["init", "--quiet", "--bare", &into.to_string_lossy()])?;
    let out = Command::new("git")
        .arg("-C")
        .arg(into)
        .args(["fast-import", "--quiet"])
        .stdin(text)
        .output()
        .map_err(|e| format!("git fast-import: {e}"))?;
    match out.status.success() {
        true => Ok(into.to_path_buf()),
        false => Err(format!(
            "git fast-import of {}: {}",
            export.display(),
            String::from_utf8_lossy(&out.stderr).trim_end()
        )),
    }
}

/// The commit a name of the made project stands for: a branch, a tag or a commit.
pub fn commit(repo: &Path, name: &str) -> Result<String, String> {
    let at = format!("{name}^{{commit}}");
    git(repo, &["rev-parse", "--verify", "--quiet", &at]).map_err(|_| format!("the made project has no commit {name}"))
}

/// A repository of the project's own at `dir`, cloned from the imported one sharing its objects, with only its main
/// branch, at the commit of this name of the made project, checked out.
pub fn project(imported: &Path, dir: &Path, main: &str) -> Result<PathBuf, String> {
    let at = commit(imported, main)?;
    let from = imported.to_string_lossy();
    let to = dir.to_string_lossy();
    git(
        Path::new("."),
        &["clone", "--quiet", "--shared", "--no-checkout", &from, &to],
    )?;
    git(dir, &["checkout", "--quiet", "-B", MAIN, &at])?;
    git(dir, &["remote", "remove", "origin"])?;
    Ok(dir.to_path_buf())
}

/// Removes a directory the run made, and what git kept of the worktrees in it.
pub fn remove(dir: &Path) -> Result<(), String> {
    match std::fs::remove_dir_all(dir) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("{}: {e}", dir.display())),
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::MetadataExt;

    use super::*;

    fn export() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../flow/cases")
            .join(PROJECTS)
            .join("tally.fast-export")
    }

    /// Each file under the directory, at any depth.
    fn files(dir: &Path) -> Vec<PathBuf> {
        let mut out = Vec::new();
        for entry in std::fs::read_dir(dir).expect("the directory is read").flatten() {
            let path = entry.path();
            match path.is_dir() {
                true => out.extend(files(&path)),
                false => out.push(path),
            }
        }
        out
    }

    #[test]
    fn the_made_project_is_imported_with_the_ids_it_was_made_with_and_cloned_with_no_file_linked_twice() {
        let dir = tempfile::tempdir().expect("a directory");
        let imported = import(&export(), &dir.path().join("made.git")).expect("imported");
        // The ids the made project's commits were made with, which the cases' texts name.
        assert!(commit(&imported, "v-bug").expect("v-bug").starts_with("a538f1b"));
        assert!(commit(&imported, "v-fixed").expect("v-fixed").starts_with("4a986ff"));
        assert!(commit(&imported, "w-cat").expect("w-cat").starts_with("d1d7040"));
        assert!(commit(&imported, "no-such-branch").is_err());
        let one = project(&imported, &dir.path().join("one"), "v-fixed").expect("cloned");
        let two = project(&imported, &dir.path().join("two"), "v-bug").expect("cloned");
        assert_eq!(git(&one, &["rev-parse", "HEAD"]), commit(&imported, "v-fixed"));
        assert_eq!(git(&two, &["rev-parse", "main"]), commit(&imported, "v-bug"));
        assert_eq!(git(&two, &["branch", "--show-current"]).as_deref(), Ok(MAIN));
        // A branch of the made project is reached from the clone, by its commit.
        let w_cat = commit(&imported, "w-cat").expect("w-cat");
        assert_eq!(git(&one, &["cat-file", "-t", &w_cat]).as_deref(), Ok("commit"));
        for file in files(&one.join(".git")).into_iter().chain(files(&imported)) {
            let links = std::fs::metadata(&file).expect("its metadata").nlink();
            assert_eq!(links, 1, "{} has {links} links", file.display());
        }
        remove(&one).expect("removed");
        assert!(!one.exists());
        remove(&one).expect("what is not there is removed already");
    }
}
