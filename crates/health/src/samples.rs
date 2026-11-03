//! The states of the engine at a moment that no record holds, which the engine samples once a minute into `samples`:
//! the processor time its process used, the reads the owner's page made, the slots held, the works with a step running,
//! and how long the oldest matter of each project's architect's queue has waited.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use db::rusqlite::types::Value as Sql;
use db::rusqlite::{Connection, params};

use crate::{Fact, Reader};

/// A minute, in milliseconds: the step the engine samples at.
pub const MINUTE: i64 = 60_000;

/// The start of the minute a time is in.
pub fn minute(at: i64) -> i64 {
    at - at.rem_euclid(MINUTE)
}

/// The reads the owner's page made since they were last taken, and how long serving them took: the server adds each
/// read as it is served, and the engine takes them all at each sample, which starts the count again.
#[derive(Debug, Default)]
pub struct PageReads {
    reads: AtomicU64,
    micros: AtomicU64,
}

impl PageReads {
    /// One read served, which took `took`.
    pub fn add(&self, took: Duration) {
        self.reads.fetch_add(1, Ordering::Relaxed);
        let micros = u64::try_from(took.as_micros()).unwrap_or(u64::MAX);
        self.micros.fetch_add(micros, Ordering::Relaxed);
    }

    /// The reads served since the last time, and how many milliseconds serving them took, all together; the count
    /// starts again.
    pub fn take(&self) -> (u64, f64) {
        let reads = self.reads.swap(0, Ordering::Relaxed);
        let micros = self.micros.swap(0, Ordering::Relaxed);
        (reads, micros as f64 / 1000.0)
    }
}

/// The processor time this process has used since it started, in user mode and in the kernel, in milliseconds.
pub fn cpu_ms() -> Result<f64, String> {
    use nix::sys::resource::{UsageWho, getrusage};
    let u = getrusage(UsageWho::RUSAGE_SELF).map_err(|e| format!("the process's processor time: {e}"))?;
    let (user, system) = (u.user_time(), u.system_time());
    let ms = |sec: f64, usec: f64| sec * 1000.0 + usec / 1000.0;
    Ok(ms(user.tv_sec() as f64, user.tv_usec() as f64) + ms(system.tv_sec() as f64, system.tv_usec() as f64))
}

/// What the engine knows at a sample that no record holds.
#[derive(Debug, Clone, PartialEq)]
pub struct Beat {
    /// When it was taken; it is kept under the start of its minute.
    pub at: i64,
    /// The processor time the engine's process used since the sample before, as a share of the time since; none at
    /// the engine's first sample, which has nothing before it.
    pub cpu: Option<f64>,
    /// The reads the owner's page made since the sample before, and how many milliseconds serving them took.
    pub reads: u64,
    pub read_ms: f64,
    /// The works holding a slot, and those with a step running.
    pub slots_used: i64,
    pub works_running: i64,
}

/// Writes a sample, under the start of its minute, of what the engine knows (`beat`) and of how long the oldest
/// matter of each project's architect's queue has waited (none waits: 0), for the projects given and any other a
/// matter waits for. A minute already sampled keeps what it has.
pub fn record(conn: &Connection, beat: &Beat, projects: &[String]) -> Result<(), String> {
    let failed = |e: db::rusqlite::Error| format!("writing the minute's sample: {e}");
    let mut rows: Vec<(&str, String, f64)> = vec![
        ("page.reads", String::new(), beat.reads as f64),
        ("page.read_ms", String::new(), beat.read_ms),
        ("slots.used", String::new(), beat.slots_used as f64),
        ("works.running", String::new(), beat.works_running as f64),
    ];
    if let Some(cpu) = beat.cpu {
        rows.push(("engine.cpu", String::new(), cpu));
    }
    let mut oldest: BTreeMap<String, i64> = projects.iter().map(|p| (p.clone(), 0)).collect();
    let mut st = conn
        .prepare("SELECT project, min(at) FROM architect_queue GROUP BY project")
        .map_err(failed)?;
    let waiting = st
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
        .map_err(failed)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(failed)?;
    for (project, since) in waiting {
        oldest.insert(project, (beat.at - since).max(0));
    }
    rows.extend(oldest.into_iter().map(|(p, ms)| ("queue.oldest", p, ms as f64)));
    let at = minute(beat.at);
    for (measure, project, value) in rows {
        conn.execute(
            "INSERT OR IGNORE INTO samples (measure, project, at, value) VALUES (?1, ?2, ?3, ?4)",
            params![measure, project, at, value],
        )
        .map_err(failed)?;
    }
    Ok(())
}

/// The samples of a measure from `from` up to `to`, by their subjects: the project's given (`""` for the engine's own),
/// or every project's.
pub(crate) fn sampled(
    r: &Reader<'_>,
    measure: &str,
    project: Option<&str>,
    from: i64,
    to: i64,
) -> Result<Vec<Fact>, String> {
    let read = |row: &db::rusqlite::Row<'_>| -> db::rusqlite::Result<Fact> {
        Ok(Fact::event(row.get(0)?, row.get::<_, String>(1)?, row.get(2)?))
    };
    let (m, from, to) = (Sql::Text(measure.to_string()), Sql::Integer(from), Sql::Integer(to));
    match project {
        Some(p) => r.rows(
            "SELECT at, subject, value FROM samples WHERE measure = ?1 AND project = ?2 AND at >= ?3 AND at < ?4",
            vec![m, Sql::Text(p.to_string()), from, to],
            read,
        ),
        None => r.rows(
            "SELECT at, subject, value FROM samples WHERE measure = ?1 AND at >= ?2 AND at < ?3",
            vec![m, from, to],
            read,
        ),
    }
}
