//! What records a merge condition, the table `merge_conditions`: the owner's tick on words said or an answer given, and
//! an item change of the project's architect, write it the same way; and which item an ask is about, which a tick on its
//! answer goes on. Where a condition stands and what the merge gate does with it are the engine's.

use std::collections::BTreeSet;

use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde_json::Value;

use crate::{Result, now};

/// A merge condition to record. `what` is the sentence that reads alone; `source`, `source_id` and `quote` the record
/// its words are in and those words; `said_by` who that record says said them; `recorded_by` who records it, with the
/// round that does when that is the architect (`round`, its work and its number in the work).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeCondition<'a> {
    pub item: i64,
    pub what: &'a str,
    pub source: &'a str,
    pub source_id: i64,
    pub quote: &'a str,
    pub said_by: &'a str,
    pub recorded_by: &'a str,
    pub round: Option<(i64, i64)>,
}

/// Records a merge condition; gives its number. The sentence is kept without the white space around it.
pub fn record_merge_condition(tx: &Transaction<'_>, c: &MergeCondition<'_>) -> Result<i64> {
    tx.execute(
        "INSERT INTO merge_conditions
           (item, what, source, source_id, quote, said_by, recorded_by, recorded_work, recorded_round, at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            c.item,
            c.what.trim(),
            c.source,
            c.source_id,
            c.quote,
            c.said_by,
            c.recorded_by,
            c.round.map(|r| r.0),
            c.round.map(|r| r.1),
            now()
        ],
    )?;
    Ok(tx.last_insert_rowid())
}

/// The one item an ask is about, which a merge condition ticked on its answer goes on: the item of the work that put it,
/// or, for an ask of an architect, which has none, the item the matters it holds are about, when they are all about one.
/// None for an ask about no item or about several, whose words the architect records a condition from if it must.
pub fn item_of_ask(conn: &Connection, ask: i64) -> Result<Option<i64>> {
    let found: Option<(Option<i64>, String)> = conn
        .query_row(
            "SELECT w.item, a.body FROM owner_asks a JOIN works w ON w.id = a.work WHERE a.id = ?1",
            [ask],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let Some((own, body)) = found else {
        return Ok(None);
    };
    if own.is_some() {
        return Ok(own);
    }
    let body: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
    let mut matters: BTreeSet<i64> = match &body["holds"] {
        Value::Array(xs) => xs.iter().filter_map(Value::as_i64).collect(),
        v => v.as_i64().into_iter().collect(),
    };
    let mut parked = conn.prepare("SELECT id FROM architect_matters WHERE asked = ?1")?;
    for m in parked.query_map([ask], |r| r.get::<_, i64>(0))? {
        matters.insert(m?);
    }
    let mut items: BTreeSet<Option<i64>> = BTreeSet::new();
    for m in matters {
        let item: Option<i64> = conn.query_row(
            "SELECT coalesce(m.item, (SELECT w.item FROM works w WHERE w.id = m.work)) FROM architect_matters m
             WHERE m.id = ?1",
            [m],
            |r| r.get(0),
        )?;
        items.insert(item);
    }
    Ok(match items.into_iter().collect::<Vec<_>>().as_slice() {
        [Some(item)] => Some(*item),
        _ => None,
    })
}
