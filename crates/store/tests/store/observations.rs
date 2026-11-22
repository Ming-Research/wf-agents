//! The observations: a run keeps, as it finishes, those of the return it was taken on and of each return of it sent
//! back that no later return of it was made in a session holding, each once, with where it came from and unread; an
//! architect adds others; each has a title and tags, and is changed with a reason, taken in by an item or deleted; and
//! the owner reads each once.

use store::{
    FromRound, ItemNumber, NewObservation, NotMade, Observation, ObservationChange, Observations, Refusal, TagCount,
};
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
        title: None,
        tags: Vec::new(),
        project: Some("one".to_string()),
        reported_by: Some("one".to_string()),
        round: Some(FromRound {
            work,
            item: ItemNumber(1),
            item_title: "the item".to_string(),
            cell: "write".to_string(),
            run,
            round: 1,
        }),
        kept: at,
        read: None,
        taken_in_by: None,
        handled: false,
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
        directory: None,
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
    // The first round is sent back, and a session was begun anew for its context, as records kept before the program
    // compacted a session instead still hold: the second runs in a session that never saw what the first returned, and
    // its return taken says nothing of the first's observation.
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

/// An observation an architect adds, of the project `one` and reported by it, with this text.
fn adding(text: &str) -> NewObservation {
    NewObservation {
        text: text.to_string(),
        title: None,
        tags: Vec::new(),
        project: "one".to_string(),
        reported_by: "one".to_string(),
    }
}

/// The changes of the observation of this number the database keeps, in order: the change as it is kept, and why.
fn kept_changes(path: &Path, number: u64) -> Vec<(String, String)> {
    let db = Connection::open(path).expect("the database opens");
    let mut query = db
        .prepare("SELECT change, why FROM observation_changes WHERE observation = ?1 ORDER BY n")
        .expect("the changes are read");
    let rows = query
        .query_map([number], |row| Ok((row.get(0)?, row.get(1)?)))
        .expect("the changes are read");
    rows.collect::<Result<_, _>>().expect("the changes are read")
}

/// A store with the one item and its work, and its file's path.
fn with_an_item() -> (Db, Store, WorkNumber) {
    let db = Db::new();
    let mut store = db.open();
    let work = first_work(&mut store);
    (db, store, work)
}

#[test]
fn an_added_observation_is_numbered_after_the_kept_ones_and_says_which_project_it_is_of_and_who_reported_it() {
    let mut d = Driver::saved();
    d.wrote("request", Outsider::Person, None);
    d.end(handing_in(&[("text", None)], &["From the run."]));
    let path = d.saving.as_ref().expect("saved").db.path.clone();
    let observations = Observations::open(&path).expect("the observations open");
    let new = NewObservation {
        text: "The merge cell names no tests.".to_string(),
        title: Some("Name the tests".to_string()),
        tags: vec!["tests".to_string(), "flow".to_string(), "tests".to_string()],
        project: "one".to_string(),
        reported_by: "two".to_string(),
    };
    assert_eq!(observations.add(&new, Time(7_000)), Ok(2));
    let all = kept(&path);
    assert_eq!(all.iter().map(|o| o.number).collect::<Vec<_>>(), [2, 1]);
    assert_eq!(
        all[0],
        Observation {
            number: 2,
            text: "The merge cell names no tests.".to_string(),
            title: Some("Name the tests".to_string()),
            tags: vec!["flow".to_string(), "tests".to_string()],
            project: Some("one".to_string()),
            reported_by: Some("two".to_string()),
            round: None,
            kept: Time(7_000),
            read: None,
            taken_in_by: None,
            handled: false,
        }
    );
    // One a run kept after it takes the next number, and the work's saving it again keeps nothing more.
    d.end(handing_in(
        &[("opinions", None), ("verdict", Some("yes"))],
        &["From the judge."],
    ));
    let s = d.saving.as_mut().expect("saved after each event");
    s.store.save(s.work, &d.work).expect("the work is saved again");
    let numbers: Vec<u64> = kept(&path).iter().map(|o| o.number).collect();
    assert_eq!(numbers, [3, 2, 1]);
}

#[test]
fn an_observation_from_a_round_is_of_and_reported_by_the_project_of_its_item() {
    let mut d = Driver::saved();
    d.wrote("request", Outsider::Person, None);
    d.end(handing_in(&[("text", None)], &["From the run."]));
    let path = d.saving.as_ref().expect("saved").db.path.clone();
    let all = kept(&path);
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].project.as_deref(), Some("one"));
    assert_eq!(all[0].reported_by.as_deref(), Some("one"));
    assert!(all[0].round.is_some());
}

#[test]
fn a_title_and_tags_are_kept_as_changes_with_why_and_the_observation_reads_as_their_fold() {
    let (db, _store, _) = with_an_item();
    let observations = Observations::open(&db.path).expect("the observations open");
    let mut new = adding("Text.");
    new.title = Some("First".to_string());
    new.tags = vec!["x".to_string()];
    assert_eq!(observations.add(&new, Time(1)), Ok(1));
    let made = [
        (1, ObservationChange::Titled("Second".to_string())),
        (1, ObservationChange::Tagged("y".to_string())),
        (1, ObservationChange::Untagged("x".to_string())),
    ];
    assert_eq!(
        observations.change("Sorting.", &made, Time(9)),
        Ok(Ok(vec![true, true, true]))
    );
    let all = observations.all().expect("the observations are read");
    assert_eq!(all[0].title.as_deref(), Some("Second"));
    assert_eq!(all[0].tags, ["y"]);
    let said = |change: &str| (change.to_string(), "Sorting.".to_string());
    assert_eq!(
        kept_changes(&db.path, 1),
        [
            said("{\"Titled\":\"Second\"}"),
            said("{\"Tagged\":\"y\"}"),
            said("{\"Untagged\":\"x\"}")
        ]
    );
    // A change that leaves it as it is is no change, and is not kept.
    let same = [(1, ObservationChange::Titled("Second".to_string()))];
    assert_eq!(observations.change("Again.", &same, Time(10)), Ok(Ok(vec![false])));
    assert_eq!(kept_changes(&db.path, 1).len(), 3);
}

#[test]
fn a_deleted_observation_is_not_listed_and_is_kept_with_why_and_takes_no_change() {
    let (db, _store, _) = with_an_item();
    let observations = Observations::open(&db.path).expect("the observations open");
    observations.add(&adding("Out of date."), Time(1)).expect("added");
    observations.add(&adding("Still true."), Time(2)).expect("added");
    let deleted = [(1, ObservationChange::Deleted)];
    assert_eq!(
        observations.change("A fix removed it.", &deleted, Time(3)),
        Ok(Ok(vec![true]))
    );
    let texts: Vec<String> = observations.all().expect("read").into_iter().map(|o| o.text).collect();
    assert_eq!(texts, ["Still true."]);
    // Its text and why it was deleted stay in the records.
    let db_file = Connection::open(&db.path).expect("the database opens");
    let text: String = (db_file.query_row("SELECT text FROM observations WHERE number = 1", [], |row| row.get(0)))
        .expect("the text is kept");
    assert_eq!(text, "Out of date.");
    assert_eq!(
        kept_changes(&db.path, 1),
        [("\"Deleted\"".to_string(), "A fix removed it.".to_string())]
    );
    // It takes no change after.
    let titled = [(1, ObservationChange::Titled("Late.".to_string()))];
    assert_eq!(
        observations.change("Late.", &titled, Time(4)),
        Ok(Err(vec![NotMade {
            at: 0,
            observation: 1,
            why: Refusal::Deleted
        }]))
    );
}

#[test]
fn an_item_takes_an_observation_in_and_a_later_taking_in_changes_it() {
    let (db, mut store, _) = with_an_item();
    store
        .add(&first("second", "2", &[]), "judged", Time(START + 1), &Work::default())
        .expect("the item is added");
    let observations = Observations::open(&db.path).expect("the observations open");
    observations.add(&adding("Taken."), Time(1)).expect("added");
    let taken = |item: u64| [(1, ObservationChange::TakenIn(ItemNumber(item)))];
    assert_eq!(
        observations.change("Item 1 holds it.", &taken(1), Time(2)),
        Ok(Ok(vec![true]))
    );
    let now = observations.all().expect("read");
    assert_eq!((now[0].taken_in_by, now[0].handled), (Some(ItemNumber(1)), true));
    assert_eq!(
        observations.change("Item 2 is nearer.", &taken(2), Time(3)),
        Ok(Ok(vec![true]))
    );
    assert_eq!(observations.all().expect("read")[0].taken_in_by, Some(ItemNumber(2)));
    // The item that has it already is no change.
    assert_eq!(observations.change("Again.", &taken(2), Time(4)), Ok(Ok(vec![false])));
    // A number that is no item takes none in.
    assert_eq!(
        observations.change("None.", &taken(9), Time(5)),
        Ok(Err(vec![NotMade {
            at: 0,
            observation: 1,
            why: Refusal::NoSuchItem(ItemNumber(9))
        }]))
    );
}

#[test]
fn a_cancelled_item_takes_no_observation_in_and_one_it_took_in_is_unhandled_again_once_it_is_cancelled() {
    let (db, mut store, work) = with_an_item();
    let observations = Observations::open(&db.path).expect("the observations open");
    observations.add(&adding("Taken."), Time(1)).expect("added");
    let taken = [(1, ObservationChange::TakenIn(ItemNumber(1)))];
    assert_eq!(
        observations.change("Item 1 holds it.", &taken, Time(2)),
        Ok(Ok(vec![true]))
    );
    assert!(observations.all().expect("read")[0].handled);
    // The item's work is cancelled: what it took in is not handled, still says which item took it, and no more is taken.
    let mut d = Driver::new();
    d.on(Event::Cancelled { why: None });
    store.save(work, &d.work).expect("the work is saved");
    let now = observations.all().expect("read");
    assert_eq!((now[0].taken_in_by, now[0].handled), (Some(ItemNumber(1)), false));
    observations.add(&adding("Another."), Time(3)).expect("added");
    let another = [(2, ObservationChange::TakenIn(ItemNumber(1)))];
    assert_eq!(
        observations.change("Cancelled.", &another, Time(4)),
        Ok(Err(vec![NotMade {
            at: 0,
            observation: 2,
            why: Refusal::ItemCancelled(ItemNumber(1))
        }]))
    );
    // Reopened, the item holds it again.
    d.on(Event::Reopened { why: "Wanted.".into() });
    store.save(work, &d.work).expect("the work is saved");
    assert!(
        observations
            .all()
            .expect("read")
            .iter()
            .any(|o| o.number == 1 && o.handled)
    );
}

#[test]
fn a_batch_of_changes_is_all_or_none_and_names_every_refusal() {
    let (db, _store, _) = with_an_item();
    let observations = Observations::open(&db.path).expect("the observations open");
    observations.add(&adding("Text."), Time(1)).expect("added");
    let asked = [
        (1, ObservationChange::Titled("Good".to_string())),
        (99, ObservationChange::Tagged("x".to_string())),
        (1, ObservationChange::Tagged("two words".to_string())),
        (1, ObservationChange::TakenIn(ItemNumber(9))),
        (1, ObservationChange::Titled("  ".to_string())),
    ];
    let not_made = |at: usize, observation: u64, why: Refusal| NotMade { at, observation, why };
    assert_eq!(
        observations.change("Together.", &asked, Time(2)),
        Ok(Err(vec![
            not_made(1, 99, Refusal::NoSuchObservation),
            not_made(2, 1, Refusal::BadTag("two words".to_string())),
            not_made(3, 1, Refusal::NoSuchItem(ItemNumber(9))),
            not_made(4, 1, Refusal::BlankTitle),
        ]))
    );
    // The good change before them was not made either.
    assert_eq!(observations.all().expect("read")[0].title, None);
    assert_eq!(kept_changes(&db.path, 1), Vec::<(String, String)>::new());
    // A number too large for the store names no observation.
    let large = [(u64::MAX, ObservationChange::Deleted)];
    assert_eq!(
        observations.change("Large.", &large, Time(3)),
        Ok(Err(vec![not_made(0, u64::MAX, Refusal::NoSuchObservation)]))
    );
}

#[test]
fn the_tags_count_the_live_observations_that_carry_them_and_those_not_handled() {
    let (db, _store, _) = with_an_item();
    let observations = Observations::open(&db.path).expect("the observations open");
    let tagged = |text: &str, tags: &[&str]| NewObservation {
        tags: tags.iter().map(|t| t.to_string()).collect(),
        ..adding(text)
    };
    observations.add(&tagged("A.", &["x", "y"]), Time(1)).expect("added");
    observations.add(&tagged("B.", &["x"]), Time(2)).expect("added");
    observations.add(&tagged("C.", &["x", "z"]), Time(3)).expect("added");
    let changes = [
        (1, ObservationChange::TakenIn(ItemNumber(1))),
        (3, ObservationChange::Deleted),
    ];
    assert_eq!(
        observations.change("Sorting.", &changes, Time(4)),
        Ok(Ok(vec![true, true]))
    );
    let count = |tag: &str, carrying: usize, unhandled: usize| TagCount {
        tag: tag.to_string(),
        carrying,
        unhandled,
    };
    // `x` is carried by A, which is handled, and B; C is deleted, and its `z` counts for nothing.
    assert_eq!(observations.tags(), Ok(vec![count("x", 2, 1), count("y", 1, 0)]));
}
