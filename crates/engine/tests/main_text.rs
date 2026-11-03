//! Reads of a project's text from its main, as the checkout holds it after a fetch, against real repositories: a bare
//! origin, the project's checkout and a second clone that moves the origin on. They test the readers of the
//! configuration crate that the engine and the pages read the project's constitution and guide with.

use std::path::{Path, PathBuf};
use std::process::Command;

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=t", "-c", "user.email=t@t"])
        .args(["-c", "init.defaultBranch=main"])
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

fn write(dir: &Path, file: &str, text: &str) {
    let p = dir.join(file);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
}

/// A bare origin whose main holds one commit, the project's checkout cloned from it, and the project as a
/// configuration names them.
struct Repos {
    root: PathBuf,
    origin: PathBuf,
    checkout: PathBuf,
    project: config::Project,
}

impl Repos {
    fn new(name: &str) -> Repos {
        let root = std::env::temp_dir().join(format!("wf-agents-main-text-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let origin = root.join("origin.git");
        git(&root, &["init", "-q", "--bare", &origin.to_string_lossy()]);
        let checkout = root.join("checkout");
        git(
            &root,
            &["clone", "-q", &origin.to_string_lossy(), &checkout.to_string_lossy()],
        );
        write(&checkout, "AGENTS.md", "How to work here.\n");
        git(&checkout, &["add", "-A"]);
        git(&checkout, &["commit", "-q", "-m", "start"]);
        git(&checkout, &["push", "-q", "origin", "HEAD:main"]);
        let data = root.join("data");
        write(
            &data,
            "install.toml",
            r#"
[owner]
language = "English"
[items]
title_max = 40
[machines]
workstation = "here"
heavy = "builds"
poll_interval = 60
[machines.hosts.here]
slots = 1
[agents.clis.claude]
kind = "claude"
command = "claude"
[agents.levels.l]
desc = "all work"
cli = "claude"
model = "m"
[gh]
allow = ["pr view"]
[query]
max_rows = 10
timeout_ms = 1000
"#,
        );
        write(
            &data,
            "projects/p.toml",
            &format!(
                r#"
repo = "o/p"
checkout = "{}"
design_goal = "simple"
test_paths = ["tests/"]
rules = ["ground"]
[ci]
fast = "ci"
full = ["ci"]
merge_checks = ["check"]
"#,
                checkout.display()
            ),
        );
        write(&data, "rules/ground.md", "Facts first.\n");
        let project = config::working(&data).unwrap().projects.remove("p").unwrap();
        Repos {
            root,
            origin,
            checkout,
            project,
        }
    }

    /// A second clone of the origin, which moves main on.
    fn other(&self) -> PathBuf {
        let other = self.root.join("other");
        git(
            &self.root,
            &["clone", "-q", &self.origin.to_string_lossy(), &other.to_string_lossy()],
        );
        other
    }
}

impl Drop for Repos {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

// A file of main that is a link (an AGENTS.md that points to another file is common) is never given as the path it
// points to, which is what git stores for the link: the text is the file's, as a read of the checkout's files followed
// the link, or else an error that names the path. Reference: `std::fs::read_to_string`, which followed links when the
// checkout's files were read before main was read with git.
#[cfg(unix)]
#[test]
fn a_link_on_main_is_never_read_as_the_path_it_points_to() {
    let r = Repos::new("links");
    let checkout = &r.checkout;
    write(checkout, "CLAUDE.md", "How to work.\n");
    write(checkout, "docs/real.md", "The real constitution.\n");
    write(checkout, "design/real.md", "Decision: real.\n");
    std::fs::remove_file(checkout.join("AGENTS.md")).unwrap();
    std::os::unix::fs::symlink("CLAUDE.md", checkout.join("AGENTS.md")).unwrap();
    std::os::unix::fs::symlink("docs/real.md", checkout.join("constitution.md")).unwrap();
    std::os::unix::fs::symlink("real.md", checkout.join("design/link.md")).unwrap();
    git(checkout, &["add", "-A"]);
    git(checkout, &["commit", "-q", "-m", "links"]);
    git(checkout, &["push", "-q", "origin", "HEAD:main"]);
    let commit = config::main_commit(&r.project).unwrap();
    match config::read_at(&r.project, &commit, Path::new("constitution.md")) {
        Ok(text) => assert_eq!(text, "The real constitution.\n"),
        Err(why) => assert!(why.contains("constitution.md"), "{why}"),
    }
    let design = ["design/".to_string()];
    match config::project_guide_at(checkout, &commit, &design, "") {
        Ok(g) => {
            assert_ne!(g["agents_md"], "CLAUDE.md", "the guide is the path the link holds");
            assert_eq!(g["agents_md"], "How to work.\n");
            assert_eq!(g["records"]["design/link.md"], "Decision: real.");
        }
        Err(why) => assert!(why.contains("AGENTS.md") || why.contains("link.md"), "{why}"),
    }
}

// The checkout's `origin/<main>` is what the remote's main is after a fetch, whatever happened to it: a main that was
// rewritten and force-pushed, and a main whose name has a slash; a main the remote does not have is an error.
// Reference: the commit ids the other clone pushed, which is what `git ls-remote origin <main>` would give.
#[test]
fn a_rewritten_main_and_a_main_with_a_slash_in_its_name_are_what_the_remote_has_after_a_fetch() {
    let mut r = Repos::new("rewritten");
    let other = r.other();
    let first = config::main_commit(&r.project).unwrap();

    // The remote's main is rewritten: its commit is replaced by another one, which is not its child.
    write(&other, "constitution.md", "# Rewritten\n");
    git(&other, &["add", "-A"]);
    git(&other, &["commit", "-q", "--amend", "-m", "start, rewritten"]);
    let rewritten = git(&other, &["rev-parse", "HEAD"]);
    assert_ne!(rewritten, first);
    git(&other, &["push", "-q", "--force", "origin", "HEAD:main"]);
    config::fetch_main(&r.project).unwrap();
    assert_eq!(config::main_commit(&r.project).unwrap(), rewritten);
    assert_eq!(
        config::read_at(&r.project, &rewritten, Path::new("constitution.md")).unwrap(),
        "# Rewritten\n"
    );

    // A main named with a slash is `origin/release/1` in the checkout, and a fetch brings it.
    write(&other, "constitution.md", "# Release\n");
    git(&other, &["add", "-A"]);
    git(&other, &["commit", "-q", "-m", "release"]);
    let release = git(&other, &["rev-parse", "HEAD"]);
    git(&other, &["push", "-q", "origin", "HEAD:refs/heads/release/1"]);
    r.project.main = "release/1".into();
    assert_eq!(r.project.main_ref(), "origin/release/1");
    assert!(config::main_commit(&r.project).is_err());
    config::fetch_main(&r.project).unwrap();
    assert_eq!(config::main_commit(&r.project).unwrap(), release);
    assert_eq!(
        config::read_at(&r.project, &release, Path::new("constitution.md")).unwrap(),
        "# Release\n"
    );

    // A main the remote has not got cannot be fetched, and the error says which.
    r.project.main = "no-such-branch".into();
    let e = config::fetch_main(&r.project).unwrap_err();
    assert!(e.contains("could not be fetched"), "{e}");
    assert!(config::main_commit(&r.project).is_err());
}

// Fetches of main that overlap in one checkout all succeed: the works of the engine take their steps at the same time,
// and every round of a cell that reads main fetches it, so a fetch must not be turned down because another moved the
// same ref a moment before ("cannot lock ref ... is at X but expected Y"). Reference: the same fetches one after
// another, which all succeed.
#[test]
fn fetches_of_main_that_overlap_in_one_checkout_all_succeed() {
    let r = Repos::new("overlap");
    let other = r.other();
    for round in 1..=15 {
        write(&other, "f.md", &format!("{round}\n"));
        git(&other, &["add", "-A"]);
        git(&other, &["commit", "-q", "-m", "next"]);
        git(&other, &["push", "-q", "origin", "HEAD:main"]);
        let pushed = git(&other, &["rev-parse", "HEAD"]);
        let results: Vec<Result<(), String>> = std::thread::scope(|scope| {
            let fetches: Vec<_> = (0..8).map(|_| scope.spawn(|| config::fetch_main(&r.project))).collect();
            fetches.into_iter().map(|f| f.join().unwrap()).collect()
        });
        for result in &results {
            assert!(result.is_ok(), "round {round}: {result:?}");
        }
        assert_eq!(config::main_commit(&r.project).unwrap(), pushed);
    }
}

// A path names a file exactly, as the checkout's files named it: a character git's pathspecs give a meaning (`*`, `?`,
// `[`, a leading `:(top)`), a space and a letter outside ASCII are only characters of a name, so a name that is not in
// the commit is an error and one that is is read. Reference: `std::fs::read_to_string` on the same file in the
// checkout, which takes a name literally.
#[cfg(unix)]
#[ignore = "held out of one CI run to see the tests after it"]
#[test]
fn a_path_is_a_name_and_never_a_pattern() {
    let r = Repos::new("literal");
    let checkout = &r.checkout;
    write(checkout, "docs/sp ace [1]/a*.md", "Star.\n");
    // A directory name with letters outside ASCII (u and i with a diaeresis), written as escapes.
    write(checkout, "docs/\u{fc}n\u{ef}/c.md", "Umlaut.\n");
    write(checkout, "docs/-dash.md", "Dash.\n");
    write(checkout, "docs/other.md", "Other.\n");
    git(checkout, &["add", "-A"]);
    git(checkout, &["commit", "-q", "-m", "names"]);
    git(checkout, &["push", "-q", "origin", "HEAD:main"]);
    let commit = config::main_commit(&r.project).unwrap();
    for path in [
        "docs/sp ace [1]/a*.md",
        "docs/\u{fc}n\u{ef}/c.md",
        "docs/-dash.md",
        // Not names of any file: patterns and pathspec magic.
        "docs/sp ace [1]/a?.md",
        "docs/sp ace [1]/a*",
        "docs/sp ace [1]/",
        "docs/oth*.md",
        "docs/other.m[d]",
        "docs/[o]ther.md",
        ":(top)AGENTS.md",
        ":(top)docs/other.md",
        ":/AGENTS.md",
        ":(glob)docs/*.md",
        ":!docs/other.md",
    ] {
        let on_disk = std::fs::read_to_string(checkout.join(path));
        let at_main = config::read_at(&r.project, &commit, Path::new(path));
        match (on_disk, at_main) {
            (Ok(disk), Ok(main)) => assert_eq!(main, disk, "{path}"),
            (Err(_), Err(why)) => assert!(why.contains(path), "{path}: {why}"),
            (disk, main) => panic!("{path}: the checkout's file gives {disk:?}, main gives {main:?}"),
        }
    }
}

// A link is followed as a read of the checkout's files followed it, whatever form its target is written in: with a
// trailing slash, with a doubled slash, through `.` and through a directory that is itself a link and back up out of it.
// Reference: `std::fs::read_to_string` on the same path in the checkout.
#[cfg(unix)]
#[test]
fn a_link_is_read_as_the_checkout_reads_it_in_every_form_of_its_target() {
    let r = Repos::new("link-forms");
    let checkout = &r.checkout;
    write(checkout, "reasoning/constitution.md", "# Articles\n");
    write(checkout, "reasoning/deeper/x.md", "Deeper.\n");
    write(checkout, "top.md", "Top.\n");
    for (link, target) in [
        ("slash", "reasoning/"),
        ("double", "reasoning//deeper"),
        ("dot", "./reasoning/./deeper/"),
        ("back", "reasoning/deeper/.."),
        ("reasoning/up", "../top.md"),
        ("reasoning/deeper/in-a-link", "../../slash/constitution.md"),
        ("viadir", "reasoning/deeper/../constitution.md"),
    ] {
        std::os::unix::fs::symlink(target, checkout.join(link)).unwrap();
    }
    git(checkout, &["add", "-A"]);
    git(checkout, &["commit", "-q", "-m", "links"]);
    git(checkout, &["push", "-q", "origin", "HEAD:main"]);
    let commit = config::main_commit(&r.project).unwrap();
    for path in [
        "slash/constitution.md",
        "double/x.md",
        "dot/x.md",
        "back/constitution.md",
        "reasoning/up",
        "reasoning/deeper/in-a-link",
        "viadir",
        "slash/deeper/x.md",
        "slash",
        "double",
    ] {
        let on_disk = std::fs::read_to_string(checkout.join(path));
        let at_main = config::read_at(&r.project, &commit, Path::new(path));
        match (on_disk, at_main) {
            (Ok(disk), Ok(main)) => assert_eq!(main, disk, "{path}"),
            (Err(_), Err(why)) => assert!(why.contains(path) && why.contains(&commit), "{path}: {why}"),
            (disk, main) => panic!("{path}: the checkout's file gives {disk:?}, main gives {main:?}"),
        }
    }
}

// The records under a design path are the files a walk of the checkout found, links to directories followed as that
// walk followed them (`Path::is_dir` is true for a link to a directory): a records directory that is a link, and one
// inside a records directory. They are in the index, or reading them is an error that names the path; they are never
// left out without a word, which gives an agent a project guide that looks complete.
#[cfg(unix)]
#[ignore = "held out of one CI run to see the tests after it"]
#[test]
fn a_link_to_a_directory_of_records_is_not_left_out_without_a_word() {
    let r = Repos::new("record-links");
    let checkout = &r.checkout;
    write(checkout, "reasoning/one.md", "# One\n");
    write(checkout, "more/two.md", "# Two\n");
    write(checkout, "design/real.md", "# Real\n");
    std::os::unix::fs::symlink("../more", checkout.join("design/inner")).unwrap();
    std::os::unix::fs::symlink("reasoning", checkout.join("linked")).unwrap();
    git(checkout, &["add", "-A"]);
    git(checkout, &["commit", "-q", "-m", "records"]);
    git(checkout, &["push", "-q", "origin", "HEAD:main"]);
    let commit = config::main_commit(&r.project).unwrap();
    // What a walk of the checkout's files finds, by the paths the records are named with.
    let on_disk = |dir: &str| -> Vec<(String, String)> {
        let mut found = Vec::new();
        let mut stack = vec![PathBuf::from(dir)];
        while let Some(d) = stack.pop() {
            for entry in std::fs::read_dir(checkout.join(&d)).unwrap() {
                let rel = d.join(entry.unwrap().file_name());
                if checkout.join(&rel).is_dir() {
                    stack.push(rel);
                } else if rel.extension().is_some_and(|x| x == "md") {
                    found.push((
                        rel.to_string_lossy().into_owned(),
                        std::fs::read_to_string(checkout.join(&rel)).unwrap(),
                    ));
                }
            }
        }
        found.sort();
        found
    };
    for dir in ["design", "linked"] {
        let want = on_disk(dir);
        assert!(!want.is_empty(), "{dir}");
        let design = [format!("{dir}/")];
        match config::project_guide_at(checkout, &commit, &design, "") {
            Ok(g) => {
                for (path, text) in &want {
                    let first = text.lines().next().unwrap();
                    assert_eq!(g["records"][path], first, "{dir}: {path} is not in {}", g["records"]);
                }
            }
            Err(why) => assert!(why.contains(dir), "{dir}: {why}"),
        }
        match config::files_at(checkout, &commit, dir) {
            Ok(found) => assert_eq!(found, want, "{dir}"),
            Err(why) => assert!(why.contains(dir), "{dir}: {why}"),
        }
    }
}

// The checkout's `origin/<main>` is the remote's branch after a fetch even when the remote also has a tag of the same
// name at another commit: the commit read is the branch's. Reference: `git rev-parse refs/heads/main` in the origin.
#[test]
fn a_tag_named_like_main_does_not_hold_back_the_fetch_of_main() {
    let r = Repos::new("tag-main");
    let other = r.other();
    let old = git(&other, &["rev-parse", "HEAD"]);
    git(&other, &["tag", "main", &old]);
    git(&other, &["push", "-q", "origin", "refs/tags/main"]);
    write(&other, "AGENTS.md", "Newer.\n");
    git(&other, &["add", "-A"]);
    git(&other, &["commit", "-q", "-m", "newer"]);
    // The full name of the branch: `main` alone is ambiguous now that the origin has a tag of that name.
    git(&other, &["push", "-q", "origin", "HEAD:refs/heads/main"]);
    let branch = git(&r.origin, &["rev-parse", "refs/heads/main"]);
    assert_ne!(branch, old);
    config::fetch_main(&r.project).unwrap();
    assert_eq!(config::main_commit(&r.project).unwrap(), branch);
}
