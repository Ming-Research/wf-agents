//! The values the engine knows about a work, which rows switch on beside what rows recorded: `engine.values` in the
//! flow, each worked out here from the work's rounds, its item, the database and the worktree.

use db::rusqlite::OptionalExtension;
use flow::{CompiledCell, CompiledGraph};
use serde_json::{Map, Value};

use crate::inputs::{Gathered, answered, last_return};
use crate::work::{Item, Work};
use crate::{ENGINEER, Engine};

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

/// The cell of the initial research, whose rounds the engine counts to ask it whether the work can be started directly.
const RESEARCH: &str = "R1";

/// The merge queue's cell, and its rows that send a failure back: to the engineer, or the third time in a row to the
/// architect.
const MERGE: &str = "M2";
const MERGE_FAILED: &str = "for_engineer";
const MERGE_FAILED_THIRD: &str = "third_failure";

/// One of the engine's values. Those of the architect's graph are worked out by [`crate::architect`]; the others are
/// about the item a work is on.
pub fn value(e: &Engine, w: &Work, g: &CompiledGraph, item: Option<&Item>, name: &str) -> Result<String, String> {
    if let Some(v) = crate::architect::value(e, w, g, name)? {
        return Ok(v);
    }
    // An interjection waiting for the task's main line; a word that needs no answer is not one that turns the work.
    if name == "interjection" {
        return Ok(if interjections(e, w, g)? > 0 { "yes" } else { "no" }.to_string());
    }
    // A work brought to the new flow version goes back to where it stopped, when that is a cell of the version.
    if name == "brought_up" {
        let back = w.pos.stopped.as_deref().is_some_and(|c| g.cell(c).is_some());
        return Ok(if back { "went_back" } else { "stuck" }.to_string());
    }
    let item = item.ok_or_else(|| {
        format!(
            "the engine's value {name} is about an item, and work {} is on none",
            w.id
        )
    })?;
    let s = |x: &str| Ok(x.to_string());
    let yes = |b: bool| Ok(if b { "yes" } else { "no" }.to_string());
    match name {
        "work" => {
            if item.deliver == "report" {
                return s("report");
            }
            s("implementation")
        }
        // Whether the item carries a decision point, which sends the work to deep research whatever its research says.
        "decision_points" => yes(decision_points(e, item.id)?),
        // Whether the initial research has run the project's bound of rounds since it last handed in its answer.
        "research_rounds" => {
            let bound = e.project_of(w)?.research_rounds.unwrap_or(config::RESEARCH_ROUNDS);
            s(if research_rounds(e, w, g)? >= bound {
                "past"
            } else {
                "within"
            })
        }
        // Whether this failure in the merge queue is the third in a row since the work last went to the architect.
        "merge_failures" => s(if merge_failures(e, w)? >= 2 { "third" } else { "fewer" }),
        // Where the review of the work's latest design stands.
        "review" => s(crate::review::stage(e.db.conn(), w.id)?),
        // What the merge gate waits for: the items it depends on while one is not merged, then the owner's approval of
        // a change that touches a path the owner approves.
        "gates" => {
            let waits_for_items = dependencies(e, item.id)?
                .iter()
                .any(|d| !item_merged(e, *d).unwrap_or(false));
            if waits_for_items {
                return s("items");
            }
            s(crate::approval::gate(e.db.conn(), &e.project_of(w)?, w)?)
        }
        "held" => crate::blockers::held(e, w, g),
        // The notice that an item the work's item depended on was cancelled, which the cells where the work waits take
        // to choose the way on by a row of their own.
        "cancelled" => {
            let waiting = match crate::blockers::standing_line(g, w) {
                Some(line) => crate::blockers::waiting_words(e.db.conn(), w.id, &line, true)?,
                None => 0,
            };
            yes(waiting > 0)
        }
        // Whether another work of the project is ahead of this one in its merge queue; while main's CI is red for the
        // project, a matter of main red open, the queue is stopped and every work in it waits as one behind.
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
            let stopped = crate::main_watch::red(e.db.conn(), &item.project)?;
            s(if stopped || ahead.is_some_and(|a| a != w.id) {
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
        "came_by" => {
            let from = arrived_from(e, w, g)?.unwrap_or_default();
            let fc = g.cell(&from);
            // The row the holder's cell took: a question withdrawn, or what was said to the work, is no reply.
            let took = crate::work::rounds_at(e.db.conn(), w.id, &from)?
                .into_iter()
                .find_map(|r| r.transition);
            s(match (fc, took.as_deref()) {
                (Some(c), Some("interjection")) if asks(c) => "interjection_at_engine",
                (Some(c), Some("holder_gone" | "cancelled")) if asks(c) => "withdrawn",
                (Some(c), _) if c.id == "A7" => "withdrawn",
                (Some(c), _) if asks(c) => "reply",
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
            yes(unmerged)
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

/// Whether a cell is one where someone answers the work: the advisor's, or a cell that puts what it holds to the
/// architect or the owner and brings back the reply.
fn asks(c: &CompiledCell) -> bool {
    c.id == "A1" || matches!(c.step.as_deref(), Some("ask_architect" | "ask_owner"))
}

/// Whether the item carries a decision point: something settled in discussion, with the option the owner chose, that
/// goes into the project's decisions once deep research confirms it.
pub fn decision_points(e: &Engine, item: i64) -> Result<bool, String> {
    let points: Option<String> =
        e.db.conn()
            .query_row("SELECT decision_points FROM items WHERE id = ?1", [item], |r| r.get(0))
            .optional()
            .map_err(|e| e.to_string())?
            .flatten();
    Ok(crate::work::json_col(points)
        .and_then(|v| v.as_array().map(|xs| !xs.is_empty()))
        .unwrap_or(false))
}

/// The rounds the initial research ran since it last handed in its own answer, or since the work began.
fn research_rounds(e: &Engine, w: &Work, g: &CompiledGraph) -> Result<u64, String> {
    let since = answered(e, w, g, RESEARCH)?.first().map(|r| r.id.n);
    let n = crate::work::rounds_at(e.db.conn(), w.id, RESEARCH)?
        .iter()
        .filter(|r| r.transition.is_some() && since.is_none_or(|s| r.id.n > s))
        .count();
    Ok(n as u64)
}

/// The failures the merge queue sent back to the engineer since it last sent one to the architect, or since the work
/// began.
fn merge_failures(e: &Engine, w: &Work) -> Result<usize, String> {
    let rounds = crate::work::rounds_at(e.db.conn(), w.id, MERGE)?;
    Ok(rounds
        .iter()
        .filter_map(|r| r.transition.as_deref())
        .take_while(|t| *t != MERGE_FAILED_THIRD)
        .filter(|t| *t == MERGE_FAILED)
        .count())
}

/// How many words said to a work are waiting for the line it stands on, which turn its way on.
pub fn interjections(e: &Engine, w: &Work, g: &CompiledGraph) -> Result<i64, String> {
    match crate::blockers::standing_line(g, w) {
        Some(line) => crate::blockers::waiting_words(e.db.conn(), w.id, &line, false),
        None => Ok(0),
    }
}

/// The reply the work brought back to choose the way on: the last return of the cell that brought it, when that is
/// the advisor, or a cell that put what it holds to the architect or the owner.
fn brought_back(e: &Engine, w: &Work, g: &CompiledGraph) -> Result<Option<Value>, String> {
    let Some(from) = arrived_from(e, w, g)? else {
        return Ok(None);
    };
    if !g.cell(&from).is_some_and(asks) {
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
                    db::cells_sql(ENGINEER, &[MERGE])
                ),
                [item],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
    Ok(n > 0)
}

/// What the merge gate waits for, in words; none when it is clear: every item depended on merged, and the owner's
/// approval given where the change needs it.
pub fn gates_waiting_for(e: &Engine, w: &Work, item: &Item) -> Result<Option<String>, String> {
    for d in dependencies(e, item.id)? {
        if !item_merged(e, d)? {
            return Ok(Some(format!("item {d}, which this item depends on, to be merged")));
        }
    }
    Ok((crate::approval::gate(e.db.conn(), &e.project_of(w)?, w)? == "owner")
        .then(|| "the owner's approval of the change".to_string()))
}

/// The head the last review started from; none before the first review: of its re-check, or of its second pass.
pub fn reviewed_head(e: &Engine, w: &Work, g: &CompiledGraph) -> Result<Option<String>, String> {
    let recheck = answered(e, w, g, "T4")?.into_iter().next();
    let review = answered(e, w, g, "T3")?.into_iter().next();
    let newest = match (recheck, review) {
        (Some(a), Some(b)) => Some(if a.id.n > b.id.n { a } else { b }),
        (a, b) => a.or(b),
    };
    Ok(newest.and_then(|r| r.input["engine"]["head"].as_str().map(str::to_string)))
}
