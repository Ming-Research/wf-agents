//! The items, numbered by the store in the order they are added, each depending only on items added before it; and
//! the works done for them.

use store::{Fault, Item, ItemNumber, WorkHead};

use super::*;

fn item(number: u64, title: &str, added: u64, depends_on: &[u64]) -> Item {
    Item {
        number: ItemNumber(number),
        title: title.to_string(),
        added: Time(added),
        depends_on: depends_on.iter().map(|n| ItemNumber(*n)).collect(),
    }
}

#[test]
fn items_are_numbered_from_one_in_the_order_they_are_added_with_what_they_depend_on() {
    let mut store = Store::in_memory().expect("the store opens");
    assert_eq!(store.items(), Ok(vec![]));
    assert_eq!(store.add_item("first", &[], Time(10)), Ok(ItemNumber(1)));
    assert_eq!(store.add_item("second", &[ItemNumber(1)], Time(20)), Ok(ItemNumber(2)));
    // What it depends on is kept in the order of the numbers, each once.
    let on = [ItemNumber(2), ItemNumber(1), ItemNumber(2)];
    assert_eq!(store.add_item("third", &on, Time(30)), Ok(ItemNumber(3)));
    assert_eq!(
        store.items(),
        Ok(vec![
            item(1, "first", 10, &[]),
            item(2, "second", 20, &[1]),
            item(3, "third", 30, &[1, 2]),
        ])
    );
}

#[test]
fn an_item_depending_on_one_the_store_does_not_have_is_refused_and_nothing_is_added() {
    let mut store = Store::in_memory().expect("the store opens");
    store.add_item("first", &[], Time(10)).expect("the item is added");
    // Its own number: the item would depend on itself.
    assert_eq!(
        store.add_item("second", &[ItemNumber(1), ItemNumber(2)], Time(20)),
        Err(Fault::NoSuchDependency(ItemNumber(2)))
    );
    // A number no item has yet: one added later.
    assert_eq!(
        store.add_item("second", &[ItemNumber(3)], Time(20)),
        Err(Fault::NoSuchDependency(ItemNumber(3)))
    );
    assert_eq!(store.items(), Ok(vec![item(1, "first", 10, &[])]));
    // The number of the next item is not taken by those refused.
    assert_eq!(store.add_item("second", &[], Time(30)), Ok(ItemNumber(2)));
}

#[test]
fn works_are_numbered_from_one_in_the_order_they_are_added_and_an_item_may_have_several() {
    let mut store = Store::in_memory().expect("the store opens");
    let first = store.add_item("first", &[], Time(10)).expect("the item is added");
    let second = store.add_item("second", &[], Time(20)).expect("the item is added");
    assert_eq!(store.works(), Ok(vec![]));
    assert_eq!(store.add_work(first, "judged", Time(30)), Ok(WorkNumber(1)));
    assert_eq!(store.add_work(second, "judged", Time(40)), Ok(WorkNumber(2)));
    assert_eq!(store.add_work(first, "other", Time(50)), Ok(WorkNumber(3)));
    let head = |number, item, flow: &str, added| WorkHead {
        number: WorkNumber(number),
        item: ItemNumber(item),
        flow: flow.to_string(),
        added: Time(added),
    };
    assert_eq!(
        store.works(),
        Ok(vec![
            head(1, 1, "judged", 30),
            head(2, 2, "judged", 40),
            head(3, 1, "other", 50),
        ])
    );
}

#[test]
fn a_work_for_an_item_the_store_does_not_have_is_refused() {
    let mut store = Store::in_memory().expect("the store opens");
    assert_eq!(
        store.add_work(ItemNumber(1), "judged", Time(10)),
        Err(Fault::NoSuchItem(ItemNumber(1)))
    );
    assert_eq!(store.works(), Ok(vec![]));
}
