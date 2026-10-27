//! The reads that fill the interface's types from the database. Where a work stands is derived here as the engine
//! derives it (`Engine::done` and `Engine::waiting`): from the work's row, its rounds and its flow version.

use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::io::Read as _;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

use db::rusqlite::types::ValueRef;
use db::rusqlite::{self, Connection, OptionalExtension, Row, params_from_iter};
use flow::format::Src;
use flow::{Compiled, CompiledCell, CompiledGraph};
use serde_json::Value;

use crate::view;
use crate::{
    Card, FlowVersion, FlowView, Interjection, ItemDetail, ItemLink, ItemState, ItemSummary, Items, KeptFile,
    LinkedItem, RoundDetail, RoundInput, RoundMaterial, RoundOutcome, RoundSummary, RunningRound, Session, ToolRun,
    WorkDetail, WorkState, WorkSummary,
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
}

/// What a work's summary needs of each of its rounds.
struct Light {
    id: i64,
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
                w.machine, w.worktree, w.branch, w.pr, w.pushed_head, w.paused, w.ended
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
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    let mut st = conn.prepare(&format!(
        "SELECT id, work, cell, transition, rejected, started_at, ended_at, json_extract(usage, '$.cost_usd')
         FROM rounds WHERE work IN (SELECT w.id FROM works w {filter}) ORDER BY id"
    ))?;
    let mut rounds: HashMap<i64, Vec<Light>> = HashMap::new();
    let rows = st.query_map(params_from_iter(args), |r| {
        Ok(Light {
            id: r.get(0)?,
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
    let waits = match (cell, rounds.last()) {
        (Some(c), Some(r)) => {
            r.cell.as_deref() == Some(c.id.as_str())
                && r.transition
                    .as_ref()
                    .is_some_and(|t| c.rows.iter().any(|x| &x.name == t && x.waits))
        }
        _ => false,
    };
    if waits { WorkState::Waiting } else { WorkState::Ready }
}

fn summary(w: &WorkRow, rounds: &[Light], g: Option<&CompiledGraph>, open_cards: i64) -> WorkSummary {
    let cell = g.and_then(|g| g.cell(&w.at));
    // A cell at no stage asks, answers or runs tools for another: the work is at the stage of the cell it stopped at.
    let stage = cell.and_then(|c| c.stage.clone()).or_else(|| {
        let stopped = w.resume_at.as_deref()?;
        g?.cell(stopped)?.stage.clone()
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
        state: state(w, rounds, g),
        paused: w.paused.clone(),
        ended: w.ended.clone(),
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
                id: r.id,
                cell: r.cell.clone(),
                started_at: r.started_at,
            }),
        cost_usd: rounds.iter().filter_map(|r| r.cost).sum(),
        open_cards,
    }
}

/// The works a filter selects, as summaries, with their rows.
fn summaries(conn: &Connection, flows: &Flows, filter: &str, args: &[i64]) -> Result<Vec<(WorkRow, WorkSummary)>> {
    let mut st = conn.prepare(
        "SELECT r.work, count(*) FROM cards c JOIN rounds r ON r.id = c.round WHERE c.status = 'open' GROUP BY r.work",
    )?;
    let open: HashMap<i64, i64> = st
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let mut graphs: HashMap<(i64, String), Option<CompiledGraph>> = HashMap::new();
    let mut out = Vec::new();
    for (w, rounds) in work_rows(conn, filter, args)? {
        let g = graphs
            .entry((w.flow_version, w.graph.clone()))
            .or_insert_with(|| flows.graph(conn, w.flow_version, &w.graph));
        let s = summary(&w, &rounds, g.as_ref(), open.get(&w.id).copied().unwrap_or(0));
        out.push((w, s));
    }
    Ok(out)
}

/// Every work, oldest first.
pub fn works(conn: &Connection, flows: &Flows) -> Result<Vec<WorkSummary>> {
    Ok(summaries(conn, flows, "", &[])?.into_iter().map(|(_, s)| s).collect())
}

const ROUND: &str = "SELECT r.id, r.cell, s.line, r.transition, r.rejected, r.started_at, r.ended_at, r.model,
       json_extract(r.usage, '$.cost_usd'), r.compactions, (SELECT count(*) FROM tool_runs t WHERE t.round = r.id),
       json_extract(r.output, '$.progress'),
       r.work, w.flow_version, w.graph, s.cli, s.cli_session, r.config_commit, r.input, r.output, r.usage
FROM rounds r JOIN works w ON w.id = r.work LEFT JOIN sessions s ON s.id = r.session";

fn round_summary(r: &Row<'_>) -> rusqlite::Result<RoundSummary> {
    let transition: Option<String> = r.get(3)?;
    let rejected = json(r.get(4)?);
    let ended_at: Option<i64> = r.get(6)?;
    Ok(RoundSummary {
        id: r.get(0)?,
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
    })
}

const CARD: &str = "SELECT c.id, c.round, r.work, w.item, c.kind, c.status, c.body, c.rulings, c.replaced_by
FROM cards c JOIN rounds r ON r.id = c.round JOIN works w ON w.id = r.work";

fn cards(conn: &Connection, filter: &str, id: i64) -> Result<Vec<Card>> {
    let mut st = conn.prepare(&format!("{CARD} {filter} ORDER BY c.id"))?;
    let rows = st.query_map([id], |r| {
        Ok(Card {
            id: r.get(0)?,
            round: r.get(1)?,
            work: r.get(2)?,
            item: r.get(3)?,
            kind: r.get(4)?,
            status: r.get(5)?,
            body: json(r.get(6)?).unwrap_or(Value::Null),
            rulings: json(r.get(7)?).unwrap_or(Value::Null),
            replaced_by: r.get(8)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

fn interjections(conn: &Connection, filter: &str, id: i64) -> Result<Vec<Interjection>> {
    let mut st = conn.prepare(&format!(
        "SELECT id, work, line, speaker, text, delivered_round FROM interjections {filter} ORDER BY id"
    ))?;
    let rows = st.query_map([id], |r| {
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
    let mut st = conn.prepare(&format!("{ROUND} WHERE r.work = ?1 ORDER BY r.id"))?;
    let rounds = st.query_map([id], round_summary)?.collect::<rusqlite::Result<_>>()?;
    Ok(WorkDetail {
        work: summary,
        machine: w.machine,
        worktree: w.worktree,
        pushed_head: w.pushed_head,
        resume_at: w.resume_at,
        waiting: json(w.waiting).unwrap_or(Value::Null),
        recorded: json(w.recorded).unwrap_or(Value::Null),
        rounds,
        cards: cards(conn, "WHERE r.work = ?1", id)?,
        interjections: interjections(conn, "WHERE work = ?1", id)?,
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

fn tool_runs(conn: &Connection, data: &Path, round: i64) -> Result<Vec<ToolRun>> {
    let mut st = conn.prepare(
        "SELECT id, tool, args, attempt, result, files, queued_at, started_at, ended_at FROM tool_runs
         WHERE round = ?1 ORDER BY id",
    )?;
    let rows = st.query_map([round], |r| {
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

/// A round in full. `data` is the data directory, under which the files its tools kept are found.
pub fn round(conn: &Connection, flows: &Flows, data: &Path, id: i64) -> Result<RoundDetail> {
    let row = conn
        .query_row(&format!("{ROUND} WHERE r.id = ?1"), [id], |r| {
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
        .ok_or_else(|| Error::NotFound(format!("no round {id}")))?;
    let (summary, work, flow_version, graph, session, config_commit, input, output, usage, rejected) = row;
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
    let materials = input["materials"]
        .as_array()
        .map(|xs| {
            xs.iter()
                .map(|m| RoundMaterial {
                    address: m["address"].as_str().unwrap_or_default().to_string(),
                    hash: m["hash"].as_str().unwrap_or_default().to_string(),
                    given: m["given"].as_bool().unwrap_or(false),
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(RoundDetail {
        work,
        flow_version,
        graph,
        session,
        config_commit,
        message: input["message"]
            .as_str()
            .or(input["prompt"].as_str())
            .map(str::to_string),
        materials,
        call: input.get("call").cloned(),
        inputs,
        input,
        output,
        rejected,
        usage,
        tool_runs: tool_runs(conn, data, id)?,
        cards: cards(conn, "WHERE c.round = ?1", id)?,
        interjections: interjections(conn, "WHERE delivered_round = ?1", id)?,
        round: summary,
    })
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
    Ok(view::view(&flows.get(conn, version)?))
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
    for w in &works {
        if let Some(i) = w.item {
            latest.insert(i, w);
        }
    }
    let mut st = conn.prepare(
        "SELECT i.id, i.project, i.title, i.deliver, i.level, i.cancelled, q.position
         FROM items i LEFT JOIN dispatch_queue q ON q.item = i.id ORDER BY i.id",
    )?;
    let items = st
        .query_map([], |r| {
            let id: i64 = r.get(0)?;
            let cancelled: Option<String> = r.get(5)?;
            let queued: Option<f64> = r.get(6)?;
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
                pr: w.and_then(|w| w.pr),
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
    let (problem, why, approach, acceptance, acceptance_round) = conn.query_row(
        "SELECT problem, why, approach, acceptance, acceptance_round FROM items WHERE id = ?1",
        [id],
        |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                json(r.get(3)?),
                r.get::<_, Option<i64>>(4)?,
            ))
        },
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
        cards: cards(conn, "WHERE w.item = ?1", id)?,
        item: summary,
    })
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
        let compiled = flow::compile(flow::FLOW, &flow::Context { relations }).unwrap();
        let first = compiled.graph("engineer").unwrap().cell("1").unwrap().inputs[0]
            .what
            .clone();
        let data = std::env::temp_dir().join(format!("wf-agents-api-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&data);
        for (dir, file, text) in [("logs", "run-1.log", "red"), ("tool-output", "round-2-1.txt", "long")] {
            std::fs::create_dir_all(data.join(dir)).unwrap();
            std::fs::write(data.join(dir).join(file), text).unwrap();
        }
        let input =
            json!({ "prompt": "Do it", "inputs": [{ "what": first, "value": "v", "held": false }], "engine": {} });
        let result = json!([
            0,
            format!("Too long to give here; saved to {}/tool-output/round-2-1.txt", "/old")
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
                "INSERT INTO rounds (work, cell, input, output, transition, config_commit, started_at, ended_at)
                 VALUES (1, '0', '{}', '{}', 'started', 'c', 10, 20)",
                &[],
            )?;
            let input = input.to_string();
            run(
                "INSERT INTO rounds (work, cell, session, input, output, rejected, config_commit, model, usage, started_at, ended_at)
                 VALUES (1, '1', 1, ?1, '{\"progress\":\"looked\"}', '{\"format\":\"no outcome\"}', 'c', 'm', '{\"cost_usd\":0.5}', 30, 40)",
                &[&input],
            )?;
            run(
                "INSERT INTO rounds (work, cell, session, input, config_commit, started_at) VALUES (1, '1', 1, ?1, 'c', 50)",
                &[&input],
            )?;
            let result = result.to_string();
            run(
                "INSERT INTO tool_runs (round, tool, args, attempt, result, files, queued_at)
                 VALUES (2, 'gh', '{}', 1, ?1, '[\"/old/logs/run-1.log\", \"/old/logs/gone.log\"]', 35)",
                &[&result],
            )?;
            Ok(())
        })
        .unwrap();
        Setup { db, compiled, data }
    }

    #[test]
    fn a_work_says_where_it_stands_and_what_each_round_came_to() {
        let s = setup("work");
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
        assert_eq!(w.running.as_ref().map(|r| r.id), Some(3));

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
        assert_eq!(work(s.db.conn(), &cache, 9), Err(Error::NotFound("no work 9".into())));

        // A round recorded before messages were gives its prompt as its message, and its inputs.
        let r = round(s.db.conn(), &cache, &s.data, 2).unwrap();
        assert_eq!(r.message.as_deref(), Some("Do it"));
        assert_eq!(r.inputs[0].from, cell.inputs[0].from);
        assert!(r.materials.is_empty() && r.call.is_none());
        assert_eq!(r.session.as_ref().map(|x| x.cli_session.as_str()), Some("s1"));
        let files: Vec<&str> = r.tool_runs[0].files.iter().map(|f| f.file.as_str()).collect();
        assert_eq!(files, ["logs/run-1.log", "tool-output/round-2-1.txt"]);
        assert_eq!(file(&s.data, "tool-output", "round-2-1.txt", 100).unwrap(), "long");
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

    // A round recorded with its message gives each material, given or referred to, and the call; a step's inputs,
    // recorded by their sources, are named by the cell's definition; a failed run says its kind.
    #[test]
    fn a_round_gives_its_message_materials_and_call() {
        let mut s = setup("message");
        let cell = s.compiled.graph("engineer").unwrap().cell("1").unwrap().clone();
        let input = json!({
            "message": "# R4 · cell 1 · Research",
            "materials": [
                { "address": "rule:ground", "hash": "h1", "given": true },
                { "address": "R2·1·understanding", "hash": "h2", "given": false },
            ],
            "call": { "command": ["claude", "-p"], "session": { "resumed": "s1" }, "engine": "c", "cli": "2" },
            "engine": {},
        });
        let step = json!({ "inputs": [{ "from": cell.inputs[0].from, "value": "v" }], "given": {} });
        let failed = json!({ "failed": { "kind": "transient", "what": "API Error: 529 Overloaded" } });
        s.db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO rounds (work, cell, session, input, rejected, config_commit, started_at, ended_at)
                 VALUES (1, '1', 1, ?1, ?2, 'c', 60, 70)",
                [input.to_string(), failed.to_string()],
            )?;
            tx.execute(
                "INSERT INTO rounds (work, cell, input, config_commit, started_at) VALUES (1, '1', ?1, 'c', 80)",
                [step.to_string()],
            )?;
            Ok(())
        })
        .unwrap();
        let cache = Flows::default();
        let r = round(s.db.conn(), &cache, &s.data, 4).unwrap();
        assert_eq!(r.message.as_deref(), Some("# R4 · cell 1 · Research"));
        assert_eq!(
            r.materials
                .iter()
                .map(|m| (m.address.as_str(), m.given))
                .collect::<Vec<_>>(),
            [("rule:ground", true), ("R2·1·understanding", false)]
        );
        assert_eq!(r.call.as_ref().unwrap()["session"]["resumed"], "s1");
        assert_eq!(r.round.outcome, RoundOutcome::Failed);
        assert_eq!(r.round.why.as_deref(), Some("transient: API Error: 529 Overloaded"));
        let step = round(s.db.conn(), &cache, &s.data, 5).unwrap();
        assert_eq!(step.message, None);
        assert_eq!(step.inputs[0].what, cell.inputs[0].what);
        assert_eq!(step.inputs[0].from, cell.inputs[0].from);
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
    }
}
