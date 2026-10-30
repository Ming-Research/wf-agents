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
    /// Who paused it, `owner` or `engine`, while it is paused.
    pub paused_by: Option<String>,
    /// For a work of the architect, the project whose architect it is.
    pub project: Option<String>,
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
    /// The round that locked the acceptance.
    pub acceptance_round: Option<RoundId>,
    pub cancelled: Option<String>,
    /// What an import recorded of work on the item outside this flow: where it came from, its branch and its pull
    /// request.
    pub imported_from: Option<String>,
    pub imported_branch: Option<String>,
    pub imported_pr: Option<i64>,
}

pub use sessions::round::json_col;
pub use sessions::{Round, RoundId};

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
                        pushed_head, paused, ended, paused_by, project
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
                        paused_by: r.get(15)?,
                        project: r.get(16)?,
                    })
                },
            )
            .map_err(|e| format!("work {id}: {e}"))?;
        if let Some(g) = g {
            let mut st = conn
                .prepare(
                    "SELECT cell, transition FROM rounds WHERE work = ?1 AND transition IS NOT NULL ORDER BY n DESC",
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

    /// What a work on no item is, by its graph, as the owner is told it: the architect of its project for a work of the
    /// architect's graph, since to the owner the architect is always there, handling one thing after another, and its
    /// works are how the engine runs it; the architect's policy setting for a work of the graph a flow version before
    /// ran the setting of a policy as.
    pub fn of_architect(&self) -> String {
        let project = self.project.as_deref().unwrap_or("no project");
        match self.graph.as_str() {
            crate::ARCHITECT => format!("the architect of {project}"),
            crate::POLICY => format!("the architect's policy setting for {project}"),
            g => format!("a work of the graph {g} for {project}"),
        }
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
            "SELECT id, project, title, problem, why, approach, deliver, level, acceptance, acceptance_work,
                    acceptance_round, cancelled, imported_from, imported_branch, imported_pr
             FROM items WHERE id = ?1",
            [id],
            |r| {
                let locked = match (r.get::<_, Option<i64>>(9)?, r.get::<_, Option<i64>>(10)?) {
                    (Some(work), Some(n)) => Some(RoundId { work, n }),
                    _ => None,
                };
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
                    acceptance_round: locked,
                    cancelled: r.get(11)?,
                    imported_from: r.get(12)?,
                    imported_branch: r.get(13)?,
                    imported_pr: r.get(14)?,
                })
            },
        )
        .map_err(|e| format!("item {id}: {e}"))
    }
}

/// A round's row, as [`sessions::round::read`] reads it.
fn round(r: &rusqlite::Row<'_>) -> rusqlite::Result<Round> {
    sessions::round::read(r)
}

/// The query of rounds a filter is put after: `rounds` named `r`.
fn rounds_query() -> String {
    format!("SELECT {} FROM rounds r", sessions::round::COLUMNS)
}

/// The work's rounds at a cell, newest first.
pub fn rounds_at(conn: &Connection, work: i64, cell: &str) -> Result<Vec<Round>, String> {
    let mut st = conn
        .prepare(&format!(
            "{} WHERE r.work = ?1 AND r.cell = ?2 ORDER BY r.n DESC",
            rounds_query()
        ))
        .map_err(sql)?;
    st.query_map(params![work, cell], round)
        .map_err(sql)?
        .collect::<rusqlite::Result<_>>()
        .map_err(sql)
}

/// The rounds named, in the order named; a round there is not is left out.
pub fn rounds_of(conn: &Connection, ids: &[RoundId]) -> Result<Vec<Round>, String> {
    let mut st = conn
        .prepare(&format!("{} WHERE r.work = ?1 AND r.n = ?2", rounds_query()))
        .map_err(sql)?;
    let mut out = Vec::new();
    for id in ids {
        if let Some(r) = st.query_row(params![id.work, id.n], round).optional().map_err(sql)? {
            out.push(r);
        }
    }
    Ok(out)
}

/// All the work's rounds, oldest first.
pub fn rounds(conn: &Connection, work: i64) -> Result<Vec<Round>, String> {
    let mut st = conn
        .prepare(&format!("{} WHERE r.work = ?1 ORDER BY r.n", rounds_query()))
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
        &format!(
            "{} WHERE r.work = ?1 AND r.ended_at IS NULL ORDER BY r.n DESC LIMIT 1",
            rounds_query()
        ),
        [work],
        round,
    )
    .optional()
    .map_err(sql)
}

/// Starts the work's next round; it is completed by [`finish_round`] and never changed after.
pub fn start_round(
    conn: &Connection,
    work: i64,
    cell: &str,
    session: Option<i64>,
    input: &Value,
    config_commit: &str,
    model: Option<&str>,
) -> Result<RoundId, String> {
    start_numbered(
        conn,
        next_round(conn, work)?,
        cell,
        session,
        input,
        config_commit,
        model,
    )
}

/// The round the work's next round started will be. Only the engine starts rounds, one at a time, so an agent's
/// message can name its round before the round is written.
pub fn next_round(conn: &Connection, work: i64) -> Result<RoundId, String> {
    let n: i64 = conn
        .query_row(
            "SELECT coalesce(max(n), 0) + 1 FROM rounds WHERE work = ?1",
            [work],
            |r| r.get(0),
        )
        .map_err(sql)?;
    Ok(RoundId { work, n })
}

/// Starts a round as the round given.
pub fn start_numbered(
    conn: &Connection,
    id: RoundId,
    cell: &str,
    session: Option<i64>,
    input: &Value,
    config_commit: &str,
    model: Option<&str>,
) -> Result<RoundId, String> {
    conn.execute(
        "INSERT INTO rounds (work, n, cell, session, input, config_commit, model, started_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            id.work,
            id.n,
            cell,
            session,
            input.to_string(),
            config_commit,
            model,
            db::now()
        ],
    )
    .map_err(sql)?;
    Ok(id)
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

pub fn finish_round(conn: &Connection, id: RoundId, e: &Ended) -> Result<(), String> {
    conn.execute(
        "UPDATE rounds SET output = ?3, transition = ?4, rejected = ?5, usage = ?6, compactions = ?7,
                model = coalesce(?8, model), ended_at = ?9
         WHERE work = ?1 AND n = ?2 AND ended_at IS NULL",
        params![
            id.work,
            id.n,
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
            "UPDATE rounds SET input = json_set(input, '$.call.command', json(?3)) WHERE work = ?1 AND n = ?2",
            params![id.work, id.n, serde_json::to_string(c).unwrap_or_default()],
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
    Ok(read_rows(conn, sql_text, args, limit, false)?.rows)
}

/// The first rows of a query, and how many more it gave.
pub struct Counted {
    pub rows: Vec<Value>,
    /// The rows after the first `limit`, which `rows` does not hold.
    pub left_out: usize,
}

/// As `query_json`, but a query with more than `limit` rows is read to its end, so that the rows left out are counted
/// exactly, from the same execution of the statement that gave the rows shown.
pub fn query_json_counted(
    conn: &Connection,
    sql_text: &str,
    args: &[&dyn rusqlite::ToSql],
    limit: usize,
) -> Result<Counted, String> {
    read_rows(conn, sql_text, args, limit, true)
}

/// The rows of a query as JSON objects, the first `limit` of them; `count_rest` reads on past them, only counting.
fn read_rows(
    conn: &Connection,
    sql_text: &str,
    args: &[&dyn rusqlite::ToSql],
    limit: usize,
    count_rest: bool,
) -> Result<Counted, String> {
    let mut st = conn.prepare(sql_text).map_err(sql)?;
    let names: Vec<String> = st.column_names().map(str::to_string).collect();
    let mut rows = st.query(args).map_err(sql)?;
    let mut out = Vec::new();
    let mut left_out = 0;
    while let Some(r) = rows.next().map_err(sql)? {
        if out.len() >= limit {
            if !count_rest {
                break;
            }
            left_out += 1;
            continue;
        }
        let mut o = Map::new();
        for (i, n) in names.iter().enumerate() {
            o.insert(n.clone(), cell_json(r.get_ref(i).map_err(sql)?));
        }
        out.push(Value::Object(o));
    }
    Ok(Counted { rows: out, left_out })
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

#[cfg(test)]
mod tests {
    use super::*;

    // A query with more rows than the limit gives the first rows and the exact number of the rest, at the limit and
    // one past it too; with no more it leaves none out; and `query_json` gives the same rows as before.
    #[test]
    fn counted_rows_give_the_exact_number_left_out() {
        let conn = Connection::open_in_memory().unwrap();
        let q = "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < ?1)
                 SELECT i FROM n WHERE i <= ?1";
        for (all, shown, left_out) in [(0, 0, 0), (2, 2, 0), (3, 3, 0), (4, 3, 1), (5, 3, 2), (1000, 3, 997)] {
            let c = query_json_counted(&conn, q, &[&all], 3).unwrap();
            let expected: Vec<Value> = (1..=shown).map(|i| serde_json::json!({ "i": i })).collect();
            assert_eq!(
                (c.rows.as_slice(), c.left_out),
                (expected.as_slice(), left_out),
                "{all} rows"
            );
            assert_eq!(
                query_json(&conn, q, &[&all], 3).unwrap(),
                expected,
                "{all} rows, uncounted"
            );
        }
    }

    // A work on no item is named to the owner by what it is, from its graph, and never as one of the architect's works.
    #[test]
    fn a_work_on_no_item_is_named_by_its_graph() {
        let work = |graph: &str| Work {
            id: 11,
            item: None,
            flow_version: 1,
            graph: graph.into(),
            pos: Position::default(),
            machine: None,
            worktree: None,
            branch: None,
            pr: None,
            pushed_head: None,
            paused: None,
            ended: None,
            paused_by: None,
            project: Some("p".into()),
        };
        assert_eq!(work(crate::ARCHITECT).of_architect(), "the architect of p");
        assert_eq!(
            work(crate::POLICY).of_architect(),
            "the architect's policy setting for p"
        );
    }
}
