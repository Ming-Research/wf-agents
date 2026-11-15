//! Runs whose round failed: to the walk, the run is at rest like any other, its work paused by the engine with the
//! reason. It goes on once the work is let on, whatever was written meanwhile, which stays new to its cell; a cancel
//! ends it as it ends any run.

use super::*;

#[test]
fn a_run_whose_round_failed_is_at_rest_and_held_by_the_engines_pause_with_its_reason() {
    let (mut w, _, _) = judged();
    let review = w.start("review");
    w.fails("review", "the agent could not start");
    assert_eq!(w.decide(), go_on("review", review));
    let pause = decide::pause(&w.records).expect("the work is paused");
    assert_eq!(
        (pause.by, pause.why.as_deref()),
        (Pauser::Engine, Some("the agent could not start"))
    );
}

#[test]
fn once_the_work_is_let_on_the_run_whose_round_failed_goes_on() {
    let (mut w, _, _) = judged();
    let review = w.start("review");
    w.fails("review", "the agent could not start");
    w.resume();
    assert!(!decide::paused(&w.records));
    assert_eq!(w.decide(), go_on("review", review));
    w.go_on("review");
    assert_eq!(w.decide(), busy("review", review));
}

#[test]
fn a_run_whose_round_failed_goes_on_once_let_on_and_what_was_written_meanwhile_stays_new_to_its_cell() {
    let (mut w, _, code) = judged();
    let review = w.start("review");
    w.fails("review", "the agent could not start");
    // The owner mends the cause by writing what runs the review.
    let mended = w.person_writes("code");
    assert_eq!(w.decide(), go_on("review", review));
    w.resume();
    w.go_on("review");
    w.finish("review", &[]);
    assert_eq!(
        w.decide(),
        runs(
            "review",
            vec![changed("code", Some(code), false)],
            &[("code", Some(mended)), ("opinions", None)]
        )
    );
}

#[test]
fn a_round_that_fails_after_its_work_was_cancelled_ends_it() {
    let (mut w, _, _) = judged();
    let review = w.start("review");
    w.cancel();
    assert_eq!(w.decide(), abandon("review", review, Abandoning::Cancelled));
    w.abandon();
    // The round that was stopped ends with nothing come back; the engine pauses nothing of a run abandoned.
    let at = w.tick();
    w.records.returned.push(Returned { run: review, at });
    assert_eq!(w.decide(), Decision::Ended(End::Cancelled));
    assert!(!decide::paused(&w.records));
}
