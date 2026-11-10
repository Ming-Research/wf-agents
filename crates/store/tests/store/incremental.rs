//! Saving a work after each thing that happens keeps what saving it once at the end keeps; saving it again keeps
//! nothing more.

use super::*;

#[test]
fn a_work_saved_after_each_thing_that_happens_is_kept_as_one_saved_once_at_the_end() {
    let mut each = Driver::saved();
    history(&mut each);
    let mut once = Driver::new();
    history(&mut once);
    assert_eq!(once.work, each.work);

    let db = Db::new();
    let mut store = db.open();
    let work = first_work(&mut store);
    store.save(work, &once.work).expect("the work is saved");
    let saving = each.saving.as_ref().expect("saved after each event");
    assert_eq!(dump(&db.path), dump(&saving.db.path));
    assert_eq!(db.open().load(work), Ok(once.work.clone()));
}

#[test]
fn saving_a_work_the_store_holds_whole_keeps_nothing() {
    let mut d = Driver::new();
    history(&mut d);
    let db = Db::new();
    let mut store = db.open();
    let work = first_work(&mut store);
    store.save(work, &d.work).expect("the work is saved");
    let before = dump(&db.path);
    store.save(work, &d.work).expect("the work is saved again");
    assert_eq!(dump(&db.path), before);
}

#[test]
fn two_works_are_kept_apart() {
    let mut d = Driver::new();
    history(&mut d);
    let mut store = Store::in_memory().expect("the store opens");
    let first = first_work(&mut store);
    let (_, second) = store
        .add("another", &[], "judged", Time(START), &Work::default())
        .expect("the item is added");
    store.save(first, &d.work).expect("the work is saved");
    assert_eq!(store.load(second), Ok(Work::default()));
    let mut short = Driver::new();
    short.wrote("request", Outsider::Person, None);
    store.save(second, &short.work).expect("the other work is saved");
    assert_eq!(store.load(first), Ok(d.work.clone()));
    assert_eq!(store.load(second), Ok(short.work.clone()));
}
