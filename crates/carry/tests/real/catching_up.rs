//! The work's branch taking main in as a run of the build, which takes main in, starts.

use carry::ROUNDS;
use decide::run::Failure;
use service::Act;
use work::Back;

use crate::{Fake, On, Real, ended, git_in};

/// A check that fails the first time, moving main by a commit of `other.txt` as it does, and passes after.
const MOVES_MAIN_ONCE: &str = "test -f ../moved || { touch ../moved; \
    r=$(dirname \"$(git rev-parse --path-format=absolute --git-common-dir)\"); \
    printf 'Good day.\\n' > \"$r/other.txt\"; git -C \"$r\" add other.txt; \
    git -C \"$r\" -c user.name=Tester -c user.email=tester@example.com commit -q -m 'Main moved'; exit 1; }";

#[tokio::test]
async fn a_run_of_the_build_after_main_moved_takes_it_in_and_is_told_so_once() {
    let mut real = Real::open(On::Acts, Fake::default(), None, MOVES_MAIN_ONCE);
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| ended(&real.decision(work))).await;
    assert_eq!(
        real.finished(work),
        ["start", "build", "check", "build", "check", "merge"]
    );
    let kept = real.work(work);
    let builds: Vec<_> = (kept.records.started.iter())
        .filter(|s| s.cell.as_str() == "build")
        .map(|s| s.run)
        .collect();
    let rounds = real.data.join(ROUNDS).join(work.0.to_string());
    let told = |run: decide::RunId| {
        std::fs::read_to_string(rounds.join(format!("{}-1.message.md", run.0))).expect("the round was told")
    };
    // The first build's branch held main, as the start made it: it is told nothing of it.
    assert!(!told(builds[0]).contains("merged into the work"), "{}", told(builds[0]));
    // Main moved during the first check; the second build took it in before its first round, and was told so.
    let second = told(builds[1]);
    let main = git_in(&real.repo, &["log", "-1", "--format=%H", "--grep=Main moved", "main"]);
    let said = format!("main at {main} was merged into the work's branch as the commit ");
    let at = second.find(&said).unwrap_or_else(|| panic!("{second}")) + said.len();
    let merge = &second[at..at + 40];
    let shows = format!("`git diff --name-only {merge}^1 {merge}` shows what it changed.");
    assert!(second.contains(&shows), "{second}");
    // The command shows what main changed.
    let changed = git_in(&real.repo, &["diff", "--name-only", &format!("{merge}^1"), merge]);
    assert_eq!(changed, "other.txt");
    // What came of it is kept for the run, once.
    for run in &builds {
        assert!(rounds.join(format!("{}.main.json", run.0)).is_file());
    }
    // The check after the run took main in, main not having moved, took nothing in of its own.
    let check = real.run_of(work, "check");
    let finished = (kept.records.finished.iter())
        .find(|f| f.run == check)
        .expect("the check finished");
    let passed = &kept.contents[&("passed".into(), finished.at)];
    assert!(!passed.contains("for the check"), "{passed}");
    // The branch holds the engine's merge of main, and the work merged it.
    let place = real.place(work);
    let log = git_in(&real.repo, &["log", &place.reference(), "--format=%s %ae"]);
    assert!(
        log.lines()
            .any(|l| l.starts_with("Merge main at ") && l.contains(" as run ") && l.ends_with(" wf-agents@localhost")),
        "{log}"
    );
}

/// A check that passes, moving main by a commit the first time, as another work's merge would: what it passed does not
/// hold main's new head.
const PASSES_MOVING_MAIN_ONCE: &str = "test -f ../moved || { touch ../moved; \
    r=$(dirname \"$(git rev-parse --path-format=absolute --git-common-dir)\"); \
    git -C \"$r\" -c user.name=Tester -c user.email=tester@example.com commit -q --allow-empty -m 'Main moved'; }";

// A merge refused because main moved since the check passed fails in passing, and the work is not paused: once the
// watch of main writes main's new head, the check runs again on it, and the merge, tried again, merges.
#[tokio::test]
async fn a_merge_refused_for_a_main_that_moved_is_checked_again_on_its_new_head_and_merges_unpaused() {
    let mut real = Real::open(On::Acts, Fake::default(), None, PASSES_MOVING_MAIN_ONCE);
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| real.failures == 1).await;
    assert!(!real.paused(work), "the work is paused");
    let merge = real.run_of(work, "merge");
    let kept = real.work(work);
    let Some(Back::Failed { failure, why, .. }) = kept.back.get(&(merge, 1)) else {
        panic!("the merge did not fail: {:?}", kept.back);
    };
    assert_eq!(*failure, Failure::Passing);
    assert!(why.contains("has moved to"), "{why}");
    let main = git_in(&real.repo, &["rev-parse", "main"]);
    real.outside("main", &format!("Main is at {main}."));
    real.until(|real| ended(&real.decision(work))).await;
    assert_eq!(real.finished(work), ["start", "build", "check", "check", "merge"]);
    assert!(!real.paused(work));
    assert_eq!(real.failures, 1, "the merge failed once");
}
