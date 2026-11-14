//! The observations of the installation: what an agent came across beyond its work and wrote in the `observations` of
//! its return. Each text of the return a run was taken on is kept, and numbered, as the run's finish is saved, in the
//! same transaction, so that it is kept once whatever stops between; a return sent back keeps none, since the agent
//! says again in its next return what that one said (decision `observations-kept-with-the-finish`). What it says, its
//! cell and when it was kept are read from the work's records, which hold them already.
//!
//! The owner reads them on the page and marks each read. Nothing the engine decides reads them and their tables are
//! only added to, so they are read, and the owner's reading of one is kept, through connections of their own beside the
//! engine's turn, as the records of the copies are (decision `observations-read-beside-the-engines-turn`). A connection
//! is opened for each call, on a file the store has opened already, so the schema is at its version; it waits for a
//! lock another connection holds rather than failing.

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::path::{Path, PathBuf};
use std::time::Duration;

use decide::{Finished, RunId};
use rusqlite::{Connection, OptionalExtension, params};
use work::{Back, Time, Work};

use crate::{Fault, ItemNumber, SCHEMA_VERSION, WorkNumber};

/// How long a write waits for a lock another connection holds.
const WAIT: Duration = Duration::from_secs(10);

/// An observation, as the owner's page shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Observation {
    /// Its number, given as it was kept, counting from one.
    pub number: u64,
    /// What it says, as the agent wrote it.
    pub text: String,
    /// The work whose run's return carried it.
    pub work: WorkNumber,
    /// The work's item, and its title as it stands.
    pub item: ItemNumber,
    pub title: String,
    /// The cell the run was of.
    pub cell: String,
    pub run: RunId,
    /// The round of the run whose return carried it, the first being one.
    pub round: usize,
    /// When it was kept: as its run finished.
    pub kept: Time,
    /// When the owner read it; none while it is unread.
    pub read: Option<Time>,
}

/// Keeps, after what came back, the observations of the return each of these runs, which finish in this save, was
/// taken on: what came back at the end of its last round.
pub(crate) fn keep(db: &Connection, work: WorkNumber, given: &Work, finished: &[Finished]) -> Result<(), Fault> {
    for f in finished {
        let Some((round, Back::Returned { said, .. })) = given.last_back(f.run) else {
            continue;
        };
        for n in 1..=said.observations.len() {
            db.execute(
                "INSERT INTO observations (work, run, round, n) VALUES (?1, ?2, ?3, ?4)",
                params![work.0, f.run.0, round, n],
            )?;
        }
    }
    Ok(())
}

/// The observations, on connections of their own.
#[derive(Clone, Debug)]
pub struct Observations {
    path: PathBuf,
}

/// A row of the observations, before its text is read out of what came back.
struct Row {
    number: u64,
    work: u64,
    item: u64,
    cell: String,
    run: u64,
    round: usize,
    n: usize,
    back: String,
    kept: Option<u64>,
    read: Option<u64>,
}

impl Observations {
    /// Opens the store's file for the observations. The file is the store's: one that is not at this version of the
    /// schema is refused.
    pub fn open(path: impl AsRef<Path>) -> Result<Observations, Fault> {
        let observations = Observations {
            path: path.as_ref().to_path_buf(),
        };
        let db = observations.connect()?;
        let found: Option<u32> = db
            .query_row("SELECT version FROM schema_version", [], |row| row.get(0))
            .optional()?;
        if found != Some(SCHEMA_VERSION) {
            return Err(Fault::Schema {
                found,
                expected: SCHEMA_VERSION,
            });
        }
        Ok(observations)
    }

    /// A connection of its own to the file.
    fn connect(&self) -> Result<Connection, Fault> {
        let db = Connection::open(&self.path)?;
        db.busy_timeout(WAIT)?;
        db.pragma_update(None, "foreign_keys", "ON")?;
        Ok(db)
    }

    /// Every observation, the newest first: in the order they were kept, the last first.
    pub fn all(&self) -> Result<Vec<Observation>, Fault> {
        let db = self.connect()?;
        let titles: BTreeMap<ItemNumber, String> = (crate::items::items(&db)?.into_iter())
            .map(|item| (item.number, item.title))
            .collect();
        let mut query = db.prepare(
            "SELECT o.number, o.work, w.item, s.cell, o.run, o.round, o.n, b.back,
                 (SELECT m.time FROM moments m WHERE m.work = o.work AND m.place <= f.at
                  ORDER BY m.place DESC LIMIT 1),
                 r.at
             FROM observations o
             JOIN works w ON w.number = o.work
             JOIN started s ON s.work = o.work AND s.run = o.run
             JOIN finished f ON f.work = o.work AND f.run = o.run
             JOIN back b ON b.work = o.work AND b.run = o.run AND b.round = o.round
             LEFT JOIN observations_read r ON r.observation = o.number
             ORDER BY o.number DESC",
        )?;
        let rows = query
            .query_map([], |row| {
                Ok(Row {
                    number: row.get(0)?,
                    work: row.get(1)?,
                    item: row.get(2)?,
                    cell: row.get(3)?,
                    run: row.get(4)?,
                    round: row.get(5)?,
                    n: row.get(6)?,
                    back: row.get(7)?,
                    kept: row.get(8)?,
                    read: row.get(9)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        // What came back is read once, however many observations it carries.
        let mut backs: BTreeMap<(u64, u64, usize), Vec<String>> = BTreeMap::new();
        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            let texts = match backs.entry((row.work, row.run, row.round)) {
                Entry::Occupied(texts) => texts.into_mut(),
                Entry::Vacant(texts) => texts.insert(said(&row.back)?),
            };
            let text = (row.n.checked_sub(1).and_then(|i| texts.get(i)).cloned()).ok_or_else(|| Fault::Value {
                table: "observations",
                message: format!(
                    "O{} is the text {} of the observations of what came back of work {}, run {}, round {}, which \
                     has fewer",
                    row.number, row.n, row.work, row.run, row.round
                ),
            })?;
            let kept = row.kept.ok_or_else(|| Fault::Value {
                table: "observations",
                message: format!(
                    "O{}: work {} has no time for its run {}'s finish",
                    row.number, row.work, row.run
                ),
            })?;
            let item = ItemNumber(row.item);
            out.push(Observation {
                number: row.number,
                text,
                work: WorkNumber(row.work),
                item,
                title: titles.get(&item).cloned().unwrap_or_default(),
                cell: row.cell,
                run: RunId(row.run),
                round: row.round,
                kept: Time(kept),
                read: row.read.map(Time),
            });
        }
        Ok(out)
    }

    /// Keeps that the owner read the observation of this number at `at`, and gives when it was read: then, or when it
    /// was read before, which stays as it was; none where there is no observation of this number.
    pub fn read(&self, number: u64, at: Time) -> Result<Option<Time>, Fault> {
        let db = self.connect()?;
        db.execute(
            "INSERT OR IGNORE INTO observations_read (observation, at)
             SELECT number, ?2 FROM observations WHERE number = ?1",
            params![number, at.0],
        )?;
        let read: Option<u64> = db
            .query_row(
                "SELECT at FROM observations_read WHERE observation = ?1",
                [number],
                |row| row.get(0),
            )
            .optional()?;
        Ok(read.map(Time))
    }
}

/// The texts of the observations of what came back, as the store keeps it.
fn said(back: &str) -> Result<Vec<String>, Fault> {
    match serde_json::from_str::<Back>(back) {
        Ok(Back::Returned { said, .. }) => Ok(said.observations),
        Ok(_) => Ok(Vec::new()),
        Err(e) => Err(Fault::Value {
            table: "back",
            message: e.to_string(),
        }),
    }
}
