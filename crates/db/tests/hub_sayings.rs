//! The sayings to the owner and to the flow's keeper as the hub keeps them: what an agent told the owner and what the
//! engine told of its own accord (`owner_messages` and `engine_notices` before) are each a message, and whom it went to,
//! and whether it was read, a delivery of it. The migration that brings them in carries every one over as it stood.

use std::path::{Path, PathBuf};

use db::rusqlite::Connection;
use db::rusqlite::types::Value;
use db::{Db, MIGRATIONS};

const MIGRATION: &str = "202611070554-sayings-to-the-owner-and-the-keeper-in-one-table";

/// A path under the temporary directory, made empty.
fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("wf-agents-db-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A database file just before the migration, made by the first version's schema and the migrations up to it, with the
/// names of the migrations taken kept where the database keeps them.
fn database_before_migration(path: &Path) -> Connection {
    let at = MIGRATIONS.iter().position(|m| m.name == MIGRATION).unwrap();
    let conn = Connection::open(path).unwrap();
    conn.pragma_update(None, "foreign_keys", false).unwrap();
    conn.execute_batch(include_str!("../../../db/migrations/0001.sql"))
        .unwrap();
    for migration in &MIGRATIONS[..at] {
        conn.execute_batch(migration.sql).unwrap();
    }
    for migration in &MIGRATIONS[..at] {
        conn.execute("INSERT OR IGNORE INTO migrations (name) VALUES (?1)", [migration.name])
            .unwrap();
    }
    conn.pragma_update(None, "user_version", at as i64 + 1).unwrap();
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

fn text(s: &str) -> Value {
    Value::Text(s.into())
}

// A database with what agents told the owner, read and unread, the engine's notices of a work, of a new ask and of a
// cache miss forwarded to an architect by the setting that did so, a reminder, and that setting, takes the migration:
// each saying is a message, in the order sent, an agent's dated at the end of the round that handed it in (its start
// while it runs), and delivered to the owner, the fault to the flow's keeper and, forwarded, to the architect as the
// matter it came as, each still read or unread as it was; the cache miss names its message; the reminder is kept as
// one, no message; and the setting is gone.
#[test]
fn the_migration_carries_every_saying_over_as_it_stood() {
    let dir = temp("hub-sayings");
    let path = dir.join("db");
    {
        let conn = database_before_migration(&path);
        conn.execute_batch(
            r#"INSERT INTO items (id, project, title, problem, why, deliver, level)
                 VALUES (1, 'p', 't', 'x', 'y', 'change', '1');
               INSERT INTO flow_versions (id, digest, compiled) VALUES (1, 'd', '{}');
               INSERT INTO works (id, item, flow_version, graph, at) VALUES (1, 1, 1, 'engineer', 'R1');
               INSERT INTO works (id, flow_version, graph, at, project) VALUES (2, 1, 'architect', 'H1', 'p');
               INSERT INTO rounds (work, n, cell, input, config_commit, started_at, ended_at)
                 VALUES (1, 1, 'R1', '{}', 'c', 5, 20), (2, 1, 'H1', '{}', 'c', 30, NULL);
               INSERT INTO owner_messages (work, round, text, read_at) VALUES (1, 1, 'Look at the log.', NULL),
                 (2, 1, 'Done.', 50);
               INSERT INTO owner_asks (id, work, round, body) VALUES (1, 1, 1, '{}');
               INSERT INTO engine_notices (id, work, text, at, read_at, ask, fault, reminder) VALUES
                 (1, 1, 'Work 1 stopped.', 10, NULL, NULL, NULL, 0),
                 (2, NULL, 'Ask 1 waits for your answer.', 15, NULL, NULL, NULL, 1),
                 (3, 1, 'R1-1 missed its cache.', 25, 26, NULL, 'cache_miss', 0),
                 (4, 1, 'The engineer asks you.', 27, NULL, 1, NULL, 0);
               INSERT INTO architect_matters (id, project, kind, detail, what, body, notice, at)
                 VALUES (1, 'wf', 'report', 'fault', 'Look into it', '{"by": "setting"}', 3, 26);
               INSERT INTO cache_misses (work, round, line, differs, written, held, cost_usd, notice)
                 VALUES (1, 1, 'engineer', '[]', 1, 2, NULL, 3);
               INSERT INTO owner_choices (name, value) VALUES ('forward_faults', 'wf');"#,
        )
        .unwrap();
    }
    let db = Db::open(&path).unwrap();
    let conn = db.conn();
    assert_eq!(
        rows(
            conn,
            "SELECT m.id, m.sender, m.work, m.round, m.text, m.body, m.ask, m.at, d.recipient, d.read_at
             FROM messages m JOIN deliveries d ON d.message = m.id AND d.recipient <> 'architect' ORDER BY m.id"
        ),
        [
            vec![
                Value::Integer(1),
                text("engine"),
                Value::Integer(1),
                Value::Null,
                text("Work 1 stopped."),
                Value::Null,
                Value::Null,
                Value::Integer(10),
                text("owner"),
                Value::Null
            ],
            vec![
                Value::Integer(2),
                text("engineer"),
                Value::Integer(1),
                Value::Integer(1),
                text("Look at the log."),
                Value::Null,
                Value::Null,
                Value::Integer(20),
                text("owner"),
                Value::Null
            ],
            vec![
                Value::Integer(3),
                text("engine"),
                Value::Integer(1),
                Value::Null,
                text("R1-1 missed its cache."),
                text(r#"{"kind":"cache_miss"}"#),
                Value::Null,
                Value::Integer(25),
                text("keeper"),
                Value::Integer(26)
            ],
            vec![
                Value::Integer(4),
                text("engine"),
                Value::Integer(1),
                Value::Null,
                text("The engineer asks you."),
                Value::Null,
                // The ask, by the number it took among the hub's messages once asks became messages.
                Value::Integer(6),
                Value::Integer(27),
                text("owner"),
                Value::Null
            ],
            vec![
                Value::Integer(5),
                text("architect"),
                Value::Integer(2),
                Value::Integer(1),
                text("Done."),
                Value::Null,
                Value::Null,
                Value::Integer(30),
                text("owner"),
                Value::Integer(50)
            ],
        ]
    );
    // The fault forwarded is delivered to the architect as its matter, which keeps who forwarded it; its miss names it.
    assert_eq!(
        rows(
            conn,
            "SELECT d.message, d.matter, json_extract(m.body, '$.by') FROM deliveries d
             JOIN architect_matters m ON m.id = d.matter WHERE d.recipient = 'architect'"
        ),
        [vec![Value::Integer(3), Value::Integer(1), text("setting")]]
    );
    assert_eq!(
        rows(conn, "SELECT message FROM cache_misses"),
        [vec![Value::Integer(3)]]
    );
    assert_eq!(
        rows(conn, "SELECT at, text FROM reminders"),
        [vec![Value::Integer(15), text("Ask 1 waits for your answer.")]]
    );
    assert!(
        rows(
            conn,
            "SELECT name FROM sqlite_schema WHERE name IN ('owner_messages', 'engine_notices', 'owner_choices')"
        )
        .is_empty()
    );
    // The owner's conversations read them as before: the work's message unread in its group, the fault read in
    // wf-agents' own, the notice of the ask in none.
    assert_eq!(
        rows(
            conn,
            "SELECT conversation, source, id, read_at, fault FROM conversation_entries
             WHERE source IN ('message', 'notice') ORDER BY id"
        ),
        [
            vec![
                text("work:1"),
                text("notice"),
                Value::Integer(1),
                Value::Null,
                Value::Null
            ],
            vec![
                text("work:1"),
                text("message"),
                Value::Integer(2),
                Value::Null,
                Value::Null
            ],
            vec![
                text("wf-agents"),
                text("notice"),
                Value::Integer(3),
                Value::Integer(26),
                text("cache_miss")
            ],
            vec![
                text("architect:p"),
                text("message"),
                Value::Integer(5),
                Value::Integer(50),
                Value::Null
            ],
        ]
    );
    drop(db);
    let _ = std::fs::remove_dir_all(&dir);
}
