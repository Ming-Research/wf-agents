//! Opening a database: a new one is made with the schema, one made before is opened with what it holds, one made with
//! an earlier version of the schema is copied whole and then migrated to this one, and one of any other version is
//! refused.

use store::{Fault, ItemNumber, SCHEMA, SCHEMA_VERSION, before_migration};

use super::*;

/// The tables the schema makes, as it names them.
fn schema_tables() -> BTreeSet<String> {
    (SCHEMA.lines())
        .filter_map(|line| line.strip_prefix("CREATE TABLE "))
        .map(|rest| rest.split_whitespace().next().expect("a name").to_string())
        .collect()
}

#[test]
fn a_new_file_is_made_with_the_schema_its_version_and_a_write_ahead_log() {
    let db = Db::new();
    let store = db.open();
    assert_eq!(store.items(), Ok(vec![]));
    assert_eq!(store.works(), Ok(vec![]));
    let tables: BTreeSet<String> = dump(&db.path).into_keys().collect();
    assert_eq!(tables, schema_tables());
    assert_eq!(tables.len(), 52);
    let file = Connection::open(&db.path).expect("the database opens");
    let version: u32 = (file.query_row("SELECT version FROM schema_version", [], |row| row.get(0))).expect("a version");
    assert_eq!(version, SCHEMA_VERSION);
    let mode: String = (file.query_row("PRAGMA journal_mode", [], |row| row.get(0))).expect("a journal mode");
    assert_eq!(mode, "wal");
}

#[test]
fn a_file_made_before_is_opened_with_what_it_holds() {
    let mut d = Driver::new();
    history(&mut d);
    let db = Db::new();
    {
        let mut store = db.open();
        let work = first_work(&mut store);
        store
            .add(
                &first("second", "2", &[ItemNumber(1)]),
                "judged",
                Time(START + 1),
                &Work::default(),
            )
            .expect("the item is added");
        store.save(work, &d.work).expect("the work is saved");
    }
    let before = dump(&db.path);
    let store = db.open();
    assert_eq!(dump(&db.path), before, "opening changed the database");
    assert_eq!(store.items().expect("the items").len(), 2);
    assert_eq!(store.works().expect("the works").len(), 2);
    assert_eq!(store.load(WorkNumber(1)), Ok(d.work));
}

#[test]
fn a_file_of_another_version_of_the_schema_is_refused() {
    let db = Db::new();
    let file = Connection::open(&db.path).expect("the database opens");
    file.execute_batch(
        "CREATE TABLE schema_version (version INTEGER PRIMARY KEY); INSERT INTO schema_version VALUES (999);",
    )
    .expect("the database is made");
    drop(file);
    let found = Store::open(&db.path).err();
    let expected = SCHEMA_VERSION;
    assert_eq!(
        found,
        Some(Fault::Schema {
            found: Some(999),
            expected
        })
    );
}

#[test]
fn a_file_with_tables_and_no_version_is_refused() {
    let db = Db::new();
    let file = Connection::open(&db.path).expect("the database opens");
    file.execute_batch("CREATE TABLE elsewhere (x INTEGER);")
        .expect("the database is made");
    drop(file);
    let expected = SCHEMA_VERSION;
    assert_eq!(
        Store::open(&db.path).err(),
        Some(Fault::Schema { found: None, expected })
    );
    // Nothing was made in it.
    let tables: BTreeSet<String> = dump(&db.path).into_keys().collect();
    assert_eq!(tables, ["elsewhere".to_string()].into());
}

/// Version 1 of the schema, as databases were made with it before a pause said who paused the work and why.
pub(crate) const SCHEMA_1: &str = include_str!("schema-1.sql");

/// The columns of each table of the database, with their types, whether they may be null, their defaults and their
/// places in the keys.
pub(crate) fn columns(path: &Path) -> BTreeMap<String, Vec<Vec<String>>> {
    let db = Connection::open(path).expect("the database opens");
    let mut out = BTreeMap::new();
    for table in dump(path).into_keys() {
        let mut query = db
            .prepare(&format!("PRAGMA table_info({table})"))
            .expect("the table is described");
        let rows: Vec<Vec<String>> = (query.query_map([], |row| {
            (1..6)
                .map(|i| row.get::<_, Value>(i).map(|v| format!("{v:?}")))
                .collect::<Result<Vec<_>, _>>()
        }))
        .expect("the table is described")
        .collect::<Result<_, _>>()
        .expect("the table is described");
        out.insert(table, rows);
    }
    out
}

/// Every row of the table of the schema's version.
pub(crate) fn versions(path: &Path) -> Vec<u32> {
    let file = Connection::open(path).expect("the database opens");
    let mut query = file
        .prepare("SELECT version FROM schema_version")
        .expect("the versions are read");
    let rows = query.query_map([], |row| row.get(0)).expect("the versions are read");
    rows.collect::<Result<_, _>>().expect("the versions are read")
}

/// A work as one kept before the engine paused works itself: the owner pauses it as it is added, and lets it on; the
/// writer hands in its text and the judge asks the owner a question.
fn kept_before() -> Work {
    let mut d = Driver::new();
    d.on(Event::Paused {
        by: Pauser::Owner,
        why: None,
    });
    d.wrote("request", Outsider::Person, None);
    d.on(Event::Resumed);
    d.end(hands_in(&[("text", None)]));
    d.end(Came::asks(vec![question(Who::Owner)]));
    // Its rounds were kept before a round said the context it ended with and how often it was compacted, and its
    // runs before a return said what it dealt with.
    for spent in d.work.spent.values_mut() {
        spent.context_tokens = None;
        spent.compactions = 0;
    }
    for finished in &mut d.work.records.finished {
        finished.dealt_with = None;
    }
    d.work
}

/// The work as a file made with this earlier version of the schema holds it, and a file of this version that holds
/// it: the rows a store of this version keeps, each copied into the earlier file in the columns its table has there.
pub(crate) fn kept_in(version: u32, work: &Work) -> (Db, Db) {
    kept_in_works(version, &[work])
}

/// As [`kept_in`], of several works, the first of the first item and each after it of an item of its own.
fn kept_in_works(version: u32, works: &[&Work]) -> (Db, Db) {
    let new = Db::new();
    {
        let mut store = new.open();
        let number = first_work(&mut store);
        store.save(number, works[0]).expect("the work is saved");
        for (i, work) in works.iter().enumerate().skip(1) {
            let (_, number) = store
                .add(
                    &first("another item", "2", &[]),
                    "judged",
                    Time(START + i as u64),
                    &Work::default(),
                )
                .expect("the item is added");
            store.save(number, work).expect("the work is saved");
        }
    }
    let old = Db::new();
    {
        let file = Connection::open(&old.path).expect("the database opens");
        // The rows are copied table by table, in no order of what they name.
        file.pragma_update(None, "foreign_keys", "OFF")
            .expect("the foreign keys are let be");
        file.execute_batch(SCHEMA_1).expect("the first schema is made");
        file.execute("INSERT INTO schema_version (version) VALUES (1)", [])
            .expect("its version is written");
        store::migrate(&file, 1, version).expect("the migrations run");
        let path = new.path.to_str().expect("the path is text");
        file.execute("ATTACH DATABASE ?1 AS new", [path])
            .expect("the new file is attached");
        for table in dump(&old.path).into_keys().filter(|t| t != "schema_version") {
            let mut query = file
                .prepare(&format!("PRAGMA main.table_info({table})"))
                .expect("the table is described");
            let names: Vec<String> = (query.query_map([], |row| row.get(1)).expect("its columns"))
                .collect::<Result<_, _>>()
                .expect("its columns");
            let names = names.join(", ");
            file.execute(
                &format!("INSERT INTO main.{table} ({names}) SELECT {names} FROM new.{table}"),
                [],
            )
            .expect("the rows are copied");
        }
    }
    (old, new)
}

#[test]
fn a_file_made_with_the_first_version_of_the_schema_is_migrated_and_its_pauses_read_as_the_owners() {
    let work = kept_before();
    let (old, new) = kept_in(1, &work);
    let pauses: u64 = (Connection::open(&old.path).expect("the database opens"))
        .query_row("SELECT count(*) FROM pauses", [], |row| row.get(0))
        .expect("the pauses are counted");
    assert_eq!(pauses, 2, "the old file holds the pause and the letting on");

    let store = old.open();
    assert_eq!(store.load(WorkNumber(1)), Ok(work));
    // The version's one row reads this version, as it does in a new file: code that reads the row finds no older one.
    assert_eq!(versions(&old.path), [SCHEMA_VERSION]);
    assert_eq!(versions(&new.path), [SCHEMA_VERSION]);
    assert_eq!(
        columns(&old.path),
        columns(&new.path),
        "the migrated file has other tables than a new one"
    );
    // Opened again, it is opened as it is.
    drop(store);
    let before = dump(&old.path);
    old.open();
    assert_eq!(dump(&old.path), before, "opening the migrated file again changed it");
}

#[test]
fn a_file_of_version_9_is_migrated_and_its_runs_read_as_dealing_with_everything_that_ran_them() {
    // A work of the engine as one kept at version 9 holds it: its runs say nothing of what they dealt with; one of its
    // versions was handed in as changing nothing in meaning.
    let mut d = Driver::new();
    history(&mut d);
    let mut work = d.work;
    for finished in &mut work.records.finished {
        finished.dealt_with = None;
    }
    work.records.finished[0].writes[0].meaning_unchanged = true;
    // Nor were its questions and answers entries of their own, and it kept no pull request.
    work.records.questions.clear();
    work.records.answers.clear();
    work.pulls.clear();
    // Its tries recorded the directory of the program's settings as their account, which version 21 keeps apart.
    for ran in &mut work.ran {
        ran.on.account = None;
    }
    let mut kept = work.clone();
    for ran in &mut kept.ran {
        ran.on.account = ran.on.directory.take();
    }
    let (old, new) = kept_in(9, &kept);
    let mut store = old.open();
    assert_eq!(versions(&old.path), [SCHEMA_VERSION]);
    assert_eq!(
        columns(&old.path),
        columns(&new.path),
        "the migrated file has other tables than a new one"
    );
    // Read as before: no run says what it dealt with, and what mattered and what did not is read as it was kept.
    let loaded = store.load(WorkNumber(1)).expect("the work loads");
    assert!(loaded.records.finished.iter().all(|f| f.dealt_with.is_none()));
    assert!(loaded.records.finished[0].writes[0].meaning_unchanged);
    assert_eq!(loaded, work);
    // A work of the new form is kept in it beside, as in a file made at this version.
    let new = of_the_new_form();
    let (_, number) = store
        .add(&first("second", "2", &[]), "judged", Time(START), &Work::default())
        .expect("the item is added");
    store.save(number, &new).expect("the work is saved");
    let opened = old.open();
    assert_eq!(opened.load(number), Ok(new));
    assert_eq!(opened.load(WorkNumber(1)), Ok(work));
}

#[test]
fn a_file_of_version_10_is_migrated_with_the_observations_of_the_returns_its_runs_were_taken_on() {
    // A work as one kept at version 10 holds it: a return of the writer's sent back and the one taken, and the judge's
    // taken, each saying observations.
    let mut d = Driver::new();
    d.wrote("request", Outsider::Person, None);
    d.end(Came {
        said: super::observations::observing(&["Said in a return sent back."]),
        ..wanting()
    });
    d.end(Came {
        said: super::observations::observing(&["The writer's first.", "The writer's second."]),
        ..hands_in(&[("text", None)])
    });
    d.end(Came {
        said: super::observations::observing(&["The judge's."]),
        ..hands_in(&[("opinions", None), ("verdict", Some("yes"))])
    });
    let (old, new) = kept_in(10, &d.work);
    old.open();
    assert_eq!(versions(&old.path), [SCHEMA_VERSION]);
    assert_eq!(
        columns(&old.path),
        columns(&new.path),
        "the migrated file has other tables than a new one"
    );
    // Its observations are kept as a file of this version keeps them as the runs finish: those of the returns taken,
    // numbered in the order the runs finished, and unread.
    let migrated = super::observations::kept(&old.path);
    // The item was kept before items had projects, so its observations are of no project and reported by none, which
    // the new file's, of an item of a project, are not.
    let without_projects: Vec<store::Observation> = (super::observations::kept(&new.path).into_iter())
        .map(|o| store::Observation {
            project: None,
            reported_by: None,
            ..o
        })
        .collect();
    assert_eq!(migrated, without_projects);
    let said: Vec<(u64, &str, &str)> = (migrated.iter())
        .map(|o| {
            (
                o.number,
                o.text.as_str(),
                o.round.as_ref().expect("a round").cell.as_str(),
            )
        })
        .collect();
    assert_eq!(
        said,
        [
            (3, "The judge's.", "judge"),
            (2, "The writer's second.", "write"),
            (1, "The writer's first.", "write")
        ]
    );
    assert!(migrated.iter().all(|o| o.read.is_none()));
}

#[test]
fn a_file_of_version_22_keeps_its_observations_and_their_readings_as_it_is_migrated() {
    let mut d = Driver::new();
    d.wrote("request", Outsider::Person, None);
    d.end(Came {
        said: super::observations::observing(&["The first.", "The second."]),
        ..hands_in(&[("text", None)])
    });
    let (old, new) = kept_in(22, &d.work);
    // The owner read the first, in the file made before.
    (Connection::open(&old.path).expect("the database opens"))
        .execute("INSERT INTO observations_read (observation, at) VALUES (1, 5000)", [])
        .expect("the reading is kept");
    old.open();
    assert_eq!(versions(&old.path), [SCHEMA_VERSION]);
    assert_eq!(
        columns(&old.path),
        columns(&new.path),
        "the migrated file has other tables than a new one"
    );
    let migrated = super::observations::kept(&old.path);
    let said: Vec<(u64, &str, Option<Time>)> = (migrated.iter()).map(|o| (o.number, o.text.as_str(), o.read)).collect();
    assert_eq!(
        said,
        [(2, "The second.", None), (1, "The first.", Some(Time(5000)))],
        "the observations or their readings were not kept"
    );
    assert!(
        migrated
            .iter()
            .all(|o| o.round.is_some() && o.title.is_none() && o.tags.is_empty())
    );
    // The migrated file takes an observation an architect adds, numbered after those kept.
    let observations = store::Observations::open(&old.path).expect("the observations open");
    let added = store::NewObservation {
        text: "Added.".to_string(),
        title: None,
        tags: Vec::new(),
        project: "one".to_string(),
        reported_by: "one".to_string(),
    };
    assert_eq!(observations.add(&added, Time(6000)), Ok(3));
}

/// A work as one kept at version 17 holds it, in which a version is empty where `empty` says so and holds words
/// otherwise, and the same with `inverse` the other way round: the same places, blocks and writers, the two works
/// differing only in which version holds nothing. A request a person wrote, the owner's main written by the outside
/// as one that does not matter and said to a cell, a choice the outside wrote with an empty content, and a run's
/// writes, one of them changing nothing in meaning and one with a value.
fn kept_at_17(inverse: bool) -> Work {
    // Whether the version of the pattern's `empty` holds nothing in this work.
    let nothing = |empty: bool| empty != inverse;
    let content = |empty: bool| if nothing(empty) { " \t\n" } else { "words" }.to_string();
    let mut d = Driver::new();
    d.on(Event::Wrote {
        block: "request".into(),
        by: Outsider::Person,
        content: content(true),
        matters: true,
        value: None,
    });
    d.on(Event::Wrote {
        block: "main".into(),
        by: Outsider::Outside,
        content: content(false),
        matters: false,
        value: None,
    });
    d.on(Event::Wrote {
        block: "checks".into(),
        by: Outsider::Outside,
        content: String::new(),
        matters: true,
        value: Some("pass".to_string()),
    });
    d.end(hands_in(&[("text", None)]));
    let mut work = d.work;
    for w in &mut work.records.written {
        w.empty = match w.block.as_str() {
            "request" => nothing(true),
            "main" => {
                w.said_to = Some(decide::CellId("write".into()));
                nothing(false)
            }
            _ => false,
        };
    }
    let finished = &mut work.records.finished[0];
    let at = finished.at;
    finished.writes[0].empty = nothing(false);
    finished.writes.push(Write {
        block: "request".into(),
        meaning_unchanged: true,
        value: None,
        empty: nothing(true),
    });
    finished.writes.push(Write {
        block: "verdict".into(),
        meaning_unchanged: false,
        value: Some("no".to_string()),
        empty: false,
    });
    work.contents.insert(("text".into(), at), content(false));
    work.contents.insert(("request".into(), at), content(true));
    work.contents.insert(("verdict".into(), at), String::new());
    work
}

#[test]
fn a_file_of_version_17_is_migrated_and_each_of_its_writes_reads_as_holding_what_its_content_does() {
    let (kept, other) = (kept_at_17(false), kept_at_17(true));
    let of = |w: &Work| -> Vec<bool> {
        (w.records.written.iter().map(|w| w.empty))
            .chain(w.records.finished[0].writes.iter().map(|w| w.empty))
            .collect()
    };
    assert_eq!(of(&kept), [true, false, false, false, true, false]);
    assert_eq!(of(&other), [false, true, false, true, false, false]);
    let (old, new) = kept_in_works(17, &[&kept, &other]);
    let store = old.open();
    assert_eq!(versions(&old.path), [SCHEMA_VERSION]);
    assert_eq!(
        columns(&old.path),
        columns(&new.path),
        "the migrated file has other tables than a new one"
    );
    // The migrated file says of each version what a file of this version does, each row with all its columns.
    let new = new.open();
    assert_eq!(new.load(WorkNumber(1)), Ok(kept.clone()));
    assert_eq!(new.load(WorkNumber(2)), Ok(other.clone()));
    assert_eq!(store.load(WorkNumber(1)), Ok(kept));
    assert_eq!(store.load(WorkNumber(2)), Ok(other));
}

/// A file of version 17 that lacks something `make` takes out of it, given the places of the request written and of the
/// run's finish, and says what the refusal says: it is not opened, nothing of it is changed, and the copy taken before
/// holds it as it was.
fn lacking(make: impl Fn(&Connection, i64, i64) -> String) {
    let (old, _) = kept_in(17, &kept_at_17(false));
    let file = Connection::open(&old.path).expect("the database opens");
    let written: i64 =
        (file.query_row("SELECT at FROM written WHERE block = 'request'", [], |r| r.get(0))).expect("a place");
    let finished: i64 = (file.query_row("SELECT at FROM finished WHERE run = 1", [], |r| r.get(0))).expect("a place");
    let wanted = make(&file, written, finished);
    drop(file);
    let before = dump(&old.path);
    match Store::open(&old.path) {
        Err(Fault::Database(said)) => assert!(said.contains(&wanted), "{said:?} does not say {wanted:?}"),
        other => panic!("opened or failed otherwise: {:?}", other.map(|_| ())),
    }
    assert_eq!(dump(&old.path), before, "the file was changed");
    let copies = before_migration::copies(&before_migration::dir(&old.path));
    assert_eq!(copies.len(), 1);
    assert_eq!(dump(&copies[0]), before, "the copy is not the file as it was");
}

#[test]
fn a_file_of_version_17_that_lacks_the_content_of_a_version_written_is_not_opened_and_is_left_as_it_was() {
    lacking(|file, written, _| {
        file.execute(
            "DELETE FROM contents WHERE block = 'request' AND version = ?1",
            [written],
        )
        .expect("the content is lacking");
        format!("the content of the version of request written at {written} of work 1 is missing")
    });
}

#[test]
fn a_file_of_version_17_that_lacks_the_content_of_a_version_a_run_wrote_is_not_opened_and_is_left_as_it_was() {
    lacking(|file, _, finished| {
        file.execute(
            "DELETE FROM contents WHERE block = 'verdict' AND version = ?1",
            [finished],
        )
        .expect("the content is lacking");
        format!("the content of the version of verdict written by run 1 at {finished} of work 1 is missing")
    });
}

#[test]
fn a_file_of_version_17_with_a_write_of_a_run_that_has_no_end_is_not_opened_and_is_left_as_it_was() {
    lacking(|file, _, _| {
        file.pragma_update(None, "foreign_keys", "OFF")
            .expect("the foreign keys are let be");
        file.execute("DELETE FROM finished WHERE run = 1", [])
            .expect("the run is lacking");
        "by run 1 of work 1 has no finished run".to_string()
    });
}

#[test]
fn a_file_of_version_20_is_migrated_keeping_every_row_it_holds_its_tries_directories_moved_apart_and_no_account() {
    // A work as the engine keeps one, its tries' accounts none; and as a file of version 20 held it, where what a try
    // recorded as its account was the directory of the program's settings.
    let mut d = Driver::new();
    history(&mut d);
    let mut work = d.work;
    for ran in &mut work.ran {
        ran.on.account = None;
    }
    let mut at_20 = work.clone();
    for ran in &mut at_20.ran {
        ran.on.account = ran.on.directory.take();
    }
    assert!(
        at_20.ran.iter().any(|ran| ran.on.account.is_some()),
        "a try ran with a directory"
    );
    let (old, new) = kept_in(20, &at_20);
    let before = dump(&old.path);
    let store = old.open();
    assert_eq!(versions(&old.path), [SCHEMA_VERSION]);
    let after = dump(&old.path);
    for (table, rows) in &before {
        if table == "schema_version" || table == "ran" {
            continue;
        }
        assert_eq!(after.get(table), Some(rows), "the rows of {table}");
    }
    // Each try's row is carried whole, what it held as its account its directory, and its account none.
    let mut moved: Vec<Vec<String>> = (before["ran"].iter())
        .map(|row| {
            let mut row = row.clone();
            row.push("Null".to_string());
            row
        })
        .collect();
    moved.sort();
    assert_eq!(after["ran"], moved);
    assert_eq!(after.get("accounts"), Some(&Vec::new()));
    assert_eq!(
        columns(&old.path),
        columns(&new.path),
        "the migrated file has other tables than a new one"
    );
    assert_eq!(store.load(WorkNumber(1)), Ok(work));
    assert_eq!(store.accounts(), Ok(Vec::new()));
}

#[test]
fn a_file_of_every_earlier_version_is_brought_to_this_one_as_it_is_opened() {
    let new = Db::new();
    new.open();
    for version in 1..SCHEMA_VERSION {
        let old = Db::new();
        {
            let file = Connection::open(&old.path).expect("the database opens");
            file.execute_batch(SCHEMA_1).expect("the first schema is made");
            file.execute("INSERT INTO schema_version (version) VALUES (1)", [])
                .expect("its version is written");
            store::migrate(&file, 1, version).expect("the migrations run");
            assert_eq!(versions(&old.path), [version]);
        }
        old.open();
        assert_eq!(versions(&old.path), [SCHEMA_VERSION], "from version {version}");
        assert_eq!(columns(&old.path), columns(&new.path), "from version {version}");
        // The migrated file keeps the records of the copies like a new one.
        let backups = store::Backups::open(&old.path).expect("the records of the copies open");
        backups.taken("store-1.sqlite.gz", Time(1)).expect("a copy is recorded");
        assert_eq!(
            backups.records().expect("the records").copies.len(),
            1,
            "from version {version}"
        );
        // And it has no observation, as a new one has none.
        assert_eq!(
            super::observations::kept(&old.path),
            Vec::<store::Observation>::new(),
            "from version {version}"
        );
    }
}

#[test]
fn an_item_kept_before_items_had_levels_is_opened_at_none() {
    let db = Db::new();
    {
        let file = Connection::open(&db.path).expect("the database opens");
        file.execute_batch(SCHEMA_1).expect("the first schema is made");
        file.execute("INSERT INTO schema_version (version) VALUES (1)", [])
            .expect("its version is written");
        store::migrate(&file, 1, 2).expect("the file is brought to version 2");
        file.execute("INSERT INTO items (title, added) VALUES ('kept before', 5)", [])
            .expect("an item is kept");
    }
    let mut store = db.open();
    assert_eq!(versions(&db.path), [SCHEMA_VERSION]);
    let items = store.items().expect("the items are read");
    assert_eq!(items.len(), 1);
    assert_eq!(
        (items[0].title.as_str(), items[0].level.as_deref()),
        ("kept before", None)
    );
    // An item added now has its level.
    store
        .add(&first("added now", "1", &[]), "judged", Time(START), &Work::default())
        .expect("the item is added");
    assert_eq!(store.items().unwrap()[1].level.as_deref(), Some("1"));
}

// An item kept before items had projects belonged to the installation's one project, which had no name: it is opened
// with none, and an item added beside it has the project it is added to.
#[test]
fn an_item_kept_before_items_had_projects_is_opened_with_none() {
    let db = Db::new();
    {
        let file = Connection::open(&db.path).expect("the database opens");
        file.execute_batch(SCHEMA_1).expect("the first schema is made");
        file.execute("INSERT INTO schema_version (version) VALUES (1)", [])
            .expect("its version is written");
        store::migrate(&file, 1, 11).expect("the migrations run");
        assert_eq!(versions(&db.path), [11]);
        file.execute(
            "INSERT INTO items (title, added, level) VALUES ('kept before', 5, '2')",
            [],
        )
        .expect("an item is kept");
    }
    let mut store = db.open();
    assert_eq!(versions(&db.path), [SCHEMA_VERSION]);
    store
        .add(
            &store::First {
                project: Some("two".to_string()),
                ..first("added now", "1", &[ItemNumber(1)])
            },
            "judged",
            Time(START),
            &Work::default(),
        )
        .expect("the item is added, depending on the one kept before");
    let items = store.items().expect("the items are read");
    assert_eq!(items[0].project, None);
    assert_eq!(items[1].project.as_deref(), Some("two"));
    assert_eq!(items[1].first.project.as_deref(), Some("two"));
    assert_eq!(items[1].depends_on, [ItemNumber(1)]);
}

#[test]
fn a_migration_that_fails_leaves_the_file_as_it_was() {
    let db = Db::new();
    let file = Connection::open(&db.path).expect("the database opens");
    // A file of version 1 whose table of pauses is not the one the migration remakes.
    file.execute_batch(
        "CREATE TABLE schema_version (version INTEGER PRIMARY KEY); INSERT INTO schema_version VALUES (1); \
         CREATE TABLE pauses (x INTEGER);",
    )
    .expect("the database is made");
    drop(file);
    let before = dump(&db.path);
    assert!(matches!(Store::open(&db.path), Err(Fault::Database(_))));
    assert_eq!(dump(&db.path), before);
    // The copy taken before it holds the file as it was too.
    let copies = before_migration::copies(&before_migration::dir(&db.path));
    assert_eq!(copies.len(), 1);
    assert_eq!(dump(&copies[0]), before);
}

/// A file made with the first version of the schema, which holds one item.
fn made_with_the_first_version(path: &Path) {
    let file = Connection::open(path).expect("the database opens");
    file.execute_batch(SCHEMA_1).expect("the first schema is made");
    file.execute("INSERT INTO schema_version (version) VALUES (1)", [])
        .expect("its version is written");
    file.execute("INSERT INTO items (number, title, added) VALUES (1, 'kept', 1)", [])
        .expect("an item is kept");
}

/// The names of these files.
fn names(files: &[PathBuf]) -> Vec<String> {
    (files.iter())
        .map(|f| f.file_name().expect("a file's name").to_string_lossy().into_owned())
        .collect()
}

#[test]
fn a_file_of_an_earlier_version_is_copied_whole_before_it_is_migrated() {
    let db = Db::new();
    made_with_the_first_version(&db.path);
    let before = dump(&db.path);
    let dir = before_migration::dir(&db.path);
    assert!(before_migration::copies(&dir).is_empty());
    db.open();
    assert_eq!(versions(&db.path), [SCHEMA_VERSION]);
    // The copy is the file as it stood, at the version it had, and its name says that version.
    let copies = before_migration::copies(&dir);
    assert_eq!(names(&copies), ["before-migration-000001-v1.sqlite"]);
    assert_eq!(dump(&copies[0]), before);
    assert_eq!(versions(&copies[0]), [1]);
    // Opened again, at this version, it is not copied again.
    db.open();
    assert_eq!(before_migration::copies(&dir).len(), 1);
}

#[test]
fn the_newest_copies_taken_before_a_migration_are_kept_and_no_other_file_is_removed() {
    let db = Db::new();
    made_with_the_first_version(&db.path);
    let dir = before_migration::dir(&db.path);
    std::fs::create_dir_all(&dir).expect("the directory of the copies is made");
    // Copies of earlier migrations, two more than are kept, and a daily copy, which is not one of them.
    let earlier = before_migration::KEEP as u64 + 2;
    for n in 1..=earlier {
        std::fs::write(dir.join(format!("before-migration-{n:06}-v1.sqlite")), b"").expect("a copy is put there");
    }
    let daily = dir.join("store-0000000000001.sqlite.gz");
    std::fs::write(&daily, b"").expect("a daily copy is put there");
    db.open();
    // The new copy is numbered after the last, and the oldest go until the newest are left.
    let kept = names(&before_migration::copies(&dir));
    let oldest = earlier + 2 - before_migration::KEEP as u64;
    let expected: Vec<String> = (oldest..=earlier + 1)
        .map(|n| format!("before-migration-{n:06}-v1.sqlite"))
        .collect();
    assert_eq!(kept.len(), before_migration::KEEP);
    assert_eq!(kept, expected);
    assert!(daily.exists());
}
