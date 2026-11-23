//! The build script of `wf-agents`, which builds the repository's `flow/` into the program and says which commit the
//! program was built from. Each test builds a small package of the workspace's layout (`crates/web/build_support`,
//! `crates/wf-agents` with the real `build.rs`, `flow/` beside them) with cargo, with a build directory of its own, and
//! asks the program it made what it carries. The reference of each verdict is what cargo's documentation says of the
//! values a script is given and the paths it watches, and what the script's own comment says it does: the build gives
//! the commit named in `WF_BUILD_COMMIT`, `unknown` where none, and the files of its own checkout's `flow/`.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// The build script under test, and the files it includes.
const BUILD: &str = include_str!("../build.rs");
const COMMIT_RS: &str = include_str!("../build_support/commit.rs");
const TREE_RS: &str = include_str!("../../web/build_support/tree.rs");
/// The configuration that gives each checkout's build the path of the checkout, which the script watches.
const CARGO_CONFIG: &str = include_str!("../../../.cargo/config.toml");

/// One checkout of the layout, written whole. `flows` are the files of its `flow/`, each by its path and text.
fn checkout(base: &Path, name: &str, flows: &[(&str, &str)]) -> PathBuf {
    let root = base.join(name);
    let package = root.join("crates/wf-agents");
    for dir in [
        package.join("src"),
        package.join("build_support"),
        root.join("crates/web/build_support"),
        root.join(".cargo"),
        root.join("flow"),
    ] {
        std::fs::create_dir_all(dir).unwrap();
    }
    let put = |path: PathBuf, text: &str| std::fs::write(path, text).unwrap();
    put(root.join(".cargo/config.toml"), CARGO_CONFIG);
    put(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/wf-agents\"]\nresolver = \"3\"\n",
    );
    put(
        package.join("Cargo.toml"),
        "[package]\nname = \"tiny\"\nversion = \"0.1.0\"\nedition = \"2024\"\npublish = false\n",
    );
    put(package.join("build.rs"), BUILD);
    put(package.join("build_support/commit.rs"), COMMIT_RS);
    put(root.join("crates/web/build_support/tree.rs"), TREE_RS);
    put(
        package.join("src/main.rs"),
        "include!(concat!(env!(\"OUT_DIR\"), \"/flows.rs\"));\n\
         fn main() {\n    \
         let wanted = std::env::args().nth(1).unwrap_or_default();\n    \
         match wanted.as_str() {\n        \
         \"commit\" => print!(\"{}\", env!(\"WF_COMMIT\")),\n        \
         \"names\" => print!(\"{}\", FLOWS.iter().map(|(name, _)| *name).collect::<Vec<_>>().join(\",\")),\n        \
         file => {\n            for (name, bytes) in FLOWS {\n                \
         if *name == file {\n                    print!(\"{}\", String::from_utf8_lossy(bytes));\n                }\n            }\n        }\n    }\n}\n",
    );
    for (path, text) in flows {
        let place = root.join("flow").join(path);
        std::fs::create_dir_all(place.parent().unwrap()).unwrap();
        std::fs::write(place, text).unwrap();
    }
    root
}

/// A build of the checkout in the build directory `target`, with `commit` as `WF_BUILD_COMMIT` or without it.
fn cargo_build(checkout: &Path, target: &Path, commit: Option<&str>) -> Output {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut command = Command::new(cargo);
    command
        .args(["build", "-v", "--color", "never"])
        .current_dir(checkout)
        .env("CARGO_TARGET_DIR", target)
        // The repository's configuration gives the checkout's path to its builds as `WF_CHECKOUT`, and the program that
        // runs the tests is given it as the repository's own, which cargo does not replace in a build it runs from
        // there. Taken away, each checkout's build gets its own value from its own configuration.
        .env_remove("WF_CHECKOUT");
    match commit {
        Some(commit) => command.env("WF_BUILD_COMMIT", commit),
        None => command.env_remove("WF_BUILD_COMMIT"),
    };
    command.output().expect("cargo runs")
}

/// Builds, and gives what cargo said.
fn built(checkout: &Path, target: &Path, commit: Option<&str>) -> String {
    let out = cargo_build(checkout, target, commit);
    let said = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(
        out.status.success(),
        "the build of {} failed: {said}",
        checkout.display()
    );
    said
}

/// What the program built last says to this question.
fn asked(target: &Path, question: &str) -> String {
    let out = Command::new(target.join("debug/tiny"))
        .arg(question)
        .output()
        .expect("the program runs");
    String::from_utf8(out.stdout).unwrap()
}

#[test]
fn the_commit_a_build_is_given_is_the_commit_the_program_says_and_a_build_without_one_says_unknown() {
    let base = tempfile::tempdir().unwrap();
    let target = base.path().join("target");
    let a = checkout(base.path(), "a", &[("alpha/flow.toml", "alpha")]);
    built(&a, &target, None);
    assert_eq!(asked(&target, "commit"), "unknown");
    // The commit given is said, though nothing in the checkout changed: the value is what the script watches.
    built(&a, &target, Some("0123abc"));
    assert_eq!(asked(&target, "commit"), "0123abc");
    // The first line only, trimmed.
    built(&a, &target, Some("  fedcba9-dirty  \nanything else"));
    assert_eq!(asked(&target, "commit"), "fedcba9-dirty");
    // And a build given none after one that was given says unknown, not the earlier one.
    built(&a, &target, None);
    assert_eq!(asked(&target, "commit"), "unknown");
    // The same value again compiles nothing.
    built(&a, &target, Some("0123abc"));
    let again = built(&a, &target, Some("0123abc"));
    assert!(
        !again.contains("Compiling"),
        "a build with the same commit compiled: {again}"
    );
}

#[test]
fn two_checkouts_that_share_a_build_directory_each_carry_their_own_flows() {
    let base = tempfile::tempdir().unwrap();
    let target = base.path().join("target");
    let a = checkout(
        base.path(),
        "a",
        &[("alpha/flow.toml", "of a"), ("shared/rules.toml", "shared by a")],
    );
    let b = checkout(
        base.path(),
        "b",
        &[("beta/flow.toml", "of b"), ("shared/rules.toml", "shared by b")],
    );
    built(&a, &target, None);
    assert_eq!(asked(&target, "names"), "alpha/flow.toml,shared/rules.toml");
    assert_eq!(asked(&target, "shared/rules.toml"), "shared by a");
    built(&b, &target, None);
    assert_eq!(asked(&target, "names"), "beta/flow.toml,shared/rules.toml");
    assert_eq!(asked(&target, "shared/rules.toml"), "shared by b");
    built(&a, &target, None);
    assert_eq!(asked(&target, "names"), "alpha/flow.toml,shared/rules.toml");
    assert_eq!(asked(&target, "shared/rules.toml"), "shared by a");
}

#[test]
fn flows_changed_after_a_build_are_the_flows_the_next_build_carries_and_a_build_with_none_changed_compiles_nothing() {
    let base = tempfile::tempdir().unwrap();
    let target = base.path().join("target");
    let a = checkout(
        base.path(),
        "a",
        &[
            ("alpha/flow.toml", "one"),
            ("alpha/cells/x.toml", "x"),
            ("shared/rules.toml", "p"),
        ],
    );
    // What a tool of the machine leaves in the directory is not a flow's file.
    std::fs::write(a.join("flow/.DS_Store"), "kept by the Finder").unwrap();
    built(&a, &target, None);
    assert_eq!(
        asked(&target, "names"),
        "alpha/cells/x.toml,alpha/flow.toml,shared/rules.toml"
    );
    let again = built(&a, &target, None);
    assert!(
        !again.contains("Compiling"),
        "a build with nothing changed compiled: {again}"
    );
    // A file edited, one added in a directory that is there, one taken away, and one in a directory that is new.
    std::fs::write(a.join("flow/alpha/flow.toml"), "two").unwrap();
    std::fs::write(a.join("flow/alpha/cells/y.toml"), "y").unwrap();
    std::fs::remove_file(a.join("flow/alpha/cells/x.toml")).unwrap();
    std::fs::create_dir_all(a.join("flow/beta")).unwrap();
    std::fs::write(a.join("flow/beta/flow.toml"), "beta").unwrap();
    built(&a, &target, None);
    assert_eq!(
        asked(&target, "names"),
        "alpha/cells/y.toml,alpha/flow.toml,beta/flow.toml,shared/rules.toml"
    );
    assert_eq!(asked(&target, "alpha/flow.toml"), "two");
}
