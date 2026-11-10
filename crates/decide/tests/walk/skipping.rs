//! Conditions: a skipped cell, and what it wrote before.

use super::*;

#[test]
fn a_skipped_cell_is_not_run_though_its_inputs_changed_and_what_it_wrote_is_still_what_later_cells_start_from() {
    let (mut w, c1, tests) = tested();
    w.person_writes("brief");
    let c2 = w.run("implement", &[("code", Matters)]);
    w.run("review", &[("more", Value("no"))]);
    assert_eq!(
        w.decide(),
        runs(
            "check",
            vec![changed("code", Some(c1), false)],
            &[("code", Some(c2)), ("tests", Some(tests))]
        )
    );
}

#[test]
fn a_condition_on_a_value_a_person_writes_puts_the_cell_back_and_it_runs_for_inputs_its_record_does_not_know() {
    let (mut w, c1, tests) = tested();
    w.person_writes("brief");
    let c2 = w.run("implement", &[("code", Matters)]);
    w.run("review", &[("more", Value("no"))]);
    w.run("check", &[]);
    assert_eq!(w.decide(), resting());
    w.person_sets("more", "yes");
    assert_eq!(
        w.decide(),
        runs(
            "testing",
            vec![changed("code", Some(c1), false)],
            &[("code", Some(c2)), ("tests", Some(tests))]
        )
    );
}

#[test]
fn a_cell_whose_condition_is_met_again_is_passed_when_its_inputs_are_what_its_record_knows() {
    let (mut w, ..) = tested();
    w.person_sets("more", "no");
    assert_eq!(w.decide(), resting());
    w.person_sets("more", "yes");
    assert_eq!(w.decide(), resting());
}

#[test]
fn a_condition_on_a_block_that_has_no_version_is_not_met() {
    let mut w = World::with_gate(vec![
        cell("ask", &[], &[]),
        when("more", "yes", cell("extra", &[], &[])),
    ]);
    w.run("ask", &[]);
    assert_eq!(w.decide(), resting());
}

#[test]
fn a_condition_that_is_a_named_test_lets_its_cell_run_while_the_test_holds() {
    let mut w = World::with_gate(vec![on_test("there is a remark", cell("remarks", &[], &[]))]);
    w.set("there is a remark", Outcome::Waits { who: Who::Owner });
    assert_eq!(w.decide(), resting());
    w.set("there is a remark", Outcome::Holds);
    assert_eq!(w.decide(), runs("remarks", vec![NO_RECORD], &[]));
}

#[test]
fn a_run_at_rest_whose_cell_becomes_skipped_is_abandoned_and_the_cell_does_not_run() {
    let (mut w, c1, tests) = tested();
    w.person_writes("brief");
    let c2 = w.run("implement", &[("code", Matters)]);
    w.run("review", &[("more", Value("yes"))]);
    let testing = w.start("testing");
    w.returns("testing");
    w.person_sets("more", "no");
    assert_eq!(w.decide(), abandon("testing", testing, Abandoning::NoLongerApplies));
    w.abandon();
    assert_eq!(
        w.decide(),
        runs(
            "check",
            vec![changed("code", Some(c1), false)],
            &[("code", Some(c2)), ("tests", Some(tests))]
        )
    );
}

#[test]
fn a_condition_on_a_block_a_later_cell_writes_is_met_once_that_cell_wrote_it() {
    let cells = vec![
        when("choice", "yes", cell("first", &[], &["x"])),
        cell("later", &[], &["choice"]),
    ];
    let mut w = World::with_gate(cells);
    assert_eq!(w.decide(), runs("later", vec![NO_RECORD], &[("choice", None)]));
    w.run("later", &[("choice", Value("yes"))]);
    assert_eq!(w.decide(), runs("first", vec![NO_RECORD], &[("x", None)]));
}
