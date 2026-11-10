//! Kept before given: after every call a store opened apart on the file holds of each work what the driver shows,
//! what the driver gives to be done that store already says is to be done, and the records it holds walk to the
//! decision the driver shows. The harness checks all of it after every call; the history here leaves an entry of every
//! kind, so that each is checked.

use super::*;

/// The calls given, of those given.
fn calls_of(todos: &[Todo]) -> Vec<CallId> {
    (todos.iter())
        .filter_map(|todo| match &todo.effect {
            Effect::Call { call, .. } => Some(*call),
            _ => None,
        })
        .collect()
}

#[test]
fn after_every_call_the_store_holds_what_the_driver_shows() {
    let mut h = H::new();
    let (a, wa) = h.add(new_item("a", &[], "judged"));
    let (_, wb) = h.add(new_item("b", &[a], "judged"));
    h.on(wb, wrote("request", Outsider::Person, None));
    h.on(wa, wrote("request", Outsider::Person, None));
    h.let_on(wa);
    // Sent back twice, then the owner is called at the limit of returns and lets the run go on: what it returned is
    // sent back once more.
    for number in [2, 3] {
        let todos = h.hands_in(wa, &[("verdict", Some("yes"))]);
        assert_eq!(rounds(&todos), [(wa, "write".to_string(), number)]);
    }
    let todos = h.hands_in(wa, &[("verdict", Some("yes"))]);
    let limit = calls_of(&todos);
    assert_eq!(limit.len(), 1, "the limit of returns was not reached: {todos:?}");
    let todos = h.answer(wa, limit[0]);
    assert_eq!(rounds(&todos), [(wa, "write".to_string(), 4)]);
    // A tool and a question for the architect.
    let todos = h.returns(wa, Came::Asks(vec![tool("a"), question(Who::Architect)]));
    let asked = calls_of(&todos);
    assert_eq!(asked.len(), 2);
    for call in asked {
        h.answer(wa, call);
    }
    let todos = h.hands_in(wa, &[("text", None)]);
    assert_eq!(rounds(&todos), [(wa, "judge".to_string(), 1)]);
    // The judge's round is under way when the work is paused; the tool it asks for is made and its result taken.
    h.on(
        wa,
        Event::Paused {
            by: Pauser::Owner,
            why: None,
        },
    );
    let todos = h.returns(wa, Came::Asks(vec![tool("b")]));
    h.answer(wa, calls_of(&todos)[0]);
    h.let_on(wa);
    // The judge rests at a question while a person writes the opinions it may write: its run is abandoned, the
    // question withdrawn, and the writer runs again for them.
    let todos = h.returns(wa, Came::Asks(vec![question(Who::Owner)]));
    let question = calls_of(&todos)[0];
    let todos = h.on(wa, wrote("opinions", Outsider::Person, None));
    assert!(
        todos.contains(&Todo {
            work: wa,
            effect: Effect::Withdraw { call: question },
        }),
        "the question was not withdrawn: {todos:?}"
    );
    h.on(
        wa,
        Event::Done {
            call: question,
            result: "withdrawn".into(),
        },
    );
    let todos = h.hands_in(wa, &[("text", None)]);
    assert_eq!(rounds(&todos), [(wa, "judge".to_string(), 1)]);
    h.on(wa, Event::LetGo { cell: "write".into() });
    // The judge writes opinions: the writer runs again for them, and the work is cancelled with its round under way.
    let todos = h.hands_in(wa, &[("opinions", None), ("verdict", Some("no"))]);
    assert_eq!(rounds(&todos), [(wa, "write".to_string(), 1)]);
    let (run, ..) = h.round(wa);
    let todos = h.on(wa, Event::Cancelled);
    assert_eq!(
        todos,
        vec![Todo {
            work: wa,
            effect: Effect::Stop { run },
        }]
    );
    let spent = h.spent();
    let came = hands_in(&[("text", None)]);
    h.on(wa, Event::Returned { run, came, spent });
    // The other work, let on, waits for the cancelled one, which never merges.
    assert_eq!(h.let_on(wb), vec![]);
    assert_eq!(h.decision(wb), waits_for_dependencies("write"));
    assert_eq!(h.d().pending(), vec![]);
    let kept = h.d().work(wa).expect("the work is there");
    let r = &kept.work.records;
    let kinds = [
        r.started.len(),
        r.rounds.len(),
        r.returned.len(),
        r.called.len(),
        r.done.len(),
        r.finished.len(),
        r.abandoned.len(),
        r.written.len(),
        r.let_go.len(),
        r.pauses.len(),
    ];
    assert!(kinds.iter().all(|n| *n > 0), "an entry of a kind is missing: {kinds:?}");
    assert!(r.cancelled.is_some());
}
