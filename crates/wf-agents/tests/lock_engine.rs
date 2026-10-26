//! The data directory's lock through the real command. A start that is refused changes nothing in the directory, and an
//! engine that is running holds the lock for as long as it runs and loses it when it is killed. The in-process tests
//! of the lock are in the engine crate; these start the `wf-agents` binary, which is where the lock has to be in
//! place before anything of the directory is touched.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use engine::{Lock, Refused};

const BIN: &str = env!("CARGO_BIN_EXE_wf-agents");

/// A new empty directory for one test.
fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("wf-agents-lock-cli-{name}-{}", std::process::id()));
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

/// A data directory with its configuration committed, as the engine reads it.
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

/// The names in a directory, in order.
fn listing(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// A start of the command on the directory: whether it succeeded, and what it said.
fn start(dir: &Path, once: bool) -> (bool, String) {
    let mut command = Command::new(BIN);
    command.arg("run");
    if once {
        command.arg("--once");
    }
    let out = command.arg("--data").arg(dir).output().unwrap();
    let said = String::from_utf8_lossy(&out.stderr).into_owned();
    (out.status.success(), said)
}

/// A process the test started, killed and reaped however the test ends.
struct Running(Child);

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// What a process that has ended wrote to its standard error.
fn stderr_of(child: &mut Child) -> String {
    let mut text = String::new();
    if let Some(mut err) = child.stderr.take() {
        let _ = err.read_to_string(&mut text);
    }
    text
}

// A1, for a start that is refused: it changes nothing. The `stop` file a running engine is watching is still there (a
// new engine deletes one as stale when it starts, which would cancel a stop meant for the engine that runs), and no
// database, log or other file appeared. The directory holds no configuration, so a start that got past the lock would
// fail at the configuration, and one that deleted `stop` first would leave a different listing. About 50 ms.
#[test]
fn a_refused_run_and_a_refused_once_leave_the_data_directory_as_it_was() {
    let d = dir("refused");
    std::fs::write(d.join("stop"), "").unwrap();
    let _holder = Lock::acquire(&d).unwrap();
    let before = listing(&d);
    assert_eq!(before, ["engine.lock", "stop"]);
    let holder = format!("engine already running: process {} holds", std::process::id());
    for once in [false, true] {
        let (succeeded, said) = start(&d, once);
        assert!(!succeeded, "once={once}: the start was not refused");
        assert!(said.contains(&holder), "once={once}: {said}");
        assert_eq!(listing(&d), before, "once={once}");
    }
    let _ = std::fs::remove_dir_all(&d);
}

// A1, A3 and A4 through a real engine. The engine starts over a lock file with a dead engine's text in it and rewrites
// it whole; it goes on holding the lock past its start-up, into its loop; a second `run` and a second `run --once` are
// refused naming it; and when it is killed outright the lock is free at once. Nothing else tests that the lock the
// command takes is still held after the engine is built (a guard dropped on the way passes the tests that hold a
// `Lock` themselves) or that a started engine's own text is what a refusal reads. About 2 s: the engine compiles the
// flow when it starts.
#[test]
fn a_running_engine_holds_the_lock_refuses_a_second_start_and_loses_it_when_killed() {
    let d = committed_dir("live");
    // A dead engine's text, longer than a holder's own, for the running engine to take over.
    std::fs::write(config::engine_lock(&d), "1 1\n".repeat(50)).unwrap();
    let child = Command::new(BIN)
        .arg("run")
        .arg("--data")
        .arg(&d)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut engine = Running(child);
    let pid = engine.0.id();

    // Wait until the engine has taken the lock, written who it is and opened its database. Reading the file, not
    // taking the lock, so that the test never holds it when the engine starts.
    let begun = Instant::now();
    loop {
        if let Some(status) = engine.0.try_wait().unwrap() {
            let said = stderr_of(&mut engine.0);
            panic!("the engine stopped by itself ({status}): {said}");
        }
        let text = std::fs::read_to_string(config::engine_lock(&d)).unwrap_or_default();
        if text.starts_with(&format!("{pid} ")) && config::database(&d).exists() {
            break;
        }
        assert!(
            begun.elapsed() < Duration::from_secs(60),
            "the engine had not started in 60 s"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    let text = std::fs::read_to_string(config::engine_lock(&d)).unwrap();
    assert!(
        text.ends_with('\n') && text.lines().count() == 1,
        "the dead engine's text was not rewritten whole: {text:?}"
    );

    // Past start-up, in its loop, the engine still holds the lock.
    std::thread::sleep(Duration::from_millis(500));
    if let Some(status) = engine.0.try_wait().unwrap() {
        let said = stderr_of(&mut engine.0);
        panic!("the engine stopped by itself ({status}): {said}");
    }
    match Lock::acquire(&d) {
        Err(Refused::Held { holder: Some(h), .. }) => assert_eq!(h.pid, pid),
        other => panic!("the running engine does not hold the lock: {other:?}"),
    }
    let holder = format!("engine already running: process {pid} holds");
    for once in [false, true] {
        let (succeeded, said) = start(&d, once);
        assert!(!succeeded, "once={once}: a second engine was started");
        assert!(said.contains(&holder), "once={once}: {said}");
    }

    // Killed outright, the engine releases nothing itself; the lock is free at once.
    engine.0.kill().unwrap();
    engine.0.wait().unwrap();
    if let Err(e) = Lock::acquire(&d) {
        panic!("the lock should be free once the engine is killed: {e}");
    }
    let _ = std::fs::remove_dir_all(&d);
}
