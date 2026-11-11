//! A paused work still answers what a person writes for a cell: while it is paused, a run starts only for a cell one
//! of whose reasons is a change of a block it reads that a person made since the cell last ran: since the run of its
//! record started, or with no record since its latest run started; a cell no run of which has started waits. Nothing
//! else starts, the order of the walk is kept, and what that run writes waits for the work to be let on.

use decide::run::Hold;

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

/// A research that ran once, and an implementation of the plan that reads what a person says to it, whose first run
/// has started: the plan and that run.
fn implementing() -> (World, Seq, RunId) {
    let mut w = World::with_gate(vec![
        cell("research", &["background"], &["plan"]),
        cell("implement", &["plan", "to implement"], &["code"]),
    ]);
    w.person_writes("background");
    let plan = w.run("research", &[("plan", Matters)]);
    let run = w.start("implement");
    (w, plan, run)
}

#[test]
fn a_cell_whose_first_run_was_abandoned_for_what_a_person_wrote_runs_again_while_the_work_is_paused() {
    let (mut w, plan, run) = implementing();
    w.returns("implement");
    w.pause();
    let to = w.person_writes("to implement");
    assert_eq!(w.decide(), abandon("implement", run, outdated("to implement")));
    w.abandon();
    // The cell has no record, and it ran before the person wrote: it runs for what the person wrote.
    assert_eq!(
        w.decide(),
        runs(
            "implement",
            vec![NO_RECORD],
            &[("plan", Some(plan)), ("to implement", Some(to)), ("code", None)]
        )
    );
}

#[test]
fn a_cell_whose_first_round_was_under_way_as_a_person_wrote_to_it_runs_again_while_the_work_is_paused() {
    let (mut w, plan, run) = implementing();
    w.pause();
    let to = w.person_writes("to implement");
    assert_eq!(w.decide(), busy("implement", run));
    w.returns("implement");
    assert_eq!(w.decide(), abandon("implement", run, outdated("to implement")));
    w.abandon();
    assert_eq!(
        w.decide(),
        runs(
            "implement",
            vec![NO_RECORD],
            &[("plan", Some(plan)), ("to implement", Some(to)), ("code", None)]
        )
    );
}

#[test]
fn a_paused_work_does_not_answer_what_a_person_wrote_before_the_cell_last_ran_that_a_changed_flow_gives_it() {
    let mut w = World::with_gate(vec![cell("implement", &["plan"], &["code"])]);
    w.person_writes("background");
    w.person_writes("plan");
    w.run("implement", &[("code", Matters)]);
    w.pause();
    // The changed flow gives the implementation the background, which a person wrote before it last ran.
    w.install(vec![cell("implement", &["plan", "background"], &["code"]), gate()]);
    assert_eq!(w.decide(), Decision::Paused);
    // Written again by a person, it is answered.
    w.person_writes("background");
    assert_eq!(
        reasons(&w.decide()),
        ("implement".into(), vec![("background".into(), false)])
    );
}

#[test]
fn a_pause_holds_back_the_rounds_of_a_run_started_for_what_a_person_wrote_before_the_cell_last_ran() {
    let mut w = World::with_gate(vec![cell("implement", &["plan"], &["code"])]);
    w.person_writes("background");
    w.person_writes("plan");
    w.run("implement", &[("code", Matters)]);
    w.install(vec![cell("implement", &["plan", "background"], &["code"]), gate()]);
    let run = w.start("implement");
    w.returns("implement");
    w.pause();
    assert_eq!(decide::hold(&w.flow, &w.records, run), Hold::Rounds);
}

#[test]
fn a_pause_holds_back_nothing_of_a_run_for_what_a_person_wrote_but_a_failed_round() {
    let (mut w, ..) = ran();
    // Started before the pause.
    w.person_writes("to implement");
    let before = w.start("implement");
    w.returns("implement");
    assert_eq!(decide::hold(&w.flow, &w.records, before), Hold::Nothing);
    w.pause();
    assert_eq!(decide::hold(&w.flow, &w.records, before), Hold::Retry);
    assert_eq!(w.decide(), go_on("implement", before));
    w.go_on("implement");
    w.finish("implement", &[("code", Matters)]);
    // Started during the pause.
    w.person_writes("to implement");
    let during = w.start("implement");
    w.returns("implement");
    assert_eq!(decide::hold(&w.flow, &w.records, during), Hold::Retry);
}

#[test]
fn a_pause_holds_back_every_round_of_a_run_for_what_no_person_wrote() {
    let (mut w, ..) = ran();
    w.outside_writes("main");
    let research = w.start("research");
    w.returns("research");
    w.pause();
    assert_eq!(decide::hold(&w.flow, &w.records, research), Hold::Rounds);
    let mut first = World::with_gate(vec![cell("research", &["background"], &["plan"])]);
    first.person_writes("background");
    let run = first.start("research");
    first.returns("research");
    first.pause();
    assert_eq!(decide::hold(&first.flow, &first.records, run), Hold::Rounds);
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
