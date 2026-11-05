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

/// Where the subcommand is in git's arguments: the first that is not one of git's own options, which come before it.
fn subcommand_at(args: &[&str]) -> Option<usize> {
    // Git's own options that take the next argument as their value.
    const VALUED: &[&str] = &["-c", "-C", "--git-dir", "--work-tree", "--namespace", "--config-env"];
    let mut at = 0;
    while let Some(a) = args.get(at) {
        if VALUED.contains(a) {
            at += 2;
        } else if !a.starts_with('-') {
            return Some(at);
        } else {
            at += 1;
        }
    }
    None
}

/// The subcommand of git's arguments, found as [`subcommand_at`] says.
fn subcommand<'a>(args: &[&'a str]) -> Option<&'a str> {
    subcommand_at(args).map(|at| args[at])
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
/// A setting is pinned only to a value that makes git run nothing, whatever the engine's user has set elsewhere:
/// settings the owner keeps in git's global or system files are no safer than the repository's, since an agent's
/// `git config --global` writes them too. The engine's git therefore uses no hook, no credential helper, no `ssh`
/// other than the one on its `PATH`, no signing and no file-system monitor.
///
/// - `core.hooksPath` is the directory git looks in for hooks, which it runs on `push`, `merge`, `fetch`, a checkout, a
///   commit and every update of a ref. `/dev/null` is no directory, so git finds no hook in it, and none in the
///   repository's own `hooks` directory either, which the setting replaces. The engine runs no hook of a project: a
///   project's hooks are for the people who work in its checkout.
/// - `core.fsmonitor` names a program git asks which files changed, on `status`, a merge and a checkout.
/// - `core.sshCommand` is the program git starts for a remote reached over ssh, on `fetch` and `push`.
/// - `credential.helper` is a list, which an empty value clears: git then asks no helper, the repository's, a URL's
///   (`credential.<url>.helper`) and the system's alike.
/// - `commit.gpgSign` makes git start `gpg.program` for every commit it makes, the commit of a merge included;
///   `push.gpgSign` for every push, when the other end takes a signed push; `log.showSignature` for each signed commit
///   a `log` lists; `merge.verifySignatures` for the commit a merge takes. All four are off. The four `gpg.*.program`
///   settings name the program for each signature format; they are pinned to `false`, which only fails, so that a
///   place that would ask for a signature in a way not listed here finds no program to run.
/// - `protocol.ext.allow` lets a remote URL of the form `ext::<command>` run the command as its transport; git allows
///   it by default for no one, and the configuration can say otherwise, so `never` is set.
/// - `uploadpack.packObjectsHook` is a shell command that `git upload-pack` runs in place of `git pack-objects`, which
///   a fetch from a remote that is a path on this machine starts here; git reads it from the system's, the global and
///   the command-line configuration only, and the command line is the last of them. `env` runs the command it is given,
///   so the pack is made as it always was.
/// - `core.attributesFile` is a file of gitattributes outside the repository; see [`ATTRIBUTE_SOURCE`].
const PINNED: &[(&str, &str)] = &[
    ("core.hooksPath", "/dev/null"),
    ("core.fsmonitor", "false"),
    ("core.sshCommand", "ssh"),
    ("credential.helper", ""),
    ("commit.gpgSign", "false"),
    ("push.gpgSign", "false"),
    ("log.showSignature", "false"),
    ("merge.verifySignatures", "false"),
    ("gpg.program", "false"),
    ("gpg.openpgp.program", "false"),
    ("gpg.x509.program", "false"),
    ("gpg.ssh.program", "false"),
    ("protocol.ext.allow", "never"),
    ("uploadpack.packObjectsHook", "env"),
    ("core.attributesFile", "/dev/null"),
];

/// The environment every git command of the engine is run with, for what `-c` does not pin. Git takes the first
/// matching value of `core.gitProxy` (the program it starts to reach a `git://` remote), so a value on the command line
/// does not replace the configuration's, while a variable outranks the configuration; and `core.askPass` is read only
/// when `GIT_ASKPASS` is unset. An empty variable means no program.
///
/// - `GIT_ATTR_NOSYSTEM` stops git reading the system's gitattributes file; see [`ATTRIBUTE_SOURCE`].
/// - `GIT_PROXY_COMMAND` empty: no proxy program for `core.gitProxy`.
/// - `GIT_ASKPASS` empty: git starts no program to ask for a password, neither `core.askPass` nor `SSH_ASKPASS`; and
///   `GIT_TERMINAL_PROMPT=0` has it not ask on the terminal either, since the engine has no one to answer.
const PINNED_ENV: &[(&str, &str)] = &[
    ("GIT_ATTR_NOSYSTEM", "1"),
    ("GIT_PROXY_COMMAND", ""),
    ("GIT_ASKPASS", ""),
    ("GIT_TERMINAL_PROMPT", "0"),
];

/// The tree git reads gitattributes from, in place of the worktree's `.gitattributes` files: the empty tree of a
/// SHA-1 repository (git 2.40 or later; with a SHA-256 repository git fails and says so, it does not run a program).
/// A `filter.<name>` (clean, smudge, process) or `merge.<name>.driver` or `diff.<name>.textconv` program runs only
/// where an attribute names it, and a name is any word the repository's configuration and a `.gitattributes` file
/// agree on, so the names cannot be listed to pin them; and listing the names that are in the configuration before a
/// command runs leaves a gap in which an agent writes one more. The engine's git reads no attributes instead: none of
/// `.gitattributes`, the global file (`core.attributesFile`), the system file (`GIT_ATTR_NOSYSTEM`). What it costs is
/// that the project's own attributes (an end-of-line conversion, `merge=union`, a large-file filter) do not apply to
/// what the engine's git checks out and merges.
///
/// What this does not cover: `$GIT_DIR/info/attributes`, which git reads whatever the source, and which only a write
/// into the shared git directory makes; that, as everything an agent can write outside its worktree, is the boundary of
/// the agent's own commands.
const ATTRIBUTE_SOURCE: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";

/// A git command in `dir` that is run as the engine runs git: with the settings of [`PINNED`], the environment of
/// [`PINNED_ENV`] and no attributes ([`ATTRIBUTE_SOURCE`]). Every git command the engine runs in a project's checkout
/// or a work's worktree is built by this function, whether it comes through [`git`] or is run for its output as bytes,
/// so that a command added later is pinned as well. A command that reaches a remote through a program the
/// repository's configuration names (`remote.<name>.uploadpack` and `receivepack`, which `-c` cannot replace) gets that
/// program from [`git`], on its command line.
pub fn command(dir: &Path) -> Command {
    let mut command = Command::new("git");
    command.arg("-C").arg(dir);
    command.arg(format!("--attr-source={ATTRIBUTE_SOURCE}"));
    for (key, value) in PINNED {
        command.arg("-c").arg(format!("{key}={value}"));
    }
    for (name, value) in PINNED_ENV {
        command.env(name, value);
    }
    command
}

/// The option that names the program a transport subcommand starts on the other side of a local or ssh remote, as git
/// itself calls the default. The repository's `remote.<name>.uploadpack` and `receivepack` are read by git as "the first
/// one wins", so a `-c` of ours is ignored behind the repository's; an option of the command line outranks both.
fn transport_program(subcommand: &str) -> Option<&'static str> {
    match subcommand {
        "fetch" | "pull" => Some("--upload-pack=git-upload-pack"),
        "push" => Some("--receive-pack=git-receive-pack"),
        _ => None,
    }
}

/// Runs git in `dir`, with no lock.
fn run(dir: &Path, args: &[&str]) -> Result<String, String> {
    let mut command = command(dir);
    if let Some(at) = subcommand_at(args) {
        // The transport's program goes right after the subcommand, before any argument of the caller's.
        command
            .args(&args[..=at])
            .args(transport_program(args[at]))
            .args(&args[at + 1..]);
    } else {
        command.args(args);
    }
    let out = command.output().map_err(|e| format!("git: {e}"))?;
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

    // The program of a transport goes after the subcommand, found past git's own options, and only for the subcommands
    // that reach a remote through one; the others get nothing, so that no option of theirs is misread.
    #[test]
    fn a_transport_subcommand_is_told_where_its_program_goes() {
        let who = ["-c", "user.name=wf-agents", "-c", "user.email=wf-agents@localhost"];
        assert_eq!(subcommand_at(&[&who[..], &["push", "-q", "origin"]].concat()), Some(4));
        assert_eq!(subcommand_at(&["fetch", "-q"]), Some(0));
        assert_eq!(subcommand_at(&["-c"]), None);
        assert_eq!(transport_program("fetch"), Some("--upload-pack=git-upload-pack"));
        assert_eq!(transport_program("pull"), Some("--upload-pack=git-upload-pack"));
        assert_eq!(transport_program("push"), Some("--receive-pack=git-receive-pack"));
        assert_eq!(transport_program("status"), None);
        assert_eq!(transport_program("merge"), None);
    }

    // Every git command of the engine is built by [`command`], which pins what the repository's configuration cannot be
    // allowed to name: a place that starts git by itself is not pinned, and the pins, tested one by one in the engine's
    // `git_programs.rs`, do not hold for it. The code before a module of tests is read, without `shared_git.rs`, whose
    // own start of git this is, and without `build.rs`, which runs before the engine and touches no project.
    #[test]
    fn no_code_starts_git_beside_the_pinned_command() {
        fn sources(dir: &Path, found: &mut Vec<PathBuf>) {
            for entry in std::fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                let name = path.file_name().unwrap().to_string_lossy().into_owned();
                if path.is_dir() {
                    if name != "tests" {
                        sources(&path, found);
                    }
                } else if name.ends_with(".rs") && name != "tests.rs" {
                    found.push(path);
                }
            }
        }
        let crates = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let mut files = Vec::new();
        for krate in std::fs::read_dir(crates).unwrap() {
            let src = krate.unwrap().path().join("src");
            if src.is_dir() {
                sources(&src, &mut files);
            }
        }
        assert!(
            files.len() > 20,
            "the sources were not found under {}",
            crates.display()
        );
        let spawn = ["Command::new(", "\"git\"", ")"].concat();
        let beside: Vec<String> = files
            .iter()
            .filter(|f| !f.ends_with("config/src/shared_git.rs"))
            .filter(|f| {
                let text = std::fs::read_to_string(f).unwrap();
                text.split("#[cfg(test)]\nmod ").next().unwrap().contains(&spawn)
            })
            .map(|f| f.display().to_string())
            .collect();
        assert!(
            beside.is_empty(),
            "these start git without `config::shared_git::command`: {beside:?}"
        );
    }
}
