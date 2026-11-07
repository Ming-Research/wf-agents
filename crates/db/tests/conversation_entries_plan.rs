//! The plan SQLite makes for the view `conversation_entries`, which the owner's conversations are read through. Each table
//! the view probes by a column is read by that column's index: a probe that scans the table reads every row of it, for
//! each entry the view lists.

use db::{Db, query_plan};

#[test]
fn conversation_entries_reads_each_table_by_its_index() {
    let db = Db::in_memory().unwrap();
    let steps = query_plan(db.conn(), "SELECT * FROM conversation_entries").unwrap();
    for index in [
        "deliveries_by_matter",
        "messages_by_ask",
        "architect_matters_by_asked",
        "blockers_by_ask",
        "blockers_by_matter",
    ] {
        assert!(steps.contains(index), "the plan does not read by {index}:\n{steps}");
    }
}
