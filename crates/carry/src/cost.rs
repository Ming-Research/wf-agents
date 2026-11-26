//! What a call to the model cost, from its tokens: the command line says what a round cost only in its result, which a
//! round that crashed or was stopped never prints, and a round tried again must still be counted against its work's
//! budget. The engine keeps no table of prices: a call the program reported no cost of is priced at what the program's
//! earlier reports of the work's calls on its model tell ([`at`], crate `driver`, module `prices`), and where they tell
//! nothing, its cost is not known, which is said and never taken as nothing (`Stream::unpriced`).

use serde_json::Value;

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

/// What the call cost, in millionths of a dollar, at its model's prices as the program's earlier reports of the work's
/// calls on it tell them ([`driver::prices::learned`]): a token sent fresh and one read from the cache at what they
/// tell, and a write to the cache and a token that comes back at what every model's prices make of the first.
pub fn at(learned: &driver::prices::Prices, tokens: &Tokens) -> u64 {
    use driver::prices::{OUTPUT, WRITE_1H, WRITE_5M};
    let input = learned.input;
    let fresh = tokens.input - tokens.written_1h - tokens.written_5m;
    let usd = fresh as f64 * input
        + tokens.read as f64 * learned.read
        + tokens.written_5m as f64 * input * WRITE_5M
        + tokens.written_1h as f64 * input * WRITE_1H
        + tokens.output as f64 * input * OUTPUT;
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
        // At a token sent fresh for four millionths of a dollar and one read for a fifth of one: 1000 × 4 + 10,000 ×
        // 0.2 + 1000 × 5 + 2000 × 8 + 500 × 20 millionths, whatever the model is named.
        let learned = driver::prices::Prices { input: 4.0, read: 0.2 };
        assert_eq!(at(&learned, &tokens), 4000 + 2000 + 5000 + 16_000 + 10_000);
    }
}
