//! The one-off repair of recorded costs: what it reads of the store, the rewrite in place, and the copy taken before it.

use super::*;

/// What a fork of the first round of the history took.
fn spent(usd_micros: u64) -> Spent {
    Spent {
        usd_micros,
        input_tokens: 120,
        cached_tokens: 18_100,
        output_tokens: 30,
        ..Spent::default()
    }
}

// The costs of a work's rounds and forks are read in the order the rounds began, each round with the number of its last
// try, and corrected in place all together; the work reads back with the new costs and nothing else changed. A few
// milliseconds, on a file; no other test holds an update of rows the store otherwise only adds to.
#[test]
fn the_costs_of_rounds_and_forks_are_read_and_corrected_in_place_and_nothing_else_changes() {
    let db = Db::new();
    let mut store = db.open();
    let number = first_work(&mut store);
    let mut d = Driver::new();
    history(&mut d);
    let run = d.work.records.rounds[0].run;
    d.on(Event::Forked {
        run,
        round: 1,
        spent: spent(400_000),
    });
    store.save(number, &d.work).expect("the work is saved");
    let rows = store.cost_rows().expect("the costs are read");
    assert_eq!(rows.rounds.len(), d.work.spent.len());
    assert_eq!(
        (rows.forks.len(), rows.forks[0].usd_micros, rows.forks[0].run),
        (1, 400_000, run.0)
    );
    let first = rows.rounds[0];
    assert_eq!(
        first.usd_micros,
        d.work.spent[&(RunId(first.run), first.round)].usd_micros
    );
    let fork = d.work.forked[0].at.0;
    store
        .correct_costs(&[(first.work, first.run, first.round, 123)], &[(number.0, fork, 456)])
        .expect("the costs are corrected");

    let mut expected = d.work.clone();
    expected
        .spent
        .get_mut(&(RunId(first.run), first.round))
        .unwrap()
        .usd_micros = 123;
    expected.forked[0].usd_micros = 456;
    assert_eq!(db.open().load(number), Ok(expected));
}

// A copy of the database is taken before the repair, as before a migration; where it cannot be taken, because the place
// for the copies is a file, the repair is told so and nothing is written. A few milliseconds, on a file.
#[test]
fn a_copy_is_taken_before_the_repair_and_one_that_cannot_be_taken_is_an_error() {
    let db = Db::new();
    let store = db.open();
    store.copy_before_repair().expect("a copy is taken");
    let copies = db.path.parent().unwrap().join("backups");
    assert_eq!(std::fs::read_dir(&copies).expect("copies are kept").count(), 1);

    let db = Db::new();
    let store = db.open();
    std::fs::write(db.path.parent().unwrap().join("backups"), "not a directory").unwrap();
    assert!(store.copy_before_repair().is_err());
}
