//! A CLI session's rounds are read with the session's `cli_session`, joined to each round by its session. The plan of
//! that read must search the index on the rounds' session, so that it does not scan every round of every work.

use db::{Db, rusqlite};
use sessions::round::own_rounds_sql;

// The plan of the statement that reads a CLI session's rounds searches `rounds_by_session`.
#[test]
fn a_cli_sessions_rounds_are_read_from_the_index_on_rounds_session() {
    let db = Db::in_memory().unwrap();
    let sql = own_rounds_sql();
    let mut st = db.conn().prepare(&format!("EXPLAIN QUERY PLAN {sql}")).unwrap();
    // The statement takes one parameter, the CLI session; the plan does not depend on its value.
    let steps: Vec<String> = st
        .query_map(rusqlite::params![None::<String>], |r| r.get::<_, String>(3))
        .unwrap()
        .map(|x| x.unwrap())
        .collect();
    let plan = steps.join("\n");
    assert!(
        plan.contains("rounds_by_session"),
        "{sql} does not search rounds_by_session:\n{plan}"
    );
}
