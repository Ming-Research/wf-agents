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

/// What every git command of the engine's is given before its own arguments: no file-system monitor, no hook, no
/// signing, and no going into submodules, so that nothing the repository's configuration names is run by these
/// settings, and nothing a submodule's own configuration names, which no check of the engine's reads. A work's
/// worktree shares that configuration with the owner's checkout, and an agent may write it; what else of it could run
/// a program is refused before each act ([`configuration`]).
pub const SAFE: [&str; 14] = [
    "-c",
    "core.fsmonitor=false",
    "-c",
    "core.hooksPath=/dev/null",
    "-c",
    "commit.gpgSign=false",
    "-c",
    "merge.verifySignatures=false",
    "-c",
    "submodule.recurse=false",
    "-c",
    "fetch.recurseSubmodules=false",
    "-c",
    "push.recurseSubmodules=no",
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

/// The code before a work's change with some files of the head of its branch put on it, as one commit, which the
/// agents' tool `check_before` checks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Before {
    /// The head of the work's branch the files are taken from.
    pub head: String,
    /// The code before the change: the commit of main the branch's change begins from.
    pub base: String,
    /// The files put on it, as the head holds them.
    pub files: Vec<String>,
    /// The commit made of the base with the files put on it.
    pub commit: String,
}

/// Makes the commit of the code before the work's change, its branch's merge base with `main`, with these files put on
/// it as the head of the branch holds them; or says why it is not made, in words for the agent that asked: a file the
/// head does not hold, or one whose changes are not committed. Made again of the same files on the same base, it is
/// the same commit. Nothing changes in the worktree. Refused where the repository's configuration is ([`configuration`]).
pub(crate) async fn before(
    project: &Project,
    place: &Place,
    main: &str,
    files: &[String],
) -> Result<Result<Before, String>, String> {
    let dir = worktree(place)?;
    configuration(project, place).await?;
    on_branch(place).await?;
    let repo = &project.repo;
    let head = git(repo, &["rev-parse", &place.reference()]).await?;
    let base = git(repo, &["merge-base", &head, main]).await?;
    let mut wrong = Vec::new();
    let mut entries = Vec::new();
    for file in files {
        // `<mode> blob <id>\t<path>` for a file the head holds at that path, and nothing else.
        let listed = git(repo, &["ls-tree", "-z", &head, "--", file]).await?;
        let entry = listed.trim_end_matches('\0').split_once('\t');
        match entry.map(|(meta, path)| (meta.split(' ').collect::<Vec<_>>(), path)) {
            Some((meta, path)) if path == file && meta.len() == 3 && meta[1] == "blob" => {
                entries.push(format!("{},{},{file}", meta[0], meta[2]));
            }
            _ => wrong.push(format!("{file} is not a file of your last commit {head}")),
        }
    }
    let mut args = vec!["status", "--porcelain", "--"];
    args.extend(files.iter().map(String::as_str));
    let changed = git(&dir, &args).await?;
    if !changed.is_empty() {
        wrong.push(format!(
            "the worktree holds changes to these that are not committed, and the files are taken as your last commit \
             holds them: commit them first\n{changed}"
        ));
    }
    if !wrong.is_empty() {
        return Ok(Err(format!("Nothing was checked: {}.", wrong.join("; "))));
    }
    // The tree is made in an index of its own, so that neither the worktree nor its index changes.
    let index = std::env::temp_dir().join(format!("wf-agents-before-{}.index", Uuid::new_v4().simple()));
    let own = With {
        settings: Vec::new(),
        env: vec![("GIT_INDEX_FILE".to_string(), text(&index))],
    };
    let tree = async {
        git_with(repo, &own, &["read-tree", &base]).await?;
        for entry in &entries {
            git_with(repo, &own, &["update-index", "--add", "--cacheinfo", entry]).await?;
        }
        git_with(repo, &own, &["write-tree"]).await
    }
    .await;
    let _ = tokio::fs::remove_file(&index).await;
    let tree = tree?;
    // By the engine, at the time of the base, so that the same files on the same base make the same commit, whose CI
    // is then read and not run again.
    let date = git(repo, &["log", "-1", "--format=%cI", &base]).await?;
    let mut env = Vec::new();
    for who in ["AUTHOR", "COMMITTER"] {
        env.push((format!("GIT_{who}_NAME"), "wf-agents".to_string()));
        env.push((format!("GIT_{who}_EMAIL"), ENGINE_EMAIL.to_string()));
        env.push((format!("GIT_{who}_DATE"), date.clone()));
    }
    let made = With {
        settings: Vec::new(),
        env,
    };
    let message = format!("The code before the change of {}, with its tests", place.branch);
    let commit = git_with(repo, &made, &["commit-tree", &tree, "-p", &base, "-m", &message]).await?;
    Ok(Ok(Before {
        head,
        base,
        files: files.to_vec(),
        commit,
    }))
}

/// Makes the worktree where the check before the change runs at the commit ([`before_dir`]), taking away one left
/// there, as by a process of the engine's that ended while the check ran.
pub(crate) async fn check_out_before(project: &Project, place: &Place, commit: &str) -> Result<PathBuf, String> {
    let repo = &project.repo;
    let dir = before_dir(place, commit);
    if dir.exists() {
        take_away(project, &dir).await?;
    }
    git(repo, &["worktree", "prune"]).await?;
    git(repo, &["worktree", "add", "--quiet", "--detach", &text(&dir), commit]).await?;
    Ok(dir)
}

/// The worktrees of the work's checks before the change that are there, as [`before_dir`] names them.
pub(crate) fn befores(place: &Place) -> Vec<PathBuf> {
    let beginning = format!("{}-", before_dir(place, "").display());
    let Some(parent) = place.dir.parent() else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(parent) else {
        return Vec::new();
    };
    (entries.flatten())
        .map(|entry| entry.path())
        .filter(|path| path.display().to_string().starts_with(&beginning))
        .collect()
}

/// Takes away the worktree of a check before the change, with what it holds.
pub(crate) async fn take_away(project: &Project, dir: &Path) -> Result<(), String> {
    let removed = said(&project.repo, &["worktree", "remove", "--force", &text(dir)]).await?;
    if !removed.ok && dir.exists() {
        tokio::fs::remove_dir_all(dir)
            .await
            .map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    git(&project.repo, &["worktree", "prune"]).await.map(drop)
}

/// Where the check before the change of the work runs at the commit, beside its worktree, while it runs: named by the
/// work and the commit, so that checks of other commits asked at once have each their own.
pub fn before_dir(place: &Place, commit: &str) -> PathBuf {
    let name = (place.dir.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let short = &commit[..commit.len().min(12)];
    let dir = place.dir.with_file_name(format!("{name}-before"));
    if short.is_empty() {
        dir
    } else {
        dir.with_file_name(format!("{name}-before-{short}"))
    }
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

/// Who the commits the engine makes are by: the merges of main into a work's branch that the check makes.
const ENGINE: [&str; 2] = ["user.name=wf-agents", "user.email=wf-agents@localhost"];

/// The address of the engine's identity, by which a commit of its own is known.
const ENGINE_EMAIL: &str = "wf-agents@localhost";

/// The words the engine's merge of main into a work's branch begins and ends with.
fn taking_in(name: &str, main: &str) -> String {
    format!("Merge {name} at {main} into the work's branch")
}

/// Merges main's head into the branch checked out in the worktree, which merges it without a conflict, as a commit
/// of the engine's, and gives the branch's head after it and whether it made that commit; where the branch holds
/// main's head already, its head as it is, so that a merge done before the engine stopped is not done again. `name`
/// is the main branch's name.
pub(crate) async fn take_in(dir: &Path, main: &str, name: &str) -> Result<(String, bool), String> {
    if said(dir, &["merge-base", "--is-ancestor", main, "HEAD"]).await?.ok {
        // The engine's own merge of this main, made by a check that stopped before it was recorded, is still the
        // engine's: a merge of main by the engine's identity.
        let head = git(dir, &["log", "-1", "--format=%H %ae %P", "HEAD"]).await?;
        let words: Vec<&str> = head.split(' ').collect();
        let ours = matches!(words.as_slice(), [_, email, _, merged] if *email == ENGINE_EMAIL && *merged == main);
        return Ok((words[0].to_string(), ours));
    }
    let with = With {
        settings: ENGINE.map(str::to_string).to_vec(),
        env: Vec::new(),
    };
    let message = taking_in(name, main);
    let args = ["merge", "--no-ff", "--no-edit", "-m", &message, main];
    if let Err(why) = git_with(dir, &with, &args).await {
        if said(dir, &["rev-parse", "--quiet", "--verify", "MERGE_HEAD"]).await?.ok {
            let _ = git(dir, &["merge", "--abort"]).await;
        }
        return Err(why);
    }
    Ok((git(dir, &["rev-parse", "HEAD"]).await?, true))
}

/// Whether a merge is under way in the worktree, begun and not committed: by an agent, or by the engine before it
/// stopped. Only whoever began it can tell what it holds, so the engine never takes one back.
pub(crate) async fn merging(dir: &Path) -> Result<bool, String> {
    Ok(said(dir, &["rev-parse", "--quiet", "--verify", "MERGE_HEAD"]).await?.ok)
}

/// That the worktree is on the work's branch, or why it is not: what the engine commits there must go on the branch.
pub(crate) async fn on_branch(place: &Place) -> Result<(), String> {
    let on = said(&place.dir, &["symbolic-ref", "--quiet", "HEAD"]).await?;
    if on.ok && on.out == place.reference() {
        return Ok(());
    }
    let at = if on.ok { on.out } else { "no branch".to_string() };
    Err(format!(
        "the worktree {} is on {at}, not on the work's branch {}: the check commits nothing there",
        place.dir.display(),
        place.reference()
    ))
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
    if checked.is_some() && !said(repo, &["merge-base", "--is-ancestor", "HEAD", &head]).await?.ok {
        let main = git(repo, &["rev-parse", "HEAD"]).await?;
        return Err(crate::host::moved(&project.main, &main, &head));
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
    refused(project, &dirs).await
}

/// Refuses, as [`configuration`] does, a repository whose own configuration could make git run a program, read in the
/// owner's checkout alone: for what the engine does there for no work, such as the watch of main.
pub async fn repository_configuration(project: &Project) -> Result<(), String> {
    refused(project, std::slice::from_ref(&project.repo)).await
}

/// Refuses what the configuration read in these directories names that git could run.
async fn refused(project: &Project, dirs: &[PathBuf]) -> Result<(), String> {
    let mut found = std::collections::BTreeSet::new();
    for dir in dirs {
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
        || within("remote.", ".uploadpack")
        || within("remote.", ".receivepack")
        || within("remote.", ".vcs")
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
