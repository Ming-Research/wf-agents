//! What the run guard's writes tell the owner's page.

use std::sync::{Arc, Mutex};

use api::Event;
use db::rusqlite::Connection;
use db::{Actor, Changed, Db};

// What the guard did goes to the page as the round's event, so that its anomalies are read again; each call of the
// ledger makes none, since the event stream carries a running round's numbers itself and a page reading the whole
// System view at every call would cost more than the call told.
#[test]
fn what_the_guard_did_is_told_and_a_call_kept_is_not() {
    let told: Arc<Mutex<Vec<Event>>> = Arc::default();
    let t = Arc::clone(&told);
    let mut db = Db::in_memory().unwrap();
    db.write(Actor::Engine, |tx| {
        tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
        tx.execute(
            "INSERT INTO works (flow_version, graph, at) VALUES (1, 'engineer', 'I1')",
            [],
        )?;
        tx.execute(
            "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES (1, 4, 'I1', '{}', 'c', 1)",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    db.on_commit(Arc::new(move |conn: &Connection, rows: &[Changed]| {
        t.lock().unwrap().extend(api::events(conn, rows));
    }))
    .unwrap();
    db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO round_calls (work, round, seq, message, input, read, written_5m, written_1h, output, subagent, at)
             VALUES (1, 4, 1, 'a', 1, 1, 0, 0, 1, 0, 1)",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    assert!(told.lock().unwrap().is_empty());
    db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO guard_events (work, round, signal, value, line, samples, scope, action, at)
             VALUES (1, 4, 'calls', 2, 1, 19, 'cell', 'noted', 1)",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    assert_eq!(*told.lock().unwrap(), [Event::Round { work: 1, round: 4 }]);
}
