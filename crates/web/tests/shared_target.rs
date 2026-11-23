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

/// The build script this crate had before the page's stamp, which fixed the checkout's path when it was compiled and
/// handed cargo an absolute path to watch.
const BEFORE: &str = r####"//! Builds the page into the binary: every file of the page's build (`client/dist`, which `npm run build` writes in
//! `client/`) becomes a file the server serves, so that one command serves the page with nothing beside it. The page
//! is part of the program, so a build without it fails and says how to make it. Brought over from the engine on
//! `old-main`, whose interface serves its page the same way.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            files(&path, out);
        } else {
            out.push(path);
        }
    }
}

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../client/dist");
    println!("cargo::rerun-if-changed={}", root.display());
    if !root.join("index.html").is_file() {
        panic!(
            "{} has no index.html: build the page first, with `npm ci` and `npm run build` in client/",
            root.display()
        );
    }
    let mut found = Vec::new();
    files(&root, &mut found);
    found.sort();
    let mut code = String::from(
        "/// The page's files, by their path under the page's root.\npub static FILES: &[(&str, &[u8])] = &[\n",
    );
    for path in &found {
        let relative = path.strip_prefix(&root).unwrap_or(path);
        let name: Vec<String> = relative
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect();
        let absolute = path.canonicalize().unwrap_or_else(|_| path.clone());
        let _ = writeln!(
            code,
            "    ({:?}, include_bytes!({:?})),",
            name.join("/"),
            absolute.display().to_string()
        );
    }
    code.push_str("];\n");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap_or_default()).join("page.rs");
    if let Err(e) = std::fs::write(&out, code) {
        panic!("{}: {e}", out.display());
    }
}
"####;

/// The SHA-256 of `abc`, the one source file of every checkout, so that the stamp is the same in each.
const ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

/// One checkout of the layout: a workspace with the package `tiny` that holds the build script, and `client/` with a
/// source, the page built from it and the stamp. Written whole, so that every file of every checkout is older than the
/// first build.
fn checkout(base: &Path, name: &str, page: &str, build: &str, stamped: bool) -> PathBuf {
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
    put(package.join("build.rs"), build);
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
    if stamped {
        put(root.join("client/dist/source.sha256"), &format!("{ABC}  src/page.ts\n"));
    }
    root
}

fn cargo_build(checkout: &Path, target: &Path) -> Output {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    Command::new(cargo)
        .args(["build", "--color", "never"])
        .current_dir(checkout)
        .env("CARGO_TARGET_DIR", target)
        .output()
        .expect("cargo runs")
}

/// Builds, and gives the page the program embeds.
fn page_built(checkout: &Path, target: &Path) -> String {
    let out = cargo_build(checkout, target);
    assert!(
        out.status.success(),
        "the build of {} failed: {}",
        checkout.display(),
        String::from_utf8_lossy(&out.stderr)
    );
    let ran = Command::new(target.join("debug/tiny"))
        .output()
        .expect("the program runs");
    String::from_utf8(ran.stdout).unwrap()
}

#[test]
fn two_checkouts_that_share_a_build_directory_each_embed_their_own_page() {
    let base = tempfile::tempdir().unwrap();
    let target = base.path().join("target");
    let a = checkout(base.path(), "a", "the page of a", BUILD, true);
    let b = checkout(base.path(), "b", "the page of b", BUILD, true);
    assert_eq!(page_built(&a, &target), "the page of a");
    assert_eq!(page_built(&b, &target), "the page of b");
    assert_eq!(page_built(&a, &target), "the page of a");
}

#[test]
fn the_script_before_the_stamp_embeds_the_page_of_another_checkout() {
    let base = tempfile::tempdir().unwrap();
    let target = base.path().join("target");
    let a = checkout(base.path(), "a", "the page of a", BEFORE, false);
    let b = checkout(base.path(), "b", "the page of b", BEFORE, false);
    assert_eq!(page_built(&a, &target), "the page of a");
    assert_eq!(page_built(&b, &target), "the page of a");
}

#[test]
fn a_build_with_nothing_changed_compiles_nothing_again() {
    let base = tempfile::tempdir().unwrap();
    let target = base.path().join("target");
    let a = checkout(base.path(), "a", "the page of a", BUILD, true);
    assert_eq!(page_built(&a, &target), "the page of a");
    let again = cargo_build(&a, &target);
    let said = String::from_utf8_lossy(&again.stderr);
    assert!(again.status.success(), "{said}");
    assert!(!said.contains("Compiling"), "the second build compiled: {said}");
}

#[test]
fn a_page_built_again_is_the_page_the_next_build_embeds() {
    let base = tempfile::tempdir().unwrap();
    let target = base.path().join("target");
    let a = checkout(base.path(), "a", "the first page", BUILD, true);
    assert_eq!(page_built(&a, &target), "the first page");
    std::fs::write(a.join("client/dist/index.html"), "the second page").unwrap();
    assert_eq!(page_built(&a, &target), "the second page");
}

#[test]
fn a_page_not_built_from_the_sources_at_hand_is_not_built_in() {
    let base = tempfile::tempdir().unwrap();
    let target = base.path().join("target");
    let a = checkout(base.path(), "a", "the page of a", BUILD, true);
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
