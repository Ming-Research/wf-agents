//! The architects' side of the engine. Each project has its architect; a work of the architect is a work of its graph
//! for a project, on no item: the engine starts one when none of the project runs and a matter is due, one at a time
//! for each project, and such a work holds no slot. What puts a matter in a project's queue is here and in the actions:
//! an engineer's question or failure, a side report, a report's what to do next, the owner's words and answers, an
//! engineer's answer to the architect, another project's architect's message, slots free with fewer items of the
//! project ready, main's CI red, and a place the plan found to set straight. Here too are the steps, inputs, values,
//! checks and actions of the architect's graph, its asks of the owner, and what a matter's session hands in: made as
//! its round ends once the engine's checks pass, or, for a change that can lose something, once the second eye finds it
//! sound. The base, the order the matters are taken in and the facts that call for a new base are in [`crate::base`].
//!
//! The engine knows a few cells of the architect's graph by their names, as the views over its rounds do: the look at
//! the queue (`Q0`), which says the matter taken; the writing of a new policy (`P2`) and where it takes effect (`P12`),
//! which the router tells apart by the owner's reply; the second eye (`G2`); and the first cell of each kind's path
//! ([`KINDS`]).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use db::attachments::{self, Words};
use db::rusqlite::{Connection, OptionalExtension, Transaction, params};
use db::{Actor, HoldReason, Matter, queue_matter};
use flow::format::Src;
use flow::{Compiled, CompiledCell, CompiledGraph};
use serde_json::{Map, Value, json};
use unicode_segmentation::UnicodeSegmentation;

use crate::actions::{self, Ctx};
use crate::checks::{self, Problems};
use crate::decisions;
use crate::inputs::{self, Gathered, Origin};
use crate::steps::{finish, what_is_asked};
use crate::work::{self, Ended, Item, Round, RoundId, Work, json_col, query_json};
use crate::{ARCHITECT, Advance, ENGINEER, Engine, git, values};

/// An hour, in milliseconds.
pub const HOUR: i64 = 3_600_000;

/// The engine's inputs the architect's cells read, each computed by [`input`].
pub const INPUTS: &[&str] = &[
    "constitution",
    "decisions",
    "guide",
    "cited_nodes",
    "since_last_base",
    "board",
    "numbers",
    "observations_overview",
    "observation_tags",
    "levels",
    "policy",
    "policy_signals",
    "slots",
    "matter",
    "matter_items",
    "item_log",
    "similar_items",
    "owner_answer",
    "cross_project",
    "told_owner",
    "owner_talk",
    "questions",
    "notebook_project_now",
    "notebook_plan",
    "notebook_unclear",
    "notebook_other",
    "decision_log",
    "since_base",
    "base_due",
    "planning_why",
    "gathered",
    "changes_to_check",
    "handed_in",
];

/// The engine's inputs that read the project's files on its main, each as one commit of it has them. A round whose cell
/// has any of them is pinned to one commit of main (`inputs::pin_main`; for a cell on a project's architect's line, the
/// commit its worktree is brought to), which every one of them is read at, after one fetch of main.
pub const MAIN_INPUTS: &[&str] = &["constitution", "decisions", "guide", "cited_nodes"];

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

/// Each kind of matter with the first cell of its path.
pub const KINDS: [(&str, &str); 9] = [
    ("set_straight", "H1"),
    ("engineer_question", "H2"),
    (crate::open_questions::KIND, "H2"),
    ("report", "H4"),
    ("failure", "H7"),
    ("owner_words", "H8"),
    ("other_architect", "H9"),
    (crate::main_watch::MAIN_RED, "L1"),
    (CHECK, "P1"),
];

/// The cells of a matter's session that hand in what a second eye may check, and that the second eye sends it back to.
const HANDING_IN: [&str; 9] = ["H1", "H3", "H5", "H7", "H8", "H9", "L2", "P1", "P2"];

/// The second eye's cell, whose problems send the matter back to the cell that handed its changes in.
const SECOND_EYE: &str = "G2";

/// The check of whether the policy in force still holds, taken with what the plan queued. One is open at a time: one
/// asked for again takes the newer asker's why.
const CHECK: &str = "check_policy";
const PLAN: &str = "make_plan";

/// The cell that writes a new policy, whose draft waits for the owner's approval, and the cell where it takes effect once
/// the owner replied to the question of its approval with exactly this word.
const SET_POLICY: &str = "P2";
const TAKES_EFFECT: &str = "P12";
const APPROVED: &str = "approved";

/// The engine's short question to the owner whether the draft of a new policy is approved.
const APPROVAL: &str = "Approve this policy? Reply exactly: approved";

/// How many times an observation no item took in yet may be reported before it is work: a merge that makes it
/// reported this many times or more folds it into an item of the batch, which a change of the batch moves ahead.
const RECURS: i64 = 3;

/// The cells the engine knows by their names: the look at the queue, the pick, the understanding, the rethink after a
/// batch, and the review of the batch.
const LOOK: &str = "Q0";

/// The cell of the owner's words, which a matter back with the owner's reply goes to when the flow no longer has the
/// cell that asked.
const TALK: &str = "H8";

/// The value the rows of the cell a project's architect waits at to start (`S0`) switch on: whether a matter is due.
const DUE: &str = "due";

/// The step of the architect's end for a work stopped before its end (its graph's `cancel`), which ends a work the flow
/// version the engine runs cannot take on ([`stop_work`]).
pub const STOP: &str = "stop_architect_work";

/// The cell of the engineer's graph a work is done at, as the views read it.
const ENGINEER_END: &str = "E1";

/// The cell of the engineer's graph that writes the design, which the look at a decision card reads it from.
const DESIGN: &str = "D2";

/// The most rows an input of the architect's gives of one list.
const ROWS: usize = 200;

/// The most entries [`since_last_work`] gives of each of its lists, the latest; how many earlier ones there are follows
/// them.
const LATEST: usize = 60;

/// The most characters [`since_last_work`] gives a commit's message by: its first sentence, cut at a word.
const COMMIT_MOST: usize = 120;

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

/// The addresses of the markdown links in a text, each whole: what stands between `](` and the next `)`.
fn markdown_link_targets(s: &str) -> Vec<&str> {
    s.split("](")
        .skip(1)
        .filter_map(|rest| rest.find(')').map(|end| rest[..end].trim()))
        .collect()
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
/// main, whatever was changed there dropped, which fetches main and gives the commit the worktree stands at, the one
/// the round reads the project at; or the empty directory of a cell on no line made afresh, which fetches nothing and
/// gives none.
pub fn ready(e: &Engine, w: &Work, g: &CompiledGraph, cell: &CompiledCell) -> Result<Option<String>, String> {
    let project = project_of(w)?;
    let dir = workdir(&e.data, g, cell, project);
    if cell.line.is_none() {
        if dir.exists() {
            std::fs::remove_dir_all(&dir).map_err(|x| format!("{}: {x}", dir.display()))?;
        }
        return std::fs::create_dir_all(&dir)
            .map(|_| None)
            .map_err(|x| format!("{}: {x}", dir.display()));
    }
    refresh(e, &g.name, project).map(|(_, commit)| Some(commit))
}

/// Brings the worktree of a project's architect for the works of a graph to the latest main, making it when it is
/// not there, with one fetch of main; gives where it is and the commit of main it stands at.
pub fn refresh(e: &Engine, graph: &str, project: &str) -> Result<(PathBuf, String), String> {
    let p = e
        .config
        .config
        .projects
        .get(project)
        .ok_or_else(|| format!("no project {project} in the configuration"))?;
    let dir = worktree(&e.data, graph, project);
    let commit = sessions::worktree::at_main(&p.checkout, &dir, &p.main).map_err(|x| {
        format!(
            "the architect's worktree {} could not be brought to main: {x}",
            dir.display()
        )
    })?;
    Ok((dir, commit))
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

/// Whether a matter is due for a project's architect, which starts a work of it ([`crate::base::is_due`]).
pub fn queue_has(e: &Engine, project: &str) -> Result<bool, String> {
    crate::base::is_due(e, project)
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
    let v = e.flow_now;
    let due = waits_to_start(e, project)?;
    let flow = e.flow(v)?;
    let g = flow.graph(ARCHITECT).ok_or("the flow has no architect's graph")?;
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
/// to start than slots free and no hold on its new starts, a matter of slots free, with how many slots are free and the
/// items ready by number, when none waits in its queue and the last was handled longer ago than the configuration's
/// wait (`agents.slots_free_wait`) or never, and those facts are not the ones the last held: the wait keeps one that
/// found nothing to start from coming back at once, and the same free slots and ready items would only bring the
/// architect to the same end. Nothing when the flow new works start on does not name the action, nor while the owner
/// has paused every work, for any project, the ones whose architects the owner started and the ones the owner steps
/// by hand included.
///
/// Any action the engine adds that queues a matter or starts a work for a slot that is free must be stopped by the
/// owner's pause of every work and by a hold on the project's new starts as this one is: neither leaves a new work
/// to start, so what it brings the architect for can only end in starting nothing.
pub fn queue_due(e: &mut Engine) -> Result<(), String> {
    // No new work can start while every work is paused, so a matter of free slots would only bring the architect to
    // find nothing to start.
    if e.global_pause().is_some() {
        return Ok(());
    }
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
        let ready_items: Vec<i64> = {
            let mut st = conn
                .prepare("SELECT item FROM ready_items WHERE project = ?1 ORDER BY item")
                .map_err(sql)?;
            st.query_map([&p], |r| r.get(0))
                .map_err(sql)?
                .collect::<Result<_, _>>()
                .map_err(sql)?
        };
        let ready = i64::try_from(ready_items.len()).unwrap_or(i64::MAX);
        let held = count(conn, "SELECT count(*) FROM dispatch_holds WHERE project = ?1", &[&p])?;
        let waiting = count(
            conn,
            "SELECT count(*) FROM architect_matters WHERE project = ?1 AND kind = 'slots_free' AND handled_round IS NULL",
            &[&p],
        )?;
        if ready >= free || held > 0 || waiting > 0 {
            continue;
        }
        let body = json!({ "free": free, "ready": ready_items });
        // The last handled: when its round ended, and the facts it held.
        let last: Option<(i64, String)> = conn
            .query_row(
                "SELECT r.ended_at, m.body FROM architect_matters m
                 JOIN rounds r ON r.work = m.handled_work AND r.n = m.handled_round
                 WHERE m.project = ?1 AND m.kind = 'slots_free' AND r.ended_at IS NOT NULL
                 ORDER BY r.ended_at DESC LIMIT 1",
                [&p],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(sql)?;
        if let Some((at, held_then)) = last {
            let same = serde_json::from_str::<Value>(&held_then).is_ok_and(|b| b == body);
            if same || db::now() - at <= wait {
                continue;
            }
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
            body,
            ..Matter::default()
        };
        e.db.write(Actor::Engine, |tx| queue_matter(tx, &m)).map_err(sql)?;
    }
    Ok(())
}

/// The matter the latest look at the queue in a work of the architect took before round `before`, or before none (cell
/// `Q0`, its `next.matter`); a pick of a flow version before (`Q1`, its `pick.matter`, or `Q0`'s `queue.matter`) is
/// read as it was. None when the latest look took no matter: a planning, a rebuild of the base, or nothing.
pub fn picked_before(conn: &Connection, work: i64, before: Option<i64>) -> Result<Option<i64>, String> {
    let row: Option<(String, Option<String>)> = conn
        .query_row(
            "SELECT cell, output FROM rounds r WHERE work = ?1 AND n < ?2 AND transition IS NOT NULL
               AND (cell = ?3 OR (cell = 'Q1' AND json_extract(output, '$.outcome.pick.matter') IS NOT NULL))
             ORDER BY n DESC LIMIT 1",
            params![work, before.unwrap_or(i64::MAX), LOOK],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(sql)?;
    let Some((cell, output)) = row else { return Ok(None) };
    let o = json_col(output).unwrap_or(Value::Null);
    Ok(if cell == LOOK {
        o["next"]["matter"].as_i64().or_else(|| {
            o["queue"]["matter"]
                .as_i64()
                .filter(|_| o["queue"]["verdict"] != "listens")
        })
    } else {
        o["outcome"]["pick"]["matter"].as_i64()
    })
}

/// The matter an architect work's latest pick chose.
pub fn picked(conn: &Connection, work: i64) -> Result<Option<i64>, String> {
    picked_before(conn, work, None)
}

/// The item changes a matter's session handed in that wait for the second eye, in order: each change's row, the round
/// that handed it in and the change.
fn held_changes(conn: &Connection, matter: i64) -> Result<Vec<(i64, i64, Value)>, String> {
    let rows = query_json(
        conn,
        "SELECT id, round, change FROM architect_item_changes WHERE matter = ?1 AND state = 'collected' ORDER BY id",
        &[&matter],
        100_000,
    )?;
    Ok(rows
        .iter()
        .map(|r| {
            (
                r["id"].as_i64().unwrap_or_default(),
                r["round"].as_i64().unwrap_or_default(),
                r["change"].clone(),
            )
        })
        .collect())
}

/// The items a change that waits for the second eye names, on which the engine starts no work until it is made or sent
/// back: the item it changes, the item a split splits or another is moved under, and the items a merge keeps and
/// absorbs.
pub fn held_by_checks(conn: &Connection) -> Result<BTreeSet<i64>, String> {
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

/// Whether a work runs on an item: an engineer's work, not stopped and not at the end of its graph.
pub fn works_on(conn: &Connection, item: i64) -> Result<bool, String> {
    Ok(running_work(conn, item)?.is_some())
}

/// Whether a project's new starts are held to set it straight.
fn held_straight(conn: &Connection, project: &str) -> Result<bool, String> {
    Ok(count(
        conn,
        "SELECT count(*) FROM dispatch_holds WHERE project = ?1 AND reason = 'set_straight'",
        &[&project],
    )? > 0)
}

/// A project's holds on new starts as its board gives them, oldest first: each one's reason, since when and why as
/// whoever held said it, or none when nothing holds the project's new starts.
fn starts_held(conn: &Connection, project: &str) -> Result<Vec<Value>, String> {
    let rows = query_json(
        conn,
        "SELECT reason, since, why FROM dispatch_holds WHERE project = ?1 ORDER BY since, reason",
        &[&project],
        ROWS,
    )?;
    Ok(rows
        .into_iter()
        .map(|r| {
            let mut hold = json!({ "reason": r["reason"], "since": r["since"].as_i64().map(utc_minute) });
            if let Some(why) = r["why"].as_str() {
                hold["why"] = json!(why);
            }
            hold
        })
        .collect())
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

/// Cell `Q0`: what comes next for the project's architect, in the engine's fixed order ([`crate::base::next`]), with
/// the facts that call for a new base as it found them.
pub fn look_at_queue(e: &mut Engine, w: &Work, _cell: &CompiledCell) -> Result<Value, String> {
    crate::base::next(e, project_of(w)?)
}

/// The owner's answer to the ask of the owner that held a matter; none while it has none.
fn answer_of(conn: &Connection, ask: i64) -> Result<Option<Value>, String> {
    let answer: Option<Option<String>> = conn
        .query_row("SELECT answer FROM owner_asks WHERE id = ?1", [ask], |r| r.get(0))
        .optional()
        .map_err(sql)?;
    Ok(json_col(answer.flatten()))
}

/// Where the matter picked goes ([`route_of`]).
fn route(e: &Engine, w: &Work, g: &CompiledGraph) -> Result<String, String> {
    route_of(e.db.conn(), w.id, g)
}

/// Where the matter the architect's work `work` took goes: back to the cell that handed its changes in, when the second
/// eye sent them back; back to the cell that asked about it, when it comes back with the owner's answer, or to the cell
/// where a new policy takes effect when that cell is the one that writes it and the answer is exactly approved; on at
/// the second cell a flow version before sent it back to the queue between; otherwise the first cell of its kind's
/// path. A cell the flow no longer has is not gone back to, and a matter back with the answer whose asking cell the flow
/// no longer has goes to the cell of the owner's words.
fn route_of(conn: &Connection, work: i64, g: &CompiledGraph) -> Result<String, String> {
    let m = picked(conn, work)?.ok_or_else(|| format!("work {work}: no matter is in hand"))?;
    if let Some(cell) = sent_back_to(conn, work)?.filter(|c| g.cell(c).is_some()) {
        return Ok(cell);
    }
    let (kind, asked, resume, goes_on): (String, Option<i64>, Option<String>, Option<String>) = conn
        .query_row(
            "SELECT kind, asked, resume_cell, goes_on_at FROM architect_matters WHERE id = ?1",
            [m],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
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
    if let Some(cell) = goes_on.filter(|c| g.cell(c).is_some()) {
        return Ok(cell);
    }
    KINDS
        .iter()
        .find(|k| k.0 == kind)
        .map(|k| k.1.to_string())
        .ok_or_else(|| format!("matter {m} is of a kind the architect's graph has no path for: {kind}"))
}

/// The cell of a matter's session the second eye sent its changes back to, in the architect's work `work`: when the
/// latest round of the second eye since the matter was taken found problems, and no cell handed changes in since, the
/// latest cell before it that handed them in. None otherwise.
fn sent_back_to(conn: &Connection, work: i64) -> Result<Option<String>, String> {
    let from: i64 = conn
        .query_row(
            "SELECT coalesce(max(n), 0) FROM rounds WHERE work = ?1 AND cell = ?2 AND transition IS NOT NULL",
            params![work, LOOK],
            |r| r.get(0),
        )
        .map_err(sql)?;
    let eye: Option<(i64, Option<String>)> = conn
        .query_row(
            "SELECT n, transition FROM rounds WHERE work = ?1 AND n > ?2 AND cell = ?3 AND transition IS NOT NULL
             ORDER BY n DESC LIMIT 1",
            params![work, from, SECOND_EYE],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(sql)?;
    let Some((n, Some(took))) = eye else { return Ok(None) };
    if took != "problems" {
        return Ok(None);
    }
    let cells = serde_json::to_string(&HANDING_IN).map_err(sql)?;
    let later: i64 = conn
        .query_row(
            "SELECT count(*) FROM rounds WHERE work = ?1 AND n > ?2 AND cell IN (SELECT value FROM json_each(?3))",
            params![work, n, cells],
            |r| r.get(0),
        )
        .map_err(sql)?;
    if later > 0 {
        return Ok(None);
    }
    conn.query_row(
        "SELECT cell FROM rounds WHERE work = ?1 AND n < ?2 AND n > ?3 AND cell IN (SELECT value FROM json_each(?4))
         ORDER BY n DESC LIMIT 1",
        params![work, n, from, cells],
        |r| r.get(0),
    )
    .optional()
    .map_err(sql)
}

/// That the owner answered an ask of kind question that holds the matter `m.id`: the owner's words on it, which the
/// answer to an open question begins with.
const RULED_BY_OWNER: &str = "EXISTS (SELECT 1 FROM owner_asks a, json_each(a.body, '$.holds') h
     WHERE a.answer IS NOT NULL AND json_extract(a.body, '$.kind') = 'question' AND h.value = m.id)";

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
        "route" => route(e, w, g)?,
        // Whether what the matter's session handed in waits for the second eye.
        "second_eye" => {
            let held = match picked(conn, w.id)? {
                Some(m) => conn
                    .query_row(
                        "SELECT effects_held IS NOT NULL FROM architect_matters WHERE id = ?1",
                        [m],
                        |r| r.get::<_, bool>(0),
                    )
                    .map_err(sql)?,
                None => false,
            };
            if held { "needed" } else { "none" }.into()
        }
        // What the base is read anew for, which the look at what comes next records as it sends the work there; a work
        // that came to reading the project otherwise, as one brought to this flow version at the reading of a version
        // before, reads it for a planning, which builds the base whatever the plan's verdict.
        "rebuild_for" => "plan".into(),
        // A flow version from before asks lost their kinds weighed a question by its kind: the kind its matter was
        // queued with, when that version has a case of it, else another question. No question is queued with a kind
        // now.
        "question" => {
            let d = picked_detail(conn, w.id)?.unwrap_or_default();
            let known = g
                .cells
                .iter()
                .any(|c| c.inenums.get("question").is_some_and(|vs| vs.iter().any(|v| v.0 == d)));
            if known { d } else { "other_question".into() }
        }
        "report" => match picked_detail(conn, w.id)?.as_deref() {
            Some("followup") => "followup",
            Some("answer") => "answer",
            Some(db::FAULT) => db::FAULT,
            _ => "side_report",
        }
        .into(),
        _ => return Ok(None),
    }))
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

/// The most characters [`owner_talk`] gives of the talk, the latest whole entries.
const TALK_MOST: usize = 60_000;

/// What was said in a project's one-to-one with the owner since its previous base began (its first round,
/// [`crate::base::previous_start`]), for a new base, which remembers nothing of it but what the notebook holds: every entry
/// the page shows in the conversation (`conversation_shown`) made at or after that moment, oldest first and whole, each
/// with when, who said it to whom and the words; an ask of the architect's with the owner's answer, counted as made when
/// it was answered if that was at or after it; an entry with images only says how many came with it. Before the
/// project's first session there is none to cut at: there is no lower bound. Over [`TALK_MOST`] characters the latest
/// whole entries are given, oldest first (the very latest always), then one line saying how many earlier ones there are.
/// None when nothing was said since.
fn owner_talk(e: &Engine, project: &str) -> Result<Option<Value>, String> {
    let conn = e.db.conn();
    let since = crate::base::previous_start(conn, project)?;
    let after = since.unwrap_or(i64::MIN);
    let rows = query_json(
        conn,
        "SELECT c.at, c.sender, c.recipient, c.text,
                CASE c.source
                  WHEN 'words' THEN (SELECT count(*) FROM attachments x WHERE x.matter = c.id)
                  WHEN 'interjection' THEN (SELECT count(*) FROM attachments x WHERE x.interjection = c.id)
                  WHEN 'ask' THEN (SELECT count(*) FROM attachments x WHERE x.ask = c.id)
                  ELSE 0 END AS images,
                json_extract(a.answer, '$.text') AS answer, json_extract(a.answer, '$.at') AS answered_at,
                json_extract(a.answer, '$.by') AS answered_by
         FROM conversation_shown c LEFT JOIN owner_asks a ON c.source = 'ask' AND a.id = c.id
         WHERE c.conversation = 'architect:' || ?1
           AND max(c.at, coalesce(json_extract(a.answer, '$.at'), c.at)) >= ?2
         ORDER BY max(c.at, coalesce(json_extract(a.answer, '$.at'), c.at)), c.source, c.id",
        &[&project, &after],
        usize::MAX,
    )?;
    let entries: Vec<(usize, Value)> = rows
        .iter()
        .map(|r| {
            let images = |n: i64| match n {
                0 => None,
                1 => Some("1 image attached".to_string()),
                n => Some(format!("{n} images attached")),
            };
            let n = r["images"].as_i64().unwrap_or(0);
            let text = r["text"].as_str().unwrap_or_default();
            let mut x = json!({
                "at": r["at"].as_i64().map(utc_minute),
                "from": r["sender"],
                "to": r["recipient"],
                "text": text,
            });
            let mut size = text.chars().count();
            let answer = r["answer"].as_str();
            let answered = r["answered_at"].as_i64();
            if answer.is_some() || answered.is_some() {
                let said = answer.unwrap_or_default();
                size += said.chars().count();
                let mut a = json!({ "at": answered.map(utc_minute), "text": said });
                if let Some(by) = r["answered_by"].as_str().filter(|b| *b != "owner") {
                    a["by"] = json!(by);
                }
                if let Some(i) = images(n) {
                    a["images"] = json!(i);
                }
                x["answer"] = a;
            } else if let Some(i) = images(n) {
                x["images"] = json!(i);
            }
            (size, x)
        })
        .collect();
    if entries.is_empty() {
        return Ok(None);
    }
    // The latest whole entries that fit, the very latest always.
    let mut kept = 0;
    let mut used = 0;
    for (size, _) in entries.iter().rev() {
        if kept > 0 && used + size > TALK_MOST {
            break;
        }
        used += size;
        kept += 1;
    }
    let earlier = entries.len() - kept;
    let mut said: Vec<Value> = entries.into_iter().skip(earlier).map(|(_, x)| x).collect();
    if earlier > 0 {
        let what = if earlier == 1 { "entry" } else { "entries" };
        said.push(json!(format!(
            "and {earlier} earlier {what} before these: read them with the history search (X5)"
        )));
    }
    let mut out = json!({ "said": said });
    if let Some(t) = since {
        out["since"] = json!(utc_minute(t));
    }
    Ok(Some(out))
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
        let gathered = inputs::gather(e, w, g, cell, None, None)?;
        let given = values::given(e, w, g, cell, None, &gathered)?;
        let images = attachments::of(e.db.conn(), Words::Ask(id)).map_err(sql)?;
        let output = answered_as(&answer, &images);
        let row = g.take(&cell.id, &output, &given)?.clone();
        return finish(e, w, g, cell, r.id, output, &row, Ended::default(), &[], None, &given);
    }
    let gathered = inputs::gather(e, w, g, cell, None, None)?;
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
                    "UPDATE architect_matters SET asked = ?2, resume_cell = ?3, goes_on_at = NULL WHERE id = ?1",
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

/// The cells of the engineer's graph that put a question of their own kind to the architect, each with its kind (the
/// matter's detail): the review of a design at the project's level, and its split along its parts.
pub const DESIGN_QUESTIONS: [(&str, &str); 1] = [("D3", "design_review")];

/// The engineer's merge queue, whose third failure in a row comes to the architect.
const MERGE_CELL: &str = "M2";

/// The kind of matter a question to the architect is, with its detail: when a cell asked, or the engine reports a flow
/// anomaly (the way on the engineer chose did not fit), an engineer's question, with none, since an ask carries no kind
/// and the architect reads what it is about from what it says; for an engine tool that failed, or main's environment
/// that changed, a failure, with which. The engineer's graph puts two questions of its own kind to the architect at
/// cells of their own ([`DESIGN_QUESTIONS`]): the review of a design at the project's level, and its split along its
/// parts; and the third failure in a row of the merge queue is a failure of the merge.
fn question_kind(cell: &CompiledCell, gathered: &[Gathered]) -> (&'static str, Option<String>) {
    fn path_of(p: &inputs::Part) -> String {
        match &p.origin {
            Origin::Place { path, .. } => path.clone(),
            _ => String::new(),
        }
    }
    if let Some((_, detail)) = DESIGN_QUESTIONS.iter().find(|(c, _)| *c == cell.id) {
        return ("engineer_question", Some((*detail).to_string()));
    }
    let parts: Vec<&inputs::Part> = gathered.iter().flat_map(|x| &x.parts).collect();
    if parts.iter().any(|p| {
        matches!(&p.origin, Origin::Place { cell, path, .. } if cell == MERGE_CELL
            && (path.starts_with("result[conflict]") || path.starts_with("result[red]")))
            && !empty(&p.value)
    }) {
        return ("failure", Some("merge_failed".into()));
    }
    if parts
        .iter()
        .any(|p| matches!(path_of(p).as_str(), "ask" | "next") && !empty(&p.value))
    {
        return ("engineer_question", None);
    }
    if parts
        .iter()
        .any(|p| path_of(p).contains("environment_changed") && !empty(&p.value))
    {
        return ("failure", Some("environment_changed".into()));
    }
    if parts
        .iter()
        .any(|p| !matches!(path_of(p).as_str(), "" | "ask" | "progress") && !empty(&p.value))
    {
        return ("failure", Some("tool_failed".into()));
    }
    ("engineer_question", None)
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
        let gathered = inputs::gather(e, w, g, cell, Some(item), None)?;
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
            return Ok(crate::steps::waits_for_slot(w));
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
    let gathered = inputs::gather(e, w, g, cell, Some(item), None)?;
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
        "tool_failure" => question_kind(cell, &gathered),
        _ => match question_kind(cell, &gathered) {
            ("failure", _) => ("engineer_question", None),
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
    let about = match (what_is_asked(&asked), &detail) {
        (a, Some(d)) if a.is_empty() => format!(" ({})", d.replace('_', " ")),
        (a, _) => a,
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
        None if detail.as_deref() == Some("design_review") => format!(
            "Work {} on item {} ({}){of}: its design waits for your review at the project's level, before the owner's",
            w.id, item.id, item.title
        ),
        None if detail.as_deref() == Some("split_design") => format!(
            "Work {} on item {} ({}){of}: the owner had its design split along its parts, for you to split",
            w.id, item.id, item.title
        ),
        // A flow anomaly asks nothing in words: the blocker says what the engine found.
        None if about.is_empty() => b.what.clone(),
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
                detail: detail.clone(),
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
    e.say(&format!(
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

/// Holds a project's new starts for a reason of the architect's, its own or setting the project straight.
fn hold(tx: &Transaction<'_>, project: &str, reason: HoldReason, why: Option<&str>) -> Result<(), String> {
    db::hold_starts(tx, project, reason, why).map_err(sql)
}

/// Releases a hold of a reason of the architect's. The owner's hold is not the architect's to release, and no action
/// of its names it.
fn release(tx: &Transaction<'_>, project: &str, reason: HoldReason) -> Result<(), String> {
    db::release_starts(tx, project, reason).map_err(sql)?;
    Ok(())
}

/// Releases the hold to set a project straight once no matter that sets it straight is left and no change waits for
/// the second eye.
fn settle_hold(tx: &Transaction<'_>, project: &str) -> Result<(), String> {
    if !held_straight(tx, project)? {
        return Ok(());
    }
    let left = count(
        tx,
        "SELECT count(*) FROM architect_matters WHERE project = ?1 AND kind = 'set_straight' AND handled_round IS NULL
           AND withdrawn IS NULL",
        &[&project],
    )?;
    let waiting = count(
        tx,
        "SELECT count(*) FROM architect_item_changes c JOIN architect_matters m ON m.id = c.matter
         WHERE m.project = ?1 AND c.state = 'collected'",
        &[&project],
    )?;
    if left == 0 && waiting == 0 {
        release(tx, project, HoldReason::SetStraight)?;
    }
    Ok(())
}

/// Carries out one action of an architect's effects, at once.
fn act_now(ctx: &Ctx, tx: &Transaction<'_>, project: &str, a: &Value) -> Result<(), String> {
    let reason = text(&a["reason"]).trim();
    match text(&a["kind"]) {
        "pause_work" => pause_item(tx, a["item"].as_i64().unwrap_or_default(), reason),
        "resume_work" => {
            let item = a["item"].as_i64().unwrap_or_default();
            let Some((work, Some(p), by)) = running_work(tx, item)? else {
                return Ok(());
            };
            // The loop lifts it, once the work's machine has a slot free: this transaction cannot know the slots.
            if db::stopped_by_architect(Some(p.as_str()), by.as_deref()) {
                db::ask_resume(tx, work, db::Actor::Architect).map_err(sql)?;
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
        "hold_starts" => hold(tx, project, HoldReason::Architect, Some(reason)),
        // Only its own: the owner's hold stays whatever the architect decides.
        "release_starts" => release(tx, project, HoldReason::Architect),
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
        // The merge that made main red is reverted by a PR the engine opens and merges first, ahead of the queue.
        "revert_merge" => {
            let p = ctx
                .projects
                .get(project)
                .ok_or_else(|| format!("no project {project} in the configuration"))?;
            actions::outbox(
                tx,
                Some(ctx.work.id),
                "revert_merge",
                &json!({
                    "repo": p.repo,
                    "main": p.main,
                    "checkout": p.checkout,
                    "merge_commit": text(&a["merge_commit"]).trim(),
                    "worktrees": config::worktrees(&ctx.data),
                }),
            )
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
pub(crate) fn observe(tx: &Transaction<'_>, project: &str, from: &str, o: &Value) -> Result<(), String> {
    match text(&o["kind"]) {
        "new" => {
            let source = format!("{} ({from})", text(&o["source"]).trim());
            let title = Some(text(&o["title"]).trim()).filter(|t| !t.is_empty());
            tx.execute(
                "INSERT INTO observations (project, title, what, tags, source, refs) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    project,
                    title,
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

/// The body of a message to another project's architect, which says the matter it comes of.
fn message_body(project: &str, m: &Value, matter: i64, work: i64) -> Value {
    json!({ "from": project, "text": m["text"], "matter": matter, "work": work })
}

/// Carries out the effects of an answer on a matter's path, which the round's checks found sound: made at once, each
/// logged, unless an item change of them is of a kind the second eye checks ([`crate::base::second_eye_checks`]); then
/// everything the round handed in waits for the second eye ([`hold_for_second_eye`]).
fn apply_effects(ctx: &Ctx, tx: &Transaction<'_>, effects: &Value) -> Result<(), String> {
    let (w, project) = architect_work_of(ctx)?;
    let matter = matter_of(tx, w)?;
    let mut checked = false;
    for c in items(effects.get("item_changes")) {
        checked |= crate::base::second_eye_checks(tx, &c)?;
    }
    if checked {
        return hold_for_second_eye(ctx, tx, matter, effects);
    }
    let changes = items(effects.get("item_changes"));
    make_changes(ctx, tx, matter, &changes)?;
    make_the_rest(ctx, tx, &project, matter, effects)
}

/// Keeps what a matter's session handed in for the second eye: each item change, as one that waits for it, and the
/// round's effects and whole answer, which take effect, its reply going with them, once the second eye finds them
/// sound ([`apply_checked`]), and are dropped when it sends them back.
fn hold_for_second_eye(ctx: &Ctx, tx: &Transaction<'_>, matter: i64, effects: &Value) -> Result<(), String> {
    for c in items(effects.get("item_changes")) {
        tx.execute(
            "INSERT INTO architect_item_changes (work, round, matter, change, state) VALUES (?1, ?2, ?3, ?4, 'collected')",
            params![ctx.round.work, ctx.round.n, matter, c.to_string()],
        )
        .map_err(sql)?;
    }
    let held = json!({
        "round": ctx.round.to_string(),
        "cell": ctx.cell.id,
        "effects": effects,
        "outcome": ctx.unwrapped,
    });
    tx.execute(
        "UPDATE architect_matters SET effects_held = ?2 WHERE id = ?1",
        params![matter, held.to_string()],
    )
    .map_err(sql)?;
    Ok(())
}

/// Whether what a matter's session handed in waits for the second eye.
fn held_for_check(tx: &Transaction<'_>, matter: i64) -> Result<bool, String> {
    tx.query_row(
        "SELECT effects_held IS NOT NULL FROM architect_matters WHERE id = ?1",
        [matter],
        |r| r.get(0),
    )
    .map_err(sql)
}

/// The changes to observations, actions and messages of a matter's effects, made at once, each logged under the
/// matter's name.
fn make_the_rest(ctx: &Ctx, tx: &Transaction<'_>, project: &str, matter: i64, effects: &Value) -> Result<(), String> {
    for o in items(effects.get("observation_changes")) {
        observe(tx, project, &format!("matter {matter}"), &o)?;
    }
    for a in items(effects.get("actions")) {
        act_now(ctx, tx, project, &a)?;
    }
    for m in items(effects.get("messages")) {
        let body = message_body(project, &m, matter, ctx.work.id);
        message(tx, project, text(&m["project"]), text(&m["text"]), "message", body)?;
    }
    Ok(())
}

/// Makes a matter's item changes, as the architect, in order, each so that a change the state the earlier ones left
/// refuses leaves nothing of itself, and logs each as made or dropped, with what it made or why; a change to an item a
/// work runs on that says why the work must change is said to that work, which decides where to go back to.
fn make_changes(ctx: &Ctx, tx: &Transaction<'_>, matter: i64, changes: &[Value]) -> Result<(), String> {
    if changes.is_empty() {
        return Ok(());
    }
    let written = crate::arch_changes::write_all(tx, changes, &ctx.projects, ctx.round)?;
    let created: BTreeMap<String, i64> = written
        .created
        .iter()
        .filter_map(|c| Some((c["key"].as_str()?.to_string(), c["id"].as_str()?.parse().ok()?)))
        .collect();
    let refused: BTreeMap<usize, &String> = written.refused.iter().map(|(i, why)| (*i, why)).collect();
    for (i, c) in changes.iter().enumerate() {
        let (state, outcome) = match refused.get(&i) {
            Some(why) => (
                "dropped",
                Some(json!({ "what": format!("It could not be made: {why}") })),
            ),
            None => (
                "made",
                (c["kind"] == "create_item")
                    .then(|| created.get(text(&c["key"]).trim()).map(|n| json!({ "created": n })))
                    .flatten(),
            ),
        };
        tx.execute(
            "INSERT INTO architect_item_changes (work, round, matter, change, state, outcome)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                ctx.round.work,
                ctx.round.n,
                matter,
                c.to_string(),
                state,
                outcome.map(|o| o.to_string())
            ],
        )
        .map_err(sql)?;
        if state == "made" {
            say_unavoidable(tx, c)?;
        }
    }
    Ok(())
}

/// Says a change made to an item a work runs on, which says why the work must change, to that work's engineer: the
/// work decides where to go back to, keeping what it did.
fn say_unavoidable(tx: &Transaction<'_>, c: &Value) -> Result<(), String> {
    let why = text(&c["unavoidable"]).trim();
    if why.is_empty() {
        return Ok(());
    }
    let Some(item) = item_number(&c["item"]) else {
        return Ok(());
    };
    let Some((work, _, _)) = running_work(tx, item)? else {
        return Ok(());
    };
    let what = match text(&c["kind"]) {
        "edit_acceptance" => format!("its acceptance is now: {}", texts(&c["acceptance"]).join("; ")),
        _ => format!("its {} is now: {}", text(&c["part"]), text(&c["text"]).trim()),
    };
    let said = format!(
        "The architect changed this work's item #{item}: {what}. Why the work must change: {why}. Decide where in the \
         flow to go back to, keeping what you did."
    );
    tx.execute(
        "INSERT INTO interjections (work, line, speaker, text) VALUES (?1, 'engineer', 'architect', ?2)",
        params![work, said],
    )
    .map_err(sql)?;
    Ok(())
}

/// The second eye found what the matter's session handed in sound: its item changes are made in place of the rows
/// that waited, then its changes to observations, actions and messages; its reply goes, with the numbers of the items
/// made, and the work that reported is told what became of it.
fn apply_checked(ctx: &Ctx, tx: &Transaction<'_>) -> Result<(), String> {
    let (w, project) = architect_work_of(ctx)?;
    let matter = matter_of(tx, w)?;
    let held: Option<String> = tx
        .query_row(
            "SELECT effects_held FROM architect_matters WHERE id = ?1",
            [matter],
            |r| r.get(0),
        )
        .map_err(sql)?;
    let Some(held) = json_col(held) else {
        return Ok(());
    };
    let rows = held_changes(tx, matter)?;
    let changes: Vec<Value> = rows.iter().map(|r| r.2.clone()).collect();
    if !changes.is_empty() {
        let written = crate::arch_changes::write_all(tx, &changes, &ctx.projects, ctx.round)?;
        let created: BTreeMap<String, i64> = written
            .created
            .iter()
            .filter_map(|c| Some((c["key"].as_str()?.to_string(), c["id"].as_str()?.parse().ok()?)))
            .collect();
        let refused: BTreeMap<usize, &String> = written.refused.iter().map(|(i, why)| (*i, why)).collect();
        for (i, (id, _, c)) in rows.iter().enumerate() {
            let (state, outcome) = match refused.get(&i) {
                Some(why) => (
                    "dropped",
                    Some(json!({ "what": format!("It could not be made: {why}") })),
                ),
                None => (
                    "made",
                    (c["kind"] == "create_item")
                        .then(|| created.get(text(&c["key"]).trim()).map(|n| json!({ "created": n })))
                        .flatten(),
                ),
            };
            tx.execute(
                "UPDATE architect_item_changes SET state = ?2, outcome = ?3 WHERE id = ?1",
                params![id, state, outcome.map(|o| o.to_string())],
            )
            .map_err(sql)?;
            if state == "made" {
                say_unavoidable(tx, c)?;
            }
        }
    }
    make_the_rest(ctx, tx, &project, matter, &held["effects"])?;
    tx.execute(
        "UPDATE architect_matters SET effects_held = NULL WHERE id = ?1",
        [matter],
    )
    .map_err(sql)?;
    let outcome = &held["outcome"];
    let round = RoundId::parse(text(&held["round"])).unwrap_or(ctx.round);
    match text(&held["cell"]) {
        TALK => reply_to_owner_now(tx, round, w, &project, matter, text(&outcome["answer"]["reply"]))?,
        "H9" => answer_architect_now(tx, w, &project, matter, text(&outcome["answer"]["reply"]))?,
        _ => {}
    }
    if outcome["reply"].is_object() {
        send_reply(tx, w, matter, &outcome["reply"])?;
    }
    answer(tx, matter, outcome)
}

/// The second eye sent what the matter's session handed in back: none of it takes effect, its reply does not go, and
/// each change that waited is marked revised with what the second eye found; the session hands it in again.
fn return_checked(ctx: &Ctx, tx: &Transaction<'_>) -> Result<(), String> {
    let (w, _) = architect_work_of(ctx)?;
    let matter = matter_of(tx, w)?;
    let found = ctx.unwrapped["check"].to_string();
    tx.execute(
        "UPDATE architect_item_changes SET state = 'revised', outcome = ?2 WHERE matter = ?1 AND state = 'collected'",
        params![matter, found],
    )
    .map_err(sql)?;
    tx.execute(
        "UPDATE architect_matters SET effects_held = NULL, reply_held = NULL WHERE id = ?1",
        [matter],
    )
    .map_err(sql)?;
    Ok(())
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

/// The items a matter's changes created, each by the key its change gave it, with its number.
fn keys_made(conn: &Connection, matter: i64) -> Result<BTreeMap<String, i64>, String> {
    let rows = query_json(
        conn,
        "SELECT change, outcome FROM architect_item_changes
         WHERE matter = ?1 AND state = 'made' AND json_extract(change, '$.kind') = 'create_item' ORDER BY id",
        &[&matter],
        10_000,
    )?;
    Ok(rows
        .iter()
        .filter_map(|r| {
            Some((
                text(&r["change"]["key"]).trim().to_string(),
                r["outcome"]["created"].as_i64()?,
            ))
        })
        .collect())
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

/// Gives the reply to the side waiting at its question, with the numbers of the items the matter's changes made in
/// place of their keys: an open question's answer goes to its design, any other reply to its matter, for the work that
/// asked to take back. A reply that names an item no change made does not go out ([`not_sent`]).
fn send_reply(tx: &Transaction<'_>, work: i64, matter: i64, reply: &Value) -> Result<(), String> {
    let keys = keys_made(tx, matter)?;
    if keys_named(reply).iter().any(|k| !keys.contains_key(k)) {
        return not_sent(tx, work, matter, reply, &keys);
    }
    let reply = numbered(reply, &keys);
    if crate::open_questions::is_one(tx, matter)? {
        return crate::open_questions::answer(tx, matter, &reply);
    }
    tx.execute(
        "UPDATE architect_matters SET reply = ?2, reply_held = NULL, body = json_remove(body, '$.reply_not_sent')
         WHERE id = ?1",
        params![matter, reply.to_string()],
    )
    .map_err(sql)?;
    // A reply to a blocker its work no longer waits on is recorded with it and moves nothing.
    if let Some(b) = crate::blockers::of_matter(tx, matter)?
        && b.ended.is_some()
    {
        crate::blockers::say(tx, b.id, "architect", "late", &reply)?;
    }
    // The review of a design whose work went on goes to the owner now.
    crate::review::after_reply(tx, matter)
}

/// Puts the reply to the owner's words, as the architect's message, in the conversation they were said in.
fn reply_to_owner_now(
    tx: &Transaction<'_>,
    round: RoundId,
    work: i64,
    project: &str,
    matter: i64,
    said: &str,
) -> Result<(), String> {
    let said = said.trim();
    if said.is_empty() {
        return Ok(());
    }
    let card: Option<i64> = tx
        .query_row(
            "SELECT json_extract(body, '$.design_card') FROM architect_matters WHERE id = ?1",
            [matter],
            |r| r.get(0),
        )
        .map_err(sql)?;
    tx.execute(
        "INSERT INTO owner_messages (work, round, text, design_card) VALUES (?1, ?2, ?3, ?4)",
        params![round.work, round.n, said, card],
    )
    .map_err(sql)?;
    let logged = format!("The architect of {project} replies: {said}");
    actions::outbox(tx, Some(work), "notify", &json!({ "text": logged }))
}

/// Sends the reply to the architect whose message the matter was, as a message in its queue.
fn answer_architect_now(
    tx: &Transaction<'_>,
    work: i64,
    project: &str,
    matter: i64,
    words: &str,
) -> Result<(), String> {
    if words.trim().is_empty() {
        return Ok(());
    }
    let body: String = tx
        .query_row("SELECT body FROM architect_matters WHERE id = ?1", [matter], |r| {
            r.get(0)
        })
        .map_err(sql)?;
    let body: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
    let Some(to) = body["from"].as_str() else {
        return Ok(());
    };
    let reply = json!({ "from": project, "text": words, "answers": matter, "work": work });
    message(tx, project, to, words, "reply", reply)?;
    Ok(())
}

/// Why the item a matter's change was to create under a key was not made, as the record of that change says: why it
/// could not be made, what the second eye found when it sent it back, or that no change of the matter was to create
/// it.
fn why_not_made(tx: &Transaction<'_>, _work: i64, matter: i64, key: &str) -> Result<Value, String> {
    let row = query_json(
        tx,
        "SELECT state, outcome FROM architect_item_changes
         WHERE matter = ?1 AND json_extract(change, '$.kind') = 'create_item' AND trim(json_extract(change, '$.key')) = ?2
         ORDER BY id DESC LIMIT 1",
        &[&matter, &key],
        1,
    )?;
    Ok(match row.first().map(|r| (r["state"].as_str(), &r["outcome"])) {
        Some((Some("dropped" | "revised"), outcome)) if !outcome.is_null() => outcome.clone(),
        _ => json!({ "what": "No change of the matter made it" }),
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
        "UPDATE architect_matters SET handled_work = NULL, handled_round = NULL, reply_held = NULL, effects_held = NULL,
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

// Telling the work that handed a matter in what became of it.

/// What the line to a work about a question calls it.
const QUESTION: &str = "question";

/// The work told what became of a matter it handed in: the work, what the line calls the matter, the cell it came from
/// and what it said.
struct Reporter {
    work: i64,
    called: &'static str,
    cell: String,
    said: String,
}

/// The work a matter came from that is told what became of it: for a side report, and for an engineer's question; none
/// for any other matter (what to do next after a report, an answer, flow feedback, a fault, the side reports a
/// migration put in one matter), nor when the work no longer runs, since nothing can be said to a work that ended or is
/// at its end.
fn reporter(tx: &Transaction<'_>, matter: i64) -> Result<Option<Reporter>, String> {
    let (kind, detail, work, body): (String, Option<String>, Option<i64>, String) = tx
        .query_row(
            "SELECT kind, detail, work, body FROM architect_matters WHERE id = ?1",
            [matter],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .map_err(sql)?;
    let Some(work) = work else { return Ok(None) };
    let body: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
    let (called, cell, said) = match (kind.as_str(), detail.as_deref()) {
        ("report", Some("followup" | "answer" | "flow_feedback" | db::FAULT)) => return Ok(None),
        ("report", _) if body["report"].is_object() => (
            "side report",
            text(&body["cell"]).to_string(),
            text(&body["report"]["what"]).to_string(),
        ),
        ("engineer_question", _) => {
            let cell = crate::blockers::of_matter(tx, matter)?
                .map(|b| b.cell)
                .unwrap_or_default();
            let what: String = tx
                .query_row("SELECT what FROM architect_matters WHERE id = ?1", [matter], |r| {
                    r.get(0)
                })
                .map_err(sql)?;
            // A flow anomaly asks nothing in words, and is said as the matter says it.
            let said = items(body.get("asked"))
                .iter()
                .find_map(|a| a["value"]["points"][0]["what"].as_str().map(str::to_string))
                .unwrap_or(what);
            (QUESTION, cell, said)
        }
        _ => return Ok(None),
    };
    let runs = count(
        tx,
        &format!(
            "SELECT count(*) FROM works WHERE id = ?1 AND graph = '{ENGINEER}' AND ended IS NULL AND at NOT IN {}",
            db::cells_sql(ENGINEER, &[ENGINEER_END])
        ),
        &[&work],
    )? > 0;
    Ok(runs.then_some(Reporter {
        work,
        called,
        cell,
        said,
    }))
}

/// An item's number, given as a number or as a text of one.
fn item_number(v: &Value) -> Option<i64> {
    v.as_i64()
        .or_else(|| v.as_str().and_then(|s| s.trim().trim_start_matches('#').parse().ok()))
}

/// What became of a matter, each part of the line that says it: the observations recorded from it and those its round
/// reported once more, each with its number, title and how many times it was reported now; the items the changes it
/// made created, and those they changed, folded observations into or moved, each with its number and title, but those
/// in `told` already. `outcome` is what its round handed in, its effects among it.
fn became(tx: &Transaction<'_>, matter: i64, outcome: &Value, told: BTreeSet<i64>) -> Result<Vec<String>, String> {
    let mut parts = Vec::new();
    let observation = |id: i64| -> Result<(String, i64), String> {
        tx.query_row(
            "SELECT coalesce(title, what), times_reported FROM observations WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|x| format!("observation {id}: {x}"))
    };
    let recorded = query_json(
        tx,
        "SELECT id FROM observations WHERE source LIKE ?1 ORDER BY id",
        &[&format!("%(matter {matter})")],
        ROWS,
    )?;
    for id in recorded.iter().filter_map(|r| r["id"].as_i64()) {
        let (title, _) = observation(id)?;
        parts.push(format!("recorded as observation O{id} \"{title}\""));
    }
    let merged: BTreeSet<i64> = items(outcome["effects"].get("observation_changes"))
        .iter()
        .filter(|o| o["kind"] == "merge")
        .filter_map(|o| o["into"].as_i64())
        .collect();
    for id in merged {
        let (title, times) = observation(id)?;
        parts.push(format!(
            "reported once more on observation O{id} \"{title}\", {times} times now"
        ));
    }
    let made = query_json(
        tx,
        "SELECT change, outcome FROM architect_item_changes WHERE matter = ?1 AND state = 'made' ORDER BY id",
        &[&matter],
        ROWS,
    )?;
    let created: BTreeMap<String, i64> = made
        .iter()
        .filter(|r| r["change"]["kind"] == "create_item")
        .filter_map(|r| {
            Some((
                text(&r["change"]["key"]).trim().to_string(),
                r["outcome"]["created"].as_i64()?,
            ))
        })
        .collect();
    let mut told = told;
    for r in &made {
        let c = &r["change"];
        let (new, id) = match c["kind"].as_str() {
            Some("create_item") => (true, r["outcome"]["created"].as_i64()),
            Some("fold_observations" | "edit_item" | "edit_acceptance" | "move" | "add_merge_condition") => (
                false,
                item_number(&c["item"]).or_else(|| created.get(text(&c["item"]).trim()).copied()),
            ),
            _ => continue,
        };
        let Some(id) = id.filter(|id| told.insert(*id)) else {
            continue;
        };
        let title: String = tx
            .query_row("SELECT title FROM items WHERE id = ?1", [id], |x| x.get(0))
            .map_err(|x| format!("item {id}: {x}"))?;
        let new = if new { "new " } else { "" };
        parts.push(format!("taken into {new}item #{id} \"{title}\""));
    }
    Ok(parts)
}

/// Why nothing was recorded of a side report, as the round that took it in says: the way it took it in and why.
fn not_recorded(outcome: &Value) -> String {
    let take = &outcome["take"];
    let why = db::without_final_terminal(text(&take["why"]).trim());
    let because = if why.is_empty() {
        String::new()
    } else {
        format!(": {why}")
    };
    match text(&take["way"]) {
        "forwarded" => {
            let to: Vec<String> = items(outcome["effects"].get("messages"))
                .iter()
                .map(|m| text(&m["project"]).to_string())
                .filter(|p| !p.is_empty())
                .collect();
            let to = if to.is_empty() {
                "another project".to_string()
            } else {
                to.join(", ")
            };
            format!("forwarded to the architect of {to}{because}")
        }
        "need_will_hit" => format!("not recorded, as a need that will certainly come meets it anyway{because}"),
        "nothing" => format!("nothing to take in{because}"),
        _ => format!("nothing recorded{because}"),
    }
}

/// Tells the work that handed a matter in what became of it, in one line to its engineer that needs no answer and
/// turns nothing (`informs`), so that it does not report it again: what [`became`] of it; for a side report of which
/// nothing was recorded, why ([`not_recorded`]); for a question, only what its reply does not say, the items it names
/// left out, and nothing when that is all. `outcome` is what the round that handled it handed in.
fn answer(tx: &Transaction<'_>, matter: i64, outcome: &Value) -> Result<(), String> {
    let Some(r) = reporter(tx, matter)? else {
        return Ok(());
    };
    let reply: Option<String> = tx
        .query_row("SELECT reply FROM architect_matters WHERE id = ?1", [matter], |x| {
            x.get(0)
        })
        .map_err(sql)?;
    let reply = json_col(reply).unwrap_or(Value::Null);
    let named: BTreeSet<i64> = ["children", "waits"]
        .iter()
        .flat_map(|list| items(reply.get(*list)))
        .filter_map(|x| item_number(&x["key"]))
        .collect();
    let mut parts = became(tx, matter, outcome, named)?;
    if parts.is_empty() {
        if r.called == QUESTION {
            return Ok(());
        }
        parts.push(not_recorded(outcome));
    }
    let at = if r.cell.is_empty() {
        String::new()
    } else {
        format!(" at cell {}", r.cell)
    };
    let said = db::one_sentence(&r.said);
    let line = format!(
        "About your {}{at} (\"{}\"): {}. You need not report it again.",
        r.called,
        db::without_final_terminal(&said),
        parts.join("; ")
    );
    tx.execute(
        "INSERT INTO interjections (work, line, speaker, text, informs) VALUES (?1, 'engineer', 'architect', ?2, 1)",
        params![r.work, line],
    )
    .map_err(sql)?;
    Ok(())
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

/// Why a check of the policy is asked for, when the cell that asks gives no why of its own.
fn head_why(cell: &str) -> String {
    match cell {
        TAKES_EFFECT => "A new policy took effect".into(),
        "U1" => "Reading the project anew asks for it".into(),
        other => format!("Cell {other} asked for it"),
    }
}

/// Queues a matter of the kind `kind`, the check of the policy (or, in a flow version before, the plan of what to do
/// next), with why the round's cell asks for it: the text at `why`, or the cell's own reason. One of that kind
/// open already, in the queue or parked for the owner's reply, stands instead, and the newer asker's why is added to
/// it. A check keeps the hash of the constitution it was queued under ([`CONSTITUTION`]); one asked for with no why of
/// its own, as a new session's understanding asks for it, is not queued while the policy in force and the constitution
/// are those the last check handled had ([`same_as_last_check`]), since it would only come to the same end.
fn queue_head(ctx: &Ctx, tx: &Transaction<'_>, kind: &str, why: Option<&Value>) -> Result<(), String> {
    let (_, project) = architect_work_of(ctx)?;
    let own = why.and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty());
    let why = own.map_or_else(|| head_why(&ctx.cell.id), str::to_string);
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
    let mut body = json!({ "asked_by": [asker] });
    let what = if kind == CHECK {
        // The constitution as the commit of main the round was pinned to has it, the one its agent was given; a round
        // pinned to none, as one the engine took itself with no agent, reads main as the checkout has it now.
        let pinned: Option<String> = tx
            .query_row(
                "SELECT json_extract(input, '$.engine.main') FROM rounds WHERE work = ?1 AND n = ?2",
                params![ctx.round.work, ctx.round.n],
                |r| r.get(0),
            )
            .optional()
            .map_err(sql)?
            .flatten();
        let constitution =
            constitution_of(&project, ctx.projects.get(&project), pinned.as_deref())?.map(|t| db::content_hash(&t));
        if own.is_none() && same_as_last_check(tx, &project, constitution.as_deref())? {
            return Ok(());
        }
        body[CONSTITUTION] = json!(constitution);
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
        body,
        item: None,
        raised: Some((ctx.round.work, ctx.round.n)),
    };
    queue_matter(tx, &m).map_err(sql)?;
    Ok(())
}

/// The field of the body of a check of the policy that keeps the hash of the project's constitution as the check was
/// queued, null when the project has none, which a later check compares ([`same_as_last_check`]). The matter's
/// material leaves it out: it tells the architect nothing.
const CONSTITUTION: &str = "constitution";

/// Whether the policy in force and the constitution are those the project's last check of the policy handled had: a
/// policy is in force and none took effect after that check was handled (a check that set the policy in force was
/// handled where it took effect), and the constitution's hash, `constitution`, is the one the check kept. False when
/// no check was handled, or the last kept no hash, as one queued before checks kept it.
fn same_as_last_check(tx: &Transaction<'_>, project: &str, constitution: Option<&str>) -> Result<bool, String> {
    let last: Option<(i64, i64, String)> = tx
        .query_row(
            "SELECT handled_work, handled_round, body FROM architect_matters
             WHERE project = ?1 AND kind = ?2 AND handled_round IS NOT NULL
             ORDER BY handled_work DESC, handled_round DESC LIMIT 1",
            params![project, CHECK],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .map_err(sql)?;
    let Some((work, round, body)) = last else {
        return Ok(false);
    };
    let body: Value = serde_json::from_str(&body).unwrap_or_default();
    let Some(kept) = body.get(CONSTITUTION) else {
        return Ok(false);
    };
    if kept.as_str() != constitution {
        return Ok(false);
    }
    let in_force = count(
        tx,
        "SELECT count(*) FROM policy_current WHERE project = ?1",
        &[&project],
    )? > 0;
    let since = count(
        tx,
        &format!(
            "SELECT count(*) FROM rounds a JOIN works w ON w.id = a.work
             WHERE w.graph = 'architect' AND w.project = ?1 AND a.cell IN {} AND a.transition IS NOT NULL
               AND (a.work > ?2 OR (a.work = ?2 AND a.n > ?3))",
            db::cells_sql(ARCHITECT, &[TAKES_EFFECT])
        ),
        &[&project, &work, &round],
    )?;
    Ok(in_force && since == 0)
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
            // One matter for each place, with every step the plan meant to take there; a place the queue holds
            // already, which the fix names, is not queued again.
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
            // approved draft hands it in: something missing, which its path makes an item of.
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
        "hold_to_set_straight" => hold(tx, &architect_work_of(ctx)?.1, HoldReason::SetStraight, None)?,
        "release_straight_hold" => release(tx, &architect_work_of(ctx)?.1, HoldReason::SetStraight)?,
        "queue_check_policy" => queue_head(ctx, tx, CHECK, at)?,
        "ask_approval" => {
            // The engine's short question whether the draft the round wrote is approved, pinned in the owner's
            // one-to-one as the architect's question, which parks the matter in hand at the cell, with its session;
            // without the draft, which is the round's return.
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
                "UPDATE architect_matters SET asked = ?2, resume_cell = ?3, goes_on_at = NULL WHERE id = ?1",
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
            // The reply to the owner's words is the architect's message in the conversation they were said in; once
            // what the round handed in waits for the second eye, the reply goes with it.
            let Some(said) = at.and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()) else {
                return Ok(true);
            };
            let m = matter_of(tx, w)?;
            if held_for_check(tx, m)? {
                return Ok(true);
            }
            let (_, project) = architect_work_of(ctx)?;
            reply_to_owner_now(tx, ctx.round, w, &project, m, said)?;
        }
        "close_matter" => {
            // The matter in hand is handled by this round, and its sessions end with it; a matter whose reply did not
            // go out, since an item it named was not made, goes back to the queue instead, to be answered again.
            let m = matter_of(tx, w)?;
            let returned: bool = tx
                .query_row(
                    "SELECT json_extract(body, '$.reply_not_sent') IS NOT NULL FROM architect_matters WHERE id = ?1",
                    [m],
                    |r| r.get(0),
                )
                .map_err(sql)?;
            if !returned {
                tx.execute(
                    "UPDATE architect_matters SET handled_work = ?2, handled_round = ?3, goes_on_at = NULL
                     WHERE id = ?1 AND handled_round IS NULL",
                    params![m, ctx.round.work, ctx.round.n],
                )
                .map_err(sql)?;
            }
            sessions::choose::end_matter(tx, m)?;
            settle_hold(tx, &architect_work_of(ctx)?.1)?;
        }
        "close_planning" => {
            // What a planning answers: slots free with nothing of the project to start, and a plan a flow version
            // before queued.
            let (_, project) = architect_work_of(ctx)?;
            tx.execute(
                "UPDATE architect_matters SET handled_work = ?2, handled_round = ?3
                 WHERE project = ?1 AND kind IN ('slots_free', ?4) AND handled_round IS NULL AND withdrawn IS NULL",
                params![project, ctx.round.work, ctx.round.n, PLAN],
            )
            .map_err(sql)?;
        }
        "log_matter" => {
            // The matter's line of the decision log and its two answers, as the latest hand-in left them.
            let Some(c) = at.filter(|c| c.is_object()) else {
                return Ok(true);
            };
            let m = matter_of(tx, w)?;
            let mut logged = c.clone();
            logged["round"] = json!(ctx.round.to_string());
            logged["cell"] = json!(ctx.cell.id);
            tx.execute(
                "UPDATE architect_matters SET logged = ?2 WHERE id = ?1",
                params![m, logged.to_string()],
            )
            .map_err(sql)?;
        }
        "apply_effects" => {
            if let Some(effects) = at {
                apply_effects(ctx, tx, effects)?;
            }
        }
        "apply_checked" => apply_checked(ctx, tx)?,
        "return_checked" => return_checked(ctx, tx)?,
        "reply_to_engineer" => {
            let Some(reply) = at.filter(|r| r.is_object()) else {
                return Ok(true);
            };
            let m = matter_of(tx, w)?;
            // A reply whose round's changes wait for the second eye goes once it finds them sound; it is said about
            // its question meanwhile.
            if held_for_check(tx, m)? {
                tx.execute(
                    "UPDATE architect_matters SET reply_held = ?2 WHERE id = ?1",
                    params![m, reply.to_string()],
                )
                .map_err(sql)?;
                if let Some(b) = crate::blockers::of_matter(tx, m)?
                    && b.ended.is_none()
                {
                    crate::blockers::say(tx, b.id, "architect", "held_reply", reply)?;
                }
                return Ok(true);
            }
            send_reply(tx, w, m, reply)?;
        }
        "answer_reporter" => {
            // The work that handed the matter in is told what became of it as the round ends, or, once what it handed
            // in waits for the second eye, when that finds it sound.
            let m = matter_of(tx, w)?;
            if !held_for_check(tx, m)? {
                answer(tx, m, &ctx.unwrapped)?;
            }
        }
        "answer_architect" => {
            let Some(words) = at.and_then(Value::as_str).filter(|s| !s.trim().is_empty()) else {
                return Ok(true);
            };
            let (_, project) = architect_work_of(ctx)?;
            let m = matter_of(tx, w)?;
            if !held_for_check(tx, m)? {
                answer_architect_now(tx, w, &project, m, words)?;
            }
        }
        "redo_matters" | "redo_batch" => {
            if let Some(n) = ctx.output["redoes_from"].as_i64() {
                redo(tx, w, n)?;
            }
        }
        _ => return Ok(false),
    }
    Ok(true)
}

/// `redo_matters`: what an architect's work did from its round `n` on that has not taken effect is undone, as the work
/// goes back to do those rounds again: what waits for the second eye is dropped, its changes marked revised, to be
/// handed in again, and the matters they closed whose reply has not gone out go back to the queue, a reply held
/// dropped, to be written again. What took effect stays; the work never goes back past it ([`in_effect`]).
pub fn redo(tx: &Transaction<'_>, work: i64, n: i64) -> Result<(), String> {
    tx.execute(
        "UPDATE architect_item_changes SET state = 'revised' WHERE work = ?1 AND round >= ?2 AND state = 'collected'",
        params![work, n],
    )
    .map_err(sql)?;
    tx.execute(
        "UPDATE architect_matters SET effects_held = NULL, reply_held = NULL
         WHERE effects_held IS NOT NULL AND id IN (SELECT matter FROM architect_item_changes WHERE work = ?1 AND round >= ?2)",
        params![work, n],
    )
    .map_err(sql)?;
    tx.execute(
        "UPDATE architect_matters SET handled_work = NULL, handled_round = NULL, reply_held = NULL, goes_on_at = NULL
         WHERE handled_work = ?1 AND handled_round >= ?2 AND reply IS NULL AND withdrawn IS NULL",
        params![work, n],
    )
    .map_err(sql)?;
    Ok(())
}

// An architect's work the flow version the engine runs cannot take on.

/// Why an architect's work that flow version `now` cannot take on ends, for the reason `why` its graph's cell for a
/// work on an older flow version gave, as the work's end records it.
pub fn stopped_why(now: i64, why: &str) -> String {
    format!(
        "the flow moved to version {now}, which this architect work cannot go on in, and the architect starts afresh: \
         {why}"
    )
}

/// The step of the architect's end for a work stopped before its end ([`STOP`], its graph's `cancel`), to which the row
/// of its graph's cell for a work on an older flow version brings a work the version the engine runs cannot take on,
/// and which runs at once ([`crate::steps::bring_up`]). In one write its round records why the work ends and what
/// became of the matters it held; those it held and did not finish go back to the queue ([`return_matters`]); and the
/// work ends, saying why ([`crate::ending::end`]), so that the next look starts a new work of the project's architect
/// when a matter is due. Why is what that cell's latest round recorded: the version the work cannot go on in, and why.
/// The matter in hand is closed instead when a round of its path that hands in took effect since it was taken, as the
/// graph of the version those rounds ran on has it ([`in_effect`]): handling it again would repeat what went out.
pub fn stop_work(e: &mut Engine, w: &Work, g: &CompiledGraph, cell: &CompiledCell) -> Result<Advance, String> {
    let version = g
        .version
        .clone()
        .ok_or("the architect's graph has no cell for a work on an older flow version")?;
    let brought = work::rounds_at(e.db.conn(), w.id, &version)?
        .into_iter()
        .find_map(|r| r.output)
        .ok_or_else(|| format!("work {}: no round brought it to cell {version}", w.id))?;
    let from = brought["from"].as_i64().unwrap_or(w.flow_version);
    let why = stopped_why(brought["to"].as_i64().unwrap_or(e.flow_now), text(&brought["why"]));
    let old = e.graph_of(&Work {
        flow_version: from,
        ..w.clone()
    })?;
    let conn = e.db.conn();
    let in_hand = match picked(conn, w.id)? {
        Some(m) => conn
            .query_row(
                "SELECT handled_round IS NULL AND withdrawn IS NULL AND asked IS NULL FROM architect_matters
                 WHERE id = ?1",
                [m],
                |r| r.get::<_, bool>(0),
            )
            .map_err(sql)?
            .then_some(m),
        None => None,
    };
    let taken = taken_at(conn, w.id)?;
    let hands_in = |r: &Round| {
        r.cell
            .as_deref()
            .and_then(|c| db::code_of(ARCHITECT, c))
            .is_some_and(|c| HANDING_IN.contains(&c))
    };
    let closed_by = match in_hand {
        Some(_) => work::rounds(conn, w.id)?
            .iter()
            .filter(|r| r.id.n > taken && hands_in(r) && in_effect(&old, r).is_some())
            .map(|r| r.id.n)
            .max(),
        None => None,
    };
    let project = project_of(w)?.to_string();
    let projects = e.config.config.projects.clone();
    let commit = e.config.commit.clone();
    e.db.write(Actor::Engine, |tx| {
        let round = work::start_round(tx, w.id, &cell.id, None, &json!({}), &commit, None).map_err(db::Error::Abort)?;
        let returned = return_matters(tx, w.id, &project, in_hand, closed_by).map_err(db::Error::Abort)?;
        let output = json!({ "why": why, "returned": returned, "closed": closed_by.and(in_hand) });
        let ended = Ended {
            output: Some(output),
            ..Ended::default()
        };
        work::finish_round(tx, round, &ended).map_err(db::Error::Abort)?;
        crate::ending::end(tx, w.id, crate::ending::End::Stopped(&why), &projects).map_err(db::Error::Abort)
    })
    .map_err(sql)?;
    e.say(&format!("work {} ended: {why}", w.id));
    Ok(Advance::Moved)
}

/// The ids a query gives, each its first column.
fn ids(conn: &Connection, q: &str, args: impl db::rusqlite::Params) -> Result<Vec<i64>, String> {
    let mut st = conn.prepare(q).map_err(sql)?;
    st.query_map(args, |r| r.get(0))
        .map_err(sql)?
        .collect::<Result<_, _>>()
        .map_err(sql)
}

/// The round of a work of the architect in which its latest look at the queue took the matter it has in hand
/// ([`picked`]); 0 before any.
fn taken_at(conn: &Connection, work: i64) -> Result<i64, String> {
    conn.query_row(
        "SELECT coalesce(max(n), 0) FROM rounds WHERE work = ?1 AND transition IS NOT NULL
           AND (cell = ?2 OR (cell = 'Q1' AND json_extract(output, '$.outcome.pick.matter') IS NOT NULL))",
        params![work, LOOK],
        |r| r.get(0),
    )
    .map_err(sql)
}

/// `return_matters`: what an architect's work that ends held and did not finish goes back to its project's queue, not
/// handled, for the next work to take afresh. What its rounds handed in that waits for the second eye is dropped: its
/// item changes are marked revised, which lets go of the items they held ([`held_by_checks`]), and the effects and the
/// reply held are dropped; the matters it closed whose reply has not gone out go back. The matter in hand (`in_hand`,
/// neither handled nor held by an ask of the owner) goes back too, to start its path again, or, when a round of its
/// path that hands in took effect (`closed_by`), is closed as handled by that round. Each matter that goes back or is
/// closed so leaves its sessions; one an ask of the owner holds keeps its own, to be taken up again with the answer. A
/// hold of the project's new starts to set it straight is then released once nothing keeps it ([`settle_hold`]). Gives
/// the matters that went back.
pub fn return_matters(
    tx: &Transaction<'_>,
    work: i64,
    project: &str,
    in_hand: Option<i64>,
    closed_by: Option<i64>,
) -> Result<Vec<i64>, String> {
    let held = ids(
        tx,
        "SELECT id FROM architect_matters
         WHERE withdrawn IS NULL
           AND (handled_work = ?1 OR id IS ?2 OR id IN (SELECT matter FROM architect_item_changes WHERE work = ?1))
           AND (effects_held IS NOT NULL OR reply_held IS NOT NULL
             OR id IN (SELECT matter FROM architect_item_changes WHERE work = ?1 AND state = 'collected'))
         ORDER BY id",
        params![work, in_hand],
    )?;
    let list = json!(held).to_string();
    tx.execute(
        "UPDATE architect_item_changes SET state = 'revised' WHERE work = ?1 AND state = 'collected'",
        [work],
    )
    .map_err(sql)?;
    tx.execute(
        "UPDATE architect_matters SET effects_held = NULL, reply_held = NULL
         WHERE id IN (SELECT value FROM json_each(?1))",
        [&list],
    )
    .map_err(sql)?;
    tx.execute(
        "UPDATE architect_matters SET handled_work = NULL, handled_round = NULL, goes_on_at = NULL
         WHERE id IN (SELECT value FROM json_each(?1)) AND handled_work = ?2 AND reply IS NULL",
        params![list, work],
    )
    .map_err(sql)?;
    let mut back = ids(
        tx,
        "SELECT id FROM architect_matters WHERE id IN (SELECT value FROM json_each(?1)) AND handled_round IS NULL",
        [&list],
    )?;
    let mut left = back.clone();
    if let Some(m) = in_hand {
        match closed_by {
            Some(n) => {
                tx.execute(
                    "UPDATE architect_matters SET handled_work = ?2, handled_round = ?3, goes_on_at = NULL
                     WHERE id = ?1 AND handled_round IS NULL",
                    params![m, work, n],
                )
                .map_err(sql)?;
            }
            None => {
                tx.execute("UPDATE architect_matters SET goes_on_at = NULL WHERE id = ?1", [m])
                    .map_err(sql)?;
                back.push(m);
            }
        }
        left.push(m);
    }
    back.sort_unstable();
    back.dedup();
    left.sort_unstable();
    left.dedup();
    for m in left {
        sessions::choose::end_matter(tx, m)?;
    }
    settle_hold(tx, project)?;
    Ok(back)
}

/// What a round of an architect's work did that took effect as it ended and cannot be undone, in words, by its cell
/// as `g` has it, its return and the row it took: an effect made at once (an item change, an action on a work or on new
/// starts, a change to observations, a message to another project's architect), a word to the owner, a reply that went
/// out to an engineer, to the work that reported or to another architect, new starts held or let go, or what the second
/// eye found sound made. None when doing it again undoes or repeats nothing that went out, as for what waits for the
/// second eye, a matter closed and the notebook.
pub fn in_effect(g: &CompiledGraph, r: &Round) -> Option<String> {
    let cell = g.cell(r.cell.as_deref()?)?;
    let took = r.transition.as_deref()?;
    let output = r.output.as_ref()?;
    let outcome = if cell.kind == "agent" {
        g.unwrap(output)
    } else {
        output.clone()
    };
    let at_once = [
        ("hold_to_set_straight", "held new starts"),
        ("release_straight_hold", "let new starts go"),
        ("apply_checked", "made what the second eye found sound"),
        // A flow version before made a batch of item changes so.
        ("settle_straight_hold", "held or let go new starts"),
        ("apply_batch", "made the batch's item changes"),
        ("apply_reviewed", "made or dropped the batch's item changes"),
    ];
    let row = cell.rows.iter().find(|x| x.name == took);
    if let Some(what) = row
        .into_iter()
        .flat_map(|x| &x.actions)
        .find_map(|d| at_once.iter().find(|a| a.0 == d.name()))
    {
        return Some(what.1.to_string());
    }
    let some = |v: &Value| v.as_array().is_some_and(|a| !a.is_empty());
    let said = |v: &Value| v.as_str().is_some_and(|s| !s.trim().is_empty());
    let does = |name: &str| cell.always.iter().any(|a| a.action.as_deref() == Some(name));
    let effects = &outcome["effects"];
    // What can lose something waited for the second eye, and took no effect as the round ended.
    let checked = cell.rows.iter().any(|x| x.to == "G1");
    let held = checked
        && items(effects.get("item_changes"))
            .iter()
            .any(|c| matches!(text(&c["kind"]), "cancel" | "merge" | "edit_acceptance"));
    if held {
        return None;
    }
    let found = if some(&effects["item_changes"]) && does("apply_effects") {
        "changed items"
    } else if some(&effects["actions"]) {
        "acted at once on a work or on new starts"
    } else if some(&effects["observation_changes"]) || some(&outcome["observation_changes"]) {
        "changed observations"
    } else if some(&effects["messages"]) {
        "wrote to another project's architect"
    } else if output["tell_owner"].as_array().is_some_and(|a| a.iter().any(said)) {
        "told the owner"
    } else if does("reply_to_owner") && said(&outcome["answer"]["reply"]) {
        "replied to the owner"
    } else if does("answer_architect") && said(&outcome["answer"]["reply"]) {
        "answered another project's architect"
    } else if does("reply_to_engineer") && outcome["reply"].is_object() {
        "replied to the engineer"
    } else if does("answer_reporter") {
        "told the work that reported what became of it"
    } else {
        return None;
    };
    Some(found.to_string())
}

// The architect's checks.

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

/// The sentences of a text, in any language, as Unicode's rules of sentences (UAX #29) find them, each without the
/// spaces and the punctuation it ends with, so that a quote of it is found however it ends.
fn sentences(text: &str) -> Vec<String> {
    text.split_sentence_bounds()
        .map(|s| s.trim_end_matches(|c: char| !c.is_alphanumeric()).trim())
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
    let f = e.shared.flow(e.flow_now)?;
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

/// What is wrong with the effects of an answer, every reason at once: its item changes, as made in order; how the
/// situation has changed under them since they were drafted; the owner's words they rest on; its changes to
/// observations; its actions; its messages.
fn effects_problems(c: &checks::Ctx<'_>, project: &str, effects: &Value) -> Result<Vec<String>, String> {
    let conn = c.e.db.conn();
    let mut problems = Vec::new();
    let new = items(effects.get("item_changes"));
    if !new.is_empty()
        && let Err(why) = crate::arch_changes::validate(c.e, project, &new)
    {
        problems.extend(why.lines().map(|l| match l.strip_prefix("change ") {
            Some(rest) => format!("item change {rest}"),
            None => l.to_string(),
        }));
    }
    // A cancel of an item the same answer creates, by its key, would make an item only to cancel it: the answer leaves
    // all of it out instead, as a flow version before dropped such a create, the changes naming its key and the cancel
    // from its batch.
    let created: BTreeSet<&str> = new
        .iter()
        .filter(|ch| ch["kind"] == "create_item")
        .map(|ch| text(&ch["key"]).trim())
        .filter(|k| !k.is_empty())
        .collect();
    for (i, ch) in new.iter().enumerate() {
        let key = text(&ch["item"]).trim();
        if ch["kind"] == "cancel" && created.contains(key) {
            problems.push(format!(
                "item change {} (cancel) cancels {key}, which this answer creates: an item is never made only to be \
                 cancelled, so leave out its create, every change that names {key} and the cancel",
                i + 1
            ));
        }
    }
    let matter = picked(conn, c.w.id)?;
    for (i, ch) in new.iter().enumerate() {
        let n = i + 1;
        for p in changed_under(c, project, matter, ch)? {
            problems.push(format!("item change {n} ({}): {p}", text(&ch["kind"])));
        }
        for p in words_problems(conn, project, ch)? {
            problems.push(format!("item change {n} ({}): {p}", text(&ch["kind"])));
        }
    }
    problems.extend(observation_problems(
        c,
        project,
        &items(effects.get("observation_changes")),
        &new,
    )?);
    for (i, a) in items(effects.get("actions")).iter().enumerate() {
        let n = i + 1;
        if a["kind"] == "revert_merge" {
            let commit = text(&a["merge_commit"]).trim();
            let known = c.e.config.config.projects.get(project).is_some_and(|p| {
                !commit.is_empty() && git::resolve(&p.checkout, &format!("{commit}^{{commit}}")).is_some()
            });
            if !known {
                problems.push(format!(
                    "action {n}: {commit} is no commit the project's checkout knows: name the merge commit as main has it"
                ));
            }
            continue;
        }
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
                if !db::stopped_by_architect(p.as_deref(), by.as_deref()) {
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

/// The version of an item: the latest change to its row, as the log of changes numbers it; 0 for an item never
/// changed since the log began.
fn version_of(conn: &Connection, item: i64) -> Result<i64, String> {
    conn.query_row(
        "SELECT coalesce(max(id), 0) FROM changes WHERE table_name = 'items' AND row_key = json_object('id', ?1)",
        [item],
        |r| r.get(0),
    )
    .map_err(sql)
}

/// Whether an item merged: a work on it took the merge queue's row of a merge.
fn merged(conn: &Connection, item: i64) -> Result<bool, String> {
    Ok(count(
        conn,
        &format!(
            "SELECT count(*) FROM rounds r JOIN works w ON w.id = r.work
             WHERE w.item = ?1 AND r.cell IN {} AND r.transition = 'merged'",
            db::cells_sql(ENGINEER, &["M2"])
        ),
        &[&item],
    )? > 0)
}

/// An item as an item change finds it: its title, why it was cancelled, why it is held and its acceptance.
type ItemState = (String, Option<String>, Option<String>, Option<String>);

/// How the situation has changed under an item change since it was drafted, every way at once: an item it changes
/// merged or was cancelled; a text it changes is at another version than the one it was drafted on, the item as it is
/// now said; an acceptance it changes is locked by a work's design; an item a work runs on is changed from outside that
/// work's own question with no word of why the work must change, or merged into or folded into, which takes in work
/// from outside; an observation it folds in was folded into another item; a dependency it makes is on an item merged,
/// cancelled or held. `matter` is the matter in hand, whose work's own item it may change.
fn changed_under(c: &checks::Ctx<'_>, project: &str, matter: Option<i64>, ch: &Value) -> Result<Vec<String>, String> {
    let conn = c.e.db.conn();
    let mut problems = Vec::new();
    let kind = text(&ch["kind"]);
    let of_work: Option<i64> = match matter {
        Some(m) => conn
            .query_row("SELECT work FROM architect_matters WHERE id = ?1", [m], |r| r.get(0))
            .map_err(sql)?,
        None => None,
    };
    let state = |id: i64| -> Result<Option<ItemState>, String> {
        conn.query_row(
            "SELECT title, cancelled, held, acceptance FROM items WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()
        .map_err(sql)
    };
    let changed: Vec<i64> = ["item", "keep"]
        .iter()
        .filter_map(|k| item_number(&ch[*k]))
        .chain(items(ch.get("absorbed")).iter().filter_map(item_number))
        .collect();
    for id in changed {
        let Some((title, cancelled, _, acceptance)) = state(id)? else {
            continue;
        };
        if merged(conn, id)? && kind != "add_known_red_check" {
            problems.push(format!(
                "item {id} \"{title}\" has merged already: its work is done, and what is left is a new item"
            ));
        }
        if cancelled.is_some() {
            problems.push(format!("item {id} \"{title}\" is cancelled already"));
        }
        if matches!(kind, "edit_item" | "edit_acceptance") {
            let now = version_of(conn, id)?;
            if ch["version"].as_i64() != Some(now) {
                problems.push(format!(
                    "item {id} was changed after you drafted this change on its version {}: it is at version {now} \
                     now, its title \"{title}\"; read it again as it is and write the change on that",
                    ch["version"]
                ));
            }
        }
        if kind == "edit_acceptance" && acceptance.is_some() {
            problems.push(format!(
                "item {id}'s acceptance is locked by its work's design: what the work must deliver changes through the \
                 work, by its own question, not from outside"
            ));
        }
        let running = running_work(conn, id)?.map(|r| r.0);
        if let Some(work) = running {
            let own_question = of_work == Some(work);
            let unavoidable = !text(&ch["unavoidable"]).trim().is_empty();
            match kind {
                "edit_item" | "edit_acceptance" if !own_question && !unavoidable => problems.push(format!(
                    "a work runs on item {id}: what comes from outside its own question is a new item, which may \
                     depend on it; change it only when the work would certainly go wrong without the change, saying \
                     why in unavoidable"
                )),
                "merge" | "fold_observations" if !own_question => problems.push(format!(
                    "a work runs on item {id}: only items not started merge or take observations in, and what follows \
                     on from a work is a new item that depends on it"
                )),
                _ => {}
            }
        }
    }
    if kind == "fold_observations" {
        for o in items(ch.get("observations")) {
            let Some(id) = o.as_i64() else { continue };
            let into: Option<Option<i64>> = conn
                .query_row(
                    "SELECT folded_into FROM observations WHERE id = ?1 AND project = ?2",
                    params![id, project],
                    |r| r.get(0),
                )
                .optional()
                .map_err(sql)?;
            if let Some(Some(other)) = into
                && item_number(&ch["item"]) != Some(other)
            {
                problems.push(format!("observation {id} was folded into item {other} already"));
            }
        }
    }
    if kind == "add_dependency"
        && let Some(on) = item_number(&ch["on"])
        && let Some((title, cancelled, held, _)) = state(on)?
    {
        if merged(conn, on)? {
            problems.push(format!(
                "item {on} \"{title}\" has merged already: there is nothing to wait for"
            ));
        }
        if cancelled.is_some() {
            problems.push(format!("item {on} \"{title}\" is cancelled: it will never deliver"));
        }
        if held.is_some() {
            problems.push(format!(
                "item {on} \"{title}\" is held: an item waiting for it waits as long as the hold"
            ));
        }
    }
    let title = match kind {
        "create_item" => Some(text(&ch["title"])),
        "edit_item" if ch["part"] == "title" => Some(text(&ch["text"])),
        _ => None,
    };
    if let Some(t) = title
        && let Some(p) = title_language_problem(&c.e.config.config.install.owner.language, t)
    {
        problems.push(p);
    }
    Ok(problems)
}

/// The languages whose text is written in characters of East Asian wide width, by their names in English, as the
/// setting `owner.language` may give them.
const WIDE_LANGUAGES: [&str; 4] = ["chinese", "japanese", "korean", "cantonese"];

/// What is wrong with a title's language, as far as its characters show it: one in a language written in East Asian
/// wide characters ([`WIDE_LANGUAGES`]) holds some, and one in English none; none for any other language, which its
/// characters do not tell.
fn title_language_problem(language: &str, title: &str) -> Option<String> {
    let language = language.trim().to_lowercase();
    let wide = title.chars().any(|c| config::title_width(&c.to_string()) == 2);
    if WIDE_LANGUAGES.iter().any(|l| language.starts_with(l)) && !wide {
        return Some(format!(
            "the title \"{title}\" is not in the owner's language, {language}: write it in {language}"
        ));
    }
    if language.starts_with("english") && wide {
        return Some(format!(
            "the title \"{title}\" is not in the owner's language, English: write it in English"
        ));
    }
    None
}

/// What is wrong with the owner's words an item change rests on (`owner_words`), every quotation at once: its record
/// is not there or another project's; the quotation is not found letter for letter in it; a delegate's session said it,
/// in no part it declared the owner's own; or the change says the owner accepted it and the record is no answer of the
/// owner's.
fn words_problems(conn: &Connection, project: &str, ch: &Value) -> Result<Vec<String>, String> {
    let mut problems = Vec::new();
    for (k, q) in items(ch.get("owner_words")).iter().enumerate() {
        let n = k + 1;
        let from = text(&q["from"]);
        let id = q["id"].as_i64().unwrap_or_default();
        let quote = text(&q["quote"]);
        let record = match crate::decisions::record(conn, project, from, id) {
            Ok(r) => r,
            Err(why) => {
                problems.push(format!("the owner's words {n}: {why}"));
                continue;
            }
        };
        if !record.texts.iter().any(|t| !quote.is_empty() && t.contains(quote)) {
            problems.push(format!(
                "the owner's words {n}: \"{quote}\" is not found as it is in {from} {id}: quote its words letter for \
                 letter, or write nothing as the owner's"
            ));
            continue;
        }
        match record.source_of(quote) {
            crate::decisions::Source::Delegate => problems.push(format!(
                "the owner's words {n}: {from} {id} is said by the delegate's session, not the owner, and declares no \
                 part of it the owner's own: write it as the delegate's, or not as the owner's meaning"
            )),
            crate::decisions::Source::Architect => problems.push(format!(
                "the owner's words {n}: {from} {id} holds the architect's words, not the owner's"
            )),
            _ => {}
        }
        if q["accepted"] == true && from != "ask" && from != "matter" {
            problems.push(format!(
                "the owner's words {n}: it says the owner accepted, and {from} {id} is no answer of the owner's: cite \
                 the owner's answer, or do not write it as accepted"
            ));
        }
    }
    Ok(problems)
}

/// What is wrong with changes to a project's observations, as an answer's effects or the revision of a batch hands them
/// in: a new one that says nothing, has no title or one wider than an item's may be, or carries no tag of the
/// project's, or one not of them; a merge or a close of an observation the project has no open one of; a merge that
/// makes one no item took in yet reported [`RECURS`] times or more, which is work, when the batch's item changes
/// (`batch`, as they would be with this round's) do not take it in ([`recurring_problem`]); a close of one an item takes
/// in, which is folded into that item instead.
fn observation_problems(
    c: &checks::Ctx<'_>,
    project: &str,
    changes: &[Value],
    batch: &[Value],
) -> Result<Vec<String>, String> {
    let conn = c.e.db.conn();
    let mut problems = Vec::new();
    // How many merges into each observation the changes so far make.
    let mut merges: BTreeMap<i64, i64> = BTreeMap::new();
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
                // A list of observations leads each with its title, which is as short as an item's.
                let title = text(&o["title"]).trim();
                if title.is_empty() {
                    problems.push(format!("observation change {n}: it has no title"));
                }
                crate::arch_changes::title_fits(
                    title,
                    c.e.config.config.install.items.title_max as usize,
                    &mut |p: String| {
                        problems.push(format!("observation change {n}: {p}"));
                    },
                );
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
                    continue;
                }
                let merged = merges.entry(id).or_default();
                *merged += 1;
                let (times, folded): (i64, Option<i64>) = conn
                    .query_row(
                        "SELECT times_reported, folded_into FROM observations WHERE id = ?1",
                        [id],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )
                    .map_err(sql)?;
                if folded.is_none() && times + *merged >= RECURS {
                    problems.extend(recurring_problem(conn, n, id, times + *merged, batch)?);
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

/// What is wrong with the merge of observation change `n`, which makes observation `id` reported `times` times, at
/// least [`RECURS`], with no item having taken it in yet: a gap that recurs is work, so the batch's item changes fold it
/// into an item, which one of them moves ahead (anywhere but last) unless a work runs on it already; none when they do.
fn recurring_problem(
    conn: &Connection,
    n: usize,
    id: i64,
    times: i64,
    batch: &[Value],
) -> Result<Option<String>, String> {
    let named = |v: &Value| -> String {
        match v {
            Value::String(s) => s.trim().trim_start_matches('#').to_string(),
            other => other.to_string(),
        }
    };
    let fold = batch.iter().find(|c| {
        c["kind"] == "fold_observations" && items(c.get("observations")).iter().any(|o| item_number(o) == Some(id))
    });
    let Some(fold) = fold else {
        return Ok(Some(format!(
            "observation change {n}: it makes observation {id} reported {times} times, and an observation reported \
             {RECURS} times is work: fold it into an item that covers it, or one this round creates, by the item \
             change fold_observations, and move that item ahead"
        )));
    };
    let item = named(&fold["item"]);
    let moved = batch
        .iter()
        .any(|c| c["kind"] == "move" && named(&c["item"]) == item && c["to"]["place"] != "last");
    let runs = match item.parse::<i64>() {
        Ok(i) => running_work(conn, i)?.is_some(),
        Err(_) => false,
    };
    Ok((!moved && !runs).then(|| {
        format!(
            "observation change {n}: it makes observation {id} reported {times} times, which is work, and item \
             {item}, which takes it in, is not moved ahead: move it first, or before or after another item"
        )
    }))
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

/// Whether the decisions an engineer's return hands in hold, which the engine writes as lines of its item's decisions.
/// Each must be one the records it quotes bear out, and none may quote words another of them quotes, or the item already
/// holds. The architect records no decision: a return of its graph has none to hold.
fn decisions_hold(c: &checks::Ctx<'_>, v: Option<&Value>) -> Result<(), Problems> {
    let list = items(v);
    let conn = c.e.db.conn();
    let mut problems: Vec<String> = Vec::new();
    let Some(item) = c.item else {
        return Ok(());
    };
    let mut quoted: BTreeSet<(String, i64, String)> = BTreeSet::new();
    for (i, d) in list.iter().enumerate() {
        match decisions::resolve(conn, &item.project, d, "engineer", Some(item.id)) {
            Ok(r) => {
                for s in r.said {
                    if !quoted.insert((s.from.clone(), s.id, s.words.clone())) {
                        problems.push(format!(
                            "decision {}: \"{}\" of {} {} is quoted by an earlier decision of this return",
                            i + 1,
                            s.words,
                            s.from,
                            s.id
                        ));
                    }
                }
            }
            Err(why) => problems.extend(why.into_iter().map(|p| format!("decision {}: {p}", i + 1))),
        }
    }
    Problems::from(problems).into_result()
}

/// Runs a check of the architect's graph.
pub fn check(c: &checks::Ctx<'_>, name: &str, at: Option<&str>, v: Option<&Value>) -> Result<(), Problems> {
    let conn = c.e.db.conn();
    match name {
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
            Problems::from(problems).into_result()
        }
        "effects_valid" => {
            let Some(effects) = v else { return Ok(()) };
            Problems::from(effects_problems(c, project_of(c.w)?, effects)?).into_result()
        }
        "reply_fits_changes" => {
            let Some(reply) = v else { return Ok(()) };
            let project = project_of(c.w)?;
            let related =
                c.e.config
                    .config
                    .projects
                    .get(project)
                    .map(|p| p.other_projects.clone())
                    .unwrap_or_default();
            let known_item = |n: i64| -> Result<bool, String> {
                Ok(count(conn, "SELECT count(*) FROM items WHERE id = ?1", &[&n])? > 0)
            };
            reply_problems(
                reply,
                &created_keys(&c.unwrapped["effects"]),
                known_item,
                project,
                &related,
                |to| c.e.config.config.projects.contains_key(to),
            )?
            .into_result()
        }
        "decision_words_hold" => decisions_hold(c, v),
        "learned_holds_no_decision" => {
            let text = v.and_then(Value::as_str).unwrap_or_default();
            let lines: Vec<&str> = text.lines().filter(|l| decisions::is_line(l)).collect();
            if lines.is_empty() {
                return Ok(());
            }
            checks::fail(format!(
                "learned holds {} line(s) that start a decision, such as \"{}\": a decision goes in decisions, from the \
                 records it quotes, and the engine makes its line; learned holds only Fact ( and Dropped ( lines",
                lines.len(),
                lines[0].trim()
            ))
        }
        "candidates_known" => {
            let project = project_of(c.w)?;
            candidates_problems(&items(v), |id| open_observation(conn, project, id))?.into_result()
        }
        "notebook_fits" => {
            let Some(edits) = v else { return Ok(()) };
            Problems::from(notebook_problems(conn, project_of(c.w)?, &items(Some(edits)))?).into_result()
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
                checks::fail(format!(
                    "rewrite the notebook's section{} {} whole in notebook",
                    if missing.len() > 1 { "s" } else { "" },
                    missing.join(" and ")
                ))
            }
        }
        "closing_given" => {
            let Some(closing) = v else { return Ok(()) };
            closing_problems(closing).into_result()
        }
        "holds_needs_policy" => {
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
            if replied && policy(c.e, project_of(c.w)?)?.is_none() {
                return Err(
                    "the owner replied and no policy is in force, so none can hold: hand in write_policy with the \
                     owner's stance, the course the project keeps when the owner says none is needed, or ask again \
                     when the reply does not answer"
                        .into(),
                );
            }
            Ok(())
        }
        "policy_or_kept" => {
            let out = &c.unwrapped;
            let has = |k: &str| out.get(k).is_some_and(|x| !x.is_null());
            match v.and_then(|d| d["draft"].as_str()) {
                Some("written") if has("policy") => Ok(()),
                Some("written") => Err("a draft written hands in the whole policy".into()),
                Some("kept") if has("policy") || has("new_measures") => {
                    Err("a draft kept hands in no policy and no new measures: the draft is dropped".into())
                }
                _ => Ok(()),
            }
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
        "departure_ruled_by_owner" => {
            if !v.is_some_and(Value::is_object) {
                return Ok(());
            }
            let Some(m) = picked(conn, c.w.id)? else { return Ok(()) };
            let rows = query_json(
                conn,
                "SELECT kind, json_extract(body, '$.departs') AS departs FROM architect_matters WHERE id = ?1",
                &[&m],
                1,
            )?;
            let Some(row) = rows.first() else { return Ok(()) };
            departure_problems(
                m,
                row["kind"] == crate::open_questions::KIND,
                &row["departs"],
                count(
                    conn,
                    &format!("SELECT count(*) FROM architect_matters m WHERE m.id = ?1 AND {RULED_BY_OWNER}"),
                    &[&m],
                )? > 0,
                // A reply that hands the question on rules on nothing.
                v.filter(|r| !r["hand_over"].is_object())
                    .map(|r| r["departure"].as_str().unwrap_or_default()),
            )
            .into_result()
        }
        "ask_only_on_open_question" => {
            let Some(ask) = v else { return Ok(()) };
            let Some(m) = picked(conn, c.w.id)? else { return Ok(()) };
            let rows = query_json(
                conn,
                "SELECT kind, json_extract(body, '$.design_link') AS link FROM architect_matters WHERE id = ?1",
                &[&m],
                1,
            )?;
            let Some(row) = rows.first() else { return Ok(()) };
            // An engineer waiting for the reply cannot be left waiting for the owner: its question is answered, and
            // what only the owner can decide goes back as an open question of the engineer's design, which waits for
            // nothing.
            if row["kind"] != crate::open_questions::KIND {
                return checks::fail(format!(
                    "matter {m} is {}: its engineer waits for your reply, so it cannot be parked for the owner. Reply \
                     to it; what only the owner can decide, the reply has the engineer put to the owner as an open \
                     question of its design",
                    row["kind"].as_str().unwrap_or("not an open question")
                ));
            }
            // The owner reads the question in their one-to-one, away from the design it is about: it links to it.
            // The whole address of a markdown link: one that only begins with the design's (another round of the same
            // work) leads somewhere else.
            if let Some(link) = row["link"].as_str().filter(|l| !l.is_empty())
                && !markdown_link_targets(text(&ask["question"])).contains(&link)
            {
                return checks::fail(format!(
                    "the question is pinned in the owner's one-to-one, away from the design it is about: link to the \
                     design in it, as a markdown link to {link}"
                ));
            }
            Ok(())
        }
        "policy_text_clean" => {
            let Some(policy) = v else { return Ok(()) };
            let text = policy.to_string();
            policy_text_problems(policy, &owner_words(conn, c.w.id)?, names_cell(c.e, &text)).into_result()
        }
        other => checks::fail(format!("no such engine check: {other}")),
    }
}

/// The most characters a line of the decision log holds.
const LOG_LINE_MOST: usize = 600;

/// What is wrong with what a matter's session leaves as it ends, every problem at once: its line of the decision log is
/// empty, more than one line, or longer than [`LOG_LINE_MOST`]; an answer of the two has no why.
fn closing_problems(closing: &Value) -> Problems {
    let mut problems = Problems::default();
    let log = text(&closing["log"]).trim();
    if log.is_empty() {
        problems.push("the decision log line is empty: say what you did, why, and which items it concerns");
    }
    if log.contains('\n') {
        problems.push("the decision log line is more than one line: write it as one");
    }
    let n = log.chars().count();
    if n > LOG_LINE_MOST {
        problems.push(format!(
            "the decision log line is {n} characters, and one holds {LOG_LINE_MOST} at most: what a later matter needs, \
             shorter"
        ));
    }
    for (answer, why) in [("plan", "plan_why"), ("base", "base_why")] {
        if text(&closing[why]).trim().is_empty() {
            problems.push(format!("{answer} has no why: say it in {why}, in a sentence"));
        }
    }
    problems
}

/// What is wrong with a reply to matter `m` (an open question when `open_question`, which says where a design departs
/// from a decision of its item when it holds `departs`, the line and who made it), the owner having answered an ask that
/// holds it or not (`owner_ruled`): a departure is ruled on only by whoever made the decision, so the architect replies
/// on its own judgment only where the decision is the architect's own, and otherwise after the owner answered. A reply
/// that answers says the ruling (`ruling`, its `departure`: whether the departure stands or is turned down), which
/// decides whether the work may carry on with its design, on a departure and on nothing else; one that hands the
/// question on (`ruling` none) says none.
fn departure_problems(
    m: i64,
    open_question: bool,
    departs: &Value,
    owner_ruled: bool,
    ruling: Option<&str>,
) -> Problems {
    use crate::open_questions::{STANDS, TURNED_DOWN};
    let mut problems = Problems::default();
    let departure = open_question && departs.is_object();
    let answers = ruling.is_some();
    let ruling = ruling.unwrap_or_default();
    if departure && answers && ruling != STANDS && ruling != TURNED_DOWN {
        problems.push(format!(
            "matter {m} says the design departs from a decision of its item: say the ruling in the reply's departure, \
             {STANDS} or {TURNED_DOWN}, as the answer it carries gives it"
        ));
    }
    if !departure && !ruling.is_empty() {
        problems.push(format!(
            "matter {m} says no design departs from a decision of its item: leave out the reply's departure, which \
             only a ruling on such a departure says"
        ));
    }
    let by = departs["by"].as_str().unwrap_or_default();
    if !departure || by == decisions::Source::Architect.as_str() || owner_ruled {
        return problems;
    }
    problems.push(format!(
        "matter {m} says the design departs from a decision of the {}: \"{}\". Only whoever made the decision rules on \
         it: pin it for the owner with an ask, quoting the line and the design's reason, and reply once the owner has \
         answered",
        if by.is_empty() { "owner" } else { by },
        departs["line"].as_str().unwrap_or_default()
    ));
    problems
}

/// What is wrong with a reply that splits a work or hands its question on, each independently: the keys its `children`
/// and `waits` name that are no item (`known_item` says for a number) and no key of an item change of the round
/// (`keys`), each list said once with all its keys; a reply that hands the question on and also splits the work or has
/// it wait; and a hand-over to the architect of a project that is not among the other projects of this one
/// (`related`), or, only when it is among them, that is no project in the configuration (`in_config`).
fn reply_problems(
    reply: &Value,
    keys: &BTreeSet<String>,
    known_item: impl Fn(i64) -> Result<bool, String>,
    project: &str,
    related: &[String],
    in_config: impl Fn(&str) -> bool,
) -> Result<Problems, String> {
    let mut problems = Problems::default();
    for list in ["children", "waits"] {
        let mut unknown: Vec<String> = Vec::new();
        for x in items(reply.get(list)) {
            let k = text(&x["key"]).trim().trim_start_matches('#').to_string();
            let known = match k.parse::<i64>() {
                Ok(n) => known_item(n)?,
                Err(_) => keys.contains(&k),
            };
            if !known && !unknown.contains(&k) {
                unknown.push(k);
            }
        }
        problems.about(
            &unknown,
            |k| format!("the reply's {list} names {k}, which is no item and no key an item change of this round makes"),
            |ks| {
                format!(
                    "the reply's {list} names {ks}, which are no items and no keys an item change of this round makes"
                )
            },
        );
    }
    // A question handed on is not answered: it splits nothing and has the work wait for nothing, and it goes to the
    // owner or to the architect of a project the project's configuration names among its other projects.
    if let Some(h) = reply.get("hand_over").filter(|h| h.is_object()) {
        if !items(reply.get("children")).is_empty() || !items(reply.get("waits")).is_empty() {
            problems.push(
                "a reply that hands the question on neither splits the work nor has it wait: leave out children and \
                 waits, or hand_over",
            );
        }
        if h["to"] == "architect" {
            let to = text(&h["project"]).trim();
            if to == project || !related.iter().any(|o| o == to) {
                problems.push(format!(
                    "hand_over goes to the architect of a project {project}'s configuration names among its other \
                     projects ({}), and names {to:?}",
                    related.join(", ")
                ));
            } else if !in_config(to) {
                problems.push(format!(
                    "hand_over names {to}, which is no project in the configuration"
                ));
            }
        }
    }
    Ok(problems)
}

/// What is wrong with the candidates of a pick (`list`): more than ten, and every candidate that is no open observation
/// of the project (`open` says for a number), each one; the two are independent.
fn candidates_problems(list: &[Value], open: impl Fn(i64) -> Result<bool, String>) -> Result<Problems, String> {
    let mut problems = Problems::default();
    if list.len() > 10 {
        problems.push(format!("at most ten candidates, and there are {}", list.len()));
    }
    for x in list {
        let id = x["id"].as_i64().unwrap_or_default();
        if !open(id)? {
            problems.push(format!("{id} is no open observation of the project"));
        }
    }
    Ok(problems)
}

/// What is wrong with the text of a policy (`policy`), each kind independent of the others: each sentence of at least
/// 30 characters of the owner's words (`said`) that it quotes; a cell of the flow it names (`cell`, as found); a
/// date; an item.
fn policy_text_problems(policy: &Value, said: &[String], cell: Option<String>) -> Problems {
    let text = policy.to_string();
    let mut problems = Problems::default();
    for s in said.iter().flat_map(|said| sentences(said)) {
        if s.chars().count() >= 30 && text.contains(s.as_str()) {
            problems.push(format!("the policy quotes the owner's words: \"{s}\""));
        }
    }
    if let Some(cell) = cell {
        problems.push(format!("the policy names the cell {cell} of the flow"));
    }
    if names_date(&text) {
        problems.push("the policy names a date");
    }
    if let Some(item) = names_item(policy) {
        problems.push(format!(
            "the policy names the item {item}: what to do next under it is thought out at cell N1, never written \
             into it"
        ));
    }
    problems
}

/// One of the engine's inputs of the architect's graph, computed for the work; `main` is the commit of the project's main
/// the round is pinned to, which the inputs that read the project's files read at.
pub fn input(
    e: &Engine,
    w: &Work,
    _g: &CompiledGraph,
    name: &str,
    main: Option<&str>,
) -> Result<Option<Value>, String> {
    let project = project_of(w)?;
    match name {
        "constitution" => Ok(constitution(e, project, main)?.map(Value::String)),
        "decisions" => decisions(e, project, main),
        "guide" => guide(e, project, main).map(Some),
        "cited_nodes" => cited_nodes(e, w, project, main),
        "since_last_base" => since_last_base(e, w, project).map(Some),
        "board" => board(e, project).map(Some),
        "numbers" => numbers(e, w, project).map(Some),
        "observations_overview" => observations_overview(e, project).map(Some),
        "observation_tags" => Ok(observation_tags(e, project)),
        "levels" => Ok(Some(levels(e))),
        "policy" => policy(e, project),
        "policy_signals" => policy_signals(e, w, project),
        "slots" => slots(e).map(Some),
        "matter" => matter(e, w),
        "matter_items" => matter_items(e, w),
        "item_log" => item_log(e, w),
        "similar_items" => similar_items(e, w).map(Some),
        "owner_answer" => owner_answer(e, w),
        "cross_project" => cross_project(e, project).map(Some),
        "told_owner" => told_owner(e, project),
        "owner_talk" => owner_talk(e, project),
        "notebook_project_now" | "notebook_plan" | "notebook_unclear" | "notebook_other" => {
            let section = NOTEBOOK.iter().find(|x| x.1 == name).map_or("", |x| x.0);
            Ok(notebook_section(e.db.conn(), project, section)?.map(Value::String))
        }
        "questions" => questions(e, project),
        "decision_log" => crate::base::decision_log(e, project),
        "since_base" => crate::base::since_base(e, project).map(Some),
        "base_due" => crate::base::base_due(e, project).map(Some),
        "planning_why" => crate::base::planning_why(e, project).map(Some),
        "gathered" => crate::base::gathered(e, project),
        "changes_to_check" => changes_to_check(e, w),
        "handed_in" => handed_in(e, w),
        other => Err(format!("no such engine input: {other}")),
    }
}

/// The item changes of the matter in hand that wait for the second eye, numbered from 1, each whole, and each item it
/// touches in full with the work running on it ([`item_in_full`]); at a look after a revision, which of them the second
/// eye has not seen as they are. None when none waits.
fn changes_to_check(e: &Engine, w: &Work) -> Result<Option<Value>, String> {
    let conn = e.db.conn();
    let Some(m) = picked(conn, w.id)? else {
        return Ok(None);
    };
    let rows = held_changes(conn, m)?;
    if rows.is_empty() {
        return Ok(None);
    }
    // What the second eye found sent back before: the changes it saw then, as they were.
    let seen: BTreeSet<String> = query_json(
        conn,
        "SELECT change FROM architect_item_changes WHERE matter = ?1 AND state = 'revised'",
        &[&m],
        10_000,
    )?
    .iter()
    .map(|r| r["change"].to_string())
    .collect();
    let mut changes = Map::new();
    let mut touched = BTreeSet::new();
    for (i, (_, _, c)) in rows.iter().enumerate() {
        for k in ["item", "keep", "on"] {
            ids_of(&c[k], &mut touched);
        }
        ids_of(&c["under"]["item"], &mut touched);
        ids_of(&c["to"]["item"], &mut touched);
        for a in items(c.get("absorbed")) {
            ids_of(&a, &mut touched);
        }
        let mut x = json!({ "change": c });
        if !seen.is_empty() {
            x["new_since_your_last_look"] = json!(!seen.contains(&c.to_string()));
        }
        changes.insert(format!("{}", i + 1), x);
    }
    let mut items_touched = Map::new();
    for id in touched.into_iter().take(24) {
        if let Some(x) = item_in_full(e, id)? {
            items_touched.insert(format!("#{id}"), x);
        }
    }
    Ok(Some(json!({ "changes": changes, "items": items_touched })))
}

/// What the round that handed the matter's changes in handed in besides them: its reply, its judgment and what it
/// found, as the matter keeps it while the changes wait for the second eye; none when nothing waits.
fn handed_in(e: &Engine, w: &Work) -> Result<Option<Value>, String> {
    let conn = e.db.conn();
    let Some(m) = picked(conn, w.id)? else {
        return Ok(None);
    };
    let held: Option<String> = conn
        .query_row("SELECT effects_held FROM architect_matters WHERE id = ?1", [m], |r| {
            r.get(0)
        })
        .map_err(sql)?;
    let Some(held) = json_col(held) else {
        return Ok(None);
    };
    let mut outcome = held["outcome"].clone();
    if let Some(o) = outcome.as_object_mut() {
        for k in ["effects", "notebook", "closing"] {
            o.remove(k);
        }
    }
    Ok(Some(
        json!({ "cell": held["cell"], "round": held["round"], "answer": outcome }),
    ))
}

/// What the architect knows of its project that a cell of an engineer's work is given, as the architect has it: the
/// constitution, which every agent's cell is given, the decisions in force, the board of the project's items, the
/// decision cards of the others, the policy in force, and two sections of its notebook, the plan (with the owner's
/// lasting instructions) and what the project is now.
pub const KNOWLEDGE: &[&str] = &[
    "constitution",
    "decisions",
    "board",
    "items_decided",
    "policy",
    "notebook_project_now",
    "notebook_plan",
];

/// What a cell of an engineer's work on an item (`item`) of a project is given of the architect's knowledge
/// ([`KNOWLEDGE`]); none when there is nothing. `main` is the commit of the project's main the round is pinned to, for
/// the constitution and the decisions.
pub fn knowledge(
    e: &Engine,
    project: &str,
    item: i64,
    name: &str,
    main: Option<&str>,
) -> Result<Option<Value>, String> {
    match name {
        "constitution" => Ok(constitution(e, project, main)?.map(Value::String)),
        "decisions" => decisions(e, project, main),
        "board" => board(e, project).map(Some),
        "items_decided" => items_decided(e.db.conn(), project, item),
        "policy" => policy(e, project),
        "notebook_project_now" | "notebook_plan" => {
            let section = NOTEBOOK.iter().find(|x| x.1 == name).map_or("", |x| x.0);
            Ok(notebook_section(e.db.conn(), project, section)?.map(Value::String))
        }
        other => Err(format!("no such engine input: {other}")),
    }
}

/// The current decision cards of a project's items other than `item` whose works run, by item: its title, and each
/// card's number, key, question, the option it recommends, whether the owner ruled it and the owner's latest ruling;
/// none when none has one.
fn items_decided(conn: &Connection, project: &str, item: i64) -> Result<Option<Value>, String> {
    let rows = query_json(
        conn,
        &format!(
            "SELECT w.item, i.title, c.id AS design_card, c.key, json_extract(c.body, '$.question') AS question,
                    json_extract(c.body, '$.recommended') AS recommended, c.status,
                    (SELECT x.value FROM json_each(c.rulings) x WHERE json_extract(x.value, '$.by') = 'owner'
                     ORDER BY x.key DESC LIMIT 1) AS ruling
             FROM design_cards c JOIN works w ON w.id = c.work JOIN items i ON i.id = w.item
             WHERE i.project = ?1 AND i.id <> ?2 AND i.cancelled IS NULL AND w.ended IS NULL
               AND w.at NOT IN {} AND c.status IN ('open', 'ruled')
             ORDER BY w.item, c.id",
            db::cells_sql(ENGINEER, &[ENGINEER_END])
        ),
        &[&project, &item],
        ROWS,
    )?;
    let mut out = Map::new();
    for mut r in rows {
        let id = r["item"].as_i64().unwrap_or_default();
        let entry = out
            .entry(format!("#{id}"))
            .or_insert_with(|| json!({ "title": r["title"], "cards": [] }));
        if let Some(o) = r.as_object_mut() {
            for k in ["item", "title"] {
                o.remove(k);
            }
            o.retain(|_, v| !v.is_null());
        }
        if let Some(cards) = entry["cards"].as_array_mut() {
            cards.push(r);
        }
    }
    Ok((!out.is_empty()).then_some(Value::Object(out)))
}

/// The flow new works start on, whose engineer's graph callers borrow (`engineer_graph`).
fn flow_now(e: &Engine) -> Option<Arc<Compiled>> {
    e.shared.flow(e.flow_now)
}

/// The engineer's graph of a flow; none when there is no flow or it has no such graph.
fn engineer_graph(f: Option<&Compiled>) -> Option<&CompiledGraph> {
    f?.graph(ENGINEER)
}

/// The commit of a project's main its files are read at: the one a round was pinned to (`main`), or, outside a round,
/// the commit `origin/<main>` is at in its checkout now. An error names the project.
fn main_at(project: &str, p: &config::Project, main: Option<&str>) -> Result<String, String> {
    match main {
        Some(commit) => Ok(commit.to_string()),
        None => config::main_commit(p).map_err(|x| format!("project {project}: {x}")),
    }
}

/// A project's constitution as its main has it, at the commit `main` pins or else the commit `origin/<main>` is at
/// now; none only when the project names none. A constitution the project names and its main does not have, or that
/// cannot be read, is an error that says which file, at which commit.
pub(crate) fn constitution(e: &Engine, project: &str, main: Option<&str>) -> Result<Option<String>, String> {
    constitution_of(project, e.config.config.projects.get(project), main)
}

/// [`constitution`] of a project's configuration: none for a project that is not configured or names no constitution.
fn constitution_of(project: &str, p: Option<&config::Project>, main: Option<&str>) -> Result<Option<String>, String> {
    let Some(p) = p else { return Ok(None) };
    let Some(path) = &p.constitution else { return Ok(None) };
    let commit = main_at(project, p, main)?;
    config::read_at(p, &commit, path).map(Some).map_err(|x| {
        format!(
            "project {project}: constitution: {x}. Until it can be read no agent of the project can judge, so every \
             work of the project stops at its next round: change the constitution's path in the project's \
             configuration to one main has (spelled exactly, in the same case), or put the file back on main, then \
             press Start."
        )
    })
}

/// A project's decisions in force as its main has them, at the commit `main` pins or else the commit `origin/<main>` is
/// at now: each file of its directory of decisions by its path, with its text ([`config::decisions::in_force`]); none
/// when the project is not configured, or keeps none. A directory that cannot be read is an error that says which, at
/// which commit, and how to set it right.
fn decisions(e: &Engine, project: &str, main: Option<&str>) -> Result<Option<Value>, String> {
    let Some(p) = e.config.config.projects.get(project) else {
        return Ok(None);
    };
    let commit = main_at(project, p, main)?;
    let files = config::decisions::in_force(p, &commit).map_err(|x| {
        format!(
            "project {project}: decisions: {x}. Until they can be read no agent of the project can judge by them, so \
             every work of the project stops at its next round: set decisions in the project's configuration to the \
             directory main keeps them in, or set that directory right on main, then press Start."
        )
    })?;
    Ok((!files.is_empty()).then(|| {
        Value::Object(
            files
                .into_iter()
                .map(|(path, text)| (path, Value::String(text)))
                .collect(),
        )
    }))
}

/// A project's guide as its main has it, at the commit `main` pins or else the commit `origin/<main>` is at now: its
/// design goal, its AGENTS.md and the index of its records.
fn guide(e: &Engine, project: &str, main: Option<&str>) -> Result<Value, String> {
    let p = e
        .config
        .config
        .projects
        .get(project)
        .ok_or_else(|| format!("no project {project} in the configuration"))?;
    let commit = main_at(project, p, main)?;
    let g = config::project_guide_at(&p.checkout, &commit, &p.design_paths(), "")
        .map_err(|x| format!("project {project}: its guide at {commit}: {x}"))?;
    Ok(json!({
        "design_goal": p.design_goal,
        "agents_md": g["agents_md"],
        "records": g["records"],
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

/// The records the matter picked names, in full, as the project's main has them at the commit `main` pins or else the
/// commit `origin/<main>` is at now; none when it names none.
fn cited_nodes(e: &Engine, w: &Work, project: &str, main: Option<&str>) -> Result<Option<Value>, String> {
    let Some(p) = e.config.config.projects.get(project) else {
        return Ok(None);
    };
    let cites = matter_text(e, w)?;
    let commit = main_at(project, p, main)?;
    let g = config::project_guide_at(&p.checkout, &commit, &p.design_paths(), &cites)
        .map_err(|x| format!("project {project}: its records at {commit}: {x}"))?;
    let nodes = g["cited"].clone();
    Ok((!empty(&nodes)).then_some(nodes))
}

/// What an engineer's work waits for, in words, besides a pause, which its row says; none when it waits for nothing.
/// `g` is the engineer's graph of the flow new works start on.
fn work_waits(e: &Engine, g: Option<&CompiledGraph>, work: i64, at: &str) -> Result<Option<String>, String> {
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
    let asks_architect = g
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

/// A project's items running and queued, each by its number, with its rank, its level, what it delivers, the route it
/// is marked for and the route its latest work goes (`work_routes`), its cell and stage, what it waits for and who
/// waits for it, each by the kind of its dependency, since when its work runs and what it spent, and its own pause
/// with who paused it (the owner, the engine or the architect) and why, or why its last work stopped, as one queued
/// again, and for one with no work running, the items not merged yet that it waits for by
/// a blocking dependency before it starts, and its place in the owner's start queue; the owner's start queue, the
/// project's items the owner put first to start, in the owner's order, which start ahead of the ranks (`dispatch_queue`);
/// the release items, each an item nothing depends on with items under it, with the items under it; and the
/// dependencies with other projects' items. A work at a cell of no stage, which asks or
/// runs tools for another, is at the stage of the cell it stopped at, or else of the innermost cell that waits for it
/// with one.
fn board(e: &Engine, project: &str) -> Result<Value, String> {
    let f = flow_now(e);
    let g = engineer_graph(f.as_deref());
    let end = g.map(|g| g.end.clone()).unwrap_or_default();
    let rows = query_json(
        e.db.conn(),
        "SELECT i.id AS item, i.title, i.rank, i.held, i.level, i.deliver, i.route,
                (SELECT max(c.id) FROM changes c WHERE c.table_name = 'items' AND c.row_key = json_object('id', i.id))
                  AS version,
                (SELECT t.route FROM work_routes t WHERE t.work = w.id) AS route_taken, w.id AS work, w.at,
                w.resume_at, w.waiting,
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
            r["waits"] = json!(work_waits(e, g, work, &at)?);
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
    let mut board = json!({
        "items": out,
        "owner_start_queue": queue,
        "releases": releases,
        "other_projects": cross_project(e, project)?,
    });
    // Read from the table the engine reads to hold new starts, at the same moment, so that the architect sees what
    // the engine acts on, the owner's hold included.
    let held = starts_held(e.db.conn(), project)?;
    if !held.is_empty() {
        board["starts_held"] = json!(held);
    }
    Ok(board)
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

/// What happened in the project since its base before this one was built, for the session that builds the next: the
/// first round of the latest base's session (with none, the week before the work started); commits merged on main,
/// works ended or stopped, the CI on main, the owner's words, decision cards ruled and reports that came in, each list
/// the latest first, at most [`LATEST`] of it, then how many earlier ones there are.
fn since_last_base(e: &Engine, w: &Work, project: &str) -> Result<Value, String> {
    let conn = e.db.conn();
    let last = crate::base::latest(conn, project)?.map(|b| b.at);
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
            &["log", "--format=%h %cs %s", after.as_str(), p.main_ref().as_str()],
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
    let main_ci = all(
        "SELECT m.id AS matter, m.what, m.handled_round IS NOT NULL AS handled FROM architect_matters m
         WHERE m.project = ?1 AND m.kind = 'main_red' AND m.at > ?2 ORDER BY m.id DESC",
    )?;
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
        "main_red": latest(main_ci, ("time main was red", "times main was red"), &from),
        "owner_said": latest(owner, ("thing the owner said", "things the owner said"), &from),
        "decision_cards_ruled": latest(cards, ("decision card", "decision cards"), &from),
        "reports": latest(reports, ("report", "reports"), &from),
    });
    if first {
        out["first_base"] = json!(true);
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
        let main = p.main_ref();
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
            let paths = p.design_paths();
            let design: Vec<&str> = paths.iter().map(String::as_str).collect();
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
                    (format!("O{id}"), r)
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

/// The levels an item may be at, lowest first, each with the model its agents run on and the work it is for, so that
/// the cells that make items or set their levels name one that is there.
fn levels(e: &Engine) -> Value {
    Value::Array(
        e.config
            .config
            .item_levels()
            .into_iter()
            .map(|(name, l)| json!({ "level": name, "model": l.model, "for": l.desc }))
            .collect(),
    )
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
    let f = flow_now(e);
    let Some(g) = engineer_graph(f.as_deref()) else {
        return Ok(0);
    };
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
    if let Some(h) = e.halt().filter(|h| db::now() < h.until) {
        return json!({
            "runs": "halted",
            "why": format!("Every work stops: {} ({})", h.kind, h.why.trim()),
            "until": utc_minute(h.until),
        });
    }
    match e.global_pause() {
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

/// The matter in hand, with its material by its kind ([`matter_given`]).
fn matter(e: &Engine, w: &Work) -> Result<Option<Value>, String> {
    let Some(m) = picked(e.db.conn(), w.id)? else {
        return Ok(None);
    };
    matter_given(e, m)
}

/// A matter, with its material by its kind.
fn matter_given(e: &Engine, m: i64) -> Result<Option<Value>, String> {
    let conn = e.db.conn();
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
        // The owner's question about a decision card: the card, the design it stands in and the change to the records.
        "owner_words" if matter["body"]["design_card"].is_i64() => {
            if let Some(card) = matter["body"]["design_card"].as_i64() {
                let rows = query_json(
                    conn,
                    "SELECT c.id AS design_card, c.work, c.round, c.key, c.body, c.revision, c.status, c.rulings,
                            c.changed, w.worktree, i.project
                     FROM design_cards c JOIN works w ON w.id = c.work LEFT JOIN items i ON i.id = w.item
                     WHERE c.id = ?1",
                    &[&card],
                    1,
                )?;
                if let Some(c) = rows.first() {
                    let design = query_json(
                        conn,
                        &format!(
                            "SELECT json_extract(output, '$.outcome') AS design FROM rounds
                             WHERE work = ?1 AND cell IN {} AND transition IS NOT NULL ORDER BY n DESC LIMIT 1",
                            db::cells_sql(ENGINEER, &[DESIGN])
                        ),
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
                        let paths = p.design_paths();
                        let diff = git::fork_point(wt, &p.main).and_then(|f| git::diff(wt, &f, "HEAD", &paths));
                        material.insert("records_diff".into(), json!(diff.unwrap_or_else(|x| x)));
                    }
                }
            }
        }
        // An open question: the design that hands it in, for the architect to answer it by.
        crate::open_questions::KIND => {
            if let Some(round) = matter["round"].as_i64() {
                let design = query_json(
                    conn,
                    "SELECT json_extract(output, '$.outcome') AS design FROM rounds WHERE work = ?1 AND n = ?2",
                    &[&matter["work"].as_i64().unwrap_or_default(), &round],
                    1,
                )?;
                if let Some(d) = design.first().map(|r| r["design"].clone()).filter(|d| !d.is_null()) {
                    material.insert("design".into(), d);
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
        // The hash of the constitution a check keeps tells the architect nothing.
        CHECK => {
            if let Some(b) = matter["body"].as_object_mut() {
                b.remove(CONSTITUTION);
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
    let end = engineer_graph(flow_now(e).as_deref())
        .map(|g| g.end.clone())
        .unwrap_or_default();
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
/// the dependency's kind, its decision cards, and its latest work's cell, state and what it waits for.
fn item_in_full(e: &Engine, item: i64) -> Result<Option<Value>, String> {
    let conn = e.db.conn();
    let Some(body) = query_json(conn, "SELECT * FROM item_body WHERE item = ?1", &[&item], 1)?
        .into_iter()
        .next()
    else {
        return Ok(None);
    };
    let mut out = json!({ "item": body, "version": version_of(conn, item)? });
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
            "SELECT design_card, key, status, body, rulings FROM item_design_cards WHERE item = ?1 AND status IN ('open', 'ruled')",
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
    // The other items whose texts name it, which a change of it may leave pointing at what it no longer is.
    let named = query_json(
        conn,
        "SELECT id, title, problem, why, approach FROM items
         WHERE id <> ?1 AND cancelled IS NULL AND project = (SELECT project FROM items WHERE id = ?1)
           AND instr(coalesce(problem, '') || ' ' || coalesce(why, '') || ' ' || coalesce(approach, ''), '#' || ?1) > 0",
        &[&item],
        ROWS,
    )?;
    let mark = format!("#{item}");
    let named_by: Vec<Value> = named
        .iter()
        .filter(|r| {
            let all = format!("{} {} {}", text(&r["problem"]), text(&r["why"]), text(&r["approach"]));
            all.match_indices(&mark)
                .any(|(at, _)| !all[at + mark.len()..].starts_with(|c: char| c.is_ascii_digit()))
        })
        .map(|r| json!({ "item": r["id"], "title": r["title"] }))
        .collect();
    if !named_by.is_empty() {
        out["named_by"] = Value::Array(named_by);
    }
    if let Some(work) = out["state"][0]["work"].as_i64() {
        let at = text(&out["state"][0]["at"]).to_string();
        let f = flow_now(e);
        out["state"][0]["waits"] = json!(work_waits(e, engineer_graph(f.as_deref()), work, &at)?);
        // A cancel or a start over asked is not done yet: the item reads as it stands until the work's step has ended.
        out["state"][0]["stopping"] = json!(db::stop_asked(conn, work).map_err(sql)?);
        // A resume asked waits for a free slot, and the work still reads as paused: this tells it from one with no resume.
        out["state"][0]["resuming"] = json!(db::resume_asked(conn).map_err(sql)?.contains(&work));
    }
    Ok(Some(out))
}

/// The items and works the matter in hand concerns, each in full, as the fields of the matter and of its material name
/// them: the item it concerns; in its material, and in the place to set straight or the cancelled item it holds, an
/// item, items, and the item of a work or of works; up to twelve. A number in a text, its words' or a step's, names no
/// item: by "matter #2" or "item 1 to 5" there, it concerns none. The observations the place to set straight names
/// follow, whole.
fn matter_items(e: &Engine, w: &Work) -> Result<Option<Value>, String> {
    let conn = e.db.conn();
    let Some(m) = picked(conn, w.id)? else {
        return Ok(None);
    };
    let mut ids = BTreeSet::new();
    let mut observations = BTreeSet::new();
    let row = query_json(conn, "SELECT item, body FROM architect_matters WHERE id = ?1", &[&m], 1)?;
    if let Some(r) = row.first() {
        ids_of(&r["item"], &mut ids);
        let body = &r["body"];
        for part in [body, &body["fix"], &body["cancelled"]] {
            ids_of(&part["item"], &mut ids);
            for i in items(part.get("items")) {
                ids_of(&i, &mut ids);
            }
            for o in items(part.get("observations")) {
                ids_of(&o, &mut observations);
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
    }
    let mut out = Map::new();
    for id in ids.into_iter().take(12) {
        if let Some(x) = item_in_full(e, id)? {
            out.insert(format!("#{id}"), x);
        }
    }
    for id in observations.into_iter().take(12) {
        let rows = query_json(
            conn,
            "SELECT id, title, what, tags, source, refs, times_reported, folded_into, closed FROM observations
             WHERE id = ?1",
            &[&id],
            1,
        )?;
        if let Some(o) = rows.into_iter().next() {
            out.insert(format!("O{id}"), o);
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
fn similar_items(e: &Engine, w: &Work) -> Result<Value, String> {
    let conn = e.db.conn();
    let at_hand_text = matter_text(e, w)?;
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
    if let Some(mine) = running
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checks::{constitution_problems, names_article};

    fn said(p: Problems) -> Vec<String> {
        p.texts().to_vec()
    }

    // A reply to an open question that says a design departs from a decision someone other than the architect made,
    // the owner or a delegate, is sent back until the owner has answered an ask that holds it; one that departs from the
    // architect's own decision, and any other matter, the architect answers by its own judgment.
    #[test]
    fn only_whoever_made_a_decision_rules_on_a_departure_from_it() {
        let stands = Some("stands");
        let owner = json!({ "decision": "L1", "line": "- Decision (2026-11-03, owner): One path.", "by": "owner" });
        let p = departure_problems(7, true, &owner, false, stands);
        assert_eq!(p.texts().len(), 1, "{:?}", p.texts());
        assert!(
            p.texts()[0].starts_with(
                "matter 7 says the design departs from a decision of the owner: \"- Decision (2026-11-03, owner): One \
                 path.\". Only whoever made the decision rules on it"
            ),
            "{:?}",
            p.texts()
        );
        assert!(departure_problems(7, true, &owner, true, stands).is_empty());
        let delegate = json!({ "line": "Decision (2026-11-03, delegate): Two.", "by": "delegate" });
        assert_eq!(departure_problems(7, true, &delegate, false, stands).texts().len(), 1);
        let architect = json!({ "line": "Decision (2026-11-03, architect): Three.", "by": "architect" });
        assert!(departure_problems(7, true, &architect, false, Some("turned_down")).is_empty());
        assert!(departure_problems(7, true, &Value::Null, false, Some("")).is_empty());
        assert!(departure_problems(7, false, &owner, false, Some("")).is_empty());
    }

    // A reply that answers a departure says whether it stands or is turned down, which the engine reads to let the work
    // carry on with its design or not; a reply to any other matter says no such ruling; one that hands the question on
    // rules on nothing. Each problem is reported with the others.
    #[test]
    fn a_ruling_on_a_departure_says_whether_it_stands() {
        let owner = json!({ "decision": "L1", "line": "Decision (2026-11-03, owner): One path.", "by": "owner" });
        let architect =
            json!({ "decision": "L2", "line": "Decision (2026-11-03, architect): Two.", "by": "architect" });
        let says = |p: &Problems, what: &str| p.texts().iter().any(|t| t.contains(what));
        for ruling in [Some(""), Some("maybe")] {
            let p = departure_problems(7, true, &architect, false, ruling);
            assert!(says(&p, "say the ruling in the reply's departure"), "{:?}", p.texts());
        }
        let p = departure_problems(7, true, &owner, false, Some(""));
        assert_eq!(p.texts().len(), 2, "{:?}", p.texts());
        assert!(departure_problems(7, true, &architect, false, None).is_empty());
        let p = departure_problems(7, false, &Value::Null, false, Some("stands"));
        assert!(says(&p, "leave out the reply's departure"), "{:?}", p.texts());
        let p = departure_problems(7, true, &Value::Null, false, Some("turned_down"));
        assert!(says(&p, "leave out the reply's departure"), "{:?}", p.texts());
    }

    // The coherence review is given the current decision cards of the project's other items whose works run, by item:
    // the question, the option recommended, and the owner's latest ruling; not the item's own, nor those of a work that
    // ended, of an item cancelled or of another project, nor a card replaced.
    #[test]
    fn the_other_items_decisions_are_their_current_cards() {
        let mut db = db::Db::in_memory().unwrap();
        db.write(Actor::Engine, |tx| {
            tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
            for (project, title, cancelled) in [
                ("p", "This one", None),
                ("p", "Cache", None),
                ("p", "Gone", Some("dropped")),
                ("q", "Elsewhere", None),
            ] {
                tx.execute(
                    "INSERT INTO items (project, title, problem, why, deliver, level, cancelled)
                     VALUES (?1, ?2, 'x', 'y', 'change', '1', ?3)",
                    params![project, title, cancelled],
                )?;
            }
            for item in 1..=4 {
                tx.execute(
                    "INSERT INTO works (item, flow_version, graph, at) VALUES (?1, 1, 'engineer', 'T1')",
                    [item],
                )?;
                tx.execute(
                    "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES (?1, 1, 'D4', '{}', 'c', 1)",
                    [item],
                )?;
            }
            let card = |work: i64, key: &str, status: &str, rulings: &str| {
                tx.execute(
                    "INSERT INTO design_cards (work, round, key, body, status, rulings)
                     VALUES (?1, 1, ?2, json_object('question', 'Which size?', 'recommended', 'Small'), ?3, ?4)",
                    params![work, key, status, rulings],
                )
            };
            card(1, "own", "open", "[]")?;
            card(2, "size", "ruled", r#"[{"by":"owner","verdict":"choose","option":"Large","at":5}]"#)?;
            card(2, "old", "replaced", "[]")?;
            card(2, "kept", "open", "[]")?;
            card(3, "gone", "open", "[]")?;
            card(4, "far", "open", "[]")?;
            Ok(())
        })
        .unwrap();
        let decided = items_decided(db.conn(), "p", 1).unwrap().unwrap();
        assert_eq!(
            decided,
            json!({ "#2": { "title": "Cache", "cards": [
                { "design_card": 2, "key": "size", "question": "Which size?", "recommended": "Small", "status": "ruled",
                  "ruling": { "by": "owner", "verdict": "choose", "option": "Large", "at": 5 } },
                { "design_card": 4, "key": "kept", "question": "Which size?", "recommended": "Small", "status": "open" },
            ] } })
        );
        assert_eq!(items_decided(db.conn(), "q", 4).unwrap(), None);
    }

    // A text's sentences end where Unicode's rules end them, never inside a version number, each without the spaces
    // and the punctuation it ends with.
    #[test]
    fn a_text_s_sentences_are_found_by_unicode_s_rules() {
        assert_eq!(
            sentences("Keep the page light. Does v1.2 load in time?! Yes\nThen (later) ship it."),
            [
                "Keep the page light",
                "Does v1.2 load in time",
                "Yes",
                "Then (later) ship it"
            ]
        );
        assert!(sentences(" \n ").is_empty());
    }

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

    // Item #216: the checks of the architect's returns say all that is wrong at once, as the engineer's do.
    #[test]
    fn a_table_of_the_constitution_with_several_rows_wrong_is_told_all_of_them() {
        let articles: Vec<String> = ["Article 1. Alpha", "Article 2. Beta", "Article 3. Gamma"]
            .map(String::from)
            .to_vec();
        let rows = |names: &[&str]| -> Vec<Value> { names.iter().map(|n| json!({ "article": n })).collect() };
        assert_eq!(
            said(constitution_problems(
                &articles,
                &rows(&["Article 2", "Article 2 again", "Article 1"])
            )),
            [
                "row 1 is for \"Article 1. Alpha\", in the constitution's order, and names \"Article 2\"",
                "row 3 is for \"Article 3. Gamma\", in the constitution's order, and names \"Article 1\""
            ]
        );
        assert!(constitution_problems(&articles, &rows(&["Article 1", "beta", "article 3"])).is_empty());
        // With the wrong number of rows, which row is for which article is not told: only the number is said.
        assert_eq!(
            said(constitution_problems(&articles, &rows(&["Article 2", "Article 2"]))),
            ["there is one row for each article of the project's constitution, 3 of them, and there are 2"]
        );
        assert_eq!(
            constitution_problems(&articles, &rows(&["x", "y", "z", "w"]))
                .texts()
                .len(),
            1
        );
    }

    #[test]
    fn a_reply_with_several_things_wrong_is_told_all_of_them() {
        let keys: BTreeSet<String> = BTreeSet::from(["new-a".to_string()]);
        let related = vec!["other".to_string()];
        let known_item = |n: i64| -> Result<bool, String> { Ok(n == 7) };
        let reply = json!({
            "children": [{ "key": "#7" }, { "key": "new-a" }, { "key": "new-b" }],
            "waits": [{ "key": "9" }, { "key": "nope" }, { "key": "9" }],
            "hand_over": { "to": "architect", "project": "zzz" }
        });
        let found = reply_problems(&reply, &keys, known_item, "mine", &related, |to| to == "other").unwrap();
        assert_eq!(
            said(found),
            [
                "the reply's children names new-b, which is no item and no key an item change of this round makes",
                "the reply's waits names 9, nope, which are no items and no keys an item change of this round makes",
                "a reply that hands the question on neither splits the work nor has it wait: leave out children and waits, or hand_over",
                "hand_over goes to the architect of a project mine's configuration names among its other projects \
                 (other), and names \"zzz\""
            ]
        );
        // A hand-over to a project among the others that the configuration does not have is said only then.
        let hand = json!({ "hand_over": { "to": "architect", "project": "other" } });
        assert_eq!(
            said(reply_problems(&hand, &keys, known_item, "mine", &related, |_| false).unwrap()),
            ["hand_over names other, which is no project in the configuration"]
        );
        assert!(
            reply_problems(&hand, &keys, known_item, "mine", &related, |_| true)
                .unwrap()
                .is_empty()
        );
        // A hand-over to the work's own project is not among its other projects.
        let own = json!({ "hand_over": { "to": "architect", "project": "mine" } });
        assert_eq!(
            reply_problems(&own, &keys, known_item, "mine", &related, |_| false)
                .unwrap()
                .texts()
                .len(),
            1
        );
        // A read that fails is not a problem of the reply.
        let failing = |_: i64| -> Result<bool, String> { Err("no database".into()) };
        let by_number = json!({ "children": [{ "key": "3" }] });
        assert_eq!(
            reply_problems(&by_number, &keys, failing, "mine", &related, |_| true).unwrap_err(),
            "no database"
        );
    }

    #[test]
    fn candidates_over_ten_and_closed_ones_are_all_told() {
        let list: Vec<Value> = (1..=11).map(|id| json!({ "id": id })).collect();
        let found = candidates_problems(&list, |id| Ok(id % 2 == 0)).unwrap();
        assert_eq!(found.texts().len(), 7, "{found:?}");
        assert_eq!(found.texts()[0], "at most ten candidates, and there are 11");
        assert_eq!(found.texts()[1], "1 is no open observation of the project");
        assert_eq!(found.texts()[6], "11 is no open observation of the project");
        assert!(candidates_problems(&list[..10], |_| Ok(true)).unwrap().is_empty());
        assert_eq!(
            candidates_problems(&list[..10], |id| Ok(id != 3))
                .unwrap()
                .texts()
                .len(),
            1
        );
    }

    #[test]
    fn a_policy_that_quotes_names_a_cell_a_date_and_an_item_is_told_all_four() {
        let said_by_owner = vec![
            "The owner wants the course kept steady for all the projects. Short one.".to_string(),
            "Something the policy does not say at all, however long it is.".to_string(),
        ];
        let policy = json!({
            "text": "The owner wants the course kept steady for all the projects. Cell Q1 is checked; see item #12 and 2026-10-27."
        });
        let found = said(policy_text_problems(&policy, &said_by_owner, Some("Q1".to_string())));
        assert_eq!(
            found,
            [
                "the policy quotes the owner's words: \"The owner wants the course kept steady for all the projects\"",
                "the policy names the cell Q1 of the flow",
                "the policy names a date",
                "the policy names the item #12: what to do next under it is thought out at cell N1, never written into it",
            ]
        );
        let clean = json!({ "text": "Keep the course steady." });
        assert!(policy_text_problems(&clean, &said_by_owner, None).is_empty());
    }

    // A title is in the owner's language as far as its characters show it: one of Chinese, Japanese or Korean holds a
    // wide character, one of English none; another language's characters tell nothing.
    #[test]
    fn a_title_s_language_is_told_by_its_wide_characters() {
        let wide: String = char::from_u32(0x4E2D)
            .into_iter()
            .chain(char::from_u32(0x6587))
            .collect();
        assert!(title_language_problem("Chinese", "Parse the input first").is_some());
        assert!(title_language_problem("Chinese", &format!("{wide} input")).is_none());
        assert!(title_language_problem("English", &format!("{wide} input")).is_some());
        assert!(title_language_problem("English", "Parse the input first").is_none());
        assert!(title_language_problem("French", "Analyser l'entree").is_none());
    }

    // Each cell whose closing is checked is told, in the formats it is given, the most characters the check lets its
    // decision log line hold: a limit the agent learns only from a send-back costs a round each time.
    #[test]
    fn a_cell_that_closes_a_matter_is_told_the_most_its_log_line_holds() {
        let f = flow::compile(
            flow::FLOW,
            &flow::Context {
                relations: db::relations(db::SCHEMA).unwrap(),
                tables: db::index_of(db::SCHEMA).unwrap(),
            },
        )
        .unwrap();
        let limit = format!("of at most {LOG_LINE_MOST} characters");
        let mut told = 0;
        for g in &f.graphs {
            for c in &g.cells {
                if !c.checks.iter().any(|x| x.check == "closing_given") {
                    continue;
                }
                let given = format!(
                    "{}\n{}",
                    flow::formats::common_format_of(g, &sessions::format_cells(g, c)),
                    flow::formats::cell_format(g, c)
                );
                let log = given
                    .split("\"description\":\"")
                    .find(|d| d.starts_with("One line, in English"))
                    .unwrap_or_else(|| panic!("{} {} is not given the log line's description", g.name, c.id));
                assert!(
                    log.split('"').next().unwrap().contains(&limit),
                    "{} {}: {log}",
                    g.name,
                    c.id
                );
                told += 1;
            }
        }
        assert!(told > 0, "no cell checks a closing");
    }

    // What a matter's session leaves as it ends is told every problem at once: an empty or long log line, one of two
    // lines, and an answer with no why.
    #[test]
    fn a_closing_with_several_things_wrong_is_told_all_of_them() {
        let good = json!({ "log": "Made #3 to fix the parser, for #2.", "plan": "no", "plan_why": "Nothing moved.",
                           "base": "holds", "base_why": "Nothing it read changed." });
        assert!(closing_problems(&good).into_result().is_ok());
        let bad = json!({ "log": format!("{}\nmore", "x".repeat(700)), "plan": "yes", "plan_why": "", "base": "holds",
                          "base_why": " " });
        let Err(p) = closing_problems(&bad).into_result() else {
            panic!("a closing with problems passed");
        };
        let said = format!("{p:?}");
        for part in ["more than one line", "characters", "plan has no why", "base has no why"] {
            assert!(said.contains(part), "{part}: {said}");
        }
    }
}
