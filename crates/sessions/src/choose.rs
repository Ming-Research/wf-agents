//! Which session an agent's round runs in: its line's session going on, or a new one, and why. A line starts a new
//! session at its first round in a work, at the work's first round of a cell the flow says it starts one at (a row
//! leads there with the action `open_session`), when the context of its session is past the configuration's limit,
//! when its rounds run on another model or under another login account than its session's, and when the owner asks for
//! one; every other round goes on in the line's session. The architect's line, at its first round in a work, takes up the session the
//! project's previous architect work left, unless it ended. A cell on no line has a session of its own every round.

use std::path::Path;

use db::rusqlite::{Connection, OptionalExtension, params};
use flow::CompiledCell;

use crate::held::{self, Held};
use crate::round::Round;

/// Why a session started, as `sessions.started` names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Start {
    /// The line had no session going on: its first round in the work, or the first after its session ended.
    Line,
    /// The round is the work's first at a cell the line starts a new session at.
    Cell,
    /// The context of the line's session was past the configuration's limit.
    Limit,
    /// The line's rounds run on another model than its session's, whose prompt cache holds nothing of it.
    Model,
    /// The line's rounds run under another login account of the CLI than its session's: in another configuration
    /// directory, which does not hold the session, whose log is in the directory it ran in.
    Account,
    /// A new work of the architect goes on in the CLI session the project's previous architect work left.
    Carried,
    /// The owner asked for a new session with the round's Start or Step on the page.
    Owner,
}

impl Start {
    pub fn name(self) -> &'static str {
        match self {
            Start::Line => "line",
            Start::Cell => "cell",
            Start::Limit => "limit",
            Start::Model => "model",
            Start::Account => "account",
            Start::Carried => "carried",
            Start::Owner => "owner",
        }
    }
}

/// Where a round runs.
#[derive(Debug)]
pub enum Plan {
    /// In its line's session going on: its row, the CLI's id for it, its rounds oldest first (those of the architect's
    /// earlier works too, for the architect's line), what it holds, and whether the CLI started it, so that it is
    /// resumed; a session the CLI never started is started under the id kept for it. A session `carried` is the one
    /// the project's previous architect work left, whose row is that work's: the round takes it up for its own work
    /// ([`carry_over`]).
    Going {
        id: i64,
        cli_session: String,
        rounds: Vec<Round>,
        held: Box<Held>,
        resume: bool,
        carried: bool,
    },
    /// In a new session, for the reason given; it holds nothing yet.
    New(Start),
}

/// The line a cell's session is on: its own, or for a cell on no line the cell itself, whose session is its own.
pub fn line_of(cell: &CompiledCell) -> String {
    cell.line.clone().unwrap_or_else(|| cell.id.clone())
}

/// The context of a session as its rounds recorded it: the prompt of the last call of its latest round that recorded
/// its calls, which is the input, the cache read and the cache written of that call.
pub fn context(rounds: &[Round]) -> Option<i64> {
    rounds.iter().rev().find_map(|r| {
        let last = r.usage.as_ref()?["usage"]["iterations"].as_array()?.last()?.clone();
        let n = |k: &str| last[k].as_i64().unwrap_or(0);
        Some(n("input_tokens") + n("cache_read_input_tokens") + n("cache_creation_input_tokens"))
    })
}

/// Whether a round at a cell only rewrites the cell's last return, which was sent back: its message says only why,
/// and that is right only in the session that wrote the return.
fn rewrites(rounds: &[Round], cell: &str) -> bool {
    rounds.last().is_some_and(|r| {
        r.cell.as_deref() == Some(cell)
            && r.transition.is_none()
            && r.rejected
                .as_ref()
                .is_some_and(|x| !["interrupted", "failed", "error"].iter().any(|k| x.get(k).is_some()))
    })
}

/// The model a session's rounds ran on: the one its latest round that ran the command line gave it.
pub fn model(rounds: &[Round]) -> Option<String> {
    rounds.iter().rev().find_map(|r| {
        let command = r.input["call"]["command"].as_array()?;
        let at = command.iter().position(|a| a == "--model")?;
        command.get(at + 1)?.as_str().map(str::to_string)
    })
}

/// The configuration directory of the CLI a session's rounds ran with, which holds the login their calls were made
/// under and the session itself: the one its latest round that ran the command line gave it; none when no round
/// recorded one.
pub fn home(rounds: &[Round]) -> Option<String> {
    rounds.iter().rev().find_map(|r| {
        r.input["call"]["command"].as_array()?;
        r.input["call"]["env"][crate::cli::CONFIG_DIR]
            .as_str()
            .map(str::to_string)
    })
}

/// Whether two configuration directories are one: the same path, or the same directory by two paths.
pub fn same_dir(a: &Path, b: &Path) -> bool {
    a == b
        || match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
}

/// What a line's rounds run on now, which a session that ran on another cannot go on with: the model, whose prompt
/// cache holds nothing of another model's session, and the configuration directory of the CLI, the login account the
/// calls are made under, which holds the session; none when the CLI runs with none of its own.
#[derive(Debug, Clone, Copy)]
pub struct On<'a> {
    pub model: &'a str,
    pub home: Option<&'a Path>,
}

/// Where the work's next round at a cell runs, on what it runs on, with the configuration's limit of a session's
/// context in tokens. It reads only: [`open`] starts a new session.
pub fn plan(conn: &Connection, work: i64, cell: &CompiledCell, limit: u64, on: On) -> Result<Plan, String> {
    let Some(line) = &cell.line else {
        return Ok(Plan::New(Start::Line));
    };
    let going: Option<(i64, String)> = conn
        .query_row(
            "SELECT id, cli_session FROM sessions WHERE work = ?1 AND line = ?2 AND ended IS NULL",
            params![work, line],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let (id, cli_session, carried) = match going {
        Some((id, cli_session)) => (id, cli_session, false),
        None => match left(conn, work, line)? {
            Some((id, cli_session)) => (id, cli_session, true),
            None => return Ok(Plan::New(Start::Line)),
        },
    };
    let (rounds, held) = held::of_session(conn, &cli_session)?;
    // A fixed place of the flow opens a new session at the work's first round there; a session set up for the line
    // that holds no round yet, as the start of a work sets up the engineer's, is new already.
    if !rounds.is_empty() && opens(conn, work, cell)? {
        return Ok(Plan::New(Start::Cell));
    }
    // Another login account's configuration directory does not hold the session at all, so a new one starts even to
    // rewrite a return sent back.
    if let (Some(was), Some(now)) = (home(&rounds), on.home)
        && !same_dir(Path::new(&was), now)
    {
        return Ok(Plan::New(Start::Account));
    }
    // A return sent back is rewritten in the session that wrote it; the limit and a new model take effect at the round
    // after. A model's prompt cache holds nothing of a session another model ran, so a session on another model would
    // be written again whole: a new one is given only what its cell needs.
    if !rewrites(&rounds, &cell.id) {
        if context(&rounds).is_some_and(|c| c > i64::try_from(limit).unwrap_or(i64::MAX)) {
            return Ok(Plan::New(Start::Limit));
        }
        if model(&rounds).is_some_and(|m| m != on.model) {
            return Ok(Plan::New(Start::Model));
        }
    }
    let resume = rounds.iter().any(|r| r.usage.as_ref().is_some_and(|u| !u.is_null()));
    Ok(Plan::Going {
        id,
        cli_session,
        rounds,
        held: Box::new(held),
        resume,
        carried,
    })
}

/// Whether the work's next round at a cell opens a new session for its line because the flow fixes one there: the
/// cell is one its line starts a new session at, and the work has had no round there yet. The session the line has
/// going on before it ends there.
pub fn opens(conn: &Connection, work: i64, cell: &CompiledCell) -> Result<bool, String> {
    if !cell.new_session {
        return Ok(false);
    }
    let before: i64 = conn
        .query_row(
            "SELECT count(*) FROM rounds WHERE work = ?1 AND cell = ?2 AND session IS NOT NULL",
            params![work, cell.id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    Ok(before == 0)
}

/// The line of the architect's works, whose session goes on from one of a project's architect works to the next.
const ARCHITECT: &str = "architect";

/// The session a work of a project's architect takes up at its line's first round in the work: the one the project's
/// previous architect work left on the line, its row and the CLI's id for it, unless it ended; none for another line,
/// once the work's line had a session, and at the project's first architect work.
fn left(conn: &Connection, work: i64, line: &str) -> Result<Option<(i64, String)>, String> {
    if line != ARCHITECT {
        return Ok(None);
    }
    let had: i64 = conn
        .query_row(
            "SELECT count(*) FROM sessions WHERE work = ?1 AND line = ?2",
            params![work, line],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    if had > 0 {
        return Ok(None);
    }
    let last: Option<(i64, String, Option<i64>)> = conn
        .query_row(
            "SELECT s.id, s.cli_session, s.ended FROM sessions s JOIN works x ON x.id = s.work
             WHERE x.graph = 'architect' AND s.line = ?2 AND s.work < ?1
               AND x.project = (SELECT project FROM works WHERE id = ?1 AND graph = 'architect')
             ORDER BY s.id DESC LIMIT 1",
            params![work, line],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    Ok(last.and_then(|(id, cli_session, ended)| ended.is_none().then_some((id, cli_session))))
}

/// Starts a new session of a work's line in the CLI named, under the CLI's id given, for the reason given: the session
/// the line had going on, if any, ends. Gives its row.
pub fn open(conn: &Connection, work: i64, line: &str, cli: &str, cli_session: &str, why: Start) -> Result<i64, String> {
    conn.execute(
        "UPDATE sessions SET ended = ?3 WHERE work = ?1 AND line = ?2 AND ended IS NULL",
        params![work, line, db::now()],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO sessions (work, line, cli, cli_session, started) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![work, line, cli, cli_session, why.name()],
    )
    .map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

/// Begins a work's line's session under the CLI's id given, as the session manager sets it up with the work, unless the
/// line has one going on already.
pub fn begin(conn: &Connection, work: i64, line: &str, cli: &str, cli_session: &str) -> Result<(), String> {
    conn.execute(
        "INSERT INTO sessions (work, line, cli, cli_session) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT (work, line) WHERE ended IS NULL DO NOTHING",
        params![work, line, cli, cli_session],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// A work of a project's architect takes up, at its line's first round in it, the session the project's previous
/// architect work left (`from`, that work's row, as [`plan`] found it): its line's session is a row of this work's with
/// the same CLI session, started as carried, which the round resumes. Gives the row.
pub fn carry_over(conn: &Connection, work: i64, from: i64) -> Result<i64, String> {
    let n = conn
        .execute(
            "INSERT INTO sessions (work, line, cli, cli_session, started)
             SELECT ?1, line, cli, cli_session, ?3 FROM sessions WHERE id = ?2",
            params![work, from, Start::Carried.name()],
        )
        .map_err(|e| e.to_string())?;
    if n != 1 {
        return Err(format!("work {work}: no session {from} to take up"));
    }
    Ok(conn.last_insert_rowid())
}

/// The CLI's id for a session, when the CLI reported another than the one the session was started under.
pub fn rename(conn: &Connection, session: i64, cli_session: &str) -> Result<(), String> {
    conn.execute(
        "UPDATE sessions SET cli_session = ?2 WHERE id = ?1",
        params![session, cli_session],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Ends a session, so that its line's next round starts a new one: a cell on no line's, once its round has.
pub fn end(conn: &Connection, session: i64) -> Result<(), String> {
    conn.execute(
        "UPDATE sessions SET ended = ?2 WHERE id = ?1 AND ended IS NULL",
        params![session, db::now()],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Ends the session a work's line has going on: a batch made ends its review's.
pub fn end_line(conn: &Connection, work: i64, line: &str) -> Result<(), String> {
    conn.execute(
        "UPDATE sessions SET ended = ?3 WHERE work = ?1 AND line = ?2 AND ended IS NULL",
        params![work, line, db::now()],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Ends every session a work has going on, once the work has ended.
pub fn end_work(conn: &Connection, work: i64) -> Result<(), String> {
    conn.execute(
        "UPDATE sessions SET ended = ?2 WHERE work = ?1 AND ended IS NULL",
        params![work, db::now()],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// The reasoning effort a round runs with: its cell's, as the flow declares it, whatever the model its level picks; none
/// for a cell of a flow version stored before cells declared one, which runs at the command line's own.
pub fn effort(cell: &CompiledCell) -> Option<String> {
    cell.effort.clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn round(n: i64, cell: &str, usage: Option<Value>, rejected: Option<Value>) -> Round {
        Round {
            id: crate::RoundId::new(1, n),
            cell: Some(cell.into()),
            session: Some(1),
            input: json!({}),
            output: Some(json!({})),
            transition: rejected.is_none().then(|| "done".to_string()),
            rejected,
            compactions: 0,
            started_at: n,
            ended_at: Some(n),
            usage,
        }
    }

    fn calls(prompts: &[(i64, i64, i64)]) -> Value {
        let it: Vec<Value> = prompts
            .iter()
            .map(|(i, r, w)| json!({ "input_tokens": i, "cache_read_input_tokens": r, "cache_creation_input_tokens": w }))
            .collect();
        json!({ "usage": { "iterations": it } })
    }

    // A session's context is the last call of its latest round that recorded its calls.
    #[test]
    fn a_sessions_context_is_its_latest_last_call() {
        let rs = [
            round(1, "R1", Some(calls(&[(3, 0, 9_000), (2, 9_000, 1_000)])), None),
            round(2, "D1", Some(json!({ "started": true })), None),
        ];
        assert_eq!(context(&rs), Some(10_002));
        assert_eq!(context(&rs[1..]), None);
    }

    // A round runs at its cell's effort whatever the model: implementing at xhigh, the first look at a design at medium.
    #[test]
    fn a_round_runs_at_its_cells_effort() {
        let relations = db::relations(db::SCHEMA).unwrap();
        let tables = db::index_of(db::SCHEMA).unwrap();
        let f = flow::compile(flow::FLOW, &flow::Context { relations, tables }).unwrap();
        let g = f.graph("engineer").unwrap();
        assert_eq!(effort(g.cell("I1").unwrap()).as_deref(), Some("xhigh"));
        assert_eq!(effort(g.cell("R2").unwrap()).as_deref(), Some("medium"));
        let mut older = g.cell("I1").unwrap().clone();
        older.effort = None;
        assert_eq!(effort(&older), None);
    }

    // Only a return sent back is rewritten in its own session: a run that failed or was cut off is done again.
    #[test]
    fn only_a_return_sent_back_stays_for_its_rewrite() {
        let sent = [round(1, "I1", None, Some(json!({ "check": "no head" })))];
        assert!(rewrites(&sent, "I1"));
        assert!(!rewrites(&sent, "T6"));
        let failed = [round(1, "I1", None, Some(json!({ "failed": { "kind": "transient" } })))];
        assert!(!rewrites(&failed, "I1"));
        assert!(!rewrites(&[round(1, "I1", None, None)], "I1"));
    }
}
