//! A stop changes nothing but that the driver, opened again on works going on, pauses everything until the owner lifts
//! it (property a-stop-changes-nothing; principle state-from-records): one history of six items, some depending on
//! others, is played to its end with no stop; then again once for every call it makes of the store, the store failing
//! at that call, and once more for every call, the process stopping right after it. Each time the driver is opened anew
//! on the same file, giving nothing that was not pending and beginning nothing; the owner lifts at once the pause it
//! made, what it said was not kept is told again, what it gives is answered, and the history goes on. Each ends with
//! the same works, each entry added at the same time, the same decisions, and the same rounds and calls given and
//! answered, each once, as the history with no stop; and the driver says it kept what it was told exactly when the
//! store holds it.
//!
//! The history: six items are added (one with a first write), five let on and one later; the work of one is cancelled
//! with its round under way; one rests at a question for the owner when a person writes what its run started from, so
//! that the question is withdrawn, and its writer, run again, hands in what it may not write; a person writes an
//! opinion for a writer while its round is under way. Every other step answers one of the things given to be done,
//! chosen by the step's number among them in a fixed order: a round hands in what its cell may write, or, in its
//! first round at some steps, asks for a tool or hands in what it may not write; a call has its result; a call
//! withdrawn is said to be; a round stopped ends.

use std::collections::BTreeSet;

use driver::NewItem;
use engine::Work;
use store::WorkHead;

use super::*;

/// The time of a step: a second after the one before, the same however often the step is told.
fn time(s: usize) -> Time {
    Time(START + 1_000_000 + s as u64 * 1000)
}

/// The work of the item added at this step of the history.
fn w(n: u64) -> WorkNumber {
    WorkNumber(n)
}

/// What is done at step `s`, or none once the history has ended.
fn act(h: &H, s: usize) -> Option<Act> {
    let item = ItemNumber;
    let scripted = match s {
        0 => Act::Add(NewItem {
            writes: vec![first_write("request", None)],
            ..new_item("a", &[], "judged")
        }),
        1 => Act::Add(new_item("b", &[item(1)], "short")),
        2 => Act::Add(new_item("c", &[], "judged")),
        3 => Act::Add(new_item("d", &[item(2), item(3)], "judged")),
        4 => Act::Add(new_item("e", &[], "short")),
        5 => Act::Add(new_item("f", &[], "judged")),
        6 => Act::On(w(1), Event::Resumed),
        7 => Act::On(w(2), Event::Resumed),
        8 => Act::On(w(4), Event::Resumed),
        9 => Act::On(w(5), Event::Resumed),
        10 => Act::On(w(5), Event::Cancelled { why: None }),
        11 => Act::On(w(6), Event::Resumed),
        12 => {
            let (run, ..) = h.round(w(6));
            let came = Came::asks(vec![question(Who::Owner)]);
            Act::On(
                w(6),
                Event::Returned {
                    run,
                    came,
                    spent: spent(s as u64),
                },
            )
        }
        13 => Act::On(w(6), wrote("request", Outsider::Person, None)),
        14 => Act::On(w(1), wrote("opinions", Outsider::Person, None)),
        // The writer of the sixth, run again, hands in what it may not write.
        15 => {
            let (run, ..) = h.round(w(6));
            let came = hands_in(&[("request", None)]);
            Act::On(
                w(6),
                Event::Returned {
                    run,
                    came,
                    spent: spent(s as u64),
                },
            )
        }
        20 => Act::On(w(3), Event::Resumed),
        _ => return answer(h, s),
    };
    Some(scripted)
}

/// One of the things given to be done and not yet answered, answered.
fn answer(h: &H, s: usize) -> Option<Act> {
    let outstanding = sorted(&h.outstanding);
    if outstanding.is_empty() {
        return (s < 20).then(|| Act::On(w(1), Event::Look));
    }
    let todo = outstanding[(s * 7) % outstanding.len()].clone();
    let event = match todo.effect {
        Effect::Round {
            run,
            cell,
            number,
            given,
        } => {
            let kept = h.d().work(todo.work).expect("the work is there");
            let came = if engine_cell(kept.flow, cell.as_str()) || matches!(given, Given::SentBack(_)) {
                proper(kept.flow, cell.as_str())
            } else if number == 1 && s % 5 == 1 {
                Came::asks(vec![tool("search")])
            } else if number == 1 && s % 7 == 3 {
                hands_in(&[("request", None)])
            } else {
                proper(kept.flow, cell.as_str())
            };
            Event::Returned {
                run,
                came,
                spent: spent(s as u64),
            }
        }
        Effect::Call { call, .. } => Event::Done {
            call,
            result: format!("the result of {}", call.0),
        },
        Effect::Withdraw { call } => Event::Done {
            call,
            result: "withdrawn".to_string(),
        },
        Effect::Stop { run } => Event::Returned {
            run,
            came: Came::asks(vec![]),
            spent: spent(s as u64),
        },
        Effect::Release { .. } => Event::Released,
    };
    Some(Act::On(todo.work, event))
}

/// How a history ended: every work with its head and all that is kept of it, each entry with the time it was added
/// ([`by_time`]), what each does next, and every round and call given and answered.
#[derive(Debug, PartialEq)]
struct Ending {
    works: Vec<(WorkHead, Work, Decision)>,
    ever: BTreeSet<String>,
    undone: BTreeSet<String>,
    answered: BTreeSet<String>,
}

/// Plays the history, the store failing as the plan says.
fn play(plan: Plan, checking: bool) -> (Ending, Stops, usize, bool) {
    let mut h = H::new();
    h.checking = checking;
    h.arm(plan);
    let mut seen = Stops::default();
    let mut s = 0;
    while let Some(act) = act(&h, s) {
        perform(&mut h, time(s), &act, &mut seen);
        s += 1;
        assert!(s < 500, "the history does not end");
    }
    h.check();
    // The calls of the store of the last driver opened: of the history's one driver where nothing stopped it.
    let calls = h.ops().len();
    let cut = seen.kept + seen.not_kept + seen.deaths > 0;
    let works = (h.d().works())
        .map(|k| (k.head.clone(), by_time(k.work), h.decision(k.head.number)))
        .collect();
    let ending = Ending {
        works,
        ever: h.ever.clone(),
        undone: h.undone.clone(),
        answered: h.answered.clone(),
    };
    (ending, seen, calls, cut)
}

/// The history with no stop, and how many calls of the store it made.
fn straight() -> (Ending, usize) {
    let (ending, _, calls, _) = play(Plan::Never, true);
    let ends: Vec<Decision> = ending.works.iter().map(|(_, _, d)| d.clone()).collect();
    let merged = Decision::Ended(End::Merged);
    let cancelled = Decision::Ended(End::Cancelled);
    assert_eq!(
        ends,
        [
            merged.clone(),
            merged.clone(),
            merged.clone(),
            merged.clone(),
            cancelled,
            merged
        ],
        "the history did not end as it was written to"
    );
    for kind in ["Stop", "Withdraw", "SentBack", "Results", "Call"] {
        let mut given = ending.ever.iter().chain(&ending.undone);
        assert!(given.any(|k| k.contains(kind)), "nothing of the kind {kind} was given");
    }
    (ending, calls)
}

#[test]
fn a_history_stopped_at_any_call_of_the_store_ends_as_it_does_with_no_stop() {
    let (expected, calls) = straight();
    assert!(calls > 20, "the history made only {calls} calls of the store");
    let mut seen = Stops::default();
    for n in 1..=calls {
        for die in [false, true] {
            let plan = if die {
                Plan::DieAfter { n }
            } else {
                Plan::Fail { op: None, n, on: false }
            };
            let (ending, s, _, cut) = play(plan, false);
            assert!(cut, "the store was never cut short as {plan:?} says");
            assert_eq!(
                ending, expected,
                "the history cut short as {plan:?} says ended otherwise"
            );
            seen.kept += s.kept;
            seen.not_kept += s.not_kept;
            seen.deaths += s.deaths;
        }
    }
    assert!(
        seen.kept > 0 && seen.not_kept > 0 && seen.deaths > 0,
        "a kind of stop never occurred: {seen:?}"
    );
}

#[test]
fn a_history_whose_store_fails_from_some_call_on_ends_as_it_does_with_no_stop() {
    let (expected, calls) = straight();
    for n in (1..=calls).step_by(7) {
        let plan = Plan::Fail { op: None, n, on: true };
        let (ending, _, _, cut) = play(plan, false);
        assert!(cut);
        assert_eq!(
            ending, expected,
            "the history cut short as {plan:?} says ended otherwise"
        );
    }
}
