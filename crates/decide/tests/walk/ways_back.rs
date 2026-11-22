//! A way back: a cell run by what a cell after it wrote is entered as soon as the walk reaches it, as any cell that
//! something new runs, before a cell after it runs for anything of its own (decision the-walk-goes-from-the-first-cell).

use super::*;

/// The implementation and three reviews of its code, each of which may write a point back to it, as the three reviews
/// of a change in the engineer's flow do; a person has written the plan.
fn three_reviews() -> World {
    let mut w = World::with_gate(vec![
        cell("implement", &["plan", "a_points", "b_points", "c_points"], &["code"]),
        cell("review_a", &["code"], &["a_points"]),
        cell("review_b", &["code"], &["b_points"]),
        cell("review_c", &["code"], &["c_points"]),
    ]);
    w.person_writes("plan");
    w
}

// The first review's point is new to the implementation, by a cell after it, and the first code is new to the two
// other reviews. The implementation answers first, the first review looks at its fix, and the second review then
// judges the fixed code: it never judges the code the point was about.
#[test]
fn a_reviews_point_runs_the_implementation_before_the_next_review_and_that_review_sees_the_fix() {
    let mut w = three_reviews();
    w.run("implement", &[("code", Matters)]);
    w.run("review_a", &[("a_points", Matters)]);
    assert_eq!(
        reasons(&w.decide()),
        ("implement".to_string(), vec![("a_points".to_string(), true)])
    );
    let fixed = w.run("implement", &[("code", Matters)]);
    assert_eq!(
        reasons(&w.decide()),
        ("review_a".to_string(), vec![("code".to_string(), false)])
    );
    w.run("review_a", &[]);
    assert_eq!(
        w.decide(),
        runs(
            "review_b",
            first(&["code"]),
            &[("code", Some(fixed)), ("b_points", None)]
        )
    );
}

// A cell that looks for a better way hands one to the plan's writer while the plan is new to both of its judges. The
// writer takes the better way first, and the first judge judges only the plan the better way led to.
#[test]
fn a_better_way_runs_the_plans_writer_before_the_judges_of_the_plan() {
    let mut w = World::with_gate(vec![
        cell(
            "write",
            &["brief", "better", "a_points", "b_points"],
            &["plan", "better"],
        ),
        cell("look", &["plan", "better"], &["better"]),
        cell("judge_a", &["plan"], &["a_points"]),
        cell("judge_b", &["plan"], &["b_points"]),
    ]);
    w.person_writes("brief");
    w.run("write", &[("plan", Matters)]);
    w.run("look", &[("better", Matters)]);
    assert_eq!(
        reasons(&w.decide()),
        ("write".to_string(), vec![("better".to_string(), true)])
    );
    // The writer changes the plan to the better way and answers; the look reads both and has nothing more to say.
    let better = w.run("write", &[("plan", Matters), ("better", Matters)]);
    w.run("look", &[]);
    assert_eq!(
        w.decide(),
        runs(
            "judge_a",
            first(&["plan"]),
            &[("plan", Some(better)), ("a_points", None)]
        )
    );
}

// The walk does not wait for a cell after a way back to finish what it is doing. The third review waits for the
// results of its calls, which holds the work there; meanwhile a person changes the plan, the implementation and the
// first review run again, and the first review's point runs the implementation at once.
#[test]
fn a_way_back_runs_while_a_cell_after_it_waits_for_the_results_of_its_calls() {
    let mut w = three_reviews();
    w.run("implement", &[("code", Matters)]);
    w.run("review_a", &[]);
    w.run("review_b", &[]);
    w.start("review_c");
    let call = w.asks("review_c", None);
    assert_eq!(w.decide(), wait("review_c", vec![Waited::Call { call, on: None }]));
    w.person_writes("plan");
    w.run("implement", &[("code", Matters)]);
    w.run("review_a", &[("a_points", Matters)]);
    assert_eq!(
        reasons(&w.decide()),
        ("implement".to_string(), vec![("a_points".to_string(), true)])
    );
}

// Nor does it wait for a cell after a way back whose run rests between two rounds: the implementation answers the
// point first, and the run at rest goes on once the walk reaches it.
#[test]
fn a_way_back_runs_while_a_cell_after_it_has_a_run_at_rest() {
    let mut w = three_reviews();
    w.run("implement", &[("code", Matters)]);
    w.run("review_a", &[]);
    w.run("review_b", &[]);
    let rests = w.start("review_c");
    w.returns("review_c");
    assert_eq!(w.decide(), go_on("review_c", rests));
    w.person_writes("plan");
    w.run("implement", &[("code", Matters)]);
    w.run("review_a", &[("a_points", Matters)]);
    assert_eq!(
        reasons(&w.decide()),
        ("implement".to_string(), vec![("a_points".to_string(), true)])
    );
}
