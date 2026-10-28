//! What a round or a keep-warm ping of a session cost: the CLI reports what its session has cost so far, the calls it
//! resumed from included, so each call's own cost is what that grew by since the session's call before it.

use db::rusqlite::{Connection, OptionalExtension};
use serde_json::{Value, json};

use crate::round::Round;

/// A usage record with its own cost written in: what the session's cost grew by since `before`, what it had cost by
/// its call before; a session the CLI counted afresh costs what it reports.
pub fn own_cost(mut usage: Value, before: Option<f64>) -> Value {
    let Some(total) = usage["session_cost_usd"].as_f64() else {
        return usage;
    };
    usage["cost_usd"] = json!(match before {
        Some(b) if b <= total => total - b,
        _ => total,
    });
    usage
}

/// What a CLI session had cost by its last call that reported it, a round's or a keep-warm ping's, whichever came
/// last; its rounds are given oldest first.
pub fn spent(conn: &Connection, cli_session: &str, rounds: &[Round]) -> Result<Option<f64>, String> {
    let round = rounds.iter().rev().find_map(|r| {
        let total = r.usage.as_ref()?["session_cost_usd"].as_f64()?;
        Some((r.ended_at.unwrap_or(r.started_at), total))
    });
    let ping: Option<(i64, f64)> = conn
        .query_row(
            "SELECT p.started_at, json_extract(p.usage, '$.session_cost_usd') FROM pings p
             JOIN sessions s ON s.id = p.session
             WHERE s.cli_session = ?1 AND json_extract(p.usage, '$.session_cost_usd') IS NOT NULL
             ORDER BY p.started_at DESC LIMIT 1",
            [cli_session],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    Ok(match (round, ping) {
        (Some(r), Some(p)) => Some(if p.0 > r.0 { p.1 } else { r.1 }),
        (r, p) => r.or(p).map(|x| x.1),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_call_costs_what_its_session_grew_by() {
        let u = |c: f64| json!({ "session_cost_usd": c });
        assert_eq!(own_cost(u(1.5), None)["cost_usd"], 1.5);
        assert_eq!(own_cost(u(4.0), Some(1.5))["cost_usd"], 2.5);
        // A session the CLI counted afresh costs what it reports.
        assert_eq!(own_cost(u(0.5), Some(1.5))["cost_usd"], 0.5);
        assert_eq!(own_cost(json!({ "started": true }), None), json!({ "started": true }));
    }
}
