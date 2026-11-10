//! Running a program as a child in a process group of its own, so that it and everything it starts can be stopped
//! together: told to stop, the group is sent TERM, and KILL once [`STOP_WAIT`] has passed without its ending.
//!
//! A program's group may be named in a file while it runs, with the start time of its first process, so that one left
//! running by a process of the engine that ended is found and stopped before the same thing is run again
//! ([`stop_left`]), and a number the system has since given to another group is told apart from it.

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
        let started = started(group).await.unwrap_or_default();
        let _ = tokio::fs::write(file, format!("{group}\n{started}\n")).await;
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
/// then KILL once [`STOP_WAIT`] has passed with any of it left; and takes the file away. The group is ours, and
/// signalled, only where its first process is alive and started when the file says, or where it is gone and a process
/// left in the group works in `dir`, the round's worktree: what a program started, a build or its tests, outlives it in
/// its group. Otherwise the number now names another group, after a restart of the machine or numbers that came
/// round again, and nothing is signalled; nor is the engine's own group ever.
pub(crate) async fn stop_left(file: &Path, dir: &Path) {
    let Ok(text) = tokio::fs::read_to_string(file).await else {
        return;
    };
    let mut lines = text.lines();
    let group = lines.next().and_then(|line| line.trim().parse::<u32>().ok());
    let written = lines.next().unwrap_or_default().trim().to_string();
    if let Some(group) = group
        && ours(group, &written, dir).await
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

/// Whether the group is the one a round of ours started: see [`stop_left`].
async fn ours(group: u32, written: &str, dir: &Path) -> bool {
    if group == 0 || Some(group) == group_of(std::process::id()).await {
        return false;
    }
    if alive_process(group).await {
        return !written.is_empty() && started(group).await.as_deref() == Some(written);
    }
    let dir = std::fs::canonicalize(dir).ok();
    for member in members(group).await {
        if let (Some(cwd), Some(dir)) = (cwd(member).await, &dir)
            && std::fs::canonicalize(&cwd).ok().as_ref() == Some(dir)
        {
            return true;
        }
    }
    false
}

/// What `ps` prints of a process, or none where it has no such process.
async fn ps(args: &[&str]) -> Option<String> {
    let shown = Command::new("ps")
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .await
        .ok()?;
    let text = String::from_utf8_lossy(&shown.stdout).trim().to_string();
    (shown.status.success() && !text.is_empty()).then_some(text)
}

/// When the process started, as `ps -o lstart=` prints it.
async fn started(process: u32) -> Option<String> {
    let started = ps(&["-o", "lstart=", "-p", &process.to_string()]).await?;
    Some(started.split_whitespace().collect::<Vec<_>>().join(" "))
}

/// The process group of the process.
async fn group_of(process: u32) -> Option<u32> {
    ps(&["-o", "pgid=", "-p", &process.to_string()]).await?.parse().ok()
}

/// The processes of the group.
async fn members(group: u32) -> Vec<u32> {
    let listed = ps(&["-A", "-o", "pid=,pgid="]).await.unwrap_or_default();
    (listed.lines())
        .filter_map(|line| {
            let mut numbers = line.split_whitespace().map(|n| n.parse::<u32>().ok());
            Some((numbers.next()??, numbers.next()??))
        })
        .filter(|(_, g)| *g == group)
        .map(|(pid, _)| pid)
        .collect()
}

/// The directory the process works in: from `/proc` where the system has it, from `lsof` where it does not; none
/// where neither can be read.
async fn cwd(process: u32) -> Option<std::path::PathBuf> {
    if let Ok(dir) = tokio::fs::read_link(format!("/proc/{process}/cwd")).await {
        return Some(dir);
    }
    let listed = Command::new("lsof")
        .args(["-a", "-d", "cwd", "-p", &process.to_string(), "-Fn"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .await
        .ok()?;
    let text = String::from_utf8_lossy(&listed.stdout);
    text.lines()
        .find_map(|line| line.strip_prefix('n'))
        .map(std::path::PathBuf::from)
}

/// Whether the process is alive.
async fn alive_process(process: u32) -> bool {
    let status = Command::new("kill")
        .args(["-0", &process.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await;
    status.is_ok_and(|status| status.success())
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

    #[tokio::test]
    async fn a_left_group_is_stopped_though_its_first_process_has_ended() {
        let dir = tempfile::tempdir().unwrap();
        // The first process starts one that outlives it in its group, and ends.
        let mut first = std::process::Command::new("sh");
        first.current_dir(dir.path());
        std::os::unix::process::CommandExt::process_group(&mut first, 0);
        let output = (first.args(["-c", "sleep 30 >/dev/null 2>&1 & echo $!"]))
            .output()
            .expect("it runs");
        let child: u32 = String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse()
            .expect("the child's number");
        let leader = group_of(child);
        let file = dir.path().join("left.pgid");
        std::fs::write(&file, format!("{leader}\nMon Jan  1 00:00:00 2001\n")).unwrap();
        assert!(alive(leader).await, "the child is alive in the group");
        stop_left(&file, dir.path()).await;
        assert!(!file.exists());
        let began = std::time::Instant::now();
        while alive(leader).await {
            assert!(began.elapsed() < Duration::from_secs(10), "the group was not stopped");
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    #[tokio::test]
    async fn a_file_naming_a_group_that_is_not_ours_is_taken_away_and_nothing_signalled() {
        let dir = tempfile::tempdir().unwrap();
        let mut other = std::process::Command::new("sleep");
        other.arg("30");
        std::os::unix::process::CommandExt::process_group(&mut other, 0);
        let mut other = other.spawn().expect("it runs");
        let group = other.id();
        let file = dir.path().join("stale.pgid");
        // Its number, with a start time it does not have: the number came round to another group.
        std::fs::write(&file, format!("{group}\nMon Jan  1 00:00:00 2001\n")).unwrap();
        stop_left(&file, dir.path()).await;
        assert!(!file.exists(), "the stale file is taken away");
        assert!(alive(group).await, "the other group was let be");
        // Nor is the engine's own group signalled, whatever the file says.
        let own = group_of(std::process::id());
        let started = started(std::process::id()).await.unwrap_or_default();
        std::fs::write(&file, format!("{own}\n{started}\n")).unwrap();
        stop_left(&file, dir.path()).await;
        assert!(alive(own).await);
        other.kill().unwrap();
        other.wait().unwrap();
    }

    /// The process group of the process, as `ps` gives it.
    fn group_of(process: u32) -> u32 {
        let shown = std::process::Command::new("ps")
            .args(["-o", "pgid=", "-p", &process.to_string()])
            .output()
            .expect("ps runs");
        String::from_utf8_lossy(&shown.stdout).trim().parse().expect("a group")
    }

    #[test]
    fn the_tail_begins_at_a_whole_character() {
        let mut output = vec![b'a'; TAIL - 1];
        output.splice(0..0, "é".bytes());
        // The first byte of é is cut off, and the second is not taken for a character.
        assert_eq!(tail(&output), "a".repeat(TAIL - 1));
        assert_eq!(tail(b"short"), "short");
    }
}
