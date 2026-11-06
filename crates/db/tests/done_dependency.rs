//! A dependency means the item depended on is done before the item depending on it can be, so a deliverable item that is
//! done does not come to depend on one that is not. A goal is done only while what it depends on is, so it may.

use db::rusqlite::params;
use db::{Actor, Db, Error};

/// Makes the items, each as (id, a deliverable item or a goal, cancelled).
fn items(db: &mut Db, items: &[(i64, bool, bool)]) {
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
                params![id, deliver, level, cancelled.then_some("not wanted")],
            )?;
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

fn depend(db: &mut Db, item: i64, on: i64) -> db::Result<()> {
    db.write(Actor::Owner, |tx| {
        db::add_dependency(tx, item, on, "its part", "blocking")
    })
}

fn links(db: &Db) -> i64 {
    db.conn()
        .query_row("SELECT count(*) FROM item_links", [], |r| r.get(0))
        .unwrap()
}

fn open_goals(db: &Db) -> Vec<i64> {
    let mut st = db.conn().prepare("SELECT item FROM goals_open ORDER BY item").unwrap();
    st.query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect()
}

// 1 is done; 2 is open; 3 is a goal over nothing, which is open; 4 is done too; 5 is cancelled.
fn setup() -> Db {
    let mut db = Db::in_memory().unwrap();
    items(
        &mut db,
        &[
            (1, true, false),
            (2, true, false),
            (3, false, false),
            (4, true, false),
            (5, true, true),
        ],
    );
    done(&mut db, &[1, 4]);
    db
}

#[test]
fn a_done_item_does_not_come_to_depend_on_an_open_item() {
    let mut db = setup();
    let e = depend(&mut db, 1, 2);
    assert!(
        matches!(e, Err(Error::DoneOverUnfinished { item: 1, depends_on: 2 })),
        "{e:?}"
    );
    let message = e.unwrap_err().to_string();
    assert!(message.contains("item 1 cannot depend on 2"), "{message}");
    assert!(message.contains("make a new item for what is left"), "{message}");
    assert_eq!(links(&db), 0);
}

#[test]
fn a_done_item_does_not_come_to_depend_on_an_open_goal() {
    let mut db = setup();
    let e = depend(&mut db, 1, 3);
    assert!(
        matches!(e, Err(Error::DoneOverUnfinished { item: 1, depends_on: 3 })),
        "{e:?}"
    );
    assert_eq!(links(&db), 0);
}

#[test]
fn a_done_item_may_depend_on_what_is_done_or_cancelled() {
    let mut db = setup();
    depend(&mut db, 1, 4).unwrap();
    depend(&mut db, 1, 5).unwrap();
    assert_eq!(links(&db), 2);
}

// A goal over done items is done; depending on an open item opens it again, and it is no fault.
#[test]
fn a_goal_may_depend_on_an_open_item_and_is_open_again() {
    let mut db = setup();
    depend(&mut db, 3, 4).unwrap();
    assert_eq!(open_goals(&db), Vec::<i64>::new());
    depend(&mut db, 3, 2).unwrap();
    assert_eq!(open_goals(&db), [3]);
}

// An open item may depend on anything: nothing is done above it yet.
#[test]
fn an_open_item_may_depend_on_an_open_item() {
    let mut db = setup();
    depend(&mut db, 2, 3).unwrap();
    assert_eq!(links(&db), 1);
}

// A goal is done through what it depends on, not by a work, so the rule leaves it alone.
#[test]
fn a_goal_that_is_done_may_depend_on_an_open_goal() {
    let mut db = setup();
    items(&mut db, &[(6, false, false), (7, false, false)]);
    depend(&mut db, 6, 1).unwrap();
    assert_eq!(open_goals(&db), [3, 7]);
    depend(&mut db, 6, 7).unwrap();
    assert_eq!(open_goals(&db), [3, 6, 7]);
}
