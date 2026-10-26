//! What the engine does with git in a project's clone and a work's worktree. Everything here is local; pushing and
//! everything on GitHub go through [`crate::hub::GitHub`].

use std::path::Path;
use std::process::Command;

/// Runs git in `dir` and gives its output, or what it said on failure.
pub fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .map_err(|e| format!("git: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let msg = if err.trim().is_empty() {
            String::from_utf8_lossy(&out.stdout).into_owned()
        } else {
            err.into_owned()
        };
        return Err(format!("git {}: {}", args.join(" "), msg.trim()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn lines(s: &str) -> Vec<String> {
    s.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect()
}

/// Fetches main and makes a worktree on a new branch from it; gives the commit it starts from.
pub fn create_worktree(checkout: &Path, dir: &Path, branch: &str, main: &str) -> Result<String, String> {
    git(checkout, &["fetch", "-q", "origin", main])?;
    let base = format!("origin/{main}");
    let d = dir.to_string_lossy();
    git(checkout, &["worktree", "add", "-q", "-b", branch, &d, &base])?;
    head(dir)
}

/// Removes a work's worktree; its branch stays.
pub fn remove_worktree(checkout: &Path, dir: &Path) -> Result<(), String> {
    if dir.exists() {
        git(checkout, &["worktree", "remove", "--force", &dir.to_string_lossy()])?;
    }
    Ok(())
}

pub fn head(dir: &Path) -> Result<String, String> {
    Ok(git(dir, &["rev-parse", "HEAD"])?.trim().to_string())
}

/// The full id of a commit, or none when there is no such commit.
pub fn resolve(dir: &Path, rev: &str) -> Option<String> {
    git(dir, &["rev-parse", "--verify", "--quiet", &format!("{rev}^{{commit}}")])
        .ok()
        .map(|s| s.trim().to_string())
}

/// Whether the worktree has nothing uncommitted, untracked files included.
pub fn is_clean(dir: &Path) -> Result<bool, String> {
    Ok(git(dir, &["status", "--porcelain"])?.trim().is_empty())
}

/// Whether `a` is `b` or an ancestor of it.
pub fn is_ancestor(dir: &Path, a: &str, b: &str) -> bool {
    git(dir, &["merge-base", "--is-ancestor", a, b]).is_ok()
}

/// Where the work's branch forked from main: the merge base of HEAD and the fetched main.
pub fn fork_point(dir: &Path, main: &str) -> Result<String, String> {
    Ok(git(dir, &["merge-base", "HEAD", &format!("origin/{main}")])?
        .trim()
        .to_string())
}

/// The files changed from one commit to another.
pub fn changed(dir: &Path, from: &str, to: &str) -> Result<Vec<String>, String> {
    Ok(lines(&git(dir, &["diff", "--name-only", &format!("{from}..{to}")])?))
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
}
