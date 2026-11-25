//! A work held back for the items it depends on, at the corners: what the hold meets besides a call, a question and a
//! limit. The reference is the design tree: decision dependencies-hold-a-started-work-where-it-would-run (what only a
//! source outside the work makes hold, of a cell a run of which has finished, holds back the run the walk comes to; the
//! work waits at that cell; no round begins), decision conditions-and-preconditions (a skipped cell holds nothing),
//! decision a-work-no-cell-can-move-is-stalled (a work that waits for a source outside the work is not stalled), and
//! decision the-walk-goes-from-the-first-cell (what comes after a cell is held by it, what comes before is not).

use decide::{Test, TestKind, TestName};

use super::*;

fn waiting_for_all(test: &str, blocks: &[&str], c: Cell) -> (Cell, (TestName, Test)) {
    let named = Test {
        kind: TestKind::All,
        reads: blocks.iter().map(|b| (*b).into()).collect(),
        is: Some("yes".to_string()),
    };
    (requiring(test, c), (test.into(), named))
}

fn items() -> Waited {
    precondition(
        "may start",
        vec![Maker::Outside {
            source: "dependencies".to_string(),
        }],
    )
}

/// A work whose first cell waits for the items, which merged: the start and the work ran, the review has something new.
fn begun(rest: Vec<Cell>) -> World {
    let (start, named) = waiting_for_all("may start", &["dependencies"], cell("start", &["go"], &[]));
    let mut cells = vec![start, cell("work", &["plan"], &["code"])];
    cells.extend(rest);
    let mut w = World::with_tests(cells, BTreeMap::from([named]));
    w.written_outside("dependencies", "dependencies");
    w.outside_sets("dependencies", "yes");
    w.person_writes("go");
    w.person_writes("plan");
    w.run("start", &[]);
    w.run("work", &[("code", Matters)]);
    w
}

fn runs_cell(w: &World, name: &str) {
    assert!(
        matches!(w.decide(), Decision::Run { cell, .. } if cell.as_str() == name),
        "{:?}",
        w.decide()
    );
}

#[test]
fn a_paused_work_held_for_items_waits_for_them_where_it_would_run_and_is_paused_once_they_merge() {
    let mut w = begun(vec![cell("review", &["code"], &["notes"]), gate()]);
    w.pause();
    // Paused, the work names the cell it would run; held, it waits for the items there, as a work not begun does.
    w.outside_sets("dependencies", "no");
    assert_eq!(w.decide(), wait("review", vec![items()]));
    w.outside_sets("dependencies", "yes");
    assert!(
        matches!(w.decide(), Decision::Paused { ref cell, .. } if cell.as_str() == "review"),
        "{:?}",
        w.decide()
    );
}

#[test]
fn a_run_at_rest_does_not_begin_its_next_round_while_the_work_is_held_and_goes_on_when_it_is_not() {
    let mut w = begun(vec![cell("review", &["code"], &["notes"]), gate()]);
    let run = w.start("review");
    w.returns("review");
    assert_eq!(w.decide(), go_on("review", run));
    w.outside_sets("dependencies", "no");
    assert_eq!(w.decide(), wait("review", vec![items()]));
    w.outside_sets("dependencies", "yes");
    assert_eq!(w.decide(), go_on("review", run));
}

#[test]
fn a_work_held_for_items_that_also_lacks_what_only_cells_write_waits_and_is_not_stalled() {
    let (merge, merge_test) = waiting_for_all("checked", &["checked"], cell("merge", &["checked"], &[]));
    let (start, start_test) = waiting_for_all("may start", &["dependencies"], cell("start", &["go"], &[]));
    let cells = vec![start, cell("work", &["plan"], &["code", "checked"]), merge];
    let mut w = World::with_tests(cells, BTreeMap::from([start_test, merge_test]));
    w.written_outside("dependencies", "dependencies");
    w.written_by_cells("checked");
    w.outside_sets("dependencies", "yes");
    w.person_writes("go");
    w.person_writes("plan");
    w.run("start", &[]);
    w.run("work", &[("code", Matters)]);
    // Nothing writes `checked`: the work is stalled at the merge, for what cells write alone.
    assert!(matches!(w.decide(), Decision::Stalled { .. }), "{:?}", w.decide());
    // The items it waits for may come: it is waiting, not stalled, and says it waits for them.
    w.outside_sets("dependencies", "no");
    assert!(
        matches!(w.decide(), Decision::Wait { ref at, ref on } if at.as_str() == "merge" && on.contains(&items())),
        "{:?}",
        w.decide()
    );
    w.outside_sets("dependencies", "yes");
    assert!(matches!(w.decide(), Decision::Stalled { .. }), "{:?}", w.decide());
}

/// The first cell waits for the items; the implementation's second run has its round under way, and the owner's words
/// to the research, which comes before it, run it.
fn implementing() -> (World, RunId) {
    let (start, named) = waiting_for_all("may start", &["dependencies"], cell("start", &["go"], &[]));
    let cells = vec![
        start,
        cell("research", &["brief", "spoken to research"], &["plan"]),
        cell("implement", &["plan"], &["code"]),
        gate(),
    ];
    let mut w = World::with_tests(cells, BTreeMap::from([named]));
    w.written_outside("dependencies", "dependencies");
    w.turning("spoken to research");
    w.outside_sets("dependencies", "yes");
    w.person_writes("go");
    w.person_writes("brief");
    w.run("start", &[]);
    w.run("research", &[("plan", Matters)]);
    let run = w.start("implement");
    (w, run)
}

#[test]
fn a_round_is_not_told_to_hand_in_for_the_owner_turning_to_a_cell_that_the_hold_keeps_from_running() {
    let (mut w, run) = implementing();
    w.outside_sets("dependencies", "no");
    w.outside_writes("spoken to research");
    // The research would not run for the items it waits for: the round goes on and ends as it would.
    assert_eq!(w.decide(), busy("implement", run));
    // Once the items have merged, the research the owner turned to runs first: the round is told to hand in what it has.
    w.outside_sets("dependencies", "yes");
    assert!(matches!(w.decide(), Decision::Hurry { .. }), "{:?}", w.decide());
}

#[test]
fn a_skipped_cell_holds_nothing_though_it_ran_and_what_only_the_outside_makes_hold_does_not_hold() {
    let (extra, named) = waiting_for_all("may start", &["dependencies"], cell("extra", &["plan"], &[]));
    let cells = vec![
        cell("ask", &["brief"], &["more"]),
        when("more", "yes", extra),
        cell("work", &["plan"], &["code"]),
        gate(),
    ];
    let mut w = World::with_tests(cells, BTreeMap::from([named]));
    w.written_outside("dependencies", "dependencies");
    w.outside_sets("dependencies", "yes");
    w.person_writes("brief");
    w.person_writes("plan");
    w.run("ask", &[("more", Value("yes"))]);
    w.run("extra", &[]);
    w.run("work", &[("code", Matters)]);
    // The cell has run and its items are open, but it no longer applies: it holds the work no more than a gate would.
    w.person_sets("more", "no");
    w.outside_sets("dependencies", "no");
    assert_eq!(w.decide(), resting());
    w.person_writes("plan");
    runs_cell(&w, "work");
}

#[test]
fn two_cells_waiting_for_the_same_items_say_so_once() {
    let (start, named) = waiting_for_all("may start", &["dependencies"], cell("start", &["go"], &[]));
    let again = requiring("may start", cell("again", &["go"], &[]));
    let cells = vec![start, again, cell("work", &["plan"], &["code"]), gate()];
    let mut w = World::with_tests(cells, BTreeMap::from([named]));
    w.written_outside("dependencies", "dependencies");
    w.outside_sets("dependencies", "yes");
    w.person_writes("go");
    w.person_writes("plan");
    w.run("start", &[]);
    w.run("again", &[]);
    w.person_writes("plan");
    runs_cell(&w, "work");
    w.outside_sets("dependencies", "no");
    assert_eq!(w.decide(), wait("work", vec![items()]));
}

#[test]
fn the_hold_does_not_reach_back_to_a_cell_before_the_one_that_waits_for_the_items() {
    let (gated, named) = waiting_for_all("may start", &["dependencies"], cell("gated", &["go"], &[]));
    let cells = vec![cell("first", &["plan"], &["code"]), gated, gate()];
    let mut w = World::with_tests(cells, BTreeMap::from([named]));
    w.written_outside("dependencies", "dependencies");
    w.outside_sets("dependencies", "yes");
    w.person_writes("go");
    w.person_writes("plan");
    w.run("first", &[("code", Matters)]);
    w.run("gated", &[]);
    w.outside_sets("dependencies", "no");
    // The cell before it has something new and nothing holds it: it runs, and what follows it waits for the items.
    w.person_writes("plan");
    runs_cell(&w, "first");
    w.run("first", &[("code", Matters)]);
    assert_eq!(w.decide(), wait("merge", vec![merge_word(), items()]));
}
