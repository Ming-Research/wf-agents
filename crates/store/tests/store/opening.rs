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
    assert_eq!(tables.len(), 41);
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
fn kept_in(version: u32, work: &Work) -> (Db, Db) {
    let new = Db::new();
    {
        let mut store = new.open();
        let number = first_work(&mut store);
        store.save(number, work).expect("the work is saved");
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
        for migration in &MIGRATIONS[..version as usize - 1] {
            file.execute_batch(migration).expect("the migration runs");
        }
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

/// The migrations, in order, each bringing a file one version further, as the store's own list holds them.
pub(crate) const MIGRATIONS: [&str; 13] = [
    include_str!("../../../../db/migrations/202611101530-say-who-paused-a-work-and-why.sql"),
    include_str!("../../../../db/migrations/202611102304-give-items-a-level.sql"),
    include_str!("../../../../db/migrations/202611111400-say-when-a-work-let-go-of-what-it-held.sql"),
    include_str!("../../../../db/migrations/202611111401-keep-the-owners-pause-of-everything.sql"),
    include_str!("../../../../db/migrations/202611111600-record-the-sessions-of-lines-and-the-context-of-rounds.sql"),
    include_str!("../../../../db/migrations/202611111830-keep-each-change-of-an-item-and-why-it-was-cancelled.sql"),
    include_str!("../../../../db/migrations/202611112000-say-who-paused-everything.sql"),
    include_str!("../../../../db/migrations/202611112100-record-backups.sql"),
    include_str!(
        "../../../../db/migrations/202611131500-keep-what-a-return-dealt-with-what-reached-a-round-and-what-filled-it.sql"
    ),
    include_str!("../../../../db/migrations/202611141600-keep-the-observations-the-returns-taken-carry.sql"),
    include_str!("../../../../db/migrations/202611141700-give-items-a-project.sql"),
    include_str!("../../../../db/migrations/202611141900-keep-the-questions-put-to-the-owner-and-the-answers.sql"),
    include_str!(
        "../../../../db/migrations/202611142000-say-why-each-change-of-an-item-was-made-tag-items-and-reopen-them.sql"
    ),
];

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
    // Nor were its questions and answers entries of their own.
    work.records.questions.clear();
    work.records.answers.clear();
    let (old, new) = kept_in(9, &work);
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
    assert_eq!(migrated, super::observations::kept(&new.path));
    let said: Vec<(u64, &str, &str)> = (migrated.iter())
        .map(|o| (o.number, o.text.as_str(), o.cell.as_str()))
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
fn a_file_of_every_earlier_version_is_brought_to_this_one_as_it_is_opened() {
    assert_eq!(
        MIGRATIONS.len() as u32 + 1,
        SCHEMA_VERSION,
        "a migration is missing here"
    );
    let new = Db::new();
    new.open();
    for version in 1..SCHEMA_VERSION {
        let old = Db::new();
        {
            let file = Connection::open(&old.path).expect("the database opens");
            file.execute_batch(SCHEMA_1).expect("the first schema is made");
            file.execute("INSERT INTO schema_version (version) VALUES (1)", [])
                .expect("its version is written");
            for migration in &MIGRATIONS[..version as usize - 1] {
                file.execute_batch(migration).expect("the migration runs");
            }
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

/// The migration that brought version 1 of the schema to version 2.
const TO_2: &str = include_str!("../../../../db/migrations/202611101530-say-who-paused-a-work-and-why.sql");

#[test]
fn an_item_kept_before_items_had_levels_is_opened_at_none() {
    let db = Db::new();
    {
        let file = Connection::open(&db.path).expect("the database opens");
        file.execute_batch(SCHEMA_1).expect("the first schema is made");
        file.execute("INSERT INTO schema_version (version) VALUES (1)", [])
            .expect("its version is written");
        file.execute_batch(TO_2).expect("the file is brought to version 2");
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
        for migration in &MIGRATIONS[..10] {
            file.execute_batch(migration).expect("the migration runs");
        }
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
