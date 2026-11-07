//! The next transaction number is read under the write lock by every write. Its plan must search the index on the
//! change log's `tx`, not scan the log: the scan is what made each write slower as the log grew.

use db::{Db, NEXT_TX};

// The plan of the statement `Db::write` reads the next number with searches `changes_by_tx`, and does not scan `changes`.
#[test]
fn the_next_transaction_number_is_read_from_the_index_on_changes() {
    let db = Db::in_memory().unwrap();
    let mut st = db.conn().prepare(&format!("EXPLAIN QUERY PLAN {NEXT_TX}")).unwrap();
    let steps: Vec<String> = st
        .query_map([], |r| r.get::<_, String>(3))
        .unwrap()
        .map(|x| x.unwrap())
        .collect();
    let plan = steps.join("\n");
    assert!(
        plan.contains("changes_by_tx"),
        "{NEXT_TX} does not search changes_by_tx:\n{plan}"
    );
    assert!(
        !plan.contains("SCAN changes"),
        "{NEXT_TX} scans the change log:\n{plan}"
    );
}
