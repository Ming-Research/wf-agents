//! The engine's git commands run no hook. A work's agent can write the git configuration its worktree shares with the
//! project's checkout (`core.hooksPath`), or a file in the checkout's `hooks` directory; git would then run that program
//! as the engine's user, with the engine's credentials, on the engine's own `push` and `merge`. The reference is git
//! itself: the same repositories, set up the same way, run by plain git commands, do run the hooks.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use engine::git::{catch_up, create_worktree, head};
use engine::github::{delete_ref, push};

/// The hooks written: each one records its own name in the markers directory and succeeds, as a program that does what
/// it likes and leaves the exit code alone would. Together they cover what the engine's commands start: a push, an
/// update of a ref, a checkout, a merge commit and a merge.
const HOOKS: &[&str] = &[
    "pre-push",
    "reference-transaction",
    "post-checkout",
    "pre-merge-commit",
    "prepare-commit-msg",
    "commit-msg",
    "post-merge",
];

/// Git as the test sets repositories up: with its hooks off, so that what ran after the hooks were written is only the
/// commands under test.
fn setup(dir: &Path, args: &[&str]) -> String {
    run(dir, &["-c", "core.hooksPath=/dev/null"], args)
}

/// Git as a person runs it, with whatever hooks the repository has.
fn plain(dir: &Path, args: &[&str]) -> String {
    run(dir, &[], args)
}

fn run(dir: &Path, options: &[&str], args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=t", "-c", "user.email=t@t"])
        .args(["-c", "init.defaultBranch=main", "-c", "commit.gpgsign=false"])
        .args(options)
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

/// How the hooks reach git: named by the repository's configuration, which every worktree of the checkout shares, or
/// written in the checkout's own `hooks` directory, which they share too.
#[derive(Clone, Copy)]
enum Route {
    HooksPath,
    HooksDirectory,
}

/// A bare origin, a clone that moves it on (the other people working on the project) and the project's checkout.
struct Fixture {
    root: PathBuf,
    markers: PathBuf,
    seed: PathBuf,
    checkout: PathBuf,
}

impl Fixture {
    fn new(name: &str, route: Route) -> Fixture {
        let root = std::env::temp_dir().join(format!("wf-agents-git-hooks-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let markers = root.join("markers");
        std::fs::create_dir_all(&markers).unwrap();
        let (origin, seed, checkout) = (root.join("origin.git"), root.join("seed"), root.join("checkout"));
        setup(&root, &["init", "-q", "--bare", &origin.to_string_lossy()]);
        setup(
            &root,
            &["clone", "-q", &origin.to_string_lossy(), &seed.to_string_lossy()],
        );
        let fixture = Fixture {
            root,
            markers,
            seed,
            checkout,
        };
        fixture.advance("first");
        setup(
            &fixture.root,
            &[
                "clone",
                "-q",
                &origin.to_string_lossy(),
                &fixture.checkout.to_string_lossy(),
            ],
        );
        let hooks = match route {
            Route::HooksPath => fixture.root.join("programs"),
            Route::HooksDirectory => fixture.checkout.join(".git").join("hooks"),
        };
        std::fs::create_dir_all(&hooks).unwrap();
        for hook in HOOKS {
            let file = hooks.join(hook);
            let marker = fixture.markers.join(hook);
            std::fs::write(&file, format!("#!/bin/sh\ntouch \"{}\"\n", marker.display())).unwrap();
            std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        if let Route::HooksPath = route {
            setup(
                &fixture.checkout,
                &["config", "core.hooksPath", &hooks.to_string_lossy()],
            );
        }
        fixture
    }

    /// The origin's main moves on by a commit of its own file.
    fn advance(&self, file: &str) {
        std::fs::write(self.seed.join(file), file).unwrap();
        setup(&self.seed, &["add", "-A"]);
        setup(&self.seed, &["commit", "-q", "-m", file]);
        setup(&self.seed, &["push", "-q", "origin", "HEAD:main"]);
    }

    /// Which hooks ran since the last call, by name; the markers are cleared.
    fn hooks_run(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(&self.markers)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        for name in &names {
            std::fs::remove_file(self.markers.join(name)).unwrap();
        }
        names
    }
}

/// A work's worktree with a commit of its own, made with hooks off.
fn commit_in(dir: &Path, file: &str) {
    std::fs::write(dir.join(file), file).unwrap();
    setup(dir, &["add", "-A"]);
    setup(dir, &["commit", "-q", "-m", file]);
}

fn engine_never_runs_a_hook(name: &str, route: Route) {
    let fx = Fixture::new(name, route);

    // The reference: plain git on the same kind of worktree, with the same steps the engine takes below, runs the hooks.
    // Without this, a hook that never ran for any reason would pass the test too.
    let control = fx.root.join("worktree-control");
    plain(
        &fx.checkout,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "control",
            &control.to_string_lossy(),
            "origin/main",
        ],
    );
    commit_in(&control, "control-own");
    fx.advance("second");
    plain(
        &control,
        &["fetch", "-q", "origin", "+refs/heads/main:refs/remotes/origin/main"],
    );
    plain(&control, &["merge", "--no-edit", "-q", "origin/main"]);
    plain(&control, &["push", "-q", "origin", "HEAD:refs/heads/control"]);
    plain(&control, &["push", "-q", "origin", ":refs/heads/control"]);
    let ran = fx.hooks_run();
    for hook in ["pre-push", "reference-transaction", "post-checkout", "post-merge"] {
        assert!(
            ran.iter().any(|r| r == hook),
            "plain git did not run {hook}: the fixture does not make git run hooks (ran {ran:?})"
        );
    }

    // The engine's own commands on a work's worktree: set up, caught up with a main that moved on, pushed, deleted.
    let work = fx.root.join("worktree-work");
    create_worktree(&fx.checkout, &work, "work", "main").unwrap();
    commit_in(&work, "work-own");
    fx.advance("third");
    let caught_up = catch_up(&work, "main").unwrap();
    assert!(caught_up.is_ok(), "the merge of main did not go through: {caught_up:?}");
    let sha = head(&work).unwrap();
    push(&work, &sha, "refs/heads/work", false).unwrap();
    delete_ref(&work, "refs/heads/work").unwrap();
    assert_eq!(fx.hooks_run(), Vec::<String>::new(), "the engine's git ran a hook");

    let _ = std::fs::remove_dir_all(&fx.root);
}

// A worktree whose configuration names a directory of hooks: the engine's worktree setup, merge of main, push and
// deletion of a branch run none of them. It catches a command of the engine's started without pinning
// `core.hooksPath`, whichever command it is.
#[test]
fn a_hooks_path_in_the_configuration_runs_no_hook_in_the_engines_git() {
    engine_never_runs_a_hook("path", Route::HooksPath);
}

// The same with the hooks in the checkout's `hooks` directory, which every worktree uses when nothing names another:
// setting `core.hooksPath` replaces that directory, so that the hooks there stay unrun too.
#[test]
fn hooks_in_the_checkouts_hooks_directory_run_no_hook_in_the_engines_git() {
    engine_never_runs_a_hook("directory", Route::HooksDirectory);
}
