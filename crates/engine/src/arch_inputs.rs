//! What the architect's cells are given about the works of its project's items, the cells they stand at and the
//! observations a batch names: the facts those cells judge by, each worked out once, here, from the engine's own
//! records. Every place that shows a work (the board, an item in full, the works since the architect's last work)
//! takes its columns from the one card [`card`] gives, so that a column added to a work's card reaches all three and
//! none shows a work another way.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use db::rusqlite::{Connection, params};
use flow::CompiledCell;
use serde_json::{Map, Value, json};

use crate::architect::{batch_start, ids_of, picked_before};
use crate::work::query_json;
use crate::{ENGINEER, Engine, git};

fn sql(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// The works that hold a slot now, as the loop counts them ([`Engine::slot_holders`]): worked out once for an input,
/// and asked of for each work it shows.
#[derive(Debug, Default)]
pub(crate) struct Holding {
    ids: BTreeSet<i64>,
    listed: Vec<Value>,
}

impl Holding {
    /// The works holding a slot now, each given by its number, its item, the cell it stands at and its machine.
    pub(crate) fn new(works: impl IntoIterator<Item = (i64, Option<i64>, String, Option<String>)>) -> Holding {
        let mut h = Holding::default();
        for (work, item, at, machine) in works {
            h.ids.insert(work);
            let mut o = Map::new();
            o.insert("work".into(), json!(work));
            o.insert("item".into(), json!(item));
            o.insert("at".into(), json!(at));
            o.insert("machine".into(), json!(machine));
            o.retain(|_, v| !v.is_null());
            h.listed.push(Value::Object(o));
        }
        h
    }

    /// The works that hold a slot in the engine now.
    pub(crate) fn of(e: &Engine) -> Result<Holding, String> {
        Ok(Holding::new(
            e.slot_holders()?
                .into_iter()
                .map(|w| (w.id, w.item, w.pos.at, w.machine)),
        ))
    }

    /// Whether a work holds a slot.
    pub(crate) fn holds(&self, work: i64) -> bool {
        self.ids.contains(&work)
    }

    /// How many works hold a slot.
    pub(crate) fn count(&self) -> usize {
        self.ids.len()
    }

    /// The works holding a slot, as `slots` lists them.
    pub(crate) fn listed(&self) -> Vec<Value> {
        self.listed.clone()
    }
}

/// What the engine recorded of a run of the CI on a work: the head it ran on and how it came out. The fast CI's
/// result holds the checks of the push or of the run; the full CI's holds the kind it came to, or its checks.
fn ci_state(run: &Value) -> Option<Value> {
    if run.is_null() {
        return None;
    }
    let result = &run["result"];
    let verdict = match result["result"]["kind"].as_str() {
        Some(kind) => kind.to_string(),
        None => {
            let checks = result["push"]["checks"]
                .as_array()
                .or_else(|| result["ran"]["checks"].as_array());
            match checks {
                Some(cs) if !cs.is_empty() && cs.iter().all(|c| c["result"] == "green") => "green".to_string(),
                Some(cs) if !cs.is_empty() => "not green".to_string(),
                _ => "unknown".to_string(),
            }
        }
    };
    Some(json!({ "head": run["head"], "result": verdict }))
}

/// A work's card: its branch, its worktree, its pull request's number and what the engine recorded of that pull request
/// (whether it merged, the head last pushed, and how the latest runs of the fast and the full CI came out), and whether
/// it holds a slot now. Everything is the engine's own record, read from the database: nothing here asks GitHub, so the
/// pull request is as the engine last recorded it, not as it stands there now. A column the work has no value for is
/// left out.
pub(crate) fn card(conn: &Connection, work: i64, holding: &Holding) -> Result<Map<String, Value>, String> {
    let merge = db::cells_sql(ENGINEER, &["M2"]);
    let rows = query_json(
        conn,
        &format!(
            "SELECT w.branch, w.worktree, w.pr, w.pushed_head, c.fast_ci, c.full_ci,
                    EXISTS (SELECT 1 FROM rounds r WHERE r.work = w.id AND r.cell IN {merge}
                              AND r.transition = 'merged') AS merged
             FROM works w JOIN work_ci c ON c.work = w.id WHERE w.id = ?1"
        ),
        &[&work],
        1,
    )?;
    let mut out = Map::new();
    let Some(r) = rows.first() else {
        return Ok(out);
    };
    for k in ["branch", "worktree"] {
        if let Some(v) = r[k].as_str() {
            out.insert(k.into(), json!(v));
        }
    }
    if let Some(pr) = r["pr"].as_i64() {
        out.insert("pr".into(), json!(pr));
        let mut state = Map::new();
        state.insert("merged".into(), json!(r["merged"] == 1));
        if let Some(head) = r["pushed_head"].as_str() {
            state.insert("pushed_head".into(), json!(head));
        }
        for k in ["fast_ci", "full_ci"] {
            if let Some(v) = ci_state(&r[k]) {
                state.insert(k.into(), v);
            }
        }
        out.insert("pr_state".into(), Value::Object(state));
    }
    out.insert("holds_slot".into(), json!(holding.holds(work)));
    Ok(out)
}

/// The most files of a branch's change that an item in full lists.
const FILES_MOST: usize = 60;

/// The files a work's branch changes against its project's main, as its worktree has them fetched: the first
/// [`FILES_MOST`] by name, and how many there are; or why git could not say.
pub(crate) fn branch_files(worktree: &Path, main: &str) -> Value {
    match git::fork_point(worktree, main).and_then(|fork| git::changed(worktree, &fork, "HEAD")) {
        Ok(files) => json!({ "total": files.len(), "files": files.iter().take(FILES_MOST).collect::<Vec<_>>() }),
        Err(why) => json!({ "unavailable": why }),
    }
}

/// What a worktree holds that is not committed, untracked files included, each as git's short status writes it (the two
/// status letters, a space and the path); or why git could not say.
pub(crate) fn uncommitted(worktree: &Path) -> Value {
    match git::git(worktree, &["status", "--porcelain", "-z", "--untracked-files=all"]) {
        Ok(out) => {
            let mut files = Vec::new();
            let mut entries = out.split('\0').filter(|e| !e.is_empty());
            while let Some(entry) = entries.next() {
                files.push(entry.to_string());
                // A rename or a copy is followed by the path it came from, which is no entry of its own.
                if entry.starts_with(['R', 'C']) || entry.get(1..2).is_some_and(|y| y == "R" || y == "C") {
                    entries.next();
                }
            }
            json!(files)
        }
        Err(why) => json!({ "unavailable": why }),
    }
}

/// What the architect is given of a cell of the engineer's graph: its name, what kind of cell it is, the stage it is in,
/// and whether a work standing at it holds a slot. `end` is the graph's end cell.
pub(crate) fn cell_entry(c: &CompiledCell, end: &str) -> Value {
    let holds = if c.id == end {
        "no: the work is over".to_string()
    } else {
        match c.step.as_deref() {
            Some("ask_owner") => "no: it waits for the owner's answer".to_string(),
            Some("ask_architect") => "no: it waits for the architect's reply".to_string(),
            _ => {
                let waits: Vec<&str> = c.rows.iter().filter(|r| r.waits).map(|r| r.name.as_str()).collect();
                if waits.is_empty() {
                    "yes".to_string()
                } else {
                    format!(
                        "yes, but not while it waits at the row {} for what it waits for",
                        waits.join(" or ")
                    )
                }
            }
        }
    };
    let mut o = Map::new();
    o.insert("title".into(), json!(c.title));
    o.insert("kind".into(), json!(c.kind));
    if let Some(stage) = &c.stage {
        o.insert("stage".into(), json!(stage));
    }
    o.insert("holds_slot".into(), json!(holds));
    Value::Object(o)
}

/// The cells of the engineer's graph the board and the lists of works name, each by its id: the current flow's, and the
/// cells that unfinished works stand at on an older flow version, which that version names (`older_flow_versions`, by
/// the version). A cell an older version names is described by that version, not by the current one, whose cell of the
/// same id may be another.
pub(crate) fn cells(e: &Engine) -> Result<Value, String> {
    let now = e.flow(e.flow_now)?;
    let mut out = Map::new();
    if let Some(g) = now.graph(ENGINEER) {
        let flow: Map<String, Value> = g.cells.iter().map(|c| (c.id.clone(), cell_entry(c, &g.end))).collect();
        out.insert("flow".into(), Value::Object(flow));
    }
    let stand = query_json(
        e.db.conn(),
        "SELECT DISTINCT flow_version, at FROM works
         WHERE graph = 'engineer' AND ended IS NULL AND flow_version <> ?1 ORDER BY flow_version, at",
        &[&e.flow_now],
        1_000,
    )?;
    let mut older: BTreeMap<i64, Map<String, Value>> = BTreeMap::new();
    for r in stand {
        let (version, at) = (r["flow_version"].as_i64().unwrap_or_default(), r["at"].as_str().unwrap_or_default());
        let entry = match e.flow(version) {
            Ok(f) => match f.graph(ENGINEER).and_then(|g| g.cell(at).map(|c| cell_entry(c, &g.end))) {
                Some(c) => c,
                None => json!({ "unknown": "that flow version has no such cell" }),
            },
            Err(why) => json!({ "unknown": why }),
        };
        older.entry(version).or_default().insert(at.to_string(), entry);
    }
    if !older.is_empty() {
        let older: Map<String, Value> = older
            .into_iter()
            .map(|(v, cells)| (v.to_string(), Value::Object(cells)))
            .collect();
        out.insert("older_flow_versions".into(), Value::Object(older));
    }
    Ok(Value::Object(out))
}

/// The most observations a batch's input names in full.
const NAMED_MOST: usize = 12;

/// The time the batch of an architect's work began: when the rethink that ended the batch before it ended, or else when
/// the work began.
pub(crate) fn batch_since(conn: &Connection, work: i64, from: i64) -> Result<i64, String> {
    conn.query_row(
        "SELECT coalesce((SELECT ended_at FROM rounds WHERE work = ?1 AND n = ?2),
                         (SELECT min(started_at) FROM rounds WHERE work = ?1), 0)",
        params![work, from],
        |r| r.get(0),
    )
    .map_err(sql)
}

/// The changes to observations the rounds of an architect work's batch handed in, with the round of each, oldest first:
/// from the effects of an answer on a matter's path and from a revision of the batch, whose changes stand at another
/// place of the return. A return sent back had no effect and is left out.
fn observation_changes(conn: &Connection, work: i64, from: i64) -> Result<Vec<(i64, Value)>, String> {
    let rows = query_json(
        conn,
        "SELECT round, change FROM (
           SELECT r.n AS round, o.value AS change, o.id AS at
           FROM rounds r, json_each(r.output, '$.outcome.effects.observation_changes') o
           WHERE r.work = ?1 AND r.n > ?2 AND r.transition IS NOT NULL
           UNION ALL
           SELECT r.n, o.value, o.id
           FROM rounds r, json_each(r.output, '$.outcome.observation_changes') o
           WHERE r.work = ?1 AND r.n > ?2 AND r.transition IS NOT NULL)
         ORDER BY round, at",
        &[&work, &from],
        10_000,
    )?;
    Ok(rows
        .into_iter()
        .filter_map(|r| Some((r["round"].as_i64()?, r["change"].clone())))
        .collect())
}

/// The changes to observations this batch made so far, each with the round that handed it in and the matter it comes
/// of (the one it names, or else the matter picked then), oldest first: what a review of the batch's item changes
/// weighs them against, since an observation change is made at once and is no item change of the batch. None when the
/// batch made none. A change of an earlier batch is not among them.
pub(crate) fn batch_observation_changes(conn: &Connection, work: i64) -> Result<Option<Value>, String> {
    let from = batch_start(conn, work)?;
    let mut out = Vec::new();
    for (round, change) in observation_changes(conn, work, from)? {
        let matter = match change["matter"].as_i64() {
            Some(m) => Some(m),
            None => picked_before(conn, work, Some(round))?,
        };
        out.push(json!({ "round": format!("R{work}-{round}"), "matter": matter, "change": change }));
    }
    Ok((!out.is_empty()).then_some(Value::Array(out)))
}

/// Why an observation is among those a batch names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Named {
    MergedInto,
    Closed,
    FoldedIn,
    Recorded,
    Found,
}

impl Named {
    fn words(self) -> &'static str {
        match self {
            Named::MergedInto => "a report of this batch was merged into it",
            Named::Closed => "this batch closed it",
            Named::FoldedIn => "a change of this batch folds it into an item",
            Named::Recorded => "this batch recorded it",
            Named::Found => "a search of this batch found it",
        }
    }
}

/// The observations the batch of an architect's work names, each in full with how many times it was reported: the ones
/// it merged a report into or closed, those a collected change folds into an item, those it recorded, and those a search
/// in it found, in that order, at most [`NAMED_MOST`], with how many more it names (`left_out`). A merge counts a report
/// once more, and a third report is work to take in an item: the reviewing and recording cells cannot tell from a
/// summary. The numbers are the values when the input is read. None when it names none.
pub(crate) fn observations_named(conn: &Connection, work: i64) -> Result<Option<Value>, String> {
    let project: Option<String> = conn
        .query_row("SELECT project FROM works WHERE id = ?1", [work], |r| r.get(0))
        .map_err(sql)?;
    let Some(project) = project else {
        return Ok(None);
    };
    let from = batch_start(conn, work)?;
    let since = batch_since(conn, work, from)?;
    let mut named: Vec<(i64, Named)> = Vec::new();
    let mut add = |id: Option<i64>, why: Named| {
        if let Some(id) = id {
            named.push((id, why));
        }
    };
    for (_, c) in observation_changes(conn, work, from)? {
        match c["kind"].as_str() {
            Some("merge") => add(c["into"].as_i64(), Named::MergedInto),
            Some("close") => add(c["id"].as_i64(), Named::Closed),
            _ => {}
        }
    }
    let folded = query_json(
        conn,
        "SELECT o.value AS observation FROM architect_item_changes c, json_each(c.change, '$.observations') o
         WHERE c.work = ?1 AND c.state = 'collected' AND json_extract(c.change, '$.kind') = 'fold_observations'
         ORDER BY c.id",
        &[&work],
        10_000,
    )?;
    for r in &folded {
        let mut ids = BTreeSet::new();
        ids_of(&r["observation"], &mut ids);
        for id in ids {
            add(Some(id), Named::FoldedIn);
        }
    }
    let recorded = query_json(
        conn,
        "SELECT json_extract(row_key, '$.id') AS id FROM changes
         WHERE table_name = 'observations' AND old_row IS NULL AND at >= ?1 ORDER BY id",
        &[&since],
        10_000,
    )?;
    for r in &recorded {
        add(r["id"].as_i64(), Named::Recorded);
    }
    let found = query_json(
        conn,
        &format!(
            "SELECT json_extract(c.value, '$.id') AS id FROM rounds r, json_each(r.output, '$.outcome.candidates') c
             WHERE r.work = ?1 AND r.n > ?2 AND r.cell IN {} AND r.rejected IS NULL ORDER BY r.n DESC, c.id",
            db::cells_sql(crate::ARCHITECT, &["X2"])
        ),
        &[&work, &from],
        10_000,
    )?;
    for r in &found {
        add(r["id"].as_i64(), Named::Found);
    }
    // Each observation once, under the first reason, and then the others it is named for.
    let mut order: Vec<i64> = Vec::new();
    let mut why: BTreeMap<i64, BTreeSet<Named>> = BTreeMap::new();
    for (id, w) in named {
        if !order.contains(&id) {
            order.push(id);
        }
        why.entry(id).or_default().insert(w);
    }
    order.sort_by_key(|id| why[id].iter().next().copied());
    let mut shown = Map::new();
    let mut skipped: Vec<i64> = Vec::new();
    for id in &order {
        if shown.len() >= NAMED_MOST {
            skipped.push(*id);
            continue;
        }
        let rows = query_json(
            conn,
            "SELECT title, what, tags, times_reported, source, refs, folded_into, closed, closed_why
             FROM observations_all WHERE project = ?1 AND id = ?2",
            &[&project, id],
            1,
        )?;
        let Some(mut row) = rows.into_iter().next() else {
            continue;
        };
        if let Some(o) = row.as_object_mut() {
            o.retain(|_, v| !v.is_null() && *v != json!([]));
            o.insert("named_because".into(), json!(why[id].iter().map(|w| w.words()).collect::<Vec<_>>()));
        }
        shown.insert(format!("O{id}"), row);
    }
    if shown.is_empty() {
        return Ok(None);
    }
    let mut out = json!({ "observations": shown });
    if !skipped.is_empty() {
        out["left_out"] = json!({
            "count": skipped.len(),
            "observations": skipped.iter().map(|id| format!("O{id}")).collect::<Vec<_>>(),
        });
    }
    Ok(Some(out))
}

/// The design an engineer's open question comes of, whole, as the return of the round that handed it in holds it: the
/// matter the architect is answering, when it is such a question. The same bytes for every question of the one design,
/// so that a session given the first is only referred to it for the others. None for any other matter.
pub(crate) fn design_asked(conn: &Connection, matter: i64) -> Result<Option<Value>, String> {
    let row = query_json(
        conn,
        "SELECT kind, work, round FROM architect_matters WHERE id = ?1",
        &[&matter],
        1,
    )?;
    let Some(r) = row.first() else {
        return Ok(None);
    };
    if r["kind"] != crate::open_questions::KIND {
        return Ok(None);
    }
    let (Some(work), Some(round)) = (r["work"].as_i64(), r["round"].as_i64()) else {
        return Ok(None);
    };
    let design = query_json(
        conn,
        "SELECT json_extract(output, '$.outcome') AS design FROM rounds WHERE work = ?1 AND n = ?2",
        &[&work, &round],
        1,
    )?;
    Ok(design.first().map(|r| r["design"].clone()).filter(|d| !d.is_null()))
}
