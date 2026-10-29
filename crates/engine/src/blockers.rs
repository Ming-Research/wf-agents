//! What a stopped task waits on: its blocker (`blockers`). Only a task stops, an engineer's work, and only at the cell
//! where its problem arose, on a blocker raised there and held by exactly one holder at a time: the work's advisor, a
//! project's architect, the owner, or the items it waits for, whose merge resolves it. The task stands at its holder's
//! cell while it waits, and every move of its blocker is a row of its graph: a row into a holder's cell raises it
//! (`raise_blocker`), a row out of it resolves it (`resolve_blocker`) or withdraws it (`withdraw_blocker`), and a work's
//! end withdraws the one still open ([`end_of`]). The engine moves a blocker only where a row says so.
//!
//! [`invariants`] checks what makes every chain close: every stopped task has exactly one open blocker, raised at the
//! cell it stopped at and held by the holder of the cell it stands at; every open blocker's holder exists; no ended
//! task, and no task that goes on, has an open blocker; and the items waited for form no cycle.

use std::collections::{BTreeMap, BTreeSet};

use db::rusqlite::{Connection, OptionalExtension, Transaction, params};
use flow::{CompiledGraph, Position};
use serde_json::{Value, json};

use crate::work::{Work, json_col, query_json};
use crate::{ENGINEER, Engine};

fn sql(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// A blocker as `blockers` holds it.
#[derive(Debug, Clone, PartialEq)]
pub struct Blocker {
    pub id: i64,
    pub work: i64,
    /// The cell that raised it, and the round of the task there.
    pub cell: String,
    pub round: Option<i64>,
    pub kind: String,
    pub what: String,
    /// Who holds it now: advisor, architect, owner or items.
    pub holder: String,
    /// The project whose architect holds it.
    pub project: Option<String>,
    /// The items that hold it, each with its project.
    pub items: Vec<(i64, String)>,
    pub matter: Option<i64>,
    pub ask: Option<i64>,
    pub at: i64,
    pub since: i64,
    pub ended: Option<String>,
}

const COLUMNS: &str = "id, work, cell, round, kind, what, holder, project, items, matter, ask, at, since, ended";

fn blocker(r: &db::rusqlite::Row<'_>) -> db::rusqlite::Result<Blocker> {
    let items: Option<String> = r.get(8)?;
    let items = json_col(items)
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(|x| {
            Some((
                x["item"].as_i64()?,
                x["project"].as_str().unwrap_or_default().to_string(),
            ))
        })
        .collect();
    Ok(Blocker {
        id: r.get(0)?,
        work: r.get(1)?,
        cell: r.get(2)?,
        round: r.get(3)?,
        kind: r.get(4)?,
        what: r.get(5)?,
        holder: r.get(6)?,
        project: r.get(7)?,
        items,
        matter: r.get(9)?,
        ask: r.get(10)?,
        at: r.get(11)?,
        since: r.get(12)?,
        ended: r.get(13)?,
    })
}

/// The task's open blocker, if it has one.
pub fn open(conn: &Connection, work: i64) -> Result<Option<Blocker>, String> {
    conn.query_row(
        &format!("SELECT {COLUMNS} FROM blockers WHERE work = ?1 AND ended IS NULL"),
        [work],
        blocker,
    )
    .optional()
    .map_err(sql)
}

/// A blocker by its number.
pub fn by_id(conn: &Connection, id: i64) -> Result<Blocker, String> {
    conn.query_row(&format!("SELECT {COLUMNS} FROM blockers WHERE id = ?1"), [id], blocker)
        .map_err(|e| format!("blocker {id}: {e}"))
}

/// The blocker a matter of the architect's queue stands for: the one that names it.
pub fn of_matter(conn: &Connection, matter: i64) -> Result<Option<Blocker>, String> {
    conn.query_row(
        &format!("SELECT {COLUMNS} FROM blockers WHERE matter = ?1 ORDER BY id DESC LIMIT 1"),
        [matter],
        blocker,
    )
    .optional()
    .map_err(sql)
}

/// Who holds a blocker: the holder, with the project of an architect and the items that hold it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Holder {
    pub holder: String,
    pub project: Option<String>,
    pub items: Vec<(i64, String)>,
}

impl Holder {
    pub fn named(holder: &str) -> Holder {
        Holder {
            holder: holder.into(),
            project: None,
            items: Vec::new(),
        }
    }

    pub fn architect(project: &str) -> Holder {
        Holder {
            holder: "architect".into(),
            project: Some(project.into()),
            items: Vec::new(),
        }
    }

    fn items_json(&self) -> Option<String> {
        (self.holder == "items").then(|| {
            Value::Array(
                self.items
                    .iter()
                    .map(|(i, p)| json!({ "item": i, "project": p }))
                    .collect(),
            )
            .to_string()
        })
    }

    fn said(&self) -> Value {
        match (self.holder.as_str(), &self.project) {
            ("architect", Some(p)) => json!({ "holder": "architect", "project": p }),
            ("items", _) => json!({
                "holder": "items",
                "items": self.items.iter().map(|(i, p)| json!({ "item": i, "project": p })).collect::<Vec<_>>(),
            }),
            (h, _) => json!({ "holder": h }),
        }
    }
}

/// What raises a blocker: its task, the cell and round that raised it, its kind, what it is, and its first holder.
pub struct Raise<'a> {
    pub work: i64,
    pub cell: &'a str,
    pub round: Option<i64>,
    pub kind: &'a str,
    pub what: String,
    pub holder: Holder,
}

/// Raises a task's blocker. A task stops on one blocker at a time: one already open is refused by the table.
pub fn raise(tx: &Transaction<'_>, r: &Raise<'_>) -> Result<i64, String> {
    let now = db::now();
    tx.execute(
        "INSERT INTO blockers (work, cell, round, kind, what, holder, project, items, at, since)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
        params![
            r.work,
            r.cell,
            r.round,
            r.kind,
            r.what,
            r.holder.holder,
            r.holder.project,
            r.holder.items_json(),
            now
        ],
    )
    .map_err(|e| format!("work {}: a blocker raised at {}: {e}", r.work, r.cell))?;
    Ok(tx.last_insert_rowid())
}

/// Hands a blocker on to another holder, saying why and what the holder that hands it on found: the move is recorded,
/// and the new holder's time to act on it counts from now.
pub fn hand_over(tx: &Transaction<'_>, id: i64, to: &Holder, why: &str, found: &str) -> Result<(), String> {
    let b = by_id(tx, id)?;
    let from = Holder {
        holder: b.holder.clone(),
        project: b.project.clone(),
        items: b.items.clone(),
    };
    let now = db::now();
    let mv = json!({ "at": now, "from": from.said(), "to": to.said(), "why": why, "found": found });
    tx.execute(
        "UPDATE blockers SET holder = ?2, project = ?3, items = ?4, since = ?5, matter = NULL,
           moves = json_insert(moves, '$[#]', json(?6))
         WHERE id = ?1 AND ended IS NULL",
        params![id, to.holder, to.project, to.items_json(), now, mv.to_string()],
    )
    .map_err(sql)?;
    Ok(())
}

/// Counts the holder's wait again from now, as a task that chose to wait on does.
pub fn wait_on(tx: &Transaction<'_>, id: i64) -> Result<(), String> {
    tx.execute(
        "UPDATE blockers SET since = ?2 WHERE id = ?1 AND ended IS NULL",
        params![id, db::now()],
    )
    .map_err(sql)?;
    Ok(())
}

/// The matter in its architect's queue that stands for the blocker now.
pub fn set_matter(tx: &Transaction<'_>, id: i64, matter: i64) -> Result<(), String> {
    tx.execute("UPDATE blockers SET matter = ?2 WHERE id = ?1", params![id, matter])
        .map_err(sql)?;
    Ok(())
}

/// The ask put before the owner that stands for the blocker now.
pub fn set_ask(tx: &Transaction<'_>, id: i64, ask: i64) -> Result<(), String> {
    tx.execute("UPDATE blockers SET ask = ?2 WHERE id = ?1", params![id, ask])
        .map_err(sql)?;
    Ok(())
}

/// Records what is said about a blocker without resolving it or moving it: by whom, of what kind, and what.
pub fn say(tx: &Transaction<'_>, id: i64, by: &str, kind: &str, what: &Value) -> Result<(), String> {
    let said = json!({ "at": db::now(), "by": by, "kind": kind, "what": what });
    tx.execute(
        "UPDATE blockers SET said = json_insert(said, '$[#]', json(?2)) WHERE id = ?1",
        params![id, said.to_string()],
    )
    .map_err(sql)?;
    Ok(())
}

/// Resolves a blocker with what its holder decided and did. One that ended already is not resolved again: a
/// resolution that comes for a withdrawn blocker is recorded with it and moves nothing.
pub fn resolve(tx: &Transaction<'_>, id: i64, outcome: &Value) -> Result<(), String> {
    let b = by_id(tx, id)?;
    if b.ended.is_some() {
        return say(tx, id, &b.holder, "late", outcome);
    }
    tx.execute(
        "UPDATE blockers SET ended = 'resolved', outcome = ?2, ended_at = ?3 WHERE id = ?1",
        params![id, outcome.to_string(), db::now()],
    )
    .map_err(sql)?;
    Ok(())
}

/// Withdraws a blocker, saying why, and tells its holder: its matter leaves its architect's queue unless the architect
/// handled it, and its ask of the owner, still unanswered, needs no answer.
pub fn withdraw(tx: &Transaction<'_>, id: i64, why: &str) -> Result<(), String> {
    let b = by_id(tx, id)?;
    if b.ended.is_some() {
        return Ok(());
    }
    tx.execute(
        "UPDATE blockers SET ended = 'withdrawn', outcome = ?2, ended_at = ?3 WHERE id = ?1",
        params![id, json!({ "why": why }).to_string(), db::now()],
    )
    .map_err(sql)?;
    if let Some(m) = b.matter {
        tx.execute(
            "UPDATE architect_matters SET withdrawn = ?2 WHERE id = ?1 AND handled_round IS NULL AND withdrawn IS NULL",
            params![m, format!("the work it held stopped waiting: {why}")],
        )
        .map_err(sql)?;
    }
    if let Some(a) = b.ask {
        let answer = json!({ "by": "engine", "at": db::now(), "text": format!("No answer is needed: {why}.") });
        tx.execute(
            "UPDATE owner_asks SET answer = ?2 WHERE id = ?1 AND answer IS NULL",
            params![a, answer.to_string()],
        )
        .map_err(sql)?;
    }
    Ok(())
}

/// A task's end ends its open blocker: withdrawn with the end's reason, its holder told.
pub fn end_of(tx: &Transaction<'_>, work: i64, why: &str) -> Result<(), String> {
    if let Some(b) = open(tx, work)? {
        withdraw(tx, b.id, why)?;
    }
    Ok(())
}

/// The items an item depends on that are not merged yet, each with its project: those a work on it waits for.
pub fn unmerged(conn: &Connection, item: i64) -> Result<Vec<(i64, String)>, String> {
    let rows = query_json(
        conn,
        &format!(
            "SELECT d.id AS item, d.project FROM item_links l JOIN items d ON d.id = l.depends_on
             WHERE l.item = ?1 AND d.cancelled IS NULL AND NOT EXISTS (
               SELECT 1 FROM rounds x JOIN works y ON y.id = x.work
               WHERE y.item = d.id AND x.cell IN {} AND x.transition = 'merged')
             ORDER BY d.id",
            db::cells_sql(ENGINEER, &["M2"])
        ),
        &[&item],
        100_000,
    )?;
    Ok(rows
        .iter()
        .filter_map(|r| {
            Some((
                r["item"].as_i64()?,
                r["project"].as_str().unwrap_or_default().to_string(),
            ))
        })
        .collect())
}

/// Where a stopped task stands: the cell of its blocker's holder, the first of the cells it is in (those that wait
/// for an answer, then the one it is at) that holds a blocker, with the cell that raised it there: the cell before it,
/// or for a cell where the work waits, itself. None for a task that does not stand on a blocker. `took` is each row
/// the task's rounds took, oldest first, by the cell of the round.
pub fn standing(g: &CompiledGraph, pos: &Position, took: &[(&str, &str)]) -> Option<(String, String)> {
    let seq: Vec<&String> = pos.callers.iter().chain(std::iter::once(&pos.at)).collect();
    let i = seq.iter().position(|c| g.holder_cell(c).is_some())?;
    let h = g.holder_cell(seq[i])?;
    let last = took.last().copied();
    // A cell that raises the blocker with its own row that waits holds it once it took that row.
    if h.waits {
        let cell = g.cell(&h.cell)?;
        let waited = last.is_some_and(|(c, t)| c == h.cell && cell.rows.iter().any(|r| r.name == t && r.waits));
        if !waited || i + 1 != seq.len() {
            return None;
        }
        return Some((h.cell.clone(), h.cell.clone()));
    }
    // A wait for items is raised where the work waits; a question at the cell that asked: the one it was called from,
    // or for a holder's cell no cell calls, as the owner's reading of a policy's study, the cell that brought it there,
    // past the round trips it made since to the cells that answer it, as the one that answers what the owner says
    // while reading.
    let asks = h
        .holders
        .iter()
        .any(|x| x == "advisor" || x == "architect" || x == "owner");
    let raised = if !asks {
        seq[i].clone()
    } else if i > 0 {
        seq[i - 1].clone()
    } else {
        took.iter()
            .rev()
            .map(|(c, _)| *c)
            .find(|c| *c != h.cell && !g.is_called(c))
            .map(str::to_string)
            .or_else(|| pos.from.clone())
            .unwrap_or_else(|| seq[i].clone())
    };
    Some((h.cell.clone(), raised))
}

/// What breaks the invariants that make every chain of blockers close, one line for each, across every project:
/// every stopped task has exactly one open blocker, raised at the cell it stopped at and held by the holder of the
/// cell it stands at; no task that goes on and no ended task has one; every open blocker's holder exists (an
/// architect of a project in the configuration, whose open matter in that project's queue stands for it; each item
/// it waits for); and the items waited for form no cycle. Empty when they all hold.
pub fn invariants(e: &mut Engine) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let projects: BTreeSet<String> = e.config.config.projects.keys().cloned().collect();
    let all: Vec<Blocker> = {
        let mut st =
            e.db.conn()
                .prepare(&format!(
                    "SELECT {COLUMNS} FROM blockers WHERE ended IS NULL ORDER BY id"
                ))
                .map_err(sql)?;
        st.query_map([], blocker)
            .map_err(sql)?
            .collect::<Result<_, _>>()
            .map_err(sql)?
    };
    let mut by_work: BTreeMap<i64, Vec<&Blocker>> = BTreeMap::new();
    for b in &all {
        by_work.entry(b.work).or_default().push(b);
    }
    let works: Vec<i64> = query_json(e.db.conn(), "SELECT id FROM works ORDER BY id", &[], 1_000_000)?
        .iter()
        .filter_map(|r| r["id"].as_i64())
        .collect();
    for id in works {
        let w = e.work(id)?;
        let open = by_work.get(&id).cloned().unwrap_or_default();
        if open.len() > 1 {
            out.push(format!("work {id} has {} open blockers", open.len()));
        }
        if w.ended.is_some() {
            if !open.is_empty() {
                out.push(format!("work {id} ended with blocker {} open", open[0].id));
            }
            continue;
        }
        let g = e.graph_of(&w)?;
        let rounds = crate::work::rounds(e.db.conn(), id)?;
        let took: Vec<(&str, &str)> = rounds
            .iter()
            .filter_map(|r| Some((r.cell.as_deref()?, r.transition.as_deref()?)))
            .collect();
        let stands = standing(&g, &w.pos, &took);
        let done = e.done(&w)?;
        match (stands, open.first()) {
            (Some((cell, _)), None) if !done => {
                out.push(format!("work {id} stands at {cell} with no open blocker"));
            }
            (Some((cell, raised)), Some(b)) => {
                let h = g.holder_cell(&cell);
                if !h.is_some_and(|h| h.holders.contains(&b.holder)) && !nested(&g, &w, b) {
                    out.push(format!(
                        "work {id} stands at {cell}, whose holders are not blocker {}'s holder {}",
                        b.id, b.holder
                    ));
                }
                if b.cell != raised {
                    out.push(format!(
                        "work {id} stopped at {raised}, but its blocker {} was raised at {}",
                        b.id, b.cell
                    ));
                }
            }
            (None, Some(b)) => {
                out.push(format!("work {id} goes on at {} with blocker {} open", w.pos.at, b.id));
            }
            _ => {}
        }
    }
    for b in &all {
        match b.holder.as_str() {
            "architect" => {
                let p = b.project.as_deref().unwrap_or_default();
                if !projects.contains(p) {
                    out.push(format!(
                        "blocker {} is held by the architect of {p}, which no project is",
                        b.id
                    ));
                }
                if let Some(m) = b.matter {
                    // A matter the owner answered in the work's group left the queue with its reply, which the work
                    // takes at its next step.
                    let row: Option<(String, Option<String>)> =
                        e.db.conn()
                            .query_row(
                                "SELECT project, CASE WHEN reply IS NULL THEN withdrawn END FROM architect_matters
                                 WHERE id = ?1",
                                [m],
                                |r| Ok((r.get(0)?, r.get(1)?)),
                            )
                            .optional()
                            .map_err(sql)?;
                    match row {
                        Some((mp, None)) if mp == p => {}
                        Some((mp, _)) if mp != p => out.push(format!(
                            "blocker {} is held by the architect of {p}, but its matter {m} is in {mp}'s queue",
                            b.id
                        )),
                        Some(_) => out.push(format!("blocker {} is open, but its matter {m} was withdrawn", b.id)),
                        None => out.push(format!("blocker {} names no such matter: {m}", b.id)),
                    }
                }
            }
            "items" => {
                for (item, _) in &b.items {
                    let n: i64 =
                        e.db.conn()
                            .query_row("SELECT count(*) FROM items WHERE id = ?1", [item], |r| r.get(0))
                            .map_err(sql)?;
                    if n == 0 {
                        out.push(format!("blocker {} waits for no such item: {item}", b.id));
                    }
                }
            }
            _ => {}
        }
    }
    // The items waited for form no cycle: an item whose work waits for another, which waits, round to the first.
    let mut waits: BTreeMap<i64, BTreeSet<i64>> = BTreeMap::new();
    for b in all.iter().filter(|b| b.holder == "items") {
        let item: Option<i64> =
            e.db.conn()
                .query_row("SELECT item FROM works WHERE id = ?1", [b.work], |r| r.get(0))
                .map_err(sql)?;
        if let Some(i) = item {
            waits.entry(i).or_default().extend(b.items.iter().map(|x| x.0));
        }
    }
    if let Some(c) = cycle(&waits) {
        out.push(format!("the items waited for form a cycle: {c:?}"));
    }
    Ok(out)
}

/// A blocker its advisor holds while the advisor asks someone itself, as a flow version before hand-overs had it: the
/// task stands at the advisor's cell, which waits for the answer.
fn nested(g: &CompiledGraph, w: &Work, b: &Blocker) -> bool {
    b.holder == "advisor"
        && w.pos.callers.iter().any(|c| {
            g.holder_cell(c)
                .is_some_and(|h| h.holders.iter().any(|x| x == "advisor"))
        })
}

/// A cycle among the items waited for, as the items on it; none when there is none.
fn cycle(waits: &BTreeMap<i64, BTreeSet<i64>>) -> Option<Vec<i64>> {
    fn visit(
        n: i64,
        waits: &BTreeMap<i64, BTreeSet<i64>>,
        on: &mut Vec<i64>,
        done: &mut BTreeSet<i64>,
    ) -> Option<Vec<i64>> {
        if let Some(p) = on.iter().position(|x| *x == n) {
            return Some(on[p..].to_vec());
        }
        if !done.insert(n) {
            return None;
        }
        on.push(n);
        for m in waits.get(&n).into_iter().flatten() {
            if let Some(c) = visit(*m, waits, on, done) {
                return Some(c);
            }
        }
        on.pop();
        None
    }
    let mut done = BTreeSet::new();
    for n in waits.keys() {
        let mut on = Vec::new();
        if let Some(c) = visit(*n, waits, &mut on, &mut done) {
            return Some(c);
        }
    }
    None
}

/// How long an architect may hold a blocker before the task weighs whether to wait longer, in the architect's own
/// time: the time its wakes' rounds ran, while it was awake and not paused, since it took the blocker. Provisional:
/// two hours, until how long the architect takes has been measured.
pub const ARCHITECT_WAIT: i64 = 2 * 3_600_000;

/// The time the architect of `project` had to act on a blocker since `since`, up to `now`: the time its wakes' rounds
/// ran in that span, in milliseconds.
pub fn architect_time(conn: &Connection, project: &str, since: i64, now: i64) -> Result<i64, String> {
    conn.query_row(
        "SELECT coalesce(sum(min(coalesce(r.ended_at, ?3), ?3) - max(r.started_at, ?2)), 0)
         FROM rounds r JOIN works w ON w.id = r.work
         WHERE w.graph = 'architect' AND w.project = ?1 AND coalesce(r.ended_at, ?3) > ?2 AND r.started_at < ?3",
        params![project, since, now],
        |r| r.get(0),
    )
    .map_err(sql)
}

/// The engine's value `held`: where the question a task put to an architect stands, at the cell it waits at for it
/// (`A2`). The architect replied, or handed it to the owner or to a related project's architect; else an item the work
/// depended on was cancelled; else something was said to the work; else the architect that holds it is gone; else it
/// waited past its limit of the architect's time; else it waits, which no row takes.
pub fn held(e: &Engine, w: &Work) -> Result<String, String> {
    let conn = e.db.conn();
    let s = |x: &str| Ok(x.to_string());
    let Some(b) = open(conn, w.id)? else {
        return s("answered");
    };
    if let Some(m) = b.matter {
        let reply: Option<String> = conn
            .query_row("SELECT reply FROM architect_matters WHERE id = ?1", [m], |r| r.get(0))
            .optional()
            .map_err(sql)?
            .flatten();
        if let Some(reply) = json_col(reply) {
            return s(match reply["hand_over"]["to"].as_str() {
                Some("owner") => "to_owner",
                Some("architect") => "to_architect",
                _ => "answered",
            });
        }
    }
    let pending = |extra: &str| -> Result<i64, String> {
        conn.query_row(
            &format!(
                "SELECT count(*) FROM interjections WHERE work = ?1 AND line = 'engineer' AND delivered_round IS NULL
                   AND informs = 0 {extra}"
            ),
            [w.id],
            |r| r.get(0),
        )
        .map_err(sql)
    };
    if pending("AND cancelled_item IS NOT NULL")? > 0 {
        return s("cancelled");
    }
    if pending("")? > 0 {
        return s("interjection");
    }
    let project = b.project.clone().unwrap_or_default();
    if !e.config.config.projects.contains_key(&project) {
        return s("holder_gone");
    }
    if architect_time(conn, &project, b.since, db::now())? > ARCHITECT_WAIT {
        return s("overdue");
    }
    s("waiting")
}

/// The rows' actions on a task's blocker, as `engine.actions` names them: `raise_blocker`, `hand_over`,
/// `wait_on_blocker`, `resolve_blocker` and `withdraw_blocker`; false for any other action.
pub(crate) fn act(
    ctx: &crate::actions::Ctx,
    tx: &Transaction<'_>,
    name: &str,
    at: Option<&Value>,
) -> Result<bool, String> {
    match name {
        "raise_blocker" => raise_at(ctx, tx)?,
        "hand_over" => hand_over_at(ctx, tx, at)?,
        "wait_on_blocker" => {
            if let Some(b) = open(tx, ctx.work.id)? {
                let why = at.and_then(|v| v["why"].as_str()).unwrap_or_default();
                say(tx, b.id, "engineer", "waits_on", &json!({ "why": why }))?;
                wait_on(tx, b.id)?;
            }
        }
        "resolve_blocker" => resolve_at(ctx, tx)?,
        "withdraw_blocker" => withdraw_at(ctx, tx)?,
        _ => return Ok(false),
    }
    Ok(true)
}

/// `hand_over`: the task's blocker, handed on to the holder of the cell the row passes the work on to, with why and
/// what the holder that hands it on found, as its return says: the advisor's ask, the architect's reply, the owner's
/// answer, or the engineer's judgment not to wait longer. An architect it goes to is a related project's, as the
/// architect's reply names it, or the work's own; the architect it leaves is told, its matter withdrawn unless it
/// handled it.
fn hand_over_at(ctx: &crate::actions::Ctx, tx: &Transaction<'_>, at: Option<&Value>) -> Result<(), String> {
    let Some(b) = open(tx, ctx.work.id)? else { return Ok(()) };
    let target = ctx.to.at.clone();
    let h =
        holder_cell(ctx, &target).ok_or_else(|| format!("a row hands a blocker on to {target}, where none is held"))?;
    let at = at.cloned().unwrap_or(Value::Null);
    let own = ctx.item.as_ref().map(|i| i.project.clone()).unwrap_or_default();
    let to = match h.holders.first().map(String::as_str) {
        Some("architect") => {
            let named = at["hand_over"]["project"].as_str().filter(|_| ctx.cell.id == target);
            Holder::architect(named.unwrap_or(&own))
        }
        Some(other) => Holder::named(other),
        None => return Err(format!("{target} holds no blocker")),
    };
    let points = |v: &Value| -> String {
        v["points"]
            .as_array()
            .map(|ps| {
                ps.iter()
                    .map(|p| {
                        let tried = p["tried"]
                            .as_str()
                            .map(|t| format!(" (tried: {t})"))
                            .unwrap_or_default();
                        format!("{}{tried}", p["what"].as_str().unwrap_or_default())
                    })
                    .collect::<Vec<_>>()
                    .join("; ")
            })
            .unwrap_or_default()
    };
    let (why, found) = match b.holder.as_str() {
        "advisor" => (
            format!(
                "the advisor hands it on: {}",
                at["points"][0]["kind"]
                    .as_str()
                    .unwrap_or("what it cannot decide")
                    .replace('_', " ")
            ),
            points(&at),
        ),
        "owner" => (
            "the owner decided, for the architect to carry out".to_string(),
            at["text"].as_str().unwrap_or_default().to_string(),
        ),
        _ if at.get("verdict").is_some() => (
            at["why"].as_str().unwrap_or_default().to_string(),
            "it waited past its limit of the architect's time with no reply".to_string(),
        ),
        _ => (
            at["hand_over"]["why"].as_str().unwrap_or_default().to_string(),
            at["text"].as_str().unwrap_or_default().to_string(),
        ),
    };
    // The architect it leaves without having handled it is told: its matter leaves the queue.
    if let Some(m) = b.matter {
        tx.execute(
            "UPDATE architect_matters SET withdrawn = ?2 WHERE id = ?1 AND handled_round IS NULL AND withdrawn IS NULL",
            params![m, format!("the work handed the question on: {why}")],
        )
        .map_err(sql)?;
    }
    hand_over(tx, b.id, &to, &why, &found)
}

fn holder_cell<'c>(ctx: &'c crate::actions::Ctx, cell: &str) -> Option<&'c flow::format::BlockerCell> {
    ctx.blockers.as_ref()?.cells.iter().find(|c| c.cell == cell)
}

/// Who holds a blocker at a cell where it is held: its one holder, or at a cell where the work waits for the items it
/// depends on or the owner's ruling, the items while one is not merged and the owner once every one is.
fn holder_now(
    ctx: &crate::actions::Ctx,
    tx: &Transaction<'_>,
    h: &flow::format::BlockerCell,
) -> Result<Holder, String> {
    let item = ctx.item.as_ref();
    let waited = match item {
        Some(i) => unmerged(tx, i.id)?,
        None => Vec::new(),
    };
    let name = if h.holders.len() == 1 {
        h.holders[0].clone()
    } else if h.holders.iter().any(|x| x == "items") && !waited.is_empty() {
        "items".to_string()
    } else {
        h.holders
            .iter()
            .find(|x| *x != "items")
            .cloned()
            .unwrap_or_else(|| "owner".into())
    };
    Ok(match name.as_str() {
        "architect" => Holder::architect(item.map(|i| i.project.as_str()).unwrap_or_default()),
        "items" => Holder {
            holder: name,
            project: None,
            items: waited,
        },
        _ => Holder::named(&name),
    })
}

/// What a work is, for a blocker's sentence.
fn about(ctx: &crate::actions::Ctx) -> String {
    match (&ctx.item, &ctx.work.project) {
        (Some(i), _) => format!("Work {} on item {} ({})", ctx.work.id, i.id, i.title),
        (None, Some(p)) => format!("The setting of the policy of {p} (work {})", ctx.work.id),
        (None, None) => format!("Work {}", ctx.work.id),
    }
}

/// The holder a holder's cell names first: who speaks for it in what is said about a blocker.
fn speaker(ctx: &crate::actions::Ctx, b: &Blocker) -> String {
    holder_cell(ctx, &ctx.cell.id)
        .and_then(|c| c.holders.first().cloned())
        .unwrap_or_else(|| b.holder.clone())
}

/// `raise_blocker`: the task's blocker, raised at the cell where its problem arose and held by the holder of the cell
/// the row leads to. A cell where the work waits for what it depends on raises it with its row that waits, and when
/// what it waits for changed since it raised it, from the items to the owner's ruling, the blocker is handed on.
fn raise_at(ctx: &crate::actions::Ctx, tx: &Transaction<'_>) -> Result<(), String> {
    let target = ctx.to.at.clone();
    let h =
        holder_cell(ctx, &target).ok_or_else(|| format!("a row raises a blocker into {target}, where none is held"))?;
    let holder = holder_now(ctx, tx, h)?;
    let w = ctx.work.id;
    if let Some(b) = open(tx, w)? {
        if h.waits && b.cell == h.cell {
            if b.holder != holder.holder {
                let why = match holder.holder.as_str() {
                    "items" => "an item it depends on is not merged",
                    _ => {
                        "every item it depends on is merged, and the owner's ruling on its design card is still needed"
                    }
                };
                hand_over(tx, b.id, &holder, why, "")?;
            }
            return Ok(());
        }
        // A holder that asks someone itself, as the advisor did before hand-overs, holds the blocker still.
        return say(
            tx,
            b.id,
            &speaker(ctx, &b),
            "asked",
            &json!({ "to": holder.said(), "round": ctx.round.n, "ask": ctx.unwrapped.get("ask") }),
        );
    }
    let kind = match holder.holder.as_str() {
        "items" => "items",
        "owner" if h.waits => "ruling",
        "owner" => "owner_ask",
        "advisor" => "question",
        _ if ctx.cell.kind == "agent" => "question",
        _ => "tool_failure",
    };
    let cell = if holder.holder == "items" || h.waits {
        h.cell.clone()
    } else {
        ctx.cell.id.clone()
    };
    let about = about(ctx);
    let what = match kind {
        "items" => {
            let items: Vec<String> = holder.items.iter().map(|(i, _)| format!("item {i}")).collect();
            format!("{about} waits for {} to deliver", items.join(", "))
        }
        "ruling" => format!("{about} waits for the owner's ruling on its design card to merge"),
        "tool_failure" => {
            let tools: Vec<String> = ctx.unwrapped["failed"]
                .as_array()
                .map(|xs| {
                    xs.iter()
                        .filter_map(|x| x["tool"].as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            if tools.is_empty() {
                format!("{about}: {} ({})", ctx.cell.title, ctx.row.name.replace('_', " "))
            } else {
                format!("{about}: the tools {} failed", tools.join(", "))
            }
        }
        _ => match ctx.unwrapped.get("ask") {
            Some(ask) => format!(
                "{about} asks the {}{}",
                holder.holder,
                crate::steps::what_is_asked(&[json!({ "value": ask })])
            ),
            None => format!(
                "{about} asks the {} how to go on: the way on chosen at cell {} does not fit",
                holder.holder, ctx.cell.id
            ),
        },
    };
    raise(
        tx,
        &Raise {
            work: w,
            cell: &cell,
            round: Some(ctx.round.n),
            kind,
            what,
            holder,
        },
    )?;
    Ok(())
}

/// Whether the cell the row leaves holds the task's open blocker: a holder that asked someone itself, as the advisor
/// did before hand-overs, still holds it when the answer comes back through another holder's cell.
fn held_here(ctx: &crate::actions::Ctx, b: &Blocker) -> bool {
    holder_cell(ctx, &ctx.cell.id).is_some_and(|h| h.holders.contains(&b.holder))
}

/// `resolve_blocker`: the task's blocker, resolved with what its holder decided and did.
fn resolve_at(ctx: &crate::actions::Ctx, tx: &Transaction<'_>) -> Result<(), String> {
    let Some(b) = open(tx, ctx.work.id)? else { return Ok(()) };
    let reply = ctx.unwrapped.get("reply").filter(|r| !r.is_null()).cloned();
    if !held_here(ctx, &b) {
        return say(tx, b.id, &speaker(ctx, &b), "answered", &reply.unwrap_or(Value::Null));
    }
    let outcome = match (reply, b.holder.as_str()) {
        (Some(r), _) => r,
        (None, "items") => json!({
            "text": "Every item it waited for is merged",
            "items": b.items.iter().map(|(i, p)| json!({ "item": i, "project": p })).collect::<Vec<_>>(),
        }),
        (None, _) if b.kind == "ruling" => json!({
            "text": "The owner confirmed every decision of its design card, and every item it depends on is merged"
        }),
        (None, _) => json!({ "text": format!("Resolved at cell {}", ctx.cell.id) }),
    };
    resolve(tx, b.id, &outcome)
}

/// `withdraw_blocker`: the task's blocker, withdrawn by a row that takes the task on without it, saying why.
fn withdraw_at(ctx: &crate::actions::Ctx, tx: &Transaction<'_>) -> Result<(), String> {
    let Some(b) = open(tx, ctx.work.id)? else { return Ok(()) };
    let why = match ctx.row.name.as_str() {
        "interjection" => "what was said to the work took it on to choose the way on".to_string(),
        "cancelled" => "an item it waited for was cancelled and will never deliver".to_string(),
        other => ctx
            .row
            .note
            .clone()
            .unwrap_or_else(|| format!("the work took row {other} at cell {}", ctx.cell.id)),
    };
    if !held_here(ctx, &b) {
        return say(tx, b.id, &speaker(ctx, &b), "withdrawn", &json!({ "why": why }));
    }
    withdraw(tx, b.id, &why)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cycle_among_the_items_waited_for_is_found() {
        let mut w = BTreeMap::new();
        w.insert(1, BTreeSet::from([2]));
        w.insert(2, BTreeSet::from([3]));
        assert_eq!(cycle(&w), None);
        w.insert(3, BTreeSet::from([1]));
        assert_eq!(cycle(&w), Some(vec![1, 2, 3]));
    }
}
