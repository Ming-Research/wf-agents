//! Where a work stands (`Reading::rest`), at the corners the unit tests of `rest.rs` leave: the kinds of named test other
//! than "all", a precondition that names a block's value, a block that has a default, the first of several stalled
//! cells, and the limit of runs that only ask. What each expects is taken from the design tree: decision
//! `a-work-no-cell-can-move-is-stalled` (a work that waits only for what cells write is stalled; one a person or a
//! source outside the work can set going waits), decision `conditions-and-preconditions` (a precondition holds the work
//! at its cell), decision `every-way-round-has-a-limit` (runs in a row that only ask for calls are limited) and
//! decision `conditions-are-values-of-blocks` in `design/decisions/flow.md` (a block counts as its default before
//! anyone writes it).

// The tests use only part of the helper the tests of the deciding code share.
#[allow(dead_code)]
#[path = "../../decide/tests/support/mod.rs"]
mod support;

use std::collections::BTreeMap;

use decide::{Cell, CellId, Condition, Test, TestKind, TestName, Who};
use reading::{Reading, Rest};
use support::Says::{Matters, Value};
use support::{World, cell, gate};

fn rest(w: &World) -> Rest {
    Reading::new(&w.flow, &w.records).rest(&w.flow)
}

fn id(cell: &str) -> CellId {
    cell.into()
}

fn named(kind: TestKind, reads: &[&str], is: &str) -> Test {
    Test {
        kind,
        reads: reads.iter().map(|b| (*b).into()).collect(),
        is: Some(is.to_string()),
    }
}

fn requiring(test: &str, mut c: Cell) -> Cell {
    c.requires.push(test.into());
    c
}

fn requiring_value(block: &str, value: &str, mut c: Cell) -> Cell {
    c.requires.push(Condition::Is {
        block: block.into(),
        value: value.into(),
    });
    c
}

fn when_test(test: &str, mut c: Cell) -> Cell {
    c.applies = vec![test.into()];
    c
}

fn stalled_at(cell: &str, on: Vec<Condition>) -> Rest {
    Rest::Stalled { at: id(cell), on }
}

/// A work whose `first` cell writes the blocks `a` (by cells alone) and `b0`, a `gated` cell after it that runs on `b0`
/// and requires the named test, and the merge; the test is the one given. People write `b`, as every block a test reads
/// is theirs unless said.
fn gated_by(test: Test, reads_by_cells: &[&str]) -> World {
    let cells = vec![
        cell("first", &["brief"], &["a", "b0"]),
        requiring("t", cell("gated", &["b0"], &[])),
        gate(),
    ];
    let mut w = World::with_tests(cells, BTreeMap::from([(TestName::from("t"), test)]));
    for block in reads_by_cells {
        w.written_by_cells(block);
    }
    w.person_writes("brief");
    w
}

#[test]
fn an_all_test_whose_missing_block_only_cells_write_is_stalled_though_a_person_writes_another_it_reads() {
    // `a` is written by cells, `b` by people, and `b` is already yes: what keeps the test from holding is `a` alone,
    // which no person can set, so the work is stalled and does not wait for anyone.
    let mut w = gated_by(named(TestKind::All, &["a", "b"], "yes"), &["a"]);
    w.person_sets("b", "yes");
    w.run("first", &[("b0", Matters)]);
    assert_eq!(rest(&w), stalled_at("gated", vec![Condition::Test("t".into())]));
}

#[test]
fn an_all_test_whose_missing_block_a_person_writes_waits() {
    let mut w = gated_by(named(TestKind::All, &["a", "b"], "yes"), &["a"]);
    w.run("first", &[("a", Value("yes")), ("b0", Matters)]);
    assert_eq!(rest(&w), Rest::Waiting { at: id("gated") });
}

#[test]
fn an_any_test_none_of_whose_blocks_holds_waits_where_a_person_writes_one_of_them() {
    let mut w = gated_by(named(TestKind::Any, &["a", "b"], "yes"), &["a"]);
    w.run("first", &[("a", Value("no")), ("b0", Matters)]);
    assert_eq!(rest(&w), Rest::Waiting { at: id("gated") });
}

#[test]
fn an_any_test_none_of_whose_blocks_a_person_writes_is_stalled() {
    let cells = vec![
        cell("first", &["brief"], &["a", "c", "b0"]),
        requiring("t", cell("gated", &["b0"], &[])),
        gate(),
    ];
    let test = named(TestKind::Any, &["a", "c"], "yes");
    let mut w = World::with_tests(cells, BTreeMap::from([(TestName::from("t"), test)]));
    w.written_by_cells("a");
    w.written_by_cells("c");
    w.person_writes("brief");
    w.run("first", &[("a", Value("no")), ("c", Value("no")), ("b0", Matters)]);
    assert_eq!(rest(&w), stalled_at("gated", vec![Condition::Test("t".into())]));
}

#[test]
fn a_none_test_kept_from_holding_by_a_block_only_cells_write_is_stalled_though_people_write_its_other_block() {
    // The test holds when no block it reads is yes. `a`, written by cells, is yes; `b`, written by people, is not. A
    // person writing `b` cannot make the test hold, so the work is stalled.
    let mut w = gated_by(named(TestKind::None, &["a", "b"], "yes"), &["a"]);
    w.person_sets("b", "no");
    w.run("first", &[("a", Value("yes")), ("b0", Matters)]);
    assert_eq!(rest(&w), stalled_at("gated", vec![Condition::Test("t".into())]));
}

#[test]
fn a_none_test_kept_from_holding_by_a_block_a_person_writes_waits() {
    let mut w = gated_by(named(TestKind::None, &["a", "b"], "yes"), &["a"]);
    w.person_sets("b", "yes");
    w.run("first", &[("a", Value("no")), ("b0", Matters)]);
    assert_eq!(rest(&w), Rest::Waiting { at: id("gated") });
}

#[test]
fn a_precondition_that_names_a_block_s_value_stalls_the_work_where_only_cells_write_the_block() {
    let cells = vec![
        cell("first", &["brief"], &["ok", "b0"]),
        requiring_value("ok", "yes", cell("gated", &["b0"], &[])),
        gate(),
    ];
    let mut w = World::new(cells);
    w.written_by_cells("ok");
    w.person_writes("brief");
    w.run("first", &[("ok", Value("no")), ("b0", Matters)]);
    // The cell has something new to run on (`b0`), and the precondition still holds the work at it.
    assert_eq!(
        rest(&w),
        stalled_at(
            "gated",
            vec![Condition::Is {
                block: "ok".into(),
                value: "yes".into()
            }]
        )
    );
}

#[test]
fn a_precondition_that_names_a_block_s_value_waits_where_a_source_outside_the_work_writes_the_block() {
    let cells = vec![
        cell("first", &["brief"], &["b0"]),
        requiring_value("ok", "yes", cell("gated", &["b0"], &[])),
        gate(),
    ];
    let mut w = World::new(cells);
    w.written_outside("ok", "dependencies");
    w.person_writes("brief");
    w.run("first", &[("b0", Matters)]);
    assert_eq!(rest(&w), Rest::Waiting { at: id("gated") });
}

#[test]
fn the_first_cell_whose_preconditions_do_not_hold_names_the_stall_though_a_later_one_waits_for_a_person() {
    // `gated` waits for what cells write; `later`, after it, for a block people write. The work cannot reach `later`
    // before `gated` runs, and nothing a person writes sets `gated` going: it is stalled at `gated`.
    let cells = vec![
        cell("first", &["brief"], &["a", "b0"]),
        requiring("only-cells", cell("gated", &["b0"], &[])),
        requiring("people", cell("later", &["b0"], &[])),
        gate(),
    ];
    let tests = BTreeMap::from([
        (TestName::from("only-cells"), named(TestKind::All, &["a"], "yes")),
        (TestName::from("people"), named(TestKind::All, &["p"], "yes")),
    ]);
    let mut w = World::with_tests(cells, tests);
    w.written_by_cells("a");
    w.person_writes("brief");
    w.run("first", &[("a", Value("no")), ("b0", Matters)]);
    assert_eq!(
        rest(&w),
        stalled_at("gated", vec![Condition::Test("only-cells".into())])
    );
}

#[test]
fn a_named_test_over_a_block_that_has_a_default_holds_while_nobody_has_written_the_block() {
    // A block counts as its default before anyone writes it (decision `conditions-are-values-of-blocks`): `later` runs
    // while `mode` is on, which it is by default, and so is moving and not passed by.
    let cells = vec![
        cell("first", &["brief"], &["b0"]),
        when_test("enabled", cell("later", &["b0"], &[])),
        gate(),
    ];
    let tests = BTreeMap::from([(TestName::from("enabled"), named(TestKind::All, &["mode"], "on"))]);
    let mut w = World::with_tests(cells, tests);
    w.flow = w
        .flow
        .clone()
        .with_defaults(BTreeMap::from([("mode".into(), "on".to_string())]));
    w.person_writes("brief");
    w.run("first", &[("b0", Matters)]);
    assert_eq!(rest(&w), Rest::Moving { at: id("later") });
}

#[test]
fn a_none_test_over_a_block_whose_default_is_the_value_waits_for_the_person_who_may_write_the_block() {
    // `later` requires that `mode` is not off; `mode` is off by default and nobody has written it, so the test does not
    // hold, and people write the block: the work waits at `later`.
    let cells = vec![
        cell("first", &["brief"], &["b0"]),
        requiring("not-off", cell("later", &["b0"], &[])),
        gate(),
    ];
    let tests = BTreeMap::from([(TestName::from("not-off"), named(TestKind::None, &["mode"], "off"))]);
    let mut w = World::with_tests(cells, tests);
    w.flow = w
        .flow
        .clone()
        .with_defaults(BTreeMap::from([("mode".into(), "off".to_string())]));
    w.person_writes("brief");
    w.run("first", &[("b0", Matters)]);
    assert_eq!(rest(&w), Rest::Waiting { at: id("later") });
}

#[test]
fn a_cell_whose_runs_in_a_row_only_asked_waits_for_the_owner_at_its_limit_of_calls_and_moves_once_let_go() {
    let mut w = World::with_gate(vec![cell("first", &["brief"], &["b0"])]);
    w.person_writes("brief");
    // The cell's limit of calls is 3: after each ask, once the call has its result, the cell is entered for the result.
    for ran in 1..=3 {
        w.start("first");
        let call = w.asks("first", Some(Who::Owner));
        w.done(call);
        let expected = if ran < 3 {
            Rest::Moving { at: id("first") }
        } else {
            Rest::Waiting { at: id("first") }
        };
        assert_eq!(rest(&w), expected, "after {ran} runs in a row that only asked");
    }
    w.let_go("first");
    assert_eq!(rest(&w), Rest::Moving { at: id("first") });
}

/// A work whose first cell waits for the items it depends on, which a source outside the work writes; the cells it
/// begins with, and the tests by which the first of them holds the work.
fn waiting_for_items(cells: Vec<Cell>, more: Vec<(TestName, Test)>) -> World {
    let mut tests = BTreeMap::from([("may start".into(), named(TestKind::All, &["dependencies"], "yes"))]);
    tests.extend(more);
    let mut w = World::with_tests(cells, tests);
    w.written_outside("dependencies", "dependencies");
    w.outside_sets("dependencies", "yes");
    w
}

// Decisions a-work-no-cell-can-move-is-stalled (a work that a source outside the work can set going waits) and
// dependencies-hold-a-started-work-where-it-would-run.
#[test]
fn a_work_held_for_the_items_that_also_lacks_what_only_cells_write_waits_where_it_would_stall() {
    let cells = vec![
        requiring("may start", cell("start", &["go"], &[])),
        cell("work", &["plan"], &["code", "checked"]),
        requiring("checked", cell("merge", &["checked"], &[])),
    ];
    let mut w = waiting_for_items(
        cells,
        vec![("checked".into(), named(TestKind::All, &["checked"], "yes"))],
    );
    w.written_by_cells("checked");
    w.person_writes("go");
    w.person_writes("plan");
    w.run("start", &[]);
    w.run("work", &[("code", Matters)]);
    assert_eq!(
        rest(&w),
        Rest::Stalled {
            at: id("merge"),
            on: vec![Condition::Test("checked".into())]
        }
    );
    w.outside_sets("dependencies", "no");
    assert_eq!(rest(&w), Rest::Waiting { at: id("merge") });
    w.outside_sets("dependencies", "yes");
    assert!(matches!(rest(&w), Rest::Stalled { .. }), "{:?}", rest(&w));
}

// Decision the-walk-goes-from-the-first-cell: what comes before a held cell is not held by it.
#[test]
fn what_a_later_cell_holds_back_does_not_hold_a_cell_before_it_that_has_something_new() {
    let cells = vec![
        cell("first", &["plan"], &["code"]),
        requiring("may start", cell("gated", &["go"], &[])),
        gate(),
    ];
    let mut w = waiting_for_items(cells, Vec::new());
    w.person_writes("go");
    w.person_writes("plan");
    w.run("first", &[("code", Matters)]);
    w.run("gated", &[]);
    w.outside_sets("dependencies", "no");
    w.person_writes("plan");
    assert_eq!(rest(&w), Rest::Moving { at: id("first") });
}
