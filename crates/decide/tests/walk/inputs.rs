//! When a cell runs: an input changed in meaning, and only then.

use super::*;

#[test]
fn a_cell_whose_record_knows_the_current_meaning_of_every_input_is_passed_and_a_block_no_cell_reads_changes_nothing() {
    let (mut w, ..) = planned();
    w.person_writes("elsewhere");
    assert_eq!(w.decide(), resting());
}

#[test]
fn a_changed_input_runs_only_the_cells_that_read_it() {
    let cells = vec![
        cell("a", &["x"], &["p"]),
        cell("b", &["y"], &[]),
        cell("c", &["p"], &[]),
    ];
    let mut w = World::with_gate(cells);
    let x1 = w.person_writes("x");
    w.person_writes("y");
    let p1 = w.run("a", &[("p", Matters)]);
    w.run("b", &[]);
    w.run("c", &[]);
    let x2 = w.person_writes("x");
    assert_eq!(
        w.decide(),
        runs(
            "a",
            vec![changed("x", Some(x1), false)],
            &[("x", Some(x2)), ("p", Some(p1))]
        )
    );
    let p2 = w.run("a", &[("p", Matters)]);
    assert_eq!(
        w.decide(),
        runs("c", vec![changed("p", Some(p1), false)], &[("p", Some(p2))])
    );
    w.run("c", &[]);
    assert_eq!(w.decide(), resting());
}

#[test]
fn a_write_its_writer_says_does_not_matter_runs_nobody() {
    let (mut w, ..) = planned();
    w.person_writes("background");
    w.run("research", &[("plan", DoesNotMatter)]);
    assert_eq!(w.decide(), resting());
}

#[test]
fn a_cell_does_not_run_for_its_own_write_of_a_block_it_reads() {
    let mut w = World::with_gate(vec![
        cell("design", &["plan"], &["plan"]),
        cell("implement", &["plan"], &[]),
    ]);
    let plan = w.run("design", &[("plan", Matters)]);
    assert_eq!(w.decide(), runs("implement", vec![NO_RECORD], &[("plan", Some(plan))]));
    w.run("implement", &[]);
    assert_eq!(w.decide(), resting());
}

#[test]
fn a_later_cells_write_of_a_block_an_earlier_cell_reads_runs_the_earlier_cell_as_behind() {
    let mut w = World::with_gate(vec![
        cell("design", &["plan"], &["plan"]),
        cell("revise", &["plan"], &["plan"]),
    ]);
    let p1 = w.run("design", &[("plan", Matters)]);
    assert_eq!(w.decide(), runs("revise", vec![NO_RECORD], &[("plan", Some(p1))]));
    let p2 = w.run("revise", &[("plan", Matters)]);
    // The design's record knows the plan it wrote itself.
    assert_eq!(
        w.decide(),
        runs("design", vec![changed("plan", Some(p1), true)], &[("plan", Some(p2))])
    );
}

#[test]
fn what_a_record_knows_is_the_version_its_run_started_from_or_the_one_it_wrote() {
    let cells = vec![
        cell("research", &[], &["plan"]),
        cell("design", &["plan"], &["plan"]),
        cell("implement", &["plan"], &[]),
    ];
    let mut w = World::with_gate(cells);
    let p1 = w.run("research", &[("plan", Matters)]);
    assert_eq!(w.decide(), runs("design", vec![NO_RECORD], &[("plan", Some(p1))]));
    let p2 = w.run("design", &[("plan", Matters)]);
    w.run("implement", &[]);
    let p3 = w.person_writes("plan");
    // The design started from the research's plan but wrote its own: its record knows its own.
    assert_eq!(
        w.decide(),
        runs("design", vec![changed("plan", Some(p2), false)], &[("plan", Some(p3))])
    );
    let p4 = w.run("design", &[("plan", Matters)]);
    // The implementation started from the design's plan.
    assert_eq!(
        w.decide(),
        runs(
            "implement",
            vec![changed("plan", Some(p2), false)],
            &[("plan", Some(p4))]
        )
    );
}

#[test]
fn a_persons_prose_always_changes_the_meaning_even_where_its_entry_says_it_does_not() {
    let (mut w, _, plan, code) = planned();
    let p2 = w.person_writes_not_mattering("plan");
    assert_eq!(
        w.decide(),
        runs(
            "implement",
            vec![changed("plan", Some(plan), false)],
            &[("plan", Some(p2)), ("code", Some(code))]
        )
    );
}

#[test]
fn a_write_from_outside_the_work_always_changes_the_meaning() {
    let mut w = World::with_gate(vec![cell("check", &["main"], &[])]);
    let m1 = w.outside_writes("main");
    w.run("check", &[]);
    assert_eq!(w.decide(), resting());
    let m2 = w.outside_writes("main");
    assert_eq!(
        w.decide(),
        runs("check", vec![changed("main", Some(m1), false)], &[("main", Some(m2))])
    );
}
