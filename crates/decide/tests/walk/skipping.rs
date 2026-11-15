//! Conditions: a skipped cell, and what it wrote before.

use super::*;

#[test]
fn a_skipped_cell_is_not_run_though_something_is_new_to_it_and_what_it_wrote_still_stands() {
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
fn a_condition_on_a_value_a_person_writes_puts_the_cell_back_and_it_runs_for_what_is_new_to_it() {
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
fn a_cell_whose_condition_is_met_again_is_passed_when_nothing_is_new_to_it() {
    let (mut w, ..) = tested();
    w.person_sets("more", "no");
    assert_eq!(w.decide(), resting());
    w.person_sets("more", "yes");
    assert_eq!(w.decide(), resting());
}

#[test]
fn a_condition_on_a_block_that_has_no_version_is_not_met() {
    let mut w = World::with_gate(vec![
        cell("ask", &["brief"], &[]),
        when("more", "yes", cell("extra", &["brief"], &[])),
    ]);
    w.person_writes("brief");
    w.run("ask", &[]);
    assert_eq!(w.decide(), resting());
}

#[test]
fn a_block_never_written_counts_as_its_default_in_a_condition() {
    let mut w = World::with_gate(vec![
        cell("ask", &["brief"], &[]),
        when("more", "no", cell("plain", &["brief"], &[])),
        when("more", "yes", cell("extra", &["brief"], &[])),
    ]);
    w.flow = (w.flow.clone()).with_defaults(BTreeMap::from([("more".into(), "no".to_string())]));
    let brief = w.person_writes("brief");
    w.run("ask", &[]);
    // Counted as no: the cell that runs on no runs, and the one that runs on yes is skipped.
    assert_eq!(w.decide(), runs("plain", first(&["brief"]), &[("brief", Some(brief))]));
    w.run("plain", &[]);
    assert_eq!(w.decide(), resting());
    // Written, its value counts, and no longer its default.
    w.person_sets("more", "yes");
    assert_eq!(w.decide(), runs("extra", first(&["brief"]), &[("brief", Some(brief))]));
}

#[test]
fn a_cell_runs_only_while_every_key_of_its_condition_holds() {
    let mut c = cell("both", &["brief"], &[]);
    c.applies = vec![
        Condition::Is {
            block: "more".into(),
            value: "yes".into(),
        },
        Condition::Is {
            block: "deep".into(),
            value: "yes".into(),
        },
    ];
    let mut w = World::with_gate(vec![cell("ask", &["brief"], &[]), c]);
    let brief = w.person_writes("brief");
    w.run("ask", &[]);
    w.person_sets("more", "yes");
    // One key holds and the other does not: the cell is skipped.
    assert_eq!(w.decide(), resting());
    w.person_sets("deep", "yes");
    assert_eq!(w.decide(), runs("both", first(&["brief"]), &[("brief", Some(brief))]));
}

#[test]
fn a_condition_that_is_a_named_test_lets_its_cell_run_while_the_test_holds() {
    let mut w = World::with_gate(vec![on_test("there is a remark", cell("remarks", &["remark"], &[]))]);
    w.says("there is a remark", false);
    let remark = w.person_writes("remark");
    assert_eq!(w.decide(), resting());
    w.says("there is a remark", true);
    assert_eq!(
        w.decide(),
        runs("remarks", first(&["remark"]), &[("remark", Some(remark))])
    );
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
        when("choice", "yes", cell("first", &["brief"], &["x"])),
        cell("later", &["brief"], &["choice"]),
    ];
    let mut w = World::with_gate(cells);
    let brief = w.person_writes("brief");
    assert_eq!(
        w.decide(),
        runs("later", first(&["brief"]), &[("brief", Some(brief)), ("choice", None)])
    );
    w.run("later", &[("choice", Value("yes"))]);
    assert_eq!(
        w.decide(),
        runs("first", first(&["brief"]), &[("brief", Some(brief)), ("x", None)])
    );
}
