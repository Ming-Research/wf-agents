//! Builds the flows into the program: every file of the repository's `flow/` becomes a file the program holds, so that
//! the flows the engine runs are those of the source the program is built from, whatever main has moved on to since.
//! It also says which commit the program is built from, as the installation gives it in `WF_BUILD_COMMIT`
//! (`unknown` where it gives none): the build does not run git, so that nothing here depends on what the checkout's
//! state is. The paths cargo is told to watch are from the package's root, so that two checkouts that share a build
//! directory each watch their own files.

use std::path::PathBuf;

#[path = "build_support/commit.rs"]
mod commit;
#[path = "../web/build_support/tree.rs"]
mod tree;

fn main() {
    println!("cargo::rerun-if-env-changed=WF_CHECKOUT");
    println!("cargo::rerun-if-changed=../../flow");
    println!("cargo::rerun-if-env-changed=WF_BUILD_COMMIT");
    let Some(manifest) = std::env::var_os("CARGO_MANIFEST_DIR").map(PathBuf::from) else {
        panic!("cargo did not say where the package is (CARGO_MANIFEST_DIR)");
    };
    let files = match tree::read_tree(&manifest.join("../../flow")) {
        Ok(files) => files,
        Err(why) => panic!("{why}"),
    };
    let code = match tree::generate(&files, "FLOWS") {
        Ok(code) => format!("/// The files of the repository's `flow/`, by their path under it.\n{code}"),
        Err(why) => panic!("{why}"),
    };
    let out = PathBuf::from(std::env::var_os("OUT_DIR").unwrap_or_default()).join("flows.rs");
    if let Err(e) = std::fs::write(&out, code) {
        panic!("{}: {e}", out.display());
    }
    let given = match std::env::var("WF_BUILD_COMMIT") {
        Ok(given) => Some(given),
        Err(std::env::VarError::NotPresent) => None,
        Err(e) => panic!("WF_BUILD_COMMIT: {e}"),
    };
    println!("cargo::rustc-env=WF_COMMIT={}", commit::commit(given.as_deref()));
}
