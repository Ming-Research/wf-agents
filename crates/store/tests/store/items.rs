//! The items, numbered by the store in the order they are added, each depending only on items added before it, and
//! each added together with the work done for it and what is kept of that work from its start.

use decide::{Pause, Seq};
use store::{Fault, Item, ItemNumber, WorkHead};

use super::*;

fn item(number: u64, title: &str, added: u64, depends_on: &[u64]) -> Item {
    Item {
        number: ItemNumber(number),
        title: title.to_string(),
        level: Some("2".to_string()),
        added: Time(added),
        depends_on: depends_on.iter().map(|n| ItemNumber(*n)).collect(),
    }
}

fn head(number: u64, item: u64, flow: &str, added: u64) -> WorkHead {
    WorkHead {
        number: WorkNumber(number),
        item: ItemNumber(item),
        flow: flow.to_string(),
        added: Time(added),
    }
}

#[test]
fn items_and_their_works_are_numbered_from_one_in_the_order_they_are_added_with_what_they_depend_on() {
    let mut store = Store::in_memory().expect("the store opens");
    let nothing = Work::default();
    assert_eq!(store.items(), Ok(vec![]));
    assert_eq!(store.works(), Ok(vec![]));
    assert_eq!(
        store.add("first", "2", &[], "judged", Time(10), &nothing),
        Ok((ItemNumber(1), WorkNumber(1)))
    );
    assert_eq!(
        store.add("second", "2", &[ItemNumber(1)], "other", Time(20), &nothing),
        Ok((ItemNumber(2), WorkNumber(2)))
    );
    // What it depends on is kept in the order of the numbers, each once.
    let on = [ItemNumber(2), ItemNumber(1), ItemNumber(2)];
    assert_eq!(
        store.add("third", "2", &on, "judged", Time(30), &nothing),
        Ok((ItemNumber(3), WorkNumber(3)))
    );
    assert_eq!(
        store.items(),
        Ok(vec![
            item(1, "first", 10, &[]),
            item(2, "second", 20, &[1]),
            item(3, "third", 30, &[1, 2]),
        ])
    );
    assert_eq!(
        store.works(),
        Ok(vec![
            head(1, 1, "judged", 10),
            head(2, 2, "other", 20),
            head(3, 3, "judged", 30),
        ])
    );
}

#[test]
fn each_item_keeps_the_level_it_was_added_at() {
    let mut store = Store::in_memory().expect("the store opens");
    let nothing = Work::default();
    store.add("easy", "1", &[], "judged", Time(10), &nothing).unwrap();
    store.add("hard", "3", &[], "judged", Time(20), &nothing).unwrap();
    let levels: Vec<Option<String>> = store.items().unwrap().into_iter().map(|i| i.level).collect();
    assert_eq!(levels, [Some("1".to_string()), Some("3".to_string())]);
}

#[test]
fn an_item_depending_on_one_the_store_does_not_have_is_refused_and_nothing_is_added() {
    let mut store = Store::in_memory().expect("the store opens");
    let nothing = Work::default();
    store
        .add("first", "2", &[], "judged", Time(10), &nothing)
        .expect("the item is added");
    // Its own number: the item would depend on itself.
    assert_eq!(
        store.add(
            "second",
            "2",
            &[ItemNumber(1), ItemNumber(2)],
            "judged",
            Time(20),
            &nothing
        ),
        Err(Fault::NoSuchDependency(ItemNumber(2)))
    );
    // A number no item has yet: one added later.
    assert_eq!(
        store.add("second", "2", &[ItemNumber(3)], "judged", Time(20), &nothing),
        Err(Fault::NoSuchDependency(ItemNumber(3)))
    );
    assert_eq!(store.items(), Ok(vec![item(1, "first", 10, &[])]));
    assert_eq!(store.works(), Ok(vec![head(1, 1, "judged", 10)]));
    // The numbers of the next item and its work are not taken by those refused.
    assert_eq!(
        store.add("second", "2", &[], "judged", Time(30), &nothing),
        Ok((ItemNumber(2), WorkNumber(2)))
    );
}

#[test]
fn what_is_kept_of_a_work_from_its_start_is_added_with_its_item() {
    let mut first = Driver::new();
    first.wrote("request", Outsider::Person, None);
    let db = Db::new();
    let work = {
        let mut store = db.open();
        let (_, work) = store
            .add("the item", "2", &[], "judged", Time(START), &first.work)
            .expect("the item is added");
        work
    };
    // A store opened anew on the file holds the work as it was given.
    let mut store = db.open();
    assert_eq!(store.load(work), Ok(first.work.clone()));
    // What was added with the item is where the work begins: saving it again keeps nothing.
    let before = dump(&db.path);
    store.save(work, &first.work).expect("the work is saved");
    assert_eq!(dump(&db.path), before);
}

#[test]
fn an_item_whose_work_cannot_be_kept_is_not_added() {
    let db = Db::new();
    let mut store = db.open();
    let before = dump(&db.path);
    // Two entries at one place, which no table holds.
    let mut wrong = Work::default();
    wrong.records.pauses = vec![
        Pause {
            paused: true,
            at: Seq(1),
            by: Pauser::Owner,
            why: None,
        },
        Pause {
            paused: false,
            at: Seq(1),
            by: Pauser::Owner,
            why: None,
        },
    ];
    assert!(matches!(
        store.add("the item", "2", &[], "judged", Time(START), &wrong),
        Err(Fault::Database(_))
    ));
    // Neither the item nor its work was kept.
    assert_eq!(dump(&db.path), before);
    assert_eq!(store.items(), Ok(vec![]));
    assert_eq!(store.works(), Ok(vec![]));
    // The numbers they would have had are given to the next.
    assert_eq!(
        store.add("the item", "2", &[], "judged", Time(START), &Work::default()),
        Ok((ItemNumber(1), WorkNumber(1)))
    );
}
