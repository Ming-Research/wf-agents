//! The checks the tool `ci` runs on a head before it pushes it: the commands the project's configuration lists
//! (`ci.push_checks`, [`config::PushCheck`]), run in the work's worktree, so that what a command finds in seconds does
//! not cost a run of the CI. The engine knows no language: the command is the project's, and the engine runs its words
//! ([`crate::command`]), bounds its time and gives what became of each. Every run, passed or not, is a row of tool_runs
//! under the tool name `push_check`, which is how a check that blocks a push that the CI would have passed is found.

use std::path::Path;
use std::time::Duration;

use serde_json::{Value, json};

use crate::Engine;
use crate::command::run_command;
use crate::hub::Failure;
use crate::tools::{attempts, inline_or_saved};
use crate::work::RoundId;

/// Runs every check, in the order listed, in the worktree `wt` on the head `sha` (the worktree is clean and at that
/// head: `ci` checked), and gives what became of each, one value each: its name, its command and its verdict, `passed`
/// or `failed`; a failed one also the exit code (none when the engine stopped it for its time), whether it was stopped
/// for its time, and what it wrote, saved to a file of its own when long. All run, whatever an earlier one found, since
/// each send-back of the request costs a round. A command that fails is a result the engineer reads; one that cannot be
/// started is a failed tool, a fault of the configuration, and nothing more runs. `n` is the request's place among the
/// round's, which names the files of the output.
pub(crate) fn run_all(
    e: &mut Engine,
    round: RoundId,
    n: usize,
    wt: &Path,
    sha: &str,
    checks: &[config::PushCheck],
) -> Result<Vec<Value>, (Failure, String)> {
    let mut results = Vec::with_capacity(checks.len());
    for (i, check) in checks.iter().enumerate() {
        let words = config::words(&check.command).map_err(|why| {
            (
                Failure::other(format!(
                    "the command of the push check {:?} cannot be read: {why}",
                    check.name
                )),
                String::new(),
            )
        })?;
        let limit = Duration::from_secs(check.timeout_secs());
        let args = json!({ "head": sha, "check": check.name, "command": check.command });
        let result = attempts(e, round, "push_check", &args, |e| {
            let ran = run_command(&words, wt, limit).map_err(Failure::other)?;
            let passed = ran.code == Some(0);
            let verdict = if passed { "passed" } else { "failed" };
            let mut result = json!({ "name": check.name, "command": check.command, "verdict": verdict });
            if !passed {
                result["timed_out"] = json!(ran.timed_out);
                if let Some(code) = ran.code {
                    result["exit"] = json!(code);
                }
                result["output"] = json!(inline_or_saved(e, round, format!("{n}-check-{i}"), ran.output));
            }
            Ok(result)
        })?;
        results.push(result);
    }
    Ok(results)
}

/// Whether every check passed.
pub(crate) fn all_passed(results: &[Value]) -> bool {
    results.iter().all(|r| r["verdict"] == "passed")
}
