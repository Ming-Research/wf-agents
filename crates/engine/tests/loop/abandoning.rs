//! Runs that are abandoned: their calls withdrawn, their cell run again, nothing they handed in kept.

use super::*;

#[test]
fn a_run_at_rest_whose_start_changed_is_abandoned_its_calls_withdrawn_and_its_cell_runs_again_for_the_change() {
    let (mut w, write, first) = W::requested();
    w.returns(write, hands_in(&[("text", None)]));
    // The writer runs again for a new request, and rests at two calls.
    let (effects, _) = w.person_writes("request");
    assert_eq!(effects, vec![]);
    let judge = w.work.records.started[1].run;
    let (rerun, ..) = round(&w.returns(judge, hands_in(&[])));
    let calls: Vec<CallId> = calls_of(&w.returns(rerun, Came::asks(vec![tool("a"), question(Who::Owner)])))
        .iter()
        .map(|(c, ..)| *c)
        .collect();
    w.done(calls[0]);
    let (effects, third) = w.person_writes("request");
    let again = w.work.records.started.last().expect("a run").run;
    let text = w.work.records.finished[0].at;
    let start = started_with(
        vec![changed("request", Some(first), false)],
        &[
            ("request", Some(third)),
            ("opinions", None),
            ("main", None),
            ("text", Some(text)),
        ],
    );
    assert_eq!(
        effects,
        vec![
            Effect::Withdraw { call: calls[1] },
            round_effect(again, "write", 1, Given::Task)
        ]
    );
    assert_eq!(start_of(&w, again), start);
    assert!(w.work.records.abandoned.iter().any(|a| a.run == rerun));
    // The withdrawal is to be done until the call is said to have its result, which then changes nothing else.
    assert_eq!(pending(&w.flow, &w.work)[0], Effect::Withdraw { call: calls[1] });
    assert_eq!(w.done(calls[1]), vec![]);
    assert_eq!(
        pending(&w.flow, &w.work),
        vec![round_effect(again, "write", 1, given_of(&w, again))]
    );
}

#[test]
fn a_run_whose_last_round_ends_after_what_it_started_from_changed_is_abandoned_and_hands_in_nothing() {
    let (mut w, write, _) = W::requested();
    // A person rewrites the request while the round is under way.
    let (effects, request) = w.person_writes("request");
    assert_eq!(effects, vec![]);
    let effects = w.returns(write, hands_in(&[("text", None)]));
    assert_eq!(w.work.records.finished, vec![]);
    assert!(w.work.records.abandoned.iter().any(|a| a.run == write));
    assert!(!w.work.contents.keys().any(|(b, _)| b.as_str() == "text"));
    let again = w.work.records.started.last().expect("a run").run;
    let start = started_with(
        vec![Cause::NoRecord],
        &[
            ("request", Some(request)),
            ("opinions", None),
            ("main", None),
            ("text", None),
        ],
    );
    assert_eq!(effects, vec![round_effect(again, "write", 1, Given::Task)]);
    assert_eq!(start_of(&w, again), start);
}
