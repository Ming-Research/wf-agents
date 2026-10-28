//! What a round or a keep-warm ping of a session cost: the CLI reports what its session has cost so far, the calls it
//! resumed from included, so each call's own cost is what that grew by since the session's call before it.

use db::rusqlite::{Connection, OptionalExtension};
use serde_json::{Value, json};

use crate::round::Round;

/// What a model's tokens cost as parts of an input token's price, as Anthropic prices every model: reading the cache,
/// writing it to keep five minutes or an hour, and output.
const READ: f64 = 0.1;
const WRITE_5M: f64 = 1.25;
const WRITE_1H: f64 = 2.0;
const OUTPUT: f64 = 5.0;

/// What writing `tokens` to the cache again cost a call beyond reading them, from its usage record with its own cost:
/// the price of an input token is that cost over the call's tokens, each counted at its part of that price, and the
/// tokens are written as the call wrote the cache, kept five minutes or an hour. None when the record has no cost or
/// no tokens.
pub fn rewritten(usage: &Value, tokens: i64) -> Option<f64> {
    let cost = usage["cost_usd"].as_f64()?;
    let u = usage.get("usage")?;
    let n = |v: &Value| v.as_f64().unwrap_or(0.0);
    let (w5, w1) = match u.get("cache_creation") {
        Some(c) => (n(&c["ephemeral_5m_input_tokens"]), n(&c["ephemeral_1h_input_tokens"])),
        None => (n(&u["cache_creation_input_tokens"]), 0.0),
    };
    let units = n(&u["input_tokens"])
        + READ * n(&u["cache_read_input_tokens"])
        + WRITE_5M * w5
        + WRITE_1H * w1
        + OUTPUT * n(&u["output_tokens"]);
    if units <= 0.0 {
        return None;
    }
    let write = if w5 + w1 > 0.0 {
        (WRITE_5M * w5 + WRITE_1H * w1) / (w5 + w1)
    } else {
        WRITE_5M
    };
    Some(tokens as f64 * (write - READ) * cost / units)
}

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

    // A call that only wrote the cache, kept an hour, cost twice an input token's price for each token: writing them
    // again beyond reading them cost 1.9 parts of 2.
    #[test]
    fn writing_the_cache_again_costs_its_price_beyond_reading_it() {
        let u = json!({ "cost_usd": 2.0, "usage": { "cache_creation_input_tokens": 1000,
            "cache_creation": { "ephemeral_5m_input_tokens": 0, "ephemeral_1h_input_tokens": 1000 } } });
        let c = rewritten(&u, 1000).unwrap();
        assert!((c - 1.9).abs() < 1e-9, "{c}");
        assert_eq!(rewritten(&json!({ "usage": {} }), 1000), None);
    }
}
