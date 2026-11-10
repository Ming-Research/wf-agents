//! A pause holds back only what would begin a round: what a round hands in is taken, the calls it asks for are made,
//! the owner is asked at a limit and a stale run is abandoned, while no round begins until the work is let on.

use super::*;

#[test]
fn a_return_handed_in_during_a_pause_finishes_the_run_and_the_next_cells_round_waits_for_the_work_to_be_let_on() {
    let (mut w, write, _) = W::requested();
    assert_eq!(w.on(Event::Paused), vec![]);
    assert_eq!(w.returns(write, hands_in(&[("text", None)])), vec![]);
    let text = w.last_finished();
    assert_eq!(
        w.work.records.finished.iter().map(|f| f.run).collect::<Vec<_>>(),
        vec![write]
    );
    let effects = w.on(Event::Resumed);
    let judge = w.work.records.started.last().expect("a run").run;
    let given = task(
        vec![Cause::NoRecord],
        &[("text", Some(text)), ("opinions", None), ("verdict", None)],
    );
    assert_eq!(effects, vec![round_effect(judge, "judge", 1, given)]);
}

#[test]
fn a_judge_whose_round_ends_during_a_pause_finishes_and_once_let_on_the_writer_runs_for_both_changes() {
    let (mut w, write, r1) = W::requested();
    let judge = round(&w.returns(write, hands_in(&[("text", None)]))).0;
    let text = w.last_finished();
    w.on(Event::Paused);
    let (effects, r2) = w.person_writes("request");
    assert_eq!(effects, vec![]);
    assert_eq!(w.returns(judge, hands_in(&[("opinions", None)])), vec![]);
    let opinions = w.last_finished();
    assert!(w.work.records.finished.iter().any(|f| f.run == judge));
    let effects = w.on(Event::Resumed);
    let again = w.work.records.started.last().expect("a run").run;
    let given = task(
        vec![changed("request", Some(r1), false), changed("opinions", None, true)],
        &[
            ("request", Some(r2)),
            ("opinions", Some(opinions)),
            ("main", None),
            ("text", Some(text)),
        ],
    );
    assert_eq!(effects, vec![round_effect(again, "write", 1, given)]);
}

#[test]
fn calls_asked_for_during_a_pause_are_made_and_the_round_given_their_results_waits_for_the_work_to_be_let_on() {
    let (mut w, write, _) = W::requested();
    w.on(Event::Paused);
    let calls = calls_of(&w.returns(write, Came::Asks(vec![tool("a")])));
    assert_eq!(
        calls.iter().map(|(_, r, what)| (*r, what.clone())).collect::<Vec<_>>(),
        vec![(write, Call::Asked(tool("a")))]
    );
    assert_eq!(w.done(calls[0].0), vec![]);
    assert_eq!(
        w.on(Event::Resumed),
        vec![round_effect(write, "write", 2, Given::Results(vec![calls[0].0]))]
    );
}

#[test]
fn a_return_sent_back_during_a_pause_is_given_back_once_the_work_is_let_on() {
    let (mut w, write, _) = W::requested();
    w.on(Event::Paused);
    assert_eq!(w.returns(write, wanting()), vec![]);
    assert_eq!(
        w.on(Event::Resumed),
        vec![round_effect(write, "write", 2, Given::SentBack(wanting_unmet()))]
    );
}

#[test]
fn the_owner_is_asked_at_a_limit_during_a_pause() {
    let (mut w, write, _) = W::requested();
    w.returns(write, wanting());
    w.returns(write, wanting());
    w.on(Event::Paused);
    let calls = calls_of(&w.returns(write, wanting()));
    assert_eq!(
        calls.iter().map(|(_, r, what)| (*r, what.clone())).collect::<Vec<_>>(),
        vec![(write, Call::AtLimit(Limit::Returns { sent_back: 2 }))]
    );
}

#[test]
fn a_stale_run_is_abandoned_during_a_pause_and_its_cell_runs_again_once_the_work_is_let_on() {
    let (mut w, write, _) = W::requested();
    let call = calls_of(&w.returns(write, Came::Asks(vec![question(Who::Owner)])))[0].0;
    w.on(Event::Paused);
    let (effects, request) = w.person_writes("request");
    assert_eq!(effects, vec![Effect::Withdraw { call }]);
    let effects = w.on(Event::Resumed);
    let again = w.work.records.started.last().expect("a run").run;
    let given = task(
        vec![Cause::NoRecord],
        &[
            ("request", Some(request)),
            ("opinions", None),
            ("main", None),
            ("text", None),
        ],
    );
    assert_eq!(effects, vec![round_effect(again, "write", 1, given)]);
}
