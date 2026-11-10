//! The order of the cells, and how a work ends.

use super::*;

#[test]
fn the_cells_run_in_their_order_and_a_work_ends_when_the_merge_has_a_record() {
    let mut w = World::new(vec![cell("a", &[], &["p"]), cell("b", &["p"], &[]), Cell::new("merge")]);
    assert_eq!(w.decide(), runs("a", vec![NO_RECORD], &[("p", None)]));
    let p = w.run("a", &[("p", Matters)]);
    assert_eq!(w.decide(), runs("b", vec![NO_RECORD], &[("p", Some(p))]));
    w.run("b", &[]);
    assert_eq!(w.decide(), runs("merge", vec![NO_RECORD], &[]));
    w.run("merge", &[]);
    assert_eq!(w.decide(), Decision::Ended(End::Merged));
}

#[test]
fn a_merge_waits_for_its_precondition() {
    let mut w = World::with_gate(vec![]);
    assert_eq!(w.decide(), wait("merge", vec![merge_word()]));
    w.set("the merge may go", Outcome::Holds);
    assert_eq!(w.decide(), runs("merge", vec![NO_RECORD], &[]));
}

#[test]
fn a_cancelled_work_has_ended() {
    let mut w = World::with_gate(vec![cell("a", &[], &[])]);
    w.cancel();
    assert_eq!(w.decide(), Decision::Ended(End::Cancelled));
}

#[test]
fn a_work_cancelled_while_a_run_is_at_rest_has_ended() {
    let mut w = World::with_gate(vec![cell("a", &[], &[]), cell("b", &[], &[])]);
    w.start("a");
    w.returns("a");
    w.cancel();
    assert_eq!(w.decide(), Decision::Ended(End::Cancelled));
}

#[test]
fn a_work_that_passed_and_is_then_cancelled_has_passed() {
    let mut w = World::new(vec![cell("a", &[], &[])]);
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
