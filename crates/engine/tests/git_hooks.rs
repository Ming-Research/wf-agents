//! The git the engine runs in a work's worktree runs no hook. A work's agent can make git run a program of its choice
//! on the engine's push, merge or commit, with the engine's credentials, by writing `core.hooksPath` into the
//! repository's configuration (which every worktree of a project shares) or a file into `.git/hooks`; git's exit code
//! is 0 either way. The reference is git's own manual (`githooks`, `git-config`): each hook below is one git runs for
//! the command that follows it.

#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use engine::git::git;
use engine::hub::{delete_ref, push};

/// Every hook that a command in these tests makes git look for.
const HOOKS: &[&str] = &[
    "pre-push",
    "reference-transaction",
    "pre-commit",
    "prepare-commit-msg",
    "commit-msg",
    "post-commit",
    "post-checkout",
    "pre-merge-commit",
    "post-merge",
    "post-rewrite",
];

/// How the agent makes git find a program.
#[derive(Clone, Copy, Debug)]
enum Plant {
    /// `git config core.hooksPath <a directory of programs>` in the worktree.
    HooksPath,
    /// A file in the repository's own `hooks` directory.
    HooksDir,
}

const PLANTS: [Plant; 2] = [Plant::HooksPath, Plant::HooksDir];

struct Repos {
    root: PathBuf,
    origin: PathBuf,
    work: PathBuf,
    /// A file every planted program appends its own name to.
    marks: PathBuf,
}

impl Repos {
    /// Programs that were run since the planting, one name for each run.
    fn ran(&self) -> Vec<String> {
        std::fs::read_to_string(&self.marks)
            .unwrap_or_default()
            .lines()
            .map(str::to_string)
            .collect()
    }
}

/// Git as the test sets things up, with the settings of the user's git that would change a result fixed.
fn raw(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=t", "-c", "user.email=t@t"])
        .args(["-c", "init.defaultBranch=main", "-c", "commit.gpgsign=false"])
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// A bare origin and a clone of it with a first commit pushed, then the programs planted as `plant` says.
fn repos(name: &str, plant: Plant) -> Repos {
    let root = std::env::temp_dir().join(format!("wf-agents-hooks-{name}-{plant:?}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let origin = root.join("origin.git");
    let work = root.join("work");
    let marks = root.join("marks");
    std::fs::create_dir_all(&root).unwrap();
    raw(&root, &["init", "-q", "--bare", "origin.git"]);
    raw(&root, &["init", "-q", "work"]);
    raw(&work, &["remote", "add", "origin", origin.to_str().unwrap()]);
    raw(&work, &["commit", "-q", "--allow-empty", "-m", "first"]);
    raw(&work, &["push", "-q", "origin", "HEAD:refs/heads/main"]);

    let programs = match plant {
        Plant::HooksPath => root.join("programs"),
        Plant::HooksDir => work.join(".git").join("hooks"),
    };
    std::fs::create_dir_all(&programs).unwrap();
    for hook in HOOKS {
        let file = programs.join(hook);
        std::fs::write(&file, format!("#!/bin/sh\necho {hook} >> '{}'\n", marks.display())).unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    if let Plant::HooksPath = plant {
        raw(&work, &["config", "core.hooksPath", programs.to_str().unwrap()]);
    }
    Repos {
        root,
        origin,
        work,
        marks,
    }
}

// The planting works: git as it is run without the engine's care runs the program on a push. Without this the tests
// below would pass on any repository where the planting does nothing.
#[test]
fn a_planted_program_runs_on_a_plain_git_push() {
    for plant in PLANTS {
        let r = repos("control", plant);
        let sha = raw(&r.work, &["rev-parse", "HEAD"]);
        raw(&r.work, &["push", "-q", "origin", &format!("{sha}:refs/heads/plain")]);
        assert!(r.ran().contains(&"pre-push".to_string()), "{plant:?}: {:?}", r.ran());
        let _ = std::fs::remove_dir_all(&r.root);
    }
}

// The engine's push and its deletion of a branch push with the engine's credentials, and succeed or fail by the exit
// code of a program the agent chose when git runs it.
#[test]
fn the_engines_push_and_branch_deletion_run_no_program() {
    for plant in PLANTS {
        let r = repos("push", plant);
        let sha = raw(&r.work, &["rev-parse", "HEAD"]);
        assert!(push(&r.work, &sha, "refs/heads/work", false).is_ok(), "{plant:?}: push");
        assert!(delete_ref(&r.work, "refs/heads/work").is_ok(), "{plant:?}: delete");
        assert_eq!(
            r.ran(),
            Vec::<String>::new(),
            "{plant:?}: programs run by the engine's push"
        );
        let _ = std::fs::remove_dir_all(&r.root);
    }
}

// The commands the engine runs that write commits or move HEAD, or update a ref: each has hooks of its own in git's
// manual, and none runs.
#[test]
fn the_engines_commit_merge_revert_checkout_fetch_and_worktree_run_no_program() {
    let who = ["-c", "user.name=t", "-c", "user.email=t@t"];
    for plant in PLANTS {
        let r = repos("commands", plant);
        let run = |args: &[&str]| git(&r.work, &[&who[..], args].concat()).unwrap_or_else(|e| panic!("{plant:?}: {e}"));
        run(&["checkout", "-q", "-b", "side"]);
        run(&["commit", "-q", "--allow-empty", "-m", "side"]);
        run(&["checkout", "-q", "main"]);
        run(&["commit", "-q", "--allow-empty", "-m", "main"]);
        run(&["merge", "-q", "--no-ff", "-m", "merge", "side"]);
        run(&["revert", "--no-edit", "HEAD"]);

        // A commit of someone else on the origin, which the fetch brings in.
        let other = r.root.join("other");
        raw(&r.root, &["clone", "-q", r.origin.to_str().unwrap(), "other"]);
        raw(&other, &["commit", "-q", "--allow-empty", "-m", "theirs"]);
        raw(&other, &["push", "-q", "origin", "HEAD:refs/heads/main"]);
        run(&["fetch", "-q", "origin", "+refs/heads/main:refs/remotes/origin/main"]);

        let tree = r.root.join("tree");
        run(&["worktree", "add", "-q", "--detach", tree.to_str().unwrap()]);
        assert_eq!(
            r.ran(),
            Vec::<String>::new(),
            "{plant:?}: programs run by the engine's git"
        );
        let _ = std::fs::remove_dir_all(&r.root);
    }
}
