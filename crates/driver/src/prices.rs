//! What a model's tokens cost, as the agents' program's own reports of a work's calls show it, and how many keep-warm
//! pings a wait of a session on the model is worth. No model is named here, and the engine keeps no table of prices: a
//! model released tomorrow is priced and kept warm from what the program reports of it, as one released before is.
//!
//! The service prices every model in one shape, from what a token sent fresh costs: a write to the prompt cache kept
//! five minutes costs 1.25 times that, one kept an hour twice, and a token that comes back five times; a read from the
//! cache costs a share of it that differs from model to model. So two prices make a model's: a token sent fresh, and a
//! token read from the cache. What the program reports a round or a keep-warm ping cost is its tokens at those two
//! prices, and the reports of the work's rounds and pings on a model, which mix reading, writing and answering in
//! different measure, tell the two prices that fit them best ([`learned`]). A cost the engine worked out itself, its
//! program having printed no result to say it, is no report and is left out. The program keeps its sessions an hour,
//! so what it writes to the cache is taken as kept an hour.
//!
//! A ping is worth sending while what it costs, reading the session once, is less than what it saves: the session
//! written to the cache again for an hour at the next round, beyond reading it then. Both are the session's tokens at
//! the model's prices, so the number of pings a wait is worth is the ratio of the two prices less one, whatever the
//! session's size ([`Prices::pings_worth`]). Where the work's reports do not tell a model's prices, as before its
//! second report, a wait is worth [`UNMEASURED_PINGS`]: a read costs no more than a tenth of a token sent fresh on any
//! model, so that many pings are worth sending whatever the model.

use std::collections::{BTreeMap, BTreeSet};

use decide::RunId;
use flow::Compiled;
use work::{Spent, Work};

/// What a write to the cache kept five minutes costs, as a number of tokens sent fresh.
pub const WRITE_5M: f64 = 1.25;

/// What a write to the cache kept an hour costs, as a number of tokens sent fresh.
pub const WRITE_1H: f64 = 2.0;

/// What a token that comes back costs, as a number of tokens sent fresh.
pub const OUTPUT: f64 = 5.0;

/// How many pings one wait of a session is worth where nothing tells its model's prices: what an hour's write costs
/// over what a read costs, at the dearest a read is on any model, a tenth of a token sent fresh, less one.
pub const UNMEASURED_PINGS: u32 = 19;

/// What a model's tokens cost, in millionths of a dollar a token, which is dollars a million tokens: one sent fresh,
/// neither read from the cache nor written to it, and one read from the cache. What a write and a token that comes
/// back cost follows from the first ([`WRITE_5M`], [`WRITE_1H`], [`OUTPUT`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Prices {
    pub input: f64,
    pub read: f64,
}

impl Prices {
    /// How many keep-warm pings one wait of a session on the model is worth: as many as cost, each reading the session,
    /// no more than writing it to the cache for an hour would beyond reading it, which a ping saves where the session's
    /// next round comes before the cache lets it go.
    pub fn pings_worth(&self) -> u32 {
        // The small margin keeps a ratio such as 16.0 / 0.2, which floating point may make 79.99..., at 80.
        let pings = WRITE_1H * self.input / self.read - 1.0 + 1e-9;
        // A float past what a u32 holds is cast to the most it holds, and one that is no number to none.
        pings.max(0.0).floor() as u32
    }
}

/// The name of a model without the context window it runs with: `claude-opus-5-5[1m]` is `claude-opus-5-5`.
pub fn id(model: &str) -> &str {
    model.split('[').next().unwrap_or(model)
}

/// What the tokens of the model of this name, with or without its context window, cost as the work's own reports
/// show them: the rounds of the work on the model whose cost the program reported, and the pings that read the
/// sessions of its lines on the model. None where they do not tell both prices: where fewer than two of them report a
/// cost, where they all mix their tokens in one measure, or where a price comes out at nothing or below.
pub fn learned(flow: &Compiled, work: &Work, model: &str) -> Option<Prices> {
    let model = id(model);
    let mut reports = Reports::default();
    for (&(run, round), spent) in &work.spent {
        let told = work::measured::of_round(&work.measured, run, round).is_some_and(|m| !m.priced);
        if told && model_of(work, run, round).is_some_and(|on| id(on) == model) {
            reports.add(spent);
        }
    }
    for ping in work.pinged.iter().filter(|p| p.failed.is_none()) {
        let on_line = |run: RunId| crate::warm::line_of_run(flow, work, run) == Some(ping.line.as_str());
        let began = work::sessions::began_on(work, &ping.line, on_line, ping.session);
        if began.is_some_and(|on| id(&on.model) == model) {
            reports.add(&ping.spent);
        }
    }
    reports.prices()
}

/// What the tokens of each model the work's rounds ran on cost, as the work's own reports show them ([`learned`]), by
/// the model's name without its context window; a model they do not tell is not there.
pub fn learned_all(flow: &Compiled, work: &Work) -> BTreeMap<String, Prices> {
    let models: BTreeSet<&str> = work.ran.iter().map(|ran| id(&ran.on.model)).collect();
    (models.into_iter())
        .filter_map(|model| Some((model.to_string(), learned(flow, work, model)?)))
        .collect()
}

/// The model the round of this number of the run ran on: its last try's.
fn model_of(work: &Work, run: RunId, round: usize) -> Option<&str> {
    (work.ran.iter().rev())
        .find(|ran| ran.run == run && ran.round == round)
        .map(|ran| ran.on.model.as_str())
}

/// Reports of what calls cost, by which the prices that fit them best are worked out: each report is its tokens sent
/// fresh, written and come back, at what each costs as a number of tokens sent fresh, `a`, and its tokens read from the
/// cache, `b`, both over its cost, so that every report weighs alike, small or large; these are the sums the least
/// squares of `input × a + read × b − 1` are found from.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Reports {
    aa: f64,
    ab: f64,
    bb: f64,
    a: f64,
    b: f64,
}

impl Reports {
    /// Adds what a round or a ping took. One that cost nothing reports no price, one whose cost was not all known
    /// reports a part of it, and one kept before what was written to the cache was kept apart cannot be split: each of
    /// them is left out.
    fn add(&mut self, spent: &Spent) {
        let Some(written) = spent.cache_written_tokens else {
            return;
        };
        if spent.usd_micros == 0 || spent.unpriced {
            return;
        }
        let cost = spent.usd_micros as f64;
        let sent = spent.uncached_tokens.saturating_sub(written) as f64;
        let a = (sent + WRITE_1H * written as f64 + OUTPUT * spent.output_tokens as f64) / cost;
        let b = spent.cached_tokens as f64 / cost;
        self.aa += a * a;
        self.ab += a * b;
        self.bb += b * b;
        self.a += a;
        self.b += b;
    }

    /// The two prices that fit the reports best: none where they cannot tell them apart, too few, or all mixing their
    /// tokens in one measure, or where a price comes out at nothing or below.
    fn prices(&self) -> Option<Prices> {
        let d = self.aa * self.bb - self.ab * self.ab;
        if d <= 1e-9 * self.aa * self.bb {
            return None;
        }
        let input = (self.a * self.bb - self.b * self.ab) / d;
        let read = (self.aa * self.b - self.ab * self.a) / d;
        (input.is_finite() && read.is_finite() && input > 0.0 && read > 0.0).then_some(Prices { input, read })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What a call of these tokens took at these prices: its cost in millionths of a dollar, and its tokens sent fresh,
    /// written to the cache for an hour, read from it and come back.
    fn spent(p: Prices, fresh: u64, written: u64, read: u64, output: u64) -> Spent {
        let usd = p.input * (fresh as f64 + WRITE_1H * written as f64 + OUTPUT * output as f64) + p.read * read as f64;
        Spent {
            usd_micros: usd.round() as u64,
            uncached_tokens: fresh + written,
            cache_written_tokens: Some(written),
            cached_tokens: read,
            output_tokens: output,
            ..Spent::default()
        }
    }

    /// Two pings, almost all reading, and a round that also wrote and answered.
    fn reports(p: Prices) -> Reports {
        let mut reports = Reports::default();
        reports.add(&spent(p, 10, 60, 400_000, 20));
        reports.add(&spent(p, 0, 40, 420_000, 12));
        reports.add(&spent(p, 5_000, 50_000, 3_000_000, 20_000));
        reports
    }

    // Of a model whose prices the code has never been told, the reports of a few calls tell both prices, and a ping on
    // it is worth as many times as they make it worth; the same reports of a dearer read are worth fewer. Fails where
    // the prices come from a model's name.
    #[test]
    fn a_few_reports_tell_a_models_prices_and_the_pings_a_wait_is_worth() {
        for (prices, worth) in [
            (Prices { input: 8.0, read: 0.2 }, 79),
            (Prices { input: 4.0, read: 0.2 }, 39),
            (Prices { input: 0.1, read: 0.01 }, 19),
            (Prices { input: 3.0, read: 0.3 }, 19),
        ] {
            let learned = reports(prices).prices().expect("three reports tell two prices");
            assert!(
                (learned.input - prices.input).abs() < prices.input * 1e-9,
                "{learned:?}"
            );
            assert!((learned.read - prices.read).abs() < prices.read * 1e-9, "{learned:?}");
            assert_eq!(learned.pings_worth(), worth, "{prices:?}");
        }
        // Two rounds alone tell them too, where they mix their tokens in different measure.
        let p = Prices { input: 8.0, read: 0.2 };
        let mut rounds = Reports::default();
        rounds.add(&spent(p, 5_000, 50_000, 3_000_000, 20_000));
        rounds.add(&spent(p, 20_000, 300_000, 100_000, 5_000));
        assert_eq!(rounds.prices().map(|p| p.pings_worth()), Some(79));
    }

    // Nothing to tell the prices by gives none, never a price of nothing: no report, one report, one report twice, a
    // report of nothing spent beside one other, a report whose cost was not all known, and one kept before what it
    // wrote was kept apart.
    #[test]
    fn reports_that_cannot_tell_the_prices_give_none() {
        let p = Prices { input: 8.0, read: 0.2 };
        let round = spent(p, 5_000, 50_000, 3_000_000, 20_000);
        let of = |spents: &[Spent]| {
            let mut reports = Reports::default();
            for spent in spents {
                reports.add(spent);
            }
            reports.prices()
        };
        assert_eq!(of(&[]), None, "no report");
        assert_eq!(of(&[round]), None, "one report");
        assert_eq!(of(&[round, round]), None, "one report twice");
        let nothing = Spent {
            usd_micros: 0,
            ..spent(p, 3, 60, 400_000, 25)
        };
        assert_eq!(of(&[nothing, round]), None, "a ping the program said cost nothing");
        let unpriced = Spent {
            unpriced: true,
            ..spent(p, 3, 60, 400_000, 25)
        };
        assert_eq!(of(&[unpriced, round]), None, "a ping whose cost was not all known");
        let old = Spent {
            cache_written_tokens: None,
            ..spent(p, 3, 60, 400_000, 25)
        };
        assert_eq!(
            of(&[old, round]),
            None,
            "what was written cannot be told from what was sent"
        );
    }

    // Where nothing tells the prices, a wait is worth as many pings as at the dearest read any model has, a tenth of a
    // token sent fresh, so that no ping is sent that is not worth it whatever the model; never none.
    #[test]
    fn a_wait_whose_prices_are_not_known_is_worth_the_pings_of_the_dearest_read() {
        assert_eq!(Prices { input: 1.0, read: 0.1 }.pings_worth(), UNMEASURED_PINGS);
    }

    #[test]
    fn a_models_name_is_read_without_its_context_window() {
        assert_eq!(id("claude-opus-5-5[1m]"), "claude-opus-5-5");
        assert_eq!(id("claude-next-9"), "claude-next-9");
    }
}
