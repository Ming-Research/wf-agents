//! A work that has ended is given nothing more, whatever happens to it afterwards.

use super::*;

fn merged() -> W {
    let (mut w, write, _) = W::requested();
    w.tests.0.insert("may_merge".into(), Outcome::Holds);
    let judge = round(&w.returns(write, hands_in(&[("text", None)]))).0;
    let merge = round(&w.returns(judge, hands_in(&[("verdict", Some("yes"))]))).0;
    assert_eq!(w.returns(merge, hands_in(&[("merged", None)])), vec![]);
    w
}

#[test]
fn after_the_merge_finishes_nothing_more_is_given() {
    let mut w = merged();
    assert_eq!(w.on(wrote("request", Outsider::Person, true, None)), vec![]);
    assert_eq!(w.on(wrote("opinions", Outsider::Person, true, None)), vec![]);
    assert_eq!(w.on(Event::LetGo { cell: "write".into() }), vec![]);
    assert_eq!(w.on(Event::Look), vec![]);
    assert_eq!(pending(&w.work), vec![]);
}

#[test]
fn a_cancel_after_the_merge_gives_nothing_and_the_work_has_merged() {
    let mut w = merged();
    assert_eq!(w.on(Event::Cancelled), vec![]);
    assert_eq!(
        decide::decide(&w.flow.flow, &w.work.records, &w.tests),
        decide::Decision::Ended(decide::End::Merged)
    );
}

#[test]
fn after_a_cancel_nothing_more_is_given() {
    let (mut w, write, _) = W::requested();
    w.on(Event::Cancelled);
    assert_eq!(w.returns(write, hands_in(&[("text", None)])), vec![]);
    assert_eq!(w.work.records.finished, vec![]);
    assert_eq!(w.on(wrote("request", Outsider::Person, true, None)), vec![]);
    assert_eq!(w.on(Event::Look), vec![]);
}
