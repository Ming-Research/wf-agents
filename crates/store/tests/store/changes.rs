//! The changes made to an item after it was added, each kept as a row of its own after the item's earlier ones, and
//! read back as the item as it stands with every change it went through; and the reason a cancel gave, kept with it.

use rusqlite::params;
use store::{Change, Fault, ItemChange, ItemNumber, SCHEMA_VERSION};

use super::opening::{MIGRATIONS, SCHEMA_1, versions};
use super::*;

#[test]
fn each_change_of_an_item_is_kept_in_order_and_the_item_is_read_as_they_leave_it() {
    let mut store = Store::in_memory().expect("the store opens");
    let nothing = Work::default();
    store.add("first", "2", &[], "judged", Time(10), &nothing).unwrap();
    store
        .add("second", "2", &[ItemNumber(1)], "judged", Time(20), &nothing)
        .unwrap();
    store.add("third", "1", &[], "judged", Time(30), &nothing).unwrap();
    let made = [
        (ItemNumber(2), ItemChange::Title("the second, narrowed".into()), 40),
        (ItemNumber(2), ItemChange::DependsOn(ItemNumber(3)), 50),
        (ItemNumber(2), ItemChange::NoLongerDependsOn(ItemNumber(1)), 60),
        (ItemNumber(2), ItemChange::Level("3".into()), 70),
        (ItemNumber(1), ItemChange::DependsOn(ItemNumber(3)), 80),
    ];
    for (item, change, at) in &made {
        store.change(*item, change, Time(*at)).expect("the change is kept");
    }
    let items = store.items().expect("the items are read");
    let second = &items[1];
    assert_eq!(second.title, "the second, narrowed");
    assert_eq!(second.level.as_deref(), Some("3"));
    assert_eq!(second.depends_on, [ItemNumber(3)]);
    assert_eq!(
        (second.first.title.as_str(), second.first.depends_on.as_slice()),
        ("second", [ItemNumber(1)].as_slice()),
        "what it was added as is kept"
    );
    let kept: Vec<Change> = (made.iter())
        .filter(|(item, ..)| *item == ItemNumber(2))
        .map(|(_, what, at)| Change {
            at: Time(*at),
            what: what.clone(),
        })
        .collect();
    assert_eq!(second.changes, kept);
    // An item may come to depend on one added after it.
    assert_eq!(items[0].depends_on, [ItemNumber(3)]);
    assert_eq!(items[2].changes, vec![]);
}

#[test]
fn a_change_of_an_item_or_onto_an_item_the_store_does_not_have_is_refused_and_nothing_is_kept() {
    let mut store = Store::in_memory().expect("the store opens");
    store
        .add("first", "2", &[], "judged", Time(10), &Work::default())
        .unwrap();
    let before = store.items().unwrap();
    assert_eq!(
        store.change(ItemNumber(4), &ItemChange::Title("x".into()), Time(20)),
        Err(Fault::NoSuchItem(ItemNumber(4)))
    );
    assert_eq!(
        store.change(ItemNumber(1), &ItemChange::DependsOn(ItemNumber(4)), Time(20)),
        Err(Fault::NoSuchDependency(ItemNumber(4)))
    );
    assert_eq!(store.items().unwrap(), before);
}

#[test]
fn a_cancel_keeps_its_reason_with_it() {
    let db = Db::new();
    let mut store = db.open();
    let work = first_work(&mut store);
    let mut d = Driver::new();
    d.on(Event::Cancelled {
        why: Some("joined into item 4".into()),
    });
    store.save(work, &d.work).expect("the work is saved");
    let loaded = db.open().load(work).expect("the work loads");
    assert_eq!(loaded.why_cancelled.as_deref(), Some("joined into item 4"));
    assert_eq!(loaded, d.work);
    // The same cancel with another reason is not what the store holds.
    let mut other = d.work.clone();
    other.why_cancelled = Some("no longer wanted".into());
    assert!(matches!(
        store.save(work, &other),
        Err(Fault::Diverged { table: "cancelled", .. })
    ));
}

#[test]
fn a_file_of_version_6_keeps_each_level_change_as_a_change_and_its_cancels_with_no_reason() {
    let db = Db::new();
    {
        let file = Connection::open(&db.path).expect("the database opens");
        file.execute_batch(SCHEMA_1).expect("the first schema is made");
        file.execute("INSERT INTO schema_version (version) VALUES (1)", [])
            .expect("its version is written");
        for migration in &MIGRATIONS[..5] {
            file.execute_batch(migration).expect("the migration runs");
        }
        assert_eq!(versions(&db.path), [6]);
        file.execute("INSERT INTO items (title, added, level) VALUES ('kept', 5, '1')", [])
            .expect("an item is kept");
        for (n, level, at) in [(1, "2", 6), (2, "3", 7)] {
            file.execute(
                "INSERT INTO level_changes (item, n, level, at) VALUES (1, ?1, ?2, ?3)",
                params![n, level, at],
            )
            .expect("a level change is kept");
        }
        file.execute("INSERT INTO works (item, flow, added) VALUES (1, 'judged', 5)", [])
            .expect("its work is kept");
        file.execute("INSERT INTO cancelled (work, at) VALUES (1, 1)", [])
            .expect("its cancel is kept");
        file.execute("INSERT INTO moments (work, place, time) VALUES (1, 1, 8)", [])
            .expect("the moment of its cancel is kept");
    }
    let store = db.open();
    assert_eq!(versions(&db.path), [SCHEMA_VERSION]);
    let items = store.items().expect("the items are read");
    assert_eq!(items[0].level.as_deref(), Some("3"));
    assert_eq!(
        items[0].changes,
        [
            Change {
                at: Time(6),
                what: ItemChange::Level("2".into()),
            },
            Change {
                at: Time(7),
                what: ItemChange::Level("3".into()),
            },
        ]
    );
    let work = store.load(WorkNumber(1)).expect("the work loads");
    assert!(work.records.cancelled.is_some());
    assert_eq!(work.why_cancelled, None);
}
