//! What the migration of the health numbers does to the rows it keeps: the merge tool's old waits for the full CI are
//! marked, and the outbox it makes afresh keeps its rows. Kept apart from the migration tests in `src/lib.rs`, as these
//! see the database through its public interface alone.

use db::MIGRATIONS;
use db::rusqlite::{Connection, params};

/// The migration these tests read, by its file's name.
const NAME: &str = "202611060130-record-the-health-numbers";

/// A database as it stood before that migration: made by the first version's schema and the migrations before it, with
/// the foreign keys off as the migrations run, so that a run needs no round to seed it.
fn before() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", false).unwrap();
    conn.execute_batch(include_str!("../../../db/migrations/0001.sql"))
        .unwrap();
    for m in &MIGRATIONS[..position()] {
        conn.execute_batch(m.sql).unwrap();
    }
    conn
}

/// Where the migration stands among the migrations.
fn position() -> usize {
    MIGRATIONS
        .iter()
        .position(|m| m.name == NAME)
        .unwrap_or_else(|| panic!("no migration {NAME}"))
}

// The tool's rows as the merge wrote them before: a wait for the full CI gave a list of checks; the call to merge gave a
// commit; the look at whether it merged already gave null; a try that failed gave what failed, and all those on record
// are of the call to merge; and a row still running has no result. Only the first is marked, however many checks it
// gave. Another tool's list of checks is not, and what each row held besides stays.
#[test]
fn the_migration_marks_the_merge_tool_s_old_waits_for_the_full_ci() {
    let conn = before();
    let args = r#"{"head":"abc","pr":7}"#;
    let rows = [
        ("merge", r#"[{"name":"check","result":"green"}]"#),
        ("merge", "[]"),
        ("merge", r#""0123abc""#),
        ("merge", "null"),
        ("merge", r#"{"failed":{"kind":"other","what":"no"}}"#),
        ("ci", r#"[{"name":"check","result":"green"}]"#),
    ];
    for (tool, result) in rows {
        conn.execute(
            "INSERT INTO tool_runs (work, round, tool, args, attempt, result, queued_at)
             VALUES (1, 1, ?1, ?2, 1, ?3, 1)",
            params![tool, args, result],
        )
        .unwrap();
    }
    conn.execute(
        "INSERT INTO tool_runs (work, round, tool, args, attempt, queued_at) VALUES (1, 1, 'merge', ?1, 1, 1)",
        [args],
    )
    .unwrap();
    conn.execute_batch(MIGRATIONS[position()].sql).unwrap();
    let mut st = conn
        .prepare(
            "SELECT tool, json_extract(args, '$.step'), json_extract(args, '$.head'), json_extract(args, '$.pr'), result
             FROM tool_runs ORDER BY id",
        )
        .unwrap();
    type Row = (String, Option<String>, String, i64, Option<String>);
    let got: Vec<Row> = st
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let want: Vec<Row> = rows
        .iter()
        .enumerate()
        .map(|(k, (tool, result))| {
            let marked = (k < 2).then(|| "full_ci".to_string());
            (tool.to_string(), marked, "abc".to_string(), 7, Some(result.to_string()))
        })
        .chain([("merge".to_string(), None, "abc".to_string(), 7, None)])
        .collect();
    assert_eq!(got, want);
}

// The outbox is made afresh for its description: its rows keep their ids, by which the engine carries them out in order,
// and everything else they held.
#[test]
fn the_migration_keeps_the_outbox_s_rows_and_their_ids() {
    let conn = before();
    conn.execute(
        "INSERT INTO outbox (id, work, kind, args, run_after, attempts, last_error)
         VALUES (5, 1, 'revert_merge', '{\"merge_commit\":\"abc\"}', 9, 2, 'no'), (8, NULL, 'notify', '{}', 3, 0, NULL)",
        [],
    )
    .unwrap();
    conn.execute_batch(MIGRATIONS[position()].sql).unwrap();
    let mut st = conn
        .prepare("SELECT id, work, kind, args, run_after, attempts, last_error FROM outbox ORDER BY id")
        .unwrap();
    type Row = (i64, Option<i64>, String, String, i64, i64, Option<String>);
    let got: Vec<Row> = st
        .query_map([], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
            ))
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        got,
        [
            (
                5,
                Some(1),
                "revert_merge".to_string(),
                "{\"merge_commit\":\"abc\"}".to_string(),
                9,
                2,
                Some("no".to_string())
            ),
            (8, None, "notify".to_string(), "{}".to_string(), 3, 0, None),
        ]
    );
}
