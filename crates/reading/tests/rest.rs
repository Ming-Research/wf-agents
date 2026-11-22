//! Where a work stands, read from its records alone (`Reading::rest`): each test builds a small work, as the walk's own
//! tests do, and says where the design tree puts it: moving, waiting for someone, stalled, or ended. What each expects
//! is taken from the design tree, never from the deciding code, which is asked only to agree.

// The tests use only part of the helper the tests of the deciding code share.
#[allow(dead_code)]
#[path = "../../decide/tests/support/mod.rs"]
mod support;

use std::collections::BTreeMap;

use decide::{Cell, CellId, Condition, Decision, Test, TestKind, TestName, Who};
use reading::{Reading, Rest};
use support::Says::{Matters, Value};
use support::{World, cell};

/// The cell requires the named test.
fn requiring(test: &str, mut c: Cell) -> Cell {
    c.requires.push(test.into());
    c
}

/// A cell that runs only while this block is this value.
fn when(block: &str, value: &str, mut c: Cell) -> Cell {
    c.applies = vec![Condition::Is {
        block: block.into(),
        value: value.into(),
    }];
    c
}

/// A cell that waits, by the named test of this name, until this block is yes; and the test.
fn waiting_for(test: &str, block: &str, c: Cell) -> (Cell, (TestName, Test)) {
    let named = Test {
        kind: TestKind::All,
        reads: vec![block.into()],
        is: Some("yes".to_string()),
    };
    (requiring(test, c), (test.into(), named))
}

fn rest(w: &World) -> Rest {
    Reading::new(&w.flow, &w.records).rest(&w.flow)
}

fn id(cell: &str) -> CellId {
    cell.into()
}

fn stalled_at(cell: &str, on: &[&str]) -> Rest {
    Rest::Stalled {
        at: id(cell),
        on: on.iter().map(|t| Condition::Test((*t).into())).collect(),
    }
}

/// The research, which says whether a deeper look is needed and writes the plan; the implementation of the plan where
/// it is not; the check of the code, which alone writes whether it passed; and the merge, which waits for that.
fn research_to_merge() -> World {
    let (merge, named) = waiting_for("checked", "checked", cell("merge", &["checked"], &[]));
    let cells = vec![
        cell("research", &["background"], &["deep", "plan"]),
        when("deep", "no", cell("implement", &["plan"], &["code"])),
        cell("check", &["code"], &["checked"]),
        merge,
    ];
    let mut w = World::with_tests(cells, BTreeMap::from([named]));
    w.written_by_cells("checked");
    w.person_writes("background");
    w
}

/// The same, the check run by main as well, which a source outside the work writes.
fn checked_on_main() -> World {
    let (merge, named) = waiting_for("checked", "checked", cell("merge", &["checked"], &[]));
    let cells = vec![
        cell("research", &["background"], &["deep", "plan"]),
        when("deep", "no", cell("implement", &["plan"], &["code"])),
        cell("check", &["code", "main"], &["checked"]),
        merge,
    ];
    let mut w = World::with_tests(cells, BTreeMap::from([named]));
    w.written_by_cells("checked");
    w.person_writes("background");
    w.written_outside("main", "main");
    w
}

#[test]
fn a_cell_with_something_new_to_run_on_is_moving_at_that_cell() {
    let w = research_to_merge();
    assert_eq!(rest(&w), Rest::Moving { at: id("research") });
}

#[test]
fn a_work_waiting_only_for_what_a_cell_writes_that_will_not_run_is_stalled() {
    let mut w = research_to_merge();
    w.run("research", &[("deep", Value("no"))]);
    // No plan: the implementation has nothing new, so neither has the check, and only the check makes the merge's
    // precondition hold.
    assert_eq!(rest(&w), stalled_at("merge", &["checked"]));
    // The same whether or not the work is paused.
    w.engine_pauses("it can go on by itself no further");
    assert_eq!(rest(&w), stalled_at("merge", &["checked"]));
}

#[test]
fn a_work_waiting_for_a_block_a_person_writes_waits_and_is_not_stalled() {
    // The merge waits for the owner's word, which people write.
    let mut w = World::with_gate(vec![cell("first", &["brief"], &["b0"])]);
    w.person_writes("brief");
    w.run("first", &[("b0", Matters)]);
    assert_eq!(rest(&w), Rest::Waiting { at: id("merge") });
}

#[test]
fn a_work_waiting_for_a_block_a_source_outside_the_work_writes_waits() {
    let (merge, named) = waiting_for("merged", "merged", cell("merge", &["merged"], &[]));
    let mut w = World::with_tests(vec![cell("first", &["brief"], &[]), merge], BTreeMap::from([named]));
    w.written_outside("merged", "dependencies");
    w.person_writes("brief");
    w.run("first", &[]);
    assert_eq!(rest(&w), Rest::Waiting { at: id("merge") });
}

#[test]
fn a_check_that_said_what_it_wrote_waits_for_main_has_the_work_wait_and_without_it_stalls_it() {
    let mut w = checked_on_main();
    w.outside_writes("main");
    w.run("research", &[("deep", Value("no")), ("plan", Matters)]);
    w.run("implement", &[("code", Matters)]);
    w.run_waiting_for("check", &[("checked", Value("no"))], "main");
    assert_eq!(rest(&w), Rest::Waiting { at: id("merge") });
    // Main moves, and the check is entered again.
    w.outside_writes("main");
    assert_eq!(rest(&w), Rest::Moving { at: id("check") });

    let mut w = checked_on_main();
    w.outside_writes("main");
    w.run("research", &[("deep", Value("no")), ("plan", Matters)]);
    w.run("implement", &[("code", Matters)]);
    w.run("check", &[("checked", Value("no"))]);
    assert_eq!(rest(&w), stalled_at("merge", &["checked"]));
}

#[test]
fn a_call_for_a_person_is_waited_for_and_one_the_engine_makes_is_not() {
    let (gated, named) = waiting_for("ok", "ok", cell("gated", &["ok"], &[]));
    let mut w = World::with_tests(vec![cell("first", &["brief"], &["ok"]), gated], BTreeMap::from([named]));
    w.written_by_cells("ok");
    w.person_writes("brief");
    w.run("first", &[("ok", Value("yes"))]);
    w.start("gated");
    let call = w.calls("gated", Some(Who::Owner));
    assert_eq!(rest(&w), Rest::Waiting { at: id("gated") });
    w.done(call);
    assert_eq!(rest(&w), Rest::Moving { at: id("gated") });

    let (gated, named) = waiting_for("ok", "ok", cell("gated", &["ok"], &[]));
    let mut w = World::with_tests(vec![cell("first", &["brief"], &["ok"]), gated], BTreeMap::from([named]));
    w.written_by_cells("ok");
    w.person_writes("brief");
    w.run("first", &[("ok", Value("yes"))]);
    w.start("gated");
    w.calls("gated", None);
    assert_eq!(rest(&w), Rest::Moving { at: id("gated") });
}

#[test]
fn a_call_for_a_person_keeps_a_work_from_being_stalled_until_it_has_its_result() {
    let (gated, named) = waiting_for("ok", "ok", cell("gated", &["ok"], &[]));
    let mut w = World::with_tests(vec![cell("first", &["brief"], &["ok"]), gated], BTreeMap::from([named]));
    w.written_by_cells("ok");
    w.person_writes("brief");
    w.run("first", &[("ok", Value("yes"))]);
    w.start("gated");
    let call = w.calls("gated", Some(Who::Owner));
    w.person_writes("brief");
    w.run("first", &[("ok", Value("no"))]);
    // The owner's answer may come, and what is waited for besides is only the first cell's, which has nothing new.
    assert_eq!(rest(&w), Rest::Waiting { at: id("gated") });
    w.done(call);
    assert_eq!(rest(&w), stalled_at("gated", &["ok"]));
}

#[test]
fn a_question_that_waits_holds_the_work_waiting_whether_or_not_its_cell_applies() {
    let mut w = World::with_gate(vec![cell("ask", &["brief"], &["b0"]), cell("later", &["b0"], &[])]);
    w.person_writes("brief");
    w.run("ask", &[("b0", Matters)]);
    let at = w.questions("ask", "which", true);
    assert_eq!(rest(&w), Rest::Waiting { at: id("ask") });
    w.answers("which", at, 1);
    assert_ne!(rest(&w), Rest::Waiting { at: id("ask") });
}

#[test]
fn a_work_whose_last_cell_has_run_has_ended_and_so_has_a_cancelled_one_with_nothing_open() {
    let mut w = World::with_gate(vec![cell("first", &["brief"], &["b0"])]);
    w.person_writes("brief");
    w.run("first", &[("b0", Matters)]);
    w.says("the merge may go", true);
    w.run("merge", &[]);
    assert_eq!(rest(&w), Rest::Ended);
    assert!(matches!(w.decide(), Decision::Ended(_)));

    let mut w = World::with_gate(vec![cell("first", &["brief"], &["b0"])]);
    w.person_writes("brief");
    w.start("first");
    w.cancel();
    // Its run is still open, which is abandoned first.
    assert_eq!(rest(&w), Rest::Moving { at: id("first") });
}

#[test]
fn a_round_under_way_is_moving_at_the_cell_of_its_run() {
    let mut w = World::with_gate(vec![cell("first", &["brief"], &["b0"])]);
    w.person_writes("brief");
    w.start("first");
    assert_eq!(rest(&w), Rest::Moving { at: id("first") });
}
