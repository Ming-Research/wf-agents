//! The engine's hold on its data directory. The test is in a file of its own because it starts git: a child that is
//! forked while another test of the same process holds a lock keeps a copy of the open file, and with it the lock,
//! until it runs its program, so a lock that test has dropped would still look held.
//!
//! What a kernel lock does is flock(2)'s: the lock belongs to the open file and is gone when the last copy of it is
//! closed. Whether a second engine can start is decided by the lock's purpose, one engine on a data directory.

use std::path::{Path, PathBuf};
use std::process::Command;

use engine::agent::{Agents, Reply, Request};
use engine::hub::{CheckRun, Failure, GitHub};
use engine::{Engine, Lock, Refused};

/// A new empty directory for one test.
fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("wf-agents-lock-attack-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=t", "-c", "user.email=t@t"])
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// What the engine's configuration needs of a data directory: the files an engine reads from its last commit.
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
worktrees = "/tmp"
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
design_paths = ["design/"]
test_paths = ["tests/"]
rules = ["ground"]
[ci]
fast = "ci"
full = ["ci"]
merge_checks = ["check"]
"#;

/// A data directory with its configuration committed.
fn committed_dir(name: &str) -> PathBuf {
    let d = dir(name);
    std::fs::create_dir_all(d.join("projects")).unwrap();
    std::fs::create_dir_all(d.join("rules")).unwrap();
    std::fs::write(d.join("install.toml"), INSTALL).unwrap();
    std::fs::write(d.join("projects/p.toml"), PROJECT).unwrap();
    std::fs::write(d.join("rules/ground.md"), "Facts first.\n").unwrap();
    git(&d, &["init", "-q"]);
    git(&d, &["add", "-A"]);
    git(&d, &["commit", "-q", "-m", "configuration"]);
    d
}

/// Agents and GitHub for an engine that starts no work.
struct Idle;

impl Agents for Idle {
    fn run(&self, _r: &Request) -> Reply {
        unreachable!("no work starts")
    }
}

impl GitHub for Idle {
    fn pull_request(
        &self,
        _repo: &str,
        _branch: &str,
        _base: &str,
        _title_body: Option<(&str, &str)>,
        _existing: Option<i64>,
    ) -> Result<i64, Failure> {
        unreachable!("no work starts")
    }

    fn checks(&self, _repo: &str, _sha: &str, _workflow: &str) -> Result<Vec<CheckRun>, Failure> {
        unreachable!("no work starts")
    }

    fn rerun(&self, _repo: &str, _sha: &str, _workflow: &str) -> Result<Vec<CheckRun>, Failure> {
        unreachable!("no work starts")
    }

    fn merge(&self, _repo: &str, _pr: i64) -> Result<String, Failure> {
        unreachable!("no work starts")
    }

    fn close(&self, _repo: &str, _pr: i64, _comment: &str) -> Result<(), Failure> {
        unreachable!("no work starts")
    }

    fn gh(&self, _args: &[String]) -> Result<(i32, String), Failure> {
        unreachable!("no work starts")
    }
}

// A1 and A2, for the engine itself: the lock handed to `Engine::new` stays held for as long as the engine lives and
// is freed when it is dropped. A guard dropped at once (`drop(lock)`, or a lock the engine never stores) passes every
// test that holds a `Lock` by itself, while a second engine starts. About 5 s: the engine compiles the flow when it
// is built (4.5 s and 7.1 s for the same construction in two CI logs of the file it was in before).
#[test]
fn an_engine_holds_its_directory_until_it_is_dropped() {
    let d = committed_dir("engine");
    let lock = Lock::acquire(&d).unwrap();
    let committed = config::committed(&d).unwrap();
    let engine = Engine::new(lock, committed, Box::new(Idle), Box::new(Idle)).unwrap();
    match Lock::acquire(&d) {
        Err(Refused::Held { holder: Some(h), .. }) => assert_eq!(h.pid, std::process::id()),
        other => panic!("a second engine could take the directory while the first lives: {other:?}"),
    }
    drop(engine);
    if let Err(e) = Lock::acquire(&d) {
        panic!("the directory should be free once the engine is dropped: {e}");
    }
    let _ = std::fs::remove_dir_all(&d);
}
