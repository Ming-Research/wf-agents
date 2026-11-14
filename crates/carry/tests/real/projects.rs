//! One installation, several projects: each work runs in the repository of its item's project and merges into that
//! project's main, and an item of one project may depend on an item of another, waiting for it as for one of its own.

use decide::{Decision, End};
use service::Act;
use store::ItemNumber;

use crate::{Fake, On, Real, ended, git_in};

#[tokio::test]
async fn two_projects_in_one_installation_each_work_in_its_own_repository() {
    let mut real = Real::open(On::Acts, Fake::default(), None, "true");
    let other = real.add_project("other");
    let ours = real.add().await;
    real.project = "other".to_string();
    let theirs = real.add().await;
    // One after the other, so that the fake command line is called by one round at a time.
    for work in [ours, theirs] {
        real.act(work, Act::Resume);
        real.until(|real| ended(&real.decision(work))).await;
    }
    for work in [ours, theirs] {
        assert_eq!(real.decision(work), Decision::Ended(End::Merged));
        assert_eq!(real.finished(work), ["start", "build", "check", "merge"]);
    }
    // Each work's branch is in its own project's repository, and each merged into its own project's main.
    let (our_branch, their_branch) = (real.place(ours).reference(), real.place(theirs).reference());
    let has = |repo: &std::path::Path, reference: &str| {
        let refs = git_in(repo, &["for-each-ref", "--format=%(refname)"]);
        refs.lines().any(|line| line == reference)
    };
    assert!(has(&real.repo, &our_branch) && !has(&real.repo, &their_branch));
    assert!(has(&other, &their_branch) && !has(&other, &our_branch));
    let merged = |repo: &std::path::Path| git_in(repo, &["log", "main", "--format=%s"]);
    let (in_ours, in_theirs) = (merged(&real.repo), merged(&other));
    assert!(
        in_ours.lines().next().is_some_and(|l| l.starts_with("Merge work-1-")),
        "{in_ours}"
    );
    assert!(
        in_theirs.lines().next().is_some_and(|l| l.starts_with("Merge work-2-")),
        "{in_theirs}"
    );
    assert!(!in_ours.contains("Merge work-2-") && !in_theirs.contains("Merge work-1-"));
}

#[tokio::test]
async fn an_item_of_one_project_waits_for_the_item_of_another_it_depends_on() {
    let mut real = Real::open(On::Deliver, Fake::default(), None, "true");
    real.add_project("other");
    let first = real.add_made_of(&[]).await;
    real.project = "other".to_string();
    let second = real.add_made_of(&[ItemNumber(1)]).await;
    real.act(second, Act::Resume);
    // Let on, it waits for the item of the other project and runs nothing.
    assert!(
        matches!(real.decision(second), Decision::Wait { .. }),
        "{:?}",
        real.decision(second)
    );
    assert!(real.finished(second).is_empty());
    real.act(first, Act::Resume);
    real.until(|real| ended(&real.decision(second))).await;
    assert_eq!(real.decision(first), Decision::Ended(End::Merged));
    assert_eq!(real.decision(second), Decision::Ended(End::Merged));
    assert_eq!(real.finished(second), ["V1"]);
}
