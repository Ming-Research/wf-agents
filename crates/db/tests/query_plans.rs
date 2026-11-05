//! The plans SQLite makes for the statements that run on every write and on the owner's page reads. A step that reads a
//! table by a column no index covers scans the whole table, and each write or read pays for that scan.

use db::{Db, NEXT_TX, query_plan};

#[test]
fn next_transaction_number_reads_changes_by_changes_by_tx() {
    let db = Db::in_memory().unwrap();
    let steps = query_plan(db.conn(), NEXT_TX).unwrap();
    assert!(
        steps.contains("changes_by_tx"),
        "the plan does not read by changes_by_tx:\n{steps}"
    );
    assert!(!steps.contains("SCAN changes"), "the plan scans changes:\n{steps}");
}

#[test]
fn conversation_entries_reads_each_table_by_its_index() {
    let db = Db::in_memory().unwrap();
    let steps = query_plan(db.conn(), "SELECT * FROM conversation_entries").unwrap();
    for index in [
        "interjections_by_matter",
        "engine_notices_by_ask",
        "architect_matters_by_asked",
        "blockers_by_ask",
        "blockers_by_matter",
    ] {
        assert!(steps.contains(index), "the plan does not read by {index}:\n{steps}");
    }
}
