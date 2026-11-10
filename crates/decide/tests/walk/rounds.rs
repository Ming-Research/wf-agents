//! Rounds, runs at rest, calls and pausing.

use super::*;

#[test]
fn a_run_is_busy_only_while_a_round_is_under_way_and_at_rest_goes_on_when_the_walk_reaches_it() {
    let mut w = World::with_gate(vec![cell("a", &[], &[]), cell("b", &[], &[])]);
    let run = w.start("a");
    assert_eq!(w.decide(), busy("a", run));
    w.returns("a");
    assert_eq!(w.decide(), go_on("a", run));
    w.go_on("a");
    assert_eq!(w.decide(), busy("a", run));
    w.finish("a", &[]);
    assert_eq!(w.decide(), runs("b", vec![NO_RECORD], &[]));
}

#[test]
fn a_cell_before_a_run_at_rest_runs_first_and_the_run_then_goes_on_if_nothing_it_started_from_changed() {
    let (mut w, _, _) = reviewed();
    let p2 = w.person_writes("plan");
    let code = w.run("implement", &[("code", Matters)]);
    let review = w.start("review");
    w.returns("review");
    // A person rewrites the plan while the review rests between two rounds: the implementation, before it, runs.
    let p3 = w.person_writes("plan");
    assert_eq!(
        w.decide(),
        runs(
            "implement",
            vec![changed("plan", Some(p2), false)],
            &[("plan", Some(p3)), ("opinions", None), ("code", Some(code))]
        )
    );
    // The new plan changes nothing in the code: the review goes on where it was.
    w.run("implement", &[]);
    assert_eq!(w.decide(), go_on("review", review));
}

#[test]
fn a_cell_after_a_run_at_rest_does_not_run_while_the_run_is_open() {
    let mut w = World::with_gate(vec![cell("a", &["x"], &[]), cell("b", &["y"], &[])]);
    w.person_writes("x");
    let y1 = w.person_writes("y");
    w.run("a", &[]);
    w.run("b", &[]);
    w.person_writes("x");
    let a = w.start("a");
    w.returns("a");
    let y2 = w.person_writes("y");
    assert_eq!(w.decide(), go_on("a", a));
    w.go_on("a");
    w.finish("a", &[]);
    assert_eq!(
        w.decide(),
        runs("b", vec![changed("y", Some(y1), false)], &[("y", Some(y2))])
    );
}

#[test]
fn a_run_at_rest_at_a_call_for_a_person_waits_for_that_person_and_goes_on_with_the_result() {
    let mut w = World::with_gate(vec![cell("a", &[], &[])]);
    let run = w.start("a");
    let call = w.calls("a", Some(Who::Architect));
    let on = Some(Who::Architect);
    assert_eq!(w.decide(), wait("a", vec![Waited::Call { call, on }]));
    w.done(call);
    assert_eq!(w.decide(), go_on("a", run));
}

#[test]
fn a_run_at_rest_at_a_call_the_engine_makes_waits_on_nobody_and_goes_on_when_the_call_is_done() {
    let mut w = World::with_gate(vec![cell("a", &[], &[])]);
    let run = w.start("a");
    let call = w.calls("a", None);
    assert_eq!(w.decide(), wait("a", vec![Waited::Call { call, on: None }]));
    w.done(call);
    assert_eq!(w.decide(), go_on("a", run));
}

#[test]
fn an_answered_call_does_not_make_its_run_go_on_while_a_round_of_another_run_is_under_way() {
    let (mut w, _, _) = reviewed();
    w.person_writes("plan");
    w.run("implement", &[("code", Matters)]);
    let review = w.start("review");
    let call = w.calls("review", Some(Who::Owner));
    w.person_writes("plan");
    let implement = w.start("implement");
    w.done(call);
    assert_eq!(w.decide(), busy("implement", implement));
    w.finish("implement", &[]);
    assert_eq!(w.decide(), go_on("review", review));
}

#[test]
fn a_paused_work_begins_no_round_while_a_round_under_way_is_still_busy_and_resuming_lets_it_on() {
    let mut w = World::with_gate(vec![cell("a", &[], &[]), cell("b", &[], &[])]);
    let run = w.start("a");
    w.pause();
    assert_eq!(w.decide(), busy("a", run));
    w.returns("a");
    assert_eq!(w.decide(), Decision::Paused);
    w.resume();
    assert_eq!(w.decide(), go_on("a", run));
    w.go_on("a");
    w.finish("a", &[]);
    w.pause();
    assert_eq!(w.decide(), Decision::Paused);
    w.resume();
    assert_eq!(w.decide(), runs("b", vec![NO_RECORD], &[]));
}

#[test]
fn a_paused_work_with_a_stale_run_at_rest_is_paused() {
    let (mut w, ..) = planned();
    w.person_writes("background");
    w.start("research");
    w.returns("research");
    w.person_writes("background");
    w.pause();
    assert_eq!(w.decide(), Decision::Paused);
}
