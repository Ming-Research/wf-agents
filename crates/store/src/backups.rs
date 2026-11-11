//! The records of the copies of the database: which were taken, which were uploaded off the machine, and what failed in
//! keeping them.
//!
//! They belong to no work, no decision of the engine reads them, and their tables are only inserted into like every
//! other, so they are kept by the backup itself through connections of its own, not through the loop that owns the
//! [`Store`](crate::Store) (decision `backup-records-are-written-beside-the-engines-turn`). A connection is opened for
//! each call, on a file the store has opened already, so the schema is at its version; it waits for a lock another
//! connection holds rather than failing. Held across an await, [`Backups`] is therefore `Sync`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
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
    /// The place in the order the failures were recorded in.
    pub seq: u64,
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
    /// The place of the newest failure that a step ending with nothing failing mended: the failures up to it are
    /// mended, by the order they were recorded in and not by time, which a clock that jumped makes wrong.
    pub mended: Option<u64>,
}

/// The records of the copies, on connections of their own.
#[derive(Clone, Debug)]
pub struct Backups {
    path: PathBuf,
}

impl Backups {
    /// Opens the store's file for the records of the copies. The file is the store's: one that is not at this version
    /// of the schema is refused.
    pub fn open(path: impl AsRef<Path>) -> Result<Backups, Fault> {
        let backups = Backups {
            path: path.as_ref().to_path_buf(),
        };
        let db = backups.connect()?;
        let found: Option<u32> = db
            .query_row("SELECT version FROM schema_version", [], |row| row.get(0))
            .optional()?;
        if found != Some(SCHEMA_VERSION) {
            return Err(Fault::Schema {
                found,
                expected: SCHEMA_VERSION,
            });
        }
        Ok(backups)
    }

    /// A connection of its own to the file.
    fn connect(&self) -> Result<Connection, Fault> {
        let db = Connection::open(&self.path)?;
        db.busy_timeout(WAIT)?;
        db.pragma_update(None, "foreign_keys", "ON")?;
        Ok(db)
    }

    /// Records that the copy of this name was taken at `taken`; it takes the next place in the order.
    pub fn taken(&self, name: &str, taken: Time) -> Result<(), Fault> {
        self.connect()?.execute(
            "INSERT INTO backups (name, taken) VALUES (?1, ?2)",
            rusqlite::params![name, taken.0],
        )?;
        Ok(())
    }

    /// Records that the copy of this name was uploaded at `at`; one recorded already stays as it was.
    pub fn uploaded(&self, name: &str, at: Time) -> Result<(), Fault> {
        let db = self.connect()?;
        let there: Option<i64> = db
            .query_row("SELECT 1 FROM backup_uploads WHERE name = ?1", [name], |row| row.get(0))
            .optional()?;
        if there.is_none() {
            db.execute(
                "INSERT INTO backup_uploads (name, at) VALUES (?1, ?2)",
                rusqlite::params![name, at.0],
            )?;
        }
        Ok(())
    }

    /// Records that this step failed at `at`, about the copy of this name, for this reason. Every failure takes the next
    /// place in the order, whatever its time: one with the same time, copy and step as an earlier one is a new failure,
    /// which a step that mended the earlier one has not mended.
    pub fn failed(&self, at: Time, name: &str, step: &str, why: &str) -> Result<(), Fault> {
        self.connect()?.execute(
            "INSERT INTO backup_failures (at, name, step, why) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![at.0, name, step, why],
        )?;
        Ok(())
    }

    /// Records that a step ended at `at` with nothing failing, which mends the failures up to the one at the place
    /// `mended`, the newest recorded when it ended; one that mended them already stays as it was.
    pub fn ended(&self, at: Time, mended: u64) -> Result<(), Fault> {
        let db = self.connect()?;
        let there: Option<i64> = db
            .query_row("SELECT 1 FROM backup_steps WHERE mended = ?1", [mended], |row| {
                row.get(0)
            })
            .optional()?;
        if there.is_none() {
            db.execute(
                "INSERT INTO backup_steps (mended, at) VALUES (?1, ?2)",
                rusqlite::params![mended, at.0],
            )?;
        }
        Ok(())
    }

    /// Everything recorded.
    pub fn records(&self) -> Result<Records, Fault> {
        let db = self.connect()?;
        let mut copies = db.prepare("SELECT seq, name, taken FROM backups ORDER BY seq")?;
        let copies = copies
            .query_map([], |row| {
                Ok(Kept {
                    seq: row.get(0)?,
                    name: row.get(1)?,
                    taken: Time(row.get(2)?),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let mut uploads = db.prepare("SELECT name, at FROM backup_uploads")?;
        let uploads = uploads
            .query_map([], |row| Ok((row.get::<_, String>(0)?, Time(row.get(1)?))))?
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        let mut failures = db.prepare("SELECT seq, at, name, step, why FROM backup_failures ORDER BY seq")?;
        let failures = failures
            .query_map([], |row| {
                Ok(Failure {
                    seq: row.get(0)?,
                    at: Time(row.get(1)?),
                    name: row.get(2)?,
                    step: row.get(3)?,
                    why: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let mended: Option<u64> = db.query_row("SELECT max(mended) FROM backup_steps", [], |row| row.get(0))?;
        Ok(Records {
            copies,
            uploads,
            failures,
            mended,
        })
    }
}
