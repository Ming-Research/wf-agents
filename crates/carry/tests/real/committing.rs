//! What a round leaves in the work's worktree not committed: the return that hands in its cell's work is sent back,
//! saying what git lists, so that the check before the merge never meets it, since nothing it leaves there would run
//! the check again once taken away.

use service::Act;
use work::Given;

use crate::{Fake, On, Real, ended};

#[tokio::test]
async fn a_build_that_leaves_a_file_not_committed_is_sent_back_and_the_check_never_meets_it() {
    let fake = Fake {
        build_leaves_a_file: true,
        ..Fake::default()
    };
    let mut real = Real::open(On::Acts, fake, None, "true");
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| ended(&real.decision(work))).await;
    // The check ran once, on a worktree holding nothing that is not committed, and passed.
    assert_eq!(real.finished(work), ["start", "build", "check", "merge"]);
    let kept = real.work(work);
    let build = real.run_of(work, "build");
    let sent_back = kept.given.get(&(build, 2));
    assert!(matches!(sent_back, Some(Given::SentBack(_))), "{sent_back:?}");
    let calls = real.calls();
    let builds: Vec<_> = calls.iter().filter(|call| call.of("Build")).collect();
    assert_eq!(builds.len(), 2);
    let told = "- The worktree holds changes that are not committed: commit what belongs to the work, and take away the \
                rest.\n?? left.txt";
    assert!(builds[1].stdin.contains(told), "{}", builds[1].stdin);
}
