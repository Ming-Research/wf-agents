//! Findings set aside: what the engine's checks find in a return of a work that changes some of those checks, found
//! only by the checks the work says it changes (`checks_changed`). The engine that judges a work is the one installed,
//! and a change to one of its checks takes effect only once it is merged and installed: until then the work is judged
//! by the check as it was, and would be sent back by the very fault it mends. Such a problem is not sent back but kept
//! on the round whose return is taken (`rounds.set_aside`), numbered in the work (S1, S2, ...), and the reviews judge
//! each against the diff: the details review and the re-check are given those no review judged yet and answer each,
//! and the merge gate sends any still not judged to the fix and the re-check before the work may merge. Only the project
//! the engine is built from can declare, since no other project's change can change the engine's checks.

use std::collections::BTreeSet;

use db::rusqlite::params;
use flow::{CompiledCell, CompiledGraph};
use serde_json::{Map, Value, json};

use crate::Engine;
use crate::checks::Problem;
use crate::inputs::{Gathered, answered, at_path};
use crate::work::{Item, RoundId, Work};

/// The field of the implementation's and the fix's return that names the checks the change changes.
pub const DECLARED: &str = "checks_changed";

/// The check that judges a declaration: no declaration sets aside what it finds.
pub const KNOWN: &str = "checks_changed_known";

/// The engine's input of the findings set aside no review judged, and the merge gate's value and return that name it.
pub const INPUT: &str = "set_aside";

/// The cells whose returns declare: the implementation's latest declaration holds, with each fix's since.
const IMPLEMENT: &str = "I1";
const FIX: &str = "T6";

/// The cells a round of which, once it handed in its answer, has judged every finding set aside before it: the details
/// review and the re-check.
const JUDGES: [&str; 2] = ["T3", "T4"];

/// The re-check, which judges the findings set aside the fix answered as rows of its table there, not among the others.
const RECHECK: &str = "T4";

/// Where a row of the fix's table comes from when it is a finding set aside the merge gate sent: the gate's return.
const FROM_GATE: &str = "M1·set_aside";

/// The names of the checks a declaration names, in its order.
pub fn names(declaration: Option<&Value>) -> Vec<String> {
    declaration
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|d| d["check"].as_str().map(str::to_string))
        .collect()
}

/// Whether the work's project is the one the engine is built from, whose changes alone can change the engine's checks.
pub fn engine_s_own(e: &Engine, item: Option<&Item>) -> bool {
    item.is_some_and(|i| e.wf_agents_project().as_deref() == Some(i.project.as_str()))
}

/// The checks the work says it changes, as they hold for the return of a round at `cell` that declares `own` (its
/// `checks_changed`, for a return of the implementation or a fix whose declaration is not refused): the declaration of
/// the latest implementation, or this one at the implementation, with that of every fix since, this one's too at a fix.
/// None for a work of another project than the engine's own.
pub fn declared(
    e: &Engine,
    w: &Work,
    g: &CompiledGraph,
    item: Option<&Item>,
    cell: &str,
    own: Option<&Value>,
) -> Result<BTreeSet<String>, String> {
    if !engine_s_own(e, item) {
        return Ok(BTreeSet::new());
    }
    let of = |r: &crate::work::Round| -> Vec<String> {
        let ret = r.output.as_ref().map(|o| g.unwrap(o)).unwrap_or(Value::Null);
        names(at_path(&ret, DECLARED).as_ref())
    };
    let mut held: BTreeSet<String> = BTreeSet::new();
    if cell != IMPLEMENT {
        let implemented = answered(e, w, g, IMPLEMENT)?.into_iter().next();
        let since = implemented.as_ref().map(|r| r.id);
        held.extend(implemented.as_ref().map(of).unwrap_or_default());
        for fix in answered(e, w, g, FIX)?.iter().filter(|r| since.is_none_or(|s| r.id > s)) {
            held.extend(of(fix));
        }
    }
    if cell == IMPLEMENT || cell == FIX {
        held.extend(names(own));
    }
    Ok(held)
}

/// The problems a return's checks found, parted into those sent back and those set aside: a problem of a check the work
/// says it changes is set aside, and any other is sent back, as is every problem of the check that judges the
/// declaration, which no declaration can set aside.
pub fn part(found: Vec<Problem>, declared: &BTreeSet<String>) -> (Vec<Problem>, Vec<Problem>) {
    found
        .into_iter()
        .partition(|p| p.check == KNOWN || !declared.contains(&p.check))
}

/// The problems set aside in a round's return as `rounds.set_aside` keeps them, each numbered after those the work set
/// aside before (S1, S2, ...): `{id, check, field, text}`; none when there are none.
pub fn record(conn: &db::rusqlite::Connection, work: i64, aside: &[Problem]) -> Result<Option<Value>, String> {
    if aside.is_empty() {
        return Ok(None);
    }
    let before: i64 = conn
        .query_row(
            "SELECT count(*) FROM work_set_aside WHERE work = ?1",
            params![work],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let entries: Vec<Value> = aside
        .iter()
        .zip(before + 1..)
        .map(|(p, n)| json!({ "id": format!("S{n}"), "check": p.check, "field": p.field, "text": p.text }))
        .collect();
    Ok(Some(Value::Array(entries)))
}

/// Every finding set aside in the work, oldest first, each with the round whose return it was found in, as the input
/// gives it: `{id, check, field, text, round}`, a field left out for a problem about the return as a whole.
fn recorded(e: &Engine, work: i64) -> Result<Vec<(RoundId, Value)>, String> {
    let rows = crate::work::query_json(
        e.db.conn(),
        "SELECT round, id, check_name, field, text FROM work_set_aside WHERE work = ?1 ORDER BY round, position",
        &[&work],
        10_000,
    )?;
    Ok(rows
        .into_iter()
        .map(|r| {
            let round = RoundId::new(work, r["round"].as_i64().unwrap_or_default());
            let mut entry = Map::new();
            entry.insert("id".into(), r["id"].clone());
            entry.insert("check".into(), r["check_name"].clone());
            if r["field"].is_string() {
                entry.insert("field".into(), r["field"].clone());
            }
            entry.insert("text".into(), r["text"].clone());
            entry.insert("round".into(), json!(round.to_string()));
            (round, Value::Object(entry))
        })
        .collect())
}

/// The findings set aside in the work that no review has judged: those after the latest round of the details review or
/// the re-check that handed in its answer, oldest first.
pub fn unjudged(e: &Engine, w: &Work, g: &CompiledGraph) -> Result<Vec<Value>, String> {
    let mut judged: Option<RoundId> = None;
    for cell in JUDGES {
        if let Some(r) = answered(e, w, g, cell)?.into_iter().next() {
            judged = judged.max(Some(r.id));
        }
    }
    Ok(recorded(e, w.id)?
        .into_iter()
        .filter(|(round, _)| judged.is_none_or(|j| *round > j))
        .map(|(_, entry)| entry)
        .collect())
}

/// What a cell is given of the findings set aside no review judged: all of them, but at the re-check those the fix
/// answered as rows of its table, which the re-check judges there. None when there are none.
pub fn input(e: &Engine, w: &Work, g: &CompiledGraph, cell: &CompiledCell) -> Result<Option<Value>, String> {
    let mut due = unjudged(e, w, g)?;
    if cell.id == RECHECK {
        let table: Vec<Value> = answered(e, w, g, FIX)?
            .into_iter()
            .next()
            .and_then(|r| r.input["engine"]["table"].as_array().cloned())
            .unwrap_or_default();
        let answered_there: Vec<&Value> = table
            .iter()
            .filter(|row| row["from"] == FROM_GATE)
            .map(|row| &row["id"])
            .collect();
        due.retain(|f| !answered_there.contains(&&f["id"]));
    }
    Ok((!due.is_empty()).then_some(Value::Array(due)))
}

/// The numbers of the findings set aside a round of `cell` is given, as [`input`] gives them.
pub fn due_ids(e: &Engine, w: &Work, g: &CompiledGraph, cell: &CompiledCell) -> Result<Vec<String>, String> {
    Ok(input(e, w, g, cell)?
        .as_ref()
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|f| f["id"].as_str().map(str::to_string))
        .collect())
}

/// What the merge gate's step returns: on the value `set_aside` of the gate, the findings set aside it was given, for
/// the fix to answer as rows of its table; nothing otherwise, nor for a flow version whose gate returns none.
pub fn gate_return(cell: &CompiledCell, gathered: &[Gathered], given: &Map<String, Value>) -> Value {
    let returns_them = cell
        .schema
        .as_ref()
        .and_then(|s| s["properties"].get(INPUT))
        .is_some();
    if !returns_them || given.get("gates").and_then(Value::as_str) != Some(INPUT) {
        return json!({});
    }
    let them = gathered
        .iter()
        .find(|x| x.input.from.iter().any(|s| matches!(s, flow::format::Src::Engine(n) if n == INPUT)))
        .and_then(|x| x.value.clone());
    match them {
        Some(v) => json!({ INPUT: v }),
        None => json!({}),
    }
}
