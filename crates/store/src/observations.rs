//! The observations of the installation: what an agent came across beyond its work and wrote in the `observations` of
//! its returns, and what an architect adds. Those a run keeps as it finishes (crate `work`, module `observations`) are
//! kept with the rest of the work, and numbered as they are, so that each is kept once whatever stops between. What it
//! says, its cell and when it was kept are read from the work's records, which hold them already. One an architect adds
//! is kept with its text, the project it is of and the project that reported it, and is numbered in the same order.
//!
//! An observation has a title and tags: those it was added with, or none for one a run kept, and each change made to it
//! since, kept with why, in the order they were made. What it stands as is worked out of them alone, as an item's is: a
//! change is a title, a tag come or gone, the item that took it in, or its being deleted, after which no change is
//! made. It is handled while an item whose work is not cancelled has taken it in.
//!
//! The owner reads them on the page and marks each read. Nothing the engine decides reads them and their tables are
//! only added to, so they are read, added to and changed, and the owner's reading of one is kept, through connections
//! of their own beside the engine's turn, as the records of the copies are. A connection
//! is opened for each call, on a file the store has opened already, so the schema is at its version; it waits for a
//! lock another connection holds rather than failing.

use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use decide::RunId;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use work::{Back, Time};

use crate::{Fault, ItemNumber, SCHEMA_VERSION, WorkNumber, tag_is_bad};

/// How long a write waits for a lock another connection holds.
const WAIT: Duration = Duration::from_secs(10);

/// An observation, as the owner's page shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Observation {
    /// Its number, given as it was kept, counting from one.
    pub number: u64,
    /// What it says, as the agent or the architect wrote it.
    pub text: String,
    /// Its title: the latest it was given, or the one it was added with; none while it has none.
    pub title: Option<String>,
    /// The tags it carries, in the order of their names: those it was added with, with each it came to carry since, but
    /// those it no longer carries.
    pub tags: Vec<String>,
    /// The project it is of, whose items may take it in; for one a run kept, the project of its work's item. None for
    /// one kept from an item that was kept before items had projects.
    pub project: Option<String>,
    /// The project that reported it; for one a run kept, the project of its work's item, as it is.
    pub reported_by: Option<String>,
    /// Where a run kept it; none for one an architect added.
    pub round: Option<FromRound>,
    /// When it was kept: as its run finished, or as it was added.
    pub kept: Time,
    /// When the owner read it; none while it is unread.
    pub read: Option<Time>,
    /// The item that took it in, the latest to have.
    pub taken_in_by: Option<ItemNumber>,
    /// Whether an item whose work is not cancelled has taken it in.
    pub handled: bool,
}

/// Where a run kept an observation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FromRound {
    /// The work whose run's return carried it.
    pub work: WorkNumber,
    /// The work's item, and its title as it stands.
    pub item: ItemNumber,
    pub item_title: String,
    /// The cell the run was of.
    pub cell: String,
    pub run: RunId,
    /// The round of the run whose return carried it, the first being one.
    pub round: usize,
}

/// An observation as an architect adds it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewObservation {
    pub text: String,
    pub title: Option<String>,
    pub tags: Vec<String>,
    /// The project it is of.
    pub project: String,
    /// The project that reported it.
    pub reported_by: String,
}

/// What a change of an observation does to it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObservationChange {
    /// It is given this title.
    Titled(String),
    /// It comes to carry this tag.
    Tagged(String),
    /// It no longer carries this tag.
    Untagged(String),
    /// This item takes it in.
    TakenIn(ItemNumber),
    /// It is deleted: it is listed no more, and no change is made to it after.
    Deleted,
}

/// Why a change of an observation is refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// There is no observation of this number.
    NoSuchObservation,
    /// The observation is deleted, and takes no change.
    Deleted,
    /// There is no item of this number to take it in.
    NoSuchItem(ItemNumber),
    /// The item's work is cancelled, and an observation it took in would be lost with it.
    ItemCancelled(ItemNumber),
    /// The item is of another project than the observation: only an item of the project an observation is of takes it
    /// in.
    ItemOfAnotherProject(ItemNumber),
    /// The tag is not one that may be carried: it is empty, longer than [`crate::TAG_CHARS`], or holds a space or a
    /// comma.
    BadTag(String),
    /// The title it was to be given has no words.
    BlankTitle,
}

/// A change refused: the place it was given at among those asked together, the first being 0, the observation it names,
/// and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotMade {
    pub at: usize,
    pub observation: u64,
    pub why: Refusal,
}

/// A tag, with how many observations carry it and how many of those are not handled.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TagCount {
    pub tag: String,
    pub carrying: usize,
    pub unhandled: usize,
}

/// What an observation stands as: what it was added or kept as, with each change made to it, in turn.
struct Standing {
    title: Option<String>,
    tags: BTreeSet<String>,
    taken_in_by: Option<ItemNumber>,
    deleted: bool,
}

impl Standing {
    /// Makes what the change does, keeping no change.
    fn made(&mut self, change: &ObservationChange) {
        match change {
            ObservationChange::Titled(title) => self.title = Some(title.clone()),
            ObservationChange::Tagged(tag) => {
                self.tags.insert(tag.clone());
            }
            ObservationChange::Untagged(tag) => {
                self.tags.remove(tag);
            }
            ObservationChange::TakenIn(item) => self.taken_in_by = Some(*item),
            ObservationChange::Deleted => self.deleted = true,
        }
    }

    /// Whether the change makes the observation other than it is.
    fn alters(&self, change: &ObservationChange) -> bool {
        match change {
            ObservationChange::Titled(title) => self.title.as_deref() != Some(title.as_str()),
            ObservationChange::Tagged(tag) => !self.tags.contains(tag),
            ObservationChange::Untagged(tag) => self.tags.contains(tag),
            ObservationChange::TakenIn(item) => self.taken_in_by != Some(*item),
            ObservationChange::Deleted => !self.deleted,
        }
    }
}

/// The observations, on connections of their own.
#[derive(Clone, Debug)]
pub struct Observations {
    path: PathBuf,
}

/// A row of the observations, before its text is read out of what came back.
struct Row {
    number: u64,
    work: Option<u64>,
    item: Option<u64>,
    cell: Option<String>,
    run: Option<u64>,
    round: Option<usize>,
    n: Option<usize>,
    back: Option<String>,
    kept: Option<u64>,
    read: Option<u64>,
    text: Option<String>,
    project: Option<String>,
    reported_by: Option<String>,
    title: Option<String>,
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

    /// Every observation not deleted, the newest first: in the order they were kept, the last first.
    pub fn all(&self) -> Result<Vec<Observation>, Fault> {
        let db = self.connect()?;
        let items: BTreeMap<ItemNumber, (String, Option<String>)> = (crate::items::items(&db)?.into_iter())
            .map(|item| (item.number, (item.title, item.project)))
            .collect();
        let cancelled = cancelled_items(&db)?;
        let mut tags: BTreeMap<u64, BTreeSet<String>> = BTreeMap::new();
        let mut query = db.prepare("SELECT observation, tag FROM observation_tagged")?;
        for row in query.query_map([], |row| Ok((row.get::<_, u64>(0)?, row.get::<_, String>(1)?)))? {
            let (observation, tag) = row?;
            tags.entry(observation).or_default().insert(tag);
        }
        let mut changes: BTreeMap<u64, Vec<ObservationChange>> = BTreeMap::new();
        let mut query = db.prepare("SELECT observation, change FROM observation_changes ORDER BY observation, n")?;
        for row in query.query_map([], |row| Ok((row.get::<_, u64>(0)?, row.get::<_, String>(1)?)))? {
            let (observation, change) = row?;
            changes.entry(observation).or_default().push(parse(&change)?);
        }
        let mut query = db.prepare(
            "SELECT o.number, o.work, w.item, s.cell, o.run, o.round, o.n, b.back,
                 CASE WHEN o.work IS NULL THEN o.added_at
                      ELSE (SELECT m.time FROM moments m WHERE m.work = o.work AND m.place <= f.at
                            ORDER BY m.place DESC LIMIT 1)
                 END,
                 r.at, o.text, o.project, o.reported_by, o.title
             FROM observations o
             LEFT JOIN works w ON w.number = o.work
             LEFT JOIN started s ON s.work = o.work AND s.run = o.run
             LEFT JOIN finished f ON f.work = o.work AND f.run = o.run
             LEFT JOIN back b ON b.work = o.work AND b.run = o.run AND b.round = o.round
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
                    text: row.get(10)?,
                    project: row.get(11)?,
                    reported_by: row.get(12)?,
                    title: row.get(13)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        // What came back is read once, however many observations it carries.
        let mut backs: BTreeMap<(u64, u64, usize), Vec<String>> = BTreeMap::new();
        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            let mut standing = Standing {
                title: row.title.clone(),
                tags: tags.remove(&row.number).unwrap_or_default(),
                taken_in_by: None,
                deleted: false,
            };
            for change in changes.remove(&row.number).unwrap_or_default() {
                standing.made(&change);
            }
            if standing.deleted {
                continue;
            }
            let (text, round, project, reported_by, kept) = match row.work {
                None => {
                    let text = row.text.clone().ok_or_else(|| Fault::Value {
                        table: "observations",
                        message: format!("O{}: it is no run's and has no text", row.number),
                    })?;
                    let kept = row.kept.ok_or_else(|| Fault::Value {
                        table: "observations",
                        message: format!("O{}: it is no run's and has no time it was added", row.number),
                    })?;
                    (text, None, row.project.clone(), row.reported_by.clone(), kept)
                }
                Some(work) => {
                    let (Some(item), Some(cell), Some(run), Some(round), Some(n), Some(back)) =
                        (row.item, row.cell.clone(), row.run, row.round, row.n, row.back.clone())
                    else {
                        return Err(Fault::Value {
                            table: "observations",
                            message: format!(
                                "O{}: {} holds no item, run or return it is of",
                                row.number,
                                WorkNumber(work)
                            ),
                        });
                    };
                    let texts = match backs.entry((work, run, round)) {
                        Entry::Occupied(texts) => texts.into_mut(),
                        Entry::Vacant(texts) => texts.insert(said(&back)?),
                    };
                    let text = (n.checked_sub(1).and_then(|i| texts.get(i)).cloned()).ok_or_else(|| Fault::Value {
                        table: "observations",
                        message: format!(
                            "O{} is the text {n} of the observations of what came back of {}, run {run}, round \
                             {round}, which has fewer",
                            row.number,
                            WorkNumber(work)
                        ),
                    })?;
                    let kept = row.kept.ok_or_else(|| Fault::Value {
                        table: "observations",
                        message: format!(
                            "O{}: {} has no time for its run {run}'s finish",
                            row.number,
                            WorkNumber(work)
                        ),
                    })?;
                    let item = ItemNumber(item);
                    let (item_title, project) = items.get(&item).cloned().unwrap_or_default();
                    let from = FromRound {
                        work: WorkNumber(work),
                        item,
                        item_title,
                        cell,
                        run: RunId(run),
                        round,
                    };
                    (text, Some(from), project.clone(), project, kept)
                }
            };
            let handled = standing.taken_in_by.is_some_and(|item| !cancelled.contains(&item.0));
            out.push(Observation {
                number: row.number,
                text,
                title: standing.title,
                tags: standing.tags.into_iter().collect(),
                project,
                reported_by,
                round,
                kept: Time(kept),
                read: row.read.map(Time),
                taken_in_by: standing.taken_in_by,
                handled,
            });
        }
        Ok(out)
    }

    /// Adds an observation, and gives its number: the next, after every one kept so far.
    pub fn add(&self, new: &NewObservation, at: Time) -> Result<u64, Fault> {
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "INSERT INTO observations (text, project, reported_by, added_at, title) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![new.text, new.project, new.reported_by, at.0, new.title],
        )?;
        let number = u64::try_from(tx.last_insert_rowid()).map_err(|e| Fault::Value {
            table: "observations",
            message: e.to_string(),
        })?;
        let tags: BTreeSet<&String> = new.tags.iter().collect();
        for tag in tags {
            tx.execute(
                "INSERT INTO observation_tagged (observation, tag) VALUES (?1, ?2)",
                params![number, tag],
            )?;
        }
        tx.commit()?;
        Ok(number)
    }

    /// Makes changes of observations, for this reason, all of them or none, and says whether each made its observation
    /// other than it was, in the order given; or every change that is refused, none then being made. The changes are
    /// judged in turn, each against the observations as the changes before it that were not refused would leave them,
    /// so that every refusal is said at once. A change that leaves an observation as it is, a title or a tag it has, or
    /// an item that took it in already, is no change: it is not refused, and not kept.
    pub fn change(
        &self,
        why: &str,
        changes: &[(u64, ObservationChange)],
        at: Time,
    ) -> Result<Result<Vec<bool>, Vec<NotMade>>, Fault> {
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut standing: BTreeMap<u64, Option<(Standing, Option<String>)>> = BTreeMap::new();
        let mut flags = Vec::new();
        let mut refused = Vec::new();
        let mut made: Vec<(u64, &ObservationChange)> = Vec::new();
        for (place, (number, change)) in changes.iter().enumerate() {
            let state = match standing.entry(*number) {
                Entry::Occupied(state) => state.into_mut(),
                Entry::Vacant(state) => state.insert(standing_of(&tx, *number)?),
            };
            let judged = match state.as_ref() {
                None => Err(Refusal::NoSuchObservation),
                Some((standing, project)) => judge(&tx, standing, project.as_deref(), change)?,
            };
            match judged {
                Ok(alters) => {
                    if alters && let Some((standing, _)) = state.as_mut() {
                        standing.made(change);
                        made.push((*number, change));
                    }
                    flags.push(alters);
                }
                Err(why) => refused.push(NotMade {
                    at: place,
                    observation: *number,
                    why,
                }),
            }
        }
        if !refused.is_empty() {
            return Ok(Err(refused));
        }
        for (number, change) in made {
            let what = serde_json::to_string(change).map_err(|e| Fault::Value {
                table: "observation_changes",
                message: e.to_string(),
            })?;
            let n: u64 = tx.query_row(
                "SELECT count(*) FROM observation_changes WHERE observation = ?1",
                [number],
                |row| row.get(0),
            )?;
            tx.execute(
                "INSERT INTO observation_changes (observation, n, at, change, why) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![number, n + 1, at.0, what, why],
            )?;
        }
        tx.commit()?;
        Ok(Ok(flags))
    }

    /// Each tag some observation not deleted carries, in the order of the names, with how many carry it and how many
    /// of those are not handled.
    pub fn tags(&self) -> Result<Vec<TagCount>, Fault> {
        let mut counts: BTreeMap<String, (usize, usize)> = BTreeMap::new();
        for observation in self.all()? {
            for tag in observation.tags {
                let count = counts.entry(tag).or_default();
                count.0 += 1;
                count.1 += usize::from(!observation.handled);
            }
        }
        Ok(counts
            .into_iter()
            .map(|(tag, (carrying, unhandled))| TagCount {
                tag,
                carrying,
                unhandled,
            })
            .collect())
    }

    /// Keeps that the owner read the observation of this number at `at`, and gives when it was read: then, or when it
    /// was read before, which stays as it was; none where there is no observation of this number, as for one past the
    /// numbers the store can give.
    pub fn read(&self, number: u64, at: Time) -> Result<Option<Time>, Fault> {
        if i64::try_from(number).is_err() {
            return Ok(None);
        }
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

/// What the observation of this number stands as, with the project it is of; none where there is no such observation. The
/// project of one a run kept is its work's item's, and is none for an item kept before items had projects.
fn standing_of(db: &Connection, number: u64) -> Result<Option<(Standing, Option<String>)>, Fault> {
    if i64::try_from(number).is_err() {
        return Ok(None);
    }
    let added: Option<(Option<String>, Option<String>)> = db
        .query_row(
            "SELECT o.title, COALESCE(o.project, i.project)
             FROM observations o
             LEFT JOIN works w ON w.number = o.work
             LEFT JOIN items i ON i.number = w.item
             WHERE o.number = ?1",
            [number],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let Some((title, project)) = added else {
        return Ok(None);
    };
    let mut standing = Standing {
        title,
        tags: BTreeSet::new(),
        taken_in_by: None,
        deleted: false,
    };
    let mut query = db.prepare("SELECT tag FROM observation_tagged WHERE observation = ?1")?;
    for tag in query.query_map([number], |row| row.get::<_, String>(0))? {
        standing.tags.insert(tag?);
    }
    let mut query = db.prepare("SELECT change FROM observation_changes WHERE observation = ?1 ORDER BY n")?;
    for change in query.query_map([number], |row| row.get::<_, String>(0))? {
        standing.made(&parse(&change?)?);
    }
    Ok(Some((standing, project)))
}

/// Whether the change makes the observation other than it stands, or why it is refused.
fn judge(
    db: &Connection,
    standing: &Standing,
    project: Option<&str>,
    change: &ObservationChange,
) -> Result<Result<bool, Refusal>, Fault> {
    if standing.deleted {
        return Ok(Err(Refusal::Deleted));
    }
    match change {
        ObservationChange::Titled(title) if title.trim().is_empty() => return Ok(Err(Refusal::BlankTitle)),
        ObservationChange::Tagged(tag) if tag_is_bad(tag) => return Ok(Err(Refusal::BadTag(tag.clone()))),
        ObservationChange::TakenIn(item) => {
            // A number the store cannot number is the number of no item.
            if i64::try_from(item.0).is_err() || !has(db, "SELECT 1 FROM items WHERE number = ?1", item.0)? {
                return Ok(Err(Refusal::NoSuchItem(*item)));
            }
            if cancelled_items(db)?.contains(&item.0) {
                return Ok(Err(Refusal::ItemCancelled(*item)));
            }
            let of: Option<String> = db.query_row("SELECT project FROM items WHERE number = ?1", [item.0], |row| {
                row.get(0)
            })?;
            if of.as_deref() != project {
                return Ok(Err(Refusal::ItemOfAnotherProject(*item)));
            }
        }
        _ => {}
    }
    Ok(Ok(standing.alters(change)))
}

/// Whether the query finds a row for the number.
fn has(db: &Connection, sql: &str, number: u64) -> Result<bool, Fault> {
    Ok(db.query_row(sql, [number], |_| Ok(())).optional()?.is_some())
}

/// The items whose work is cancelled: a work is cancelled while it has been cancelled more often than it was reopened.
fn cancelled_items(db: &Connection) -> Result<BTreeSet<u64>, Fault> {
    let mut query = db.prepare(
        "SELECT w.item FROM works w
         WHERE (SELECT count(*) FROM cancelled c WHERE c.work = w.number)
             > (SELECT count(*) FROM reopened r WHERE r.work = w.number)",
    )?;
    let items = query
        .query_map([], |row| row.get::<_, u64>(0))?
        .collect::<Result<BTreeSet<_>, _>>()?;
    Ok(items)
}

/// A change, read out of what the store keeps.
fn parse(change: &str) -> Result<ObservationChange, Fault> {
    serde_json::from_str(change).map_err(|e| Fault::Value {
        table: "observation_changes",
        message: e.to_string(),
    })
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
