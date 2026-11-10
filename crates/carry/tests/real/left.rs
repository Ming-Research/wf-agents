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

#[tokio::test]
async fn a_round_stopped_after_its_engine_ended_has_its_left_program_stopped() {
    let fake = Fake {
        plan_sleeps: 30,
        ..Fake::default()
    };
    let mut real = Real::trial(fake);
    // The first process of the engine ends before it stops anything: its stops are dropped.
    real.deafen();
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.wait(|real| real.calls().len() == 1).await;
    let plan = real.run_of(work, "plan");
    let pgid = real.data.join(format!("rounds/1/{}-1.pgid", plan.0));
    real.wait(|_| pgid.exists()).await;
    real.act(work, Act::Cancel);

    // Started again, the stop finds nothing held, and stops the program left running before it ends the round.
    real.restart();
    real.until(|real| real.ended_rounds(work, plan)).await;
    assert_eq!(real.calls().len(), 1, "the round was not run again");
    let leader = real.call_process(1);
    real.wait(|_| !alive(leader)).await;
}

/// Whether the process is alive.
fn alive(process: u32) -> bool {
    let status = std::process::Command::new("kill")
        .args(["-0", &process.to_string()])
        .stderr(std::process::Stdio::null())
        .status()
        .expect("kill runs");
    status.success()
}
