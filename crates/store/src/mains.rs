//! The main dependency of each item: of the dependencies on an item, the one whose item the owner's page draws it
//! whole under, a shadow of it standing under the item of each other (decision `an-item-stands-in-one-place`). An item
//! that some item depends on has exactly one, at all times, and an item nothing depends on has none. The store keeps
//! that itself, in every write that can change what depends on what or which items are cancelled, so that no caller
//! can break it. No change takes the mark off a dependency; it only moves to another:
//!
//! - The first dependency on an item is its main one.
//! - A dependency chosen as the main one ([`ItemChange::Main`](crate::ItemChange::Main)) takes the place of the one
//!   before it.
//! - The main one goes when its dependency is taken away, or the item it is of is cancelled, and one of those left is
//!   chosen. A cancel takes no dependency away (decision `a-cancelled-item-keeps-its-place`): where every item that
//!   depends on the item is cancelled, the main one stays where it was, and goes as soon as an item not cancelled comes
//!   to depend on the item, or one that does is reopened.
//!
//! Where one is chosen, it is of the dependencies on the item one of an item not cancelled before one of a cancelled
//! item, then one of an item of the item's own project before one of another, as the page draws each project's tree
//! apart, then the earliest made, then that of the item of the lowest number ([`settled`]).
//!
//! The store keeps a row each time the main dependency of an item changes (`mains`), so that its rows are only added
//! to: the latest row of an item names its main dependency, or none once nothing depends on it.

use std::collections::{BTreeMap, BTreeSet};

use rusqlite::{Connection, params};
use work::Time;

use crate::{Fault, Item, ItemNumber, WorkNumber, items};

/// The main dependency of each item that some item depends on: by that item, the item whose dependency on it is the
/// main one.
pub type Mains = BTreeMap<ItemNumber, ItemNumber>;

/// The main dependencies as a write leaves them. `kept` holds them as they stood before the write, with each one the
/// write chose set over them in turn; `items`, and `cancelled`, the items whose works are cancelled, are as the write
/// leaves them. A main dependency stays while it is there, and while its item is not cancelled or every item that
/// depends on the same item is; otherwise one is chosen, as the module says.
pub(crate) fn settled(items: &[Item], cancelled: &BTreeSet<ItemNumber>, kept: &Mains) -> Mains {
    let project: BTreeMap<ItemNumber, Option<&str>> = (items.iter())
        .map(|item| (item.number, item.project.as_deref()))
        .collect();
    // Each dependency on an item, by that item: the item that depends on it, and when it was made.
    let mut on: BTreeMap<ItemNumber, Vec<(ItemNumber, Time)>> = BTreeMap::new();
    for item in items {
        for (of, made) in item.dependencies_made() {
            on.entry(of).or_default().push((item.number, made));
        }
    }
    let is_cancelled = |item: &ItemNumber| cancelled.contains(item);
    (on.into_iter())
        .filter_map(|(of, by)| {
            let every_one_cancelled = by.iter().all(|(dependent, _)| is_cancelled(dependent));
            let stays = (kept.get(&of)).filter(|main| {
                by.iter().any(|(dependent, _)| dependent == *main) && (every_one_cancelled || !is_cancelled(main))
            });
            let chosen = || {
                (by.iter())
                    .min_by_key(|(dependent, made)| {
                        let other_project = project.get(dependent) != project.get(&of);
                        (is_cancelled(dependent), other_project, *made, *dependent)
                    })
                    .map(|(dependent, _)| *dependent)
            };
            Some((of, stays.copied().or_else(chosen)?))
        })
        .collect()
}

/// The main dependency of each item some item depends on, as the store keeps them. Refused where what it keeps
/// breaks the rule: an item depended on with none, or with the dependency of an item that does not depend on it, or
/// an item nothing depends on with one.
pub(crate) fn mains(db: &Connection) -> Result<Mains, Fault> {
    let kept = kept(db)?;
    let mut depended: BTreeMap<ItemNumber, BTreeSet<ItemNumber>> = BTreeMap::new();
    for item in items::items(db)? {
        for on in &item.depends_on {
            depended.entry(*on).or_default().insert(item.number);
        }
    }
    for (of, by) in &depended {
        match kept.get(of) {
            Some(main) if by.contains(main) => {}
            Some(main) => {
                return Err(broken(format!(
                    "the main dependency of {of} is that of {main}, which does not depend on it"
                )));
            }
            None => return Err(broken(format!("{of} is depended on, and has no main dependency"))),
        }
    }
    if let Some(of) = kept.keys().find(|of| !depended.contains_key(*of)) {
        return Err(broken(format!("nothing depends on {of}, and it has a main dependency")));
    }
    Ok(kept)
}

/// Keeps the main dependencies as a write leaves them, at its time, none for a write that kept no time: each one the
/// write chose (`chosen`, of an item by the item that depends on it), in the order chosen, set over those kept before
/// it, and the rest as [`settled`] makes them. A row is kept for each item whose main dependency changed, and one that
/// names none for each item nothing depends on any more.
pub(crate) fn settle(db: &Connection, at: Option<Time>, chosen: &[(ItemNumber, ItemNumber)]) -> Result<(), Fault> {
    let before = kept(db)?;
    let mut wanted = before.clone();
    wanted.extend(chosen.iter().copied());
    let after = settled(&items::items(db)?, &cancelled(db)?, &wanted);
    for (of, main) in &after {
        if before.get(of) != Some(main) {
            keep(db, *of, Some(*main), at)?;
        }
    }
    for of in before.keys().filter(|of| !after.contains_key(*of)) {
        keep(db, *of, None, at)?;
    }
    Ok(())
}

/// How many cancels and how many reopens the store keeps of the work: a save that adds to either may change which
/// dependency is the main one.
pub(crate) fn ends(db: &Connection, work: WorkNumber) -> Result<(u64, u64), Fault> {
    Ok(db.query_row(
        "SELECT (SELECT count(*) FROM cancelled WHERE work = ?1), (SELECT count(*) FROM reopened WHERE work = ?1)",
        [work.0],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?)
}

/// Marks the main dependency of each item some item depends on, as the store first keeps them: each chosen as the
/// module says, none being kept before, with no time.
pub(crate) fn mark(db: &Connection) -> Result<(), Fault> {
    settle(db, None, &[])
}

/// The main dependencies the store keeps: of each item, the one its latest row names, where that names one.
fn kept(db: &Connection) -> Result<Mains, Fault> {
    let mut query = db.prepare(
        "SELECT on_item, item FROM mains AS latest \
         WHERE n = (SELECT max(n) FROM mains WHERE on_item = latest.on_item)",
    )?;
    let rows = query.query_map([], |row| Ok((row.get::<_, u64>(0)?, row.get::<_, Option<u64>>(1)?)))?;
    let mut kept = Mains::new();
    for row in rows {
        if let (of, Some(main)) = row? {
            kept.insert(ItemNumber(of), ItemNumber(main));
        }
    }
    Ok(kept)
}

/// The items whose works are cancelled: cancelled, and not reopened since.
fn cancelled(db: &Connection) -> Result<BTreeSet<ItemNumber>, Fault> {
    let mut query = db.prepare(
        "SELECT works.item FROM works \
         WHERE (SELECT max(at) FROM cancelled WHERE work = works.number) \
             > coalesce((SELECT max(at) FROM reopened WHERE work = works.number), -1)",
    )?;
    let rows = query.query_map([], |row| row.get::<_, u64>(0))?;
    let mut cancelled = BTreeSet::new();
    for row in rows {
        cancelled.insert(ItemNumber(row?));
    }
    Ok(cancelled)
}

/// Keeps that the main dependency of `of` is that of `main` from this time on, or that it has none.
fn keep(db: &Connection, of: ItemNumber, main: Option<ItemNumber>, at: Option<Time>) -> Result<(), Fault> {
    db.execute(
        "INSERT INTO mains (on_item, item, at) VALUES (?1, ?2, ?3)",
        params![of.0, main.map(|main| main.0), at.map(|at| at.0)],
    )?;
    Ok(())
}

/// A fault of what the store keeps of the main dependencies.
fn broken(message: String) -> Fault {
    Fault::Value {
        table: "mains",
        message,
    }
}
