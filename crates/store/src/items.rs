//! The items, the things to be done, and the works done for them.

use std::collections::{BTreeMap, BTreeSet};

use rusqlite::{Connection, OptionalExtension, params};
use work::Time;

use crate::Fault;

/// The number of an item, given by the store, counting from one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ItemNumber(pub u64);

/// The number of a work, given by the store, counting from one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WorkNumber(pub u64);

/// A thing to be done.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub number: ItemNumber,
    pub title: String,
    /// The name of its level among the installation's levels: the latest the owner put it at, or the one it was added
    /// at; none for an item kept before items had levels and never put at one.
    pub level: Option<String>,
    /// When it was added.
    pub added: Time,
    /// The items it depends on, each added before it, in the order of their numbers.
    pub depends_on: Vec<ItemNumber>,
}

/// What a work is, apart from what is kept of it: whose it is and on what flow.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkHead {
    pub number: WorkNumber,
    /// The item it is done for.
    pub item: ItemNumber,
    /// The name of the flow it runs on. Which version of the flow it ran on is not kept yet.
    pub flow: String,
    /// When it was added.
    pub added: Time,
}

pub(crate) fn add_item(
    db: &Connection,
    title: &str,
    level: &str,
    depends_on: &[ItemNumber],
    at: Time,
) -> Result<ItemNumber, Fault> {
    // Every item the store has was added before the new one, so an item it has is one the new one may depend on.
    let depends_on: BTreeSet<ItemNumber> = depends_on.iter().copied().collect();
    for &on in &depends_on {
        if !exists(db, "SELECT 1 FROM items WHERE number = ?1", on.0)? {
            return Err(Fault::NoSuchDependency(on));
        }
    }
    db.execute(
        "INSERT INTO items (title, added, level) VALUES (?1, ?2, ?3)",
        params![title, at.0, level],
    )?;
    let number = ItemNumber(db.last_insert_rowid().cast_unsigned());
    for on in depends_on {
        db.execute(
            "INSERT INTO depends (item, on_item) VALUES (?1, ?2)",
            params![number.0, on.0],
        )?;
    }
    Ok(number)
}

/// Keeps a change of the item's level, after its earlier changes.
pub(crate) fn set_level(db: &Connection, item: ItemNumber, level: &str, at: Time) -> Result<(), Fault> {
    if !exists(db, "SELECT 1 FROM items WHERE number = ?1", item.0)? {
        return Err(Fault::NoSuchItem(item));
    }
    let n: u64 = db.query_row("SELECT count(*) FROM level_changes WHERE item = ?1", [item.0], |row| {
        row.get(0)
    })?;
    let _ = (n, level, at);
    Ok(())
}

pub(crate) fn items(db: &Connection) -> Result<Vec<Item>, Fault> {
    let mut changed: BTreeMap<u64, String> = BTreeMap::new();
    let mut query = db.prepare("SELECT item, level FROM level_changes ORDER BY item, n")?;
    for row in query.query_map([], |row| Ok((row.get::<_, u64>(0)?, row.get::<_, String>(1)?)))? {
        let (item, level) = row?;
        changed.insert(item, level);
    }
    let mut depends: BTreeMap<u64, Vec<ItemNumber>> = BTreeMap::new();
    let mut query = db.prepare("SELECT item, on_item FROM depends ORDER BY item, on_item")?;
    for row in query.query_map([], |row| Ok((row.get::<_, u64>(0)?, row.get::<_, u64>(1)?)))? {
        let (item, on) = row?;
        depends.entry(item).or_default().push(ItemNumber(on));
    }
    let mut query = db.prepare("SELECT number, title, added, level FROM items ORDER BY number")?;
    let rows = query.query_map([], |row| {
        Ok((
            row.get::<_, u64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, u64>(2)?,
            row.get::<_, Option<String>>(3)?,
        ))
    })?;
    let mut items = Vec::new();
    for row in rows {
        let (number, title, added, level) = row?;
        items.push(Item {
            number: ItemNumber(number),
            title,
            level: changed.remove(&number).or(level),
            added: Time(added),
            depends_on: depends.remove(&number).unwrap_or_default(),
        });
    }
    Ok(items)
}

/// Adds the work of an item that was added in the same transaction.
pub(crate) fn add_work(db: &Connection, item: ItemNumber, flow: &str, at: Time) -> Result<WorkNumber, Fault> {
    db.execute(
        "INSERT INTO works (item, flow, added) VALUES (?1, ?2, ?3)",
        params![item.0, flow, at.0],
    )?;
    Ok(WorkNumber(db.last_insert_rowid().cast_unsigned()))
}

pub(crate) fn works(db: &Connection) -> Result<Vec<WorkHead>, Fault> {
    let mut query = db.prepare("SELECT number, item, flow, added FROM works ORDER BY number")?;
    let rows = query.query_map([], |row| {
        Ok(WorkHead {
            number: WorkNumber(row.get(0)?),
            item: ItemNumber(row.get(1)?),
            flow: row.get(2)?,
            added: Time(row.get(3)?),
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Refuses a work the store does not have.
pub(crate) fn known(db: &Connection, work: WorkNumber) -> Result<(), Fault> {
    if exists(db, "SELECT 1 FROM works WHERE number = ?1", work.0)? {
        Ok(())
    } else {
        Err(Fault::NoSuchWork(work))
    }
}

/// Whether the query finds a row for the number.
fn exists(db: &Connection, sql: &str, number: u64) -> Result<bool, Fault> {
    Ok(db.query_row(sql, [number], |_| Ok(())).optional()?.is_some())
}
