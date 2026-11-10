//! Running a program as a child in a process group of its own, so that it and everything it starts can be stopped
//! together: told to stop, the group is sent TERM, and KILL once [`STOP_WAIT`] has passed without its ending.
//!
//! A program's group may be named in a file while it runs, so that one left running by a process of the engine that
//! ended is found and stopped before the same thing is run again ([`stop_left`]).

use std::path::Path;
use std::process::{ExitStatus, Stdio};
use std::time::Duration;

use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::held::Stop;

/// How long a process group told to stop with TERM is given to end before it is killed.
pub const STOP_WAIT: Duration = Duration::from_secs(3);

/// How much of the end of a command's output is kept to say what came of it, in bytes.
pub const TAIL: usize = 4000;

/// What came of running a program.
pub(crate) enum Ran {
    /// It ended by itself.
    Ended {
        status: ExitStatus,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
    },
    /// It was told to stop, and was stopped.
    Stopped,
}

/// Runs the program, with this on its standard input where something is given, until it ends or is told to stop;
/// or says why it could not be started. Where `group_file` is given, the program's process group is written there
/// while it runs, and the file taken away once it has ended.
pub(crate) async fn run(
    mut command: Command,
    stdin: Option<Vec<u8>>,
    group_file: Option<&Path>,
    stop: &mut Stop,
) -> Result<Ran, String> {
    command
        .process_group(0)
        .stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let program = command.as_std().get_program().to_string_lossy().into_owned();
    let child = command
        .spawn()
        .map_err(|e| format!("{program} cannot be started: {e}"))?;
    let group = child.id();
    if let (Some(file), Some(group)) = (group_file, group) {
        // A group not written down is only not stopped after a stop of the engine: no reason to stop this one.
        let _ = tokio::fs::write(file, format!("{group}\n")).await;
    }
    let ran = wait(child, stdin, group, &program, stop).await;
    if let Some(file) = group_file {
        let _ = tokio::fs::remove_file(file).await;
    }
    ran
}

/// Waits for the child to end, writing its input, or stops its group once told to.
async fn wait(
    mut child: tokio::process::Child,
    stdin: Option<Vec<u8>>,
    group: Option<u32>,
    program: &str,
    stop: &mut Stop,
) -> Result<Ran, String> {
    if let (Some(bytes), Some(mut pipe)) = (stdin, child.stdin.take()) {
        // Written beside the reading of the output, so that a child that writes before it has read all does not
        // wait on a pipe nobody empties. A child that ends without reading all of it closes the pipe: that is no
        // fault of the writing.
        tokio::spawn(async move {
            let _ = pipe.write_all(&bytes).await;
        });
    }
    let output = child.wait_with_output();
    tokio::pin!(output);
    tokio::select! {
        output = &mut output => {
            let output = output.map_err(|e| format!("{program} could not be waited for: {e}"))?;
            Ok(Ran::Ended {
                status: output.status,
                stdout: output.stdout,
                stderr: output.stderr,
            })
        }
        () = stop.told() => {
            if let Some(group) = group {
                signal(group, "TERM").await;
                if tokio::time::timeout(STOP_WAIT, &mut output).await.is_err() {
                    signal(group, "KILL").await;
                    let _ = output.await;
                }
            }
            Ok(Ran::Stopped)
        }
    }
}

/// Stops the process group the file names, where one was left running by a process of the engine that ended: TERM,
/// then KILL once [`STOP_WAIT`] has passed with any of it left; and takes the file away. The group is stopped only
/// where its first process still runs `program`, so that a number the system has since given to another program's
/// group is let be.
pub(crate) async fn stop_left(file: &Path, program: &str) {
    let Ok(text) = tokio::fs::read_to_string(file).await else {
        return;
    };
    if let Ok(group) = text.trim().parse::<u32>()
        && runs(group, program).await
    {
        signal(group, "TERM").await;
        let gone = async {
            while alive(group).await {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        };
        if tokio::time::timeout(STOP_WAIT, gone).await.is_err() {
            signal(group, "KILL").await;
        }
    }
    let _ = tokio::fs::remove_file(file).await;
}

/// Whether the process of this number runs the program: its command line names it.
async fn runs(process: u32, program: &str) -> bool {
    let shown = Command::new("ps")
        .args(["-o", "command=", "-p", &process.to_string()])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .await;
    shown.is_ok_and(|shown| shown.status.success() && String::from_utf8_lossy(&shown.stdout).contains(program))
}

/// Whether any process of the group is left.
async fn alive(group: u32) -> bool {
    let status = Command::new("kill")
        .args(["-0", "--", &format!("-{group}")])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await;
    status.is_ok_and(|status| status.success())
}

/// Sends the signal to the process group, with the system's `kill`. A group that has ended already is no fault.
async fn signal(group: u32, name: &str) {
    let _ = Command::new("kill")
        .args(["-s", name, "--", &format!("-{group}")])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await;
}

/// A shell command run in a directory, its standard error written where its standard output is, so that the two
/// come in the order they were written.
pub(crate) fn shell(command: &str, dir: &std::path::Path) -> Command {
    let mut shell = Command::new("sh");
    shell
        .arg("-c")
        .arg("exec 2>&1; eval \"$1\"")
        .arg("sh")
        .arg(command)
        .current_dir(dir);
    shell
}

/// The last [`TAIL`] bytes of an output, as text, beginning at a whole character.
pub(crate) fn tail(output: &[u8]) -> String {
    let mut from = output.len().saturating_sub(TAIL);
    while from < output.len() && (output[from] & 0b1100_0000) == 0b1000_0000 {
        from += 1;
    }
    String::from_utf8_lossy(&output[from..]).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tail_begins_at_a_whole_character() {
        let mut output = vec![b'a'; TAIL - 1];
        output.splice(0..0, "é".bytes());
        // The first byte of é is cut off, and the second is not taken for a character.
        assert_eq!(tail(&output), "a".repeat(TAIL - 1));
        assert_eq!(tail(b"short"), "short");
    }
}
