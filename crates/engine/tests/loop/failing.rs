//! A round that fails: it ends with nothing come back, what it took is kept, and the engine pauses its work with the
//! reason in the same moment. Once the owner lets the work on, the run has its next round, given what the failed one
//! was given, whatever was written meanwhile, which stays new to its cell. A failure counts against no limit, and of a
//! run that was abandoned it pauses nothing.

use decide::run::Failure;
use decide::{Decision, End};
use engine::Back;

use super::*;

const WHY: &str = "The agent's program could not start.";

/// The round under way of the run fails, having taken what `spent(1)` says.
fn failed(run: RunId) -> Event {
    Event::Failed {
        run,
        why: WHY.to_string(),
        failure: Failure::Other,
        resets: None,
        hindrance: None,
        spent: spent(1),
    }
}

#[test]
fn a_round_that_fails_ends_and_pauses_its_work_with_the_reason_in_one_moment() {
    let (mut w, write, _) = W::requested();
    let moments = w.work.moments.len();
    assert_eq!(w.on(failed(write)), vec![]);
    assert_eq!(w.work.moments.len(), moments + 1);
    let r = &w.work.records;
    assert_eq!(r.returned.iter().filter(|e| e.run == write).count(), 1);
    assert_eq!(
        w.work.back.get(&(write, 1)),
        Some(&Back::Failed {
            why: WHY.to_string(),
            failure: Failure::Other,
            resets: None,
            hindrance: None,
        })
    );
    assert_eq!(w.work.spent.get(&(write, 1)), Some(&spent(1)));
    let pause = decide::pause(r).expect("the work is paused");
    // A sentence of the files' words, the failure's own words after a colon.
    let reason = format!("The round failed. The cause: {WHY}");
    assert_eq!(
        (pause.by, pause.why.as_deref()),
        (Pauser::Engine, Some(reason.as_str()))
    );
    // The round that failed is not under way, and nothing is to be done while the work is paused.
    assert_eq!(pending(&w.flow, &w.work), vec![]);
}

#[test]
fn once_let_on_the_run_whose_round_failed_has_its_next_round_given_what_the_failed_one_was() {
    let (mut w, write, _) = W::requested();
    w.on(failed(write));
    assert_eq!(w.on(Event::Resumed), vec![round_effect(write, "write", 2, Given::Task)]);
    // A round given the results of calls fails, and the round after it is given the same results.
    let call = calls_of(&w.returns(write, Came::asks(vec![tool("a")])))[0].0;
    let results = Given::Results(vec![call]);
    let effects = w.done(call);
    let given = w.work.records.started.last().expect("a run").run;
    assert_eq!(effects, vec![round_effect(given, "write", 1, results.clone())]);
    w.on(failed(given));
    assert_eq!(w.on(Event::Resumed), vec![round_effect(given, "write", 2, results)]);
    // And it goes on to finish as any run does.
    let judge = round(&w.returns(given, hands_in(&[("text", None)]))).0;
    assert_ne!(judge, given);
    assert!(w.work.records.finished.iter().any(|f| f.run == given));
}

#[test]
fn a_run_whose_round_failed_goes_on_once_let_on_and_what_the_owner_wrote_meanwhile_runs_its_cell_again() {
    let (mut w, write, first) = W::requested();
    w.on(failed(write));
    // The owner mends the cause by writing the request again: the run, at rest, stays.
    let (effects, request) = w.person_writes("request");
    assert_eq!(effects, vec![]);
    assert!(w.work.records.abandoned.is_empty());
    assert_eq!(w.on(Event::Resumed), vec![round_effect(write, "write", 2, Given::Task)]);
    w.returns(write, hands_in(&[("text", None)]));
    let text = w.last_finished();
    let again = w.work.records.started.last().expect("a run").run;
    assert_ne!(again, write);
    let start = started_with(
        vec![changed("request", Some(first), false)],
        &[
            ("request", Some(request)),
            ("opinions", None),
            ("main", None),
            ("text", Some(text)),
        ],
    );
    assert_eq!(start_of(&w, again), start);
}

#[test]
fn a_round_that_fails_after_its_work_was_cancelled_ends_and_pauses_nothing() {
    let (mut w, write, _) = W::requested();
    assert_eq!(w.on(Event::Cancelled { why: None }), vec![Effect::Stop { run: write }]);
    let release = vec![Effect::Release { end: End::Cancelled }];
    assert_eq!(w.on(failed(write)), release);
    assert!(!decide::paused(&w.work.records));
    // What came of it is dropped, as what comes back from a stopped round is; what it took is kept.
    assert_eq!(w.work.back.get(&(write, 1)), None);
    assert_eq!(w.work.spent.get(&(write, 1)), Some(&spent(1)));
    assert_eq!(pending(&w.flow, &w.work), release);
    assert_eq!(w.decision(), Decision::Ended(End::Cancelled));
}

#[test]
fn a_driver_that_starts_anew_while_the_work_is_paused_is_given_nothing_of_the_failed_round() {
    let (mut w, write, _) = W::requested();
    w.on(failed(write));
    let mut again = W {
        flow: w.flow.clone(),
        work: w.work.clone(),
        clock: w.clock,
    };
    // Started anew, the driver takes what is pending and looks at the work: nothing is to be done.
    assert_eq!(pending(&again.flow, &again.work), vec![]);
    assert_eq!(again.on(Event::Look), vec![]);
    assert_eq!(again.work.records, w.work.records);
    // Let on, it is given what the driver that did not stop is given.
    let resumed = w.on(Event::Resumed);
    assert_eq!(again.on(Event::Resumed), resumed);
    assert_eq!(again.work.records, w.work.records);
}

#[test]
fn a_failure_of_a_run_that_has_no_round_under_way_is_refused() {
    let (mut w, write, _) = W::requested();
    assert_eq!(w.refused(failed(RunId(99))), Refused::NoRoundUnderWay(RunId(99)));
    w.on(failed(write));
    assert_eq!(w.refused(failed(write)), Refused::NoRoundUnderWay(write));
}

#[test]
fn failed_rounds_count_against_no_limit() {
    let (mut w, write, _) = W::requested();
    let sent_back = Given::SentBack(wanting_unmet());
    assert_eq!(
        w.returns(write, wanting()),
        vec![round_effect(write, "write", 2, sent_back.clone())]
    );
    w.on(failed(write));
    assert_eq!(
        w.on(Event::Resumed),
        vec![round_effect(write, "write", 3, sent_back.clone())]
    );
    w.on(failed(write));
    w.on(Event::Resumed);
    // Two returns are sent back before the run reaches its limit, however many rounds failed between them.
    assert_eq!(
        w.returns(write, wanting()),
        vec![round_effect(write, "write", 5, sent_back)]
    );
    let calls = calls_of(&w.returns(write, wanting()));
    assert_eq!(
        calls.iter().map(|(_, r, what)| (*r, what.clone())).collect::<Vec<_>>(),
        vec![(write, Call::AtLimit(Limit::Returns { sent_back: 2 }))]
    );
}

/// What a return that asks for a person, and hands in a block beside, comes to: the engine takes it as a round that
/// failed, so what it hands in is not written. (Reference: the engine's own rule in `take` that the return of a run that
/// no carrier can mend is a failed round, and that a failed round keeps nothing of what came back.)
fn asking_for_a_person_and_handing_in() -> Came {
    Came {
        asks: vec![question(Who::Owner)],
        ..hands_in(&[("text", None)])
    }
}

#[test]
fn a_return_that_asks_for_a_person_hands_nothing_in_and_keeps_what_the_round_took() {
    let (mut w, write, _) = W::requested();
    let written = w.work.records.written.len();
    assert_eq!(
        w.returns_spending(write, asking_for_a_person_and_handing_in(), spent(7)),
        vec![]
    );
    // The block it handed in is not written, no run finished and no call was made.
    assert_eq!(w.work.records.written.len(), written);
    assert!(w.work.records.finished.is_empty());
    assert!(w.work.records.called.is_empty());
    assert_eq!(w.work.spent.get(&(write, 1)), Some(&spent(7)));
    assert!(matches!(
        w.work.back.get(&(write, 1)),
        Some(Back::Failed {
            failure: Failure::Other,
            ..
        })
    ));
    assert!(decide::paused(&w.work.records));
}

#[test]
fn a_return_that_asks_for_a_person_after_its_work_was_cancelled_ends_and_pauses_nothing_as_a_failure_does() {
    let (mut w, write, _) = W::requested();
    assert_eq!(w.on(Event::Cancelled { why: None }), vec![Effect::Stop { run: write }]);
    let release = vec![Effect::Release { end: End::Cancelled }];
    assert_eq!(
        w.returns_spending(write, asking_for_a_person_and_handing_in(), spent(1)),
        release
    );
    assert!(!decide::paused(&w.work.records));
    // What came back is dropped, as what comes back from a stopped round is; what it took is kept.
    assert_eq!(w.work.back.get(&(write, 1)), None);
    assert_eq!(w.work.spent.get(&(write, 1)), Some(&spent(1)));
    assert!(w.work.records.called.is_empty());
    assert_eq!(w.decision(), Decision::Ended(End::Cancelled));
}

#[test]
fn a_return_that_asks_for_a_person_from_a_run_with_no_round_under_way_is_refused_leaving_the_work_as_it_was() {
    let (mut w, write, _) = W::requested();
    assert_eq!(
        w.refused(Event::Returned {
            run: RunId(99),
            came: Came::asks(vec![question(Who::Owner)]),
            spent: spent(1),
        }),
        Refused::NoRoundUnderWay(RunId(99))
    );
    // The round that failed has ended: a second return of it is refused as any return of an ended round is.
    w.returns(write, Came::asks(vec![question(Who::Owner)]));
    assert_eq!(
        w.refused(Event::Returned {
            run: write,
            came: Came::asks(vec![question(Who::Owner)]),
            spent: spent(1),
        }),
        Refused::NoRoundUnderWay(write)
    );
}
