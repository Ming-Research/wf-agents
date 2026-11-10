//! Cells that make no flow.

use super::*;

#[test]
fn a_flow_of_no_cells_is_refused() {
    assert_eq!(Flow::new(vec![]), Err(FlowFault::Empty));
}

#[test]
fn two_cells_of_one_name_are_refused() {
    let cells = vec![cell("a", &[], &[]), cell("a", &[], &[]), gate()];
    assert_eq!(Flow::new(cells), Err(FlowFault::Twice("a".into())));
}

#[test]
fn a_condition_on_a_block_the_cell_itself_writes_is_refused() {
    let cells = vec![when("more", "yes", cell("a", &[], &["more"])), gate()];
    assert_eq!(Flow::new(cells), Err(FlowFault::ConditionOnOwnOutput("a".into())));
}

#[test]
fn a_merge_with_a_condition_on_a_value_is_refused() {
    let cells = vec![cell("a", &[], &["more"]), when("more", "yes", Cell::new("merge"))];
    assert_eq!(Flow::new(cells), Err(FlowFault::LastHasCondition("merge".into())));
}

#[test]
fn a_merge_with_a_condition_on_a_named_test_is_refused() {
    let cells = vec![cell("a", &[], &[]), on_test("t", Cell::new("merge"))];
    assert_eq!(Flow::new(cells), Err(FlowFault::LastHasCondition("merge".into())));
}
