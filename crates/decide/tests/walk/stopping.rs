//! The owner turning to a cell (design document `data-and-context`, 35): a round under way of an agent's cell after it
//! is told to hand in what it has; an act of the engine's after it, or the flow's last cell, whose finish ends the work,
//! is stopped, consuming nothing; the cell turned to runs, and the cell of the round runs again later, for what its
//! round left undealt with.

use super::*;

/// A research that the brief and the owner's words to it run, writing the plan, an implementation of the plan and a
/// merge after it; the research and the implementation ran once, and the implementation's second run has its round
/// under way. Returns the first plan, the second,
/// and that run.
fn implementing() -> (World, Seq, Seq, RunId) {
    let mut w = World::with_gate(vec![
        cell("research", &["brief", "spoken to research"], &["plan"]),
        cell("implement", &["plan"], &["code"]),
        cell("merge", &["code"], &["merged"]),
    ]);
    w.turning("spoken to research");
    w.person_writes("brief");
    let first = w.run("research", &[("plan", Matters)]);
    w.run("implement", &[("code", Matters)]);
    w.person_writes("brief");
    let second = w.run("research", &[("plan", Matters)]);
    let run = w.start("implement");
    (w, first, second, run)
}

/// The hurry the walk decides on for the run, for the owner's words at this version.
fn hurry(run: RunId, version: Seq) -> Decision {
    Decision::Hurry {
        cell: "implement".into(),
        run,
        turned_to: "research".into(),
        block: "spoken to research".into(),
        version,
    }
}

#[test]
fn the_owner_turning_to_an_earlier_cell_tells_the_round_of_a_later_agent_to_hand_in_what_it_has_once_for_a_version() {
    let (mut w, _, _, run) = implementing();
    let words = w.outside_writes("spoken to research");
    assert_eq!(w.decide(), hurry(run, words));
    assert_eq!(w.hurry().as_str(), "research");
    // Told once: the round is under way until it ends, and nothing else begins.
    assert_eq!(w.decide(), busy("implement", run));
    // A later version tells it again.
    let later = w.outside_writes("spoken to research");
    assert_eq!(w.decide(), hurry(run, later));
    w.hurry();
    assert_eq!(w.decide(), busy("implement", run));
}

#[test]
fn a_round_told_to_hand_in_what_it_has_is_taken_as_any_return_is_and_the_earlier_cell_runs_before_its_cell_again() {
    let (mut w, first, _, run) = implementing();
    let words = w.outside_writes("spoken to research");
    w.hurry();
    w.given_the_hurry("implement", "spoken to research", words);
    // It hands in what it has, dealing with nothing: the run finishes, and is not abandoned.
    let told = decide::told(&w.flow, &w.records, run).expect("the run started");
    assert!(told.hurried);
    w.finish_dealing("implement", &[], &[("code", Matters)]);
    assert!(w.records.abandoned.is_empty());
    // The earlier cell runs first, for the owner's words.
    let Decision::Run { cell, because, .. } = w.decide() else {
        panic!("nothing runs: {:?}", w.decide())
    };
    assert_eq!(cell.as_str(), "research");
    assert_eq!(because, vec![changed("spoken to research", None, false)]);
    // Then the later cell runs again, for the plan it did not deal with.
    w.run("research", &[("plan", Matters)]);
    let Decision::Run { cell, because, .. } = w.decide() else {
        panic!("nothing runs: {:?}", w.decide())
    };
    assert_eq!(cell.as_str(), "implement");
    assert!(because.contains(&changed("plan", Some(first), false)), "{because:?}");
}

#[test]
fn only_a_round_given_the_telling_after_it_began_is_hurried() {
    let (mut w, _, _, run) = implementing();
    let words = w.outside_writes("spoken to research");
    w.hurry();
    // The telling was decided on, and the round was not given it: it is held to what its cell must write.
    let told = decide::told(&w.flow, &w.records, run).expect("the run started");
    assert!(!told.hurried);
    // Given the telling after it began, it is hurried.
    w.given_the_hurry("implement", "spoken to research", words);
    assert!(decide::told(&w.flow, &w.records, run).expect("the run started").hurried);
    // A later round of the same run was not given it.
    w.returns("implement");
    let at = w.tick();
    w.records.rounds.push(Round { run, at });
    assert!(!decide::told(&w.flow, &w.records, run).expect("the run started").hurried);
}

#[test]
fn what_is_put_in_to_a_round_that_its_own_cell_runs_on_is_no_telling_to_hand_in() {
    let mut w = World::with_gate(vec![cell("research", &["brief", "spoken to research"], &["plan"])]);
    w.turning("spoken to research");
    w.person_writes("brief");
    let run = w.start("research");
    let words = w.outside_writes("spoken to research");
    w.given_the_hurry("research", "spoken to research", words);
    assert!(!decide::told(&w.flow, &w.records, run).expect("the run started").hurried);
}

#[test]
fn a_round_under_way_is_not_hurried_for_what_is_not_the_owner_turning_to_an_earlier_cell() {
    // A person writes the brief, which runs the research too.
    let (mut w, _, _, run) = implementing();
    w.person_writes("brief");
    assert_eq!(w.decide(), busy("implement", run));
    // The owner turns to the cell whose round is under way, or to one after it: nothing is stopped.
    let mut w = World::with_gate(vec![
        cell("research", &["brief"], &["plan"]),
        cell("implement", &["plan", "spoken to implement"], &["code"]),
    ]);
    w.turning("spoken to implement");
    w.person_writes("brief");
    let research = w.start("research");
    w.outside_writes("spoken to implement");
    assert_eq!(w.decide(), busy("research", research));
}

#[test]
fn a_round_under_way_is_not_hurried_for_an_earlier_cell_that_could_not_run() {
    // The research waits on a precondition that does not hold.
    let mut w = World::with_gate(vec![
        requiring("ready", cell("research", &["brief", "spoken to research"], &["plan"])),
        cell("implement", &["plan"], &["code"]),
        cell("merge", &["code"], &["merged"]),
    ]);
    w.turning("spoken to research");
    w.person_writes("brief");
    w.run("research", &[("plan", Matters)]);
    let run = w.start("implement");
    w.says("ready", false);
    w.outside_writes("spoken to research");
    assert_eq!(w.decide(), busy("implement", run));
    // The precondition holds again: the round is told to hand in what it has.
    w.says("ready", true);
    assert!(matches!(w.decide(), Decision::Hurry { .. }));
}

#[test]
fn a_round_of_an_act_of_the_engine_cannot_be_told_and_is_stopped_for_the_owner_turning_to_an_earlier_cell() {
    let mut act = cell("check", &["plan"], &["checked"]);
    act.slot = false;
    let mut w = World::with_gate(vec![cell("research", &["brief", "spoken to research"], &["plan"]), act]);
    w.turning("spoken to research");
    w.person_writes("brief");
    w.run("research", &[("plan", Matters)]);
    w.person_writes("brief");
    w.run("research", &[("plan", Matters)]);
    let run = w.start("check");
    w.outside_writes("spoken to research");
    let stopped = Abandoning::Stopped {
        cell: "research".into(),
        block: "spoken to research".into(),
    };
    assert_eq!(w.decide(), abandon("check", run, stopped));
    w.abandon();
    // Nothing begins until the stopped round has ended; the cell it was stopped for then runs first.
    assert_eq!(w.decide(), busy("check", run));
    w.stopped(run);
    let Decision::Run { cell, .. } = w.decide() else {
        panic!("nothing runs: {:?}", w.decide())
    };
    assert_eq!(cell.as_str(), "research");
}

#[test]
fn a_round_of_the_last_cell_cannot_be_told_to_hand_in_for_its_finish_ends_the_work_and_is_stopped() {
    let mut w = World::with_gate(vec![
        cell("research", &["brief", "spoken to research"], &["plan"]),
        cell("implement", &["plan"], &["code"]),
    ]);
    w.turning("spoken to research");
    w.person_writes("brief");
    w.run("research", &[("plan", Matters)]);
    w.person_writes("brief");
    w.run("research", &[("plan", Matters)]);
    let run = w.start("implement");
    w.outside_writes("spoken to research");
    let stopped = Abandoning::Stopped {
        cell: "research".into(),
        block: "spoken to research".into(),
    };
    assert_eq!(w.decide(), abandon("implement", run, stopped));
}

#[test]
fn the_owner_turning_to_a_cell_that_waits_for_the_results_of_its_calls_enters_it_and_it_goes_on_waiting() {
    let mut w = World::with_gate(vec![cell("research", &["brief", "spoken to research"], &["plan"])]);
    w.turning("spoken to research");
    let brief = w.person_writes("brief");
    w.start("research");
    let call = w.asks("research", None);
    assert_eq!(w.decide(), wait("research", vec![Waited::Call { call, on: None }]));
    let words = w.outside_writes("spoken to research");
    // Entered for the words alone: the brief it has not dealt with is no reason while it waits.
    assert_eq!(
        w.decide(),
        runs(
            "research",
            vec![changed("spoken to research", None, false)],
            &[
                ("brief", Some(brief)),
                ("spoken to research", Some(words)),
                ("plan", None)
            ]
        )
    );
    let answering = w.start("research");
    // The round answers the owner and the cell goes on waiting: it is held to nothing its cell must write.
    let told = decide::told(&w.flow, &w.records, answering).expect("the run started");
    assert!(told.waits);
    assert_eq!(
        decide::owed(&w.flow, &w.records),
        BTreeMap::from([("research".into(), vec![call])])
    );
    w.finish("research", &[]);
    assert_eq!(w.decide(), wait("research", vec![Waited::Call { call, on: None }]));
    w.done(call);
    assert_eq!(
        w.decide(),
        runs(
            "research",
            vec![changed("brief", None, false), results(&[call])],
            &[
                ("brief", Some(brief)),
                ("spoken to research", Some(words)),
                ("plan", None)
            ]
        )
    );
    // The run entered for the results is the cell's work again, held to what it must write.
    let given = w.start("research");
    let told = decide::told(&w.flow, &w.records, given).expect("the run started");
    assert!(!told.waits);
}

#[test]
fn a_run_for_the_owner_that_asks_for_calls_in_turn_leaves_the_owners_words_for_the_run_their_results_begin() {
    let mut w = World::with_gate(vec![cell("research", &["brief", "spoken to research"], &["plan"])]);
    w.turning("spoken to research");
    w.person_writes("brief");
    w.start("research");
    let first = w.asks("research", None);
    w.outside_writes("spoken to research");
    w.start("research");
    // The run entered for the words asks for a call of its own, dealing with nothing: it is not entered again for
    // the same words while its calls have no results.
    let second = w.asks("research", None);
    assert_eq!(
        w.decide(),
        wait(
            "research",
            vec![
                Waited::Call { call: first, on: None },
                Waited::Call { call: second, on: None }
            ]
        )
    );
    // Once they have, the run their results begin deals with the words as well.
    w.done(first);
    w.done(second);
    let Decision::Run { because, .. } = w.decide() else {
        panic!("nothing runs: {:?}", w.decide())
    };
    assert!(
        because.contains(&changed("spoken to research", None, false)),
        "{because:?}"
    );
    assert!(because.contains(&results(&[first, second])), "{because:?}");
}

#[test]
fn the_owners_words_told_to_a_run_that_was_abandoned_enter_the_waiting_cell_again() {
    let mut w = World::with_gate(vec![cell("research", &["brief", "spoken to research"], &["plan"])]);
    w.turning("spoken to research");
    w.person_writes("brief");
    w.start("research");
    let call = w.asks("research", None);
    w.outside_writes("spoken to research");
    let answering = w.start("research");
    // The run entered for the words is abandoned with its round under way, and nothing it returned is taken.
    let at = w.tick();
    w.records.abandoned.push(decide::Abandoned { run: answering, at });
    w.stopped(answering);
    let Decision::Run { because, .. } = w.decide() else {
        panic!("nothing runs: {:?}", w.decide())
    };
    assert_eq!(because, [changed("spoken to research", None, false)]);
    // Seen by a run that asked for calls in turn, they wait for the results.
    w.start("research");
    let second = w.asks("research", None);
    assert_eq!(
        w.decide(),
        wait(
            "research",
            vec![Waited::Call { call, on: None }, Waited::Call { call: second, on: None }]
        )
    );
}
