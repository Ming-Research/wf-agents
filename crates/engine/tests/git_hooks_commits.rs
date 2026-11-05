//! The engine's git commands that make commits run no hook either: the revert of a commit that broke main
//! (`actions::revert_in`) and any other command run through `engine::git::git`. `git_hooks.rs` covers the worktree
//! setup, merge, push and branch deletion; the commit hooks (`pre-commit`, `post-commit`, `commit-msg`) are not started
//! by those. The reference is git itself: plain git on the same repository runs the hooks.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

const HOOKS: &[&str] = &[
    "pre-commit",
    "prepare-commit-msg",
    "commit-msg",
    "post-commit",
    "post-rewrite",
    "reference-transaction",
];

fn plain(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn markers_run(markers: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(markers)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    for name in &names {
        std::fs::remove_file(markers.join(name)).unwrap();
    }
    names
}

// A repository whose configuration names a directory of commit hooks: a revert and a commit made by the engine's
// `git` run none of them, while the same commands run by plain git do.
#[test]
fn the_engines_commits_and_reverts_run_no_hook() {
    let root: PathBuf = std::env::temp_dir().join(format!("wf-agents-git-hooks-commits-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (repo, programs, markers) = (root.join("repo"), root.join("programs"), root.join("markers"));
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::create_dir_all(&programs).unwrap();
    std::fs::create_dir_all(&markers).unwrap();
    plain(&repo, &["-c", "init.defaultBranch=main", "init", "-q"]);
    for (file, text) in [("a", "a"), ("b", "b"), ("c", "c")] {
        std::fs::write(repo.join(file), text).unwrap();
        plain(&repo, &["add", "-A"]);
        plain(&repo, &["commit", "-q", "-m", file]);
    }
    for hook in HOOKS {
        let file = programs.join(hook);
        std::fs::write(
            &file,
            format!("#!/bin/sh\ntouch \"{}\"\n", markers.join(hook).display()),
        )
        .unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    plain(
        &repo,
        &[
            "-c",
            "core.hooksPath=/dev/null",
            "config",
            "core.hooksPath",
            &programs.to_string_lossy(),
        ],
    );

    // The reference: plain git runs the commit hooks on a commit and on a revert.
    std::fs::write(repo.join("d"), "d").unwrap();
    plain(&repo, &["add", "-A"]);
    plain(&repo, &["commit", "-q", "-m", "d"]);
    let ran = markers_run(&markers);
    for hook in ["pre-commit", "commit-msg", "post-commit"] {
        assert!(
            ran.iter().any(|r| r == hook),
            "plain git did not run {hook} (ran {ran:?})"
        );
    }
    plain(&repo, &["revert", "--no-edit", "HEAD"]);
    assert!(
        !markers_run(&markers).is_empty(),
        "plain git ran no hook on a revert: the fixture does not make git run hooks"
    );

    // The engine's git: the same two commands, no hook.
    let who = ["-c", "user.name=wf-agents", "-c", "user.email=wf-agents@localhost"];
    std::fs::write(repo.join("e"), "e").unwrap();
    engine::git::git(&repo, &[&who[..], &["add", "-A"]].concat()).unwrap();
    engine::git::git(&repo, &[&who[..], &["commit", "-q", "-m", "e"]].concat()).unwrap();
    engine::git::git(&repo, &[&who[..], &["revert", "--no-edit", "HEAD"]].concat()).unwrap();
    assert_eq!(
        markers_run(&markers),
        Vec::<String>::new(),
        "the engine's git ran a hook"
    );

    let _ = std::fs::remove_dir_all(&root);
}
