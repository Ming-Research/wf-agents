//! A program a process of the engine left running is stopped before its round is run again, so that a session has
//! one program at a time.

use service::Act;

use crate::{Fake, Real, ended};

#[tokio::test]
async fn a_round_left_running_by_an_engine_that_stopped_is_stopped_before_it_runs_again() {
    let fake = Fake {
        plan_sleeps: 30,
        ..Fake::default()
    };
    let mut real = Real::trial(fake);
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.wait(|real| real.calls().len() == 1).await;
    let plan = real.run_of(work, "plan");
    let pgid = real.data.join(format!("rounds/1/{}-1.pgid", plan.0));
    real.wait(|_| pgid.exists()).await;

    real.restart();
    real.until(|real| ended(&real.decision(work))).await;
    let calls = real.calls();
    assert_eq!(calls.len(), 4, "the plan twice, the build and the review");
    assert!(
        !calls[1].overlapped,
        "the plan's first program was stopped before its second began"
    );
    assert!(calls.iter().all(|call| !call.overlapped));
}
