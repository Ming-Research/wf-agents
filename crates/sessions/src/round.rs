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
/// each work with a session row of its own, so these are the rounds of every work whose session has it. A fork holds
/// what its base held when it was forked: its rounds come after the base's that started before the fork's first.
pub fn of_cli_session(conn: &Connection, cli_session: &str) -> Result<Vec<Round>, String> {
    let own = own_rounds(conn, cli_session)?;
    let base: Option<String> = conn
        .query_row(
            "SELECT b.cli_session FROM sessions s JOIN sessions b ON b.id = s.fork_of
             WHERE s.cli_session = ?1 ORDER BY s.id LIMIT 1",
            [cli_session],
            |r| r.get(0),
        )
        .map(Some)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            e => Err(e.to_string()),
        })?;
    let Some(base) = base.filter(|b| b != cli_session) else {
        return Ok(own);
    };
    let until = own.first().map(|r| r.started_at);
    let mut out: Vec<Round> = own_rounds(conn, &base)?
        .into_iter()
        .filter(|r| until.is_none_or(|t| r.started_at < t))
        .collect();
    out.extend(own);
    Ok(out)
}

/// The statement `own_rounds` runs: the rounds run in a CLI session, found through the session's row.
pub fn own_rounds_sql() -> String {
    format!(
        "SELECT {COLUMNS} FROM rounds r JOIN sessions s ON s.id = r.session
         WHERE s.cli_session = ?1 ORDER BY r.work, r.n"
    )
}

/// The rounds run in a CLI session itself, oldest first.
fn own_rounds(conn: &Connection, cli_session: &str) -> Result<Vec<Round>, String> {
    let mut st = conn.prepare(&own_rounds_sql()).map_err(|e| e.to_string())?;
    st.query_map([cli_session], read)
        .map_err(|e| e.to_string())?
        .collect::<rusqlite::Result<_>>()
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    #[test]
    fn own_rounds_reads_rounds_by_session() {
        let database = db::Db::in_memory().unwrap();
        let steps = db::query_plan(database.conn(), &super::own_rounds_sql()).unwrap();
        assert!(
            steps.contains("rounds_by_session"),
            "the plan does not read by rounds_by_session:\n{steps}"
        );
    }

    #[test]
    fn own_rounds_finds_the_session_by_its_cli_session_and_scans_no_table() {
        let database = db::Db::in_memory().unwrap();
        let steps = db::query_plan(database.conn(), &super::own_rounds_sql()).unwrap();
        assert!(
            steps.contains("sessions_by_cli_session"),
            "the plan does not find the session by sessions_by_cli_session:\n{steps}"
        );
        assert!(!steps.contains("SCAN"), "the plan scans a table:\n{steps}");
    }
}
