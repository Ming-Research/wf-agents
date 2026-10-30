//! The architects' side of the engine. Each project has its architect; a work of the architect is a work of its graph
//! for a project, on no item: the engine starts one when none of the project runs and the project's queue has a matter,
//! one at a time for each project, and such a work holds no slot. What puts a matter in a project's queue is here and
//! in the actions: an engineer's question or failure, a side report, a report's what to do next, a design card, the
//! owner's words and answers, an engineer's answer to the architect, another project's architect's message, slots free
//! with fewer items of the project ready, and a place the understanding found to set straight. Here too are the steps,
//! inputs, values, checks and actions of the architect's graph, its asks of the owner, the batch of item changes a
//! review looks at before they are made, and the matters that come at the head of its queue: the check of the policy in
//! force, which may set a new one, and the plan of what to do next. A project's architect's line goes on in one CLI
//! session across the architect's works, the setting of a new policy included.
//!
//! The engine knows a few cells of the architect's graph by their names, as the views over its rounds do: the look at
//! the queue (`Q0`) and the pick (`Q1`), which say the matter picked; the understanding
//! (`U1`); the rethink after a batch (`B4`), which ends a batch; the review of the batch
//! (`B2`); the writing of a new policy (`P2`) and where it takes effect (`P12`), which the router tells apart by the
//! owner's reply; and the first and deciding cell of each kind's path ([`KINDS`]).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use db::attachments::{self, Words};
use db::rusqlite::{Connection, OptionalExtension, Transaction, params};
use db::{Actor, Matter, queue_matter};
use flow::format::Src;
use flow::{CompiledCell, CompiledGraph};
use serde_json::{Map, Value, json};

use crate::actions::{self, Ctx};
use crate::checks;
use crate::inputs::{self, Gathered, Origin, Part};
use crate::steps::{finish, what_is_asked};
use crate::work::{self, Ended, Item, Round, RoundId, Work, json_col, query_json};
use crate::{ARCHITECT, Advance, ENGINEER, Engine, git, values};

/// An hour, in milliseconds.
pub const HOUR: i64 = 3_600_000;

/// The engine's inputs the architect's cells read, each computed by [`input`], but [`BEFORE`], which [`before`] gives.
pub const INPUTS: [&str; 29] = [
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
    "cross_project",
    "batch_changes",
    "changes_touch",
    "words_behind_changes",
    "group_talk",
    "told_owner",
    "questions",
    "notebook_project_now",
    "notebook_plan",
    "notebook_unclear",
    "notebook_other",
];

/// The input that gives what the cell before this one on the matter's path answered of it, as that round's return:
/// addressed as that return, so that a session that holds it is only referred to it.
pub const BEFORE: &str = "before";

/// The sections of a project's architect's notebook, each with the engine's input that gives it: an input of its own
/// for each, so that a session given a section is referred to it while it stays the same.
pub const NOTEBOOK: [(&str, &str); 4] = [
    ("project_now", "notebook_project_now"),
    ("plan", "notebook_plan"),
    ("unclear", "notebook_unclear"),
    ("other", "notebook_other"),
];

/// The most characters a section of the notebook holds, and the four in all.
const SECTION_MOST: usize = 4_000;
const NOTEBOOK_MOST: usize = 12_000;

/// A section of a project's architect's notebook as it stands: its latest version's text; none before any round wrote
/// it, and none when it is empty.
fn notebook_section(conn: &Connection, project: &str, section: &str) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT text FROM notebook WHERE project = ?1 AND section = ?2",
        params![project, section],
        |r| r.get::<_, String>(0),
    )
    .optional()
    .map(|t| t.filter(|t| !t.trim().is_empty()))
    .map_err(sql)
}

/// What is wrong with the sections of the notebook a round rewrote: a section named twice, one past its size, or the
/// four past theirs in all, with the sections the round leaves as they are.
fn notebook_problems(conn: &Connection, project: &str, edits: &[Value]) -> Result<Vec<String>, String> {
    let mut problems = Vec::new();
    let mut sizes: BTreeMap<String, usize> = BTreeMap::new();
    for (section, _) in NOTEBOOK {
        let now = notebook_section(conn, project, section)?;
        sizes.insert(section.to_string(), now.map_or(0, |t| t.chars().count()));
    }
    let mut seen = BTreeSet::new();
    for x in edits {
        let section = text(&x["section"]).to_string();
        if !seen.insert(section.clone()) {
            problems.push(format!(
                "the notebook's section {section} is rewritten twice: rewrite it once, whole"
            ));
            continue;
        }
        let n = text(&x["text"]).chars().count();
        if n > SECTION_MOST {
            problems.push(format!(
                "the notebook's section {section} is {n} characters, and a section holds {SECTION_MOST} at most: \
                 write what holds now, shorter"
            ));
        }
        sizes.insert(section, n);
    }
    let all: usize = sizes.values().sum();
    if all > NOTEBOOK_MOST {
        problems.push(format!(
            "the notebook would be {all} characters in all, and it holds {NOTEBOOK_MOST} at most: shorten the \
             sections you rewrite, or rewrite another shorter"
        ));
    }
    Ok(problems)
}

/// Each kind of matter with the first cell of its path and the cell that decides it, where the matter goes back to when
/// the owner is asked about item changes of it the review dropped.
pub const KINDS: [(&str, &str, &str); 11] = [
    ("set_straight", "H1", "H1"),
    ("engineer_question", "H2", "H3"),
    ("report", "H4", "H5"),
    ("design_card", "H6", "H6"),
    ("failure", "H7", "H7"),
    ("owner_words", "H8", "H8"),
    ("other_architect", "H9", "H9"),
    ("slots_free", "H10", "H10"),
    (GROUP_TALK, "H12", "H12"),
    (CHECK, "P1", "P1"),
    (PLAN, "N1", "N1"),
];

/// The kind of matter that holds what was said in the groups of a project's works since its architect last read it,
/// other than to it: one accumulates for each project, its material worked out when it is handled, and, like flow
/// feedback, it starts no work of the architect and waits for one something else started, or one that runs queues it at
/// the end of a batch.
const GROUP_TALK: &str = "group_talk";

/// The field of the body of a matter of what was said in the groups that names the architect's work that queued it at
/// the end of a batch, which takes it; none when it was queued as the architect waited to start.
const QUEUED_IN: &str = "queued_in";

/// The kinds of matter that come at the head of the queue, taken in this order before any other but the owner's words,
/// a matter back with the owner's reply and the wait for a question just put: the check of whether the policy in force
/// still holds, and the plan of what to do next. One of each kind is open at a time: one asked for again takes the
/// newer asker's why.
const CHECK: &str = "check_policy";
const PLAN: &str = "make_plan";
const HEAD: [&str; 2] = [CHECK, PLAN];

/// The cell that writes a new policy, whose draft waits for the owner's approval, and the cell where it takes effect once
/// the owner replied to the question of its approval with exactly this word.
const SET_POLICY: &str = "P2";
const TAKES_EFFECT: &str = "P12";
const APPROVED: &str = "approved";

/// The engine's short question to the owner whether the draft of a new policy is approved.
const APPROVAL: &str = "Approve this policy? Reply exactly: approved";

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
const UNDERSTAND: &str = "U1";
const RETHINK: &str = "B4";
const REVIEW: &str = "B2";

/// The cell of the owner's words, which a matter back with the owner's reply goes to when the flow no longer has the
/// cell that asked.
const TALK: &str = "H8";

/// The architect's line, whose session goes on across the architect's works; the cell that asks whether that session
/// understood the project.
const LINE: &str = "architect";
const UNDERSTOOD_YET: &str = "S1";

/// The value the rows of the cell a project's architect waits at to start (`S0`) switch on: whether a matter is due.
const DUE: &str = "due";

/// The stage of the cells that set a new policy, and the check of the policy in force that opens it, which a flow
/// version with no row for a new session in the middle of a work names the cells of such a session by.
const SETTING_POLICY: &str = "policy";
const POLICY_CHECK: &str = "P1";

/// The cell of the engineer's graph a work is done at, as the views read it.
const ENGINEER_END: &str = "E1";

/// The most rows an input of the architect's gives of one list.
const ROWS: usize = 200;

/// The most entries [`since_last_work`] gives of each of its lists, the latest; how many earlier ones there are follows
/// them.
const LATEST: usize = 60;

/// The most characters [`since_last_work`] gives a commit's message by: its first sentence, cut at a word.
const COMMIT_MOST: usize = 120;

/// The most matters a batch handles before its item changes are reviewed.
const BATCH: i64 = 20;

/// The rows a round of the architect's cells takes when it hands in no answer of its own, as the views say it: it
/// asked for tools or a search, paused, asked the owner, or answered what the owner said.
const NO_ANSWER: &str = "('tools', 'search', 'pause', 'ask_owner', 'answers')";

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
/// forwarded by another project's architect, and what was said in the works' groups, which a work of the architect
/// takes when something else started it.
pub fn queue_has(e: &Engine, project: &str) -> Result<bool, String> {
    Ok(count(
        e.db.conn(),
        &format!(
            "SELECT count(*) FROM architect_queue
             WHERE project = ?1 AND NOT (kind IN {FEEDBACK_KINDS} AND detail IS ?2) AND kind <> ?3"
        ),
        &[&project, &FLOW_FEEDBACK, &GROUP_TALK],
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
/// work, without a round, as the engine takes the engineer's for an item that waits, once its action queued what was
/// said in the works' groups ([`queue_group_talk`]): when the row it takes leads on, a
/// work of the architect's graph starts for the project, on no item and on the flow version new works start on, at the
/// cell that row leads to, paused by the owner when the owner's step started it. Gives the work started; none while
/// the project waits there. A flow version whose start is no such cell starts the work at its start when one is due.
pub fn start(e: &mut Engine, project: &str, paused: bool) -> Result<Option<i64>, String> {
    let v = e.flow_now;
    let queues_talk = e
        .flow(v)?
        .graph(ARCHITECT)
        .and_then(|g| g.cell(&g.start))
        .is_some_and(|c| c.actions.iter().any(|a| a == "queue_group_talk"));
    if queues_talk {
        queue_group_talk(e, project)?;
    }
    let due = waits_to_start(e, project)?;
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

/// The matter the latest pick in a work of the architect chose before round `before`, or before none: the matter the
/// look at the queue took (cell `Q0`), or the one the architect picked (cell `Q1`); none when the latest look at the
/// queue found none, or many with no pick after it.
pub fn picked_before(conn: &Connection, work: i64, before: Option<i64>) -> Result<Option<i64>, String> {
    let row: Option<(String, Option<String>)> = conn
        .query_row(
            "SELECT cell, output FROM rounds r WHERE work = ?1 AND n < ?2 AND transition IS NOT NULL
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

/// How many matters an architect work's batch handled, those the look at the queue closed without a look, as a flow
/// version before did, left out.
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

/// The matters of a project's queue that the architect's work `work` can take now, oldest first: every one, flow
/// feedback, its own or forwarded, and what was said in the works' groups only when they came before the work started,
/// which wait for the architect's next work otherwise; what was said in the groups that the work itself queued at the
/// end of a batch ([`QUEUED_IN`]) it takes.
fn queued(conn: &Connection, project: &str, work: i64) -> Result<Vec<Value>, String> {
    query_json(
        conn,
        &format!(
            "SELECT * FROM architect_queue q WHERE q.project = ?1
               AND NOT ((q.kind IN {FEEDBACK_KINDS} AND q.detail IS ?3
                         OR q.kind = ?4 AND (SELECT json_extract(m.body, '$.{QUEUED_IN}') FROM architect_matters m
                                             WHERE m.id = q.matter) IS NOT ?2)
                 AND q.at >= coalesce((SELECT min(r.started_at) FROM rounds r WHERE r.work = ?2), q.at + 1))
             ORDER BY q.matter"
        ),
        &[&project, &work, &FLOW_FEEDBACK, &GROUP_TALK],
        100_000,
    )
}

/// The matters of a project's queue that the architect's work `work` can pick from the head of the queue on, oldest
/// first: those it can take ([`queued`]); but while new starts are held to set the project straight and some matter
/// sets it straight or has an agent stopped waiting, only those. The owner's words, a matter back with the owner's
/// reply and the wait for a question just put come before, taken from all it can take.
fn pickable(conn: &Connection, project: &str, work: i64) -> Result<Vec<Value>, String> {
    straight_first(conn, project, queued(conn, project, work)?)
}

/// The matters a work can take, but, while new starts are held to set the project straight and some matter sets it
/// straight or has an agent stopped waiting, only those.
fn straight_first(conn: &Connection, project: &str, rows: Vec<Value>) -> Result<Vec<Value>, String> {
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

/// The questions a project's architect put to the owner that wait for the owner's reply, newest first, as the owner's
/// interface reads them too ([`db::QUESTIONS_PENDING`]): with the question, the cell that asked, when it was put and the
/// matter it parked.
pub fn questions_pending(conn: &Connection, project: &str) -> Result<Vec<Value>, String> {
    query_json(conn, db::QUESTIONS_PENDING, &[&project], ROWS)
}

/// The questions pending, as the cells that may ask are given them; none when none waits.
fn questions(e: &Engine, project: &str) -> Result<Option<Value>, String> {
    let rows: Vec<Value> = questions_pending(e.db.conn(), project)?
        .into_iter()
        .map(|mut q| {
            q["at"] = json!(q["at"].as_i64().map(utc_minute));
            q
        })
        .collect();
    Ok((!rows.is_empty()).then_some(Value::Array(rows)))
}

/// The question the architect waits for the owner's reply to at the look at the queue (`Q0`), as the owner's interface
/// reads it too ([`db::question_waited_on`]): the newest question pending, when it was put less than the installation's
/// wait for the owner's reply (`agents.owner_reply_wait`) ago and the owner said nothing to the architect since in their
/// one-to-one. None otherwise: the question then waits at the back of the queue, and blocks nothing.
pub fn waits_on(e: &Engine, project: &str) -> Result<Option<i64>, String> {
    let wait = i64::try_from(e.config.config.install.agents.owner_reply_wait)
        .unwrap_or(i64::MAX / 1000)
        .saturating_mul(1000);
    db::question_waited_on(e.db.conn(), project, wait, db::now()).map_err(sql)
}

/// Whether a cell has a row of that name.
fn has_row(cell: &CompiledCell, row: &str) -> bool {
    cell.rows.iter().any(|r| r.name == row)
}

/// The matters the look at the queue takes before any other, where the cell has the row for them: the oldest of the
/// owner's words, and the oldest matter back with the owner's reply.
fn taken_first<'a>(cell: &CompiledCell, can: &'a [Value]) -> (Option<&'a Value>, Option<&'a Value>) {
    let said = can
        .iter()
        .find(|r| has_row(cell, "owner_words") && r["kind"] == "owner_words");
    let back = can
        .iter()
        .find(|r| has_row(cell, "answered") && (r["answered"] == json!(1) || r["answered"] == json!(true)));
    (said, back)
}

/// The matter at the head of the queue the look at the queue takes next, where the cell has the row for it: the check
/// of the policy before the plan of what to do next, the oldest of its kind.
fn at_head<'a>(cell: &CompiledCell, can: &'a [Value]) -> Option<&'a Value> {
    if !has_row(cell, "head") {
        return None;
    }
    HEAD.iter().find_map(|kind| can.iter().find(|r| r["kind"] == *kind))
}

/// Whether the look at the queue would wait for the owner's reply to a question now, as [`look_at_queue`] works it
/// out, without closing anything: so that a wait taken again records no new round.
pub fn would_wait(e: &Engine, w: &Work, cell: &CompiledCell) -> Result<bool, String> {
    if !has_row(cell, "waits") {
        return Ok(false);
    }
    let project = project_of(w)?;
    let all = queued(e.db.conn(), project, w.id)?;
    let (said, back) = taken_first(cell, &all);
    Ok(said.is_none() && back.is_none() && waits_on(e, project)?.is_some())
}

/// Cell `Q0`: what can be picked from the project's queue now; the owner's words, the oldest first, then a matter back
/// with the owner's reply, the oldest first, then the matters at the head of the queue ([`at_head`]), and a lone matter
/// are picked here, without a round; while the newest question pending was put just now and the owner said nothing
/// since, the architect waits for the reply ([`waits_on`]) before the matters at the head of the queue, which a flow
/// version before did not. The owner's words, a matter back with the reply and the wait come from all the work can
/// take ([`queued`]), never held back by the hold to set the project straight, which holds back only the matters from
/// the head of the queue on ([`pickable`]). A design card matter whose card the owner ruled or another replaced left
/// the queue as that happened ([`db::withdraw_card_looks`]); a flow version before closed it here, as the cell's
/// `close_settled_cards`, which a database migrated since has none of left to close.
pub fn look_at_queue(e: &mut Engine, w: &Work, cell: &CompiledCell) -> Result<Value, String> {
    let project = project_of(w)?;
    let conn = e.db.conn();
    let all = queued(conn, project, w.id)?;
    let can = straight_first(conn, project, all.clone())?;
    let handled = handled_in_batch(conn, w.id)?;
    let (said, back) = taken_first(cell, &all);
    let waits = if said.is_none() && back.is_none() && has_row(cell, "waits") {
        waits_on(e, project)?
    } else {
        None
    };
    let head = at_head(cell, &can);
    let queue = match (said, back, waits, head, can.as_slice()) {
        (Some(said), _, _, _, _) => json!({ "verdict": "owner_words", "matter": said["matter"] }),
        (None, Some(back), _, _, _) => json!({ "verdict": "answered", "matter": back["matter"] }),
        (None, None, Some(q), _, _) => json!({ "verdict": "waits", "question": q }),
        (None, None, None, Some(h), _) => json!({ "verdict": "head", "matter": h["matter"] }),
        (None, None, None, None, []) if handled > 0 || !collected(conn, w.id)?.is_empty() => {
            json!({ "verdict": "none_after_batch" })
        }
        (None, None, None, None, []) => json!({ "verdict": "none" }),
        (None, None, None, None, [one]) => json!({ "verdict": "one", "matter": one["matter"] }),
        (None, None, None, None, many) => json!({ "verdict": "many", "count": many.len() }),
    };
    Ok(json!({ "queue": queue }))
}

/// The owner's answer to the ask of the owner that held a matter; none while it has none.
fn answer_of(conn: &Connection, ask: i64) -> Result<Option<Value>, String> {
    let answer: Option<Option<String>> = conn
        .query_row("SELECT answer FROM owner_asks WHERE id = ?1", [ask], |r| r.get(0))
        .optional()
        .map_err(sql)?;
    Ok(json_col(answer.flatten()))
}

/// Where the matter picked goes: back to the cell that asked about it, when it comes back with the owner's answer, or to
/// the cell where a new policy takes effect when that cell is the one that writes it and the answer is exactly approved;
/// otherwise the first cell of its kind's path.
fn route(e: &Engine, w: &Work, g: &CompiledGraph) -> Result<String, String> {
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
        && let Some(answer) = answer_of(conn, a)?
    {
        let approved = text(&answer["text"]).trim() == APPROVED;
        if cell == SET_POLICY && approved && g.cell(TAKES_EFFECT).is_some() {
            return Ok(TAKES_EFFECT.into());
        }
        return Ok(if g.cell(&cell).is_some() { cell } else { TALK.into() });
    }
    KINDS
        .iter()
        .find(|k| k.0 == kind)
        .map(|k| k.1.to_string())
        .ok_or_else(|| format!("matter {m} is of a kind the architect's graph has no path for: {kind}"))
}

/// Whether the batch goes on, goes to its review, or is done; done with nothing to rethink when it handled no matter
/// and changed no item since the last rethink, as when the matter in hand was parked again for the owner's reply, for
/// which a flow version before ran the rethink all the same.
fn next_in_batch(e: &Engine, w: &Work, g: &CompiledGraph) -> Result<&'static str, String> {
    let conn = e.db.conn();
    let can = pickable(conn, project_of(w)?, w.id)?.len();
    let handled = handled_in_batch(conn, w.id)?;
    let nothing = || -> Result<bool, String> {
        let row = g.cells.iter().any(|c| {
            c.inenums
                .get("next_in_batch")
                .is_some_and(|vs| vs.iter().any(|v| v.0 == "nothing"))
        });
        let changed = count(
            conn,
            "SELECT count(*) FROM architect_item_changes WHERE work = ?1 AND round > ?2",
            &[&w.id, &batch_start(conn, w.id)?],
        )?;
        Ok(row && handled == 0 && changed == 0)
    };
    Ok(if can > 0 && handled < BATCH {
        "more"
    } else if !collected(conn, w.id)?.is_empty() {
        "review"
    } else if nothing()? {
        "nothing"
    } else {
        "done"
    })
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
    let level = crate::steps::level_of(e, g, cell, None)?.1;
    let home = e.cli_dir(&level.cli);
    let on = sessions::On {
        model: &level.model,
        home: home.as_deref(),
    };
    let limit = e.config.config.install.agents.context_limit;
    e.sessions
        .plan(e.db.conn(), w.id, w.project.as_deref(), cell, limit, on)
}

/// Whether the session the architect's work goes on in is the one its previous work left: a new one, or one the next
/// round would leave for a new one, is not, and understands the project first; a session that goes on never does again,
/// compacted or not.
fn understood(e: &Engine, w: &Work, g: &CompiledGraph) -> Result<bool, String> {
    Ok(matches!(plan_at_understanding(e, w, g)?, sessions::Plan::Going { .. }))
}

/// How the work came to the understanding question, which only a new session comes to: at the project's first
/// architect work (before any answer of the question, a round that asked for tools or paused being none), or for
/// another reason.
fn entry(e: &Engine, w: &Work) -> Result<String, String> {
    let project = project_of(w)?;
    let before = count(
        e.db.conn(),
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
        "entry" => entry(e, w)?,
        "route" => route(e, w, g)?,
        "next_in_batch" => next_in_batch(e, w, g)?.into(),
        "fixes_before" => if fixes_found(conn, w.id)? >= 2 { "two" } else { "fewer" }.into(),
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
/// or the login account changed), past the understanding, and a new session remembers nothing but the notebook. The
/// matter it had in hand goes back to the queue, and is taken again as any other.
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
    let level = crate::steps::level_of(e, g, cell, None)?.1;
    let home = e.cli_dir(&level.cli);
    let on = sessions::On {
        model: &level.model,
        home: home.as_deref(),
    };
    let limit = e.config.config.install.agents.context_limit;
    let plan = e
        .sessions
        .plan(e.db.conn(), w.id, w.project.as_deref(), cell, limit, on)?;
    Ok(match plan {
        sessions::Plan::New(why) => Some(crate::steps::new_session_why(why, cell, limit, &level.model)),
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
    crate::steps::engine(e, &back, g, &at, false)
}

// What the architect told the owner.

/// What a project's architect told the owner lately: its latest messages to the owner, by their numbers, each with when
/// and at which cell, oldest first, as an object by number, so that a session that holds them is given only those new
/// to it; none before the first.
fn told_owner(e: &Engine, project: &str) -> Result<Option<Value>, String> {
    let rows = query_json(
        e.db.conn(),
        "SELECT m.id, m.text, r.cell, coalesce(r.ended_at, r.started_at) AS at FROM owner_messages m
         JOIN rounds r ON r.work = m.work AND r.n = m.round JOIN works w ON w.id = m.work
         WHERE w.graph = 'architect' AND w.project = ?1
         ORDER BY m.id DESC LIMIT 30",
        &[&project],
        30,
    )?;
    let told: Map<String, Value> = rows
        .iter()
        .rev()
        .map(|r| {
            let at = r["at"].as_i64().map(utc_minute);
            (
                format!("#{}", r["id"]),
                json!({ "at": at, "cell": r["cell"], "text": r["text"] }),
            )
        })
        .collect();
    Ok((!told.is_empty()).then_some(Value::Object(told)))
}

// Asks of the owner.

/// The ask of the owner a round waits for or took, and its answer once the owner gave it: the one the round put, or the
/// one an earlier round of its cell put that the round's input names, as the round that took the owner's view of a
/// study or approval of a policy in a flow version before named the ask that held the setting.
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

/// What an answer of the owner's comes back to an owner's cell of the architect's graph as: a reply with the words,
/// ending with the line that names the images the owner sent with them.
fn answered_as(answer: &Value, images: &[i64]) -> Value {
    json!({ "reply": { "text": attachments::with_named(text(&answer["text"]), images) } })
}

/// The addresses of images, as the agents are given them: `image:<id>`.
fn image_addresses(ids: &[i64]) -> Value {
    json!(ids.iter().map(|i| attachments::address(*i)).collect::<Vec<_>>())
}

/// The owner's cell of the architect's graph: what the architect asks becomes an ask of the owner that holds only the
/// matter of the path that asked, and records the cell that asked, which takes the answer; the round ends at once
/// (`held`), and the architect's work goes on without it; the matter is back in the queue once the owner answers. An ask
/// that holds no matter, as an earlier architect put, waits for the answer.
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
        let images = attachments::of(e.db.conn(), Words::Ask(id)).map_err(sql)?;
        let output = answered_as(&answer, &images);
        let row = g.take(&cell.id, &output, &given)?.clone();
        return finish(e, w, g, cell, r.id, output, &row, Ended::default(), &[], None, &given);
    }
    let gathered = inputs::gather(e, w, g, cell, None)?;
    let given = values::given(e, w, g, cell, None, &gathered)?;
    let project = project_of(w)?.to_string();
    let commit = e.config.commit.clone();
    // A work of the architect never waits for the owner: its ask holds only the matter of the path that asked.
    let holds = picked(e.db.conn(), w.id)?;
    let mut body = json!({ "from": "architect", "project": project, "work": w.id, "cell": cell.id });
    let asked = asked_of(&gathered);
    let question = asked
        .iter()
        .find_map(|a| a["value"]["question"].as_str().map(str::to_string));
    let about = match &question {
        Some(q) => format!(": {}", db::one_sentence(q)),
        None => what_is_asked(&asked),
    };
    body["kind"] = json!(if question.is_some() { "question" } else { "ask" });
    if let Some(q) = question {
        body["question"] = json!(q);
    }
    body["asked"] = Value::Array(asked);
    let notice = format!("The architect of {project} asks you{about}");
    if let Some(m) = holds {
        body["holds"] = json!([m]);
    }
    // The cell the answer goes back to: the one that asked.
    let resume = w.pos.callers.last().or(w.pos.stopped.as_ref()).cloned();
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
    let text = format!(
        "The ask is with the owner as ask {id}. Matter {m} waits for the answer out of the queue, and comes back to \
         cell {c} with it; the work goes on with the other matters."
    );
    let output = json!({ "reply": { "text": text } });
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
/// to the cell that asked unchanged, a hand-over passes the work on, and the rest withdraw it; a row that takes the work
/// on to hold a slot again waits, in a step held to the slots (`slotted`), for its machine to have one free
/// ([`crate::steps::no_slot`]).
pub fn ask_architect(
    e: &mut Engine,
    w: &Work,
    g: &CompiledGraph,
    cell: &CompiledCell,
    item: &Item,
    open: Option<Round>,
    slotted: bool,
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
                let mut reply = json_col(reply).unwrap_or(Value::Null);
                // The owner's answer in the group ends with the line that names the images the owner sent with it.
                if let Some(m) = b.matter
                    && reply["by"] == "owner"
                {
                    let images = attachments::of(e.db.conn(), Words::Matter(m)).map_err(sql)?;
                    if !images.is_empty() {
                        reply["text"] = json!(attachments::with_named(text(&reply["text"]), &images));
                    }
                }
                json!({ "reply": reply })
            }
            _ => json!({}),
        };
        if crate::steps::no_slot(e, w, g, &row, slotted)? {
            return Ok(crate::steps::waits_for_slot(e, w));
        }
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
        "start_work_over" => {
            let item = a["item"].as_i64().unwrap_or_default();
            let Some((work, _, _)) = running_work(tx, item)? else {
                return Ok(());
            };
            if db::cannot_start_over(tx, work).map_err(sql)?.is_some() {
                return Ok(());
            }
            db::start_over(tx, work, db::Actor::Architect, Some(reason)).map_err(sql)
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

/// Makes one change to a project's observations, at once; `from` says where it came from, such as the matter in hand.
fn observe(tx: &Transaction<'_>, project: &str, from: &str, o: &Value) -> Result<(), String> {
    match text(&o["kind"]) {
        "new" => {
            let source = format!("{} ({from})", text(&o["source"]).trim());
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
                refs.push(json!(format!("Reported again ({from}): {seen}")));
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
/// changes to observations, actions and messages are made at once.
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
    cancel_within_batch(tx, w)?;
    for o in items(effects.get("observation_changes")) {
        observe(tx, &project, &format!("matter {matter}"), &o)?;
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

/// The keys of items the batch was to create that a reply names, in its children and its waits: those that are no
/// item's number.
fn keys_named(reply: &Value) -> Vec<String> {
    ["children", "waits"]
        .iter()
        .flat_map(|list| items(reply.get(*list)))
        .filter_map(|x| x["key"].as_str().map(|k| k.trim().trim_start_matches('#').to_string()))
        .filter(|k| !k.is_empty() && k.parse::<i64>().is_err())
        .collect()
}

/// The replies of an architect work's matters held for their item changes, once the batch is made: each goes out with
/// the numbers of the items made in place of their keys; one that names an item no change made, as when the review
/// dropped the change that creates it or the revision left it out, does not go out ([`not_sent`]). The matters in
/// `except` keep theirs: an ask of the owner holds them.
fn release_held_replies(
    tx: &Transaction<'_>,
    work: i64,
    keys: &BTreeMap<String, i64>,
    except: &BTreeSet<i64>,
) -> Result<(), String> {
    let held = query_json(
        tx,
        "SELECT id, reply_held FROM architect_matters WHERE handled_work = ?1 AND reply_held IS NOT NULL ORDER BY id",
        &[&work],
        10_000,
    )?;
    for h in held {
        let Some(m) = h["id"].as_i64().filter(|m| !except.contains(m)) else {
            continue;
        };
        let reply = &h["reply_held"];
        let named = keys_named(reply);
        if named.iter().any(|k| !keys.contains_key(k)) {
            not_sent(tx, work, m, reply, keys)?;
            continue;
        }
        let reply = numbered(reply, keys);
        tx.execute(
            "UPDATE architect_matters SET reply = ?2, reply_held = NULL WHERE id = ?1",
            params![m, reply.to_string()],
        )
        .map_err(sql)?;
        if let Some(b) = crate::blockers::of_matter(tx, m)?
            && b.ended.is_some()
        {
            crate::blockers::say(tx, b.id, "architect", "late", &reply)?;
        }
    }
    Ok(())
}

/// Why the item a matter's change was to create under a key was not made, as the batch's record of that change says:
/// what the review found when it dropped it, or why the batch dropped it, such as a cancel of it in the same batch;
/// that the revision left it out; or that no change of the batch was to create it.
fn why_not_made(tx: &Transaction<'_>, work: i64, matter: i64, key: &str) -> Result<Value, String> {
    let row = query_json(
        tx,
        "SELECT state, outcome FROM architect_item_changes
         WHERE work = ?1 AND matter = ?2 AND json_extract(change, '$.kind') = 'create_item'
           AND trim(json_extract(change, '$.key')) = ?3
         ORDER BY id DESC LIMIT 1",
        &[&work, &matter, &key],
        1,
    )?;
    Ok(match row.first().map(|r| (r["state"].as_str(), &r["outcome"])) {
        Some((Some("dropped"), outcome)) if !outcome.is_null() => outcome.clone(),
        Some((Some("revised"), _)) => {
            json!({ "what": "The revision of the batch left out the change that creates it" })
        }
        _ => json!({ "what": "No change of the batch made it" }),
    })
}

/// A held reply that names an item no change made does not go out, since the engineer must not go on from an item
/// that is not there: its matter goes back to the queue, not handled, with the reply that was not sent, the items it
/// named that were made, by their numbers, and why each other one was not, for its path to answer it again; the
/// question's blocker says so.
fn not_sent(
    tx: &Transaction<'_>,
    work: i64,
    matter: i64,
    reply: &Value,
    keys: &BTreeMap<String, i64>,
) -> Result<(), String> {
    let mut made = Map::new();
    let mut not_made = Vec::new();
    for k in keys_named(reply) {
        match keys.get(&k) {
            Some(n) => {
                made.insert(k, json!(n));
            }
            None => not_made.push(json!({ "key": k, "why": why_not_made(tx, work, matter, &k)? })),
        }
    }
    let record = json!({ "reply": reply, "made": made, "not_made": not_made });
    tx.execute(
        "UPDATE architect_matters SET handled_work = NULL, handled_round = NULL, reply_held = NULL,
           body = json_set(body, '$.reply_not_sent', json(?2))
         WHERE id = ?1",
        params![matter, record.to_string()],
    )
    .map_err(sql)?;
    if let Some(b) = crate::blockers::of_matter(tx, matter)?
        && b.ended.is_none()
    {
        crate::blockers::say(
            tx,
            b.id,
            "architect",
            "reply_not_sent",
            &json!({ "not_made": not_made }),
        )?;
    }
    Ok(())
}

/// The review of a batch made its changes or dropped them: its session ends, so that the next batch's review starts
/// afresh, and the hold to set the project straight is let go once nothing of it is left.
fn batch_over(tx: &Transaction<'_>, work: i64, project: &str) -> Result<(), String> {
    sessions::choose::end_line(tx, work, "architect_review")?;
    settle_hold(tx, work, project)
}

/// The item a change names by a key an earlier change of the batch creates it under, among those it names: the item it
/// changes, the item a dependency is on, a merge keeps or absorbs, a known red check's fix, and the item another is
/// made or moved under.
fn names_key(c: &Value, key: &str) -> bool {
    ["item", "on", "keep", "fix_item"]
        .iter()
        .map(|k| c[*k].clone())
        .chain([c["under"]["item"].clone(), c["to"]["item"].clone()])
        .chain(items(c.get("absorbed")))
        .any(|v| v.as_str().is_some_and(|s| s.trim() == key))
}

/// The rows of a batch the review found sound, and those it leaves out with what it found, by the review's rows,
/// numbered as the batch was given: those it drops (`drop`) and those it found to fix (`fix`), each with the review's
/// row; a change that names an item a dropped one creates is dropped with it, with the verdict of the change that
/// creates it.
fn sorted_by_review(rows: Vec<Change>, review: &[Value]) -> (Vec<Change>, Vec<(Change, Value)>) {
    let mut sound = Vec::new();
    let mut dropped: Vec<(Change, Value)> = Vec::new();
    for (i, row) in rows.into_iter().enumerate() {
        let finding = review
            .iter()
            .find(|r| r["change"].as_i64() == Some(i as i64 + 1) && (r["verdict"] == "fix" || r["verdict"] == "drop"));
        match finding {
            Some(f) => dropped.push((row, f.clone())),
            None => sound.push(row),
        }
    }
    loop {
        let gone: Vec<(String, Value)> = dropped
            .iter()
            .filter(|(r, _)| r.3["kind"] == "create_item")
            .map(|(r, f)| (text(&r.3["key"]).trim().to_string(), f["verdict"].clone()))
            .collect();
        let Some((i, verdict)) = sound.iter().enumerate().find_map(|(i, r)| {
            gone.iter()
                .find(|(k, _)| names_key(&r.3, k))
                .map(|(_, v)| (i, v.clone()))
        }) else {
            break;
        };
        let row = sound.remove(i);
        dropped.push((
            row,
            json!({ "verdict": verdict, "what": "It names an item a dropped change was to create" }),
        ));
    }
    (sound, dropped)
}

/// Marks rows of an architect work's batch dropped, each with why.
fn drop_rows(tx: &Transaction<'_>, rows: &[(Change, Value)]) -> Result<(), String> {
    for ((id, _, _, _), why) in rows {
        tx.execute(
            "UPDATE architect_item_changes SET state = 'dropped', outcome = ?2 WHERE id = ?1",
            params![id, why.to_string()],
        )
        .map_err(sql)?;
    }
    Ok(())
}

/// A cancel the batch collected of an item a create of the same batch makes, named by its key, cancels that create
/// out: the item is never made, so the create, every change of the batch that names its key and the cancel are
/// dropped, saying so, before the review looks at the batch, instead of the review being given an item made, moved
/// and cancelled in one batch, as in the sixth trial. A reply held for the batch that names such an item does not go
/// out ([`not_sent`]), and the rest of its answer's changes are dropped with it, since its matter is answered again.
fn cancel_within_batch(tx: &Transaction<'_>, work: i64) -> Result<(), String> {
    let rows = collected(tx, work)?;
    let created: BTreeSet<String> = rows
        .iter()
        .filter(|r| r.3["kind"] == "create_item")
        .map(|r| text(&r.3["key"]).trim().to_string())
        .filter(|k| !k.is_empty())
        .collect();
    let mut gone: Vec<(Change, Value)> = Vec::new();
    for (id, _, matter, c) in &rows {
        let key = text(&c["item"]).trim();
        if c["kind"] != "cancel" || !created.contains(key) {
            continue;
        }
        let why = json!({
            "what": format!("The item the batch was to create as {key} was cancelled in the same batch, by a change of \
                             matter {matter}: it is never made"),
        });
        for r in &rows {
            let creates = r.3["kind"] == "create_item" && text(&r.3["key"]).trim() == key;
            if (r.0 == *id || creates || names_key(&r.3, key)) && !gone.iter().any(|(g, _)| g.0 == r.0) {
                gone.push((r.clone(), why.clone()));
            }
        }
    }
    // A reply held for the batch that names an item cancelled out does not go out: its matter is answered again, and
    // the rest of its answer's changes go with the answer.
    let cancelled: BTreeSet<String> = rows
        .iter()
        .filter(|r| r.3["kind"] == "cancel")
        .map(|r| text(&r.3["item"]).trim().to_string())
        .filter(|k| created.contains(k))
        .collect();
    let held = query_json(
        tx,
        "SELECT id, reply_held FROM architect_matters WHERE handled_work = ?1 AND reply_held IS NOT NULL ORDER BY id",
        &[&work],
        10_000,
    )?;
    let again: Vec<(i64, Value)> = held
        .into_iter()
        .filter_map(|h| {
            let m = h["id"].as_i64()?;
            let names = keys_named(&h["reply_held"]).iter().any(|k| cancelled.contains(k));
            names.then(|| (m, h["reply_held"].clone()))
        })
        .collect();
    for (m, _) in &again {
        let why = json!({
            "what": format!(
                "Matter {m} is answered again: its reply named an item the batch cancelled in the same batch"
            ),
        });
        for r in rows.iter().filter(|r| r.2 == *m) {
            if !gone.iter().any(|(g, _)| g.0 == r.0) {
                gone.push((r.clone(), why.clone()));
            }
        }
    }
    drop_rows(tx, &gone)?;
    for (m, reply) in &again {
        not_sent(tx, work, *m, reply, &BTreeMap::new())?;
    }
    Ok(())
}

/// The review gave up on what it still found to fix at its third look: the changes it found sound are made, those it
/// drops are left out, and those it still finds to fix are dropped with what it found, and the owner is asked about
/// them in an ask that holds the matters they came from, each to go back, with the answer, to the cell that decides it.
fn give_up(ctx: &Ctx, tx: &Transaction<'_>) -> Result<(), String> {
    let (w, project) = architect_work_of(ctx)?;
    let review = items(ctx.unwrapped.get("review"));
    let (sound, left_out) = sorted_by_review(collected(tx, w)?, &review);
    let keys = make(tx, &sound, &ctx.projects)?;
    drop_rows(tx, &left_out)?;
    // What the review drops is left out, and what it still finds to fix is put to the owner.
    let dropped: Vec<(Change, Value)> = left_out.into_iter().filter(|(_, f)| f["verdict"] == "fix").collect();
    let held: BTreeSet<i64> = dropped.iter().map(|(r, _)| r.2).collect();
    release_held_replies(tx, w, &keys, &held)?;
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

/// The new measures of the draft that takes effect at round `at`: the latest the project's architect wrote (`P2`) before
/// it, as the policy in force reads its text; none when that draft names none.
fn approved_measures(tx: &Transaction<'_>, project: &str, at: RoundId) -> Result<Option<Value>, String> {
    let measures: Option<Option<String>> = tx
        .query_row(
            "SELECT json_extract(r.output, '$.outcome.new_measures') FROM rounds r JOIN works w ON w.id = r.work
             WHERE w.graph = ?1 AND w.project = ?2 AND r.cell = ?3 AND r.transition IS NOT NULL
               AND json_extract(r.output, '$.outcome.policy') IS NOT NULL
               AND (r.work < ?4 OR (r.work = ?4 AND r.n < ?5))
             ORDER BY r.work DESC, r.n DESC LIMIT 1",
            params![ARCHITECT, project, SET_POLICY, at.work, at.n],
            |r| r.get(0),
        )
        .optional()
        .map_err(sql)?;
    Ok(json_col(measures.flatten()))
}

/// A new measure as the queue tells two apart: its words however spaced and cased.
fn measure_key(measure: &str) -> String {
    measure.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

/// The new measures a project's queue holds, each a matter not handled nor withdrawn, by [`measure_key`].
fn queued_measures(tx: &Transaction<'_>, project: &str) -> Result<BTreeSet<String>, String> {
    let rows = query_json(
        tx,
        "SELECT json_extract(body, '$.measure.measure') AS measure FROM architect_matters
         WHERE project = ?1 AND kind = 'set_straight' AND handled_round IS NULL AND withdrawn IS NULL
           AND json_extract(body, '$.measure.measure') IS NOT NULL",
        &[&project],
        100_000,
    )?;
    Ok(rows.iter().map(|r| measure_key(text(&r["measure"]))).collect())
}

/// Why a matter at the head of the queue is asked for, when the cell that asks gives no why of its own: a new session
/// understood the project, or a new policy took effect.
fn head_why(cell: &str) -> String {
    match cell {
        UNDERSTAND => "A new session understood the project".into(),
        TAKES_EFFECT => "A new policy took effect".into(),
        other => format!("Cell {other} asked for it"),
    }
}

/// Queues a matter at the head of the project's queue, of the kind `kind`, the check of the policy or the plan of what
/// to do next, with why the round's cell asks for it: the text at `why`, or the cell's own reason. One of that kind
/// open already, in the queue or parked for the owner's reply, stands instead, and the newer asker's why is added to
/// it.
fn queue_head(ctx: &Ctx, tx: &Transaction<'_>, kind: &str, why: Option<&Value>) -> Result<(), String> {
    let (_, project) = architect_work_of(ctx)?;
    let why = match why.and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()) {
        Some(w) => w.to_string(),
        None => head_why(&ctx.cell.id),
    };
    let asker = json!({ "cell": ctx.cell.id, "round": format!("R{}-{}", ctx.round.work, ctx.round.n), "why": why });
    let open: Option<(i64, String)> = tx
        .query_row(
            "SELECT id, body FROM architect_matters WHERE project = ?1 AND kind = ?2 AND handled_round IS NULL
               AND withdrawn IS NULL ORDER BY id LIMIT 1",
            params![project, kind],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(sql)?;
    if let Some((id, body)) = open {
        let mut body: Value = serde_json::from_str(&body)
            .ok()
            .filter(Value::is_object)
            .unwrap_or_else(|| json!({}));
        if !body["asked_by"].is_array() {
            body["asked_by"] = json!([]);
        }
        if let Some(xs) = body["asked_by"].as_array_mut() {
            xs.push(asker);
        }
        tx.execute(
            "UPDATE architect_matters SET body = ?2 WHERE id = ?1",
            params![id, body.to_string()],
        )
        .map_err(sql)?;
        return Ok(());
    }
    let what = if kind == CHECK {
        format!(
            "Check whether the policy of {project} still holds: {}",
            db::one_sentence(&why)
        )
    } else {
        format!("Plan what to do next for {project}: {}", db::one_sentence(&why))
    };
    let m = Matter {
        project,
        kind: kind.into(),
        detail: None,
        what,
        body: json!({ "asked_by": [asker] }),
        item: None,
        raised: Some((ctx.round.work, ctx.round.n)),
    };
    queue_matter(tx, &m).map_err(sql)?;
    Ok(())
}

/// Carries out an action of the architect's graph; false for an action of another.
pub fn act(ctx: &Ctx, tx: &Transaction<'_>, name: &str, at: Option<&Value>) -> Result<bool, String> {
    let w = ctx.work.id;
    match name {
        "edit_notebook" => {
            // Each section the round rewrote is the notebook's latest version of it; the versions before stay.
            let (_, project) = architect_work_of(ctx)?;
            for x in items(at) {
                tx.execute(
                    "INSERT INTO notebook_edits (project, section, text, work, round) VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        project,
                        text(&x["section"]),
                        text(&x["text"]),
                        ctx.round.work,
                        ctx.round.n
                    ],
                )
                .map_err(sql)?;
            }
        }
        "queue_fixes" => {
            let (_, project) = architect_work_of(ctx)?;
            let fit = &ctx.unwrapped["fit"];
            // One matter for each place, with every step the understanding meant to take there; a place the queue
            // holds already, which the fix names, is not queued again.
            for fix in items(at).into_iter().filter(|x| x["queued"].as_i64().is_none()) {
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
        "queue_measures" => {
            // Each number a signal of the policy that takes effect needs that the engine does not measure yet, as the
            // approved draft hands it in: something missing, which its path makes an item of. A flow version before
            // queued them where each draft was written, at the place given.
            let (_, project) = architect_work_of(ctx)?;
            let measures = match at {
                Some(v) => items(Some(v)),
                None => items(approved_measures(tx, &project, ctx.round)?.as_ref()),
            };
            let mut held = queued_measures(tx, &project)?;
            for x in measures {
                let measure = text(&x["measure"]).trim().to_string();
                // A measure the queue holds already is not queued again.
                if measure.is_empty() || !held.insert(measure_key(&measure)) {
                    continue;
                }
                let fix = json!({
                    "where": "The numbers the engine measures",
                    "items": [],
                    "works": [],
                    "kind": "missing",
                    "wrong": format!(
                        "A signal of the policy being set needs a number the engine does not measure: {}",
                        text(&x["signal"]).trim()
                    ),
                    "steps": [format!("Make an item that has the engine measure {measure}")],
                });
                let m = Matter {
                    project: project.clone(),
                    kind: "set_straight".into(),
                    detail: Some("missing".into()),
                    what: format!(
                        "A new measure a signal of the policy needs: {}",
                        db::one_sentence(&measure)
                    ),
                    body: json!({ "fix": fix, "measure": x }),
                    item: None,
                    raised: Some((ctx.round.work, ctx.round.n)),
                };
                queue_matter(tx, &m).map_err(sql)?;
            }
        }
        "hold_to_set_straight" => hold(tx, &architect_work_of(ctx)?.1, "set_straight", None)?,
        "release_straight_hold" => release(tx, &architect_work_of(ctx)?.1, "set_straight")?,
        "settle_straight_hold" => settle_hold(tx, w, &architect_work_of(ctx)?.1)?,
        "queue_check_policy" => queue_head(ctx, tx, CHECK, at)?,
        "queue_make_plan" => queue_head(ctx, tx, PLAN, at)?,
        "queue_group_talk" => {
            // At the end of a batch, what was said in the groups since the architect last read it, for this work,
            // which takes it, so that a work that runs long is not blind to it until it ends.
            let (_, project) = architect_work_of(ctx)?;
            if let Some(m) = group_talk_due(tx, &project, Some(w))? {
                queue_matter(tx, &m).map_err(sql)?;
            }
        }
        "ask_approval" => {
            // The engine's short question whether the draft the round wrote is approved, pinned in the owner's
            // one-to-one as the architect's question, which parks the matter in hand at the cell; without the draft,
            // which is the round's return.
            let (_, project) = architect_work_of(ctx)?;
            let m = matter_of(tx, w)?;
            let body = json!({
                "from": "architect", "project": project, "work": w, "cell": ctx.cell.id, "kind": "question",
                "question": APPROVAL, "holds": [m],
            });
            tx.execute(
                "INSERT INTO owner_asks (work, round, body) VALUES (?1, ?2, ?3)",
                params![ctx.round.work, ctx.round.n, body.to_string()],
            )
            .map_err(sql)?;
            let ask = tx.last_insert_rowid();
            tx.execute(
                "UPDATE architect_matters SET asked = ?2, resume_cell = ?3 WHERE id = ?1",
                params![m, ask, ctx.cell.id],
            )
            .map_err(sql)?;
            actions::notify_ask(
                tx,
                w,
                ask,
                &format!("The architect of {project} asks you: {APPROVAL} (ask {ask})"),
            )?;
        }
        "reply_to_owner" => {
            // The reply to the owner's words is the architect's message in the conversation they were said in, which
            // the conversations work out from the matter the round had in hand; the notifications log says it too.
            let Some(said) = at.and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()) else {
                return Ok(true);
            };
            tx.execute(
                "INSERT INTO owner_messages (work, round, text) VALUES (?1, ?2, ?3)",
                params![ctx.round.work, ctx.round.n, said],
            )
            .map_err(sql)?;
            let (_, project) = architect_work_of(ctx)?;
            let logged = format!("The architect of {project} replies: {said}");
            actions::outbox(tx, Some(w), "notify", &json!({ "text": logged }))?;
        }
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
        "change_observations" => {
            // The changes to the project's observations a round hands in beside its own answer, such as the revision of
            // a batch closing one its review found done already: made at once, as an answer's effects make theirs.
            let (_, project) = architect_work_of(ctx)?;
            let from = format!("R{}-{}, {}", ctx.round.work, ctx.round.n, ctx.cell.title);
            for o in items(at) {
                observe(tx, &project, &from, &o)?;
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
            cancel_within_batch(tx, w)?;
        }
        "apply_batch" => {
            // What the review drops is left out; a flow version before named no review, and the batch was made whole.
            let (_, project) = architect_work_of(ctx)?;
            let (rows, dropped) = sorted_by_review(collected(tx, w)?, &items(at));
            let keys = make(tx, &rows, &ctx.projects)?;
            drop_rows(tx, &dropped)?;
            release_held_replies(tx, w, &keys, &BTreeSet::new())?;
            batch_over(tx, w, &project)?;
        }
        "apply_reviewed" => give_up(ctx, tx)?,
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

/// The texts of the owner's words in the asks of the work's project's architect, in the owner's matters and in the
/// entries of its talks: what a policy may not quote.
fn owner_words(conn: &Connection, work: i64) -> Result<Vec<String>, String> {
    let rows = query_json(
        conn,
        "SELECT json_extract(a.answer, '$.text') AS text FROM owner_asks a JOIN works w ON w.id = a.work
           JOIN works x ON x.id = ?1
         WHERE w.project = x.project AND w.graph IN ('architect', 'policy') AND a.answer IS NOT NULL
         UNION ALL SELECT json_extract(body, '$.text') FROM architect_matters WHERE kind = 'owner_words'
         UNION ALL SELECT json_extract(body, '$.answer') FROM architect_matters WHERE kind = 'owner_words'
         UNION ALL SELECT text FROM talk_entries",
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

/// Whether a project's observation is open.
fn open_observation(conn: &Connection, project: &str, id: i64) -> Result<bool, String> {
    Ok(count(
        conn,
        "SELECT count(*) FROM observations_open WHERE project = ?1 AND id = ?2",
        &[&project, &id],
    )? > 0)
}

/// What is wrong with the effects of an answer, every reason at once: its item changes, as the batch would have them
/// after the ones it collected; its changes to observations; its actions; its messages.
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
    problems.extend(observation_problems(
        c,
        project,
        &items(effects.get("observation_changes")),
    )?);
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
            ("pause_work" | "interject" | "start_work_over", None) | ("resume_work", None) => problems.push(format!(
                "action {n}: item {} has no work running",
                item.unwrap_or_default()
            )),
            ("start_work_over", Some((work, _, _))) => {
                if let Some(no) = db::cannot_start_over(conn, *work).map_err(sql)? {
                    problems.push(format!("action {n}: {no}"));
                }
            }
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
    Ok(problems)
}

/// What is wrong with changes to a project's observations, as an answer's effects or the revision of a batch hands them
/// in: a new one that says nothing or carries no tag of the project's, or one not of them; a merge or a close of an
/// observation the project has no open one of; a close of one an item takes in, which is folded into that item instead.
fn observation_problems(c: &checks::Ctx<'_>, project: &str, changes: &[Value]) -> Result<Vec<String>, String> {
    let conn = c.e.db.conn();
    let mut problems = Vec::new();
    let tags: BTreeSet<String> =
        c.e.config
            .config
            .projects
            .get(project)
            .map(|p| p.observation_tags.iter().map(|t| t.name.clone()).collect())
            .unwrap_or_default();
    for (i, o) in changes.iter().enumerate() {
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
    Ok(problems)
}

/// Whether two places to set straight are the same place: of one kind, and at the same place in words, or about the
/// same items.
fn same_place(a: &Value, b: &Value) -> bool {
    let place = |x: &Value| {
        text(&x["where"])
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase()
    };
    let ids = |x: &Value| -> BTreeSet<i64> { items(x.get("items")).iter().filter_map(Value::as_i64).collect() };
    a["kind"] == b["kind"] && (place(a) == place(b) || (!ids(a).is_empty() && ids(a) == ids(b)))
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
        "fix_not_queued_again" => {
            let fixes = match (at, v) {
                (Some("fit"), Some(fit)) => items(fit.get("fixes")),
                (_, v) => items(v),
            };
            let open = query_json(
                conn,
                "SELECT id, json_extract(body, '$.fix') AS fix FROM architect_matters
                 WHERE project = ?1 AND kind = 'set_straight' AND handled_round IS NULL AND withdrawn IS NULL
                 ORDER BY id",
                &[&project_of(c.w)?],
                ROWS,
            )?;
            let row = if at == Some("fit") { "fix" } else { "thing missing" };
            let mut problems = Vec::new();
            for (i, x) in fixes.iter().enumerate() {
                let n = i + 1;
                if let Some(m) = x["queued"].as_i64() {
                    if !open.iter().any(|o| o["id"].as_i64() == Some(m)) {
                        problems.push(format!(
                            "{row} {n} names matter {m} as queued, which is no place to set straight your queue holds"
                        ));
                    }
                    continue;
                }
                if let Some(o) = open.iter().find(|o| same_place(&o["fix"], x)) {
                    problems.push(format!(
                        "{row} {n} is matter {}, which your queue holds already: name it in queued, and it is not \
                         queued again",
                        o["id"]
                    ));
                }
            }
            if problems.is_empty() {
                Ok(())
            } else {
                Err(problems.join("\n"))
            }
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
        "observation_changes_valid" => {
            let problems = observation_problems(c, project_of(c.w)?, &items(v))?;
            if problems.is_empty() {
                Ok(())
            } else {
                Err(problems.join("\n"))
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
        "notebook_fits" => {
            let Some(edits) = v else { return Ok(()) };
            let problems = notebook_problems(conn, project_of(c.w)?, &items(Some(edits)))?;
            if problems.is_empty() {
                Ok(())
            } else {
                Err(problems.join("\n"))
            }
        }
        "understanding_written" | "plan_written" => {
            let wants: &[&str] = if name == "plan_written" {
                &["plan"]
            } else {
                &["project_now", "unclear"]
            };
            let given = items(v);
            let missing: Vec<&str> = wants
                .iter()
                .copied()
                .filter(|w| !given.iter().any(|x| text(&x["section"]) == *w))
                .collect();
            if missing.is_empty() {
                Ok(())
            } else {
                Err(format!(
                    "rewrite the notebook's section{} {} whole in notebook",
                    if missing.len() > 1 { "s" } else { "" },
                    missing.join(" and ")
                ))
            }
        }
        "no_holds_after_reply" => {
            if v.is_none_or(|x| x["verdict"] != "holds") {
                return Ok(());
            }
            let Some(m) = picked(conn, c.w.id)? else { return Ok(()) };
            let replied = count(
                conn,
                "SELECT count(*) FROM architect_matters m JOIN owner_asks a ON a.id = m.asked
                 WHERE m.id = ?1 AND a.answer IS NOT NULL",
                &[&m],
            )? > 0;
            if replied {
                return Err(
                    "the owner replied to your question whether to research a new policy: once the owner replied, \
                     the policy is not handed in as holding; hand in write_policy with the owner's stance, or ask \
                     again when the reply does not answer the question"
                        .into(),
                );
            }
            Ok(())
        }
        "asked_before_writing" => {
            if v.is_none_or(|x| x["verdict"] != "write_policy") {
                return Ok(());
            }
            let Some(m) = picked(conn, c.w.id)? else { return Ok(()) };
            // The owner said yes: by the reply to the question whether to research a new policy, or by the owner's own
            // words, which asked for this check.
            let yes = count(
                conn,
                "SELECT count(*) FROM architect_matters m
                 WHERE m.id = ?1 AND (EXISTS (SELECT 1 FROM owner_asks a WHERE a.id = m.asked AND a.answer IS NOT NULL)
                   OR EXISTS (SELECT 1 FROM json_each(m.body, '$.asked_by') b
                              WHERE json_extract(b.value, '$.cell') = ?2))",
                &[&m, &TALK],
            )? > 0;
            if !yes {
                return Err(
                    "the owner has not said yes to a new policy: ask the owner first whether to research one, saying \
                     why you doubt the policy in force, and hand in write_policy once the owner replied; only a check \
                     the owner's own words asked for (queued by the answer to them, H8, as why it is checked says) \
                     goes on to write a new policy without asking"
                        .into(),
                );
            }
            Ok(())
        }
        "policy_text_clean" => {
            let Some(policy) = v else { return Ok(()) };
            let text = policy.to_string();
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
            if let Some(item) = names_item(policy) {
                return Err(format!(
                    "the policy names the item {item}: what to do next under it is thought out at cell N1, never written \
                     into it"
                ));
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
        "owner_answer" => owner_answer(e, w),
        "cross_project" => cross_project(e, project).map(Some),
        "batch_changes" => batch_changes(e, w).map(Some),
        "changes_touch" => changes_touch(e, w),
        "words_behind_changes" => words_behind_changes(e, w),
        "group_talk" => group_talk(e, w, project),
        "told_owner" => told_owner(e, project),
        "notebook_project_now" | "notebook_plan" | "notebook_unclear" | "notebook_other" => {
            let section = NOTEBOOK.iter().find(|x| x.1 == name).map_or("", |x| x.0);
            Ok(notebook_section(e.db.conn(), project, section)?.map(Value::String))
        }
        "questions" => questions(e, project),
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
/// who waits for it, each by the kind of its dependency, since when its work runs and what it spent, and its own pause
/// with who paused it (the owner, the engine or the architect) and why, or why its last work stopped, as one queued
/// again, and for one with no work running, the items not merged yet that it waits for by
/// a blocking dependency before it starts, and its place in the owner's start queue; the owner's start queue, the
/// project's items the owner put first to start, in the owner's order, which start ahead of the ranks (`dispatch_queue`);
/// the release items, each an item nothing depends on with items under it, with the items under it; and the
/// dependencies with other projects' items. A work at a cell of no stage, which asks or
/// runs tools for another, is at the stage of the cell it stopped at, or else of the innermost cell that waits for it
/// with one.
fn board(e: &Engine, project: &str) -> Result<Value, String> {
    let g = engineer_graph(e);
    let end = g.map(|g| g.end.clone()).unwrap_or_default();
    let rows = query_json(
        e.db.conn(),
        "SELECT i.id AS item, i.title, i.rank, i.held, i.level, i.deliver, w.id AS work, w.at, w.resume_at, w.waiting,
                w.paused, w.paused_by, w.ended, (SELECT min(r.started_at) FROM rounds r WHERE r.work = w.id) AS since,
                (SELECT json_group_object('#' || l.depends_on, l.kind) FROM item_links l WHERE l.item = i.id)
                  AS waits_for,
                (SELECT json_group_object('#' || l.item, l.kind) FROM item_links l WHERE l.depends_on = i.id)
                  AS waited_by,
                EXISTS (SELECT 1 FROM ready_items r WHERE r.item = i.id) AS ready,
                (SELECT json_group_array('#' || b.depends_on) FROM item_blocked_by b WHERE b.item = i.id)
                  AS blocked_by,
                CASE WHEN EXISTS (SELECT 1 FROM dispatch_queue p WHERE p.item = i.id) THEN
                  (SELECT count(*) FROM dispatch_queue q JOIN items y ON y.id = q.item
                   WHERE y.project = i.project
                     AND q.position <= (SELECT p.position FROM dispatch_queue p WHERE p.item = i.id)) END
                  AS in_owner_queue
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
            if let Some(why) = r["paused"].as_str().map(str::to_string) {
                let by = if why.starts_with(db::STOPPED_BY_ARCHITECT) {
                    json!("architect")
                } else {
                    r["paused_by"].clone()
                };
                r["paused"] = json!({ "by": by, "why": why });
            }
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
            for k in ["resume_at", "waiting", "ended", "paused_by"] {
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
    let queue: Vec<String> = query_json(
        e.db.conn(),
        "SELECT q.item FROM dispatch_queue q JOIN items i ON i.id = q.item WHERE i.project = ?1 ORDER BY q.position",
        &[&project],
        ROWS,
    )?
    .iter()
    .map(|r| format!("#{}", r["item"]))
    .collect();
    Ok(json!({
        "items": out,
        "owner_start_queue": queue,
        "releases": releases,
        "other_projects": cross_project(e, project)?,
    }))
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

/// When a project's architect last read what was said in the groups of its works, in its works up to `upto`: the start
/// of its latest round that was given it, or referred to it, as the round recorded its inputs: at its cell (`H12`), or
/// at the pick (`Q1`) or the wrap-up (`E1`) a flow version before gave it to. None before the first.
fn last_read(conn: &Connection, project: &str, upto: i64) -> Result<Option<i64>, String> {
    conn.query_row(
        &format!(
            "SELECT max(r.started_at) FROM rounds r JOIN works x ON x.id = r.work
             WHERE x.graph = 'architect' AND x.project = ?1 AND x.id <= ?2 AND r.cell IN {}
               AND r.ended_at IS NOT NULL
               AND EXISTS (SELECT 1 FROM json_each(r.input, '$.conditional') c
                           WHERE json_extract(c.value, '$.address') LIKE '%·engine·group_talk')",
            db::cells_sql(ARCHITECT, &["H12", "Q1", "E1"])
        ),
        params![project, upto],
        |r| r.get(0),
    )
    .map_err(sql)
}

/// What was said in the groups of a project's works after `since`, other than to its architect, oldest first: the
/// owner's words to the engineers, what the engineers told the owner, and the owner's answers, to the engineers' asks
/// and, in a group, to a question the architect held; each with its work and item, and the owner's words with the
/// images the owner sent with them, by their addresses, everything said in a group reaching all three.
fn said_in_groups(conn: &Connection, project: &str, since: i64) -> Result<Vec<Value>, String> {
    query_json(
        conn,
        "SELECT * FROM (
           SELECT j.work, w.item, i.title, 'owner' AS who, 'engineer' AS \"to\", j.text, NULL AS asked,
             (SELECT min(c.at) FROM changes c
              WHERE c.table_name = 'interjections' AND c.row_key = json_object('id', j.id)) AS at,
             nullif((SELECT json_group_array('image:' || x.id ORDER BY x.position) FROM attachments x
                     WHERE x.interjection = j.id), '[]') AS images
           FROM interjections j JOIN works w ON w.id = j.work JOIN items i ON i.id = w.item
           WHERE i.project = ?1 AND w.graph = 'engineer' AND j.speaker = 'owner' AND j.matter IS NULL
             AND j.informs = 0 AND j.cancelled_item IS NULL AND j.text NOT LIKE 'Ruling on design card %'
           UNION ALL
           SELECT m.work, w.item, i.title, 'engineer', 'owner', m.text, NULL, coalesce(r.ended_at, r.started_at), NULL
           FROM owner_messages m JOIN rounds r ON r.work = m.work AND r.n = m.round JOIN works w ON w.id = m.work
           JOIN items i ON i.id = w.item LEFT JOIN sessions s ON s.id = r.session
           WHERE i.project = ?1 AND w.graph = 'engineer' AND coalesce(s.line, 'engineer') = 'engineer'
           UNION ALL
           SELECT a.work, w.item, i.title, 'owner', 'engineer', json_extract(a.answer, '$.text'),
             (SELECT group_concat(json_extract(p.value, '$.what'), char(10))
              FROM json_each(a.body, '$.asked') q, json_each(q.value, '$.value.points') p),
             json_extract(a.answer, '$.at'),
             nullif((SELECT json_group_array('image:' || x.id ORDER BY x.position) FROM attachments x
                     WHERE x.ask = a.id), '[]')
           FROM owner_asks a JOIN works w ON w.id = a.work JOIN items i ON i.id = w.item
           WHERE i.project = ?1 AND w.graph = 'engineer' AND json_extract(a.answer, '$.by') = 'owner'
           UNION ALL
           SELECT m.work, w.item, i.title, 'owner', 'engineer', json_extract(m.reply, '$.text'), m.what,
             json_extract(m.reply, '$.at'),
             nullif((SELECT json_group_array('image:' || x.id ORDER BY x.position) FROM attachments x
                     WHERE x.matter = m.id), '[]')
           FROM architect_matters m JOIN works w ON w.id = m.work JOIN items i ON i.id = w.item
           WHERE m.project = ?1 AND w.graph = 'engineer' AND json_extract(m.reply, '$.by') = 'owner')
         WHERE at > ?2 ORDER BY at",
        &[&project, &since],
        ROWS,
    )
}

/// The action of the cell a project's architect waits at to start (`S0`), `queue_group_talk`: what was said in the
/// groups of the project's works since its architect last read it, other than to it, as one matter of the kind
/// group_talk, when there is such talk and none is open ([`group_talk_due`]); new talk joins the one open, whose
/// material is worked out when it is handled ([`group_talk`]). The first time, the week before now.
fn queue_group_talk(e: &mut Engine, project: &str) -> Result<(), String> {
    let Some(m) = group_talk_due(e.db.conn(), project, None)? else {
        return Ok(());
    };
    e.db.write(Actor::Engine, |tx| queue_matter(tx, &m)).map_err(sql)?;
    Ok(())
}

/// The matter of what was said in the groups of a project's works since its architect last read it, other than to it,
/// to queue: none when nothing was said since, or one is open, which the new talk joins. Queued by a work of the
/// architect at the end of a batch, it names that work ([`QUEUED_IN`]), which takes it; queued as the architect waits
/// to start, it waits for a work something else starts.
fn group_talk_due(conn: &Connection, project: &str, by: Option<i64>) -> Result<Option<Matter>, String> {
    let open = count(
        conn,
        "SELECT count(*) FROM architect_matters WHERE project = ?1 AND kind = ?2 AND handled_round IS NULL
           AND withdrawn IS NULL",
        &[&project, &GROUP_TALK],
    )?;
    if open > 0 {
        return Ok(None);
    }
    let since = last_read(conn, project, i64::MAX)?.unwrap_or_else(|| db::now() - 7 * 24 * HOUR);
    if said_in_groups(conn, project, since)?.is_empty() {
        return Ok(None);
    }
    let mut body = json!({});
    if let Some(w) = by {
        body[QUEUED_IN] = json!(w);
    }
    Ok(Some(Matter {
        project: project.to_string(),
        kind: GROUP_TALK.into(),
        detail: None,
        what: format!("What was said in the groups of {project}'s works since its architect last read it"),
        body,
        ..Matter::default()
    }))
}

/// What was said in the groups of a project's works since its architect last read it ([`last_read`]), other than to
/// the architect, oldest first ([`said_in_groups`]), the material of the matter that holds it (`H12`). The architect
/// gets it as information, which needs no answer: everything said in a group reaches all three. The first time, the
/// week before the work; none when nothing was said.
fn group_talk(e: &Engine, w: &Work, project: &str) -> Result<Option<Value>, String> {
    let conn = e.db.conn();
    let since = match last_read(conn, project, w.id)? {
        Some(t) => t,
        None => work_start(conn, w.id)? - 7 * 24 * HOUR,
    };
    let said = said_in_groups(conn, project, since)?;
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

/// The latest [`LATEST`] of a list given latest first, and after them how many earlier ones there are since `since`,
/// as `and 367 earlier commits since 2026-10-23 11:35 UTC`: what [`since_last_work`] gives of each of its lists. `what`
/// names one of them and more.
fn latest(mut rows: Vec<Value>, what: (&str, &str), since: &str) -> Value {
    let earlier = rows.len().saturating_sub(LATEST);
    rows.truncate(LATEST);
    if earlier > 0 {
        let what = if earlier == 1 { what.0 } else { what.1 };
        rows.push(json!(format!("and {earlier} earlier {what} since {since}")));
    }
    Value::Array(rows)
}

/// The commits of a `git log --format='%h %cs %s'`, the latest first, as [`since_last_work`] gives them: each by its
/// hash, its date and its message's first sentence, cut at a word to [`COMMIT_MOST`] characters; the latest
/// [`LATEST`], then how many earlier ones there are since `since`.
fn commits(log: &str, since: &str) -> Value {
    let rows: Vec<Value> = log
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let mut parts = l.splitn(3, ' ');
            let (hash, date) = (parts.next().unwrap_or_default(), parts.next().unwrap_or_default());
            let first = flow::cut(&db::one_sentence(parts.next().unwrap_or_default()), COMMIT_MOST);
            json!(format!("{hash} {date} {first}"))
        })
        .collect();
    latest(rows, ("commit", "commits"), since)
}

/// What happened in a project since its architect's last work started; at the architect's first work, in the week
/// before it. Each list is the latest first, the latest [`LATEST`] of it, then how many earlier ones there are: a
/// repository with long messages and many commits would otherwise fill the input, and the agent reads the rest of
/// the history with git.
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
    let from = utc_minute(since);
    let main = e.config.config.projects.get(project).map_or(Value::Null, |p| {
        let after = format!("--since=@{}", since / 1000);
        git::git(
            &p.checkout,
            &["log", "--format=%h %cs %s", after.as_str(), main_ref(p).as_str()],
        )
        .map_or(Value::Null, |log| commits(&log, &from))
    });
    // Each list whole, the latest first, of which `latest` keeps the latest.
    let all = |q: &str| query_json(conn, q, &[&project, &since], usize::MAX);
    let merged = all(&format!(
        "SELECT w.item, i.title, w.pr, json_extract(r.output, '$.result.merge_commit') AS commit_merged
         FROM rounds r JOIN works w ON w.id = r.work JOIN items i ON i.id = w.item
         WHERE i.project = ?1 AND r.cell IN {} AND r.transition = 'merged' AND r.ended_at > ?2
         ORDER BY r.ended_at DESC",
        db::cells_sql(ENGINEER, &["M2"])
    ))?;
    let works = all(&format!(
        "SELECT w.id AS work, w.item, i.title, w.at, w.paused, w.ended FROM works w JOIN items i ON i.id = w.item
         WHERE i.project = ?1 AND w.graph = 'engineer'
           AND EXISTS (SELECT 1 FROM rounds r WHERE r.work = w.id AND r.started_at > ?2)
           AND (w.ended IS NOT NULL OR w.at IN {})
         ORDER BY w.id DESC",
        db::cells_sql(ENGINEER, &[ENGINEER_END])
    ))?;
    let main_ci = all(&format!(
        "SELECT w.item, r.transition, json_extract(r.output, '$.main.kind') AS kind, r.ended_at AS at
         FROM rounds r JOIN works w ON w.id = r.work JOIN items i ON i.id = w.item
         WHERE i.project = ?1 AND r.cell IN {} AND r.ended_at > ?2 ORDER BY r.ended_at DESC",
        db::cells_sql(ENGINEER, &["L1"])
    ))?;
    let owner = all("SELECT m.id AS matter, m.what FROM architect_matters m
         WHERE m.project = ?1 AND m.kind = 'owner_words' AND m.at > ?2 ORDER BY m.id DESC")?;
    let cards = all(
        "SELECT c.id AS design_card, w.item, c.status FROM design_cards c JOIN works w ON w.id = c.work
         JOIN items i ON i.id = w.item
         WHERE i.project = ?1 AND c.status = 'ruled' AND EXISTS (
           SELECT 1 FROM changes x WHERE x.table_name = 'design_cards' AND x.row_key = json_object('id', c.id)
             AND x.at > ?2)
         ORDER BY c.id DESC",
    )?;
    let reports = all(
        "SELECT id AS matter, what FROM architect_matters WHERE project = ?1 AND kind = 'report' AND at > ?2
         ORDER BY id DESC",
    )?;
    let mut out = json!({
        "since": from,
        "main": main,
        "merged": latest(merged, ("merge", "merges"), &from),
        "works_ended": latest(works, ("work", "works"), &from),
        "main_after_merges": latest(main_ci, ("check of main", "checks of main"), &from),
        "owner_said": latest(owner, ("thing the owner said", "things the owner said"), &from),
        "design_cards_ruled": latest(cards, ("design card", "design cards"), &from),
        "reports": latest(reports, ("report", "reports"), &from),
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

/// Whether the engine runs works now: every work going on that is not paused by itself; every work paused by the owner,
/// but those the owner started; or every work stopped by a failed run's cause until when it says.
fn engine_state(e: &Engine) -> Value {
    if let Some(h) = e.halt.as_ref().filter(|h| db::now() < h.until) {
        return json!({
            "runs": "halted",
            "why": format!("Every work stops: {} ({})", h.kind, h.why.trim()),
            "until": utc_minute(h.until),
        });
    }
    match &e.global_pause {
        Some(p) => json!({
            "runs": "paused",
            "why": "The owner paused every work: none takes a step and no new work starts until the owner starts them \
                    again, but the works the owner started, which go on",
            "started": p.runs,
        }),
        None => json!({ "runs": "running" }),
    }
}

/// The machines' slots, which every project shares: how many there are, used and free, how many items are ready, by
/// project, and whether the engine runs works now ([`engine_state`]).
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
    Ok(json!({
        "engine": engine_state(e),
        "machines": machines,
        "total": total,
        "used": held,
        "free": total - held,
        "ready": ready,
    }))
}

/// A project's queue, by matter: each matter that can be picked, with its kind and detail, the item it concerns, who
/// waits for it and since when, and what it is; those held for the owner's answer, listed as held; and whether new
/// starts are held to set things straight.
fn queue(e: &Engine, w: &Work, project: &str) -> Result<Value, String> {
    let conn = e.db.conn();
    let ids = |rows: &[Value]| -> BTreeSet<i64> { rows.iter().filter_map(|r| r["matter"].as_i64()).collect() };
    let all = queued(conn, project, w.id)?;
    let taken = ids(&all);
    let can = ids(&straight_first(conn, project, all)?);
    let rows = query_json(
        conn,
        "SELECT * FROM architect_queue WHERE project = ?1 ORDER BY matter",
        &[&project],
        ROWS,
    )?;
    let mut matters = Map::new();
    for r in rows {
        let id = r["matter"].as_i64().unwrap_or_default();
        // Flow feedback and what was said in the groups that came after the work started, and that it did not queue
        // itself, wait for the architect's next work.
        let later = !taken.contains(&id);
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
            m["not_now"] = json!("it came after this work started: the architect's next work takes it");
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
/// handled it; those held for the owner's answer; its item changes, those collected so far each whole with its key,
/// which the review makes once it finds them sound, and those it made or dropped; and whether a hold to set things
/// straight was released in it. Each matter is by its number, so that a session given only what changed is given the
/// matters new to it.
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
    // The batch's item changes, numbered as its review numbers them: those collected so far, each whole, with the key
    // of an item it creates, by which a later change names it; once the review made or dropped them, each with what
    // came of it.
    let changes: Map<String, Value> = query_json(
        conn,
        "SELECT matter, state, change, outcome FROM architect_item_changes
         WHERE work = ?1 AND round > ?2 AND state IN ('collected', 'made', 'dropped') ORDER BY id",
        &[&w.id, &from],
        ROWS,
    )?
    .into_iter()
    .enumerate()
    .map(|(i, mut r)| {
        if let Some(o) = r.as_object_mut() {
            o.retain(|_, v| !v.is_null());
        }
        ((i + 1).to_string(), r)
    })
    .collect();
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
    Ok(json!({
        "handled_matters": numbers,
        "handled": by_matter(handled),
        "held_for_owner": by_matter(asked),
        "item_changes": changes,
        "straight_hold_released": released,
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
    // The images the owner sent with the words it holds, or with the answer it holds.
    let mut images = attachments::of(conn, Words::Matter(m)).map_err(sql)?;
    if let Some(a) = matter["body"]["owner_ask"].as_i64() {
        images.extend(attachments::of(conn, Words::Ask(a)).map_err(sql)?);
    }
    if !images.is_empty() {
        matter["images"] = image_addresses(&images);
    }
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
        // The side reports handed in before the architect ran, which a migration put in one matter: each once, by its
        // work and cell, instead of the list as it was handed in, round after round.
        "report" if matter["body"]["side_reports"].is_array() => {
            let reports = items(matter["body"].get("side_reports"));
            if let Some(b) = matter["body"].as_object_mut() {
                b.remove("side_reports");
            }
            material.insert("side_reports".into(), side_reports_once(e, &reports)?);
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

/// Side reports, each as a list of them holds it (`{ work, round, cell, report }`), by work and cell in the order they
/// came: each text once, trimmed and its spaces collapsed, with its kind, whether it was urgent, how many times it was
/// handed in and in which rounds, and how the work it came from stands now (it goes on, it merged, it is done, it
/// ended and why, or its item was cancelled), which tells whether a report can still matter. A report handed in as the
/// JSON text of a list holds that list; one that holds no text holds nothing and is left out.
fn side_reports_once(e: &Engine, reports: &[Value]) -> Result<Value, String> {
    let conn = e.db.conn();
    let end = engineer_graph(e).map(|g| g.end.clone()).unwrap_or_default();
    let mut groups: Vec<Value> = Vec::new();
    for r in reports {
        let said: Vec<Value> = match &r["report"] {
            Value::String(t) => match serde_json::from_str::<Value>(t) {
                Ok(Value::Array(xs)) => xs,
                Ok(o @ Value::Object(_)) => vec![o],
                _ => vec![json!({ "what": t })],
            },
            Value::Array(xs) => xs.clone(),
            other => vec![other.clone()],
        };
        for one in said {
            let what = text(&one["what"]).split_whitespace().collect::<Vec<_>>().join(" ");
            if what.is_empty() {
                continue;
            }
            let at = |g: &Value| g["work"] == r["work"] && g["cell"] == r["cell"];
            let i = match groups.iter().position(at) {
                Some(i) => i,
                None => {
                    groups.push(json!({ "work": r["work"], "cell": r["cell"], "reports": [] }));
                    groups.len() - 1
                }
            };
            let Some(list) = groups[i]["reports"].as_array_mut() else {
                continue;
            };
            let (round, urgent) = (r["round"].clone(), one["urgent"] == true);
            match list.iter_mut().find(|x| x["what"] == what.as_str()) {
                Some(x) => {
                    x["times"] = json!(x["times"].as_i64().unwrap_or(1) + 1);
                    if let Some(rounds) = x["rounds"].as_array_mut() {
                        rounds.push(round);
                    }
                    if urgent {
                        x["urgent"] = json!(true);
                    }
                }
                None => list.push(json!({
                    "kind": one["kind"], "urgent": urgent, "what": what, "times": 1, "rounds": [round],
                })),
            }
        }
    }
    for g in &mut groups {
        let Some(work) = g["work"].as_i64() else { continue };
        let row: Option<(String, Option<String>, Option<String>, bool)> = conn
            .query_row(
                &format!(
                    "SELECT w.at, w.ended, i.cancelled,
                       EXISTS (SELECT 1 FROM rounds r WHERE r.work = w.id AND r.cell IN {} AND r.transition = 'merged')
                     FROM works w LEFT JOIN items i ON i.id = w.item WHERE w.id = ?1",
                    db::cells_sql(ENGINEER, &["M2"])
                ),
                [work],
                |x| Ok((x.get(0)?, x.get(1)?, x.get(2)?, x.get(3)?)),
            )
            .optional()
            .map_err(sql)?;
        g["work_now"] = json!(match row {
            None => "no such work".to_string(),
            Some((_, _, Some(_), _)) => "its item was cancelled".to_string(),
            Some((_, Some(why), None, _)) => format!("it ended: {why}"),
            Some((_, None, None, true)) => "it merged".to_string(),
            Some((at, None, None, false)) if at == end => "it is done".to_string(),
            Some(_) => "it goes on".to_string(),
        });
    }
    Ok(Value::Array(groups))
}

/// An item's number given as a number or as a text of one, added to a set.
fn ids_of(v: &Value, ids: &mut BTreeSet<i64>) {
    match v {
        Value::Number(n) => ids.extend(n.as_i64()),
        Value::String(s) => ids.extend(s.trim().trim_start_matches('#').parse::<i64>().ok()),
        _ => {}
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

/// The items and works the matter picked concerns, each in full, as the fields of the matter and of its material name
/// them: the item it concerns; in its material, and in the place to set straight or the cancelled item it holds, an
/// item, items, and the item of a work or of works. A number in a text, its words' or a step's, names no item: by
/// "matter #2" or "item 1 to 5" there, it concerns none.
fn matter_items(e: &Engine, w: &Work) -> Result<Option<Value>, String> {
    let conn = e.db.conn();
    let Some(m) = picked(conn, w.id)? else {
        return Ok(None);
    };
    let row = query_json(conn, "SELECT item, body FROM architect_matters WHERE id = ?1", &[&m], 1)?;
    let Some(r) = row.first() else { return Ok(None) };
    let mut ids = BTreeSet::new();
    ids_of(&r["item"], &mut ids);
    let body = &r["body"];
    for part in [body, &body["fix"], &body["cancelled"]] {
        ids_of(&part["item"], &mut ids);
        for i in items(part.get("items")) {
            ids_of(&i, &mut ids);
        }
        let mut works = BTreeSet::new();
        ids_of(&part["work"], &mut works);
        for x in items(part.get("works")) {
            ids_of(&x, &mut works);
        }
        for work in works {
            let item: Option<i64> = conn
                .query_row("SELECT item FROM works WHERE id = ?1", [work], |r| {
                    r.get::<_, Option<i64>>(0)
                })
                .optional()
                .map_err(sql)?
                .flatten();
            ids.extend(item);
        }
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
    let images = match row.first().and_then(|r| r["ask"].as_i64()) {
        Some(a) => attachments::of(conn, Words::Ask(a)).map_err(sql)?,
        None => Vec::new(),
    };
    Ok(row.into_iter().next().map(|r| {
        let mut back = json!({
            "ask": r["ask"],
            "asked": r["body"]["asked"],
            "answer": r["answer"]["text"],
            "answered_at": r["answer"]["at"].as_i64().map(utc_minute),
        });
        if !images.is_empty() {
            back["images"] = image_addresses(&images);
        }
        if let (Some(q), Some(o)) = (r["body"].get("question"), back.as_object_mut()) {
            o.remove("asked");
            o.insert("question".into(), q.clone());
        }
        back
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

/// The owner's words behind the batch's changes: each matter of the owner's words a change collected came from, by
/// its number, with the words whole and where they were said; none when no change came from the owner's words. An
/// input of its own, so that a review given them is only referred to them at its next look, while they stay the same.
fn words_behind_changes(e: &Engine, w: &Work) -> Result<Option<Value>, String> {
    let conn = e.db.conn();
    let matters: BTreeSet<i64> = collected(conn, w.id)?.iter().map(|r| r.2).collect();
    let mut out = Map::new();
    for m in matters {
        let row = query_json(
            conn,
            "SELECT body, at FROM architect_matters WHERE id = ?1 AND kind = 'owner_words'",
            &[&m],
            1,
        )?;
        let Some(r) = row.first() else { continue };
        let b = &r["body"];
        let mut said = json!({ "words": b["text"], "where": b["where"], "at": r["at"].as_i64().map(utc_minute) });
        if !b["work"].is_null() {
            said["work"] = b["work"].clone();
        }
        out.insert(format!("#{m}"), said);
    }
    Ok((!out.is_empty()).then_some(Value::Object(out)))
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

    // A place to set straight the queue holds already is the same place of the same kind, at the same place in words
    // however spaced and cased, or about the same items; another kind or other items is another place.
    #[test]
    fn a_place_to_set_straight_is_the_same_by_its_place_or_its_items() {
        let queued = json!({ "kind": "order", "where": "The parser  before the lexer", "items": [4, 5] });
        let again = json!({ "kind": "order", "where": "the parser before the lexer", "items": [] });
        assert!(same_place(&queued, &again));
        assert!(same_place(
            &queued,
            &json!({ "kind": "order", "where": "Elsewhere", "items": [5, 4] })
        ));
        assert!(!same_place(
            &queued,
            &json!({ "kind": "split", "where": "The parser before the lexer" })
        ));
        assert!(!same_place(
            &queued,
            &json!({ "kind": "order", "where": "Elsewhere", "items": [4] })
        ));
        assert!(!same_place(
            &json!({ "kind": "missing", "where": "a", "items": [] }),
            &json!({ "kind": "missing", "where": "b", "items": [] })
        ));
    }

    // The commits since the architect's last work are the latest 60, each by its message's first sentence, a long one
    // cut at a word to 120 characters, then how many earlier ones there are.
    #[test]
    fn the_commits_since_the_last_work_are_the_latest_sixty_by_their_first_sentence() {
        let long = format!("Make the {}parser faster. Then measure it", "very ".repeat(40));
        let log = |n: usize| -> String {
            (0..n)
                .map(|i| {
                    let message = if i == 0 {
                        long.as_str()
                    } else {
                        "Fix the lexer. It dropped a token"
                    };
                    format!("{i:07x} 2026-10-{:02} {message}\n", 29 - i / 10)
                })
                .collect()
        };
        let v = commits(&log(75), "2026-10-23 11:35 UTC");
        let rows: Vec<&str> = v.as_array().unwrap().iter().map(|r| r.as_str().unwrap()).collect();
        assert_eq!(rows.len(), 61, "{v}");
        assert_eq!(
            rows[0],
            format!("0000000 2026-10-29 Make the {}very…", "very ".repeat(21))
        );
        assert!(rows[0].chars().count() - "0000000 2026-10-29 ".len() <= COMMIT_MOST);
        assert_eq!(rows[1], "0000001 2026-10-29 Fix the lexer.");
        assert_eq!(rows[59], "000003b 2026-10-24 Fix the lexer.");
        assert_eq!(rows[60], "and 15 earlier commits since 2026-10-23 11:35 UTC");
        let v = commits(&log(61), "2026-10-23 11:35 UTC");
        assert_eq!(v[60], "and 1 earlier commit since 2026-10-23 11:35 UTC");
        assert_eq!(commits(&log(60), "x").as_array().unwrap().len(), 60);
    }

    #[test]
    fn a_row_names_an_article_by_its_number_or_its_title() {
        let a = "Article 1. Independent of any project; developed by itself";
        assert!(names_article("Article 1", a));
        assert!(names_article("independent of any project; developed by itself", a));
        assert!(!names_article("Article 10", a));
        assert!(!names_article("Article 2", a));
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

    // The review's rows sort the batch into what is made and what is left out, what it drops and what it finds to fix;
    // a change naming an item a dropped change was to create is dropped with it, a fold of observations into it among
    // them.
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
        let (sound, dropped) = sorted_by_review(rows.clone(), &review);
        assert_eq!(sound.iter().map(|r| r.0).collect::<Vec<_>>(), [3]);
        assert_eq!(dropped.iter().map(|r| r.0.0).collect::<Vec<_>>(), [1, 2, 4]);
        // A change the review drops is left out as one it finds to fix is, and what names its item goes with it,
        // dropped too.
        let mut review = review;
        review[0] = json!({ "change": 1, "verdict": "drop", "why": "Item 5 does it already." });
        let (sound, dropped) = sorted_by_review(rows, &review);
        assert_eq!(sound.iter().map(|r| r.0).collect::<Vec<_>>(), [3]);
        let verdicts: Vec<(i64, &str)> = dropped
            .iter()
            .map(|r| (r.0.0, r.1["verdict"].as_str().unwrap()))
            .collect();
        assert_eq!(verdicts, [(1, "drop"), (2, "drop"), (4, "drop")]);
    }

    /// A database with a work of project p's architect, its rounds 1 to 4, and matter 1, an engineer's question the
    /// work handled at round 2 whose reply, naming the items the batch was to create as `cover` and `split`, is held
    /// for the batch; and the changes of the batch, each with its matter and state.
    fn held_for(changes: &[(i64, Value, &str, Value)]) -> db::Db {
        let mut db = db::Db::in_memory().unwrap();
        db.write(Actor::Engine, |tx| {
            tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
            tx.execute(
                "INSERT INTO works (flow_version, graph, at, project) VALUES (1, 'architect', 'B2', 'p')",
                [],
            )?;
            for n in 1..=4 {
                tx.execute(
                    "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES (1, ?1, 'H3', '{}', 'c', ?1)",
                    [n],
                )?;
            }
            let waits = json!([{ "key": "cover", "title": "c", "delivers": "d" }, { "key": "#12", "title": "x" }]);
            let reply = json!({ "text": "Wait for the new items.", "children": [{ "key": "split", "title": "s" }],
                                "waits": waits });
            for m in [1, 2] {
                tx.execute(
                    "INSERT INTO architect_matters (project, kind, what, body, at, handled_work, handled_round)
                     VALUES ('p', 'engineer_question', 'Asks', '{}', 1, 1, ?1)",
                    [m + 1],
                )?;
            }
            tx.execute(
                "UPDATE architect_matters SET reply_held = ?1 WHERE id = 1",
                [reply.to_string()],
            )?;
            for (matter, change, state, outcome) in changes {
                tx.execute(
                    "INSERT INTO architect_item_changes (work, round, matter, change, state, outcome)
                     VALUES (1, 2, ?1, ?2, ?3, ?4)",
                    params![matter, change.to_string(), state, (!outcome.is_null()).then(|| outcome.to_string())],
                )?;
            }
            Ok(())
        })
        .unwrap();
        db
    }

    fn create(key: &str) -> Value {
        json!({ "kind": "create_item", "key": key, "title": key })
    }

    /// Matter 1 as it stands: whether it is handled, its reply sent and held, and the reply not sent.
    fn matter_one(db: &db::Db) -> (Option<i64>, Option<String>, Option<String>, Value) {
        db.conn()
            .query_row(
                "SELECT handled_round, reply, reply_held, json_extract(body, '$.reply_not_sent') FROM architect_matters
                 WHERE id = 1",
                [],
                |r| {
                    let not_sent: Option<String> = r.get(3)?;
                    Ok((
                        r.get(0)?,
                        r.get(1)?,
                        r.get(2)?,
                        not_sent.map_or(Value::Null, |s| serde_json::from_str(&s).unwrap()),
                    ))
                },
            )
            .unwrap()
    }

    // A held reply whose named items were all made goes out with their numbers; one naming an item the review dropped,
    // or the revision left out, does not: its matter goes back to the queue, not handled, with the reply, the items
    // made by their numbers and why the other was not.
    #[test]
    fn a_held_reply_goes_out_only_when_every_item_it_names_was_made() {
        let release = |db: &mut db::Db, keys: &[(&str, i64)]| {
            let keys: BTreeMap<String, i64> = keys.iter().map(|(k, n)| (k.to_string(), *n)).collect();
            db.write(Actor::Engine, |tx| {
                release_held_replies(tx, 1, &keys, &BTreeSet::new()).map_err(db::Error::Abort)
            })
            .unwrap();
        };
        let mut made = held_for(&[
            (1, create("cover"), "made", Value::Null),
            (1, create("split"), "made", Value::Null),
        ]);
        release(&mut made, &[("cover", 41), ("split", 42)]);
        let (handled, reply, held, _) = matter_one(&made);
        assert_eq!((handled, held), (Some(2), None));
        let reply: Value = serde_json::from_str(&reply.unwrap()).unwrap();
        assert_eq!(
            (&reply["waits"][0]["key"], &reply["children"][0]["key"]),
            (&json!("41"), &json!("42"))
        );

        let finding = json!({ "change": 1, "verdict": "drop", "why": "Item 5 covers it already." });
        let mut dropped = held_for(&[
            (1, create("cover"), "dropped", finding.clone()),
            (1, create("split"), "made", Value::Null),
        ]);
        release(&mut dropped, &[("split", 42)]);
        let (handled, reply, held, not_sent) = matter_one(&dropped);
        assert_eq!((handled, reply, held), (None, None, None));
        assert_eq!(not_sent["made"], json!({ "split": 42 }));
        assert_eq!(not_sent["not_made"], json!([{ "key": "cover", "why": finding }]));
        assert_eq!(not_sent["reply"]["waits"][0]["key"], "cover");

        let mut revised = held_for(&[
            (1, create("cover"), "revised", Value::Null),
            (1, create("split"), "made", Value::Null),
        ]);
        release(&mut revised, &[("split", 42)]);
        let (handled, _, _, not_sent) = matter_one(&revised);
        assert_eq!(handled, None);
        assert!(
            not_sent["not_made"][0]["why"]["what"]
                .as_str()
                .unwrap()
                .contains("revision"),
            "{not_sent}"
        );
    }

    // A cancel of an item the batch creates, by another matter, cancels its create out, and the reply held that names
    // it does not go out: its matter goes back to the queue with why, and the rest of its answer's changes go with it.
    #[test]
    fn a_held_reply_naming_an_item_cancelled_out_goes_back_to_the_queue() {
        let mut db = held_for(&[
            (1, create("cover"), "collected", Value::Null),
            (1, create("split"), "collected", Value::Null),
            (
                1,
                json!({ "kind": "move", "item": 3, "to": { "place": "first" } }),
                "collected",
                Value::Null,
            ),
            (
                2,
                json!({ "kind": "cancel", "item": "cover", "reason": "Not needed." }),
                "collected",
                Value::Null,
            ),
            (
                2,
                json!({ "kind": "move", "item": 4, "to": { "place": "last" } }),
                "collected",
                Value::Null,
            ),
        ]);
        db.write(Actor::Engine, |tx| cancel_within_batch(tx, 1).map_err(db::Error::Abort))
            .unwrap();
        let states: Vec<String> = {
            let mut st = db
                .conn()
                .prepare("SELECT state FROM architect_item_changes ORDER BY id")
                .unwrap();
            st.query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect()
        };
        assert_eq!(states, ["dropped", "dropped", "dropped", "dropped", "collected"]);
        let (handled, reply, held, not_sent) = matter_one(&db);
        assert_eq!((handled, reply, held), (None, None, None));
        let why = |k: usize| {
            not_sent["not_made"][k]["why"]["what"]
                .as_str()
                .unwrap_or_default()
                .to_string()
        };
        assert_eq!(not_sent["not_made"][1]["key"], "cover", "{not_sent}");
        assert!(why(1).contains("cancelled in the same batch"), "{not_sent}");
        assert!(why(0).contains("answered again"), "{not_sent}");
    }
}
