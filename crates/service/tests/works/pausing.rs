//! A pause holds back what would begin a round, and nothing else (decision `stops-from-outside-are-no-cells`).

use decide::Decision;
use service::Act;

use crate::Trial;

#[tokio::test(start_paused = true)]
async fn a_pause_lets_the_round_under_way_end_and_its_calls_be_made_and_holds_the_next() {
    let mut trial = Trial::open();
    let (_, work) = trial.add("Greet the user", &[]);
    trial.act(work, Act::Resume).unwrap();
    trial.step().await.unwrap();
    trial.answer(work, "Yes, by name.");
    trial
        .until(
            work,
            |d| matches!(d, Decision::Busy { cell, .. } if cell.as_str() == "build"),
        )
        .await;

    trial.act(work, Act::Pause).unwrap();
    // The build's round ends while the work is paused, and the CI it asks for is run.
    trial.step().await.unwrap();
    assert!(matches!(trial.decision(work), Decision::Wait { at, .. } if at.as_str() == "build"));
    trial.step().await.unwrap();
    // Entering it again for the result is held: nothing more comes.
    assert_eq!(trial.decision(work), Decision::Paused);
    assert!(trial.step().await.is_none());
    let runs = trial.work(work).records.started.len();

    trial.act(work, Act::Resume).unwrap();
    assert_eq!(trial.work(work).records.started.len(), runs + 1);
    assert!(matches!(trial.decision(work), Decision::Busy { cell, .. } if cell.as_str() == "build"));
}

#[tokio::test(start_paused = true)]
async fn a_paused_work_begins_no_run() {
    let mut trial = Trial::open();
    let (_, work) = trial.add("Greet the user", &[]);
    trial.act(work, Act::Resume).unwrap();
    trial.step().await.unwrap();
    trial.answer(work, "Yes, by name.");
    trial.act(work, Act::Pause).unwrap();
    // The plan, entered for the answer, has its round end and finishes; the build does not begin.
    trial.step().await.unwrap();
    assert_eq!(trial.finished(work), ["plan"]);
    assert_eq!(trial.decision(work), Decision::Paused);
    assert!(trial.step().await.is_none());
}
