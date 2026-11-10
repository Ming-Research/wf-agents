//! Builds the page into the binary: every file of the page's build (`client/dist`, which `npm run build` writes in
//! `client/`) becomes a file the server serves, so that one command serves the page with nothing beside it. The page
//! is part of the program, so a build without it fails and says how to make it. Brought over from the engine on `main`,
//! whose interface serves its page the same way.

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
