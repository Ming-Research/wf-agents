//! An item that depends on another: its work waits before the plan until the other has merged, and a work that has run
//! waits where it stands for one it comes to depend on.

use decide::{Condition, Decision, End, Maker, TestName, Waited};
use driver::Asked;
use service::{Act, Message};
use store::{ItemChange, WorkNumber};
use tokio::sync::oneshot;

use crate::Trial;

#[tokio::test(start_paused = true)]
async fn a_second_item_waits_for_the_first_to_merge() {
    let mut trial = Trial::open();
    let (first_item, first) = trial.add("Greet the user", &[]);
    let (_, second) = trial.add("Say goodbye", &[first_item.0]);
    trial.act(first, Act::Resume).unwrap();
    trial.act(second, Act::Resume).unwrap();
    assert_eq!(
        trial.decision(second),
        Decision::Wait {
            at: "plan".into(),
            on: vec![Waited::Precondition {
                on: Condition::Test(TestName("ready".to_string())),
                by: vec![Maker::Outside {
                    source: "dependencies".to_string()
                }],
            }],
        }
    );

    assert_eq!(trial.drive(first).await, Decision::Ended(End::Merged));
    assert!(matches!(trial.decision(second), Decision::Busy { cell, .. } if cell.as_str() == "plan"));
    let kept = trial.work(second);
    let values: Vec<Option<&str>> = (kept.records.written.iter())
        .filter(|w| w.block.as_str() == "dependencies")
        .map(|w| w.value.as_deref())
        .collect();
    assert_eq!(values, [Some("waiting"), Some("merged")]);

    assert_eq!(trial.drive(second).await, Decision::Ended(End::Merged));
}

/// Drives the work, the owner letting each run go on that waits for it, until what it does is as `wanted` says.
async fn until_held(trial: &mut Trial, work: WorkNumber, wanted: impl Fn(&Decision) -> bool) -> Decision {
    loop {
        let decision = trial.decision(work);
        if wanted(&decision) {
            return decision;
        }
        if trial.owners_call(work).is_some() {
            trial.let_go_on(work);
            continue;
        }
        if trial.step().await.is_none() {
            panic!("nothing more comes, and work {} is at {decision:?}", work.0);
        }
    }
}

#[tokio::test(start_paused = true)]
async fn a_work_that_has_run_waits_where_it_stands_for_an_item_it_comes_to_depend_on_and_goes_on_from_there() {
    let mut trial = Trial::open();
    let (first_item, first) = trial.add("Greet the user", &[]);
    let (second_item, second) = trial.add("Say goodbye", &[]);
    trial.act(first, Act::Resume).unwrap();
    trial.act(second, Act::Resume).unwrap();
    // The second work has made its plan and is building when it is made to depend on the first.
    let building = until_held(
        &mut trial,
        second,
        |d| matches!(d, Decision::Busy { cell, .. } if cell.as_str() == "build"),
    )
    .await;
    assert!(matches!(building, Decision::Busy { .. }));
    let (reply, mut answer) = oneshot::channel();
    trial.take(Message::Change {
        why: "The goodbye needs the greeting.".to_string(),
        changes: vec![(second_item, Asked::Item(ItemChange::DependsOn(first_item)))],
        reply,
    });
    assert_eq!(answer.try_recv().expect("the loop answers at once"), Ok(vec![true]));
    // Its round ends as it would, and then it waits, not at its first cell but where it stands, for the first item.
    let holds = |d: &Decision| {
        matches!(d, Decision::Wait { on, .. } if on.iter().any(|w| matches!(w,
            Waited::Precondition { by, .. } if by == &[Maker::Outside { source: "dependencies".to_string() }])))
    };
    let held = until_held(&mut trial, second, holds).await;
    let Decision::Wait { ref at, .. } = held else {
        unreachable!()
    };
    assert_ne!(at.as_str(), "plan", "{held:?}");
    // The first merges, and the second goes on from where it was and ends: its plan is not made again.
    assert_eq!(trial.drive(first).await, Decision::Ended(End::Merged));
    assert_eq!(trial.drive(second).await, Decision::Ended(End::Merged));
    let plans = trial.finished(second).into_iter().filter(|cell| cell == "plan").count();
    assert_eq!(plans, 1, "{:?}", trial.finished(second));
}
