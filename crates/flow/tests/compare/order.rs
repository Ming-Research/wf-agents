//! The order of the whole list, with changes of every part at once.

use flow::{CellChange, Change, FLOW_FILE, Limit};

use crate::support::{edit, set, valid};
use crate::{from_valid, without};

#[test]
fn the_flow_s_changes_come_first_then_the_cells_in_the_new_order_then_the_blocks_by_name() {
    // A block that people write, first by name among the blocks.
    let after = edit(
        valid(),
        FLOW_FILE,
        "[blocks.ci]",
        "[blocks.agenda]\npeople = true\nshown = true\n\n[blocks.ci]",
    );
    let after = edit(after, FLOW_FILE, "returns = 2", "returns = 1");
    let after = edit(after, FLOW_FILE, "name = \"small\"", "name = \"smaller\"");
    // Check is taken out, answer moves to the front, and a cell is put in where check was. Check followed answer,
    // which moved, so it is listed after design, the cell before it that stayed.
    let after = edit(
        after,
        FLOW_FILE,
        r#"order = ["plan", "judge", "design", "answer", "check", "merge"]"#,
        r#"order = ["answer", "plan", "judge", "design", "test", "merge"]"#,
    );
    let after = without(after, "check");
    let test = "title = \"Test\"\nkind = \"engine\"\ninputs = [\"main\", \"design\"]\noutputs = [\"ci\"]\n";
    let after = set(after, "test.toml", test);
    let after = edit(
        after,
        "judge.toml",
        "inputs = [\"plan\"]",
        "inputs = [\"plan\", \"agenda\"]",
    );
    let after = edit(
        after,
        FLOW_FILE,
        "says = \"The item has a comment\"",
        "says = \"Someone commented\"",
    );
    assert_eq!(
        from_valid(&after),
        [
            Change::Name {
                before: "small".to_string(),
                after: "smaller".to_string(),
            },
            Change::Limit {
                limit: Limit::Returns,
                before: 2,
                after: 1,
            },
            Change::TestReworded {
                test: "has_comment".into(),
                before: "The item has a comment".to_string(),
                after: "Someone commented".to_string(),
            },
            Change::CellMoved {
                cell: "answer".into(),
                before: 3,
                after: 0,
            },
            Change::Cell {
                cell: "judge".into(),
                change: CellChange::InputAdded("agenda".into()),
            },
            Change::CellRemoved {
                cell: "check".into(),
                place: 4,
            },
            Change::CellAdded {
                cell: "test".into(),
                place: 4,
            },
            Change::BlockAdded { block: "agenda".into() },
        ]
    );
}
