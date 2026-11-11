//! What changed of what a cell's rounds run on: its effort, and the levels above its item's its line runs at.

use flow::{CellChange, Change, Effort, FLOW_FILE};

use crate::from_valid;
use crate::support::{edit, valid};

fn of(cell: &str, change: CellChange) -> Change {
    Change::Cell {
        cell: cell.into(),
        change,
    }
}

#[test]
fn an_effort_changed_is_a_change_of_its_cell() {
    let after = edit(valid(), "plan.toml", "effort = \"high\"", "effort = \"xhigh\"");
    let changed = CellChange::Effort {
        before: Some(Effort::High),
        after: Some(Effort::Xhigh),
    };
    assert_eq!(from_valid(&after), [of("plan", changed)]);
}

#[test]
fn a_line_that_comes_to_run_above_its_item_s_level_is_a_change_of_each_cell_on_it() {
    let after = edit(
        valid(),
        FLOW_FILE,
        "[lines.engineer]",
        "[lines.engineer]\nlevels_above = 1",
    );
    let above = || CellChange::LevelsAbove { before: 0, after: 1 };
    assert_eq!(
        from_valid(&after),
        [of("plan", above()), of("design", above()), of("answer", above())]
    );
}
