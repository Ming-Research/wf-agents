//! The work's place in the project's repository, with plain `git`: a branch and a worktree for each work, the check
//! before the merge, and the merge. Each is idempotent, since a round may be given again after the process started
//! again: what was done already is found and said again, not done a second time.

use std::path::{Path, PathBuf};

use store::WorkNumber;
use tokio::process::Command;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::held::Stop;
use crate::process::{self, Ran};
use crate::settings::Project;

/// The directory in the data directory that holds the works' worktrees.
pub const WORKTREES: &str = "worktrees";

/// Where a work is done: its branch and its worktree, both named by the work's number and by the installation, so
/// that an installation with a new data directory never takes up a branch another left.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Place {
    /// The branch's name, as `git worktree add` takes it to check it out.
    pub branch: String,
    /// The work's worktree.
    pub dir: PathBuf,
}

impl Place {
    /// The branch's full reference, by which every other command names it.
    pub fn reference(&self) -> String {
        format!("refs/heads/{}", self.branch)
    }
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

/// The installation's part of the names of its branches and worktrees: the first eight characters of its id.
pub fn tag(installation: &Uuid) -> String {
    installation.simple().to_string()[..8].to_string()
}

/// The work's place, by its number and the installation's tag, in the data directory given.
pub fn place(data: &Path, tag: &str, work: WorkNumber) -> Place {
    let name = format!("{}-{tag}", work.0);
    Place {
        branch: format!("work-{name}"),
        dir: data.join(WORKTREES).join(name),
    }
}

/// What a git command gave: whether it exited with 0, and its standard output and error.
struct Said {
    ok: bool,
    code: Option<i32>,
    out: String,
    err: String,
}

/// What every git command of the engine's is given before its own arguments: no file-system monitor, no hook and
/// no signing, so that nothing the repository's configuration names is run by these settings. A work's worktree
/// shares that configuration with the owner's checkout, and an agent may write it; what else of it could run a
/// program is refused before each act ([`configuration`]).
pub const SAFE: [&str; 8] = [
    "-c",
    "core.fsmonitor=false",
    "-c",
    "core.hooksPath=/dev/null",
    "-c",
    "commit.gpgSign=false",
    "-c",
    "merge.verifySignatures=false",
];

/// Runs git in the directory; says why where it cannot be started. Git is given, of the engine's environment, only
/// what the agents are given of it (`cli::KEPT` and every `LC_*`), so that no `GIT_*` variable of the engine's
/// reaches it but the one set here.
async fn said(dir: &Path, args: &[&str]) -> Result<Said, String> {
    let kept =
        std::env::vars().filter(|(name, _)| crate::cli::KEPT.contains(&name.as_str()) || name.starts_with("LC_"));
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(SAFE)
        .args(args)
        .env_clear()
        .envs(kept)
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
pub async fn start(project: &Project, place: &Place) -> Result<Started, String> {
    configuration(project, place).await?;
    let repo = &project.repo;
    let (branch, main) = (place.reference(), format!("refs/heads/{}", project.main));
    if place.dir.exists() {
        let on = git(&place.dir, &["symbolic-ref", "--quiet", "HEAD"]).await?;
        if on != branch {
            return Err(format!(
                "{} is there already, and is not the worktree of the branch {branch}: it is on {on}",
                place.dir.display(),
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
        // `worktree add` checks a branch out by its short name; given the full reference it would detach HEAD.
        if exists {
            git(repo, &["worktree", "add", &dir, &place.branch]).await?;
        } else {
            git(repo, &["worktree", "add", "-b", &place.branch, &dir, &main]).await?;
        }
    }
    let from = git(repo, &["merge-base", &main, &branch]).await?;
    Ok(Started {
        place: place.clone(),
        from,
    })
}

/// The work's worktree, or why there is none: a round of the work is done in it, and `start` makes it.
pub fn worktree(place: &Place) -> Result<PathBuf, String> {
    if place.dir.is_dir() {
        Ok(place.dir.clone())
    } else {
        Err(format!(
            "the work has no worktree at {}: the cell that starts the work makes it",
            place.dir.display()
        ))
    }
}

/// Checks the work before its merge: that its worktree holds nothing uncommitted, that its branch merges into the
/// main branch's head with no conflict (worked out without changing anything), and that the project's check passes
/// in the worktree. What is done in the repository is done holding `repository`, and the check's command is run
/// without it. Says why where it could not check, and gives none where it was stopped; what failed is what was found,
/// not a reason to fail.
pub(crate) async fn check(
    project: &Project,
    place: &Place,
    repository: &Mutex<()>,
    stop: &mut Stop,
) -> Result<Option<Checked>, String> {
    let dir = worktree(place)?;
    let (commit, main, merged) = {
        let _held = repository.lock().await;
        configuration(project, place).await?;
        let commit = git(&project.repo, &["rev-parse", &place.reference()]).await?;
        let main = git(&project.repo, &["rev-parse", &format!("refs/heads/{}", project.main)]).await?;
        let changes = git(&dir, &["status", "--porcelain"]).await?;
        if !changes.is_empty() {
            let failed = format!("The worktree has changes that are not committed:\n{changes}");
            return Ok(Some(Checked {
                commit,
                main,
                failed: Some(failed),
            }));
        }
        let args = ["merge-tree", "--write-tree", "--name-only", &main, &commit];
        let merged = said(&project.repo, &args).await?;
        (commit, main, merged)
    };
    let checked = |failed| Checked {
        commit: commit.clone(),
        main: main.clone(),
        failed,
    };
    match merged.code {
        Some(0) => {}
        Some(1) => {
            return Ok(Some(checked(Some(format!(
                "The branch does not merge into {} at {main} without a conflict:\n{}",
                project.main, merged.out
            )))));
        }
        _ => return Err(format!("git merge-tree failed: {}", merged.err)),
    }
    match process::run(process::shell(&project.check, &dir), None, None, stop).await? {
        Ran::Ended { status, stdout, .. } if !status.success() => Ok(Some(checked(Some(format!(
            "The check `{}` failed ({status}). The end of what it said:\n{}",
            project.check,
            process::tail(&stdout)
        ))))),
        Ran::Ended { .. } => Ok(Some(checked(None))),
        Ran::Stopped => Ok(None),
    }
}

/// Merges the work's branch into the main branch in the repository's own checkout, which must be on the main branch,
/// hold no uncommitted change and no merge under way, and gives the head of the main branch after it: the merge
/// commit, or, where the branch holds nothing the main branch does not, the head as it was. Where `checked` names the
/// commit the check passed, the branch's head must be that commit, and it is what is merged. A merge is refused while
/// the repository's own configuration names a program git could run ([`configuration`]), as every act is. Where the
/// merge fails, the merge it began is taken back.
pub async fn merge(project: &Project, place: &Place, checked: Option<&str>) -> Result<String, String> {
    let repo = &project.repo;
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
    configuration(project, place).await?;
    let merging = || async {
        said(repo, &["rev-parse", "--quiet", "--verify", "MERGE_HEAD"])
            .await
            .map(|s| s.ok)
    };
    if merging().await? {
        return Err(format!(
            "a merge is under way in the repository's checkout {}",
            repo.display()
        ));
    }
    let head = git(repo, &["rev-parse", &place.reference()]).await?;
    if let Some(checked) = checked
        && head != checked
    {
        return Err(format!(
            "the branch {} is at {head}, not at {checked}, the commit the check passed: the work is checked again \
             before it merges",
            place.reference()
        ));
    }
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
    let args = ["merge", "--no-ff", "-m", &message, &head];
    if let Err(why) = git(repo, &args).await {
        if merging().await? {
            let _ = git(repo, &["merge", "--abort"]).await;
        }
        return Err(why);
    }
    git(repo, &["rev-parse", "HEAD"]).await
}

/// Refuses, saying each key, a repository whose own configuration could make git run a program during the engine's
/// commands: the keys of [`runs_code`] in its local and worktree scopes, and in what a local `include.path` or
/// `includeIf` brings in, read in the owner's checkout and in the work's worktree where it is made. A key that matches
/// one of the project's `allowed_config` (a whole key, or a beginning followed by `*`) is let be, for a repository that
/// needs Git LFS, git-crypt or git-annex. This narrows the gap and does not close it: an agent that may run `git` can
/// write the repository's configuration at any moment, between this check and the command it guards. Closing it needs
/// the agents confined to their worktree by the system, which is not built.
pub async fn configuration(project: &Project, place: &Place) -> Result<(), String> {
    let mut dirs = vec![project.repo.clone()];
    if place.dir.is_dir() {
        dirs.push(place.dir.clone());
    }
    let mut found = std::collections::BTreeSet::new();
    for dir in &dirs {
        let listed = git(dir, &["config", "--list", "--show-scope", "--includes"]).await?;
        for line in listed.lines() {
            let Some((scope, entry)) = line.split_once('\t') else {
                continue;
            };
            if scope != "local" && scope != "worktree" {
                continue;
            }
            let key = entry.split_once('=').map_or(entry, |(key, _)| key).to_ascii_lowercase();
            if runs_code(&key) && !allowed(&project.allowed_config, &key) {
                found.insert(format!("{key} ({scope})"));
            }
        }
    }
    if found.is_empty() {
        return Ok(());
    }
    let found: Vec<String> = found.into_iter().collect();
    Err(format!(
        "the repository's own configuration in {} names what git could run in the engine's commands: {}. Take each \
         out, or, where the repository needs it, allow it in [project] allowed_config",
        project.repo.display(),
        found.join(", ")
    ))
}

/// Whether a key, in lower case, of a repository's configuration names a program git may run, or brings in another
/// file of configuration.
fn runs_code(key: &str) -> bool {
    const KEYS: [&str; 13] = [
        "core.fsmonitor",
        "core.hookspath",
        "diff.external",
        "gpg.program",
        "include.path",
        "extensions.worktreeconfig",
        "core.sshcommand",
        "core.gitproxy",
        "credential.helper",
        "core.editor",
        "sequence.editor",
        "core.pager",
        "core.askpass",
    ];
    let within = |section: &str, name: &str| key.starts_with(section) && key.ends_with(name);
    KEYS.contains(&key)
        || key.starts_with("filter.")
        || key.starts_with("includeif.")
        || within("merge.", ".driver")
        || within("diff.", ".textconv")
        || within("diff.", ".command")
        || within("gpg.", ".program")
        || within("credential.", ".helper")
}

/// Whether the key matches one of the patterns: the whole key, or, for a pattern ending in `*`, its beginning.
fn allowed(patterns: &[String], key: &str) -> bool {
    patterns.iter().any(|pattern| {
        let pattern = pattern.to_ascii_lowercase();
        match pattern.strip_suffix('*') {
            Some(beginning) => key.starts_with(beginning),
            None => key == pattern,
        }
    })
}

#[cfg(test)]
pub(crate) mod tests;
