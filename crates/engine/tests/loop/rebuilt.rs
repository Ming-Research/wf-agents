//! The engine holds nothing between two things that happen: a work rebuilt from its parts goes on as the original.

use super::*;

/// A work built anew from the parts of this one.
fn rebuilt(work: &Work) -> Work {
    Work {
        records: work.records.clone(),
        contents: work.contents.clone(),
        given: work.given.clone(),
        back: work.back.clone(),
        calls: work.calls.clone(),
        results: work.results.clone(),
        spent: work.spent.clone(),
        moments: work.moments.clone(),
    }
}

#[test]
fn a_work_rebuilt_from_its_parts_goes_on_exactly_as_the_original() {
    let (mut w, write, _) = W::requested();
    let judge = round(&w.returns(write, hands_in(&[("text", None)]))).0;
    let calls = calls_of(&w.returns(judge, Came::Asks(vec![tool("a"), question(Who::Owner)])));
    w.done(calls[0].0);
    let mut copy = W {
        flow: judged(),
        work: rebuilt(&w.work),
        clock: w.clock,
    };
    assert_eq!(copy.work, w.work);
    let events = [
        Event::Done {
            call: calls[1].0,
            result: "yes".into(),
        },
        Event::Returned {
            run: judge,
            came: hands_in(&[("opinions", None)]),
            spent: spent(7),
        },
        wrote("request", Outsider::Person, true, None),
        Event::Cancelled,
    ];
    for event in events {
        assert_eq!(copy.on(event.clone()), w.on(event));
        assert_eq!(copy.clock, w.clock);
        assert_eq!(copy.work, w.work);
        assert_eq!(pending(&copy.flow, &copy.work), pending(&w.flow, &w.work));
    }
}
