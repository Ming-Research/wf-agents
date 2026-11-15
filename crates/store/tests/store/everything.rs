//! The owner's pause of everything, as the store keeps it: set and lifted, the latest saying whether it holds, and
//! kept across an opening; and a file made before the store kept it opens with none. And the other things of the
//! installation it keeps: the accounts taken, and the switch that lets the engine move on.

use store::{AccountTaken, EverythingPaused, MovingOn, PauseOfEverything, SCHEMA_VERSION, Taker};

use super::opening::{SCHEMA_1, columns, versions};
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
fn every_setting_and_lifting_of_the_pause_of_everything_is_given_back_in_order_with_who_and_when_across_an_opening() {
    let db = Db::new();
    let mut store = db.open();
    assert_eq!(store.everything_pauses(), Ok(Vec::new()));
    let kept = [
        (true, Pauser::Owner, 0),
        (false, Pauser::Owner, 1000),
        (true, Pauser::Engine, 2000),
        (false, Pauser::Owner, 3000),
    ];
    for (paused, by, at) in kept {
        store
            .pause_everything(paused, by, Time(START + at))
            .expect("it is kept");
    }
    let expected: Vec<PauseOfEverything> = (kept.iter())
        .map(|(paused, by, at)| PauseOfEverything {
            paused: *paused,
            by: *by,
            at: Time(START + at),
        })
        .collect();
    assert_eq!(store.everything_pauses(), Ok(expected.clone()));
    drop(store);
    assert_eq!(db.open().everything_pauses(), Ok(expected));
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
        // The migration that says who paused everything brings a file to version 7.
        store::migrate(&file, 1, 7).expect("the migrations run");
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

#[test]
fn every_account_taken_is_given_back_in_order_with_who_took_it_and_what_the_engine_left_across_an_opening() {
    let db = Db::new();
    let mut store = db.open();
    assert_eq!(store.accounts_taken(), Ok(Vec::new()));
    let kept = vec![
        AccountTaken {
            account: "/data/claude-second".to_string(),
            by: Taker::Owner,
            at: Time(START),
            left: None,
            resets: None,
        },
        AccountTaken {
            account: "/data/claude".to_string(),
            by: Taker::Engine,
            at: Time(START + 1000),
            left: Some("/data/claude-second".to_string()),
            resets: Some(Time(START + 7 * 24 * 3_600_000)),
        },
        AccountTaken {
            account: "/data/claude-third".to_string(),
            by: Taker::Engine,
            at: Time(START + 2000),
            left: Some("/data/claude".to_string()),
            resets: None,
        },
    ];
    for taken in &kept {
        store.take_account(taken).expect("it is kept");
    }
    assert_eq!(store.accounts_taken(), Ok(kept.clone()));
    drop(store);
    assert_eq!(db.open().accounts_taken(), Ok(kept));
}

#[test]
fn the_switch_that_moves_on_is_off_until_set_and_stands_as_set_last_across_an_opening() {
    let db = Db::new();
    let mut store = db.open();
    assert_eq!(store.moving_on(), Ok(None));
    store.set_moving_on(true, Time(START)).expect("it is kept");
    store.set_moving_on(false, Time(START + 1000)).expect("it is kept");
    store.set_moving_on(true, Time(START + 2000)).expect("it is kept");
    let last = Ok(Some(MovingOn {
        on: true,
        at: Time(START + 2000),
    }));
    assert_eq!(store.moving_on(), last);
    drop(store);
    assert_eq!(db.open().moving_on(), last);
}
