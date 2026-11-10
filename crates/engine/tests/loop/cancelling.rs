//! A cancel: nothing of the work goes on. Its round under way is stopped, the calls it rests at are withdrawn, and
//! its runs are abandoned, run by run in the order they started.

use super::*;

/// The runs recorded as abandoned, in order.
fn abandoned(w: &W) -> Vec<RunId> {
    w.work.records.abandoned.iter().map(|a| a.run).collect()
}

#[test]
fn a_cancel_stops_the_round_under_way_and_abandons_its_run() {
    let (mut w, write, _) = W::requested();
    assert_eq!(w.on(Event::Cancelled), vec![Effect::Stop { run: write }]);
    assert_eq!(abandoned(&w), vec![write]);
    assert_eq!(pending(&w.work), vec![]);
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
    assert_eq!(w.on(Event::Cancelled), withdrawn);
    assert_eq!(abandoned(&w), vec![write]);
    assert_eq!(pending(&w.work), vec![]);
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
    assert_eq!(
        w.on(Event::Cancelled),
        vec![Effect::Withdraw { call }, Effect::Stop { run: again }]
    );
    assert_eq!(abandoned(&w), vec![judge, again]);
    assert_eq!(pending(&w.work), vec![]);
}

#[test]
fn after_a_cancel_the_stopped_rounds_return_and_the_withdrawn_calls_result_are_refused() {
    let (mut w, _, call, again) = two_open();
    w.on(Event::Cancelled);
    let late = Event::Returned {
        run: again,
        came: hands_in(&[("text", None)]),
    };
    assert_eq!(w.refused(late), Refused::NoRoundUnderWay(again));
    let late = Event::Done {
        call,
        result: "too late".into(),
    };
    assert_eq!(w.refused(late), Refused::Withdrawn(call));
}

#[test]
fn a_second_cancel_gives_nothing_and_changes_nothing() {
    let (mut w, ..) = two_open();
    w.on(Event::Cancelled);
    let before = w.work.clone();
    assert_eq!(w.on(Event::Cancelled), vec![]);
    assert_eq!(w.work, before);
}
