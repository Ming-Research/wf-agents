//! A round that fails is handled as the kind of its cause says. In passing it waits, as a call the engine makes whose
//! time the clock gives, before it is tried again, and so many in a row pause the work; what could not be read is sent
//! back; a flagged round is tried again at once, and flagged again pauses the work; a cause every work shares pauses
//! nothing, and holds the agents' rounds only through the gate whatever drives the engine gives it.

use decide::run::{FIRST_WAIT, Failure, PASSING};
use engine::{Back, Gate, handle_with};

use super::*;

/// The round under way of the run fails for a cause of this kind.
fn failed_as(run: RunId, failure: Failure) -> Event {
    Event::Failed {
        run,
        why: format!("it failed: {failure:?}"),
        failure,
        resets: None,
        spent: spent(1),
    }
}

/// The one call given to be made, the engine's wait, with when it is over.
fn wait_of(effects: &[Effect]) -> (CallId, Time, u32) {
    match calls_of(effects)[..] {
        [(call, _, Call::Wait { until, failed })] => (call, until, failed),
        ref other => panic!("not one wait: {other:?}"),
    }
}

#[test]
fn a_round_that_failed_in_passing_waits_from_its_end_and_is_then_given_again() {
    let (mut w, write, _) = W::requested();
    let failed_at = w.clock;
    let effects = w.on(failed_as(write, Failure::Passing));
    assert!(!decide::paused(&w.work.records), "a failure in passing pauses nothing");
    let (call, until, failed) = wait_of(&effects);
    assert_eq!((until, failed), (Time(failed_at + FIRST_WAIT), 1));
    assert_eq!(
        w.work.back.get(&(write, 1)),
        Some(&Back::Failed {
            why: "it failed: Passing".to_string(),
            failure: Failure::Passing,
            resets: None,
        })
    );
    // The work waits for the engine's call, which is pending until its time has come.
    assert!(matches!(w.decision(), decide::Decision::Wait { .. }));
    assert_eq!(pending(&w.work), effects);
    assert_eq!(w.done(call), vec![round_effect(write, "write", 2, Given::Task)]);
    // The second in a row waits twice as long.
    let failed_at = w.clock;
    let (_, until, failed) = wait_of(&w.on(failed_as(write, Failure::Passing)));
    assert_eq!((until, failed), (Time(failed_at + 2 * FIRST_WAIT), 2));
}

#[test]
fn so_many_failures_in_passing_in_a_row_pause_the_work_and_once_let_on_it_is_tried_at_once() {
    let (mut w, write, _) = W::requested();
    for n in 1..=PASSING as usize {
        let (call, ..) = wait_of(&w.on(failed_as(write, Failure::Passing)));
        assert_eq!(w.done(call), vec![round_effect(write, "write", n + 1, Given::Task)]);
    }
    assert_eq!(w.on(failed_as(write, Failure::Passing)), vec![]);
    let pause = decide::pause(&w.work.records).expect("the work is paused");
    assert_eq!(pause.by, Pauser::Engine);
    let why = pause.why.clone().unwrap_or_default();
    assert!(why.contains(&format!("{} times in a row", PASSING + 1)), "{why}");
    let next = PASSING as usize + 2;
    assert_eq!(
        w.on(Event::Resumed),
        vec![round_effect(write, "write", next, Given::Task)]
    );
    // Counted anew from there: the next failure waits as the first did.
    let (_, _, failed) = wait_of(&w.on(failed_as(write, Failure::Passing)));
    assert_eq!(failed, 1);
}

#[test]
fn a_return_that_could_not_be_read_is_sent_back_in_its_session_without_a_pause() {
    let (mut w, write, _) = W::requested();
    let effects = w.on(failed_as(write, Failure::Unread));
    assert!(!decide::paused(&w.work.records));
    let unmet = vec![Unmet::Unreadable {
        why: "it failed: Unread".to_string(),
    }];
    assert_eq!(effects, vec![round_effect(write, "write", 2, Given::SentBack(unmet))]);
}

#[test]
fn a_flagged_round_is_given_again_at_once_and_flagged_again_pauses_the_work() {
    let (mut w, write, _) = W::requested();
    assert_eq!(
        w.on(failed_as(write, Failure::Flagged)),
        vec![round_effect(write, "write", 2, Given::Task)]
    );
    assert!(!decide::paused(&w.work.records));
    assert_eq!(w.on(failed_as(write, Failure::Flagged)), vec![]);
    let pause = decide::pause(&w.work.records).expect("the work is paused");
    assert!(
        pause
            .why
            .as_deref()
            .unwrap_or_default()
            .contains("flagged the round again"),
        "{pause:?}"
    );
}

#[test]
fn a_prompt_too_long_pauses_the_work_saying_so() {
    let (mut w, write, _) = W::requested();
    assert_eq!(w.on(failed_as(write, Failure::PromptTooLong)), vec![]);
    let pause = decide::pause(&w.work.records).expect("the work is paused");
    assert!(
        pause.why.as_deref().unwrap_or_default().contains("prompt is too long"),
        "{pause:?}"
    );
}

#[test]
fn a_cause_every_work_shares_pauses_nothing_and_the_shut_gate_holds_only_agents_rounds() {
    let (mut w, write, _) = W::requested();
    let now = w.tick();
    let shut = |w: &mut W, now, event| handle_with(&w.flow, &mut w.work, now, event, Gate::Shut).unwrap();
    assert_eq!(shut(&mut w, now, failed_as(write, Failure::UsageLimit)), vec![]);
    assert!(!decide::paused(&w.work.records), "the limit is no pause of the work");
    // While the gate is shut, the round is not given again, and nothing is started: an agent's run waits.
    let now = w.tick();
    assert_eq!(shut(&mut w, now, Event::Look), vec![]);
    assert_eq!(w.work.records.rounds.len(), 1);
    // Once it opens, the round is given again, given what the failed one was.
    assert_eq!(w.on(Event::Look), vec![round_effect(write, "write", 2, Given::Task)]);
    // The judge, an agent's cell, is not started while it is shut; the merge, the engine's own act, is.
    let now = w.tick();
    let returned = Event::Returned {
        run: write,
        came: hands_in(&[("text", None)]),
        spent: spent(2),
    };
    assert_eq!(shut(&mut w, now, returned), vec![]);
    let judges = (w.work.records.started.iter())
        .filter(|s| s.cell.as_str() == "judge")
        .count();
    assert_eq!(judges, 0, "an agent's cell started while the gate was shut");
    let (judge, ..) = round(&w.on(Event::Look));
    let effects = w.returns(judge, hands_in(&[("verdict", Some("yes"))]));
    assert_eq!(effects, vec![]);
    let now = w.tick();
    let go = wrote("go", Outsider::Person, true, Some("yes"));
    let (_, cell, ..) = round(&shut(&mut w, now, go));
    assert_eq!(cell, "merge");
}

#[test]
fn a_notes_round_that_failed_in_passing_over_its_budget_waits_and_is_then_held_for_the_owner() {
    let (mut w, write, _) = W::requested();
    let (judge, ..) = round(&w.returns(write, hands_in(&[("text", None)])));
    assert_eq!(w.returns(judge, hands_in(&[("verdict", Some("yes"))])), vec![]);
    w.on(Event::Paused {
        by: Pauser::Owner,
        why: None,
    });
    // A note to the writer is answered while the work is paused.
    let (effects, _) = w.person_writes("opinions");
    let (note, ..) = round(&effects);
    // Its round takes the work over its budget, which its carrier says before the round's end, and fails in passing.
    w.on(Event::Paused {
        by: Pauser::Engine,
        why: Some("over its budget".to_string()),
    });
    let (call, ..) = wait_of(&w.on(failed_as(note, Failure::Passing)));
    assert_eq!(
        w.done(call),
        vec![],
        "the round after it was given though the engine paused the work for it"
    );
    assert_eq!(w.on(Event::Resumed), vec![round_effect(note, "write", 2, Given::Task)]);
}
