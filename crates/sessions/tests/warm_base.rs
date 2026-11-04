//! Keeping a project's architect's base warm for its forks: the base is warmed before a fork when the cache may not keep
//! its end under the fork's command line (once after it is built, once after an install, once its cache may have let it
//! go), pinged as a waiting session is while its architect has a work running, never past the most pings of its wait;
//! a fork holds the pings its base was sent before it was made, and a fork that misses the cache after a warm ping is
//! compared with the ping's call.

use db::Db;
use db::rusqlite::params;
use serde_json::{Value, json};
use sessions::choose::On;
use sessions::round::{RoundId, of_cli_session};

const HOUR: i64 = 60 * 60_000;
const MINUTE: i64 = 60_000;
const SONNET: &str = "claude-sonnet-5-5";
/// When the round that built the base ended.
const BUILT: i64 = 1_000_000_000;

/// A call as a round's record keeps one, on `model`, with the schema the command line gives kept as `schema`: the
/// base's own, which resumes it, or a fork's, which forks it with hooks of its own.
fn call(model: &str, schema: &str, fork: bool) -> Value {
    let mut command = vec![
        json!("claude"),
        json!("-p"),
        json!("--model"),
        json!(model),
        json!("--json-schema"),
        json!({ "content": schema }),
        json!("--effort"),
        json!("high"),
    ];
    if fork {
        command.extend([
            json!("--settings"),
            json!("{\"hooks\":{}}"),
            json!("--resume"),
            json!("base"),
            json!("--fork-session"),
            json!("--session-id"),
            json!("fork"),
        ]);
    } else {
        command.extend([json!("--resume"), json!("base")]);
    }
    json!({ "command": command, "env": {}, "cwd": "/w", "engine": "0.1.0", "cli": "2.1.3" })
}

/// What the CLI reports a round or a ping used: what its first call read from the cache, the session's cost so far, and
/// a last call whose prompt is what it read and wrote.
fn used(read: i64, session_cost: f64) -> Value {
    json!({ "cost_usd": 0.1, "session_cost_usd": session_cost,
        "first_call": { "input_tokens": 3, "cache_read_input_tokens": read, "cache_creation_input_tokens": 500 },
        "usage": { "input_tokens": 3, "cache_read_input_tokens": read, "cache_creation_input_tokens": 500,
            "output_tokens": 50,
            "iterations": [{ "input_tokens": 3, "cache_read_input_tokens": read, "cache_creation_input_tokens": 500 }] } })
}

/// A database holding one architect work whose base, the CLI session `base`, was built by one round on `model` under
/// the schema `schema`, which ended at [`BUILT`] holding 100,503 tokens, and a fork of it, `fork`, with no round yet.
fn base_db(model: &str, schema: &str) -> Db {
    let mut db = Db::in_memory().unwrap();
    let input = json!({ "call": call(model, schema, false) });
    db.write(db::Actor::Engine, |tx| {
        tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
        tx.execute(
            "INSERT INTO works (flow_version, graph, at, project) VALUES (1, 'architect', 'H1', 'p')",
            [],
        )?;
        tx.execute(
            "INSERT INTO sessions (work, line, cli, cli_session, started) VALUES (1, 'architect', 'claude', 'base', 'cell')",
            [],
        )?;
        tx.execute(
            "INSERT INTO sessions (work, line, cli, cli_session, started, fork_of)
             VALUES (1, 'matter', 'claude', 'fork', 'fork', 1)",
            [],
        )?;
        tx.execute(
            "INSERT INTO rounds (work, n, cell, session, input, config_commit, started_at, ended_at, usage)
             VALUES (1, 1, 'N1', 1, ?1, 'c', ?2, ?3, ?4)",
            params![
                input.to_string(),
                BUILT - MINUTE,
                BUILT,
                used(100_000, 1.0).to_string()
            ],
        )?;
        Ok(())
    })
    .unwrap();
    db
}

/// A ping of the base that ended at `ended`, with what it used (none for one that failed) and the call it ran; gives its
/// row.
fn ping(db: &mut Db, ended: i64, usage: Option<Value>, call: Option<Value>) -> i64 {
    db.write(db::Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO pings (session, waited, usage, started_at, ended_at, call) VALUES (1, '{}', ?1, ?2, ?3, ?4)",
            params![
                usage.map(|u| u.to_string()),
                ended - 1_000,
                ended,
                call.map(|c| c.to_string())
            ],
        )?;
        Ok(tx.last_insert_rowid())
    })
    .unwrap()
}

/// A warm ping of the base under the fork's command line with the schema `schema`, which read it, ended at `ended`.
fn warm_ping(db: &mut Db, ended: i64, schema: &str, session_cost: f64) -> i64 {
    ping(
        db,
        ended,
        Some(used(100_500, session_cost)),
        Some(call(SONNET, schema, false)),
    )
}

/// The fork's first round, started at `started`, ending a minute later with what it used.
fn fork_round(db: &mut Db, started: i64, usage: &Value) {
    let input = json!({ "call": call(SONNET, "s1", true) });
    db.write(db::Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO rounds (work, n, cell, session, input, config_commit, started_at, ended_at, usage)
             VALUES (1, 2, 'H1', 2, ?1, 'c', ?2, ?3, ?4)",
            params![input.to_string(), started, started + MINUTE, usage.to_string()],
        )?;
        Ok(())
    })
    .unwrap();
}

fn on(model: &str) -> On<'_> {
    On {
        model,
        home: None,
        cache: Some(HOUR),
    }
}

/// Whether the base is warmed at `now` before a fork whose call has the schema `schema`, and how many pings its wait had.
fn warmed(db: &Db, schema: &str, now: i64) -> Option<usize> {
    sessions::warm::before_fork(db.conn(), (1, "base"), &call(SONNET, schema, true), on(SONNET), now)
        .unwrap()
        .map(|d| d.pings)
}

// The base's last round, which built it, ends its call before the return it handed in: the first fork after it warms
// it, under the fork's command line, and a fork after that ping reads it, until the ping is 55 minutes old, when the next
// fork warms it again.
#[test]
fn a_base_is_warmed_once_after_it_is_built_and_once_its_ping_is_old() {
    let mut db = base_db(SONNET, "s1");
    assert_eq!(warmed(&db, "s1", BUILT + MINUTE), Some(0));
    let pinged = BUILT + 2 * MINUTE;
    warm_ping(&mut db, pinged, "s1", 1.1);
    assert_eq!(warmed(&db, "s1", pinged + MINUTE), None);
    assert_eq!(warmed(&db, "s1", pinged + 55 * MINUTE - 1), None);
    assert_eq!(warmed(&db, "s1", pinged + 55 * MINUTE), Some(1));
    assert_eq!(warmed(&db, "s1", pinged + 3 * HOUR), Some(1));
}

// After an install changed what comes before the messages (here the schema the command line gives), the base's last
// ping is no prefix of a fork's prompt: the next fork warms it again under its own.
#[test]
fn a_base_is_warmed_again_after_an_install_changed_what_comes_before_its_messages() {
    let mut db = base_db(SONNET, "s1");
    warm_ping(&mut db, BUILT + 2 * MINUTE, "s1", 1.1);
    assert_eq!(warmed(&db, "s1", BUILT + 3 * MINUTE), None);
    assert_eq!(warmed(&db, "s2", BUILT + 3 * MINUTE), Some(1));
}

// A ping that failed is tried again a minute after it, not at once; none is sent for a base on another model than the
// fork's, when the CLI has no prompt cache, for a model with no prices, or past the most pings of its wait.
#[test]
fn a_base_is_not_warmed_where_a_ping_is_worth_nothing() {
    let mut db = base_db(SONNET, "s1");
    let failed = BUILT + 2 * MINUTE;
    ping(&mut db, failed, None, Some(call(SONNET, "s1", false)));
    assert_eq!(warmed(&db, "s1", failed + 30_000), None);
    assert_eq!(warmed(&db, "s1", failed + MINUTE), Some(1));
    let fork = call(SONNET, "s1", true);
    let at = BUILT + 5 * MINUTE;
    let other = sessions::warm::before_fork(db.conn(), (1, "base"), &fork, on("claude-opus-5-5"), at).unwrap();
    assert!(other.is_none());
    let uncached = On {
        cache: None,
        ..on(SONNET)
    };
    assert!(
        sessions::warm::before_fork(db.conn(), (1, "base"), &fork, uncached, at)
            .unwrap()
            .is_none()
    );
    let unpriced = base_db("m", "s1");
    let some = sessions::warm::before_fork(unpriced.conn(), (1, "base"), &call("m", "s1", true), on("m"), at).unwrap();
    assert!(some.is_none());
    // Thirty-nine pings since the base's last round, under another command line, are as many as one write of it is
    // worth on Sonnet 5.5.
    let mut full = base_db(SONNET, "s1");
    for i in 0..39 {
        warm_ping(&mut full, BUILT + (i + 1) * MINUTE, "s0", 1.0);
    }
    assert_eq!(warmed(&full, "s1", BUILT + HOUR), None);
}

// While its architect has a work running, the base is pinged as a waiting session is: from 55 minutes after its last
// call, a round's or a ping's, until the cache lets it go.
#[test]
fn a_base_is_due_its_ping_55_minutes_after_its_last_call() {
    let mut db = base_db(SONNET, "s1");
    let due = |db: &Db, now: i64| {
        sessions::warm::base_due(db.conn(), (1, "base"), on(SONNET), now)
            .unwrap()
            .map(|d| d.pings)
    };
    assert_eq!(due(&db, BUILT + 55 * MINUTE - 1), None);
    assert_eq!(due(&db, BUILT + 55 * MINUTE), Some(0));
    assert_eq!(due(&db, BUILT + HOUR), None);
    let pinged = BUILT + 56 * MINUTE;
    warm_ping(&mut db, pinged, "s1", 1.1);
    assert_eq!(due(&db, pinged + 55 * MINUTE - 1), None);
    assert_eq!(due(&db, pinged + 55 * MINUTE), Some(1));
    let other = sessions::warm::base_due(db.conn(), (1, "base"), on("claude-opus-5-5"), pinged + 55 * MINUTE).unwrap();
    assert!(other.is_none());
}

// A fork holds the pings its base was sent before its first round, as it holds the base's rounds from before then: they
// are its last call, and what its command line counts it had cost; a ping of the base after the fork was made is not
// the fork's.
#[test]
fn a_fork_holds_the_pings_its_base_was_sent_before_it_was_made() {
    let mut db = base_db(SONNET, "s1");
    let warm = warm_ping(&mut db, BUILT + 10 * MINUTE, "s1", 1.25);
    let (base_rounds, _) = sessions::held::of_session(db.conn(), "base").unwrap();
    assert_eq!(
        sessions::warm::last_call(db.conn(), "fork", &base_rounds).unwrap(),
        Some(BUILT + 10 * MINUTE)
    );
    assert_eq!(
        sessions::cost::spent(db.conn(), "fork", &base_rounds).unwrap(),
        Some(1.25)
    );
    fork_round(&mut db, BUILT + 11 * MINUTE, &used(100_500, 2.0));
    warm_ping(&mut db, BUILT + 20 * MINUTE, "s1", 1.5);
    let held: Vec<i64> = sessions::warm::held_pings(db.conn(), "fork")
        .unwrap()
        .iter()
        .map(|p| p.id)
        .collect();
    assert_eq!(held, [warm]);
    assert_eq!(sessions::warm::held_pings(db.conn(), "base").unwrap().len(), 2);
    let rounds = of_cli_session(db.conn(), "fork").unwrap();
    assert_eq!(
        sessions::warm::last_call(db.conn(), "fork", &rounds).unwrap(),
        Some(BUILT + 12 * MINUTE)
    );
    assert_eq!(sessions::cost::spent(db.conn(), "fork", &rounds).unwrap(), Some(2.0));
}

// A fork that reads little of its base after the ping that warmed it under the fork's command line missed the cache,
// and is compared with the ping's call, not with the base's last round, which ran under another schema before an
// install: what comes before the messages is the same, so the miss is a fault. Compared with the round, as before pings
// recorded their call, the changed schema would have passed it off as expected.
#[test]
fn a_fork_that_misses_after_a_warm_ping_is_compared_with_the_pings_call() {
    let miss_of = |pinged: bool| {
        let mut db = base_db(SONNET, "s0");
        let warm = pinged.then(|| warm_ping(&mut db, BUILT + 10 * MINUTE, "s1", 1.25));
        let usage = used(33_881, 2.0);
        fork_round(&mut db, BUILT + 11 * MINUTE, &usage);
        let (base_rounds, _) = sessions::held::of_session(db.conn(), "base").unwrap();
        let m = sessions::miss::missed(
            db.conn(),
            "fork",
            &base_rounds,
            RoundId::new(1, 2),
            &call(SONNET, "s1", true),
            &usage,
            Some(SONNET),
            HOUR,
        )
        .unwrap()
        .unwrap();
        (m, warm)
    };
    let (m, warm) = miss_of(true);
    assert_eq!((m.before, m.ping), (RoundId::new(1, 1), warm));
    assert_eq!((m.read, m.held, m.after_ms), (33_881, 100_503, MINUTE));
    assert!(!m.expected(), "{:?}", m.differs);
    let parts: Vec<&str> = m.differs.iter().map(|d| d.part.as_str()).collect();
    assert_eq!(parts, ["--settings", "--fork-session"]);
    let (m, _) = miss_of(false);
    assert_eq!(m.ping, None);
    assert!(m.expected(), "{:?}", m.differs);
}
