//! Running a program as a child in a process group of its own, so that it and everything it starts can be stopped
//! together: told to stop, the group is sent TERM, and KILL once [`STOP_WAIT`] has passed without its ending.

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
/// or says why it could not be started.
pub(crate) async fn run(mut command: Command, stdin: Option<Vec<u8>>, stop: &mut Stop) -> Result<Ran, String> {
    command
        .process_group(0)
        .stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let program = command.as_std().get_program().to_string_lossy().into_owned();
    let mut child = command
        .spawn()
        .map_err(|e| format!("{program} cannot be started: {e}"))?;
    let group = child.id();
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
