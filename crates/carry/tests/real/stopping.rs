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
    real.act(work, Act::Cancel { why: None });
    real.until(|real| real.ended_rounds(work, check)).await;
    assert!(began.elapsed() < Duration::from_secs(20), "the check was stopped");
    assert!(real.work(work).records.abandoned.iter().any(|a| a.run == check));
}

#[tokio::test]
async fn a_compaction_in_a_round_that_is_stopped_is_still_told_as_the_session_reopened() {
    let fake = Fake {
        compacts: true,
        plan_sleeps: 30,
        ..Fake::default()
    };
    let mut real = Real::trial(fake);
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.wait(|real| real.calls().len() == 1).await;
    // The compaction is the program's hook, which gives its message once it has told the loop: a round stopped before
    // that has not compacted, so the round is stopped after the hook ended.
    let again = real.calls_dir().join("1.again");
    real.wait(|_| std::fs::read_to_string(&again).is_ok_and(|text| !text.is_empty()))
        .await;
    let plan = real.run_of(work, "plan");
    real.act(work, Act::Cancel { why: None });
    real.until(|real| real.ended_rounds(work, plan)).await;
    let kept = real.work(work);
    assert!(
        kept.sessions.iter().any(|s| s.why == work::Why::Compacted),
        "{:?}",
        kept.sessions
    );
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
    real.act(work, Act::Cancel { why: None });
    real.until(|real| real.ended_rounds(work, plan)).await;
    assert!(began.elapsed() < Duration::from_secs(20), "the program was stopped");
    let pgid = real.data.join(format!("rounds/1/{}-1.pgid", plan.0));
    assert!(!pgid.exists(), "no program runs");
    // What the hook gives the session again after a compaction is the program's while it runs, and no second copy of
    // the message is kept past a round that was stopped.
    let again = real.data.join(format!("rounds/1/{}-1.again.md", plan.0));
    assert!(!again.exists(), "{}", again.display());
}
