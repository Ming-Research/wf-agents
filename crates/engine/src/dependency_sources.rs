//! The tool `dependency_sources` (cell `X1`): puts the source of the dependencies a lock file of the project locks in a
//! directory of the work's worktree, for the agents of the work to read, by the command the project's configuration
//! names for that lock file ([`config::DependencySources`]). The engine knows no language and no lock file format: the
//! command is the project's, and the engine runs its words, bounds its time and gives what it did as the tool's result.
//! Every request, refused or not, is a row of tool_runs, which is how a failure to fetch is summed up.

use std::io::Read;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use sessions::cli::{STOP_WAIT, signal_group};

use crate::Engine;
use crate::hub::Failure;
use crate::tools::{attempts, inline_or_saved};
use crate::work::{RoundId, Work};

/// What the tool refuses with when the project's configuration lists no lock file.
const NONE_LISTED: &str = "This project's configuration lists no lock file for dependency_sources, so the tool does \
    not run. Do not wait for it: where your answer rests on what a dependency does, write that it is inferred, and how \
    it would be confirmed.";

/// How often a running command is looked at to see whether it has ended or run out of time.
const POLL: Duration = Duration::from_millis(20);

/// How long what a command wrote is waited for once the command has ended, in case a process it started still holds
/// the pipe it writes to; what has come by then is what the result gives.
const DRAIN: Duration = Duration::from_secs(2);

/// What the tool gives, as its result names the verdict.
fn sources(result: Value) -> Value {
    json!({ "sources": result })
}

/// A refusal: it is a result, not a failure, and is recorded as a request of its own.
fn refused(e: &mut Engine, round: RoundId, req: &Value, why: String) -> Result<Value, (Failure, String)> {
    attempts(e, round, "dependency_sources", req, |_| {
        Ok(sources(json!({ "verdict": "refused", "why": why })))
    })
}

/// Runs the request: finds the lock file among those the project lists, checks that git ignores the directory the
/// command fills, runs the command in the work's worktree and gives what became of it. A command that fails is a result
/// the agent reads; one that fails while the network is down is a failed tool, so that the cell waits for the network
/// and runs again; one that cannot be started is a failed tool too, a fault of the configuration.
pub(crate) fn run(
    e: &mut Engine,
    w: &Work,
    project: Option<&config::Project>,
    round: RoundId,
    n: usize,
    req: &Value,
) -> Result<Value, (Failure, String)> {
    let asked = req["lock"].as_str().unwrap_or_default().trim();
    let Some(project) = project else {
        return refused(e, round, req, "The work has no project".into());
    };
    let Some(wt) = w.worktree.clone() else {
        return refused(e, round, req, "The work has no worktree to put the source in".into());
    };
    let listed = &project.dependency_sources;
    if listed.is_empty() {
        return refused(e, round, req, NONE_LISTED.into());
    }
    let Some(entry) = listed.iter().find(|d| d.locks(asked)) else {
        let names: Vec<&str> = listed.iter().map(|d| d.lock.as_str()).collect();
        let why = format!(
            "{asked:?} is not a lock file this project lists (dependency_sources); they are: {}",
            names.join(", ")
        );
        return refused(e, round, req, why);
    };
    let into = entry.directory();
    match ignored(&wt, &into) {
        Ok(true) => {}
        Ok(false) => {
            let why = format!(
                "Git does not ignore {} in the worktree, where the command would write: no command ran. The project's \
                 .gitignore must ignore it, as a directory, so that what the command writes never shows as a change",
                into.display()
            );
            return refused(e, round, req, why);
        }
        Err(why) => {
            return attempts(e, round, "dependency_sources", req, |_| {
                Err(Failure::other(why.clone()))
            });
        }
    }
    let words = config::words(&entry.command).map_err(|why| {
        (
            Failure::other(format!("the command of {:?} cannot be read: {why}", entry.lock)),
            String::new(),
        )
    })?;
    let limit = Duration::from_secs(entry.timeout_secs());
    let path = wt.join(&into);
    attempts(e, round, "dependency_sources", req, |e| {
        let ran = run_command(&words, &wt, limit).map_err(Failure::other)?;
        if ran.code == Some(0) {
            return Ok(sources(
                json!({ "verdict": "fetched", "path": path.display().to_string() }),
            ));
        }
        // A fetch that failed while the network is down says nothing of the lock file or of the command: the cell waits
        // for the network, and runs the tool again when it is back.
        if e.network_down() {
            let last = ran.output.trim().lines().last().unwrap_or("no output");
            return Err(Failure::network(format!(
                "the command of {:?} failed while the network is down: {last}",
                entry.lock
            )));
        }
        let mut failed = json!({
            "verdict": "failed",
            "timed_out": ran.timed_out,
            "output": inline_or_saved(e, round, n, ran.output),
        });
        if let Some(code) = ran.code {
            failed["exit"] = json!(code);
        }
        Ok(sources(failed))
    })
}

/// Whether git ignores a directory of a worktree, taken with a trailing slash: a rule that names a directory (as
/// `/client/node_modules/` does) matches a path only as one, and a directory that is not there yet can only be told so.
fn ignored(wt: &Path, dir: &Path) -> Result<bool, String> {
    let mut at = dir.as_os_str().to_os_string();
    at.push("/");
    let out = Command::new("git")
        .arg("-C")
        .arg(wt)
        .args(["check-ignore", "-q", "--"])
        .arg(&at)
        .stdin(Stdio::null())
        .output()
        .map_err(|x| format!("git: {x}"))?;
    match out.status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err(format!(
            "git check-ignore {} failed: {}",
            dir.display(),
            String::from_utf8_lossy(&out.stderr).trim()
        )),
    }
}

/// What a command left: its exit code, none when it was stopped (by the engine for its time, or by a signal); whether
/// the engine stopped it for its time; and what it wrote to its standard output and error, in the order it wrote it.
#[derive(Debug)]
struct Ran {
    code: Option<i32>,
    timed_out: bool,
    output: String,
}

/// Runs a command in `dir`, without a shell, in a process group of its own and with the engine's environment, to its end
/// or to `limit`, and gives what it left. A command that runs past `limit` is stopped with everything it started
/// ([`stop`]). What it writes is read as it runs, beside the wait for its end, so that a command that writes more than
/// a pipe holds is never waiting for the reader while the reader waits for it. Fails when the command cannot be
/// started.
fn run_command(words: &[String], dir: &Path, limit: Duration) -> Result<Ran, String> {
    let (program, args) = words.split_first().ok_or("the command holds no word")?;
    let (mut reader, writer) = std::io::pipe().map_err(|x| format!("no pipe for the output: {x}"))?;
    let errors = writer.try_clone().map_err(|x| format!("no pipe for the output: {x}"))?;
    let mut command = Command::new(program);
    command
        .args(args)
        .current_dir(dir)
        .process_group(0)
        .stdin(Stdio::null())
        .stdout(writer)
        .stderr(errors);
    let spawned = command.spawn();
    // The command value holds the writing ends of the pipe until it is dropped, and the reader ends only when none is
    // left open.
    drop(command);
    let mut child = spawned.map_err(|x| format!("{program}: {x}"))?;
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
                stop(&mut child, &ends);
                return Err(format!("waiting for {program}: {x}"));
            }
        }
    };
    if timed_out {
        stop(&mut child, &ends);
    }
    let _ = ends.recv_timeout(DRAIN);
    let output = String::from_utf8_lossy(&kept.lock().unwrap_or_else(PoisonError::into_inner)).into_owned();
    Ok(Ran {
        code: status.and_then(|s| s.code()),
        timed_out,
        output,
    })
}

/// Stops a command that ran out of its time, and everything it started: asks its process group to end, gives it
/// [`STOP_WAIT`] at most (the pipe it writes to closing is the sign that all of it ended), and kills the group whatever
/// came of that. Only then is the child reaped, so that the group's id is the child's for every signal
/// ([`signal_group`] says why that matters); the child was not reaped before, since `try_wait` reaps a child only when
/// it has ended.
fn stop(child: &mut Child, ends: &Receiver<()>) {
    let group = child.id();
    signal_group(group, "TERM");
    let _ = ends.recv_timeout(STOP_WAIT);
    signal_group(group, "KILL");
    // The child itself, in case there is no `kill` to run.
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh directory for a test, named by it, under the system's temporary directory.
    fn dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("wf-agents-dependency-sources-{name}-{}", std::process::id()));
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

    // Git ignores a directory by a rule that names a directory only when the path is taken as one: a directory that is
    // not there yet is told so by its trailing slash, which the check adds. A directory no rule names is not ignored.
    #[test]
    fn a_directory_is_ignored_as_a_rule_for_a_directory_reads_it() {
        let d = dir("ignored");
        let git = |args: &[&str]| {
            let out = Command::new("git").arg("-C").arg(&d).args(args).output().unwrap();
            assert!(
                out.status.success(),
                "{args:?}: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        };
        git(&["init", "-q"]);
        std::fs::write(d.join(".gitignore"), "/target/\n/client/node_modules/\n").unwrap();
        for ignored_dir in ["target/dependency-sources", "client/node_modules"] {
            assert_eq!(ignored(&d, Path::new(ignored_dir)), Ok(true), "{ignored_dir}");
        }
        for kept in ["client/src", "client", "src/node_modules"] {
            assert_eq!(ignored(&d, Path::new(kept)), Ok(false), "{kept}");
        }
        let _ = std::fs::remove_dir_all(&d);
    }
}
