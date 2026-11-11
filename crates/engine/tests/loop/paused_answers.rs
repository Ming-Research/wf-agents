//! A paused work still answers what a person writes for a cell: the run that starts for it has every round it needs,
//! its calls made and its returns sent back, and finishes while the work stays paused; what it wrote then waits for
//! the work to be let on. A round of it that fails is given again only once the work is let on.

use super::*;

/// A work whose writer and judge each ran once, the judge with a verdict and no opinions, resting at the merge for
/// the owner's word, then paused by the owner. Returns the request and the text.
fn ran_and_paused() -> (W, Seq, Seq) {
    let (mut w, write, request) = W::requested();
    let judge = round(&w.returns(write, hands_in(&[("text", None)]))).0;
    let text = w.last_finished();
    assert_eq!(w.returns(judge, hands_in(&[("verdict", Some("yes"))])), vec![]);
    assert_eq!(
        w.on(Event::Paused {
            by: Pauser::Owner,
            why: None
        }),
        vec![]
    );
    (w, request, text)
}

#[test]
fn a_run_for_what_a_person_wrote_during_a_pause_has_its_rounds_and_calls_and_finishes_while_the_work_stays_paused() {
    let (mut w, request, text) = ran_and_paused();
    let (effects, opinions) = w.person_writes("opinions");
    let again = w.work.records.started.last().expect("a run").run;
    assert_eq!(effects, vec![round_effect(again, "write", 1, Given::Task)]);
    let start = started_with(
        vec![changed("opinions", None, false)],
        &[
            ("request", Some(request)),
            ("opinions", Some(opinions)),
            ("main", None),
            ("text", Some(text)),
        ],
    );
    assert_eq!(start_of(&w, again), start);
    // A tool, then a question for the owner: each made, and the next round given its result.
    let tools = calls_of(&w.returns(again, Came::Asks(vec![tool("a")])));
    assert_eq!(
        w.done(tools[0].0),
        vec![round_effect(again, "write", 2, Given::Results(vec![tools[0].0]))]
    );
    let questions = calls_of(&w.returns(again, Came::Asks(vec![question(Who::Owner)])));
    assert_eq!(
        w.done(questions[0].0),
        vec![round_effect(again, "write", 3, Given::Results(vec![questions[0].0]))]
    );
    // What does not meet what the flow requires is sent back.
    assert_eq!(
        w.returns(again, wanting()),
        vec![round_effect(again, "write", 4, Given::SentBack(wanting_unmet()))]
    );
    // It finishes; the judge would run for the new text, and waits for the work to be let on.
    assert_eq!(w.returns(again, hands_in(&[("text", None)])), vec![]);
    assert!(w.work.records.finished.iter().any(|f| f.run == again));
    assert_eq!(w.decision(), decide::Decision::Paused);
    let effects = w.on(Event::Resumed);
    let judge = w.work.records.started.last().expect("a run").run;
    assert_eq!(effects, vec![round_effect(judge, "judge", 1, Given::Task)]);
}

#[test]
fn a_failed_round_of_a_run_for_what_a_person_wrote_is_given_again_only_once_the_work_is_let_on() {
    let (mut w, ..) = ran_and_paused();
    let (effects, _) = w.person_writes("opinions");
    let (again, ..) = round(&effects);
    let failed = Event::Failed {
        run: again,
        why: "the agent could not start".to_string(),
        spent: spent(1),
    };
    assert_eq!(w.on(failed), vec![]);
    let pause = decide::pause(&w.work.records).expect("the work is paused");
    assert_eq!(pause.by, Pauser::Engine);
    assert_eq!(w.on(Event::Resumed), vec![round_effect(again, "write", 2, Given::Task)]);
}

#[test]
fn a_first_run_abandoned_for_what_a_person_wrote_during_a_pause_is_run_again_at_once() {
    let (mut w, write, request) = W::requested();
    w.on(Event::Paused {
        by: Pauser::Owner,
        why: None,
    });
    let (effects, opinions) = w.person_writes("opinions");
    assert_eq!(effects, vec![]);
    // What the round hands in is dropped, the run stale; the writer has no record, and it ran before the person wrote.
    let effects = w.returns(write, hands_in(&[("text", None)]));
    assert!(w.work.records.abandoned.iter().any(|a| a.run == write));
    let again = w.work.records.started.last().expect("a run").run;
    assert_eq!(effects, vec![round_effect(again, "write", 1, Given::Task)]);
    let start = started_with(
        vec![Cause::NoRecord],
        &[
            ("request", Some(request)),
            ("opinions", Some(opinions)),
            ("main", None),
            ("text", None),
        ],
    );
    assert_eq!(start_of(&w, again), start);
}

#[test]
fn a_run_for_what_a_person_wrote_that_started_before_the_pause_has_its_next_round_during_it() {
    let (mut w, ..) = ran_and_paused();
    w.on(Event::Resumed);
    let (effects, _) = w.person_writes("opinions");
    let (again, ..) = round(&effects);
    let call = calls_of(&w.returns(again, Came::Asks(vec![tool("a")])))[0].0;
    w.on(Event::Paused {
        by: Pauser::Owner,
        why: None,
    });
    assert_eq!(
        w.done(call),
        vec![round_effect(again, "write", 2, Given::Results(vec![call]))]
    );
}

#[test]
fn a_run_at_rest_for_what_no_person_wrote_is_held_while_a_later_cell_has_what_a_person_wrote() {
    let (mut w, write, _) = W::requested();
    let call = calls_of(&w.returns(write, Came::Asks(vec![tool("a")])))[0].0;
    w.on(Event::Paused {
        by: Pauser::Owner,
        why: None,
    });
    // The writer ran for having no record; its round given the tool's result waits for the work to be let on, whatever
    // a person writes for a cell after it.
    assert_eq!(w.done(call), vec![]);
    assert_eq!(w.on(wrote("verdict", Outsider::Person, true, Some("no"))), vec![]);
    assert_eq!(
        w.on(Event::Resumed),
        vec![round_effect(write, "write", 2, Given::Results(vec![call]))]
    );
}
