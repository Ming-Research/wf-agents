//! A work that has spent more than its budget is paused as a round ends, the round's return taken.

use decide::Decision;
use service::Act;

use crate::{Fake, Real};

#[tokio::test]
async fn a_work_over_its_budget_is_paused_after_each_round_that_ends_over_it() {
    // Each call says the session has cost four dollars so far; the budget is three.
    let fake = Fake {
        cost: "4.0",
        fail_first: false,
    };
    let mut real = Real::open(fake, Some(3.0));
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| real.finished(work) == ["plan"] && real.decision(work) == Decision::Paused)
        .await;
    assert_eq!(real.holds, 1);
    assert_eq!(real.calls().len(), 1, "no round began after the one over the budget");

    // Let on, the build runs, and the work, still over its budget, is paused again.
    real.act(work, Act::Resume);
    real.until(|real| real.finished(work) == ["plan", "build"] && real.decision(work) == Decision::Paused)
        .await;
    assert_eq!(real.holds, 2);
    assert_eq!(real.calls().len(), 2);
}
