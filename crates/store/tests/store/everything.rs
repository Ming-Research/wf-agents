//! The owner's pause of everything, as the store keeps it: set and lifted, the latest saying whether it holds, and
//! kept across an opening; and a file made before the store kept it opens with none, its works as they were.

use store::SCHEMA_VERSION;

use super::opening::{columns, versions};
use super::*;

#[test]
fn the_pause_of_everything_is_kept_set_and_lifted_and_holds_across_an_opening() {
    let db = Db::new();
    let mut store = db.open();
    assert_eq!(store.everything_paused(), Ok(None));
    store.pause_everything(true, Time(START)).expect("it is kept");
    assert_eq!(store.everything_paused(), Ok(Some(Time(START))));
    store.pause_everything(false, Time(START + 1000)).expect("it is kept");
    assert_eq!(store.everything_paused(), Ok(None));
    store.pause_everything(true, Time(START + 2000)).expect("it is kept");
    drop(store);
    assert_eq!(db.open().everything_paused(), Ok(Some(Time(START + 2000))));
}

#[test]
fn a_file_made_before_the_pause_of_everything_was_kept_opens_with_none_and_its_works() {
    let db = Db::new();
    let work = {
        let mut store = db.open();
        let number = first_work(&mut store);
        let mut d = Driver::new();
        d.wrote("request", Outsider::Person, None);
        store.save(number, &d.work).expect("the work is saved");
        d.work
    };
    {
        let file = Connection::open(&db.path).expect("the database opens");
        file.execute_batch("DROP TABLE everything_paused; UPDATE schema_version SET version = 2")
            .expect("the file is made as version 2 made it");
    }
    let store = db.open();
    assert_eq!(store.everything_paused(), Ok(None));
    assert_eq!(store.load(WorkNumber(1)), Ok(work));
    assert_eq!(versions(&db.path), [SCHEMA_VERSION]);
    let new = Db::new();
    new.open();
    assert_eq!(
        columns(&db.path),
        columns(&new.path),
        "the migrated file has other tables than a new one"
    );
}
