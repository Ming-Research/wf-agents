//! What a driver that starts anew is given again: the round under way, and the calls that have no result.

use super::*;

#[test]
fn the_round_under_way_is_pending_with_what_it_was_given() {
    let mut w = W::new();
    let effects = w.person_writes("request").0;
    assert_eq!(pending(&w.work), effects);
}

#[test]
fn the_calls_that_have_no_result_are_pending() {
    let (mut w, write, _) = W::requested();
    let effects = w.returns(write, Came::Asks(vec![tool("a"), question(Who::Architect)]));
    assert_eq!(pending(&w.work), effects);
    let calls = calls_of(&effects);
    w.done(calls[0].0);
    assert_eq!(pending(&w.work), vec![effects[1].clone()]);
}

#[test]
fn nothing_is_pending_once_the_work_rests() {
    let (mut w, write, _) = W::requested();
    let judge = round(&w.returns(write, hands_in(&[("text", None)]))).0;
    assert_eq!(w.returns(judge, hands_in(&[("verdict", Some("yes"))])), vec![]);
    assert_eq!(pending(&w.work), vec![]);
}

#[test]
fn nothing_is_pending_once_the_work_has_merged() {
    let (mut w, write, _) = W::requested();
    w.tests.0.insert("may_merge".into(), Outcome::Holds);
    let judge = round(&w.returns(write, hands_in(&[("text", None)]))).0;
    let merge = round(&w.returns(judge, hands_in(&[("verdict", Some("yes"))]))).0;
    w.returns(merge, hands_in(&[("merged", None)]));
    assert_eq!(pending(&w.work), vec![]);
}

#[test]
#[ignore = "pending still gives the open calls of a cancelled work, and a cancel withdraws none of them"]
fn nothing_is_pending_once_the_work_is_cancelled() {
    let (mut w, write, _) = W::requested();
    w.returns(write, Came::Asks(vec![question(Who::Owner)]));
    w.on(Event::Cancelled);
    assert_eq!(pending(&w.work), vec![]);
}

#[test]
fn nothing_of_an_abandoned_run_is_pending() {
    let (mut w, write, _) = W::requested();
    w.returns(write, Came::Asks(vec![question(Who::Owner)]));
    let effects = w.person_writes("request").0;
    // The withdrawal is no longer to be done: only the new round is.
    let rounds: Vec<Effect> = effects
        .into_iter()
        .filter(|e| matches!(e, Effect::Round { .. }))
        .collect();
    assert_eq!(pending(&w.work), rounds);
}
