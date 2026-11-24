//! What the calls of an agent's round have come to so far is told to the loop while the round runs, and let go of when
//! it ends.

use service::Act;

use crate::{Fake, Real, ended};

// The fake's plan makes one call to Opus 5.5 of 100 tokens sent and 1,000 read from the cache and sleeps: while the
// round sleeps, the loop holds that call's prompt as the round's context, no growth for a one-call round, and what the
// call costs at the model's price, told once the call's own line has named its model; once the work has ended it holds
// nothing.
#[tokio::test]
async fn what_the_calls_of_a_round_have_come_to_is_told_while_it_runs_and_let_go_of_when_it_ends() {
    let fake = Fake {
        plan_sleeps_once_begun: 3,
        ..Fake::default()
    };
    let mut real = Real::trial(fake);
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| {
        (real.service.driver().rounds_come_to().iter())
            .any(|(_, _, _, going)| going.cost.is_some_and(|cost| cost.value > 0.0))
    })
    .await;
    let held = real.service.driver().rounds_come_to();
    assert_eq!(held.len(), 1, "{held:?}");
    let (held_work, run, round, going) = held[0];
    assert_eq!((held_work, run, round), (work, real.run_of(work, "plan"), 1));
    assert_eq!(going.context, Some(1_100), "100 sent and 1,000 read from the cache");
    assert_eq!(
        going.added.map(|added| added.value),
        Some(0.0),
        "one call grows nothing"
    );
    assert!(going.per_call.is_none(), "no call comes after the first");
    let cost = going.cost.expect("what the call costs").value;
    assert!(cost > 0.0, "the call is priced at its model's: {cost}");

    real.until(|real| ended(&real.decision(work))).await;
    assert!(real.service.driver().rounds_come_to().is_empty());
}
