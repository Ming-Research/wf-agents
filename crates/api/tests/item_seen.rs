//! An item as the owner's page lists it is `unseen` while it is new to the owner: made, or its content changed, since
//! the owner last opened it. The owner marks an item seen when they open it, as the owner, and the page's live updates
//! name the item so that the list it holds is read again.

use api::{Flows, ItemState};
use db::rusqlite::Connection;
use db::{Actor, Changed, Db};

/// A database holding the items 1 and 2, neither seen.
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

/// Whether the list of items shows each of the items 1 and 2 as new to the owner.
fn unseen(conn: &Connection) -> (bool, bool) {
    let all = api::items(conn, &Flows::default()).unwrap().items;
    let of = |id| all.iter().find(|i| i.id == id).unwrap().unseen;
    (of(1), of(2))
}

#[test]
fn an_item_is_unseen_until_the_owner_marks_it_seen_and_again_after_its_content_changes() {
    let mut db = world();
    assert_eq!(unseen(db.conn()), (true, true), "a new item is unseen");

    api::mark_item_seen(&mut db, 1).unwrap();
    assert_eq!(unseen(db.conn()), (false, true), "only the item marked is seen");
    api::mark_item_seen(&mut db, 1).unwrap();
    assert_eq!(unseen(db.conn()), (false, true), "marking it again changes nothing");

    // What the engine and the architect set apart from its content does not make it new.
    db.write(Actor::Engine, |tx| {
        tx.execute(
            "UPDATE items SET level = '2', cancelled = 'not wanted' WHERE id = 1",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    let all = api::items(db.conn(), &Flows::default()).unwrap().items;
    assert_eq!(all[0].state, ItemState::Cancelled);
    assert!(!all[0].unseen);

    // A change to what its creator wrote does.
    db.write(Actor::Engine, |tx| {
        tx.execute("UPDATE items SET problem = 'z' WHERE id = 1", [])?;
        Ok(())
    })
    .unwrap();
    assert_eq!(unseen(db.conn()), (true, true));
    api::mark_item_seen(&mut db, 1).unwrap();
    api::mark_item_seen(&mut db, 2).unwrap();
    assert_eq!(unseen(db.conn()), (false, false));

    // The item in full carries the same mark as its line in the list.
    db.write(Actor::Engine, |tx| {
        tx.execute("UPDATE items SET why = 'w' WHERE id = 2", [])?;
        Ok(())
    })
    .unwrap();
    let detail = api::item(db.conn(), &Flows::default(), 2).unwrap();
    assert!(detail.item.unseen);
}

#[test]
fn marking_an_item_seen_is_the_owners_write_and_names_the_item_to_read_again() {
    let mut db = world();
    let done = api::mark_item_seen(&mut db, 2).unwrap();
    assert_eq!(done, "item 2 is seen");
    let actor: String = db
        .conn()
        .query_row(
            "SELECT actor FROM changes WHERE table_name = 'owner_seen' AND row_key = json_object('item', 2)",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(actor, "owner");
    let touched = [Changed {
        table: "owner_seen".to_string(),
        rowid: 2,
    }];
    assert_eq!(
        api::events(db.conn(), &touched),
        [api::Event::Item { item: Some(2) }],
        "the page reads the item and the list again"
    );
}

#[test]
fn an_item_that_does_not_exist_cannot_be_marked_seen() {
    let mut db = world();
    let refused = api::mark_item_seen(&mut db, 9).unwrap_err();
    assert_eq!(refused, "no item 9");
    let rows: i64 = db
        .conn()
        .query_row("SELECT count(*) FROM owner_seen", [], |r| r.get(0))
        .unwrap();
    assert_eq!(rows, 0);
}
