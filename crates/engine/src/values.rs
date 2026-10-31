//! The values the engine knows about a work, which rows switch on beside what rows recorded: `engine.values` in the
//! flow, each worked out here from the work's rounds, its item, the database and the worktree.

use db::rusqlite::OptionalExtension;
use flow::{CompiledCell, CompiledGraph};
use serde_json::{Map, Value};

use crate::inputs::{Gathered, answered, at_path, last_return};
use crate::work::{Item, Work};
use crate::{ENGINEER, Engine, git};

/// What the rows of a cell are given: the values rows recorded, the engine's values the cell names, and each input
/// read by id under `@<id>`.
pub fn given(
    e: &Engine,
    w: &Work,
    g: &CompiledGraph,
    cell: &CompiledCell,
    item: Option<&Item>,
    gathered: &[Gathered],
) -> Result<Map<String, Value>, String> {
    let mut out = Map::new();
    for (k, v) in &w.pos.recorded {
        out.insert(k.clone(), Value::String(v.clone()));
    }
    for k in cell.inenums.keys() {
        if !w.pos.recorded.contains_key(k) {
            out.insert(k.clone(), Value::String(value(e, w, g, item, k)?));
        }
    }
    for x in gathered {
        if let (Some(id), Some(v)) = (&x.input.id, &x.value) {
            // An input read by id whose one source is a path is given as the value at that path.
            out.insert(format!("@{id}"), v.clone());
        }
    }
    Ok(out)
}

/// One of the engine's values. Those of the architect's graph are worked out by [`crate::architect`]; the others are
/// about the item a work is on.
pub fn value(e: &Engine, w: &Work, g: &CompiledGraph, item: Option<&Item>, name: &str) -> Result<String, String> {
    if let Some(v) = crate::architect::value(e, w, g, name)? {
        return Ok(v);
    }
    // An interjection waiting for the task's main line; a word that needs no answer, as the owner confirming the design
    // card, is not one that turns the work.
    if name == "interjection" {
        return Ok(if interjections(e, w)? > 0 { "yes" } else { "no" }.to_string());
    }
    let item = item.ok_or_else(|| {
        format!(
            "the engine's value {name} is about an item, and work {} is on none",
            w.id
        )
    })?;
    let s = |x: &str| Ok(x.to_string());
    match name {
        "deliver" => Ok(item.deliver.clone()),
        "work" => {
            if item.deliver == "report" {
                return s("report");
            }
            let tree = last_return(e, w, g, "D2", "design_tree")?.and_then(|r| at_path(&r, "design_tree.verdict"));
            if tree.as_ref().and_then(Value::as_str) == Some("design_paths_only") {
                s("design_paths_only")
            } else {
                s("implementation")
            }
        }
        "review_round" => {
            let reviewed = !answered(e, w, g, "T3")?.is_empty() || !answered(e, w, g, "T4")?.is_empty();
            s(if reviewed { "table" } else { "blind" })
        }
        "tree" => {
            let project = e.project_of(w)?;
            let Some(wt) = &w.worktree else { return s("unchanged") };
            let base = match reviewed_head(e, w, g)? {
                Some(h) => h,
                None => git::fork_point(wt, &project.main)?,
            };
            let changed = git::own_changes(wt, &project.main, &base, "HEAD")?;
            s(if changed.iter().any(|f| git::under(f, &project.design_paths)) {
                "changed"
            } else {
                "unchanged"
            })
        }
        // What the merge gate waits for: the items it depends on while one is not merged, then the owner's ruling.
        "gates" => s(match gates_waiting_for(e, w, item)? {
            None => "clear",
            Some(_)
                if dependencies(e, item.id)?
                    .iter()
                    .any(|d| !item_merged(e, *d).unwrap_or(false)) =>
            {
                "items"
            }
            Some(_) => "ruling",
        }),
        "held" => crate::blockers::held(e, w),
        // The notice that an item the work's item depended on was cancelled, which the cells where the work waits take
        // to choose the way on by a row of their own.
        "cancelled" => {
            let waiting: i64 = e
                .db
                .conn()
                .query_row(
                    "SELECT count(*) FROM interjections WHERE work = ?1 AND line = 'engineer' AND delivered_round IS NULL
                       AND cancelled_item IS NOT NULL",
                    [w.id],
                    |r| r.get(0),
                )
                .map_err(|e| e.to_string())?;
            s(if waiting > 0 { "yes" } else { "no" })
        }
        // Whether another work of the project is ahead of this one in its merge queue.
        "turn" => {
            let ahead: Option<i64> =
                e.db.conn()
                    .query_row(
                        "SELECT work FROM merge_queue WHERE project = ?1 LIMIT 1",
                        [&item.project],
                        |r| r.get(0),
                    )
                    .optional()
                    .map_err(|e| e.to_string())?;
            s(if ahead.is_some_and(|a| a != w.id) {
                "behind"
            } else {
                "first"
            })
        }
        "start" => Err(format!(
            "work {}: what holds an item back from starting is worked out for the item, before any work",
            w.id
        )),
        "split" => {
            let reply = brought_back(e, w, g)?;
            let children = reply
                .as_ref()
                .and_then(|r| r["children"].as_array())
                .is_some_and(|c| !c.is_empty());
            let kept = reply.as_ref().and_then(|r| r["rest"].as_str()) != Some("none_kept");
            s(match (children, kept) {
                (false, _) => "not_split",
                (true, true) => "split",
                (true, false) => "all_split",
            })
        }
        // A flow version stored before this value was named so calls it `card`.
        "design_card_disputed" | "card" => s(if design_card_disputed(e, w)? { "yes" } else { "no" }),
        "came_by" => {
            let from = arrived_from(e, w, g)?.unwrap_or_default();
            let fc = g.cell(&from);
            // The row the holder's cell took: a question withdrawn, or what was said to the work, is no reply.
            let took = crate::work::rounds_at(e.db.conn(), w.id, &from)?
                .into_iter()
                .find_map(|r| r.transition);
            s(match (fc, took.as_deref()) {
                (Some(c), Some("interjection")) if matches!(c.id.as_str(), "A2" | "A3") => "interjection_at_engine",
                (Some(c), Some("holder_gone" | "cancelled")) if c.id == "A2" => "withdrawn",
                (Some(c), _) if c.id == "A7" => "withdrawn",
                (Some(c), _) if matches!(c.id.as_str(), "A1" | "A2" | "A3") => "reply",
                (Some(c), _) if c.id == "A6" => "waited",
                (Some(c), _) if c.kind == "agent" => "interjection",
                _ => "interjection_at_engine",
            })
        }
        "must_wait" => {
            let reply = brought_back(e, w, g)?;
            let waits = reply
                .as_ref()
                .and_then(|r| r["waits"].as_array())
                .cloned()
                .unwrap_or_default();
            let mut unmerged = false;
            for x in waits {
                let key = x["key"]
                    .as_str()
                    .and_then(|k| k.trim_start_matches('#').parse::<i64>().ok());
                if key.is_none_or(|k| !item_merged(e, k).unwrap_or(false)) {
                    unmerged = true;
                }
            }
            s(if unmerged { "yes" } else { "no" })
        }
        "waited" => {
            let deps = dependencies(e, item.id)?;
            let mut all = true;
            for d in deps {
                all &= item_merged(e, d)?;
            }
            s(if all { "all_merged" } else { "waiting" })
        }
        other => Err(format!("no such engine value: {other}")),
    }
}

/// How many words said to a work are waiting for its engineer's line, which turn its way on.
pub fn interjections(e: &Engine, w: &Work) -> Result<i64, String> {
    e.db.conn()
        .query_row(
            "SELECT count(*) FROM interjections WHERE work = ?1 AND line = ?2 AND delivered_round IS NULL AND informs = 0",
            db::rusqlite::params![w.id, ENGINEER],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())
}

/// The reply the work brought back to choose the way on: the last return of the cell that brought it, when that is
/// the advisor, the architect or the owner.
fn brought_back(e: &Engine, w: &Work, g: &CompiledGraph) -> Result<Option<Value>, String> {
    let Some(from) = arrived_from(e, w, g)? else {
        return Ok(None);
    };
    if !matches!(from.as_str(), "A1" | "A2" | "A3") {
        return Ok(None);
    }
    Ok(last_return(e, w, g, &from, "reply")?.and_then(|r| r.get("reply").cloned()))
}

/// The cell that brought the work to the cell it is at, past the round trips the cell made since to the cells that
/// only do something for it and come back: running tools, waiting for the owner's Start after a pause, handing its
/// answers to who interjected.
fn arrived_from(e: &Engine, w: &Work, g: &CompiledGraph) -> Result<Option<String>, String> {
    let back = |id: &str, t: &str| {
        g.cell(id).is_some_and(|c| {
            c.kind != "agent" && !g.asking.contains(&c.id) && c.rows.iter().any(|x| x.name == t && x.to == "@caller")
        })
    };
    for r in crate::work::rounds(e.db.conn(), w.id)?.iter().rev() {
        let (Some(cell), Some(t)) = (r.cell.as_deref(), r.transition.as_deref()) else {
            continue;
        };
        if cell == w.pos.at || back(cell, t) {
            continue;
        }
        return Ok(Some(cell.to_string()));
    }
    Ok(w.pos.from.clone())
}

/// The items an item depends on; a cancelled one, which never delivers, holds back nothing, and cancelling an item
/// removes the dependencies on it.
pub fn dependencies(e: &Engine, item: i64) -> Result<Vec<i64>, String> {
    let mut st =
        e.db.conn()
            .prepare(
                "SELECT l.depends_on FROM item_links l JOIN items d ON d.id = l.depends_on
                 WHERE l.item = ?1 AND d.cancelled IS NULL",
            )
            .map_err(|e| e.to_string())?;
    st.query_map([item], |r| r.get(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())
}

/// Whether an item is merged: one of its works took the merge cell's merged row.
pub fn item_merged(e: &Engine, item: i64) -> Result<bool, String> {
    let n: i64 =
        e.db.conn()
            .query_row(
                &format!(
                    "SELECT count(*) FROM rounds r JOIN works w ON w.id = r.work
                     WHERE w.item = ?1 AND r.cell IN {} AND r.transition = 'merged'",
                    db::cells_sql(ENGINEER, &["M2"])
                ),
                [item],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
    Ok(n > 0)
}

/// A design card of a work, with the head of the round that handed it in, when the design paths matched it.
#[derive(Debug, Clone)]
pub struct DesignCard {
    pub id: i64,
    pub status: String,
    pub rulings: Value,
    pub head: Option<String>,
    /// How many decisions it holds; one for a card whose body holds none.
    pub decisions: usize,
}

/// The work's latest design card that was not replaced, if any.
pub fn design_card(e: &Engine, w: &Work) -> Result<Option<DesignCard>, String> {
    let r = e.db.conn().query_row(
        "SELECT c.status, c.rulings, json_extract(r.input, '$.engine.head_after'), c.id,
                max(1, coalesce(json_array_length(c.body), 1))
         FROM design_cards c JOIN rounds r ON r.work = c.work AND r.n = c.round
         WHERE c.work = ?1 AND c.status <> 'replaced' ORDER BY c.id DESC LIMIT 1",
        [w.id],
        |r| {
            Ok(DesignCard {
                id: r.get(3)?,
                status: r.get(0)?,
                rulings: serde_json::from_str(&r.get::<_, String>(1)?).unwrap_or(Value::Null),
                head: r.get(2)?,
                decisions: r.get::<_, i64>(4)? as usize,
            })
        },
    );
    match r {
        Ok(c) => Ok(Some(c)),
        Err(db::rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

/// Whether the work's design card has a ruling that disagrees with any of its decisions.
fn design_card_disputed(e: &Engine, w: &Work) -> Result<bool, String> {
    Ok(design_card(e, w)?.is_some_and(|c| {
        c.rulings
            .as_array()
            .is_some_and(|rs| rs.iter().any(db::ruling_disagrees))
    }))
}

/// Whether the work may enter the merge queue: every item it depends on is merged, and the design paths did not
/// change on the branch, or the owner confirmed its design card and they have not changed since.
/// What the merge gate waits for, in words; none when it is clear: every item depended on merged, and the design tree
/// unchanged on the branch, or changed only as the design card the owner confirmed says.
pub fn gates_waiting_for(e: &Engine, w: &Work, item: &Item) -> Result<Option<String>, String> {
    for d in dependencies(e, item.id)? {
        if !item_merged(e, d)? {
            return Ok(Some(format!("item {d}, which this item depends on, to be merged")));
        }
    }
    let project = e.project_of(w)?;
    let Some(wt) = &w.worktree else { return Ok(None) };
    let fork = git::fork_point(wt, &project.main)?;
    let changed = git::changed(wt, &fork, "HEAD")?;
    if !changed.iter().any(|f| git::under(f, &project.design_paths)) {
        return Ok(None);
    }
    let Some(card) = design_card(e, w)? else {
        return Ok(Some("a design card for the change to the design tree".to_string()));
    };
    // The owner's last ruling confirms every decision of the card.
    let confirmed = card.status == "ruled"
        && card
            .rulings
            .as_array()
            .and_then(|rs| rs.last())
            .is_some_and(|r| r["by"] == "owner" && db::ruling_confirms(r, card.decisions));
    if !confirmed {
        return Ok(Some(format!("the owner's ruling on design card {}", card.id)));
    }
    let Some(head) = card.head else {
        return Ok(Some(format!("the owner's ruling on design card {}", card.id)));
    };
    let since = git::own_changes(wt, &project.main, &head, "HEAD")?;
    Ok(since.iter().any(|f| git::under(f, &project.design_paths)).then(|| {
        format!(
            "a new design card: the design tree changed after the owner confirmed design card {}",
            card.id
        )
    }))
}

/// The head the last review started from; none before the first review.
pub fn reviewed_head(e: &Engine, w: &Work, g: &CompiledGraph) -> Result<Option<String>, String> {
    head_reviewed_by(e, w, g, "T4")
}

/// The head a review cell last looked at: that of its newest answered round, if any.
pub fn head_reviewed_by(e: &Engine, w: &Work, g: &CompiledGraph, cell: &str) -> Result<Option<String>, String> {
    Ok(answered(e, w, g, cell)?
        .first()
        .and_then(|r| r.input["engine"]["head"].as_str().map(str::to_string)))
}

/// What the PR's own change is, apart from catching up with main: a hash of its diff from the fork point.
pub fn change_hash(wt: &std::path::Path, main: &str) -> Option<String> {
    let fork = git::fork_point(wt, main).ok()?;
    let d = git::diff(wt, &fork, "HEAD", &[]).ok()?;
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in d.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    Some(format!("{h:016x}"))
}
