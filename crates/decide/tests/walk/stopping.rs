//! The owner turning to a cell (design document `data-and-context`, 33, 37, 41): a round under way of a cell after it
//! is stopped, consuming nothing, the cell turned to runs, and the stopped cell runs again later.

use super::*;

/// A research that the brief and the owner's words to it run, writing the plan, and an implementation of the plan;
/// both ran once, and the implementation's second run has its round under way. Returns the first plan, the second,
/// and that run.
fn implementing() -> (World, Seq, Seq, RunId) {
    let mut w = World::with_gate(vec![
        cell("research", &["brief", "spoken to research"], &["plan"]),
        cell("implement", &["plan"], &["code"]),
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

#[test]
fn the_owner_turning_to_an_earlier_cell_stops_a_later_round_and_the_earlier_cell_runs_first() {
    let (mut w, _, _, run) = implementing();
    w.outside_writes("spoken to research");
    let stopped = Abandoning::Stopped {
        cell: "research".into(),
        block: "spoken to research".into(),
    };
    assert_eq!(w.decide(), abandon("implement", run, stopped));
    w.abandon();
    // Nothing begins until the stopped round has ended.
    assert_eq!(w.decide(), busy("implement", run));
    w.stopped(run);
    let Decision::Run { cell, because, .. } = w.decide() else {
        panic!("nothing runs: {:?}", w.decide())
    };
    assert_eq!(cell.as_str(), "research");
    assert_eq!(because, vec![changed("spoken to research", None, false)]);
}

#[test]
fn a_stopped_round_consumed_nothing_and_its_cell_runs_again_for_what_ran_it() {
    let (mut w, first, second, run) = implementing();
    w.outside_writes("spoken to research");
    w.abandon();
    w.stopped(run);
    w.run("research", &[]);
    let Decision::Run { cell, because, .. } = w.decide() else {
        panic!("nothing runs: {:?}", w.decide())
    };
    assert_eq!(cell.as_str(), "implement");
    assert_eq!(because, vec![changed("plan", Some(first), false)]);
    let _ = second;
}

#[test]
fn a_round_under_way_is_not_stopped_for_what_is_not_the_owner_turning_to_an_earlier_cell() {
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
fn a_round_under_way_is_not_stopped_for_an_earlier_cell_that_could_not_run() {
    // The research waits on a precondition that does not hold.
    let mut w = World::with_gate(vec![
        requiring("ready", cell("research", &["brief", "spoken to research"], &["plan"])),
        cell("implement", &["plan"], &["code"]),
    ]);
    w.turning("spoken to research");
    w.person_writes("brief");
    w.run("research", &[("plan", Matters)]);
    let run = w.start("implement");
    w.says("ready", false);
    w.outside_writes("spoken to research");
    assert_eq!(w.decide(), busy("implement", run));
    // The precondition holds again: the round is stopped.
    w.says("ready", true);
    assert!(matches!(
        w.decide(),
        Decision::Abandon {
            because: Abandoning::Stopped { .. },
            ..
        }
    ));
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
