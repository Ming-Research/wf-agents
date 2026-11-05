//! Git commands that change what every worktree of a repository shares, run so that they take turns within this
//! process: the session manager's worktree and branch commands, and the fetch of a project's main that the readers of
//! the project's text start from, all of which may run at the same time on one checkout.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};

/// The git subcommands that change what every worktree of a repository shares: `fetch` and `pull` move the
/// remote-tracking branches; `worktree` writes the repository's list of worktrees and, adding one on a new branch, its
/// configuration; `branch`, `config` and `remote` write its configuration, as deleting a branch drops the branch's
/// section. A git command the engine comes to run that changes what its worktrees share is named here.
const SHARED: &[&str] = &["branch", "config", "fetch", "pull", "remote", "worktree"];

/// Runs git in `dir` and gives its output, or what it said on failure. A command that changes what every worktree of
/// the repository shares ([`SHARED`]) first takes this process's lock of the repository, found by its common git
/// directory whichever worktree `dir` is, and holds it until the command ends, since git fails at once when another
/// command holds a lock file it needs. Where `dir` is in no repository, the command runs as it is, and fails as git
/// says.
pub fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let repository = subcommand(args)
        .filter(|s| SHARED.contains(s))
        .and_then(|_| repository(dir));
    let _held = repository.as_deref().map(lock);
    run(dir, args)
}

/// The subcommand of git's arguments: the first that is not one of git's own options, which come before it.
fn subcommand<'a>(args: &[&'a str]) -> Option<&'a str> {
    // Git's own options that take the next argument as their value.
    const VALUED: &[&str] = &["-c", "-C", "--git-dir", "--work-tree", "--namespace", "--config-env"];
    let mut rest = args.iter();
    while let Some(a) = rest.next() {
        if VALUED.contains(a) {
            rest.next();
        } else if !a.starts_with('-') {
            return Some(a);
        }
    }
    None
}

/// This process's lock of the repository `dir` is in, by the repository's common git directory, which all its
/// worktrees share; none when `dir` is in no repository.
fn repository(dir: &Path) -> Option<Arc<Mutex<()>>> {
    static LOCKS: OnceLock<Mutex<BTreeMap<PathBuf, Arc<Mutex<()>>>>> = OnceLock::new();
    let common = run(dir, &["rev-parse", "--path-format=absolute", "--git-common-dir"]).ok()?;
    let common = std::fs::canonicalize(common.trim()).ok()?;
    let mut locks = lock(LOCKS.get_or_init(Mutex::default));
    Some(Arc::clone(locks.entry(common).or_default()))
}

/// What a lock guards, also after a thread panicked while it held it: a repository's lock guards nothing but the
/// order of the commands, and the map of the locks is whole after every change.
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The settings every git command of the engine is run with, written as `-c` options. Command-line settings outrank
/// every file of git's configuration, the repository's own included, and every worktree of a project's checkout shares
/// that one repository configuration, which an agent working in a worktree can write. A setting that names a program
/// git runs is pinned here, so that what an agent writes in the configuration cannot make the engine's git run it with
/// the engine's user and credentials.
///
/// `core.hooksPath` is the directory git looks in for hooks, which it runs on `push`, `merge`, `fetch`, a checkout, a
/// commit and every update of a ref. `/dev/null` is no directory, so git finds no hook in it, and none in the
/// repository's own `hooks` directory either, which the setting replaces. The engine runs no hook of a project: a
/// project's hooks are for the people who work in its checkout.
const PINNED: &[(&str, &str)] = &[("core.hooksPath", "/dev/null")];

/// A git command in `dir` that is run as the engine runs git: with the settings of [`PINNED`]. Every git command the
/// engine runs in a project's checkout or a work's worktree is built by this function, whether it comes through
/// [`git`] or is run for its output as bytes, so that a command added later is pinned as well.
pub fn command(dir: &Path) -> Command {
    let mut command = Command::new("git");
    command.arg("-C").arg(dir);
    for (key, value) in PINNED {
        command.arg("-c").arg(format!("{key}={value}"));
    }
    command
}

/// Runs git in `dir`, with no lock.
fn run(dir: &Path, args: &[&str]) -> Result<String, String> {
    let out = command(dir).args(args).output().map_err(|e| format!("git: {e}"))?;
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

#[cfg(test)]
mod tests {
    use super::*;

    // The subcommand is found past git's own options, those that take a value included, whatever a caller puts first.
    #[test]
    fn a_subcommand_is_found_past_gits_own_options() {
        let who = ["-c", "user.name=wf-agents", "-c", "user.email=wf-agents@localhost"];
        assert_eq!(subcommand(&[&who[..], &["worktree", "add"]].concat()), Some("worktree"));
        assert_eq!(subcommand(&["-C", "/x", "--no-pager", "fetch", "-q"]), Some("fetch"));
        assert_eq!(subcommand(&["--git-dir=/x/.git", "branch", "-D", "b"]), Some("branch"));
        assert_eq!(subcommand(&["--work-tree", "/x", "status"]), Some("status"));
        assert_eq!(subcommand(&["-p"]), None);
    }

    // Every worktree of a repository finds the one lock of its common git directory, however its path is written; a
    // directory in no repository finds none.
    #[test]
    fn the_worktrees_of_a_repository_share_its_lock() {
        let root = std::env::temp_dir().join(format!("wf-agents-sessions-lock-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let repo = root.join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        let ok = |dir: &Path, args: &[&str]| {
            let out = Command::new("git")
                .arg("-C")
                .arg(dir)
                .args(["-c", "user.name=t", "-c", "user.email=t@t"])
                .args(args)
                .output()
                .unwrap();
            assert!(
                out.status.success(),
                "{args:?}: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        };
        ok(&repo, &["init", "-q"]);
        ok(&repo, &["commit", "-q", "--allow-empty", "-m", "one"]);
        let wt = root.join("wt");
        ok(&repo, &["worktree", "add", "-q", "--detach", &wt.to_string_lossy()]);
        let of = |dir: &Path| repository(dir).unwrap_or_else(|| panic!("no lock for {}", dir.display()));
        assert!(Arc::ptr_eq(&of(&repo), &of(&wt)));
        assert!(Arc::ptr_eq(&of(&repo), &of(&root.join("wt").join("..").join("repo"))));
        let elsewhere = root.join("elsewhere");
        std::fs::create_dir_all(&elsewhere).unwrap();
        assert!(repository(&elsewhere).is_none());
        let _ = std::fs::remove_dir_all(&root);
    }
}
