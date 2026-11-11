//! What changed of the blocks a cell says what the work is by.

use flow::{CellChange, Change};

use crate::from_valid;
use crate::support::{edit, valid};

#[test]
fn a_block_a_cell_now_describes_the_work_by_or_no_longer_does_is_a_change() {
    let outputs = r#"outputs = ["ci"]"#;
    let after = edit(
        valid(),
        "check.toml",
        outputs,
        &format!("{outputs}\ndescribes = [\"design\"]"),
    );
    assert_eq!(
        from_valid(&after),
        [Change::Cell {
            cell: "check".into(),
            change: CellChange::DescribesAdded("design".into()),
        }]
    );
    let before = after;
    let changes = flow::compare(&crate::compiled(&before), &crate::compiled(&valid()));
    assert_eq!(
        changes,
        [Change::Cell {
            cell: "check".into(),
            change: CellChange::DescribesRemoved("design".into()),
        }]
    );
}
