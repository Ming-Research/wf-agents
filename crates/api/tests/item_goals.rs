//! A goal as the owner's page lists it: an item with no work of its own, with no deliver and no tier, new while anything
//! it depends on is open and done once all of it is, and never held back, since it never starts.

use api::{Flows, ItemState, ItemSummary};
use db::{Actor, Db};

/// A database holding the flow, a goal (1) over two items with a work each (2 and 3), and a cancelled goal (4).
fn world() -> Db {
    let compiled = flow::compile(
        flow::FLOW,
        &flow::Context {
            relations: db::relations(db::SCHEMA).unwrap(),
            tables: db::index_of(db::SCHEMA).unwrap(),
        },
    )
    .unwrap();
    let mut db = Db::in_memory().unwrap();
    db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO flow_versions (digest, compiled) VALUES ('d', ?1)",
            [&compiled.json()],
        )?;
        tx.execute(
            "INSERT INTO items (id, project, title, problem, why, deliver, level, cancelled)
             VALUES (1, 'p', 'Goal', 'x', 'y', NULL, NULL, NULL),
                    (2, 'p', 'Done', 'x', 'y', 'change', '1', NULL),
                    (3, 'p', 'Open', 'x', 'y', 'change', '1', NULL),
                    (4, 'p', 'Dropped', 'x', 'y', NULL, NULL, 'not wanted')",
            [],
        )?;
        db::add_dependency(tx, 1, 2, "its first part", "blocking")?;
        db::add_dependency(tx, 1, 3, "its second part", "blocking")?;
        tx.execute(
            "INSERT INTO works (item, flow_version, graph, at, ended) VALUES (2, 1, 'engineer', 'E1', 'done')",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    db
}

fn summary(db: &Db, id: i64) -> ItemSummary {
    let all = api::items(db.conn(), &Flows::default()).unwrap();
    all.items.into_iter().find(|i| i.id == id).unwrap()
}

#[test]
fn a_goal_is_new_until_what_it_depends_on_is_done_and_has_no_deliver_tier_or_blockers() {
    let mut db = world();
    let goal = summary(&db, 1);
    assert_eq!(goal.state, ItemState::New);
    assert_eq!((goal.deliver, goal.level), (None, None));
    assert_eq!(goal.blocked_by, Vec::<i64>::new());
    assert_eq!(summary(&db, 4).state, ItemState::Cancelled);
    db.write(Actor::Engine, |tx| {
        tx.execute(
            "INSERT INTO works (item, flow_version, graph, at, ended) VALUES (3, 1, 'engineer', 'E1', 'done')",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    assert_eq!(summary(&db, 1).state, ItemState::Done);
}
