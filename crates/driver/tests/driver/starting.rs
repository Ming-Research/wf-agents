//! Opening the driver on works going on: it pauses everything itself before it looks at any work, and keeps that it
//! did, so that after a stop nothing begins a round the owner has not let on since they could see how things stand
//! (principle no-unseen-spending-loop). What was to be done is given all the same, a round under way at the stop among
//! it; the pause holds back exactly what the owner's pause of everything holds back, a run that answers a note having
//! its rounds; the owner lifts it as they lift their own. With no work going on, or under the owner's own pause, the
//! driver pauses nothing.

use decide::Holder;
use driver::NewItem;
use engine::Work;

use super::*;

/// A writer that reads a request and a note to it, both of which people write, and the merge.
fn noted() -> flow::Compiled {
    let flow = r#"
        name = "noted"
        order = ["write", "merge"]
        [limits]
        runs = 2
        returns = 2
        calls = 3
        [lines.main]
        [blocks.request]
        people = true
        [blocks.words]
        people = true
        note = true
        [blocks.text]
        [blocks.merged]
        shown = true
    "#;
    let write = r#"
        title = "Write"
        kind = "agent"
        effort = "high"
        line = "main"
        task = "Write the text the request asks for."
        inputs = ["request", "words"]
        outputs = ["text"]
    "#;
    let merge = r#"
        title = "Merge"
        kind = "engine"
        act = "merge"
        inputs = ["text"]
        outputs = ["merged"]
    "#;
    compiled(flow, &[("write", write), ("merge", merge)])
}

/// The driver's own pause of everything, made as it opened at the harness's clock.
fn the_starts(h: &H) -> Option<EverythingPaused> {
    Some(EverythingPaused {
        since: Time(h.clock),
        by: Pauser::Engine,
    })
}

/// How many rounds the work has begun.
fn begun(work: &Work) -> usize {
    work.records.rounds.len()
}

#[test]
fn opened_on_works_going_on_it_pauses_everything_and_begins_nothing_until_the_owner_lifts_it() {
    let mut h = H::new();
    // A work with a round under way; one resting at a tool's call; one never let on.
    let (_, under_way) = h.add_let_on(new_item("under way", &[], "judged"));
    let (_, at_call) = h.add_let_on(new_item("at a call", &[], "judged"));
    h.returns(at_call, Came::asks(vec![tool("search")]));
    let (_, never) = h.add(new_item("never let on", &[], "plain"));
    let pending = h.d().pending();

    let todos = h.reopen();
    assert_eq!(todos, pending, "opening gives what was to be done, and nothing more");
    assert_eq!(h.d().everything_paused(), the_starts(&h));
    assert!(h.ops().contains(&Op::PauseEverything));
    assert_eq!(h.d().held(never), Some(Holder::Pause));

    // The round under way ends and the call has its result: neither work begins its next round.
    assert_eq!(rounds(&h.hands_in(under_way, &[("text", None)])), []);
    assert_eq!(h.d().held(under_way), Some(Holder::Everything));
    let call = match &h.d().pending()[..] {
        [
            Todo {
                effect: Effect::Call { call, .. },
                ..
            },
        ] => *call,
        other => panic!("only the tool's call is to be done: {other:?}"),
    };
    assert_eq!(rounds(&h.answer(at_call, call)), []);
    assert_eq!(h.d().held(at_call), Some(Holder::Everything));

    // The owner lifts it as they lift their own pause: the works let on go on, and the one never let on does not.
    let todos = h.pause_everything(false);
    assert_eq!(
        rounds(&todos),
        [(under_way, "judge".to_string(), 1), (at_call, "write".to_string(), 2)]
    );
    assert_eq!(h.d().everything_paused(), None);
    assert_eq!(h.decision(never), Decision::Paused);
}

#[test]
fn opened_with_no_work_going_on_it_pauses_nothing() {
    let mut h = H::new();
    assert_eq!(h.reopen(), []);
    assert_eq!(h.d().everything_paused(), None, "on an empty store");
    // A work never let on, one the owner paused with its round under way, and one merged.
    h.add(new_item("never let on", &[], "plain"));
    let (_, paused) = h.add_let_on(new_item("paused", &[], "plain"));
    h.on(
        paused,
        Event::Paused {
            by: Pauser::Owner,
            why: None,
        },
    );
    let (_, merged) = h.add_let_on(new_item("merged", &[], "plain"));
    h.through(merged);
    let pending = h.d().pending();
    assert_eq!(h.reopen(), pending);
    assert_eq!(h.d().everything_paused(), None, "with no work going on");
    assert!(!h.ops().contains(&Op::PauseEverything));
}

#[test]
fn opened_again_under_its_own_pause_it_keeps_the_first_and_writes_nothing() {
    let mut h = H::new();
    h.add_let_on(new_item("going", &[], "plain"));
    h.reopen();
    let first = h.d().everything_paused();
    assert_eq!(first, the_starts(&h));
    h.now();
    h.reopen();
    assert_eq!(h.d().everything_paused(), first);
    assert!(!h.ops().contains(&Op::PauseEverything));
}

#[test]
fn the_owners_pause_set_over_the_drivers_own_is_the_owners() {
    let mut h = H::new();
    let (_, work) = h.add_let_on(new_item("going", &[], "plain"));
    h.reopen();
    assert_eq!(h.pause_everything(true), []);
    let theirs = h.d().everything_paused().expect("everything is paused");
    assert_eq!(theirs.by, Pauser::Owner);
    assert_eq!(theirs.since, Time(h.clock));
    // Opened again, the pause is the owner's still.
    h.reopen();
    assert_eq!(h.d().everything_paused(), Some(theirs));
    h.hands_in(work, &[("text", None)]);
    assert_eq!(
        h.pause_everything(false).len(),
        1,
        "the merge is carried out once it is lifted"
    );
}

#[test]
fn a_note_written_while_the_drivers_own_pause_holds_has_its_round() {
    let mut h = H::with(vec![noted()]);
    let (_, work) = h.add_let_on(NewItem {
        writes: vec![first_write("request", None)],
        ..new_item("noted", &[], "noted")
    });
    h.reopen();
    assert_eq!(h.d().everything_paused(), the_starts(&h));
    // The writer's round ends; the merge waits for the pause to be lifted.
    assert_eq!(rounds(&h.hands_in(work, &[("text", None)])), []);
    let before = begun(h.d().work(work).expect("the work").work);
    // A person's words to the writer are answered under the pause.
    let todos = h.on(work, wrote("words", Outsider::Person, None));
    assert_eq!(rounds(&todos), [(work, "write".to_string(), 1)]);
    assert_eq!(begun(h.d().work(work).expect("the work").work), before + 1);
}
