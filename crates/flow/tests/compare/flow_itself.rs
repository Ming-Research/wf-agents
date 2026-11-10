//! What belongs to the flow as a whole: its name, its limits, its named tests.

use flow::{CellChange, Change, FLOW_FILE, Limit};

use crate::from_valid;
use crate::support::{edit, valid};

#[test]
fn the_same_definition_has_no_changes() {
    assert_eq!(from_valid(&valid()), []);
}

#[test]
fn a_new_name_is_a_change_of_the_flow() {
    let after = edit(valid(), FLOW_FILE, "name = \"small\"", "name = \"smaller\"");
    assert_eq!(
        from_valid(&after),
        [Change::Name {
            before: "small".to_string(),
            after: "smaller".to_string(),
        }]
    );
}

#[test]
fn each_limit_of_the_flow_that_changed_is_one_change_and_the_cells_that_follow_it_have_none() {
    let after = edit(valid(), FLOW_FILE, "runs = 3", "runs = 4");
    let after = edit(after, FLOW_FILE, "calls = 40", "calls = 50");
    assert_eq!(
        from_valid(&after),
        [
            Change::Limit {
                limit: Limit::Runs,
                before: 3,
                after: 4,
            },
            Change::Limit {
                limit: Limit::Calls,
                before: 40,
                after: 50,
            },
        ]
    );
}

#[test]
fn tests_added_removed_and_reworded_come_in_the_order_of_their_names() {
    let after = edit(
        valid(),
        FLOW_FILE,
        "has_comment = \"The item has a comment\"",
        "has_comment = \"Someone commented on the item\"",
    );
    let after = edit(
        after,
        FLOW_FILE,
        "may_merge = \"The work may merge\"",
        "ci_green = \"The CI is green\"",
    );
    let after = edit(after, "merge.toml", "\"may_merge\"", "\"ci_green\"");
    assert_eq!(
        from_valid(&after),
        [
            Change::TestAdded {
                test: "ci_green".into(),
                words: "The CI is green".to_string(),
            },
            Change::TestReworded {
                test: "has_comment".into(),
                before: "The item has a comment".to_string(),
                after: "Someone commented on the item".to_string(),
            },
            Change::TestRemoved {
                test: "may_merge".into(),
                words: "The work may merge".to_string(),
            },
            Change::Cell {
                cell: "merge".into(),
                change: CellChange::PreconditionRemoved("may_merge".into()),
            },
            Change::Cell {
                cell: "merge".into(),
                change: CellChange::PreconditionAdded("ci_green".into()),
            },
        ]
    );
}
