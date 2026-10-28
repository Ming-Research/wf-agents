//! Which session an agent's round runs in: its line's session going on, or a new one, and why. A line starts a new
//! session at its first round in a work, at the work's first round of a cell the flow says it starts one at, and when
//! the context of its session is past the configuration's limit; every other round goes on in the line's session. A
//! cell on no line has a session of its own every round.

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
    /// A wake of the architect goes on in the CLI session the last wake left.
    Wake,
}

impl Start {
    pub fn name(self) -> &'static str {
        match self {
            Start::Line => "line",
            Start::Cell => "cell",
            Start::Limit => "limit",
            Start::Wake => "wake",
        }
    }
}

/// Where a round runs.
#[derive(Debug)]
pub enum Plan {
    /// In its line's session going on: its row, the CLI's id for it, its rounds oldest first (those of earlier wakes
    /// too, for the architect's line), what it holds, and whether the CLI started it, so that it is resumed; a
    /// session the CLI never started is started under the id kept for it.
    Going {
        id: i64,
        cli_session: String,
        rounds: Vec<Round>,
        held: Box<Held>,
        resume: bool,
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

/// Where the work's next round at a cell runs, with the configuration's limit of a session's context in tokens. It
/// reads only: [`open`] starts a new session.
pub fn plan(conn: &Connection, work: i64, cell: &CompiledCell, limit: u64) -> Result<Plan, String> {
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
    let Some((id, cli_session)) = going else {
        return Ok(Plan::New(Start::Line));
    };
    if cell.new_session {
        let before: i64 = conn
            .query_row(
                "SELECT count(*) FROM rounds WHERE work = ?1 AND cell = ?2 AND session IS NOT NULL",
                params![work, cell.id],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        if before == 0 {
            return Ok(Plan::New(Start::Cell));
        }
    }
    let (rounds, held) = held::of_session(conn, &cli_session)?;
    // A return sent back is rewritten in the session that wrote it; the limit takes effect at the round after.
    let over = context(&rounds).is_some_and(|c| c > i64::try_from(limit).unwrap_or(i64::MAX));
    if over && !rewrites(&rounds, &cell.id) {
        return Ok(Plan::New(Start::Limit));
    }
    let resume = rounds.iter().any(|r| r.usage.as_ref().is_some_and(|u| !u.is_null()));
    Ok(Plan::Going {
        id,
        cli_session,
        rounds,
        held: Box::new(held),
        resume,
    })
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

/// A wake of the architect goes on in the CLI session the last wake left, unless a new policy ended it: its line's
/// session is a row of this wake's with the same CLI session, which the next round of the line resumes. The first wake,
/// and the first after a new policy took effect, start a new one at their first round; so does a wake that comes back
/// to its start after its new policy ended its session.
pub fn wake(conn: &Connection, work: i64) -> Result<(), String> {
    let had: i64 = conn
        .query_row(
            "SELECT count(*) FROM sessions WHERE work = ?1 AND line = 'architect'",
            [work],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    if had > 0 {
        return Ok(());
    }
    let last: Option<(String, String, Option<i64>)> = conn
        .query_row(
            "SELECT s.cli, s.cli_session, s.ended FROM sessions s JOIN works x ON x.id = s.work
             WHERE x.graph = 'architect' AND s.line = 'architect' AND s.work < ?1 ORDER BY s.id DESC LIMIT 1",
            [work],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    if let Some((cli, cli_session, None)) = last {
        conn.execute(
            "INSERT INTO sessions (work, line, cli, cli_session, started) VALUES (?1, 'architect', ?2, ?3, ?4)",
            params![work, cli, cli_session, Start::Wake.name()],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
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

/// Ends the session a work's line has going on: a new policy ends the architect's.
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
            round(1, "1", Some(calls(&[(3, 0, 9_000), (2, 9_000, 1_000)])), None),
            round(2, "2", Some(json!({ "started": true })), None),
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
        assert_eq!(effort(g.cell("4").unwrap()).as_deref(), Some("xhigh"));
        assert_eq!(effort(g.cell("1a").unwrap()).as_deref(), Some("medium"));
        let mut older = g.cell("4").unwrap().clone();
        older.effort = None;
        assert_eq!(effort(&older), None);
    }

    // Only a return sent back is rewritten in its own session: a run that failed or was cut off is done again.
    #[test]
    fn only_a_return_sent_back_stays_for_its_rewrite() {
        let sent = [round(1, "4", None, Some(json!({ "check": "no head" })))];
        assert!(rewrites(&sent, "4"));
        assert!(!rewrites(&sent, "8"));
        let failed = [round(1, "4", None, Some(json!({ "failed": { "kind": "transient" } })))];
        assert!(!rewrites(&failed, "4"));
        assert!(!rewrites(&[round(1, "4", None, None)], "4"));
    }
}
