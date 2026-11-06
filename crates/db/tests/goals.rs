//! Goals: items with no work of their own, reached through the items they depend on. Seen through the views the engine
//! starts works by and the agents read, on a new database, and through the migration that lets an item be one, on a
//! database file as it was just before it, opened as the engine opens it.

use std::path::PathBuf;

use db::rusqlite::Connection;
use db::{Actor, Db, MIGRATIONS};

const GOALS: &str = "202611060135-items-that-are-goals";

/// The items of one column of a view, in order.
fn ids(db: &Db, sql: &str) -> Vec<i64> {
    let mut st = db.conn().prepare(sql).unwrap();
    st.query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect()
}

/// Makes items, each as (id, deliver and level, or none for a goal, cancelled), and dependencies, each as (item,
/// depends_on, kind).
fn items(db: &mut Db, items: &[(i64, bool, bool)], links: &[(i64, i64, &str)]) {
    db.write(Actor::Engine, |tx| {
        for (id, work, cancelled) in items {
            let (deliver, level) = if *work {
                (Some("change"), Some("1"))
            } else {
                (None, None)
            };
            tx.execute(
                "INSERT INTO items (id, project, title, problem, why, deliver, level, cancelled)
                 VALUES (?1, 'p', 't', 'x', 'y', ?2, ?3, ?4)",
                db::rusqlite::params![id, deliver, level, cancelled.then_some("not wanted")],
            )?;
        }
        for (item, on, kind) in links {
            db::add_dependency(tx, *item, *on, "what it needs", kind)?;
        }
        Ok(())
    })
    .unwrap();
}

/// Ends a work on each item at the end of the engineer's graph, which makes the item done.
fn done(db: &mut Db, items: &[i64]) {
    db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT OR IGNORE INTO flow_versions (id, digest, compiled) VALUES (1, 'd', '{}')",
            [],
        )?;
        for item in items {
            tx.execute(
                "INSERT INTO works (item, flow_version, graph, at, ended) VALUES (?1, 1, 'engineer', 'E1', 'done')",
                [item],
            )?;
        }
        Ok(())
    })
    .unwrap();
}

// A goal never starts, is open while anything it depends on is, ignores what is cancelled, and holds back an item that
// waits for it by a blocking dependency until it is done. Being open goes up only through goals: a goal that depends
// on a done item is done, whatever that item depended on.
#[test]
fn a_goal_is_done_once_everything_it_depends_on_is_and_no_work_starts_on_it() {
    let mut db = Db::in_memory().unwrap();
    // 1 is a goal over 2 (done), 3 (open) and 4 (cancelled); 5 is a goal over nothing; 6 is a goal over 7, done
    // though it depends on 8, which is open; 9 waits for goal 1 to be done before it starts; 10 is a goal over goal 1.
    items(
        &mut db,
        &[
            (1, false, false),
            (2, true, false),
            (3, true, false),
            (4, true, true),
            (5, false, false),
            (6, false, false),
            (7, true, false),
            (8, true, false),
            (9, true, false),
            (10, false, false),
        ],
        &[
            (1, 2, "blocking"),
            (1, 3, "parallel"),
            (1, 4, "blocking"),
            (6, 7, "blocking"),
            (7, 8, "blocking"),
            (9, 1, "blocking"),
            (10, 1, "blocking"),
        ],
    );
    done(&mut db, &[2, 7]);
    assert_eq!(ids(&db, "SELECT item FROM ready_items ORDER BY item"), [3, 8]);
    assert_eq!(ids(&db, "SELECT item FROM goals_open ORDER BY item"), [1, 5, 10]);
    assert_eq!(
        ids(&db, "SELECT item FROM open_items WHERE goal ORDER BY item"),
        [1, 5, 10]
    );
    // Goal 10 waits for goal 1 by a blocking dependency too, though it never starts either way.
    assert_eq!(
        ids(
            &db,
            "SELECT item FROM item_blocked_by WHERE depends_on = 1 ORDER BY item"
        ),
        [9, 10]
    );
    // Placed first in the start order, a goal that nothing holds back still does not start.
    db.write(Actor::Architect, |tx| {
        tx.execute("INSERT INTO start_order (item, place) VALUES (5, 1)", [])?;
        Ok(())
    })
    .unwrap();
    assert!(!ids(&db, "SELECT item FROM ready_items").contains(&5));

    done(&mut db, &[3]);
    assert_eq!(ids(&db, "SELECT item FROM goals_open ORDER BY item"), [5]);
    assert_eq!(
        ids(
            &db,
            "SELECT item FROM open_items WHERE item IN (1, 6, 10) ORDER BY item"
        ),
        Vec::<i64>::new()
    );
    assert_eq!(ids(&db, "SELECT item FROM ready_items ORDER BY item"), [8, 9]);
}

// A goal has neither a deliver nor a level, and an item with a work has both.
#[test]
fn a_goal_has_no_deliver_and_no_level() {
    let mut db = Db::in_memory().unwrap();
    for (deliver, level) in [(Some("change"), None), (None, Some("1"))] {
        let made = db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO items (project, title, problem, why, deliver, level) VALUES ('p', 't', 'x', 'y', ?1, ?2)",
                db::rusqlite::params![deliver, level],
            )?;
            Ok(())
        });
        assert!(
            made.is_err(),
            "an item with deliver {deliver:?} and level {level:?} was made"
        );
    }
}

/// A path under the temporary directory, made empty.
fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("wf-agents-db-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A database file just before the migration `GOALS`: made by the first version's schema and the migrations up to it,
/// with the foreign keys off as the migrations run, and the names of the migrations taken kept where the database
/// keeps them, so that opening it takes `GOALS` and those after it and no other.
fn database_before_goals(path: &std::path::Path) -> Connection {
    let at = MIGRATIONS.iter().position(|m| m.name == GOALS).unwrap();
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

/// Every column of the items table, in the order the rows below give them.
const COLUMNS: &str = "id, project, title, problem, why, approach, deliver, level, acceptance, acceptance_work, \
                       acceptance_round, cancelled, imported_from, imported_branch, imported_pr, wanted, \
                       decision_points";

// The items come out of the migrations as they went in, with their dependencies, and their place in the owner's queue,
// their rank and their hold taken into the start order by the migrations after this one; a goal
// can be made after it, and a change to an item is recorded in `changes`, which the migration must not have lost with
// the table it made afresh. Before it, an item without a deliver is refused.
#[test]
fn every_item_survives_the_migration_that_lets_an_item_be_a_goal() {
    let dir = temp("items-that-are-goals");
    let path = dir.join("db");
    {
        let conn = database_before_goals(&path);
        conn.execute(
            "INSERT INTO items (id, project, title, problem, why, approach, deliver, level, cancelled, imported_from,
                                imported_branch, imported_pr, wanted, held, rank, decision_points)
             VALUES (1, 'p', 'one', 'pro', 'why', 'how', 'change', 'l1', NULL, NULL, NULL, NULL, '[{\"id\":\"V1\"}]',
                     'held for now', 4, '[{\"id\":\"P1\"}]'),
                    (2, 'p', 'two', 'pro2', 'why2', NULL, 'report', 'l2', 'dup', 'old board', 'br', 9, NULL, NULL,
                     NULL, NULL)",
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
        let refused = conn.execute(
            "INSERT INTO items (project, title, problem, why, deliver, level) VALUES ('p', 'g', 'x', 'y', NULL, NULL)",
            [],
        );
        assert!(refused.is_err(), "the database is not the one before the migration");
    }
    let mut db = Db::open(&path).unwrap();
    let count = |db: &Db, sql: &str| -> i64 { db.conn().query_row(sql, [], |r| r.get(0)).unwrap() };
    assert_eq!(count(&db, "SELECT count(*) FROM pragma_foreign_key_check"), 0);
    let rows: Vec<String> = {
        let mut st = db
            .conn()
            .prepare(&format!("SELECT json_array({COLUMNS}) FROM items ORDER BY id"))
            .unwrap();
        st.query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect()
    };
    assert_eq!(
        rows,
        [
            r#"[1,"p","one","pro","why","how","change","l1",null,null,null,null,null,null,null,"[{\"id\":\"V1\"}]","[{\"id\":\"P1\"}]"]"#,
            r#"[2,"p","two","pro2","why2",null,"report","l2",null,null,null,"dup","old board","br",9,null,null]"#,
        ]
    );
    assert_eq!(
        count(
            &db,
            "SELECT count(*) FROM item_links l JOIN items a ON a.id = l.item JOIN items b ON b.id = l.depends_on
             WHERE l.item = 2 AND l.depends_on = 1"
        ),
        1
    );
    assert_eq!(
        count(
            &db,
            "SELECT count(*) FROM start_order WHERE (item, place, held) IN (VALUES (2, 1, 0), (1, 2, 1))"
        ),
        2
    );
    assert_eq!(count(&db, "SELECT count(*) FROM changes WHERE table_name = 'items'"), 0);
    db.write(Actor::Owner, |tx| {
        tx.execute(
            "INSERT INTO items (id, project, title, problem, why) VALUES (3, 'p', 'goal', 'x', 'y')",
            [],
        )?;
        db::add_dependency(tx, 3, 1, "its one", "blocking")?;
        Ok(())
    })
    .unwrap();
    assert_eq!(
        count(
            &db,
            "SELECT count(*) FROM changes WHERE table_name = 'items' AND row_key = json_object('id', 3)"
        ),
        1,
        "a goal made after the migration is not recorded"
    );
    assert_eq!(count(&db, "SELECT count(*) FROM goals_open WHERE item = 3"), 1);
    let _ = std::fs::remove_dir_all(&dir);
}
