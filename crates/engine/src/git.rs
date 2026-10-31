//! What the engine does with git in a project's clone and a work's worktree. Everything here is local; pushing and
//! everything on GitHub go through [`crate::hub::GitHub`]; setting a worktree up and removing it are the session
//! manager's ([`sessions::worktree`]).

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

/// A work's worktree is the session manager's: it sets it up, removes it, and says what its head is and whether it is
/// clean.
pub use sessions::worktree::{create_worktree, git, head, is_clean, remove_worktree, resolve, take_over_worktree};

fn lines(s: &str) -> Vec<String> {
    s.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect()
}

/// Whether `a` is `b` or an ancestor of it.
pub fn is_ancestor(dir: &Path, a: &str, b: &str) -> bool {
    git(dir, &["merge-base", "--is-ancestor", a, b]).is_ok()
}

/// Where the work's branch forked from main: the merge base of HEAD and the fetched main.
pub fn fork_point(dir: &Path, main: &str) -> Result<String, String> {
    main_merged_into(dir, "HEAD", main)
}

/// Main as merged into a commit: the merge base of the commit and the fetched main, the latest commit of main it
/// contains.
fn main_merged_into(dir: &Path, rev: &str, main: &str) -> Result<String, String> {
    Ok(git(dir, &["merge-base", rev, &format!("origin/{main}")])?
        .trim()
        .to_string())
}

/// The files changed from one commit to another.
pub fn changed(dir: &Path, from: &str, to: &str) -> Result<Vec<String>, String> {
    Ok(lines(&git(dir, &["diff", "--name-only", &format!("{from}..{to}")])?))
}

/// The files the work itself changed from one commit to another: those changed between them that `to` also changes
/// against main as merged into it. A file that only main changed, brought in by merging main, drops out; a file both
/// main and the work changed stays.
pub fn own_changes(dir: &Path, main: &str, from: &str, to: &str) -> Result<Vec<String>, String> {
    let all = changed(dir, from, to)?;
    if all.is_empty() {
        return Ok(all);
    }
    let merged = main_merged_into(dir, to, main)?;
    let own: BTreeSet<String> = changed(dir, &merged, to)?.into_iter().collect();
    Ok(all.into_iter().filter(|f| own.contains(f)).collect())
}

/// The files changed in the worktree against a commit, committed or not, untracked files included.
pub fn changed_since(dir: &Path, from: &str) -> Result<Vec<String>, String> {
    let mut out = lines(&git(dir, &["diff", "--name-only", from])?);
    out.extend(lines(&git(dir, &["ls-files", "--others", "--exclude-standard"])?));
    out.sort();
    out.dedup();
    Ok(out)
}

/// The diff from one commit to another, of some paths or of all.
pub fn diff(dir: &Path, from: &str, to: &str, paths: &[String]) -> Result<String, String> {
    let range = format!("{from}..{to}");
    let mut args = vec!["diff", range.as_str()];
    if !paths.is_empty() {
        args.push("--");
        args.extend(paths.iter().map(String::as_str));
    }
    git(dir, &args)
}

/// Whether a path is under one of the given paths: a directory given with its trailing slash, a file by its name,
/// and `*` matching one segment.
pub fn under(path: &str, paths: &[String]) -> bool {
    let got: Vec<&str> = path.split('/').collect();
    paths.iter().any(|p| {
        let dir = p.ends_with('/');
        let pat: Vec<&str> = p.trim_end_matches('/').split('/').collect();
        let long_enough = if dir {
            got.len() > pat.len()
        } else {
            got.len() >= pat.len()
        };
        long_enough && pat.iter().zip(&got).all(|(a, b)| *a == "*" || a == b)
    })
}

/// Merges the fetched main into the worktree's branch. On a conflict the merge is undone and the files in conflict
/// are given.
pub fn catch_up(dir: &Path, main: &str) -> Result<Result<String, Vec<String>>, String> {
    git(dir, &["fetch", "-q", "origin", main])?;
    let theirs = format!("origin/{main}");
    if is_ancestor(dir, &theirs, "HEAD") {
        return Ok(Ok(head(dir)?));
    }
    match git(
        dir,
        &[
            "-c",
            "user.name=wf-agents",
            "-c",
            "user.email=wf-agents@localhost",
            "merge",
            "--no-edit",
            "-q",
            &theirs,
        ],
    ) {
        Ok(_) => Ok(Ok(head(dir)?)),
        Err(_) => {
            let files = lines(&git(dir, &["diff", "--name-only", "--diff-filter=U"]).unwrap_or_default());
            let _ = git(dir, &["merge", "--abort"]);
            Ok(Err(files))
        }
    }
}

/// A commit on `base` that has the given files as they are at `from`: the tests of a change put on the code before
/// it. Nothing changes in the worktree.
pub fn files_onto(dir: &Path, files: &[String], from: &str, base: &str) -> Result<String, String> {
    let tmp = dir.join(".git-wf-agents-index");
    let index = tmp.to_string_lossy().into_owned();
    let run = |args: &[&str]| -> Result<String, String> {
        let out = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .env("GIT_INDEX_FILE", &index)
            .output()
            .map_err(|e| format!("git: {e}"))?;
        if !out.status.success() {
            return Err(format!(
                "git {}: {}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    };
    let result = commit_with(&run, files, from, base);
    let _ = std::fs::remove_file(&tmp);
    result
}

fn commit_with(
    run: &impl Fn(&[&str]) -> Result<String, String>,
    files: &[String],
    from: &str,
    base: &str,
) -> Result<String, String> {
    run(&["read-tree", base])?;
    for f in files {
        let blob = run(&["rev-parse", &format!("{from}:{f}")])?;
        run(&["update-index", "--add", "--cacheinfo", &format!("100644,{blob},{f}")])?;
    }
    let tree = run(&["write-tree"])?;
    let who = ["-c", "user.name=wf-agents", "-c", "user.email=wf-agents@localhost"];
    let msg = "Tests of the change on the code before it";
    run(&[&who[..], &["commit-tree", tree.as_str(), "-p", base, "-m", msg]].concat())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_are_matched_by_directory_name_and_segment() {
        let ps = |xs: &[&str]| xs.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(under("design/a.md", &ps(&["design/"])));
        assert!(!under("designs/a.md", &ps(&["design/"])));
        assert!(under("flow/flow.json", &ps(&["flow/flow.json"])));
        assert!(under("crates/db/tests/x.rs", &ps(&["crates/*/tests/"])));
        assert!(!under("crates/db/src/x.rs", &ps(&["crates/*/tests/"])));
        assert!(!under("crates/tests", &ps(&["crates/*/tests/"])));
    }

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

    fn commit(dir: &Path, files: &[(&str, &str)], message: &str) -> String {
        for (f, text) in files {
            let p = dir.join(f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
        }
        run(dir, &["add", "-A"]);
        run(dir, &["commit", "-q", "-m", message]);
        run(dir, &["rev-parse", "HEAD"])
    }

    // What a merge of main brings in is not the work's own change, unless the work changed that file too.
    #[test]
    fn a_merge_of_main_brings_in_no_change_of_the_works_own() {
        let dir = std::env::temp_dir().join(format!("wf-agents-own-changes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        run(&dir, &["init", "-q"]);
        let start = commit(&dir, &[("design/a.md", "# A\nOld.\n"), ("src/x.txt", "old\n")], "Start");
        run(&dir, &["checkout", "-q", "-b", "work"]);
        let own = commit(
            &dir,
            &[("design/a.md", "# A\nOld.\nThe work's.\n"), ("src/x.txt", "new\n")],
            "The work's change",
        );
        run(&dir, &["checkout", "-q", "main"]);
        commit(
            &dir,
            &[("design/a.md", "# The A\nOld.\n"), ("design/b.md", "Main's.\n")],
            "Main's change",
        );
        run(&dir, &["update-ref", "refs/remotes/origin/main", "main"]);
        run(&dir, &["checkout", "-q", "work"]);
        run(&dir, &["merge", "-q", "--no-edit", "origin/main"]);
        // Since the work's own commit, the merge changed both files; main's new file drops out, and the file both
        // changed stays.
        assert_eq!(changed(&dir, &own, "HEAD").unwrap(), vec!["design/a.md", "design/b.md"]);
        assert_eq!(own_changes(&dir, "main", &own, "HEAD").unwrap(), vec!["design/a.md"]);
        // Across the whole branch, the work's own files.
        assert_eq!(
            own_changes(&dir, "main", &start, "HEAD").unwrap(),
            vec!["design/a.md", "src/x.txt"]
        );
        assert!(own_changes(&dir, "main", "HEAD", "HEAD").unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
