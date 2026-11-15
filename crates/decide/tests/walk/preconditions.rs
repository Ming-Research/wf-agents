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
        is: "yes".to_string(),
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
