//! The migration that drops the mark of a bug fix or a feature from the items, seen through the public interface: a
//! database file as it was just before it, with a row in every table that points to an item, opened as the engine
//! opens it. Kept apart from the migration test in `src/lib.rs`, which reaches the migrations by the code's own helpers.

use std::path::PathBuf;

use db::rusqlite::{Connection, params};
use db::{Actor, Db, MIGRATIONS};

const DROP: &str = "202611042230-drop-the-route-mark-of-items";

/// A path under the temporary directory, made empty.
fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("wf-agents-db-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A database file just before the migration `DROP`: made by the first version's schema and the migrations up to it,
/// with the foreign keys off as the migrations run, and the names of the migrations taken kept where the database
/// keeps them, so that opening it takes `DROP` and those after it and no other.
fn database_before_the_drop(path: &std::path::Path) -> Connection {
    let at = MIGRATIONS.iter().position(|m| m.name == DROP).unwrap();
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

/// The columns of the items table that survive the migration, in the order the rows below give them.
const KEPT: &str = "id, project, title, problem, why, approach, deliver, level, acceptance, acceptance_work, \
                    acceptance_round, cancelled, imported_from, imported_branch, imported_pr, wanted, held, rank, \
                    decision_points";

// Every column of every item, and every row of every table that points to an item, comes out of the migration as it
// went in, with nothing left pointing at a row that is gone; the column `route` is gone; and a write to an item after
// it is recorded in `changes`, which the migration must not have lost with the table it made afresh, though it records
// nothing for the rows it copied. The expected rows are the ones written below, not what the migration copies. Before
// the migration the table has the column and the migration does not exist.
#[test]
fn every_column_and_every_row_pointing_to_an_item_survives_the_drop_of_the_route_mark() {
    let dir = temp("items-without-route");
    let path = dir.join("db");
    {
        let conn = database_before_the_drop(&path);
        conn.execute(
            "INSERT INTO flow_versions (id, digest, compiled) VALUES (1, 'd', '{}')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO works (id, item, flow_version, graph, at) VALUES (1, 1, 1, 'engineer', 'I1')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO rounds (work, n, cell, input, config_commit, started_at, transition)
             VALUES (1, 1, 'R1', '{}', 'c', 1, 'start')",
            [],
        )
        .unwrap();
        // Item 1 holds a value in every column and points to the round that gave its acceptance; item 2 is the
        // other mark, cancelled and imported, with the optional columns empty.
        conn.execute(
            "INSERT INTO items (id, project, title, problem, why, approach, deliver, level, acceptance,
                                acceptance_work, acceptance_round, cancelled, imported_from, imported_branch,
                                imported_pr, wanted, held, rank, decision_points, route)
             VALUES (1, 'p', 'one', 'pro', 'why', 'how', 'change', 'l1', '[{\"id\":\"A1\"}]', 1, 1, NULL, NULL, NULL,
                     NULL, '[{\"id\":\"V1\"}]', 'held for now', 4, '[{\"id\":\"P1\"}]', 'bugfix'),
                    (2, 'p', 'two', 'pro2', 'why2', NULL, 'report', 'l2', NULL, NULL, NULL, 'dup', 'old board', 'br',
                     9, NULL, NULL, NULL, NULL, 'normal')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO item_links (item, depends_on, needs, kind) VALUES (2, 1, 'n', 'blocking')",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO dispatch_queue (item, position) VALUES (2, 1.5)", [])
            .unwrap();
        conn.execute(
            "INSERT INTO known_red_checks (project, check_name, why, fix_item) VALUES ('p', 'lint', 'broken', 1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO observations (project, what, source, folded_into) VALUES ('p', 'w', 's', 2)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO architect_matters (project, kind, what, body, item, at) VALUES ('p', 'k', 'w', '{}', 2, 1)",
            [],
        )
        .unwrap();
        let has_route: i64 = conn
            .query_row(
                "SELECT count(*) FROM pragma_table_info('items') WHERE name = 'route'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(has_route, 1, "the database is not the one before the migration");
    }
    let mut db = Db::open(&path).unwrap();
    let count = |sql: &str| -> i64 { db.conn().query_row(sql, [], |r| r.get(0)).unwrap() };
    assert_eq!(
        count("SELECT count(*) FROM pragma_table_info('items') WHERE name = 'route'"),
        0
    );
    assert_eq!(count("SELECT count(*) FROM pragma_foreign_key_check"), 0);
    let items: Vec<String> = {
        let mut st = db
            .conn()
            .prepare(&format!("SELECT json_array({KEPT}) FROM items ORDER BY id"))
            .unwrap();
        st.query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect()
    };
    assert_eq!(
        items,
        [
            r#"[1,"p","one","pro","why","how","change","l1","[{\"id\":\"A1\"}]",1,1,null,null,null,null,"[{\"id\":\"V1\"}]","held for now",4,"[{\"id\":\"P1\"}]"]"#,
            r#"[2,"p","two","pro2","why2",null,"report","l2",null,null,null,"dup","old board","br",9,null,null,null,null]"#,
        ]
    );
    // Each table that points to an item still has its row, and the row still reaches the item it pointed to.
    for (what, sql) in [
        (
            "item_links",
            "SELECT count(*) FROM item_links l JOIN items a ON a.id = l.item JOIN items b ON b.id = l.depends_on
             WHERE l.item = 2 AND l.depends_on = 1",
        ),
        (
            "dispatch_queue",
            "SELECT count(*) FROM dispatch_queue q JOIN items i ON i.id = q.item WHERE q.item = 2 AND q.position = 1.5",
        ),
        (
            "known_red_checks",
            "SELECT count(*) FROM known_red_checks k JOIN items i ON i.id = k.fix_item WHERE k.fix_item = 1",
        ),
        (
            "observations",
            "SELECT count(*) FROM observations o JOIN items i ON i.id = o.folded_into WHERE o.folded_into = 2",
        ),
        (
            "architect_matters",
            "SELECT count(*) FROM architect_matters m JOIN items i ON i.id = m.item WHERE m.item = 2",
        ),
        (
            "works",
            "SELECT count(*) FROM works w JOIN items i ON i.id = w.item WHERE w.item = 1",
        ),
    ] {
        assert_eq!(count(sql), 1, "{what} lost its row or its item");
    }
    assert_eq!(
        count("SELECT count(*) FROM changes WHERE table_name = 'items'"),
        0,
        "the migration recorded the rows it copied as changes"
    );
    db.write(Actor::Engine, |tx| {
        tx.execute("UPDATE items SET held = NULL WHERE id = 1", [])?;
        Ok(())
    })
    .unwrap();
    let recorded: i64 = db
        .conn()
        .query_row(
            "SELECT count(*) FROM changes WHERE table_name = 'items' AND row_key = json_object('id', 1)",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(recorded, 1, "a change to an item after the migration is not recorded");
    // The route the work went is read from its rows: the research took `start`.
    let way: Option<String> = db
        .conn()
        .query_row("SELECT route FROM work_routes WHERE work = ?1", params![1], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(way.as_deref(), Some("direct"));
    let _ = std::fs::remove_dir_all(&dir);
}
