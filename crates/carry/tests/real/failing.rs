//! A round that fails pauses its work, and is given again, the same round, once the owner lets the work on.

use service::Act;

use crate::{Fake, Real, ended};

#[tokio::test]
async fn a_failed_round_pauses_the_work_and_runs_again_on_resume() {
    let fake = Fake {
        cost: "0.01",
        fail_first: true,
    };
    let mut real = Real::open(fake, None);
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| real.failures == 1).await;
    assert!(real.paused(work), "the work is paused");
    let kept = real.work(work);
    assert_eq!(kept.records.rounds.len(), 1, "the plan's round is under way");
    assert!(kept.records.returned.is_empty(), "and has not ended");
    assert!(kept.spent.is_empty());

    real.act(work, Act::Resume);
    real.until(|real| ended(&real.decision(work))).await;
    assert_eq!(real.finished(work), ["plan", "build", "review", "merge"]);
    let calls = real.calls();
    assert_eq!(calls.len(), 4, "the plan's round was run twice");
    assert_eq!(calls[0].stdin, calls[1].stdin, "the same round, told the same");
    let kept = real.work(work);
    let plan_run = kept.records.started[0].run;
    let plan_rounds = kept.records.rounds.iter().filter(|r| r.run == plan_run).count();
    assert_eq!(plan_rounds, 1, "the round given again is the same round");
}
