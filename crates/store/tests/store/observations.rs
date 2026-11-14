//! The observations: a run keeps, as it finishes, those of the return it was taken on and of each return of it sent
//! back that no later return of it was made in a session holding, each once, with where it came from and unread; and
//! the owner reads each once.

use store::{ItemNumber, Observation, Observations};
use work::Said;

use super::*;

/// What is said beside a return's work: these observations.
pub(crate) fn observing(observations: &[&str]) -> Said {
    Said {
        observations: observations.iter().map(|o| o.to_string()).collect(),
        ..Said::default()
    }
}

/// What came back: these blocks handed in, with these observations.
fn handing_in(blocks: &[(&str, Option<&str>)], observations: &[&str]) -> Came {
    Came {
        said: observing(observations),
        ..hands_in(blocks)
    }
}

/// Every observation the file holds, as the page reads them.
pub(crate) fn kept(path: &Path) -> Vec<Observation> {
    (Observations::open(path).expect("the observations open"))
        .all()
        .expect("the observations are read")
}

/// The run whose round is under way.
fn running(d: &Driver) -> RunId {
    d.under_way().0
}

#[test]
fn each_observation_of_a_return_taken_is_kept_once_with_where_it_came_from_and_unread() {
    let mut d = Driver::saved();
    d.wrote("request", Outsider::Person, None);
    let run = running(&d);
    d.end(handing_in(
        &[("text", None)],
        &["The greeting has no test.", "The lexer drops a token."],
    ));
    assert!(
        d.work.records.finished.iter().any(|f| f.run == run),
        "the writer's return was not taken"
    );
    let at = Time(d.clock);
    let s = d.saving.as_ref().expect("saved after each event");
    let (work, path) = (s.work, s.db.path.clone());
    let from = |number: u64, text: &str| Observation {
        number,
        text: text.to_string(),
        work,
        item: ItemNumber(1),
        title: "the item".to_string(),
        cell: "write".to_string(),
        run,
        round: 1,
        kept: at,
        read: None,
    };
    // The newest first.
    let both = vec![
        from(2, "The lexer drops a token."),
        from(1, "The greeting has no test."),
    ];
    assert_eq!(kept(&path), both);
    // What follows keeps them as they are: the judge's return, taken, carries none.
    d.end(hands_in(&[("opinions", None), ("verdict", Some("yes"))]));
    assert_eq!(kept(&path), both);
    // Saved again whole, the work keeps nothing more.
    let s = d.saving.as_mut().expect("saved after each event");
    s.store.save(s.work, &d.work).expect("the work is saved again");
    assert_eq!(kept(&path), both);
}

#[test]
fn a_return_sent_back_keeps_no_observation_where_the_return_taken_was_made_in_its_session() {
    let mut d = Driver::saved();
    d.wrote("request", Outsider::Person, None);
    let run = running(&d);
    // The writer hands in the judge's verdict, which it may not write: the return is sent back with its observation.
    d.end(Came {
        said: observing(&["Said in a return sent back."]),
        ..wanting()
    });
    assert!(
        !d.work.records.finished.iter().any(|f| f.run == run),
        "the return was taken"
    );
    let path = d.saving.as_ref().expect("saved").db.path.clone();
    assert_eq!(kept(&path), Vec::<Observation>::new());
    // The run's next round says it again, and its return is taken: that one is kept, from the second round.
    d.end(handing_in(&[("text", None)], &["Said again in the return taken."]));
    let taken: Vec<(u64, String, RunId, usize)> = (kept(&path).into_iter())
        .map(|o| (o.number, o.text, o.run, o.round))
        .collect();
    assert_eq!(taken, [(1, "Said again in the return taken.".to_string(), run, 2)]);
}

/// The writer's line begins a new session, for this reason, and the try of the round under way begins in it.
fn anew(d: &mut Driver, why: engine::Why) {
    let run = running(d);
    d.on(Event::Anew {
        line: "main".to_string(),
        why,
    });
    let on = engine::RanOn {
        level: "2".to_string(),
        model: "a model".to_string(),
        effort: None,
        account: None,
    };
    d.on(Event::Ran { run, attempt: 1, on });
}

/// What the work keeps of the observations, oldest first: the text and the round of each.
fn said(path: &Path) -> Vec<(String, usize)> {
    let mut all: Vec<(String, usize)> = (kept(path).into_iter()).map(|o| (o.text, o.round)).collect();
    all.reverse();
    all
}

#[test]
fn a_return_sent_back_keeps_its_observations_where_the_return_taken_was_made_in_a_new_session() {
    let mut d = Driver::saved();
    d.wrote("request", Outsider::Person, None);
    // The first round is sent back, and its context was past the limit: the second runs in a new session, which never
    // saw what the first returned, and its return taken says nothing of the first's observation.
    d.end(Came {
        said: observing(&["Said in the first session."]),
        ..wanting()
    });
    anew(&mut d, engine::Why::Limit { context: 9, limit: 5 });
    d.end(handing_in(&[("text", None)], &["Said in the second session."]));
    let path = d.saving.as_ref().expect("saved").db.path.clone();
    assert_eq!(
        said(&path),
        [
            ("Said in the first session.".to_string(), 1),
            ("Said in the second session.".to_string(), 2)
        ]
    );
}

#[test]
fn of_the_returns_sent_back_in_one_session_only_the_last_is_kept_once_a_later_session_takes_the_run_on() {
    let mut d = Driver::saved();
    d.wrote("request", Outsider::Person, None);
    // Two rounds sent back in the first session, the second saying again what still holds of the first's; then the
    // third, in a new session the owner asked for, is taken.
    d.end(Came {
        said: observing(&["The first said, and taken back."]),
        ..wanting()
    });
    d.end(Came {
        said: observing(&["The second said."]),
        ..wanting()
    });
    anew(&mut d, engine::Why::Owner);
    d.end(handing_in(&[("text", None)], &["The third said."]));
    let path = d.saving.as_ref().expect("saved").db.path.clone();
    assert_eq!(
        said(&path),
        [("The second said.".to_string(), 2), ("The third said.".to_string(), 3)]
    );
    // Saved again whole, the work keeps nothing more.
    let s = d.saving.as_mut().expect("saved after each event");
    s.store.save(s.work, &d.work).expect("the work is saved again");
    assert_eq!(said(&path).len(), 2);
}

#[test]
fn a_return_sent_back_keeps_its_observations_where_its_session_was_compacted_before_the_return_taken() {
    let mut d = Driver::saved();
    d.wrote("request", Outsider::Person, None);
    d.end(Came {
        said: observing(&["Said before the compaction."]),
        ..wanting()
    });
    anew(&mut d, engine::Why::Compacted);
    d.end(handing_in(&[("text", None)], &["Said after it."]));
    let path = d.saving.as_ref().expect("saved").db.path.clone();
    assert_eq!(
        said(&path),
        [
            ("Said before the compaction.".to_string(), 1),
            ("Said after it.".to_string(), 2)
        ]
    );
}

#[test]
fn an_observation_is_unread_until_the_owner_reads_it_and_stays_read_as_first_read() {
    let mut d = Driver::saved();
    d.wrote("request", Outsider::Person, None);
    d.end(handing_in(&[("text", None)], &["The first.", "The second."]));
    let path = d.saving.as_ref().expect("saved").db.path.clone();
    let observations = Observations::open(&path).expect("the observations open");
    let read = |e: &Observations, number: u64, at: u64| e.read(number, Time(at)).expect("the read is kept");
    assert_eq!(read(&observations, 1, START + 50_000), Some(Time(START + 50_000)));
    // Read again later, on a connection of its own, it keeps the time it was first read.
    let again = Observations::open(&path).expect("the observations open");
    assert_eq!(read(&again, 1, START + 60_000), Some(Time(START + 50_000)));
    // There is no observation of this number, nor of one past the numbers the store gives: nothing is kept.
    for none in [3, i64::MAX.unsigned_abs() + 1, u64::MAX] {
        assert_eq!(read(&again, none, START + 60_000), None, "{none}");
    }
    let states: Vec<(u64, Option<Time>)> = (kept(&path).iter()).map(|o| (o.number, o.read)).collect();
    assert_eq!(states, [(2, None), (1, Some(Time(START + 50_000)))]);
    let reads: u64 = (Connection::open(&path).expect("the database opens"))
        .query_row("SELECT count(*) FROM observations_read", [], |row| row.get(0))
        .expect("the reads are counted");
    assert_eq!(reads, 1);
}

#[test]
fn a_file_of_another_version_is_not_opened_for_the_observations() {
    let db = Db::new();
    {
        let file = Connection::open(&db.path).expect("the database opens");
        file.execute_batch(opening::SCHEMA_1).expect("the first schema is made");
        file.execute("INSERT INTO schema_version (version) VALUES (1)", [])
            .expect("its version is written");
    }
    assert_eq!(
        Observations::open(&db.path).map(|_| ()),
        Err(store::Fault::Schema {
            found: Some(1),
            expected: store::SCHEMA_VERSION
        })
    );
}
