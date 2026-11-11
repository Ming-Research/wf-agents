//! A paused work still answers what a person writes for a cell: while it is paused, a run starts only for a cell one
//! of whose reasons is a change of a block it reads that a person made since the cell last ran. Nothing else starts,
//! the order of the walk is kept, and what that run writes waits for the work to be let on.

use super::*;

/// A research that reads the background, the outside's main and what a person says to it, and an implementation that
/// reads the plan and what a person says to it; both ran once. Returns the plan and the code.
fn ran() -> (World, Seq, Seq) {
    let cells = vec![
        cell("research", &["background", "main", "to research"], &["plan"]),
        cell("implement", &["plan", "main", "to implement"], &["code"]),
    ];
    let mut w = World::with_gate(cells);
    w.person_writes("background");
    let plan = w.run("research", &[("plan", Matters)]);
    let code = w.run("implement", &[("code", Matters)]);
    assert_eq!(w.decide(), resting());
    (w, plan, code)
}

#[test]
fn a_paused_work_runs_a_cell_for_what_a_person_wrote_to_it_since_it_last_ran() {
    let (mut w, plan, code) = ran();
    w.pause();
    let to = w.person_writes("to implement");
    assert_eq!(
        w.decide(),
        runs(
            "implement",
            vec![changed("to implement", None, false)],
            &[
                ("plan", Some(plan)),
                ("main", None),
                ("to implement", Some(to)),
                ("code", Some(code))
            ]
        )
    );
}

#[test]
fn a_paused_work_whose_cell_needs_to_run_for_what_no_person_wrote_is_paused() {
    let (mut w, ..) = ran();
    w.pause();
    w.outside_writes("main");
    assert_eq!(w.decide(), Decision::Paused);
    w.resume();
    assert_eq!(reasons(&w.decide()), ("research".into(), vec![("main".into(), false)]));
}

#[test]
fn a_paused_work_engine_paused_still_answers_what_a_person_wrote() {
    let (mut w, ..) = ran();
    w.engine_pauses("the work spent more than it may");
    w.person_writes("to research");
    assert_eq!(
        reasons(&w.decide()),
        ("research".into(), vec![("to research".into(), false)])
    );
}

#[test]
fn once_the_run_for_what_a_person_wrote_finishes_a_paused_work_runs_nothing_for_what_it_wrote() {
    let (mut w, ..) = ran();
    w.pause();
    w.person_writes("to research");
    w.run("research", &[("plan", Matters)]);
    // The implementation reads the new plan, which a cell wrote: it waits for the work to be let on.
    assert_eq!(w.decide(), Decision::Paused);
    w.resume();
    assert_eq!(reasons(&w.decide()), ("implement".into(), vec![("plan".into(), false)]));
}

#[test]
fn a_paused_work_whose_earlier_cell_needs_to_run_for_something_else_answers_a_later_cell_only_once_let_on() {
    let (mut w, ..) = ran();
    w.pause();
    w.outside_writes("main");
    w.person_writes("to implement");
    // The research comes first in the order, and runs for what no person wrote.
    assert_eq!(w.decide(), Decision::Paused);
    w.resume();
    assert_eq!(reasons(&w.decide()), ("research".into(), vec![("main".into(), false)]));
}

#[test]
fn a_cell_whose_reasons_include_what_a_person_wrote_and_more_runs_while_the_work_is_paused() {
    let (mut w, ..) = ran();
    w.pause();
    w.outside_writes("main");
    w.person_writes("to research");
    assert_eq!(
        reasons(&w.decide()),
        (
            "research".into(),
            vec![("main".into(), false), ("to research".into(), false)]
        )
    );
}

#[test]
fn a_cell_that_never_ran_waits_for_the_work_to_be_let_on_though_a_person_wrote_what_it_reads() {
    let mut w = World::with_gate(vec![cell("research", &["background"], &["plan"])]);
    w.pause();
    w.person_writes("background");
    // It runs because it has no record: the work, added paused, begins nothing before it is let on.
    assert_eq!(w.decide(), Decision::Paused);
}

#[test]
fn a_persons_confirmation_of_a_choice_runs_nothing_while_the_work_is_paused() {
    let (mut w, _) = chosen();
    w.pause();
    // A person writes the choice as it stands: its meaning does not change, and no cell has a reason to run.
    w.person_confirms("choice", "yes");
    assert_eq!(w.decide(), resting());
    // Changed by the person, the choice is a reason of the cell that reads it, which runs.
    w.person_sets("choice", "no");
    assert_eq!(reasons(&w.decide()), ("use".into(), vec![("choice".into(), false)]));
}
