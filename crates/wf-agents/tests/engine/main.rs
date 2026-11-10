//! `wf-agents run` and the commands that talk to it, each run as the command: the engine runs the trial flow (the
//! service crate's `tests/trial/flow`) with a script whose rounds are short, over a data directory of its own. The
//! tests are kept in modules by subject, so that a new test goes into the module of what it tests.

mod failing;
mod items;
mod starting;

use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

/// The trial flow.
pub fn trial() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../service/tests/trial/flow")
}

/// A script for the trial flow whose rounds and tools take a few milliseconds: the plan asks the owner, the build
/// asks for the CI, and the review is content at once.
pub const SCRIPT: &str = r#"
[took]
ms = 20

[tools.ci]
answer = "Every check passed."
ms = 20

[[flows.trial.rounds]]
cell = "plan"
asks = [{ question = "Should the greeting name the user?" }]

[[flows.trial.rounds]]
cell = "build"
asks = [{ tool = "ci", input = "the work's branch" }]

[[flows.trial.rounds]]
cell = "review"
hands_in = [{ block = "verdict", value = "yes" }]
"#;

/// A data directory and a script beside it.
pub struct Place {
    pub dir: tempfile::TempDir,
}

impl Place {
    pub fn new(script: &str) -> Place {
        let dir = tempfile::tempdir().expect("a directory");
        std::fs::create_dir(dir.path().join("data")).expect("the data directory is made");
        std::fs::write(dir.path().join("script.toml"), script).expect("the script is written");
        Place { dir }
    }

    pub fn data(&self) -> PathBuf {
        self.dir.path().join("data")
    }

    pub fn script(&self) -> PathBuf {
        self.dir.path().join("script.toml")
    }
}

/// `wf-agents run` on the trial flow, on any free port.
pub fn run(data: &Path, script: &Path) -> Command {
    run_on(data, &trial(), script, 0)
}

/// `wf-agents run` on this flow and port.
pub fn run_on(data: &Path, flow: &Path, script: &Path, port: u16) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_wf-agents"));
    command
        .arg("run")
        .arg("--data")
        .arg(data)
        .arg("--flow")
        .arg(flow)
        .arg("--script")
        .arg(script);
    command.args(["--port", &port.to_string()]);
    command
}

/// A running engine, stopped when it is dropped.
pub struct Engine {
    child: Child,
    pub port: u16,
}

impl Engine {
    /// Starts the engine on the place's data directory and script, and waits until it serves.
    pub fn start(place: &Place) -> Engine {
        let mut child = (run(&place.data(), &place.script()))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the engine starts");
        let mut line = String::new();
        let stdout = child.stdout.take().expect("the engine's output");
        BufReader::new(stdout)
            .read_line(&mut line)
            .expect("the engine's first line");
        let port = (line.trim().strip_prefix("The owner's pages are at http://127.0.0.1:"))
            .and_then(|rest| rest.strip_suffix("/#/works"))
            .and_then(|port| port.parse().ok());
        let Some(port) = port else {
            let mut errors = String::new();
            if let Some(mut stderr) = child.stderr.take() {
                let _ = stderr.read_to_string(&mut errors);
            }
            let _ = child.kill();
            panic!("the engine did not say where it serves: {line:?}; it said {errors:?}");
        };
        Engine { child, port }
    }

    /// Runs a command against the engine.
    pub fn command(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_wf-agents"))
            .args(args)
            .args(["--port", &self.port.to_string()])
            .output()
            .expect("the command runs")
    }

    /// What a command that succeeds prints.
    pub fn says(&self, args: &[&str]) -> String {
        let out = self.command(args);
        assert!(
            out.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).expect("the output is text")
    }

    /// What a command that fails says on its error stream.
    pub fn refuses(&self, args: &[&str]) -> String {
        let out = self.command(args);
        assert!(
            !out.status.success(),
            "{args:?} succeeded: {}",
            String::from_utf8_lossy(&out.stdout)
        );
        String::from_utf8(out.stderr).expect("the output is text")
    }

    /// Runs the command until what it prints has a line that `wanted` takes, and gives that line.
    pub fn until(&self, args: &[&str], wanted: impl Fn(&str) -> bool) -> String {
        let start = Instant::now();
        loop {
            let said = self.says(args);
            if let Some(line) = said.lines().find(|line| wanted(line)) {
                return line.to_string();
            }
            assert!(
                start.elapsed() < Duration::from_secs(30),
                "{args:?} never said it: {said}"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
