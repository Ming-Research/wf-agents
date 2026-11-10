//! What each input shows: its current version, the version the cell's record knows, and whether its meaning changed.

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
fn a_cell_that_wrote_a_block_it_reads_knows_its_own_version() {
    let mut w = World::with_gate(vec![cell("design", &["plan"], &["plan"])]);
    let first = w.person_writes("plan");
    let own = w.run("design", &[("plan", Matters)]);
    assert_eq!(
        input_of(&w, "design", "plan"),
        input("plan", Some(own), Some(own), false)
    );
    assert_ne!(first, own);
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
fn an_input_of_a_cell_with_no_record_knows_nothing_and_has_not_changed() {
    let mut w = planned();
    let background = w.person_writes("background");
    assert_eq!(
        input_of(&w, "research", "background"),
        input("background", Some(background), None, false)
    );
}
