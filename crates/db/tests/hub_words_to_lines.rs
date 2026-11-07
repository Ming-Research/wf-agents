//! What is said to the lines of a work as the hub keeps it: the owner's, the architect's and the engine's words to a
//! work's line (`interjections` before) are each a message, delivered to that line, which a round takes. The migration
//! that brings them in keeps each word's number, which rounds and sessions name, and moves the hub's other messages
//! after them.

use std::path::{Path, PathBuf};

use db::rusqlite::Connection;
use db::rusqlite::types::Value;
use db::{Db, MIGRATIONS};

const MIGRATION: &str = "202611070626-words-to-the-lines-of-a-work-through-the-hub";

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

// A database with words to a work's lines of every kind (the owner's with a delegate and a part relayed, taken by a
// round; the owner's to the architect in the work's group, its copy told to the engineer; a question about a decision
// card with an image; the architect's word that needs no answer; the notice of an item cancelled, not taken), an entry of
// a talk with the architect, and the hub's messages before (a fault with its cache miss, passed on to an architect)
// takes the migration: each word keeps its number as its message, delivered to its line, taken by the round that took
// it, and to the architect too when a matter holds it; the talk's entry is the owner's message to the architect, naming
// the talk; the messages before come after them, with their deliveries and their cache miss; the image belongs to its
// message; and the views read them as they read the words before.
#[test]
fn the_migration_keeps_every_word_s_number_and_where_it_stands() {
    let dir = temp("hub-words");
    let path = dir.join("db");
    {
        let conn = database_before_migration(&path);
        conn.execute_batch(
            r#"INSERT INTO items (id, project, title, problem, why, deliver, level)
                 VALUES (1, 'p', 't', 'x', 'y', 'change', '1'), (2, 'p', 'gone', 'x', 'y', 'change', '1');
               INSERT INTO flow_versions (id, digest, compiled) VALUES (1, 'd', '{}');
               INSERT INTO works (id, item, flow_version, graph, at) VALUES (1, 1, 1, 'engineer', 'I1');
               INSERT INTO works (id, flow_version, graph, at, project) VALUES (2, 1, 'architect', 'H1', 'p');
               INSERT INTO rounds (work, n, cell, input, config_commit, started_at, ended_at)
                 VALUES (1, 1, 'R1', '{}', 'c', 5, 20), (2, 1, 'H1', '{}', 'c', 30, 40);
               INSERT INTO design_cards (id, work, round, key, body, status) VALUES (1, 1, 1, 'k', '{}', 'open');
               INSERT INTO architect_matters (id, project, kind, detail, what, body, at)
                 VALUES (1, 'p', 'owner_words', 'said', 'w', '{"text":"Hold it.","where":"work","work":1}', 9),
                        (2, 'p', 'owner_words', 'said', 'w', '{"text":"Hi.","where":"page"}', 9),
                        (3, 'wf', 'report', 'fault', 'f', '{"by":"owner"}', 12);
               INSERT INTO interjections (id, work, line, speaker, declared, relayed, text, delivered_round, informs,
                                          matter, design_card, cancelled_item) VALUES
                 (1, 1, 'engineer', 'owner', 'delegate', '["Go on."]', 'Fine. Go on.', 1, 0, NULL, NULL, NULL),
                 (2, 1, 'engineer', 'owner', NULL, NULL, 'Said to the architect, for you to know: Hold it.', NULL, 1, 1,
                  NULL, NULL),
                 (3, 1, 'engineer', 'owner', 'owner', NULL, 'Why this size?', NULL, 0, NULL, 1, NULL),
                 (4, 1, 'review', 'architect', NULL, NULL, 'Nothing changes for you.', NULL, 1, NULL, NULL, NULL),
                 (5, 1, 'engineer', 'architect', NULL, NULL, 'Item 2 was cancelled.', NULL, 0, NULL, NULL, 2);
               INSERT INTO attachments (id, interjection, position, media_type, size, hash, bytes)
                 VALUES (1, 3, 1, 'image/png', 4, 'h', zeroblob(4));
               INSERT INTO talk_entries (id, matter, text, at, taken_work, taken_round) VALUES (1, 2, 'And more.', 11, 2, 1);
               INSERT INTO messages (id, sender, work, round, text, body, ask, at)
                 VALUES (1, 'engineer', 1, 1, 'Told you.', NULL, NULL, 20),
                        (2, 'engine', 1, NULL, 'R1-1 missed its cache.', '{"kind":"cache_miss"}', NULL, 12);
               INSERT INTO deliveries (id, message, recipient, matter, read_at)
                 VALUES (1, 1, 'owner', NULL, NULL), (2, 2, 'keeper', NULL, 13), (3, 2, 'architect', 3, NULL);
               INSERT INTO cache_misses (work, round, line, differs, written, held, cost_usd, message)
                 VALUES (1, 1, 'engineer', '[]', 1, 2, NULL, 2);"#,
        )
        .unwrap();
        // When each word was said, as the record of changes keeps it.
        for (id, at) in [(1, 15), (2, 16), (3, 17), (4, 18), (5, 19)] {
            conn.execute(
                "INSERT INTO changes (tx, at, actor, table_name, row_key, new_row)
                 VALUES (1, ?2, 'owner', 'interjections', json_object('id', ?1), '{}')",
                [id, at],
            )
            .unwrap();
        }
    }
    let db = Db::open(&path).unwrap();
    let conn = db.conn();
    assert_eq!(
        rows(
            conn,
            "SELECT id, speaker, line, text, declared, relayed, informs, design_card, cancelled_item, matter, at,
                    taken_round
             FROM line_sayings ORDER BY id"
        ),
        [
            vec![
                int(1),
                text("owner"),
                text("engineer"),
                text("Fine. Go on."),
                text("delegate"),
                text(r#"["Go on."]"#),
                int(0),
                Value::Null,
                Value::Null,
                Value::Null,
                int(15),
                int(1)
            ],
            vec![
                int(2),
                text("owner"),
                text("engineer"),
                text("Said to the architect, for you to know: Hold it."),
                Value::Null,
                Value::Null,
                int(1),
                Value::Null,
                Value::Null,
                int(1),
                int(16),
                Value::Null
            ],
            vec![
                int(3),
                text("owner"),
                text("engineer"),
                text("Why this size?"),
                text("owner"),
                Value::Null,
                int(0),
                int(1),
                Value::Null,
                Value::Null,
                int(17),
                Value::Null
            ],
            vec![
                int(4),
                text("architect"),
                text("review"),
                text("Nothing changes for you."),
                Value::Null,
                Value::Null,
                int(1),
                Value::Null,
                Value::Null,
                Value::Null,
                int(18),
                Value::Null
            ],
            vec![
                int(5),
                text("architect"),
                text("engineer"),
                text("Item 2 was cancelled."),
                Value::Null,
                Value::Null,
                int(0),
                Value::Null,
                int(2),
                Value::Null,
                int(19),
                Value::Null
            ],
        ]
    );
    // The messages before come after the words, with their deliveries; the talk's entry after them.
    assert_eq!(
        rows(
            conn,
            "SELECT m.id, m.sender, m.text, d.recipient, d.matter, d.read_at, d.taken_work, d.taken_round
             FROM messages m JOIN deliveries d ON d.message = m.id WHERE m.id > 5 ORDER BY m.id, d.id"
        ),
        [
            vec![
                int(6),
                text("engineer"),
                text("Told you."),
                text("owner"),
                Value::Null,
                Value::Null,
                Value::Null,
                Value::Null
            ],
            vec![
                int(7),
                text("engine"),
                text("R1-1 missed its cache."),
                text("keeper"),
                Value::Null,
                int(13),
                Value::Null,
                Value::Null
            ],
            vec![
                int(7),
                text("engine"),
                text("R1-1 missed its cache."),
                text("architect"),
                int(3),
                Value::Null,
                Value::Null,
                Value::Null
            ],
            vec![
                int(8),
                text("owner"),
                text("And more."),
                text("architect"),
                int(2),
                Value::Null,
                int(2),
                int(1)
            ],
        ]
    );
    assert_eq!(rows(conn, "SELECT message FROM cache_misses"), [vec![int(7)]]);
    assert_eq!(rows(conn, "SELECT message FROM attachments"), [vec![int(3)]]);
    assert_eq!(
        rows(
            conn,
            "SELECT interjection FROM waiting_interjections ORDER BY interjection"
        ),
        [vec![int(2)], vec![int(3)], vec![int(4)], vec![int(5)]]
    );
    // The owner's words to the architect in the group show once, as the copy the engineer is told; the talk's entry
    // in the one-to-one.
    assert_eq!(
        rows(
            conn,
            "SELECT conversation, source, id FROM conversation_entries
             WHERE source IN ('interjection', 'words', 'talk') ORDER BY source, id"
        ),
        [
            vec![text("work:1"), text("interjection"), int(1)],
            vec![text("work:1"), text("interjection"), int(2)],
            vec![text("work:1"), text("interjection"), int(3)],
            vec![text("work:1"), text("interjection"), int(4)],
            vec![text("work:1"), text("interjection"), int(5)],
            vec![text("architect:p"), text("talk"), int(8)],
            vec![text("architect:p"), text("words"), int(2)],
        ]
    );
    assert!(
        rows(
            conn,
            "SELECT name FROM sqlite_schema WHERE name IN ('interjections', 'talk_entries')"
        )
        .is_empty()
    );
    drop(db);
    let _ = std::fs::remove_dir_all(&dir);
}
