//! The values the engine knows about a work, which rows switch on beside what rows recorded: `engine.values` in the
//! flow, each worked out here from the work's rounds, its item, the database and the worktree.

use db::rusqlite::OptionalExtension;
use flow::{CompiledCell, CompiledGraph};
use serde_json::{Map, Value};

use crate::inputs::{Gathered, answered, last_return};
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
        return Ok(if interjections(e, w, g)? > 0 { "yes" } else { "no" }.to_string());
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
            s("implementation")
        }
        "review_round" => {
            let reviewed = !answered(e, w, g, "T3")?.is_empty() || !answered(e, w, g, "T4")?.is_empty();
            s(if reviewed { "table" } else { "blind" })
        }
        // What the project's configuration says of it, which the design stage's cells choose the way by.
        "records" => s(if e.project_of(w)?.records.is_some() {
            "keeps"
        } else {
            "none"
        }),
        "under_test" => s(if e.project_of(w)?.under_test.is_empty() {
            "no"
        } else {
            "yes"
        }),
        "other_projects" => s(if e.project_of(w)?.other_projects.is_empty() {
            "no"
        } else {
            "yes"
        }),
        // What the merge gate waits for: the items it depends on while one is not merged, then the answers to the
        // design's open questions.
        "gates" => s(match gates_waiting_for(e, w, item)? {
            None => "clear",
            Some(_)
                if dependencies(e, item.id)?
                    .iter()
                    .any(|d| !item_merged(e, *d).unwrap_or(false)) =>
            {
                "items"
            }
            Some(_) => "questions",
        }),
        "held" => crate::blockers::held(e, w, g),
        // The notice that an item the work's item depended on was cancelled, which the cells where the work waits take
        // to choose the way on by a row of their own.
        "cancelled" => {
            let waiting = match crate::blockers::standing_line(g, w) {
                Some(line) => crate::blockers::waiting_words(e.db.conn(), w.id, &line, true)?,
                None => 0,
            };
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

/// How many words said to a work are waiting for the line it stands on, which turn its way on.
pub fn interjections(e: &Engine, w: &Work, g: &CompiledGraph) -> Result<i64, String> {
    match crate::blockers::standing_line(g, w) {
        Some(line) => crate::blockers::waiting_words(e.db.conn(), w.id, &line, false),
        None => Ok(0),
    }
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

/// Whether the owner chose another option on a current decision card of the work, or disagreed with one: a work that
/// holds such a card from a flow version before open questions designs again. An answer to an open question is free
/// text, which only the engineer can judge against the design: it turns the work as an interjection, and a redirect
/// takes it back to design by the rows of cell `A4`.
fn design_card_disputed(e: &Engine, w: &Work) -> Result<bool, String> {
    Ok(crate::cards::current(e.db.conn(), w.id)?
        .iter()
        .any(|c| c.owner_ruling().is_some_and(db::ruling_disagrees)))
}

/// What the merge gate waits for, in words; none when it is clear: every item depended on merged, and every open
/// question of the work answered.
pub fn gates_waiting_for(e: &Engine, w: &Work, item: &Item) -> Result<Option<String>, String> {
    for d in dependencies(e, item.id)? {
        if !item_merged(e, d)? {
            return Ok(Some(format!("item {d}, which this item depends on, to be merged")));
        }
    }
    let open: Vec<String> = crate::open_questions::unanswered(e.db.conn(), w.id)?
        .iter()
        .map(i64::to_string)
        .collect();
    Ok(match open.as_slice() {
        [] => None,
        [one] => Some(format!("the answer to open question {one}")),
        more => Some(format!(
            "the answers to open questions {}",
            crate::cards::and_list(more)
        )),
    })
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
