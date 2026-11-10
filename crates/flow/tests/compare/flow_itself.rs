//! What belongs to the flow as a whole: its name, its limits, its named tests.

use flow::{CellChange, Change, FLOW_FILE, Limit, Test, TestKind};

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
        "says = \"The item has a comment\"",
        "says = \"Someone commented on the item\"",
    );
    let after = edit(
        after,
        FLOW_FILE,
        "[tests.may_merge]\nsays = \"The work may merge\"",
        "[tests.ci_green]\nsays = \"The CI is green\"",
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

#[test]
fn a_test_s_kind_blocks_read_or_value_changed_is_one_change_after_its_rewording() {
    let may_merge = "says = \"The work may merge\"\nkind = \"none\"\nreads = [\"ci\"]\nis = \"red\"";
    let changed = "says = \"The CI is green\"\nkind = \"all\"\nreads = [\"ci\"]\nis = \"green\"";
    let after = edit(valid(), FLOW_FILE, may_merge, changed);
    let after = edit(
        after,
        FLOW_FILE,
        "reads = [\"confirmed\"]",
        "reads = [\"confirmed\", \"needs_design\"]",
    );
    let test = |says: &str, kind, reads: &[&str], is: &str| Test {
        says: says.to_string(),
        kind,
        reads: reads.iter().map(|&b| b.into()).collect(),
        is: is.to_string(),
    };
    let confirmed = "The owner confirmed the plan";
    assert_eq!(
        from_valid(&after),
        [
            Change::TestReworded {
                test: "may_merge".into(),
                before: "The work may merge".to_string(),
                after: "The CI is green".to_string(),
            },
            Change::TestChanged {
                test: "may_merge".into(),
                before: test("The work may merge", TestKind::None, &["ci"], "red"),
                after: test("The CI is green", TestKind::All, &["ci"], "green"),
            },
            Change::TestChanged {
                test: "plan_confirmed".into(),
                before: test(confirmed, TestKind::All, &["confirmed"], "yes"),
                after: test(confirmed, TestKind::All, &["confirmed", "needs_design"], "yes"),
            },
        ]
    );
}
