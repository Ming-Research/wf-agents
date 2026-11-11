//! The process stopped and started again: at every step of a work, the work ends as it would have without the stop
//! (entry `a-stop-changes-nothing`), with what was under way given again and answered again by the script, once the
//! owner has lifted the pause of everything the engine makes as it starts again with the work going on. Until then,
//! nothing begins a round but what was under way when it stopped.

use decide::{Decision, End};
use service::Act;
use store::WorkNumber;
use work::Work;

use crate::Trial;

/// Everything kept of the work but when each thing happened, which a stop puts off.
fn kept_but_times(work: &Work) -> Work {
    Work {
        moments: Vec::new(),
        ..work.clone()
    }
}

/// Stops the process and starts it again; the engine holds everything where the work was going on, which it was
/// exactly when it had been let on and had not ended, and the owner lifts that at once.
fn restart(trial: &mut Trial, work: WorkNumber) {
    let going_on = trial.work(work).records.pauses.last().is_some_and(|p| !p.paused)
        && !matches!(trial.decision(work), Decision::Ended(_));
    trial.restart();
    assert_eq!(
        trial.held_by_its_start(),
        going_on,
        "started again, the engine holds everything or not"
    );
    if going_on {
        trial.everything(false);
    }
}

/// The trial work taken to its end, with the process stopped and started again after each of these steps.
async fn history(stops: &[usize]) -> (Work, usize) {
    let mut trial = Trial::open();
    let (_, work) = trial.add("Greet the user", &[]);
    if stops.contains(&0) {
        restart(&mut trial, work);
    }
    trial.act(work, Act::Resume).unwrap();
    if stops.contains(&trial.steps) {
        restart(&mut trial, work);
    }
    loop {
        if matches!(trial.decision(work), Decision::Ended(_)) {
            break;
        }
        if trial.owners_call(work).is_some() {
            trial.answer(work, "Yes, by name.");
        } else {
            assert!(trial.step().await.is_some(), "the work comes to its end");
        }
        if stops.contains(&trial.steps) {
            restart(&mut trial, work);
        }
    }
    assert_eq!(trial.decision(work), Decision::Ended(End::Merged));
    (kept_but_times(&trial.work(work)), trial.steps)
}

#[tokio::test(start_paused = true)]
async fn a_stop_at_any_step_changes_nothing() {
    let (without, steps) = history(&[]).await;
    for stop in 0..=steps {
        let (with, _) = history(&[stop]).await;
        assert_eq!(with, without, "stopped after step {stop}");
    }
}

#[tokio::test(start_paused = true)]
async fn stops_at_many_steps_change_nothing() {
    let (without, steps) = history(&[]).await;
    let every: Vec<usize> = (0..=steps).collect();
    let (with, _) = history(&every).await;
    assert_eq!(with, without);
}

#[tokio::test(start_paused = true)]
async fn started_again_with_a_work_going_on_it_gives_the_round_under_way_and_begins_nothing_more_until_lifted() {
    let mut trial = Trial::open();
    let (_, work) = trial.add("Greet the user", &[]);
    trial.act(work, Act::Resume).unwrap();
    assert!(matches!(trial.decision(work), Decision::Busy { .. }));
    let rounds = trial.work(work).records.rounds.len();
    trial.restart();
    assert!(trial.held_by_its_start());
    // The round under way at the stop is given again, and ends; the plan's question is asked and answered.
    trial.until(work, |d| matches!(d, Decision::Wait { .. })).await;
    trial.answer(work, "Yes, by name.");
    // Nothing more begins: the plan's next round waits for the owner.
    assert!(trial.step().await.is_none());
    assert_eq!(trial.work(work).records.rounds.len(), rounds);
    assert!(
        matches!(trial.decision(work), Decision::GoOn { .. }),
        "{:?}",
        trial.decision(work)
    );
    trial.everything(false);
    assert_eq!(trial.work(work).records.rounds.len(), rounds + 1);
    assert_eq!(trial.drive(work).await, Decision::Ended(End::Merged));
}
