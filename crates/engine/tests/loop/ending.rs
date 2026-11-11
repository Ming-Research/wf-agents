//! A work that has ended is given nothing more, whatever happens to it afterwards, but to let go of what it holds
//! outside the engine: once, as it ends with nothing of it under way, and again only where it is not said to be done.

use super::*;

const MERGED: Effect = Effect::Release {
    end: decide::End::Merged,
};
const CANCELLED: Effect = Effect::Release {
    end: decide::End::Cancelled,
};

fn merged() -> W {
    let (mut w, write, _) = W::requested();
    w.lets_merge();
    let judge = round(&w.returns(write, hands_in(&[("text", None)]))).0;
    let merge = round(&w.returns(judge, hands_in(&[("verdict", Some("yes"))]))).0;
    assert_eq!(w.returns(merge, hands_in(&[("merged", None)])), vec![MERGED]);
    w
}

#[test]
fn as_the_merge_finishes_what_the_work_holds_is_let_go_and_nothing_more_is_given() {
    let mut w = merged();
    assert_eq!(pending(&w.flow, &w.work), vec![MERGED]);
    assert_eq!(w.on(wrote("request", Outsider::Person, true, None)), vec![]);
    assert_eq!(w.on(wrote("opinions", Outsider::Person, true, None)), vec![]);
    assert_eq!(w.on(Event::LetGo { cell: "write".into() }), vec![]);
    assert_eq!(w.on(Event::Look), vec![]);
    assert_eq!(
        pending(&w.flow, &w.work),
        vec![MERGED],
        "it is to be done until it is said done"
    );
    assert_eq!(w.on(Event::Released), vec![]);
    assert!(w.work.records.released.is_some());
    assert_eq!(pending(&w.flow, &w.work), vec![]);
}

#[test]
fn letting_go_said_twice_is_kept_once() {
    let mut w = merged();
    w.on(Event::Released);
    let before = w.work.clone();
    assert_eq!(w.on(Event::Released), vec![]);
    assert_eq!(w.work, before);
}

#[test]
fn a_work_that_has_not_ended_has_nothing_to_let_go() {
    let (mut w, ..) = W::requested();
    let before = w.work.clone();
    assert_eq!(
        handle(&w.flow, &mut w.work, Time(99), Event::Released),
        Err(Refused::NotToRelease)
    );
    assert_eq!(w.work, before);
}

#[test]
fn what_could_not_be_let_go_pauses_the_ended_work_saying_why_and_letting_it_on_tries_again() {
    let mut w = merged();
    let why = "the worktree is locked".to_string();
    assert_eq!(w.on(Event::NotReleased { why: why.clone() }), vec![]);
    let pause = decide::pause(&w.work.records).expect("the work is paused");
    assert_eq!((pause.by, pause.why.as_deref()), (Pauser::Engine, Some(why.as_str())));
    assert_eq!(pending(&w.flow, &w.work), vec![], "it waits for the owner");
    assert_eq!(w.on(Event::Look), vec![]);
    assert_eq!(w.on(Event::Resumed), vec![MERGED], "let on, it is tried again");
    assert_eq!(w.on(Event::Released), vec![]);
    assert!(w.work.records.released.is_some());
}

#[test]
fn a_work_paused_as_it_was_cancelled_lets_go_all_the_same() {
    let (mut w, write, _) = W::requested();
    w.on(Event::Paused {
        by: Pauser::Owner,
        why: None,
    });
    w.on(Event::Cancelled { why: None });
    assert_eq!(w.returns(write, hands_in(&[("text", None)])), vec![CANCELLED]);
}

#[test]
fn a_cancel_after_the_merge_gives_nothing_and_the_work_has_merged() {
    let mut w = merged();
    assert_eq!(w.on(Event::Cancelled { why: None }), vec![]);
    assert_eq!(w.decision(), decide::Decision::Ended(decide::End::Merged));
}

#[test]
fn after_a_cancel_nothing_more_is_given_but_letting_go_once_the_stopped_round_ended() {
    let (mut w, write, _) = W::requested();
    // The round under way may be working in the worktree: nothing is let go before it has ended.
    assert_eq!(w.on(Event::Cancelled { why: None }), vec![Effect::Stop { run: write }]);
    assert_eq!(w.returns(write, hands_in(&[("text", None)])), vec![CANCELLED]);
    assert_eq!(w.work.records.finished, vec![]);
    assert_eq!(w.on(wrote("request", Outsider::Person, true, None)), vec![]);
    assert_eq!(w.on(Event::Look), vec![]);
    assert_eq!(pending(&w.flow, &w.work), vec![CANCELLED]);
}

#[test]
fn a_cancelled_work_lets_go_once_its_calls_are_withdrawn() {
    let (mut w, write, _) = W::requested();
    let call = calls_of(&w.returns(write, Came::Asks(vec![tool("a")])))[0].0;
    // A tool may be working in the worktree until its call has its result.
    assert_eq!(w.on(Event::Cancelled { why: None }), vec![Effect::Withdraw { call }]);
    assert_eq!(w.done(call), vec![CANCELLED]);
}
