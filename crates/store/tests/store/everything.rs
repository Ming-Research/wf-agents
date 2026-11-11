//! The owner's pause of everything, as the store keeps it: set and lifted, the latest saying whether it holds, and
//! kept across an opening; and a file made before the store kept it opens with none.

use store::{EverythingPaused, SCHEMA_VERSION};

use super::opening::{MIGRATIONS, SCHEMA_1, columns, versions};
use super::*;

#[test]
fn the_pause_of_everything_is_kept_set_and_lifted_with_who_set_it_and_holds_across_an_opening() {
    let db = Db::new();
    let mut store = db.open();
    let paused = |at: u64, by: Pauser| {
        Ok(Some(EverythingPaused {
            since: Time(START + at),
            by,
        }))
    };
    assert_eq!(store.everything_paused(), Ok(None));
    store
        .pause_everything(true, Pauser::Owner, Time(START))
        .expect("it is kept");
    assert_eq!(store.everything_paused(), paused(0, Pauser::Owner));
    store
        .pause_everything(false, Pauser::Owner, Time(START + 1000))
        .expect("it is kept");
    assert_eq!(store.everything_paused(), Ok(None));
    store
        .pause_everything(true, Pauser::Engine, Time(START + 2000))
        .expect("it is kept");
    assert_eq!(store.everything_paused(), paused(2000, Pauser::Engine));
    drop(store);
    assert_eq!(db.open().everything_paused(), paused(2000, Pauser::Engine));
}

#[test]
fn a_pause_of_everything_kept_before_it_said_who_set_it_is_read_as_the_owners() {
    // A file of the version before, holding a pause and its lifting and a pause again, each kept with no pauser.
    let db = Db::new();
    {
        let file = Connection::open(&db.path).expect("the database opens");
        file.execute_batch(SCHEMA_1).expect("the first schema is made");
        file.execute("INSERT INTO schema_version (version) VALUES (1)", [])
            .expect("its version is written");
        for migration in &MIGRATIONS[..MIGRATIONS.len() - 1] {
            file.execute_batch(migration).expect("the migration runs");
        }
        for (paused, at) in [(1, 0), (0, 1000), (1, 2000)] {
            file.execute(
                "INSERT INTO everything_paused (paused, time) VALUES (?1, ?2)",
                rusqlite::params![paused, START + at],
            )
            .expect("a pause is kept");
        }
    }
    let store = db.open();
    assert_eq!(
        store.everything_paused(),
        Ok(Some(EverythingPaused {
            since: Time(START + 2000),
            by: Pauser::Owner,
        }))
    );
    assert_eq!(versions(&db.path), [SCHEMA_VERSION]);
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
