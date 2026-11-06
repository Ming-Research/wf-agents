//! The main dependency of an item, the one it stands under in the tree of items: an item others depend on has exactly
//! one at all times, kept by the database whatever writes the dependencies, and the migration that brings it in gives
//! one to each item a database already has dependencies on.

use std::path::PathBuf;

use db::rusqlite::Connection;
use db::{Actor, Db, MIGRATIONS};

const MAIN: &str = "202611060230-a-main-dependency-for-each-item";

/// The items whose dependency on `on` is its main one.
fn mains(db: &Db, on: i64) -> Vec<i64> {
    let mut st = db
        .conn()
        .prepare("SELECT item FROM item_links WHERE depends_on = ?1 AND main = 1 ORDER BY item")
        .unwrap();
    st.query_map([on], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect()
}

/// Holds that every item others depend on has exactly one main dependency.
fn exactly_one_everywhere(db: &Db, after: &str) {
    let mut st = db
        .conn()
        .prepare("SELECT depends_on, sum(main) FROM item_links GROUP BY depends_on HAVING sum(main) <> 1")
        .unwrap();
    let wrong: Vec<(i64, i64)> = st
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(|x| x.unwrap())
        .collect();
    assert!(
        wrong.is_empty(),
        "after {after}, items with other than one main dependency (item, mains): {wrong:?}"
    );
}

/// Runs `sql` as one write, and holds the rule after it, whether the write went through or not.
fn write(db: &mut Db, sql: &str) -> bool {
    let done = db
        .write(Actor::Engine, |tx| {
            tx.execute_batch(sql)?;
            Ok(())
        })
        .is_ok();
    exactly_one_everywhere(db, sql);
    done
}

// Every way a dependency is made, changed or removed leaves each item others depend on with exactly one main
// dependency: the first made; one made or set main in place of the one before; not unset while it stands; the earliest
// made of those left when it goes, also when one statement removes it and the one that took its place, and when an
// item is cancelled and all its dependencies go at once. An item nothing depends on any more has none.
#[test]
fn an_item_others_depend_on_has_exactly_one_main_dependency_whatever_is_written() {
    let mut db = Db::in_memory().unwrap();
    let items: String = (1..=9)
        .map(|id| format!("INSERT INTO items (id, project, title, problem, why) VALUES ({id}, 'p', 't', 'x', 'y');"))
        .collect();
    assert!(write(&mut db, &items));
    assert!(write(
        &mut db,
        "INSERT INTO item_links (item, depends_on, needs, kind) VALUES (3, 1, 'n', 'blocking');
         INSERT INTO item_links (item, depends_on, needs, kind) VALUES (2, 1, 'n', 'blocking');
         INSERT INTO item_links (item, depends_on, needs, kind) VALUES (4, 1, 'n', 'blocking');"
    ));
    assert_eq!(mains(&db, 1), [3], "the first made is the main one");

    assert!(write(
        &mut db,
        "DELETE FROM item_links WHERE item = 2 AND depends_on = 1"
    ));
    assert_eq!(mains(&db, 1), [3], "removing another leaves the main one");
    assert!(write(
        &mut db,
        "INSERT INTO item_links (item, depends_on, needs, kind) VALUES (2, 1, 'n', 'blocking')"
    ));
    assert_eq!(mains(&db, 1), [3], "one made later is not main");

    assert!(write(
        &mut db,
        "UPDATE item_links SET main = 1 WHERE item = 4 AND depends_on = 1"
    ));
    assert_eq!(mains(&db, 1), [4], "one set main takes the place of the one before");
    assert!(!write(
        &mut db,
        "UPDATE item_links SET main = 0 WHERE item = 4 AND depends_on = 1"
    ));
    assert_eq!(mains(&db, 1), [4], "the main one is not unset");
    assert!(!write(&mut db, "UPDATE item_links SET main = 0 WHERE depends_on = 1"));
    assert_eq!(mains(&db, 1), [4], "nor all at once");

    assert!(write(
        &mut db,
        "INSERT INTO item_links (item, depends_on, needs, kind, main) VALUES (5, 1, 'n', 'blocking', 1)"
    ));
    assert_eq!(mains(&db, 1), [5], "one made main takes the place of the one before");

    assert!(write(
        &mut db,
        "DELETE FROM item_links WHERE item = 5 AND depends_on = 1"
    ));
    assert_eq!(
        mains(&db, 1),
        [3],
        "the earliest made of those left takes the place of one that goes"
    );

    assert!(write(
        &mut db,
        "DELETE FROM item_links WHERE depends_on = 1 AND item IN (3, 2)"
    ));
    assert_eq!(mains(&db, 1), [4], "and so on, when one statement removes several");

    // 6 depends on 7 and 8 as their main dependency, and 9 depends on them too; 6 is cancelled, which removes every
    // dependency from it and on it at once.
    assert!(write(
        &mut db,
        "INSERT INTO item_links (item, depends_on, needs, kind) VALUES (6, 7, 'n', 'blocking');
         INSERT INTO item_links (item, depends_on, needs, kind) VALUES (6, 8, 'n', 'blocking');
         INSERT INTO item_links (item, depends_on, needs, kind) VALUES (9, 7, 'n', 'parallel');
         INSERT INTO item_links (item, depends_on, needs, kind) VALUES (9, 8, 'n', 'parallel');
         INSERT INTO item_links (item, depends_on, needs, kind) VALUES (4, 6, 'n', 'blocking');"
    ));
    assert_eq!((mains(&db, 7), mains(&db, 8)), (vec![6], vec![6]));
    assert!(write(
        &mut db,
        "DELETE FROM item_links WHERE item = 6 OR depends_on = 6"
    ));
    assert_eq!(
        (mains(&db, 7), mains(&db, 8), mains(&db, 6)),
        (vec![9], vec![9], vec![])
    );

    assert!(write(&mut db, "DELETE FROM item_links WHERE depends_on = 1"));
    assert_eq!(mains(&db, 1), Vec::<i64>::new(), "an item nothing depends on has none");
}

/// A path under the temporary directory, made empty.
fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("wf-agents-db-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A database file just before the migration `MAIN`, made by the first version's schema and the migrations up to it,
/// with the names of the migrations taken kept where the database keeps them.
fn database_before_main(path: &std::path::Path) -> Connection {
    let at = MIGRATIONS.iter().position(|m| m.name == MAIN).unwrap();
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

// Each item a database already has dependencies on gets the earliest made of them as its main one, and the database
// keeps exactly one from then on.
#[test]
fn the_migration_gives_each_item_depended_on_its_earliest_dependency_as_the_main_one() {
    let dir = temp("main-dependency");
    let path = dir.join("db");
    {
        let conn = database_before_main(&path);
        conn.execute(
            "INSERT INTO items (id, project, title, problem, why, deliver, level)
             VALUES (1, 'p', 't', 'x', 'y', 'change', '1'), (2, 'p', 't', 'x', 'y', 'change', '1'),
                    (3, 'p', 't', 'x', 'y', 'change', '1'), (4, 'p', 't', 'x', 'y', 'change', '1')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO item_links (item, depends_on, needs, kind)
             VALUES (3, 1, 'n', 'blocking'), (2, 1, 'n', 'blocking'), (2, 4, 'n', 'parallel')",
            [],
        )
        .unwrap();
    }
    let mut db = Db::open(&path).unwrap();
    exactly_one_everywhere(&db, "the migration");
    assert_eq!(mains(&db, 1), [3]);
    assert_eq!(mains(&db, 4), [2]);
    assert!(write(
        &mut db,
        "DELETE FROM item_links WHERE item = 3 AND depends_on = 1"
    ));
    assert_eq!(mains(&db, 1), [2]);
    let _ = std::fs::remove_dir_all(&dir);
}
