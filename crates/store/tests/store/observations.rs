//! The observations: each text of `observations` in the return a run was taken on is kept once, as the run's finish is
//! saved, with where it came from and unread; a return sent back keeps none; and the owner reads each once.

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
fn a_return_sent_back_keeps_no_observation() {
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
    // There is no observation of this number: nothing is kept.
    assert_eq!(read(&again, 3, START + 60_000), None);
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
