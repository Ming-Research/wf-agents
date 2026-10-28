//! The architects' side of the engine. Each project has its architect; a wake is a work of the architect's graph for a
//! project, on no item: the engine starts one when none of the project runs and the project's queue has a matter, one
//! at a time for each project, and a wake holds no slot. What puts a matter in a project's queue is here and in the
//! actions: an engineer's question or failure, a side report, a report's what to do next, a design card, the owner's
//! words and answers, another project's architect's message, slots free with nothing of the project ready, and a place
//! the understanding found to set straight. Here too are the steps, inputs, values, checks and actions of the
//! architect's graph, its asks of the owner, and the batch of item changes a review looks at before they are made. A
//! project's architect's line goes on in one CLI session across the project's wakes until a new policy ends it.
//!
//! The engine knows a few cells of the architect's graph by their ids, as the views over its rounds do: the look at
//! the queue (D0) and the pick (D1), which say the matter picked; the understanding (U1); the rethink after a batch
//! (C1), which ends a batch; the review of the batch (R1); and the first and deciding cell of each kind's path
//! ([`KINDS`]).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use db::rusqlite::{Connection, OptionalExtension, Transaction, params};
use db::{Actor, Matter, queue_matter};
use flow::format::Src;
use flow::{CompiledCell, CompiledGraph};
use serde_json::{Map, Value, json};

use crate::actions::{self, Ctx};
use crate::checks;
use crate::inputs::{self, Gathered, Origin, Part, at_path};
use crate::steps::{finish, what_is_asked};
use crate::work::{self, Ended, Item, Round, RoundId, Work, json_col, query_json};
use crate::{ARCHITECT, Advance, ENGINEER, Engine, git, values};

/// An hour, in milliseconds: the least time between two matters of slots free with nothing ready for one project.
pub const HOUR: i64 = 3_600_000;

/// The engine's inputs the architect's cells read, each computed by [`input`], but [`BEFORE`], which [`before`] gives,
/// [`STAYS`], and those of the policy being set, which [`draft_places`] names the places of.
pub const INPUTS: [&str; 21] = [
    "constitution",
    "guide",
    "cited_nodes",
    "since_last_wake",
    "board",
    "numbers",
    "observations_overview",
    "observation_tags",
    "policy",
    "policy_signals",
    "slots",
    "queue",
    "batch",
    "matter",
    "matter_items",
    "item_log",
    "similar_items",
    "owner_answer",
    "cross_project",
    "batch_changes",
    "changes_touch",
];

/// The input that gives what the cell before this one on the matter's path answered of it, as that round's return:
/// addressed as that return, so that a session that holds it is only referred to it.
pub const BEFORE: &str = "before";

/// The input that gives the state of things as the architect last wrote it: when the wake's understanding (U1) wrote
/// it, as the places of that round's return, which the wake's session holds, with what was learned and noted since read
/// from `architect_state`; otherwise that view whole.
pub const STATE: &str = "state_of_things";

/// The input that gives the study of a new policy and the owner's answer to it, as places of the returns of P1 and P2,
/// when the owner's latest answer to a study in the wake said the policy in force stays; none otherwise.
pub const STAYS: &str = "policy_stays";

/// The places of the returns [`STAYS`] gives.
pub const STAYS_PLACES: [(&str, &str); 2] = [("P1", "study"), ("P2", "ruling")];

/// Whether the owner's latest answer in the wake to a study of a new policy (P2) said the policy in force stays.
pub fn policy_stays(e: &Engine, w: &Work, g: &CompiledGraph) -> Result<bool, String> {
    let latest = inputs::returns_holding(e, w, g, "P2", "ruling")?.into_iter().next();
    Ok(latest.is_some_and(|(_, r)| r["ruling"]["choice"] == "policy_stays"))
}

/// The inputs that give the policy being set, its text and its probes, as the wake's round that last wrote (P3) or
/// corrected (P7) them handed them in, each with the places of those cells' returns it is at: addressed as that return,
/// so that the session that wrote it is only referred to it.
const DRAFT: [(&str, [(&str, &str); 2]); 2] = [
    ("draft_text", [("P3", "policy"), ("P7", "revision.policy")]),
    ("draft_probes", [("P3", "probes"), ("P7", "revision.probes")]),
];

/// The places of the returns an input of the policy being set is taken from, the newest of them; none for another
/// input.
pub fn draft_places(name: &str) -> Option<&'static [(&'static str, &'static str); 2]> {
    DRAFT.iter().find(|d| d.0 == name).map(|d| &d.1)
}

/// Each kind of matter with the first cell of its path and the cell that decides it, where the matter goes back to when
/// the owner is asked about item changes of it the review dropped.
pub const KINDS: [(&str, &str, &str); 8] = [
    ("set_straight", "T1", "T1"),
    ("engineer_question", "Q1", "Q2"),
    ("report", "I1", "I2"),
    ("design_card", "K1", "K1"),
    ("failure", "F1", "F1"),
    ("owner_words", "O1", "O1"),
    ("other_architect", "M1", "M1"),
    ("slots_free", "N1", "N1"),
];

/// The kinds of point an engineer's question is queued with as its detail; a question of any other kind is an
/// other_question.
const QUESTION_KINDS: [&str; 6] = [
    "split",
    "duplicate",
    "depends_on_other_item",
    "out_of_scope_defect",
    "root_elsewhere",
    "too_costly",
];

/// The cells the engine knows by their ids: the look at the queue, the pick, the understanding, the rethink after a
/// batch, and the review of the batch.
const LOOK: &str = "D0";
const PICK: &str = "D1";
const UNDERSTAND: &str = "U1";
const RETHINK: &str = "C1";
const REVIEW: &str = "R1";

/// The cell of the engineer's graph a work is done at, as the views read it.
const ENGINEER_END: &str = "20";

/// The most rows an input of the architect's gives of one list.
const ROWS: usize = 200;

/// The most matters a batch handles before its item changes are reviewed.
const BATCH: i64 = 20;

/// The rows a round of the architect's cells takes when it hands in no answer of its own, as the views say it: it
/// asked for tools or a search, paused, or asked the owner.
const NO_ANSWER: &str = "('tools', 'search', 'pause', 'ask_owner')";

fn sql(e: impl std::fmt::Display) -> String {
    e.to_string()
}

fn count(conn: &Connection, q: &str, args: &[&dyn db::rusqlite::ToSql]) -> Result<i64, String> {
    conn.query_row(q, args, |r| r.get(0)).map_err(sql)
}

/// The items of a list, a value that is no list being one item and none being none.
fn items(v: Option<&Value>) -> Vec<Value> {
    match v {
        Some(Value::Array(xs)) => xs.clone(),
        Some(Value::Null) | None => Vec::new(),
        Some(other) => vec![other.clone()],
    }
}

/// Whether a value has nothing in it.
fn empty(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::String(s) => s.trim().is_empty(),
        Value::Array(xs) => xs.is_empty(),
        Value::Object(o) => o.is_empty(),
        _ => false,
    }
}

fn text(v: &Value) -> &str {
    v.as_str().unwrap_or_default()
}

/// A time to the minute, in UTC, as `2026-10-28 14:03 UTC`.
fn utc_minute(ms: i64) -> String {
    crate::utc(ms)
}

// Where the architect works.

/// A project's architect's working directory: `worktrees/architect/<project>` in the data directory, which its git
/// leaves out.
pub fn workdir(data: &Path, project: &str) -> PathBuf {
    config::worktrees(data).join(ARCHITECT).join(project)
}

/// A project's architect's working directory, made when it is not there yet.
pub fn made_workdir(data: &Path, project: &str) -> Result<PathBuf, String> {
    let dir = workdir(data, project);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    Ok(dir)
}

/// The project whose architect a wake is.
pub fn project_of(w: &Work) -> Result<&str, String> {
    w.project
        .as_deref()
        .ok_or_else(|| format!("work {} is a wake of no project's architect", w.id))
}

// Wakes.

/// The wake of a project's architect that runs: the project's newest wake not stopped, unless it reached its end. A
/// wake starts only while none of its project runs, so an older one has ended or reached its end. A paused wake still
/// runs.
pub fn running(e: &mut Engine, project: &str) -> Result<Option<i64>, String> {
    let newest: Option<i64> =
        e.db.conn()
            .query_row(
                "SELECT max(id) FROM works WHERE graph = 'architect' AND project = ?1 AND ended IS NULL",
                [project],
                |r| r.get(0),
            )
            .map_err(sql)?;
    let Some(id) = newest else { return Ok(None) };
    let w = e.work(id)?;
    Ok((!e.done(&w)?).then_some(id))
}

/// Whether a project's queue has a matter.
pub fn queue_has(e: &Engine, project: &str) -> Result<bool, String> {
    Ok(count(
        e.db.conn(),
        "SELECT count(*) FROM architect_queue WHERE project = ?1",
        &[&project],
    )? > 0)
}

/// Starts a wake of a project's architect: a work of the architect's graph for the project, on no item, at its start,
/// on the flow version new works start on; paused by the owner when the owner's step started it.
pub fn start(e: &mut Engine, project: &str, paused: bool) -> Result<i64, String> {
    let v = e.flow_now;
    let start = e
        .flow(v)?
        .graph(ARCHITECT)
        .map(|g| g.start.clone())
        .ok_or("the flow has no architect's graph")?;
    let paused = paused.then_some(db::PAUSED_BY_OWNER);
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO works (item, flow_version, graph, at, paused, paused_by, project)
             VALUES (NULL, ?1, ?2, ?3, ?4, CASE WHEN ?4 IS NULL THEN NULL ELSE 'owner' END, ?5)",
            params![v, ARCHITECT, start, paused, project],
        )?;
        Ok(tx.last_insert_rowid())
    })
    .map_err(sql)
}

/// Queues what the engine raises itself once it is due: for each project with nothing ready to start and no hold on
/// its new starts, while slots are free, a matter of slots free, when none waits in its queue and the last was handled
/// more than an hour ago or never; the hour keeps one that found nothing to start from coming back at once.
pub fn queue_due(e: &mut Engine) -> Result<(), String> {
    let free = e.free_slots()?;
    if free <= 0 {
        return Ok(());
    }
    let projects: Vec<String> = e.config.config.projects.keys().cloned().collect();
    for p in projects {
        let conn = e.db.conn();
        let ready = count(conn, "SELECT count(*) FROM ready_items WHERE project = ?1", &[&p])?;
        let held = count(conn, "SELECT count(*) FROM dispatch_holds WHERE project = ?1", &[&p])?;
        let waiting = count(
            conn,
            "SELECT count(*) FROM architect_matters WHERE project = ?1 AND kind = 'slots_free' AND handled_round IS NULL",
            &[&p],
        )?;
        if ready > 0 || held > 0 || waiting > 0 {
            continue;
        }
        let last: Option<i64> = conn
            .query_row(
                "SELECT max(r.ended_at) FROM architect_matters m
                 JOIN rounds r ON r.work = m.handled_work AND r.n = m.handled_round
                 WHERE m.project = ?1 AND m.kind = 'slots_free'",
                [&p],
                |r| r.get(0),
            )
            .map_err(sql)?;
        if last.is_some_and(|t| db::now() - t <= HOUR) {
            continue;
        }
        let m = Matter {
            project: p.clone(),
            kind: "slots_free".into(),
            what: format!("{free} slots are free, and no item of {p} is ready to start"),
            body: json!({ "free": free }),
            ..Matter::default()
        };
        e.db.write(Actor::Engine, |tx| queue_matter(tx, &m)).map_err(sql)?;
    }
    Ok(())
}

/// Cell W0: the wake goes on in the CLI session the project's last wake left, unless a new policy ended it: its line's
/// session is a row of this wake's with the same CLI session, which the next round of the line resumes.
pub fn wake(e: &mut Engine, w: &Work) -> Result<Value, String> {
    let id = w.id;
    e.db.write(Actor::Engine, |tx| {
        sessions::choose::wake(tx, id).map_err(db::Error::Abort)
    })
    .map_err(sql)?;
    Ok(json!({}))
}

// The queue and the batch.

/// The matter the latest pick in a wake chose before round `before`, or before none: the lone matter the queue held
/// (cell D0), or the one the architect picked (cell D1); none when the latest look at the queue found none, or many
/// with no pick after it.
pub fn picked_before(conn: &Connection, work: i64, before: Option<i64>) -> Result<Option<i64>, String> {
    let row: Option<(String, Option<String>)> = conn
        .query_row(
            "SELECT cell, output FROM rounds WHERE work = ?1 AND n < ?2 AND transition IS NOT NULL
               AND (cell = ?3 OR (cell = ?4 AND json_extract(output, '$.outcome.pick.matter') IS NOT NULL))
             ORDER BY n DESC LIMIT 1",
            params![work, before.unwrap_or(i64::MAX), LOOK, PICK],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(sql)?;
    let Some((cell, output)) = row else { return Ok(None) };
    let o = json_col(output).unwrap_or(Value::Null);
    Ok(if cell == LOOK {
        o["queue"]["matter"].as_i64()
    } else {
        o["outcome"]["pick"]["matter"].as_i64()
    })
}

/// The matter a wake's latest pick chose.
pub fn picked(conn: &Connection, work: i64) -> Result<Option<i64>, String> {
    picked_before(conn, work, None)
}

/// The round a wake's batch began after: its latest rethink, which ends a batch; 0 when it had none.
fn batch_start(conn: &Connection, work: i64) -> Result<i64, String> {
    count(
        conn,
        "SELECT coalesce(max(n), 0) FROM rounds WHERE work = ?1 AND cell = ?2 AND transition IS NOT NULL",
        &[&work, &RETHINK],
    )
}

/// How many matters a wake's batch handled, those the look at the queue closed without a look left out.
fn handled_in_batch(conn: &Connection, work: i64) -> Result<i64, String> {
    let from = batch_start(conn, work)?;
    count(
        conn,
        "SELECT count(*) FROM architect_matters m JOIN rounds r ON r.work = m.handled_work AND r.n = m.handled_round
         WHERE m.handled_work = ?1 AND m.handled_round > ?2 AND r.cell <> ?3",
        &[&work, &from, &LOOK],
    )
}

/// An item change of a batch: its row's id, the round that handed it in, the matter it came from and the change.
type Change = (i64, i64, i64, Value);

/// The item changes a wake's batch collected, in order.
fn collected(conn: &Connection, work: i64) -> Result<Vec<Change>, String> {
    let rows = query_json(
        conn,
        "SELECT id, round, matter, change FROM architect_item_changes WHERE work = ?1 AND state = 'collected' ORDER BY id",
        &[&work],
        100_000,
    )?;
    Ok(rows
        .iter()
        .map(|r| {
            (
                r["id"].as_i64().unwrap_or_default(),
                r["round"].as_i64().unwrap_or_default(),
                r["matter"].as_i64().unwrap_or_default(),
                r["change"].clone(),
            )
        })
        .collect())
}

/// The items a change collected in a wake's open batch names, on which the engine starts no work until the batch ends:
/// the item it changes, the item a split splits or another is moved under, and the items a merge keeps and absorbs.
pub fn held_by_batches(conn: &Connection) -> Result<BTreeSet<i64>, String> {
    let rows = query_json(
        conn,
        "SELECT c.change FROM architect_item_changes c JOIN works w ON w.id = c.work
         WHERE c.state = 'collected' AND w.ended IS NULL",
        &[],
        100_000,
    )?;
    let mut ids = BTreeSet::new();
    for r in rows {
        let c = &r["change"];
        for k in ["item", "keep", "under"] {
            ids_of(&c[k], &mut ids);
        }
        if c["to"]["place"] == "under" {
            ids_of(&c["to"]["item"], &mut ids);
        }
        for a in items(c.get("absorbed")) {
            ids_of(&a, &mut ids);
        }
    }
    Ok(ids)
}

/// Whether a project's new starts are held to set it straight.
fn held_straight(conn: &Connection, project: &str) -> Result<bool, String> {
    Ok(count(
        conn,
        "SELECT count(*) FROM dispatch_holds WHERE project = ?1 AND reason = 'set_straight'",
        &[&project],
    )? > 0)
}

/// The matters of a project's queue that can be picked now, oldest first: every one, but while new starts are held to
/// set the project straight and some matter sets it straight or has an agent stopped waiting, only those.
fn pickable(conn: &Connection, project: &str) -> Result<Vec<Value>, String> {
    let rows = query_json(
        conn,
        "SELECT * FROM architect_queue WHERE project = ?1 ORDER BY matter",
        &[&project],
        100_000,
    )?;
    if !held_straight(conn, project)? {
        return Ok(rows);
    }
    let first: Vec<Value> = rows
        .iter()
        .filter(|r| r["kind"] == "set_straight" || r["waits"] == "agent")
        .cloned()
        .collect();
    Ok(if first.is_empty() { rows } else { first })
}

/// Closes the design card matters of a project's queue whose card needs no look any longer, confirmed by the owner or
/// replaced by another, as handled in the wake's round that runs now; gives each, with why.
fn close_settled_cards(e: &mut Engine, w: &Work, project: &str) -> Result<Vec<String>, String> {
    let conn = e.db.conn();
    let rows = query_json(
        conn,
        "SELECT q.matter, c.id AS card, c.status, c.rulings, max(1, coalesce(json_array_length(c.body), 1)) AS decisions
         FROM architect_queue q JOIN architect_matters m ON m.id = q.matter
         JOIN design_cards c ON c.id = json_extract(m.body, '$.design_card')
         WHERE q.project = ?1 AND q.kind = 'design_card' ORDER BY q.matter",
        &[&project],
        ROWS,
    )?;
    let mut closed: Vec<(i64, String)> = Vec::new();
    for r in rows {
        let decisions = r["decisions"].as_u64().unwrap_or(1) as usize;
        let confirmed = items(r.get("rulings"))
            .last()
            .is_some_and(|x| x["by"] == "owner" && db::ruling_confirms(x, decisions));
        let why = match r["status"].as_str() {
            Some("replaced") => "replaced by another",
            Some("ruled") if confirmed => "confirmed by the owner",
            _ => continue,
        };
        let m = r["matter"].as_i64().unwrap_or_default();
        closed.push((m, format!("matter {m}, design card {} {why}", r["card"])));
    }
    if closed.is_empty() {
        return Ok(Vec::new());
    }
    let round = count(conn, "SELECT max(n) FROM rounds WHERE work = ?1", &[&w.id])?;
    e.db.write(Actor::Engine, |tx| {
        for (m, _) in &closed {
            tx.execute(
                "UPDATE architect_matters SET handled_work = ?2, handled_round = ?3 WHERE id = ?1 AND handled_round IS NULL",
                params![m, w.id, round],
            )?;
        }
        Ok(())
    })
    .map_err(sql)?;
    Ok(closed.into_iter().map(|x| x.1).collect())
}

/// Cell D0: what can be picked from the project's queue now; a lone matter is picked here, without a round. A design
/// card matter whose card needs no look any longer is closed first, as the round's progress says.
pub fn look_at_queue(e: &mut Engine, w: &Work) -> Result<Value, String> {
    let project = project_of(w)?;
    let closed = close_settled_cards(e, w, project)?;
    let conn = e.db.conn();
    let can = pickable(conn, project)?;
    let handled = handled_in_batch(conn, w.id)?;
    let queue = match can.as_slice() {
        [] if handled > 0 || !collected(conn, w.id)?.is_empty() => json!({ "verdict": "none_after_batch" }),
        [] => json!({ "verdict": "none" }),
        [one] => json!({ "verdict": "one", "matter": one["matter"] }),
        many => json!({ "verdict": "many", "count": many.len() }),
    };
    let mut out = json!({ "queue": queue });
    if !closed.is_empty() {
        out["progress"] = json!(format!("Closed without a look: {}.", closed.join("; ")));
    }
    Ok(out)
}

/// The cells on a matter's path: those the router sends a matter to, which an ask of the owner holds only the matter
/// of.
fn path_cells(g: &CompiledGraph) -> Vec<String> {
    g.cells
        .iter()
        .find(|c| c.inenums.contains_key("route"))
        .map(|c| c.rows.iter().map(|r| r.to.clone()).collect())
        .unwrap_or_default()
}

/// Whether the ask of the owner that held a matter has its answer.
fn answered(conn: &Connection, ask: i64) -> Result<bool, String> {
    Ok(count(
        conn,
        "SELECT count(*) FROM owner_asks WHERE id = ?1 AND answer IS NOT NULL",
        &[&ask],
    )? > 0)
}

/// Where the matter picked goes: back to the cell that asked about it, when it comes back with the owner's answer;
/// otherwise the first cell of its kind's path.
fn route(e: &Engine, w: &Work) -> Result<String, String> {
    let conn = e.db.conn();
    let m = picked(conn, w.id)?.ok_or_else(|| format!("work {}: no matter is picked", w.id))?;
    let (kind, asked, resume): (String, Option<i64>, Option<String>) = conn
        .query_row(
            "SELECT kind, asked, resume_cell FROM architect_matters WHERE id = ?1",
            [m],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .map_err(|x| format!("matter {m}: {x}"))?;
    if let (Some(a), Some(cell)) = (asked, resume)
        && answered(conn, a)?
    {
        return Ok(cell);
    }
    KINDS
        .iter()
        .find(|k| k.0 == kind)
        .map(|k| k.1.to_string())
        .ok_or_else(|| format!("matter {m} is of a kind the architect's graph has no path for: {kind}"))
}

/// Whether the batch goes on, goes to its review, or is done.
fn next_in_batch(e: &Engine, w: &Work) -> Result<&'static str, String> {
    let conn = e.db.conn();
    let can = pickable(conn, project_of(w)?)?.len();
    let handled = handled_in_batch(conn, w.id)?;
    Ok(if can > 0 && handled < BATCH {
        "more"
    } else if !collected(conn, w.id)?.is_empty() {
        "review"
    } else {
        "done"
    })
}

/// How many reviews of a wake's batch found something to fix.
fn fixes_found(conn: &Connection, work: i64) -> Result<i64, String> {
    let from = batch_start(conn, work)?;
    count(
        conn,
        "SELECT count(*) FROM rounds WHERE work = ?1 AND n > ?2 AND cell = ?3 AND transition = 'fix'",
        &[&work, &from, &REVIEW],
    )
}

// Sessions.

/// Where the next round at the understanding question would run.
fn plan_at_understanding(e: &Engine, w: &Work, g: &CompiledGraph) -> Result<sessions::Plan, String> {
    let cell = g
        .cell(UNDERSTAND)
        .ok_or("the architect's graph has no understanding question")?;
    let model = crate::steps::level_of(e, g, cell, None)?.1.model;
    let limit = e.config.config.install.agents.context_limit;
    e.sessions
        .plan(e.db.conn(), w.id, w.project.as_deref(), cell, limit, &model)
}

/// Whether the session the wake goes on in answered the understanding question since it started or was last
/// compacted: a new one, or one the next round would leave for a new one, did not. A round that asked for tools or
/// paused answered nothing, as the state of things the views read has it.
fn understood(e: &Engine, w: &Work, g: &CompiledGraph) -> Result<bool, String> {
    let sessions::Plan::Going { held, .. } = plan_at_understanding(e, w, g)? else {
        return Ok(false);
    };
    for r in &held.rounds {
        let n = count(
            e.db.conn(),
            &format!(
                "SELECT count(*) FROM rounds WHERE work = ?1 AND n = ?2 AND cell = ?3 AND transition IS NOT NULL
                   AND transition NOT IN {NO_ANSWER}"
            ),
            &[&r.work, &r.n, &UNDERSTAND],
        )?;
        if n > 0 {
            return Ok(true);
        }
    }
    Ok(false)
}

/// How the work came to the understanding question: from a rethink after a batch; in a session compacted since it
/// last understood the project; or in a new one, at the project's first wake (before any answer of the question, a
/// round that asked for tools or paused being none), after a new policy, or for another reason.
fn entry(e: &Engine, w: &Work, g: &CompiledGraph) -> Result<String, String> {
    let conn = e.db.conn();
    let last: Option<String> = conn
        .query_row(
            "SELECT cell FROM rounds WHERE work = ?1 AND cell IN (?2, 'W1') AND transition IS NOT NULL
             ORDER BY n DESC LIMIT 1",
            params![w.id, RETHINK],
            |r| r.get(0),
        )
        .optional()
        .map_err(sql)?;
    if last.as_deref() == Some(RETHINK) {
        return Ok("rethink".into());
    }
    if let sessions::Plan::Going { held, .. } = plan_at_understanding(e, w, g)?
        && held.compacted.is_some()
    {
        return Ok("compacted".into());
    }
    let project = project_of(w)?;
    let before = count(
        conn,
        &format!(
            "SELECT count(*) FROM rounds r JOIN works x ON x.id = r.work
             WHERE x.graph = 'architect' AND x.project = ?1 AND r.cell = ?2 AND r.transition IS NOT NULL
               AND r.transition NOT IN {NO_ANSWER}"
        ),
        &[&project, &UNDERSTAND],
    )?;
    if before == 0 {
        return Ok("first".into());
    }
    let policy = count(
        conn,
        "SELECT count(*) FROM rounds WHERE work = ?1 AND cell = 'P10' AND transition IS NOT NULL",
        &[&w.id],
    )?;
    Ok(if policy > 0 { "new_policy" } else { "new" }.into())
}

/// The detail of the matter picked.
fn picked_detail(conn: &Connection, work: i64) -> Result<Option<String>, String> {
    let Some(m) = picked(conn, work)? else { return Ok(None) };
    conn.query_row("SELECT detail FROM architect_matters WHERE id = ?1", [m], |r| r.get(0))
        .map_err(sql)
}

/// One of the engine's values of the architect's graph; none for a value of another graph.
pub fn value(e: &Engine, w: &Work, g: &CompiledGraph, name: &str) -> Result<Option<String>, String> {
    let conn = e.db.conn();
    Ok(Some(match name {
        "understood" => if understood(e, w, g)? { "yes" } else { "no" }.into(),
        "entry" => entry(e, w, g)?,
        "route" => route(e, w)?,
        "next_in_batch" => next_in_batch(e, w)?.into(),
        "fixes_before" => if fixes_found(conn, w.id)? >= 2 { "two" } else { "fewer" }.into(),
        "asker" => {
            let path = path_cells(g);
            let on_path = w.pos.callers.last().is_some_and(|c| path.contains(c));
            if on_path { "path" } else { "waits" }.into()
        }
        "question" => {
            let d = picked_detail(conn, w.id)?.unwrap_or_default();
            if QUESTION_KINDS.contains(&d.as_str()) {
                d
            } else {
                "other_question".into()
            }
        }
        "report" => if picked_detail(conn, w.id)?.as_deref() == Some("followup") {
            "followup"
        } else {
            "side_report"
        }
        .into(),
        _ => return Ok(None),
    }))
}

// Asks of the owner.

/// The ask of the owner a round put, and its answer once the owner gave it.
pub fn owner_ask_of(e: &Engine, round: RoundId) -> Result<Option<(i64, Option<Value>)>, String> {
    e.db.conn()
        .query_row(
            "SELECT id, answer FROM owner_asks WHERE work = ?1 AND round = ?2 ORDER BY id LIMIT 1",
            [round.work, round.n],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<String>>(1)?)),
        )
        .optional()
        .map(|o| o.map(|(id, a)| (id, json_col(a))))
        .map_err(sql)
}

/// What a step that is not an agent's round records of its inputs: the value of each, by its sources.
fn recorded(gathered: &[Gathered]) -> Value {
    Value::Array(
        gathered
            .iter()
            .filter(|x| !x.itself())
            .map(|x| json!({ "from": x.input.from, "value": x.value }))
            .collect(),
    )
}

/// What a round asking someone holds of what the cells handed in: each input read from a return, with what it is.
fn asked_of(gathered: &[Gathered]) -> Vec<Value> {
    gathered
        .iter()
        .filter(|x| x.value.is_some() && x.input.from.iter().all(|s| matches!(s, Src::Place(..))))
        .map(|x| json!({ "what": x.input.what, "value": x.value }))
        .collect()
}

/// The options an owner's cell offers, each with what it means, as its return's schema lists them for `ruling.choice`.
fn choices(cell: &CompiledCell) -> Vec<Value> {
    let Some(f) = cell
        .schema
        .as_ref()
        .and_then(|s| s.pointer("/properties/ruling/properties/choice"))
    else {
        return Vec::new();
    };
    let values: Vec<String> = f["enum"]
        .as_array()
        .map(|xs| xs.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
        .unwrap_or_default();
    let desc = f["description"].as_str().unwrap_or_default();
    let listed = desc.rsplit_once("Values: ").map(|x| x.1).unwrap_or(desc);
    values
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let start = listed.find(&format!("{v}: ")).map(|p| p + v.len() + 2);
            let end = values
                .get(i + 1)
                .and_then(|n| listed.find(&format!("; {n}: ")))
                .unwrap_or(listed.len());
            let means = start.filter(|s| *s <= end).map(|s| &listed[s..end]).unwrap_or_default();
            json!([v, means])
        })
        .collect()
}

/// What an answer of the owner's comes back to an owner's cell of the architect's graph as: a ruling with the option
/// chosen and the words, for a cell that rules; a reply with the words otherwise.
fn answered_as(cell: &CompiledCell, answer: &Value) -> Value {
    let text = answer["text"].as_str().unwrap_or_default();
    let rules = cell
        .schema
        .as_ref()
        .and_then(|s| s.pointer("/properties/ruling"))
        .is_some();
    if rules {
        json!({ "ruling": { "choice": answer["choice"], "text": text } })
    } else {
        json!({ "reply": { "text": text } })
    }
}

/// The owner's cells of the architect's graph (asking, reading a study, approving a policy): what the architect asks,
/// or puts before the owner to read or approve, becomes an ask of the owner. An ask from a cell on a matter's path
/// holds only that matter: the ask records the cell that asked, the round ends at once, and the wake goes on without
/// it; the matter is back in the queue once the owner answers. Any other waits for the answer, holding no slot.
pub fn ask_owner(
    e: &mut Engine,
    w: &Work,
    g: &CompiledGraph,
    cell: &CompiledCell,
    open: Option<Round>,
) -> Result<Advance, String> {
    if let Some(r) = open.filter(|r| r.cell.as_deref() == Some(cell.id.as_str())) {
        let Some((id, answer)) = owner_ask_of(e, r.id)? else {
            return Err(format!("{} asks, but put no ask to the owner", r.id));
        };
        let Some(answer) = answer else {
            return Ok(Advance::Waits(format!("ask {id} waits for the owner")));
        };
        let output = answered_as(cell, &answer);
        let gathered = inputs::gather(e, w, g, cell, None)?;
        let given = values::given(e, w, g, cell, None, &gathered)?;
        let row = g.take(&cell.id, &output, &given)?.clone();
        return finish(e, w, g, cell, r.id, output, &row, Ended::default(), &[], None, &given);
    }
    let project = project_of(w)?.to_string();
    let gathered = inputs::gather(e, w, g, cell, None)?;
    let given = values::given(e, w, g, cell, None, &gathered)?;
    let holds = if given.get("asker").and_then(Value::as_str) == Some("path") {
        picked(e.db.conn(), w.id)?
    } else {
        None
    };
    let mut body = json!({ "from": "architect", "project": project, "work": w.id, "cell": cell.id });
    let trials = gathered
        .iter()
        .flat_map(|x| &x.parts)
        .find(|p| p.origin == Origin::Db("policy_trials".into()))
        .and_then(|p| items(Some(&p.value)).into_iter().next());
    let notice = if let Some(t) = trials {
        body["kind"] = json!("policy_approval");
        body["policy"] = t["policy"].clone();
        body["probes"] = t["probes"].clone();
        body["readings"] = t["readings"].clone();
        body["choices"] = Value::Array(choices(cell));
        format!("The architect of {project}'s new policy, read by new architects, waits for your approval")
    } else if !choices(cell).is_empty() {
        body["kind"] = json!("study");
        body["study"] = gathered.iter().find_map(|x| x.value.clone()).unwrap_or(Value::Null);
        body["choices"] = Value::Array(choices(cell));
        format!("The architect of {project}'s study of a new policy is ready for you to read")
    } else {
        let asked = asked_of(&gathered);
        let about = what_is_asked(&asked);
        body["kind"] = json!("ask");
        body["asked"] = Value::Array(asked);
        if let Some(m) = holds {
            body["holds"] = json!([m]);
        }
        format!("The architect of {project} asks you{about}")
    };
    let caller = w.pos.callers.last().cloned();
    let input = json!({ "inputs": recorded(&gathered) });
    let commit = e.config.commit.clone();
    let (round, id) =
        e.db.write(Actor::Engine, |tx| {
            let round = work::start_round(tx, w.id, &cell.id, None, &input, &commit, None).map_err(db::Error::Abort)?;
            tx.execute(
                "INSERT INTO owner_asks (work, round, body) VALUES (?1, ?2, ?3)",
                params![round.work, round.n, body.to_string()],
            )?;
            let id = tx.last_insert_rowid();
            actions::notify_ask(tx, w.id, id, &format!("{notice} (ask {id})")).map_err(db::Error::Abort)?;
            // The matter the ask holds waits for the answer out of the queue, and goes back to the cell that asked.
            if let (Some(m), Some(c)) = (holds, &caller) {
                tx.execute(
                    "UPDATE architect_matters SET asked = ?2, resume_cell = ?3 WHERE id = ?1",
                    params![m, id, c],
                )?;
            }
            Ok((round, id))
        })
        .map_err(sql)?;
    let Some(m) = holds else {
        return Ok(Advance::Waits(format!("ask {id} waits for the owner")));
    };
    let text = format!(
        "The ask is with the owner as ask {id}. Matter {m} waits for the answer out of the queue, and comes back to the \
         cell that asked with it; the wake goes on with the other matters."
    );
    let output = json!({ "reply": { "text": text } });
    let row = g.take(&cell.id, &output, &given)?.clone();
    finish(e, w, g, cell, round, output, &row, Ended::default(), &[], None, &given)
}

// An engineer's question.

/// The matter an engineer's question was queued as, by the round that asked, with the architect's reply once it gave
/// one.
pub fn question(e: &Engine, round: RoundId) -> Result<Option<(i64, Option<Value>)>, String> {
    e.db.conn()
        .query_row(
            "SELECT id, reply FROM architect_matters WHERE work = ?1 AND round = ?2 ORDER BY id LIMIT 1",
            [round.work, round.n],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<String>>(1)?)),
        )
        .optional()
        .map(|o| o.map(|(id, reply)| (id, json_col(reply))))
        .map_err(sql)
}

/// The kind of matter a question to the architect is, with its detail: by the kind of the ask's first point when a cell
/// asked, an engineer's question; for an engine tool that failed, or main's environment that changed, a failure.
fn question_kind(gathered: &[Gathered]) -> (&'static str, String) {
    fn path_of(p: &inputs::Part) -> String {
        match &p.origin {
            Origin::Place { path, .. } => path.clone(),
            _ => String::new(),
        }
    }
    let parts: Vec<&inputs::Part> = gathered.iter().flat_map(|x| &x.parts).collect();
    if let Some(ask) = parts.iter().find(|p| path_of(p) == "ask" && !empty(&p.value)) {
        let kind = ask.value["points"][0]["kind"].as_str().unwrap_or_default();
        let detail = QUESTION_KINDS
            .iter()
            .find(|k| **k == kind)
            .copied()
            .unwrap_or("other_question");
        return ("engineer_question", detail.into());
    }
    if parts
        .iter()
        .any(|p| path_of(p).contains("environment_changed") && !empty(&p.value))
    {
        return ("failure", "environment_changed".into());
    }
    if parts
        .iter()
        .any(|p| !matches!(path_of(p).as_str(), "" | "ask" | "progress") && !empty(&p.value))
    {
        return ("failure", "tool_failed".into());
    }
    ("engineer_question", "other_question".into())
}

/// Cell 15: what is asked is queued for the architect of the item's project as a matter, which wakes it if it sleeps;
/// the work waits, holding no slot, until the architect replies, and the reply goes back to the cell that asked
/// unchanged.
pub fn ask_architect(
    e: &mut Engine,
    w: &Work,
    g: &CompiledGraph,
    cell: &CompiledCell,
    item: &Item,
    open: Option<Round>,
) -> Result<Advance, String> {
    if let Some(r) = open.filter(|r| r.cell.as_deref() == Some(cell.id.as_str())) {
        let Some((matter, reply)) = question(e, r.id)? else {
            return Err(format!("{} asks the architect, but queued no matter", r.id));
        };
        let Some(reply) = reply else {
            return Ok(Advance::Waits(format!(
                "matter {matter} waits for the architect's reply"
            )));
        };
        let output = json!({ "reply": reply });
        let gathered = inputs::gather(e, w, g, cell, Some(item))?;
        let given = values::given(e, w, g, cell, Some(item), &gathered)?;
        let row = g.take(&cell.id, &output, &given)?.clone();
        return finish(e, w, g, cell, r.id, output, &row, Ended::default(), &[], None, &given);
    }
    let gathered = inputs::gather(e, w, g, cell, Some(item))?;
    let asked = asked_of(&gathered);
    let (kind, detail) = question_kind(&gathered);
    let about = match what_is_asked(&asked) {
        a if a.is_empty() => format!(" ({})", detail.replace('_', " ")),
        a => a,
    };
    let what = format!("Work {} on item {} ({}) asks{about}", w.id, item.id, item.title);
    let body = json!({ "to": "architect", "work": w.id, "item": item.id, "title": item.title, "asked": asked });
    let input = json!({ "inputs": recorded(&gathered) });
    let commit = e.config.commit.clone();
    let matter =
        e.db.write(Actor::Engine, |tx| {
            let round = work::start_round(tx, w.id, &cell.id, None, &input, &commit, None).map_err(db::Error::Abort)?;
            let m = Matter {
                project: item.project.clone(),
                kind: kind.into(),
                detail: Some(detail.clone()),
                what: what.clone(),
                body: body.clone(),
                item: Some(item.id),
                raised: Some((round.work, round.n)),
            };
            queue_matter(tx, &m)
        })
        .map_err(sql)?;
    (e.say)(&format!(
        "work {} asks the architect of {}: matter {matter}",
        w.id, item.project
    ));
    Ok(Advance::Waits(format!(
        "matter {matter} waits for the architect's reply"
    )))
}

/// Cell P8: how many new sessions in a row read this version of the policy's text and were judged to understand it;
/// two pass it, one asks for another.
pub fn count_trials(e: &mut Engine, w: &Work) -> Result<Value, String> {
    let rows = query_json(
        e.db.conn(),
        "SELECT readings FROM policy_trials WHERE work = ?1",
        &[&w.id],
        1,
    )?;
    let readings = rows.first().map(|r| items(r.get("readings"))).unwrap_or_default();
    let passed = readings
        .iter()
        .rev()
        .take_while(|r| r["verdict"]["verdict"] == "understood")
        .count();
    let verdict = if passed >= 2 { "two_passed" } else { "one_more" };
    Ok(json!({ "count": { "verdict": verdict } }))
}

// The architect's actions.

/// The wake an action is carried out for, and its project.
fn wake_of(ctx: &Ctx) -> Result<(i64, String), String> {
    Ok((ctx.work.id, project_of(&ctx.work)?.to_string()))
}

/// The matter a wake picked, which the action is about.
fn matter_of(tx: &Transaction<'_>, work: i64) -> Result<i64, String> {
    picked(tx, work)?.ok_or_else(|| format!("work {work}: no matter is picked"))
}

/// A work running on an item: its id, why it is paused and who paused it.
type Running = (i64, Option<String>, Option<String>);

/// The work running on an item, as the database has it: not stopped and not at the end of the engineer's graph.
fn running_work(conn: &Connection, item: i64) -> Result<Option<Running>, String> {
    conn.query_row(
        "SELECT id, paused, paused_by FROM works
         WHERE item = ?1 AND graph = 'engineer' AND ended IS NULL AND at <> ?2 ORDER BY id DESC LIMIT 1",
        params![item, ENGINEER_END],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )
    .optional()
    .map_err(sql)
}

/// Pauses the work running on an item as the architect's pause: the engine's pause, its reason saying the architect
/// stopped it and why, which the owner reads on the page.
fn pause_item(tx: &Transaction<'_>, item: i64, why: &str) -> Result<(), String> {
    let Some((work, None, _)) = running_work(tx, item)? else {
        return Ok(());
    };
    let why = format!("{}: {why}", db::STOPPED_BY_ARCHITECT);
    tx.execute(
        "UPDATE works SET paused = ?2, paused_by = 'engine' WHERE id = ?1",
        params![work, why],
    )
    .map_err(sql)?;
    actions::notify(tx, Some(work), &format!("Work {work} on item {item} is {why}"))
}

/// Holds or releases a project's new starts for a reason.
fn hold(tx: &Transaction<'_>, project: &str, reason: &str, why: Option<&str>) -> Result<(), String> {
    tx.execute(
        "INSERT INTO dispatch_holds (project, reason, since, why) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT (project, reason) DO UPDATE SET why = coalesce(excluded.why, why)",
        params![project, reason, db::now(), why],
    )
    .map_err(sql)?;
    Ok(())
}

fn release(tx: &Transaction<'_>, project: &str, reason: &str) -> Result<(), String> {
    tx.execute(
        "DELETE FROM dispatch_holds WHERE project = ?1 AND reason = ?2",
        params![project, reason],
    )
    .map_err(sql)?;
    Ok(())
}

/// Releases the hold to set a project straight once no matter that sets it straight is left and the wake's batch
/// holds no item change still to be made.
fn settle_hold(tx: &Transaction<'_>, work: i64, project: &str) -> Result<(), String> {
    if !held_straight(tx, project)? {
        return Ok(());
    }
    let left = count(
        tx,
        "SELECT count(*) FROM architect_matters WHERE project = ?1 AND kind = 'set_straight' AND handled_round IS NULL",
        &[&project],
    )?;
    if left == 0 && collected(tx, work)?.is_empty() {
        release(tx, project, "set_straight")?;
    }
    Ok(())
}

/// Carries out one action of an architect's effects, at once.
fn act_now(tx: &Transaction<'_>, project: &str, a: &Value) -> Result<(), String> {
    let reason = text(&a["reason"]).trim();
    match text(&a["kind"]) {
        "pause_work" => pause_item(tx, a["item"].as_i64().unwrap_or_default(), reason),
        "resume_work" => {
            let item = a["item"].as_i64().unwrap_or_default();
            let Some((work, Some(p), by)) = running_work(tx, item)? else {
                return Ok(());
            };
            if by.as_deref() == Some("engine") && p.starts_with(db::STOPPED_BY_ARCHITECT) {
                tx.execute("UPDATE works SET paused = NULL, paused_by = NULL WHERE id = ?1", [work])
                    .map_err(sql)?;
            }
            Ok(())
        }
        "hold_starts" => hold(tx, project, "architect", Some(reason)),
        "release_starts" => release(tx, project, "architect"),
        "interject" => {
            let item = a["item"].as_i64().unwrap_or_default();
            let Some((work, _, _)) = running_work(tx, item)? else {
                return Ok(());
            };
            tx.execute(
                "INSERT INTO interjections (work, line, speaker, text) VALUES (?1, 'engineer', 'architect', ?2)",
                params![work, text(&a["say"])],
            )
            .map_err(sql)?;
            Ok(())
        }
        other => Err(format!("no such action: {other}")),
    }
}

/// The texts of a list, each trimmed.
fn texts(v: &Value) -> Vec<String> {
    items(Some(v))
        .iter()
        .filter_map(|x| x.as_str().map(|s| s.trim().to_string()))
        .filter(|s| !s.is_empty())
        .collect()
}

/// Makes one change to a project's observations, at once.
fn observe(tx: &Transaction<'_>, project: &str, matter: i64, o: &Value) -> Result<(), String> {
    match text(&o["kind"]) {
        "new" => {
            let source = format!("{} (matter {matter})", text(&o["source"]).trim());
            tx.execute(
                "INSERT INTO observations (project, what, tags, source, refs) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    project,
                    text(&o["what"]).trim(),
                    json!(texts(&o["tags"])).to_string(),
                    source,
                    json!(texts(&o["refs"])).to_string()
                ],
            )
            .map_err(sql)?;
        }
        "merge" => {
            let id = o["into"].as_i64().unwrap_or_default();
            let old: String = tx
                .query_row("SELECT refs FROM observations WHERE id = ?1", [id], |r| r.get(0))
                .map_err(|x| format!("observation {id}: {x}"))?;
            let mut refs: Vec<Value> = serde_json::from_str(&old).unwrap_or_default();
            refs.extend(texts(&o["refs"]).into_iter().map(Value::String));
            let seen = text(&o["seen"]).trim();
            if !seen.is_empty() {
                refs.push(json!(format!("Reported again (matter {matter}): {seen}")));
            }
            tx.execute(
                "UPDATE observations SET times_reported = times_reported + 1, refs = ?2 WHERE id = ?1",
                params![id, Value::Array(refs).to_string()],
            )
            .map_err(sql)?;
        }
        "close" => {
            tx.execute(
                "UPDATE observations SET closed = ?2, closed_why = ?3 WHERE id = ?1",
                params![
                    o["id"].as_i64().unwrap_or_default(),
                    text(&o["why"]),
                    text(&o["text"]).trim()
                ],
            )
            .map_err(sql)?;
        }
        other => return Err(format!("no such change to an observation: {other}")),
    }
    Ok(())
}

/// Queues a message for another project's architect.
fn message(tx: &Transaction<'_>, from: &str, to: &str, words: &str, detail: &str, body: Value) -> Result<i64, String> {
    let m = Matter {
        project: to.to_string(),
        kind: "other_architect".into(),
        detail: Some(detail.into()),
        what: format!("The architect of {from} says: {}", db::one_sentence(words)),
        body,
        ..Matter::default()
    };
    queue_matter(tx, &m).map_err(sql)
}

/// Carries out the effects of an answer on a matter's path: item changes are collected into the wake's batch; changes
/// to observations, actions and messages are made at once; notes are the round's own, which the views read.
fn apply_effects(ctx: &Ctx, tx: &Transaction<'_>, effects: &Value) -> Result<(), String> {
    let (w, project) = wake_of(ctx)?;
    let matter = matter_of(tx, w)?;
    for c in items(effects.get("item_changes")) {
        tx.execute(
            "INSERT INTO architect_item_changes (work, round, matter, change, state) VALUES (?1, ?2, ?3, ?4, 'collected')",
            params![ctx.round.work, ctx.round.n, matter, c.to_string()],
        )
        .map_err(sql)?;
    }
    for o in items(effects.get("observation_changes")) {
        observe(tx, &project, matter, &o)?;
    }
    for a in items(effects.get("actions")) {
        act_now(tx, &project, &a)?;
    }
    for m in items(effects.get("messages")) {
        let body = json!({ "from": project, "text": m["text"], "matter": matter, "work": w });
        message(tx, &project, text(&m["project"]), text(&m["text"]), "message", body)?;
    }
    Ok(())
}

/// Whether a round's effects hold item changes.
fn changes_items(ctx: &Ctx) -> bool {
    !items(ctx.unwrapped["effects"].get("item_changes")).is_empty()
}

/// A reply with the keys of the items a batch made put back as their numbers.
fn numbered(reply: &Value, keys: &BTreeMap<String, i64>) -> Value {
    let mut r = reply.clone();
    for list in ["children", "waits"] {
        if let Some(xs) = r.get_mut(list).and_then(Value::as_array_mut) {
            for x in xs {
                if let Some(id) = x["key"].as_str().and_then(|k| keys.get(k.trim())) {
                    x["key"] = json!(id.to_string());
                }
            }
        }
    }
    r
}

/// Makes the given rows of a wake's batch, as the architect, each in order; a key a change made the item under is
/// given its number. Gives the keys and the numbers they were given.
fn make(tx: &Transaction<'_>, rows: &[Change]) -> Result<BTreeMap<String, i64>, String> {
    let changes: Vec<Value> = rows.iter().map(|r| r.3.clone()).collect();
    let created = crate::arch_changes::write_all(tx, &changes)?;
    let keys: BTreeMap<String, i64> = created
        .iter()
        .filter_map(|c| Some((c["key"].as_str()?.to_string(), c["id"].as_str()?.parse().ok()?)))
        .collect();
    for (id, _, _, change) in rows {
        let outcome = match change["kind"].as_str() {
            Some("create_item") => keys.get(text(&change["key"]).trim()).map(|n| json!({ "created": n })),
            _ => None,
        };
        tx.execute(
            "UPDATE architect_item_changes SET state = 'made', outcome = ?2 WHERE id = ?1",
            params![id, outcome.map(|o| o.to_string())],
        )
        .map_err(sql)?;
    }
    Ok(keys)
}

/// Sends the replies that waited for the item changes of their matters, once those are made.
fn send_held_replies(
    tx: &Transaction<'_>,
    matters: &BTreeSet<i64>,
    keys: &BTreeMap<String, i64>,
) -> Result<(), String> {
    for m in matters {
        let held: Option<String> = tx
            .query_row("SELECT reply_held FROM architect_matters WHERE id = ?1", [m], |r| {
                r.get(0)
            })
            .map_err(sql)?;
        let Some(reply) = json_col(held) else { continue };
        tx.execute(
            "UPDATE architect_matters SET reply = ?2, reply_held = NULL WHERE id = ?1",
            params![m, numbered(&reply, keys).to_string()],
        )
        .map_err(sql)?;
    }
    Ok(())
}

/// The review of a batch made its changes or dropped them: its session ends, so that the next batch's review starts
/// afresh, and the hold to set the project straight is let go once nothing of it is left.
fn batch_over(tx: &Transaction<'_>, work: i64, project: &str) -> Result<(), String> {
    sessions::choose::end_line(tx, work, "architect_review")?;
    settle_hold(tx, work, project)
}

/// The rows of a batch the review found sound, and those it found to fix with what it found: by the review's rows,
/// numbered as the batch was given; a change that names an item a dropped one creates is dropped with it.
fn sorted_by_review(rows: Vec<Change>, review: &[Value]) -> (Vec<Change>, Vec<(Change, Value)>) {
    let mut sound = Vec::new();
    let mut dropped: Vec<(Change, Value)> = Vec::new();
    for (i, row) in rows.into_iter().enumerate() {
        let finding = review
            .iter()
            .find(|r| r["change"].as_i64() == Some(i as i64 + 1) && r["verdict"] == "fix");
        match finding {
            Some(f) => dropped.push((row, f.clone())),
            None => sound.push(row),
        }
    }
    loop {
        let gone: BTreeSet<String> = dropped
            .iter()
            .filter(|(r, _)| r.3["kind"] == "create_item")
            .map(|(r, _)| text(&r.3["key"]).trim().to_string())
            .collect();
        let names_gone = |c: &Value| {
            ["item", "on", "under", "keep", "fix_item"]
                .iter()
                .map(|k| c[*k].clone())
                .chain(std::iter::once(c["to"]["item"].clone()))
                .chain(items(c.get("absorbed")))
                .any(|v| v.as_str().is_some_and(|s| gone.contains(s.trim())))
        };
        let Some(i) = sound.iter().position(|r| names_gone(&r.3)) else {
            break;
        };
        let row = sound.remove(i);
        dropped.push((
            row,
            json!({ "what": "It names an item a dropped change was to create" }),
        ));
    }
    (sound, dropped)
}

/// The review gave up on what it still found to fix at its third look: the changes it found sound are made, the rest
/// dropped with what it found, and the owner is asked about them in an ask that holds the matters they came from, each
/// to go back, with the answer, to the cell that decides it.
fn give_up(ctx: &Ctx, tx: &Transaction<'_>) -> Result<(), String> {
    let (w, project) = wake_of(ctx)?;
    let review = items(ctx.unwrapped.get("review"));
    let (sound, dropped) = sorted_by_review(collected(tx, w)?, &review);
    let keys = make(tx, &sound)?;
    let matters: BTreeSet<i64> = sound.iter().map(|r| r.2).collect();
    let held: BTreeSet<i64> = dropped.iter().map(|(r, _)| r.2).collect();
    send_held_replies(tx, &matters.difference(&held).copied().collect(), &keys)?;
    for ((id, _, _, _), finding) in &dropped {
        tx.execute(
            "UPDATE architect_item_changes SET state = 'dropped', outcome = ?2 WHERE id = ?1",
            params![id, finding.to_string()],
        )
        .map_err(sql)?;
    }
    if !dropped.is_empty() {
        let asked: Vec<Value> = dropped
            .iter()
            .map(|((_, _, m, change), finding)| json!({ "matter": m, "change": change, "review": finding }))
            .collect();
        let body = json!({
            "from": "architect", "project": project, "work": w, "cell": ctx.cell.id, "kind": "ask",
            "asked": [{ "what": "The item changes the review of the batch still found wrong at its third look, each \
                with what it found: they were not made. Say what to do about them", "value": asked }],
            "holds": held.iter().collect::<Vec<_>>(),
        });
        tx.execute(
            "INSERT INTO owner_asks (work, round, body) VALUES (?1, ?2, ?3)",
            params![ctx.round.work, ctx.round.n, body.to_string()],
        )
        .map_err(sql)?;
        let ask = tx.last_insert_rowid();
        for m in &held {
            let kind: String = tx
                .query_row("SELECT kind FROM architect_matters WHERE id = ?1", [m], |r| r.get(0))
                .map_err(sql)?;
            let decides = KINDS.iter().find(|k| k.0 == kind).map_or("O1", |k| k.2);
            tx.execute(
                "UPDATE architect_matters SET asked = ?2, resume_cell = ?3, handled_work = NULL, handled_round = NULL
                 WHERE id = ?1",
                params![m, ask, decides],
            )
            .map_err(sql)?;
        }
        let n = dropped.len();
        actions::notify_ask(
            tx,
            w,
            ask,
            &format!(
                "The architect of {project} dropped {n} item changes its review still found wrong, and asks you about \
                 them (ask {ask})"
            ),
        )?;
    }
    batch_over(tx, w, &project)
}

/// Carries out an action of the architect's graph; false for an action of another.
pub fn act(ctx: &Ctx, tx: &Transaction<'_>, name: &str, at: Option<&Value>) -> Result<bool, String> {
    let w = ctx.work.id;
    match name {
        "queue_fixes" => {
            let (_, project) = wake_of(ctx)?;
            let fit = &ctx.unwrapped["fit"];
            for fix in items(at) {
                let what = format!(
                    "To set straight, {}: {}",
                    db::one_sentence(text(&fix["where"])),
                    db::one_sentence(text(&fix["fix"]))
                );
                let m = Matter {
                    project: project.clone(),
                    kind: "set_straight".into(),
                    detail: fix["kind"].as_str().map(str::to_string),
                    what,
                    body: json!({ "fix": fix, "verdict": fit["verdict"], "why": fit["why"] }),
                    item: fix["item"].as_i64(),
                    raised: Some((ctx.round.work, ctx.round.n)),
                };
                queue_matter(tx, &m).map_err(sql)?;
                if fix["pause_now"] == true
                    && let Some(item) = fix["item"].as_i64()
                {
                    pause_item(tx, item, text(&fix["wrong"]))?;
                }
            }
        }
        "hold_to_set_straight" => hold(tx, &wake_of(ctx)?.1, "set_straight", None)?,
        "release_straight_hold" => release(tx, &wake_of(ctx)?.1, "set_straight")?,
        "settle_straight_hold" => settle_hold(tx, w, &wake_of(ctx)?.1)?,
        "hold_for_policy" => hold(tx, &wake_of(ctx)?.1, "policy", None)?,
        "release_policy_hold" => release(tx, &wake_of(ctx)?.1, "policy")?,
        "close_matter" => {
            let m = matter_of(tx, w)?;
            tx.execute(
                "UPDATE architect_matters SET handled_work = ?2, handled_round = ?3 WHERE id = ?1 AND handled_round IS NULL",
                params![m, ctx.round.work, ctx.round.n],
            )
            .map_err(sql)?;
        }
        "apply_effects" => {
            if let Some(effects) = at {
                apply_effects(ctx, tx, effects)?;
            }
        }
        "reply_to_engineer" => {
            let Some(reply) = at.filter(|r| r.is_object()) else {
                return Ok(true);
            };
            let m = matter_of(tx, w)?;
            let column = if changes_items(ctx) { "reply_held" } else { "reply" };
            tx.execute(
                &format!("UPDATE architect_matters SET {column} = ?2 WHERE id = ?1"),
                params![m, reply.to_string()],
            )
            .map_err(sql)?;
        }
        "answer_architect" => {
            let Some(words) = at.and_then(Value::as_str).filter(|s| !s.trim().is_empty()) else {
                return Ok(true);
            };
            let (_, project) = wake_of(ctx)?;
            let m = matter_of(tx, w)?;
            let body: String = tx
                .query_row("SELECT body FROM architect_matters WHERE id = ?1", [m], |r| r.get(0))
                .map_err(sql)?;
            let body: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
            let Some(to) = body["from"].as_str() else {
                return Ok(true);
            };
            let reply = json!({ "from": project, "text": words, "answers": m, "work": w });
            message(tx, &project, to, words, "reply", reply)?;
        }
        "interject_card" => {
            let Some(wrong) = at
                .map(|v| text(&v["wrong"]).trim().to_string())
                .filter(|s| !s.is_empty())
            else {
                return Ok(true);
            };
            let m = matter_of(tx, w)?;
            let card: Option<i64> = tx
                .query_row(
                    "SELECT json_extract(body, '$.design_card') FROM architect_matters WHERE id = ?1",
                    [m],
                    |r| r.get(0),
                )
                .map_err(sql)?;
            let Some(card) = card else { return Ok(true) };
            let work: Option<i64> = tx
                .query_row(
                    "SELECT c.work FROM design_cards c JOIN works w ON w.id = c.work
                     WHERE c.id = ?1 AND w.ended IS NULL",
                    [card],
                    |r| r.get(0),
                )
                .optional()
                .map_err(sql)?;
            if let Some(work) = work {
                tx.execute(
                    "INSERT INTO interjections (work, line, speaker, text) VALUES (?1, 'engineer', 'architect', ?2)",
                    params![work, format!("About design card {card}: {wrong}")],
                )
                .map_err(sql)?;
            }
        }
        "revise_batch" => {
            tx.execute(
                "UPDATE architect_item_changes SET state = 'revised' WHERE work = ?1 AND state = 'collected'",
                [w],
            )
            .map_err(sql)?;
            for x in items(at) {
                tx.execute(
                    "INSERT INTO architect_item_changes (work, round, matter, change, state)
                     VALUES (?1, ?2, ?3, ?4, 'collected')",
                    params![
                        ctx.round.work,
                        ctx.round.n,
                        x["matter"].as_i64(),
                        x["change"].to_string()
                    ],
                )
                .map_err(sql)?;
            }
        }
        "apply_batch" => {
            let (_, project) = wake_of(ctx)?;
            let rows = collected(tx, w)?;
            let keys = make(tx, &rows)?;
            let matters: BTreeSet<i64> = rows.iter().map(|r| r.2).collect();
            send_held_replies(tx, &matters, &keys)?;
            batch_over(tx, w, &project)?;
        }
        "apply_reviewed" => give_up(ctx, tx)?,
        "new_architect_session" => sessions::choose::end_line(tx, w, ARCHITECT)?,
        _ => return Ok(false),
    }
    Ok(true)
}

// The architect's checks.

/// The articles of a project's constitution, in its order: its level-two headings that start with "Article".
fn articles(e: &Engine, project: &str) -> Vec<String> {
    let Some(text) = constitution(e, project) else {
        return Vec::new();
    };
    text.lines()
        .filter_map(|line| line.strip_prefix("## "))
        .filter(|h| h.trim_start().to_lowercase().starts_with("article"))
        .map(|h| h.trim().to_string())
        .collect()
}

/// Whether a row names an article: by its number, as "Article 2", or by its title.
fn names_article(principle: &str, article: &str) -> bool {
    let p = principle.to_lowercase();
    let a = article.to_lowercase();
    let (number, title) = a.split_once('.').unwrap_or((a.as_str(), ""));
    let number = number.trim();
    let by_number = p
        .match_indices(number)
        .any(|(i, _)| !p[i + number.len()..].chars().next().is_some_and(|c| c.is_ascii_digit()));
    let title = title.trim();
    by_number || (!title.is_empty() && p.contains(title))
}

/// The texts of the owner's words in a wake's asks and in the owner's matters: what a policy may not quote.
fn owner_words(conn: &Connection, work: i64) -> Result<Vec<String>, String> {
    let rows = query_json(
        conn,
        "SELECT json_extract(answer, '$.text') AS text FROM owner_asks WHERE work = ?1 AND answer IS NOT NULL
         UNION ALL SELECT json_extract(body, '$.text') FROM architect_matters WHERE kind = 'owner_words'
         UNION ALL SELECT json_extract(body, '$.answer') FROM architect_matters WHERE kind = 'owner_words'",
        &[&work],
        10_000,
    )?;
    Ok(rows
        .iter()
        .filter_map(|r| r["text"].as_str().map(str::to_string))
        .collect())
}

/// The sentences of a text, each trimmed.
fn sentences(text: &str) -> Vec<String> {
    text.split(['.', '!', '?', '\n', '\u{3002}'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

/// Whether a text names a date, as `2026-10-27`.
fn names_date(text: &str) -> bool {
    let b = text.as_bytes();
    (0..b.len().saturating_sub(9)).any(|i| {
        let d = |k: usize| b[i + k].is_ascii_digit();
        d(0) && d(1) && d(2) && d(3) && b[i + 4] == b'-' && d(5) && d(6) && b[i + 7] == b'-' && d(8) && d(9)
    })
}

/// The cells of the flow a text names: a cell of the architect's graph by its id, as `D1`, or any cell as `cell 15`.
fn names_cell(e: &Engine, text: &str) -> Option<String> {
    let f = e.flows.get(&e.flow_now)?;
    let words: Vec<&str> = text
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    for g in &f.graphs {
        for c in &g.cells {
            let own = g.name == ARCHITECT && words.contains(&c.id.as_str());
            let named = words
                .windows(2)
                .any(|w| w[0].eq_ignore_ascii_case("cell") && w[1] == c.id);
            if own || named {
                return Some(c.id.clone());
            }
        }
    }
    None
}

/// The point numbers of a policy.
fn point_numbers(policy: &Value) -> Vec<i64> {
    items(policy.get("points"))
        .iter()
        .filter_map(|p| p["no"].as_i64())
        .collect()
}

/// The place beside another in a return: `policy` beside `probes`, at the same depth.
fn beside(at: Option<&str>, from: &str, to: &str) -> String {
    match at.and_then(|a| a.rsplit_once('.')) {
        Some((head, _)) => format!("{head}.{to}"),
        None => at.unwrap_or(from).replace(from, to),
    }
}

/// Whether a project's observation is open.
fn open_observation(conn: &Connection, project: &str, id: i64) -> Result<bool, String> {
    Ok(count(
        conn,
        "SELECT count(*) FROM observations_open WHERE project = ?1 AND id = ?2",
        &[&project, &id],
    )? > 0)
}

/// What is wrong with the effects of an answer, every reason at once: its item changes, as the batch would have them
/// after the ones it collected; its changes to observations; its actions; its messages; its notes.
fn effects_problems(c: &checks::Ctx<'_>, project: &str, effects: &Value) -> Result<Vec<String>, String> {
    let conn = c.e.db.conn();
    let mut problems = Vec::new();
    let new = items(effects.get("item_changes"));
    if !new.is_empty() {
        let mut all: Vec<Value> = collected(conn, c.w.id)?.into_iter().map(|r| r.3).collect();
        let before = all.len();
        all.extend(new);
        if let Err(why) = crate::arch_changes::validate(c.e, project, &all) {
            problems.extend(why.lines().map(|l| match l.strip_prefix("change ") {
                Some(rest) => {
                    let (n, rest) = rest.split_once(' ').unwrap_or((rest, ""));
                    match n.parse::<usize>() {
                        Ok(k) if k > before => format!("item change {} {rest}", k - before),
                        Ok(k) => format!("change {k} the batch collected before, with these: {rest}"),
                        Err(_) => l.to_string(),
                    }
                }
                None => l.to_string(),
            }));
        }
    }
    let tags: BTreeSet<String> =
        c.e.config
            .config
            .projects
            .get(project)
            .map(|p| p.observation_tags.iter().map(|t| t.name.clone()).collect())
            .unwrap_or_default();
    for (i, o) in items(effects.get("observation_changes")).iter().enumerate() {
        let n = i + 1;
        match text(&o["kind"]) {
            "new" => {
                if text(&o["what"]).trim().is_empty() {
                    problems.push(format!("observation change {n}: it says nothing of what it is"));
                }
                for t in texts(&o["tags"]) {
                    if !tags.contains(&t) {
                        problems.push(format!(
                            "observation change {n}: {t} is not one of the project's tags: {}",
                            tags.iter().cloned().collect::<Vec<_>>().join(", ")
                        ));
                    }
                }
            }
            "merge" => {
                let id = o["into"].as_i64().unwrap_or_default();
                if !open_observation(conn, project, id)? {
                    problems.push(format!(
                        "observation change {n}: {id} is no open observation of the project"
                    ));
                }
            }
            "close" => {
                let id = o["id"].as_i64().unwrap_or_default();
                if !open_observation(conn, project, id)? {
                    problems.push(format!(
                        "observation change {n}: {id} is no open observation of the project"
                    ));
                }
            }
            other => problems.push(format!("observation change {n}: no such kind {other}")),
        }
    }
    for (i, a) in items(effects.get("actions")).iter().enumerate() {
        let n = i + 1;
        let item = a["item"].as_i64();
        let running = match item {
            Some(it) => running_work(conn, it)?,
            None => None,
        };
        let own = match item {
            Some(it) => {
                count(
                    conn,
                    "SELECT count(*) FROM items WHERE id = ?1 AND project = ?2",
                    &[&it, &project],
                )? > 0
            }
            None => true,
        };
        if !own {
            problems.push(format!(
                "action {n}: item {} is not one of this project's items",
                item.unwrap_or_default()
            ));
            continue;
        }
        match (text(&a["kind"]), &running) {
            ("pause_work" | "interject", None) | ("resume_work", None) => problems.push(format!(
                "action {n}: item {} has no work running",
                item.unwrap_or_default()
            )),
            ("pause_work", Some((_, Some(_), _))) => problems.push(format!(
                "action {n}: the work on item {} is paused already",
                item.unwrap_or_default()
            )),
            ("resume_work", Some((_, p, by))) => {
                let mine = by.as_deref() == Some("engine")
                    && p.as_deref().is_some_and(|p| p.starts_with(db::STOPPED_BY_ARCHITECT));
                if !mine {
                    problems.push(format!(
                        "action {n}: the work on item {} is not paused by you",
                        item.unwrap_or_default()
                    ));
                }
            }
            ("interject", _) if text(&a["say"]).trim().is_empty() => {
                problems.push(format!("action {n}: it says nothing"))
            }
            _ => {}
        }
    }
    for (i, m) in items(effects.get("messages")).iter().enumerate() {
        let to = text(&m["project"]);
        if to == project || !c.e.config.config.projects.contains_key(to) {
            problems.push(format!("message {}: {to} is no other project", i + 1));
        }
    }
    for (i, note) in items(effects.get("notes")).iter().enumerate() {
        if let Some(it) = note["item"].as_i64()
            && count(conn, "SELECT count(*) FROM items WHERE id = ?1", &[&it])? == 0
        {
            problems.push(format!("note {}: there is no item {it}", i + 1));
        }
    }
    Ok(problems)
}

/// The keys the create_item changes of a round's effects give.
fn created_keys(effects: &Value) -> BTreeSet<String> {
    items(effects.get("item_changes"))
        .iter()
        .filter(|c| c["kind"] == "create_item")
        .map(|c| text(&c["key"]).trim().to_string())
        .collect()
}

/// Runs a check of the architect's graph.
pub fn check(c: &checks::Ctx<'_>, name: &str, at: Option<&str>, v: Option<&Value>) -> Result<(), String> {
    let conn = c.e.db.conn();
    match name {
        "constitution_covered" => {
            let articles = articles(c.e, project_of(c.w)?);
            let rows = items(v);
            if rows.len() != articles.len() {
                return Err(format!(
                    "there is one row for each article of the project's constitution, {} of them, and there are {}",
                    articles.len(),
                    rows.len()
                ));
            }
            for (i, (row, article)) in rows.iter().zip(&articles).enumerate() {
                let named = row["article"].as_str().unwrap_or_default();
                if !names_article(named, article) {
                    return Err(format!(
                        "row {} is for \"{article}\", in the constitution's order, and names \"{named}\"",
                        i + 1
                    ));
                }
            }
            Ok(())
        }
        "picked_pickable" => {
            let Some(m) = v.and_then(|p| p["matter"].as_i64()) else {
                return Ok(());
            };
            let can: Vec<i64> = pickable(conn, project_of(c.w)?)?
                .iter()
                .filter_map(|r| r["matter"].as_i64())
                .collect();
            if can.contains(&m) {
                Ok(())
            } else {
                Err(format!(
                    "matter {m} cannot be picked now; those that can: {}",
                    can.iter().map(i64::to_string).collect::<Vec<_>>().join(", ")
                ))
            }
        }
        "effects_valid" => {
            let Some(effects) = v else { return Ok(()) };
            let problems = effects_problems(c, project_of(c.w)?, effects)?;
            if problems.is_empty() {
                Ok(())
            } else {
                Err(problems.join("\n"))
            }
        }
        "reply_fits_changes" => {
            let Some(reply) = v else { return Ok(()) };
            let keys = created_keys(&c.unwrapped["effects"]);
            for list in ["children", "waits"] {
                for x in items(reply.get(list)) {
                    let k = text(&x["key"]).trim().trim_start_matches('#');
                    let known = match k.parse::<i64>() {
                        Ok(n) => count(conn, "SELECT count(*) FROM items WHERE id = ?1", &[&n])? > 0,
                        Err(_) => keys.contains(k),
                    };
                    if !known {
                        return Err(format!(
                            "the reply's {list} names {k}, which is no item and no key an item change of this round \
                             makes"
                        ));
                    }
                }
            }
            Ok(())
        }
        "review_covers_changes" => {
            let n = collected(conn, c.w.id)?.len() as i64;
            let mut seen: Vec<i64> = items(v).iter().filter_map(|r| r["change"].as_i64()).collect();
            seen.sort_unstable();
            if seen != (1..=n).collect::<Vec<_>>() {
                return Err(format!(
                    "there is one row for each of the batch's {n} changes, by its number from 1, and the rows name \
                     {seen:?}"
                ));
            }
            Ok(())
        }
        "changes_valid" => {
            let list = items(v);
            let changes: Vec<Value> = list.iter().map(|x| x["change"].clone()).collect();
            let mut problems: Vec<String> = Vec::new();
            if let Err(why) = crate::arch_changes::validate(c.e, project_of(c.w)?, &changes) {
                problems.push(why);
            }
            let from = batch_start(conn, c.w.id)?;
            let mine: BTreeSet<i64> = query_json(
                conn,
                "SELECT DISTINCT matter FROM architect_item_changes WHERE work = ?1 AND round > ?2",
                &[&c.w.id, &from],
                100_000,
            )?
            .iter()
            .filter_map(|r| r["matter"].as_i64())
            .collect();
            for (i, x) in list.iter().enumerate() {
                let m = x["matter"].as_i64().unwrap_or_default();
                if !mine.contains(&m) {
                    problems.push(format!("change {}: matter {m} is no matter of this batch", i + 1));
                }
            }
            if problems.is_empty() {
                Ok(())
            } else {
                Err(problems.join("\n"))
            }
        }
        "candidates_known" => {
            let list = items(v);
            if list.len() > 10 {
                return Err(format!("at most ten candidates, and there are {}", list.len()));
            }
            let project = project_of(c.w)?;
            for x in list {
                let id = x["id"].as_i64().unwrap_or_default();
                if !open_observation(conn, project, id)? {
                    return Err(format!("{id} is no open observation of the project"));
                }
            }
            Ok(())
        }
        "rethink_when_asked" => {
            let from = batch_start(conn, c.w.id)?;
            let asked = count(
                conn,
                "SELECT count(*) FROM rounds WHERE work = ?1 AND n > ?2 AND cell = 'O1' AND transition IS NOT NULL
                   AND json_extract(output, '$.outcome.take.way') = 'rethink'",
                &[&c.w.id, &from],
            )?;
            let yes = v.is_some_and(|r| r["verdict"] == "yes");
            if asked > 0 && !yes {
                return Err("the owner asked in this batch to rethink the policy: the answer is yes".into());
            }
            Ok(())
        }
        "policy_text_clean" => {
            let Some(policy) = v else { return Ok(()) };
            let text = policy.to_string();
            for p in items(at_path(&c.unwrapped, &beside(at, "policy", "probes")).as_ref()) {
                let ask = p["ask"].as_str().unwrap_or_default();
                if ask.len() >= 20 && text.contains(ask) {
                    return Err(format!("the policy holds the probe {}", p["id"]));
                }
            }
            for said in owner_words(conn, c.w.id)? {
                if let Some(s) = sentences(&said)
                    .into_iter()
                    .find(|s| s.chars().count() >= 30 && text.contains(s.as_str()))
                {
                    return Err(format!("the policy quotes the owner's words: \"{s}\""));
                }
            }
            if let Some(cell) = names_cell(c.e, &text) {
                return Err(format!("the policy names the cell {cell} of the flow"));
            }
            if names_date(&text) {
                return Err("the policy names a date".into());
            }
            Ok(())
        }
        "probes_cover_points" => {
            let probes = items(v);
            let policy = at_path(&c.unwrapped, &beside(at, "probes", "policy")).unwrap_or(Value::Null);
            let numbers = point_numbers(&policy);
            for p in &probes {
                for n in items(p.get("points")).iter().filter_map(Value::as_i64) {
                    if !numbers.contains(&n) {
                        return Err(format!(
                            "the probe {} names point {n}, which the text does not have",
                            p["id"]
                        ));
                    }
                }
            }
            for n in numbers {
                let probed = probes
                    .iter()
                    .any(|p| items(p.get("points")).iter().any(|x| x.as_i64() == Some(n)));
                if !probed {
                    return Err(format!("point {n} has no probe"));
                }
            }
            Ok(())
        }
        "failed_probes_gone" => {
            let gaps = query_json(
                conn,
                "SELECT output FROM rounds WHERE work = ?1 AND cell = 'P6' AND transition = 'gaps'",
                &[&c.w.id],
                ROWS,
            )?;
            let wrong: BTreeSet<String> = gaps
                .iter()
                .flat_map(|r| texts(&at_path(&c.g.unwrap(&r["output"]), "verdict.wrong_probes").unwrap_or_default()))
                .collect();
            let still: Vec<String> = items(v)
                .iter()
                .filter_map(|p| p["id"].as_str().map(str::trim))
                .filter(|id| wrong.contains(*id))
                .map(str::to_string)
                .collect();
            if still.is_empty() {
                Ok(())
            } else {
                Err(format!(
                    "these probes were answered wrongly, and are not used again: {}",
                    still.join(", ")
                ))
            }
        }
        "reading_complete" => {
            let reading = v.cloned().unwrap_or(Value::Null);
            let policy = query_json(conn, "SELECT policy FROM policy_text WHERE work = ?1", &[&c.w.id], 1)?
                .first()
                .map(|r| r["policy"].clone())
                .unwrap_or(Value::Null);
            let probes = query_json(
                conn,
                "SELECT probes FROM policy_probes_asked WHERE work = ?1",
                &[&c.w.id],
                1,
            )?
            .first()
            .map(|r| items(r.get("probes")))
            .unwrap_or_default();
            let restated: Vec<i64> = items(reading.get("restate"))
                .iter()
                .filter_map(|r| r["no"].as_i64())
                .collect();
            if let Some(n) = point_numbers(&policy).into_iter().find(|n| !restated.contains(n)) {
                return Err(format!("point {n} is not restated"));
            }
            let answered: Vec<String> = items(reading.get("answers"))
                .iter()
                .filter_map(|a| a["id"].as_str().map(str::to_string))
                .collect();
            if let Some(p) = probes
                .iter()
                .filter_map(|p| p["id"].as_str())
                .find(|id| !answered.iter().any(|a| a == id))
            {
                return Err(format!("the probe {p} is not answered"));
            }
            Ok(())
        }
        other => Err(format!("no such engine check: {other}")),
    }
}

// The architect's inputs.

/// What the engine computes for an input of the architect's cells; none when there is nothing.
pub fn input(e: &Engine, w: &Work, g: &CompiledGraph, name: &str) -> Result<Option<Value>, String> {
    let project = project_of(w)?;
    match name {
        "constitution" => Ok(constitution(e, project).map(Value::String)),
        "guide" => guide(e, project).map(Some),
        "cited_nodes" => cited_nodes(e, w, project),
        "since_last_wake" => since_last_wake(e, w, project).map(Some),
        "board" => board(e, project).map(Some),
        "numbers" => numbers(e, w, project).map(Some),
        "observations_overview" => observations_overview(e, project).map(Some),
        "observation_tags" => Ok(observation_tags(e, project)),
        "policy" => policy(e, project),
        "policy_signals" => policy_signals(e, w, project),
        "slots" => slots(e).map(Some),
        "queue" => queue(e, project).map(Some),
        "batch" => batch(e, w, project).map(Some),
        "matter" => matter(e, w),
        "matter_items" => matter_items(e, w),
        "item_log" => item_log(e, w),
        "similar_items" => similar_items(e, w, g).map(Some),
        "owner_answer" => owner_answer(e, w),
        "cross_project" => cross_project(e, project).map(Some),
        "batch_changes" => batch_changes(e, w).map(Some),
        "changes_touch" => changes_touch(e, w),
        other => Err(format!("no such engine input: {other}")),
    }
}

/// The engineer's graph of the flow new works start on.
fn engineer_graph(e: &Engine) -> Option<&CompiledGraph> {
    e.flows.get(&e.flow_now).and_then(|f| f.graph(ENGINEER))
}

/// A project's constitution, as its configuration names it; none when it names none.
fn constitution(e: &Engine, project: &str) -> Option<String> {
    let p = e.config.config.projects.get(project)?;
    let path = p.constitution.as_ref()?;
    std::fs::read_to_string(p.checkout.join(path)).ok()
}

/// The ref of a project's main its checkout has: the remote's, when it was fetched, or else its own.
fn main_ref(p: &config::Project) -> String {
    let remote = format!("origin/{}", p.main);
    if git::git(&p.checkout, &["rev-parse", "--verify", "-q", &remote]).is_ok() {
        remote
    } else {
        p.main.clone()
    }
}

/// A project's guide as its main has it: its design goal, its AGENTS.md and the index of its design tree.
fn guide(e: &Engine, project: &str) -> Result<Value, String> {
    let p = e
        .config
        .config
        .projects
        .get(project)
        .ok_or_else(|| format!("no project {project} in the configuration"))?;
    let g = config::project_guide_at(&p.checkout, &main_ref(p), &p.design_paths, "")
        .unwrap_or_else(|_| config::project_guide(&p.checkout, &p.design_paths, ""));
    Ok(json!({
        "design_goal": p.design_goal,
        "agents_md": g["agents_md"],
        "design_tree": g["design_tree"],
    }))
}

/// The text of the matter picked, as nodes are cited by it and similar items looked up by.
fn matter_text(e: &Engine, w: &Work) -> Result<String, String> {
    let conn = e.db.conn();
    let Some(m) = picked(conn, w.id)? else {
        return Ok(String::new());
    };
    let row = query_json(conn, "SELECT what, body FROM architect_matters WHERE id = ?1", &[&m], 1)?;
    Ok(row.first().map(Value::to_string).unwrap_or_default())
}

/// The design tree nodes the matter picked names, in full, as the project's main has them; none when it names none.
fn cited_nodes(e: &Engine, w: &Work, project: &str) -> Result<Option<Value>, String> {
    let Some(p) = e.config.config.projects.get(project) else {
        return Ok(None);
    };
    let cites = matter_text(e, w)?;
    let g = config::project_guide_at(&p.checkout, &main_ref(p), &p.design_paths, &cites)
        .unwrap_or_else(|_| config::project_guide(&p.checkout, &p.design_paths, &cites));
    let nodes = g["cited_nodes"].clone();
    Ok((!empty(&nodes)).then_some(nodes))
}

/// What an engineer's work waits for, in words, besides a pause, which its row says; none when it waits for nothing.
fn work_waits(e: &Engine, work: i64, at: &str) -> Result<Option<String>, String> {
    let ask: Option<i64> =
        e.db.conn()
            .query_row(
                "SELECT max(id) FROM owner_asks WHERE work = ?1 AND answer IS NULL",
                [work],
                |r| r.get(0),
            )
            .map_err(sql)?;
    if let Some(a) = ask {
        return Ok(Some(format!("the owner's answer to ask {a}")));
    }
    let asks_architect = engineer_graph(e)
        .and_then(|g| g.cell(at))
        .is_some_and(|c| c.step.as_deref() == Some("ask_architect"));
    if asks_architect {
        return Ok(Some("the architect's reply to its question".into()));
    }
    Ok(None)
}

/// What a project's works spent, in US dollars, by work, as their rounds recorded it.
fn spent(e: &Engine, work: i64) -> Result<f64, String> {
    e.db.conn()
        .query_row(
            "SELECT coalesce(sum(json_extract(usage, '$.cost_usd')), 0) FROM rounds WHERE work = ?1",
            [work],
            |r| r.get(0),
        )
        .map_err(sql)
}

/// A project's items running and queued, each by its number, with its rank, its cell and stage, what it waits for and
/// who waits for it, since when its work runs and what it spent, or why its last work stopped, as one queued again;
/// the release items, each an item nothing depends on with items under it, with the items under it; and the
/// dependencies with other projects' items. A work at a cell of no stage, which asks or runs tools for another, is at
/// the stage of the cell it stopped at, or else of the innermost cell that waits for it with one.
fn board(e: &Engine, project: &str) -> Result<Value, String> {
    let g = engineer_graph(e);
    let end = g.map(|g| g.end.clone()).unwrap_or_default();
    let rows = query_json(
        e.db.conn(),
        "SELECT i.id AS item, i.title, i.rank, i.held, i.level, i.deliver, w.id AS work, w.at, w.resume_at, w.waiting,
                w.paused, w.ended, (SELECT min(r.started_at) FROM rounds r WHERE r.work = w.id) AS since,
                (SELECT json_group_array(l.depends_on) FROM item_links l WHERE l.item = i.id) AS waits_for,
                (SELECT json_group_array(l.item) FROM item_links l WHERE l.depends_on = i.id) AS waited_by,
                EXISTS (SELECT 1 FROM ready_items r WHERE r.item = i.id) AS ready
         FROM items i LEFT JOIN works w ON w.id = (SELECT max(x.id) FROM works x WHERE x.item = i.id)
         WHERE i.project = ?1 AND i.cancelled IS NULL
         ORDER BY i.rank NULLS LAST, i.id",
        &[&project],
        10_000,
    )?;
    let mut out = Map::new();
    let mut open: BTreeSet<i64> = BTreeSet::new();
    for mut r in rows {
        let at = r["at"].as_str().unwrap_or_default().to_string();
        if !at.is_empty() && at == end {
            continue;
        }
        let id = r["item"].as_i64().unwrap_or_default();
        open.insert(id);
        let stopped = r["ended"].as_str().map(str::to_string);
        if let (Some(work), None) = (r["work"].as_i64(), &stopped) {
            let stage_of = |c: &str| g.and_then(|g| g.cell(c)).and_then(|c| c.stage.clone());
            let callers = items(r.get("waiting"));
            let stage = stage_of(&at)
                .or_else(|| r["resume_at"].as_str().and_then(stage_of))
                .or_else(|| callers.iter().rev().find_map(|c| c.as_str().and_then(stage_of)));
            r["stage"] = json!(stage);
            r["waits"] = json!(work_waits(e, work, &at)?);
            r["since"] = json!(r["since"].as_i64().map(utc_minute));
            r["spent_usd"] = json!((spent(e, work)? * 100.0).round() / 100.0);
        } else if let Some(o) = r.as_object_mut() {
            for k in ["work", "at", "paused", "since"] {
                o.remove(k);
            }
        }
        if let Some(o) = r.as_object_mut() {
            for k in ["resume_at", "waiting", "ended"] {
                o.remove(k);
            }
        }
        if let Some(why) = stopped {
            r["last_work_ended"] = json!(why);
        }
        r["ready"] = json!(r["ready"] == 1);
        if let Some(o) = r.as_object_mut() {
            o.remove("item");
            o.retain(|_, v| !v.is_null() && *v != json!([]));
        }
        out.insert(format!("#{id}"), r);
        if out.len() >= ROWS {
            break;
        }
    }
    let releases = releases(e, project, &open)?;
    Ok(json!({ "items": out, "releases": releases, "other_projects": cross_project(e, project)? }))
}

/// A project's release items: each item nothing depends on that items are under, with the items under it, how many of
/// them are done, and which are open.
fn releases(e: &Engine, project: &str, open: &BTreeSet<i64>) -> Result<Value, String> {
    let conn = e.db.conn();
    let roots = query_json(
        conn,
        "SELECT i.id, i.title FROM items i WHERE i.project = ?1 AND i.cancelled IS NULL
           AND NOT EXISTS (SELECT 1 FROM item_links l WHERE l.depends_on = i.id)
           AND EXISTS (SELECT 1 FROM item_links l WHERE l.item = i.id)
         ORDER BY i.rank NULLS LAST, i.id",
        &[&project],
        ROWS,
    )?;
    let mut out = Map::new();
    for r in roots {
        let id = r["id"].as_i64().unwrap_or_default();
        let under = query_json(
            conn,
            "WITH RECURSIVE down(id) AS (SELECT depends_on FROM item_links WHERE item = ?1
               UNION SELECT l.depends_on FROM item_links l JOIN down ON l.item = down.id)
             SELECT id FROM down ORDER BY id",
            &[&id],
            10_000,
        )?;
        let ids: Vec<i64> = under.iter().filter_map(|x| x["id"].as_i64()).collect();
        let open_under: Vec<i64> = ids.iter().copied().filter(|x| open.contains(x)).collect();
        out.insert(
            format!("#{id}"),
            json!({ "title": r["title"], "under": ids.len(), "done": ids.len() - open_under.len(), "open": open_under }),
        );
    }
    Ok(Value::Object(out))
}

/// The dependencies between a project's items and other projects' items: what its items wait for there, and what
/// waits there for its items.
fn cross_project(e: &Engine, project: &str) -> Result<Value, String> {
    let conn = e.db.conn();
    let waits_for = query_json(
        conn,
        "SELECT l.item, l.depends_on AS on_item, d.project, d.title, l.needs,
                (SELECT w.at FROM works w WHERE w.item = d.id ORDER BY w.id DESC LIMIT 1) AS at, d.cancelled
         FROM item_links l JOIN items i ON i.id = l.item JOIN items d ON d.id = l.depends_on
         WHERE i.project = ?1 AND d.project <> ?1 ORDER BY l.item, l.depends_on",
        &[&project],
        ROWS,
    )?;
    let waited_by = query_json(
        conn,
        "SELECT l.depends_on AS item, l.item AS by_item, i.project, i.title, l.needs
         FROM item_links l JOIN items i ON i.id = l.item JOIN items d ON d.id = l.depends_on
         WHERE d.project = ?1 AND i.project <> ?1 ORDER BY l.depends_on, l.item",
        &[&project],
        ROWS,
    )?;
    Ok(json!({ "waits_for": waits_for, "waited_by": waited_by }))
}

/// When a wake started: its first round's start; now, before it has one.
fn wake_start(conn: &Connection, work: i64) -> Result<i64, String> {
    let at: Option<i64> = conn
        .query_row("SELECT min(started_at) FROM rounds WHERE work = ?1", [work], |r| {
            r.get(0)
        })
        .map_err(sql)?;
    Ok(at.unwrap_or_else(db::now))
}

/// What happened in a project since its architect's last wake started; at its first wake, in the week before it.
fn since_last_wake(e: &Engine, w: &Work, project: &str) -> Result<Value, String> {
    let conn = e.db.conn();
    let last: Option<i64> = conn
        .query_row(
            "SELECT min(r.started_at) FROM rounds r
             WHERE r.work = (SELECT max(x.id) FROM works x WHERE x.graph = 'architect' AND x.project = ?1 AND x.id < ?2)",
            params![project, w.id],
            |r| r.get(0),
        )
        .map_err(sql)?;
    let first = last.is_none();
    let since = match last {
        Some(t) => t,
        None => wake_start(conn, w.id)? - 7 * 24 * HOUR,
    };
    let p = e.config.config.projects.get(project);
    let log = |args: &[&str]| -> Value {
        let Some(p) = p else { return Value::Null };
        let main = main_ref(p);
        let mut a = vec!["log", "--format=%h %cs %s"];
        a.extend_from_slice(args);
        a.push(&main);
        git::git(&p.checkout, &a)
            .map(|t| json!(t.lines().map(str::to_string).collect::<Vec<_>>()))
            .unwrap_or(Value::Null)
    };
    let after = format!("--since=@{}", since / 1000);
    let merged = query_json(
        conn,
        "SELECT w.item, i.title, w.pr, json_extract(r.output, '$.result.merge_commit') AS commit_merged
         FROM rounds r JOIN works w ON w.id = r.work JOIN items i ON i.id = w.item
         WHERE i.project = ?1 AND r.cell = '9' AND r.transition = 'merged' AND r.ended_at > ?2 ORDER BY r.ended_at",
        &[&project, &since],
        ROWS,
    )?;
    let works = query_json(
        conn,
        "SELECT w.id AS work, w.item, i.title, w.at, w.paused, w.ended FROM works w JOIN items i ON i.id = w.item
         WHERE i.project = ?1 AND w.graph = 'engineer'
           AND EXISTS (SELECT 1 FROM rounds r WHERE r.work = w.id AND r.started_at > ?2)
           AND (w.ended IS NOT NULL OR w.at = ?3)
         ORDER BY w.id",
        &[&project, &since, &ENGINEER_END],
        ROWS,
    )?;
    let main_ci = query_json(
        conn,
        "SELECT w.item, r.transition, json_extract(r.output, '$.main.kind') AS kind, r.ended_at AS at
         FROM rounds r JOIN works w ON w.id = r.work JOIN items i ON i.id = w.item
         WHERE i.project = ?1 AND r.cell = '10' AND r.ended_at > ?2 ORDER BY r.ended_at",
        &[&project, &since],
        ROWS,
    )?;
    let owner = query_json(
        conn,
        "SELECT m.id AS matter, m.what FROM architect_matters m
         WHERE m.project = ?1 AND m.kind = 'owner_words' AND m.at > ?2 ORDER BY m.id",
        &[&project, &since],
        ROWS,
    )?;
    let cards = query_json(
        conn,
        "SELECT c.id AS design_card, w.item, c.status FROM design_cards c JOIN works w ON w.id = c.work
         JOIN items i ON i.id = w.item
         WHERE i.project = ?1 AND c.status = 'ruled' AND EXISTS (
           SELECT 1 FROM changes x WHERE x.table_name = 'design_cards' AND x.row_key = json_object('id', c.id)
             AND x.at > ?2)
         ORDER BY c.id",
        &[&project, &since],
        ROWS,
    )?;
    let reports = query_json(
        conn,
        "SELECT id AS matter, what FROM architect_matters WHERE project = ?1 AND kind = 'report' AND at > ?2 ORDER BY id",
        &[&project, &since],
        ROWS,
    )?;
    let mut out = json!({
        "since": utc_minute(since),
        "main": log(&[&after]),
        "merged": merged,
        "works_ended": works,
        "main_after_merges": main_ci,
        "owner_said": owner,
        "design_cards_ruled": cards,
        "reports": reports,
    });
    if first {
        out["first_wake"] = json!(true);
    }
    Ok(out)
}

/// The numbers the engine measures for a project, each as the wake started and earlier for the trend, so that they hold
/// through the wake: the size of its design paths and of every file on main; its items made, merged and cancelled, and
/// what its items' works and its architect's wakes spent, each for the four weeks before the wake one by one.
fn numbers(e: &Engine, w: &Work, project: &str) -> Result<Value, String> {
    let conn = e.db.conn();
    let week = 7 * 24 * HOUR;
    let now = wake_start(conn, w.id)?;
    let mut weeks = Vec::new();
    for k in 0..4 {
        // Each week ends the millisecond before the next begins, the last before the wake.
        let (to, from) = (now - k * week - 1, now - (k + 1) * week);
        let made = count(
            conn,
            "SELECT count(*) FROM items i WHERE i.project = ?1 AND (SELECT min(c.at) FROM changes c
               WHERE c.table_name = 'items' AND c.row_key = json_object('id', i.id)) BETWEEN ?2 AND ?3",
            &[&project, &from, &to],
        )?;
        let merged = count(
            conn,
            "SELECT count(*) FROM rounds r JOIN works w ON w.id = r.work JOIN items i ON i.id = w.item
             WHERE i.project = ?1 AND r.cell = '9' AND r.transition = 'merged' AND r.ended_at BETWEEN ?2 AND ?3",
            &[&project, &from, &to],
        )?;
        let cancelled = count(
            conn,
            "SELECT count(*) FROM changes c WHERE c.table_name = 'items' AND c.at BETWEEN ?2 AND ?3
               AND json_extract(c.new_row, '$.project') = ?1 AND json_extract(c.new_row, '$.cancelled') IS NOT NULL
               AND json_extract(c.old_row, '$.cancelled') IS NULL",
            &[&project, &from, &to],
        )?;
        let (items, wakes): (f64, f64) = conn
            .query_row(
                "SELECT coalesce(sum(CASE WHEN w.graph = 'engineer' THEN json_extract(r.usage, '$.cost_usd') END), 0),
                        coalesce(sum(CASE WHEN w.graph = 'architect' THEN json_extract(r.usage, '$.cost_usd') END), 0)
                 FROM rounds r JOIN works w ON w.id = r.work LEFT JOIN items i ON i.id = w.item
                 WHERE coalesce(i.project, w.project) = ?1 AND r.started_at BETWEEN ?2 AND ?3",
                params![project, from, to],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(sql)?;
        let usd = |x: f64| (x * 100.0).round() / 100.0;
        weeks.push(json!({
            "weeks_ago": k, "items_made": made, "merged": merged, "cancelled": cancelled,
            "spent_usd": { "items": usd(items), "architect": usd(wakes) },
        }));
    }
    let size = e.config.config.projects.get(project).map(|p| {
        let main = main_ref(p);
        let at = |rev: &str| -> Value {
            let lines = |paths: &[&str]| -> Option<(usize, usize)> {
                let mut args = vec!["ls-tree", "-r", "-l", rev, "--"];
                args.extend_from_slice(paths);
                let out = git::git(&p.checkout, &args).ok()?;
                let mut files = 0;
                let mut bytes = 0;
                for l in out.lines() {
                    let size = l.split_whitespace().nth(3).and_then(|s| s.parse::<usize>().ok());
                    if let Some(s) = size {
                        files += 1;
                        bytes += s;
                    }
                }
                Some((files, bytes))
            };
            let design: Vec<&str> = p.design_paths.iter().map(String::as_str).collect();
            match (lines(&design), lines(&[])) {
                (Some((df, db)), Some((af, ab))) => json!({
                    "design_files": df, "design_bytes": db, "all_files": af, "all_bytes": ab
                }),
                _ => Value::Null,
            }
        };
        let back = |days: i64| -> Value {
            let before = format!("--before=@{}", (now - days * 24 * HOUR) / 1000);
            match git::git(&p.checkout, &["rev-list", "-1", &before, &main]) {
                Ok(rev) if !rev.trim().is_empty() => at(rev.trim()),
                _ => Value::Null,
            }
        };
        json!({ "now": back(0), "a_week_ago": back(7), "a_month_ago": back(30) })
    });
    Ok(json!({
        "size_on_main": size,
        "weeks": weeks,
        "not_measured": "The engine measures nothing else yet: no test count, no CI time, no specification beyond the \
                         design paths' size."
    }))
}

/// A project's observations: how many are open under each tag and how many times they were reported; the ten
/// recorded or reported again last; the ten closed last, with why.
fn observations_overview(e: &Engine, project: &str) -> Result<Value, String> {
    let conn = e.db.conn();
    let open = query_json(
        conn,
        "SELECT id, tags, times_reported FROM observations_open WHERE project = ?1",
        &[&project],
        100_000,
    )?;
    let mut per_tag: BTreeMap<String, (i64, i64)> = BTreeMap::new();
    for o in &open {
        let mut tags = texts(&o["tags"]);
        if tags.is_empty() {
            tags.push("(no tag)".into());
        }
        for t in tags {
            let x = per_tag.entry(t).or_default();
            x.0 += 1;
            x.1 += o["times_reported"].as_i64().unwrap_or(1);
        }
    }
    let per_tag: Map<String, Value> = per_tag
        .into_iter()
        .map(|(t, (n, times))| (t, json!({ "open": n, "times_reported": times })))
        .collect();
    let recent = query_json(
        conn,
        "SELECT id, substr(what, 1, 200) AS what, tags, times_reported FROM observations_all
         WHERE project = ?1 AND closed IS NULL ORDER BY updated DESC LIMIT 10",
        &[&project],
        10,
    )?;
    let closed = query_json(
        conn,
        "SELECT id, substr(what, 1, 200) AS what, closed, closed_why FROM observations_all
         WHERE project = ?1 AND closed IS NOT NULL ORDER BY coalesce(merged, updated) DESC LIMIT 10",
        &[&project],
        10,
    )?;
    let keyed = |rows: Vec<Value>| -> Value {
        Value::Object(
            rows.into_iter()
                .map(|mut r| {
                    let id = r["id"].as_i64().unwrap_or_default();
                    r.as_object_mut().map(|o| o.remove("id"));
                    (format!("#{id}"), r)
                })
                .collect(),
        )
    };
    Ok(json!({ "open": open.len(), "per_tag": per_tag, "recent": keyed(recent), "closed_lately": keyed(closed) }))
}

/// A project's tags for observations, each with what it covers; none when it has none.
fn observation_tags(e: &Engine, project: &str) -> Option<Value> {
    let p = e.config.config.projects.get(project)?;
    let tags: Map<String, Value> = p
        .observation_tags
        .iter()
        .map(|t| (t.name.clone(), Value::String(t.desc.clone())))
        .collect();
    (!tags.is_empty()).then_some(Value::Object(tags))
}

/// The policy in force for a project; none before the first takes effect.
fn policy(e: &Engine, project: &str) -> Result<Option<Value>, String> {
    let rows = query_json(
        e.db.conn(),
        "SELECT policy FROM policy_current WHERE project = ?1",
        &[&project],
        1,
    )?;
    Ok(rows.first().map(|r| r["policy"].clone()).filter(|p| p.is_object()))
}

/// The signals of the policy in force for a project that the engine measures, each with its numbers: a signal whose
/// words name a measure of the numbers by its name (`merged`, or `items made` for `items_made`), with that measure week
/// by week, the latest first, or now, a week ago and a month ago. None when the engine measures none of them, or before
/// the first policy takes effect.
fn policy_signals(e: &Engine, w: &Work, project: &str) -> Result<Option<Value>, String> {
    let Some(policy) = policy(e, project)? else {
        return Ok(None);
    };
    let numbers = numbers(e, w, project)?;
    let mut measures: Vec<(String, Value)> = Vec::new();
    let weeks = items(numbers.get("weeks"));
    if let Some(first) = weeks.first().and_then(Value::as_object) {
        for k in first.keys().filter(|k| *k != "weeks_ago") {
            let by_week: Vec<Value> = weeks.iter().map(|x| x[k].clone()).collect();
            measures.push((k.clone(), json!({ "by_week": by_week })));
        }
    }
    let size = &numbers["size_on_main"];
    if let Some(now) = size["now"].as_object() {
        for k in now.keys() {
            let at = |when: &str| size[when][k].clone();
            measures.push((
                k.clone(),
                json!({ "now": at("now"), "a_week_ago": at("a_week_ago"), "a_month_ago": at("a_month_ago") }),
            ));
        }
    }
    let mut out = Map::new();
    for (i, s) in items(policy.get("signals")).iter().enumerate() {
        let said: String = format!("{} {}", text(&s["what"]), text(&s["how"]))
            .to_lowercase()
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { ' ' })
            .collect();
        let said = format!(" {} ", said.split_whitespace().collect::<Vec<_>>().join(" "));
        let found: Map<String, Value> = measures
            .iter()
            .filter(|(k, _)| said.contains(&format!(" {} ", k.replace('_', " "))))
            .cloned()
            .collect();
        if !found.is_empty() {
            out.insert(
                format!("{}", i + 1),
                json!({ "what": s["what"], "how": s["how"], "numbers": found }),
            );
        }
    }
    Ok((!out.is_empty()).then_some(Value::Object(out)))
}

/// The machines' slots, all together.
fn total_slots(e: &Engine) -> i64 {
    e.config
        .config
        .install
        .machines
        .hosts
        .values()
        .map(|h| i64::from(h.slots))
        .sum()
}

/// How many works hold a slot: the engineer's works going on by themselves that wait for nothing.
fn holding(e: &Engine) -> Result<i64, String> {
    let Some(g) = engineer_graph(e) else { return Ok(0) };
    let works = query_json(
        e.db.conn(),
        "SELECT w.id, w.at, (SELECT r.transition FROM rounds r WHERE r.work = w.id ORDER BY r.n DESC LIMIT 1) AS last,
                (SELECT r.cell FROM rounds r WHERE r.work = w.id ORDER BY r.n DESC LIMIT 1) AS last_cell
         FROM works w WHERE w.graph = 'engineer' AND w.ended IS NULL AND w.paused IS NULL",
        &[],
        10_000,
    )?;
    let mut n = 0;
    for w in works {
        let at = w["at"].as_str().unwrap_or_default();
        let Some(cell) = g.cell(at) else { continue };
        if at == g.end || matches!(cell.step.as_deref(), Some("ask_owner" | "ask_architect")) {
            continue;
        }
        let waits = w["last_cell"] == at
            && w["last"]
                .as_str()
                .is_some_and(|t| cell.rows.iter().any(|r| r.name == t && r.waits));
        if !waits {
            n += 1;
        }
    }
    Ok(n)
}

/// The machines' slots, which every project shares: how many there are, used and free, and how many items are ready,
/// by project.
fn slots(e: &Engine) -> Result<Value, String> {
    let total = total_slots(e);
    let held = holding(e)?;
    let ready = query_json(
        e.db.conn(),
        "SELECT project, count(*) AS ready FROM ready_items GROUP BY project ORDER BY project",
        &[],
        ROWS,
    )?;
    let ready: Map<String, Value> = ready
        .into_iter()
        .map(|r| (text(&r["project"]).to_string(), r["ready"].clone()))
        .collect();
    let machines: Map<String, Value> = e
        .config
        .config
        .install
        .machines
        .hosts
        .iter()
        .map(|(n, h)| (n.clone(), json!(h.slots)))
        .collect();
    Ok(json!({ "machines": machines, "total": total, "used": held, "free": total - held, "ready": ready }))
}

/// A project's queue, by matter: each matter that can be picked, with its kind and detail, the item it concerns, who
/// waits for it and since when, and what it is; those held for the owner's answer, listed as held; and whether new
/// starts are held to set things straight.
fn queue(e: &Engine, project: &str) -> Result<Value, String> {
    let conn = e.db.conn();
    let can: BTreeSet<i64> = pickable(conn, project)?
        .iter()
        .filter_map(|r| r["matter"].as_i64())
        .collect();
    let rows = query_json(
        conn,
        "SELECT * FROM architect_queue WHERE project = ?1 ORDER BY matter",
        &[&project],
        ROWS,
    )?;
    let mut matters = Map::new();
    for r in rows {
        let id = r["matter"].as_i64().unwrap_or_default();
        let mut m = json!({
            "kind": r["kind"], "detail": r["detail"], "item": r["item"], "waits": r["waits"],
            "since": r["at"].as_i64().map(utc_minute), "what": r["what"],
        });
        if r["answered"] == 1 {
            m["back_with_answer"] = json!(true);
        }
        if r["urgent"] == 1 || r["urgent"] == true {
            m["urgent"] = json!(true);
        }
        if !can.contains(&id) {
            m["not_now"] = json!("set straight first");
        }
        if let Some(o) = m.as_object_mut() {
            o.retain(|_, v| !v.is_null());
        }
        matters.insert(format!("#{id}"), m);
    }
    let held = query_json(
        conn,
        "SELECT m.id, m.kind, m.what, m.asked FROM architect_matters m JOIN owner_asks a ON a.id = m.asked
         WHERE m.project = ?1 AND m.handled_round IS NULL AND a.answer IS NULL ORDER BY m.id",
        &[&project],
        ROWS,
    )?;
    let held: Map<String, Value> = held
        .into_iter()
        .map(|r| {
            (
                format!("#{}", r["id"]),
                json!({ "kind": r["kind"], "what": r["what"], "held_by_ask": r["asked"] }),
            )
        })
        .collect();
    Ok(json!({
        "now": utc_minute(db::now()),
        "pickable": can,
        "held_to_set_straight": held_straight(conn, project)?,
        "matters": matters,
        "held_for_owner": held,
    }))
}

/// The batch so far: the numbers of the matters it handled; each, by its number, with how it ended, by the round that
/// handled it; those held for the owner's answer; the item changes it made; whether a hold to set things straight was
/// released in it; and whether the owner asked to rethink the policy. Each matter is by its number, so that a session
/// given only what changed is given the matters new to it.
fn batch(e: &Engine, w: &Work, project: &str) -> Result<Value, String> {
    let conn = e.db.conn();
    let from = batch_start(conn, w.id)?;
    let by_matter = |rows: Vec<Value>| -> Map<String, Value> {
        rows.into_iter()
            .map(|mut r| {
                let id = r["matter"].clone();
                r.as_object_mut().map(|o| o.remove("matter"));
                (format!("#{id}"), r)
            })
            .collect()
    };
    let handled = query_json(
        conn,
        "SELECT m.id AS matter, m.kind, 'R' || m.handled_work || '-' || m.handled_round AS round, r.cell,
                json_extract(r.output, '$.progress') AS said
         FROM architect_matters m JOIN rounds r ON r.work = m.handled_work AND r.n = m.handled_round
         WHERE m.handled_work = ?1 AND m.handled_round > ?2 ORDER BY m.handled_round",
        &[&w.id, &from],
        ROWS,
    )?;
    let numbers: Vec<Value> = handled.iter().map(|r| r["matter"].clone()).collect();
    let asked = query_json(
        conn,
        "SELECT m.id AS matter, m.kind, m.asked AS ask, m.resume_cell FROM architect_matters m
         JOIN rounds r ON r.work = ?1 AND r.n > ?2 AND r.cell = 'ASK'
         JOIN owner_asks a ON a.work = r.work AND a.round = r.n AND a.id = m.asked ORDER BY m.id",
        &[&w.id, &from],
        ROWS,
    )?;
    let made = query_json(
        conn,
        "SELECT c.matter, json_extract(c.change, '$.kind') AS kind, json_extract(c.change, '$.item') AS item,
                c.state, c.outcome
         FROM architect_item_changes c WHERE c.work = ?1 AND c.round > ?2 AND c.state IN ('made', 'dropped') ORDER BY c.id",
        &[&w.id, &from],
        ROWS,
    )?;
    let since: i64 = conn
        .query_row(
            "SELECT coalesce((SELECT ended_at FROM rounds WHERE work = ?1 AND n = ?2),
                             (SELECT min(started_at) FROM rounds WHERE work = ?1), 0)",
            params![w.id, from],
            |r| r.get(0),
        )
        .map_err(sql)?;
    let released = count(
        conn,
        "SELECT count(*) FROM changes WHERE table_name = 'dispatch_holds' AND new_row IS NULL AND at >= ?1
           AND json_extract(old_row, '$.project') = ?2 AND json_extract(old_row, '$.reason') = 'set_straight'",
        &[&since, &project],
    )? > 0;
    let rethink = count(
        conn,
        "SELECT count(*) FROM rounds WHERE work = ?1 AND n > ?2 AND cell = 'O1' AND transition IS NOT NULL
           AND json_extract(output, '$.outcome.take.way') = 'rethink'",
        &[&w.id, &from],
    )? > 0;
    Ok(json!({
        "handled_matters": numbers,
        "handled": by_matter(handled),
        "held_for_owner": by_matter(asked),
        "item_changes": made,
        "straight_hold_released": released,
        "owner_asked_to_rethink": rethink,
    }))
}

/// The matter picked, with its material by its kind.
fn matter(e: &Engine, w: &Work) -> Result<Option<Value>, String> {
    let conn = e.db.conn();
    let Some(m) = picked(conn, w.id)? else {
        return Ok(None);
    };
    let row = query_json(
        conn,
        "SELECT id AS matter, kind, detail, what, body, item, work, round, at FROM architect_matters WHERE id = ?1",
        &[&m],
        1,
    )?;
    let Some(mut matter) = row.into_iter().next() else {
        return Ok(None);
    };
    matter["at"] = json!(matter["at"].as_i64().map(utc_minute));
    let mut material = Map::new();
    // What the work the matter came from did, when it came from an engineer's work.
    if let Some(work) = matter["work"].as_i64() {
        let graph: Option<String> = conn
            .query_row("SELECT graph FROM works WHERE id = ?1", [work], |r| r.get(0))
            .optional()
            .map_err(sql)?;
        if graph.as_deref() == Some(ENGINEER) {
            material.insert(
                "log".into(),
                Value::Array(query_json(
                    conn,
                    "SELECT round, cell, progress FROM work_progress WHERE work = ?1 AND progress IS NOT NULL
                     ORDER BY round DESC LIMIT 30",
                    &[&work],
                    30,
                )?),
            );
        }
    }
    match matter["kind"].as_str().unwrap_or_default() {
        "design_card" => {
            if let Some(card) = matter["body"]["design_card"].as_i64() {
                let rows = query_json(
                    conn,
                    "SELECT c.id AS design_card, c.work, c.round, c.body, c.status, c.rulings, c.changed, w.worktree,
                            i.project
                     FROM design_cards c JOIN works w ON w.id = c.work LEFT JOIN items i ON i.id = w.item
                     WHERE c.id = ?1",
                    &[&card],
                    1,
                )?;
                if let Some(c) = rows.first() {
                    let design = query_json(
                        conn,
                        "SELECT json_extract(output, '$.outcome') AS design FROM rounds
                         WHERE work = ?1 AND cell = '2a' AND transition IS NOT NULL ORDER BY n DESC LIMIT 1",
                        &[&c["work"].as_i64().unwrap_or_default()],
                        1,
                    )?;
                    material.insert("design_card".into(), c.clone());
                    material.insert(
                        "design".into(),
                        design.first().map(|d| d["design"].clone()).unwrap_or(Value::Null),
                    );
                    let project = c["project"].as_str().and_then(|p| e.config.config.projects.get(p));
                    if let (Some(wt), Some(p)) = (c["worktree"].as_str(), project) {
                        let wt = Path::new(wt);
                        let diff =
                            git::fork_point(wt, &p.main).and_then(|f| git::diff(wt, &f, "HEAD", &p.design_paths));
                        material.insert("design_tree_diff".into(), json!(diff.unwrap_or_else(|x| x)));
                    }
                }
            }
        }
        "set_straight" => {
            if let Some(round) = matter["round"].as_i64() {
                let fit = query_json(
                    conn,
                    "SELECT json_extract(output, '$.outcome.fit') AS fit FROM rounds WHERE work = ?1 AND n = ?2",
                    &[&matter["work"].as_i64().unwrap_or_default(), &round],
                    1,
                )?;
                if let Some(f) = fit.first().map(|r| r["fit"].clone()).filter(|f| !f.is_null()) {
                    material.insert("understanding_verdict".into(), f);
                }
            }
        }
        "other_architect" => {
            if let Some(answers) = matter["body"]["answers"].as_i64() {
                material.insert(
                    "answers".into(),
                    Value::Array(query_json(
                        conn,
                        "SELECT id AS matter, what, body FROM architect_matters WHERE id = ?1",
                        &[&answers],
                        1,
                    )?),
                );
            }
        }
        _ => {}
    }
    Ok(Some(json!({ "matter": matter, "material": material })))
}

/// An item's number given as a number or as a text of one, added to a set.
fn ids_of(v: &Value, ids: &mut BTreeSet<i64>) {
    match v {
        Value::Number(n) => ids.extend(n.as_i64()),
        Value::String(s) => ids.extend(s.trim().trim_start_matches('#').parse::<i64>().ok()),
        _ => {}
    }
}

/// The numbers a text names as items, written `#12` or `item 12`.
fn named_items(text: &str, ids: &mut BTreeSet<i64>) {
    let words: Vec<&str> = text
        .split(|c: char| c.is_whitespace() || ",.;:()".contains(c))
        .collect();
    for (i, w) in words.iter().enumerate() {
        if let Some(n) = w.strip_prefix('#').and_then(|n| n.parse::<i64>().ok()) {
            ids.insert(n);
        } else if w.eq_ignore_ascii_case("item")
            && let Some(n) = words.get(i + 1).and_then(|n| n.parse::<i64>().ok())
        {
            ids.insert(n);
        }
    }
}

/// One item in full: as its creator wrote it, its locked acceptance, its dependencies, who waits for it, its design
/// cards, and its latest work's cell, state and what it waits for.
fn item_in_full(e: &Engine, item: i64) -> Result<Option<Value>, String> {
    let conn = e.db.conn();
    let Some(body) = query_json(conn, "SELECT * FROM item_body WHERE item = ?1", &[&item], 1)?
        .into_iter()
        .next()
    else {
        return Ok(None);
    };
    let mut out = json!({ "item": body });
    for (name, q) in [
        (
            "acceptance",
            "SELECT acceptance FROM item_acceptance WHERE item = ?1 AND locked",
        ),
        (
            "depends_on",
            "SELECT depends_on, needs, title, cancelled, at, pr FROM item_dependencies WHERE item = ?1",
        ),
        (
            "waited_by",
            "SELECT l.item, i.title, l.needs FROM item_links l JOIN items i ON i.id = l.item WHERE l.depends_on = ?1",
        ),
        (
            "design_cards",
            "SELECT design_card, status, body, rulings FROM item_design_cards WHERE item = ?1 AND status <> 'replaced'",
        ),
        (
            "state",
            "SELECT i.rank, i.held, i.cancelled, i.level, w.id AS work, w.at, w.paused, w.pr FROM items i
             LEFT JOIN works w ON w.id = (SELECT max(x.id) FROM works x WHERE x.item = i.id) WHERE i.id = ?1",
        ),
    ] {
        let rows = query_json(conn, q, &[&item], ROWS)?;
        if !rows.is_empty() {
            out[name] = Value::Array(rows);
        }
    }
    if let Some(work) = out["state"][0]["work"].as_i64() {
        let at = text(&out["state"][0]["at"]).to_string();
        out["state"][0]["waits"] = json!(work_waits(e, work, &at)?);
    }
    Ok(Some(out))
}

/// The items and works the matter picked concerns, each in full: the item it names, those its material names by number,
/// and the item of a work it names.
fn matter_items(e: &Engine, w: &Work) -> Result<Option<Value>, String> {
    let conn = e.db.conn();
    let Some(m) = picked(conn, w.id)? else {
        return Ok(None);
    };
    let row = query_json(
        conn,
        "SELECT item, what, body FROM architect_matters WHERE id = ?1",
        &[&m],
        1,
    )?;
    let Some(r) = row.first() else { return Ok(None) };
    let mut ids = BTreeSet::new();
    ids_of(&r["item"], &mut ids);
    ids_of(&r["body"]["fix"]["item"], &mut ids);
    named_items(text(&r["what"]), &mut ids);
    let mut out = Map::new();
    for id in ids.into_iter().take(12) {
        if let Some(x) = item_in_full(e, id)? {
            out.insert(format!("#{id}"), x);
        }
    }
    Ok((!out.is_empty()).then_some(Value::Object(out)))
}

/// The architect's recent log entries on the item the matter picked concerns: each matter about it handled, with what
/// the round that handled it said.
fn item_log(e: &Engine, w: &Work) -> Result<Option<Value>, String> {
    let conn = e.db.conn();
    let Some(m) = picked(conn, w.id)? else {
        return Ok(None);
    };
    let item: Option<i64> = conn
        .query_row("SELECT item FROM architect_matters WHERE id = ?1", [m], |r| r.get(0))
        .map_err(sql)?;
    let Some(item) = item else { return Ok(None) };
    let rows = query_json(
        conn,
        "SELECT h.matter, h.kind, h.what, 'R' || h.work || '-' || h.round AS round, h.cell, h.said
         FROM wake_handled h JOIN architect_matters m ON m.id = h.matter
         WHERE m.item = ?1 ORDER BY h.work DESC, h.round DESC LIMIT 12",
        &[&item],
        12,
    )?;
    Ok((!rows.is_empty()).then_some(Value::Array(rows)))
}

/// The significant words of a text: four letters or more, in lower case.
fn words(text: &str) -> BTreeSet<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() >= 4)
        .map(str::to_lowercase)
        .collect()
}

/// The items whose title or problem is close to those at hand, and the works under way whose branches changed the
/// same paths as the work the matter came from. At hand is the matter picked, and at the review the batch's changes.
/// An item is close when it shares two words with them that most of its project's items do not have.
fn similar_items(e: &Engine, w: &Work, g: &CompiledGraph) -> Result<Value, String> {
    let conn = e.db.conn();
    let review = w.pos.at == REVIEW && g.cell(REVIEW).is_some();
    let at_hand_text = if review {
        collected(conn, w.id)?
            .iter()
            .map(|r| r.3.to_string())
            .collect::<Vec<_>>()
            .join(" ")
    } else {
        matter_text(e, w)?
    };
    let at_hand = words(&at_hand_text);
    let rows: Vec<(Value, BTreeSet<String>)> = query_json(
        conn,
        "SELECT id AS item, project, title, problem FROM items WHERE cancelled IS NULL",
        &[],
        100_000,
    )?
    .into_iter()
    .map(|r| {
        let mine = words(&format!("{} {}", text(&r["title"]), text(&r["problem"])));
        (r, mine)
    })
    .collect();
    // How many of each project's items have each word, and how many items it has.
    let mut df: BTreeMap<&str, (usize, BTreeMap<&str, usize>)> = BTreeMap::new();
    for (r, mine) in &rows {
        let p = df.entry(text(&r["project"])).or_default();
        p.0 += 1;
        for x in mine {
            *p.1.entry(x.as_str()).or_default() += 1;
        }
    }
    let common = |project: &str, word: &str| {
        df.get(project)
            .and_then(|(n, f)| f.get(word).map(|k| *k > 1 && k * 2 > *n))
            .unwrap_or(false)
    };
    let mut close: Vec<(usize, Value)> = rows
        .iter()
        .filter_map(|(r, mine)| {
            let shared: Vec<&String> = mine
                .intersection(&at_hand)
                .filter(|x| !common(text(&r["project"]), x))
                .collect();
            (shared.len() >= 2).then(|| {
                (
                    shared.len(),
                    json!({ "item": r["item"], "project": r["project"], "title": r["title"], "shared_words": shared }),
                )
            })
        })
        .collect();
    close.sort_by_key(|x| std::cmp::Reverse(x.0));
    let close: Vec<Value> = close.into_iter().take(10).map(|x| x.1).collect();
    // The paths each work under way changed on its branch, against the work the matter came from.
    let mut same_paths = Vec::new();
    let from_work = picked(conn, w.id)?.and_then(|m| {
        conn.query_row("SELECT work FROM architect_matters WHERE id = ?1", [m], |r| {
            r.get::<_, Option<i64>>(0)
        })
        .ok()
        .flatten()
    });
    let changed_by = |work: &Value| -> Option<BTreeSet<String>> {
        let wt = Path::new(work["worktree"].as_str()?);
        let main = e
            .config
            .config
            .projects
            .get(work["project"].as_str()?)
            .map(|p| p.main.clone())?;
        let fork = git::fork_point(wt, &main).ok()?;
        Some(git::changed(wt, &fork, "HEAD").ok()?.into_iter().collect())
    };
    let running = query_json(
        conn,
        "SELECT w.id AS work, w.item, w.worktree, i.project FROM works w JOIN items i ON i.id = w.item
         WHERE w.graph = 'engineer' AND w.ended IS NULL AND w.worktree IS NOT NULL",
        &[],
        ROWS,
    )?;
    if !review
        && let Some(mine) = running
            .iter()
            .find(|r| r["work"].as_i64() == from_work)
            .and_then(changed_by)
    {
        for r in running.iter().filter(|r| r["work"].as_i64() != from_work) {
            let theirs = changed_by(r).unwrap_or_default();
            let both: Vec<&String> = mine.intersection(&theirs).collect();
            if !both.is_empty() {
                same_paths.push(json!({ "work": r["work"], "item": r["item"], "paths": both }));
            }
        }
    }
    Ok(json!({ "close_items": close, "same_paths": same_paths }))
}

/// The owner's answer to the ask that held the matter picked, when it comes back with one.
fn owner_answer(e: &Engine, w: &Work) -> Result<Option<Value>, String> {
    let conn = e.db.conn();
    let Some(m) = picked(conn, w.id)? else {
        return Ok(None);
    };
    let row = query_json(
        conn,
        "SELECT a.id AS ask, a.body, a.answer FROM architect_matters m JOIN owner_asks a ON a.id = m.asked
         WHERE m.id = ?1 AND a.answer IS NOT NULL",
        &[&m],
        1,
    )?;
    Ok(row.into_iter().next().map(|r| {
        json!({
            "ask": r["ask"],
            "asked": r["body"]["asked"],
            "answer": r["answer"]["text"],
            "answered_at": r["answer"]["at"].as_i64().map(utc_minute),
        })
    }))
}

/// What the cell before this one on the matter picked's path answered of it: the last round of that cell after the
/// matter was picked, in whichever wake, its own answer given as a place of that round's return, so that a session
/// that holds it is only referred to it. None when that cell gave no answer on this matter.
pub fn before(e: &Engine, w: &Work, g: &CompiledGraph) -> Result<Option<Part>, String> {
    let conn = e.db.conn();
    let Some(m) = picked(conn, w.id)? else {
        return Ok(None);
    };
    // The agent's cell whose own answer leads here, as Q1's crux leads to Q2.
    let Some(prev) = g.cells.iter().find(|c| {
        c.kind == "agent"
            && c.rows
                .iter()
                .any(|r| !r.common && r.to == w.pos.at && r.conditions.contains(&flow::format::Atom::Own))
    }) else {
        return Ok(None);
    };
    let Some(field) = prev
        .schema
        .as_ref()
        .and_then(|s| s.pointer(&format!("/properties/{}/anyOf/0/required/0", g.within)))
        .and_then(Value::as_str)
        .map(str::to_string)
    else {
        return Ok(None);
    };
    let wakes = query_json(
        conn,
        "SELECT DISTINCT r.work, r.n FROM rounds r JOIN works x ON x.id = r.work
         WHERE x.graph = 'architect' AND x.project = ?1 AND r.cell = ?2 AND r.transition IS NOT NULL
           AND json_extract(r.output, ?3) IS NOT NULL
         ORDER BY r.work DESC, r.n DESC",
        &[&project_of(w)?, &prev.id, &format!("$.{}.{field}", g.within)],
        1000,
    )?;
    for r in wakes {
        let (work, n) = (
            r["work"].as_i64().unwrap_or_default(),
            r["n"].as_i64().unwrap_or_default(),
        );
        if picked_before(conn, work, Some(n))? != Some(m) {
            continue;
        }
        let output: Option<String> = conn
            .query_row("SELECT output FROM rounds WHERE work = ?1 AND n = ?2", [work, n], |x| {
                x.get(0)
            })
            .map_err(sql)?;
        let Some(value) = json_col(output).and_then(|o| o[&g.within].get(&field).cloned()) else {
            continue;
        };
        let id = RoundId::new(work, n);
        return Ok(Some(Part {
            origin: Origin::Place {
                round: id,
                first: id,
                cell: prev.id.clone(),
                path: field,
            },
            value,
        }));
    }
    Ok(None)
}

/// The item changes of a wake's batch, numbered from 1, each with the matter it came from and its kind.
fn batch_changes(e: &Engine, w: &Work) -> Result<Value, String> {
    let conn = e.db.conn();
    let rows = collected(conn, w.id)?;
    let mut out = Vec::new();
    for (i, (_, round, matter, change)) in rows.iter().enumerate() {
        let what: Option<String> = conn
            .query_row(
                "SELECT kind || ': ' || what FROM architect_matters WHERE id = ?1",
                [matter],
                |r| r.get(0),
            )
            .optional()
            .map_err(sql)?;
        out.push(json!({
            "change": i + 1, "matter": matter, "matter_is": what, "round": format!("R{}-{round}", w.id),
            "item_change": change,
        }));
    }
    Ok(Value::Array(out))
}

/// The items the batch's changes touch, each with its rank, who waits for it and what it waits for.
fn changes_touch(e: &Engine, w: &Work) -> Result<Option<Value>, String> {
    let conn = e.db.conn();
    let rows = collected(conn, w.id)?;
    if rows.is_empty() {
        return Ok(None);
    }
    let mut item_ids: BTreeSet<i64> = BTreeSet::new();
    for (_, _, _, c) in &rows {
        for k in ["item", "on", "keep", "under", "fix_item"] {
            ids_of(&c[k], &mut item_ids);
        }
        ids_of(&c["to"]["item"], &mut item_ids);
        for a in items(c.get("absorbed")) {
            ids_of(&a, &mut item_ids);
        }
    }
    let mut out = Map::new();
    for id in &item_ids {
        if let Some(r) = query_json(
            conn,
            "SELECT i.project, i.title, i.rank, i.cancelled, i.held,
                    (SELECT json_group_array(l.item) FROM item_links l WHERE l.depends_on = i.id) AS waited_by,
                    (SELECT json_group_array(l.depends_on) FROM item_links l WHERE l.item = i.id) AS waits_for,
                    (SELECT w.at FROM works w WHERE w.item = i.id ORDER BY w.id DESC LIMIT 1) AS at
             FROM items i WHERE i.id = ?1",
            &[id],
            1,
        )?
        .into_iter()
        .next()
        {
            out.insert(format!("#{id}"), r);
        }
    }
    Ok(Some(Value::Object(out)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_row_names_an_article_by_its_number_or_its_title() {
        let a = "Article 1. Independent of any project; developed by itself";
        assert!(names_article("Article 1", a));
        assert!(names_article("independent of any project; developed by itself", a));
        assert!(!names_article("Article 10", a));
        assert!(!names_article("Article 2", a));
    }

    #[test]
    fn a_policy_names_no_date() {
        assert!(names_date("as said on 2026-10-27, keep it"));
        assert!(!names_date("version 2026-10 and 27"));
    }

    // A text names an item as #12 or item 12.
    #[test]
    fn a_text_names_items_by_number() {
        let mut ids = BTreeSet::new();
        named_items("Item 7 waits for #12, (item 9) and not 13.", &mut ids);
        assert_eq!(ids.into_iter().collect::<Vec<_>>(), [7, 9, 12]);
    }

    // A reply names the items a batch made by their numbers once they are made, and keeps the numbers it named.
    #[test]
    fn a_reply_gets_the_numbers_of_the_items_made() {
        let keys = BTreeMap::from([("cover".to_string(), 41)]);
        let reply = json!({ "text": "t", "children": [{ "key": "cover", "title": "c" }, { "key": "12", "title": "d" }],
                            "waits": [{ "key": " cover", "title": "c", "delivers": "x" }] });
        let r = numbered(&reply, &keys);
        assert_eq!(r["children"][0]["key"], "41");
        assert_eq!(r["children"][1]["key"], "12");
        assert_eq!(r["waits"][0]["key"], "41");
    }

    // The review's rows sort the batch into what is made and what is dropped; a change naming an item a dropped
    // change was to create is dropped with it.
    #[test]
    fn a_change_naming_a_dropped_item_is_dropped_with_it() {
        let rows = vec![
            (1, 5, 9, json!({ "kind": "create_item", "key": "new" })),
            (2, 5, 9, json!({ "kind": "add_dependency", "item": 3, "on": "new" })),
            (
                3,
                5,
                9,
                json!({ "kind": "move", "item": 3, "to": { "place": "first" } }),
            ),
        ];
        let review = vec![
            json!({ "change": 1, "verdict": "fix", "what": "w", "how": "h" }),
            json!({ "change": 2, "verdict": "ok", "why": "y" }),
            json!({ "change": 3, "verdict": "ok", "why": "y" }),
        ];
        let (sound, dropped) = sorted_by_review(rows, &review);
        assert_eq!(sound.iter().map(|r| r.0).collect::<Vec<_>>(), [3]);
        assert_eq!(dropped.iter().map(|r| r.0.0).collect::<Vec<_>>(), [1, 2]);
    }
}
