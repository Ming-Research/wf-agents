//! The work's place in the project's repository, with plain `git`: a branch and a worktree for each work, the check
//! before the merge, and the merge. Each is idempotent, since a round may be given again after the process started
//! again: what was done already is found and said again, not done a second time.

use std::path::{Path, PathBuf};

use store::WorkNumber;
use tokio::process::Command;

use crate::held::Stop;
use crate::process::{self, Ran};
use crate::settings::Project;

/// The directory in the data directory that holds the works' worktrees, one for each work, named by its number.
pub const WORKTREES: &str = "worktrees";

/// Where a work is done.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Place {
    pub branch: String,
    /// The work's worktree.
    pub dir: PathBuf,
}

/// Where the work is done, and the commit of the main branch it started from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Started {
    pub place: Place,
    pub from: String,
}

/// What the check before the merge found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checked {
    /// The commit checked: the head of the work's branch.
    pub commit: String,
    /// The head of the main branch it was checked against.
    pub main: String,
    /// What failed, with the end of what it said; none where everything passed.
    pub failed: Option<String>,
}

/// The work's place, by its number, in the data directory given.
pub fn place(data: &Path, work: WorkNumber) -> Place {
    Place {
        branch: format!("work-{}", work.0),
        dir: data.join(WORKTREES).join(work.0.to_string()),
    }
}

/// What a git command gave: whether it exited with 0, and its standard output and error.
struct Said {
    ok: bool,
    code: Option<i32>,
    out: String,
    err: String,
}

/// Runs git in the directory; says why where it cannot be started.
async fn said(dir: &Path, args: &[&str]) -> Result<Said, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(std::process::Stdio::null())
        .output()
        .await
        .map_err(|e| format!("git cannot be started: {e}"))?;
    Ok(Said {
        ok: output.status.success(),
        code: output.status.code(),
        out: String::from_utf8_lossy(&output.stdout).trim_end().to_string(),
        err: String::from_utf8_lossy(&output.stderr).trim_end().to_string(),
    })
}

/// Runs git in the directory and gives its standard output; or says what failed, with what git said.
async fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let said = said(dir, args).await?;
    if said.ok {
        Ok(said.out)
    } else {
        let words = if said.err.is_empty() { said.out } else { said.err };
        Err(format!("git {} failed in {}: {words}", args.join(" "), dir.display()))
    }
}

/// The path as git is given it: the worktree's path must not be read against the repository's directory.
fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// Makes the work's branch from the main branch's head and its worktree, keeping both where they are made already.
pub async fn start(project: &Project, data: &Path, work: WorkNumber) -> Result<Started, String> {
    let place = place(data, work);
    let repo = &project.repo;
    let branch = format!("refs/heads/{}", place.branch);
    let main = format!("refs/heads/{}", project.main);
    if place.dir.exists() {
        let on = git(&place.dir, &["rev-parse", "--abbrev-ref", "HEAD"]).await?;
        if on != place.branch {
            return Err(format!(
                "{} is there already, and is not the worktree of the branch {}: it is on {on}",
                place.dir.display(),
                place.branch
            ));
        }
    } else {
        if let Some(parent) = place.dir.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        // A worktree whose directory was taken away is still named by the repository until it is pruned.
        git(repo, &["worktree", "prune"]).await?;
        let dir = text(&place.dir);
        let exists = said(repo, &["rev-parse", "--verify", "--quiet", &branch]).await?.ok;
        if exists {
            git(repo, &["worktree", "add", &dir, &place.branch]).await?;
        } else {
            git(repo, &["worktree", "add", "-b", &place.branch, &dir, &main]).await?;
        }
    }
    let from = git(repo, &["merge-base", &main, &branch]).await?;
    Ok(Started { place, from })
}

/// The work's worktree, or why there is none: a round of the work is done in it, and `start` makes it.
pub fn worktree(data: &Path, work: WorkNumber) -> Result<PathBuf, String> {
    let place = place(data, work);
    if place.dir.is_dir() {
        Ok(place.dir)
    } else {
        Err(format!(
            "the work has no worktree at {}: the cell that starts the work makes it",
            place.dir.display()
        ))
    }
}

/// Checks the work before its merge: that its worktree holds nothing uncommitted, that its branch merges into the
/// main branch's head with no conflict (worked out without changing anything), and that the project's check passes
/// in the worktree. Says why where it could not check; what failed is what was found, not a reason to fail.
pub(crate) async fn check(
    project: &Project,
    data: &Path,
    work: WorkNumber,
    stop: &mut Stop,
) -> Result<Checked, String> {
    let dir = worktree(data, work)?;
    let commit = git(&dir, &["rev-parse", "HEAD"]).await?;
    let main = git(&project.repo, &["rev-parse", &format!("refs/heads/{}", project.main)]).await?;
    let checked = |failed| Checked {
        commit: commit.clone(),
        main: main.clone(),
        failed,
    };
    let changes = git(&dir, &["status", "--porcelain"]).await?;
    if !changes.is_empty() {
        return Ok(checked(Some(format!(
            "The worktree has changes that are not committed:\n{changes}"
        ))));
    }
    let merged = said(
        &project.repo,
        &["merge-tree", "--write-tree", "--name-only", &main, &commit],
    )
    .await?;
    match merged.code {
        Some(0) => {}
        Some(1) => {
            return Ok(checked(Some(format!(
                "The branch does not merge into {} at {main} without a conflict:\n{}",
                project.main, merged.out
            ))));
        }
        _ => return Err(format!("git merge-tree failed: {}", merged.err)),
    }
    match process::run(process::shell(&project.check, &dir), None, stop).await? {
        Ran::Ended { status, stdout, .. } if !status.success() => Ok(checked(Some(format!(
            "The check `{}` failed ({status}). The end of what it said:\n{}",
            project.check,
            process::tail(&stdout)
        )))),
        Ran::Ended { .. } => Ok(checked(None)),
        Ran::Stopped => Err("the check was stopped".to_string()),
    }
}

/// Merges the work's branch into the main branch in the repository's own checkout, which must be on the main branch
/// and hold no uncommitted change, and gives the head of the main branch after it: the merge commit, or, where the
/// branch holds nothing the main branch does not, the head as it was.
pub async fn merge(project: &Project, data: &Path, work: WorkNumber) -> Result<String, String> {
    let repo = &project.repo;
    let place = place(data, work);
    let on = git(repo, &["symbolic-ref", "--quiet", "--short", "HEAD"]).await?;
    if on != project.main {
        return Err(format!(
            "the repository's checkout {} is on {on}, not on the main branch {}",
            repo.display(),
            project.main
        ));
    }
    let changes = git(repo, &["status", "--porcelain", "--untracked-files=no"]).await?;
    if !changes.is_empty() {
        return Err(format!(
            "the repository's checkout {} has changes that are not committed:\n{changes}",
            repo.display()
        ));
    }
    let head = git(repo, &["rev-parse", &format!("refs/heads/{}", place.branch)]).await?;
    if said(repo, &["merge-base", "--is-ancestor", &head, "HEAD"]).await?.ok {
        // Merged already, before the process stopped, or nothing to merge: the merge commit is the newest commit of
        // the main branch with the branch's head among its later parents.
        let lines = git(repo, &["rev-list", "--first-parent", "--parents", "HEAD"]).await?;
        let merge = (lines.lines())
            .map(|line| line.split(' ').collect::<Vec<_>>())
            .find(|commits| commits.iter().skip(2).any(|parent| *parent == head))
            .map(|commits| commits[0].to_string());
        return match merge {
            Some(merge) => Ok(merge),
            None => git(repo, &["rev-parse", "HEAD"]).await,
        };
    }
    let message = format!("Merge {}", place.branch);
    if let Err(why) = git(repo, &["merge", "--no-ff", "-m", &message, &place.branch]).await {
        let _ = git(repo, &["merge", "--abort"]).await;
        return Err(why);
    }
    git(repo, &["rev-parse", "HEAD"]).await
}

#[cfg(test)]
pub(crate) mod tests;
