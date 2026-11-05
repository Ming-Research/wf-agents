//! The plans SQLite makes for the engine's statements that read a table by a column an index covers. Each test takes
//! the statement from the function or constant that holds it, so the test sees the statement the code runs.

use db::{Db, query_plan};

/// Asserts that the plan SQLite makes for `sql` reads a table by the index `index`.
fn reads_by(sql: &str, index: &str) {
    let db = Db::in_memory().unwrap();
    let steps = query_plan(db.conn(), sql).unwrap();
    assert!(
        steps.contains(index),
        "the plan does not read by {index}:\n{steps}\nthe statement:\n{sql}"
    );
}

#[test]
fn rounds_of_matter_reads_rounds_by_cell() {
    reads_by(&crate::work::rounds_of_matter_sql(), "rounds_by_cell");
}

#[test]
fn words_given_reads_interjections_by_work_delivered() {
    reads_by(
        &crate::verification::words_given_sql(),
        "interjections_by_work_delivered",
    );
}

#[test]
fn asked_reads_architect_matters_by_work() {
    reads_by(crate::review::ASKED_SQL, "architect_matters_by_work");
}

#[test]
fn blockers_of_matter_reads_blockers_by_matter() {
    reads_by(&crate::blockers::of_matter_sql(), "blockers_by_matter");
}
