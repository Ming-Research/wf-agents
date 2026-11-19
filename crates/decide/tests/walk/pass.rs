//! The pass: a cell run only by what cells after it wrote, a way back, waits while a cell after it, short of the next
//! cell with preconditions, is to run for something else (design document `data-and-context`, 6).

use super::*;

/// A writer and two judges of what it writes, each of which may write a point back to it; the writer has written once,
/// and the first judge has judged.
fn first_judged(point: bool) -> World {
    let mut w = World::with_gate(vec![
        cell("write", &["brief", "a_points", "b_points"], &["plan"]),
        cell("judge_a", &["plan"], &["a_points"]),
        cell("judge_b", &["plan"], &["b_points"]),
    ]);
    w.person_writes("brief");
    w.run("write", &[("plan", Matters)]);
    let points: &[(&str, _)] = if point { &[("a_points", Matters)] } else { &[] };
    w.run("judge_a", points);
    w
}

#[test]
fn a_way_back_waits_while_a_later_judge_has_the_writers_version_new() {
    let w = first_judged(true);
    // The first judge's point is new to the writer, and the writer's version is new to the second judge, which judges
    // it before the writer answers.
    assert_eq!(
        reasons(&w.decide()),
        ("judge_b".to_string(), vec![("plan".to_string(), false)])
    );
}

#[test]
fn a_way_back_runs_once_nothing_after_it_is_to_run() {
    let mut w = first_judged(true);
    w.run("judge_b", &[("b_points", Matters)]);
    // Both judges spoke on the same version, and the writer answers them at once.
    assert_eq!(
        reasons(&w.decide()),
        (
            "write".to_string(),
            vec![("a_points".to_string(), true), ("b_points".to_string(), true)]
        )
    );
}

#[test]
fn a_way_back_waits_while_a_later_cell_waits_for_the_results_of_its_calls() {
    let mut w = first_judged(true);
    w.start("judge_b");
    let call = w.asks("judge_b", None);
    // The second judge is halfway through its judgment: the writer does not move what it judges under it.
    assert_eq!(w.decide(), wait("judge_b", vec![Waited::Call { call, on: None }]));
    w.done(call);
    assert_eq!(
        reasons(&w.decide()),
        (
            "judge_b".to_string(),
            vec![("plan".to_string(), false), ("results".to_string(), false)]
        )
    );
}

#[test]
fn a_way_back_waits_while_a_later_cell_has_a_run_at_rest() {
    let mut w = first_judged(true);
    w.start("judge_b");
    w.returns("judge_b");
    assert!(matches!(w.decide(), Decision::GoOn { cell, .. } if cell.as_str() == "judge_b"));
}

#[test]
fn a_way_back_is_not_passed_by_a_cell_with_preconditions() {
    let mut w = World::with_gate(vec![
        cell("write", &["brief", "points"], &["plan"]),
        requiring("go", cell("gate", &["plan", "note"], &[])),
        cell("judge", &["plan"], &["points"]),
    ]);
    w.person_writes("brief");
    w.run("write", &[("plan", Matters)]);
    w.run("gate", &[]);
    w.run("judge", &[("points", Matters)]);
    // The gate has something of a person's new to it, and its precondition holds; it is reached only from the start, so
    // the writer, whose answer may change what the gate's judge judged, runs first.
    w.person_writes("note");
    assert_eq!(
        reasons(&w.decide()),
        ("write".to_string(), vec![("points".to_string(), true)])
    );
}

#[test]
fn a_way_back_is_passed_only_by_what_comes_before_the_next_cell_with_preconditions() {
    let mut w = World::with_gate(vec![
        cell("write", &["brief", "points"], &["plan"]),
        cell("judge", &["plan"], &["points"]),
        requiring("go", cell("gate", &["plan"], &[])),
        cell("late", &["plan"], &[]),
    ]);
    w.person_writes("brief");
    w.run("write", &[("plan", Matters)]);
    w.run("judge", &[("points", Matters)]);
    // The plan is new to the gate and to the cell after it, but the look past the way back ends at the gate.
    assert_eq!(
        reasons(&w.decide()),
        ("write".to_string(), vec![("points".to_string(), true)])
    );
}

#[test]
fn a_persons_write_to_an_earlier_cell_runs_it_before_the_pass_goes_on() {
    let mut w = first_judged(true);
    assert_eq!(reasons(&w.decide()).0, "judge_b");
    // What a person writes is no way back: the writer runs at once, though the second judge has the plan new.
    w.person_writes("brief");
    assert_eq!(
        reasons(&w.decide()),
        (
            "write".to_string(),
            vec![("brief".to_string(), false), ("a_points".to_string(), true)]
        )
    );
}

#[test]
fn a_way_back_is_not_passed_by_a_later_cell_at_its_limit() {
    let mut judge_b = cell("judge_b", &["plan"], &["b_points"]);
    judge_b.calls = 1;
    let mut w = World::with_gate(vec![
        cell("write", &["brief", "a_points", "b_points"], &["plan"]),
        cell("judge_a", &["plan"], &["a_points"]),
        judge_b,
    ]);
    w.person_writes("brief");
    w.run("write", &[("plan", Matters)]);
    w.run("judge_a", &[("a_points", Matters)]);
    w.start("judge_b");
    let call = w.asks("judge_b", None);
    w.done(call);
    // The second judge is at its limit of calls and waits for the owner: the pass does not go on through it, and the
    // writer answers the first judge meanwhile.
    assert_eq!(
        reasons(&w.decide()),
        ("write".to_string(), vec![("a_points".to_string(), true)])
    );
}

// Reference: design document `data-and-context`, 6: what waits for the pass is "a cell run only by what cells after it
// wrote". A cell entered again for the results of calls its own run asked for is run by those results as well, so it is
// no way back, and it goes on with its work before the cells after it judge what it is still at work on, as the walk
// before the pass entered it.
#[test]
fn a_cell_entered_for_the_results_of_its_own_calls_is_no_way_back() {
    let mut w = first_judged(true);
    w.run("judge_b", &[]);
    // Nothing after the writer is to run, and it takes the first judge's point.
    w.start("write");
    let call = w.asks("write", None);
    // The round that asked also handed in a new version of the plan, as a round that commits hands in the head it
    // committed as it ends: the plan is new to both judges, by a cell before them.
    let asked = w.records.finished.last_mut().expect("the run that asked finished");
    asked.writes.push(Write {
        block: "plan".into(),
        meaning_unchanged: false,
        value: None,
        empty: false,
    });
    w.done(call);
    assert_eq!(
        reasons(&w.decide()),
        (
            "write".to_string(),
            vec![("a_points".to_string(), true), ("results".to_string(), false)]
        )
    );
}
