//! A work and its item as the engine reads them from the database, and the rounds it writes.

use std::collections::BTreeMap;
use std::path::PathBuf;

use db::rusqlite::{self, Connection, OptionalExtension, params};
use flow::{CompiledGraph, Position};
use serde_json::{Map, Value};

/// A work as `works` holds it, with its position in the graph.
#[derive(Debug, Clone)]
pub struct Work {
    pub id: i64,
    pub item: Option<i64>,
    pub flow_version: i64,
    pub graph: String,
    pub pos: Position,
    pub machine: Option<String>,
    pub worktree: Option<PathBuf>,
    pub branch: Option<String>,
    pub pr: Option<i64>,
    pub pushed_head: Option<String>,
    pub paused: Option<String>,
    pub ended: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Item {
    pub id: i64,
    pub project: String,
    pub title: String,
    pub problem: String,
    pub why: String,
    pub approach: Option<String>,
    pub deliver: String,
    pub level: String,
    pub acceptance: Option<Value>,
    pub acceptance_round: Option<i64>,
    pub cancelled: Option<String>,
    /// What an import recorded of work on the item outside this flow: where it came from, its branch and its pull
    /// request.
    pub imported_from: Option<String>,
    pub imported_branch: Option<String>,
    pub imported_pr: Option<i64>,
}

/// A round as `rounds` holds it.
#[derive(Debug, Clone)]
pub struct Round {
    pub id: i64,
    pub cell: Option<String>,
    pub session: Option<i64>,
    pub input: Value,
    pub output: Option<Value>,
    pub transition: Option<String>,
    pub rejected: Option<Value>,
    pub compactions: i64,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    /// Tokens and cost as the CLI reported them; none when no CLI ran, or it never started its session.
    pub usage: Option<Value>,
}

pub fn json_col(s: Option<String>) -> Option<Value> {
    s.and_then(|t| serde_json::from_str(&t).ok())
}

fn sql(e: rusqlite::Error) -> String {
    e.to_string()
}

impl Work {
    /// Reads a work. Which cell brought it to its cell is not stored: it is the cell of the last round that took a
    /// row out of its cell.
    pub fn load(conn: &Connection, id: i64, g: Option<&CompiledGraph>) -> Result<Work, String> {
        let mut w = conn
            .query_row(
                "SELECT id, item, flow_version, graph, at, resume_at, waiting, recorded, machine, worktree, branch, pr,
                        pushed_head, paused, ended
                 FROM works WHERE id = ?1",
                [id],
                |r| {
                    let callers: Vec<String> = serde_json::from_str(&r.get::<_, String>(6)?).unwrap_or_default();
                    let recorded: BTreeMap<String, String> =
                        serde_json::from_str(&r.get::<_, String>(7)?).unwrap_or_default();
                    Ok(Work {
                        id: r.get(0)?,
                        item: r.get(1)?,
                        flow_version: r.get(2)?,
                        graph: r.get(3)?,
                        pos: Position {
                            at: r.get(4)?,
                            stopped: r.get(5)?,
                            callers,
                            from: None,
                            recorded,
                        },
                        machine: r.get(8)?,
                        worktree: r.get::<_, Option<String>>(9)?.map(PathBuf::from),
                        branch: r.get(10)?,
                        pr: r.get(11)?,
                        pushed_head: r.get(12)?,
                        paused: r.get(13)?,
                        ended: r.get(14)?,
                    })
                },
            )
            .map_err(|e| format!("work {id}: {e}"))?;
        if let Some(g) = g {
            let mut st = conn
                .prepare(
                    "SELECT cell, transition FROM rounds WHERE work = ?1 AND transition IS NOT NULL ORDER BY id DESC",
                )
                .map_err(sql)?;
            let rows = st
                .query_map([id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
                .map_err(sql)?;
            for row in rows {
                let (cell, t) = row.map_err(sql)?;
                let stays = g
                    .cell(&cell)
                    .and_then(|c| c.rows.iter().find(|r| r.name == t))
                    .is_some_and(|r| r.to == "@self");
                if !stays {
                    w.pos.from = Some(cell);
                    break;
                }
            }
        }
        Ok(w)
    }

    /// Writes where the work stands.
    pub fn save_position(&self, conn: &Connection) -> Result<(), String> {
        conn.execute(
            "UPDATE works SET at = ?2, resume_at = ?3, waiting = ?4, recorded = ?5 WHERE id = ?1",
            params![
                self.id,
                self.pos.at,
                self.pos.stopped,
                serde_json::to_string(&self.pos.callers).unwrap_or_default(),
                serde_json::to_string(&self.pos.recorded).unwrap_or_default()
            ],
        )
        .map_err(sql)?;
        Ok(())
    }
}

impl Item {
    pub fn load(conn: &Connection, id: i64) -> Result<Item, String> {
        conn.query_row(
            "SELECT id, project, title, problem, why, approach, deliver, level, acceptance, acceptance_round, cancelled,
                    imported_from, imported_branch, imported_pr
             FROM items WHERE id = ?1",
            [id],
            |r| {
                Ok(Item {
                    id: r.get(0)?,
                    project: r.get(1)?,
                    title: r.get(2)?,
                    problem: r.get(3)?,
                    why: r.get(4)?,
                    approach: r.get(5)?,
                    deliver: r.get(6)?,
                    level: r.get(7)?,
                    acceptance: json_col(r.get(8)?),
                    acceptance_round: r.get(9)?,
                    cancelled: r.get(10)?,
                    imported_from: r.get(11)?,
                    imported_branch: r.get(12)?,
                    imported_pr: r.get(13)?,
                })
            },
        )
        .map_err(|e| format!("item {id}: {e}"))
    }
}

const ROUND: &str = "SELECT id, cell, session, input, output, transition, rejected, compactions, started_at, ended_at,
                     usage FROM rounds";

fn round(r: &rusqlite::Row<'_>) -> rusqlite::Result<Round> {
    Ok(Round {
        id: r.get(0)?,
        cell: r.get(1)?,
        session: r.get(2)?,
        input: json_col(r.get(3)?).unwrap_or(Value::Null),
        output: json_col(r.get(4)?),
        transition: r.get(5)?,
        rejected: json_col(r.get(6)?),
        compactions: r.get(7)?,
        started_at: r.get(8)?,
        ended_at: r.get(9)?,
        usage: json_col(r.get(10)?),
    })
}

/// The work's rounds at a cell, newest first.
pub fn rounds_at(conn: &Connection, work: i64, cell: &str) -> Result<Vec<Round>, String> {
    let mut st = conn
        .prepare(&format!("{ROUND} WHERE work = ?1 AND cell = ?2 ORDER BY id DESC"))
        .map_err(sql)?;
    st.query_map(params![work, cell], round)
        .map_err(sql)?
        .collect::<rusqlite::Result<_>>()
        .map_err(sql)
}

/// All the work's rounds, oldest first.
pub fn rounds(conn: &Connection, work: i64) -> Result<Vec<Round>, String> {
    let mut st = conn
        .prepare(&format!("{ROUND} WHERE work = ?1 ORDER BY id"))
        .map_err(sql)?;
    st.query_map([work], round)
        .map_err(sql)?
        .collect::<rusqlite::Result<_>>()
        .map_err(sql)
}

/// When the owner last let the work go on after it paused, if ever.
pub fn resumed_at(conn: &Connection, work: i64) -> Result<Option<i64>, String> {
    conn.query_row(
        "SELECT max(at) FROM changes
         WHERE table_name = 'works' AND json_extract(row_key, '$.id') = ?1
           AND json_extract(old_row, '$.paused') IS NOT NULL AND json_extract(new_row, '$.paused') IS NULL",
        [work],
        |r| r.get(0),
    )
    .map_err(sql)
}

/// When the owner last let the work go on after it paused, and why it had been paused, if ever.
pub fn last_resume(conn: &Connection, work: i64) -> Result<Option<(i64, String)>, String> {
    conn.query_row(
        "SELECT at, json_extract(old_row, '$.paused') FROM changes
         WHERE table_name = 'works' AND json_extract(row_key, '$.id') = ?1
           AND json_extract(old_row, '$.paused') IS NOT NULL AND json_extract(new_row, '$.paused') IS NULL
         ORDER BY at DESC LIMIT 1",
        [work],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .optional()
    .map_err(sql)
}

/// The round of the work still running or waiting, if any.
pub fn open_round(conn: &Connection, work: i64) -> Result<Option<Round>, String> {
    conn.query_row(
        &format!("{ROUND} WHERE work = ?1 AND ended_at IS NULL ORDER BY id DESC LIMIT 1"),
        [work],
        round,
    )
    .optional()
    .map_err(sql)
}

/// Starts a round; it is completed by [`finish_round`] and never changed after.
pub fn start_round(
    conn: &Connection,
    work: i64,
    cell: &str,
    session: Option<i64>,
    input: &Value,
    config_commit: &str,
    model: Option<&str>,
) -> Result<i64, String> {
    start_numbered(conn, None, work, cell, session, input, config_commit, model)
}

/// The number the next round started will have. Only the engine starts rounds, one at a time, so an agent's message
/// can name its round before the round is written.
pub fn next_round(conn: &Connection) -> Result<i64, String> {
    conn.query_row("SELECT coalesce(max(id), 0) + 1 FROM rounds", [], |r| r.get(0))
        .map_err(sql)
}

/// Starts a round under the number given, or the next one.
#[allow(clippy::too_many_arguments)]
pub fn start_numbered(
    conn: &Connection,
    id: Option<i64>,
    work: i64,
    cell: &str,
    session: Option<i64>,
    input: &Value,
    config_commit: &str,
    model: Option<&str>,
) -> Result<i64, String> {
    conn.execute(
        "INSERT INTO rounds (id, work, cell, session, input, config_commit, model, started_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            id,
            work,
            cell,
            session,
            input.to_string(),
            config_commit,
            model,
            db::now()
        ],
    )
    .map_err(sql)?;
    Ok(conn.last_insert_rowid())
}

/// How a round ended.
#[derive(Debug, Default)]
pub struct Ended {
    pub output: Option<Value>,
    pub transition: Option<String>,
    pub rejected: Option<Value>,
    pub usage: Option<Value>,
    pub model: Option<String>,
    pub compactions: i64,
    /// The command line the round ran with in the end, when it is not the one recorded at its start.
    pub command: Option<Vec<String>>,
}

pub fn finish_round(conn: &Connection, id: i64, e: &Ended) -> Result<(), String> {
    conn.execute(
        "UPDATE rounds SET output = ?2, transition = ?3, rejected = ?4, usage = ?5, compactions = ?6,
                model = coalesce(?7, model), ended_at = ?8
         WHERE id = ?1 AND ended_at IS NULL",
        params![
            id,
            e.output.as_ref().map(Value::to_string),
            e.transition,
            e.rejected.as_ref().map(Value::to_string),
            e.usage.as_ref().map(Value::to_string),
            e.compactions,
            e.model,
            db::now()
        ],
    )
    .map_err(sql)?;
    if let Some(c) = &e.command {
        conn.execute(
            "UPDATE rounds SET input = json_set(input, '$.call.command', json(?2)) WHERE id = ?1",
            params![id, serde_json::to_string(c).unwrap_or_default()],
        )
        .map_err(sql)?;
    }
    Ok(())
}

/// Every row a query gives, as JSON objects; a text that holds a JSON object or list is given as that JSON.
pub fn query_json(
    conn: &Connection,
    sql_text: &str,
    args: &[&dyn rusqlite::ToSql],
    limit: usize,
) -> Result<Vec<Value>, String> {
    let mut st = conn.prepare(sql_text).map_err(sql)?;
    let names: Vec<String> = st.column_names().map(str::to_string).collect();
    let mut rows = st.query(args).map_err(sql)?;
    let mut out = Vec::new();
    while let Some(r) = rows.next().map_err(sql)? {
        if out.len() >= limit {
            break;
        }
        let mut o = Map::new();
        for (i, n) in names.iter().enumerate() {
            o.insert(n.clone(), cell_json(r.get_ref(i).map_err(sql)?));
        }
        out.push(Value::Object(o));
    }
    Ok(out)
}

pub fn cell_json(v: rusqlite::types::ValueRef<'_>) -> Value {
    use rusqlite::types::ValueRef;
    match v {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(i) => Value::from(i),
        ValueRef::Real(f) => Value::from(f),
        ValueRef::Text(t) => {
            let s = String::from_utf8_lossy(t);
            let st = s.trim_start();
            if (st.starts_with('{') || st.starts_with('['))
                && let Ok(j) = serde_json::from_str::<Value>(&s)
            {
                return j;
            }
            Value::String(s.into_owned())
        }
        ValueRef::Blob(b) => Value::String(format!("<{} bytes>", b.len())),
    }
}
