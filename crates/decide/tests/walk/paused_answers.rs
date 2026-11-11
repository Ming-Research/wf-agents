//! A paused work still answers a note, a person's words to a cell: while it is paused, a run starts only for a cell
//! one of whose reasons is a change of a note it reads that a person made since the cell last ran: since the run of its
//! record started, or with no record since its first run started; a cell no run of which has started waits. Nothing
//! else starts, whatever else a person writes, the order of the walk is kept, and what that run writes waits for the
//! work to be let on.

use decide::run::Hold;

use super::*;

/// A research that reads the background, which people write, the outside's main and a note to it, and an
/// implementation that reads the plan and a note to it; both ran once. Returns the plan and the code.
fn ran() -> (World, Seq, Seq) {
    let cells = vec![
        cell("research", &["background", "main", "to research"], &["plan"]),
        cell("implement", &["plan", "main", "to implement"], &["code"]),
    ];
    let mut w = World::with_gate(cells);
    w.notes(&["to research", "to implement"]);
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
fn a_paused_work_does_not_answer_what_a_person_wrote_that_is_no_note() {
    let (mut w, ..) = ran();
    w.pause();
    w.person_writes("background");
    assert_eq!(w.decide(), Decision::Paused);
    // A note besides it is answered, and the run is for both.
    w.person_writes("to research");
    assert_eq!(
        reasons(&w.decide()),
        (
            "research".into(),
            vec![("background".into(), false), ("to research".into(), false)]
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
fn a_cell_that_never_ran_waits_for_the_work_to_be_let_on_though_a_person_wrote_a_note_it_reads() {
    let mut w = World::with_gate(vec![cell("research", &["to research"], &["plan"])]);
    w.notes(&["to research"]);
    w.pause();
    w.person_writes("to research");
    // It runs because it has no record: the work, added paused, begins nothing before it is let on.
    assert_eq!(w.decide(), Decision::Paused);
}

/// A research that ran once, and an implementation of the plan that reads a note to it, whose first run has started:
/// the plan and that run.
fn implementing() -> (World, Seq, RunId) {
    let mut w = World::with_gate(vec![
        cell("research", &["background"], &["plan"]),
        cell("implement", &["plan", "to implement"], &["code"]),
    ]);
    w.notes(&["to implement"]);
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
fn a_note_is_answered_until_its_cell_finishes_a_run_though_the_run_for_it_was_abandoned() {
    let (mut w, plan, first) = implementing();
    w.returns("implement");
    w.pause();
    let to = w.person_writes("to implement");
    assert_eq!(w.decide(), abandon("implement", first, outdated("to implement")));
    w.abandon();
    // The run that answers the note starts, and the outside writes the code, which it may write: it is abandoned.
    let answering = w.start("implement");
    w.returns("implement");
    w.outside_writes("code");
    assert_eq!(w.decide(), abandon("implement", answering, outdated("code")));
    w.abandon();
    // The cell has still no record, and the note came after its first run started: it is still answered.
    let code = w.records.written.last().expect("a write").at;
    assert_eq!(
        w.decide(),
        runs(
            "implement",
            vec![NO_RECORD],
            &[("plan", Some(plan)), ("to implement", Some(to)), ("code", Some(code))]
        )
    );
}

#[test]
fn a_paused_work_does_not_answer_a_note_written_before_the_cell_last_ran_that_a_changed_flow_gives_it() {
    let mut w = World::with_gate(vec![cell("implement", &["plan"], &["code"])]);
    w.notes(&["to implement"]);
    w.person_writes("to implement");
    w.person_writes("plan");
    w.run("implement", &[("code", Matters)]);
    w.pause();
    // The changed flow gives the implementation the note, which a person wrote before it last ran.
    w.install(vec![cell("implement", &["plan", "to implement"], &["code"]), gate()]);
    assert_eq!(w.decide(), Decision::Paused);
    // Written again by a person, it is answered.
    w.person_writes("to implement");
    assert_eq!(
        reasons(&w.decide()),
        ("implement".into(), vec![("to implement".into(), false)])
    );
}

#[test]
fn a_pause_holds_back_the_rounds_of_a_run_started_for_a_note_written_before_the_cell_last_ran() {
    let mut w = World::with_gate(vec![cell("implement", &["plan"], &["code"])]);
    w.notes(&["to implement"]);
    w.person_writes("to implement");
    w.person_writes("plan");
    w.run("implement", &[("code", Matters)]);
    w.install(vec![cell("implement", &["plan", "to implement"], &["code"]), gate()]);
    let run = w.start("implement");
    w.returns("implement");
    w.pause();
    assert_eq!(decide::hold(&w.flow, &w.records, run), Hold::Rounds);
}

#[test]
fn a_pause_holds_back_nothing_of_a_run_for_what_a_person_wrote_but_a_round_after_one_whose_failure_paused_it() {
    let (mut w, ..) = ran();
    // Started before the owner's pause: nothing is held, a round that failed without pausing the work included.
    w.person_writes("to implement");
    let before = w.start("implement");
    w.returns("implement");
    assert_eq!(decide::hold(&w.flow, &w.records, before), Hold::Nothing);
    w.pause();
    assert_eq!(decide::hold(&w.flow, &w.records, before), Hold::Nothing);
    assert_eq!(w.decide(), go_on("implement", before));
    w.go_on("implement");
    // Its round fails, and the engine pauses the work for it: the round after it waits for the work to be let on.
    w.fails("implement", "it could not start");
    assert_eq!(decide::hold(&w.flow, &w.records, before), Hold::Retry);
    // Let on and paused again by the owner, nothing is held any more.
    w.resume();
    w.pause();
    assert_eq!(decide::hold(&w.flow, &w.records, before), Hold::Nothing);
    w.go_on("implement");
    w.finish("implement", &[("code", Matters)]);
    // Started during the pause, and its round failed so too.
    w.person_writes("to implement");
    let during = w.start("implement");
    w.returns("implement");
    assert_eq!(decide::hold(&w.flow, &w.records, during), Hold::Nothing);
    w.engine_pauses("the round failed");
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
    let mut first = World::with_gate(vec![cell("research", &["to research"], &["plan"])]);
    first.notes(&["to research"]);
    first.person_writes("to research");
    let run = first.start("research");
    first.returns("research");
    first.pause();
    assert_eq!(decide::hold(&first.flow, &first.records, run), Hold::Rounds);
}

#[test]
fn a_persons_choice_runs_nothing_while_the_work_is_paused() {
    let (mut w, _) = chosen();
    w.pause();
    // A person writes the choice as it stands: its meaning does not change, and no cell has a reason to run.
    w.person_confirms("choice", "yes");
    assert_eq!(w.decide(), resting());
    // Changed by the person, the choice is a reason of the cell that reads it, and no note: it waits.
    w.person_sets("choice", "no");
    assert_eq!(w.decide(), Decision::Paused);
    w.resume();
    assert_eq!(reasons(&w.decide()), ("use".into(), vec![("choice".into(), false)]));
}
