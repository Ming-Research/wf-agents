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

/// A choice the run writes, with this value.
fn chose(block: &str, value: &str) -> Write {
    Write {
        block: block.into(),
        meaning_unchanged: false,
        value: Some(value.to_string()),
        empty: false,
    }
}

/// Prose the run writes.
fn wrote(block: &str) -> Write {
    Write {
        block: block.into(),
        meaning_unchanged: false,
        value: None,
        empty: false,
    }
}

/// A question the run asks, waiting for its answer or not, or withdraws.
fn asking(name: &str, waits: bool, withdrawn: bool) -> decide::run::Asking {
    decide::run::Asking {
        name: name.to_string(),
        withdrawn,
        waits,
    }
}

/// What of these writes of the run, with these questions, leaves its cell no longer applying while it still waits.
fn skips(w: &World, run: RunId, writes: &[Write], questions: &[decide::run::Asking]) -> Vec<String> {
    (decide::skipping(&w.flow, &w.records, run, writes, questions).into_iter())
        .map(|b| b.0)
        .collect()
}

/// A research that runs while the deep research is not needed, which it alone decides (`deep`), and asks for tests in
/// `tests`; the owner's words turn to it (`spoken`); the deep research, run once it is needed; and the cell that runs
/// the tests, which gives the research their result.
fn deepening() -> World {
    let mut research = cell(
        "research",
        &["brief", "spoken", "research_tested"],
        &["deep", "plan", "tests"],
    );
    research.asks.push(decide::Asks {
        record: "tests".into(),
        result: "research_tested".into(),
    });
    let mut run_tests = cell("run tests", &["tests"], &["research_tested"]);
    run_tests.slot = false;
    let mut w = World::with_gate(vec![
        when("deep", "no", research),
        when("deep", "yes", cell("deep research", &["brief"], &["plan"])),
        run_tests,
    ]);
    w.turning("spoken");
    w.flow = (w.flow.clone()).with_defaults(BTreeMap::from([("deep".into(), "no".to_string())]));
    w
}

// What a run writes so that its own cell no longer applies while the cell, the run finished with those writes and its
// questions, still waits, for an answer it asks for or for the tests it asks for: the research that writes that the
// deep research is needed as it waits leaves itself, which the run's check sends back (decision
// a-return-that-waits-keeps-its-cell). Leaving as it waits for nothing, or waiting as it stays, names nothing. Well
// under a millisecond alone; no other test holds what a run's own writes skip while it waits.
#[test]
fn a_run_s_writes_that_leave_its_own_cell_while_it_waits_are_named() {
    let mut w = deepening();
    w.person_writes("brief");
    let run = w.start("research");
    let waits = asking("naming", true, false);
    assert_eq!(
        skips(
            &w,
            run,
            &[chose("deep", "yes"), wrote("plan")],
            std::slice::from_ref(&waits)
        ),
        ["deep"]
    );
    assert_eq!(skips(&w, run, &[chose("deep", "yes"), wrote("tests")], &[]), ["deep"]);
    assert_eq!(
        skips(&w, run, &[chose("deep", "no"), wrote("plan")], &[waits]),
        Vec::<String>::new()
    );
    assert_eq!(
        skips(&w, run, &[chose("deep", "yes"), wrote("plan")], &[]),
        Vec::<String>::new()
    );
    let going_on = asking("naming", false, false);
    assert_eq!(
        skips(&w, run, &[chose("deep", "yes")], &[going_on]),
        Vec::<String>::new()
    );
    assert_eq!(
        skips(&w, RunId(99), &[chose("deep", "yes")], &[asking("naming", true, false)]),
        Vec::<String>::new()
    );
}

// A wait left before holds the cell as much: a run entered for the owner's words while the research waits on its
// question, or on its tests, still waits as it writes that the deep research is needed, unless it withdraws the
// question or asks it so that it does not wait; an entry asking for tests is withdrawn by nothing, and waits for its
// result (decision a-return-that-waits-keeps-its-cell). Well under a millisecond alone.
#[test]
fn a_wait_left_before_that_the_run_neither_ended_nor_withdrew_holds_its_cell_as_much() {
    let mut w = deepening();
    w.person_writes("brief");
    w.start("research");
    w.finish("research", &[("plan", Matters)]);
    w.questions("research", "naming", true);
    w.outside_writes("spoken");
    let run = w.start("research");
    assert_eq!(skips(&w, run, &[chose("deep", "yes")], &[]), ["deep"]);
    let withdrawn = asking("naming", false, true);
    assert_eq!(
        skips(&w, run, &[chose("deep", "yes")], &[withdrawn]),
        Vec::<String>::new()
    );
    let going_on = asking("naming", false, false);
    assert_eq!(
        skips(&w, run, &[chose("deep", "yes")], &[going_on]),
        Vec::<String>::new()
    );

    let mut w = deepening();
    w.person_writes("brief");
    w.start("research");
    w.finish("research", &[("tests", Matters)]);
    w.outside_writes("spoken");
    let run = w.start("research");
    assert_eq!(skips(&w, run, &[chose("deep", "yes")], &[]), ["deep"]);
}

// A cell whose condition is a named test of its own choice is left by a write that keeps the test from holding; one
// that does not apply as the records stand, as where a person's word skipped it while its round ran, is left by none
// of its writes, which do not skip it. Well under a millisecond alone.
#[test]
fn a_named_test_condition_is_left_by_the_run_s_write_and_a_cell_already_skipped_by_none() {
    let mut research = cell("research", &["brief"], &["deep", "plan"]);
    research.applies = vec![
        Condition::Is {
            block: "go".into(),
            value: "yes".into(),
        },
        Condition::Test("researching".into()),
    ];
    let researching = decide::Test {
        kind: decide::TestKind::All,
        reads: vec!["deep".into()],
        is: Some("no".to_string()),
    };
    let tests = BTreeMap::from([
        ("researching".into(), researching),
        (support::GATE.into(), support::gate_word()),
    ]);
    let people = decide::Outsiders {
        people: true,
        ..Default::default()
    };
    let outsiders = BTreeMap::from([(support::GATE.into(), people.clone()), ("go".into(), people)]);
    let flow = Flow::new(vec![research, gate()], tests, outsiders).expect("the cells make a flow");
    let mut w = World::of(flow.with_defaults(BTreeMap::from([("deep".into(), "no".to_string())])));
    w.person_writes("brief");
    w.person_sets("go", "yes");
    let run = w.start("research");
    let waits = asking("naming", true, false);
    assert_eq!(
        skips(&w, run, &[chose("deep", "yes")], std::slice::from_ref(&waits)),
        ["deep"]
    );
    w.person_sets("go", "no");
    assert_eq!(skips(&w, run, &[chose("deep", "yes")], &[waits]), Vec::<String>::new());
}

// Where the run's writes leave its cell through another cell's applying, which decides whether a version a test of
// what stands reads still stands, the run's own block that does it is named, never the other cell's, which the run
// could not leave out: the fix, run while no refusal of the judge's stands, writes what has the judge apply again, and
// the judge's refusal stands. Well under a millisecond alone; no other test names a block through another cell.
#[test]
fn a_run_that_leaves_its_cell_through_another_cell_s_applying_names_its_own_write() {
    let judge = when("again", "yes", cell("judge", &["brief"], &["verdict"]));
    let mut fix = cell("fix", &["brief"], &["again", "plan"]);
    fix.applies = vec![Condition::Test("unrefused".into())];
    let unrefused = decide::Test {
        kind: decide::TestKind::NoneStanding,
        reads: vec!["verdict".into()],
        is: Some("no".to_string()),
    };
    let tests = BTreeMap::from([
        ("unrefused".into(), unrefused),
        (support::GATE.into(), support::gate_word()),
    ]);
    let people = decide::Outsiders {
        people: true,
        ..Default::default()
    };
    let outsiders = BTreeMap::from([(support::GATE.into(), people.clone()), ("again".into(), people)]);
    let flow = Flow::new(vec![judge, fix, gate()], tests, outsiders).expect("the cells make a flow");
    let mut w = World::of(flow.with_defaults(BTreeMap::from([("again".into(), "yes".to_string())])));
    w.person_writes("brief");
    w.run("judge", &[("verdict", Value("no"))]);
    w.person_sets("again", "no");
    let run = w.start("fix");
    let waits = asking("naming", true, false);
    assert_eq!(
        skips(&w, run, &[chose("again", "yes"), wrote("plan")], &[waits]),
        ["again"]
    );
    assert_eq!(
        skips(&w, run, &[chose("again", "yes"), wrote("plan")], &[]),
        Vec::<String>::new()
    );
}
