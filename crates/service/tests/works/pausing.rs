//! A pause holds back what would begin a round, and a check's taking of the project's turn to check and merge, for
//! which the carriers are told of each work's pause; nothing else.

use decide::Decision;
use service::Act;

use crate::Trial;

#[tokio::test(start_paused = true)]
async fn a_pause_lets_the_round_under_way_end_and_its_calls_be_made_and_holds_the_next() {
    let mut trial = Trial::open();
    let (_, work) = trial.add("Greet the user", &[]);
    trial.act(work, Act::Resume).unwrap();
    trial.to_the_owners_call(work).await;
    trial.let_go_on(work);
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
    assert!(matches!(trial.decision(work), Decision::Paused { .. }));
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
    trial.to_the_owners_call(work).await;
    trial.let_go_on(work);
    trial.act(work, Act::Pause).unwrap();
    // The plan's run, let go on, has its round end and finishes; the build does not begin.
    trial.step().await.unwrap();
    assert_eq!(trial.finished(work), ["plan"]);
    assert!(matches!(trial.decision(work), Decision::Paused { .. }));
    assert!(trial.step().await.is_none());
}

// The carriers are told whether a pause holds each work back, its own or the owner's of everything, as it changes and
// of every work as the loop opens, before any round is given: what a carrier holds a round back for itself, as the acts
// a check for the turn to check and merge, a pause holds back too.
#[tokio::test(start_paused = true)]
async fn the_carriers_are_told_of_each_works_pause_as_it_changes_and_as_the_loop_opens() {
    let mut trial = Trial::open();
    let (_, work) = trial.add("Greet the user", &[]);
    assert_eq!(trial.told_paused(work), Some(true), "a work is added paused");
    trial.act(work, Act::Resume).unwrap();
    assert_eq!(trial.told_paused(work), Some(false));
    trial.everything(true);
    assert_eq!(trial.told_paused(work), Some(true), "the pause of everything holds it");
    trial.everything(false);
    assert_eq!(trial.told_paused(work), Some(false));
    trial.act(work, Act::Pause).unwrap();
    trial.restart();
    assert_eq!(
        trial.told_paused(work),
        Some(true),
        "the carriers of the loop opened again are told"
    );
}
