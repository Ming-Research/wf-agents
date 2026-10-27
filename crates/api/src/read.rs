//! The reads that fill the interface's types from the database. Where a work stands is derived here as the engine
//! derives it (`Engine::done` and `Engine::waiting`): from the work's row, its rounds and its flow version.

use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::io::Read as _;
use std::path::Path;
use std::sync::{Arc, LazyLock, Mutex, MutexGuard};

use db::rusqlite::types::ValueRef;
use db::rusqlite::{self, Connection, OptionalExtension, Row, params_from_iter};
use flow::format::Src;
use flow::{Compiled, CompiledCell, CompiledGraph};
use serde_json::Value;

use crate::view;
use crate::{
    AskKind, DesignCard, FlowVersion, FlowView, Interjection, ItemDetail, ItemLink, ItemState, ItemSummary, Items,
    KeptFile, LinkedItem, OwnerAsk, OwnerMessage, PausedBy, Replacement, RoundConditional, RoundDetail, RoundInput,
    RoundOutcome, RoundSummary, RunningRound, Session, Shown, ToolRun, Waits, WorkDetail, WorkState, WorkSummary,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// What was asked for does not exist.
    NotFound(String),
    /// What was asked for is not what the interface takes, such as a ref that is no ref or a flow that does not
    /// compile.
    Invalid(String),
    /// The read failed.
    Failed(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotFound(s) | Error::Invalid(s) | Error::Failed(s) => write!(f, "{s}"),
        }
    }
}

impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Error {
        Error::Failed(e.to_string())
    }
}

type Result<T> = std::result::Result<T, Error>;

/// The compiled flows read so far, by version. A stored version never changes, so each is read and parsed once.
#[derive(Default)]
pub struct Flows(Mutex<BTreeMap<i64, Arc<Compiled>>>);

impl Flows {
    fn lock(&self) -> MutexGuard<'_, BTreeMap<i64, Arc<Compiled>>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn get(&self, conn: &Connection, version: i64) -> Result<Arc<Compiled>> {
        if let Some(c) = self.lock().get(&version) {
            return Ok(Arc::clone(c));
        }
        let c: Compiled = serde_json::from_str(&compiled_flow(conn, version)?)
            .map_err(|e| Error::Failed(format!("flow version {version}: {e}")))?;
        let c = Arc::new(c);
        self.lock().insert(version, Arc::clone(&c));
        Ok(c)
    }

    /// The graph a work runs, when its flow version can be read.
    fn graph(&self, conn: &Connection, version: i64, graph: &str) -> Option<CompiledGraph> {
        self.get(conn, version).ok()?.graph(graph).cloned()
    }
}

fn json(s: Option<String>) -> Option<Value> {
    s.and_then(|t| serde_json::from_str(&t).ok())
}

/// A column as text, whatever its type: a returned field may be text or anything else.
fn text(r: &Row<'_>, i: usize) -> rusqlite::Result<Option<String>> {
    Ok(match r.get_ref(i)? {
        ValueRef::Null => None,
        ValueRef::Integer(n) => Some(n.to_string()),
        ValueRef::Real(f) => Some(f.to_string()),
        ValueRef::Text(t) | ValueRef::Blob(t) => Some(String::from_utf8_lossy(t).into_owned()),
    })
}

/// How a round ended, from what it took or why it was rejected. A failed run is recorded under `failed` with its
/// kind, or, in a round recorded before the kinds were, under `error`.
fn outcome(transition: Option<&str>, rejected: Option<&Value>, ended: bool) -> RoundOutcome {
    match (transition, rejected) {
        (Some(_), _) => RoundOutcome::Took,
        (None, Some(r)) if r.get("interrupted").is_some() => RoundOutcome::Interrupted,
        (None, Some(r)) if r.get("failed").is_some() || r.get("error").is_some() => RoundOutcome::Failed,
        (None, Some(_)) => RoundOutcome::SentBack,
        (None, None) if ended => RoundOutcome::Ended,
        (None, None) => RoundOutcome::Running,
    }
}

/// Why a round was rejected, in its own words: the text of the one reason it holds; for a failed run, its kind and
/// what the command line reported.
fn why(rejected: Option<&Value>) -> Option<String> {
    let (_, v) = rejected?.as_object()?.iter().next()?;
    Some(match v {
        Value::String(s) => s.clone(),
        Value::Object(f) if f.contains_key("kind") => format!(
            "{}: {}",
            f["kind"].as_str().unwrap_or_default(),
            f.get("what").and_then(Value::as_str).unwrap_or_default()
        ),
        other => other.to_string(),
    })
}

fn is_ask(cell: &CompiledCell) -> bool {
    matches!(cell.step.as_deref(), Some("ask_owner" | "ask_architect"))
}

/// A work's row.
struct WorkRow {
    id: i64,
    item: Option<i64>,
    title: Option<String>,
    project: Option<String>,
    flow_version: i64,
    graph: String,
    at: String,
    resume_at: Option<String>,
    waiting: Option<String>,
    recorded: Option<String>,
    machine: Option<String>,
    worktree: Option<String>,
    branch: Option<String>,
    pr: Option<i64>,
    pushed_head: Option<String>,
    paused: Option<String>,
    ended: Option<String>,
    paused_by: Option<String>,
}

/// What a work's summary needs of each of its rounds.
struct Light {
    n: i64,
    work: i64,
    cell: Option<String>,
    transition: Option<String>,
    rejected: Option<Value>,
    started_at: i64,
    ended_at: Option<i64>,
    cost: Option<f64>,
}

impl Light {
    fn outcome(&self) -> RoundOutcome {
        outcome(
            self.transition.as_deref(),
            self.rejected.as_ref(),
            self.ended_at.is_some(),
        )
    }
}

/// The works a filter on `works w` selects, with their rounds.
fn work_rows(conn: &Connection, filter: &str, args: &[i64]) -> Result<Vec<(WorkRow, Vec<Light>)>> {
    let mut st = conn.prepare(&format!(
        "SELECT w.id, w.item, i.title, i.project, w.flow_version, w.graph, w.at, w.resume_at, w.waiting, w.recorded,
                w.machine, w.worktree, w.branch, w.pr, w.pushed_head, w.paused, w.ended, w.paused_by
         FROM works w LEFT JOIN items i ON i.id = w.item {filter} ORDER BY w.id"
    ))?;
    let works: Vec<WorkRow> = st
        .query_map(params_from_iter(args), |r| {
            Ok(WorkRow {
                id: r.get(0)?,
                item: r.get(1)?,
                title: r.get(2)?,
                project: r.get(3)?,
                flow_version: r.get(4)?,
                graph: r.get(5)?,
                at: r.get(6)?,
                resume_at: r.get(7)?,
                waiting: r.get(8)?,
                recorded: r.get(9)?,
                machine: r.get(10)?,
                worktree: r.get(11)?,
                branch: r.get(12)?,
                pr: r.get(13)?,
                pushed_head: r.get(14)?,
                paused: r.get(15)?,
                ended: r.get(16)?,
                paused_by: r.get(17)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    let mut st = conn.prepare(&format!(
        "SELECT n, work, cell, transition, rejected, started_at, ended_at, json_extract(usage, '$.cost_usd')
         FROM rounds WHERE work IN (SELECT w.id FROM works w {filter}) ORDER BY work, n"
    ))?;
    let mut rounds: HashMap<i64, Vec<Light>> = HashMap::new();
    let rows = st.query_map(params_from_iter(args), |r| {
        Ok(Light {
            n: r.get(0)?,
            work: r.get(1)?,
            cell: r.get(2)?,
            transition: r.get(3)?,
            rejected: json(r.get(4)?),
            started_at: r.get(5)?,
            ended_at: r.get(6)?,
            cost: r.get(7)?,
        })
    })?;
    for l in rows {
        let l = l?;
        rounds.entry(l.work).or_default().push(l);
    }
    Ok(works
        .into_iter()
        .map(|w| {
            let rs = rounds.remove(&w.id).unwrap_or_default();
            (w, rs)
        })
        .collect())
}

/// Where a work stands, as the engine sees it.
fn state(w: &WorkRow, rounds: &[Light], g: Option<&CompiledGraph>) -> WorkState {
    if w.ended.is_some() {
        return WorkState::Ended;
    }
    if let Some(g) = g
        && w.at == g.end
        && rounds
            .iter()
            .any(|r| r.cell.as_deref() == Some(g.end.as_str()) && r.ended_at.is_some() && r.rejected.is_none())
    {
        return WorkState::Done;
    }
    if w.paused.is_some() {
        return WorkState::Paused;
    }
    let cell = g.and_then(|g| g.cell(&w.at));
    if cell.is_some_and(is_ask) {
        return WorkState::Asking;
    }
    if rounds.iter().any(|r| r.ended_at.is_none()) {
        return WorkState::Running;
    }
    if waiting_row(cell, rounds).is_some() {
        WorkState::Waiting
    } else {
        WorkState::Ready
    }
}

/// The row that waits which the work's last round took at the cell the work is at, when it took one.
fn waiting_row<'r>(cell: Option<&CompiledCell>, rounds: &'r [Light]) -> Option<&'r str> {
    let (c, r) = (cell?, rounds.last()?);
    let t = r.transition.as_deref()?;
    (r.cell.as_deref() == Some(c.id.as_str()) && c.rows.iter().any(|x| x.name == t && x.waits)).then_some(t)
}

/// What a work waits for, as the engine judges it, read from the database alone, never from the kind of cell it is
/// at: the owner's answer to the ask it put, at a cell that asks; at a row that waits, the items its item depends on
/// that are not merged, then the owner's ruling on its design card waiting for one, else the row. A paused work is
/// said to wait too, so that the owner sees both. Beside it, the ask whose answer came at the cell that asked, which
/// the work's next step takes in.
fn waits(
    conn: &Connection,
    w: &WorkRow,
    rounds: &[Light],
    g: Option<&CompiledGraph>,
) -> Result<(Option<Waits>, Option<i64>)> {
    if w.ended.is_some() {
        return Ok((None, None));
    }
    let cell = g.and_then(|g| g.cell(&w.at));
    if cell.is_some_and(is_ask) {
        let open = rounds
            .iter()
            .rev()
            .find(|r| r.ended_at.is_none() && r.cell.as_deref() == Some(w.at.as_str()));
        let Some(r) = open else { return Ok((None, None)) };
        let ask: Option<(i64, bool)> = conn
            .query_row(
                "SELECT id, answer IS NOT NULL FROM owner_asks WHERE work = ?1 AND round = ?2 ORDER BY id DESC LIMIT 1",
                [r.work, r.n],
                |x| Ok((x.get(0)?, x.get(1)?)),
            )
            .optional()?;
        return Ok(match ask {
            Some((ask, false)) => (Some(Waits::Answer { ask }), None),
            Some((ask, true)) => (None, Some(ask)),
            None => (None, None),
        });
    }
    let Some(row) = waiting_row(cell, rounds) else {
        return Ok((None, None));
    };
    let mut st = conn.prepare(
        "SELECT item FROM waited_items WHERE work = ?1 AND merged IS NULL AND cancelled IS NULL ORDER BY item",
    )?;
    let items: Vec<i64> = st.query_map([w.id], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
    if !items.is_empty() {
        return Ok((Some(Waits::Items { items }), None));
    }
    let card: Option<i64> = conn
        .query_row(
            "SELECT id FROM design_cards WHERE work = ?1 AND status = 'open' ORDER BY id DESC LIMIT 1",
            [w.id],
            |x| x.get(0),
        )
        .optional()?;
    let waits = match card {
        Some(design_card) => Waits::Ruling { design_card },
        None => Waits::Row {
            cell: w.at.clone(),
            row: row.to_string(),
        },
    };
    Ok((Some(waits), None))
}

fn summary(
    w: &WorkRow,
    rounds: &[Light],
    g: Option<&CompiledGraph>,
    open: (i64, i64),
    (waits, answered): (Option<Waits>, Option<i64>),
) -> WorkSummary {
    let cell = g.and_then(|g| g.cell(&w.at));
    // A cell at no stage asks, answers or runs tools for another: the work is at the stage of the cell it stopped at,
    // which is where it goes back to, or else the last cell with a stage it had a round at (the rounds are in order).
    let stage = cell
        .and_then(|c| c.stage.clone())
        .or_else(|| {
            let stopped = w.resume_at.as_deref()?;
            g?.cell(stopped)?.stage.clone()
        })
        .or_else(|| {
            rounds
                .iter()
                .rev()
                .find_map(|r| g?.cell(r.cell.as_deref()?)?.stage.clone())
        });
    let count = |want: &[RoundOutcome]| rounds.iter().filter(|r| want.contains(&r.outcome())).count() as i64;
    WorkSummary {
        id: w.id,
        item: w.item,
        title: w.title.clone(),
        project: w.project.clone(),
        graph: w.graph.clone(),
        flow_version: w.flow_version,
        at: w.at.clone(),
        at_title: cell.map(|c| c.title.clone()),
        stage,
        // A work at a cell that asks whose ask was answered waits no more: its next step takes the answer in.
        state: match state(w, rounds, g) {
            WorkState::Asking if waits.is_none() => WorkState::Ready,
            s => s,
        },
        paused: w.paused.clone(),
        paused_by: match w.paused_by.as_deref() {
            Some("owner") => Some(PausedBy::Owner),
            Some(_) => Some(PausedBy::Engine),
            None => None,
        },
        ended: w.ended.clone(),
        waits,
        answered,
        branch: w.branch.clone(),
        pr: w.pr,
        started_at: rounds.first().map(|r| r.started_at),
        last_at: rounds.iter().map(|r| r.ended_at.unwrap_or(r.started_at)).max(),
        rounds: rounds.len() as i64,
        sent_back: count(&[RoundOutcome::SentBack]),
        failed: count(&[RoundOutcome::Failed, RoundOutcome::Interrupted]),
        running: rounds
            .iter()
            .rev()
            .find(|r| r.ended_at.is_none())
            .map(|r| RunningRound {
                n: r.n,
                cell: r.cell.clone(),
                started_at: r.started_at,
            }),
        cost_usd: rounds.iter().filter_map(|r| r.cost).sum(),
        open_asks: open.0,
        open_design_cards: open.1,
    }
}

/// The works a filter selects, as summaries, with their rows.
fn summaries(conn: &Connection, flows: &Flows, filter: &str, args: &[i64]) -> Result<Vec<(WorkRow, WorkSummary)>> {
    let count = |q: &str| -> Result<HashMap<i64, i64>> {
        let mut st = conn.prepare(q)?;
        Ok(st
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?)
    };
    let asks = count("SELECT work, count(*) FROM owner_asks WHERE answer IS NULL GROUP BY work")?;
    let design_cards = count("SELECT work, count(*) FROM design_cards WHERE status = 'open' GROUP BY work")?;
    let mut graphs: HashMap<(i64, String), Option<CompiledGraph>> = HashMap::new();
    let mut out = Vec::new();
    for (w, rounds) in work_rows(conn, filter, args)? {
        let g = graphs
            .entry((w.flow_version, w.graph.clone()))
            .or_insert_with(|| flows.graph(conn, w.flow_version, &w.graph));
        let open = (
            asks.get(&w.id).copied().unwrap_or(0),
            design_cards.get(&w.id).copied().unwrap_or(0),
        );
        let done = matches!(state(&w, &rounds, g.as_ref()), WorkState::Done | WorkState::Ended);
        let waits = if done {
            (None, None)
        } else {
            waits(conn, &w, &rounds, g.as_ref())?
        };
        let s = summary(&w, &rounds, g.as_ref(), open, waits);
        out.push((w, s));
    }
    Ok(out)
}

/// Every work, oldest first.
pub fn works(conn: &Connection, flows: &Flows) -> Result<Vec<WorkSummary>> {
    Ok(summaries(conn, flows, "", &[])?.into_iter().map(|(_, s)| s).collect())
}

const ROUND: &str = "SELECT r.n, r.cell, s.line, r.transition, r.rejected, r.started_at, r.ended_at, r.model,
       json_extract(r.usage, '$.cost_usd'), r.compactions,
       (SELECT count(*) FROM tool_runs t WHERE t.work = r.work AND t.round = r.n),
       json_extract(r.output, '$.progress'),
       r.work, w.flow_version, w.graph, s.cli, s.cli_session, r.config_commit, r.input, r.output, r.usage,
       json_extract(r.input, '$.engine.not_applicable')
FROM rounds r JOIN works w ON w.id = r.work LEFT JOIN sessions s ON s.id = r.session";

fn round_summary(r: &Row<'_>) -> rusqlite::Result<RoundSummary> {
    let transition: Option<String> = r.get(3)?;
    let rejected = json(r.get(4)?);
    let ended_at: Option<i64> = r.get(6)?;
    Ok(RoundSummary {
        n: r.get(0)?,
        cell: r.get(1)?,
        line: r.get(2)?,
        outcome: outcome(transition.as_deref(), rejected.as_ref(), ended_at.is_some()),
        why: why(rejected.as_ref()),
        transition,
        started_at: r.get(5)?,
        ended_at,
        model: r.get(7)?,
        cost_usd: r.get(8)?,
        compactions: r.get(9)?,
        tool_runs: r.get(10)?,
        progress: text(r, 11)?,
        not_asked: r.get(21)?,
    })
}

/// A design card, with the one that replaced it: the round that handed that one in, and that round's work.
const DESIGN_CARD: &str = "SELECT c.id, c.round, c.work, w.item, c.status, c.body, c.rulings, c.replaced_by,
       n.round, n.work, nr.cell, coalesce(nr.ended_at, nr.started_at), nw.flow_version, nw.graph, n.changed
FROM design_cards c JOIN works w ON w.id = c.work
LEFT JOIN design_cards n ON n.id = c.replaced_by LEFT JOIN rounds nr ON nr.work = n.work AND nr.n = n.round
LEFT JOIN works nw ON nw.id = n.work";

fn design_cards(conn: &Connection, flows: &Flows, filter: &str, args: &[i64]) -> Result<Vec<DesignCard>> {
    let mut st = conn.prepare(&format!("{DESIGN_CARD} {filter} ORDER BY c.id"))?;
    let rows = st.query_map(params_from_iter(args), |r| {
        let by: Option<i64> = r.get(7)?;
        let replaced = match (by, r.get::<_, Option<i64>>(8)?, r.get::<_, Option<i64>>(9)?) {
            (Some(design_card), Some(round), Some(work)) => Some(Replacement {
                design_card,
                round,
                work,
                cell: r.get(10)?,
                title: None,
                at: r.get(11)?,
                changed: r.get(14)?,
            }),
            _ => None,
        };
        let pinned = match (r.get::<_, Option<i64>>(12)?, r.get::<_, Option<String>>(13)?) {
            (Some(version), Some(graph)) => Some((version, graph)),
            _ => None,
        };
        let card = DesignCard {
            id: r.get(0)?,
            round: r.get(1)?,
            work: r.get(2)?,
            item: r.get(3)?,
            status: r.get(4)?,
            body: json(r.get(5)?).unwrap_or(Value::Null),
            rulings: json(r.get(6)?).unwrap_or(Value::Null),
            replaced_by: by,
            replaced,
        };
        Ok((card, pinned))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (mut card, pinned) = row?;
        if let (Some(x), Some((version, graph))) = (card.replaced.as_mut(), pinned) {
            let cell = x.cell.clone();
            x.title = cell.and_then(|cell| {
                let c = flows.get(conn, version).ok()?;
                Some(c.graph(&graph)?.cell(&cell)?.title.clone())
            });
        }
        out.push(card);
    }
    Ok(out)
}

const ASK: &str = "SELECT a.id, a.round, a.work, w.item, a.body, a.answer
FROM owner_asks a JOIN works w ON w.id = a.work";

/// The group of each kind of question an agent may ask, by kind, as the flow built into this program groups them: a
/// compiled flow version keeps the kinds, not their groups.
static ASK_GROUPS: LazyLock<HashMap<String, String>> = LazyLock::new(|| {
    let Ok(source) = serde_json::from_str::<flow::format::Source>(flow::FLOW) else {
        return HashMap::new();
    };
    let mut out = HashMap::new();
    for g in source.flows.iter().flat_map(|f| &f.ask_kinds) {
        for (kind, _) in &g.kinds {
            out.entry(kind.clone()).or_insert_with(|| g.group.clone());
        }
    }
    out
});

/// The kind of each point an ask is about, in order, with its group. The engine keeps what the cells handed in under
/// `asked`, each with its value; an ask written as one is its own.
fn ask_kinds(body: &Value) -> Vec<AskKind> {
    let asked: Vec<&Value> = body["asked"]
        .as_array()
        .map(|a| a.iter().map(|x| &x["value"]).filter(|v| v.is_object()).collect())
        .unwrap_or_default();
    let parts = if asked.is_empty() { vec![body] } else { asked };
    parts
        .into_iter()
        .flat_map(|p| p["points"].as_array().into_iter().flatten())
        .filter_map(|p| p["kind"].as_str())
        .map(|kind| AskKind {
            kind: kind.to_string(),
            group: ASK_GROUPS.get(kind).cloned(),
        })
        .collect()
}

fn asks(conn: &Connection, filter: &str, args: &[i64]) -> Result<Vec<OwnerAsk>> {
    let mut st = conn.prepare(&format!("{ASK} {filter} ORDER BY a.id"))?;
    let rows = st.query_map(params_from_iter(args), |r| {
        let body = json(r.get(4)?).unwrap_or(Value::Null);
        Ok(OwnerAsk {
            id: r.get(0)?,
            round: r.get(1)?,
            work: r.get(2)?,
            item: r.get(3)?,
            kinds: ask_kinds(&body),
            body,
            answer: json(r.get(5)?),
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

const MESSAGE: &str = "SELECT m.id, m.round, r.cell, s.line, r.ended_at, m.work, w.item, i.title, m.text, m.read_at
FROM owner_messages m JOIN rounds r ON r.work = m.work AND r.n = m.round JOIN works w ON w.id = m.work
LEFT JOIN sessions s ON s.id = r.session LEFT JOIN items i ON i.id = w.item";

fn messages_where(conn: &Connection, filter: &str, args: &[i64]) -> Result<Vec<OwnerMessage>> {
    let mut st = conn.prepare(&format!("{MESSAGE} {filter} ORDER BY m.id"))?;
    let rows = st.query_map(params_from_iter(args), |r| {
        Ok(OwnerMessage {
            id: r.get(0)?,
            engine: false,
            round: Some(r.get(1)?),
            cell: r.get(2)?,
            line: r.get(3)?,
            at: r.get(4)?,
            work: Some(r.get(5)?),
            item: r.get(6)?,
            title: r.get(7)?,
            text: r.get(8)?,
            read_at: r.get(9)?,
            ask: None,
            ask_kinds: Vec::new(),
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

const NOTICE: &str = "SELECT n.id, n.work, w.item, i.title, n.text, n.at, n.read_at, n.ask, a.body
FROM engine_notices n LEFT JOIN works w ON w.id = n.work LEFT JOIN items i ON i.id = w.item
LEFT JOIN owner_asks a ON a.id = n.ask";

fn notices_where(conn: &Connection, filter: &str, args: &[i64]) -> Result<Vec<OwnerMessage>> {
    let mut st = conn.prepare(&format!("{NOTICE} {filter} ORDER BY n.id"))?;
    let rows = st.query_map(params_from_iter(args), |r| {
        Ok(OwnerMessage {
            id: r.get(0)?,
            engine: true,
            round: None,
            cell: None,
            line: None,
            work: r.get(1)?,
            item: r.get(2)?,
            title: r.get(3)?,
            text: r.get(4)?,
            at: Some(r.get(5)?),
            read_at: r.get(6)?,
            ask: r.get(7)?,
            ask_kinds: json(r.get(8)?).map(|b| ask_kinds(&b)).unwrap_or_default(),
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// What agents told the owner and what the engine told the owner of its own accord, oldest first: those unread, or
/// every one.
pub fn messages(conn: &Connection, all: bool) -> Result<Vec<OwnerMessage>> {
    let mut out = messages_where(conn, if all { "" } else { "WHERE m.read_at IS NULL" }, &[])?;
    let unread = if all { "" } else { "WHERE n.read_at IS NULL" };
    out.extend(notices_where(conn, unread, &[])?);
    out.sort_by_key(|m| m.at.unwrap_or(0));
    Ok(out)
}

fn interjections(conn: &Connection, filter: &str, args: &[i64]) -> Result<Vec<Interjection>> {
    let mut st = conn.prepare(&format!(
        "SELECT id, work, line, speaker, text, delivered_round FROM interjections {filter} ORDER BY id"
    ))?;
    let rows = st.query_map(params_from_iter(args), |r| {
        Ok(Interjection {
            id: r.get(0)?,
            work: r.get(1)?,
            line: r.get(2)?,
            speaker: r.get(3)?,
            text: r.get(4)?,
            delivered_round: r.get(5)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// A work in full, with every round.
pub fn work(conn: &Connection, flows: &Flows, id: i64) -> Result<WorkDetail> {
    let (w, summary) = summaries(conn, flows, "WHERE w.id = ?1", &[id])?
        .into_iter()
        .next()
        .ok_or_else(|| Error::NotFound(format!("no work {id}")))?;
    let mut st = conn.prepare(&format!("{ROUND} WHERE r.work = ?1 ORDER BY r.n"))?;
    let rounds = st.query_map([id], round_summary)?.collect::<rusqlite::Result<_>>()?;
    let mut messages = messages_where(conn, "WHERE m.work = ?1", &[id])?;
    messages.extend(notices_where(conn, "WHERE n.work = ?1", &[id])?);
    messages.sort_by_key(|m| m.at.unwrap_or(0));
    Ok(WorkDetail {
        work: summary,
        machine: w.machine,
        worktree: w.worktree,
        pushed_head: w.pushed_head,
        resume_at: w.resume_at,
        waiting: json(w.waiting).unwrap_or(Value::Null),
        recorded: json(w.recorded).unwrap_or(Value::Null),
        rounds,
        design_cards: design_cards(conn, flows, "WHERE c.work = ?1", &[id])?,
        asks: asks(conn, "WHERE a.work = ?1", &[id])?,
        messages,
        interjections: interjections(conn, "WHERE work = ?1", &[id])?,
    })
}

/// The directories under the data directory whose files the interface serves.
const DIRS: [&str; 2] = ["logs", "tool-output"];

/// Where the interface serves a path under a logs or tool-output directory: that directory and the file's name.
fn served(path: &str) -> Option<String> {
    let p = Path::new(path);
    let name = p.file_name()?.to_str()?;
    let dir = p.parent()?.file_name()?.to_str()?;
    (DIRS.contains(&dir) && !name.starts_with('.')).then(|| format!("{dir}/{name}"))
}

/// The paths the texts in a tool's result point to under a logs or tool-output directory, such as a long output
/// saved instead of given inline.
fn pointed(v: &Value, out: &mut Vec<String>) {
    let stop = |c: char| c.is_whitespace() || matches!(c, '"' | '\'' | '`' | '(' | ')' | ',' | ';');
    match v {
        Value::String(s) => {
            for dir in DIRS {
                let needle = format!("/{dir}/");
                let mut from = 0;
                while let Some(i) = s[from..].find(&needle) {
                    let at = from + i;
                    let begin = s[..at]
                        .char_indices()
                        .rev()
                        .find(|(_, c)| stop(*c))
                        .map_or(0, |(b, c)| b + c.len_utf8());
                    let rest = &s[at + needle.len()..];
                    let end = at + needle.len() + rest.find(stop).unwrap_or(rest.len());
                    let path = s[begin..end].trim_end_matches('.');
                    if path.len() > at - begin + needle.len() && !out.iter().any(|p| p == path) {
                        out.push(path.to_string());
                    }
                    from = at + needle.len();
                }
            }
        }
        Value::Array(xs) => xs.iter().for_each(|x| pointed(x, out)),
        Value::Object(o) => o.values().for_each(|x| pointed(x, out)),
        _ => {}
    }
}

fn tool_runs(conn: &Connection, data: &Path, work: i64, n: i64) -> Result<Vec<ToolRun>> {
    let mut st = conn.prepare(
        "SELECT id, tool, args, attempt, result, files, queued_at, started_at, ended_at FROM tool_runs
         WHERE work = ?1 AND round = ?2 ORDER BY id",
    )?;
    let rows = st.query_map([work, n], |r| {
        let result = json(r.get(4)?);
        let mut paths: Vec<String> = json(r.get(5)?)
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default()
            .iter()
            .filter_map(|x| x.as_str().map(str::to_string))
            .collect();
        if let Some(v) = &result {
            pointed(v, &mut paths);
        }
        let files = paths
            .into_iter()
            .filter_map(|path| {
                let file = served(&path)?;
                data.join(&file).is_file().then_some(KeptFile { path, file })
            })
            .collect();
        Ok(ToolRun {
            id: r.get(0)?,
            tool: r.get(1)?,
            args: json(r.get(2)?).unwrap_or(Value::Null),
            attempt: r.get(3)?,
            result,
            files,
            queued_at: r.get(6)?,
            started_at: r.get(7)?,
            ended_at: r.get(8)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// A round in full, by its work and its number in the work. `data` is the data directory, under which the files its
/// tools kept are found.
pub fn round(conn: &Connection, flows: &Flows, data: &Path, work: i64, n: i64) -> Result<RoundDetail> {
    let row = conn
        .query_row(&format!("{ROUND} WHERE r.work = ?1 AND r.n = ?2"), [work, n], |r| {
            let line: Option<String> = r.get(2)?;
            let cli: Option<String> = r.get(15)?;
            let cli_session: Option<String> = r.get(16)?;
            let session = match (line, cli, cli_session) {
                (Some(line), Some(cli), Some(cli_session)) => Some(Session { line, cli, cli_session }),
                _ => None,
            };
            Ok((
                round_summary(r)?,
                r.get::<_, i64>(12)?,
                r.get::<_, i64>(13)?,
                r.get::<_, String>(14)?,
                session,
                r.get::<_, String>(17)?,
                json(r.get(18)?).unwrap_or(Value::Null),
                json(r.get(19)?),
                json(r.get(20)?),
                json(r.get(4)?),
            ))
        })
        .optional()?
        .ok_or_else(|| Error::NotFound(format!("no round R{work}-{n}")))?;
    let (summary, _, flow_version, graph, session, config_commit, input, output, usage, rejected) = row;
    // The record as the round was given it: each text it keeps once put back.
    let input = db::put_back(conn, &input);
    let cell = summary
        .cell
        .as_deref()
        .and_then(|c| flows.graph(conn, flow_version, &graph)?.cell(c).cloned());
    // An input is recorded with what the flow says it is (in a round recorded before messages were), or with its
    // sources only; either names the cell's input it is.
    let inputs = input["inputs"]
        .as_array()
        .map(|xs| {
            xs.iter()
                .map(|x| {
                    let recorded_from: Option<Vec<Src>> = serde_json::from_value(x["from"].clone()).ok();
                    let def = cell.as_ref().and_then(|c| {
                        c.inputs.iter().find(|i| match &recorded_from {
                            Some(f) => i.from == *f,
                            None => x["what"].as_str() == Some(i.what.as_str()),
                        })
                    });
                    RoundInput {
                        value: x.get("value").cloned().unwrap_or(Value::Null),
                        what: x["what"]
                            .as_str()
                            .map(str::to_string)
                            .or_else(|| def.map(|i| i.what.clone()))
                            .unwrap_or_default(),
                        from: recorded_from
                            .or_else(|| def.map(|i| i.from.clone()))
                            .unwrap_or_default(),
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    // A round recorded before the conditional inputs were called so keeps them under `materials`.
    let conditional = input
        .get("conditional")
        .or_else(|| input.get("materials"))
        .unwrap_or(&Value::Null)
        .as_array()
        .map(|xs| {
            xs.iter()
                .map(|m| RoundConditional {
                    address: m["address"].as_str().unwrap_or_default().to_string(),
                    hash: m["hash"].as_str().unwrap_or_default().to_string(),
                    given: m["given"].as_bool().unwrap_or(false),
                })
                .collect()
        })
        .unwrap_or_default();
    let call = input.get("call").cloned();
    let effort = call.as_ref().and_then(|c| flag(c, "--effort"));
    let level = call.as_ref().and_then(|c| {
        c["level"].as_str().map(str::to_string).or_else(|| {
            let model = summary.model.clone().or_else(|| flag(c, "--model"))?;
            let cfg = config_at(data, &config_commit)?;
            level_of(&cfg.install.agents.levels, &model, effort.as_deref())
        })
    });
    let project: Option<String> = conn
        .query_row(
            "SELECT i.project FROM works w JOIN items i ON i.id = w.item WHERE w.id = ?1",
            [work],
            |r| r.get(0),
        )
        .optional()?;
    let limits = project
        .and_then(|p| Some(config_at(data, &config_commit)?.projects.get(&p)?.limits.clone()))
        .unwrap_or_default();
    Ok(RoundDetail {
        work,
        flow_version,
        graph,
        session,
        level,
        effort,
        acceptance_before: acceptance_before(conn, work, n)?,
        limits,
        config_commit,
        message: input["message"]
            .as_str()
            .or(input["prompt"].as_str())
            .map(str::to_string),
        conditional,
        call,
        inputs,
        input,
        output,
        rejected,
        usage,
        tool_runs: tool_runs(conn, data, work, n)?,
        design_cards: design_cards(conn, flows, "WHERE c.work = ?1 AND c.round = ?2", &[work, n])?,
        asks: asks(conn, "WHERE a.work = ?1 AND a.round = ?2", &[work, n])?,
        interjections: interjections(conn, "WHERE work = ?1 AND delivered_round = ?2", &[work, n])?,
        round: summary,
    })
}

/// The word a command line gives after a flag, such as the model after `--model`.
fn flag(call: &Value, name: &str) -> Option<String> {
    let args = call["command"].as_array()?;
    let i = args.iter().position(|a| a.as_str() == Some(name))?;
    args.get(i + 1)?.as_str().map(str::to_string)
}

/// The configuration a round ran with, as the commit of the data directory it names has it; each commit is read once,
/// since a commit never changes.
fn config_at(data: &Path, commit: &str) -> Option<Arc<config::Config>> {
    type Read = BTreeMap<(std::path::PathBuf, String), Option<Arc<config::Config>>>;
    static READ: LazyLock<Mutex<Read>> = LazyLock::new(Mutex::default);
    let key = (data.to_path_buf(), commit.to_string());
    if let Some(c) = READ.lock().unwrap_or_else(|e| e.into_inner()).get(&key) {
        return c.clone();
    }
    let c = config::at(data, commit).ok().map(Arc::new);
    READ.lock().unwrap_or_else(|e| e.into_inner()).insert(key, c.clone());
    c
}

/// The one level of the configuration that runs a model with an effort; none when no level does, or several do.
fn level_of(levels: &BTreeMap<String, config::Level>, model: &str, effort: Option<&str>) -> Option<String> {
    let mut hits = levels
        .iter()
        .filter(|(_, l)| l.model == model && l.effort.as_deref() == effort);
    match (hits.next(), hits.next()) {
        (Some((name, _)), None) => Some(name.clone()),
        _ => None,
    }
}

/// The acceptance a round's lock replaced, as the record of changes keeps the item before it; none when the round
/// locked none, or locked the first. The record keeps an item as it was written: one written before rounds were named
/// by their work names the round that locked it by a number no round has now, and has no work for it, so that lock is
/// found as the change of the round's work's item to another round's acceptance while the round ran, its lock being
/// written in the transaction that ended it.
fn acceptance_before(conn: &Connection, work: i64, n: i64) -> Result<Option<Value>> {
    let before: Option<Option<String>> = conn
        .query_row(
            "SELECT json_extract(c.old_row, '$.acceptance')
             FROM changes c JOIN rounds r ON r.work = ?1 AND r.n = ?2 JOIN works w ON w.id = r.work
             WHERE c.table_name = 'items' AND json_extract(c.row_key, '$.id') = w.item
               AND ((json_extract(c.new_row, '$.acceptance_work') = ?1
                     AND json_extract(c.new_row, '$.acceptance_round') = ?2
                     AND NOT (json_extract(c.old_row, '$.acceptance_work') IS ?1
                              AND json_extract(c.old_row, '$.acceptance_round') IS ?2))
                    OR (json_type(c.new_row, '$.acceptance_work') IS NULL
                        AND c.at BETWEEN r.started_at AND r.ended_at
                        AND json_extract(c.new_row, '$.acceptance_round')
                            IS NOT json_extract(c.old_row, '$.acceptance_round')))
             ORDER BY c.id LIMIT 1",
            [work, n],
            |r| r.get(0),
        )
        .optional()?;
    Ok(before.flatten().and_then(|t| serde_json::from_str(&t).ok()))
}

/// Every stored flow version, newest first.
pub fn flows(conn: &Connection) -> Result<Vec<FlowVersion>> {
    let mut st = conn.prepare(
        "SELECT v.id, v.digest, (SELECT count(*) FROM works w WHERE w.flow_version = v.id) FROM flow_versions v
         ORDER BY v.id DESC",
    )?;
    let rows = st.query_map([], |r| {
        Ok(FlowVersion {
            id: r.get(0)?,
            digest: r.get(1)?,
            works: r.get(2)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// A flow version's compiled flow, as the database keeps it: the JSON of [`Compiled`].
pub fn compiled_flow(conn: &Connection, version: i64) -> Result<String> {
    conn.query_row("SELECT compiled FROM flow_versions WHERE id = ?1", [version], |r| {
        r.get(0)
    })
    .optional()?
    .ok_or_else(|| Error::NotFound(format!("no flow version {version}")))
}

/// What the flow page shows of a stored flow version besides the flow itself.
pub fn flow_view(conn: &Connection, flows: &Flows, version: i64) -> Result<FlowView> {
    let c = flows.get(conn, version)?;
    Ok(view::view(&c))
}

/// The stored flow version a compiled flow is, by its digest.
pub fn version_of(conn: &Connection, digest: &str) -> Result<Option<i64>> {
    Ok(conn
        .query_row("SELECT id FROM flow_versions WHERE digest = ?1", [digest], |r| r.get(0))
        .optional()?)
}

fn item_state(cancelled: bool, queued: bool, work: Option<&WorkSummary>) -> ItemState {
    match work {
        _ if cancelled => ItemState::Cancelled,
        Some(w) if w.state == WorkState::Done => ItemState::Done,
        // An item whose work stopped and that waits in the queue for a new work, such as one taking over after a new
        // flow version, is queued.
        Some(w) if w.state == WorkState::Ended && queued => ItemState::Queued,
        Some(w) if w.state == WorkState::Ended => ItemState::Stopped,
        Some(_) => ItemState::Working,
        None if queued => ItemState::Queued,
        None => ItemState::New,
    }
}

/// Every item and every dependency, and every work.
fn all_items(conn: &Connection, flows: &Flows) -> Result<(Items, Vec<WorkSummary>)> {
    let works = works(conn, flows)?;
    let mut latest: HashMap<i64, &WorkSummary> = HashMap::new();
    // The item's pull request is that of its latest work that has one: a new work takes over the PR of the work before
    // it before it records it as its own.
    let mut prs: HashMap<i64, i64> = HashMap::new();
    for w in &works {
        if let Some(i) = w.item {
            latest.insert(i, w);
            if let Some(pr) = w.pr {
                prs.insert(i, pr);
            }
        }
    }
    // The stages of each graph a latest work runs, in their order: the order of the graph's cells, each stage where its
    // first cell is.
    let mut stages: HashMap<(i64, String), Vec<String>> = HashMap::new();
    for w in latest.values() {
        stages.entry((w.flow_version, w.graph.clone())).or_insert_with(|| {
            let Ok(c) = flows.get(conn, w.flow_version) else {
                return Vec::new();
            };
            let mut out: Vec<String> = Vec::new();
            for s in c
                .graph(&w.graph)
                .into_iter()
                .flat_map(|g| &g.cells)
                .filter_map(|x| x.stage.as_ref())
            {
                if !out.contains(s) {
                    out.push(s.clone());
                }
            }
            out
        });
    }
    let mut st = conn.prepare(
        "SELECT i.id, i.project, i.title, i.deliver, i.level, i.cancelled, q.position, i.imported_pr
         FROM items i LEFT JOIN dispatch_queue q ON q.item = i.id ORDER BY i.id",
    )?;
    let items = st
        .query_map([], |r| {
            let id: i64 = r.get(0)?;
            let cancelled: Option<String> = r.get(5)?;
            let queued: Option<f64> = r.get(6)?;
            let imported_pr: Option<i64> = r.get(7)?;
            let w = latest.get(&id).copied();
            Ok(ItemSummary {
                id,
                project: r.get(1)?,
                title: r.get(2)?,
                deliver: r.get(3)?,
                level: r.get(4)?,
                state: item_state(cancelled.is_some(), queued.is_some(), w),
                cancelled,
                queued,
                work: w.map(|w| w.id),
                at: w.map(|w| w.at.clone()),
                stage: w.and_then(|w| w.stage.clone()),
                stages: w
                    .and_then(|w| stages.get(&(w.flow_version, w.graph.clone())))
                    .cloned()
                    .unwrap_or_default(),
                open_asks: w.map_or(0, |w| w.open_asks),
                open_design_cards: w.map_or(0, |w| w.open_design_cards),
                pr: prs.get(&id).copied().or(imported_pr),
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    let mut st = conn.prepare("SELECT item, depends_on, needs FROM item_links ORDER BY item, depends_on")?;
    let links = st
        .query_map([], |r| {
            Ok(ItemLink {
                item: r.get(0)?,
                depends_on: r.get(1)?,
                needs: r.get(2)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok((Items { items, links }, works))
}

/// Every item, and the dependencies between them.
pub fn items(conn: &Connection, flows: &Flows) -> Result<Items> {
    Ok(all_items(conn, flows)?.0)
}

/// An item in full.
pub fn item(conn: &Connection, flows: &Flows, id: i64) -> Result<ItemDetail> {
    let (all, works) = all_items(conn, flows)?;
    let by_id: HashMap<i64, &ItemSummary> = all.items.iter().map(|i| (i.id, i)).collect();
    let summary = by_id
        .get(&id)
        .map(|i| (*i).clone())
        .ok_or_else(|| Error::NotFound(format!("no item {id}")))?;
    let (problem, why, approach, acceptance, acceptance_work, acceptance_round) = conn.query_row(
        "SELECT problem, why, approach, acceptance, acceptance_work, acceptance_round FROM items WHERE id = ?1",
        [id],
        |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                json(r.get(3)?),
                r.get::<_, Option<i64>>(4)?,
                r.get::<_, Option<i64>>(5)?,
            ))
        },
    )?;
    let (imported_from, imported_branch, imported_pr) = conn.query_row(
        "SELECT imported_from, imported_branch, imported_pr FROM items WHERE id = ?1",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    let linked = |other: i64, needs: &str| {
        by_id.get(&other).map(|i| LinkedItem {
            item: (*i).clone(),
            needs: needs.to_string(),
        })
    };
    Ok(ItemDetail {
        problem,
        why,
        approach,
        acceptance,
        acceptance_work,
        acceptance_round,
        depends_on: all
            .links
            .iter()
            .filter(|l| l.item == id)
            .filter_map(|l| linked(l.depends_on, &l.needs))
            .collect(),
        depended_on_by: all
            .links
            .iter()
            .filter(|l| l.depends_on == id)
            .filter_map(|l| linked(l.item, &l.needs))
            .collect(),
        works: works.into_iter().filter(|w| w.item == Some(id)).collect(),
        imported_from,
        imported_branch,
        imported_pr,
        design_cards: design_cards(conn, flows, "WHERE w.item = ?1", &[id])?,
        asks: asks(conn, "WHERE w.item = ?1", &[id])?,
        shown: shown(conn, flows, id)?,
        item: summary,
    })
}

/// The values at a path of a return, as an input gives them: a shape named in brackets keeps only what is of that
/// shape, and the values at a path inside a list are given as a list. None when the return has nothing there.
fn value_at(x: &Value, path: &str) -> Option<Value> {
    let of_shape = |v: &Value, b: &str| v.as_object().is_some_and(|o| o.values().any(|x| x == b));
    let mut cur = vec![x.clone()];
    for seg in path.split('.') {
        let (name, shape) = match seg.split_once('[') {
            Some((n, rest)) => (n, Some(rest.trim_end_matches(']'))),
            None => (seg, None),
        };
        let mut next = Vec::new();
        for c in cur {
            match c {
                Value::Array(xs) => next.extend(xs.iter().filter_map(|it| it.get(name).cloned())),
                o => next.extend(o.get(name).cloned()),
            }
        }
        if let Some(b) = shape {
            next = next
                .into_iter()
                .filter_map(|v| match v {
                    Value::Array(xs) => {
                        let keep: Vec<Value> = xs.into_iter().filter(|it| of_shape(it, b)).collect();
                        (!keep.is_empty()).then_some(Value::Array(keep))
                    }
                    o => of_shape(&o, b).then_some(o),
                })
                .collect();
        }
        cur = next;
    }
    match cur.len() {
        0 => None,
        1 => cur.pop(),
        _ => Some(Value::Array(cur)),
    }
}

/// What the rounds of an item's works handed back that their flow versions show on the item, oldest first.
fn shown(conn: &Connection, flows: &Flows, item: i64) -> Result<Vec<Shown>> {
    let mut st = conn.prepare(
        "SELECT r.n, r.work, r.cell, r.output, w.flow_version, w.graph FROM rounds r JOIN works w ON w.id = r.work
         WHERE w.item = ?1 AND r.cell IS NOT NULL AND r.output IS NOT NULL AND r.transition IS NOT NULL
         ORDER BY r.work, r.n",
    )?;
    let rows = st.query_map([item], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, String>(2)?,
            text(r, 3)?,
            r.get::<_, i64>(4)?,
            r.get::<_, String>(5)?,
        ))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (round, work, cell, output, version, graph) = row?;
        let Some(g) = flows.graph(conn, version, &graph) else {
            continue;
        };
        let Some(c) = g.cell(&cell) else { continue };
        if c.shown.is_empty() {
            continue;
        }
        let Some(ret) = json(output) else { continue };
        let ret = if c.kind == "agent" { g.unwrap(&ret) } else { ret };
        for path in &c.shown {
            if let Some(value) = value_at(&ret, path) {
                out.push(Shown {
                    work,
                    round,
                    cell: cell.clone(),
                    path: path.clone(),
                    value,
                });
            }
        }
    }
    Ok(out)
}

/// A file under the data directory's `logs` or `tool-output` directory, as text, cut after `max` bytes.
pub fn file(data: &Path, dir: &str, name: &str, max: u64) -> Result<String> {
    let missing = || Error::NotFound(format!("no file {dir}/{name}"));
    if !DIRS.contains(&dir) || name.is_empty() || name.starts_with('.') || name.contains(['/', '\\']) {
        return Err(missing());
    }
    let path = data.join(dir).join(name);
    // Only a plain file, not a link that leads out of the directory.
    if !std::fs::symlink_metadata(&path).is_ok_and(|m| m.is_file()) {
        return Err(missing());
    }
    let failed = |e: std::io::Error| Error::Failed(format!("{}: {e}", path.display()));
    let f = std::fs::File::open(&path).map_err(failed)?;
    let len = f.metadata().map_err(failed)?.len();
    let mut bytes = Vec::new();
    f.take(max).read_to_end(&mut bytes).map_err(failed)?;
    let mut out = String::from_utf8_lossy(&bytes).into_owned();
    if len > max {
        out.push_str(&format!("\n[cut after {max} of {len} bytes]\n"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use db::{Actor, Db};
    use serde_json::json;

    use super::*;

    struct Setup {
        db: Db,
        compiled: Compiled,
        data: std::path::PathBuf,
    }

    fn setup(name: &str) -> Setup {
        let relations = db::relations(db::SCHEMA).unwrap();
        let compiled = flow::compile(
            flow::FLOW,
            &flow::Context {
                relations,
                tables: db::index_of(db::SCHEMA).unwrap(),
            },
        )
        .unwrap();
        let first = compiled.graph("engineer").unwrap().cell("1").unwrap().inputs[0]
            .what
            .clone();
        let data = std::env::temp_dir().join(format!("wf-agents-api-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&data);
        for (dir, file, text) in [("logs", "run-1.log", "red"), ("tool-output", "R1-2-1.txt", "long")] {
            std::fs::create_dir_all(data.join(dir)).unwrap();
            std::fs::write(data.join(dir).join(file), text).unwrap();
        }
        let input =
            json!({ "prompt": "Do it", "inputs": [{ "what": first, "value": "v", "held": false }], "engine": {} });
        let result = json!([
            0,
            format!("Too long to give here; saved to {}/tool-output/R1-2-1.txt", "/old")
        ]);
        let mut db = Db::in_memory().unwrap();
        let json = compiled.json();
        db.write(Actor::Engine, |tx| {
            let run = |sql: &str, args: &[&dyn db::rusqlite::ToSql]| tx.execute(sql, args).map(|_| ());
            run("INSERT INTO flow_versions (digest, compiled) VALUES ('d', ?1)", &[&json])?;
            for title in ["First", "Second"] {
                run(
                    "INSERT INTO items (project, title, problem, why, deliver, level) VALUES ('p', ?1, 'x', 'y', 'change', '1')",
                    &[&title],
                )?;
            }
            run("INSERT INTO item_links (item, depends_on, needs) VALUES (1, 2, 'its schema')", &[])?;
            run("INSERT INTO dispatch_queue (item, position) VALUES (2, 1.0)", &[])?;
            run("INSERT INTO works (item, flow_version, graph, at) VALUES (1, 1, 'engineer', '1')", &[])?;
            run("INSERT INTO sessions (work, line, cli, cli_session) VALUES (1, 'engineer', 'claude', 's1')", &[])?;
            run(
                "INSERT INTO rounds (work, n, cell, input, output, transition, config_commit, started_at, ended_at)
                 VALUES (1, 1, '0', '{}', '{}', 'started', 'c', 10, 20)",
                &[],
            )?;
            let input = input.to_string();
            run(
                "INSERT INTO rounds (work, n, cell, session, input, output, rejected, config_commit, model, usage, started_at, ended_at)
                 VALUES (1, 2, '1', 1, ?1, '{\"progress\":\"looked\"}', '{\"format\":\"no outcome\"}', 'c', 'm', '{\"cost_usd\":0.5}', 30, 40)",
                &[&input],
            )?;
            run(
                "INSERT INTO rounds (work, n, cell, session, input, config_commit, started_at) VALUES (1, 3, '1', 1, ?1, 'c', 50)",
                &[&input],
            )?;
            let result = result.to_string();
            run(
                "INSERT INTO tool_runs (work, round, tool, args, attempt, result, files, queued_at)
                 VALUES (1, 2, 'gh', '{}', 1, ?1, '[\"/old/logs/run-1.log\", \"/old/logs/gone.log\"]', 35)",
                &[&result],
            )?;
            Ok(())
        })
        .unwrap();
        Setup { db, compiled, data }
    }

    #[test]
    fn a_work_says_where_it_stands_and_what_each_round_came_to() {
        let mut s = setup("work");
        s.db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO owner_messages (work, round, text) VALUES (1, 2, 'told')",
                [],
            )?;
            tx.execute(
                "INSERT INTO engine_notices (work, text, at) VALUES (1, 'Work 1 waits.', 45), (NULL, 'Halted.', 46)",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        let cache = Flows::default();
        let cell = s.compiled.graph("engineer").unwrap().cell("1").unwrap().clone();
        let ws = works(s.db.conn(), &cache).unwrap();
        assert_eq!(ws.len(), 1);
        let w = &ws[0];
        assert_eq!(w.state, WorkState::Running);
        assert_eq!(w.at_title.as_deref(), Some(cell.title.as_str()));
        assert_eq!(w.stage, cell.stage);
        assert_eq!((w.rounds, w.sent_back, w.failed), (3, 1, 0));
        assert_eq!((w.started_at, w.last_at, w.cost_usd), (Some(10), Some(50), 0.5));
        assert_eq!(w.running.as_ref().map(|r| r.n), Some(3));

        let d = work(s.db.conn(), &cache, 1).unwrap();
        let outcomes: Vec<RoundOutcome> = d.rounds.iter().map(|r| r.outcome).collect();
        assert_eq!(
            outcomes,
            [RoundOutcome::Took, RoundOutcome::SentBack, RoundOutcome::Running]
        );
        let back = &d.rounds[1];
        assert_eq!(back.why.as_deref(), Some("no outcome"));
        assert_eq!(back.line.as_deref(), Some("engineer"));
        assert_eq!(back.progress.as_deref(), Some("looked"));
        assert_eq!((back.cost_usd, back.tool_runs), (Some(0.5), 1));
        // What its agents and the engine told the owner of it, oldest first; a notice of no work is not its.
        let told: Vec<(bool, &str)> = d.messages.iter().map(|m| (m.engine, m.text.as_str())).collect();
        assert_eq!(told, [(false, "told"), (true, "Work 1 waits.")]);
        assert_eq!(work(s.db.conn(), &cache, 9), Err(Error::NotFound("no work 9".into())));

        // A round recorded before messages were gives its prompt as its message, and its inputs.
        let r = round(s.db.conn(), &cache, &s.data, 1, 2).unwrap();
        assert_eq!(r.message.as_deref(), Some("Do it"));
        assert_eq!(r.inputs[0].from, cell.inputs[0].from);
        assert!(r.conditional.is_empty() && r.call.is_none());
        assert_eq!(r.session.as_ref().map(|x| x.cli_session.as_str()), Some("s1"));
        let files: Vec<&str> = r.tool_runs[0].files.iter().map(|f| f.file.as_str()).collect();
        assert_eq!(files, ["logs/run-1.log", "tool-output/R1-2-1.txt"]);
        assert_eq!(file(&s.data, "tool-output", "R1-2-1.txt", 100).unwrap(), "long");
        assert_eq!(
            round(s.db.conn(), &cache, &s.data, 2, 2),
            Err(Error::NotFound("no round R2-2".into()))
        );
        assert!(
            file(&s.data, "logs", "run-1.log", 2)
                .unwrap()
                .starts_with("re\n[cut after 2 of 3 bytes]")
        );
        assert!(matches!(
            file(&s.data, "logs", "../logs/run-1.log", 100),
            Err(Error::NotFound(_))
        ));
        assert!(matches!(file(&s.data, "db", "x", 100), Err(Error::NotFound(_))));
    }

    // What a work waits for comes from what is open, never from the kind of cell it is at: at a cell that asks, the
    // ask still unanswered, and once answered the work is ready and says the answer is in; at a row that waits, the
    // items its item depends on. A paused work says both who paused it and what it waits for.
    #[test]
    fn a_work_says_what_it_waits_for_from_what_is_open() {
        let mut s = setup("waits");
        let g = s.compiled.graph("engineer").unwrap();
        let asking = g.cells.iter().find(|c| is_ask(c)).unwrap().id.clone();
        let (waiting, row) = g
            .cells
            .iter()
            .find_map(|c| c.rows.iter().find(|r| r.waits).map(|r| (c.id.clone(), r.name.clone())))
            .unwrap();
        s.db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO works (item, flow_version, graph, at) VALUES (1, 1, 'engineer', ?1), (1, 1, 'engineer', ?2)",
                [&asking, &waiting],
            )?;
            tx.execute(
                "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES (2, 1, ?1, '{}', 'c', 60)",
                [&asking],
            )?;
            tx.execute("INSERT INTO owner_asks (work, round, body) VALUES (2, 1, '{\"to\": \"owner\"}')", [])?;
            tx.execute(
                "INSERT INTO rounds (work, n, cell, input, output, transition, config_commit, started_at, ended_at)
                 VALUES (3, 1, ?1, '{}', '{}', ?2, 'c', 60, 70)",
                [&waiting, &row],
            )?;
            tx.execute("UPDATE works SET paused = 'later', paused_by = 'owner' WHERE id = 3", [])?;
            Ok(())
        })
        .unwrap();
        let cache = Flows::default();
        let ws = works(s.db.conn(), &cache).unwrap();
        assert_eq!(
            (ws[1].state, &ws[1].waits, ws[1].answered),
            (WorkState::Asking, &Some(Waits::Answer { ask: 1 }), None)
        );
        assert_eq!(
            (ws[2].state, ws[2].paused_by, &ws[2].waits),
            (
                WorkState::Paused,
                Some(PausedBy::Owner),
                &Some(Waits::Items { items: vec![2] })
            )
        );
        s.db.write(Actor::Owner, |tx| {
            tx.execute("UPDATE owner_asks SET answer = '{\"text\": \"yes\"}' WHERE id = 1", [])?;
            Ok(())
        })
        .unwrap();
        let ws = works(s.db.conn(), &cache).unwrap();
        assert_eq!(
            (ws[1].state, &ws[1].waits, ws[1].answered),
            (WorkState::Ready, &None, Some(1))
        );
    }

    // A round recorded with its message gives each conditional input, given or referred to, and the call; a step's inputs,
    // recorded by their sources, are named by the cell's definition; a failed run says its kind.
    #[test]
    fn a_round_gives_its_message_conditional_inputs_and_call() {
        let mut s = setup("message");
        let cell = s.compiled.graph("engineer").unwrap().cell("1").unwrap().clone();
        let input = json!({
            "message": "# R1-4 · cell 1 · Research",
            "conditional": [
                { "address": "rule:ground", "hash": "h1", "given": true },
                { "address": "R1-2·1·understanding", "hash": "h2", "given": false },
            ],
            "call": {
                "command": ["claude", "-p", "--effort", "high"],
                "session": { "resumed": "s1" },
                "engine": "c",
                "cli": "2",
                "level": "3",
            },
            "engine": {},
        });
        let step = json!({ "inputs": [{ "from": cell.inputs[0].from, "value": "v" }], "given": {} });
        let failed = json!({ "failed": { "kind": "transient", "what": "API Error: 529 Overloaded" } });
        s.db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO rounds (work, n, cell, session, input, rejected, config_commit, started_at, ended_at)
                 VALUES (1, 4, '1', 1, ?1, ?2, 'c', 60, 70)",
                [input.to_string(), failed.to_string()],
            )?;
            tx.execute(
                "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES (1, 5, '1', ?1, 'c', 80)",
                [step.to_string()],
            )?;
            // R1-2 locked the first acceptance, and R1-4 a new one in its place.
            for (acceptance, round) in [(r#"[{"id":"A1"}]"#, 2), (r#"[{"id":"A2"}]"#, 4)] {
                tx.execute(
                    "UPDATE items SET acceptance = ?1, acceptance_work = 1, acceptance_round = ?2 WHERE id = 1",
                    db::rusqlite::params![acceptance, round],
                )?;
            }
            Ok(())
        })
        .unwrap();
        let cache = Flows::default();
        let r = round(s.db.conn(), &cache, &s.data, 1, 4).unwrap();
        assert_eq!(r.message.as_deref(), Some("# R1-4 · cell 1 · Research"));
        assert_eq!(
            r.conditional
                .iter()
                .map(|m| (m.address.as_str(), m.given))
                .collect::<Vec<_>>(),
            [("rule:ground", true), ("R1-2·1·understanding", false)]
        );
        assert_eq!(r.call.as_ref().unwrap()["session"]["resumed"], "s1");
        assert_eq!((r.level.as_deref(), r.effort.as_deref()), (Some("3"), Some("high")));
        assert_eq!(r.acceptance_before, Some(json!([{ "id": "A1" }])));
        assert_eq!(r.round.outcome, RoundOutcome::Failed);
        assert_eq!(r.round.why.as_deref(), Some("transient: API Error: 529 Overloaded"));
        let step = round(s.db.conn(), &cache, &s.data, 1, 5).unwrap();
        assert_eq!(step.message, None);
        assert_eq!((step.level, step.acceptance_before), (None, None));
        assert_eq!(step.inputs[0].what, cell.inputs[0].what);
        assert_eq!(step.inputs[0].from, cell.inputs[0].from);
        // A lock recorded before rounds were named by their work names its round by its old number: it is found by
        // when the round ran.
        s.db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO rounds (work, n, cell, input, config_commit, started_at, ended_at)
                 VALUES (1, 6, '12', '{}', 'c', 100, 110)",
                [],
            )?;
            tx.execute(
                "INSERT INTO changes (tx, at, actor, table_name, row_key, old_row, new_row)
                 VALUES (99, 105, 'engine', 'items', '{\"id\":1}', ?1, ?2)",
                [
                    r#"{"acceptance":[{"id":"A0"}],"acceptance_round":7}"#,
                    r#"{"acceptance":[{"id":"A9"}],"acceptance_round":9}"#,
                ],
            )?;
            Ok(())
        })
        .unwrap();
        let old = round(s.db.conn(), &cache, &s.data, 1, 6).unwrap();
        assert_eq!(old.acceptance_before, Some(json!([{ "id": "A0" }])));
        let open = round(s.db.conn(), &cache, &s.data, 1, 5).unwrap();
        assert_eq!(open.acceptance_before, None);
    }

    // A round recorded before its level was finds it as the one level that runs its model with its effort.
    #[test]
    fn an_older_round_finds_its_level_by_its_model_and_effort() {
        let level = |model: &str, effort: Option<&str>| config::Level {
            desc: String::new(),
            cli: "claude".into(),
            model: model.into(),
            effort: effort.map(str::to_string),
        };
        let levels = BTreeMap::from([
            ("2".to_string(), level("m", Some("high"))),
            ("3".to_string(), level("n", Some("high"))),
            ("4".to_string(), level("n", Some("high"))),
        ]);
        assert_eq!(level_of(&levels, "m", Some("high")).as_deref(), Some("2"));
        assert_eq!(level_of(&levels, "m", None), None);
        assert_eq!(level_of(&levels, "n", Some("high")), None);
    }

    #[test]
    fn items_say_where_they_stand_and_what_they_depend_on() {
        let s = setup("items");
        let cache = Flows::default();
        let all = items(s.db.conn(), &cache).unwrap();
        let states: Vec<ItemState> = all.items.iter().map(|i| i.state).collect();
        assert_eq!(states, [ItemState::Working, ItemState::Queued]);
        assert_eq!(all.items[0].at.as_deref(), Some("1"));
        assert_eq!(all.links.len(), 1);
        let first = item(s.db.conn(), &cache, 1).unwrap();
        assert_eq!(first.depends_on[0].item.id, 2);
        assert_eq!(first.depends_on[0].needs, "its schema");
        assert_eq!(first.works.len(), 1);
        let second = item(s.db.conn(), &cache, 2).unwrap();
        assert_eq!(second.depended_on_by[0].item.id, 1);
        assert!(second.works.is_empty());
        assert_eq!(flows(s.db.conn()).unwrap()[0].works, 1);
        // The stage its work is at, among the stages of its graph in their order; nothing for an item never worked on.
        assert_eq!(all.items[0].stage.as_deref(), Some("research"));
        assert_eq!(all.items[0].stages[..2], ["queued", "research"]);
        assert!(all.items[1].stage.is_none() && all.items[1].stages.is_empty());
        assert_eq!((all.items[0].open_asks, all.items[0].open_design_cards), (0, 0));
    }

    // A replaced design card names the one that replaced it, with the round that handed that one in, its cell and
    // when, and what that one's agent said changed and why.
    #[test]
    fn a_replaced_design_card_says_what_replaced_it() {
        let mut s = setup("replaced");
        s.db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO design_cards (work, round, body, status) VALUES (1, 2, '[]', 'open')",
                [],
            )?;
            tx.execute(
                "INSERT INTO design_cards (work, round, body, status, changed) VALUES (1, 3, '[]', 'open', 'Kept one table.')",
                [],
            )?;
            tx.execute(
                "UPDATE design_cards SET status = 'replaced', replaced_by = 2 WHERE id = 1",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        let title = s.compiled.graph("engineer").unwrap().cell("1").unwrap().title.clone();
        let d = item(s.db.conn(), &Flows::default(), 1).unwrap();
        assert_eq!(
            d.design_cards[0].replaced,
            Some(Replacement {
                design_card: 2,
                round: 3,
                work: 1,
                cell: Some("1".into()),
                title: Some(title),
                at: Some(50),
                changed: Some("Kept one table.".into()),
            })
        );
        assert_eq!(d.design_cards[1].replaced, None);
        assert_eq!(
            item(s.db.conn(), &Flows::default(), 1).unwrap().item.open_design_cards,
            1
        );
    }

    // An ask's kinds come with the group the flow puts each in; a kind the flow does not have has none.
    #[test]
    fn an_ask_says_the_group_of_each_kind_it_asks_about() {
        let held = json!({ "asked": [{ "what": "w", "value": { "to": "owner", "points": [
            { "kind": "blocked_by_permission", "what": "x" },
            { "kind": "no_such_kind", "what": "y" },
        ] } }] });
        let kind = |kind: &str, group: Option<&str>| AskKind {
            kind: kind.into(),
            group: group.map(Into::into),
        };
        assert_eq!(
            ask_kinds(&held),
            [
                kind("blocked_by_permission", Some("environment")),
                kind("no_such_kind", None)
            ]
        );
        let own = json!({ "to": "owner", "points": [{ "kind": "split", "what": "z" }] });
        assert_eq!(ask_kinds(&own), [kind("split", Some("technical"))]);
    }

    #[test]
    fn an_item_shows_what_the_flow_marks_shown_of_its_rounds() {
        let mut s = setup("shown");
        let ret = json!({
            "outcome": { "understanding": { "problem": "what is wrong" } },
            "progress": "read the code",
            "side_reports": []
        })
        .to_string();
        s.db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO rounds (work, n, cell, session, input, output, transition, config_commit, started_at, ended_at)
                 VALUES (1, 4, '1', 1, '{}', ?1, 'done', 'c', 90, 91)",
                [&ret],
            )?;
            Ok(())
        })
        .unwrap();
        let d = item(s.db.conn(), &Flows::default(), 1).unwrap();
        let at = |path: &str| {
            d.shown
                .iter()
                .find(|x| x.cell == "1" && x.path == path)
                .map(|x| x.value.clone())
        };
        assert_eq!(at("understanding"), Some(json!({ "problem": "what is wrong" })));
        assert_eq!(at("progress"), Some(json!("read the code")));
        // The start's return holds nothing the flow shows.
        assert!(d.shown.iter().all(|x| x.cell == "1"));
    }
}
