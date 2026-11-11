//! The records of the copies of the database: which were taken, which were uploaded off the machine, and what failed in
//! keeping them.
//!
//! They belong to no work, no decision of the engine reads them, and their tables are only inserted into like every
//! other, so they are kept by the backup's own task through a connection of its own, not through the loop that owns the
//! [`Store`](crate::Store) (decision `backup-records-are-written-beside-the-loop`). The connection is opened on a file
//! the store has opened already, so the schema is at its version, and it waits for a lock another connection holds
//! rather than failing.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use rusqlite::{Connection, OptionalExtension};
use work::Time;

use crate::{Fault, SCHEMA_VERSION};

/// How long a write waits for a lock another connection holds.
const WAIT: Duration = Duration::from_secs(10);

/// A copy that was taken.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Kept {
    /// The place in the order the copies were recorded in.
    pub seq: u64,
    /// The copy's name, which is also its file's.
    pub name: String,
    /// When it was taken, by the clock of the time.
    pub taken: Time,
}

/// A step of keeping a copy that failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure {
    pub at: Time,
    /// The copy it was about.
    pub name: String,
    /// Which step: `snapshot`, `private`, `upload` or `prune`.
    pub step: String,
    pub why: String,
}

/// Everything recorded of the copies.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Records {
    /// The copies taken, in the order they were recorded in.
    pub copies: Vec<Kept>,
    /// When each copy that was uploaded was, by its name.
    pub uploads: BTreeMap<String, Time>,
    /// The failures, oldest first.
    pub failures: Vec<Failure>,
    /// When the newest step ended with nothing failing, kept as it mended failures: a failure before it is mended.
    pub clean: Option<Time>,
}

/// The records of the copies, on a connection of their own.
pub struct Backups {
    db: Connection,
}

impl Backups {
    /// Opens the store's file for the records of the copies. The file is the store's: one that is not at this version
    /// of the schema is refused.
    pub fn open(path: impl AsRef<Path>) -> Result<Backups, Fault> {
        let db = Connection::open(path)?;
        db.busy_timeout(WAIT)?;
        db.pragma_update(None, "foreign_keys", "ON")?;
        let found: Option<u32> = db
            .query_row("SELECT version FROM schema_version", [], |row| row.get(0))
            .optional()?;
        if found != Some(SCHEMA_VERSION) {
            return Err(Fault::Schema {
                found,
                expected: SCHEMA_VERSION,
            });
        }
        Ok(Backups { db })
    }

    /// Records that the copy of this name was taken at `taken`; it takes the next place in the order.
    pub fn taken(&self, name: &str, taken: Time) -> Result<(), Fault> {
        self.db.execute(
            "INSERT INTO backups (name, taken) VALUES (?1, ?2)",
            rusqlite::params![name, taken.0],
        )?;
        Ok(())
    }

    /// Records that the copy of this name was uploaded at `at`; one recorded already stays as it was.
    pub fn uploaded(&self, name: &str, at: Time) -> Result<(), Fault> {
        self.db.execute(
            "INSERT OR IGNORE INTO backup_uploads (name, at) VALUES (?1, ?2)",
            rusqlite::params![name, at.0],
        )?;
        Ok(())
    }

    /// Records that this step failed at `at`, about the copy of this name, for this reason; the same failure at the
    /// same moment is recorded once.
    pub fn failed(&self, at: Time, name: &str, step: &str, why: &str) -> Result<(), Fault> {
        self.db.execute(
            "INSERT OR IGNORE INTO backup_failures (at, name, step, why) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![at.0, name, step, why],
        )?;
        Ok(())
    }

    /// Records that a step ended at `at` with nothing failing, which mends every failure before it.
    pub fn ended(&self, at: Time) -> Result<(), Fault> {
        self.db
            .execute("INSERT OR IGNORE INTO backup_steps (at) VALUES (?1)", [at.0])?;
        Ok(())
    }

    /// Everything recorded.
    pub fn records(&self) -> Result<Records, Fault> {
        let mut copies = self.db.prepare("SELECT seq, name, taken FROM backups ORDER BY seq")?;
        let copies = copies
            .query_map([], |row| {
                Ok(Kept {
                    seq: row.get(0)?,
                    name: row.get(1)?,
                    taken: Time(row.get(2)?),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let mut uploads = self.db.prepare("SELECT name, at FROM backup_uploads")?;
        let uploads = uploads
            .query_map([], |row| Ok((row.get::<_, String>(0)?, Time(row.get(1)?))))?
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        let mut failures = self
            .db
            .prepare("SELECT at, name, step, why FROM backup_failures ORDER BY at, name, step")?;
        let failures = failures
            .query_map([], |row| {
                Ok(Failure {
                    at: Time(row.get(0)?),
                    name: row.get(1)?,
                    step: row.get(2)?,
                    why: row.get(3)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let clean: Option<u64> = self
            .db
            .query_row("SELECT max(at) FROM backup_steps", [], |row| row.get(0))?;
        Ok(Records {
            copies,
            uploads,
            failures,
            clean: clean.map(Time),
        })
    }
}
