//! The changes made to items after they were added, each kept as a row of its own after the item's earlier ones with
//! why it was made, changes made together with the works they grew kept all or none, and read back as the items as
//! they stand with every change they went through; and the reasons of a cancel and of a reopen, kept with them.

use rusqlite::params;
use store::{Change, Fault, ItemChange, ItemNumber, SCHEMA_VERSION};

use super::opening::{MIGRATIONS, SCHEMA_1, versions};
use super::*;

#[test]
fn each_change_of_an_item_is_kept_in_order_with_why_and_the_item_is_read_as_they_leave_it() {
    let mut store = Store::in_memory().expect("the store opens");
    let nothing = Work::default();
    store
        .add(&first("first", "2", &[]), "judged", Time(10), &nothing)
        .unwrap();
    store
        .add(&first("second", "2", &[ItemNumber(1)]), "judged", Time(20), &nothing)
        .unwrap();
    store
        .add(&first("third", "1", &[]), "judged", Time(30), &nothing)
        .unwrap();
    let made = [
        (ItemNumber(2), ItemChange::Title("the second, narrowed".into()), 40),
        (ItemNumber(2), ItemChange::DependsOn(ItemNumber(3)), 50),
        (ItemNumber(2), ItemChange::NoLongerDependsOn(ItemNumber(1)), 60),
        (ItemNumber(2), ItemChange::Level("3".into()), 70),
        (ItemNumber(2), ItemChange::Tagged("parser".into()), 70),
        (ItemNumber(2), ItemChange::Tagged("page".into()), 70),
        (ItemNumber(2), ItemChange::Untagged("page".into()), 75),
        (ItemNumber(1), ItemChange::DependsOn(ItemNumber(3)), 80),
    ];
    for (item, change, at) in &made {
        let why = format!("why {at}");
        (store.change(Time(*at), Some(&why), &[(*item, change.clone())], &[])).expect("the change is kept");
    }
    let items = store.items().expect("the items are read");
    let second = &items[1];
    assert_eq!(second.title, "the second, narrowed");
    assert_eq!(second.level.as_deref(), Some("3"));
    assert_eq!(second.depends_on, [ItemNumber(3)]);
    assert_eq!(second.tags, ["parser"]);
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
            why: Some(format!("why {at}")),
        })
        .collect();
    assert_eq!(second.changes, kept);
    // An item may come to depend on one added after it.
    assert_eq!(items[0].depends_on, [ItemNumber(3)]);
    assert_eq!(items[2].changes, vec![]);
}

#[test]
fn an_item_is_added_with_its_tags_and_read_back_with_them_in_order() {
    let mut store = Store::in_memory().expect("the store opens");
    let mut tagged = first("tagged", "2", &[]);
    tagged.tags = vec!["page".into(), "engine".into()];
    store.add(&tagged, "judged", Time(10), &Work::default()).unwrap();
    let items = store.items().expect("the items are read");
    assert_eq!(items[0].tags, ["engine", "page"]);
    assert_eq!(items[0].first.tags, ["engine", "page"]);
}

#[test]
fn changes_made_together_are_kept_with_the_works_they_grew_or_none_of_them() {
    let db = Db::new();
    let mut store = db.open();
    let work = first_work(&mut store);
    store
        .add(&first("second", "2", &[]), "judged", Time(START), &Work::default())
        .unwrap();
    let before = store.items().unwrap();
    let mut d = Driver::new();
    d.on(Event::Cancelled {
        why: Some("joined into item 2".into()),
    });
    // A change onto an item the store does not have refuses all of it: the change before it and the work's cancel too.
    let changes = [
        (ItemNumber(2), ItemChange::Title("kept?".into())),
        (ItemNumber(2), ItemChange::DependsOn(ItemNumber(9))),
    ];
    assert_eq!(
        store.change(Time(50), Some("together"), &changes, &[(work, &d.work)]),
        Err(Fault::NoSuchDependency(ItemNumber(9)))
    );
    assert_eq!(store.items().unwrap(), before);
    assert_eq!(db.open().load(work).unwrap(), Work::default());
    // Given whole, all of it is kept at once.
    let changes = [(ItemNumber(2), ItemChange::Title("kept".into()))];
    (store.change(Time(60), Some("together"), &changes, &[(work, &d.work)])).expect("the changes are kept");
    assert_eq!(store.items().unwrap()[1].title, "kept");
    assert_eq!(db.open().load(work).unwrap(), d.work);
}

#[test]
fn a_change_of_an_item_or_onto_an_item_the_store_does_not_have_is_refused_and_nothing_is_kept() {
    let mut store = Store::in_memory().expect("the store opens");
    store
        .add(&first("first", "2", &[]), "judged", Time(10), &Work::default())
        .unwrap();
    let before = store.items().unwrap();
    let change = |item: u64, change: ItemChange| [(ItemNumber(item), change)];
    assert_eq!(
        store.change(Time(20), Some("x"), &change(4, ItemChange::Title("x".into())), &[]),
        Err(Fault::NoSuchItem(ItemNumber(4)))
    );
    assert_eq!(
        store.change(
            Time(20),
            Some("x"),
            &change(1, ItemChange::DependsOn(ItemNumber(4))),
            &[]
        ),
        Err(Fault::NoSuchDependency(ItemNumber(4)))
    );
    assert_eq!(store.items().unwrap(), before);
}

#[test]
fn a_cancel_and_a_reopen_keep_their_reasons_and_a_work_is_cancelled_and_reopened_again() {
    let db = Db::new();
    let mut store = db.open();
    let work = first_work(&mut store);
    let mut d = Driver::new();
    d.on(Event::Cancelled {
        why: Some("joined into item 4".into()),
    });
    d.on(Event::Reopened {
        why: "item 4 was too big".into(),
    });
    d.on(Event::Cancelled { why: None });
    store.save(work, &d.work).expect("the work is saved");
    let loaded = db.open().load(work).expect("the work loads");
    assert_eq!(loaded.records.cancels.len(), 2);
    assert_eq!(loaded.records.reopens.len(), 1);
    assert_eq!(
        loaded.why_cancelled.values().collect::<Vec<_>>(),
        ["joined into item 4"]
    );
    assert_eq!(loaded.why_reopened.values().collect::<Vec<_>>(), ["item 4 was too big"]);
    assert_eq!(loaded, d.work);
    // The same cancel with another reason is not what the store holds.
    let mut other = d.work.clone();
    let first = other.records.cancels[0];
    other.why_cancelled.insert(first, "no longer wanted".into());
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
    assert_eq!(items[0].tags, Vec::<String>::new());
    assert_eq!(
        items[0].changes,
        [
            Change {
                at: Time(6),
                what: ItemChange::Level("2".into()),
                why: None,
            },
            Change {
                at: Time(7),
                what: ItemChange::Level("3".into()),
                why: None,
            },
        ]
    );
    let work = store.load(WorkNumber(1)).expect("the work loads");
    assert!(work.records.cancelled().is_some());
    assert!(work.why_cancelled.is_empty());
}

#[test]
fn a_file_of_version_10_keeps_its_cancel_its_release_and_its_changes_with_no_reason() {
    let db = Db::new();
    {
        let file = Connection::open(&db.path).expect("the database opens");
        file.execute_batch(SCHEMA_1).expect("the first schema is made");
        file.execute("INSERT INTO schema_version (version) VALUES (1)", [])
            .expect("its version is written");
        for migration in &MIGRATIONS[..9] {
            file.execute_batch(migration).expect("the migration runs");
        }
        assert_eq!(versions(&db.path), [10]);
        file.execute("INSERT INTO items (title, added, level) VALUES ('kept', 5, '1')", [])
            .expect("an item is kept");
        file.execute(
            "INSERT INTO item_changes (item, n, at, change) VALUES (1, 1, 6, '{\"Title\":\"renamed\"}')",
            [],
        )
        .expect("a change is kept");
        file.execute("INSERT INTO works (item, flow, added) VALUES (1, 'judged', 5)", [])
            .expect("its work is kept");
        file.execute("INSERT INTO cancelled (work, at, why) VALUES (1, 1, 'joined')", [])
            .expect("its cancel is kept");
        file.execute("INSERT INTO released (work, at) VALUES (1, 2)", [])
            .expect("its release is kept");
        file.execute("INSERT INTO moments (work, place, time) VALUES (1, 1, 8)", [])
            .expect("the moment is kept");
    }
    let store = db.open();
    assert_eq!(versions(&db.path), [SCHEMA_VERSION]);
    let items = store.items().expect("the items are read");
    assert_eq!(items[0].title, "renamed");
    assert_eq!(items[0].changes[0].why, None);
    let work = store.load(WorkNumber(1)).expect("the work loads");
    assert_eq!(work.records.cancels, [Seq(1)]);
    assert_eq!(work.records.releases, [Seq(2)]);
    assert!(work.records.reopens.is_empty());
    assert_eq!(work.why_cancelled.get(&Seq(1)).map(String::as_str), Some("joined"));
}
