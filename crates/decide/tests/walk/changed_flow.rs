//! A changed flow walked on the same records.

use super::*;

#[test]
fn a_cell_new_to_the_flow_has_consumed_nothing_and_runs_for_what_has_a_version_when_the_walk_reaches_it() {
    let mut w = World::with_gate(vec![cell("a", &["brief"], &["p"]), cell("b", &["brief"], &[])]);
    w.person_writes("brief");
    let p = w.run("a", &[("p", Matters)]);
    w.run("b", &[]);
    w.install(vec![
        cell("a", &["brief"], &["p"]),
        cell("new", &["p"], &[]),
        cell("b", &["brief"], &[]),
        gate(),
    ]);
    assert_eq!(w.decide(), runs("new", first(&["p"]), &[("p", Some(p))]));
}

#[test]
fn what_a_cell_taken_out_of_the_flow_wrote_stays_current() {
    let mut w = World::with_gate(vec![cell("a", &["brief"], &["p"]), cell("b", &["p"], &[])]);
    w.person_writes("brief");
    let p = w.run("a", &[("p", Matters)]);
    w.run("b", &[]);
    w.install(vec![cell("b", &["p"], &[]), cell("c", &["p"], &[]), gate()]);
    assert_eq!(w.decide(), runs("c", first(&["p"]), &[("p", Some(p))]));
}

#[test]
fn a_run_at_rest_of_a_cell_taken_out_of_the_flow_is_abandoned() {
    let mut w = World::with_gate(vec![cell("a", &["brief"], &[]), cell("b", &["brief"], &[])]);
    w.person_writes("brief");
    w.run("a", &[]);
    let b = w.start("b");
    w.returns("b");
    w.install(vec![cell("a", &["brief"], &[]), gate()]);
    assert_eq!(w.decide(), abandon("b", b, Abandoning::NoLongerApplies));
    w.abandon();
    assert_eq!(w.decide(), resting());
}

#[test]
fn a_changed_specification_runs_its_cell_when_the_owner_says_it_matters_and_not_otherwise() {
    let mut w = World::with_gate(vec![cell("design", &["design's specification"], &[])]);
    let s1 = w.outside_writes("design's specification");
    w.run("design", &[]);
    w.outside_writes_not_mattering("design's specification");
    assert_eq!(w.decide(), resting());
    let s3 = w.outside_writes("design's specification");
    assert_eq!(
        w.decide(),
        runs(
            "design",
            vec![changed("design's specification", Some(s1), false)],
            &[("design's specification", Some(s3))]
        )
    );
}
