//! The work's place in the project's local repository, with plain `git`: a branch and a worktree for each work, what
//! the check before the merge finds in them, and the merge into the repository's own main branch. Each is idempotent,
//! since a round may be given again after the process started again: what was done already is found and said again,
//! not done a second time.

use std::path::{Path, PathBuf};

use store::WorkNumber;
use tokio::process::Command;
use uuid::Uuid;

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
pub(crate) struct Said {
    pub(crate) ok: bool,
    pub(crate) code: Option<i32>,
    pub(crate) out: String,
    pub(crate) err: String,
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
pub(crate) async fn said(dir: &Path, args: &[&str]) -> Result<Said, String> {
    said_with(dir, &With::default(), args).await
}

/// How long a git command may take before it is given up: a fetch or a push of a large repository over a slow
/// connection, at the most. A connection that stalls is given up sooner, by the low-speed limit pushes and fetches
/// are given.
pub const LIMIT: std::time::Duration = std::time::Duration::from_secs(15 * 60);

/// What a git command is given beside its arguments: settings before them, as `-c` pairs, and variables of its
/// environment beside those [`said`] keeps. Neither is said where the command fails, so that a credential given in
/// the environment is said nowhere.
#[derive(Clone, Debug, Default)]
pub(crate) struct With {
    pub(crate) settings: Vec<String>,
    pub(crate) env: Vec<(String, String)>,
}

/// Runs git in the directory as [`said`] does, with what is given beside its arguments.
pub(crate) async fn said_with(dir: &Path, with: &With, args: &[&str]) -> Result<Said, String> {
    let kept =
        std::env::vars().filter(|(name, _)| crate::cli::KEPT.contains(&name.as_str()) || name.starts_with("LC_"));
    let settings = (with.settings.iter()).flat_map(|setting| ["-c", setting.as_str()]);
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(dir)
        .args(SAFE)
        .args(settings)
        .args(args)
        .env_clear()
        .envs(kept)
        .envs(with.env.iter().cloned())
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(std::process::Stdio::null());
    let output = crate::process::output(command, LIMIT).await?;
    Ok(Said {
        ok: output.status.success(),
        code: output.status.code(),
        out: String::from_utf8_lossy(&output.stdout).trim_end().to_string(),
        err: String::from_utf8_lossy(&output.stderr).trim_end().to_string(),
    })
}

/// Runs git in the directory and gives its standard output; or says what failed, with what git said.
pub(crate) async fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    git_with(dir, &With::default(), args).await
}

/// Runs git in the directory as [`git`] does, with what is given beside its arguments.
pub(crate) async fn git_with(dir: &Path, with: &With, args: &[&str]) -> Result<String, String> {
    let said = said_with(dir, with, args).await?;
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

/// Makes the work's branch from `from`, the head of main as the host gives it, and its worktree, keeping both where
/// they are made already.
pub async fn start(project: &Project, place: &Place, from: &str) -> Result<Started, String> {
    configuration(project, place).await?;
    let repo = &project.repo;
    let branch = place.reference();
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
            git(repo, &["worktree", "add", "-b", &place.branch, &dir, from]).await?;
        }
    }
    let from = git(repo, &["merge-base", from, &branch]).await?;
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

/// What a work's worktree holds that is not committed, as `git status --porcelain` lists it; nothing where all is.
pub(crate) async fn uncommitted(dir: &Path) -> Result<String, String> {
    git(dir, &["status", "--porcelain"]).await
}

/// The files of a conflict between the commit and main's head, worked out without changing anything; none where the
/// commit merges into main cleanly.
pub(crate) async fn conflicts(project: &Project, main: &str, commit: &str) -> Result<Option<String>, String> {
    let args = ["merge-tree", "--write-tree", "--name-only", main, commit];
    let merged = said(&project.repo, &args).await?;
    match merged.code {
        Some(0) => Ok(None),
        Some(1) => Ok(Some(merged.out)),
        _ => Err(format!("git merge-tree failed: {}", merged.err)),
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
