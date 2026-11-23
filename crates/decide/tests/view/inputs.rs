//! What each block that runs a cell shows: its current version, the version the cell consumed, and the versions that are new
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
        input("background", Some(second), Some(first), &[second])
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
        input("plan", Some(own), Some(first), &[])
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
        input("plan", Some(again), Some(plan), &[])
    );
    assert_eq!(cell_of(&w, "implement").needs, vec![]);
}

#[test]
fn a_block_with_a_version_is_new_to_a_cell_that_consumed_none() {
    let mut w = planned();
    let background = w.person_writes("background");
    assert_eq!(
        input_of(&w, "research", "background"),
        input("background", Some(background), None, &[background])
    );
}

#[test]
fn of_several_versions_in_at_once_only_those_that_changed_the_meaning_are_new_to_the_cell() {
    let mut w = planned();
    w.written_outside("background", "spec");
    let first = w.outside_writes("background");
    w.run("research", &[("plan", Matters)]);
    let second = w.outside_writes("background");
    w.outside_writes_not_mattering("background");
    let fourth = w.outside_writes("background");
    w.outside_writes_not_mattering("background");
    let latest = w.outside_writes_not_mattering("background");
    let seen = input_of(&w, "research", "background");
    assert_eq!(seen.consumed, Some(first));
    assert_eq!(seen.current, Some(latest));
    assert_eq!(seen.new_versions, vec![second, fourth]);
}

#[test]
fn a_version_the_cell_wrote_is_not_new_to_it_though_an_earlier_one_of_another_is() {
    let mut w = World::with_gate(vec![cell("design", &["plan"], &["plan"])]);
    let first = w.person_writes("plan");
    w.run("design", &[("plan", Matters)]);
    let theirs = w.person_writes("plan");
    // The cell wrote a later version without dealing with the one before it: it consumed nothing of it.
    w.start("design");
    w.finish_dealing("design", &[], &[("plan", Matters)]);
    let seen = input_of(&w, "design", "plan");
    assert_eq!(seen.consumed, Some(first));
    assert_eq!(seen.new_versions, vec![theirs]);
}
