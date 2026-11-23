//! The work's place in the project's local repository, with plain `git`: a branch and a worktree for each work, what
//! the check before the merge finds in them, and the merge into the repository's own main branch. Each is idempotent,
//! since a round may be given again after the process started again: what was done already is found and said again,
//! not done a second time.

use std::path::{Path, PathBuf};

use store::WorkNumber;
use tokio::process::Command;
use uuid::Uuid;

use crate::commit_lines::Whose;
use crate::host::NotMerged;
use crate::settings::Project;
use flow::agents::{Commits, fill};

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

/// Where the work is done, the commit of the main branch it started from (the latest commit of main its branch holds,
/// whatever the branch was made at), and whether its branch was there already, as a work's that was cancelled and
/// reopened, whose cancel kept its branch, or one whose start was cut off.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Started {
    pub place: Place,
    pub from: String,
    pub kept: bool,
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

/// Whether the name is one the engine gives a work's branch, `work-<the work's number>-<an installation's tag>`, of any
/// installation: a branch of that name is a work's, and what is on it is not the engine's to close.
pub fn is_a_works_branch(name: &str) -> bool {
    let Some((number, tag)) = name.strip_prefix("work-").and_then(|rest| rest.split_once('-')) else {
        return false;
    };
    !number.is_empty()
        && number.chars().all(|c| c.is_ascii_digit())
        && tag.len() == 8
        && tag.chars().all(|c| c.is_ascii_hexdigit())
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
    let output = crate::process::output(command(dir, with, args), LIMIT).await?;
    Ok(Said {
        ok: output.status.success(),
        code: output.status.code(),
        out: String::from_utf8_lossy(&output.stdout).trim_end().to_string(),
        err: String::from_utf8_lossy(&output.stderr).trim_end().to_string(),
    })
}

/// The git command [`said_with`] runs in the directory: git by its path ([`crate::programs`]), given of the engine's
/// environment only what [`said`] says.
fn command(dir: &Path, with: &With, args: &[&str]) -> Command {
    let settings = (with.settings.iter()).flat_map(|setting| ["-c", setting.as_str()]);
    let mut command = crate::programs::command("git");
    command
        .arg("-C")
        .arg(dir)
        .args(SAFE)
        .args(settings)
        .args(args)
        .env_clear()
        .envs(crate::programs::kept().iter().cloned())
        .envs(with.env.iter().cloned())
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(std::process::Stdio::null());
    command
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

/// The content of each of these files of the commit, in their order, as one run of `git cat-file --batch` in the
/// directory reads them; none of one the commit does not hold. The paths are given on git's input, never as its
/// arguments, so that no path is read as an option; one that holds a line break, which that input cannot carry, is
/// refused.
pub(crate) async fn blobs(dir: &Path, commit: &str, paths: &[&str]) -> Result<Vec<Option<Vec<u8>>>, String> {
    use tokio::io::AsyncWriteExt;
    if let Some(path) = paths.iter().find(|p| p.contains('\n')) {
        return Err(format!(
            "git cat-file cannot be given the path {path:?}, which holds a line break"
        ));
    }
    let mut command = crate::programs::command("git");
    command
        .arg("-C")
        .arg(dir)
        .args(SAFE)
        .args(["cat-file", "--batch"])
        .env_clear()
        .envs(crate::programs::kept().iter().cloned())
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    let mut child = command.spawn().map_err(|e| format!("git cannot be run: {e}"))?;
    let mut stdin = child.stdin.take().ok_or("git cat-file was given no input")?;
    let input: String = paths.iter().map(|path| format!("{commit}:{path}\n")).collect();
    // The input is written while the output is read: git answers each line as it reads it, and would wait for its
    // output to be read while its input waits to be written. It is closed once written, so that git ends.
    let writing = async move {
        stdin
            .write_all(input.as_bytes())
            .await
            .map_err(|e| format!("git cat-file: {e}"))?;
        drop(stdin);
        Ok::<(), String>(())
    };
    let (written, output) = tokio::join!(writing, async {
        match tokio::time::timeout(LIMIT, child.wait_with_output()).await {
            Ok(output) => output.map_err(|e| format!("git cat-file: {e}")),
            Err(_) => Err(format!("git cat-file did not end within {} seconds", LIMIT.as_secs())),
        }
    });
    written?;
    let output = output?;
    if !output.status.success() {
        let said = String::from_utf8_lossy(&output.stderr).trim_end().to_string();
        return Err(format!("git cat-file --batch failed in {}: {said}", dir.display()));
    }
    batch(&output.stdout, paths.len())
}

/// The objects a run of `git cat-file --batch` gave for this many lines of its input, in their order: each one's
/// content after the line `<id> <type> <size>`, and none of one it said was missing.
fn batch(out: &[u8], count: usize) -> Result<Vec<Option<Vec<u8>>>, String> {
    let mut objects = Vec::with_capacity(count);
    let mut rest = out;
    for _ in 0..count {
        let end = (rest.iter().position(|b| *b == b'\n')).ok_or("git cat-file --batch ended before its last object")?;
        let header = String::from_utf8_lossy(&rest[..end]).into_owned();
        rest = &rest[end + 1..];
        if header.ends_with(" missing") || header.ends_with(" ambiguous") {
            objects.push(None);
            continue;
        }
        let size = (header.rsplit(' ').next())
            .and_then(|size| size.parse::<usize>().ok())
            .ok_or_else(|| format!("git cat-file --batch said {header:?}, which gives no size"))?;
        let content = rest.get(..size).ok_or("git cat-file --batch ended within an object")?;
        objects.push(Some(content.to_vec()));
        rest = rest.get(size + 1..).unwrap_or_default();
    }
    Ok(objects)
}

/// The path as git is given it: the worktree's path must not be read against the repository's directory.
fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// Whether the work's branch is made already, as a work's that was cancelled and reopened, or one whose start was cut
/// off, has it.
pub(crate) async fn branch_made(project: &Project, place: &Place) -> Result<bool, String> {
    let found = said(&project.repo, &["rev-parse", "--verify", "--quiet", &place.reference()]).await?;
    Ok(found.ok)
}

/// Makes the work's branch at `at`, the commit its work starts from: the head of main as the host gives it, or what its
/// item continues; and its worktree, keeping both where they are made already. `main` is the head of main, by which
/// the commit of main the branch started from is worked out.
pub async fn start(project: &Project, place: &Place, at: &str, main: &str) -> Result<Started, String> {
    configuration(project, place).await?;
    let repo = &project.repo;
    let branch = place.reference();
    let kept = said(repo, &["rev-parse", "--verify", "--quiet", &branch]).await?.ok;
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
        // `worktree add` checks a branch out by its short name; given the full reference it would detach HEAD.
        if kept {
            git(repo, &["worktree", "add", &dir, &place.branch]).await?;
        } else {
            // A branch with no history in common with main could never be merged into it. It is refused before the
            // branch is made: a branch made is taken as it is by the next start, which reads nothing of what the item
            // continues, so the work would stay stuck at it.
            let shared = said(repo, &["merge-base", main, at]).await?;
            match shared.code {
                Some(0) => {}
                Some(1) => {
                    return Err(format!(
                        "the commit {at} has no history in common with main, which is {main}: a branch made there \
                         could never be merged into main, so the work's branch is not made"
                    ));
                }
                _ => return Err(format!("git merge-base {main} {at}: {}", shared.err)),
            }
            git(repo, &["worktree", "add", "-b", &place.branch, &dir, at]).await?;
        }
    }
    let from = git(repo, &["merge-base", main, &branch]).await?;
    Ok(Started {
        place: place.clone(),
        from,
        kept,
    })
}

/// Whether there is a symbolic link at the path itself, read by the link's own metadata and not its target's.
pub(crate) fn a_link(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_symlink())
}

/// Takes the work's worktree away, uncommitted changes and all, and the checkout the local check ran on if a stopped
/// engine left one ([`checking`]), and keeps its branch, whose commits are the work's: a worktree is a place to work in,
/// and the work has ended. One taken away already is nothing to do; what is at its place and is no worktree of the
/// repository, or is a symbolic link, which git would follow to whatever it leads to, another work's worktree among
/// them, was not made by the engine, and is left as it is. Refused, as every act is, while the repository's
/// configuration names a program git could run ([`configuration`]).
pub async fn remove(project: &Project, place: &Place) -> Result<(), String> {
    configuration(project, place).await?;
    take_away(project, &place.dir).await?;
    take_away(project, &checking(place)).await?;
    // A worktree whose directory was taken away by hand is named by the repository until it is pruned.
    git(&project.repo, &["worktree", "prune"]).await?;
    Ok(())
}

/// Takes away the worktree of the repository that is at this directory, as [`remove`] does, and prunes nothing but
/// the name of one whose directories a command left unwritable, which it takes away itself.
async fn take_away(project: &Project, place_dir: &Path) -> Result<(), String> {
    let repo = &project.repo;
    if place_dir.exists() && !a_link(place_dir) {
        let dir = tokio::fs::canonicalize(place_dir)
            .await
            .map_err(|e| format!("{}: {e}", place_dir.display()))?;
        // Asked of git's list rather than read from what it answers, which is in the language of the machine's locale.
        let listed = git(repo, &["worktree", "list", "--porcelain"]).await?;
        let mut ours = false;
        for listed in listed.lines().filter_map(|line| line.strip_prefix("worktree ")) {
            ours |= tokio::fs::canonicalize(listed).await.is_ok_and(|listed| listed == dir);
        }
        if ours {
            let removed = git(repo, &["worktree", "remove", "--force", &text(place_dir)]).await;
            if let Err(why) = removed {
                // A command run in the worktree may leave a directory that does not let its owner take a file out of
                // it, which git cannot take away, and git has taken away part of the worktree by then, so that it
                // cannot be asked again. The engine made the worktree and owns it: where that was why git failed, it
                // opens the directories up, takes the rest away itself and has the repository drop its name. Any other
                // failure is said as git says it.
                let opened = dir.clone();
                let opened_up = tokio::task::spawn_blocking(move || open_up(&opened)).await;
                if !opened_up.unwrap_or(false) {
                    return Err(why);
                }
                tokio::fs::remove_dir_all(&dir)
                    .await
                    .map_err(|e| format!("{}: {e}", dir.display()))?;
                git(repo, &["worktree", "prune"]).await?;
            }
        }
    }
    Ok(())
}

/// Gives the owner every right over the directory and each directory under it, links not followed, so that what is in
/// them can be taken away; whether any of them lacked one. What cannot be changed is left: the removal that follows
/// says what still stands in its way.
fn open_up(dir: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let Ok(meta) = std::fs::symlink_metadata(dir) else {
            return false;
        };
        if !meta.is_dir() {
            return false;
        }
        let mut permissions = meta.permissions();
        let lacked = permissions.mode() & 0o700 != 0o700;
        if lacked {
            permissions.set_mode(permissions.mode() | 0o700);
            let _ = std::fs::set_permissions(dir, permissions);
        }
        let Ok(entries) = std::fs::read_dir(dir) else {
            return lacked;
        };
        let mut opened = lacked;
        for entry in entries.flatten() {
            opened |= open_up(&entry.path());
        }
        opened
    }
    #[cfg(not(unix))]
    {
        let _ = dir;
        false
    }
}

/// Where the local check runs, beside the work's worktree and named after it: a checkout of the commit checked and of
/// nothing else, so that what the worktree holds besides the commit is no part of what is judged. One place for each
/// work, so that a checkout a stopped engine left is found and taken away before the next is made.
pub(crate) fn checking(place: &Place) -> PathBuf {
    let mut dir = place.dir.clone().into_os_string();
    dir.push(".check");
    PathBuf::from(dir)
}

/// Makes the checkout of the commit the local check runs on ([`checking`]), detached from every branch, and gives its
/// directory. Refused, as every act is, while the repository's configuration names a program git could run
/// ([`configuration`]), since making it runs git's filters.
pub(crate) async fn check_out(project: &Project, place: &Place, commit: &str) -> Result<PathBuf, String> {
    configuration(project, place).await?;
    let dir = checking(place);
    take_away(project, &dir).await?;
    // `--force`: the repository may name a checkout of this place whose directory a stopped engine's run lost.
    git(
        &project.repo,
        &["worktree", "add", "--detach", "--force", &text(&dir), commit],
    )
    .await?;
    Ok(dir)
}

/// Takes away the checkout the local check ran on ([`check_out`]); one taken away already is nothing to do.
pub(crate) async fn take_away_check(project: &Project, place: &Place) -> Result<(), String> {
    take_away(project, &checking(place)).await
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

/// What a work's worktree holds that is not committed, as `git status --porcelain` lists it; nothing where all is. A
/// file git does not track is not a change: it is in no commit, and is left as it is ([`untracked`]).
pub(crate) async fn uncommitted(dir: &Path) -> Result<String, String> {
    git(dir, &["status", "--porcelain", "--untracked-files=no"]).await
}

/// The files and directories of the worktree that git does not track and does not ignore, one to a line, a directory
/// that holds only such files as its own name with its trailing slash; nothing where there are none.
pub(crate) async fn untracked(dir: &Path) -> Result<String, String> {
    git(dir, &["ls-files", "--others", "--exclude-standard", "--directory"]).await
}

/// The words the check's merge of main into a work's branch is made with, as the project's `commits` says: a run's
/// merge has words of its own, so that neither takes the other's for its own.
fn taking_in(project: &Project, main: &str, whose: Whose) -> String {
    let words = fill(&project.commits.check, &[("main", &project.main), ("head", main)]);
    whose.ending(&project.commits, &words)
}

/// Merges main's head into the branch checked out in the worktree, which merges it without a conflict, as a commit
/// of the engine's, and gives the branch's head after it and whether the check made that commit; where the branch
/// holds main's head already, its head as it is, so that a merge done before the engine stopped is not done again.
pub(crate) async fn take_in(dir: &Path, main: &str, project: &Project, whose: Whose) -> Result<(String, bool), String> {
    let message = taking_in(project, main, whose);
    let commits = &project.commits;
    if said(dir, &["merge-base", "--is-ancestor", main, "HEAD"]).await?.ok {
        // The check's own merge of this main, made by a check that stopped before it was recorded, is still its own.
        let ours = merged_as(dir, main, &message, commits).await?.is_some();
        return Ok((git(dir, &["rev-parse", "HEAD"]).await?, ours));
    }
    Ok((merge_as(dir, main, &message, commits).await?, true))
}

/// The head of the branch checked out in the worktree and its first parent, where the head is the engine's merge of
/// main's head at `main` made with this message, whole and not its first line alone, by the address `commits` gives;
/// none where it is not. It says only that a run, or the check, made the merge before the engine stopped, which what
/// the round is told of it rests on: whether a merge brings only main's code is known by its content alone
/// (`crate::head`).
pub(crate) async fn merged_as(
    dir: &Path,
    main: &str,
    message: &str,
    commits: &Commits,
) -> Result<Option<(String, String)>, String> {
    let head = git(dir, &["log", "-1", "--format=%H%x00%ae%x00%P%x00%B", "HEAD"]).await?;
    let fields: Vec<&str> = head.splitn(4, '\0').collect();
    let [commit, email, parents, body] = fields.as_slice() else {
        return Ok(None);
    };
    let parents: Vec<&str> = parents.split(' ').collect();
    Ok(match parents.as_slice() {
        [first, merged] if *email == commits.email && *merged == main && body.trim_end() == message.trim_end() => {
            Some((commit.to_string(), first.to_string()))
        }
        _ => None,
    })
}

/// Merges main's head at `main` into the branch checked out in the worktree, which merges it without a conflict, as a
/// commit of the engine's made with these words, by whom `commits` says, and gives the branch's head after it; a merge
/// that fails is taken back.
pub(crate) async fn merge_as(dir: &Path, main: &str, message: &str, commits: &Commits) -> Result<String, String> {
    let with = With {
        settings: vec![
            format!("user.name={}", commits.name),
            format!("user.email={}", commits.email),
        ],
        env: Vec::new(),
    };
    let args = ["merge", "--no-ff", "--no-edit", "-m", message, main];
    if let Err(why) = git_with(dir, &with, &args).await {
        if said(dir, &["rev-parse", "--quiet", "--verify", "MERGE_HEAD"]).await?.ok {
            let _ = git(dir, &["merge", "--abort"]).await;
        }
        return Err(why);
    }
    git(dir, &["rev-parse", "HEAD"]).await
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

/// A conflict between a commit and main's head.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Conflict {
    /// The files in conflict, one on each line.
    pub(crate) files: String,
    /// What git says of each conflict's kind, a line each.
    pub(crate) kinds: String,
}

/// The conflict between the commit and main's head, worked out without changing anything; none where the commit
/// merges into main cleanly.
pub(crate) async fn conflicts(project: &Project, main: &str, commit: &str) -> Result<Option<Conflict>, String> {
    let args = ["merge-tree", "--write-tree", "--name-only", main, commit];
    let merged = said(&project.repo, &args).await?;
    match merged.code {
        Some(0) => Ok(None),
        // The tree it wrote, the files in conflict, then a blank line before what it says of each.
        Some(1) => {
            let mut lines = merged.out.lines().skip(1);
            let files: Vec<&str> = lines.by_ref().take_while(|line| !line.is_empty()).collect();
            let kinds: Vec<&str> = lines.filter(|line| line.starts_with("CONFLICT")).collect();
            Ok(Some(Conflict {
                files: files.join("\n"),
                kinds: kinds.join("\n"),
            }))
        }
        _ => Err(format!("git merge-tree failed: {}", merged.err)),
    }
}

/// The messages of the commits the head holds that `base` does not, oldest first, those that merge excluded: what a
/// work's own commits say, which the squash that merges the work gathers.
pub(crate) async fn work_messages(repo: &Path, base: &str, head: &str) -> Result<Vec<String>, String> {
    let range = format!("{base}..{head}");
    let log = git(repo, &["log", "--reverse", "--no-merges", "--format=%B%x00", &range]).await?;
    Ok(log
        .split('\0')
        .map(|message| message.trim().to_string())
        .filter(|message| !message.is_empty())
        .collect())
}

/// Merges the work's branch into the main branch in the repository's own checkout, which must be on the main branch,
/// hold no uncommitted change and no merge under way, by a commit named `name` in the words of the project's
/// `commits` and ending with the lines that name the item and the work, and gives the head of the main branch after
/// it: the merge commit, or, where the branch holds nothing the main branch does not, the head as it was. Where
/// `checked` names the commit the check passed, the branch's head must be that commit, and it is what is merged. A
/// merge is refused while the repository's own configuration names a program git could run ([`configuration`]), as
/// every act is. Where the merge fails, the merge it began is taken back.
pub async fn merge(
    project: &Project,
    place: &Place,
    name: &str,
    whose: Whose,
    checked: Option<&str>,
) -> Result<String, NotMerged> {
    let repo = &project.repo;
    let on = git(repo, &["symbolic-ref", "--quiet", "--short", "HEAD"]).await?;
    if on != project.main {
        return Err(NotMerged::refused(format!(
            "the repository's checkout {} is on {on}, not on the main branch {}",
            repo.display(),
            project.main
        )));
    }
    let changes = uncommitted(repo).await?;
    if !changes.is_empty() {
        return Err(NotMerged::refused(format!(
            "the repository's checkout {} has changes that are not committed:\n{changes}",
            repo.display()
        )));
    }
    configuration(project, place).await?;
    let merging = || async {
        said(repo, &["rev-parse", "--quiet", "--verify", "MERGE_HEAD"])
            .await
            .map(|s| s.ok)
    };
    if merging().await? {
        return Err(NotMerged::refused(format!(
            "a merge is under way in the repository's checkout {}",
            repo.display()
        )));
    }
    let head = git(repo, &["rev-parse", &place.reference()]).await?;
    if let Some(checked) = checked
        && head != checked
    {
        return Err(NotMerged::refused(format!(
            "the branch {} is at {head}, not at {checked}, the commit the check passed: the work is checked again \
             before it merges",
            place.reference()
        )));
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
            None => Ok(git(repo, &["rev-parse", "HEAD"]).await?),
        };
    }
    if checked.is_some() && !said(repo, &["merge-base", "--is-ancestor", "HEAD", &head]).await?.ok {
        let main = git(repo, &["rev-parse", "HEAD"]).await?;
        return Err(NotMerged::Moved { main, checked: head });
    }
    let words = fill(&project.commits.merge, &[("branch", &place.branch), ("name", name)]);
    let message = whose.ending(&project.commits, &words);
    let args = ["merge", "--no-ff", "-m", &message, &head];
    if let Err(why) = git(repo, &args).await {
        if merging().await? {
            let _ = git(repo, &["merge", "--abort"]).await;
        }
        return Err(why.into());
    }
    Ok(git(repo, &["rev-parse", "HEAD"]).await?)
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
