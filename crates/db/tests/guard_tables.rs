//! What the run guard keeps, as agents and the record of changes see it.

use std::time::Duration;

use db::{Actor, Db};

// The design holds that the ledger is written far oftener than any state table, so it stays out of `changes`, and that
// agents read neither it nor what lines are computed from; what the guard did is theirs to trace, as the schema says.
#[test]
fn what_the_guard_keeps_is_hidden_from_agents_and_is_no_change_of_state() {
    let dir = std::env::temp_dir().join(format!("wf-agents-guard-tables-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("db");
    let mut db = Db::create(&path).unwrap();
    db.write(Actor::Engine, |tx| {
        tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
        tx.execute(
            "INSERT INTO works (flow_version, graph, at) VALUES (1, 'engineer', 'I1')",
            [],
        )?;
        tx.execute(
            "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES (1, 1, 'I1', '{}', 'c', 1)",
            [],
        )?;
        tx.execute(
            "INSERT INTO round_calls (work, round, seq, message, input, read, written_5m, written_1h, output, subagent, at)
             VALUES (1, 1, 1, 'a', 1, 1, 0, 0, 1, 0, 1)",
            [],
        )?;
        tx.execute(
            "INSERT INTO round_measures (work, round, graph, cell, calls, added, compactions, ledger)
             VALUES (1, 1, 'engineer', 'I1', 1, 0, 0, 1)",
            [],
        )?;
        tx.execute(
            "INSERT INTO guard_events (work, round, signal, value, line, samples, scope, action, at)
             VALUES (1, 1, 'calls', 2, 1, 19, 'cell', 'noted', 1)",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    let tracked: i64 = db
        .conn()
        .query_row(
            "SELECT count(*) FROM changes WHERE table_name IN ('round_calls', 'round_measures', 'guard_events')",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let agents = db::open_for_agents(&path, Duration::from_secs(5)).unwrap();
    let rows = |table: &str| -> Result<i64, _> {
        agents.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
    };
    let (calls, measures, events) = (rows("round_calls"), rows("round_measures"), rows("guard_events"));
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(tracked, 0);
    assert!(calls.is_err(), "{calls:?}");
    assert!(measures.is_err(), "{measures:?}");
    assert_eq!(events.ok(), Some(1));
}
