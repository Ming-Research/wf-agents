//! One engine for each data directory, through the built command: while another process holds the directory's lock,
//! `run` and `run --once` fail naming it and change nothing in the directory. A command that is not given a database is
//! still refused for that, as before, with nothing made.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

use engine::Lock;

const BIN: &str = env!("CARGO_BIN_EXE_wf-agents");

const INSTALL: &str = r#"
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
"#;

const PROJECT: &str = r#"
repo = "o/p"
checkout = "/tmp/p"
design_goal = "simple"
test_paths = ["tests/"]
rules = ["ground"]
[ci]
fast = "ci"
full = ["ci"]
merge_checks = ["check"]
[limits]
fixes = 5
"#;

fn command(data: &Path, args: &[&str]) -> Command {
    let mut c = Command::new(BIN);
    c.args(args)
        .arg("--data")
        .arg(data)
        .env("GIT_CEILING_DIRECTORIES", std::env::temp_dir())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    c
}

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@t")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@t")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// A data directory as the owner keeps one, with its database made by `wf-agents init`.
fn data_directory(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("wf-agents-one-engine-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("projects")).unwrap();
    std::fs::create_dir_all(dir.join("rules")).unwrap();
    std::fs::write(dir.join("install.toml"), INSTALL).unwrap();
    std::fs::write(dir.join("projects/p.toml"), PROJECT).unwrap();
    std::fs::write(dir.join("rules/ground.md"), "Facts first.").unwrap();
    git(&dir, &["init", "-q"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "configuration"]);
    let out = command(&dir, &["init"]).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    dir
}

/// Every file of the data directory with its size, apart from the lock file: what a refused start must leave as it was.
fn files(dir: &Path) -> BTreeMap<String, u64> {
    let mut out = BTreeMap::new();
    let mut todo = vec![dir.to_path_buf()];
    while let Some(d) = todo.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let e = e.unwrap();
            let name = e.path();
            if e.file_name() == ".git" || e.file_name() == "engine.lock" {
                continue;
            }
            if e.file_type().unwrap().is_dir() {
                todo.push(name);
            } else {
                let rel = name.strip_prefix(dir).unwrap().display().to_string();
                out.insert(rel, e.metadata().unwrap().len());
            }
        }
    }
    out
}

/// Runs the command to its end, or kills it after `limit` and says so: an engine that was not refused would serve for
/// ever.
fn finished(mut child: Child, limit: Duration) -> Result<Output, String> {
    let start = Instant::now();
    while child.try_wait().unwrap().is_none() {
        if start.elapsed() > limit {
            child.kill().unwrap();
            child.wait().unwrap();
            return Err(format!("still running after {limit:?}: the engine was not refused"));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Ok(child.wait_with_output().unwrap())
}

// While the lock is held by another process, a second `run` and a second `run --once` fail with exit status 1 and say
// which process holds it, before they open the database or take the stop file away; the directory is as it was.
#[test]
fn a_second_run_and_a_second_run_once_are_refused_naming_the_holder_and_change_nothing() {
    let dir = data_directory("refused");
    // This process is the holder, so the number in the message is its own.
    let held = Lock::acquire(&dir).unwrap();
    std::fs::write(dir.join("stop"), "").unwrap();
    let before = files(&dir);
    for args in [&["run", "--once"][..], &["run"][..]] {
        let out = finished(command(&dir, args).spawn().unwrap(), Duration::from_secs(20)).unwrap();
        let said = String::from_utf8_lossy(&out.stderr);
        assert_eq!(out.status.code(), Some(1), "{args:?}: {said}");
        assert!(
            said.contains(&format!("process {}", std::process::id())),
            "{args:?}: {said}"
        );
        assert!(dir.join("stop").exists(), "{args:?} took the stop file away");
        assert_eq!(files(&dir), before, "{args:?} changed the directory");
    }
    // The refused commands left nothing holding the lock: once the holder is dropped it is taken again.
    drop(held);
    Lock::acquire(&dir).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}
