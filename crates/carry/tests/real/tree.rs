//! The design tree on the work's branch as a block of the work: a version where the start made the branch, where the
//! branch took main in, and where a round of the work committed to the tree.

use service::Act;
use store::WorkNumber;

use crate::{Fake, On, Real, ended, git_in};

/// A check that fails the first time, moving main by a commit of a file of the design tree as it does, and passes after.
const MOVES_DESIGN_ONCE: &str = "test -f ../moved || { touch ../moved; \
    r=$(dirname \"$(git rev-parse --path-format=absolute --git-common-dir)\"); \
    printf 'Good day.\\n' > \"$r/design/other.md\"; git -C \"$r\" add design/other.md; \
    git -C \"$r\" -c user.name=Tester -c user.email=tester@example.com commit -q -m 'Main moved'; exit 1; }";

/// Each version of the tree block of the work, as its content: the commit and the id of the tree.
fn versions(real: &Real, work: WorkNumber) -> Vec<(String, String)> {
    let kept = real.work(work);
    let block = decide::BlockId::from("tree");
    let written = kept.records.written.iter().filter(|w| w.block == block);
    written
        .map(|w| {
            let content = &kept.contents[&(block.clone(), w.at)];
            let (commit, tree) = content.split_once(' ').expect("a commit and a tree");
            (commit.to_string(), tree.to_string())
        })
        .collect()
}

/// The subject of the commit and the number of its parents.
fn commit(real: &Real, commit: &str) -> (String, usize) {
    let subject = git_in(&real.repo, &["log", "-1", "--format=%s", commit]);
    let parents = git_in(&real.repo, &["log", "-1", "--format=%P", commit]);
    (subject, parents.split_whitespace().count())
}

#[tokio::test]
async fn the_start_and_a_round_that_commits_to_the_design_tree_each_make_a_version() {
    let fake = Fake {
        design: true,
        ..Fake::default()
    };
    let mut real = Real::open(On::Acts, fake, None, "true");
    let main = git_in(&real.repo, &["rev-parse", "main"]);
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| ended(&real.decision(work))).await;
    let versions = versions(&real, work);
    assert_eq!(versions.len(), 2, "{versions:?}");
    // The start made the branch from main: the first version is main's, before any round of an agent.
    assert_eq!(versions[0].0, main);
    let tree = git_in(&real.repo, &["rev-parse", &format!("{main}:design")]);
    assert_eq!(versions[0].1, tree);
    // The build committed to the tree: the second is that commit's.
    assert_eq!(commit(&real, &versions[1].0), ("Build the greeting".to_string(), 1));
    assert_ne!(versions[1].1, tree);
}

#[tokio::test]
async fn the_branch_taking_main_in_as_a_run_starts_makes_a_version_before_the_round_ends() {
    let mut real = Real::open(On::Acts, Fake::default(), None, MOVES_DESIGN_ONCE);
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| ended(&real.decision(work))).await;
    let versions = versions(&real, work);
    assert_eq!(versions.len(), 2, "{versions:?}");
    // The second build committed nothing of the tree: the version is the merge of the catch-up, which it made as the
    // run started, and not the build's own commit that ended the round.
    let (subject, parents) = commit(&real, &versions[1].0);
    assert!(
        subject.starts_with("Merge main at ") && subject.contains(" as run "),
        "{subject}"
    );
    assert_eq!(parents, 2);
    // The run took the version in as it began, and was told of it: it is put into the round of its run.
    let kept = real.work(work);
    let second = kept
        .records
        .started
        .iter()
        .filter(|s| s.cell.as_str() == "build")
        .nth(1);
    let run = second.expect("a second build").run;
    let put = kept.records.put_in.iter().filter(|p| p.run == run);
    assert_eq!(put.map(|p| p.block.as_str()).collect::<Vec<_>>(), ["tree"]);
}

#[tokio::test]
async fn the_check_that_takes_main_in_makes_a_version() {
    let fake = Fake {
        moves_main: true,
        ..Fake::default()
    };
    let mut real = Real::open(On::Acts, fake, None, "true");
    let work = real.add().await;
    real.act(work, Act::Resume);
    real.until(|real| ended(&real.decision(work))).await;
    let versions = versions(&real, work);
    assert_eq!(versions.len(), 2, "{versions:?}");
    // The build moved main after it took main in, and committed nothing of the tree; the check merged main.
    let (subject, parents) = commit(&real, &versions[1].0);
    assert!(
        subject.starts_with("Merge main at ") && !subject.contains(" as run "),
        "{subject}"
    );
    assert_eq!(parents, 2);
}
