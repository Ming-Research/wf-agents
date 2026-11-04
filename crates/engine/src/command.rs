//! Runs a command the project's configuration holds, for the tools that run one (`dependency_sources`, and the checks
//! `ci` runs before it pushes): without a shell, in a directory of the work's worktree, with the engine's environment, a
//! time limit, and everything it started stopped with it. The engine knows no language: the words are the project's, and
//! this only runs them and gives what became of them.

use std::io::Read;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use sessions::cli::{STOP_WAIT, signal_group};

/// How often a running command is looked at to see whether it has ended or run out of time.
const POLL: Duration = Duration::from_millis(20);

/// How long what a command wrote is waited for once the command has ended, in case a process it started still holds
/// the pipe it writes to; what has come by then is what the result gives.
const DRAIN: Duration = Duration::from_secs(2);

/// What a command left: its exit code, none when it was stopped (by the engine for its time, or by a signal); whether
/// the engine stopped it for its time; and what it wrote to its standard output and error, in the order it wrote it.
#[derive(Debug)]
pub(crate) struct Ran {
    pub(crate) code: Option<i32>,
    pub(crate) timed_out: bool,
    pub(crate) output: String,
}

/// How many seconds the process that holds a command's process group lives at most, which is far more than the longest
/// time limit a command is given and only matters if the engine ends without ending it.
const HOLD_SECS: &str = "86400";

/// Runs a command in `dir`, without a shell, in a process group of its own and with the engine's environment, to its end
/// or to `limit`, and gives what it left. Whatever the command started and left running, when it ended or ran past
/// `limit`, is stopped with it ([`stop`]). What it writes is read as it runs, beside the wait for its end, so that a
/// command that writes more than a pipe holds is never waiting for the reader while the reader waits for it. Fails when
/// the command cannot be started.
///
/// The group is held by a process of the engine's own (`sleep`) that stays until the group has been stopped, not by the
/// command: the command is reaped when it ends, and from then on a group it led would have no process of the engine's
/// to keep its id from being taken by another, while a signal to the group is still to be sent.
pub(crate) fn run_command(words: &[String], dir: &Path, limit: Duration) -> Result<Ran, String> {
    let (program, args) = words.split_first().ok_or("the command holds no word")?;
    let (mut reader, writer) = std::io::pipe().map_err(|x| format!("no pipe for the output: {x}"))?;
    let errors = writer.try_clone().map_err(|x| format!("no pipe for the output: {x}"))?;
    let mut holder = Command::new("sleep")
        .arg(HOLD_SECS)
        .process_group(0)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|x| format!("sleep, which holds the command's process group: {x}"))?;
    let group = i32::try_from(holder.id()).map_err(|x| format!("process group {}: {x}", holder.id()))?;
    let mut command = Command::new(program);
    command
        .args(args)
        .current_dir(dir)
        .process_group(group)
        .stdin(Stdio::null())
        .stdout(writer)
        .stderr(errors);
    let spawned = command.spawn();
    // The command value holds the writing ends of the pipe until it is dropped, and the reader ends only when none is
    // left open.
    drop(command);
    let mut child = match spawned {
        Ok(child) => child,
        Err(x) => {
            let _ = holder.kill();
            let _ = holder.wait();
            return Err(format!("{program}: {x}"));
        }
    };
    let kept = Arc::new(Mutex::new(Vec::<u8>::new()));
    let (ended, ends) = mpsc::channel();
    let reading = Arc::clone(&kept);
    std::thread::spawn(move || {
        let mut chunk = [0u8; 8192];
        while let Ok(n) = reader.read(&mut chunk) {
            if n == 0 {
                break;
            }
            reading
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .extend_from_slice(&chunk[..n]);
        }
        let _ = ended.send(());
    });
    let began = Instant::now();
    let (status, timed_out) = loop {
        match child.try_wait() {
            Ok(Some(status)) => break (Some(status), false),
            Ok(None) if began.elapsed() >= limit => break (None, true),
            Ok(None) => std::thread::sleep(POLL),
            Err(x) => {
                let _ = child.kill();
                stop(&mut holder, &ends);
                let _ = child.wait();
                return Err(format!("waiting for {program}: {x}"));
            }
        }
    };
    // The command ended, or ran out of its time: what it started and left running goes with it, whether it holds the
    // command's output or not.
    stop(&mut holder, &ends);
    if timed_out {
        let _ = child.wait();
    }
    let _ = ends.recv_timeout(DRAIN);
    let output = String::from_utf8_lossy(&kept.lock().unwrap_or_else(PoisonError::into_inner)).into_owned();
    Ok(Ran {
        code: status.and_then(|s| s.code()),
        timed_out,
        output,
    })
}

/// Stops everything in a command's process group, given the process that holds the group (`holder`): asks the group to
/// end, gives it [`STOP_WAIT`] at most (the pipe the command writes to closing is the sign that all of it ended, and
/// where nothing is left that is at once), and kills the group whatever came of that. Only then is the holder reaped, so
/// that the group's id is its for every signal ([`signal_group`] says why that matters).
fn stop(holder: &mut Child, ends: &Receiver<()>) {
    let group = holder.id();
    signal_group(group, "TERM");
    let _ = ends.recv_timeout(STOP_WAIT);
    signal_group(group, "KILL");
    // The holder itself, in case there is no `kill` to run.
    let _ = holder.kill();
    let _ = holder.wait();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh directory for a test, named by it, under the system's temporary directory.
    fn dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("wf-agents-command-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn sh(script: &str) -> Vec<String> {
        ["sh", "-c", script].iter().map(|s| s.to_string()).collect()
    }

    // A command that ends gives its exit code and what it wrote to its standard output and error in the order it wrote
    // it, and a command that writes more than a pipe holds ends all the same, since its output is read as it runs.
    #[test]
    fn a_command_that_ends_gives_its_exit_code_and_everything_it_wrote_in_order() {
        let d = dir("ends");
        let ran = run_command(
            &sh("echo first; echo second >&2; echo third; exit 3"),
            &d,
            Duration::from_secs(30),
        )
        .unwrap();
        assert_eq!((ran.code, ran.timed_out), (Some(3), false), "{ran:?}");
        assert_eq!(ran.output, "first\nsecond\nthird\n");
        let big = run_command(&sh("yes x | head -c 1000000"), &d, Duration::from_secs(30)).unwrap();
        assert_eq!((big.code, big.timed_out, big.output.len()), (Some(0), false, 1_000_000));
        let _ = std::fs::remove_dir_all(&d);
    }

    // What a command started and left running is stopped when the command ends, with 0 or not, whether the process holds
    // the command's output or has it closed away: a process that outlived the result would go on writing into the
    // directory the agents read. The result does not wait for it (the processes here would run a minute).
    #[test]
    fn a_process_a_command_leaves_running_is_stopped_when_the_command_ends() {
        let d = dir("left");
        let began = Instant::now();
        for (script, code) in [
            ("sleep 60 & echo $! > holding; echo done", 0),
            ("sleep 60 >/dev/null 2>&1 & echo $! > away; exit 4", 4),
        ] {
            let ran = run_command(&sh(script), &d, Duration::from_secs(30)).unwrap();
            assert_eq!((ran.code, ran.timed_out), (Some(code), false), "{script}: {ran:?}");
        }
        assert!(began.elapsed() < Duration::from_secs(20), "{:?}", began.elapsed());
        for file in ["holding", "away"] {
            let pid = std::fs::read_to_string(d.join(file)).unwrap().trim().to_string();
            let gone = (0..100).any(|_| {
                let alive = Command::new("kill")
                    .args(["-0", &pid])
                    .stderr(Stdio::null())
                    .status()
                    .is_ok_and(|s| s.success());
                if alive {
                    std::thread::sleep(Duration::from_millis(50));
                }
                !alive
            });
            assert!(gone, "process {pid} ({file}) outlived the command");
        }
        let _ = std::fs::remove_dir_all(&d);
    }

    // A command that cannot be started is an error, not a result of the command.
    #[test]
    fn a_command_that_cannot_be_started_is_an_error() {
        let d = dir("missing");
        let error = run_command(&["wf-agents-no-such-program".to_string()], &d, Duration::from_secs(30)).unwrap_err();
        assert!(error.starts_with("wf-agents-no-such-program: "), "{error}");
        assert!(run_command(&[], &d, Duration::from_secs(30)).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }

    // A command that runs past its limit is stopped with everything it started, what ignores the polite signal too: the
    // command here and the process it starts both ignore it. What it wrote before is kept, and it has no exit code.
    #[test]
    fn a_command_past_its_limit_is_stopped_with_everything_it_started() {
        let d = dir("limit");
        let script = "trap '' TERM; sleep 120 & echo $! > child; echo started; wait";
        let began = Instant::now();
        let ran = run_command(&sh(script), &d, Duration::from_secs(1)).unwrap();
        assert_eq!((ran.code, ran.timed_out), (None, true), "{ran:?}");
        assert_eq!(ran.output, "started\n");
        assert!(began.elapsed() < Duration::from_secs(30), "{:?}", began.elapsed());
        let pid = std::fs::read_to_string(d.join("child")).unwrap().trim().to_string();
        let alive = || {
            Command::new("kill")
                .args(["-0", &pid])
                .stderr(Stdio::null())
                .status()
                .is_ok_and(|s| s.success())
        };
        // What is killed is gone at once, but for its parent's taking it in.
        let gone = (0..100).any(|_| {
            if alive() {
                std::thread::sleep(Duration::from_millis(50));
                return false;
            }
            true
        });
        let _ = std::fs::remove_dir_all(&d);
        assert!(gone, "process {pid} outlived the stop");
    }
}
