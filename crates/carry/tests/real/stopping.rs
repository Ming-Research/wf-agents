//! A work cancelled while a round of it runs has that round stopped where it runs, and the round ends with nothing,
//! in the same process.

use std::time::{Duration, Instant};

use decide::Decision;
use service::Act;

use crate::{Fake, On, Real};

#[tokio::test]
async fn a_check_under_way_is_stopped_when_its_work_is_cancelled() {
    let mut real = Real::open(On::Acts, Fake::default(), None, "sleep 30");
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| matches!(real.decision(work), Decision::Busy { cell, .. } if cell.as_str() == "check"))
        .await;
    let check = real.run_of(work, "check");
    let began = Instant::now();
    real.act(work, Act::Cancel);
    real.until(|real| real.ended_rounds(work, check)).await;
    assert!(began.elapsed() < Duration::from_secs(20), "the check was stopped");
    assert!(real.work(work).records.abandoned.iter().any(|a| a.run == check));
}

#[tokio::test]
async fn an_agents_round_under_way_is_stopped_when_its_work_is_cancelled() {
    let fake = Fake {
        plan_sleeps: 30,
        ..Fake::default()
    };
    let mut real = Real::trial(fake);
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.wait(|real| real.calls().len() == 1).await;
    let plan = real.run_of(work, "plan");
    let began = Instant::now();
    real.act(work, Act::Cancel);
    real.until(|real| real.ended_rounds(work, plan)).await;
    assert!(began.elapsed() < Duration::from_secs(20), "the program was stopped");
    let pgid = real.data.join(format!("rounds/1/{}-1.pgid", plan.0));
    assert!(!pgid.exists(), "no program runs");
}
