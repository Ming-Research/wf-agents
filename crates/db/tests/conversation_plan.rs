//! The owner's conversations are read from the view `conversation_entries`, one conversation at a time. Its correlated
//! subqueries look up the rows of one table by a column each, and each lookup must search that column's index: without
//! it, every entry of the view scans the table it looks in.

use db::Db;

// The plan of the view, read for one conversation with every column, searches the index on each column its subqueries
// look up by. The query reads every column: when it read only `id`, the plan showed none of the subqueries of the other
// columns, so their indexes could not be seen.
#[test]
fn the_conversation_entries_view_searches_the_indexes_of_its_subqueries() {
    let db = Db::in_memory().unwrap();
    let mut st = db
        .conn()
        .prepare("EXPLAIN QUERY PLAN SELECT * FROM conversation_entries WHERE conversation = 'work:1'")
        .unwrap();
    let steps: Vec<String> = st
        .query_map([], |r| r.get::<_, String>(3))
        .unwrap()
        .map(|x| x.unwrap())
        .collect();
    let plan = steps.join("\n");
    for index in [
        "interjections_by_matter",
        "engine_notices_by_ask",
        "architect_matters_by_asked",
        "blockers_by_ask",
        "blockers_by_matter",
    ] {
        assert!(
            plan.contains(index),
            "the plan of conversation_entries does not search {index}:\n{plan}"
        );
    }
}
