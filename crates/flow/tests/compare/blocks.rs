//! Blocks declared, no longer declared, and changed.

use flow::{BlockChange, CellChange, Change, FLOW_FILE};

use crate::support::{edit, valid};
use crate::{compiled, from_valid};

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
fn a_block_that_becomes_a_note_or_stops_being_one_is_a_change() {
    let noted = edit(
        valid(),
        FLOW_FILE,
        "[blocks.opinions]\npeople = true\n",
        "[blocks.opinions]\npeople = true\nnote = true\n",
    );
    let note = |before, after| of("opinions", BlockChange::Note { before, after });
    assert_eq!(from_valid(&noted), [note(false, true)]);
    assert_eq!(
        flow::compare(&compiled(&noted), &compiled(&valid())),
        [note(true, false)]
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
                change: CellChange::RunsOnAdded("notes".into()),
            },
            Change::Cell {
                cell: "answer".into(),
                change: CellChange::GivenAdded("notes".into()),
            },
            Change::BlockAdded { block: "notes".into() },
        ]
    );
}

#[test]
fn what_a_block_is_is_a_change_with_both_values() {
    let says = |what: &str| {
        edit(
            valid(),
            FLOW_FILE,
            "[blocks.plan]",
            &format!("[blocks.plan]\nwhat = \"{what}\""),
        )
    };
    assert_eq!(
        from_valid(&says("What will be done.")),
        [of(
            "plan",
            BlockChange::What {
                before: None,
                after: Some("What will be done.".to_string()),
            },
        )]
    );
    assert_eq!(
        flow::compare(&compiled(&says("What will be done.")), &compiled(&says("The steps."))),
        [of(
            "plan",
            BlockChange::What {
                before: Some("What will be done.".to_string()),
                after: Some("The steps.".to_string()),
            },
        )]
    );
}
