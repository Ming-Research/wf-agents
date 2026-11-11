//! A cancelled work: its round under way is stopped, and the calls it rests at are withdrawn, each answered once; then
//! what it held is let go.

use decide::{CallId, Decision, End};
use engine::Event;
use service::{Act, WITHDRAWN};

use crate::Trial;

#[tokio::test(start_paused = true)]
async fn cancelling_in_a_round_stops_it() {
    let mut trial = Trial::open();
    let (_, work) = trial.add("Greet the user", &[]);
    trial.act(work, Act::Resume).unwrap();
    trial.act(work, Act::Cancel { why: None }).unwrap();
    assert_eq!(trial.decision(work), Decision::Ended(End::Cancelled));
    // The round's end comes from its stop, and once.
    trial.step().await.unwrap();
    let kept = trial.work(work);
    assert_eq!(kept.records.returned.len(), 1);
    assert!(kept.back.is_empty(), "what came back from a stopped round is dropped");
    assert_eq!(
        trial.step().await,
        Some(Event::Released),
        "what the work held is let go"
    );
    assert!(trial.step().await.is_none());
}

#[tokio::test(start_paused = true)]
async fn cancelling_at_a_question_withdraws_it() {
    let mut trial = Trial::open();
    let (_, work) = trial.add("Greet the user", &[]);
    trial.act(work, Act::Resume).unwrap();
    trial.step().await.unwrap();
    trial.act(work, Act::Cancel { why: None }).unwrap();
    assert_eq!(trial.decision(work), Decision::Ended(End::Cancelled));
    assert_eq!(trial.work(work).results[&CallId(2)], WITHDRAWN);
    assert_eq!(
        trial.step().await,
        Some(Event::Released),
        "what the work held is let go"
    );
    assert!(trial.step().await.is_none());
}

#[tokio::test(start_paused = true)]
async fn cancelling_at_a_tool_withdraws_it() {
    let mut trial = Trial::open();
    let (_, work) = trial.add("Greet the user", &[]);
    trial.act(work, Act::Resume).unwrap();
    trial.step().await.unwrap();
    trial.answer(work, "Yes, by name.");
    trial
        .until(
            work,
            |d| matches!(d, Decision::Wait { at, .. } if at.as_str() == "build"),
        )
        .await;
    trial.act(work, Act::Cancel { why: None }).unwrap();
    assert_eq!(trial.decision(work), Decision::Ended(End::Cancelled));
    trial.step().await.unwrap();
    assert_eq!(trial.work(work).results[&CallId(4)], WITHDRAWN);
    assert_eq!(
        trial.step().await,
        Some(Event::Released),
        "what the work held is let go"
    );
    assert!(trial.step().await.is_none());
}

#[tokio::test(start_paused = true)]
async fn a_stop_after_a_cancel_stops_the_round_again() {
    let mut trial = Trial::open();
    let (_, work) = trial.add("Greet the user", &[]);
    trial.act(work, Act::Resume).unwrap();
    trial.act(work, Act::Cancel { why: None }).unwrap();
    // The process stops before the round's end is taken: the round is stopped again when it starts.
    trial.restart();
    trial.step().await.unwrap();
    assert_eq!(trial.work(work).records.returned.len(), 1);
    assert_eq!(
        trial.step().await,
        Some(Event::Released),
        "what the work held is let go"
    );
    assert!(trial.step().await.is_none());
}
