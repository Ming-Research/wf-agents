//! Cells put into the order, taken out of it, and moved within it.

use flow::{BlockChange, CellChange, Change, FLOW_FILE};

use crate::support::{edit, set, valid};
use crate::{from_valid, without};

const ORDER: &str = r#"order = ["plan", "judge", "design", "answer", "check", "merge"]"#;

#[test]
fn a_cell_put_in_is_added_at_its_place_and_the_cells_after_it_are_not_moved() {
    let after = edit(
        valid(),
        FLOW_FILE,
        ORDER,
        r#"order = ["plan", "research", "judge", "design", "answer", "check", "merge"]"#,
    );
    let research = "title = \"Research\"\nkind = \"agent\"\neffort = \"high\"\nline = \"engineer\"\n\
                    task = \"Research the request.\"\ninputs = [\"request\"]\n";
    let after = set(after, "research.toml", research);
    assert_eq!(
        from_valid(&after),
        [Change::CellAdded {
            cell: "research".into(),
            place: 1,
        }]
    );
}

#[test]
fn a_cell_taken_out_is_listed_where_it_stood_before_the_cells_that_followed_it() {
    let after = edit(
        valid(),
        FLOW_FILE,
        ORDER,
        r#"order = ["plan", "judge", "answer", "check", "merge"]"#,
    );
    let after = without(after, "design");
    let after = edit(after, FLOW_FILE, "[blocks.design]\n", "");
    let after = edit(
        after,
        FLOW_FILE,
        "values = [\"yes\", \"no\"]",
        "values = [\"yes\", \"no\"]\nshown = true",
    );
    let after = edit(after, "check.toml", "[\"main\", \"design\"]", "[\"main\"]");
    assert_eq!(
        from_valid(&after),
        [
            Change::CellRemoved {
                cell: "design".into(),
                place: 2,
            },
            Change::Cell {
                cell: "check".into(),
                change: CellChange::InputRemoved("design".into()),
            },
            Change::BlockRemoved { block: "design".into() },
            Change::Block {
                block: "needs_design".into(),
                change: BlockChange::Shown {
                    before: false,
                    after: true,
                },
            },
        ]
    );
}

#[test]
fn a_cell_taken_out_before_every_cell_kept_is_listed_first() {
    let after = edit(
        valid(),
        FLOW_FILE,
        ORDER,
        r#"order = ["judge", "design", "answer", "check", "merge"]"#,
    );
    let after = without(after, "plan");
    let after = edit(
        after,
        FLOW_FILE,
        "[blocks.request]\npeople = true",
        "[blocks.request]\npeople = true\nshown = true",
    );
    let after = edit(after, FLOW_FILE, "[blocks.plan]", "[blocks.plan]\npeople = true");
    let after = edit(
        after,
        FLOW_FILE,
        "[blocks.opinions]\npeople = true",
        "[blocks.opinions]\nshown = true",
    );
    let block = |block: &str, change| Change::Block {
        block: block.into(),
        change,
    };
    assert_eq!(
        from_valid(&after),
        [
            Change::CellRemoved {
                cell: "plan".into(),
                place: 0,
            },
            block(
                "opinions",
                BlockChange::People {
                    before: true,
                    after: false,
                },
            ),
            block(
                "opinions",
                BlockChange::Shown {
                    before: false,
                    after: true,
                },
            ),
            block(
                "plan",
                BlockChange::People {
                    before: false,
                    after: true,
                },
            ),
            block(
                "request",
                BlockChange::Shown {
                    before: false,
                    after: true,
                },
            ),
        ]
    );
}

#[test]
fn a_cell_that_moved_is_one_change_and_the_cells_it_passed_have_none() {
    let after = edit(
        valid(),
        FLOW_FILE,
        ORDER,
        r#"order = ["plan", "answer", "judge", "design", "check", "merge"]"#,
    );
    assert_eq!(
        from_valid(&after),
        [Change::CellMoved {
            cell: "answer".into(),
            before: 3,
            after: 1,
        }]
    );
}

#[test]
fn of_two_cells_that_changed_places_the_one_later_in_the_new_order_moved() {
    let after = edit(
        valid(),
        FLOW_FILE,
        ORDER,
        r#"order = ["plan", "judge", "design", "check", "answer", "merge"]"#,
    );
    assert_eq!(
        from_valid(&after),
        [Change::CellMoved {
            cell: "answer".into(),
            before: 3,
            after: 4,
        }]
    );
}

#[test]
fn a_cell_that_moved_and_changed_is_said_to_have_moved_before_what_changed_of_it() {
    let after = edit(
        valid(),
        FLOW_FILE,
        ORDER,
        r#"order = ["plan", "answer", "judge", "design", "check", "merge"]"#,
    );
    let after = edit(
        after,
        "answer.toml",
        "title = \"Answer comments\"",
        "title = \"Answer\"",
    );
    assert_eq!(
        from_valid(&after),
        [
            Change::CellMoved {
                cell: "answer".into(),
                before: 3,
                after: 1,
            },
            Change::Cell {
                cell: "answer".into(),
                change: CellChange::Title {
                    before: "Answer comments".to_string(),
                    after: "Answer".to_string(),
                },
            },
        ]
    );
}
