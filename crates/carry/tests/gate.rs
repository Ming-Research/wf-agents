//! The gate that keeps a machine of self-hosted runners quiet while a measurement times on it (`runner/measure-gate.sh`),
//! run as a runner runs it, on a machine of the test's own: a job is a process named `Runner.Worker` that the test starts,
//! the runners' home is a directory of the test's, and a stand-in for `pgrep` lists the test's workers alone, since the
//! machine that runs the tests runs real ones. Each job holds until the test lets it end. Linux alone: the gate reads
//! `/proc`, as the machines it is installed on have it.
#![cfg(target_os = "linux")]

use std::io::{BufRead, BufReader, Write};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

/// The gate as the repository holds it.
fn gate() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../runner/measure-gate.sh")
}

/// A worker runs its job and ends with it, as a runner's does; its job is given the fifo it holds on as its fd 3.
const WORKER: &str = "#!/bin/bash\nexec 3<> \"$1\"\nshift\n\"$@\"\n";

/// The test's own workers, as `pgrep -x Runner.Worker` lists a machine's: by their name, and by the file they run.
const PGREP: &str = "#!/bin/bash
for d in /proc/[0-9]*; do
  [ \"$(cat \"$d/comm\" 2> /dev/null)\" = Runner.Worker ] || continue
  tr '\\0' '\\n' < \"$d/cmdline\" 2> /dev/null | grep -qxF \"$WORKER\" && echo \"${d#/proc/}\"
done
exit 0
";

/// What a job does: waits at the gate as a runner's hook, then holds; holds as a job that runs; or takes the gate for
/// a measurement and holds it.
const HOOK: &str = "bash \"$GATE\"; echo started; read -r _ <&3";
const RUNNING: &str = "echo running; read -r _ <&3";
const MEASUREMENT: &str = ". \"$GATE\"; wait_for_a_quiet_machine; echo quiet; read -r _ <&3";

/// A machine of the test's own, its jobs, and every line they printed, each with the job's name.
struct Machine {
    dir: tempfile::TempDir,
    jobs: Vec<(String, Child, PathBuf)>,
    to: Sender<(String, String)>,
    came: Receiver<(String, String)>,
    said: Vec<(String, String)>,
}

fn executable(path: &Path, text: &str) {
    std::fs::write(path, text).expect("written");
    let made = Command::new("chmod").arg("+x").arg(path).status().expect("chmod runs");
    assert!(made.success());
}

impl Machine {
    fn new() -> Machine {
        let dir = tempfile::tempdir().expect("a directory");
        let bin = dir.path().join("bin");
        std::fs::create_dir_all(&bin).expect("a directory");
        executable(&bin.join("Runner.Worker"), WORKER);
        executable(&bin.join("pgrep"), PGREP);
        let (to, came) = channel();
        Machine {
            dir,
            jobs: Vec::new(),
            to,
            came,
            said: Vec::new(),
        }
    }

    fn home(&self) -> &Path {
        self.dir.path()
    }

    /// Starts a job of this name that does `what`.
    fn start(&mut self, name: &str, what: &str) {
        let bin = self.home().join("bin");
        let fifo = self.home().join(format!("{name}.hold"));
        let made = Command::new("mkfifo").arg(&fifo).status().expect("mkfifo runs");
        assert!(made.success());
        let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap_or_default());
        let mut child = Command::new(bin.join("Runner.Worker"))
            .arg(&fifo)
            .args(["bash", "-c", what])
            .env("HOME", self.home())
            .env("PATH", path)
            .env("WORKER", bin.join("Runner.Worker"))
            .env("GATE", gate())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            // A group of its own, so that the job and what it started end with it.
            .process_group(0)
            .spawn()
            .expect("the worker starts");
        let out = child.stdout.take().expect("its output is piped");
        let (to, name_of) = (self.to.clone(), name.to_string());
        std::thread::spawn(move || {
            for line in BufReader::new(out).lines().map_while(Result::ok) {
                let _ = to.send((name_of.clone(), line));
            }
        });
        self.jobs.push((name.to_string(), child, fifo));
    }

    /// Lets the job of this name end what it holds.
    fn let_go(&mut self, name: &str) {
        let (_, _, fifo) = (self.jobs.iter())
            .find(|(n, ..)| n == name)
            .expect("a job of that name");
        let mut hold = std::fs::OpenOptions::new()
            .write(true)
            .open(fifo)
            .expect("its fifo opens");
        hold.write_all(b"\n").expect("written");
    }

    /// Ends the job of this name at once, with every process of it, as a runner that dies or a job cancelled at the
    /// hook ends: its mark at the gate is left behind.
    fn kill(&mut self, name: &str) {
        let (_, child, _) = (self.jobs.iter_mut())
            .find(|(n, ..)| n == name)
            .expect("a job of that name");
        let group = format!("-{}", child.id());
        let _ = Command::new("kill").args(["-KILL", "--", &group]).status();
        let _ = child.wait();
    }

    /// Whether the job of this name printed this line within the time given, every line printed meanwhile kept.
    fn says(&mut self, name: &str, line: &str, within: Duration) -> bool {
        let end = Instant::now() + within;
        loop {
            if (self.said.iter()).any(|(n, l)| n == name && l == line) {
                return true;
            }
            let left = end.saturating_duration_since(Instant::now());
            match self.came.recv_timeout(left) {
                Ok(said) => self.said.push(said),
                Err(_) => return false,
            }
        }
    }
}

impl Drop for Machine {
    fn drop(&mut self) {
        let names: Vec<String> = self.jobs.iter().map(|(name, ..)| name.clone()).collect();
        for name in names {
            self.kill(&name);
        }
    }
}

const SOON: Duration = Duration::from_secs(10);
const A_WHILE: Duration = Duration::from_millis(2500);

// With no measurement a job passes the hook at once; while a measurement holds the gate, a job that starts waits in the
// hook, and starts once the measurement lets the gate go. A few seconds alone, the waits the test gives a job to start
// where it must not; the one test of the hook.
#[test]
fn a_job_starts_at_once_with_no_measurement_and_waits_in_the_hook_while_one_holds_the_gate() {
    let mut machine = Machine::new();
    machine.start("first", HOOK);
    assert!(machine.says("first", "started", SOON), "{:?}", machine.said);
    machine.let_go("first");
    machine.start("measurement", MEASUREMENT);
    assert!(machine.says("measurement", "quiet", SOON), "{:?}", machine.said);
    machine.start("job", HOOK);
    assert!(
        !machine.says("job", "started", A_WHILE),
        "a job starts beside a measurement"
    );
    machine.let_go("measurement");
    assert!(machine.says("job", "started", SOON), "{:?}", machine.said);
}

// A measurement takes the gate, then waits for each job that runs on the machine to end, and for nothing else: not for
// a job waiting in the hook, not for a second measurement waiting for the gate, and not for the mark of a worker that has
// ended, which it lets go of. The second measurement times only once the first lets the gate go. A few seconds alone,
// the waits the test gives each to go on where it must not; the one test of what a measurement waits for.
#[test]
fn a_measurement_waits_for_the_jobs_that_run_and_not_for_those_that_wait_at_the_gate() {
    let mut machine = Machine::new();
    let waiting = machine.home().join(".wf-agents-runner/waiting");
    std::fs::create_dir_all(&waiting).expect("a directory");
    // No process has this pid: it is the most a pid can be, and pids are below it.
    let ended = waiting.join("4194304");
    std::fs::write(&ended, "").expect("a mark");
    machine.start("running", RUNNING);
    assert!(machine.says("running", "running", SOON), "{:?}", machine.said);
    machine.start("first", MEASUREMENT);
    assert!(
        !machine.says("first", "quiet", A_WHILE),
        "a measurement times beside a job that runs"
    );
    machine.start("second", MEASUREMENT);
    machine.start("job", HOOK);
    assert!(
        !machine.says("job", "started", A_WHILE),
        "a job starts beside a measurement"
    );
    machine.let_go("running");
    assert!(machine.says("first", "quiet", SOON), "{:?}", machine.said);
    assert!(!ended.exists(), "the mark of a worker that has ended is let go of");
    // The job is cancelled while it waits in the hook, and leaves its mark, which holds nothing.
    machine.kill("job");
    assert!(
        !machine.says("second", "quiet", A_WHILE),
        "two measurements time together"
    );
    machine.let_go("first");
    assert!(machine.says("second", "quiet", SOON), "{:?}", machine.said);
}
