//! A cancel: nothing of the work goes on. Each open run is abandoned, run by run in the order they started: its round
//! under way is stopped and the calls it rests at are withdrawn, each of which is to be done until the driver says it
//! was.

use super::*;

/// The runs recorded as abandoned, in order.
fn abandoned(w: &W) -> Vec<RunId> {
    w.work.records.abandoned.iter().map(|a| a.run).collect()
}

#[test]
fn a_cancel_stops_the_round_under_way_and_abandons_its_run() {
    let (mut w, write, _) = W::requested();
    let stop = vec![Effect::Stop { run: write }];
    assert_eq!(w.on(Event::Cancelled { why: None }), stop);
    assert_eq!(abandoned(&w), vec![write]);
    assert_eq!(pending(&w.flow, &w.work), stop);
}

#[test]
fn a_cancel_withdraws_the_calls_a_run_rests_at_in_the_order_they_were_made() {
    let (mut w, write, _) = W::requested();
    let calls = calls_of(&w.returns(write, Came::Asks(vec![tool("a"), question(Who::Owner), tool("b")])));
    w.done(calls[0].0);
    let withdrawn = vec![
        Effect::Withdraw { call: calls[1].0 },
        Effect::Withdraw { call: calls[2].0 },
    ];
    assert_eq!(w.on(Event::Cancelled { why: None }), withdrawn);
    assert_eq!(abandoned(&w), vec![write]);
    assert_eq!(pending(&w.flow, &w.work), withdrawn);
}

/// The judge rests at a question for the owner, and the writer, before it, runs again for a new request.
fn two_open() -> (W, RunId, CallId, RunId) {
    let (mut w, write, _) = W::requested();
    let judge = round(&w.returns(write, hands_in(&[("text", None)]))).0;
    let call = calls_of(&w.returns(judge, Came::Asks(vec![question(Who::Owner)])))[0].0;
    let (effects, _) = w.person_writes("request");
    let again = round(&effects).0;
    (w, judge, call, again)
}

#[test]
fn a_cancel_ends_every_open_run_in_the_order_the_runs_started() {
    let (mut w, judge, call, again) = two_open();
    let undo = vec![Effect::Withdraw { call }, Effect::Stop { run: again }];
    assert_eq!(w.on(Event::Cancelled { why: None }), undo);
    assert_eq!(abandoned(&w), vec![judge, again]);
    assert_eq!(pending(&w.flow, &w.work), undo);
}

#[test]
fn a_stop_and_a_withdrawal_are_pending_until_the_round_has_ended_and_the_call_has_its_result() {
    let (mut w, _, call, again) = two_open();
    w.on(Event::Cancelled { why: None });
    assert_eq!(w.returns(again, hands_in(&[("text", None)])), vec![]);
    assert_eq!(pending(&w.flow, &w.work), vec![Effect::Withdraw { call }]);
    // Nothing of it is under way then, and what it holds is let go.
    let release = vec![Effect::Release {
        end: decide::End::Cancelled,
    }];
    assert_eq!(w.done(call), release);
    assert_eq!(pending(&w.flow, &w.work), release);
}

#[test]
fn what_a_stopped_round_returns_and_a_withdrawn_calls_result_change_nothing_but_saying_they_were_done() {
    let (mut w, _, call, again) = two_open();
    w.on(Event::Cancelled { why: None });
    let before = w.work.clone();
    w.returns(again, hands_in(&[("text", None)]));
    w.done(call);
    let mut after = w.work.clone();
    // What changed is the round's end and the call's result, and nothing else.
    assert_eq!(after.records.returned.pop().map(|r| r.run), Some(again));
    assert_eq!(after.records.done.pop().map(|d| d.call), Some(call));
    assert!(after.results.remove(&call).is_some());
    // And when each was taken, and what the stopped round took.
    assert_eq!(after.moments.len(), before.moments.len() + 2);
    after.moments.truncate(before.moments.len());
    assert!(after.spent.remove(&(again, 1)).is_some());
    assert_eq!(after, before);
    assert_eq!(w.decision(), decide::Decision::Ended(decide::End::Cancelled));
}

#[test]
fn a_second_cancel_gives_nothing_and_changes_nothing() {
    let (mut w, ..) = two_open();
    w.on(Event::Cancelled { why: None });
    let before = w.work.clone();
    assert_eq!(w.on(Event::Cancelled { why: None }), vec![]);
    assert_eq!(w.work, before);
}

#[test]
fn a_cancel_withdraws_the_owners_call_at_a_limit() {
    let (mut w, write, _) = W::requested();
    w.returns(write, wanting());
    w.returns(write, wanting());
    let call = calls_of(&w.returns(write, wanting()))[0].0;
    assert_eq!(w.on(Event::Cancelled { why: None }), vec![Effect::Withdraw { call }]);
}
