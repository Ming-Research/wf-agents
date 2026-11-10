//! Calls: one made for each thing asked, the round given the results only once all have them.

use super::*;

#[test]
fn each_thing_asked_for_is_one_call_for_a_tool_with_nobody_and_for_a_person_with_that_person() {
    let (mut w, write, _) = W::requested();
    let effects = w.returns(write, Came::Asks(vec![tool("search"), question(Who::Architect)]));
    let calls = calls_of(&effects);
    assert_eq!(
        calls
            .iter()
            .map(|(_, run, what)| (*run, what.clone()))
            .collect::<Vec<_>>(),
        vec![
            (write, Call::Asked(tool("search"))),
            (write, Call::Asked(question(Who::Architect)))
        ]
    );
    let on: Vec<_> = (w.work.records.called.iter())
        .map(|c| (c.call, c.run, c.on.clone()))
        .collect();
    assert_eq!(
        on,
        vec![(calls[0].0, write, None), (calls[1].0, write, Some(Who::Architect))]
    );
    assert_eq!(w.work.calls.get(&calls[0].0), Some(&Call::Asked(tool("search"))));
}

#[test]
fn the_round_after_calls_is_given_their_results_only_once_the_last_has_its_result() {
    let (mut w, write, _) = W::requested();
    let effects = w.returns(write, Came::Asks(vec![tool("search"), question(Who::Architect)]));
    let calls: Vec<CallId> = calls_of(&effects).iter().map(|(c, ..)| *c).collect();
    assert_eq!(w.done(calls[0]), vec![]);
    let effects = w.done(calls[1]);
    assert_eq!(
        effects,
        vec![round_effect(write, "write", 2, Given::Results(calls.clone()))]
    );
}

#[test]
fn a_calls_result_is_kept() {
    let (mut w, write, _) = W::requested();
    let effects = w.returns(write, Came::Asks(vec![tool("search")]));
    let call = calls_of(&effects)[0].0;
    w.on(Event::Done {
        call,
        result: "found it".to_string(),
    });
    assert_eq!(w.work.results, BTreeMap::from([(call, "found it".to_string())]));
    assert_eq!(
        w.work.records.done.iter().map(|d| d.call).collect::<Vec<_>>(),
        vec![call]
    );
}
