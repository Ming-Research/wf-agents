//! Kept before given: after every call the store holds of each work what the driver holds, and what the driver gives
//! to be done the store already says is to be done. The harness checks both after every call; the history here leaves
//! an entry of every kind, so that each is checked.

use super::*;

/// The call given, of those given.
fn call_of(todos: &[Todo]) -> CallId {
    (todos.iter())
        .find_map(|todo| match &todo.effect {
            Effect::Call { call, .. } => Some(*call),
            _ => None,
        })
        .expect("a call was given")
}

#[test]
fn after_every_call_the_store_holds_what_the_driver_holds() {
    let mut h = H::new();
    let a = h.item("a", &[]);
    let b = h.item("b", &[a]);
    let (wa, _) = h.work(a, "judged", false);
    let (wb, _) = h.work(b, "judged", true);
    h.on(wb, wrote("request", Outsider::Person, None));
    h.on(wa, wrote("request", Outsider::Person, None));
    // Sent back twice, then the owner is called at the limit of returns and lets the run go on.
    h.hands_in(wa, &[("text", None)]);
    for _ in 0..3 {
        h.hands_in(wa, &[("verdict", Some("yes"))]);
    }
    let todos = h.returns(wa, Came::Asks(vec![]));
    assert_eq!(todos, vec![], "asking for nothing was not sent back at the limit");
    let limit = call_of(&h.d().pending());
    h.answer(wa, limit);
    let todos = h.returns(wa, Came::Asks(vec![tool("a"), question(Who::Architect)]));
    let first = call_of(&todos);
    for todo in todos {
        if let Effect::Call { call, .. } = todo.effect {
            h.answer(wa, call);
        }
    }
    assert!(first.0 > 0);
    h.hands_in(wa, &[("text", None)]);
    // The judge's round is under way when the work is paused; what it asks is made, its result taken.
    h.on(wa, Event::Paused);
    let todos = h.returns(wa, Came::Asks(vec![tool("b")]));
    h.answer(wa, call_of(&todos));
    h.on(wa, Event::Resumed);
    // The judge rests at a question while a person writes an opinion for the writer: its run is abandoned and the
    // question withdrawn.
    let todos = h.returns(wa, Came::Asks(vec![question(Who::Owner)]));
    let asked = call_of(&todos);
    let todos = h.on(wa, wrote("opinions", Outsider::Person, None));
    assert!(
        todos.contains(&Todo {
            work: wa,
            effect: Effect::Withdraw { call: asked },
        }),
        "the question was not withdrawn: {todos:?}"
    );
    h.answer(wa, asked);
    h.hands_in(wa, &[("text", None)]);
    h.on(wa, Event::LetGo { cell: "write".into() });
    h.hands_in(wa, &[("opinions", None), ("verdict", Some("no"))]);
    // The writer runs again for its opinions; the work is cancelled with its round under way.
    let (run, cell, ..) = h.round(wa);
    assert_eq!(cell, "write");
    let todos = h.on(wa, Event::Cancelled);
    assert_eq!(
        todos,
        vec![Todo {
            work: wa,
            effect: Effect::Stop { run },
        }]
    );
    let spent = h.spent();
    h.on(
        wa,
        Event::Returned {
            run,
            came: hands_in(&[("text", None)]),
            spent,
        },
    );
    // The other work was let on and waits for the cancelled one, which will never merge.
    h.on(wb, Event::Resumed);
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
