//! The order of the cells, and how a work ends.

use super::*;

#[test]
fn the_cells_run_in_their_order_each_for_what_is_new_to_it_and_a_work_ends_when_the_merge_has_a_record() {
    let mut w = World::new(vec![
        cell("a", &["brief"], &["p"]),
        cell("b", &["p"], &["q"]),
        cell("merge", &["q"], &[]),
    ]);
    // Nothing has a version, so nothing runs a cell: the work is stalled at the merge, which only b writes what runs.
    assert_eq!(w.decide(), stalled("merge", &[], &[("b", Unrun::NothingNew)]));
    let brief = w.person_writes("brief");
    assert_eq!(
        w.decide(),
        runs("a", first(&["brief"]), &[("brief", Some(brief)), ("p", None)])
    );
    let p = w.run("a", &[("p", Matters)]);
    assert_eq!(w.decide(), runs("b", first(&["p"]), &[("p", Some(p)), ("q", None)]));
    let q = w.run("b", &[("q", Matters)]);
    assert_eq!(w.decide(), runs("merge", first(&["q"]), &[("q", Some(q))]));
    w.run("merge", &[]);
    assert_eq!(w.decide(), Decision::Ended(End::Merged));
}

#[test]
fn a_work_in_which_nothing_new_runs_a_cell_is_stalled_at_its_last_cell_until_someone_writes_what_runs_one() {
    let mut w = World::new(vec![cell("a", &["brief"], &["p"]), cell("merge", &["p"], &[])]);
    w.person_writes("brief");
    // The cell deals with the brief and hands in nothing new: nothing runs the merge, and only a writes what runs it.
    w.run("a", &[]);
    assert_eq!(w.decide(), stalled("merge", &[], &[("a", Unrun::NothingNew)]));
    w.person_writes("brief");
    assert!(matches!(w.decide(), Decision::Run { cell, .. } if cell.as_str() == "a"));
}

#[test]
fn a_merge_waits_for_its_precondition() {
    let mut w = World::with_gate(vec![]);
    // Nothing runs the merge yet, and its precondition holds the work there all the same.
    assert_eq!(w.decide(), wait("merge", vec![merge_word()]));
    let word = w.says("the merge may go", true);
    assert_eq!(
        w.decide(),
        runs(
            "merge",
            first(&["the merge may go"]),
            &[("the merge may go", Some(word))]
        )
    );
}

#[test]
fn a_cancelled_work_has_ended() {
    let mut w = World::with_gate(vec![cell("a", &["brief"], &[])]);
    w.person_writes("brief");
    w.cancel();
    assert_eq!(w.decide(), Decision::Ended(End::Cancelled));
}

#[test]
fn a_cancelled_work_abandons_its_run_at_rest_and_then_has_ended() {
    let mut w = World::with_gate(vec![cell("a", &["brief"], &[]), cell("b", &["brief"], &[])]);
    w.person_writes("brief");
    let a = w.start("a");
    w.returns("a");
    w.cancel();
    assert_eq!(w.decide(), abandon("a", a, Abandoning::Cancelled));
    w.abandon();
    assert_eq!(w.decide(), Decision::Ended(End::Cancelled));
}

#[test]
fn a_cancelled_work_abandons_even_a_run_with_a_round_under_way() {
    let mut w = World::with_gate(vec![cell("a", &["brief"], &[])]);
    w.person_writes("brief");
    let a = w.start("a");
    w.cancel();
    assert_eq!(w.decide(), abandon("a", a, Abandoning::Cancelled));
}

#[test]
fn a_cancelled_work_abandons_each_open_run_in_turn_in_the_order_they_started() {
    let mut w = World::with_gate(vec![cell("a", &["x"], &[]), cell("b", &["y"], &[])]);
    w.person_writes("x");
    w.person_writes("y");
    w.run("a", &[]);
    let b = w.start("b");
    w.calls("b", Some(Who::Owner));
    w.person_writes("x");
    let a = w.start("a");
    w.cancel();
    assert_eq!(w.decide(), abandon("b", b, Abandoning::Cancelled));
    w.abandon();
    assert_eq!(w.decide(), abandon("a", a, Abandoning::Cancelled));
    w.abandon();
    assert_eq!(w.decide(), Decision::Ended(End::Cancelled));
}

#[test]
fn a_work_that_passed_and_is_then_cancelled_has_passed() {
    let mut w = World::new(vec![cell("a", &["brief"], &[])]);
    w.person_writes("brief");
    w.run("a", &[]);
    w.cancel();
    assert_eq!(w.decide(), Decision::Ended(End::Merged));
}

#[test]
fn an_ended_work_stays_ended_whatever_is_written_after() {
    let mut w = World::new(vec![cell("a", &["topic"], &["plan"]), cell("merge", &["plan"], &[])]);
    w.person_writes("topic");
    w.run("a", &[("plan", Matters)]);
    w.run("merge", &[]);
    w.person_writes("topic");
    w.person_writes("plan");
    w.pause();
    assert_eq!(w.decide(), Decision::Ended(End::Merged));
}
