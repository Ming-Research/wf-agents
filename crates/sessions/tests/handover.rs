//! The hand-over rule at the edges its own tests leave: prices that change with the size of the prompt, a model named
//! with its context window, a session whose model changes, a cache written for an hour after the session's first call,
//! subagents' calls that name a model with no prices, and the ledger's rows of subagents. Each case's numbers come from
//! the rule as the design states it (rent: what each of the session's own calls in the round under way read past the
//! session's opening, at that call's read price; buy: the last prompt at the write price of the cache the session
//! kept) and from the price table the engine already has (`sessions::cost`, the command line's model catalog).

use db::rusqlite::params;
use db::{Actor, Db};
use sessions::cli::Call;
use sessions::handover::{self, Judged, Rent, judge};

const SONNET: &str = "claude-sonnet-5-5";
const OPUS: &str = "claude-opus-5-5";
const HAIKU: &str = "claude-haiku-5-5";

/// A call of the session's own that read `prompt` tokens from the cache, on the model the round runs on.
fn read(id: &str, prompt: i64) -> Call {
    Call {
        id: id.into(),
        read: prompt,
        ..Call::default()
    }
}

/// A session that opens at 10,000 tokens and then holds `prompt` for `calls` calls.
fn session(calls: usize, prompt: i64) -> Vec<Call> {
    let mut out = vec![read("a", 10_000)];
    out.extend((1..=calls).map(|i| read(&format!("c{i}"), prompt)));
    out
}

/// The index of the first call at which the calls are due to hand over; none when they never are.
fn due_at(calls: &[Call], model: &str) -> Option<usize> {
    let mut r = Rent::default();
    calls.iter().position(|c| {
        r.take(c, model);
        r.judged().is_some_and(|j| j.due())
    })
}

/// Whether a judgment is not due, with the rent and the buy given, to a billionth of a dollar.
fn not_yet(j: &Option<Judged>, rent: f64, buy: f64) -> bool {
    matches!(j, Some(Judged::NotYet { rent: r, buy: b }) if (r - rent).abs() < 1e-9 && (b - buy).abs() < 1e-9)
}

// Haiku 5.5 reads at $0.01 a million and writes for five minutes at $0.125, and five times that for a call whose prompt
// is over 100,000 tokens. Each call pays rent at its own prompt's read price, and the buy is at the last prompt's write
// price. Ten calls at 90,000 past an opening of 10,000 pay 10 × 80,000 × $0.01 = $0.008 against a buy of
// 90,000 × $0.125 = $0.01125; a call at 110,000 then pays 100,000 × $0.05 = $0.005, and the buy is
// 110,000 × $0.625 = $0.06875. Priced at the lower prices, the last call would pay $0.001 against $0.01375; priced
// all at the higher, the ten would have paid $0.04.
#[test]
fn each_call_pays_rent_at_its_own_prompts_price_and_the_buy_is_at_the_last() {
    let mut calls = session(10, 90_000);
    assert!(
        not_yet(&judge(&calls, HAIKU), 0.008, 0.01125),
        "{:?}",
        judge(&calls, HAIKU)
    );
    calls.push(read("long", 110_000));
    assert!(
        not_yet(&judge(&calls, HAIKU), 0.013, 0.06875),
        "{:?}",
        judge(&calls, HAIKU)
    );
    // At 90,000 a call pays $0.0008 against $0.01125: due at the 15th call past the opening ($0.012), not the 14th
    // ($0.0112).
    assert_eq!(due_at(&session(30, 90_000), HAIKU), Some(15));
}

// A model named with its context window, as the command line names Sonnet 5.5 with a million tokens, is priced as the
// model (`cost::prices`): its session is judged, and due at the call a session of the model is.
#[test]
fn a_model_named_with_its_context_window_is_judged_at_the_models_prices() {
    let calls = session(39, 110_000);
    let wide = "claude-sonnet-5-5[1m]";
    assert_eq!(due_at(&calls, wide), due_at(&calls, SONNET));
    assert_eq!(due_at(&calls, wide), Some(28));
    assert!(judge(&calls, wide).is_some_and(|j| j.due()));
}

// A session whose calls change model in its middle pays each call's rent at that call's model's read price, and buys
// at the last call's model's write price: ten calls of Sonnet 5.5 at 110,000 past 10,000 pay 10 × 100,000 × $0.10 =
// $0.10, a call of Opus 5.5 at 110,000 then 100,000 × $0.20 = $0.02, and the buy is 110,000 × $5.00 = $0.55 (at
// Sonnet's $2.50 it would be $0.275, which the $0.12 paid is still short of, so only the rent and the buy tell).
#[test]
fn a_session_whose_model_changes_buys_at_its_last_calls_model() {
    let mut calls = session(10, 110_000);
    calls.push(Call {
        model: Some(OPUS.into()),
        ..read("opus", 110_000)
    });
    assert!(
        not_yet(&judge(&calls, SONNET), 0.12, 0.55),
        "{:?}",
        judge(&calls, SONNET)
    );
}

// A session that wrote the cache for an hour at any of its calls, not only its first, buys at the hour's price, though
// its later calls write for five minutes: at 105,000 tokens past 10,000 on Sonnet 5.5 it pays $0.0095 a call, due at
// the 28th call against $0.2625 for five minutes, and at the 45th against $0.42 for an hour.
#[test]
fn a_cache_written_for_an_hour_at_a_later_call_prices_the_buy_for_an_hour() {
    let five = session(59, 105_000);
    assert_eq!(due_at(&five, SONNET), Some(28));
    let mut hour = five.clone();
    hour[10].written_1h = 1_000;
    hour[10].read -= 1_000;
    hour[20].written_5m = 500;
    hour[20].read -= 500;
    assert_eq!(due_at(&hour, SONNET), Some(45));
}

// A subagent's calls are not in the session's context: one that names a model with no prices does not leave the
// session unjudged, and one that writes the cache for an hour does not price the session's buy for an hour.
#[test]
fn a_subagents_call_on_a_model_with_no_prices_or_for_an_hour_changes_nothing() {
    let calls = session(39, 110_000);
    let mut with = calls.clone();
    for i in [2, 9, 30] {
        with.insert(
            i,
            Call {
                model: Some("a-model-with-no-prices".into()),
                subagent: true,
                written_1h: 5_000,
                ..read(&format!("s{i}"), 60_000)
            },
        );
    }
    assert_eq!(judge(&with, SONNET), judge(&calls, SONNET));
    assert!(judge(&with, SONNET).is_some_and(|j| j.due()));
}

// The ledger keeps a subagent's calls among the session's (`round_calls.subagent`), and a call that names its model:
// read back for the rule, a subagent's call pays no rent, though it names a model with no prices, and the session's own
// calls are read across its rounds, another session's round between them, each with the round it was made in.
#[test]
fn the_ledgers_subagent_rows_pay_no_rent_when_read_back() {
    let mut db = Db::in_memory().unwrap();
    db.write(Actor::Engine, |tx| {
        tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
        tx.execute(
            "INSERT INTO works (flow_version, graph, at) VALUES (1, 'engineer', 'I1')",
            [],
        )?;
        tx.execute(
            "INSERT INTO sessions (work, line, cli, cli_session) VALUES (1, 'engineer', 'claude', 'own')",
            [],
        )?;
        tx.execute(
            "INSERT INTO sessions (work, line, cli, cli_session, ended) VALUES (1, 'reviewer', 'claude', 'other', 1)",
            [],
        )?;
        // Rounds 1 and 3 run in the session, round 2 in another between them.
        for (n, session) in [(1, 1), (2, 2), (3, 1)] {
            tx.execute(
                "INSERT INTO rounds (work, n, cell, session, input, config_commit, model, started_at)
                 VALUES (1, ?1, 'I1', ?2, '{}', 'c', ?3, ?1)",
                params![n, session, SONNET],
            )?;
        }
        let keep = |round: i64, seq: i64, id: &str, model: Option<&str>, read: i64, subagent: bool| {
            tx.execute(
                "INSERT INTO round_calls (work, round, seq, message, model, input, read, written_5m, written_1h,
                                          output, subagent, at)
                 VALUES (1, ?1, ?2, ?3, ?4, 0, ?5, 0, 0, 10, ?6, ?1)",
                params![round, seq, id, model, read, i64::from(subagent)],
            )
        };
        keep(1, 1, "a", None, 10_000, false)?;
        for i in 1..=10 {
            keep(1, i + 1, &format!("c{i}"), None, 110_000, false)?;
        }
        keep(1, 12, "s1", Some("a-model-with-no-prices"), 900_000, true)?;
        for i in 1..=50 {
            keep(2, i, &format!("o{i}"), None, 400_000, false)?;
        }
        for i in 1..=20 {
            keep(3, i, &format!("d{i}"), Some(SONNET), 110_000, false)?;
        }
        keep(3, 21, "s2", Some("a-model-with-no-prices"), 900_000, true)?;
        Ok(())
    })
    .unwrap();
    let calls = handover::calls(db.conn(), "own").unwrap();
    assert_eq!(calls.len(), 33);
    assert_eq!(calls.iter().filter(|(_, c)| c.subagent).count(), 2);
    assert_eq!(calls.iter().filter(|(of, _)| *of == (1, 3)).count(), 21);
    // Round 3 pays rent for its own twenty calls only, past the session's opening in round 1: $0.20 against $0.275.
    // Across the session's thirty calls it would have paid $0.30, and been due.
    let j = handover::of_round(db.conn(), 1, 3).unwrap();
    assert!(not_yet(&j, 0.20, 0.275), "{j:?}");
}

/// Writes the calls of an architect's CLI session `cli` that goes on across two works, each work with a row of its own
/// in the sessions table under the same CLI session, as the schema says the architect's line keeps it: work 1's round
/// opens at 10,000 tokens and makes five calls at 50,000 (the first on `first_model`, the opening writing for an hour
/// when `hour`); work 2's round makes `calls` calls at 110,000. Works 1 and 2 are made once; returns work 2's round.
fn across_works(db: &mut Db, cli: &str, hour: bool, first_model: Option<&str>, calls: i64) -> (i64, i64) {
    db.write(Actor::Engine, |tx| {
        let works: i64 = tx.query_row("SELECT count(*) FROM works", [], |r| r.get(0))?;
        if works == 0 {
            tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
            for _ in 0..2 {
                tx.execute(
                    "INSERT INTO works (flow_version, graph, at) VALUES (1, 'architect', 'U1')",
                    [],
                )?;
            }
        }
        let mut rounds = Vec::new();
        for work in [1, 2] {
            tx.execute(
                "INSERT INTO sessions (work, line, cli, cli_session, ended) VALUES (?1, ?2, 'claude', ?3, 1)",
                params![work, format!("architect-{cli}"), cli],
            )?;
            let session = tx.last_insert_rowid();
            let n: i64 = tx.query_row(
                "SELECT coalesce(max(n), 0) + 1 FROM rounds WHERE work = ?1",
                [work],
                |r| r.get(0),
            )?;
            tx.execute(
                "INSERT INTO rounds (work, n, cell, session, input, config_commit, model, started_at)
                 VALUES (?1, ?2, 'U1', ?3, '{}', 'c', ?4, ?2)",
                params![work, n, session, SONNET],
            )?;
            rounds.push((work, n));
        }
        let keep = |(work, n): (i64, i64), seq: i64, model: Option<&str>, read: i64, written_1h: i64| {
            tx.execute(
                "INSERT INTO round_calls (work, round, seq, message, model, input, read, written_5m, written_1h,
                                          output, subagent, at)
                 VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6, 0, ?7, 10, 0, ?2)",
                params![work, n, seq, format!("{cli}-{work}-{seq}"), model, read, written_1h],
            )
        };
        let opening_1h = if hour { 1_000 } else { 0 };
        keep(rounds[0], 1, first_model, 10_000 - opening_1h, opening_1h)?;
        for seq in 2..=6 {
            keep(rounds[0], seq, None, 50_000, 0)?;
        }
        for seq in 1..=calls {
            keep(rounds[1], seq, None, 110_000, 0)?;
        }
        Ok(rounds[1])
    })
    .unwrap()
}

// The architect's CLI session goes on across works, so its opening is its first call in the first work, and the rent
// of a round in the next work counts that round's own calls only, past that opening (the design's rule and pitfalls).
// On Sonnet 5.5, a round at 110,000 past an opening of 10,000 pays $0.01 a call against a buy of $0.275: NotYet at its
// 27th call ($0.27), Due at its 28th ($0.28). Taking the opening from the second work's first call would pay nothing;
// carrying work 1's rent ($0.02) over would be due at the 27th. The session's hour's write in work 1 prices the buy for
// an hour in work 2 ($0.44), and a call of work 1 on a model with no prices leaves work 2's round judged.
#[test]
fn an_architects_session_across_works_pays_rent_by_the_round_past_its_first_works_opening() {
    let mut db = Db::in_memory().unwrap();
    let r27 = across_works(&mut db, "a27", false, None, 27);
    let r28 = across_works(&mut db, "a28", false, None, 28);
    let hour = across_works(&mut db, "hour", true, None, 28);
    let unpriced = across_works(&mut db, "unpriced", false, Some("a-model-with-no-prices"), 28);
    let at = |(w, n): (i64, i64)| handover::of_round(db.conn(), w, n).unwrap();
    assert!(not_yet(&at(r27), 0.27, 0.275), "{:?}", at(r27));
    let due = at(r28);
    assert!(
        matches!(&due, Some(Judged::Due { rent, buy }) if (rent - 0.28).abs() < 1e-9 && (buy - 0.275).abs() < 1e-9),
        "{due:?}"
    );
    assert!(not_yet(&at(hour), 0.28, 0.44), "{:?}", at(hour));
    assert!(at(unpriced).is_some_and(|j| j.due()), "{:?}", at(unpriced));
    // The watch of the round about to start in work 2 starts from the same opening, having paid nothing.
    let start = handover::rent_in(db.conn(), "a28", SONNET, (2, r28.1 + 1)).unwrap();
    assert_eq!((start.opening, start.rent), (Some(10_000), 0.0));
}
