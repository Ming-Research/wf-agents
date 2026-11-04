//! The tool `dependency_sources` (cell `X1`): puts the source of the dependencies a lock file of the project locks in a
//! directory of the work's worktree, for the agents of the work to read, by the command the project's configuration
//! names for that lock file ([`config::DependencySources`]). The engine knows no language and no lock file format: the
//! command is the project's, and the engine runs its words ([`crate::command`]), bounds its time and gives what it did
//! as the tool's result. Every request, refused or not, is a row of tool_runs, which is how a failure to fetch is summed
//! up.

use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use serde_json::{Value, json};

use crate::Engine;
use crate::command::run_command;
use crate::hub::Failure;
use crate::tools::{attempts, inline_or_saved};
use crate::work::{RoundId, Work};

/// What the tool refuses with when the project's configuration lists no lock file.
const NONE_LISTED: &str = "This project's configuration lists no lock file for dependency_sources, so the tool does \
    not run. Do not wait for it: where your answer rests on what a dependency does, write that it is inferred, and how \
    it would be confirmed.";

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

#[cfg(test)]
mod tests {
    use super::*;

    // Git ignores a directory by a rule that names a directory only when the path is taken as one: a directory that is
    // not there yet is told so by its trailing slash, which the check adds. A directory no rule names is not ignored.
    #[test]
    fn a_directory_is_ignored_as_a_rule_for_a_directory_reads_it() {
        let d = std::env::temp_dir().join(format!("wf-agents-dependency-sources-ignored-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
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
