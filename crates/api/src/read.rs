//! The reads that fill the interface's types from the database. Where a work stands is derived here as the engine
//! derives it (`Engine::done` and `Engine::waiting`): from the work's row, its rounds and its flow version.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;
use std::io::Read as _;
use std::path::Path;
use std::sync::{Arc, LazyLock, Mutex, MutexGuard};

use db::rusqlite::types::ValueRef;
use db::rusqlite::{self, Connection, OptionalExtension, Row, params, params_from_iter};
use flow::format::Src;
use flow::{Compiled, CompiledCell, CompiledGraph};
use serde_json::Value;

use crate::view;
use crate::{
    Architect, AskKind, DesignCard, DispatchHold, Draft, FlowVersion, FlowView, Handled, HandledAction, InputsNow,
    Interjection, ItemChange, ItemDetail, ItemLink, ItemState, ItemSummary, Items, KeptFile, LevelChange, LinkedItem,
    Matter, MatterCard, NamedText, NextPlan, Observation, OwnerAsk, OwnerMessage, PausedBy, Ping, PolicyInForce,
    PolicySetting, ProjectNow, Queued, Reading, Replacement, RoundConditional, RoundDetail, RoundInput, RoundOutcome,
    RoundSummary, RoundTokens, RunningRound, Said, Session, Shown, Tag, ToolRun, Waits, Wake, WorkDetail, WorkSession,
    WorkState, WorkSummary,
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
    pub(crate) fn graph(&self, conn: &Connection, version: i64, graph: &str) -> Option<CompiledGraph> {
        self.get(conn, version).ok()?.graph(graph).cloned()
    }
}

pub(crate) fn json(s: Option<String>) -> Option<Value> {
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
pub(crate) fn outcome(transition: Option<&str>, rejected: Option<&Value>, ended: bool) -> RoundOutcome {
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
        "SELECT w.id, w.item, i.title, coalesce(i.project, w.project), w.flow_version, w.graph, w.at, w.resume_at,
                w.waiting, w.recorded, w.machine, w.worktree, w.branch, w.pr, w.pushed_head, w.paused, w.ended,
                w.paused_by
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
/// at: first from its open blocker, by who holds it: the owner's answer to the ask that stands for it, the reply of
/// the architect that holds it (of the work's project or a related one, which is named) to its matter, the items it
/// waits for, or the owner's ruling on its design card. A work of a flow version before blockers is read as then: the
/// owner's answer to the ask it put, at a cell that asks, or else the architect's reply to the matter it queued; at a
/// row that waits, the items its item depends on that are not merged, then the owner's ruling on its design card
/// waiting for one, else the row. A paused work is said to wait too, so that the owner sees both. Beside it, the ask
/// whose answer came, which the work's next step takes in.
fn waits(
    conn: &Connection,
    w: &WorkRow,
    rounds: &[Light],
    g: Option<&CompiledGraph>,
) -> Result<(Option<Waits>, Option<i64>)> {
    if w.ended.is_some() {
        return Ok((None, None));
    }
    if let Some(held) = held_by(conn, w.id)? {
        return Ok(held);
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
            None => (
                asked_architect(conn, r)?.map(|matter| Waits::Architect { matter, project: None }),
                None,
            ),
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

/// What a work waits for by its open blocker, by who holds it; none when it has none, or one the owner holds by no ask
/// and for no ruling, nor the advisor, whose round runs.
fn held_by(conn: &Connection, work: i64) -> Result<Option<(Option<Waits>, Option<i64>)>> {
    type Open = (String, String, Option<String>, Option<String>, Option<i64>, Option<i64>);
    let open: Option<Open> = conn
        .query_row(
            "SELECT kind, holder, project, items, matter, ask FROM blockers WHERE work = ?1 AND ended IS NULL",
            [work],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
        )
        .optional()?;
    let Some((kind, holder, project, items, matter, ask)) = open else {
        return Ok(None);
    };
    Ok(match (holder.as_str(), ask, matter) {
        ("owner", Some(ask), _) => {
            let answered: bool =
                conn.query_row("SELECT answer IS NOT NULL FROM owner_asks WHERE id = ?1", [ask], |r| {
                    r.get(0)
                })?;
            Some(if answered {
                (None, Some(ask))
            } else {
                (Some(Waits::Answer { ask }), None)
            })
        }
        ("owner", None, _) if kind == "ruling" => {
            let card: Option<i64> = conn
                .query_row(
                    "SELECT id FROM design_cards WHERE work = ?1 AND status = 'open' ORDER BY id DESC LIMIT 1",
                    [work],
                    |x| x.get(0),
                )
                .optional()?;
            card.map(|design_card| (Some(Waits::Ruling { design_card }), None))
        }
        ("architect", _, Some(m)) => {
            let replied: bool = conn.query_row(
                "SELECT reply IS NOT NULL FROM architect_matters WHERE id = ?1",
                [m],
                |r| r.get(0),
            )?;
            Some(if replied {
                (None, None)
            } else {
                (Some(Waits::Architect { matter: m, project }), None)
            })
        }
        ("items", _, _) => {
            let waited: Vec<i64> = serde_json::from_str::<Value>(items.as_deref().unwrap_or("[]"))
                .ok()
                .and_then(|v| v.as_array().cloned())
                .unwrap_or_default()
                .iter()
                .filter_map(|x| x["item"].as_i64())
                .collect();
            Some((Some(Waits::Items { items: waited }), None))
        }
        _ => None,
    })
}

/// The matter a round queued for the architect, while the architect has not replied to it.
fn asked_architect(conn: &Connection, r: &Light) -> Result<Option<i64>> {
    Ok(conn
        .query_row(
            "SELECT id FROM architect_matters WHERE work = ?1 AND round = ?2 AND reply IS NULL AND withdrawn IS NULL
             ORDER BY id DESC LIMIT 1",
            [r.work, r.n],
            |x| x.get(0),
        )
        .optional()?)
}

fn summary(
    w: &WorkRow,
    rounds: &[Light],
    g: Option<&CompiledGraph>,
    open: (i64, i64),
    (waits, answered): (Option<Waits>, Option<i64>),
    pinged: f64,
    tools: &HashSet<(i64, i64)>,
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
    let (paused, paused_by) = paused(w);
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
        paused,
        paused_by,
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
                tool: tools.contains(&(r.work, r.n)),
            }),
        cost_usd: rounds.iter().filter_map(|r| r.cost).sum::<f64>() + pinged,
        open_asks: open.0,
        open_design_cards: open.1,
    }
}

/// Why a work is paused and who paused it, as the owner reads it: the architect's stop is the engine's pause whose
/// reason says the architect stopped it, and is shown as the architect's, with the architect's reason.
fn paused(w: &WorkRow) -> (Option<String>, Option<PausedBy>) {
    let stopped = (w.paused_by.as_deref() == Some("engine"))
        .then(|| w.paused.as_deref()?.strip_prefix(db::STOPPED_BY_ARCHITECT))
        .flatten();
    if let Some(why) = stopped {
        let why = why.trim_start_matches(':').trim().to_string();
        return (Some(why), Some(PausedBy::Architect));
    }
    let by = match w.paused_by.as_deref() {
        Some("owner") => Some(PausedBy::Owner),
        Some(_) => Some(PausedBy::Engine),
        None => None,
    };
    (w.paused.clone(), by)
}

/// A count by key, as a query that gives each key with its count reads it.
fn counts(conn: &Connection, q: &str) -> Result<HashMap<i64, i64>> {
    let mut st = conn.prepare(q)?;
    Ok(st
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?)
}

/// The works a filter selects, as summaries, with their rows.
fn summaries(conn: &Connection, flows: &Flows, filter: &str, args: &[i64]) -> Result<Vec<(WorkRow, WorkSummary)>> {
    let asks = counts(
        conn,
        "SELECT work, count(*) FROM owner_asks WHERE answer IS NULL GROUP BY work",
    )?;
    let design_cards = counts(
        conn,
        "SELECT work, count(*) FROM design_cards WHERE status = 'open' GROUP BY work",
    )?;
    // What each work's keep-warm pings cost, which is the work's.
    let pinged: HashMap<i64, f64> = {
        let mut st = conn.prepare(
            "SELECT s.work, sum(coalesce(json_extract(p.usage, '$.cost_usd'), 0)) FROM pings p
             JOIN sessions s ON s.id = p.session GROUP BY s.work",
        )?;
        st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?
    };
    // The rounds a tool runs in now: started, and not ended.
    let tools: HashSet<(i64, i64)> = {
        let mut st = conn
            .prepare("SELECT DISTINCT work, round FROM tool_runs WHERE started_at IS NOT NULL AND ended_at IS NULL")?;
        st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?
    };
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
        let pinged = pinged.get(&w.id).copied().unwrap_or(0.0);
        let s = summary(&w, &rounds, g.as_ref(), open, waits, pinged, &tools);
        out.push((w, s));
    }
    Ok(out)
}

/// Every work but the architect's wakes, which its tab shows, oldest first.
pub fn works(conn: &Connection, flows: &Flows) -> Result<Vec<WorkSummary>> {
    work_summaries(conn, flows, "WHERE w.graph NOT IN ('architect', 'policy')")
}

/// The works a filter on `works w` selects, as summaries, oldest first.
pub(crate) fn work_summaries(conn: &Connection, flows: &Flows, filter: &str) -> Result<Vec<WorkSummary>> {
    Ok(summaries(conn, flows, filter, &[])?
        .into_iter()
        .map(|(_, s)| s)
        .collect())
}

const ROUND: &str = "SELECT r.n, r.cell, s.line, r.transition, r.rejected, r.started_at, r.ended_at, r.model,
       json_extract(r.usage, '$.cost_usd'), r.compactions,
       (SELECT count(*) FROM tool_runs t WHERE t.work = r.work AND t.round = r.n),
       json_extract(r.output, '$.progress'),
       r.work, w.flow_version, w.graph, s.cli, s.cli_session, r.config_commit, r.input, r.output, r.usage,
       json_extract(r.input, '$.engine.not_applicable'), r.session, json_extract(r.input, '$.call.level'),
       json_extract(r.input, '$.call.command'),
       CASE WHEN s.started <> 'wake'
         AND r.n = (SELECT min(x.n) FROM rounds x WHERE x.work = r.work AND x.session = r.session)
       THEN s.started END,
       CASE WHEN r.session IS NULL THEN json_extract(r.input, '$.engine.new_session') END
FROM rounds r JOIN works w ON w.id = r.work LEFT JOIN sessions s ON s.id = r.session";

/// Where an agent's round spent its tokens, from its usage record as the command line reported it: the totals of the
/// round's calls, and the context of its last call; the context at its start is the caller's to find.
fn round_tokens(usage: Option<&Value>) -> Option<RoundTokens> {
    let u = usage?.get("usage")?;
    let n = |v: &Value| v.as_i64().unwrap_or(0);
    let written = n(&u["cache_creation_input_tokens"]);
    // A record that does not split what was written to the cache by how long it is kept: all of it for five minutes.
    let (written_5m, written_1h) = match u.get("cache_creation") {
        Some(c) => (n(&c["ephemeral_5m_input_tokens"]), n(&c["ephemeral_1h_input_tokens"])),
        None => (written, 0),
    };
    let last = u["iterations"].as_array().and_then(|xs| xs.last());
    Some(RoundTokens {
        steps: n(&usage?["turns"]),
        input: n(&u["input_tokens"]),
        read: n(&u["cache_read_input_tokens"]),
        written_5m,
        written_1h,
        output: n(&u["output_tokens"]),
        thinking: n(&u["output_tokens_details"]["thinking_tokens"]),
        context_end: last
            .map(|c| n(&c["input_tokens"]) + n(&c["cache_read_input_tokens"]) + n(&c["cache_creation_input_tokens"])),
        window: usage?["models"]
            .as_object()
            .and_then(|m| m.values().filter_map(|v| v["contextWindow"].as_i64()).max()),
        context_before: None,
        level: None,
        effort: None,
        others: Vec::new(),
    })
}

/// The tokens each model has counted in a session so far, as a round's usage record gives them per model.
fn per_model(usage: Option<&Value>) -> BTreeMap<String, i64> {
    let Some(models) = usage.and_then(|u| u.get("models")).and_then(Value::as_object) else {
        return BTreeMap::new();
    };
    models
        .iter()
        .map(|(k, v)| {
            let n = |f: &str| v[f].as_i64().unwrap_or(0);
            let all = n("inputTokens") + n("outputTokens") + n("cacheReadInputTokens") + n("cacheCreationInputTokens");
            (k.clone(), all)
        })
        .collect()
}

/// The models besides a round's own whose count grew since the session's round before it.
fn others(now: &BTreeMap<String, i64>, before: Option<&BTreeMap<String, i64>>, own: Option<&str>) -> Vec<String> {
    now.iter()
        .filter(|(m, n)| Some(m.as_str()) != own && **n > before.and_then(|b| b.get(*m)).copied().unwrap_or(0))
        .map(|(m, _)| m.clone())
        .collect()
}

fn round_summary(r: &Row<'_>) -> rusqlite::Result<RoundSummary> {
    let transition: Option<String> = r.get(3)?;
    // The level as the round's call record says it, and the effort its command line gave.
    let level: Option<String> = r.get(23)?;
    let effort = json(r.get(24)?).and_then(|command| {
        let args = command.as_array()?;
        let i = args.iter().position(|a| a.as_str() == Some("--effort"))?;
        args.get(i + 1)?.as_str().map(str::to_string)
    });
    let rejected = json(r.get(4)?);
    let ended_at: Option<i64> = r.get(6)?;
    Ok(RoundSummary {
        n: r.get(0)?,
        cell: r.get(1)?,
        line: r.get(2)?,
        outcome: outcome(transition.as_deref(), rejected.as_ref(), ended_at.is_some()),
        // A row the engine took for a line whose session would start afresh says why it would.
        why: why(rejected.as_ref()).or(r.get(26)?),
        transition,
        started_at: r.get(5)?,
        ended_at,
        model: r.get(7)?,
        cost_usd: r.get(8)?,
        compactions: r.get(9)?,
        tool_runs: r.get(10)?,
        progress: text(r, 11)?,
        not_asked: r.get(21)?,
        tokens: round_tokens(json(r.get(20)?).as_ref()).map(|t| RoundTokens { level, effort, ..t }),
        session: r.get(22)?,
        new_session: r.get(25)?,
    })
}

/// A design card, with the one that replaced it: the round that handed that one in, and that round's work.
const DESIGN_CARD: &str = "SELECT c.id, c.round, c.work, w.item, c.status, c.body, c.rulings, c.replaced_by,
       n.round, n.work, nr.cell, coalesce(nr.ended_at, nr.started_at), nw.flow_version, nw.graph, n.changed,
       cr.cell, coalesce(cr.ended_at, cr.started_at), w.flow_version, w.graph,
       (SELECT p.id FROM design_cards p WHERE p.replaced_by = c.id ORDER BY p.id DESC LIMIT 1), c.changed
FROM design_cards c JOIN works w ON w.id = c.work
LEFT JOIN rounds cr ON cr.work = c.work AND cr.n = c.round
LEFT JOIN design_cards n ON n.id = c.replaced_by LEFT JOIN rounds nr ON nr.work = n.work AND nr.n = n.round
LEFT JOIN works nw ON nw.id = n.work";

pub(crate) fn design_cards(conn: &Connection, flows: &Flows, filter: &str, args: &[i64]) -> Result<Vec<DesignCard>> {
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
        let own: (i64, String) = (r.get(17)?, r.get(18)?);
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
            cell: r.get(15)?,
            title: None,
            at: r.get(16)?,
            replaces: r.get(19)?,
            changed: r.get(20)?,
        };
        Ok((card, pinned, own))
    })?;
    // The title of a cell in a flow version's graph, when it has the cell.
    let title = |version: i64, graph: &str, cell: Option<String>| -> Option<String> {
        let c = flows.get(conn, version).ok()?;
        Some(c.graph(graph)?.cell(&cell?)?.title.clone())
    };
    let mut out = Vec::new();
    for row in rows {
        let (mut card, pinned, (version, graph)) = row?;
        if let (Some(x), Some((version, graph))) = (card.replaced.as_mut(), pinned) {
            x.title = title(version, &graph, x.cell.clone());
        }
        card.title = title(version, &graph, card.cell.clone());
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

pub(crate) fn asks(conn: &Connection, filter: &str, args: &[i64]) -> Result<Vec<OwnerAsk>> {
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

const MESSAGE: &str = "SELECT m.id, m.round, r.cell, s.line, r.ended_at, m.work, w.item, i.title, m.text, m.read_at,
       CASE WHEN w.graph IN ('architect', 'policy') THEN w.project END
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
            architect: r.get(10)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

const NOTICE: &str = "SELECT n.id, n.work, w.item, i.title, n.text, n.at, n.read_at, n.ask, a.body,
       CASE WHEN w.graph IN ('architect', 'policy') THEN w.project END
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
            architect: r.get(9)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
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
    let rows: Vec<(RoundSummary, Option<i64>, Option<String>)> = st
        .query_map([id], |r| Ok((round_summary(r)?, r.get(22)?, r.get(20)?)))?
        .collect::<rusqlite::Result<_>>()?;
    // Each round's context at its start is where the latest round before it in its session ended, and the models
    // besides its own that it used are those whose count in the session grew since that session's round before it.
    let mut ended: HashMap<i64, i64> = HashMap::new();
    let mut counted: HashMap<i64, BTreeMap<String, i64>> = HashMap::new();
    let mut rounds = Vec::with_capacity(rows.len());
    for (mut round, session, usage) in rows {
        if let (Some(session), Some(tokens)) = (session, round.tokens.as_mut()) {
            tokens.context_before = ended.get(&session).copied();
            if let Some(end) = tokens.context_end {
                ended.insert(session, end);
            }
            let now = per_model(json(usage).as_ref());
            tokens.others = others(&now, counted.get(&session), round.model.as_deref());
            counted.insert(session, now);
        }
        rounds.push(round);
    }
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
        sessions: work_sessions(conn, id)?,
        pings: pings(conn, id)?,
    })
}

/// The sessions a work's lines ran in, oldest first.
fn work_sessions(conn: &Connection, work: i64) -> Result<Vec<WorkSession>> {
    let mut st = conn.prepare("SELECT id, line, started, ended FROM sessions WHERE work = ?1 ORDER BY id")?;
    let rows = st.query_map([work], |r| {
        Ok(WorkSession {
            id: r.get(0)?,
            line: r.get(1)?,
            started: r.get(2)?,
            ended: r.get(3)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// The keep-warm pings sent to a work's sessions, oldest first.
fn pings(conn: &Connection, work: i64) -> Result<Vec<Ping>> {
    let mut st = conn.prepare(
        "SELECT p.id, p.session, s.line, json_extract(p.waited, '$.cell'), json_extract(p.waited, '$.on'),
                json_extract(p.waited, '$.what'), p.started_at, p.ended_at, p.model,
                json_extract(p.usage, '$.cost_usd'),
                coalesce(json_extract(p.usage, '$.usage.cache_read_input_tokens'), 0),
                coalesce(json_extract(p.usage, '$.usage.cache_creation_input_tokens'), 0), p.error
         FROM pings p JOIN sessions s ON s.id = p.session WHERE s.work = ?1 ORDER BY p.id",
    )?;
    let rows = st.query_map([work], |r| {
        Ok(Ping {
            id: r.get(0)?,
            session: r.get(1)?,
            line: r.get(2)?,
            cell: r.get::<_, Option<String>>(3)?.unwrap_or_default(),
            on: r.get::<_, Option<String>>(4)?.unwrap_or_default(),
            what: r.get::<_, Option<String>>(5)?.unwrap_or_default(),
            started_at: r.get(6)?,
            ended_at: r.get(7)?,
            model: r.get(8)?,
            cost_usd: r.get(9)?,
            read: r.get(10)?,
            written: r.get(11)?,
            error: r.get(12)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
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
                r.get::<_, Option<i64>>(22)?,
            ))
        })
        .optional()?
        .ok_or_else(|| Error::NotFound(format!("no round R{work}-{n}")))?;
    let (mut summary, _, flow_version, graph, session, config_commit, input, output, usage, rejected, session_id) = row;
    // Its context at its start is where the latest round before it in its session ended, and the models besides its
    // own that it used are those whose count in the session grew since that session's round before it.
    if let (Some(id), Some(tokens)) = (session_id, summary.tokens.as_mut()) {
        let mut st = conn.prepare(
            "SELECT usage FROM rounds WHERE work = ?1 AND session = ?2 AND n < ?3 AND usage IS NOT NULL ORDER BY n DESC",
        )?;
        let usages = st.query_map(rusqlite::params![work, id, n], |r| r.get::<_, Option<String>>(0))?;
        let mut before: Option<BTreeMap<String, i64>> = None;
        for u in usages {
            let u = json(u?);
            if before.is_none() {
                before = Some(per_model(u.as_ref()));
            }
            if let Some(end) = round_tokens(u.as_ref()).and_then(|x| x.context_end) {
                tokens.context_before = Some(end);
                break;
            }
        }
        tokens.others = others(&per_model(usage.as_ref()), before.as_ref(), summary.model.as_deref());
    }
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

/// What a conditional input of one of a work's rounds holds, by the hash the round's record names it with, whether the
/// round was given it or its session already held it: the content a round of the work gave under that hash, as its
/// message gave it; or, for a value the session's own return produced and was never given, the value with that hash
/// at the cell and path the address names in a return of the work. The hash finds it whatever round the address
/// names, so an input of a round recorded before rounds were named by their work is found too.
pub fn content(conn: &Connection, flows: &Flows, work: i64, hash: &str, address: &str) -> Result<Value> {
    let missing = || Error::NotFound(format!("no content {hash} in work {work}"));
    let mut st = conn.prepare("SELECT input FROM rounds WHERE work = ?1 AND instr(input, ?2) > 0 ORDER BY n")?;
    let inputs: Vec<String> = st
        .query_map(rusqlite::params![work, hash], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    for text in inputs {
        let Some(input) = json(Some(text)) else { continue };
        // A round recorded before the conditional inputs were called so keeps them under `materials`.
        let entries = input
            .get("conditional")
            .or_else(|| input.get("materials"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let given: Vec<&str> = entries
            .iter()
            .filter(|m| m["given"] == true)
            .filter_map(|m| m["address"].as_str())
            .collect();
        let Some(at) = entries
            .iter()
            .find(|m| m["hash"] == hash && m["given"] == true)
            .and_then(|m| m["address"].as_str())
        else {
            continue;
        };
        let record = db::put_back(conn, &input);
        if let Some(text) = record["message"].as_str().and_then(|m| given_in(m, at, &given)) {
            return Ok(as_given(text, hash));
        }
    }
    // A value of the session's own return: at the cell and path its address names, in a return of the work.
    let mut parts = address.splitn(3, '·');
    let (Some(_), Some(cell), Some(path)) = (parts.next(), parts.next(), parts.next()) else {
        return Err(missing());
    };
    if !address.starts_with('R') || cell == "engine" {
        return Err(missing());
    }
    let (version, graph): (i64, String) = conn
        .query_row("SELECT flow_version, graph FROM works WHERE id = ?1", [work], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .optional()?
        .ok_or_else(missing)?;
    let g = flows.graph(conn, version, &graph);
    let agent = g.as_ref().and_then(|g| g.cell(cell)).is_some_and(|c| c.kind == "agent");
    let mut st =
        conn.prepare("SELECT output FROM rounds WHERE work = ?1 AND cell = ?2 AND output IS NOT NULL ORDER BY n DESC")?;
    let outputs: Vec<String> = st
        .query_map(rusqlite::params![work, cell], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    for out in outputs {
        let Some(ret) = json(Some(out)) else { continue };
        let ret = match (&g, agent) {
            (Some(g), true) => g.unwrap(&ret),
            _ => ret,
        };
        if let Some(v) = value_at(&ret, path)
            && flow::formats::hash(&v) == hash
        {
            return Ok(v);
        }
    }
    Err(missing())
}

/// The text a message gave under an address, among the addresses it gave in the order it gave them: from that
/// address's heading to the next given one's, or to the message's next part.
fn given_in<'m>(message: &'m str, address: &str, given: &[&str]) -> Option<&'m str> {
    let head = format!("\n### {address}\n");
    let rest = &message[message.find(&head)? + head.len()..];
    let end = given
        .iter()
        .skip_while(|a| **a != address)
        .nth(1)
        .and_then(|next| rest.find(&format!("\n### {next}\n")))
        .or_else(|| rest.find("\n## "))
        .unwrap_or(rest.len());
    Some(rest[..end].trim_end_matches('\n'))
}

/// A content as a message gave it: what was given as JSON as that JSON, anything else as the text.
fn as_given(text: &str, hash: &str) -> Value {
    match serde_json::from_str::<Value>(text) {
        Ok(v) if flow::formats::hash(&v) == hash => v,
        _ => Value::String(text.to_string()),
    }
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

/// What the inputs that come from outside the flow hold now, as far as the page can know outside a round: each
/// project's settings, project guide and rules, as the configuration and the project's checkout have them, and what
/// each table and view of the database holds.
pub fn inputs_now(conn: &Connection, config: &config::Config) -> Result<InputsNow> {
    let text = |v: Value| match v {
        Value::String(s) => s,
        other => other.to_string(),
    };
    let projects = config
        .projects
        .iter()
        .map(|(name, p)| {
            let guide = config::project_guide(&p.checkout, &p.design_paths, "");
            ProjectNow {
                project: name.clone(),
                settings: config::SETTINGS
                    .iter()
                    .filter_map(|s| {
                        p.setting(s).filter(|v| !v.is_null()).map(|v| NamedText {
                            name: s.to_string(),
                            text: text(v),
                        })
                    })
                    .collect(),
                agents_md: guide["agents_md"].as_str().unwrap_or_default().to_string(),
                design_tree: guide["design_tree"]
                    .as_object()
                    .into_iter()
                    .flatten()
                    .map(|(node, first)| NamedText {
                        name: node.clone(),
                        text: first.as_str().unwrap_or_default().to_string(),
                    })
                    .collect(),
                rules: p
                    .rules
                    .iter()
                    .filter_map(|r| {
                        config.rules.get(r).map(|t| NamedText {
                            name: r.clone(),
                            text: t.trim().to_string(),
                        })
                    })
                    .collect(),
            }
        })
        .collect();
    let tables = db::descriptions(conn)?
        .into_iter()
        .map(|(name, text)| NamedText { name, text })
        .collect();
    Ok(InputsNow { projects, tables })
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
            // No work stands where an item waits to start, the graph's start that waits there, nor at the end of a work
            // cancelled, which it ends at from any cell: neither is a stage a work goes through.
            let goes_through = |g: &CompiledGraph, x: &CompiledCell| {
                let waits_to_start = x.id == g.start && x.rows.iter().any(|r| r.waits && r.to == x.id);
                !waits_to_start && g.cancel.as_deref() != Some(x.id.as_str())
            };
            for s in c
                .graph(&w.graph)
                .into_iter()
                .flat_map(|g| g.cells.iter().filter(move |x| goes_through(g, x)))
                .filter_map(|x| x.stage.as_ref())
            {
                if !out.contains(s) {
                    out.push(s.clone());
                }
            }
            out
        });
    }
    // The unmerged items each item depends on by a blocking dependency, which keep it from starting.
    let mut blocked: HashMap<i64, Vec<i64>> = HashMap::new();
    let mut st = conn.prepare("SELECT item, depends_on FROM item_blocked_by ORDER BY item, depends_on")?;
    for row in st.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))? {
        let (item, on) = row?;
        blocked.entry(item).or_default().push(on);
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
            let state = item_state(cancelled.is_some(), queued.is_some(), w);
            // Only an item that would start but for them is held back by them.
            let waiting = matches!(state, ItemState::New | ItemState::Queued | ItemState::Stopped);
            let blocked_by = blocked.get(&id).filter(|_| waiting).cloned().unwrap_or_default();
            Ok(ItemSummary {
                id,
                project: r.get(1)?,
                title: r.get(2)?,
                deliver: r.get(3)?,
                level: r.get(4)?,
                state,
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
                blocked_by,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    let mut st = conn.prepare("SELECT item, depends_on, needs, kind FROM item_links ORDER BY item, depends_on")?;
    let links = st
        .query_map([], |r| {
            Ok(ItemLink {
                item: r.get(0)?,
                depends_on: r.get(1)?,
                needs: r.get(2)?,
                kind: r.get(3)?,
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
    let linked = |other: i64, l: &ItemLink| {
        by_id.get(&other).map(|i| LinkedItem {
            item: (*i).clone(),
            needs: l.needs.clone(),
            kind: l.kind.clone(),
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
            .filter_map(|l| linked(l.depends_on, l))
            .collect(),
        depended_on_by: all
            .links
            .iter()
            .filter(|l| l.depends_on == id)
            .filter_map(|l| linked(l.item, l))
            .collect(),
        works: works.into_iter().filter(|w| w.item == Some(id)).collect(),
        imported_from,
        imported_branch,
        imported_pr,
        design_cards: design_cards(conn, flows, "WHERE w.item = ?1", &[id])?,
        asks: asks(conn, "WHERE w.item = ?1", &[id])?,
        shown: shown(conn, flows, id)?,
        level_changes: level_changes(conn, id)?,
        item: summary,
    })
}

/// Each time an item's tier changed after it was made, oldest first, as `changes` keeps it.
fn level_changes(conn: &Connection, item: i64) -> Result<Vec<LevelChange>> {
    let mut st = conn.prepare(
        "SELECT at, actor, json_extract(old_row, '$.level'), json_extract(new_row, '$.level') FROM changes
         WHERE table_name = 'items' AND json_extract(row_key, '$.id') = ?1 AND old_row IS NOT NULL AND new_row IS NOT NULL
           AND json_extract(old_row, '$.level') <> json_extract(new_row, '$.level')
         ORDER BY id",
    )?;
    let rows = st.query_map([item], |r| {
        Ok(LevelChange {
            at: r.get(0)?,
            by: r.get(1)?,
            from: r.get(2)?,
            to: r.get(3)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
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
    shown_where(conn, flows, "w.item = ?1", &[item])
}

/// What the rounds a filter on `rounds r` and `works w` selects handed back that their flow versions show on the
/// item, oldest first.
pub(crate) fn shown_where(conn: &Connection, flows: &Flows, filter: &str, args: &[i64]) -> Result<Vec<Shown>> {
    let mut st = conn.prepare(&format!(
        "SELECT r.n, r.work, r.cell, r.output, w.flow_version, w.graph FROM rounds r JOIN works w ON w.id = r.work
         WHERE {filter} AND r.cell IS NOT NULL AND r.output IS NOT NULL AND r.transition IS NOT NULL
         ORDER BY r.work, r.n"
    ))?;
    let rows = st.query_map(params_from_iter(args), |r| {
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

/// A project's architect, as its tab shows it: `projects` are every project of the configuration, `tags` the project's
/// tags for observations, each with what it covers, and `own` whether the project is wf-agents itself. Its cells are
/// named as the views over its rounds name them.
pub fn architect(
    conn: &Connection,
    flows: &Flows,
    project: &str,
    projects: Vec<String>,
    tags: Vec<Tag>,
    own: bool,
) -> Result<Architect> {
    // The project's wakes, and the settings of its policy beside them, by their numbers: the filter takes numbers only.
    let mine: Vec<i64> = {
        let mut st =
            conn.prepare("SELECT id FROM works WHERE graph IN ('architect', 'policy') AND project = ?1 ORDER BY id")?;
        st.query_map([project], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?
    };
    let listed: Vec<String> = mine.iter().map(i64::to_string).collect();
    let mut wakes = summaries(conn, flows, &format!("WHERE w.id IN ({})", listed.join(", ")), &[])?;
    wakes.reverse();
    let handled = counts(
        conn,
        "SELECT handled_work, count(*) FROM architect_matters WHERE handled_work IS NOT NULL GROUP BY handled_work",
    )?;
    let mut woke = woke_for(conn, project)?;
    // The settings of the project's policy, each a task of its own beside the wakes, and the one that runs.
    let settings: Vec<(i64, bool)> = {
        let mut st = conn.prepare(
            "SELECT w.id, w.ended IS NULL AND NOT EXISTS (
               SELECT 1 FROM rounds r WHERE r.work = w.id AND r.cell = 'E1' AND r.ended_at IS NOT NULL)
             FROM works w WHERE w.graph = 'policy' AND w.project = ?1 ORDER BY w.id",
        )?;
        st.query_map([project], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?
    };
    let running = settings.iter().rev().find(|x| x.1).map(|x| x.0);
    // A new policy is being set while a setting of it runs; before the setting was a task of its own, while the latest
    // wake, not over, was at the stage the study of one is in, past the check of the policy in force that opens it.
    let setting = match (running, wakes.first()) {
        (Some(p), _) => Some(setting(conn, p)?),
        (None, Some((_, s))) if !over(s) && s.stage.is_some() && !db::is_cell("architect", "P1", &s.at) => {
            let study = flows.graph(conn, s.flow_version, &s.graph).and_then(|g| {
                g.cells
                    .iter()
                    .find(|c| db::is_cell("architect", "P2", &c.id))
                    .and_then(|c| c.stage.clone())
            });
            if study == s.stage {
                Some(setting(conn, s.id)?)
            } else {
                None
            }
        }
        (None, _) => None,
    };
    let mut out_wakes = Vec::new();
    for (_, s) in wakes {
        let handling = if over(&s) { None } else { handling(conn, s.id)? };
        out_wakes.push(Wake {
            woke_for: woke.remove(&s.id).unwrap_or_default(),
            handled: handled.get(&s.id).copied().unwrap_or(0),
            handling,
            work: s,
        });
    }
    let mut asks = asks(conn, "WHERE w.graph IN ('architect', 'policy')", &[])?;
    asks.retain(|a| mine.contains(&a.work));
    Ok(Architect {
        project: project.to_string(),
        projects,
        wakes: out_wakes,
        queue: queue(conn, project)?,
        holds: holds(conn, project)?,
        asks,
        handled: matters_handled(conn, project)?,
        changes: item_changes(conn, project)?,
        policy: policy(conn, project)?,
        setting,
        check: latest_of(conn, project, "P1")?,
        next: next(conn, project)?,
        observations: observations(conn, project)?,
        tags,
        own,
    })
}

pub(crate) fn over(w: &WorkSummary) -> bool {
    matches!(w.state, WorkState::Done | WorkState::Ended)
}

/// What a round handed back under a field of its answer, as the round's record keeps it: an agent's own answer, or the
/// whole return of a cell that is not an agent's.
fn returned(output: Option<String>, field: Option<&str>) -> Value {
    let whole = json(output).unwrap_or(Value::Null);
    let o = match whole.get("outcome") {
        Some(o) => o.clone(),
        None => whole,
    };
    match field {
        Some(f) => o[f].clone(),
        None => o,
    }
}

/// The matter a wake handles now: the one its latest pick chose, while it is neither handled nor held by an ask.
pub(crate) fn handling(conn: &Connection, wake: i64) -> Result<Option<i64>> {
    let picked: Option<Option<i64>> = conn
        .query_row(
            &format!(
                "SELECT coalesce(json_extract(output, '$.queue.matter'), json_extract(output, '$.outcome.pick.matter'))
                 FROM rounds WHERE work = ?1 AND transition IS NOT NULL
                   AND (cell IN {} OR (cell IN {} AND json_extract(output, '$.outcome.pick.matter') IS NOT NULL))
                 ORDER BY n DESC LIMIT 1",
                db::cells_sql("architect", &["Q0"]),
                db::cells_sql("architect", &["Q1"])
            ),
            [wake],
            |r| r.get(0),
        )
        .optional()?;
    let Some(m) = picked.flatten() else { return Ok(None) };
    let open: i64 = conn.query_row(
        "SELECT count(*) FROM architect_matters m WHERE m.id = ?1 AND m.handled_round IS NULL AND m.withdrawn IS NULL
           AND (m.asked IS NULL OR EXISTS (SELECT 1 FROM owner_asks a WHERE a.id = m.asked AND a.answer IS NOT NULL))",
        [m],
        |r| r.get(0),
    )?;
    Ok((open > 0).then_some(m))
}

/// What each wake of a project's architect was given of the queue at its first round, by wake: the matters that woke
/// it.
fn woke_for(conn: &Connection, project: &str) -> Result<HashMap<i64, Vec<Queued>>> {
    let mut st = conn.prepare(
        "SELECT r.work, r.input FROM rounds r JOIN works w ON w.id = r.work
         WHERE w.graph = 'architect' AND w.project = ?1 AND r.n = 1",
    )?;
    let rows = st.query_map([project], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
    let mut out = HashMap::new();
    for row in rows {
        let (work, input) = row?;
        let Some(input) = json(Some(input)) else { continue };
        let input = db::put_back(conn, &input);
        let queued = input["inputs"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|x| {
                x["from"]
                    .as_array()
                    .is_some_and(|f| f.iter().any(|s| s["db"] == "architect_queue"))
            })
            .filter_map(|x| x["value"].as_array())
            .flatten()
            .filter_map(|m| {
                Some(Queued {
                    matter: m["matter"].as_i64()?,
                    kind: m["kind"].as_str()?.to_string(),
                    detail: m["detail"].as_str().map(str::to_string),
                    what: m["what"].as_str().unwrap_or_default().to_string(),
                    item: m["item"].as_i64(),
                })
            })
            .collect();
        out.insert(work, queued);
    }
    Ok(out)
}

/// The cells on a matter's path, whose rounds do what becomes of a matter.
const PATH: [&str; 11] = ["H1", "H2", "H3", "H4", "H5", "H6", "H7", "H8", "H9", "H10", "A3"];

/// The first `why` a return says, looking through it in order: why a cell closed a matter with nothing else done.
fn first_why(v: &Value) -> Option<String> {
    match v {
        Value::Object(o) => o
            .get("why")
            .and_then(Value::as_str)
            .filter(|w| !w.trim().is_empty())
            .map(str::to_string)
            .or_else(|| o.values().find_map(first_why)),
        Value::Array(xs) => xs.iter().find_map(first_why),
        _ => None,
    }
}

/// The matters a project's architect handled, newest first, the latest hundred, each with what became of it: what its
/// path did, from the pick that took it to the round that closed it, as the records of what was done have it (the
/// reply kept with the matter, the item changes it collected, the effects its rounds handed in and the engine carried
/// out, what they told the owner, the ask put about it); and when nothing came of it, why.
pub(crate) fn matters_handled(conn: &Connection, project: &str) -> Result<Vec<Handled>> {
    let mut st = conn.prepare(
        "SELECT m.id, m.kind, m.detail, m.what, m.handled_work, m.handled_round, coalesce(r.cell, ''), r.ended_at,
                m.work, json_extract(m.reply, '$.text'), m.asked, r.output
         FROM architect_matters m JOIN rounds r ON r.work = m.handled_work AND r.n = m.handled_round
         WHERE m.project = ?1 AND m.handled_round IS NOT NULL
         ORDER BY m.handled_work DESC, m.handled_round DESC, m.id DESC LIMIT 100",
    )?;
    let rows: Vec<(Handled, Option<String>)> = st
        .query_map([project], |r| {
            let reply: Option<String> = r.get(9)?;
            let from: Option<i64> = r.get(8)?;
            Ok((
                Handled {
                    matter: r.get(0)?,
                    kind: r.get(1)?,
                    detail: r.get(2)?,
                    what: r.get(3)?,
                    work: r.get(4)?,
                    round: r.get(5)?,
                    cell: r.get(6)?,
                    at: r.get(7)?,
                    reply_to: reply.as_ref().and(from),
                    reply,
                    told: 0,
                    made: 0,
                    dropped: 0,
                    collected: 0,
                    observations: Vec::new(),
                    actions: Vec::new(),
                    messages: Vec::new(),
                    ask: r.get(10)?,
                    why: None,
                },
                r.get(11)?,
            ))
        })?
        .collect::<rusqlite::Result<_>>()?;
    let picks = db::cells_sql("architect", &["Q1", "Q0"]);
    let path = db::cells_sql("architect", &PATH);
    let mut out = Vec::new();
    for (mut h, closing) in rows {
        // Its path began after the round that took it: a pick, or a look at a queue that held it alone.
        let taken: Option<i64> = conn.query_row(
            &format!(
                "SELECT max(n) FROM rounds WHERE work = ?1 AND n <= ?2 AND cell IN {picks}
                   AND ?3 IN (json_extract(output, '$.outcome.pick.matter'), json_extract(output, '$.outcome.queue.matter'))"
            ),
            params![h.work, h.round, h.matter],
            |r| r.get(0),
        )?;
        let from = taken.unwrap_or(h.round - 1);
        let mut st = conn.prepare(&format!(
            "SELECT json_extract(output, '$.outcome.effects') FROM rounds
             WHERE work = ?1 AND n > ?2 AND n <= ?3 AND transition IS NOT NULL AND cell IN {path} ORDER BY n"
        ))?;
        let effects: Vec<Option<String>> = st
            .query_map(params![h.work, from, h.round], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        for e in effects.into_iter().filter_map(json) {
            for c in e["observation_changes"].as_array().into_iter().flatten() {
                h.observations.push(c["kind"].as_str().unwrap_or_default().to_string());
            }
            for a in e["actions"].as_array().into_iter().flatten() {
                h.actions.push(HandledAction {
                    kind: a["kind"].as_str().unwrap_or_default().to_string(),
                    item: a["item"].as_i64(),
                });
            }
            for m in e["messages"].as_array().into_iter().flatten() {
                h.messages.push(m["project"].as_str().unwrap_or_default().to_string());
            }
        }
        h.told = conn.query_row(
            "SELECT count(*) FROM owner_messages WHERE work = ?1 AND round > ?2 AND round <= ?3",
            params![h.work, from, h.round],
            |r| r.get(0),
        )?;
        let mut st =
            conn.prepare("SELECT state, count(*) FROM architect_item_changes WHERE matter = ?1 GROUP BY state")?;
        let states: Vec<(String, i64)> = st
            .query_map([h.matter], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        for (state, n) in states {
            match state.as_str() {
                "made" => h.made = n,
                "dropped" => h.dropped = n,
                "collected" => h.collected = n,
                _ => {}
            }
        }
        let nothing = h.reply.is_none()
            && h.told == 0
            && h.made + h.dropped + h.collected == 0
            && h.observations.is_empty()
            && h.actions.is_empty()
            && h.messages.is_empty()
            && h.ask.is_none();
        if nothing {
            h.why = first_why(&returned(closing, None));
        }
        out.push(h);
    }
    Ok(out)
}

/// The matters a project's architect has not handled: those in its queue, in the order they came; then those an ask of
/// the owner holds. A place to set straight comes with the place, and a design card to look at with how the card
/// stands.
fn queue(conn: &Connection, project: &str) -> Result<Vec<Matter>> {
    let mut st = conn.prepare(
        "SELECT m.id, m.kind, m.detail, m.what,
           CASE WHEN w.graph = 'architect' THEN 'architect' WHEN m.work IS NOT NULL THEN 'work'
                WHEN m.kind = 'owner_words' THEN 'owner' ELSE 'engine' END,
           m.item, i.title, m.work, m.round, m.at, m.asked, m.resume_cell,
           m.asked IS NOT NULL AND EXISTS (SELECT 1 FROM owner_asks a WHERE a.id = m.asked AND a.answer IS NOT NULL),
           q.waits, q.waited_ms, coalesce(json_extract(m.body, '$.urgent'), 0),
           CASE WHEN m.kind = 'set_straight' THEN json_extract(m.body, '$.fix') END,
           d.id, d.status, d.rulings, d.replaced_by
         FROM architect_matters m LEFT JOIN architect_queue q ON q.matter = m.id
         LEFT JOIN items i ON i.id = m.item LEFT JOIN works w ON w.id = m.work
         LEFT JOIN design_cards d ON m.kind = 'design_card' AND d.id = json_extract(m.body, '$.design_card')
         WHERE m.project = ?1 AND m.handled_round IS NULL AND m.withdrawn IS NULL
         ORDER BY CASE WHEN q.matter IS NOT NULL THEN 0 ELSE 1 END, m.id",
    )?;
    let rows = st.query_map([project], |r| {
        Ok(Matter {
            id: r.get(0)?,
            kind: r.get(1)?,
            detail: r.get(2)?,
            what: r.get(3)?,
            source: r.get(4)?,
            item: r.get(5)?,
            title: r.get(6)?,
            work: r.get(7)?,
            round: r.get(8)?,
            at: r.get(9)?,
            asked: r.get(10)?,
            resume_cell: r.get(11)?,
            answered: r.get(12)?,
            waits: r.get(13)?,
            waited_ms: r.get(14)?,
            urgent: r.get::<_, i64>(15)? != 0,
            fix: json(r.get(16)?),
            card: match r.get::<_, Option<i64>>(17)? {
                Some(id) => Some(MatterCard {
                    id,
                    status: r.get(18)?,
                    rulings: json(r.get(19)?).unwrap_or(Value::Null),
                    replaced_by: r.get(20)?,
                }),
                None => None,
            },
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

fn holds(conn: &Connection, project: &str) -> Result<Vec<DispatchHold>> {
    let mut st =
        conn.prepare("SELECT reason, since, why FROM dispatch_holds WHERE project = ?1 ORDER BY since, reason")?;
    let rows = st.query_map([project], |r| {
        Ok(DispatchHold {
            reason: r.get(0)?,
            since: r.get(1)?,
            why: r.get(2)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// The item changes a project's architect's batches made or dropped, newest first.
fn item_changes(conn: &Connection, project: &str) -> Result<Vec<ItemChange>> {
    let mut st = conn.prepare(
        "SELECT c.id, c.work, c.round, c.matter, c.change, c.state, c.outcome, coalesce(r.cell, '')
         FROM architect_item_changes c JOIN works w ON w.id = c.work
         LEFT JOIN rounds r ON r.work = c.work AND r.n = c.round
         WHERE w.project = ?1 AND c.state IN ('made', 'dropped') ORDER BY c.id DESC LIMIT 500",
    )?;
    let rows = st.query_map([project], |r| {
        Ok(ItemChange {
            id: r.get(0)?,
            work: r.get(1)?,
            round: r.get(2)?,
            matter: r.get(3)?,
            change: json(r.get(4)?).unwrap_or(Value::Null),
            state: r.get(5)?,
            outcome: json(r.get(6)?),
            cell: r.get(7)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// A project's observations: the open ones first, then those closed, each newest first.
pub(crate) fn observations(conn: &Connection, project: &str) -> Result<Vec<Observation>> {
    let mut st = conn.prepare(
        "SELECT id, what, tags, source, times_reported, refs, folded_into, closed, closed_why, created, updated
         FROM observations_all WHERE project = ?1
         ORDER BY closed IS NOT NULL, coalesce(updated, created) DESC, id DESC",
    )?;
    let texts = |s: Option<String>| -> Vec<String> {
        json(s)
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default()
            .iter()
            .filter_map(|x| x.as_str().map(str::to_string))
            .collect()
    };
    let rows = st.query_map([project], |r| {
        Ok(Observation {
            id: r.get(0)?,
            what: r.get(1)?,
            tags: texts(r.get(2)?),
            source: r.get(3)?,
            times_reported: r.get(4)?,
            refs: texts(r.get(5)?),
            folded_into: r.get(6)?,
            closed: r.get(7)?,
            closed_why: r.get(8)?,
            created: r.get(9)?,
            updated: r.get(10)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// The policy in force for a project, with the round in which it took effect.
pub(crate) fn policy(conn: &Connection, project: &str) -> Result<Option<PolicyInForce>> {
    Ok(conn
        .query_row(
            "SELECT p.work, p.round, r.ended_at, p.policy, coalesce(r.cell, '') FROM policy_current p
             JOIN rounds r ON r.work = p.work AND r.n = p.round WHERE p.project = ?1",
            [project],
            |r| {
                Ok(PolicyInForce {
                    work: r.get(0)?,
                    round: r.get(1)?,
                    cell: r.get(4)?,
                    at: r.get(2)?,
                    policy: json(r.get(3)?).unwrap_or(Value::Null),
                })
            },
        )
        .optional()?)
}

/// The policy a wake is setting: its latest study, and its latest text with the readings of it, unless a newer study
/// has sent the text back to be written again.
fn setting(conn: &Connection, work: i64) -> Result<PolicySetting> {
    let study = conn
        .query_row(
            &format!(
                "SELECT n, cell, ended_at, output FROM rounds WHERE work = ?1 AND cell IN {} AND transition IS NOT NULL
                   AND json_extract(output, '$.outcome.study') IS NOT NULL
                 ORDER BY n DESC LIMIT 1",
                db::cells_sql("architect", &["P2"])
            ),
            [work],
            |r| {
                Ok(Said {
                    work,
                    round: r.get(0)?,
                    cell: r.get(1)?,
                    at: r.get(2)?,
                    value: returned(r.get(3)?, Some("study")),
                })
            },
        )
        .optional()?;
    // The cell of each round of the wake, which each part of the policy being set names.
    let cells: HashMap<i64, String> = {
        let mut st = conn.prepare("SELECT n, coalesce(cell, '') FROM rounds WHERE work = ?1")?;
        st.query_map([work], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?
    };
    let cell = |n: i64| cells.get(&n).cloned().unwrap_or_default();
    let draft = conn
        .query_row(
            "SELECT round, policy, probes, readings FROM policy_trials WHERE work = ?1",
            [work],
            |r| {
                let readings = json(r.get(3)?)
                    .and_then(|v| v.as_array().cloned())
                    .unwrap_or_default()
                    .into_iter()
                    .map(|x| {
                        let round = x["round"].as_i64().unwrap_or_default();
                        Reading {
                            round,
                            cell: cell(round),
                            reading: x["reading"].clone(),
                            verdict: x["verdict"].clone(),
                        }
                    })
                    .collect();
                let round: i64 = r.get(0)?;
                Ok(Draft {
                    round,
                    cell: cell(round),
                    policy: json(r.get(1)?).unwrap_or(Value::Null),
                    probes: json(r.get(2)?).unwrap_or(Value::Null),
                    readings,
                })
            },
        )
        .optional()?
        .filter(|d| study.as_ref().is_none_or(|s| d.round > s.round));
    Ok(PolicySetting { work, study, draft })
}

/// The latest round of a project's architect at a cell that answered, by what it answered.
fn latest_of(conn: &Connection, project: &str, cell: &str) -> Result<Option<Said>> {
    Ok(conn
        .query_row(
            &format!(
                "SELECT r.work, r.n, r.cell, r.ended_at, r.output FROM rounds r JOIN works w ON w.id = r.work
                 WHERE w.graph = 'architect' AND w.project = ?1 AND r.cell IN {} AND r.transition IS NOT NULL
                   AND r.transition NOT IN ('tools', 'search', 'pause', 'ask_owner')
                 ORDER BY r.work DESC, r.n DESC LIMIT 1",
                db::cells_sql("architect", &[cell])
            ),
            [project],
            |r| {
                Ok(Said {
                    work: r.get(0)?,
                    round: r.get(1)?,
                    cell: r.get(2)?,
                    at: r.get(3)?,
                    value: returned(r.get(4)?, None),
                })
            },
        )
        .optional()?)
}

/// What to do next under the policy, as a project's architect last planned it, with the matters its rows became: its
/// round queued one for each thing missing and then one for each place to set straight, in their order.
fn next(conn: &Connection, project: &str) -> Result<NextPlan> {
    let plan = latest_of(conn, project, "N1")?;
    let matters: Vec<i64> = match &plan {
        Some(f) => {
            let mut st = conn.prepare(
                "SELECT id FROM architect_matters WHERE work = ?1 AND round = ?2 AND kind = 'set_straight' ORDER BY id",
            )?;
            st.query_map([f.work, f.round], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?
        }
        None => Vec::new(),
    };
    Ok(NextPlan { plan, matters })
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
        let first = compiled.graph("engineer").unwrap().cell("R1").unwrap().inputs[0]
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
            run(
                "INSERT INTO item_links (item, depends_on, needs, kind) VALUES (1, 2, 'its schema', 'blocking')",
                &[],
            )?;
            run("INSERT INTO dispatch_queue (item, position) VALUES (2, 1.0)", &[])?;
            run("INSERT INTO works (item, flow_version, graph, at) VALUES (1, 1, 'engineer', 'R1')", &[])?;
            run("INSERT INTO sessions (work, line, cli, cli_session) VALUES (1, 'engineer', 'claude', 's1')", &[])?;
            run(
                "INSERT INTO rounds (work, n, cell, input, output, transition, config_commit, started_at, ended_at)
                 VALUES (1, 1, 'S0', '{}', '{}', 'started', 'c', 10, 20)",
                &[],
            )?;
            let input = input.to_string();
            run(
                "INSERT INTO rounds (work, n, cell, session, input, output, rejected, config_commit, model, usage, started_at, ended_at)
                 VALUES (1, 2, 'R1', 1, ?1, '{\"progress\":\"looked\"}', '{\"format\":\"no outcome\"}', 'c', 'm', '{\"cost_usd\":0.5}', 30, 40)",
                &[&input],
            )?;
            run(
                "INSERT INTO rounds (work, n, cell, session, input, config_commit, started_at) VALUES (1, 3, 'R1', 1, ?1, 'c', 50)",
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

    // The round that runs says whether a tool of it is under way: started and not ended; one only queued, or ended, is
    // not.
    #[test]
    fn the_running_round_says_whether_its_tool_is_under_way() {
        let mut s = setup("tool");
        let cache = Flows::default();
        let tool = |s: &Setup| {
            works(s.db.conn(), &cache).unwrap()[0]
                .running
                .as_ref()
                .map(|r| (r.n, r.tool))
        };
        assert_eq!(tool(&s), Some((3, false)));
        s.db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO tool_runs (work, round, tool, args, attempt, queued_at, started_at)
                 VALUES (1, 3, 'full_ci', '{}', 1, 51, 52)",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        assert_eq!(tool(&s), Some((3, true)));
        s.db.write(Actor::Engine, |tx| {
            tx.execute("UPDATE tool_runs SET result = '{}', ended_at = 60 WHERE round = 3", [])?;
            Ok(())
        })
        .unwrap();
        assert_eq!(tool(&s), Some((3, false)));
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
            tx.execute(
                "INSERT INTO pings (session, waited, usage, started_at, ended_at) VALUES (1,
                   '{\"cell\":\"R1\",\"on\":\"pause\",\"what\":\"to look\"}',
                   '{\"cost_usd\":0.25,\"usage\":{\"cache_read_input_tokens\":9000,\"cache_creation_input_tokens\":20}}',
                   41, 42)",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        let cache = Flows::default();
        let cell = s.compiled.graph("engineer").unwrap().cell("R1").unwrap().clone();
        let ws = works(s.db.conn(), &cache).unwrap();
        assert_eq!(ws.len(), 1);
        let w = &ws[0];
        assert_eq!(w.state, WorkState::Running);
        assert_eq!(w.at_title.as_deref(), Some(cell.title.as_str()));
        assert_eq!(w.stage, cell.stage);
        assert_eq!((w.rounds, w.sent_back, w.failed), (3, 1, 0));
        // The work's cost counts the keep-warm pings of its sessions.
        assert_eq!((w.started_at, w.last_at, w.cost_usd), (Some(10), Some(50), 0.75));
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
        // The first round of a session says it started it, and why; the work lists its sessions and their pings.
        assert_eq!((back.session, back.new_session.as_deref()), (Some(1), Some("line")));
        assert_eq!(
            (d.rounds[2].session, d.rounds[2].new_session.as_deref()),
            (Some(1), None)
        );
        assert_eq!(
            d.sessions,
            [WorkSession {
                id: 1,
                line: "engineer".into(),
                started: "line".into(),
                ended: None
            }]
        );
        let p = &d.pings[0];
        assert_eq!(
            (p.line.as_str(), p.cell.as_str(), p.on.as_str(), p.what.as_str()),
            ("engineer", "R1", "pause", "to look")
        );
        assert_eq!((p.cost_usd, p.read, p.written), (Some(0.25), 9000, 20));
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
    // items its item depends on. A paused work says both who paused it and what it waits for; one the architect
    // stopped says it was the architect, with the architect's reason.
    #[test]
    fn a_work_says_what_it_waits_for_from_what_is_open() {
        let mut s = setup("waits");
        let g = s.compiled.graph("engineer").unwrap();
        let asking = g.cells.iter().find(|c| is_ask(c)).unwrap().id.clone();
        let (waiting, row) = g
            .cells
            .iter()
            .filter(|c| c.id != g.start)
            .find_map(|c| {
                c.rows
                    .iter()
                    .find(|r| r.waits && r.to == c.id)
                    .map(|r| (c.id.clone(), r.name.clone()))
            })
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
        s.db.write(Actor::Architect, |tx| {
            tx.execute(
                "UPDATE works SET paused = ?1, paused_by = 'engine' WHERE id = 3",
                [format!("{}: it waits for the parser", db::STOPPED_BY_ARCHITECT)],
            )?;
            Ok(())
        })
        .unwrap();
        let ws = works(s.db.conn(), &cache).unwrap();
        assert_eq!(
            (ws[2].paused_by, ws[2].paused.as_deref()),
            (Some(PausedBy::Architect), Some("it waits for the parser"))
        );
    }

    // A work at the cell that asks the architect waits for the architect's reply to the matter its round queued, and
    // is ready once the reply has come.
    #[test]
    fn a_work_asking_the_architect_waits_for_its_reply() {
        let mut s = setup("architect-reply");
        let g = s.compiled.graph("engineer").unwrap();
        let asking = g
            .cells
            .iter()
            .find(|c| c.step.as_deref() == Some("ask_architect"))
            .unwrap()
            .id
            .clone();
        s.db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO works (item, flow_version, graph, at) VALUES (2, 1, 'engineer', ?1)",
                [&asking],
            )?;
            tx.execute(
                "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES (2, 1, ?1, '{}', 'c', 60)",
                [&asking],
            )?;
            tx.execute(
                "INSERT INTO architect_matters (project, kind, what, body, item, work, round, at)
                 VALUES ('p', 'engineer_question', 'Work 2 asks', '{}', 2, 2, 1, 60)",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        let cache = Flows::default();
        let ws = works(s.db.conn(), &cache).unwrap();
        assert_eq!(
            (ws[1].state, &ws[1].waits),
            (
                WorkState::Asking,
                &Some(Waits::Architect {
                    matter: 1,
                    project: None
                })
            )
        );
        s.db.write(Actor::Architect, |tx| {
            tx.execute(
                "UPDATE architect_matters SET reply = '{\"text\": \"go on\"}' WHERE id = 1",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        let ws = works(s.db.conn(), &cache).unwrap();
        assert_eq!((ws[1].state, &ws[1].waits), (WorkState::Ready, &None));
    }

    // What a work waits for is read from its open blocker: a question handed to a related project's architect waits for
    // that architect's reply to the matter in its queue, which names the project; a wait for items names them.
    #[test]
    fn a_work_s_wait_names_the_holder_of_its_blocker() {
        let mut s = setup("blocker-holder");
        let g = s.compiled.graph("engineer").unwrap();
        let asking = g
            .cells
            .iter()
            .find(|c| c.step.as_deref() == Some("ask_architect"))
            .unwrap()
            .id
            .clone();
        s.db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO works (item, flow_version, graph, at) VALUES (2, 1, 'engineer', ?1)",
                [&asking],
            )?;
            tx.execute(
                "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES (2, 1, ?1, '{}', 'c', 60)",
                [&asking],
            )?;
            tx.execute(
                "INSERT INTO architect_matters (project, kind, what, body, item, work, round, at)
                 VALUES ('q', 'engineer_question', 'Work 2 of p asks', '{}', 2, 2, 1, 60)",
                [],
            )?;
            tx.execute(
                "INSERT INTO blockers (work, cell, round, kind, what, holder, project, matter, at, since, moves)
                 VALUES (2, 'R1', NULL, 'question', 'Work 2 asks', 'architect', 'q', 1, 50, 60,
                         '[{\"from\": {\"holder\": \"architect\", \"project\": \"p\"}}]')",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        let cache = Flows::default();
        let ws = works(s.db.conn(), &cache).unwrap();
        assert_eq!(
            ws[1].waits,
            Some(Waits::Architect {
                matter: 1,
                project: Some("q".into())
            })
        );
        s.db.write(Actor::Engine, |tx| {
            tx.execute(
                "UPDATE blockers SET holder = 'items', project = NULL, matter = NULL,
                   items = '[{\"item\": 1, \"project\": \"p\"}]' WHERE work = 2",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        let ws = works(s.db.conn(), &cache).unwrap();
        assert_eq!(ws[1].waits, Some(Waits::Items { items: vec![1] }));
    }

    // A round recorded with its message gives each conditional input, given or referred to, and the call; a step's inputs,
    // recorded by their sources, are named by the cell's definition; a failed run says its kind.
    #[test]
    fn a_round_gives_its_message_conditional_inputs_and_call() {
        let mut s = setup("message");
        let cell = s.compiled.graph("engineer").unwrap().cell("R1").unwrap().clone();
        let input = json!({
            "message": "# R1-4 · cell R1 · Research",
            "conditional": [
                { "address": "rule:ground", "hash": "h1", "given": true },
                { "address": "R1-2·R1·understanding", "hash": "h2", "given": false },
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
                 VALUES (1, 4, 'R1', 1, ?1, ?2, 'c', 60, 70)",
                [input.to_string(), failed.to_string()],
            )?;
            tx.execute(
                "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES (1, 5, 'R1', ?1, 'c', 80)",
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
        assert_eq!(r.message.as_deref(), Some("# R1-4 · cell R1 · Research"));
        assert_eq!(
            r.conditional
                .iter()
                .map(|m| (m.address.as_str(), m.given))
                .collect::<Vec<_>>(),
            [("rule:ground", true), ("R1-2·R1·understanding", false)]
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
                 VALUES (1, 6, 'A1', '{}', 'c', 100, 110)",
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
        let mut s = setup("items");
        let cache = Flows::default();
        let all = items(s.db.conn(), &cache).unwrap();
        let states: Vec<ItemState> = all.items.iter().map(|i| i.state).collect();
        assert_eq!(states, [ItemState::Working, ItemState::Queued]);
        assert_eq!(all.items[0].at.as_deref(), Some("R1"));
        assert_eq!(all.links.len(), 1);
        assert_eq!(all.links[0].kind, "blocking");
        // A work that runs goes on whatever it depends on: nothing holds it back.
        assert!(all.items[0].blocked_by.is_empty());
        let first = item(s.db.conn(), &cache, 1).unwrap();
        assert_eq!(first.depends_on[0].item.id, 2);
        assert_eq!(first.depends_on[0].needs, "its schema");
        assert_eq!(first.depends_on[0].kind, "blocking");
        assert_eq!(first.works.len(), 1);
        let second = item(s.db.conn(), &cache, 2).unwrap();
        assert_eq!(second.depended_on_by[0].item.id, 1);
        assert!(second.works.is_empty());
        assert_eq!(flows(s.db.conn()).unwrap()[0].works, 1);
        // The stage its work is at, among the stages of its graph in their order; nothing for an item never worked on.
        assert_eq!(all.items[0].stage.as_deref(), Some("research"));
        assert_eq!(all.items[0].stages[..2], ["start", "research"]);
        assert!(all.items[1].stage.is_none() && all.items[1].stages.is_empty());
        assert_eq!((all.items[0].open_asks, all.items[0].open_design_cards), (0, 0));
        // An item that would start but for an unmerged item it depends on by a blocking dependency waits for it.
        s.db.write(Actor::Owner, |tx| {
            tx.execute(
                "INSERT INTO items (project, title, problem, why, deliver, level) VALUES ('p', 'Third', 'x', 'y', 'change', '1')",
                [],
            )?;
            tx.execute(
                "INSERT INTO item_links (item, depends_on, needs, kind) VALUES (3, 2, 'its parser', 'blocking')",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        let all = items(s.db.conn(), &cache).unwrap();
        assert_eq!(all.items[2].state, ItemState::New);
        assert_eq!(all.items[2].blocked_by, [2]);
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
        let title = s.compiled.graph("engineer").unwrap().cell("R1").unwrap().title.clone();
        let d = item(s.db.conn(), &Flows::default(), 1).unwrap();
        assert_eq!(
            d.design_cards[0].replaced,
            Some(Replacement {
                design_card: 2,
                round: 3,
                work: 1,
                cell: Some("R1".into()),
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

    // What a round held is found by the hash its record names it with: as the round of the work that gave it gave it,
    // text or JSON; or, never given, as the session's own return produced it, though the address names a round by a
    // number no round has now.
    #[test]
    fn a_held_input_is_found_by_its_hash() {
        let mut s = setup("held");
        let rule = json!("Facts first.\n\n- Check what you claim.");
        let rows = json!([{ "a": 1 }]);
        let design = json!("Keep one table.");
        let message = format!(
            "# R1-4 · cell R1 · Research\n\n## Given\n\n### rule:ground\n{}\n\n### R1-1·engine·rows\n{rows}\n\n## Task\n\nDo it.\n",
            rule.as_str().unwrap()
        );
        let input = json!({ "message": message, "conditional": [
            { "address": "rule:ground", "hash": flow::formats::hash(&rule), "given": true },
            { "address": "R1-1·engine·rows", "hash": flow::formats::hash(&rows), "given": true },
        ] });
        let output = json!({ "outcome": { "design": design }, "progress": "p" });
        s.db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO rounds (work, n, cell, session, input, output, transition, config_commit, started_at, ended_at)
                 VALUES (1, 4, 'R1', 1, ?1, ?2, 'done', 'c', 60, 70)",
                [input.to_string(), output.to_string()],
            )?;
            Ok(())
        })
        .unwrap();
        let cache = Flows::default();
        let found = |v: &Value, address: &str| content(s.db.conn(), &cache, 1, &flow::formats::hash(v), address);
        assert_eq!(found(&rule, "rule:ground"), Ok(rule.clone()));
        assert_eq!(found(&rows, "R1-1·engine·rows"), Ok(rows.clone()));
        assert_eq!(found(&design, "R353·R1·design"), Ok(design.clone()));
        assert!(matches!(found(&json!("never"), "rule:ground"), Err(Error::NotFound(_))));
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
                 VALUES (1, 4, 'R1', 1, '{}', ?1, 'done', 'c', 90, 91)",
                [&ret],
            )?;
            Ok(())
        })
        .unwrap();
        let d = item(s.db.conn(), &Flows::default(), 1).unwrap();
        let at = |path: &str| {
            d.shown
                .iter()
                .find(|x| x.cell == "R1" && x.path == path)
                .map(|x| x.value.clone())
        };
        assert_eq!(at("understanding"), Some(json!({ "problem": "what is wrong" })));
        assert_eq!(at("progress"), Some(json!("read the code")));
        // The start's return holds nothing the flow shows.
        assert!(d.shown.iter().all(|x| x.cell == "R1"));
    }

    /// Two wakes of project p's architect: one over, which handled an engineer's question, and one at the owner's
    /// reading of the study of a new policy; the matters it has not handled, a hold on new works, an item change made
    /// and two observations, one closed.
    fn architect_setup(name: &str) -> Setup {
        let mut s = setup(name);
        let queued = json!({ "inputs": [{ "from": [{ "db": "architect_queue", "by": "project" }], "value": [
            { "project": "p", "matter": 1, "kind": "engineer_question", "detail": "split", "item": 1,
              "what": "Split #1?", "waits": "agent", "waited_ms": 5 },
        ] }], "given": {} });
        let none = json!({ "inputs": [{ "from": [{ "db": "architect_queue", "by": "project" }], "value": [] }],
                           "given": {} });
        let out = |v: Value| json!({ "outcome": v, "progress": "p" }).to_string();
        let wake = [
            ("W0", queued.to_string(), Some("{}".to_string()), Some("woke")),
            ("W1", "{}".into(), Some("{}".into()), Some("not_understood")),
            (
                "U1",
                "{}".into(),
                Some(out(
                    json!({ "state": [{ "line": "l", "where": "w", "stuck": "no", "recent": "r", "grounds": "g" }],
                    "unclear": [], "changed": "c" }),
                )),
                Some("understood"),
            ),
            (
                "P1",
                "{}".into(),
                Some(out(
                    json!({ "against": [{ "article": "Article 2", "status": "broken", "where": "w",
                    "grounds": "e" }], "verdict": { "verdict": "set_again", "reasons": [] } }),
                )),
                Some("set_again"),
            ),
            (
                "N1",
                "{}".into(),
                Some(out(json!({ "fit": { "verdict": "fits", "why": "w" } }))),
                Some("fits"),
            ),
            (
                "Q0",
                "{}".into(),
                Some(json!({ "queue": { "verdict": "one", "matter": 1 } }).to_string()),
                Some("one"),
            ),
            ("Q2", "{}".into(), Some("{}".into()), Some("to_crux")),
            (
                "H2",
                "{}".into(),
                Some(out(json!({ "crux": { "root": "itself", "question": "q" } }))),
                Some("found"),
            ),
            (
                "H3",
                "{}".into(),
                Some(out(
                    json!({ "judgment": { "chosen": "one" }, "reply": { "text": "t", "why": "w" } }),
                )),
                Some("done"),
            ),
            ("B1", "{}".into(), Some("{}".into()), Some("done")),
            (
                "B4",
                "{}".into(),
                Some(out(json!({ "rethink": { "verdict": "go_on", "why": "w" } }))),
                Some("go_on"),
            ),
            (
                "Q0",
                "{}".into(),
                Some(json!({ "queue": { "verdict": "none" } }).to_string()),
                Some("none"),
            ),
            (
                "E1",
                "{}".into(),
                Some(out(json!({ "learned": "L", "notes": [{ "text": "N" }] }))),
                Some("wrapped_up"),
            ),
            ("E2", "{}".into(), Some("{}".into()), None),
        ];
        let setting = [(
            "P2",
            none.to_string(),
            Some(out(json!({ "study": { "intent": "i" } }))),
            Some("studied"),
        )];
        s.db.write(Actor::Engine, |tx| {
            let run = |sql: &str, args: &[&dyn db::rusqlite::ToSql]| tx.execute(sql, args).map(|_| ());
            run(
                "INSERT INTO works (item, flow_version, graph, at, project) VALUES (NULL, 1, 'architect', 'E2', 'p'),
                 (NULL, 1, 'policy', 'P3', 'p'), (NULL, 1, 'architect', 'E2', 'q')",
                &[],
            )?;
            for (work, rounds) in [(2_i64, &wake[..]), (3, &setting[..])] {
                for (i, (cell, input, output, transition)) in rounds.iter().enumerate() {
                    let n = i as i64 + 1;
                    let at = work * 100 + n;
                    run(
                        "INSERT INTO rounds (work, n, cell, input, output, transition, config_commit, started_at,
                         ended_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'c', ?7, ?7)",
                        &[&work, &n, cell, input, output, transition, &at],
                    )?;
                }
            }
            run(
                "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES (3, 2, 'P3', '{}', 'c', 400)",
                &[],
            )?;
            let study = json!({ "from": "architect", "project": "p", "kind": "study", "work": 3, "cell": "P3",
                "study": { "intent": "i" }, "choices": [["satisfied", "s"]] })
            .to_string();
            run("INSERT INTO owner_asks (work, round, body) VALUES (3, 2, ?1)", &[&study])?;
            // The setting stopped for the owner to read its study: its blocker, which the ask stands for.
            run(
                "INSERT INTO blockers (work, cell, round, kind, what, holder, ask, at, since)
                 VALUES (3, 'P2', 1, 'owner_ask', 'The study waits for the owner', 'owner', 1, 400, 400)",
                &[],
            )?;
            let held = json!({ "from": "architect", "project": "p", "kind": "ask", "work": 2, "cell": "A3",
                "asked": [], "holds": [3] })
            .to_string();
            run("INSERT INTO owner_asks (work, round, body) VALUES (2, 9, ?1)", &[&held])?;
            run(
                "INSERT INTO architect_matters (project, kind, detail, what, body, item, work, round, at, handled_work,
                 handled_round) VALUES ('p', 'engineer_question', 'split', 'Split #1?', '{}', 1, 1, 1, 5, 2, 9)",
                &[],
            )?;
            run(
                "INSERT INTO architect_matters (project, kind, detail, what, body, at)
                 VALUES ('p', 'owner_words', 'said', 'The owner said', '{\"text\":\"Put the parser first.\"}', 200)",
                &[],
            )?;
            run(
                "INSERT INTO architect_matters (project, kind, detail, what, body, item, work, round, at, asked,
                 resume_cell) VALUES ('p', 'report', 'other', 'Held', '{\"urgent\":true}', 1, 1, 1, 220, 2, 'H4')",
                &[],
            )?;
            run(
                "INSERT INTO architect_matters (project, kind, what, body, at) VALUES ('q', 'owner_words', 'Q', '{}', 230)",
                &[],
            )?;
            run("INSERT INTO dispatch_holds (project, reason, since) VALUES ('p', 'policy', 300)", &[])?;
            run("INSERT INTO dispatch_holds (project, reason, since) VALUES ('q', 'architect', 300)", &[])?;
            run(
                "INSERT INTO architect_item_changes (work, round, matter, change, state, outcome)
                 VALUES (2, 9, 1, '{\"kind\":\"hold\",\"item\":2,\"reason\":\"r\"}', 'made', NULL)",
                &[],
            )?;
            run(
                "INSERT INTO observations (project, what, tags, source) VALUES ('p', 'Slow.', '[\"perf\"]', 's'),
                 ('p', 'Old.', '[]', 's'), ('q', 'Theirs.', '[]', 's')",
                &[],
            )?;
            run("UPDATE observations SET closed = 'outdated', closed_why = 'Gone.' WHERE id = 2", &[])?;
            run("INSERT INTO owner_messages (work, round, text) VALUES (2, 9, 'Learned L'), (1, 2, 'told')", &[])?;
            Ok(())
        })
        .unwrap();
        s
    }

    // The owner's conversations: the one-to-ones with the architects and wf-agents' own first, then a group for each
    // work. The architect's one-to-one holds the owner's words, still waiting for it, and its asks, waiting for the
    // owner where they were asked, beside a card for each of its matters, where it stands; what a wake told the owner
    // while it handled a matter of a work, and what the work's engineer told the owner and asked the architect, are in
    // the work's group, with a card for each stage the work went through. Another project's are its own.
    #[test]
    fn the_owner_s_conversations_gather_what_was_said_by_subject() {
        let s = architect_setup("talk");
        let t = crate::Talks {
            projects: vec!["p".into(), "q".into()],
            own: Some("p".into()),
            sets: Vec::new(),
        };
        let all = crate::conversations(s.db.conn(), &t).unwrap();
        let keys: Vec<&str> = all.iter().map(|c| c.key.as_str()).collect();
        assert_eq!(keys[..4], ["architect:p", "architect:q", "wf-agents", "work:1"]);
        let p = all.iter().find(|c| c.key == "architect:p").unwrap();
        assert_eq!(
            (p.project.as_deref(), p.waiting, p.unread, p.work),
            (Some("p"), 2, 0, None)
        );
        let w = all.iter().find(|c| c.key == "work:1").unwrap();
        assert_eq!(
            (w.kind.as_str(), w.work, w.item, w.title.as_deref(), w.over),
            ("work", Some(1), Some(1), Some("First"), false)
        );
        assert!(w.unread > 0, "{w:?}");
        let d = crate::conversation(s.db.conn(), &Flows::default(), &t, "architect:p").unwrap();
        type Seen<'a> = (&'a str, &'a str, &'a str, i64, bool, bool);
        let seen: Vec<Seen> = d
            .entries
            .iter()
            .filter(|e| e.source != "matter" && e.source != "policy")
            .map(|e| {
                (
                    e.source.as_str(),
                    e.sender.as_str(),
                    e.text.as_str(),
                    e.at,
                    e.waiting,
                    e.pending,
                )
            })
            .collect();
        assert_eq!(
            seen,
            [
                ("words", "owner", "Put the parser first.", 200, false, true),
                ("ask", "architect", "", 209, true, false),
                ("ask", "architect", "", 400, true, false)
            ]
        );
        assert_eq!(d.asks.iter().map(|a| a.id).collect::<Vec<_>>(), [1, 2]);
        assert!(d.design_cards.is_empty());
        let cards: Vec<(i64, &str, Option<i64>)> = d
            .entries
            .iter()
            .filter_map(|e| e.matter_card.as_ref())
            .map(|m| (m.matter, m.state.as_str(), m.work))
            .collect();
        assert_eq!(cards, [(1, "done", Some(1)), (2, "queued", None), (3, "held", Some(1))]);
        let work = crate::conversation(s.db.conn(), &Flows::default(), &t, "work:1").unwrap();
        let said: Vec<(&str, &str, &str)> = work
            .entries
            .iter()
            .map(|e| (e.source.as_str(), e.sender.as_str(), e.text.as_str()))
            .collect();
        assert!(
            said.contains(&("message", "architect", "Learned L"))
                && said.contains(&("message", "engineer", "told"))
                && said.contains(&("question", "engineer", "Split #1?")),
            "{said:?}"
        );
        assert!(work.entries.iter().any(|e| e.stage.is_some()), "{said:?}");
    }

    // An ask the owner answered is one entry of its conversation, the ask, which holds the answer: the answer is no
    // entry of its own beside it. The ask waits no longer, and nothing the owner did is unread.
    #[test]
    fn an_answered_ask_is_one_entry_holding_its_answer() {
        let mut s = architect_setup("answered");
        let t = crate::Talks {
            projects: vec!["p".into(), "q".into()],
            own: Some("p".into()),
            sets: Vec::new(),
        };
        let counts = |s: &Setup| {
            let all = crate::conversations(s.db.conn(), &t).unwrap();
            let p = all.iter().find(|c| c.key == "architect:p").unwrap();
            (p.waiting, p.unread)
        };
        let (waiting, unread) = counts(&s);
        let given = json!({ "by": "owner", "at": 500, "choice": "satisfied", "text": "Good." }).to_string();
        s.db.write(Actor::Owner, |tx| {
            tx.execute("UPDATE owner_asks SET answer = ?1 WHERE id = 1", [&given])?;
            Ok(())
        })
        .unwrap();
        let d = crate::conversation(s.db.conn(), &Flows::default(), &t, "architect:p").unwrap();
        let of_ask: Vec<&str> = d
            .entries
            .iter()
            .filter(|e| e.ask == Some(1))
            .map(|e| e.source.as_str())
            .collect();
        assert_eq!(of_ask, ["ask"]);
        let e = d.entries.iter().find(|e| e.ask == Some(1)).unwrap();
        assert!(!e.waiting);
        let answer = e.ask_detail.as_ref().and_then(|a| a.answer.clone());
        assert_eq!(answer.map(|a| a["choice"].clone()), Some(json!("satisfied")));
        assert_eq!(d.asks.iter().map(|a| a.id).collect::<Vec<_>>(), [2]);
        assert_eq!(counts(&s), (waiting - 1, unread));
    }

    // A place to set straight comes into the queue with the place as the check of what runs found it, and a design card
    // to look at with how the card stands; every other matter with neither.
    #[test]
    fn the_queue_says_the_place_to_set_straight_and_how_a_card_stands() {
        let mut s = architect_setup("queue_words");
        s.db.write(Actor::Engine, |tx| {
            let run = |sql: &str, args: &[&dyn db::rusqlite::ToSql]| tx.execute(sql, args).map(|_| ());
            run(
                "INSERT INTO design_cards (work, round, body, status, rulings)
                 VALUES (1, 1, '{}', 'ruled', '[{\"by\":\"owner\",\"decisions\":[{\"decision\":1,\"verdict\":\"confirm\"}]}]')",
                &[],
            )?;
            run(
                "INSERT INTO architect_matters (project, kind, detail, what, body, at) VALUES
                 ('p', 'set_straight', 'dependencies', 'Set #1 straight',
                  '{\"fix\":{\"where\":\"#1\",\"wrong\":\"It waits for #2.\",\"steps\":[\"Drop the link.\"]}}', 240),
                 ('p', 'design_card', NULL, 'Look at card 1', '{\"design_card\":1}', 250)",
                &[],
            )?;
            Ok(())
        })
        .unwrap();
        let q = queue(s.db.conn(), "p").unwrap();
        let fix = q.iter().find(|m| m.kind == "set_straight").unwrap();
        assert_eq!(
            fix.fix,
            Some(json!({ "where": "#1", "wrong": "It waits for #2.", "steps": ["Drop the link."] }))
        );
        assert_eq!(fix.card, None);
        let card = q.iter().find(|m| m.kind == "design_card").unwrap();
        assert_eq!(card.fix, None);
        assert_eq!(
            card.card,
            Some(MatterCard {
                id: 1,
                status: "ruled".into(),
                rulings: json!([{ "by": "owner", "decisions": [{ "decision": 1, "verdict": "confirm" }] }]),
                replaced_by: None,
            })
        );
        assert!(
            q.iter()
                .filter(|m| m.kind == "owner_words")
                .all(|m| m.fix.is_none() && m.card.is_none())
        );
    }

    // The works list leaves the wakes out; a wake reads as a work on no item.
    #[test]
    fn the_architect_wakes_are_its_tab_s_not_the_works_list_s() {
        let s = architect_setup("wakes");
        let cache = Flows::default();
        let ws = works(s.db.conn(), &cache).unwrap();
        assert_eq!(ws.iter().map(|w| w.id).collect::<Vec<_>>(), [1]);
        let d = work(s.db.conn(), &cache, 3).unwrap();
        assert_eq!((d.work.item, d.rounds.len(), d.asks.len()), (None, 2, 1));
        let r = round(s.db.conn(), &cache, &s.data, 3, 1).unwrap();
        assert!(r.limits.is_empty());
    }

    // A project's tab: its wakes newest first, with what woke each and what it handled; its queue, then what an ask
    // holds; its holds; its asks; its log newest first; the item changes made; the policy being set; where things
    // stand; its observations, open ones first. Another project's are not in it.
    #[test]
    fn the_architect_says_its_wakes_queue_log_policy_understanding_and_observations() {
        let s = architect_setup("architect");
        let tags = vec![Tag {
            name: "perf".into(),
            desc: "How fast".into(),
        }];
        let a = architect(
            s.db.conn(),
            &Flows::default(),
            "p",
            vec!["p".into(), "q".into()],
            tags,
            false,
        )
        .unwrap();
        assert_eq!((a.project.as_str(), a.projects.len(), a.tags.len()), ("p", 2, 1));
        assert_eq!(a.wakes.iter().map(|w| w.work.id).collect::<Vec<_>>(), [3, 2]);
        let (now, last) = (&a.wakes[0], &a.wakes[1]);
        assert_eq!(
            (now.work.state, &now.work.waits),
            (WorkState::Asking, &Some(Waits::Answer { ask: 1 }))
        );
        assert_eq!(last.work.state, WorkState::Done);
        assert_eq!(
            last.woke_for,
            [Queued {
                matter: 1,
                kind: "engineer_question".into(),
                detail: Some("split".into()),
                what: "Split #1?".into(),
                item: Some(1),
            }]
        );
        assert!(now.woke_for.is_empty());
        assert_eq!((last.handled, now.handled), (1, 0));
        assert_eq!((last.handling, now.handling), (None, None));

        type InQueue<'a> = (i64, Option<&'a str>, Option<i64>, bool, &'a str);
        let queue: Vec<InQueue> = a
            .queue
            .iter()
            .map(|m| (m.id, m.waits.as_deref(), m.asked, m.urgent, m.source.as_str()))
            .collect();
        assert_eq!(
            queue,
            [
                (2, Some("owner"), None, false, "owner"),
                (3, None, Some(2), true, "work")
            ]
        );
        assert_eq!(a.queue[1].title.as_deref(), Some("First"));
        assert_eq!(a.queue[1].resume_cell.as_deref(), Some("H4"));
        assert_eq!(
            a.holds,
            [DispatchHold {
                reason: "policy".into(),
                since: 300,
                why: None,
            }]
        );
        assert_eq!(a.asks.iter().map(|x| x.id).collect::<Vec<_>>(), [1, 2]);

        // The matter handled, by the round that closed it, with the item change it collected, which the batch made.
        let h = a.handled.iter().find(|h| h.matter == 1).unwrap();
        assert_eq!(
            (h.kind.as_str(), h.work, h.round, h.made, h.dropped),
            ("engineer_question", 2, 9, 1, 0)
        );
        assert!(h.why.is_none(), "{h:?}");
        assert_eq!(a.changes.len(), 1);
        assert_eq!(a.changes[0].change["kind"], "hold");

        assert_eq!(a.policy, None);
        let setting = a.setting.unwrap();
        assert_eq!(setting.work, 3);
        assert_eq!(
            setting.study.map(|x| (x.round, x.value)),
            Some((1, json!({ "intent": "i" })))
        );
        assert_eq!(setting.draft, None);

        // The check of the policy in force, in the policy's part.
        assert_eq!(a.check.as_ref().map(|x| (x.work, x.round)), Some((2, 4)));
        assert_eq!(a.check.as_ref().unwrap().value["against"][0]["status"], "broken");
        // The plan of what to do next, by its cell and round; it found everything fits and nothing missing, so it
        // queued no matter.
        let next = &a.next;
        assert_eq!(
            next.plan.as_ref().map(|x| (x.work, x.round, x.cell.as_str())),
            Some((2, 5, "N1"))
        );
        assert_eq!(next.plan.as_ref().unwrap().value["fit"]["verdict"], "fits");
        assert!(next.matters.is_empty());

        let seen: Vec<(i64, Option<&str>)> = a.observations.iter().map(|o| (o.id, o.closed.as_deref())).collect();
        assert_eq!(seen, [(1, None), (2, Some("outdated"))]);
        assert_eq!(a.observations[0].tags, ["perf"]);
    }
}
