//! The view `sent_back_problems`, which gives each problem of each return the engine sent back as `rounds.rejected`
//! keeps them under `problems`. Read through the database's public interface alone, as a person or an agent queries it.
//! That a database migrated from version 1 has the view is the last test here; that it has the description of `rounds`
//! the schema writes is the test of the migrations in `src/lib.rs`; that the rounds recorded before keep every column
//! is the test in `rounds_counts.rs`, which migrates through every migration.

use db::rusqlite::{Connection, params};
use db::{Actor, Db, SCHEMA};

/// One row of the view: work, round, cell, position, check_name, field, text.
type Problem = (i64, i64, String, i64, String, Option<String>, String);

/// A database made from the schema, with two works.
fn new_database() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(SCHEMA).unwrap();
    two_works(&conn);
    conn
}

/// The flow version and the two works the rounds of the tests belong to.
fn two_works(conn: &Connection) {
    conn.execute_batch(
        "INSERT INTO flow_versions (id, digest, compiled) VALUES (1, 'd', '{}');
         INSERT INTO works (id, flow_version, graph, at) VALUES (1, 1, 'engineer', 'T4'), (2, 1, 'engineer', 'T4');",
    )
    .unwrap();
}

/// A round of a work at cell `T4` that ended with the return sent back, or with `transition` when the return was taken.
fn round(conn: &Connection, work: i64, n: i64, transition: Option<&str>, rejected: Option<&str>) {
    conn.execute(
        "INSERT INTO rounds (work, n, cell, input, transition, rejected, config_commit, started_at, ended_at)
         VALUES (?1, ?2, 'T4', '{}', ?3, ?4, 'c', 1, 2)",
        params![work, n, transition, rejected],
    )
    .unwrap();
}

fn problems(conn: &Connection) -> Vec<Problem> {
    let mut st = conn
        .prepare(
            "SELECT work, round, cell, position, check_name, field, text FROM sent_back_problems
             ORDER BY work, round, position",
        )
        .unwrap();
    st.query_map([], |r| {
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
    .map(|x| x.unwrap())
    .collect()
}

fn row(work: i64, round: i64, position: i64, check: &str, field: Option<&str>, text: &str) -> Problem {
    (
        work,
        round,
        "T4".to_string(),
        position,
        check.to_string(),
        field.map(str::to_string),
        text.to_string(),
    )
}

// The view gives one row for each problem of each return sent back, with the columns a person groups by, and no row
// for a round that holds no `problems`: a return sent back before they were kept, a failed run, a cut-off run, a
// return taken. Returns and problems are counted apart, as one return may hold several problems of one check and
// field; the check and the field are grouped together, as the field of a check and the field of `format` are written
// in two ways. The rounds are written here as the engine writes them, with the columns by name, not by the code that
// writes them.
#[test]
fn the_view_gives_a_row_for_each_problem_and_none_for_a_round_without_problems() {
    let conn = new_database();
    // Two checks at once, and a problem of no field: a check about the whole return.
    round(
        &conn,
        1,
        1,
        None,
        Some(
            r#"{"check":"evidence_holds (columns[pass].evidence): a\ncolumns_answered (columns.id): b\nno_field (the return): c",
                "problems":[
                  {"check":"evidence_holds","field":"columns[pass].evidence","text":"a"},
                  {"check":"columns_answered","field":"columns.id","text":"b"},
                  {"check":"no_field","field":null,"text":"c"}]}"#,
        ),
    );
    // Sent back before the problems were kept: the text alone.
    round(
        &conn,
        1,
        2,
        None,
        Some(r#"{"check":"evidence_holds (columns[pass].evidence): a"}"#),
    );
    // A return that does not fit the schema, in two places of one field of every item, and one of no field.
    round(
        &conn,
        1,
        3,
        None,
        Some(
            r#"{"format":"x",
                "problems":[
                  {"check":"format","field":"columns[].evidence","text":"columns[0] has no evidence"},
                  {"check":"format","field":"columns[].evidence","text":"columns[1] has no evidence"},
                  {"check":"format","field":null,"text":"the return is 3"}]}"#,
        ),
    );
    // Two problems of one check and one field in one return.
    round(
        &conn,
        2,
        1,
        None,
        Some(
            r#"{"check":"p\nq",
                "problems":[
                  {"check":"evidence_holds","field":"columns[pass].evidence","text":"p"},
                  {"check":"evidence_holds","field":"columns[pass].evidence","text":"q"}]}"#,
        ),
    );
    // A failed run, a cut-off run, a run that ended in an error and a return that was taken.
    round(&conn, 2, 2, None, Some(r#"{"failed":{"kind":"k","what":"w"}}"#));
    round(&conn, 2, 3, None, Some(r#"{"interrupted":"i"}"#));
    round(&conn, 2, 4, None, Some(r#"{"error":"e"}"#));
    round(&conn, 2, 5, Some("done"), None);

    assert_eq!(
        problems(&conn),
        [
            row(1, 1, 0, "evidence_holds", Some("columns[pass].evidence"), "a"),
            row(1, 1, 1, "columns_answered", Some("columns.id"), "b"),
            row(1, 1, 2, "no_field", None, "c"),
            row(
                1,
                3,
                0,
                "format",
                Some("columns[].evidence"),
                "columns[0] has no evidence"
            ),
            row(
                1,
                3,
                1,
                "format",
                Some("columns[].evidence"),
                "columns[1] has no evidence"
            ),
            row(1, 3, 2, "format", None, "the return is 3"),
            row(2, 1, 0, "evidence_holds", Some("columns[pass].evidence"), "p"),
            row(2, 1, 1, "evidence_holds", Some("columns[pass].evidence"), "q"),
        ]
    );

    // The columns, by name and in order.
    let columns: Vec<String> = conn
        .prepare("SELECT name FROM pragma_table_info('sent_back_problems')")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(|x| x.unwrap())
        .collect();
    assert_eq!(
        columns,
        ["work", "round", "cell", "position", "check_name", "field", "text"]
    );

    // Grouped by check and field: the returns apart from the problems, and the check by a name that needs no quotes.
    let mut st = conn
        .prepare(
            "SELECT check_name, field, count(*), count(DISTINCT work || '-' || round) FROM sent_back_problems
             GROUP BY check_name, field ORDER BY check_name, field",
        )
        .unwrap();
    let groups: Vec<(String, Option<String>, i64, i64)> = st
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .unwrap()
        .map(|x| x.unwrap())
        .collect();
    let group = |check: &str, field: Option<&str>, problems: i64, returns: i64| {
        (check.to_string(), field.map(str::to_string), problems, returns)
    };
    assert_eq!(
        groups,
        [
            group("columns_answered", Some("columns.id"), 1, 1),
            // Three problems in two returns: two of them in one.
            group("evidence_holds", Some("columns[pass].evidence"), 3, 2),
            group("format", None, 1, 1),
            group("format", Some("columns[].evidence"), 2, 1),
            group("no_field", None, 1, 1),
        ]
    );
}

// A return with many problems is read in the order they were found, `position` counting as a number (the eleventh after
// the tenth, not after the second), and a column that is valid JSON of another shape than the engine writes gives no
// row and no error, since one such row would make every query of the view fail. The reference is SQLite's own rule for
// `json_each` over an array (its `key` is the index) and the check of `rounds.rejected` that it is valid JSON.
#[test]
fn the_view_keeps_the_order_of_many_problems_and_reads_a_column_of_another_shape() {
    let conn = new_database();
    let many: Vec<String> = (0..13)
        .map(|i| format!(r#"{{"check":"columns_answered","field":"columns.id","text":"column {i} is not answered"}}"#))
        .collect();
    round(
        &conn,
        1,
        1,
        None,
        Some(&format!(r#"{{"check":"x","problems":[{}]}}"#, many.join(","))),
    );
    // Valid JSON the engine does not write for a return sent back: no row, and the view still answers.
    for (n, odd) in [r#""text""#, "3", "null", "[]", r#"{"problems":[]}"#]
        .into_iter()
        .enumerate()
    {
        round(&conn, 2, n as i64 + 1, None, Some(odd));
    }
    let found = problems(&conn);
    assert_eq!(found.len(), 13, "{found:?}");
    for (i, p) in found.iter().enumerate() {
        assert_eq!(
            p,
            &row(
                1,
                1,
                i as i64,
                "columns_answered",
                Some("columns.id"),
                &format!("column {i} is not answered")
            )
        );
    }
    // The position is a number, so that it orders by value in any query.
    let kinds: Vec<String> = conn
        .prepare("SELECT DISTINCT typeof(position) FROM sent_back_problems")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(|x| x.unwrap())
        .collect();
    assert_eq!(kinds, ["integer"]);
}

// A database made at version 1 and migrated through every migration has the view, and it reads the rounds written
// after the migration as one made from the schema does. The schema's own test of the view cannot show that the
// migration makes it, since the schema has it whether or not a migration does. The expected row is worked out by hand
// from the round written; before the change the migrated database has no view, and the query fails.
#[test]
fn a_database_migrated_from_version_1_has_the_view_of_the_problems_of_returns_sent_back() {
    let dir = std::env::temp_dir().join(format!("wf-agents-db-sent-back-problems-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("wf.db");
    {
        // The first version's schema, as the migrations start from it.
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(include_str!("../../../db/migrations/0001.sql"))
            .unwrap();
        conn.pragma_update(None, "user_version", 1).unwrap();
    }
    let mut db = Db::open(&path).unwrap();
    // The state tables take writes through the database's own, as the engine makes them.
    db.write(Actor::Engine, |tx| {
        two_works(tx);
        round(
            tx,
            1,
            1,
            None,
            Some(r#"{"check":"c (f): t","problems":[{"check":"c","field":"f","text":"t"}]}"#),
        );
        Ok(())
    })
    .unwrap();
    assert_eq!(problems(db.conn()), [row(1, 1, 0, "c", Some("f"), "t")]);
    let _ = std::fs::remove_dir_all(&dir);
}
