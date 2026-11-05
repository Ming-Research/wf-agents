//! What the lock does with another process: a holder that is killed frees it at once, even while a child the holder
//! started still runs, and a holder that is alive keeps it. The holder is this test binary run again on the test
//! `holder`, so that no other program is needed.

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use engine::{Lock, Refused};

/// What tells the test `holder` it is the holder run by `lock_is_freed_when_its_holder_is_killed` and not a test run.
const HOLD: &str = "WF_AGENTS_TEST_HOLD_LOCK_ON";

/// The holder: takes the lock of the directory named by the environment, starts a child that outlives the lock's
/// need of it, says so and waits to be killed. It gives up by itself after a minute, so that a test that fails before it
/// kills the holder leaves nothing running for long. In an ordinary run of the tests, where the environment names no
/// directory, it does nothing.
#[test]
fn holder() {
    let Some(dir) = std::env::var_os(HOLD) else { return };
    let _lock = Lock::acquire(dir.as_ref()).unwrap();
    let mut child = Command::new("sleep").arg("60").spawn().unwrap();
    println!("ready {}", child.id());
    std::thread::sleep(Duration::from_secs(60));
    child.kill().unwrap();
    child.wait().unwrap();
}

/// Runs the holder on `dir` and waits until it holds the lock; gives it with the process number of the child it started.
fn start_holder(dir: &Path) -> (Child, u32) {
    let mut holder = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "holder", "--nocapture", "--test-threads=1"])
        .env(HOLD, dir)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let out = BufReader::new(holder.stdout.take().unwrap());
    for line in out.lines() {
        // The test harness prints the test's name before what the test prints, on the same line.
        if let Some((_, pid)) = line.unwrap().split_once("ready ") {
            return (holder, pid.trim().parse().unwrap());
        }
    }
    panic!("the holder ended without holding the lock");
}

/// Whether the process is still there, by the shell's own `kill -0`, which sends no signal.
fn alive(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stderr(Stdio::null())
        .status()
        .unwrap()
        .success()
}

// While another process holds the lock it is refused and names that process; when that process is killed, the lock is
// taken at once, though the child the holder started is still running, because the child does not hold the lock.
#[test]
fn the_lock_is_freed_when_its_holder_is_killed_though_a_child_of_the_holder_runs() {
    let dir = std::env::temp_dir().join(format!("wf-agents-lock-process-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("db")).unwrap();
    let (mut holder, child) = start_holder(&dir);
    match Lock::acquire(&dir) {
        Err(Refused::Held { holder: Some(h), .. }) => assert_eq!(h.pid, holder.id()),
        other => panic!("{other:?}"),
    }
    // SIGKILL: the holder gets no chance to release anything.
    holder.kill().unwrap();
    holder.wait().unwrap();
    assert!(alive(child), "the holder's child ended with it");
    let taken = Lock::acquire(&dir);
    // The child was started by the process that is gone, and is stopped here by the number it was given.
    Command::new("kill").arg(child.to_string()).status().unwrap();
    taken.unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}
