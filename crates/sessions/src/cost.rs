//! What a round or a keep-warm ping of a session cost: the CLI reports what its session has cost so far, the calls it
//! resumed from included, so each call's own cost is what that grew by since the session's call before it. And what
//! each kind of token costs each model, by which a round's cost is split by kind and a miss of the cache is priced.

use db::rusqlite::{Connection, OptionalExtension};
use serde_json::{Value, json};

use crate::cli::FIRST_CALL;
use crate::round::Round;

/// What a million tokens of each kind cost a model, in US dollars: input not read from the cache, a read from the
/// cache, a write to it kept five minutes or an hour, and output.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Prices {
    pub input: f64,
    pub read: f64,
    pub write_5m: f64,
    pub write_1h: f64,
    pub output: f64,
}

/// A model's prices, and those of a call whose prompt is over a number of tokens, where they are higher.
struct Model {
    id: &'static str,
    prices: Prices,
    above: Option<(i64, Prices)>,
}

/// The prices of the models the agents run on, as the command line prices each call whose cost it reports, which is
/// what each round records it cost (its model catalog, Claude Code 2.1.312): Anthropic's list prices, but for a read
/// from the cache on Sonnet 5.5, $0.10 where the list says $0.20. A read is not one part of the input's price on every
/// model: 0.025 on Fable 5.1, 0.05 on Opus 5.5 and Sonnet 5.5, 0.1 on Haiku 5.5. A call of Haiku 5.5 whose prompt is
/// over 100,000 tokens costs five times as much.
const MODELS: [Model; 4] = [
    Model {
        id: "claude-fable-5-1",
        prices: Prices {
            input: 10.0,
            read: 0.25,
            write_5m: 12.5,
            write_1h: 20.0,
            output: 50.0,
        },
        above: None,
    },
    Model {
        id: "claude-opus-5-5",
        prices: Prices {
            input: 4.0,
            read: 0.2,
            write_5m: 5.0,
            write_1h: 8.0,
            output: 20.0,
        },
        above: None,
    },
    Model {
        id: "claude-sonnet-5-5",
        prices: Prices {
            input: 2.0,
            read: 0.1,
            write_5m: 2.5,
            write_1h: 4.0,
            output: 10.0,
        },
        above: None,
    },
    Model {
        id: "claude-haiku-5-5",
        prices: Prices {
            input: 0.1,
            read: 0.01,
            write_5m: 0.125,
            write_1h: 0.2,
            output: 0.5,
        },
        above: Some((
            100_000,
            Prices {
                input: 0.5,
                read: 0.05,
                write_5m: 0.625,
                write_1h: 1.0,
                output: 2.5,
            },
        )),
    },
];

/// A model's prices for a call whose prompt (its input, what it read from the cache and what it wrote to it) was
/// `prompt` tokens; none for a model with no prices here. A model named with its context window, as `[1m]`, is priced
/// as the model.
pub fn prices(model: &str, prompt: i64) -> Option<Prices> {
    let id = model.split('[').next().unwrap_or(model);
    let m = MODELS.iter().find(|m| m.id == id)?;
    Some(match m.above {
        Some((tokens, above)) if prompt > tokens => above,
        _ => m.prices,
    })
}

/// What each kind of token cost, in US dollars: input not read from the cache, reads from the cache, writes to it, and
/// output.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Parts {
    pub input: f64,
    pub read: f64,
    pub written: f64,
    pub output: f64,
}

impl Parts {
    pub fn total(&self) -> f64 {
        self.input + self.read + self.written + self.output
    }
}

/// The tokens of a call or of a run's calls, as the CLI reports them (`usage`), by kind: input, read from the cache,
/// written to it for five minutes and for an hour, and output. A record that does not split what was written by how
/// long it is kept: all of it for five minutes.
fn kinds(u: &Value) -> [f64; 5] {
    let n = |v: &Value| v.as_f64().unwrap_or(0.0);
    let (w5, w1) = match u.get("cache_creation") {
        Some(c) => (n(&c["ephemeral_5m_input_tokens"]), n(&c["ephemeral_1h_input_tokens"])),
        None => (n(&u["cache_creation_input_tokens"]), 0.0),
    };
    [
        n(&u["input_tokens"]),
        n(&u["cache_read_input_tokens"]),
        w5,
        w1,
        n(&u["output_tokens"]),
    ]
}

/// The prompt of a call as the CLI reports it: its input, what it read from the cache and what it wrote to it.
fn prompt(call: &Value) -> i64 {
    let n = |k: &str| call[k].as_i64().unwrap_or(0);
    n("input_tokens") + n("cache_read_input_tokens") + n("cache_creation_input_tokens")
}

/// What each kind of a round's own tokens cost, from its usage record: the totals of the calls the round made on its
/// model, at the model's prices for its last call's prompt, since the record sums the calls and keeps only the last.
/// They add up to the round's cost when those calls are all it cost; what a compaction or a subagent spent is in its
/// cost and not in these tokens. None for a model with no prices here, or a record with no tokens.
pub fn parts(model: &str, usage: &Value) -> Option<Parts> {
    let u = usage.get("usage")?;
    let last = u["iterations"]
        .as_array()
        .and_then(|xs| xs.last())
        .map(prompt)
        .unwrap_or(0);
    let p = prices(model, last)?;
    let [input, read, w5, w1, output] = kinds(u);
    Some(Parts {
        input: input * p.input / 1e6,
        read: read * p.read / 1e6,
        written: (w5 * p.write_5m + w1 * p.write_1h) / 1e6,
        output: output * p.output / 1e6,
    })
}

/// What writing `tokens` to the cache again cost a round's first call beyond reading them, at the prices of the
/// round's model for that call's prompt, from the round's usage record: the tokens are written as the round wrote the
/// cache, kept five minutes or an hour. None for a model with no prices here, or a record with no tokens.
pub fn rewritten(model: &str, usage: &Value, tokens: i64) -> Option<f64> {
    let u = usage.get("usage")?;
    let p = prices(model, prompt(&usage[FIRST_CALL]))?;
    let [_, _, w5, w1, _] = kinds(u);
    let write = if w5 + w1 > 0.0 {
        (p.write_5m * w5 + p.write_1h * w1) / (w5 + w1)
    } else {
        p.write_5m
    };
    Some(tokens as f64 * (write - p.read) / 1e6)
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

    // Writing a million tokens again for an hour on Sonnet 5.5 cost $4.00 where reading them would have cost $0.10; on
    // Haiku 5.5, $0.20 where reading them would have cost $0.01, and five times both when the first call's prompt is
    // over 100,000 tokens.
    #[test]
    fn writing_the_cache_again_costs_its_price_beyond_reading_it() {
        let used = |first: i64| {
            json!({
                "first_call": { "input_tokens": 2, "cache_read_input_tokens": 0, "cache_creation_input_tokens": first },
                "usage": { "cache_creation_input_tokens": 1000,
                    "cache_creation": { "ephemeral_5m_input_tokens": 0, "ephemeral_1h_input_tokens": 1000 } }
            })
        };
        let near = |c: Option<f64>, want: f64| assert!(c.is_some_and(|c| (c - want).abs() < 1e-9), "{c:?}");
        near(rewritten("claude-sonnet-5-5", &used(1000), 1_000_000), 3.9);
        near(rewritten("claude-haiku-5-5", &used(1000), 1_000_000), 0.19);
        near(rewritten("claude-haiku-5-5", &used(200_000), 1_000_000), 0.95);
        assert_eq!(rewritten("some-model", &used(1000), 1000), None);
        assert_eq!(rewritten("claude-sonnet-5-5", &json!({}), 1000), None);
    }

    // Rounds of the sixth trial, each record as the command line reported it, with the cost the session grew by: the
    // split of a round's tokens by kind adds up to that cost.
    #[test]
    fn a_rounds_tokens_by_kind_add_up_to_what_it_cost() {
        let rounds = [
            // R1-69, on Sonnet 5.5.
            (
                "claude-sonnet-5-5",
                3.7835335000000043,
                r#"{"input_tokens":30,"cache_creation_input_tokens":501118,"cache_read_input_tokens":13278015,"output_tokens":45120,"output_tokens_details":{"thinking_tokens":39860},"server_tool_use":{"web_search_requests":0,"web_fetch_requests":0},"service_tier":"standard","cache_creation":{"ephemeral_1h_input_tokens":501118,"ephemeral_5m_input_tokens":0},"inference_geo":"global","iterations":[{"input_tokens":2,"output_tokens":735,"cache_read_input_tokens":951412,"cache_creation_input_tokens":277,"cache_creation":{"ephemeral_5m_input_tokens":0,"ephemeral_1h_input_tokens":277},"type":"message"}],"speed":"standard","fallback_credit":null}"#,
            ),
            // R9-8, on Opus 5.5.
            (
                "claude-opus-5-5",
                0.6675610000000001,
                r#"{"input_tokens":12,"cache_creation_input_tokens":65580,"cache_read_input_tokens":313165,"output_tokens":4012,"output_tokens_details":{"thinking_tokens":722},"server_tool_use":{"web_search_requests":0,"web_fetch_requests":0},"service_tier":"standard","cache_creation":{"ephemeral_1h_input_tokens":65580,"ephemeral_5m_input_tokens":0},"inference_geo":"global","iterations":[{"input_tokens":2,"output_tokens":2685,"cache_read_input_tokens":64783,"cache_creation_input_tokens":797,"cache_creation":{"ephemeral_5m_input_tokens":0,"ephemeral_1h_input_tokens":797},"type":"message"}],"speed":"standard","fallback_credit":null}"#,
            ),
            // R14-2, on Haiku 5.5, its prompts under 100,000 tokens.
            (
                "claude-haiku-5-5",
                0.040748179999999995,
                r#"{"input_tokens":32,"cache_creation_input_tokens":73103,"cache_read_input_tokens":859738,"output_tokens":35054,"output_tokens_details":{"thinking_tokens":28120},"server_tool_use":{"web_search_requests":0,"web_fetch_requests":0},"service_tier":"standard","cache_creation":{"ephemeral_1h_input_tokens":73103,"ephemeral_5m_input_tokens":0},"inference_geo":"global","iterations":[{"input_tokens":2,"output_tokens":11538,"cache_read_input_tokens":71097,"cache_creation_input_tokens":2006,"cache_creation":{"ephemeral_5m_input_tokens":0,"ephemeral_1h_input_tokens":2006},"type":"message"}],"speed":"standard","fallback_credit":null}"#,
            ),
            // R2-71, on Haiku 5.5, its prompt over 100,000 tokens.
            (
                "claude-haiku-5-5",
                0.42755220000000005,
                r#"{"input_tokens":2,"cache_creation_input_tokens":398266,"cache_read_input_tokens":371704,"output_tokens":4280,"output_tokens_details":{"thinking_tokens":1080},"server_tool_use":{"web_search_requests":0,"web_fetch_requests":0},"service_tier":"standard","cache_creation":{"ephemeral_1h_input_tokens":398266,"ephemeral_5m_input_tokens":0},"inference_geo":"global","iterations":[{"input_tokens":2,"output_tokens":4280,"cache_read_input_tokens":371704,"cache_creation_input_tokens":398266,"cache_creation":{"ephemeral_5m_input_tokens":0,"ephemeral_1h_input_tokens":398266},"type":"message"}],"speed":"standard","fallback_credit":null}"#,
            ),
        ];
        for (model, cost, usage) in rounds {
            let usage: Value = serde_json::from_str(usage).unwrap();
            let p = parts(model, &json!({ "cost_usd": cost, "usage": usage })).unwrap();
            assert!(
                (p.total() - cost).abs() < cost * 1e-6,
                "{model}: {p:?} add up to {} for {cost}",
                p.total()
            );
        }
        // A read from the cache on Sonnet 5.5 is a twentieth of an input token, not a tenth.
        let read = parts(
            "claude-sonnet-5-5",
            &json!({ "usage": { "cache_read_input_tokens": 1_000_000 } }),
        )
        .unwrap();
        assert!((read.read - 0.1).abs() < 1e-9, "{read:?}");
        assert_eq!(parts("some-model", &json!({ "usage": {} })), None);
        assert_eq!(prices("claude-fable-5-1[1m]", 0).map(|p| p.read), Some(0.25));
    }
}
