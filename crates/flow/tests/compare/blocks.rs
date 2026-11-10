//! Blocks declared, no longer declared, and changed.

use flow::{BlockChange, CellChange, Change, FLOW_FILE};

use crate::from_valid;
use crate::support::{edit, valid};

fn of(block: &str, change: BlockChange) -> Change {
    Change::Block {
        block: block.into(),
        change,
    }
}

#[test]
fn a_block_s_values_who_writes_it_besides_cells_and_whether_it_is_shown_are_each_a_change() {
    let after = edit(
        valid(),
        FLOW_FILE,
        "values = [\"green\", \"red\"]",
        "values = [\"green\", \"red\", \"cancelled\"]",
    );
    let after = edit(
        after,
        FLOW_FILE,
        "[blocks.comments]\npeople = true",
        "[blocks.comments]\npeople = true\noutside = \"main\"",
    );
    let after = edit(
        after,
        FLOW_FILE,
        "outside = \"main\"\n\n[blocks.ci]",
        "outside = \"mirror\"\n\n[blocks.ci]",
    );
    let after = edit(
        after,
        FLOW_FILE,
        "[blocks.opinions]\npeople = true\n",
        "[blocks.opinions]\n",
    );
    let after = edit(after, FLOW_FILE, "[blocks.plan]", "[blocks.plan]\nshown = true");
    let names = |names: &[&str]| Some(names.iter().map(|n| n.to_string()).collect());
    assert_eq!(
        from_valid(&after),
        [
            of(
                "ci",
                BlockChange::Values {
                    before: names(&["green", "red"]),
                    after: names(&["green", "red", "cancelled"]),
                },
            ),
            of(
                "comments",
                BlockChange::Outside {
                    before: None,
                    after: Some("main".to_string()),
                },
            ),
            of(
                "main",
                BlockChange::Outside {
                    before: Some("main".to_string()),
                    after: Some("mirror".to_string()),
                },
            ),
            of(
                "opinions",
                BlockChange::People {
                    before: true,
                    after: false,
                },
            ),
            of(
                "plan",
                BlockChange::Shown {
                    before: false,
                    after: true,
                },
            ),
        ]
    );
}

#[test]
fn a_block_that_becomes_a_choice_changes_its_values_from_none() {
    let after = edit(
        valid(),
        FLOW_FILE,
        "[blocks.design]",
        "[blocks.design]\nvalues = [\"done\"]",
    );
    assert_eq!(
        from_valid(&after),
        [of(
            "design",
            BlockChange::Values {
                before: None,
                after: Some(vec!["done".to_string()]),
            },
        )]
    );
}

#[test]
fn a_block_declared_is_added_after_the_changes_of_the_cells_that_use_it() {
    let after = edit(
        valid(),
        FLOW_FILE,
        "[blocks.comments]",
        "[blocks.notes]\npeople = true\n\n[blocks.comments]",
    );
    let after = edit(
        after,
        "answer.toml",
        "inputs = [\"comments\", \"plan\"]",
        "inputs = [\"comments\", \"plan\", \"notes\"]",
    );
    assert_eq!(
        from_valid(&after),
        [
            Change::Cell {
                cell: "answer".into(),
                change: CellChange::InputAdded("notes".into()),
            },
            Change::BlockAdded { block: "notes".into() },
        ]
    );
}
