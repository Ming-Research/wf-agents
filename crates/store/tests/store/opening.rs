//! Opening a database: a new one is made with the schema, one made before is opened with what it holds, and one of
//! another version of the schema is refused.

use store::{Fault, ItemNumber, SCHEMA, SCHEMA_VERSION};

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
    assert_eq!(tables.len(), 24);
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
            .add_item("second", &[ItemNumber(1)], Time(START + 1))
            .expect("the item is added");
        store.save(work, &d.work).expect("the work is saved");
    }
    let before = dump(&db.path);
    let store = db.open();
    assert_eq!(dump(&db.path), before, "opening changed the database");
    assert_eq!(store.items().expect("the items").len(), 2);
    assert_eq!(store.works().expect("the works").len(), 1);
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
