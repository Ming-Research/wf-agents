//! The items, the things to be done, and the works done for them.

use std::collections::{BTreeMap, BTreeSet};

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use work::Time;

use crate::Fault;

/// The number of an item, given by the store, counting from one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ItemNumber(pub u64);

/// The number of a work, given by the store, counting from one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WorkNumber(pub u64);

/// A thing to be done, as it stands: what it was added as, with each change made to it since, in turn. What it stands
/// as is worked out of those by [`Item::new`] and [`Item::changed`] alone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub number: ItemNumber,
    /// Its title: the latest it was given, or the one it was added with.
    pub title: String,
    /// The name of its level among the installation's levels: the latest it was put at, or the one it was added at;
    /// none for an item kept before items had levels and never put at one.
    pub level: Option<String>,
    /// The name of the project it belongs to, as the installation's settings name it, which it keeps; none for an item
    /// kept before items had projects.
    pub project: Option<String>,
    /// When it was added.
    pub added: Time,
    /// The items it depends on, in the order of their numbers: those it was added depending on, with each it came to
    /// depend on since, but those it no longer depends on.
    pub depends_on: Vec<ItemNumber>,
    /// The tags it carries, in the order of their names: those it was added with, with each it came to carry since,
    /// but those it no longer carries.
    pub tags: Vec<String>,
    /// What it was as it was added.
    pub first: First,
    /// Each change made to it since it was added, in the order they were made.
    pub changes: Vec<Change>,
}

/// An item as it was added.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct First {
    pub title: String,
    /// None for an item kept before items had levels.
    pub level: Option<String>,
    /// None for an item kept before items had projects.
    pub project: Option<String>,
    /// Each added before it, in the order of their numbers.
    pub depends_on: Vec<ItemNumber>,
    /// In the order of their names; none for an item kept before items carried tags.
    pub tags: Vec<String>,
}

/// A change made to an item, at this time, and why, as whoever made it said; none for a change kept before changes
/// said why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub at: Time,
    pub what: ItemChange,
    pub why: Option<String>,
}

/// What a change of an item does to it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemChange {
    /// It is given this title.
    Title(String),
    /// It is put at this level, by the name of a level of the installation.
    Level(String),
    /// It comes to depend on this item.
    DependsOn(ItemNumber),
    /// It no longer depends on this item.
    NoLongerDependsOn(ItemNumber),
    /// It comes to carry this tag.
    Tagged(String),
    /// It no longer carries this tag.
    Untagged(String),
}

impl Item {
    /// The item as it stands once these changes are made, in turn, to what it was added as.
    pub fn new(number: ItemNumber, added: Time, first: First, changes: Vec<Change>) -> Item {
        let mut item = Item {
            number,
            title: first.title.clone(),
            level: first.level.clone(),
            project: first.project.clone(),
            added,
            depends_on: first.depends_on.clone(),
            tags: first.tags.clone(),
            first,
            changes: Vec::new(),
        };
        for change in changes {
            item.changed(change);
        }
        item
    }

    /// Makes this change to the item, after those made before it.
    pub fn changed(&mut self, change: Change) {
        self.is(&change.what);
        self.changes.push(change);
    }

    /// Makes what the change does to the item, keeping no change: what it would stand as, were the change made.
    pub fn is(&mut self, what: &ItemChange) {
        match what {
            ItemChange::Title(title) => self.title.clone_from(title),
            ItemChange::Level(level) => self.level = Some(level.clone()),
            ItemChange::DependsOn(on) => {
                if let Err(at) = self.depends_on.binary_search(on) {
                    self.depends_on.insert(at, *on);
                }
            }
            ItemChange::NoLongerDependsOn(on) => self.depends_on.retain(|kept| kept != on),
            ItemChange::Tagged(tag) => {
                if let Err(at) = self.tags.binary_search(tag) {
                    self.tags.insert(at, tag.clone());
                }
            }
            ItemChange::Untagged(tag) => self.tags.retain(|kept| kept != tag),
        }
    }
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

pub(crate) fn add_item(db: &Connection, new: &First, at: Time) -> Result<ItemNumber, Fault> {
    // Every item the store has was added before the new one, so an item it has is one the new one may depend on.
    let depends_on: BTreeSet<ItemNumber> = new.depends_on.iter().copied().collect();
    for &on in &depends_on {
        if !exists(db, "SELECT 1 FROM items WHERE number = ?1", on.0)? {
            return Err(Fault::NoSuchDependency(on));
        }
    }
    db.execute(
        "INSERT INTO items (title, added, level, project) VALUES (?1, ?2, ?3, ?4)",
        params![new.title, at.0, new.level, new.project],
    )?;
    let number = ItemNumber(db.last_insert_rowid().cast_unsigned());
    for on in depends_on {
        db.execute(
            "INSERT INTO depends (item, on_item) VALUES (?1, ?2)",
            params![number.0, on.0],
        )?;
    }
    let tags: BTreeSet<&String> = new.tags.iter().collect();
    for tag in tags {
        db.execute("INSERT INTO tagged (item, tag) VALUES (?1, ?2)", params![number.0, tag])?;
    }
    Ok(number)
}

/// Keeps each change of an item, after its earlier changes and in the order given, all at one time and for one reason.
/// An item it comes to depend on must be one the store has.
pub(crate) fn change(
    db: &Connection,
    at: Time,
    why: Option<&str>,
    changes: &[(ItemNumber, ItemChange)],
) -> Result<(), Fault> {
    for (item, change) in changes {
        if !exists(db, "SELECT 1 FROM items WHERE number = ?1", item.0)? {
            return Err(Fault::NoSuchItem(*item));
        }
        if let ItemChange::DependsOn(on) = change
            && !exists(db, "SELECT 1 FROM items WHERE number = ?1", on.0)?
        {
            return Err(Fault::NoSuchDependency(*on));
        }
        let what = serde_json::to_string(change).map_err(|e| Fault::Value {
            table: "item_changes",
            message: e.to_string(),
        })?;
        let n: u64 = db.query_row("SELECT count(*) FROM item_changes WHERE item = ?1", [item.0], |row| {
            row.get(0)
        })?;
        db.execute(
            "INSERT INTO item_changes (item, n, at, change, why) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![item.0, n + 1, at.0, what, why],
        )?;
    }
    Ok(())
}

pub(crate) fn items(db: &Connection) -> Result<Vec<Item>, Fault> {
    let mut changes: BTreeMap<u64, Vec<Change>> = BTreeMap::new();
    let mut query = db.prepare("SELECT item, at, change, why FROM item_changes ORDER BY item, n")?;
    let rows = query.query_map([], |row| {
        Ok((
            row.get::<_, u64>(0)?,
            row.get::<_, u64>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Option<String>>(3)?,
        ))
    })?;
    for row in rows {
        let (item, at, what, why) = row?;
        let what = serde_json::from_str(&what).map_err(|e| Fault::Value {
            table: "item_changes",
            message: e.to_string(),
        })?;
        changes.entry(item).or_default().push(Change {
            at: Time(at),
            what,
            why,
        });
    }
    let mut depends: BTreeMap<u64, Vec<ItemNumber>> = BTreeMap::new();
    let mut query = db.prepare("SELECT item, on_item FROM depends ORDER BY item, on_item")?;
    for row in query.query_map([], |row| Ok((row.get::<_, u64>(0)?, row.get::<_, u64>(1)?)))? {
        let (item, on) = row?;
        depends.entry(item).or_default().push(ItemNumber(on));
    }
    let mut tagged: BTreeMap<u64, Vec<String>> = BTreeMap::new();
    let mut query = db.prepare("SELECT item, tag FROM tagged ORDER BY item, tag")?;
    for row in query.query_map([], |row| Ok((row.get::<_, u64>(0)?, row.get::<_, String>(1)?)))? {
        let (item, tag) = row?;
        tagged.entry(item).or_default().push(tag);
    }
    let mut query = db.prepare("SELECT number, title, added, level, project FROM items ORDER BY number")?;
    let rows = query.query_map([], |row| {
        Ok((
            row.get::<_, u64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, u64>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<String>>(4)?,
        ))
    })?;
    let mut items = Vec::new();
    for row in rows {
        let (number, title, added, level, project) = row?;
        let first = First {
            title,
            level,
            project,
            depends_on: depends.remove(&number).unwrap_or_default(),
            tags: tagged.remove(&number).unwrap_or_default(),
        };
        let changes = changes.remove(&number).unwrap_or_default();
        items.push(Item::new(ItemNumber(number), Time(added), first, changes));
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
