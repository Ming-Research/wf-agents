//! The plan SQLite makes for the flow versions the owner's interface lists, each with the number of works on it.

use db::{Db, query_plan};

#[test]
fn flows_count_works_by_works_by_flow_version() {
    let db = Db::in_memory().unwrap();
    let steps = query_plan(db.conn(), crate::read::FLOWS_SQL).unwrap();
    assert!(
        steps.contains("works_by_flow_version"),
        "the plan does not read by works_by_flow_version:\n{steps}"
    );
}
