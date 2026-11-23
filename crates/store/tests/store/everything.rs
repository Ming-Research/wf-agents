//! What the store keeps of the engine as a whole: the owner's pause of everything, set and lifted, the latest saying
//! whether it holds, whether the sessions of works that wait are kept warm, which the owner switches, and the accounts
//! the agents may run under, each added, removed and taken; each kept across an opening, and a file made before the
//! store kept one opening as if it had never been set.

use store::{AccountAct, AccountRecord, AccountTaker, EverythingPaused, PauseOfEverything, SCHEMA_VERSION};

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
fn whether_the_sessions_of_works_that_wait_are_kept_warm_is_switched_and_holds_across_an_opening() {
    let db = Db::new();
    let mut store = db.open();
    assert_eq!(store.keeping_warm(), Ok(true), "kept warm until it is switched off");
    store.keep_warm(false, Time(START)).expect("it is kept");
    assert_eq!(store.keeping_warm(), Ok(false));
    store.keep_warm(true, Time(START + 1000)).expect("it is kept");
    assert_eq!(store.keeping_warm(), Ok(true));
    store.keep_warm(false, Time(START + 2000)).expect("it is kept");
    drop(store);
    assert_eq!(db.open().keeping_warm(), Ok(false), "the latest switch holds");
}

#[test]
fn a_file_made_before_the_switch_of_keeping_sessions_warm_was_kept_opens_keeping_them_warm() {
    let db = Db::new();
    {
        let file = Connection::open(&db.path).expect("the database opens");
        file.execute_batch(SCHEMA_1).expect("the first schema is made");
        file.execute("INSERT INTO schema_version (version) VALUES (1)", [])
            .expect("its version is written");
    }
    let store = db.open();
    assert_eq!(store.keeping_warm(), Ok(true));
    assert_eq!(versions(&db.path), [SCHEMA_VERSION]);
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
fn whether_the_engine_may_move_to_another_account_at_a_limit_is_off_until_switched_and_holds_across_an_opening() {
    let db = Db::new();
    let mut store = db.open();
    assert_eq!(store.moving_on(), Ok(false), "off until it is switched on");
    store.set_moving_on(true, Time(START)).expect("it is kept");
    assert_eq!(store.moving_on(), Ok(true));
    store.set_moving_on(false, Time(START + 1000)).expect("it is kept");
    assert_eq!(store.moving_on(), Ok(false));
    store.set_moving_on(true, Time(START + 2000)).expect("it is kept");
    drop(store);
    assert_eq!(db.open().moving_on(), Ok(true), "the latest switch holds");
}

#[test]
fn an_account_the_engine_took_keeps_who_took_it_and_the_reset_of_the_limit_it_left_across_an_opening() {
    let db = Db::new();
    let mut store = db.open();
    let record = |name: &str, what: AccountAct, taker: AccountTaker, left_resets: Option<u64>, at: u64| AccountRecord {
        name: name.to_string(),
        what,
        at: Time(START + at),
        taker,
        left_resets: left_resets.map(|t| Time(START + t)),
    };
    let kept = [
        record("work", AccountAct::Added, AccountTaker::Owner, None, 0),
        record("home", AccountAct::Added, AccountTaker::Owner, None, 1000),
        record("work", AccountAct::Taken, AccountTaker::Owner, None, 2000),
        record("home", AccountAct::Taken, AccountTaker::Engine, Some(900_000), 3000),
        record("work", AccountAct::Taken, AccountTaker::Engine, None, 4000),
    ];
    for record in &kept {
        store.keep_account(record).expect("it is kept");
    }
    assert_eq!(store.accounts(), Ok(kept.to_vec()));
    drop(store);
    assert_eq!(db.open().accounts(), Ok(kept.to_vec()));
}

#[test]
fn every_account_added_removed_and_taken_is_given_back_in_order_with_its_name_and_when_across_an_opening() {
    let db = Db::new();
    let mut store = db.open();
    assert_eq!(store.accounts(), Ok(Vec::new()), "no account until one is added");
    let record = |name: &str, what: AccountAct, at: u64| AccountRecord {
        name: name.to_string(),
        what,
        at: Time(START + at),
        taker: AccountTaker::Owner,
        left_resets: None,
    };
    let kept = [
        record("work", AccountAct::Added, 0),
        record("home", AccountAct::Added, 1000),
        record("work", AccountAct::Taken, 2000),
        record("home", AccountAct::Taken, 3000),
        record("work", AccountAct::Removed, 4000),
        record("work", AccountAct::Added, 5000),
    ];
    for record in &kept {
        store.keep_account(record).expect("it is kept");
    }
    assert_eq!(store.accounts(), Ok(kept.to_vec()));
    drop(store);
    assert_eq!(db.open().accounts(), Ok(kept.to_vec()));
}
