//! What a driver that starts anew is given again: the round under way, and the calls that have no result.

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
    let effects = w.returns(write, Came::Asks(vec![tool("a"), question(Who::Architect)]));
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
    let call = calls_of(&w.returns(write, Came::Asks(vec![question(Who::Owner)])))[0].0;
    w.on(Event::Cancelled);
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
fn of_an_abandoned_run_only_the_withdrawing_of_its_calls_is_pending_until_it_is_done() {
    let (mut w, write, _) = W::requested();
    let call = calls_of(&w.returns(write, Came::Asks(vec![question(Who::Owner)])))[0].0;
    let effects = w.person_writes("request").0;
    assert_eq!(pending(&w.flow, &w.work), effects);
    assert_eq!(effects[0], Effect::Withdraw { call });
    w.done(call);
    assert_eq!(pending(&w.flow, &w.work), effects[1..].to_vec());
}
