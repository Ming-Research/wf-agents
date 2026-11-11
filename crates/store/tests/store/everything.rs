//! The owner's pause of everything, as the store keeps it: set and lifted, the latest saying whether it holds, and
//! kept across an opening; and a file made before the store kept it opens with none.

use store::SCHEMA_VERSION;

use super::opening::{SCHEMA_1, columns, versions};
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
fn a_file_made_before_the_pause_of_everything_was_kept_opens_with_none() {
    // A file of the first version of the schema, which every migration since brings to this one, whatever their
    // order: what a file holds of its works through them is held by the tests of opening.
    let db = Db::new();
    {
        let file = Connection::open(&db.path).expect("the database opens");
        file.execute_batch(SCHEMA_1).expect("the first schema is made");
        file.execute("INSERT INTO schema_version (version) VALUES (1)", [])
            .expect("its version is written");
    }
    let store = db.open();
    assert_eq!(store.everything_paused(), Ok(None));
    assert_eq!(versions(&db.path), [SCHEMA_VERSION]);
    let new = Db::new();
    new.open();
    assert_eq!(
        columns(&db.path),
        columns(&new.path),
        "the migrated file has other tables than a new one"
    );
}
