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

/// Puts a work on the item at a cell of the engineer's graph, not ended: `stopped` the cell it stopped at, and `waiting`
/// the cells that wait for its answer, as the engine keeps them.
fn work_at(db: &mut Db, item: i64, at: &str, stopped: Option<&str>, waiting: &str) {
    db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT OR IGNORE INTO flow_versions (id, digest, compiled) VALUES (1, 'd', '{}')",
            [],
        )?;
        tx.execute(
            "INSERT INTO works (item, flow_version, graph, at, resume_at, waiting)
             VALUES (?1, 1, 'engineer', ?2, ?3, ?4)",
            params![item, at, stopped, waiting],
        )?;
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

// Past the merge gate nothing waits for an item the work's item comes to depend on: the work merges, or has merged, and
// goes to the end without the gate. At the merge itself the item is as done.
#[test]
fn an_item_at_the_merge_does_not_come_to_depend_on_an_open_item() {
    let mut db = setup();
    items(&mut db, &[(6, true, false)]);
    work_at(&mut db, 6, "M2", None, "[]");
    let e = depend(&mut db, 6, 2);
    assert!(
        matches!(e, Err(Error::DoneOverUnfinished { item: 6, depends_on: 2 })),
        "{e:?}"
    );
    let message = e.unwrap_err().to_string();
    assert!(message.contains("past the merge gate"), "{message}");
    assert!(message.contains("make a new item for what is left"), "{message}");
    assert_eq!(links(&db), 0);
}

// At a cell that asks for the merge, or brings the work to a new flow version to go back to it, the work merges
// without passing the gate again, as at the merge itself: the item is as done.
#[test]
fn an_item_whose_work_asks_from_the_merge_does_not_come_to_depend_on_an_open_item() {
    let mut db = setup();
    let places = [
        // Asking the architect when the merge's step did not run, and brought to a new version to merge again.
        (6, "A2", Some("M2"), "[\"M2\"]"),
        (7, "Y1", Some("M2"), "[]"),
    ];
    for (item, at, stopped, waiting) in places {
        items(&mut db, &[(item, true, false)]);
        work_at(&mut db, item, at, stopped, waiting);
        let e = depend(&mut db, item, 2);
        assert!(
            matches!(e, Err(Error::DoneOverUnfinished { depends_on: 2, .. })),
            "{at}: {e:?}"
        );
    }
    assert_eq!(links(&db), 0);
}

// A merge that fails sends the work back to the fix, and through the gate again, which waits for what its item comes to
// depend on: at the fix, at a question asked there and at the gate, the item may depend on an open item. So may one
// whose work at the merge was stopped, which ends with its item not done.
#[test]
fn an_item_sent_back_from_the_merge_may_depend_on_an_open_item() {
    let mut db = setup();
    let places = [
        (6, "T6", None, "[]"),
        (7, "A1", Some("T6"), "[\"T6\"]"),
        (8, "M1", None, "[]"),
        (9, "M2", None, "[]"),
    ];
    for (item, at, stopped, waiting) in places {
        items(&mut db, &[(item, true, false)]);
        work_at(&mut db, item, at, stopped, waiting);
    }
    db.write(Actor::Engine, |tx| {
        tx.execute("UPDATE works SET ended = 'started over' WHERE item = 9", [])?;
        Ok(())
    })
    .unwrap();
    for (item, ..) in places {
        depend(&mut db, item, 2).unwrap_or_else(|e| panic!("{item}: {e}"));
    }
    assert_eq!(links(&db), 4);
}

// At the merge, as when done, the item may depend on what is done or cancelled.
#[test]
fn an_item_at_the_merge_may_depend_on_what_is_done_or_cancelled() {
    let mut db = setup();
    items(&mut db, &[(6, true, false)]);
    work_at(&mut db, 6, "M2", None, "[]");
    depend(&mut db, 6, 4).unwrap();
    depend(&mut db, 6, 5).unwrap();
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
