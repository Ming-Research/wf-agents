//! The process stopped and started again: at every step of a work, the work ends as it would have without the stop,
//! with what was under way given again and answered again by the script.

use decide::{Decision, End};
use service::Act;
use work::Work;

use crate::Trial;

/// Everything kept of the work but when each thing happened, which a stop puts off.
fn kept_but_times(work: &Work) -> Work {
    Work {
        moments: Vec::new(),
        ..work.clone()
    }
}

/// The trial work taken to its end, with the process stopped and started again after each of these steps.
async fn history(stops: &[usize]) -> (Work, usize) {
    let mut trial = Trial::open();
    let (_, work) = trial.add("Greet the user", &[]);
    if stops.contains(&0) {
        trial.restart();
    }
    trial.act(work, Act::Resume).unwrap();
    if stops.contains(&trial.steps) {
        trial.restart();
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
            trial.restart();
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
