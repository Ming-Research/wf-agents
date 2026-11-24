//! The observations: a run keeps, as it finishes, those of each return of it, the ones sent back and the one it was
//! taken on, each once, with where it came from and unread; an architect adds others; each has a title and tags, and is
//! changed with a reason, taken in by an item or deleted; and the owner reads each once.

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

// A return sent back is completed by the next, which says only what was wrong, so no later return says again what an
// earlier one observed: each return of a run keeps its own, whatever sessions the rounds ran in
// (decision a-sent-back-return-is-completed-by-what-is-missing).
#[test]
fn each_return_of_a_run_keeps_its_own_observations_the_one_sent_back_and_the_one_taken() {
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
    let kept_now = |path: &Path| -> Vec<(u64, String, RunId, usize)> {
        let mut all: Vec<_> = (kept(path).into_iter())
            .map(|o| {
                let round = o.round.expect("kept from a round");
                (o.number, o.text, round.run, round.round)
            })
            .collect();
        all.reverse();
        all
    };
    // A run keeps its observations as it finishes: until then none is kept.
    assert!(
        kept_now(&path).is_empty(),
        "an observation was kept before its run finished"
    );
    // The next return says what was wrong with it only, and is taken: its observation is kept beside the first.
    d.end(handing_in(&[("text", None)], &["Said in the return taken."]));
    assert_eq!(
        kept_now(&path),
        [
            (1, "Said in a return sent back.".to_string(), run, 1),
            (2, "Said in the return taken.".to_string(), run, 2)
        ]
    );
    // Saved again whole, the work keeps nothing more.
    let s = d.saving.as_mut().expect("saved after each event");
    s.store.save(s.work, &d.work).expect("the work is saved again");
    assert_eq!(kept_now(&path).len(), 2);
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

/// The changes of the observation of this number the database keeps, in order: when each was made, the change as it is
/// kept, and why.
fn kept_changes(path: &Path, number: u64) -> Vec<(u64, String, String)> {
    let db = Connection::open(path).expect("the database opens");
    let mut query = db
        .prepare("SELECT at, change, why FROM observation_changes WHERE observation = ?1 ORDER BY n")
        .expect("the changes are read");
    let rows = query
        .query_map([number], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
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
    // Each is kept with the time of the call that made it, and the one reason.
    let said = |change: &str| (9, change.to_string(), "Sorting.".to_string());
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
        [(3, "\"Deleted\"".to_string(), "A fix removed it.".to_string())]
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
    assert_eq!(kept_changes(&db.path, 1), Vec::<(u64, String, String)>::new());
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

/// An item of this project, or of none, as it is added.
fn an_item_of(project: Option<&str>) -> store::First {
    store::First {
        title: "another".to_string(),
        level: Some("2".to_string()),
        project: project.map(str::to_string),
        depends_on: Vec::new(),
        tags: Vec::new(),
        continues: None,
    }
}

/// The refusal of taking observation 1 in by this item, as the one change asked for.
fn of_another_project(item: u64) -> Result<Result<Vec<bool>, Vec<NotMade>>, store::Fault> {
    Ok(Err(vec![NotMade {
        at: 0,
        observation: 1,
        why: Refusal::ItemOfAnotherProject(ItemNumber(item)),
    }]))
}

#[test]
fn an_item_of_another_project_or_of_none_takes_no_observation_in() {
    let (db, mut store, _) = with_an_item();
    for project in [Some("other"), None] {
        store
            .add(&an_item_of(project), "judged", Time(START + 1), &Work::default())
            .expect("the item is added");
    }
    let observations = Observations::open(&db.path).expect("the observations open");
    observations.add(&adding("Of `one`."), Time(1)).expect("added");
    for item in [2, 3] {
        let taken = [(1, ObservationChange::TakenIn(ItemNumber(item)))];
        assert_eq!(
            observations.change("Not its project.", &taken, Time(2)),
            of_another_project(item)
        );
    }
    // An item of the project it is of takes it in.
    let taken = [(1, ObservationChange::TakenIn(ItemNumber(1)))];
    assert_eq!(observations.change("Its project.", &taken, Time(3)), Ok(Ok(vec![true])));
}

#[test]
fn an_observation_a_run_kept_is_taken_in_only_by_an_item_of_its_items_project() {
    let mut d = Driver::saved();
    d.wrote("request", Outsider::Person, None);
    d.end(handing_in(&[("text", None)], &["From the run."]));
    let s = d.saving.as_mut().expect("saved");
    let path = s.db.path.clone();
    s.store
        .add(&an_item_of(Some("other")), "judged", Time(START + 1), &Work::default())
        .expect("the item is added");
    // The run's item is of the project `one`, and so is the observation: it has no project of its own but that.
    assert_eq!(kept(&path)[0].project.as_deref(), Some("one"));
    let observations = Observations::open(&path).expect("the observations open");
    let by_other = [(1, ObservationChange::TakenIn(ItemNumber(2)))];
    assert_eq!(
        observations.change("Not its project.", &by_other, Time(2)),
        of_another_project(2)
    );
    let by_its_item = [(1, ObservationChange::TakenIn(ItemNumber(1)))];
    assert_eq!(
        observations.change("Its item.", &by_its_item, Time(3)),
        Ok(Ok(vec![true]))
    );
}

#[test]
fn an_item_number_past_what_the_store_can_number_is_no_item() {
    let (db, _store, _) = with_an_item();
    let observations = Observations::open(&db.path).expect("the observations open");
    observations.add(&adding("Text."), Time(1)).expect("added");
    for item in [i64::MAX.unsigned_abs() + 1, u64::MAX] {
        let taken = [(1, ObservationChange::TakenIn(ItemNumber(item)))];
        assert_eq!(
            observations.change("Large.", &taken, Time(2)),
            Ok(Err(vec![NotMade {
                at: 0,
                observation: 1,
                why: Refusal::NoSuchItem(ItemNumber(item))
            }])),
            "{item}"
        );
    }
}

// The tests below are the testing's own. Their references: SQLite's documented transaction and foreign-key behaviour,
// the tag rule the owner chose for observations (the one item tags have: 1 to 40 characters, no whitespace, no comma),
// and the definition of a cancelled work in `db/schema.sql` (more `cancelled` rows than `reopened` rows).

#[test]
fn the_owner_reads_an_added_observation_once_and_the_reading_stays_through_a_change() {
    let (db, _store, _) = with_an_item();
    let observations = Observations::open(&db.path).expect("the observations open");
    observations.add(&adding("Added."), Time(1)).expect("added");
    observations.add(&adding("Other."), Time(2)).expect("added");
    assert_eq!(observations.read(1, Time(50)), Ok(Some(Time(50))));
    assert_eq!(observations.read(1, Time(60)), Ok(Some(Time(50))));
    let titled = [(1, ObservationChange::Titled("Named".to_string()))];
    assert_eq!(observations.change("Naming.", &titled, Time(70)), Ok(Ok(vec![true])));
    let now: Vec<(u64, Option<Time>, Option<String>)> = (observations.all().expect("read").into_iter())
        .map(|o| (o.number, o.read, o.title))
        .collect();
    assert_eq!(now, [(2, None, None), (1, Some(Time(50)), Some("Named".to_string()))]);
    // The reading of a deleted observation is kept in the records, and the observation is listed no more.
    let deleted = [(1, ObservationChange::Deleted)];
    assert_eq!(observations.change("Gone.", &deleted, Time(80)), Ok(Ok(vec![true])));
    let numbers: Vec<u64> = observations.all().expect("read").iter().map(|o| o.number).collect();
    assert_eq!(numbers, [2]);
    let reads: u64 = (Connection::open(&db.path).expect("the database opens"))
        .query_row("SELECT count(*) FROM observations_read", [], |row| row.get(0))
        .expect("the reads are counted");
    assert_eq!(reads, 1);
}

#[test]
fn changes_given_together_are_judged_in_the_order_given_each_against_those_before_it() {
    let (db, _store, _) = with_an_item();
    let observations = Observations::open(&db.path).expect("the observations open");
    observations.add(&adding("One."), Time(1)).expect("added");
    // A deletion before a title of the same batch: the title is judged against the observation as deleted.
    let late = [
        (1, ObservationChange::Deleted),
        (1, ObservationChange::Titled("Late".to_string())),
    ];
    assert_eq!(
        observations.change("Late.", &late, Time(2)),
        Ok(Err(vec![NotMade {
            at: 1,
            observation: 1,
            why: Refusal::Deleted
        }]))
    );
    // Nothing was made, so it is not deleted.
    assert_eq!(observations.all().expect("read").len(), 1);
    assert_eq!(kept_changes(&db.path, 1), Vec::<(u64, String, String)>::new());
    // A tag given and taken away in one batch is two changes, each made against what the one before left; a tag given
    // twice is one change and a no change; a title given, changed and given again is three changes.
    let made = [
        (1, ObservationChange::Tagged("x".to_string())),
        (1, ObservationChange::Untagged("x".to_string())),
        (1, ObservationChange::Tagged("y".to_string())),
        (1, ObservationChange::Tagged("y".to_string())),
        (1, ObservationChange::Titled("A".to_string())),
        (1, ObservationChange::Titled("B".to_string())),
        (1, ObservationChange::Titled("A".to_string())),
    ];
    assert_eq!(
        observations.change("Together.", &made, Time(3)),
        Ok(Ok(vec![true, true, true, false, true, true, true]))
    );
    let now = &observations.all().expect("read")[0];
    assert_eq!(
        (now.tags.clone(), now.title.clone()),
        (vec!["y".to_string()], Some("A".to_string()))
    );
    let kept: Vec<String> = kept_changes(&db.path, 1)
        .into_iter()
        .map(|(_, change, _)| change)
        .collect();
    assert_eq!(
        kept,
        [
            "{\"Tagged\":\"x\"}",
            "{\"Untagged\":\"x\"}",
            "{\"Tagged\":\"y\"}",
            "{\"Titled\":\"A\"}",
            "{\"Titled\":\"B\"}",
            "{\"Titled\":\"A\"}"
        ]
    );
}

#[test]
fn an_observation_a_run_kept_takes_a_title_tags_and_a_deletion_as_an_added_one_does() {
    let mut d = Driver::saved();
    d.wrote("request", Outsider::Person, None);
    d.end(handing_in(&[("text", None)], &["From the run.", "Also from the run."]));
    let path = d.saving.as_ref().expect("saved").db.path.clone();
    let observations = Observations::open(&path).expect("the observations open");
    let changes = [
        (1, ObservationChange::Titled("Named".to_string())),
        (1, ObservationChange::Tagged("flow".to_string())),
        (2, ObservationChange::Deleted),
    ];
    assert_eq!(
        observations.change("Sorting.", &changes, Time(START + 9)),
        Ok(Ok(vec![true, true, true]))
    );
    let all = observations.all().expect("read");
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].number, 1);
    assert_eq!(all[0].text, "From the run.");
    assert_eq!(all[0].title.as_deref(), Some("Named"));
    assert_eq!(all[0].tags, ["flow"]);
    assert!(all[0].round.is_some(), "it lost where the run kept it");
    // A run that keeps another afterwards gives it the next number, past the deleted one, and the work saved again
    // keeps nothing twice.
    d.end(handing_in(
        &[("opinions", None), ("verdict", Some("yes"))],
        &["From the judge."],
    ));
    let s = d.saving.as_mut().expect("saved after each event");
    s.store.save(s.work, &d.work).expect("the work is saved again");
    let listed: Vec<(u64, String)> = (observations.all().expect("read").into_iter())
        .map(|o| (o.number, o.text))
        .collect();
    assert_eq!(
        listed,
        [(3, "From the judge.".to_string()), (1, "From the run.".to_string())]
    );
}

#[test]
fn a_tag_of_forty_characters_of_any_script_is_carried_and_one_of_forty_one_or_with_a_space_or_a_comma_is_not() {
    let (db, _store, _) = with_an_item();
    let observations = Observations::open(&db.path).expect("the observations open");
    observations.add(&adding("Text."), Time(1)).expect("added");
    let tag = |tag: String| [(1, ObservationChange::Tagged(tag))];
    // Forty Chinese characters are 120 bytes: a tag is measured in characters.
    let wide = "标".repeat(40);
    assert_eq!(
        observations.change("Wide.", &tag(wide.clone()), Time(2)),
        Ok(Ok(vec![true]))
    );
    assert_eq!(observations.all().expect("read")[0].tags, [wide]);
    for bad in [
        "标".repeat(41),
        "x".repeat(41),
        String::new(),
        "a,b".to_string(),
        "a\tb".to_string(),
        "a\u{a0}b".to_string(),
        " ".to_string(),
    ] {
        assert_eq!(
            observations.change("Bad.", &tag(bad.clone()), Time(3)),
            Ok(Err(vec![NotMade {
                at: 0,
                observation: 1,
                why: Refusal::BadTag(bad.clone())
            }])),
            "{bad:?}"
        );
    }
    assert_eq!(
        observations.change("Edge.", &tag("x".repeat(40)), Time(4)),
        Ok(Ok(vec![true]))
    );
}

#[test]
fn observations_added_from_connections_at_once_are_numbered_without_a_gap_or_a_repeat() {
    let (db, _store, _) = with_an_item();
    let path = db.path.clone();
    let threads: Vec<_> = (0..4)
        .map(|t| {
            let path = path.clone();
            std::thread::spawn(move || {
                let observations = Observations::open(&path).expect("the observations open");
                (0..8)
                    .map(|i| {
                        let mut new = adding(&format!("{t}-{i}"));
                        new.tags = vec![format!("t{t}")];
                        observations.add(&new, Time(1)).expect("added")
                    })
                    .collect::<Vec<u64>>()
            })
        })
        .collect();
    let mut numbers: Vec<u64> = threads
        .into_iter()
        .flat_map(|t| t.join().expect("the thread ends"))
        .collect();
    numbers.sort_unstable();
    assert_eq!(numbers, (1..=32).collect::<Vec<u64>>());
    let observations = Observations::open(&db.path).expect("the observations open");
    let all = observations.all().expect("read");
    assert_eq!(all.len(), 32);
    // Each carries its own tag, which the one connection that added it gave.
    assert!(
        all.iter()
            .all(|o| o.tags == [format!("t{}", o.text.split('-').next().unwrap())])
    );
}

#[test]
fn an_observation_taken_in_is_handled_only_while_its_work_is_cancelled_no_oftener_than_reopened() {
    let (db, mut store, work) = with_an_item();
    let observations = Observations::open(&db.path).expect("the observations open");
    observations.add(&adding("Taken."), Time(1)).expect("added");
    let taken = [(1, ObservationChange::TakenIn(ItemNumber(1)))];
    assert_eq!(observations.change("Held.", &taken, Time(2)), Ok(Ok(vec![true])));
    let handled = |observations: &Observations| observations.all().expect("read")[0].handled;
    let mut d = Driver::new();
    let mut says = Vec::new();
    for event in [
        Event::Cancelled { why: None },
        Event::Reopened { why: "Wanted.".into() },
        Event::Cancelled { why: None },
        Event::Reopened { why: "Again.".into() },
    ] {
        d.on(event);
        store.save(work, &d.work).expect("the work is saved");
        says.push(handled(&observations));
    }
    // Cancelled once, reopened once, cancelled twice, reopened twice.
    assert_eq!(says, [false, true, false, true]);
}

// A row of the observations that no longer matches what came back, or whose run has no time, is a fault that names the
// work as a work's number is written everywhere (`W1`), not bare. The rows are made to disagree by hand, since the
// engine never writes them so. Reference: the one writer of a work's number, `WorkNumber`'s `Display`.
#[test]
fn an_observation_that_disagrees_with_its_records_is_a_fault_naming_its_work_as_a_works_number_is_written() {
    let mut d = Driver::saved();
    d.wrote("request", Outsider::Person, None);
    d.end(handing_in(&[("text", None)], &["The only one."]));
    let s = d.saving.as_ref().expect("saved after each event");
    let (work, path) = (s.work, s.db.path.clone());
    let named = work.to_string();
    assert!(named.starts_with('W') && named.len() > 1, "{named}");
    let fault = |path: &Path| match Observations::open(path).expect("the observations open").all() {
        Err(store::Fault::Value { table, message }) => (table, message),
        other => panic!("not a fault of a value: {other:?}"),
    };
    let tamper = |sql: &str| {
        let file = Connection::open(path.as_path()).expect("the database opens");
        file.execute_batch(sql).expect("the rows are changed by hand");
    };
    // The observation is the 7th text of what came back, which has one.
    tamper("UPDATE observations SET n = 7");
    let (table, message) = fault(&path);
    assert_eq!(table, "observations");
    assert!(
        message.starts_with(&format!(
            "O1 is the text 7 of the observations of what came back of {named}, run "
        )),
        "{message}"
    );
    tamper("UPDATE observations SET n = 1");
    assert_eq!(kept(&path).len(), 1, "the row is whole again");
    // The run's finish has no time in the work's moments.
    tamper("DELETE FROM moments");
    let (_, message) = fault(&path);
    assert!(
        message.starts_with(&format!("O1: {named} has no time for its run ")),
        "{message}"
    );
}
