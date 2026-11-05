//! The tool `lock_update` (cell `X1`): brings a lock file of the project in line with its manifest, which an agent changed
//! as it changes code, by the command the project's configuration names for that lock file ([`config::LockUpdate`]). The
//! agent chooses nothing of the command. The engine knows no language and no lock file format: it runs the command's
//! words in the directory that holds the lock file, bounds their time, and gives what they did as the tool's result.
//! Every request, refused or not, is a row of tool_runs.

use std::path::Path;
use std::time::Duration;

use serde_json::{Value, json};

use crate::Engine;
use crate::command::run_command;
use crate::hub::Failure;
use crate::tools::{attempts, inline_or_saved};
use crate::work::{RoundId, Work};

/// What the tool refuses with when the project's configuration lists no lock file. It holds while the commands a work's
/// message names are there and after they are gone: a lock file written by hand is what the tool is there to end.
const NONE_LISTED: &str = "This project's configuration lists no lock file for lock_update, so the tool does not run. \
    Do not write the lock file by hand: use the lock commands your work's message names, and where it names none, ask \
    the architect.";

/// What the tool gives, as its result names the verdict.
fn result(of: Value) -> Value {
    json!({ "lock_update": of })
}

/// A refusal: it is a result, not a failure, and is recorded as a request of its own.
fn refused(e: &mut Engine, round: RoundId, req: &Value, why: String) -> Result<Value, (Failure, String)> {
    attempts(e, round, "lock_update", req, |_| {
        Ok(result(json!({ "verdict": "refused", "why": why })))
    })
}

/// What a lock file holds as the worktree has it now, none when it is not there: a lock file a command writes for the
/// first time has changed too. A lock file that cannot be read says nothing of what a command did with it, so it fails.
fn holds(path: &Path) -> Result<Option<Vec<u8>>, String> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(x) if x.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(x) => Err(format!("reading {}: {x}", path.display())),
    }
}

/// Runs the request: finds the lock file among those the project lists, runs the command in the directory that holds it
/// in the work's worktree, and gives what became of it, with whether the command changed the lock file. It compares the
/// file's content and not git's status, since the agent may have changed the file already. A command that fails is a
/// result the agent reads; one that fails while the network is down is a failed tool, so that the cell waits for the
/// network and runs again; one that cannot be started is a failed tool too, a fault of the configuration.
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
        return refused(
            e,
            round,
            req,
            "The work has no worktree to update a lock file in".into(),
        );
    };
    let listed = &project.lock_updates;
    if listed.is_empty() {
        return refused(e, round, req, NONE_LISTED.into());
    }
    let Some(entry) = listed.iter().find(|u| u.locks(asked)) else {
        let names: Vec<&str> = listed.iter().map(|u| u.lock.as_str()).collect();
        let why = format!(
            "{asked:?} is not a lock file this project lists (lock_updates); they are: {}",
            names.join(", ")
        );
        return refused(e, round, req, why);
    };
    let lock = wt.join(entry.file());
    let dir = lock.parent().unwrap_or(&wt).to_path_buf();
    // A command cannot start in a directory that is not there, and the error of its start would name only the program.
    if !dir.is_dir() {
        let why = format!(
            "The worktree has no directory {} to hold {:?}: no command ran",
            dir.display(),
            entry.lock
        );
        return refused(e, round, req, why);
    }
    let words = config::words(&entry.command).map_err(|why| {
        (
            Failure::other(format!("the command of {:?} cannot be read: {why}", entry.lock)),
            String::new(),
        )
    })?;
    let limit = Duration::from_secs(entry.timeout_secs());
    attempts(e, round, "lock_update", req, |e| {
        let before = holds(&lock).map_err(Failure::other)?;
        let ran = run_command(&words, &dir, limit).map_err(Failure::other)?;
        let lock_changed = holds(&lock).map_err(Failure::other)? != before;
        if ran.code == Some(0) {
            return Ok(result(json!({
                "verdict": "updated",
                "lock_changed": lock_changed,
                "output": inline_or_saved(e, round, n, ran.output),
            })));
        }
        // A command that failed while the network is down says nothing of the manifest or of the command: the cell waits
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
            "lock_changed": lock_changed,
            "output": inline_or_saved(e, round, n, ran.output),
        });
        if let Some(code) = ran.code {
            failed["exit"] = json!(code);
        }
        Ok(result(failed))
    })
}
