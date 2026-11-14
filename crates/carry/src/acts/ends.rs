//! What the acts do around a work's end and its checks: letting go of what they made for a work that ended, what the
//! work's pull request says of it, and writing what the host says beside its verdict.

use std::sync::Arc;

use decide::{Outsider, Written};
use store::{First, Item, ItemNumber};
use work::Time;

use super::*;
use crate::git::tests::{Repo, TAG, commit, run};
use crate::host::fake::{Asked, Fake};

const WORK: WorkNumber = WorkNumber(1);

/// The acts on the fake host in the repository.
fn acts(repo: &Repo, host: Fake) -> Acts<Fake> {
    Acts::new(
        host,
        repo.project.clone(),
        repo.data.clone(),
        TAG.to_string(),
        super::words(),
    )
}

/// The work, which ended so, as its release is given.
fn release(end: End) -> Release {
    let flow = Arc::new(acts_flow());
    Release {
        work: WORK,
        end,
        flow,
        item: Item::new(
            ItemNumber(1),
            Time(0),
            First {
                title: "Greet the user".to_string(),
                level: None,
                project: None,
                depends_on: Vec::new(),
                tags: Vec::new(),
            },
            Vec::new(),
        ),
        kept: Arc::new(Work::default()),
    }
}

/// The flow of the end-to-end tests, which has every act.
fn acts_flow() -> Compiled {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/real/flow");
    let source = flow::read(&dir).unwrap();
    let names = |names: &[&str]| names.iter().map(|n| n.to_string()).collect();
    let has = flow::Has {
        sources: names(&["tree", "main"]),
        acts: names(&["start", "check", "merge"]),
        ..Default::default()
    };
    flow::compile(&source, &has).unwrap()
}

/// Starts the work in the repository and commits on its branch.
async fn started(repo: &Repo) {
    let main = run(&repo.project.repo, &["rev-parse", "main"]);
    let place = repo.place(WORK);
    let branch = Branch {
        work: WORK,
        title: "Greet the user",
        about: "",
        place: &place,
    };
    let started = start(&repo.project, &Fake::new(&main), &branch, &Mutex::new(()))
        .await
        .expect("the work starts");
    commit(&started.place.dir, "farewell.txt", "Bye.\n");
}

#[tokio::test]
async fn a_merged_work_has_its_worktrees_taken_away_its_branch_kept_and_the_host_told_it_merged() {
    let repo = Repo::new("true");
    started(&repo).await;
    let place = repo.place(WORK);
    let main = run(&repo.project.repo, &["rev-parse", "main"]);
    let head = run(&repo.project.repo, &["rev-parse", &place.reference()]);
    let acts = acts(&repo, Fake::new(&main));
    acts.inner.release(&release(End::Merged)).await.expect("it lets go");
    assert!(!place.dir.exists(), "the worktree is taken away");
    let listed = run(&repo.project.repo, &["worktree", "list", "--porcelain"]);
    assert!(!listed.contains(&place.dir.display().to_string()), "{listed}");
    assert_eq!(
        run(&repo.project.repo, &["rev-parse", &place.reference()]),
        head,
        "the branch stays"
    );
    assert_eq!(
        acts.inner.host.asked(),
        [Asked::End(place.branch.clone(), true)],
        "the host lets go knowing the work merged, and closes no pull request"
    );
    // Given again, as after the engine started again, it finds it done.
    acts.inner.release(&release(End::Merged)).await.expect("it lets go");
}

#[tokio::test]
async fn a_cancelled_work_has_what_the_host_holds_closed_and_its_worktree_taken_away_uncommitted_and_all() {
    let repo = Repo::new("true");
    started(&repo).await;
    let place = repo.place(WORK);
    std::fs::write(place.dir.join("draft.txt"), "Not yet.\n").unwrap();
    let main = run(&repo.project.repo, &["rev-parse", "main"]);
    let acts = acts(&repo, Fake::new(&main));
    acts.inner.release(&release(End::Cancelled)).await.expect("it lets go");
    assert_eq!(acts.inner.host.asked(), [Asked::End(place.branch.clone(), false)]);
    assert!(!place.dir.exists());
    run(&repo.project.repo, &["rev-parse", "--verify", &place.reference()]);
}

#[tokio::test]
async fn a_work_that_never_started_has_nothing_to_take_away() {
    let repo = Repo::new("true");
    let main = run(&repo.project.repo, &["rev-parse", "main"]);
    let acts = acts(&repo, Fake::new(&main));
    acts.inner.release(&release(End::Cancelled)).await.expect("it lets go");
    assert_eq!(acts.inner.host.asked(), [Asked::End(repo.place(WORK).branch, false)]);
}

#[tokio::test]
async fn letting_go_that_fails_is_said_once_for_the_owner_and_not_tried_again() {
    let repo = Repo::new("true");
    started(&repo).await;
    let main = run(&repo.project.repo, &["rev-parse", "main"]);
    let host = Fake::new(&main);
    host.ending(Some("GitHub cannot be reached"));
    let acts = acts(&repo, host);
    let tried = acts.inner.release(&release(End::Cancelled)).await;
    assert_eq!(
        tried,
        Err("GitHub cannot be reached".to_string()),
        "what failed is said"
    );
    let tries = (acts.inner.host.asked().iter())
        .filter(|a| matches!(a, Asked::End(..)))
        .count();
    assert_eq!(tries, 1);
    assert!(
        repo.place(WORK).dir.exists(),
        "nothing is taken away before the host let go"
    );
}

/// Starts another work in the repository, with a change not committed in its worktree.
async fn another(repo: &Repo) -> git::Place {
    let main = run(&repo.project.repo, &["rev-parse", "main"]);
    let other = repo.place(WorkNumber(2));
    let branch = Branch {
        work: WorkNumber(2),
        title: "Say goodbye",
        about: "",
        place: &other,
    };
    start(&repo.project, &Fake::new(&main), &branch, &Mutex::new(()))
        .await
        .expect("it starts");
    std::fs::write(other.dir.join("draft.txt"), "Not yet.\n").unwrap();
    other
}

/// Whether the other work's worktree is there as it was, its change not committed included.
fn intact(repo: &Repo, other: &git::Place) -> bool {
    let listed = run(&repo.project.repo, &["worktree", "list", "--porcelain"]);
    other.dir.join("draft.txt").is_file() && listed.contains(&other.dir.display().to_string())
}

#[tokio::test]
async fn a_works_place_that_is_a_symbolic_link_is_left_and_the_end_goes_on() {
    let repo = Repo::new("true");
    let other = another(&repo).await;
    let place = repo.place(WORK);
    std::os::unix::fs::symlink(&other.dir, &place.dir).unwrap();
    let main = run(&repo.project.repo, &["rev-parse", "main"]);
    let acts = acts(&repo, Fake::new(&main));
    let done = tokio::time::timeout(Duration::from_secs(10), acts.inner.release(&release(End::Cancelled))).await;
    assert!(done.is_ok(), "the end is not held for ever by the link");
    assert!(intact(&repo, &other), "another work's worktree was taken away");
}

/// A work whose people wrote these blocks, in order.
fn written(blocks: &[(&str, &str)]) -> Work {
    let mut work = Work::default();
    for (block, content) in blocks {
        let at = work.tick();
        work.records.written.push(Written {
            block: (*block).into(),
            by: Outsider::Person,
            matters: true,
            value: None,
            at,
        });
        work.contents.insert(((*block).into(), at), (*content).to_string());
    }
    work
}

#[test]
fn the_work_says_of_itself_the_latest_of_each_block_the_cell_names_in_its_order() {
    let work = written(&[
        ("code", "Greets the user."),
        ("tests", "One test."),
        ("code", "Greets the user by name."),
        ("other", "Not named."),
    ]);
    let named: Vec<BlockId> = vec!["code".into(), "tests".into(), "missing".into()];
    assert_eq!(described(&named, &work), "Greets the user by name.\n\nOne test.");
    assert_eq!(described(&[], &work), "");
}

#[test]
fn the_work_says_of_itself_what_the_flows_check_names_for_the_acts_and_the_tools_alike() {
    let work = written(&[("code", "Greets the user by name."), ("request", "Greet them.")]);
    assert_eq!(about(&acts_flow(), &work), "Greets the user by name.");
}

#[test]
fn what_the_host_says_beside_its_verdict_is_written_with_the_check() {
    let checked = Checked {
        commit: "c0ffee".to_string(),
        main: "beef".to_string(),
        failed: None,
        by: None,
        took_in: false,
        notes: vec!["test: its latest run on this commit passed; its earlier runs on it: failure.".to_string()],
        asks: None,
    };
    let outputs = vec![
        ("passed".into(), Some(vec!["yes".to_string(), "no".to_string()])),
        ("failures".into(), None),
    ];
    let handed = checked_in(&outputs, &checked, "main", &super::words().check).unwrap();
    assert_eq!(
        handed[0].content,
        "Checked commit c0ffee\n\nThe work's branch was checked against main at beef: it passed.\n\nAlso:\n- test: \
         its latest run on this commit passed; its earlier runs on it: failure."
    );
}
