//! The lock across processes: while the holder lives a second taker is refused and told who holds it; when the holder
//! is killed outright the lock is free at once, even though a child the holder started still runs, because the child
//! did not inherit it.
//!
//! This target has its own `main` (`harness = false`): the holder must be a separate process that starts a child as the
//! engine does, so the test starts a copy of itself. Run with no arguments it is the test; run as `hold DIR` it is the
//! holder. It needs `sleep` and `kill`, which the Unix systems the engine runs on have.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use engine::{Lock, Refused};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("hold") => hold(Path::new(&args[2])),
        _ => test(),
    }
}

/// The holder: takes the lock, starts a long-lived child, says the child's process number, and waits for it.
fn hold(dir: &Path) {
    let _lock = Lock::acquire(dir).unwrap();
    let mut child = Command::new("sleep").arg("60").spawn().unwrap();
    println!("{}", child.id());
    child.wait().unwrap();
}

/// What the test started, cleaned up however it ends: the holder, the holder's child by the process number the
/// holder gave, and the directory.
struct Started {
    holder: Child,
    sleeper: u32,
    dir: PathBuf,
}

impl Drop for Started {
    fn drop(&mut self) {
        let _ = self.holder.kill();
        let _ = self.holder.wait();
        let _ = Command::new("kill").arg(self.sleeper.to_string()).status();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn alive(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .status()
        .unwrap()
        .success()
}

// A3. About 1 s.
fn test() {
    let dir = std::env::temp_dir().join(format!("wf-agents-lock-process-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut holder = Command::new(std::env::current_exe().unwrap())
        .arg("hold")
        .arg(&dir)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut line = String::new();
    BufReader::new(holder.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let sleeper: u32 = line
        .trim()
        .parse()
        .unwrap_or_else(|_| panic!("the holder did not take the lock and start its child: {line:?}"));
    let mut started = Started {
        holder,
        sleeper,
        dir: dir.clone(),
    };

    // While the holder lives, the lock is refused and the holder is named.
    match Lock::acquire(&dir) {
        Err(Refused::Held { holder: Some(h), .. }) => assert_eq!(h.pid, started.holder.id()),
        other => panic!("{other:?}"),
    }

    // Killed outright, the holder cannot release anything itself; its child goes on running.
    started.holder.kill().unwrap();
    started.holder.wait().unwrap();
    assert!(alive(sleeper), "the holder's child should still run");
    if let Err(e) = Lock::acquire(&dir) {
        panic!("the lock should be free once the holder is gone, though its child runs: {e}");
    }
    println!("ok");
}
