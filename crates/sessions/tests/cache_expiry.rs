//! The edges of the expiry of a session with the CLI's prompt cache, through the chooser and the keep-warm module: the
//! last call a session's cache holds may be a keep-warm ping of an earlier work, the next ping is due a lifetime less
//! five minutes after a ping that read the cache, and a return sent back is rewritten in its session whatever its age.

use db::rusqlite::params;
use flow::CompiledCell;
use serde_json::json;
use sessions::choose::{On, Plan, Start, plan};

const HOUR: i64 = 60 * 60_000;
const SONNET: &str = "claude-sonnet-5-5";
const RAN: &str = r#"{"cost_usd": 0.01, "usage": {"iterations": [
    {"input_tokens": 10, "cache_read_input_tokens": 1000, "cache_creation_input_tokens": 0}]}}"#;

/// A database holding one architect work with one CLI session, `cs1`, and no round yet.
fn session_db() -> db::Db {
    let mut db = db::Db::in_memory().unwrap();
    db.write(db::Actor::Engine, |tx| {
        tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
        tx.execute(
            "INSERT INTO works (flow_version, graph, at, project) VALUES (1, 'architect', 'B2', 'p')",
            [],
        )?;
        tx.execute(
            "INSERT INTO sessions (work, line, cli, cli_session) VALUES (1, 'architect', 'claude', 'cs1')",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    db
}

/// A round of that session that ended at `ended` and ran the CLI, on the model `SONNET`, with `RAN` as its usage.
fn session_round(db: &mut db::Db, n: i64, ended: i64) {
    let input = json!({ "call": { "command": ["claude", "-p", "--model", SONNET] } });
    db.write(db::Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO rounds (work, n, cell, session, input, config_commit, started_at, ended_at, usage)
             VALUES (1, ?1, 'B2', 1, ?2, 'c', ?3, ?4, ?5)",
            params![n, input.to_string(), ended - 1_000, ended, RAN],
        )?;
        Ok(())
    })
    .unwrap();
}

/// A keep-warm ping of that session that ended at `ended`, having read the cache.
fn session_ping(db: &mut db::Db, ended: i64) {
    db.write(db::Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO pings (session, waited, usage, started_at, ended_at) VALUES (1, '{}', ?1, ?2, ?3)",
            params![RAN, ended - 1_000, ended],
        )?;
        Ok(())
    })
    .unwrap();
}

/// The architect's cell B2 of the flow as it is, as on a line that expires with the cache: the base does not, since a
/// cold cache is written once, cheaper than reading the project again, and the expiry is the chooser's for any line
/// that says so.
fn architect_cell() -> CompiledCell {
    let relations = db::relations(db::SCHEMA).unwrap();
    let tables = db::index_of(db::SCHEMA).unwrap();
    let f = flow::compile(flow::FLOW, &flow::Context { relations, tables }).unwrap();
    let mut cell = f.graph("architect").unwrap().cell("B2").unwrap().clone();
    assert!(
        !cell.cache_expiry,
        "the architect's base does not expire with the cache"
    );
    cell.cache_expiry = true;
    cell
}

fn on() -> On<'static> {
    On {
        model: SONNET,
        home: None,
        cache: Some(HOUR),
    }
}

fn goes_on(p: &Plan) -> bool {
    matches!(p, Plan::Going { .. })
}

fn expired(p: &Plan) -> bool {
    matches!(p, Plan::New(Start::Expired))
}

// A return sent back is rewritten in the session that wrote it, whatever the age of the session's cache: the expiry
// takes effect at the round after the rewrite. A round that failed or was cut off is no return sent back, and its
// session is let go as any other.
#[test]
fn a_return_sent_back_is_rewritten_in_its_session_though_its_cache_let_go() {
    let mut db = session_db();
    session_round(&mut db, 1, 1_000_000);
    let cell = architect_cell();
    let later = 1_000_000 + 3 * HOUR;
    let rejected = |db: &mut db::Db, why: &str| {
        let why = why.to_string();
        db.write(db::Actor::Engine, |tx| {
            tx.execute("UPDATE rounds SET rejected = ?1 WHERE work = 1 AND n = 1", [why])?;
            Ok(())
        })
        .unwrap();
    };
    let at = |db: &db::Db| plan(db.conn(), 1, &cell, 500_000, on(), later, None).unwrap();
    assert!(expired(&at(&db)));
    rejected(&mut db, r#"{"unparsed": "the return is not an object"}"#);
    assert!(goes_on(&at(&db)));
    rejected(&mut db, r#"{"failed": "the CLI stopped"}"#);
    assert!(expired(&at(&db)));
}

// A session is taken up by the project's next architect work with the cache of its last call, which may be a
// keep-warm ping made while the previous work had the session: that work's rows and the next work's have the CLI
// session in common, and the call is the one on any of them.
#[test]
fn a_ping_made_in_the_previous_work_keeps_the_session_the_next_work_takes_up() {
    let mut db = session_db();
    session_round(&mut db, 1, 1_000_000);
    session_ping(&mut db, 1_000_000 + 50 * 60_000);
    db.write(db::Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO works (flow_version, graph, at, project) VALUES (1, 'architect', 'S1', 'p')",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    let cell = architect_cell();
    let next = |now: i64| plan(db.conn(), 2, &cell, 500_000, on(), now, None).unwrap();
    // One millisecond short of an hour after the ping, 109 minutes after the round: the session is taken up.
    assert!(matches!(
        next(1_000_000 + 110 * 60_000 - 1),
        Plan::Going { carried: true, .. }
    ));
    assert!(expired(&next(1_000_000 + 110 * 60_000)));
}

// A ping that read the cache moves the next ping and the cache's expiry a lifetime on from it: the next ping is due 55
// minutes after it, and the session goes on until an hour after it.
#[test]
fn a_ping_that_read_the_cache_moves_the_next_ping_and_the_expiry_on_from_itself() {
    let cell = architect_cell();
    let mut db = session_db();
    session_round(&mut db, 1, 1_000_000);
    let pinged = 1_000_000 + 56 * 60_000;
    session_ping(&mut db, pinged);
    let due_at = |now: i64| {
        sessions::warm::due(db.conn(), 1, &[&cell], 500_000, on(), now)
            .unwrap()
            .map(|d| d.pings)
    };
    assert_eq!(due_at(pinged + 55 * 60_000 - 1), None);
    assert_eq!(due_at(pinged + 55 * 60_000), Some(1));
    assert_eq!(due_at(pinged + HOUR - 1), Some(1));
    assert_eq!(due_at(pinged + HOUR), None);
    let at = |now: i64| plan(db.conn(), 1, &cell, 500_000, on(), now, None).unwrap();
    assert!(goes_on(&at(pinged + HOUR - 1)));
    assert!(expired(&at(pinged + HOUR)));
}
