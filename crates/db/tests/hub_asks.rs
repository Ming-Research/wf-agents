//! The asks of the owner as the hub keeps them: each ask (`owner_asks` before) is a message of the kind ask, put by its
//! work's line in the round that put it, holding what is asked and the answer once given. The migration that brings
//! them in numbers them after the hub's other messages, and whatever named an ask names it by that number.

use std::path::{Path, PathBuf};

use db::rusqlite::Connection;
use db::rusqlite::types::Value;
use db::{Db, MIGRATIONS};

const MIGRATION: &str = "202611070655-asks-of-the-owner-in-the-hub-s-table";

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

fn int(n: i64) -> Value {
    Value::Integer(n)
}

// A database with asks of every kind (an engineer's question, answered, which the engine told the owner of, holding a
// blocker and with an image to its answer; a question of the architect, open, which parks a matter; an ask of decision
// points, with no question in words) beside the hub's other messages, with the change log of them, takes the
// migration: each ask is a message of the kind ask after the others, sent by its work's line in its round, its text the
// question it holds, keeping its answer and when the owner was told of it; the engine's word of it, the matter, the
// blocker, the image and the change log name it by its number; and the views read the asks as they read them before.
#[test]
fn the_migration_makes_every_ask_a_message_and_renames_it_everywhere() {
    let dir = temp("hub-asks");
    let path = dir.join("db");
    {
        let conn = database_before_migration(&path);
        conn.execute_batch(
            r#"INSERT INTO items (id, project, title, problem, why, deliver, level)
                 VALUES (1, 'p', 't', 'x', 'y', 'change', '1');
               INSERT INTO flow_versions (id, digest, compiled) VALUES (1, 'd', '{}');
               INSERT INTO works (id, item, flow_version, graph, at) VALUES (1, 1, 1, 'engineer', 'A3');
               INSERT INTO works (id, flow_version, graph, at, project) VALUES (2, 1, 'architect', 'H8', 'p');
               INSERT INTO rounds (work, n, cell, input, config_commit, started_at, ended_at)
                 VALUES (1, 1, 'R1', '{}', 'c', 5, 20), (1, 2, 'A3', '{}', 'c', 30, 31), (2, 1, 'H8', '{}', 'c', 40, 41);
               INSERT INTO owner_asks (id, work, round, body, answer) VALUES
                 (1, 1, 2, '{"to":"owner","question":"Which one?"}',
                  '{"by":"owner","at":60,"text":"The first.","speaker":"owner"}'),
                 (2, 2, 1, '{"from":"architect","project":"p","kind":"question","question":"Research it?"}', NULL),
                 (3, 1, 2, '{"kind":"points","asked":[]}', NULL);
               INSERT INTO messages (id, sender, work, round, text, body, ask, at)
                 VALUES (1, 'engine', 1, NULL, 'Work 1 asks you.', NULL, 1, 35),
                        (2, 'owner', 1, NULL, 'Go on.', NULL, NULL, 50);
               INSERT INTO deliveries (id, message, recipient, line) VALUES (1, 1, 'owner', NULL), (2, 2, 'line', 'engineer');
               INSERT INTO architect_matters (id, project, kind, what, body, at, asked, resume_cell)
                 VALUES (1, 'p', 'open_question', 'Which?', '{}', 38, 2, 'H8');
               INSERT INTO blockers (id, work, cell, round, kind, what, holder, ask, at, since)
                 VALUES (1, 1, 'A3', 2, 'owner_ask', 'Which one?', 'owner', 1, 31, 31);
               INSERT INTO attachments (id, ask, position, media_type, size, hash, bytes)
                 VALUES (1, 1, 1, 'image/png', 4, 'h', zeroblob(4));
               INSERT INTO changes (tx, at, actor, table_name, row_key, old_row, new_row) VALUES
                 (1, 31, 'engine', 'owner_asks', '{"id":1}', NULL, '{"id":1,"answer":null}'),
                 (2, 35, 'engine', 'messages', '{"id":1}', NULL, '{"id":1,"ask":1}'),
                 (3, 38, 'engine', 'architect_matters', '{"id":1}', NULL, '{"id":1,"asked":2}'),
                 (4, 60, 'owner', 'owner_asks', '{"id":1}', '{"id":1,"answer":null}', '{"id":1,"answer":"x"}'),
                 (5, 60, 'owner', 'blockers', '{"id":1}', '{"id":1,"ask":1,"ended":null}',
                  '{"id":1,"ask":1,"ended":"resolved"}');"#,
        )
        .unwrap();
    }
    let db = Db::open(&path).unwrap();
    let conn = db.conn();
    // The asks come after the other messages, in the order they were put.
    assert_eq!(
        rows(
            conn,
            "SELECT id, sender, work, round, text, json_extract(answer, '$.text'), at FROM messages WHERE kind = 'ask'
             ORDER BY id"
        ),
        [
            vec![
                int(3),
                text("engineer"),
                int(1),
                int(2),
                text("Which one?"),
                text("The first."),
                int(35)
            ],
            vec![
                int(4),
                text("architect"),
                int(2),
                int(1),
                text("Research it?"),
                Value::Null,
                int(40)
            ],
            vec![int(5), text("engineer"), int(1), int(2), text(""), Value::Null, int(30)],
        ]
    );
    assert_eq!(
        rows(
            conn,
            "SELECT id, kind, ask FROM messages WHERE kind = 'say' ORDER BY id"
        ),
        [
            vec![int(1), text("say"), int(3)],
            vec![int(2), text("say"), Value::Null]
        ]
    );
    // An ask goes to no recipient: the owner answers it where it is.
    assert!(rows(conn, "SELECT id FROM deliveries WHERE message >= 3").is_empty());
    assert_eq!(rows(conn, "SELECT asked FROM architect_matters"), [vec![int(4)]]);
    assert_eq!(rows(conn, "SELECT ask FROM blockers"), [vec![int(3)]]);
    assert_eq!(rows(conn, "SELECT ask FROM attachments"), [vec![int(3)]]);
    // The views read the asks as they read them before.
    assert_eq!(
        rows(conn, "SELECT id, work, round FROM work_asks ORDER BY id"),
        [
            vec![int(3), int(1), int(2)],
            vec![int(4), int(2), int(1)],
            vec![int(5), int(1), int(2)]
        ]
    );
    assert_eq!(
        rows(conn, "SELECT item, ask FROM item_asks ORDER BY ask"),
        [vec![int(1), int(3)], vec![Value::Null, int(4)], vec![int(1), int(5)]]
    );
    assert_eq!(
        rows(
            conn,
            "SELECT conversation, id, at FROM conversation_entries WHERE source = 'ask' ORDER BY id"
        ),
        [
            vec![text("work:1"), int(3), int(35)],
            vec![text("architect:p"), int(4), int(40)],
            vec![text("work:1"), int(5), int(30)],
        ]
    );
    // The change log names each ask by its number: its own rows are rows of the messages.
    assert_eq!(
        rows(
            conn,
            "SELECT table_name, row_key, old_row, new_row FROM changes
             WHERE table_name IN ('owner_asks', 'messages', 'architect_matters', 'blockers') ORDER BY id"
        ),
        [
            vec![
                text("messages"),
                text(r#"{"id":3}"#),
                Value::Null,
                text(r#"{"id":3,"answer":null}"#)
            ],
            vec![
                text("messages"),
                text(r#"{"id":1}"#),
                Value::Null,
                text(r#"{"id":1,"ask":3}"#)
            ],
            vec![
                text("architect_matters"),
                text(r#"{"id":1}"#),
                Value::Null,
                text(r#"{"id":1,"asked":4}"#)
            ],
            vec![
                text("messages"),
                text(r#"{"id":3}"#),
                text(r#"{"id":3,"answer":null}"#),
                text(r#"{"id":3,"answer":"x"}"#)
            ],
            vec![
                text("blockers"),
                text(r#"{"id":1}"#),
                text(r#"{"id":1,"ask":3,"ended":null}"#),
                text(r#"{"id":1,"ask":3,"ended":"resolved"}"#)
            ],
        ]
    );
    assert!(
        rows(
            conn,
            "SELECT name FROM sqlite_schema WHERE name IN ('owner_asks', 'owner_asks_by_round')"
        )
        .is_empty()
    );
    drop(db);
    let _ = std::fs::remove_dir_all(&dir);
}
