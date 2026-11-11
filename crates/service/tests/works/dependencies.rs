//! An item that depends on another: its work waits before the plan until the other has merged.

use decide::{Decision, End, Maker, TestName, Waited};
use service::Act;

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
                test: TestName("ready".to_string()),
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
