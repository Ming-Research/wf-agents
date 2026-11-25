//! Preconditions, and everything a work waits for at one cell.

use decide::{Test, TestKind, TestName};

use super::*;

#[test]
fn a_precondition_holds_the_work_before_its_cell_even_when_nothing_is_new_to_the_cell() {
    let mut w = World::with_gate(vec![
        requiring("a may go", cell("a", &["x"], &[])),
        cell("b", &["y"], &[]),
    ]);
    w.person_writes("x");
    w.person_writes("y");
    w.run("a", &[]);
    w.run("b", &[]);
    w.says("a may go", false);
    w.person_writes("y");
    assert_eq!(
        w.decide(),
        wait("a", vec![precondition("a may go", vec![Maker::Owner])])
    );
}

#[test]
fn a_precondition_of_a_skipped_cell_holds_nothing() {
    let extra = requiring("extra may go", when("more", "yes", cell("extra", &["brief"], &[])));
    let mut w = World::with_gate(vec![cell("ask", &["brief"], &["more"]), extra]);
    w.person_writes("brief");
    w.says("extra may go", false);
    w.run("ask", &[("more", Value("no"))]);
    assert_eq!(w.decide(), resting());
}

#[test]
fn a_call_and_an_unmet_precondition_at_one_cell_are_waited_for_at_once() {
    let mut w = World::with_gate(vec![requiring("a may go", cell("a", &["brief"], &[]))]);
    w.person_writes("brief");
    w.start("a");
    let call = w.calls("a", Some(Who::Owner));
    w.says("a may go", false);
    let call = Waited::Call {
        call,
        on: Some(Who::Owner),
    };
    assert_eq!(
        w.decide(),
        wait("a", vec![call, precondition("a may go", vec![Maker::Owner])])
    );
}

#[test]
fn an_unmet_precondition_and_the_limit_at_one_cell_are_waited_for_at_once() {
    let (mut w, ..) = judged();
    w.install(vec![
        requiring("implement may go", cell("implement", &["plan", "opinions"], &["code"])),
        cell("review", &["code"], &["opinions"]),
        gate(),
    ]);
    for _ in 0..3 {
        send_back(&mut w);
    }
    w.run("review", &[("opinions", Matters)]);
    w.says("implement may go", false);
    assert_eq!(
        w.decide(),
        wait(
            "implement",
            vec![
                precondition("implement may go", vec![Maker::Owner]),
                Waited::Limit { runs: 3 }
            ]
        )
    );
}

/// A cell that waits, by the named test of this name, until every one of these blocks is yes; and the test.
fn waiting_for_all(test: &str, blocks: &[&str], c: Cell) -> (Cell, (TestName, Test)) {
    let named = Test {
        kind: TestKind::All,
        reads: blocks.iter().map(|b| (*b).into()).collect(),
        is: Some("yes".to_string()),
    };
    (requiring(test, c), (test.into(), named))
}

/// The source outside the work that writes whether the items a work depends on merged.
fn dependencies() -> Maker {
    Maker::Outside {
        source: "dependencies".to_string(),
    }
}

#[test]
fn a_precondition_on_what_a_source_outside_the_work_writes_waits_for_that_source_and_not_the_owner() {
    let (start, named) = waiting_for_all("may start", &["dependencies"], cell("start", &["dependencies"], &[]));
    let mut w = World::with_tests(vec![start, gate()], BTreeMap::from([named]));
    w.written_outside("dependencies", "dependencies");
    assert_eq!(
        w.decide(),
        wait("start", vec![precondition("may start", vec![dependencies()])])
    );
    w.outside_sets("dependencies", "yes");
    assert!(matches!(w.decide(), Decision::Run { cell, .. } if cell.as_str() == "start"));
}

#[test]
fn a_precondition_on_what_a_source_writes_as_the_owner_acts_waits_for_the_owner_as_the_flow_declares_it() {
    let (start, named) = waiting_for_all("answered", &["asked"], cell("start", &["code"], &[]));
    let mut w = World::with_tests(vec![start, gate()], BTreeMap::from([named]));
    w.written_outside_by_owner("asked", "asked");
    w.person_writes("code");
    // Who can make it hold is what the flow declares of the block, and not the name of its source.
    assert_eq!(
        w.decide(),
        wait("start", vec![precondition("answered", vec![Maker::Owner])])
    );
    w.written_outside("asked", "asked");
    let source = Maker::Outside {
        source: "asked".to_string(),
    };
    assert_eq!(w.decide(), wait("start", vec![precondition("answered", vec![source])]));
}

#[test]
fn a_precondition_on_what_only_a_cell_writes_stalls_the_work_once_that_cell_has_nothing_new() {
    let (merge, named) = waiting_for_all("checked", &["checked"], cell("merge", &["code"], &[]));
    let cells = vec![cell("check", &["code"], &["checked"]), merge];
    let mut w = World::with_tests(cells, BTreeMap::from([named]));
    w.written_by_cells("checked");
    w.person_writes("code");
    w.run("check", &[("checked", Value("no"))]);
    assert_eq!(
        w.decide(),
        stalled("merge", &["checked"], &[("check", Unrun::NothingNew)])
    );
    // New code runs the check again.
    w.person_writes("code");
    assert!(matches!(w.decide(), Decision::Run { cell, .. } if cell.as_str() == "check"));
}

#[test]
fn a_precondition_on_what_a_cell_and_people_write_waits_for_either() {
    let (merge, named) = waiting_for_all("settled", &["settled"], cell("merge", &["code"], &[]));
    let cells = vec![cell("review", &["code"], &["settled"]), merge];
    let mut w = World::with_tests(cells, BTreeMap::from([named]));
    w.person_writes("code");
    w.run("review", &[("settled", Value("no"))]);
    let either = vec![Maker::Owner, by_cell("review")];
    assert_eq!(w.decide(), wait("merge", vec![precondition("settled", either)]));
}

#[test]
fn a_precondition_waits_only_for_the_writers_of_the_blocks_that_keep_it_from_holding() {
    let (merge, named) = waiting_for_all("both", &["dependencies", "checked"], cell("merge", &["code"], &[]));
    let cells = vec![cell("check", &["code"], &["checked"]), merge];
    let mut w = World::with_tests(cells, BTreeMap::from([named]));
    w.written_outside("dependencies", "dependencies");
    w.written_by_cells("checked");
    w.person_writes("code");
    w.run("check", &[("checked", Value("no"))]);
    let both = vec![dependencies(), by_cell("check")];
    assert_eq!(w.decide(), wait("merge", vec![precondition("both", both)]));
    // The items it depends on merge: only the check keeps the test from holding, and it has nothing new.
    w.outside_sets("dependencies", "yes");
    assert_eq!(w.decide(), stalled("merge", &["both"], &[("check", Unrun::NothingNew)]));
}

/// A work whose first cell waits for the items it depends on, which the outside writes, then a cell that works, a cell
/// that goes over what it made, and the owner's merge. The items have merged and the first two cells have run, so
/// that the review is to run.
fn past_its_start() -> World {
    let (start, named) = waiting_for_all("may start", &["dependencies"], cell("start", &["go"], &[]));
    let cells = vec![
        start,
        cell("work", &["plan"], &["code"]),
        cell("review", &["code", "review answered"], &["notes"]),
        gate(),
    ];
    let mut w = World::with_tests(cells, BTreeMap::from([named]));
    w.written_outside("dependencies", "dependencies");
    w.outside_sets("dependencies", "yes");
    w.person_writes("go");
    w.person_writes("plan");
    w.run("start", &[]);
    w.run("work", &[("code", Matters)]);
    w
}

fn runs_review(w: &World) {
    assert!(
        matches!(w.decide(), Decision::Run { cell, .. } if cell.as_str() == "review"),
        "{:?}",
        w.decide()
    );
}

fn items() -> Waited {
    precondition("may start", vec![dependencies()])
}

#[test]
fn a_precondition_only_the_outside_makes_hold_of_a_cell_already_run_holds_back_the_cell_the_walk_comes_to() {
    let mut w = past_its_start();
    runs_review(&w);
    // An item it depends on is not merged: the work waits at the cell it would run, and goes on there once it is.
    w.outside_sets("dependencies", "no");
    assert_eq!(w.decide(), wait("review", vec![items()]));
    w.outside_sets("dependencies", "yes");
    runs_review(&w);
}

#[test]
fn a_round_under_way_goes_on_when_the_outside_holds_the_work_back_and_what_it_hands_in_is_taken() {
    let mut w = past_its_start();
    let run = w.start("review");
    w.outside_sets("dependencies", "no");
    assert_eq!(
        w.decide(),
        Decision::Busy {
            cell: "review".into(),
            run
        }
    );
    w.finish("review", &[("notes", Matters)]);
    // Nothing is run, and at the merge the work waits for the owner and for the items.
    assert_eq!(w.decide(), wait("merge", vec![merge_word(), items()]));
    w.outside_sets("dependencies", "yes");
    assert_eq!(w.decide(), resting());
}

#[test]
fn what_the_outside_holds_back_is_said_beside_a_call_owed_to_the_cell() {
    let mut w = past_its_start();
    w.start("review");
    let call = w.calls("review", Some(Who::Owner));
    w.outside_sets("dependencies", "no");
    let call = Waited::Call {
        call,
        on: Some(Who::Owner),
    };
    assert_eq!(w.decide(), wait("review", vec![call, items()]));
}

#[test]
fn what_the_outside_holds_back_is_said_beside_a_question_the_cell_waits_for() {
    let mut w = past_its_start();
    w.answering("review answered");
    w.run("review", &[("notes", Matters)]);
    let version = w.questions("review", "which", true);
    w.outside_sets("dependencies", "no");
    let answer = Waited::Answer {
        question: "which".to_string(),
    };
    assert_eq!(w.decide(), wait("review", vec![answer.clone(), items()]));
    w.answers("which", version, 1);
    assert!(
        matches!(w.decide(), Decision::Wait { on, .. } if !on.contains(&answer) && on.contains(&items())),
        "{:?}",
        w.decide()
    );
}

#[test]
fn what_the_outside_holds_back_is_said_beside_the_limit_of_the_cell() {
    let (start, named) = waiting_for_all("may start", &["dependencies"], cell("start", &["go"], &[]));
    let cells = vec![
        start,
        cell("implement", &["plan", "opinions"], &["code"]),
        cell("review", &["code"], &["opinions"]),
        gate(),
    ];
    let mut w = World::with_tests(cells, BTreeMap::from([named]));
    w.written_outside("dependencies", "dependencies");
    w.outside_sets("dependencies", "yes");
    w.person_writes("go");
    w.person_writes("plan");
    w.run("start", &[]);
    w.run("implement", &[("code", Matters)]);
    for _ in 0..3 {
        w.run("review", &[("opinions", Matters)]);
        w.run("implement", &[("code", Matters)]);
    }
    w.run("review", &[("opinions", Matters)]);
    assert_eq!(w.decide(), wait("implement", vec![Waited::Limit { runs: 3 }]));
    w.outside_sets("dependencies", "no");
    assert_eq!(w.decide(), wait("implement", vec![Waited::Limit { runs: 3 }, items()]));
}

#[test]
fn a_work_that_has_not_started_waits_at_its_first_cell_though_a_later_cell_has_something_new() {
    let (start, named) = waiting_for_all("may start", &["dependencies"], cell("start", &["go"], &[]));
    let cells = vec![start, cell("work", &["plan"], &["code"]), gate()];
    let mut w = World::with_tests(cells, BTreeMap::from([named]));
    w.written_outside("dependencies", "dependencies");
    w.outside_sets("dependencies", "no");
    w.person_writes("plan");
    assert_eq!(w.decide(), wait("start", vec![items()]));
    // Let on, its first cell has something new, and still waits at it.
    w.person_writes("go");
    assert_eq!(w.decide(), wait("start", vec![items()]));
}

#[test]
fn a_first_cell_that_has_not_finished_a_run_holds_the_work_at_itself() {
    let (start, named) = waiting_for_all("may start", &["dependencies"], cell("start", &["go"], &[]));
    let cells = vec![start, cell("work", &["plan"], &["code"]), gate()];
    let mut w = World::with_tests(cells, BTreeMap::from([named]));
    w.written_outside("dependencies", "dependencies");
    w.outside_sets("dependencies", "yes");
    w.person_writes("go");
    w.person_writes("plan");
    w.start("start");
    w.returns("start");
    w.outside_sets("dependencies", "no");
    assert_eq!(w.decide(), wait("start", vec![items()]));
}
