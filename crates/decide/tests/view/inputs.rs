//! What each block that runs a cell shows: its current version, the version the cell consumed, and whether it is new
//! to the cell.

use super::*;

#[test]
fn an_input_changed_since_the_record_shows_the_version_the_record_started_from_and_that_it_changed() {
    let mut w = planned();
    let first = w.person_writes("background");
    w.run("research", &[("plan", Matters)]);
    let second = w.person_writes("background");
    assert_eq!(
        input_of(&w, "research", "background"),
        input("background", Some(second), Some(first), true)
    );
    let because = vec![Cause::Changed {
        block: "background".into(),
        was: Some(first),
        behind: false,
    }];
    assert_eq!(cell_of(&w, "research").needs, because);
}

#[test]
fn a_cell_that_wrote_a_block_that_runs_it_consumed_the_version_it_was_told_of_and_its_own_is_not_new_to_it() {
    let mut w = World::with_gate(vec![cell("design", &["plan"], &["plan"])]);
    let first = w.person_writes("plan");
    let own = w.run("design", &[("plan", Matters)]);
    assert_eq!(
        input_of(&w, "design", "plan"),
        input("plan", Some(own), Some(first), false)
    );
}

#[test]
fn a_write_said_not_to_matter_moves_the_current_version_and_leaves_the_input_unchanged() {
    let mut w = planned();
    w.person_writes("background");
    let plan = w.run("research", &[("plan", Matters)]);
    w.run("implement", &[("code", Matters)]);
    w.person_writes("background");
    let again = w.run("research", &[("plan", DoesNotMatter)]);
    assert_eq!(
        input_of(&w, "implement", "plan"),
        input("plan", Some(again), Some(plan), false)
    );
    assert_eq!(cell_of(&w, "implement").needs, vec![]);
}

#[test]
fn a_block_with_a_version_is_new_to_a_cell_that_consumed_none() {
    let mut w = planned();
    let background = w.person_writes("background");
    assert_eq!(
        input_of(&w, "research", "background"),
        input("background", Some(background), None, true)
    );
}
