//! The page a program carries is the page of the checkout it is built in, whatever else shares its build directory:
//! two checkouts of one layout built into one target directory, as the owner's worktrees may be, each embed their own
//! page, and a build with nothing changed compiles nothing again. Each test builds a small package of the same layout
//! as the workspace (`crates/<name>`, `client/` beside it) with the real `build.rs` of this crate, with the build
//! directory of its own, and reads the page the program it made prints.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// The build script under test, and the files it includes.
const BUILD: &str = include_str!("../build.rs");
const STAMP_RS: &str = include_str!("../build_support/stamp.rs");
const TREE_RS: &str = include_str!("../build_support/tree.rs");
/// The configuration that gives each checkout's build the path of the checkout, which the script watches.
const CARGO_CONFIG: &str = include_str!("../../../.cargo/config.toml");

/// The SHA-256 of `abc`, the one source file of every checkout, so that the stamp is the same in each.
const ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

/// One checkout of the layout: a workspace with the package `tiny` that holds the build script and the repository's
/// cargo configuration, and `client/` with a source, the page built from it and the stamp. Written whole, so that every
/// file of every checkout is older than the first build.
fn checkout(base: &Path, name: &str, page: &str) -> PathBuf {
    let root = base.join(name);
    let package = root.join("crates/tiny");
    for dir in [
        package.join("src"),
        package.join("build_support"),
        root.join(".cargo"),
        root.join("client/src"),
        root.join("client/dist"),
    ] {
        std::fs::create_dir_all(dir).unwrap();
    }
    let put = |path: PathBuf, text: &str| std::fs::write(path, text).unwrap();
    put(root.join(".cargo/config.toml"), CARGO_CONFIG);
    put(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/*\"]\nresolver = \"3\"\n",
    );
    put(
        package.join("Cargo.toml"),
        "[package]\nname = \"tiny\"\nversion = \"0.1.0\"\nedition = \"2024\"\npublish = false\n",
    );
    put(package.join("build.rs"), BUILD);
    put(package.join("build_support/stamp.rs"), STAMP_RS);
    put(package.join("build_support/tree.rs"), TREE_RS);
    put(
        package.join("src/main.rs"),
        "include!(concat!(env!(\"OUT_DIR\"), \"/page.rs\"));\n\
         fn main() {\n    for (name, bytes) in FILES {\n        if *name == \"index.html\" {\n            \
         print!(\"{}\", String::from_utf8_lossy(bytes));\n        }\n    }\n}\n",
    );
    put(root.join("client/src/page.ts"), "abc");
    put(root.join("client/dist/index.html"), page);
    put(root.join("client/dist/source.sha256"), &format!("{ABC}  src/page.ts\n"));
    root
}

fn cargo_build(checkout: &Path, target: &Path) -> Output {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    Command::new(cargo)
        .args(["build", "-v", "--color", "never"])
        .current_dir(checkout)
        .env("CARGO_TARGET_DIR", target)
        // The repository's configuration gives the checkout's path to its builds as `WF_CHECKOUT`, and the program that
        // runs the tests is given it as the repository's own, which cargo does not replace in a build it runs from
        // there. Taken away, each checkout's build gets its own value from its own configuration.
        .env_remove("WF_CHECKOUT")
        .output()
        .expect("cargo runs")
}

/// Builds, and gives the page the program embeds with what cargo said, which tells whether it ran the script again.
fn built(checkout: &Path, target: &Path) -> (String, String) {
    let out = cargo_build(checkout, target);
    let said = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(
        out.status.success(),
        "the build of {} failed: {said}",
        checkout.display()
    );
    let ran = Command::new(target.join("debug/tiny"))
        .output()
        .expect("the program runs");
    (String::from_utf8(ran.stdout).unwrap(), said)
}

/// Builds, and gives the page the program embeds.
fn page_built(checkout: &Path, target: &Path) -> String {
    built(checkout, target).0
}

/// The checkout's build script with `line` as the first line of its `main`, so that a test can make the script wrong in
/// one way and see what the test of the right one would say.
fn with_line(checkout: &Path, line: &str) {
    let main = "fn main() {\n";
    let at = BUILD.find(main).expect("the script has a main") + main.len();
    let changed = format!("{}    {line}\n{}", &BUILD[..at], &BUILD[at..]);
    std::fs::write(checkout.join("crates/tiny/build.rs"), changed).unwrap();
}

/// The checkout's build script with `from` replaced by `to`.
fn with_replaced(checkout: &Path, from: &str, to: &str) {
    assert!(BUILD.contains(from), "the script has no {from}");
    std::fs::write(checkout.join("crates/tiny/build.rs"), BUILD.replacen(from, to, 1)).unwrap();
}

#[test]
fn two_checkouts_that_share_a_build_directory_each_embed_their_own_page() {
    let base = tempfile::tempdir().unwrap();
    let target = base.path().join("target");
    let a = checkout(base.path(), "a", "the page of a");
    let b = checkout(base.path(), "b", "the page of b");
    let (page, said) = built(&a, &target);
    assert_eq!(page, "the page of a", "{said}");
    let (page, said) = built(&b, &target);
    assert_eq!(page, "the page of b", "{said}");
    let (page, said) = built(&a, &target);
    assert_eq!(page, "the page of a", "{said}");
}

#[test]
fn a_build_with_nothing_changed_compiles_nothing_again() {
    let base = tempfile::tempdir().unwrap();
    let target = base.path().join("target");
    let a = checkout(base.path(), "a", "the page of a");
    assert_eq!(page_built(&a, &target), "the page of a");
    let again = cargo_build(&a, &target);
    let said = String::from_utf8_lossy(&again.stderr);
    assert!(again.status.success(), "{said}");
    assert!(!said.contains("Compiling"), "the second build compiled: {said}");
}

#[test]
fn the_check_that_nothing_compiled_again_sees_a_build_that_does() {
    // A script that watches a file that is not there is run again by every build, and so is the program it builds.
    let base = tempfile::tempdir().unwrap();
    let target = base.path().join("target");
    let a = checkout(base.path(), "a", "the page of a");
    with_line(&a, "println!(\"cargo::rerun-if-changed=../../no-such-file\");");
    assert_eq!(page_built(&a, &target), "the page of a");
    let again = cargo_build(&a, &target);
    let said = String::from_utf8_lossy(&again.stderr);
    assert!(again.status.success(), "{said}");
    assert!(said.contains("Compiling"), "the second build did not compile: {said}");
}

#[test]
fn a_page_built_again_is_the_page_the_next_build_embeds() {
    let base = tempfile::tempdir().unwrap();
    let target = base.path().join("target");
    let a = checkout(base.path(), "a", "the first page");
    assert_eq!(page_built(&a, &target), "the first page");
    std::fs::write(a.join("client/dist/index.html"), "the second page").unwrap();
    assert_eq!(page_built(&a, &target), "the second page");
}

#[test]
fn a_page_not_built_from_the_sources_at_hand_is_not_built_in() {
    let base = tempfile::tempdir().unwrap();
    let target = base.path().join("target");
    let a = checkout(base.path(), "a", "the page of a");
    assert_eq!(page_built(&a, &target), "the page of a");
    std::fs::write(a.join("client/src/page.ts"), "changed after the page was built").unwrap();
    let out = cargo_build(&a, &target);
    assert!(!out.status.success());
    let said = String::from_utf8_lossy(&out.stderr);
    assert!(
        said.contains("src/page.ts is not as the page was built from it"),
        "{said}"
    );
    assert!(said.contains("npm run build"), "{said}");
}

#[test]
fn temporary_a_script_that_does_not_watch_the_page_keeps_the_page_it_embedded() {
    let base = tempfile::tempdir().unwrap();
    let target = base.path().join("target");
    let a = checkout(base.path(), "a", "the first page");
    with_replaced(&a, "println!(\"cargo::rerun-if-changed=../../client/dist\");", "");
    assert_eq!(page_built(&a, &target), "the first page");
    std::fs::write(a.join("client/dist/index.html"), "the second page").unwrap();
    assert_eq!(page_built(&a, &target), "the first page");
}

#[test]
fn temporary_a_script_that_does_not_check_the_sources_builds_a_page_that_is_not_theirs() {
    let base = tempfile::tempdir().unwrap();
    let target = base.path().join("target");
    let a = checkout(base.path(), "a", "the page of a");
    with_replaced(
        &a,
        "stamp::check(&client, &stamped)",
        "Ok::<Vec<String>, String>(Vec::new())",
    );
    std::fs::write(a.join("client/src/page.ts"), "changed after the page was built").unwrap();
    assert_eq!(page_built(&a, &target), "the page of a");
}
