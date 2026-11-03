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
        let (version, at) = (
            r["flow_version"].as_i64().unwrap_or_default(),
            r["at"].as_str().unwrap_or_default(),
        );
        let entry = match e.flow(version) {
            Ok(f) => match f
                .graph(ENGINEER)
                .and_then(|g| g.cell(at).map(|c| cell_entry(c, &g.end)))
            {
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
            o.insert(
                "named_because".into(),
                json!(why[id].iter().map(|w| w.words()).collect::<Vec<_>>()),
            );
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::process::Command;
    use std::sync::OnceLock;

    use db::rusqlite::Transaction;
    use db::{Actor, Db};
    use flow::format::Src;

    use super::*;

    /// Inserts a round of work 1 that ended at `at`, with the row it took.
    fn round(
        tx: &Transaction<'_>,
        n: i64,
        cell: &str,
        output: &Value,
        transition: Option<&str>,
        at: i64,
    ) -> Result<(), db::Error> {
        tx.execute(
            "INSERT INTO rounds (work, n, cell, input, output, transition, config_commit, started_at, ended_at)
             VALUES (1, ?1, ?2, '{}', ?3, ?4, 'c', ?5, ?5)",
            params![n, cell, output.to_string(), transition, at],
        )?;
        Ok(())
    }

    fn observation(tx: &Transaction<'_>, id: i64, project: &str, what: &str) -> Result<(), db::Error> {
        tx.execute(
            "INSERT INTO observations (id, project, title, what, tags, source, refs)
             VALUES (?1, ?2, ?3, ?4, '[\"perf\"]', 'a test', '[]')",
            params![id, project, format!("Observation {id}"), what],
        )?;
        Ok(())
    }

    /// A database with a work of project p's architect, whose batch (after its rethink, round 3) holds the rounds that
    /// name observations: round 5 merges a report into 15 and closes 3, rounds 6 and 7 are searches, and a change
    /// collected in round 5 folds 7 and 40 into an item; a batch before the rethink merged into 99 and found 98. The
    /// rethink ended in the future, so that no observation recorded by the test is recorded in the batch.
    fn batch_db() -> Db {
        let mut db = Db::in_memory().unwrap();
        db.write(Actor::Engine, |tx| {
            tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
            tx.execute(
                "INSERT INTO works (flow_version, graph, at, project) VALUES (1, 'architect', 'B2', 'p')",
                [],
            )?;
            for m in 1..=2 {
                tx.execute(
                    "INSERT INTO architect_matters (project, kind, what, body, at) VALUES ('p', 'report', 'w', '{}', ?1)",
                    [m],
                )?;
            }
            let merge = |into: i64| json!({ "kind": "merge", "into": into, "seen": "", "refs": [], "reason": "r" });
            let effects = |changes: Value| json!({ "outcome": { "effects": { "observation_changes": changes } } });
            let found = |ids: Vec<i64>| {
                let candidates: Vec<Value> = ids.iter().map(|i| json!({ "id": i, "why": "w" })).collect();
                json!({ "outcome": { "candidates": candidates } })
            };
            round(tx, 1, "H5", &effects(json!([merge(99)])), Some("done"), 1_000)?;
            round(tx, 2, "X2", &found(vec![98]), None, 1_001)?;
            round(tx, 3, "B4", &json!({}), Some("done"), db::now() + 3_600_000)?;
            round(tx, 4, "Q0", &json!({ "queue": { "verdict": "one", "matter": 1 } }), Some("one"), 1_002)?;
            let close = json!({ "kind": "close", "id": 3, "why": "folded", "text": "An item took it.", "matter": 2,
                                "reason": "r" });
            round(tx, 5, "H5", &effects(json!([merge(15), close])), Some("done"), 1_003)?;
            round(tx, 6, "X2", &found(vec![7, 15]), None, 1_004)?;
            round(tx, 7, "X2", &found((20..=33).collect()), None, 1_005)?;
            let fold = json!({ "kind": "fold_observations", "item": "12", "observations": [7, 40], "reason": "r" });
            tx.execute(
                "INSERT INTO architect_item_changes (work, round, matter, change, state)
                 VALUES (1, 5, 1, ?1, 'collected')",
                [fold.to_string()],
            )?;
            let long = "The slot count is stale. ".repeat(20);
            for id in [3, 7, 15, 40, 98, 99].into_iter().chain(20..=33) {
                observation(tx, id, "p", &long)?;
            }
            tx.execute("UPDATE observations SET times_reported = 3 WHERE id = 15", [])?;
            Ok(())
        })
        .unwrap();
        db
    }

    // The observations a batch names are those it merged a report into or closed, those a collected change folds into an
    // item, and those a search in it found, each in full with how many times it was reported; at most twelve, the
    // named ones before the found, and the rest counted. What an earlier batch named is not among them.
    #[test]
    fn the_observations_a_batch_names_come_in_full_with_how_many_times_they_were_reported() {
        let db = batch_db();
        let named = observations_named(db.conn(), 1).unwrap().expect("some are named");
        let mut shown: Vec<String> = named["observations"].as_object().unwrap().keys().cloned().collect();
        shown.sort();
        let mut want: Vec<String> = ["O15", "O3", "O7", "O40"].iter().map(|s| s.to_string()).collect();
        want.extend((20..=27).map(|i| format!("O{i}")));
        want.sort();
        assert_eq!(shown, want, "{named}");
        let o15 = &named["observations"]["O15"];
        assert_eq!(o15["times_reported"], 3, "{o15}");
        assert_eq!(o15["what"], "The slot count is stale. ".repeat(20), "the whole text");
        assert_eq!(
            o15["named_because"],
            json!(["a report of this batch was merged into it", "a search of this batch found it"]),
            "{o15}"
        );
        assert_eq!(
            named["observations"]["O7"]["named_because"],
            json!(["a change of this batch folds it into an item", "a search of this batch found it"])
        );
        assert_eq!(named["left_out"]["count"], 6, "{named}");
        assert_eq!(named["left_out"]["observations"][0], "O28");
        assert!(named["observations"].get("O99").is_none() && named["observations"].get("O98").is_none());
    }

    // An observation the batch recorded is named, once recorded since the batch began; another project's is not.
    #[test]
    fn an_observation_recorded_in_the_batch_is_named_and_another_project_s_is_not() {
        let mut db = Db::in_memory().unwrap();
        db.write(Actor::Engine, |tx| {
            tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
            tx.execute(
                "INSERT INTO works (flow_version, graph, at, project) VALUES (1, 'architect', 'H5', 'p')",
                [],
            )?;
            round(tx, 1, "H5", &json!({}), Some("done"), db::now() - 600_000)?;
            observation(tx, 1, "p", "Recorded by the batch.")?;
            observation(tx, 2, "q", "Another project's.")?;
            Ok(())
        })
        .unwrap();
        let named = observations_named(db.conn(), 1).unwrap().expect("one is named");
        let keys: Vec<&String> = named["observations"].as_object().unwrap().keys().collect();
        assert_eq!(keys, ["O1"], "{named}");
        assert_eq!(
            named["observations"]["O1"]["named_because"],
            json!(["this batch recorded it"])
        );
        assert!(named.get("left_out").is_none());
        // A work that names none gives none.
        let empty = Db::in_memory().unwrap();
        empty
            .conn()
            .execute_batch(
                "INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}');
                 INSERT INTO works (flow_version, graph, at, project) VALUES (1, 'architect', 'H5', 'p');",
            )
            .unwrap();
        assert_eq!(observations_named(empty.conn(), 1).unwrap(), None);
    }

    // The changes to observations a batch made are those of its own rounds, each with its round and the matter it comes
    // of: the one it names, else the one picked then. An earlier batch's, before the rethink, are not among them.
    #[test]
    fn the_batch_s_changes_to_observations_come_with_their_round_and_matter_and_not_the_last_batch_s() {
        let db = batch_db();
        let changes = batch_observation_changes(db.conn(), 1).unwrap().expect("two were made");
        let list = changes.as_array().unwrap();
        assert_eq!(list.len(), 2, "{changes}");
        let first = (&list[0]["round"], &list[0]["matter"], &list[0]["change"]["into"]);
        let second = (&list[1]["round"], &list[1]["matter"], &list[1]["change"]["id"]);
        assert_eq!(first, (&json!("R1-5"), &json!(1), &json!(15)));
        assert_eq!(second, (&json!("R1-5"), &json!(2), &json!(3)));
        // A batch that made none gives none.
        let none = Db::in_memory().unwrap();
        none.conn()
            .execute_batch(
                "INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}');
                 INSERT INTO works (flow_version, graph, at, project) VALUES (1, 'architect', 'H5', 'p');",
            )
            .unwrap();
        assert_eq!(batch_observation_changes(none.conn(), 1).unwrap(), None);
    }

    // Every question of one design is given the same design, whole, as the round that handed the questions in returned
    // it; a matter that is no open question has none.
    #[test]
    fn two_questions_of_one_design_are_given_the_same_design() {
        let mut db = Db::in_memory().unwrap();
        let design = json!({
            "chosen": "B",
            "design": { "solution": "S", "where": "W" },
            "open_questions": [{ "question": "Q1" }, { "question": "Q2" }],
        });
        db.write(Actor::Engine, |tx| {
            tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
            tx.execute("INSERT INTO works (flow_version, graph, at) VALUES (1, 'engineer', 'D2')", [])?;
            round(tx, 2, "D2", &json!({ "outcome": design }), Some("done"), 1)?;
            for (kind, what) in [("open_question", "Q1"), ("open_question", "Q2"), ("failure", "F")] {
                tx.execute(
                    "INSERT INTO architect_matters (project, kind, what, body, work, round, at)
                     VALUES ('p', ?1, ?2, '{}', 1, 2, 1)",
                    [kind, what],
                )?;
            }
            Ok(())
        })
        .unwrap();
        let first = design_asked(db.conn(), 1).unwrap();
        assert_eq!(first, Some(design.clone()));
        assert_eq!(
            design_asked(db.conn(), 2).unwrap().map(|d| d.to_string()),
            first.map(|d| d.to_string())
        );
        assert_eq!(design_asked(db.conn(), 3).unwrap(), None);
        assert_eq!(design_asked(db.conn(), 9).unwrap(), None);
    }

    fn card_of(db: &Db, work: i64, holding: &Holding) -> Map<String, Value> {
        card(db.conn(), work, holding).unwrap()
    }

    // A work's card is its branch, its worktree, its pull request's number and what the engine recorded of that pull
    // request (whether it merged, the head last pushed and the latest runs of the fast and the full CI), and whether it
    // holds a slot; a work with no pull request has no state of one, and a work there is not has no card.
    #[test]
    fn a_work_s_card_is_what_the_works_table_and_the_engine_recorded_of_its_pull_request() {
        let mut db = Db::in_memory().unwrap();
        db.write(Actor::Engine, |tx| {
            tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
            tx.execute(
                "INSERT INTO works (flow_version, graph, at, branch, worktree, pr, pushed_head)
                 VALUES (1, 'engineer', 'T3', 'wf-agents/7-1', '/w/7', 12, 'abc')",
                [],
            )?;
            tx.execute(
                "INSERT INTO works (flow_version, graph, at, branch) VALUES (1, 'engineer', 'I1', 'wf-agents/8-2')",
                [],
            )?;
            round(tx, 1, "M2", &json!({}), Some("merged"), 1)?;
            let checks = |test: &str| {
                json!({ "push": { "checks": [{ "name": "lint", "result": "green" }, { "name": "test", "result": test }] } })
            };
            let full = json!({ "result": { "kind": "red", "checks": [] } });
            for (tool, head, result) in [
                ("ci", "old", checks("red")),
                ("ci", "abc", checks("green")),
                ("full_ci", "abc", full),
            ] {
                tx.execute(
                    "INSERT INTO tool_runs (work, round, tool, args, attempt, result, queued_at)
                     VALUES (1, 1, ?1, ?2, 1, ?3, 1)",
                    params![tool, json!({ "head": head }).to_string(), result.to_string()],
                )?;
            }
            Ok(())
        })
        .unwrap();
        let holding = Holding::new([(1, None, "T3".to_string(), Some("m".to_string()))]);
        assert_eq!(
            Value::Object(card_of(&db, 1, &holding)),
            json!({
                "branch": "wf-agents/7-1", "worktree": "/w/7", "pr": 12, "holds_slot": true,
                "pr_state": {
                    "merged": true, "pushed_head": "abc",
                    "fast_ci": { "head": "abc", "result": "green" },
                    "full_ci": { "head": "abc", "result": "red" },
                },
            })
        );
        assert_eq!(
            Value::Object(card_of(&db, 2, &holding)),
            json!({ "branch": "wf-agents/8-2", "holds_slot": false })
        );
        assert_eq!(card_of(&db, 99, &holding), Map::new());
        assert_eq!(holding.count(), 1);
        assert_eq!(holding.listed(), vec![json!({ "work": 1, "at": "T3", "machine": "m" })]);
    }

    fn cell(v: Value) -> CompiledCell {
        serde_json::from_value(v).unwrap()
    }

    // A cell is described by its name, kind and stage and by whether a work standing at it holds a slot: not at the
    // end, not at a cell that asks the owner or the architect, and at a cell with a row that waits only while it
    // waits at that row. Four cells, counted by hand.
    #[test]
    fn a_cell_is_told_with_whether_a_work_standing_at_it_holds_a_slot() {
        let row = |name: &str, waits: bool| {
            json!({ "name": name, "if": [], "to": "E1", "waits": waits, "interject": false, "main": false, "common": false })
        };
        let base = |id: &str, kind: &str, title: &str, rows: Vec<Value>| {
            json!({ "id": id, "kind": kind, "gate": false, "title": title, "inputs": [], "rows": rows })
        };
        let mut implement = base("I1", "agent", "Implement", vec![row("done", false)]);
        implement["stage"] = json!("build");
        let mut ask = base("A2", "engine", "To the architect", vec![row("answered", false)]);
        ask["step"] = json!("ask_architect");
        let wait = base("W1", "engine", "Wait", vec![row("ready", false), row("wait", true)]);
        let end = base("E1", "engine", "End", vec![]);
        let told = |c: Value| cell_entry(&cell(c), "E1");
        assert_eq!(
            told(implement),
            json!({ "title": "Implement", "kind": "agent", "stage": "build", "holds_slot": "yes" })
        );
        assert_eq!(
            told(ask),
            json!({ "title": "To the architect", "kind": "engine", "holds_slot": "no: it waits for the architect's reply" })
        );
        assert_eq!(
            told(wait)["holds_slot"],
            "yes, but not while it waits at the row wait for what it waits for"
        );
        assert_eq!(told(end)["holds_slot"], "no: the work is over");
    }

    fn architect_graph() -> &'static flow::CompiledGraph {
        static FLOW: OnceLock<flow::Compiled> = OnceLock::new();
        FLOW.get_or_init(|| {
            let relations = db::relations(db::SCHEMA).unwrap();
            let tables = db::index_of(db::SCHEMA).unwrap();
            flow::compile(flow::FLOW, &flow::Context { relations, tables }).unwrap()
        })
        .graph(crate::ARCHITECT)
        .unwrap()
    }

    /// Whether a JSON Schema, anywhere in it, has a property of this name.
    fn mentions(v: &Value, name: &str) -> bool {
        match v {
            Value::Object(o) => o.contains_key(name) || o.values().any(|x| mentions(x, name)),
            Value::Array(xs) => xs.iter().any(|x| mentions(x, name)),
            _ => false,
        }
    }

    fn declares(c: &CompiledCell, input: &str) -> bool {
        c.inputs
            .iter()
            .any(|x| x.from.iter().any(|s| matches!(s, Src::Engine(n) if n == input)))
    }

    // A cell of the architect that can hand in changes to observations is given the observations the batch names, in full,
    // and so is the review of the batch's item changes, which weighs them; the review and the revision are given the
    // changes to observations the batch made. The cells that can are found from their return, not from a list.
    #[test]
    fn every_cell_that_can_change_observations_is_given_the_observations_the_batch_names() {
        let g = architect_graph();
        let can = |c: &CompiledCell| c.schema.as_ref().is_some_and(|s| mentions(s, "observation_changes"));
        // The search sees the cells that can: a taking in of reports and the revision of a batch do, a pick does not.
        assert!(can(g.cell("H5").unwrap()) && can(g.cell("B3").unwrap()) && !can(g.cell("Q1").unwrap()));
        for c in g.cells.iter().filter(|c| c.kind == "agent" && can(c)) {
            assert!(declares(c, "observations_named"), "{} can change observations", c.id);
        }
        let b2 = g.cell("B2").unwrap();
        assert!(declares(b2, "observations_named") && declares(b2, "batch_observation_changes"));
        assert!(declares(g.cell("B3").unwrap(), "batch_observation_changes"));
        assert!(declares(g.cell("H2").unwrap(), "design_asked") && declares(g.cell("H3").unwrap(), "design_asked"));
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("wf-agents-arch-inputs-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn git_in(dir: &Path, args: &[&str]) {
        let out = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["-c", "user.name=t", "-c", "user.email=t@t", "-c", "init.defaultBranch=main"])
            .args(["-c", "commit.gpgsign=false"])
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    // A worktree tells what a branch changes against main, committed files only, and what is not committed: a changed
    // file, an untracked one and a rename (whose old path is no entry of its own); git failing is told, not hidden.
    #[test]
    fn a_worktree_tells_the_files_its_branch_changes_and_the_files_not_committed() {
        let dir = scratch("git");
        git_in(&dir, &["init", "-q"]);
        for f in ["a.txt", "e.txt"] {
            std::fs::write(dir.join(f), "one\n").unwrap();
        }
        git_in(&dir, &["add", "-A"]);
        git_in(&dir, &["commit", "-q", "-m", "base"]);
        git_in(&dir, &["update-ref", "refs/remotes/origin/main", "main"]);
        git_in(&dir, &["checkout", "-q", "-b", "work"]);
        std::fs::write(dir.join("b.txt"), "two\n").unwrap();
        git_in(&dir, &["add", "-A"]);
        git_in(&dir, &["commit", "-q", "-m", "work"]);
        assert_eq!(branch_files(&dir, "main"), json!({ "total": 1, "files": ["b.txt"] }));
        assert_eq!(uncommitted(&dir), json!([]));
        std::fs::write(dir.join("e.txt"), "changed\n").unwrap();
        std::fs::write(dir.join("c.txt"), "new\n").unwrap();
        git_in(&dir, &["mv", "a.txt", "d.txt"]);
        let mut files: Vec<String> = uncommitted(&dir)
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f.as_str().unwrap().to_string())
            .collect();
        files.sort();
        assert_eq!(files, [" M e.txt", "?? c.txt", "R  d.txt"]);
        // Not a repository: told, with the reason.
        let nowhere = scratch("none").join("missing");
        assert!(branch_files(&nowhere, "main")["unavailable"].is_string());
        assert!(uncommitted(&nowhere)["unavailable"].is_string());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
