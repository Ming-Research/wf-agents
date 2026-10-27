//! Builds the page into the binary: every file of the client's build (`client/dist`, which `npm run build` writes in
//! `client/`) becomes a static the server serves. The page is part of the program, so a build without it fails.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            files(&p, out);
        } else {
            out.push(p);
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
    for p in &found {
        let rel = p.strip_prefix(&root).unwrap_or(p);
        let name: Vec<String> = rel
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect();
        let abs = p.canonicalize().unwrap_or_else(|_| p.clone());
        let _ = writeln!(
            code,
            "    ({:?}, include_bytes!({:?})),",
            name.join("/"),
            abs.display().to_string()
        );
    }
    code.push_str("];\n");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap_or_default()).join("page.rs");
    if let Err(e) = std::fs::write(&out, code) {
        panic!("{}: {e}", out.display());
    }
}
