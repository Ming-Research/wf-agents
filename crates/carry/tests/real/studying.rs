//! A study, on the study flow as the repository defines it (`flow/study`), with the real carriers: the start's act makes
//! its branch and worktree, its two agents read in the worktree, the first naming the entries of the design tree that
//! bear on the item and the second handing in a conclusion, and the act that concludes the study ends the work, with
//! nothing merged. What the engine does for a work that ends is let go of what it made for it: the worktree is taken
//! away, and main is as it was.

use decide::{BlockId, Decision, End};
use service::Act;
use store::WorkNumber;

use crate::{Fake, GREETING_ENTRY, GREETING_FILE, On, Real, ended, git_in};

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
async fn a_study_ends_with_its_conclusion_leaving_main_as_it_was_and_its_worktree_taken_away() {
    let mut real = Real::open(On::Study, Fake::default(), None, "true");
    // The design tree holds the entry the first agent names.
    let path = real.repo.join(GREETING_FILE);
    std::fs::create_dir_all(path.parent().expect("a directory")).unwrap();
    std::fs::write(
        &path,
        format!(
            "# Greeting\n\nHow the work greets.\n\n### {GREETING_ENTRY}\nGreet by name.\n- rejected: the owner proposed a plain hello; the owner rejected it, because it names no one.\n"
        ),
    )
    .unwrap();
    git_in(&real.repo, &["add", GREETING_FILE]);
    git_in(&real.repo, &["commit", "-q", "-m", "Decide the greeting"]);
    let main = git_in(&real.repo, &["rev-parse", "main"]);
    let worktrees = git_in(&real.repo, &["worktree", "list", "--porcelain"]);
    let work = real.add().await;
    real.act(
        work,
        Act::Write {
            block: "background".into(),
            content: "What is to be studied.".to_string(),
            value: None,
        },
    );
    real.act(work, Act::Resume);
    real.until(|real| ended(&real.decision(work))).await;
    assert_eq!(real.decision(work), Decision::Ended(End::Merged));
    assert_eq!(real.finished(work), ["S1", "R0", "R1", "C1"]);
    assert_eq!(latest(&real, work, "tree_entries"), GREETING_ENTRY);
    assert_eq!(
        latest(&real, work, "conclusion"),
        "What was found, and the items to make."
    );
    assert_eq!(latest(&real, work, "concluded"), "The study is concluded.");
    // What the work held outside the engine is let go, and the repository is as it was: no merge, so main is where it
    // was, and the worktree the start made is taken away.
    real.until(|real| real.work(work).records.released().is_some()).await;
    assert_eq!(git_in(&real.repo, &["rev-parse", "main"]), main);
    assert_eq!(git_in(&real.repo, &["worktree", "list", "--porcelain"]), worktrees);
    // The two agents ran, and the study's, which only reads, is told so and refused what edits a file.
    let calls = real.calls();
    assert_eq!(calls.len(), 2, "one call for each agent cell");
    let study = calls.iter().find(|call| call.of("Study")).expect("the study ran");
    assert!(study.args.iter().any(|arg| arg.contains("You only read it")));
    assert!(study.args.iter().any(|arg| arg == "Edit"));
}
