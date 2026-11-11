//! What the acts do around a work's end and its checks: letting go of what they made for a work that ended, what the
//! work's pull request says of it, telling the host what an earlier check of the same commit found, and writing what
//! failed and was not the change's.

use std::sync::Arc;

use decide::{Finished, Outsider, Seq, Started, Write, Written};
use store::{Item, ItemNumber};
use work::Time;

use super::*;
use crate::git::tests::{Repo, TAG, commit, run};
use crate::host::fake::{Asked, Fake};
use crate::host::{Local, Verdict};

const WORK: WorkNumber = WorkNumber(1);

/// The acts on the fake host in the repository, letting go again within milliseconds where it failed.
fn acts(repo: &Repo, host: Fake) -> Acts<Fake> {
    let acts = Acts::new(host, repo.project.clone(), repo.data.clone(), TAG.to_string());
    let mut inner = Arc::into_inner(acts.inner).expect("held once");
    inner.release_wait = Duration::from_millis(20);
    Acts { inner: Arc::new(inner) }
}

/// The work, which ended so, as its release is given.
fn release(end: End) -> Release {
    let flow = Arc::new(acts_flow());
    Release {
        work: WORK,
        end,
        flow,
        item: Item {
            number: ItemNumber(1),
            title: "Greet the user".to_string(),
            added: Time(0),
            depends_on: Vec::new(),
            level: None,
        },
        kept: Arc::new(Work::default()),
    }
}

/// The flow of the end-to-end tests, which has every act.
fn acts_flow() -> Compiled {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/real/flow");
    let source = flow::read(&dir).unwrap();
    let names = |names: &[&str]| names.iter().map(|n| n.to_string()).collect();
    let has = flow::Has {
        sources: names(&[]),
        acts: names(&["start", "check", "merge"]),
    };
    flow::compile(&source, &has).unwrap()
}

/// Starts the work in the repository and commits on its branch.
async fn started(repo: &Repo) {
    let main = run(&repo.project.repo, &["rev-parse", "main"]);
    let started = start(&repo.project, &Fake::new(&main), &repo.place(WORK), &Mutex::new(()))
        .await
        .expect("the work starts");
    commit(&started.place.dir, "farewell.txt", "Bye.\n");
}

#[tokio::test]
async fn a_merged_work_has_its_worktrees_taken_away_its_branch_kept_and_the_host_told_it_merged() {
    let repo = Repo::new("true");
    started(&repo).await;
    let place = repo.place(WORK);
    // A check before the change left its worktree, as by an engine that ended while it ran.
    let main = run(&repo.project.repo, &["rev-parse", "main"]);
    let left = git::check_out_before(&repo.project, &place, &main).await.unwrap();
    let head = run(&repo.project.repo, &["rev-parse", &place.reference()]);
    let acts = acts(&repo, Fake::new(&main));
    acts.inner.release(&release(End::Merged)).await;
    assert!(!place.dir.exists(), "the worktree is taken away");
    assert!(
        !left.exists(),
        "the worktree a check before the change left is taken away"
    );
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
    acts.inner.release(&release(End::Merged)).await;
}

#[tokio::test]
async fn a_cancelled_work_has_what_the_host_holds_closed_and_its_worktree_taken_away_uncommitted_and_all() {
    let repo = Repo::new("true");
    started(&repo).await;
    let place = repo.place(WORK);
    std::fs::write(place.dir.join("draft.txt"), "Not yet.\n").unwrap();
    let main = run(&repo.project.repo, &["rev-parse", "main"]);
    let acts = acts(&repo, Fake::new(&main));
    acts.inner.release(&release(End::Cancelled)).await;
    assert_eq!(acts.inner.host.asked(), [Asked::End(place.branch.clone(), false)]);
    assert!(!place.dir.exists());
    run(&repo.project.repo, &["rev-parse", "--verify", &place.reference()]);
}

#[tokio::test]
async fn a_work_that_never_started_has_nothing_to_take_away() {
    let repo = Repo::new("true");
    let main = run(&repo.project.repo, &["rev-parse", "main"]);
    let acts = acts(&repo, Fake::new(&main));
    acts.inner.release(&release(End::Cancelled)).await;
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
    let tried = tokio::time::timeout(Duration::from_secs(2), acts.inner.release(&release(End::Cancelled))).await;
    assert!(tried.is_ok(), "it is not tried again unseen");
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
    start(&repo.project, &Fake::new(&main), &other, &Mutex::new(()))
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
async fn a_symbolic_link_among_a_works_worktrees_takes_away_no_other_works() {
    let repo = Repo::new("true");
    started(&repo).await;
    let other = another(&repo).await;
    let place = repo.place(WORK);
    let link = git::before_dir(&place, "0123456789ab");
    std::os::unix::fs::symlink(&other.dir, &link).unwrap();
    let main = run(&repo.project.repo, &["rev-parse", "main"]);
    let acts = acts(&repo, Fake::new(&main));
    let done = tokio::time::timeout(Duration::from_secs(10), acts.inner.release(&release(End::Merged))).await;
    assert!(done.is_ok());
    assert!(intact(&repo, &other), "another work's worktree was taken away");
    assert!(!place.dir.exists(), "the work's own worktree is taken away");
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

/// A work whose check finished once, writing this in its choice and this in its other output.
fn checked_once(value: &str, choice: &str, other: &str) -> Work {
    let mut work = Work::default();
    let run = decide::RunId(1);
    work.records.started.push(Started {
        run,
        cell: "check".into(),
        at: Seq(1),
        because: Vec::new(),
        basis: Default::default(),
    });
    let writes = vec![
        Write {
            block: "passed".into(),
            matters: true,
            value: Some(value.to_string()),
        },
        Write {
            block: "failures".into(),
            matters: true,
            value: None,
        },
    ];
    work.records.finished.push(Finished {
        run,
        at: Seq(2),
        writes,
    });
    work.contents.insert(("passed".into(), Seq(2)), choice.to_string());
    work.contents.insert(("failures".into(), Seq(2)), other.to_string());
    work
}

#[test]
fn the_work_says_of_itself_what_the_flows_check_names_for_the_acts_and_the_tools_alike() {
    let work = written(&[("code", "Greets the user by name."), ("request", "Greet them.")]);
    assert_eq!(about(&acts_flow(), &work), "Greets the user by name.");
}

#[test]
fn what_the_work_was_told_of_failures_is_its_latest_failed_check_and_its_tools_latest_check() {
    let flow = acts_flow();
    let failed = checked_once(
        "no",
        "Checked commit c0ffee\n\nIt did not pass.",
        "test failed at job/7",
    );
    assert_eq!(
        seen(&flow, &failed).as_deref(),
        Some("Checked commit c0ffee\n\nIt did not pass.\n\ntest failed at job/7")
    );
    let passed = checked_once("yes", "Checked commit c0ffee\n\nIt passed.", "");
    assert_eq!(seen(&flow, &passed), None);
    assert_eq!(seen(&flow, &Work::default()), None);
    // The agents' tool's latest answer, beside what the check before the merge wrote.
    let mut told = failed;
    for (call, name, result) in [
        (1, "check", "lint failed at job/3"),
        (2, "read", "blocks"),
        (3, "check", "lint failed at job/4"),
    ] {
        let call = decide::CallId(call);
        let ask = work::Ask::Tool {
            name: name.to_string(),
            input: String::new(),
        };
        told.calls.insert(call, work::Call::Asked(ask));
        told.results.insert(call, result.to_string());
    }
    let said = seen(&flow, &told).expect("told");
    assert!(said.ends_with("\n\nlint failed at job/4"), "{said}");
    assert!(!said.contains("job/3") && !said.contains("blocks"), "{said}");
}

#[tokio::test]
async fn the_host_is_told_what_the_work_was_told_of_failures_before() {
    let repo = Repo::new("true");
    started(&repo).await;
    let place = repo.place(WORK);
    let head = run(&repo.project.repo, &["rev-parse", &place.reference()]);
    let main = run(&repo.project.repo, &["rev-parse", "main"]);
    let passing = || {
        Ok(Some(Verdict {
            failed: None,
            by: None,
            passed_over: Vec::new(),
        }))
    };
    let host = Fake::new(&main).judges(passing()).judges(passing());
    let branch = Branch {
        work: WORK,
        title: "Greet the user",
        about: "",
        place: &place,
    };
    let seen = Some("test failed at job/7");
    check(&repo.project, &host, &branch, seen, &Mutex::new(()), &mut Stop::never())
        .await
        .unwrap();
    check(&repo.project, &host, &branch, None, &Mutex::new(()), &mut Stop::never())
        .await
        .unwrap();
    assert_eq!(
        host.asked(),
        [
            Asked::Main,
            Asked::Seen(head.clone(), "test failed at job/7".to_string()),
            Asked::Main,
            Asked::Verdict(head)
        ]
    );
}

#[test]
fn what_failed_and_was_not_the_changes_is_written_with_the_check() {
    let checked = Checked {
        commit: "c0ffee".to_string(),
        main: "beef".to_string(),
        failed: None,
        by: None,
        took_in: false,
        passed_over: vec!["test failed, as it fails on main at beef: it is not the change's".to_string()],
    };
    let outputs = vec![
        ("passed".into(), Some(vec!["yes".to_string(), "no".to_string()])),
        ("failures".into(), None),
    ];
    let handed = checked_in(&outputs, &checked, "main").unwrap();
    assert_eq!(
        handed[0].content,
        "Checked commit c0ffee\n\nThe work's branch was checked against main at beef: it passed.\n\nFailed, and not the \
         change's:\n- test failed, as it fails on main at beef: it is not the change's"
    );
}

#[tokio::test]
async fn the_local_check_that_passes_on_a_commit_it_failed_before_is_told_apart() {
    let repo = Repo::new("true");
    started(&repo).await;
    let place = repo.place(WORK);
    let branch = Branch {
        work: WORK,
        title: "Greet the user",
        about: "",
        place: &place,
    };
    let host = Local::new(repo.project.clone());
    let verdict = (host
        .verdict(&branch, "c0ffee", Some("it failed"), &mut Stop::never())
        .await)
        .unwrap()
        .unwrap();
    assert_eq!(verdict.failed, None);
    assert_eq!(
        verdict.passed_over,
        [crate::host::now_and_then("The check `true`", "", "")]
    );
    let repo = Repo::new("false");
    started(&repo).await;
    let host = Local::new(repo.project.clone());
    let place = repo.place(WORK);
    let branch = Branch {
        work: WORK,
        title: "Greet the user",
        about: "",
        place: &place,
    };
    let verdict = (host
        .verdict(&branch, "c0ffee", Some("it failed"), &mut Stop::never())
        .await)
        .unwrap()
        .unwrap();
    let failed = verdict.failed.expect("it fails");
    assert!(failed.contains("as it failed on this same commit before"), "{failed}");
}
