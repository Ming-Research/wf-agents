//! What a round of a line that may change the work leaves in the worktree: as it ends with a return that asks for no
//! call, the return is sent back while a file git tracks is changed and not committed, or a merge begun in the worktree
//! is not committed, so that its agent puts it right while it is there. A file git does not track holds nothing back,
//! and neither does a round that asks for a call or one of a line that only reads.

use decide::run::Unmet;
use decide::{Decision, End, RunId};
use service::Act;
use work::Given;

use crate::{Fake, On, Real, ended, git_in};

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
    assert_eq!(
        kept.given.get(&(results, 3)),
        None,
        "the round that took the change back left nothing"
    );
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

// The item's own case, end to end on the flow with every act: a command left a file in the worktree and the build
// committed none of it. Reference: the item's background (a cache left by a local command, in no commit, held a work
// before its merge for good); a work is done when it merges, whatever such a file is. The project's check judges the
// commit that will merge, so a file in no commit is not there for it, and the merge carries none of it.
#[tokio::test]
async fn a_cache_left_in_the_worktree_stops_neither_the_check_nor_the_merge_and_is_in_neither() {
    let fake = Fake {
        build_leaves_a_file: true,
        ..Fake::default()
    };
    let mut real = Real::open(On::Acts, fake, None, "test ! -e cache.txt && test -e greeting.txt");
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| ended(&real.decision(work)) || real.failures > 0 || real.holds > 0)
        .await;
    assert_eq!(real.decision(work), Decision::Ended(End::Merged));
    assert_eq!(real.finished(work), ["start", "build", "check", "merge"]);
    // What the work held is let go once it has ended, as the engine's own effect: the worktrees are looked at after.
    real.until(|real| real.work(work).records.released().is_some()).await;
    let tree = git_in(&real.repo, &["ls-tree", "-r", "--name-only", "main"]);
    assert!(!tree.lines().any(|line| line == "cache.txt"), "{tree}");
    let worktrees = git_in(&real.repo, &["worktree", "list", "--porcelain"]);
    assert_eq!(
        worktrees.lines().filter(|line| line.starts_with("worktree ")).count(),
        1,
        "no checkout of the work or of its check is left:\n{worktrees}"
    );
}

// Reference: git's `worktree remove --force` fails on a directory that has no write permission, which a command run
// by the check may leave (a cache made read-only is one). The item is that what a command left on the machine must
// not hold a work before its merge: the check's checkout is the engine's own and is taken away whatever the command
// left in it, and the work, which has nothing more to do to put it right, is not held by it.
#[tokio::test]
async fn a_directory_the_check_command_leaves_read_only_does_not_hold_the_work_before_its_merge() {
    let mut real = Real::open(
        On::Acts,
        Fake::default(),
        None,
        "mkdir sealed && touch sealed/entry && chmod a-w sealed",
    );
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| ended(&real.decision(work)) || real.failures > 0 || real.holds > 0)
        .await;
    assert_eq!(real.decision(work), Decision::Ended(End::Merged));
}
