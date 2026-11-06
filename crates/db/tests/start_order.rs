//! The architect's start order (`start_order`), as the views the engine starts works by and the agents read see it: the
//! order items start in, what the plan holds back, the item whose work was taken over going first, and what a work
//! going on on an open item does.

use db::rusqlite::params;
use db::{Actor, Db};

/// The items of one column of a view, in order.
fn ids(db: &Db, sql: &str) -> Vec<i64> {
    let mut st = db.conn().prepare(sql).unwrap();
    st.query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect()
}

/// Makes items 1 to `n` of project `p`, each with a work of its own to do.
fn items(db: &mut Db, n: i64) {
    db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO flow_versions (id, digest, compiled) VALUES (1, 'd', '{}')",
            [],
        )?;
        for id in 1..=n {
            tx.execute(
                "INSERT INTO items (id, project, title, problem, why, deliver, level)
                 VALUES (?1, 'p', 't', 'x', 'y', 'change', '1')",
                [id],
            )?;
        }
        Ok(())
    })
    .unwrap();
}

// Items start in the start order, lower places first and those placed nowhere after, in the order made; what the plan
// holds back does not start; and an item whose last work was taken over, stopped for a new work to take it on, starts
// before them all, as it waits only for a slot. An item whose work reached the end is done and does not start again.
#[test]
fn items_start_in_the_start_order_the_one_taken_over_first() {
    let mut db = Db::in_memory().unwrap();
    items(&mut db, 6);
    db.write(Actor::Architect, |tx| {
        for (item, place, held) in [(4, Some(1), 0), (2, Some(2), 0), (3, Some(3), 1), (5, None, 1)] {
            tx.execute(
                "INSERT INTO start_order (item, place, held) VALUES (?1, ?2, ?3)",
                params![item, place, held],
            )?;
        }
        Ok(())
    })
    .unwrap();
    assert_eq!(ids(&db, "SELECT item FROM ready_items"), [4, 2, 1, 6]);
    db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO works (item, flow_version, graph, at, ended) VALUES (6, 1, 'engineer', 'I1', 'stopped: started over')",
            [],
        )?;
        tx.execute(
            "INSERT INTO works (item, flow_version, graph, at, ended) VALUES (1, 1, 'engineer', 'E1', 'done')",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    assert_eq!(ids(&db, "SELECT item FROM ready_items"), [6, 4, 2]);
    assert_eq!(ids(&db, "SELECT item FROM ready_items WHERE taken_over"), [6]);
    assert_eq!(ids(&db, "SELECT item FROM open_items WHERE held ORDER BY item"), [3, 5]);
    // A place or a hold is the plan's: neither changes the item, nor the version a change of the item is written on.
    assert_eq!(
        ids(
            &db,
            "SELECT count(*) FROM changes WHERE table_name = 'items' AND old_row IS NOT NULL"
        ),
        [0]
    );
    // A row neither places nor holds an item is no row of the plan.
    let empty = db.write(Actor::Architect, |tx| {
        tx.execute("INSERT INTO start_order (item, place, held) VALUES (6, NULL, 0)", [])?;
        Ok(())
    });
    assert!(empty.is_err());
}

// What a work going on on an open item does is its own column: waiting for the holder of its blocker, waiting for a
// free slot once what it waited for came, or running.
#[test]
fn an_open_item_says_what_its_work_does() {
    let mut db = Db::in_memory().unwrap();
    items(&mut db, 3);
    db.write(Actor::Engine, |tx| {
        for item in 1..=3 {
            tx.execute(
                "INSERT INTO works (id, item, flow_version, graph, at) VALUES (?1, ?1, 1, 'engineer', 'A2')",
                [item],
            )?;
        }
        tx.execute(
            "INSERT INTO blockers (work, cell, kind, what, holder, project, at, since)
             VALUES (1, 'A2', 'question', 'asked', 'architect', 'p', 1, 1)",
            [],
        )?;
        tx.execute("INSERT INTO slot_waits (work, since) VALUES (2, 1)", [])?;
        Ok(())
    })
    .unwrap();
    let states: Vec<String> = {
        let mut st = db
            .conn()
            .prepare("SELECT work_state FROM open_items ORDER BY item")
            .unwrap();
        st.query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect()
    };
    assert_eq!(states, ["waiting", "waiting_for_slot", "running"]);
}
