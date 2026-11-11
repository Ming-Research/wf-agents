//! A deliverable item, on the deliver flow as the repository defines it: once the items it is made of have merged, its
//! one cell's act writes that it is delivered and what it was made of, runs no agent, costs nothing and touches nothing
//! in the project's repository.

use decide::{BlockId, Decision, End};
use service::Act;
use store::{ItemNumber, WorkNumber};
use work::Spent;

use crate::{Fake, On, Real, ended, git_in};

/// What the repository holds that an act could change: its references, its worktrees, the state of its own worktree
/// and its head.
fn repository(real: &Real) -> Vec<String> {
    [
        "for-each-ref",
        "worktree list --porcelain",
        "status --porcelain",
        "rev-parse HEAD",
    ]
    .iter()
    .map(|args| git_in(&real.repo, &args.split(' ').collect::<Vec<_>>()))
    .collect()
}

/// What the work's latest version of the block says.
fn latest(real: &Real, work: WorkNumber, block: &str) -> String {
    let kept = real.work(work);
    let block = BlockId::from(block);
    let at = (kept.records.finished.iter())
        .filter(|f| f.writes.iter().any(|w| w.block == block))
        .map(|f| f.at)
        .max()
        .expect("the block was written");
    kept.contents[&(block, at)].clone()
}

#[tokio::test]
async fn a_deliverable_is_delivered_saying_what_it_was_made_of_touching_nothing_in_the_repository() {
    let mut real = Real::open(On::Deliver, Fake::default(), None, "true");
    let before = repository(&real);
    let alone = real.add_made_of(&[]).await;
    let of_one = real.add_made_of(&[ItemNumber(1)]).await;
    let of_two = real.add_made_of(&[ItemNumber(1), ItemNumber(2)]).await;
    for work in [of_two, of_one, alone] {
        real.act(work, Act::Resume);
    }
    real.until(|real| ended(&real.decision(of_two))).await;
    for work in [alone, of_one, of_two] {
        assert_eq!(real.decision(work), Decision::Ended(End::Merged));
        assert_eq!(real.finished(work), ["V1"]);
        let spent: Vec<Spent> = real.work(work).spent.values().copied().collect();
        assert_eq!(spent, [Spent::default()], "the act took nothing");
    }
    assert_eq!(latest(&real, alone, "delivered"), "Delivered, made of no other item.");
    assert_eq!(latest(&real, of_one, "delivered"), "Delivered, made of the item 1.");
    assert_eq!(
        latest(&real, of_two, "delivered"),
        "Delivered, made of the items 1 and 2."
    );
    assert!(real.calls().is_empty(), "an agent ran");
    assert_eq!(repository(&real), before, "the act changed the repository");
}
