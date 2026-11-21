//! A work that has spent more than it may is paused as a round ends, the round's return taken, and the words beside it
//! say what it spent, what it may spend and what letting it on allows; and each letting-on of a work the engine paused
//! so allows it what the settings say beyond its budget, so that the work goes on rather than stopping at every
//! round.

use decide::Decision;
use service::Act;

use crate::{Fake, On, Real};

#[tokio::test]
async fn a_work_over_its_budget_is_paused_after_the_round_that_ends_over_it() {
    // Each call says the session has cost four dollars so far; the budget is three.
    let fake = Fake {
        cost: "4.0",
        ..Fake::default()
    };
    let mut real = Real::open(On::Trial, fake, Some(3.0), "true");
    let work = real.add().await;
    // A work is added paused by the owner, and the letting-on that starts it allows nothing beyond its budget: its
    // first round, at four dollars, is over the three it may spend.
    real.act(work, Act::Resume);
    real.until(|real| real.finished(work) == ["plan"] && matches!(real.decision(work), Decision::Paused { .. }))
        .await;
    assert_eq!(real.holds, 1);
    assert_eq!(real.calls().len(), 1, "no round began after the one over the budget");
}

#[tokio::test]
async fn a_work_let_on_after_its_budget_paused_it_is_held_to_what_the_letting_on_allows() {
    let fake = Fake {
        cost: "4.0",
        ..Fake::default()
    };
    let mut real = Real::open(On::Trial, fake, Some(3.0), "true");
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| real.finished(work) == ["plan"] && matches!(real.decision(work), Decision::Paused { .. }))
        .await;
    assert_eq!((real.holds, real.calls().len()), (1, 1));
    // Let on after that pause, the work is held to the twenty dollars beyond its budget that these settings allow a
    // letting-on: its rounds after it, at eight dollars and at twelve, are under the twenty-three it may now spend,
    // and it goes on round after round rather than stopping at each.
    real.act(work, Act::Resume);
    real.until(|real| real.calls().len() == 3 || real.paused(work)).await;
    assert!(
        !real.paused(work),
        "paused again within what the letting-on allowed: {:?}",
        real.decision(work)
    );
    assert_eq!((real.holds, real.calls().len()), (1, 3));
}

#[tokio::test]
async fn the_words_beside_a_work_over_its_budget_say_what_letting_it_on_allows() {
    let fake = Fake {
        cost: "4.0",
        ..Fake::default()
    };
    let mut real = Real::open(On::Trial, fake, Some(3.0), "true");
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| real.finished(work) == ["plan"] && matches!(real.decision(work), Decision::Paused { .. }))
        .await;
    // The owner presses the button knowing what it buys: what the work spent, the three dollars it may spend, and the
    // twenty these settings allow a letting-on.
    let why = decide::pause(&real.work(work).records)
        .and_then(|pause| pause.why.clone())
        .expect("a reason");
    assert_eq!(
        why,
        "It has spent $4.00, over its budget of $3.00. If you let it go on, it may spend $20.00 more."
    );
}
