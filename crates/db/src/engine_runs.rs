//! The runs of the engine's loop: one row of `engine_runs` for each, from its start to its stop.

use rusqlite::{Transaction, params};

use crate::Result;

/// What a row's `stop_reason` says of a run that was found never completed: its process ended without recording a stop.
pub const LOST: &str = "lost";

/// Records the start of a run at `at`: the runs found never completed are marked lost first, since the engine runs one
/// loop on a database at a time, and the new run's row is written. Gives its id.
pub fn start_engine_run(
    tx: &Transaction<'_>,
    at: i64,
    built_from: &str,
    config_commit: &str,
    flow_version: i64,
) -> Result<i64> {
    tx.execute(
        "UPDATE engine_runs SET stop_reason = ?1 WHERE stopped_at IS NULL AND stop_reason IS NULL",
        [LOST],
    )?;
    tx.execute(
        "INSERT INTO engine_runs (started_at, built_from, config_commit, flow_version) VALUES (?1, ?2, ?3, ?4)",
        params![at, built_from, config_commit, flow_version],
    )?;
    Ok(tx.last_insert_rowid())
}

/// Records the stop of run `run` at `at`, and why.
pub fn stop_engine_run(tx: &Transaction<'_>, run: i64, at: i64, reason: &str) -> Result<()> {
    tx.execute(
        "UPDATE engine_runs SET stopped_at = ?2, stop_reason = ?3 WHERE id = ?1 AND stopped_at IS NULL",
        params![run, at, reason],
    )?;
    Ok(())
}
