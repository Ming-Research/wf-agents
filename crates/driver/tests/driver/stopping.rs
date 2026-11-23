//! A stop of the engine, as for installing a new build: from when it is asked for, no round of any work begins, a round
//! under way goes on to its end, which is kept, and the engine may end once nothing is under way; asked at once, it
//! may end at once. It is kept, and the next start ends it, keeping that too, so that the stretch it stood is known.

use store::{StopAct, StopRecord, Storage};

use super::*;

/// The rounds given among these.
fn begun(todos: &[Todo]) -> Vec<(WorkNumber, String, usize)> {
    (todos.iter())
        .filter_map(|t| match &t.effect {
            Effect::Round { cell, number, .. } => Some((t.work, cell.to_string(), *number)),
            _ => None,
        })
        .collect()
}

// A stop waits for the round under way: the work's next round, its merge, does not begin as the round ends, nor does
// the round of a work let on meanwhile, nor a run a person's write starts; once the round under way has ended and its
// end is kept, the engine may end, and not before. Fails where a round begins under the stop, or where the engine may
// end with a round under way or not once none is. A few milliseconds; no other test holds rounds under a stop.
#[test]
fn a_stop_waits_for_the_round_under_way_and_begins_no_other() {
    let mut h = H::new();
    let (_, a) = h.add(new_item("a", &[], "plain"));
    let (_, b) = h.add(new_item("b", &[], "plain"));
    assert_eq!(begun(&h.let_on(a)), vec![(a, "write".to_string(), 1)]);
    assert!(!h.d().may_end(), "no stop stands");
    let now = h.now();
    h.dm().ask_to_stop(now, false).expect("the stop is kept");
    assert_eq!(h.d().stopping().map(|s| (s.since, s.at_once)), Some((now, false)));
    assert!(!h.d().may_end(), "a round is under way");

    assert_eq!(begun(&h.let_on(b)), vec![], "a work let on begins no round");
    assert!(matches!(h.decision(b), Decision::Run { .. }), "{:?}", h.decision(b));
    let todos = h.hands_in(a, &[("text", None)]);
    assert_eq!(begun(&todos), vec![], "the merge does not begin");
    assert!(h.d().may_end(), "nothing is under way, and the round's end is kept");
    let records = &h.d().work(a).unwrap().work.records;
    assert_eq!(records.rounds.len(), records.returned.len());
    let wrote = Event::Wrote {
        block: "request".into(),
        by: Outsider::Person,
        content: "Write it again.".to_string(),
        matters: true,
        value: None,
    };
    assert_eq!(begun(&h.on(a, wrote)), vec![], "a person's write begins no run's round");
    assert!(h.d().may_end());
}

// A stop asked at once lets the engine end with a round under way, which it cuts off; one asked once what is under
// way has ended, then at once, stands at once from then, since the first was asked, and a stop asked again changes
// nothing. Each is kept as it was asked. Fails where a stop at once waits, or where one asked again is kept again.
// Milliseconds.
#[test]
fn a_stop_at_once_lets_the_engine_end_with_a_round_under_way() {
    let mut h = H::new();
    let (_, a) = h.add(new_item("a", &[], "plain"));
    h.let_on(a);
    let asked = h.now();
    h.dm().ask_to_stop(asked, false).expect("kept");
    let again = Time(h.clock);
    h.dm().ask_to_stop(again, false).expect("changes nothing");
    assert!(!h.d().may_end());
    let at_once = h.now();
    h.dm().ask_to_stop(at_once, true).expect("kept");
    let again = Time(h.clock);
    h.dm().ask_to_stop(again, true).expect("changes nothing");
    assert_eq!(h.d().stopping().map(|s| (s.since, s.at_once)), Some((asked, true)));
    assert!(h.d().may_end(), "a round is under way, and the stop is at once");
    let kept = h.reader.stops().expect("read");
    let record = |what, at| StopRecord { what, at };
    assert_eq!(kept, [record(StopAct::Asked, asked), record(StopAct::Now, at_once)]);
}

// The engine started again after a stop keeps that the stop is over, at its start, before it looks at any work: no
// stop stands then, every work goes on as any start lets it, and the stretch the stop stood ends there. A start with
// no stop standing keeps nothing. Fails where a stop outlasts the start, or where a start keeps an end of no stop.
// Milliseconds.
#[test]
fn the_start_after_a_stop_ends_it_and_keeps_where_it_ended() {
    let mut h = H::new();
    let (_, a) = h.add(new_item("a", &[], "plain"));
    h.let_on(a);
    let asked = h.now();
    h.dm().ask_to_stop(asked, false).expect("kept");
    h.hands_in(a, &[("text", None)]);
    assert!(h.d().may_end());
    let todos = h.restart();
    assert_eq!(h.d().stopping(), None);
    let started = Time(h.clock);
    let kept = h.reader.stops().expect("read");
    assert_eq!(
        kept.last(),
        Some(&StopRecord {
            what: StopAct::Over,
            at: started
        })
    );
    assert_eq!(
        h.d().stop_spans(),
        [driver::StopSpan {
            from: asked,
            until: Some(started)
        }]
    );
    assert_eq!(
        begun(&todos),
        vec![(a, "merge".to_string(), 1)],
        "the merge begins once let on"
    );
    h.reopen();
    assert_eq!(
        h.reader.stops().expect("read").len(),
        2,
        "a start after none keeps nothing"
    );
}
