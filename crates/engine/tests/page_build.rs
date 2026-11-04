//! What keeps a kept build of the owner's page (`.github/actions/page`) the right page. A build is kept under the content
//! of `client/` alone, so a stale page is embedded silently when the build reads anything outside it, or when a job
//! compiles the crate `web` before the page is in place. The action's own header states both rules; GitHub's workflow
//! syntax and cargo's rule that `crates/web/build.rs` reads `client/dist` are the references.

use std::path::{Component, Path, PathBuf};

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(rel: &str) -> String {
    let path = repo().join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The lines of a job of a workflow (from its key to the next job's), or the whole file for an action; comments out.
fn code_lines(text: &str, job: Option<&str>) -> Vec<String> {
    let mut inside = job.is_none();
    let mut out = Vec::new();
    for l in text.lines() {
        if let Some(name) = job {
            if l.starts_with("  ") && !l.starts_with("   ") {
                inside = l.trim_end() == format!("  {name}:");
            }
        }
        if inside && !l.trim().is_empty() && !l.trim_start().starts_with('#') {
            out.push(l.to_string());
        }
    }
    out
}

// A job that compiles `web` before the page is at `client/dist` fails to build, or, with a restored build state, embeds
// the page that state was built with. So in each place the page is got, the page action comes before every cargo
// command that compiles the workspace.
#[test]
fn the_page_is_in_place_before_any_cargo_command_that_compiles_web() {
    let places = [
        ("ci.yml test job", read(".github/workflows/ci.yml"), Some("test")),
        ("fast.yml test job", read(".github/workflows/fast.yml"), Some("test")),
        ("the lint action", read(".github/actions/lint/action.yml"), None),
    ];
    // Commands that build more than one crate's tests or lints; `cargo fmt` and the single-crate `api` run build no `web`.
    let compiles = [
        "cargo nextest run",
        "cargo build",
        "cargo clippy",
        "cargo test --workspace",
    ];
    for (place, text, job) in places {
        let lines = code_lines(&text, job);
        let page = lines
            .iter()
            .position(|l| l.contains("uses: ./.github/actions/page"))
            .unwrap_or_else(|| panic!("{place} does not get the page through the action"));
        let mut seen = 0;
        for (i, l) in lines.iter().enumerate() {
            if compiles.iter().any(|c| l.contains(c)) {
                seen += 1;
                assert!(i > page, "{place}: `{}` is before the page is in place", l.trim());
            }
        }
        assert!(
            seen > 0,
            "{place} has no command that compiles the workspace: the test is out of date"
        );
    }
}

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .flatten()
    {
        let p = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        if p.is_dir() {
            if name != "node_modules" && name != "dist" {
                files(&p, out);
            }
        } else {
            out.push(p);
        }
    }
}

/// The relative module specifiers a source file names: the quoted text after `from` or `import`.
fn specifiers(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for l in text.lines() {
        for key in ["from ", "import(", "import ", "@import "] {
            let mut rest = l;
            while let Some(at) = rest.find(key) {
                rest = &rest[at + key.len()..];
                let t = rest.trim_start();
                if let Some(q) = t.chars().next().filter(|c| *c == '\'' || *c == '"') {
                    if let Some(end) = t[1..].find(q) {
                        out.push(t[1..1 + end].to_string());
                    }
                }
            }
        }
    }
    out
}

// The kept build is named by the files of `client/`, so a file of the build that reads another directory (a relative
// import that leaves `client/`) or the environment would make two builds of one name differ. Such a change must widen the
// key in the same change (the action says so); this test is where it is told. Test files are not part of the build and
// are left out; the one environment variable the build's configuration reads, `WF_AGENTS_URL`, only addresses the dev
// server's proxy.
#[test]
fn the_build_of_the_page_reads_nothing_outside_client() {
    let client = repo().join("client");
    let mut all = Vec::new();
    files(&client, &mut all);
    let mut read_files = 0;
    for p in all {
        let rel = p.strip_prefix(&client).unwrap().to_path_buf();
        let name = rel.file_name().unwrap().to_string_lossy().into_owned();
        let source = ["ts", "tsx", "js", "mjs", "css"]
            .iter()
            .any(|e| name.ends_with(&format!(".{e}")));
        if !source || name.contains(".test.") {
            continue;
        }
        let text = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        read_files += 1;
        for spec in specifiers(&text).into_iter().filter(|s| s.starts_with('.')) {
            // The path of the module relative to `client/`, with `..` folded; one that climbs past `client/` has left it.
            let mut depth: i32 = 0;
            let from = rel.parent().unwrap().join(&spec);
            for c in from.components() {
                match c {
                    Component::ParentDir => depth -= 1,
                    Component::Normal(_) => depth += 1,
                    _ => {}
                }
                assert!(
                    depth >= 0,
                    "client/{} imports `{spec}`, which is outside client/",
                    rel.display()
                );
            }
        }
        for l in text
            .lines()
            .filter(|l| l.contains("process.env") || l.contains("import.meta.env"))
        {
            assert!(
                name == "vite.config.ts" && l.contains("process.env.WF_AGENTS_URL"),
                "client/{} reads the environment (`{}`): the key of the kept page must include it",
                rel.display(),
                l.trim()
            );
        }
    }
    assert!(
        read_files > 20,
        "only {read_files} source files found: the walk is wrong"
    );
}
