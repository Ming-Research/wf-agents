//! A round as the session manager reads it: what a session was given and handed back, and what it cost.

use db::rusqlite::{self, Connection};
use serde_json::Value;

/// A round, by its work and its number among the work's rounds, counted from 1 in the order they started. Ordered by
/// its work, then its number: a later work's rounds come after an earlier one's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RoundId {
    pub work: i64,
    pub n: i64,
}

impl RoundId {
    pub fn new(work: i64, n: i64) -> RoundId {
        RoundId { work, n }
    }

    /// The round a name such as `R5-9` names.
    pub fn parse(name: &str) -> Option<RoundId> {
        let (work, n) = name.strip_prefix('R')?.split_once('-')?;
        Some(RoundId {
            work: work.parse().ok()?,
            n: n.parse().ok()?,
        })
    }
}

/// A round's name, as people and agents see it: `R<work>-<n>`.
impl std::fmt::Display for RoundId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "R{}-{}", self.work, self.n)
    }
}

/// A round as `rounds` holds it.
#[derive(Debug, Clone)]
pub struct Round {
    pub id: RoundId,
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

/// The columns [`read`] reads a round from, in its order, of `rounds` named `r`.
pub const COLUMNS: &str = "r.work, r.n, r.cell, r.session, r.input, r.output, r.transition, r.rejected, r.compactions, r.started_at, r.ended_at, r.usage";

/// A text column that holds JSON, read as JSON; none for NULL or a text that is not JSON.
pub fn json_col(s: Option<String>) -> Option<Value> {
    s.and_then(|t| serde_json::from_str(&t).ok())
}

/// A round from a row of [`COLUMNS`].
pub fn read(r: &rusqlite::Row<'_>) -> rusqlite::Result<Round> {
    Ok(Round {
        id: RoundId {
            work: r.get(0)?,
            n: r.get(1)?,
        },
        cell: r.get(2)?,
        session: r.get(3)?,
        input: json_col(r.get(4)?).unwrap_or(Value::Null),
        output: json_col(r.get(5)?),
        transition: r.get(6)?,
        rejected: json_col(r.get(7)?),
        compactions: r.get(8)?,
        started_at: r.get(9)?,
        ended_at: r.get(10)?,
        usage: json_col(r.get(11)?),
    })
}

/// The rounds run in a CLI session, oldest first: the architect's line goes on in one CLI session across its works,
/// each work with a session row of its own, so these are the rounds of every work whose session has it.
pub fn of_cli_session(conn: &Connection, cli_session: &str) -> Result<Vec<Round>, String> {
    let mut st = conn
        .prepare(&format!(
            "SELECT {COLUMNS} FROM rounds r JOIN sessions s ON s.id = r.session
             WHERE s.cli_session = ?1 ORDER BY r.work, r.n"
        ))
        .map_err(|e| e.to_string())?;
    st.query_map([cli_session], read)
        .map_err(|e| e.to_string())?
        .collect::<rusqlite::Result<_>>()
        .map_err(|e| e.to_string())
}
