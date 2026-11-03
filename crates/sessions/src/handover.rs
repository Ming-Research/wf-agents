//! When a line's session hands its progress over to a new one. Keeping a session costs, at each of its calls, a read
//! of all it added past its first prompt (the rent); a new session costs at most writing its whole context to the
//! cache again (the buy). Once the rent paid comes to the buy, the session is due to hand over: the rule of the
//! rent-or-buy problem, which spends at most twice what the best choice in hindsight would, however much work is left.
//! Only the session's own calls and the models' prices are read: no other session's history, and no number set by hand.
//! The flow's sessions section says the rule for people; who asks, and when, is the engine's.

use db::rusqlite::{self, Connection, OptionalExtension, params};

use crate::cli::Call;
use crate::cost;

/// What a session's calls say about handing it over, in US dollars.
#[derive(Debug, Clone, PartialEq)]
pub enum Judged {
    /// Keeping it has cost less than starting anew would.
    NotYet { rent: f64, buy: f64 },
    /// Keeping it has cost as much as starting anew: it is due to hand over.
    Due { rent: f64, buy: f64 },
    /// A call's model has no prices: nothing is judged.
    NoPrices { model: String },
}

impl Judged {
    pub fn due(&self) -> bool {
        matches!(self, Judged::Due { .. })
    }

    /// The rent paid and the buy, when judged.
    pub fn costs(&self) -> Option<(f64, f64)> {
        match self {
            Judged::NotYet { rent, buy } | Judged::Due { rent, buy } => Some((*rent, *buy)),
            Judged::NoPrices { .. } => None,
        }
    }
}

/// The rent a session has paid, taken call by call in the order the calls were made. A subagent's call is not in the
/// session's context, and is left out.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Rent {
    /// The prompt of the session's first call of its own: what it opened with, which no new session saves.
    pub opening: Option<i64>,
    /// What its calls paid to read again what it added past its opening, in US dollars.
    pub rent: f64,
    /// The prompt of its last call of its own, and that call's model.
    pub last: Option<(i64, String)>,
    /// Whether it wrote the cache to be kept an hour, which a new session of it would write the same way.
    hour: bool,
    /// The first model of its calls with no prices.
    unpriced: Option<String>,
}

impl Rent {
    /// Takes the session's next call, on the model the call names or else `model`.
    pub fn take(&mut self, c: &Call, model: &str) {
        if c.subagent {
            return;
        }
        let name = c.model.clone().unwrap_or_else(|| model.to_string());
        let prompt = c.prompt();
        let opening = *self.opening.get_or_insert(prompt);
        self.hour |= c.written_1h > 0;
        match cost::prices(&name, prompt) {
            // Each call reads again all the session added past its opening; after a compaction it holds less.
            Some(p) => self.rent += (prompt - opening).max(0) as f64 * p.read / 1e6,
            None => {
                self.unpriced.get_or_insert_with(|| name.clone());
            }
        }
        self.last = Some((prompt, name));
    }

    /// What the calls taken say; none before the session's first call of its own. A session that has paid no rent is
    /// not due, whatever a new one would cost.
    pub fn judged(&self) -> Option<Judged> {
        if let Some(model) = &self.unpriced {
            return Some(Judged::NoPrices { model: model.clone() });
        }
        let (prompt, model) = self.last.as_ref()?;
        let Some(p) = cost::prices(model, *prompt) else {
            return Some(Judged::NoPrices { model: model.clone() });
        };
        // A new session writes at most the whole context again, kept as long as this one kept it.
        let write = if self.hour { p.write_1h } else { p.write_5m };
        let buy = *prompt as f64 * write / 1e6;
        let rent = self.rent;
        Some(if rent > 0.0 && rent >= buy {
            Judged::Due { rent, buy }
        } else {
            Judged::NotYet { rent, buy }
        })
    }
}

/// What a session's calls, in the order made, say about handing it over, each on the model it names or else `model`.
pub fn judge(calls: &[Call], model: &str) -> Option<Judged> {
    let mut r = Rent::default();
    for c in calls {
        r.take(c, model);
    }
    r.judged()
}

/// The calls the ledger kept of a CLI session, in the order made: each of its rounds' in the order the rounds ran, the
/// architect's across its works, subagents' included. A call that names no model is on its round's. A round from
/// before the ledger has none.
pub fn calls(conn: &Connection, cli_session: &str) -> Result<Vec<Call>, String> {
    let mut st = conn
        .prepare(
            "SELECT c.message, coalesce(c.model, r.model), c.input, c.read, c.written_5m, c.written_1h, c.output,
                    c.subagent
             FROM round_calls c JOIN rounds r ON r.work = c.work AND r.n = c.round JOIN sessions s ON s.id = r.session
             WHERE s.cli_session = ?1 ORDER BY c.work, c.round, c.seq",
        )
        .map_err(|e| e.to_string())?;
    st.query_map([cli_session], |r| {
        Ok(Call {
            id: r.get(0)?,
            model: r.get(1)?,
            input: r.get(2)?,
            read: r.get(3)?,
            written_5m: r.get(4)?,
            written_1h: r.get(5)?,
            output: r.get(6)?,
            subagent: r.get::<_, i64>(7)? != 0,
        })
    })
    .map_err(|e| e.to_string())?
    .collect::<rusqlite::Result<_>>()
    .map_err(|e| e.to_string())
}

/// The rent a CLI session has paid by its calls the ledger kept, `model` for a call whose round names none.
pub fn rent_of(conn: &Connection, cli_session: &str, model: &str) -> Result<Rent, String> {
    let mut r = Rent::default();
    for c in calls(conn, cli_session)? {
        r.take(&c, model);
    }
    Ok(r)
}

/// What a CLI session's calls the ledger kept say about handing it over.
pub fn of_session(conn: &Connection, cli_session: &str, model: &str) -> Result<Option<Judged>, String> {
    Ok(rent_of(conn, cli_session, model)?.judged())
}

/// What the calls of the session a round runs in say about handing it over, those the ledger kept so far: the round's
/// own as far as its watch has kept them. None for a round that runs in no session.
pub fn of_round(conn: &Connection, work: i64, n: i64) -> Result<Option<Judged>, String> {
    let found: Option<(String, Option<String>)> = conn
        .query_row(
            "SELECT s.cli_session, r.model FROM rounds r JOIN sessions s ON s.id = r.session
             WHERE r.work = ?1 AND r.n = ?2",
            params![work, n],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let Some((cli_session, model)) = found else {
        return Ok(None);
    };
    of_session(conn, &cli_session, model.as_deref().unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SONNET: &str = "claude-sonnet-5-5";

    /// A call of the session's own that read `prompt` tokens from the cache, on the model the round runs on.
    fn read(id: &str, prompt: i64) -> Call {
        Call {
            id: id.into(),
            read: prompt,
            ..Call::default()
        }
    }

    /// The index of the first call at which a session's calls are due to hand over; none when they never are.
    fn due_at(calls: &[Call], model: &str) -> Option<usize> {
        let mut r = Rent::default();
        calls.iter().position(|c| {
            r.take(c, model);
            r.judged().is_some_and(|j| j.due())
        })
    }

    fn near(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    // On Sonnet 5.5 a read costs $0.10 a million tokens and a write kept five minutes $2.50: a session that opened at
    // 10,000 tokens and holds 110,000 pays $0.01 a call in rent against a buy of $0.275, so it is due at the 29th call
    // (28 calls past its opening, $0.28), not at the 28th ($0.27).
    #[test]
    fn a_session_is_due_at_the_first_call_whose_rent_comes_to_the_buy() {
        let mut calls = vec![read("a", 10_000)];
        calls.extend((1..40).map(|i| read(&format!("c{i}"), 110_000)));
        assert_eq!(due_at(&calls, SONNET), Some(28));
        let before = judge(&calls[..28], SONNET).unwrap();
        let at = judge(&calls[..29], SONNET).unwrap();
        let (rent, buy) = before.costs().unwrap();
        assert!(!before.due() && near(rent, 0.27) && near(buy, 0.275), "{before:?}");
        let (rent, buy) = at.costs().unwrap();
        assert!(at.due() && near(rent, 0.28) && near(buy, 0.275), "{at:?}");
    }

    // A subagent's calls are not in the session's context: they pay no rent, and its prompt is not the last.
    #[test]
    fn a_subagents_calls_pay_no_rent() {
        let mut calls = vec![read("a", 10_000)];
        calls.extend((1..40).map(|i| read(&format!("c{i}"), 110_000)));
        let mut with = calls.clone();
        for i in [3, 10, 20] {
            with.insert(
                i,
                Call {
                    subagent: true,
                    ..read(&format!("s{i}"), 900_000)
                },
            );
        }
        let own = due_at(&calls, SONNET).unwrap();
        let at = due_at(&with, SONNET).unwrap();
        assert_eq!(with[at].id, calls[own].id);
        assert_eq!(judge(&with, SONNET), judge(&calls, SONNET));
    }

    // After a compaction the context falls back near its opening: a call after it pays rent only for what it holds past
    // the opening, and the buy is of what the session holds now; what was paid before stays paid.
    #[test]
    fn a_compaction_lowers_both_the_rent_of_each_call_and_the_buy() {
        let mut calls = vec![read("a", 10_000)];
        calls.extend((1..30).map(|i| read(&format!("c{i}"), 110_000)));
        assert!(judge(&calls, SONNET).unwrap().due());
        calls.push(read("compacted", 12_000));
        let after = judge(&calls, SONNET).unwrap();
        let (rent, buy) = after.costs().unwrap();
        assert!(after.due() && near(rent, 0.29 + 0.0002) && near(buy, 0.03), "{after:?}");
        let mut low = vec![read("a", 10_000)];
        low.extend((1..4).map(|i| read(&format!("c{i}"), 110_000)));
        low.push(read("compacted", 12_000));
        low.extend((5..20).map(|i| read(&format!("d{i}"), 12_000)));
        let (rent, _) = judge(&low, SONNET).unwrap().costs().unwrap();
        assert!(near(rent, 0.03 + 16.0 * 0.0002), "{rent}");
    }

    // A session that wrote the cache to be kept an hour would write a new one so too, at $4.00 a million on Sonnet 5.5
    // where five minutes cost $2.50: the buy is that much higher, and the session due that much later. At 105,000
    // tokens, $0.0095 of rent a call: due at $0.266 against $0.2625 (call 28), or at $0.4275 against $0.42 (call 45).
    #[test]
    fn a_session_that_wrote_for_an_hour_buys_at_the_hours_price() {
        let mut five = vec![read("a", 10_000)];
        five.extend((1..60).map(|i| read(&format!("c{i}"), 105_000)));
        let mut hour = five.clone();
        hour[0].written_1h = 1_000;
        hour[0].read -= 1_000;
        assert_eq!(due_at(&five, SONNET), Some(28));
        assert_eq!(due_at(&hour, SONNET), Some(45));
        let (_, buy) = judge(&hour, SONNET).unwrap().costs().unwrap();
        assert!(near(buy, 0.42), "{buy}");
    }

    // A call of a model with no prices leaves the session unjudged, whatever the other calls say, and names the model;
    // the price of each call is its own model's, so a call on another model with prices is judged by those.
    #[test]
    fn a_model_with_no_prices_is_never_judged() {
        let mut calls = vec![read("a", 10_000)];
        calls.extend((1..40).map(|i| read(&format!("c{i}"), 110_000)));
        calls[5].model = Some("some-model".into());
        assert_eq!(
            judge(&calls, SONNET),
            Some(Judged::NoPrices {
                model: "some-model".into()
            })
        );
        assert_eq!(judge(&calls[..3], "m"), Some(Judged::NoPrices { model: "m".into() }));
        // Opus 5.5 reads at $0.20 and writes for five minutes at $5.00, the ratio Sonnet 5.5 has: due at the same call.
        let opus: Vec<Call> = calls[..5]
            .iter()
            .chain(&calls[6..])
            .map(|c| Call {
                model: Some("claude-opus-5-5".into()),
                ..c.clone()
            })
            .collect();
        assert_eq!(due_at(&opus, SONNET), Some(28));
    }

    // A single call, and a session whose calls held nothing, have paid no rent: neither is due.
    #[test]
    fn a_session_that_paid_no_rent_is_not_due() {
        assert_eq!(judge(&[], SONNET), None);
        assert!(!judge(&[read("a", 400_000)], SONNET).unwrap().due());
        assert!(!judge(&[read("a", 0), read("b", 0)], SONNET).unwrap().due());
    }

    /// A database with one engineer work and a CLI session `name` of its, with a round at `cell` for each list of
    /// prompts, each call a row of the ledger: `subagent` marks the session's calls of subagents.
    fn sessions_db() -> db::Db {
        let mut db = db::Db::in_memory().unwrap();
        db.write(db::Actor::Engine, |tx| {
            tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
            tx.execute(
                "INSERT INTO works (flow_version, graph, at) VALUES (1, 'engineer', 'I1')",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        db
    }

    fn add_session(db: &mut db::Db, name: &str, rounds: &[Vec<i64>]) {
        db.write(db::Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO sessions (work, line, cli, cli_session, ended) VALUES (1, 'engineer', 'claude', ?1, 1)",
                [name],
            )?;
            let session = tx.last_insert_rowid();
            for prompts in rounds {
                let n: i64 = tx.query_row("SELECT coalesce(max(n), 0) + 1 FROM rounds WHERE work = 1", [], |r| {
                    r.get(0)
                })?;
                tx.execute(
                    "INSERT INTO rounds (work, n, cell, session, input, config_commit, model, started_at)
                     VALUES (1, ?1, 'I1', ?2, '{}', 'c', ?3, ?1)",
                    params![n, session, SONNET],
                )?;
                for (i, p) in prompts.iter().enumerate() {
                    tx.execute(
                        "INSERT INTO round_calls (work, round, seq, message, model, input, read, written_5m,
                                                  written_1h, output, subagent, at)
                         VALUES (1, ?1, ?2, ?3, NULL, 0, ?4, 0, 0, 10, 0, ?1)",
                        params![n, i as i64 + 1, format!("{name}-{n}-{i}"), p],
                    )?;
                }
            }
            Ok(())
        })
        .unwrap();
    }

    // The rule reads only the session's own calls: a run of 362 calls and a hundred rounds of two calls each, at the same
    // cell in other sessions, leave the call at which a long session is due where it was. Its calls are read across its
    // rounds in order, on the model of their round.
    #[test]
    fn other_sessions_of_the_cell_leave_a_sessions_due_call_where_it_was() {
        let mut db = sessions_db();
        let long: Vec<Vec<i64>> = vec![
            (0..20).map(|i| 30_000 + i * 1_500).collect(),
            (0..40).map(|i| 60_000 + i * 1_500).collect(),
            (0..40).map(|i| 120_000 + i * 1_500).collect(),
        ];
        add_session(&mut db, "long", &long);
        let due = |db: &db::Db| due_at(&calls(db.conn(), "long").unwrap(), "unused");
        let alone = due(&db);
        assert!(alone.is_some(), "the long session is due somewhere");
        add_session(&mut db, "runaway", &[(0..362).map(|i| 47_662 + i * 1_537).collect()]);
        for k in 0..100 {
            add_session(&mut db, &format!("short-{k}"), &[vec![200_000, 230_000]]);
        }
        assert_eq!(due(&db), alone);
        assert_eq!(calls(db.conn(), "long").unwrap().len(), 100);
        // The long session's last round, the third of the work, is judged by its session's calls; the first short
        // session's round, the fifth, by its own two.
        assert!(of_round(db.conn(), 1, 3).unwrap().is_some_and(|j| j.due()));
        assert!(of_round(db.conn(), 1, 5).unwrap().is_some_and(|j| !j.due()));
    }
}
