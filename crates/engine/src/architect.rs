//! The architects' side of the engine. Each project has its architect; a work of the architect is a work of its graph
//! for a project, on no item: the engine starts one when none of the project runs and the project's queue has a matter,
//! one at a time for each project, and such a work holds no slot. What puts a matter in a project's queue is here and
//! in the actions: an engineer's question or failure, a side report, a report's what to do next, a design card, the
//! owner's words and answers, an engineer's answer to the architect, another project's architect's message, slots free
//! with fewer items of the project ready, and a place the understanding found to set straight. Here too are the steps,
//! inputs, values, checks and actions of the architect's graph, its asks of the owner, and the batch of item changes a
//! review looks at before they are made, and the setting of a new policy, a stage of the architect's works whose matter
//! may go on over several of them. A project's architect's line goes on in one CLI session across the architect's
//! works, the setting of a new policy included.
//!
//! The engine knows a few cells of the architect's graph by their names, as the views over its rounds do: the look at
//! the queue (`Q0`) and the pick (`Q1`), which say the matter picked; the understanding
//! (`U1`); the rethink after a batch (`B4`), which ends a batch; the review of the batch
//! (`B2`); and the first and deciding cell of each kind's path ([`KINDS`]).

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

/// An hour, in milliseconds.
pub const HOUR: i64 = 3_600_000;

/// The engine's inputs the architect's cells read, each computed by [`input`], but [`BEFORE`], which [`before`] gives,
/// and those of the policy being set, which [`draft_places`] names the places of.
pub const INPUTS: [&str; 24] = [
    "constitution",
    "guide",
    "cited_nodes",
    "since_last_work",
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
    "policy_stays",
    "set_again",
    "cross_project",
    "batch_changes",
    "changes_touch",
    "group_talk",
];

/// The input that gives what the cell before this one on the matter's path answered of it, as that round's return:
/// addressed as that return, so that a session that holds it is only referred to it.
pub const BEFORE: &str = "before";

/// The input that gives the state of things as the architect last wrote it: when this work's understanding
/// (`U1`) wrote it, as the places of that round's return, which this work's session holds, with what was
/// learned and noted since read from `architect_state`; otherwise that view whole.
pub const STATE: &str = "state_of_things";

/// The study of a new policy and what the owner said of it, when the setting of the project's policy that ended last
/// ended with the owner saying the policy in force stays (`P3` `policy_stays`); none otherwise. The owner's view of a
/// study is the latest round of `P3` that took it, in a work of the architect, or of a setting of a flow version that
/// ran it as a work of its own; the study, the latest before it.
fn policy_stays(e: &Engine, project: &str) -> Result<Option<Value>, String> {
    let rows = query_json(
        e.db.conn(),
        "SELECT json_extract(r.output, '$.ruling') AS ruling,
                (SELECT json_extract(x.output, '$.outcome.study') FROM rounds x JOIN works y ON y.id = x.work
                 WHERE y.graph IN ('architect', 'policy') AND y.project = w.project AND x.cell = 'P2'
                   AND x.transition IS NOT NULL AND json_extract(x.output, '$.outcome.study') IS NOT NULL
                   AND (x.work < r.work OR (x.work = r.work AND x.n < r.n))
                 ORDER BY x.work DESC, x.n DESC LIMIT 1) AS study
         FROM rounds r JOIN works w ON w.id = r.work
         WHERE w.graph IN ('architect', 'policy') AND w.project = ?1 AND r.cell = 'P3' AND r.transition IS NOT NULL
           AND json_extract(r.output, '$.ruling.choice') IS NOT NULL
         ORDER BY r.work DESC, r.n DESC LIMIT 1",
        &[&project],
        1,
    )?;
    Ok(rows
        .into_iter()
        .next()
        .filter(|r| r["ruling"]["choice"] == "policy_stays")
        .map(|r| json!({ "study": r["study"], "owner": r["ruling"] })))
}

/// Why the policy is set again, for the setting of it: what the check of the policy in force that started it (`P1`)
/// found of the policy in force against the constitution, article by article, and each reason it may no longer govern,
/// as the round that raised the setting's matter handed them in; for a work with no setting in hand, the latest such
/// round of the project's.
fn set_again(e: &Engine, w: &Work, project: &str) -> Result<Option<Value>, String> {
    let conn = e.db.conn();
    let raised: Option<(i64, i64)> = match setting_of(conn, w.id)? {
        Some(m) => conn
            .query_row(
                "SELECT work, round FROM architect_matters WHERE id = ?1 AND work IS NOT NULL",
                [m],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(sql)?,
        None => None,
    };
    let rows = query_json(
        conn,
        &format!(
            "SELECT json_extract(r.output, '$.outcome.against') AS against,
                    json_extract(r.output, '$.outcome.verdict') AS verdict
             FROM rounds r JOIN works x ON x.id = r.work
             WHERE x.graph = 'architect' AND x.project = ?1 AND r.cell IN {} AND r.transition = 'set_again'
               AND (?2 IS NULL OR (r.work = ?2 AND r.n = ?3))
             ORDER BY r.work DESC, r.n DESC LIMIT 1",
            db::cells_sql(ARCHITECT, &[POLICY_CHECK])
        ),
        &[&project, &raised.map(|x| x.0), &raised.map(|x| x.1)],
        1,
    )?;
    Ok(rows.into_iter().next())
}

/// The inputs that give the policy being set, each with the places of the returns it is at: its text, as the work's
/// round that wrote it (`P4`) handed it in, or as the engine put it together (`P5`) from a
/// correction (`P8`) or a minor fix a reading found (`P7`), which hand in only their edits; and its probes, as the
/// round that wrote them handed them in, or as the engine put them together from a correction, which hands in only those
/// it adds or changes beside those it keeps. Each is addressed as the oldest of those places that holds it as it is
/// now, so that the session that wrote it, or was given it once, is only referred to it.
const DRAFT: [(&str, &[(&str, &str)]); 2] = [
    ("draft_text", &[("P4", "policy"), ("P5", "draft.policy")]),
    (
        "draft_probes",
        &[("P4", "probes"), ("P8", "revision.probes"), ("P5", "draft.probes")],
    ),
];

/// The places of the returns an input of the policy being set is taken from, the newest of them; none for another
/// input.
pub fn draft_places(name: &str) -> Option<&'static [(&'static str, &'static str)]> {
    DRAFT.iter().find(|d| d.0 == name).map(|d| d.1)
}

/// The cells of the policy's reading loop the engine knows by their names: the writing of the text, its putting
/// together, the judgment of a reading and the correction.
const WRITE: &str = "P4";
const PUT_TOGETHER: &str = "P5";
const JUDGE: &str = "P7";
const CORRECT: &str = "P8";

/// How many times the text of a policy being set may be corrected since it was written before a reading that leaves
/// gaps goes on with them to the owner, as the flow's values `corrections` says it.
const CORRECTIONS: i64 = 3;

/// Each kind of matter with the first cell of its path and the cell that decides it, where the matter goes back to when
/// the owner is asked about item changes of it the review dropped.
pub const KINDS: [(&str, &str, &str); 9] = [
    ("set_straight", "H1", "H1"),
    ("engineer_question", "H2", "H3"),
    ("report", "H4", "H5"),
    ("design_card", "H6", "H6"),
    ("failure", "H7", "H7"),
    ("owner_words", "H8", "H8"),
    ("other_architect", "H9", "H9"),
    ("slots_free", "H10", "H10"),
    (SETTING, "P2", "P2"),
];

/// The kind of matter the setting of a new policy is: the check of the policy in force that sets it again raises it and
/// takes it in hand at once (`start_setting`), and it may go on over several works of the architect, leaving the work
/// while an ask of the owner holds it and when a new session would start in the middle of it.
pub const SETTING: &str = "set_policy";

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

/// The cells the engine knows by their names: the look at the queue, the pick, the understanding, the rethink after a
/// batch, and the review of the batch.
const LOOK: &str = "Q0";
const PICK: &str = "Q1";
const ROUTER: &str = "Q2";
const UNDERSTAND: &str = "U1";
const RETHINK: &str = "B4";
const REVIEW: &str = "B2";

/// The architect's line, whose session goes on across the architect's works; the cell that asks whether that session
/// understood the project.
const LINE: &str = "architect";
const UNDERSTOOD_YET: &str = "S1";

/// The value the rows of the cell a project's architect waits at to start (`S0`) switch on: whether a matter is due.
const DUE: &str = "due";

/// The stage of the cells that set a new policy, and the check of the policy in force that opens it and starts a
/// setting when it sets the policy again.
const SETTING_POLICY: &str = "policy";
const POLICY_CHECK: &str = "P1";

/// The cell of the engineer's graph a work is done at, as the views read it.
const ENGINEER_END: &str = "E1";

/// The most rows an input of the architect's gives of one list.
const ROWS: usize = 200;

/// The most matters a batch handles before its item changes are reviewed.
const BATCH: i64 = 20;

/// The rows a round of the architect's cells takes when it hands in no answer of its own, as the views say it: it
/// asked for tools or a search, paused, asked the owner, or answered what the owner said.
const NO_ANSWER: &str = "('tools', 'search', 'pause', 'ask_owner', 'answers')";

/// The row an owner's cell of the setting of a policy takes when it puts its ask, which holds the setting's matter
/// until the owner answers.
const HELD: &str = "held";

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

/// The worktree a project's architect reads the project in for the works of a graph: `worktrees/<graph>/<project>` in
/// the data directory, which its git leaves out, detached at the project's main; the lines of the architect's works,
/// the setting of a new policy included, take turns in it.
pub fn worktree(data: &Path, graph: &str, project: &str) -> PathBuf {
    config::worktrees(data).join(graph).join(project)
}

/// Where a round of a project's architect at a cell runs: its graph's worktree; for a cell on no line, which knows
/// only what its inputs hold, as the new architect that reads a policy, an empty directory of its own,
/// `worktrees/<cell>/<project>`.
pub fn workdir(data: &Path, g: &CompiledGraph, cell: &CompiledCell, project: &str) -> PathBuf {
    match cell.line {
        Some(_) => worktree(data, &g.name, project),
        None => config::worktrees(data).join(&cell.id).join(project),
    }
}

/// Readies where a round of a project's architect runs, before the round: its graph's worktree brought to the latest
/// main, whatever was changed there dropped, or the empty directory of a cell on no line made afresh.
pub fn ready(e: &Engine, w: &Work, g: &CompiledGraph, cell: &CompiledCell) -> Result<(), String> {
    let project = project_of(w)?;
    let dir = workdir(&e.data, g, cell, project);
    if cell.line.is_none() {
        if dir.exists() {
            std::fs::remove_dir_all(&dir).map_err(|x| format!("{}: {x}", dir.display()))?;
        }
        return std::fs::create_dir_all(&dir).map_err(|x| format!("{}: {x}", dir.display()));
    }
    refresh(e, &g.name, project).map(|_| ())
}

/// Brings the worktree of a project's architect for the works of a graph to the latest main, making it when it is
/// not there; gives where it is.
pub fn refresh(e: &Engine, graph: &str, project: &str) -> Result<PathBuf, String> {
    let p = e
        .config
        .config
        .projects
        .get(project)
        .ok_or_else(|| format!("no project {project} in the configuration"))?;
    let dir = worktree(&e.data, graph, project);
    sessions::worktree::at_main(&p.checkout, &dir, &p.main).map_err(|x| {
        format!(
            "the architect's worktree {} could not be brought to main: {x}",
            dir.display()
        )
    })?;
    Ok(dir)
}

/// The project whose architect a work is.
pub fn project_of(w: &Work) -> Result<&str, String> {
    w.project
        .as_deref()
        .ok_or_else(|| format!("work {} names no project for its architect", w.id))
}

// The architect's works.

/// The work of a project's architect that runs: the project's newest architect work not stopped, unless it reached its
/// end. A work of the architect starts only while none of its project runs, so an older one has ended or reached its
/// end. A paused work still runs.
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

/// Whether a project's queue has a matter that starts a work of its architect: any but flow feedback, its own or
/// forwarded by another project's architect, which a work of the architect takes when something else started it.
pub fn queue_has(e: &Engine, project: &str) -> Result<bool, String> {
    Ok(count(
        e.db.conn(),
        &format!(
            "SELECT count(*) FROM architect_queue
             WHERE project = ?1 AND NOT (kind IN {FEEDBACK_KINDS} AND detail IS ?2)"
        ),
        &[&project, &FLOW_FEEDBACK],
    )? > 0)
}

/// The detail of a matter that holds flow feedback, what agents said was wrong with their cells: of the kind report for
/// a project's own, of the kind other_architect for what another project's architect forwarded.
pub const FLOW_FEEDBACK: &str = "flow_feedback";

/// The kinds of matter that may hold flow feedback, as an SQL list.
const FEEDBACK_KINDS: &str = "('report', 'other_architect')";

/// Whether a matter of the queue holds flow feedback, which waits for a work of the architect something else started.
fn is_flow_feedback(kind: &Value, detail: &Value) -> bool {
    (kind == "report" || kind == "other_architect") && detail == FLOW_FEEDBACK
}

/// Whether flow feedback says there is nothing: empty, or none.
pub fn says_none(feedback: &str) -> bool {
    let t = feedback.trim().trim_end_matches('.').trim();
    t.is_empty() || t.eq_ignore_ascii_case("none")
}

/// Adds one round's flow feedback, `entry`, to its project's architect's flow feedback, under its graph and cell: to the
/// matter of the kind report that holds it and is not handled, withdrawn, held by an ask or the pick of the project's
/// running architect work, or else to a new one. It starts no work of the architect ([`queue_has`]).
pub fn add_flow_feedback(
    tx: &Transaction<'_>,
    project: &str,
    graph: &str,
    cell: &str,
    entry: Value,
) -> Result<(), String> {
    let running: Option<i64> = tx
        .query_row(
            "SELECT max(id) FROM works WHERE graph = 'architect' AND project = ?1 AND ended IS NULL",
            [project],
            |r| r.get(0),
        )
        .map_err(sql)?;
    let taken = match running {
        Some(w) => picked(tx, w)?,
        None => None,
    };
    let open: Option<(i64, String)> = tx
        .query_row(
            "SELECT id, body FROM architect_matters
             WHERE project = ?1 AND kind = 'report' AND detail = ?2 AND handled_round IS NULL AND withdrawn IS NULL
               AND asked IS NULL AND id IS NOT ?3
             ORDER BY id DESC LIMIT 1",
            params![project, FLOW_FEEDBACK, taken],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(sql)?;
    let mut body = open
        .as_ref()
        .and_then(|(_, b)| serde_json::from_str::<Value>(b).ok())
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({ "feedback": {} }));
    if !body["feedback"].is_object() {
        body["feedback"] = json!({});
    }
    if !body["feedback"][graph].is_object() {
        body["feedback"][graph] = json!({});
    }
    let said = &mut body["feedback"][graph][cell];
    if !said.is_array() {
        *said = json!([]);
    }
    if let Some(xs) = said.as_array_mut() {
        xs.push(entry);
    }
    let mut rounds = 0;
    let mut cells = Vec::new();
    for (g, by_cell) in body["feedback"].as_object().into_iter().flatten() {
        for (c, xs) in by_cell.as_object().into_iter().flatten() {
            rounds += xs.as_array().map_or(0, Vec::len);
            cells.push(format!("{c} ({g})"));
        }
    }
    let what = match rounds {
        1 => format!("Flow feedback of 1 round, at cell {}", cells.join(", ")),
        n => format!("Flow feedback of {n} rounds, at cells {}", cells.join(", ")),
    };
    match open {
        Some((id, _)) => {
            tx.execute(
                "UPDATE architect_matters SET body = ?2, what = ?3 WHERE id = ?1",
                params![id, body.to_string(), what],
            )
            .map_err(sql)?;
        }
        None => {
            let m = Matter {
                project: project.to_string(),
                kind: "report".into(),
                detail: Some(FLOW_FEEDBACK.into()),
                what,
                body,
                ..Matter::default()
            };
            queue_matter(tx, &m).map_err(sql)?;
        }
    }
    Ok(())
}

/// Whether a matter is due for a project's architect, by the name of the row of the cell it waits at to start (`S0`)
/// that says it: `due` when its queue has a matter that starts a work of it ([`queue_has`]), `nothing_due` otherwise.
pub fn waits_to_start(e: &Engine, project: &str) -> Result<&'static str, String> {
    Ok(if queue_has(e, project)? { "due" } else { "nothing_due" })
}

/// Takes the rows of the cell a project's architect waits at to start (`S0`) for a project whose architect runs no
/// work, without a round, as the engine takes the engineer's for an item that waits: when the row it takes leads on, a
/// work of the architect's graph starts for the project, on no item and on the flow version new works start on, at the
/// cell that row leads to, paused by the owner when the owner's step started it. Gives the work started; none while
/// the project waits there. A flow version whose start is no such cell starts the work at its start when one is due.
pub fn start(e: &mut Engine, project: &str, paused: bool) -> Result<Option<i64>, String> {
    let due = waits_to_start(e, project)?;
    let v = e.flow_now;
    let g = e.flow(v)?.graph(ARCHITECT).ok_or("the flow has no architect's graph")?;
    let at = match g.cell(&g.start) {
        Some(c) if c.inenums.contains_key(DUE) => {
            let mut given = Map::new();
            given.insert(DUE.into(), json!(due));
            let row = g.take(&g.start, &json!({}), &given)?;
            if row.to == g.start {
                return Ok(None);
            }
            g.step(&g.start_position(), row)?.at
        }
        _ if due == "due" => g.start.clone(),
        _ => return Ok(None),
    };
    let paused = paused.then_some(db::PAUSED_BY_OWNER);
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO works (item, flow_version, graph, at, paused, paused_by, project)
             VALUES (NULL, ?1, ?2, ?3, ?4, CASE WHEN ?4 IS NULL THEN NULL ELSE 'owner' END, ?5)",
            params![v, ARCHITECT, at, paused, project],
        )?;
        Ok(Some(tx.last_insert_rowid()))
    })
    .map_err(sql)
}

/// The action of the cell an item waits at to start (`S0`), `queue_slots_free`, which the engine takes for every
/// project as it looks for an item to start, since no work stands at that cell: for each project with fewer items ready
/// to start than slots free and no hold on its new starts, a matter of slots free, when none waits in its queue and
/// the last was handled longer ago than the configuration's wait (`agents.slots_free_wait`) or never; the wait keeps
/// one that found nothing to start from coming back at once. Nothing when the flow new works start on does not name
/// the action.
pub fn queue_due(e: &mut Engine) -> Result<(), String> {
    let now = e.flow_now;
    let named = e
        .flow(now)?
        .graph(ENGINEER)
        .and_then(|g| g.cell(&g.start))
        .is_some_and(|c| c.actions.iter().any(|a| a == "queue_slots_free"));
    if !named {
        return Ok(());
    }
    let wait = i64::try_from(e.config.config.install.agents.slots_free_wait)
        .unwrap_or(i64::MAX / 1000)
        .saturating_mul(1000);
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
        if ready >= free || held > 0 || waiting > 0 {
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
        if last.is_some_and(|t| db::now() - t <= wait) {
            continue;
        }
        let slots = match free {
            1 => "1 slot is free".to_string(),
            n => format!("{n} slots are free"),
        };
        let what = match ready {
            0 => format!("{slots}, and no item of {p} is ready to start"),
            1 => format!("{slots}, and only 1 item of {p} is ready to start"),
            n => format!("{slots}, and only {n} items of {p} are ready to start"),
        };
        let m = Matter {
            project: p.clone(),
            kind: "slots_free".into(),
            what,
            body: json!({ "free": free, "ready": ready }),
            ..Matter::default()
        };
        e.db.write(Actor::Engine, |tx| queue_matter(tx, &m)).map_err(sql)?;
    }
    Ok(())
}

// The queue and the batch.

/// The matter the latest pick in a work of the architect chose before round `before`, or before none: the lone matter
/// the queue held (cell `Q0`), or the one the architect picked (cell `Q1`); none when the latest look at the queue
/// found none, or many with no pick after it. The check of the policy in force that sets it again (`P1`) takes the
/// setting of a new policy in hand as a pick does: the setting's matter it raised, or the one under way.
pub fn picked_before(conn: &Connection, work: i64, before: Option<i64>) -> Result<Option<i64>, String> {
    let row: Option<(i64, String, Option<String>)> = conn
        .query_row(
            "SELECT n, cell, output FROM rounds WHERE work = ?1 AND n < ?2 AND transition IS NOT NULL
               AND (cell = ?3 OR (cell = ?4 AND json_extract(output, '$.outcome.pick.matter') IS NOT NULL)
                 OR (cell = ?5 AND transition = 'set_again'))
             ORDER BY n DESC LIMIT 1",
            params![work, before.unwrap_or(i64::MAX), LOOK, PICK, POLICY_CHECK],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .map_err(sql)?;
    let Some((n, cell, output)) = row else { return Ok(None) };
    if cell == POLICY_CHECK {
        return setting_raised(conn, RoundId::new(work, n));
    }
    let o = json_col(output).unwrap_or(Value::Null);
    Ok(if cell == LOOK {
        o["queue"]["matter"].as_i64()
    } else {
        o["outcome"]["pick"]["matter"].as_i64()
    })
}

/// The setting of a new policy a round of the check of the policy in force that set it again took in hand: the latest
/// matter of a setting of its project raised up to that round.
fn setting_raised(conn: &Connection, round: RoundId) -> Result<Option<i64>, String> {
    conn.query_row(
        "SELECT max(m.id) FROM architect_matters m JOIN works w ON w.id = ?1
         WHERE m.project = w.project AND m.kind = ?3 AND (m.work < ?1 OR (m.work = ?1 AND m.round <= ?2))",
        params![round.work, round.n, SETTING],
        |r| r.get(0),
    )
    .map_err(sql)
}

/// The matter of the setting of a new policy a work of the architect has in hand; none when it has another in hand,
/// or none.
pub fn setting_of(conn: &Connection, work: i64) -> Result<Option<i64>, String> {
    let Some(m) = picked(conn, work)? else { return Ok(None) };
    let kind: Option<String> = conn
        .query_row("SELECT kind FROM architect_matters WHERE id = ?1", [m], |r| r.get(0))
        .optional()
        .map_err(sql)?;
    Ok((kind.as_deref() == Some(SETTING)).then_some(m))
}

/// Whether a cell of the architect's graph is one of the setting of a new policy: of the stage the check of the policy
/// in force opens, past that check.
pub fn in_setting(g: &CompiledGraph, cell: &str) -> bool {
    g.name == ARCHITECT
        && cell != POLICY_CHECK
        && g.cell(cell).is_some_and(|c| c.stage.as_deref() == Some(SETTING_POLICY))
}

/// A round a matter was in hand at: the round, its cell and the row it took.
pub struct MatterRound {
    pub id: RoundId,
    pub cell: Option<String>,
    pub transition: Option<String>,
}

/// The rounds of a project's architect works that had matter `m` in hand, oldest first, in whichever works: after the
/// round that raised it, for a matter an architect's round raised, and after each pick of it; as [`picked_before`]
/// tells the matter in hand.
pub fn matter_rounds(conn: &Connection, project: &str, m: i64) -> Result<Vec<MatterRound>, String> {
    let raised: Option<(Option<i64>, Option<i64>)> = conn
        .query_row("SELECT work, round FROM architect_matters WHERE id = ?1", [m], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .optional()
        .map_err(sql)?;
    let from = match raised {
        Some((Some(w), Some(n))) => RoundId::new(w, n),
        _ => RoundId::new(0, 0),
    };
    type Row = (i64, i64, Option<String>, Option<String>, Option<i64>, Option<i64>);
    let rows: Vec<Row> = {
        let mut st = conn
            .prepare(
                "SELECT r.work, r.n, r.cell, r.transition, json_extract(r.output, '$.queue.matter'),
                        json_extract(r.output, '$.outcome.pick.matter')
                 FROM rounds r JOIN works x ON x.id = r.work
                 WHERE x.graph = 'architect' AND x.project = ?1 AND (r.work > ?2 OR (r.work = ?2 AND r.n > ?3))
                 ORDER BY r.work, r.n",
            )
            .map_err(sql)?;
        st.query_map(params![project, from.work, from.n], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))
        })
        .map_err(sql)?
        .collect::<db::rusqlite::Result<_>>()
        .map_err(sql)?
    };
    let mut out = Vec::new();
    let mut in_hand: Option<i64> = None;
    let mut at_work = None;
    for (work, n, cell, transition, queued, chosen) in rows {
        if at_work != Some(work) {
            at_work = Some(work);
            in_hand = (work == from.work).then_some(m);
        }
        if in_hand == Some(m) {
            out.push(MatterRound {
                id: RoundId::new(work, n),
                cell: cell.clone(),
                transition: transition.clone(),
            });
        }
        if transition.is_none() {
            continue;
        }
        match cell.as_deref() {
            Some(LOOK) => in_hand = queued,
            Some(PICK) if chosen.is_some() => in_hand = chosen,
            Some(POLICY_CHECK) if transition.as_deref() == Some("set_again") => {
                in_hand = setting_raised(conn, RoundId::new(work, n))?;
            }
            _ => {}
        }
    }
    Ok(out)
}

/// The rounds at a cell of the setting of a new policy that the work's setting had there, newest first, in whichever of
/// its project's architect works; none for a cell of no setting, or a work with no setting in hand, whose own rounds
/// are what it has.
pub fn setting_rounds_at(e: &Engine, w: &Work, g: &CompiledGraph, cell: &str) -> Result<Option<Vec<Round>>, String> {
    if w.graph != ARCHITECT || !in_setting(g, cell) {
        return Ok(None);
    }
    let conn = e.db.conn();
    let Some(m) = setting_of(conn, w.id)? else {
        return Ok(None);
    };
    let ids: Vec<RoundId> = matter_rounds(conn, project_of(w)?, m)?
        .into_iter()
        .rev()
        .filter(|r| r.cell.as_deref() == Some(cell))
        .map(|r| r.id)
        .collect();
    work::rounds_of(conn, &ids).map(Some)
}

/// The cells of the setting a work's setting went through, oldest first, in whichever of its project's architect
/// works; none for a work with no setting in hand.
pub fn setting_cells(e: &Engine, w: &Work, g: &CompiledGraph) -> Result<Option<Vec<String>>, String> {
    if w.graph != ARCHITECT {
        return Ok(None);
    }
    let conn = e.db.conn();
    let Some(m) = setting_of(conn, w.id)? else {
        return Ok(None);
    };
    Ok(Some(
        matter_rounds(conn, project_of(w)?, m)?
            .into_iter()
            .filter_map(|r| r.cell)
            .filter(|c| in_setting(g, c))
            .collect(),
    ))
}

/// Whether the work came to its cell from the cell that routes a matter, which takes a matter in hand back to the
/// cell it stood at.
pub fn routed(from: &str) -> bool {
    from == ROUTER
}

/// Whether a round of a cell of the setting handed in an answer of its own: it took a row, and not one that asked for
/// tools or a search, paused, asked the owner, answered the owner's words, held the setting for the owner's answer,
/// or gave way to a new session.
fn answered_own(g: &CompiledGraph, r: &MatterRound) -> bool {
    let (Some(cell), Some(t)) = (r.cell.as_deref(), r.transition.as_deref()) else {
        return false;
    };
    let quiet = ["tools", "search", "pause", "ask_owner", "answers", HELD];
    !quiet.contains(&t)
        && !g
            .cell(cell)
            .is_some_and(|c| c.rows.iter().any(|x| x.name == t && crate::steps::anew(x)))
}

/// The owner's answer to the ask that holds the setting of a new policy whose matter is `m`, while it is still to be
/// taken: the ask is answered, and no round of the setting handed in an answer of its own since the round that put
/// it. Gives the ask, the cell that put it, which takes the answer, and the answer.
fn answer_back(
    conn: &Connection,
    g: &CompiledGraph,
    project: &str,
    m: i64,
) -> Result<Option<(i64, String, Value)>, String> {
    let held: Option<(Option<i64>, Option<String>)> = conn
        .query_row(
            "SELECT asked, resume_cell FROM architect_matters WHERE id = ?1",
            [m],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(sql)?;
    let Some((Some(ask), Some(cell))) = held else {
        return Ok(None);
    };
    let put: Option<(i64, i64, Option<String>)> = conn
        .query_row("SELECT work, round, answer FROM owner_asks WHERE id = ?1", [ask], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .optional()
        .map_err(sql)?;
    let Some((work, n, Some(answer))) = put else {
        return Ok(None);
    };
    let put = RoundId::new(work, n);
    let taken = matter_rounds(conn, project, m)?
        .iter()
        .any(|r| r.id > put && r.cell.as_deref().is_some_and(|c| in_setting(g, c)) && answered_own(g, r));
    Ok((!taken).then(|| (ask, cell, json_col(Some(answer)).unwrap_or(Value::Null))))
}

/// Where the setting of a new policy whose matter is `m` goes on when it is taken again: the cell that put the ask
/// that held it, with the owner's answer still to be taken; else the cell a new session would have started at, or the
/// cell the last row of the setting led to; else its study. Only a cell the router takes a matter back to.
fn stands_at(conn: &Connection, g: &CompiledGraph, project: &str, m: i64) -> Result<String, String> {
    let routes: Vec<String> = g
        .cell(ROUTER)
        .and_then(|c| c.inenums.get("route"))
        .map(|vs| vs.iter().map(|v| v.0.clone()).collect())
        .unwrap_or_default();
    let first = KINDS.iter().find(|k| k.0 == SETTING).map_or("P2", |k| k.1).to_string();
    let back = match answer_back(conn, g, project, m)? {
        Some((_, cell, _)) => Some(cell),
        None => matter_rounds(conn, project, m)?
            .into_iter()
            .rev()
            .find(|r| r.transition.is_some() && r.cell.as_deref().is_some_and(|c| in_setting(g, c)))
            .and_then(|r| {
                let cell = r.cell?;
                let row = g
                    .cell(&cell)?
                    .rows
                    .iter()
                    .find(|x| Some(&x.name) == r.transition.as_ref())?;
                Some(if crate::steps::anew(row) { cell } else { row.to.clone() })
            }),
    };
    Ok(back.filter(|c| routes.contains(c)).unwrap_or(first))
}

/// The matter an architect work's latest pick chose.
pub fn picked(conn: &Connection, work: i64) -> Result<Option<i64>, String> {
    picked_before(conn, work, None)
}

/// The round an architect work's batch began after: its latest rethink, which ends a batch; 0 when it had none.
fn batch_start(conn: &Connection, work: i64) -> Result<i64, String> {
    count(
        conn,
        "SELECT coalesce(max(n), 0) FROM rounds WHERE work = ?1 AND cell = ?2 AND transition IS NOT NULL",
        &[&work, &RETHINK],
    )
}

/// How many matters an architect work's batch handled, those the look at the queue closed without a look left out.
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

/// The item changes an architect work's batch collected, in order.
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

/// The items a change collected in an architect work's open batch names, on which the engine starts no work until the
/// batch ends: the item it changes, the item a split splits or another is moved under, and the items a merge keeps and
/// absorbs.
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
        for k in ["item", "keep"] {
            ids_of(&c[k], &mut ids);
        }
        ids_of(&c["under"]["item"], &mut ids);
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

/// The matters of a project's queue that the architect's work `work` can pick now, oldest first: every one, flow
/// feedback, its own or forwarded, only when it came before the work started, which waits for the architect's next work
/// otherwise; but while new starts are held to set the project straight and some matter sets it straight or has an
/// agent stopped waiting, only those.
fn pickable(conn: &Connection, project: &str, work: i64) -> Result<Vec<Value>, String> {
    let rows = query_json(
        conn,
        &format!(
            "SELECT * FROM architect_queue q WHERE q.project = ?1
               AND NOT (q.kind IN {FEEDBACK_KINDS} AND q.detail IS ?3
                 AND q.at >= coalesce((SELECT min(r.started_at) FROM rounds r WHERE r.work = ?2), q.at + 1))
             ORDER BY q.matter"
        ),
        &[&project, &work, &FLOW_FEEDBACK],
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
/// replaced by another, as handled in this work's round that runs now; gives each, with why.
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

/// Cell `Q0`: what can be picked from the project's queue now; a lone matter is picked here, without a
/// round. A design card matter whose card needs no look any longer is closed first (the cell's `close_settled_cards`,
/// which a flow version before the cell named what it does did too), as the round's progress says.
pub fn look_at_queue(e: &mut Engine, w: &Work, cell: &CompiledCell) -> Result<Value, String> {
    let project = project_of(w)?;
    let closes = cell.actions.is_empty() || cell.actions.iter().any(|a| a == "close_settled_cards");
    let closed = if closes {
        close_settled_cards(e, w, project)?
    } else {
        Vec::new()
    };
    let conn = e.db.conn();
    let can = pickable(conn, project, w.id)?;
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

/// Whether the ask of the owner that held a matter has its answer.
fn answered(conn: &Connection, ask: i64) -> Result<bool, String> {
    Ok(count(
        conn,
        "SELECT count(*) FROM owner_asks WHERE id = ?1 AND answer IS NOT NULL",
        &[&ask],
    )? > 0)
}

/// Where the matter picked goes: back to the cell that asked about it, when it comes back with the owner's answer;
/// otherwise the first cell of its kind's path. The setting of a new policy goes on from where it stood
/// ([`stands_at`]).
fn route(e: &Engine, w: &Work, g: &CompiledGraph) -> Result<String, String> {
    let conn = e.db.conn();
    let m = picked(conn, w.id)?.ok_or_else(|| format!("work {}: no matter is picked", w.id))?;
    if setting_of(conn, w.id)?.is_some() {
        return stands_at(conn, g, project_of(w)?, m);
    }
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
    let can = pickable(conn, project_of(w)?, w.id)?.len();
    let handled = handled_in_batch(conn, w.id)?;
    Ok(if can > 0 && handled < BATCH {
        "more"
    } else if !collected(conn, w.id)?.is_empty() {
        "review"
    } else {
        "done"
    })
}

/// How many times the setting a work has in hand corrected the text of the policy it sets since the text was last
/// written, in whichever works.
fn corrections(conn: &Connection, work: i64) -> Result<i64, String> {
    let Some(m) = setting_of(conn, work)? else { return Ok(0) };
    count(
        conn,
        "SELECT count(*) FROM policy_setting_rounds c
         WHERE c.matter = ?1 AND c.cell = ?2 AND c.transition = 'corrected'
           AND NOT EXISTS (SELECT 1 FROM policy_setting_rounds x
                           WHERE x.matter = ?1 AND x.cell = ?3 AND x.transition IS NOT NULL
                             AND (x.work > c.work OR (x.work = c.work AND x.n > c.n)))",
        &[&m, &CORRECT, &WRITE],
    )
}

/// Whether a project has the setting of a new policy under way: the matter of one is open.
fn setting_under_way(conn: &Connection, project: &str) -> Result<bool, String> {
    Ok(count(
        conn,
        "SELECT count(*) FROM architect_matters WHERE project = ?1 AND kind = ?2 AND handled_round IS NULL
           AND withdrawn IS NULL",
        &[&project, &SETTING],
    )? > 0)
}

/// Whether a work's last round that ended is the judgment of a reading that found a minor fix, which the text is put
/// together for without a new reading.
fn minor_fix_last(conn: &Connection, work: i64) -> Result<bool, String> {
    let last: Option<(Option<String>, Option<String>)> = conn
        .query_row(
            "SELECT cell, transition FROM rounds WHERE work = ?1 AND ended_at IS NOT NULL ORDER BY n DESC LIMIT 1",
            [work],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(sql)?;
    Ok(last.is_some_and(|(c, t)| c.as_deref() == Some(JUDGE) && t.as_deref() == Some("minor_fix")))
}

/// How many reviews of an architect work's batch found something to fix.
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

/// Whether the session the architect's work goes on in answered the understanding question since it started or was
/// last compacted: a new one, or one the next round would leave for a new one, did not. A round that asked for tools or
/// paused answered nothing, as the state of things the views read has it.
fn understood(e: &Engine, w: &Work, g: &CompiledGraph) -> Result<bool, String> {
    let sessions::Plan::Going { held, .. } = plan_at_understanding(e, w, g)? else {
        return Ok(false);
    };
    for r in &held.rounds {
        let n = count(
            e.db.conn(),
            &format!(
                "SELECT count(*) FROM rounds WHERE work = ?1 AND n = ?2 AND cell IN {} AND transition IS NOT NULL
                   AND transition NOT IN {NO_ANSWER}",
                db::cells_sql(ARCHITECT, &[UNDERSTAND])
            ),
            &[&r.work, &r.n],
        )?;
        if n > 0 {
            return Ok(true);
        }
    }
    Ok(false)
}

/// How the work came to the understanding question: from a rethink after a batch; in a session compacted since it
/// last understood the project; or in a new one, at the project's first architect work (before any answer of the
/// question, a round that asked for tools or paused being none), or for another reason.
fn entry(e: &Engine, w: &Work, g: &CompiledGraph) -> Result<String, String> {
    let conn = e.db.conn();
    let last: Option<String> = conn
        .query_row(
            "SELECT cell FROM rounds WHERE work = ?1 AND cell IN (?2, ?3) AND transition IS NOT NULL
             ORDER BY n DESC LIMIT 1",
            params![w.id, RETHINK, UNDERSTOOD_YET],
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
             WHERE x.graph = 'architect' AND x.project = ?1 AND r.cell IN {} AND r.transition IS NOT NULL
               AND r.transition NOT IN {NO_ANSWER}",
            db::cells_sql(ARCHITECT, &[UNDERSTAND])
        ),
        &[&project],
    )?;
    Ok(if before == 0 { "first" } else { "new" }.into())
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
        DUE => waits_to_start(e, project_of(w)?)?.into(),
        "understanding" => if understood(e, w, g)? {
            "understood"
        } else {
            "not_understood"
        }
        .into(),
        "entry" => entry(e, w, g)?,
        "route" => route(e, w, g)?,
        "setting" => if setting_under_way(conn, project_of(w)?)? {
            "under_way"
        } else {
            "none"
        }
        .into(),
        "next_in_batch" => next_in_batch(e, w)?.into(),
        "fixes_before" => if fixes_found(conn, w.id)? >= 2 { "two" } else { "fewer" }.into(),
        "corrections" => if corrections(conn, w.id)? >= CORRECTIONS {
            "three"
        } else {
            "fewer"
        }
        .into(),
        "next_reading" => if minor_fix_last(conn, w.id)? {
            "no_new_reading"
        } else {
            "new_reader"
        }
        .into(),
        "question" => {
            let d = picked_detail(conn, w.id)?.unwrap_or_default();
            if QUESTION_KINDS.contains(&d.as_str()) {
                d
            } else {
                "other_question".into()
            }
        }
        "report" => match picked_detail(conn, w.id)?.as_deref() {
            Some("followup") => "followup",
            Some("answer") => "answer",
            Some(FLOW_FEEDBACK) => FLOW_FEEDBACK,
            Some(db::FAULT) => db::FAULT,
            _ => "side_report",
        }
        .into(),
        _ => return Ok(None),
    }))
}

/// Whether the architect's work goes back to understanding the project before a round of the architect's line at a
/// cell: the round would start a new session there (the owner asked for one, the last was past the limit, or the model
/// changed), past the understanding, and a new session holds no state of things and no notes. A setting of a new
/// policy it was in goes back to the queue with its matter, and goes on from that cell when it is taken again.
///
/// The cells it happens at are those with the line's row for it (`session_anew`); a flow version without such rows
/// names them here. Gives why the session would start afresh, in words; none when the work goes on.
pub fn understand_first(
    e: &Engine,
    w: &Work,
    g: &CompiledGraph,
    cell: &CompiledCell,
) -> Result<Option<String>, String> {
    let rowed = g.cells.iter().any(|c| c.rows.iter().any(crate::steps::anew));
    let here = if rowed {
        cell.rows.iter().any(crate::steps::anew)
    } else {
        g.name == ARCHITECT
            && cell.line.as_deref() == Some(LINE)
            && cell.id != UNDERSTAND
            && (cell.stage.as_deref() != Some(SETTING_POLICY) || cell.id == POLICY_CHECK)
    };
    if !here {
        return Ok(None);
    }
    let model = crate::steps::level_of(e, g, cell, None)?.1.model;
    let limit = e.config.config.install.agents.context_limit;
    let plan = e
        .sessions
        .plan(e.db.conn(), w.id, w.project.as_deref(), cell, limit, &model)?;
    Ok(match plan {
        sessions::Plan::New(why) => Some(crate::steps::new_session_why(why, cell, limit, &model)),
        sessions::Plan::Going { .. } => None,
    })
}

/// Sends the architect's work back to the cell that asks whether its session understood the project, which a new
/// session has not, and takes that cell's step: the matter picked goes back to the queue, not handled, to be picked
/// again once the new session understood the project.
pub fn back_to_understanding(
    e: &mut Engine,
    w: &Work,
    g: &CompiledGraph,
    cell: &CompiledCell,
) -> Result<Advance, String> {
    let mut back = w.clone();
    back.pos = flow::Position {
        at: UNDERSTOOD_YET.into(),
        recorded: w.pos.recorded.clone(),
        ..flow::Position::default()
    };
    e.db.write(Actor::Engine, |tx| back.save_position(tx).map_err(db::Error::Abort))
        .map_err(sql)?;
    (e.say)(&format!(
        "work {}: a new session would start at cell {}: the work understands the project first",
        w.id, cell.id
    ));
    let back = e.work(w.id)?;
    let at = g
        .cell(UNDERSTOOD_YET)
        .cloned()
        .ok_or("the architect's graph has no cell that asks whether the session understood the project")?;
    crate::steps::engine(e, &back, g, &at)
}

// Asks of the owner.

/// The ask of the owner a round waits for or took, and its answer once the owner gave it: the one the round put, or the
/// one an earlier round of its cell put that the round's input names, as the round that takes the owner's view of a
/// study or approval of a policy names the ask that held the setting.
pub fn owner_ask_of(e: &Engine, round: RoundId) -> Result<Option<(i64, Option<Value>)>, String> {
    e.db.conn()
        .query_row(
            "SELECT a.id, a.answer FROM owner_asks a JOIN rounds r ON r.work = a.work
             WHERE r.work = ?1 AND r.n = ?2 AND (a.round = r.n OR a.id = json_extract(r.input, '$.ask'))
             ORDER BY a.id LIMIT 1",
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
/// or puts before the owner to read or approve, becomes an ask of the owner that holds only the matter of the path that
/// asked. The ask records the cell that takes the answer, the cell that asked or, for the owner's reading of a study or
/// approval of a policy, the owner's cell itself; the round ends at once (`held`), and the architect's work goes on
/// without it; the matter is back in the queue once the owner answers. The setting of a policy taken back to the cell
/// that put its ask goes on by the owner's choice, which the cell takes as its answer. An ask that holds no matter, as
/// an earlier architect put, waits for the answer.
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
        let gathered = inputs::gather(e, w, g, cell, None)?;
        let given = values::given(e, w, g, cell, None, &gathered)?;
        let output = answered_as(cell, &answer);
        let row = g.take(&cell.id, &output, &given)?.clone();
        return finish(e, w, g, cell, r.id, output, &row, Ended::default(), &[], None, &given);
    }
    let gathered = inputs::gather(e, w, g, cell, None)?;
    let given = values::given(e, w, g, cell, None, &gathered)?;
    let project = project_of(w)?.to_string();
    let commit = e.config.commit.clone();
    // The setting of a policy taken back to the cell that put its ask, with the owner's answer: its rows go on by it.
    let back = match setting_of(e.db.conn(), w.id)? {
        Some(m) => answer_back(e.db.conn(), g, &project, m)?.filter(|x| x.1 == cell.id),
        None => None,
    };
    if let Some((ask, _, answer)) = back {
        let output = answered_as(cell, &answer);
        let input = json!({ "inputs": recorded(&gathered), "ask": ask });
        let round =
            e.db.write(Actor::Engine, |tx| {
                work::start_round(tx, w.id, &cell.id, None, &input, &commit, None).map_err(db::Error::Abort)
            })
            .map_err(sql)?;
        let row = g.take(&cell.id, &output, &given)?.clone();
        return finish(e, w, g, cell, round, output, &row, Ended::default(), &[], None, &given);
    }
    // A work of the architect never waits for the owner: its ask holds only the matter of the path that asked.
    let holds = picked(e.db.conn(), w.id)?;
    let rules = !choices(cell).is_empty();
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
        format!("The new policy the architect of {project} wrote, read by new architects, waits for your approval")
    } else if rules {
        body["kind"] = json!("study");
        body["study"] = gathered.iter().find_map(|x| x.value.clone()).unwrap_or(Value::Null);
        body["choices"] = Value::Array(choices(cell));
        format!("The study of a new policy by the architect of {project} is ready for you to read")
    } else {
        let asked = asked_of(&gathered);
        let about = what_is_asked(&asked);
        body["kind"] = json!("ask");
        body["asked"] = Value::Array(asked);
        format!("The architect of {project} asks you{about}")
    };
    if let Some(m) = holds {
        body["holds"] = json!([m]);
    }
    // The cell the answer goes back to: the one that asked, or the owner's cell itself, which rules by it.
    let resume = if rules {
        Some(cell.id.clone())
    } else {
        w.pos.callers.last().or(w.pos.stopped.as_ref()).cloned()
    };
    let input = json!({ "inputs": recorded(&gathered) });
    let (round, id) =
        e.db.write(Actor::Engine, |tx| {
            let round = work::start_round(tx, w.id, &cell.id, None, &input, &commit, None).map_err(db::Error::Abort)?;
            tx.execute(
                "INSERT INTO owner_asks (work, round, body) VALUES (?1, ?2, ?3)",
                params![round.work, round.n, body.to_string()],
            )?;
            let id = tx.last_insert_rowid();
            actions::notify_ask(tx, w.id, id, &format!("{notice} (ask {id})")).map_err(db::Error::Abort)?;
            // The matter the ask holds waits for the answer out of the queue, and goes back to the cell that takes it.
            if let (Some(m), Some(c)) = (holds, &resume) {
                tx.execute(
                    "UPDATE architect_matters SET asked = ?2, resume_cell = ?3 WHERE id = ?1",
                    params![m, id, c],
                )?;
            }
            Ok((round, id))
        })
        .map_err(sql)?;
    let (Some(m), Some(c)) = (holds, resume) else {
        return Ok(Advance::Waits(format!("ask {id} waits for the owner")));
    };
    let output = if rules {
        json!({})
    } else {
        let text = format!(
            "The ask is with the owner as ask {id}. Matter {m} waits for the answer out of the queue, and comes back to \
             cell {c} with it; the work goes on with the other matters."
        );
        json!({ "reply": { "text": text } })
    };
    let row = g.take(&cell.id, &output, &given)?.clone();
    finish(e, w, g, cell, round, output, &row, Ended::default(), &[], None, &given)
}

// An engineer's question.

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

/// Cell `A2`: the work's blocker, held by an architect: the question, or the tool's failure, is queued for the
/// architect that holds it as a matter, which starts a work of it when none runs, saying which project and work it is
/// for and how it moved; the work waits, holding no slot. A matter that stands for the blocker in that architect's
/// queue already, as when the work comes back from weighing whether to wait longer, is not queued again; a blocker the
/// owner handed back goes with the owner's decision to the cell that carries it out. Each time the engine takes the
/// work's step here it works out where the question stands (the value `held`) and takes that row: the reply goes back
/// to the cell that asked unchanged, a hand-over passes the work on, and the rest withdraw it.
pub fn ask_architect(
    e: &mut Engine,
    w: &Work,
    g: &CompiledGraph,
    cell: &CompiledCell,
    item: &Item,
    open: Option<Round>,
) -> Result<Advance, String> {
    let conn = e.db.conn();
    let b = crate::blockers::open(conn, w.id)?
        .ok_or_else(|| format!("work {} waits for an architect, but holds no blocker", w.id))?;
    if let Some(r) = open.filter(|r| r.cell.as_deref() == Some(cell.id.as_str())) {
        let gathered = inputs::gather(e, w, g, cell, Some(item))?;
        let given = values::given(e, w, g, cell, Some(item), &gathered)?;
        let held = given
            .get("held")
            .and_then(Value::as_str)
            .unwrap_or("waiting")
            .to_string();
        let Some(row) = crate::row(cell, &held).cloned() else {
            let m = b
                .matter
                .map(|m| format!("matter {m}"))
                .unwrap_or_else(|| "the question".into());
            return Ok(Advance::Waits(format!("{m} waits for the architect's reply")));
        };
        let output = match held.as_str() {
            "answered" | "to_owner" | "to_architect" => {
                let reply: Option<String> = match b.matter {
                    Some(m) => {
                        e.db.conn()
                            .query_row("SELECT reply FROM architect_matters WHERE id = ?1", [m], |r| r.get(0))
                            .optional()
                            .map_err(sql)?
                            .flatten()
                    }
                    None => None,
                };
                json!({ "reply": json_col(reply).unwrap_or(Value::Null) })
            }
            _ => json!({}),
        };
        return finish(e, w, g, cell, r.id, output, &row, Ended::default(), &[], None, &given);
    }
    let project = b.project.clone().unwrap_or_else(|| item.project.clone());
    let live: Option<i64> = match b.matter {
        Some(m) => conn
            .query_row(
                "SELECT id FROM architect_matters
                 WHERE id = ?1 AND project = ?2 AND handled_round IS NULL AND withdrawn IS NULL",
                params![m, project],
                |r| r.get(0),
            )
            .optional()
            .map_err(sql)?,
        None => None,
    };
    let gathered = inputs::gather(e, w, g, cell, Some(item))?;
    let input = json!({ "inputs": recorded(&gathered) });
    let commit = e.config.commit.clone();
    if let Some(m) = live {
        e.db.write(Actor::Engine, |tx| {
            work::start_round(tx, w.id, &cell.id, None, &input, &commit, None).map_err(db::Error::Abort)?;
            Ok(())
        })
        .map_err(sql)?;
        return Ok(Advance::Waits(format!("matter {m} waits for the architect's reply")));
    }
    let asked = asked_of(&gathered);
    let (kind, detail) = match b.kind.as_str() {
        "tool_failure" => question_kind(&gathered),
        _ => match question_kind(&gathered) {
            ("failure", _) => ("engineer_question", "other_question".to_string()),
            x => x,
        },
    };
    let moves: Value = conn
        .query_row("SELECT moves FROM blockers WHERE id = ?1", [b.id], |r| {
            r.get::<_, String>(0)
        })
        .map(|m| serde_json::from_str(&m).unwrap_or(Value::Null))
        .map_err(sql)?;
    // A blocker the owner handed back goes with the owner's decision to the cell that carries it out.
    let decided = moves
        .as_array()
        .and_then(|ms| ms.last())
        .is_some_and(|m| m["from"]["holder"] == "owner");
    let back = b.ask.filter(|_| decided).map(|a| {
        let cell = if kind == "failure" { "H7" } else { "H3" };
        (a, cell)
    });
    let about = match what_is_asked(&asked) {
        a if a.is_empty() => format!(" ({})", detail.replace('_', " ")),
        a => a,
    };
    let of = if project == item.project {
        String::new()
    } else {
        format!(" of project {}", item.project)
    };
    let what = match back {
        Some(_) => format!(
            "Work {} on item {} ({}){of}: the owner decided its question, for you to carry out{about}",
            w.id, item.id, item.title
        ),
        None => format!("Work {} on item {} ({}){of} asks{about}", w.id, item.id, item.title),
    };
    let body = json!({
        "to": "architect",
        "work": w.id,
        "item": item.id,
        "title": item.title,
        "for": { "project": item.project, "work": w.id },
        "asked": asked,
        "moves": moves,
    });
    let matter =
        e.db.write(Actor::Engine, |tx| {
            let round = work::start_round(tx, w.id, &cell.id, None, &input, &commit, None).map_err(db::Error::Abort)?;
            let m = Matter {
                project: project.clone(),
                kind: kind.into(),
                detail: Some(detail.clone()),
                what: what.clone(),
                body: body.clone(),
                item: Some(item.id),
                raised: Some((round.work, round.n)),
            };
            let matter = queue_matter(tx, &m)?;
            if let Some((ask, resume)) = back {
                tx.execute(
                    "UPDATE architect_matters SET asked = ?2, resume_cell = ?3 WHERE id = ?1",
                    params![matter, ask, resume],
                )?;
            }
            // The matter stands for the work's blocker in the queue of the architect that holds it.
            crate::blockers::set_matter(tx, b.id, matter).map_err(db::Error::Abort)?;
            Ok(matter)
        })
        .map_err(sql)?;
    (e.say)(&format!(
        "work {} asks the architect of {project}: matter {matter}",
        w.id
    ));
    Ok(Advance::Waits(format!(
        "matter {matter} waits for the architect's reply"
    )))
}

/// Cell `P9`: how many new sessions in a row read this version of the policy's text and were judged to
/// understand it; two pass it, one asks for another.
pub fn count_trials(e: &mut Engine, w: &Work) -> Result<Value, String> {
    let m = setting_of(e.db.conn(), w.id)?.ok_or_else(|| format!("work {}: no setting of a policy in hand", w.id))?;
    let rows = query_json(
        e.db.conn(),
        "SELECT readings FROM policy_trials WHERE matter = ?1",
        &[&m],
        1,
    )?;
    let readings = rows.first().map(|r| items(r.get("readings"))).unwrap_or_default();
    let passed = readings
        .iter()
        .rev()
        .take_while(|r| matches!(r["verdict"]["verdict"].as_str(), Some("understood" | "minor_fix")))
        .count();
    let verdict = if passed >= 2 { "two_passed" } else { "one_more" };
    Ok(json!({ "count": { "verdict": verdict } }))
}

// The architect's actions.

/// The work of the architect an action is carried out for, and its project.
fn architect_work_of(ctx: &Ctx) -> Result<(i64, String), String> {
    Ok((ctx.work.id, project_of(&ctx.work)?.to_string()))
}

/// The matter an architect work picked, which the action is about.
fn matter_of(tx: &Transaction<'_>, work: i64) -> Result<i64, String> {
    picked(tx, work)?.ok_or_else(|| format!("work {work}: no matter is picked"))
}

/// A work running on an item: its id, why it is paused and who paused it.
type Running = (i64, Option<String>, Option<String>);

/// The work running on an item, as the database has it: not stopped and not at the end of the engineer's graph.
fn running_work(conn: &Connection, item: i64) -> Result<Option<Running>, String> {
    conn.query_row(
        &format!(
            "SELECT id, paused, paused_by FROM works
             WHERE item = ?1 AND graph = 'engineer' AND ended IS NULL AND at NOT IN {} ORDER BY id DESC LIMIT 1",
            db::cells_sql(ENGINEER, &[ENGINEER_END])
        ),
        params![item],
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

/// Releases the hold to set a project straight once no matter that sets it straight is left and the architect work's
/// batch holds no item change still to be made.
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

/// Whether the matter an architect work handles holds flow feedback, so that what its path sends on is flow feedback
/// forwarded.
fn forwards_feedback(tx: &Transaction<'_>, matter: i64) -> Result<bool, String> {
    let (kind, detail): (String, Option<String>) = tx
        .query_row(
            "SELECT kind, detail FROM architect_matters WHERE id = ?1",
            [matter],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(sql)?;
    Ok(is_flow_feedback(&json!(kind), &json!(detail)))
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

/// Carries out the effects of an answer on a matter's path: item changes are collected into the architect work's batch;
/// changes to observations, actions and messages are made at once; notes are the round's own, which the views read.
fn apply_effects(ctx: &Ctx, tx: &Transaction<'_>, effects: &Value) -> Result<(), String> {
    let (w, project) = architect_work_of(ctx)?;
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
    // What a matter of flow feedback has sent on is flow feedback forwarded: it waits for a work of the architect, as
    // the project's own.
    let detail = if forwards_feedback(tx, matter)? {
        FLOW_FEEDBACK
    } else {
        "message"
    };
    for m in items(effects.get("messages")) {
        let body = json!({ "from": project, "text": m["text"], "matter": matter, "work": w });
        message(tx, &project, text(&m["project"]), text(&m["text"]), detail, body)?;
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

/// Makes the given rows of an architect work's batch, as the architect, each in order; a key a change made the item
/// under is given its number. Gives the keys and the numbers they were given.
fn make(
    tx: &Transaction<'_>,
    rows: &[Change],
    projects: &BTreeMap<String, config::Project>,
) -> Result<BTreeMap<String, i64>, String> {
    let changes: Vec<Value> = rows.iter().map(|r| r.3.clone()).collect();
    let created = crate::arch_changes::write_all(tx, &changes, projects)?;
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
        let reply = numbered(&reply, keys);
        tx.execute(
            "UPDATE architect_matters SET reply = ?2, reply_held = NULL WHERE id = ?1",
            params![m, reply.to_string()],
        )
        .map_err(sql)?;
        if let Some(b) = crate::blockers::of_matter(tx, *m)?
            && b.ended.is_some()
        {
            crate::blockers::say(tx, b.id, "architect", "late", &reply)?;
        }
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
            ["item", "on", "keep", "fix_item"]
                .iter()
                .map(|k| c[*k].clone())
                .chain([c["under"]["item"].clone(), c["to"]["item"].clone()])
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
    let (w, project) = architect_work_of(ctx)?;
    let review = items(ctx.unwrapped.get("review"));
    let (sound, dropped) = sorted_by_review(collected(tx, w)?, &review);
    let keys = make(tx, &sound, &ctx.projects)?;
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
            let decides = KINDS.iter().find(|k| k.0 == kind).map_or("H8", |k| k.2);
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
        "tell_owner_unclear" => {
            // What the understanding could not settle goes to the owner as one message of the round, as an agent's
            // message does; the notifications log says it too.
            let rows: Vec<String> = items(Some(&ctx.unwrapped["unclear"]))
                .iter()
                .map(|u| format!("- {}: {}", text(&u["what"]), text(&u["why"])))
                .collect();
            if !rows.is_empty() {
                let (_, project) = architect_work_of(ctx)?;
                let said = format!(
                    "What the architect of {project} could not settle about the project now:\n{}",
                    rows.join("\n")
                );
                tx.execute(
                    "INSERT INTO owner_messages (work, round, text) VALUES (?1, ?2, ?3)",
                    params![ctx.round.work, ctx.round.n, said],
                )
                .map_err(sql)?;
                actions::outbox(tx, Some(w), "notify", &json!({ "text": said }))?;
            }
        }
        "queue_fixes" => {
            let (_, project) = architect_work_of(ctx)?;
            let fit = &ctx.unwrapped["fit"];
            // One matter for each place, with every step the understanding meant to take there.
            for fix in items(at) {
                let what = format!(
                    "To set straight, {}: {}",
                    db::one_sentence(text(&fix["where"])),
                    db::one_sentence(&texts(&fix["steps"]).join("; "))
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
        "hold_to_set_straight" => hold(tx, &architect_work_of(ctx)?.1, "set_straight", None)?,
        "release_straight_hold" => release(tx, &architect_work_of(ctx)?.1, "set_straight")?,
        "settle_straight_hold" => settle_hold(tx, w, &architect_work_of(ctx)?.1)?,
        "hold_for_policy" => hold(tx, &architect_work_of(ctx)?.1, "policy", None)?,
        "release_policy_hold" => release(tx, &architect_work_of(ctx)?.1, "policy")?,
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
            let held = changes_items(ctx);
            let column = if held { "reply_held" } else { "reply" };
            tx.execute(
                &format!("UPDATE architect_matters SET {column} = ?2 WHERE id = ?1"),
                params![m, reply.to_string()],
            )
            .map_err(sql)?;
            // A reply to a blocker its work no longer waits on is recorded with it and moves nothing; one that waits
            // for its batch's review is said about it until then.
            if let Some(b) = crate::blockers::of_matter(tx, m)? {
                if b.ended.is_some() {
                    crate::blockers::say(tx, b.id, "architect", "late", reply)?;
                } else if held {
                    crate::blockers::say(tx, b.id, "architect", "held_reply", reply)?;
                }
            }
        }
        "answer_architect" => {
            let Some(words) = at.and_then(Value::as_str).filter(|s| !s.trim().is_empty()) else {
                return Ok(true);
            };
            let (_, project) = architect_work_of(ctx)?;
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
            let (_, project) = architect_work_of(ctx)?;
            let rows = collected(tx, w)?;
            let keys = make(tx, &rows, &ctx.projects)?;
            let matters: BTreeSet<i64> = rows.iter().map(|r| r.2).collect();
            send_held_replies(tx, &matters, &keys)?;
            batch_over(tx, w, &project)?;
        }
        "apply_reviewed" => give_up(ctx, tx)?,
        "start_setting" => {
            let (_, project) = architect_work_of(ctx)?;
            let open = count(
                tx,
                "SELECT count(*) FROM architect_matters WHERE project = ?1 AND kind = ?2 AND handled_round IS NULL
                   AND withdrawn IS NULL",
                &[&project, &SETTING],
            )?;
            // One setting at a time: one under way is taken up instead, as the latest raised.
            if open == 0 {
                let reasons = items(ctx.unwrapped["verdict"].get("reasons"));
                let why: Vec<String> = reasons.iter().map(|r| text(&r["why"]).trim().to_string()).collect();
                let m = Matter {
                    project: project.clone(),
                    kind: SETTING.into(),
                    detail: None,
                    what: format!("Set a new policy for {project}: {}", db::one_sentence(&why.join("; "))),
                    body: json!({ "reasons": reasons }),
                    item: None,
                    raised: Some((ctx.round.work, ctx.round.n)),
                };
                queue_matter(tx, &m).map_err(sql)?;
            }
        }
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

/// The texts of the owner's words in the asks of the work's project's architect and in the owner's matters: what a
/// policy may not quote.
fn owner_words(conn: &Connection, work: i64) -> Result<Vec<String>, String> {
    let rows = query_json(
        conn,
        "SELECT json_extract(a.answer, '$.text') AS text FROM owner_asks a JOIN works w ON w.id = a.work
           JOIN works x ON x.id = ?1
         WHERE w.project = x.project AND w.graph IN ('architect', 'policy') AND a.answer IS NOT NULL
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

/// The first item a policy's text names by its number, as `#12` or `item 12`; none when it names none.
fn names_item(policy: &Value) -> Option<String> {
    let text = policy.to_string();
    if let Some(i) = text
        .as_bytes()
        .windows(2)
        .position(|w| w[0] == b'#' && w[1].is_ascii_digit())
    {
        let n: String = text[i + 1..].chars().take_while(char::is_ascii_digit).collect();
        return Some(format!("#{n}"));
    }
    let words: Vec<&str> = text
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    words
        .windows(2)
        .find(|w| {
            (w[0].eq_ignore_ascii_case("item") || w[0].eq_ignore_ascii_case("items"))
                && w[1].chars().all(|c| c.is_ascii_digit())
        })
        .map(|w| format!("item {}", w[1]))
}

/// The cells of the flow a text names: as the flow's own texts name one ([`flow::cells_named`]), or by its name after
/// the word cell, as `cell Q1`; a name alone is a word.
fn names_cell(e: &Engine, text: &str) -> Option<String> {
    let f = e.flows.get(&e.flow_now)?;
    let named = flow::cells_named(text);
    let words: Vec<&str> = text
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '.'))
        .map(|w| w.trim_matches('.'))
        .filter(|w| !w.is_empty())
        .collect();
    for g in &f.graphs {
        for c in &g.cells {
            let after = words
                .windows(2)
                .any(|w| w[0].eq_ignore_ascii_case("cell") && w[1] == c.id);
            if after || named.contains(&c.id.as_str()) {
                return Some(c.id.clone());
            }
        }
    }
    None
}

/// The numbers of a policy's principles.
fn principle_numbers(policy: &Value) -> Vec<i64> {
    items(policy.get("principles"))
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

/// The text of the policy a return writes, for the checks of it: whole, as the policy beside the place checked; or,
/// from a correction that hands in only its edits, beside the place checked, the text being set with them applied.
/// None when the return holds neither, or its edits do not apply, which edits_apply says.
fn revised(c: &checks::Ctx<'_>, at: Option<&str>) -> Result<Option<Value>, String> {
    let place = |to: &str| match at.and_then(|a| a.rsplit_once('.')) {
        Some((head, _)) => format!("{head}.{to}"),
        None => to.to_string(),
    };
    if let Some(p) = at_path(&c.unwrapped, &place("policy")) {
        return Ok(Some(p));
    }
    let Some(edits) = at_path(&c.unwrapped, &place("edits")) else {
        return Ok(None);
    };
    let Some((_, draft)) = draft_now(c.e.db.conn(), c.w.id)? else {
        return Ok(None);
    };
    Ok(applied(&draft, &items(Some(&edits))).ok())
}

/// The text of the policy the setting a work has in hand is setting as it stands, whole, with the round that holds it,
/// in whichever work: the latest the architect wrote (`P4`) or the engine put together (`P5`); none before one is
/// written.
fn draft_now(conn: &Connection, work: i64) -> Result<Option<(RoundId, Value)>, String> {
    let Some(m) = setting_of(conn, work)? else {
        return Ok(None);
    };
    let rows = query_json(
        conn,
        "SELECT work, n, coalesce(json_extract(output, '$.draft.policy'), json_extract(output, '$.outcome.policy'))
           AS policy
         FROM policy_setting_rounds WHERE matter = ?1 AND cell IN ('P4', 'P5') AND transition IS NOT NULL
           AND coalesce(json_extract(output, '$.draft.policy'), json_extract(output, '$.outcome.policy')) IS NOT NULL
         ORDER BY work DESC, n DESC LIMIT 1",
        &[&m],
        1,
    )?;
    Ok(rows.into_iter().next().map(|r| {
        (
            RoundId::new(
                r["work"].as_i64().unwrap_or_default(),
                r["n"].as_i64().unwrap_or_default(),
            ),
            r["policy"].clone(),
        )
    }))
}

/// Cell `P5`: this version of the policy's text and probes, put together for a new architect to read: as the
/// architect wrote them; after a correction, the text before with each of the correction's edits applied in order, and
/// the probes it keeps with those it adds or changes; after a minor fix a reading found, the text with its edits
/// applied.
pub fn put_draft_together(e: &Engine, w: &Work) -> Result<Value, String> {
    let conn = e.db.conn();
    let (round, text) =
        draft_now(conn, w.id)?.ok_or_else(|| format!("work {}: no text of a policy is written", w.id))?;
    let m = setting_of(conn, w.id)?.unwrap_or_default();
    let change = query_json(
        conn,
        "SELECT cell, coalesce(json_extract(output, '$.outcome.revision'), json_extract(output, '$.outcome.verdict'))
           AS change
         FROM policy_setting_rounds
         WHERE matter = ?1 AND (work > ?2 OR (work = ?2 AND n > ?3)) AND transition IS NOT NULL
           AND ((cell = ?4 AND json_extract(output, '$.outcome.revision.edits') IS NOT NULL)
             OR (cell = ?5 AND transition = 'minor_fix'))
         ORDER BY work DESC, n DESC LIMIT 1",
        &[&m, &round.work, &round.n, &CORRECT, &JUDGE],
        1,
    )?;
    let change = change.first().map(|r| r["change"].clone());
    let text = match &change {
        Some(c) => applied(&text, &items(c.get("edits")))?,
        None => text,
    };
    let probes = match change.filter(|c| c.get("kept").is_some() || c.get("probes").is_some()) {
        Some(c) => assembled_probes(&probes_now(conn, w.id)?, &c)?,
        None => probes_now(conn, w.id)?,
    };
    Ok(json!({ "draft": { "policy": text, "probes": probes } }))
}

/// The probes of the policy the setting a work has in hand is setting as they stand, in whichever work: as the engine
/// last put them together (`P5`), or as the round that wrote them handed them in (`P4`), whichever came last; none
/// before any is written.
fn probes_now(conn: &Connection, work: i64) -> Result<Vec<Value>, String> {
    let Some(m) = setting_of(conn, work)? else {
        return Ok(Vec::new());
    };
    let rows = query_json(
        conn,
        "SELECT coalesce(json_extract(output, '$.draft.probes'), json_extract(output, '$.outcome.probes')) AS probes
         FROM policy_setting_rounds WHERE matter = ?1 AND cell IN (?2, ?3) AND transition IS NOT NULL
           AND coalesce(json_extract(output, '$.draft.probes'), json_extract(output, '$.outcome.probes')) IS NOT NULL
         ORDER BY work DESC, n DESC LIMIT 1",
        &[&m, &WRITE, &PUT_TOGETHER],
        1,
    )?;
    Ok(rows.first().map(|r| items(r.get("probes"))).unwrap_or_default())
}

/// The probes a correction leaves: those of `current` it keeps, by id and in their order, then those it writes out,
/// a changed one under its id. A correction that keeps none by name, as one of a flow version before held them all,
/// leaves those it writes out. Refused when it keeps a probe there is not, or keeps one it writes out again.
fn assembled_probes(current: &[Value], revision: &Value) -> Result<Vec<Value>, String> {
    let written = items(revision.get("probes"));
    let Some(kept) = revision.get("kept") else {
        return Ok(written);
    };
    let kept = texts(kept);
    let id = |p: &Value| p["id"].as_str().map(str::trim).unwrap_or_default().to_string();
    let mut problems = Vec::new();
    for k in &kept {
        if !current.iter().any(|p| id(p) == *k) {
            problems.push(format!("{k} is no probe of this version"));
        }
        if written.iter().any(|p| id(p) == *k) {
            problems.push(format!(
                "{k} is kept and written out again: keep it or write it, not both"
            ));
        }
    }
    if !problems.is_empty() {
        return Err(problems.join("; "));
    }
    let mut out: Vec<Value> = current.iter().filter(|&p| kept.contains(&id(p))).cloned().collect();
    out.extend(written);
    Ok(out)
}

/// The probes a correction's return leaves, for the checks of them: the probes of this version it keeps with those it
/// writes out, from the place checked and the list of those it keeps beside it; the place's probes alone when nothing is
/// kept beside them.
fn probes_left(c: &checks::Ctx<'_>, at: Option<&str>, v: Option<&Value>) -> Result<Vec<Value>, String> {
    let kept = at
        .and_then(|a| a.rsplit_once('.'))
        .and_then(|(head, _)| at_path(&c.unwrapped, &format!("{head}.kept")));
    let Some(kept) = kept else {
        return Ok(items(v));
    };
    let revision = json!({ "kept": kept, "probes": v.cloned().unwrap_or_else(|| json!([])) });
    assembled_probes(&probes_now(c.e.db.conn(), c.w.id)?, &revision)
}

/// A policy with a correction's edits applied in order: each passage changed is in its texts, as the edits before
/// leave them, exactly once, and is replaced there; a principle or signal added goes last, a principle numbered after
/// the highest; one removed is taken out, the other principles keeping their numbers. When an edit does not apply,
/// none is, and why each does not is said, each by its number.
pub fn applied(policy: &Value, edits: &[Value]) -> Result<Value, String> {
    let mut p = policy.clone();
    let mut problems: Vec<String> = Vec::new();
    for (i, x) in edits.iter().enumerate() {
        let n = i + 1;
        match text(&x["kind"]) {
            // A minor fix a reading found changes passages only, and names no kind.
            "change" | "" => {
                let old = text(&x["old"]);
                let times = if old.is_empty() { 0 } else { occurrences(&p, old) };
                if times == 1 {
                    replace_once(&mut p, old, text(&x["new"]));
                } else {
                    problems.push(format!(
                        "edit {n} (change): its old passage is in the text {times} times, not once; quote it word \
                         for word, and enough of it to be there once"
                    ));
                }
            }
            "add_principle" => {
                let no = principle_numbers(&p).into_iter().max().unwrap_or(0) + 1;
                push_row(&mut p, "principles", json!({ "no": no, "text": x["text"] }));
            }
            "add_signal" => push_row(&mut p, "signals", json!({ "what": x["what"], "how": x["how"] })),
            "remove" => {
                // A principle by its number; a signal by its place in its list, from 1.
                let no = x["no"].as_i64().unwrap_or_default();
                let part = text(&x["part"]);
                let list = format!("{part}s");
                let rows = items(p.get(&list));
                let at = if part == "principle" {
                    rows.iter().position(|q| q["no"].as_i64() == Some(no))
                } else {
                    usize::try_from(no - 1).ok().filter(|i| *i < rows.len())
                };
                match (at, p.get_mut(&list).and_then(Value::as_array_mut)) {
                    (Some(i), Some(xs)) => {
                        xs.remove(i);
                    }
                    _ => problems.push(format!("edit {n} (remove): the text has no {part} {no}")),
                }
            }
            other => problems.push(format!("edit {n}: there is no kind of edit {other}")),
        }
    }
    if problems.is_empty() {
        Ok(p)
    } else {
        Err(problems.join("\n"))
    }
}

/// How many times a passage is in the texts a value holds.
fn occurrences(v: &Value, passage: &str) -> usize {
    match v {
        Value::String(s) => s.matches(passage).count(),
        Value::Array(xs) => xs.iter().map(|x| occurrences(x, passage)).sum(),
        Value::Object(o) => o.values().map(|x| occurrences(x, passage)).sum(),
        _ => 0,
    }
}

/// Replaces the first place a passage is in the texts a value holds; whether it was there.
fn replace_once(v: &mut Value, old: &str, new: &str) -> bool {
    match v {
        Value::String(s) => match s.find(old) {
            Some(at) => {
                s.replace_range(at..at + old.len(), new);
                true
            }
            None => false,
        },
        Value::Array(xs) => xs.iter_mut().any(|x| replace_once(x, old, new)),
        Value::Object(o) => o.values_mut().any(|x| replace_once(x, old, new)),
        _ => false,
    }
}

/// Adds a row at the end of a list of a policy.
fn push_row(p: &mut Value, list: &str, row: Value) {
    if !p[list].is_array() {
        p[list] = json!([]);
    }
    if let Some(xs) = p[list].as_array_mut() {
        xs.push(row);
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
                // Observations are summed up and looked for by their tags: once the project has tags, a new one carries
                // one at least.
                if !tags.is_empty() && texts(&o["tags"]).is_empty() {
                    problems.push(format!(
                        "observation change {n}: it carries no tag; give it at least one of the project's tags: {}",
                        tags.iter().cloned().collect::<Vec<_>>().join(", ")
                    ));
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
                if o["why"] == "folded" {
                    problems.push(format!(
                        "observation change {n}: an observation an item takes in is folded into that item, by the item \
                         change fold_observations naming the item (or the key of the item the batch creates), not \
                         closed: it closes once that item's work merges"
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
            let can: Vec<i64> = pickable(conn, project_of(c.w)?, c.w.id)?
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
            // A question handed on is not answered: it splits nothing and has the work wait for nothing, and it goes
            // to the owner or to the architect of a project the project's configuration names among its other
            // projects.
            if let Some(h) = reply.get("hand_over").filter(|h| h.is_object()) {
                if !items(reply.get("children")).is_empty() || !items(reply.get("waits")).is_empty() {
                    return Err(
                        "a reply that hands the question on neither splits the work nor has it wait: leave out \
                         children and waits, or hand_over"
                            .into(),
                    );
                }
                if h["to"] == "architect" {
                    let project = project_of(c.w)?;
                    let to = text(&h["project"]).trim();
                    let related =
                        c.e.config
                            .config
                            .projects
                            .get(project)
                            .map(|p| p.other_projects.clone())
                            .unwrap_or_default();
                    if to == project || !related.iter().any(|o| o == to) {
                        return Err(format!(
                            "hand_over goes to the architect of a project {project}'s configuration names among its \
                             other projects ({}), and names {to:?}",
                            related.join(", ")
                        ));
                    }
                    if !c.e.config.config.projects.contains_key(to) {
                        return Err(format!(
                            "hand_over names {to}, which is no project in the configuration"
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
                "SELECT count(*) FROM rounds WHERE work = ?1 AND n > ?2 AND cell = 'H8'
                   AND transition IS NOT NULL AND json_extract(output, '$.outcome.take.way') = 'rethink'",
                &[&c.w.id, &from],
            )?;
            let again = v.is_some_and(|r| r["verdict"] == "think_again");
            if asked > 0 && !again {
                return Err("the owner asked in this batch to rethink the policy: the answer is think_again".into());
            }
            Ok(())
        }
        "edits_apply" => {
            let (_, draft) = draft_now(conn, c.w.id)?.ok_or("no text of the policy is written yet")?;
            applied(&draft, &items(v)).map(|_| ())
        }
        "policy_text_clean" => {
            let Some(policy) = revised(c, at)? else { return Ok(()) };
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
            if let Some(item) = names_item(&policy) {
                return Err(format!(
                    "the policy names the item {item}: what to do next under it is thought out at cell N1, never written \
                     into it"
                ));
            }
            Ok(())
        }
        "probes_kept" => {
            let revision = json!({ "kept": v.cloned().unwrap_or_else(|| json!([])), "probes": [] });
            assembled_probes(&probes_now(conn, c.w.id)?, &revision).map(|_| ())
        }
        "probes_cover_principles" => {
            let probes = probes_left(c, at, v)?;
            let Some(policy) = revised(c, at)? else { return Ok(()) };
            let numbers = principle_numbers(&policy);
            for p in &probes {
                for n in items(p.get("principles")).iter().filter_map(Value::as_i64) {
                    if !numbers.contains(&n) {
                        return Err(format!(
                            "the probe {} names principle {n}, which the text does not have",
                            p["id"]
                        ));
                    }
                }
            }
            for n in numbers {
                let probed = probes
                    .iter()
                    .any(|p| items(p.get("principles")).iter().any(|x| x.as_i64() == Some(n)));
                if !probed {
                    return Err(format!("principle {n} has no probe"));
                }
            }
            Ok(())
        }
        "failed_probes_gone" => {
            let m = setting_of(conn, c.w.id)?.unwrap_or_default();
            let gaps = query_json(
                conn,
                "SELECT output FROM policy_setting_rounds
                 WHERE matter = ?1 AND cell = 'P7' AND transition IN ('gaps', 'gaps_left')",
                &[&m],
                ROWS,
            )?;
            let wrong: BTreeSet<String> = gaps
                .iter()
                .flat_map(|r| texts(&at_path(&c.g.unwrap(&r["output"]), "verdict.wrong_probes").unwrap_or_default()))
                .collect();
            let still: Vec<String> = probes_left(c, at, v)?
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
            let m = setting_of(conn, c.w.id)?.unwrap_or_default();
            let policy = query_json(conn, "SELECT policy FROM policy_text WHERE matter = ?1", &[&m], 1)?
                .first()
                .map(|r| r["policy"].clone())
                .unwrap_or(Value::Null);
            let probes = query_json(
                conn,
                "SELECT probes FROM policy_probes_asked WHERE matter = ?1",
                &[&m],
                1,
            )?
            .first()
            .map(|r| items(r.get("probes")))
            .unwrap_or_default();
            let restated: Vec<i64> = items(reading.get("restate"))
                .iter()
                .filter_map(|r| r["no"].as_i64())
                .collect();
            if let Some(n) = principle_numbers(&policy).into_iter().find(|n| !restated.contains(n)) {
                return Err(format!("principle {n} is not restated"));
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
        "since_last_work" => since_last_work(e, w, project).map(Some),
        "board" => board(e, project).map(Some),
        "numbers" => numbers(e, w, project).map(Some),
        "observations_overview" => observations_overview(e, project).map(Some),
        "observation_tags" => Ok(observation_tags(e, project)),
        "policy" => policy(e, project),
        "policy_signals" => policy_signals(e, w, project),
        "slots" => slots(e).map(Some),
        "queue" => queue(e, w, project).map(Some),
        "batch" => batch(e, w, project).map(Some),
        "matter" => matter(e, w),
        "matter_items" => matter_items(e, w),
        "item_log" => item_log(e, w),
        "similar_items" => similar_items(e, w, g).map(Some),
        "owner_answer" => owner_answer(e, w, g),
        "policy_stays" => policy_stays(e, project),
        "set_again" => set_again(e, w, project),
        "cross_project" => cross_project(e, project).map(Some),
        "batch_changes" => batch_changes(e, w).map(Some),
        "changes_touch" => changes_touch(e, w),
        "group_talk" => group_talk(e, w, project),
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
/// who waits for it, each by the kind of its dependency, since when its work runs and what it spent, or why its last
/// work stopped, as one queued again, and for one with no work running, the items not merged yet that it waits for by
/// a blocking dependency before it starts; the release items, each an item nothing depends on with items under it, with
/// the items under it; and the dependencies with other projects' items. A work at a cell of no stage, which asks or
/// runs tools for another, is at the stage of the cell it stopped at, or else of the innermost cell that waits for it
/// with one.
fn board(e: &Engine, project: &str) -> Result<Value, String> {
    let g = engineer_graph(e);
    let end = g.map(|g| g.end.clone()).unwrap_or_default();
    let rows = query_json(
        e.db.conn(),
        "SELECT i.id AS item, i.title, i.rank, i.held, i.level, i.deliver, w.id AS work, w.at, w.resume_at, w.waiting,
                w.paused, w.ended, (SELECT min(r.started_at) FROM rounds r WHERE r.work = w.id) AS since,
                (SELECT json_group_object('#' || l.depends_on, l.kind) FROM item_links l WHERE l.item = i.id)
                  AS waits_for,
                (SELECT json_group_object('#' || l.item, l.kind) FROM item_links l WHERE l.depends_on = i.id)
                  AS waited_by,
                EXISTS (SELECT 1 FROM ready_items r WHERE r.item = i.id) AS ready,
                (SELECT json_group_array('#' || b.depends_on) FROM item_blocked_by b WHERE b.item = i.id)
                  AS blocked_by
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
            // A work that runs goes on whatever it depends on.
            if let Some(o) = r.as_object_mut() {
                o.remove("blocked_by");
            }
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
            o.retain(|_, v| !v.is_null() && *v != json!([]) && *v != json!({}));
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

/// The dependencies between a project's items and other projects' items, each with its kind: what its items wait for
/// there, and what waits there for its items.
fn cross_project(e: &Engine, project: &str) -> Result<Value, String> {
    let conn = e.db.conn();
    let waits_for = query_json(
        conn,
        "SELECT l.item, l.depends_on AS on_item, l.kind, d.project, d.title, l.needs,
                (SELECT w.at FROM works w WHERE w.item = d.id ORDER BY w.id DESC LIMIT 1) AS at, d.cancelled
         FROM item_links l JOIN items i ON i.id = l.item JOIN items d ON d.id = l.depends_on
         WHERE i.project = ?1 AND d.project <> ?1 ORDER BY l.item, l.depends_on",
        &[&project],
        ROWS,
    )?;
    let waited_by = query_json(
        conn,
        "SELECT l.depends_on AS item, l.item AS by_item, l.kind, i.project, i.title, l.needs
         FROM item_links l JOIN items i ON i.id = l.item JOIN items d ON d.id = l.depends_on
         WHERE d.project = ?1 AND i.project <> ?1 ORDER BY l.depends_on, l.item",
        &[&project],
        ROWS,
    )?;
    Ok(json!({ "waits_for": waits_for, "waited_by": waited_by }))
}

/// When a work started: its first round's start; now, before it has one.
fn work_start(conn: &Connection, work: i64) -> Result<i64, String> {
    let at: Option<i64> = conn
        .query_row("SELECT min(started_at) FROM rounds WHERE work = ?1", [work], |r| {
            r.get(0)
        })
        .map_err(sql)?;
    Ok(at.unwrap_or_else(db::now))
}

/// What was said in the groups of a project's works since its architect last read it, at its last pick or wrap-up
/// (`Q1`, `E1`), other than to the architect, oldest first: the owner's words to the engineers, what the engineers told
/// the owner, and the owner's answers, to the engineers' asks and, in a group, to a question the architect held; each
/// with its work and item. The architect gets it as information, which starts no work of it and needs no answer:
/// everything said in a group reaches all three. A work of the architect with one matter picks it with no pick of the
/// architect's, and every such work ends at its wrap-up, so the wrap-up takes what no pick took. The first time, the
/// week before the work; none when nothing was said.
fn group_talk(e: &Engine, w: &Work, project: &str) -> Result<Option<Value>, String> {
    let conn = e.db.conn();
    let last: Option<i64> = conn
        .query_row(
            &format!(
                "SELECT max(r.started_at) FROM rounds r JOIN works x ON x.id = r.work
                 WHERE x.graph = 'architect' AND x.project = ?1 AND x.id <= ?2 AND r.cell IN {}
                   AND r.ended_at IS NOT NULL",
                db::cells_sql(ARCHITECT, &["Q1", "E1"])
            ),
            params![project, w.id],
            |r| r.get(0),
        )
        .map_err(sql)?;
    let since = match last {
        Some(t) => t,
        None => work_start(conn, w.id)? - 7 * 24 * HOUR,
    };
    let said = query_json(
        conn,
        "SELECT * FROM (
           SELECT j.work, w.item, i.title, 'owner' AS who, 'engineer' AS \"to\", j.text, NULL AS asked,
             (SELECT min(c.at) FROM changes c
              WHERE c.table_name = 'interjections' AND c.row_key = json_object('id', j.id)) AS at
           FROM interjections j JOIN works w ON w.id = j.work JOIN items i ON i.id = w.item
           WHERE i.project = ?1 AND w.graph = 'engineer' AND j.speaker = 'owner' AND j.matter IS NULL
             AND j.informs = 0 AND j.cancelled_item IS NULL AND j.text NOT LIKE 'Ruling on design card %'
           UNION ALL
           SELECT m.work, w.item, i.title, 'engineer', 'owner', m.text, NULL, coalesce(r.ended_at, r.started_at)
           FROM owner_messages m JOIN rounds r ON r.work = m.work AND r.n = m.round JOIN works w ON w.id = m.work
           JOIN items i ON i.id = w.item LEFT JOIN sessions s ON s.id = r.session
           WHERE i.project = ?1 AND w.graph = 'engineer' AND coalesce(s.line, 'engineer') = 'engineer'
           UNION ALL
           SELECT a.work, w.item, i.title, 'owner', 'engineer', json_extract(a.answer, '$.text'),
             (SELECT group_concat(json_extract(p.value, '$.what'), char(10))
              FROM json_each(a.body, '$.asked') q, json_each(q.value, '$.value.points') p),
             json_extract(a.answer, '$.at')
           FROM owner_asks a JOIN works w ON w.id = a.work JOIN items i ON i.id = w.item
           WHERE i.project = ?1 AND w.graph = 'engineer' AND json_extract(a.answer, '$.by') = 'owner'
           UNION ALL
           SELECT m.work, w.item, i.title, 'owner', 'engineer', json_extract(m.reply, '$.text'), m.what,
             json_extract(m.reply, '$.at')
           FROM architect_matters m JOIN works w ON w.id = m.work JOIN items i ON i.id = w.item
           WHERE m.project = ?1 AND w.graph = 'engineer' AND json_extract(m.reply, '$.by') = 'owner')
         WHERE at > ?2 ORDER BY at",
        &[&project, &since],
        ROWS,
    )?;
    if said.is_empty() {
        return Ok(None);
    }
    let said: Vec<Value> = said
        .into_iter()
        .map(|mut x| {
            if let Some(at) = x["at"].as_i64() {
                x["at"] = json!(utc_minute(at));
            }
            if let Some(o) = x.as_object_mut() {
                o.retain(|_, v| !v.is_null());
            }
            x
        })
        .collect();
    Ok(Some(json!({ "since": utc_minute(since), "said": said })))
}

/// What happened in a project since its architect's last work started; at the architect's first work, in the week
/// before it.
fn since_last_work(e: &Engine, w: &Work, project: &str) -> Result<Value, String> {
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
        None => work_start(conn, w.id)? - 7 * 24 * HOUR,
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
        &format!(
            "SELECT w.item, i.title, w.pr, json_extract(r.output, '$.result.merge_commit') AS commit_merged
             FROM rounds r JOIN works w ON w.id = r.work JOIN items i ON i.id = w.item
             WHERE i.project = ?1 AND r.cell IN {} AND r.transition = 'merged' AND r.ended_at > ?2
             ORDER BY r.ended_at",
            db::cells_sql(ENGINEER, &["M2"])
        ),
        &[&project, &since],
        ROWS,
    )?;
    let works = query_json(
        conn,
        &format!(
            "SELECT w.id AS work, w.item, i.title, w.at, w.paused, w.ended FROM works w JOIN items i ON i.id = w.item
             WHERE i.project = ?1 AND w.graph = 'engineer'
               AND EXISTS (SELECT 1 FROM rounds r WHERE r.work = w.id AND r.started_at > ?2)
               AND (w.ended IS NOT NULL OR w.at IN {})
             ORDER BY w.id",
            db::cells_sql(ENGINEER, &[ENGINEER_END])
        ),
        &[&project, &since],
        ROWS,
    )?;
    let main_ci = query_json(
        conn,
        &format!(
            "SELECT w.item, r.transition, json_extract(r.output, '$.main.kind') AS kind, r.ended_at AS at
             FROM rounds r JOIN works w ON w.id = r.work JOIN items i ON i.id = w.item
             WHERE i.project = ?1 AND r.cell IN {} AND r.ended_at > ?2 ORDER BY r.ended_at",
            db::cells_sql(ENGINEER, &["L1"])
        ),
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
        out["first_work"] = json!(true);
    }
    Ok(out)
}

/// The numbers the engine measures for a project, each as the architect's work started and earlier for the trend, so
/// that they hold through the work: the size of its design paths and of every file on main; its items made, merged and
/// cancelled, and what its items' works and its architect's works spent, each for the four weeks before the work one by
/// one.
fn numbers(e: &Engine, w: &Work, project: &str) -> Result<Value, String> {
    let conn = e.db.conn();
    let week = 7 * 24 * HOUR;
    let now = work_start(conn, w.id)?;
    let mut weeks = Vec::new();
    for k in 0..4 {
        // Each week ends the millisecond before the next begins, the last before the work.
        let (to, from) = (now - k * week - 1, now - (k + 1) * week);
        let made = count(
            conn,
            "SELECT count(*) FROM items i WHERE i.project = ?1 AND (SELECT min(c.at) FROM changes c
               WHERE c.table_name = 'items' AND c.row_key = json_object('id', i.id)) BETWEEN ?2 AND ?3",
            &[&project, &from, &to],
        )?;
        let merged = count(
            conn,
            &format!(
                "SELECT count(*) FROM rounds r JOIN works w ON w.id = r.work JOIN items i ON i.id = w.item
                 WHERE i.project = ?1 AND r.cell IN {} AND r.transition = 'merged' AND r.ended_at BETWEEN ?2 AND ?3",
                db::cells_sql(ENGINEER, &["M2"])
            ),
            &[&project, &from, &to],
        )?;
        let cancelled = count(
            conn,
            "SELECT count(*) FROM changes c WHERE c.table_name = 'items' AND c.at BETWEEN ?2 AND ?3
               AND json_extract(c.new_row, '$.project') = ?1 AND json_extract(c.new_row, '$.cancelled') IS NOT NULL
               AND json_extract(c.old_row, '$.cancelled') IS NULL",
            &[&project, &from, &to],
        )?;
        let (items, architect): (f64, f64) = conn
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
            "spent_usd": { "items": usd(items), "architect": usd(architect) },
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
fn queue(e: &Engine, w: &Work, project: &str) -> Result<Value, String> {
    let conn = e.db.conn();
    let can: BTreeSet<i64> = pickable(conn, project, w.id)?
        .iter()
        .filter_map(|r| r["matter"].as_i64())
        .collect();
    let started: Option<i64> = conn
        .query_row("SELECT min(started_at) FROM rounds WHERE work = ?1", [w.id], |r| {
            r.get(0)
        })
        .map_err(sql)?;
    let rows = query_json(
        conn,
        "SELECT * FROM architect_queue WHERE project = ?1 ORDER BY matter",
        &[&project],
        ROWS,
    )?;
    let mut matters = Map::new();
    for r in rows {
        // Flow feedback that came after the work started waits for the architect's next work.
        let later = is_flow_feedback(&r["kind"], &r["detail"])
            && started.is_some_and(|s| r["at"].as_i64().is_some_and(|at| at >= s));
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
        if later {
            m["not_now"] = json!("flow feedback that came after this work started: the architect's next work takes it");
        } else if !can.contains(&id) {
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
         WHERE m.project = ?1 AND m.handled_round IS NULL AND m.withdrawn IS NULL AND a.answer IS NULL ORDER BY m.id",
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
         JOIN rounds r ON r.work = ?1 AND r.n > ?2 AND r.cell = 'A3'
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
        "SELECT count(*) FROM rounds WHERE work = ?1 AND n > ?2 AND cell = 'H8' AND transition IS NOT NULL
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
        // Flow feedback names the project that is wf-agents, whose architect what belongs to wf-agents goes to.
        "report" if matter["detail"] == FLOW_FEEDBACK => {
            material.insert("wf_agents_project".into(), json!(e.wf_agents_project()));
        }
        // A fault of wf-agents forwarded to the architect, as it stands now: misses that came after the forward are
        // in it too.
        "report" if matter["detail"] == db::FAULT => {
            if let Some(f) = crate::faults::forwarded(conn, m)? {
                material.insert("fault".into(), f);
            }
        }
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
                         WHERE work = ?1 AND cell = 'D2' AND transition IS NOT NULL ORDER BY n DESC LIMIT 1",
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

/// One item in full: as its creator wrote it, its locked acceptance, its dependencies and who waits for it, each with
/// the dependency's kind, its design cards, and its latest work's cell, state and what it waits for.
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
            "SELECT depends_on, kind, needs, title, cancelled, at, pr FROM item_dependencies WHERE item = ?1",
        ),
        (
            "waited_by",
            "SELECT l.item, l.kind, i.title, l.needs FROM item_links l JOIN items i ON i.id = l.item
             WHERE l.depends_on = ?1",
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
    for step in texts(&r["body"]["fix"]["steps"]) {
        named_items(&step, &mut ids);
    }
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
         FROM work_handled h JOIN architect_matters m ON m.id = h.matter
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

/// The owner's answer to the ask that held the matter picked, when it comes back with one; for the setting of a
/// policy, only at the cell that asked, until that cell answered after it.
fn owner_answer(e: &Engine, w: &Work, g: &CompiledGraph) -> Result<Option<Value>, String> {
    let conn = e.db.conn();
    let Some(m) = picked(conn, w.id)? else {
        return Ok(None);
    };
    if setting_of(conn, w.id)?.is_some() && !answer_back(conn, g, project_of(w)?, m)?.is_some_and(|x| x.1 == w.pos.at) {
        return Ok(None);
    }
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
/// matter was picked, in whichever architect work, its own answer given as a place of that round's return, so that a
/// session that holds it is only referred to it. None when that cell gave no answer on this matter.
pub fn before(e: &Engine, w: &Work, g: &CompiledGraph) -> Result<Option<Part>, String> {
    let conn = e.db.conn();
    let Some(m) = picked(conn, w.id)? else {
        return Ok(None);
    };
    // The agent's cell whose own answer leads here, as the crux `H2` finds leads to `H3`.
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
    let rounds = query_json(
        conn,
        &format!(
            "SELECT DISTINCT r.work, r.n FROM rounds r JOIN works x ON x.id = r.work
             WHERE x.graph = 'architect' AND x.project = ?1 AND r.cell IN {} AND r.transition IS NOT NULL
               AND json_extract(r.output, ?2) IS NOT NULL
             ORDER BY r.work DESC, r.n DESC",
            db::cells_sql(ARCHITECT, &[prev.id.as_str()])
        ),
        &[&project_of(w)?, &format!("$.{}.{field}", g.within)],
        1000,
    )?;
    for r in rounds {
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

/// The item changes of an architect work's batch, numbered from 1, each with the matter it came from and its kind.
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

/// The items the batch's changes touch, each with its rank, who waits for it and what it waits for, each by the kind of
/// its dependency.
fn changes_touch(e: &Engine, w: &Work) -> Result<Option<Value>, String> {
    let conn = e.db.conn();
    let rows = collected(conn, w.id)?;
    if rows.is_empty() {
        return Ok(None);
    }
    let mut item_ids: BTreeSet<i64> = BTreeSet::new();
    for (_, _, _, c) in &rows {
        for k in ["item", "on", "keep", "fix_item"] {
            ids_of(&c[k], &mut item_ids);
        }
        ids_of(&c["under"]["item"], &mut item_ids);
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
                    (SELECT json_group_object('#' || l.item, l.kind) FROM item_links l WHERE l.depends_on = i.id)
                      AS waited_by,
                    (SELECT json_group_object('#' || l.depends_on, l.kind) FROM item_links l WHERE l.item = i.id)
                      AS waits_for,
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

    // A correction's edits apply in order: a passage changed where it is once, as the edits before leave the text, a
    // principle added after the highest, rows removed; when one does not apply, none does, and each that does not is
    // named.
    #[test]
    fn a_correction_s_edits_apply_in_order_or_not_at_all() {
        let policy = json!({ "direction": "The parser first: a parser others build on.", "why": "Users wait.",
                             "principles": [{ "no": 1, "text": "Parse first." }, { "no": 2, "text": "Check second." }],
                             "signals": [{ "what": "Speed", "how": "merged" }, { "what": "Bugs", "how": "reports" }] });
        let edits = [
            json!({ "kind": "change", "old": "Users wait.", "new": "Users wait for it." }),
            json!({ "kind": "change", "old": "for it", "new": "for the parser" }),
            json!({ "kind": "add_principle", "text": "Measure third." }),
            json!({ "kind": "remove", "part": "principle", "no": 1 }),
            json!({ "kind": "remove", "part": "signal", "no": 2 }),
            json!({ "kind": "add_signal", "what": "Size", "how": "design_bytes" }),
        ];
        let p = applied(&policy, &edits).unwrap();
        assert_eq!(p["why"], "Users wait for the parser.");
        assert_eq!(
            p["principles"],
            json!([{ "no": 2, "text": "Check second." }, { "no": 3, "text": "Measure third." }])
        );
        assert_eq!(p["signals"][1]["what"], "Size");
        assert_eq!(p["signals"].as_array().map(Vec::len), Some(2));
        let wrong = [
            json!({ "kind": "change", "old": "parser", "new": "lexer" }),
            json!({ "kind": "change", "old": "Users wait.", "new": "Users wait long." }),
            json!({ "kind": "change", "old": "Nowhere.", "new": "Somewhere." }),
            json!({ "kind": "remove", "part": "signal", "no": 5 }),
        ];
        let why = applied(&policy, &wrong).unwrap_err();
        assert!(
            why.contains("edit 1 (change): its old passage is in the text 2 times"),
            "{why}"
        );
        assert!(
            why.contains("edit 3 (change): its old passage is in the text 0 times"),
            "{why}"
        );
        assert!(why.contains("edit 4 (remove): the text has no signal 5"), "{why}");
        assert!(!why.contains("edit 2"), "{why}");
    }

    #[test]
    fn a_policy_names_no_item() {
        assert_eq!(names_item(&json!({ "why": "As #12 showed." })).as_deref(), Some("#12"));
        assert_eq!(
            names_item(&json!({ "direction": "Finish item 7 first." })).as_deref(),
            Some("item 7")
        );
        assert_eq!(
            names_item(&json!({ "direction": "Items come in by principle 2." })),
            None
        );
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
    // change was to create is dropped with it, a fold of observations into it among them.
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
            (
                4,
                5,
                9,
                json!({ "kind": "fold_observations", "item": "new", "observations": [2] }),
            ),
        ];
        let review = vec![
            json!({ "change": 1, "verdict": "fix", "what": "w", "how": "h" }),
            json!({ "change": 2, "verdict": "ok", "why": "y" }),
            json!({ "change": 3, "verdict": "ok", "why": "y" }),
            json!({ "change": 4, "verdict": "ok", "why": "y" }),
        ];
        let (sound, dropped) = sorted_by_review(rows, &review);
        assert_eq!(sound.iter().map(|r| r.0).collect::<Vec<_>>(), [3]);
        assert_eq!(dropped.iter().map(|r| r.0.0).collect::<Vec<_>>(), [1, 2, 4]);
    }
}
