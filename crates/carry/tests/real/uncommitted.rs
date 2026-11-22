//! What a round of a line that may change the work leaves in the worktree: as it ends with a return that asks for no
//! call, the return is sent back while a file git tracks is changed and not committed, or a merge begun in the worktree
//! is not committed, so that its agent puts it right while it is there. A file git does not track holds nothing back,
//! and neither does a round that asks for a call or one of a line that only reads.

use decide::RunId;
use decide::run::Unmet;
use service::Act;
use work::Given;

use crate::{Fake, Real, ended};

/// The message of each round of the build, in the order the fake was run.
fn builds(real: &Real) -> Vec<String> {
    let calls = real.calls();
    (calls.iter().filter(|call| call.of("Build")))
        .map(|call| call.stdin.clone())
        .collect()
}

#[tokio::test]
async fn a_build_that_leaves_a_tracked_file_changed_and_not_committed_is_sent_back_until_it_takes_the_change_back() {
    let fake = Fake {
        build_leaves_an_edit: true,
        ..Fake::default()
    };
    let mut real = Real::trial(fake);
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| ended(&real.decision(work))).await;
    let kept = real.work(work);
    let build = real.run_of(work, "build");
    let left = Unmet::Uncommitted {
        changes: " M greeting.txt".to_string(),
    };
    assert_eq!(kept.given.get(&(build, 2)), Some(&Given::SentBack(vec![left])));
    assert_eq!(kept.given.get(&(build, 3)), None, "the second round left nothing");
    let sent_back = "- The worktree has changes to tracked files that are not committed:";
    let builds = builds(&real);
    assert_eq!(builds.len(), 2, "the build ran twice: its return was sent back once");
    assert!(builds[1].contains(sent_back), "{}", builds[1]);
    assert!(builds[1].contains(" M greeting.txt"), "{}", builds[1]);
}

// The cache a command left, or a file the build forgot, is in no commit whoever left it: nothing waits for it and
// nothing is asked about it, and the round's return is taken as it is.
#[tokio::test]
async fn a_build_that_leaves_a_file_git_does_not_track_is_not_sent_back() {
    let fake = Fake {
        build_leaves_a_file: true,
        ..Fake::default()
    };
    let mut real = Real::trial(fake);
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| ended(&real.decision(work))).await;
    let kept = real.work(work);
    let build = real.run_of(work, "build");
    assert_eq!(
        kept.given.get(&(build, 2)),
        None,
        "the build's return was taken as it was"
    );
    assert_eq!(builds(&real).len(), 1);
}

#[tokio::test]
async fn a_build_that_leaves_a_merge_not_committed_is_sent_back_until_it_finishes_or_takes_it_back() {
    let fake = Fake {
        build_leaves_a_merge: true,
        ..Fake::default()
    };
    let mut real = Real::trial(fake);
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| ended(&real.decision(work))).await;
    let kept = real.work(work);
    let build = real.run_of(work, "build");
    let left = Unmet::Merging { changes: String::new() };
    assert_eq!(kept.given.get(&(build, 2)), Some(&Given::SentBack(vec![left])));
    assert_eq!(
        kept.given.get(&(build, 3)),
        None,
        "the second round took the merge back"
    );
    let sent_back = "- A merge is under way in the worktree and is not committed:";
    let builds = builds(&real);
    assert!(builds[1].contains(sent_back), "{}", builds[1]);
}

// A return that asks for a call asks for nothing else, and the run entered for the results goes on in the same
// worktree: what is left is held against the return that does the cell's work, which is the run's second.
#[tokio::test]
async fn a_build_that_asks_for_a_call_with_a_tracked_file_changed_is_held_to_it_only_in_the_return_that_hands_in() {
    let fake = Fake {
        build_then_asks: true,
        build_leaves_an_edit: true,
        ..Fake::default()
    };
    let mut real = Real::trial(fake);
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| ended(&real.decision(work))).await;
    let kept = real.work(work);
    let runs: Vec<RunId> = (kept.records.started.iter())
        .filter(|started| started.cell.as_str() == "build")
        .map(|started| started.run)
        .collect();
    let [asked, results] = runs[..] else {
        panic!("the build ran twice: {runs:?}");
    };
    assert_eq!(kept.given.get(&(asked, 2)), None, "the return that asked was not held");
    assert!(
        matches!(kept.given.get(&(results, 1)), Some(Given::Results(_))),
        "{:?}",
        kept.given.get(&(results, 1))
    );
    let left = Unmet::Uncommitted {
        changes: " M greeting.txt".to_string(),
    };
    assert_eq!(kept.given.get(&(results, 2)), Some(&Given::SentBack(vec![left])));
}

// A line that only reads changes nothing of the work, and a file its round touched is the outside's.
#[tokio::test]
async fn a_review_that_leaves_a_tracked_file_changed_is_not_sent_back() {
    let fake = Fake {
        review_leaves_an_edit: true,
        ..Fake::default()
    };
    let mut real = Real::trial(fake);
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| ended(&real.decision(work))).await;
    let kept = real.work(work);
    let review = real.run_of(work, "review");
    assert_eq!(
        kept.given.get(&(review, 2)),
        None,
        "the review's return was taken as it was"
    );
}
