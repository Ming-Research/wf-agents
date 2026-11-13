//! What a driver that starts anew is given again: the round under way, and the calls that have no result and are still
//! wanted.

use super::*;

#[test]
fn the_round_under_way_is_pending_with_what_it_was_given() {
    let mut w = W::new();
    let effects = w.person_writes("request").0;
    assert_eq!(pending(&w.flow, &w.work), effects);
}

#[test]
fn the_calls_that_have_no_result_are_pending() {
    let (mut w, write, _) = W::requested();
    let effects = w.returns(write, Came::asks(vec![tool("a"), question(Who::Architect)]));
    assert_eq!(pending(&w.flow, &w.work), effects);
    let calls = calls_of(&effects);
    w.done(calls[0].0);
    assert_eq!(pending(&w.flow, &w.work), vec![effects[1].clone()]);
}

#[test]
fn nothing_is_pending_once_the_work_rests() {
    let (mut w, write, _) = W::requested();
    let judge = round(&w.returns(write, hands_in(&[("text", None)]))).0;
    assert_eq!(w.returns(judge, hands_in(&[("verdict", Some("yes"))])), vec![]);
    assert_eq!(pending(&w.flow, &w.work), vec![]);
}

#[test]
fn nothing_is_pending_once_the_work_has_merged_and_let_go_of_what_it_held() {
    let (mut w, write, _) = W::requested();
    w.lets_merge();
    let judge = round(&w.returns(write, hands_in(&[("text", None)]))).0;
    let merge = round(&w.returns(judge, hands_in(&[("verdict", Some("yes"))]))).0;
    w.returns(merge, hands_in(&[("merged", None)]));
    let release = Effect::Release {
        end: decide::End::Merged,
    };
    assert_eq!(pending(&w.flow, &w.work), vec![release]);
    w.on(Event::Released);
    assert_eq!(pending(&w.flow, &w.work), vec![]);
}

#[test]
fn nothing_is_pending_once_the_work_is_cancelled_what_was_to_be_undone_is_done_and_what_it_held_let_go() {
    let (mut w, write, _) = W::requested();
    let call = calls_of(&w.returns(write, Came::asks(vec![question(Who::Owner)])))[0].0;
    w.on(Event::Cancelled { why: None });
    assert_eq!(pending(&w.flow, &w.work), vec![Effect::Withdraw { call }]);
    w.done(call);
    let release = Effect::Release {
        end: decide::End::Cancelled,
    };
    assert_eq!(pending(&w.flow, &w.work), vec![release]);
    w.on(Event::Released);
    assert_eq!(pending(&w.flow, &w.work), vec![]);
}

#[test]
fn the_calls_a_finished_run_asked_for_stay_pending_while_the_work_goes_on() {
    let (mut w, write, _) = W::requested();
    let effects = w.returns(write, Came::asks(vec![question(Who::Owner)]));
    assert!(w.work.records.finished.iter().any(|f| f.run == write));
    // A new request does not enter the writer while it waits for the answer.
    assert_eq!(w.person_writes("request").0, vec![]);
    assert_eq!(pending(&w.flow, &w.work), effects);
}
