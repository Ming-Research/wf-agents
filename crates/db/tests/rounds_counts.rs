//! What the change that keeps a round's counter on the round (`rounds.counts`) must leave as it was, and how the
//! counters views are read as the engine reads them. Kept apart from the migration tests in `src/lib.rs`, which reach
//! the migrations by the code's own helpers, so that these see the database through its public interface alone.

use std::path::{Path, PathBuf};

use db::rusqlite::types::Value;
use db::rusqlite::{Connection, params_from_iter};
use db::{Db, MIGRATIONS};

/// A database file at version 67, the one before the version that adds `rounds.counts`: made by the first version's
/// schema and the migrations up to 67, with the foreign keys off as the migrations run.
fn database_at_version_67(path: &Path) -> Connection {
    let conn = Connection::open(path).unwrap();
    conn.pragma_update(None, "foreign_keys", false).unwrap();
    conn.execute_batch(include_str!("../../../db/migrations/0001.sql"))
        .unwrap();
    for migration in &MIGRATIONS[..66] {
        conn.execute_batch(migration).unwrap();
    }
    conn.pragma_update(None, "user_version", 67).unwrap();
    conn
}

/// Every row of a query, each value as it is stored.
fn rows(conn: &Connection, sql: &str) -> Vec<Vec<Value>> {
    let mut st = conn.prepare(sql).unwrap();
    let width = st.column_count();
    st.query_map([], |r| (0..width).map(|i| r.get::<_, Value>(i)).collect())
        .unwrap()
        .map(|x| x.unwrap())
        .collect()
}

/// The columns a round has had since before version 68, in its rows' own order.
const ROUND_COLUMNS: &str = "rowid, work, n, cell, session, input, output, transition, rejected, config_commit, model, \
                             usage, compactions, started_at, ended_at";

// A database at version 67 with rounds that fill every column, written in an order the rounds' key does not follow,
// and rows of other tables that point at them, is migrated to now: every round has every column it had, in the order
// it had, the rows that point at rounds still do, and the database holds together. The migration makes the table of
// rounds afresh, and which column goes to which, and whether the order and the references are kept, is not what its
// counters say. The reference is the database as it stood, read before the migration. It passes before the change
// too, as the database is then at the version it has.
#[test]
fn the_migration_keeps_every_round_and_what_points_at_it() {
    let dir = temp("keeps-rounds");
    let path = dir.join("wf.db");
    let before_rounds;
    let before_children;
    {
        let conn = database_at_version_67(&path);
        conn.execute_batch(
            r#"INSERT INTO flow_versions (id, digest, compiled) VALUES
                 (1, 'd', '{"graphs": [{"name": "engineer", "cells": [{"id": "A4", "rows": [{"name": "ask", "counts": "asks"}]}]}]}');
               INSERT INTO works (id, flow_version, graph, at) VALUES (1, 1, 'engineer', 'A4'), (2, 1, 'engineer', 'A4');
               INSERT INTO sessions (id, work, line, cli, cli_session) VALUES (1, 1, 'engineer', 'c', 's');
               -- Written in the order (2,1), (1,1), (1,3), (1,2): not that of the key.
               INSERT INTO rounds (work, n, cell, session, input, output, transition, rejected, config_commit, model, usage,
                                   compactions, started_at, ended_at) VALUES
                 (2, 1, 'A4', NULL, '{"a": 1}', '{"outcome": {}}', 'ask', NULL, 'c1', NULL, NULL, 0, 10, 20),
                 (1, 1, 'A4', 1, '{"b": "x"}', '{"outcome": {"ask": 1}}', 'ask', NULL, 'c2', 'm', '{"cost_usd": 0.5}', 2, 30, 40),
                 (1, 3, 'A4', 1, '{}', NULL, NULL, '{"failed": {"kind": "k", "what": "w"}}', 'c3', 'm', NULL, 1, 60, 70),
                 (1, 2, 'ZZ', NULL, '{}', '{}', 'none', NULL, 'c4', NULL, NULL, 0, 45, NULL);
               INSERT INTO owner_asks (work, round, body) VALUES (1, 2, '{"q": 1}'), (2, 1, '{"q": 2}');
               INSERT INTO owner_messages (work, round, text) VALUES (1, 1, 'hello');
               INSERT INTO tool_runs (work, round, tool, args, attempt, queued_at) VALUES (1, 3, 't', '{}', 1, 5);"#,
        )
        .unwrap();
        before_rounds = rows(&conn, &format!("SELECT {ROUND_COLUMNS} FROM rounds ORDER BY rowid"));
        before_children = rows(
            &conn,
            "SELECT 'a', work, round, body FROM owner_asks UNION ALL SELECT 'm', work, round, text FROM owner_messages
             UNION ALL SELECT 't', work, round, args FROM tool_runs ORDER BY 1, 2, 3",
        );
    }
    assert_eq!(before_rounds.len(), 4);
    let db = Db::open(&path).unwrap();
    let conn = db.conn();
    assert_eq!(
        rows(conn, &format!("SELECT {ROUND_COLUMNS} FROM rounds ORDER BY rowid")),
        before_rounds
    );
    assert_eq!(
        rows(
            conn,
            "SELECT 'a', work, round, body FROM owner_asks UNION ALL SELECT 'm', work, round, text FROM owner_messages
             UNION ALL SELECT 't', work, round, args FROM tool_runs ORDER BY 1, 2, 3",
        ),
        before_children
    );
    assert!(rows(conn, "SELECT * FROM pragma_foreign_key_check").is_empty());
    assert_eq!(
        rows(conn, "SELECT * FROM pragma_integrity_check"),
        [[Value::Text("ok".into())]]
    );
    // The foreign keys are on again for the connection the migration ran on.
    assert_eq!(rows(conn, "PRAGMA foreign_keys"), [[Value::Integer(1)]]);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A path under the temporary directory, made empty.
fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("wf-agents-db-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

// The engine reads the counters views for one work with the work and the counter as bound parameters, not as the
// literals a person types: the plans of those reads search the index on counted rounds and the work's key, parse no
// JSON and scan no table. The plan with a literal is checked by the tests of the migration; a plan can differ once the
// value is a parameter, and these are the reads the engine's passes make for each work. Before the change the views
// parsed the compiled flow for every round and `work_asking` scanned every round.
#[test]
fn the_counters_the_engine_reads_for_one_work_are_not_scans() {
    let db = Db::in_memory().unwrap();
    let work = Value::Integer(7);
    let counter = Value::Text("asks".into());
    let reads: [(&str, Vec<&Value>); 3] = [
        ("SELECT asks FROM work_asking WHERE work = ?1", vec![&work]),
        ("SELECT counter, times FROM work_counters WHERE work = ?1", vec![&work]),
        (
            "SELECT coalesce(max(times), 0) FROM work_counters WHERE work = ?1 AND counter = ?2",
            vec![&work, &counter],
        ),
    ];
    for (sql, bound) in reads {
        let mut st = db.conn().prepare(&format!("EXPLAIN QUERY PLAN {sql}")).unwrap();
        let steps: Vec<String> = st
            .query_map(params_from_iter(bound), |r| r.get::<_, String>(3))
            .unwrap()
            .map(|x| x.unwrap())
            .collect();
        let plan = steps.join("\n");
        assert!(
            !plan.contains("json_each") && !plan.contains("VIRTUAL TABLE"),
            "{sql} parses JSON:\n{plan}"
        );
        assert!(
            !steps.iter().any(|s| s.starts_with("SCAN")),
            "{sql} scans a table:\n{plan}"
        );
        assert!(
            steps
                .iter()
                .any(|s| s.contains("rounds_counts") || s.contains("autoindex_rounds")),
            "{sql} does not read rounds by an index:\n{plan}"
        );
    }
}
