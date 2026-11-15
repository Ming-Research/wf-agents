//! What the acts do around a work's end and its checks: letting go of what they made for a work that ended, what the
//! work's pull request says of it, and writing what the host says beside its verdict.

use std::sync::Arc;

use decide::{Outsider, Written};
use store::{First, Item, ItemNumber};
use work::Time;

use super::*;
use crate::git::tests::{Repo, TAG, commit, run};
use crate::host::Verdict;
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

/// The work's item, of this title.
fn item(title: &str) -> Item {
    let first = First {
        title: title.to_string(),
        level: None,
        project: None,
        depends_on: Vec::new(),
        tags: Vec::new(),
    };
    Item::new(ItemNumber(1), Time(0), first, Vec::new())
}

/// The work, which ended so, as its release is given.
fn release(end: End) -> Release {
    let flow = Arc::new(acts_flow());
    Release {
        work: WORK,
        end,
        flow,
        item: item("Greet the user"),
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
            said_to: None,
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
fn the_merge_is_named_by_the_first_line_of_the_latest_name_the_work_wrote_and_else_by_its_number() {
    let commits = super::writes().commits;
    let flow = acts_flow();
    let named = |flow: &Compiled, work: &Work| name(flow, work, WORK, &commits);
    assert_eq!(named(&flow, &Work::default()), "Work 1");
    assert_eq!(named(&flow, &written(&[("code", "Greets the user.")])), "Work 1");
    let work = written(&[
        ("title", "Greet the user"),
        ("title", "\n  Greet the user by name  \nIt greets them."),
    ]);
    assert_eq!(named(&flow, &work), "Greet the user by name");
    // A flow whose merge is named by no block names it by the work's number, whatever the work wrote.
    let mut unnamed = flow.clone();
    for cell in &mut unnamed.cells {
        cell.named_by = None;
    }
    assert_eq!(named(&unnamed, &work), "Work 1");
}

// The host is given the name of the work's merge, the line the work wrote for it or, where it wrote none, the work's
// number, and never the title of its item, which is in the owner's language. Its verdict is asked for by the round of
// the check, which it finds what it ran for again by after the engine started again.
#[tokio::test]
async fn the_host_is_asked_by_the_checks_round_and_given_the_name_of_the_works_merge_never_its_items_title() {
    let owners = "向用户问好";
    let repo = Repo::new("true");
    started(&repo).await;
    let main = run(&repo.project.repo, &["rev-parse", "main"]);
    let head = run(&repo.project.repo, &["rev-parse", &repo.place(WORK).reference()]);
    let passes = Verdict {
        failed: None,
        held: None,
        by: None,
        notes: Vec::new(),
        observations: Vec::new(),
    };
    let acts = acts(&repo, Fake::new(&main).judges(Ok(Some(passes))));
    let code = format!("Greets the user, in commit {head}.");
    let kept = written(&[("code", code.as_str()), ("title", "Greet the user by name")]);
    let round = checking(item(owners), kept);
    let (to, _told) = tokio::sync::mpsc::unbounded_channel();
    let teller = RoundReply::new(WORK, RunId(4), to).teller();
    let checked = acts.inner.act(&round, &teller, &mut Stop::never()).await;
    assert!(
        matches!(checked, Ok(Done::HandsIn(..))),
        "the check hands in what it found"
    );
    let ended = Release {
        item: item(owners),
        ..release(End::Cancelled)
    };
    acts.inner.release(&ended).await.expect("it lets go");
    assert_eq!(acts.inner.host.titles(), ["Greet the user by name", "Work 1"]);
    assert_eq!(acts.inner.host.askings(), [Asking::Round(RunId(4), 2)]);
}

/// The second round of the check's run 4 of the work as it is kept.
fn checking(item: Item, kept: Work) -> Round {
    let flow = Arc::new(acts_flow());
    let check = (flow.cells.iter()).find(|cell| cell.act.as_deref() == Some("check"));
    Round {
        work: WORK,
        flow: flow.clone(),
        cell: check.expect("the flow has a check").clone(),
        run: RunId(4),
        number: 2,
        given: work::Given::Task,
        item,
        kept: Arc::new(kept),
        lines: Default::default(),
    }
}

// The check's round is told of the head of main the check takes in, in the words the watch of main writes it, as the
// check reads it: main's block at that head is then what the round read, and runs the check no more. A check that
// waited for its turn while main moved, and took that main in, was run again for it (work 5).
#[tokio::test]
async fn the_checks_round_is_told_of_the_head_of_main_it_takes_in() {
    let repo = Repo::new("true");
    started(&repo).await;
    let head = run(&repo.project.repo, &["rev-parse", &repo.place(WORK).reference()]);
    let main = commit(&repo.project.repo, "weather.txt", "Sunny.\n");
    let passes = Verdict {
        failed: None,
        held: None,
        by: None,
        notes: Vec::new(),
        observations: Vec::new(),
    };
    let acts = acts(&repo, Fake::new(&main).judges(Ok(Some(passes))));
    let code = format!("Greets the user, in commit {head}.");
    let round = checking(item("Greet the user"), written(&[("code", code.as_str())]));
    let (to, mut told) = tokio::sync::mpsc::unbounded_channel();
    let teller = RoundReply::new(WORK, RunId(4), to).teller();
    let checked = acts.inner.act(&round, &teller, &mut Stop::never()).await;
    assert!(
        matches!(checked, Ok(Done::HandsIn(..))),
        "the check hands in what it found"
    );
    let mut said = Vec::new();
    while let Ok(message) = told.try_recv() {
        if let service::Message::Happened {
            event: engine::Event::WroteForRound { run, block, content },
            ..
        } = message
        {
            said.push((run, block.to_string(), content));
        }
    }
    assert_eq!(said, [(RunId(4), "main".to_string(), format!("Main is at {main}."))]);
}

// A verdict that holds the work, what failed failing on main as well, writes the check's choice alone, saying so, and
// none of the blocks a failure is written into, so that no cell is told of a failure that is not the work's; and what
// the host found beyond the work is handed in as the check's observations. Well under a second alone: a repository on
// disk and the fake host; no other test holds what a held check hands in.
#[tokio::test]
async fn a_held_check_writes_its_choice_alone_and_hands_in_what_the_host_observed() {
    let repo = Repo::new("true");
    started(&repo).await;
    let main = run(&repo.project.repo, &["rev-parse", "main"]);
    let head = run(&repo.project.repo, &["rev-parse", &repo.place(WORK).reference()]);
    let held = Verdict {
        held: Some("Failed on main as well: `b`.".to_string()),
        observations: vec!["The test `a` may be flaky.".to_string()],
        ..Verdict::default()
    };
    let acts = acts(&repo, Fake::new(&main).judges(Ok(Some(held))));
    let code = format!("Greets the user, in commit {head}.");
    let round = checking(item("Greet the user"), written(&[("code", code.as_str())]));
    let (to, mut told) = tokio::sync::mpsc::unbounded_channel();
    acts.give(round, RoundReply::new(WORK, RunId(4), to));
    let came = loop {
        if let service::Message::Happened {
            event: engine::Event::Returned { came, .. },
            ..
        } = told.recv().await.expect("the round ends")
        {
            break came;
        }
    };
    let written: Vec<(String, Option<String>)> = (came.writes.iter())
        .map(|h| (h.block.to_string(), h.value.clone()))
        .collect();
    assert_eq!(written, [("passed".to_string(), Some("no".to_string()))]);
    let content = &came.writes[0].content;
    assert!(
        content.contains("which the work's change is not what fails: the work waits for main to pass it")
            && content.ends_with("Failed on main as well: `b`."),
        "{content}"
    );
    assert_eq!(came.said.observations, ["The test `a` may be flaky."]);
}

#[test]
fn what_the_host_says_beside_its_verdict_is_written_with_the_check() {
    let checked = Checked {
        commit: "c0ffee".to_string(),
        main: "beef".to_string(),
        failed: None,
        held: None,
        by: None,
        took_in: false,
        notes: vec!["test: its latest run on this commit passed; its earlier runs on it: failure.".to_string()],
        observations: Vec::new(),
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
