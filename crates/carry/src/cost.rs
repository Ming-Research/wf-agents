//! What a call to the model cost, from its tokens at its model's prices: the command line says what a round cost only
//! in its result, which a round that crashed never prints, and a round tried again must still be counted against its
//! work's budget.

use serde_json::Value;

/// What a million tokens of each kind cost a model, in US dollars: input not read from the cache, a read from the
/// cache, a write to it kept five minutes or an hour, and output.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Prices {
    input: f64,
    read: f64,
    write_5m: f64,
    write_1h: f64,
    output: f64,
}

/// The prices of the models the agents run on, as the command line prices each call: Anthropic's list prices, but for
/// a read from the cache on Sonnet 5.5, $0.10 where the list says $0.20. A call of Haiku 5.5 whose prompt is over
/// 100,000 tokens costs five times as much.
const MODELS: [Model; 4] = [
    (
        "claude-fable-5-1",
        Prices {
            input: 10.0,
            read: 0.25,
            write_5m: 12.5,
            write_1h: 20.0,
            output: 50.0,
        },
        None,
    ),
    (
        "claude-opus-5-5",
        Prices {
            input: 4.0,
            read: 0.2,
            write_5m: 5.0,
            write_1h: 8.0,
            output: 20.0,
        },
        None,
    ),
    (
        "claude-sonnet-5-5",
        Prices {
            input: 2.0,
            read: 0.1,
            write_5m: 2.5,
            write_1h: 4.0,
            output: 10.0,
        },
        None,
    ),
    (
        "claude-haiku-5-5",
        Prices {
            input: 0.1,
            read: 0.01,
            write_5m: 0.125,
            write_1h: 0.2,
            output: 0.5,
        },
        Some((
            100_000,
            Prices {
                input: 0.5,
                read: 0.05,
                write_5m: 0.625,
                write_1h: 1.0,
                output: 2.5,
            },
        )),
    ),
];

/// A model by its name, with its prices, and those of a call whose prompt is over a number of tokens where they are
/// higher.
type Model = (&'static str, Prices, Option<(u64, Prices)>);

/// The tokens of one call to the model, as its `usage` gives them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tokens {
    /// Sent and not read from the cache: input, and what was written to the cache.
    pub input: u64,
    /// Read from the cache.
    pub read: u64,
    /// Written to the cache to be kept an hour, among `input`.
    pub written_1h: u64,
    /// Written to the cache to be kept five minutes, among `input`; all that was written, where the call does not
    /// split it.
    pub written_5m: u64,
    pub output: u64,
}

impl Tokens {
    /// The tokens a call's `usage` gives.
    pub fn of(usage: &Value) -> Tokens {
        let n = |v: &Value| v.as_u64().unwrap_or(0);
        let written = n(&usage["cache_creation_input_tokens"]);
        let written_1h = n(&usage["cache_creation"]["ephemeral_1h_input_tokens"]).min(written);
        Tokens {
            input: n(&usage["input_tokens"]) + written,
            read: n(&usage["cache_read_input_tokens"]),
            written_1h,
            written_5m: written - written_1h,
            output: n(&usage["output_tokens"]),
        }
    }

    /// The prompt the call was sent, read from the cache or not.
    pub fn prompt(&self) -> u64 {
        self.input + self.read
    }
}

/// How many keep-warm pings one wait of a session on the model is worth: as many as cost, each reading the session from
/// the cache, no more than writing it to the cache for an hour would beyond reading it, which a ping saves where the
/// session's next round comes before the cache lets it go. None for a model with no prices here, whose worth is not
/// known; the ratio of the prices is the same over Haiku's bound on a prompt.
pub fn pings_worth(model: &str) -> Option<u32> {
    let id = model.split('[').next().unwrap_or(model);
    let (_, p, _) = MODELS.iter().find(|(name, ..)| *name == id)?;
    // The small margin keeps a ratio such as (4.0 - 0.1) / 0.1, which floating point makes 38.99..., at 39.
    let pings = (p.write_1h - p.read) / p.read + 1e-9;
    Some(pings.max(0.0).floor() as u32)
}

/// What the call cost, in millionths of a dollar, at the prices of its model, named with or without the context window
/// it runs with (`[1m]`). A model with no prices here is priced as the dearest here, so that a cost not known errs high
/// against the budget.
pub fn of_call(model: &str, tokens: &Tokens) -> u64 {
    let id = model.split('[').next().unwrap_or(model);
    let (_, base, above) = (MODELS.iter())
        .find(|(name, ..)| *name == id)
        .copied()
        .unwrap_or(MODELS[0]);
    let p = match above {
        Some((over, above)) if tokens.prompt() > over => above,
        _ => base,
    };
    let fresh = tokens.input - tokens.written_1h - tokens.written_5m;
    let usd = fresh as f64 * p.input
        + tokens.read as f64 * p.read
        + tokens.written_5m as f64 * p.write_5m
        + tokens.written_1h as f64 * p.write_1h
        + tokens.output as f64 * p.output;
    // A million tokens at a dollar a million is a dollar, a million millionths.
    usd.round() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_call_costs_its_tokens_at_its_models_prices() {
        let usage = serde_json::json!({ "input_tokens": 1000, "cache_creation_input_tokens": 3000,
            "cache_creation": { "ephemeral_1h_input_tokens": 2000, "ephemeral_5m_input_tokens": 1000 },
            "cache_read_input_tokens": 10_000, "output_tokens": 500 });
        let tokens = Tokens::of(&usage);
        assert_eq!(tokens.input, 4000);
        assert_eq!((tokens.written_1h, tokens.written_5m), (2000, 1000));
        // Opus 5.5: 1000 × 4 + 10,000 × 0.2 + 1000 × 5 + 2000 × 8 + 500 × 20 millionths.
        assert_eq!(
            of_call("claude-opus-5-5", &tokens),
            4000 + 2000 + 5000 + 16_000 + 10_000
        );
        assert_eq!(
            of_call("claude-opus-5-5[1m]", &tokens),
            of_call("claude-opus-5-5", &tokens)
        );
        // A model not here costs as the dearest does.
        assert_eq!(of_call("claude-other", &tokens), of_call("claude-fable-5-1", &tokens));
        // A Haiku call whose prompt is past 100,000 tokens costs its dearer prices.
        let short = Tokens { read: 90_000, ..tokens };
        assert_eq!(of_call("claude-haiku-5-5", &short), 100 + 900 + 125 + 400 + 250);
        let long = Tokens {
            read: 200_000,
            ..tokens
        };
        assert_eq!(of_call("claude-haiku-5-5", &long), 500 + 10_000 + 625 + 2000 + 1250);
    }
}
