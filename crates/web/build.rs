//! Builds the page into the binary: every file of the page's build (`client/dist`, which `npm run build` writes in
//! `client/`) becomes a file the server serves, so that one command serves the page with nothing beside it. The page
//! is part of the program, so a build without it fails and says how to make it; so does a build of a page that is not
//! made from the sources the program is built with (`build_support/stamp.rs`). The checkout is found from where cargo
//! runs this script, not from where it was compiled, and the paths cargo is told to watch are from the package's root,
//! so that two checkouts that share a build directory each watch their own files.

use std::path::PathBuf;

#[path = "build_support/stamp.rs"]
mod stamp;
#[path = "build_support/tree.rs"]
mod tree;

/// The stamp `npm run build` writes into the page's files, which is not part of the page.
const STAMP: &str = "source.sha256";

fn main() {
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR").map(PathBuf::from);
    let Some(manifest) = manifest else {
        panic!("cargo did not say where the package is (CARGO_MANIFEST_DIR)");
    };
    let client = manifest.join("../../client");
    let dist = client.join("dist");
    println!("cargo::rerun-if-env-changed=WF_CHECKOUT");
    println!("cargo::rerun-if-changed=../../client/dist");
    if !dist.join("index.html").is_file() {
        panic!(
            "{} has no index.html: build the page first, with `npm ci` and `npm run build` in client/",
            dist.display()
        );
    }
    let stamped = match std::fs::read_to_string(dist.join(STAMP)) {
        Ok(stamped) => stamped,
        Err(e) => panic!(
            "{}: {e}; the page was not built by `npm run build`, or by one that leaves it: {}",
            dist.join(STAMP).display(),
            stamp::MEND
        ),
    };
    let sources = match stamp::check(&client, &stamped) {
        Ok(sources) => sources,
        Err(why) => panic!("{why}"),
    };
    for source in sources {
        println!("cargo::rerun-if-changed=../../client/{source}");
    }
    let files = match tree::read_tree(&dist) {
        Ok(files) => files.into_iter().filter(|(name, _)| name != STAMP).collect::<Vec<_>>(),
        Err(why) => panic!("{why}"),
    };
    let code = match tree::generate(&files, "FILES") {
        Ok(code) => format!("/// The page's files, by their path under the page's root.\n{code}"),
        Err(why) => panic!("{why}"),
    };
    let out = PathBuf::from(std::env::var_os("OUT_DIR").unwrap_or_default()).join("page.rs");
    if let Err(e) = std::fs::write(&out, code) {
        panic!("{}: {e}", out.display());
    }
}
