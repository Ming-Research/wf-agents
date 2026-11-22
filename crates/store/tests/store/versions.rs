//! The versions of the flows works run on: each kept once, each file once, however many versions hold it; a work's
//! entries in `on_flow` and what its time went to read back as written; the records keep every name as recorded, and a
//! work read through its moves saves as it was read (decision `a-move-is-read-through-the-renumberings`).

use decide::{BlockId, CellId, Reading};
use store::{KeptFlow, KeptOutline, SCHEMA_VERSION};
use work::{KeptTime, OnFlow, Stretch, Went};

use super::opening::{columns, versions};
use super::*;

/// A version of the judged flow as the store is given it: these files, each with a digest of its own making, and an
/// outline that is the version's name in words.
fn kept(version: &str, files: &[(&str, &str)]) -> KeptFlow {
    KeptFlow {
        version: version.to_string(),
        name: "judged".to_string(),
        files: (files.iter())
            .map(|(path, text)| (path.to_string(), (format!("digest of {text}"), text.to_string())))
            .collect(),
        outline: format!("{{\"of\": \"{version}\"}}"),
    }
}

/// The rows of a table of the file.
fn count(path: &Path, table: &str) -> u64 {
    let db = Connection::open(path).expect("the database opens");
    (db.query_row(&format!("SELECT count(*) FROM {table}"), [], |row| row.get(0))).expect("the rows are counted")
}

#[test]
fn a_version_kept_twice_is_kept_once_and_a_file_two_versions_hold_once() {
    let db = Db::new();
    let mut store = db.open();
    let a = kept("a", &[("flow.toml", "one"), ("cells/S1.toml", "start")]);
    let b = kept("b", &[("flow.toml", "two"), ("cells/S1.toml", "start")]);
    store.keep_flow(&a).expect("a is kept");
    store.keep_flow(&a).expect("a is kept again, as nothing");
    store.keep_flow(&b).expect("b is kept");
    assert_eq!(count(&db.path, "flows"), 2);
    assert_eq!(count(&db.path, "flow_files"), 3, "the file both hold is kept once");
    let outline = |version: &str| KeptOutline {
        version: version.to_string(),
        name: "judged".to_string(),
        outline: format!("{{\"of\": \"{version}\"}}"),
    };
    assert_eq!(db.open().outlines(), Ok(vec![outline("a"), outline("b")]));
}

#[test]
fn a_work_s_moves_and_what_its_time_went_to_read_back_as_written_its_records_as_recorded() {
    let db = Db::new();
    let mut store = db.open();
    store.keep_flow(&kept("a", &[("flow.toml", "one")])).expect("a is kept");
    store.keep_flow(&kept("b", &[("flow.toml", "two")])).expect("b is kept");
    let number = first_work(&mut store);
    let mut d = Driver::new();
    d.work.onto(Time(START), Some("a".to_string()), Reading::default());
    d.wrote("request", Outsider::Person, None);
    d.end(hands_in(&[("text", None)]));
    // Saved as it was recorded, before the move; the move is then saved as the work's only new entry.
    store.save(number, &d.work).expect("the work is saved as recorded");
    let reading = Reading {
        cells: [(CellId::from("write"), CellId::from("drafting"))]
            .into_iter()
            .collect(),
        blocks: [(BlockId::from("text"), BlockId::from("draft"))].into_iter().collect(),
    };
    d.work.onto(Time(START + 60_000), Some("b".to_string()), reading);
    store.save(number, &d.work).expect("the work is saved with its move");
    let stretch = Stretch {
        from: Time(START),
        until: Time(START + 1000),
        went: Went::Model,
    };
    let at = d.work.tick();
    d.work.kept_time.push(KeptTime {
        at,
        stretches: vec![stretch],
    });
    store
        .save(number, &d.work)
        .expect("the work read through its move saves again, with what came after");
    let loaded = db.open().load(number).expect("the work loads");
    assert_eq!(loaded.flows, d.work.flows);
    assert_eq!(
        loaded.flows,
        [
            OnFlow {
                at: Seq(1),
                version: Some("a".to_string())
            },
            OnFlow {
                at: d.work.flows[1].at,
                version: Some("b".to_string())
            }
        ]
    );
    assert_eq!(loaded.kept_time, d.work.kept_time);
    // The store keeps every name as it was recorded; read through the move, the records are the work's.
    assert!(loaded.records.started.iter().any(|s| s.cell.as_str() == "write"));
    assert!(loaded.contents.keys().any(|(block, _)| block.as_str() == "text"));
    let mut read = loaded.clone();
    read.read_through(&d.work.readings);
    assert_eq!(read.records, d.work.records);
    assert_eq!(read.contents, d.work.contents);
    assert!(read.records.started.iter().all(|s| s.cell.as_str() != "write"));
}

#[test]
fn a_file_of_version_21_is_migrated_with_no_version_kept_no_work_on_one_and_no_time_kept() {
    let mut d = Driver::new();
    history(&mut d);
    let (old, new) = kept_in_21(&d.work);
    let store = old.open();
    assert_eq!(versions(&old.path), [SCHEMA_VERSION]);
    assert_eq!(SCHEMA_VERSION, 22);
    assert_eq!(
        columns(&old.path),
        columns(&new.path),
        "the migrated file has other tables than a new one"
    );
    assert_eq!(store.load(WorkNumber(1)), Ok(d.work));
    for table in ["flow_files", "flows", "on_flow", "kept_time"] {
        assert_eq!(count(&old.path, table), 0, "{table} holds rows");
    }
    assert_eq!(store.outlines(), Ok(Vec::new()));
}

/// The work as a file of version 21 of the schema holds it, and a file of this version that holds it.
fn kept_in_21(work: &Work) -> (Db, Db) {
    super::opening::kept_in(21, work)
}
