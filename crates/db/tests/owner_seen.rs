//! What is new to the owner: an item is new when it was made, or its content changed, since the owner last opened it.
//! The database keeps the content version of each item (`item_content_version`) and the version the owner last saw
//! (`owner_seen`); the migration that brings them in marks every item already there seen.

use std::path::PathBuf;

use db::rusqlite::Connection;
use db::{Actor, Db, MIGRATIONS};

const MIGRATION: &str = "202611061000-mark-the-items-new-to-the-owner";

/// A database holding the items 1 and 2.
fn world() -> Db {
    let mut db = Db::in_memory().unwrap();
    db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO items (id, project, title, problem, why, deliver, level)
             VALUES (1, 'p', 'First', 'x', 'y', 'change', '1'), (2, 'p', 'Second', 'x', 'y', 'change', '1')",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    db
}

/// The content version of an item, and whether it is new to the owner: it has no row in `owner_seen`, or its content
/// version is newer than the row's.
fn state(conn: &Connection, item: i64) -> (i64, bool) {
    let version: i64 = conn
        .query_row("SELECT change FROM item_content_version WHERE item = ?1", [item], |r| {
            r.get(0)
        })
        .unwrap();
    let unseen: bool = conn
        .query_row(
            "SELECT NOT EXISTS (SELECT 1 FROM owner_seen s WHERE s.item = ?1 AND s.change >= ?2)",
            (item, version),
            |r| r.get(0),
        )
        .unwrap();
    (version, unseen)
}

/// The owner opens an item: it is seen at its content version now.
fn open(db: &mut Db, item: i64) {
    db.write(Actor::Owner, |tx| {
        tx.execute(
            "INSERT INTO owner_seen (item, change) SELECT item, change FROM item_content_version WHERE item = ?1
             ON CONFLICT (item) DO UPDATE SET change = max(change, excluded.change)",
            [item],
        )?;
        Ok(())
    })
    .unwrap();
}

fn change(db: &mut Db, actor: Actor, set: &str) {
    db.write(actor, |tx| {
        tx.execute(&format!("UPDATE items SET {set} WHERE id = 1"), [])?;
        Ok(())
    })
    .unwrap();
}

// An item is new when made, seen once the owner opens it, and new again after a change to what its creator writes;
// a change to the level, to deliver, to its cancellation or to anything else the engine and the architect set leaves it
// seen, and so does a write that changes nothing. The other item is untouched by all of it.
#[test]
fn an_item_is_new_when_made_and_when_its_content_changes_and_seen_once_opened() {
    let mut db = world();
    let (made, unseen) = state(db.conn(), 1);
    assert!(made > 0 && unseen, "a new item is unseen: {made}");
    assert!(state(db.conn(), 2).1);

    open(&mut db, 1);
    assert_eq!(state(db.conn(), 1), (made, false), "opened, it is seen");
    assert!(state(db.conn(), 2).1, "and the other is not");

    for (set, what) in [
        ("level = '2'", "its level"),
        ("deliver = 'report'", "its deliver"),
        ("cancelled = 'not wanted'", "its cancellation"),
        ("imported_from = 'board'", "where it was imported from"),
        ("title = title", "nothing"),
    ] {
        change(&mut db, Actor::Engine, set);
        assert_eq!(
            state(db.conn(), 1),
            (made, false),
            "a change to {what} is no new content"
        );
    }

    // Each column its creator writes makes it new once changed, and seen again once opened.
    let mut last = made;
    for set in [
        "title = 'Other'",
        "problem = 'z'",
        "why = 'w'",
        "approach = 'a'",
        "approach = NULL",
        "wanted = '[\"one\"]'",
        "wanted = '[\"one\", \"two\"]'",
        "decision_points = '[{\"point\": 1}]'",
    ] {
        change(&mut db, Actor::Engine, set);
        let (version, unseen) = state(db.conn(), 1);
        assert!(
            version > last && unseen,
            "after {set} the item is new: {version} after {last}"
        );
        open(&mut db, 1);
        assert_eq!(state(db.conn(), 1), (version, false), "opened after {set}, it is seen");
        last = version;
    }
    assert!(state(db.conn(), 2).1);
}

// Opening an item before a change of its content that comes after leaves it new, however many changes of other
// columns come between; marking an item seen again marks nothing back.
#[test]
fn a_content_change_after_the_last_change_of_something_else_is_what_makes_an_item_new() {
    let mut db = world();
    open(&mut db, 1);
    change(&mut db, Actor::Engine, "level = '3'");
    change(&mut db, Actor::Engine, "why = 'because'");
    change(&mut db, Actor::Engine, "cancelled = 'no'");
    let (version, unseen) = state(db.conn(), 1);
    assert!(unseen);
    open(&mut db, 1);
    open(&mut db, 1);
    assert_eq!(state(db.conn(), 1), (version, false));
    let rows: i64 = db
        .conn()
        .query_row("SELECT count(*) FROM owner_seen WHERE item = 1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(rows, 1);
}

/// A path under the temporary directory, made empty.
fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("wf-agents-db-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A database file just before the migration, made by the first version's schema and the migrations up to it, with the
/// names of the migrations taken kept where the database keeps them.
fn database_before_migration(path: &std::path::Path) -> Connection {
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

// A database that already has items takes the migration with every one of them seen at its content version, so that
// nothing is new at the start; an item made after it is new.
#[test]
fn the_migration_marks_the_items_already_there_seen() {
    let dir = temp("owner-seen");
    let path = dir.join("db");
    {
        let conn = database_before_migration(&path);
        conn.execute(
            "INSERT INTO items (id, project, title, problem, why, deliver, level)
             VALUES (1, 'p', 't', 'x', 'y', 'change', '1'), (2, 'p', 't', 'x', 'y', 'change', '1'),
                    (3, 'p', 't', 'x', 'y', 'change', '1')",
            [],
        )
        .unwrap();
        // The record of changes as the engine wrote it: item 1 made; item 2 made, its title changed, then its level.
        let record = |old: Option<&str>, new: &str, id: i64| {
            conn.execute(
                "INSERT INTO changes (tx, at, actor, table_name, row_key, old_row, new_row)
                 VALUES (1, 1, 'architect', 'items', json_object('id', ?1), ?2, ?3)",
                (id, old, new),
            )
            .unwrap();
        };
        record(None, r#"{"id":1,"title":"t","level":"1"}"#, 1);
        record(None, r#"{"id":2,"title":"t","level":"1"}"#, 2);
        record(
            Some(r#"{"id":2,"title":"t","level":"1"}"#),
            r#"{"id":2,"title":"u","level":"1"}"#,
            2,
        );
        record(
            Some(r#"{"id":2,"title":"u","level":"1"}"#),
            r#"{"id":2,"title":"u","level":"2"}"#,
            2,
        );
    }
    let mut db = Db::open(&path).unwrap();
    for (item, version) in [(1, 1), (2, 3), (3, 0)] {
        assert_eq!(
            state(db.conn(), item),
            (version, false),
            "item {item} after the migration"
        );
    }
    db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO items (id, project, title, problem, why, deliver, level)
             VALUES (4, 'p', 't', 'x', 'y', 'change', '1')",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    assert!(state(db.conn(), 4).1, "an item made after the migration is new");
    change(&mut db, Actor::Engine, "why = 'changed'");
    assert!(state(db.conn(), 1).1, "and so is one whose content changes");
    let _ = std::fs::remove_dir_all(&dir);
}
