//! The plan SQLite makes for the rounds a CLI session ran: they are found through the session's row, so the rounds table
//! is read by its `session` column and not scanned whole.

use db::{Db, query_plan};

#[test]
fn own_rounds_reads_rounds_by_session() {
    let db = Db::in_memory().unwrap();
    let steps = query_plan(db.conn(), &sessions::round::own_rounds_sql()).unwrap();
    assert!(
        steps.contains("rounds_by_session"),
        "the plan does not read by rounds_by_session:\n{steps}"
    );
}
