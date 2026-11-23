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
        continues: None,
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
        continues: None,
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
        continues: None,
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
            empty: false,
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

/// The first round of the start's run 1 of the work, whose item is this.
fn starting(item: Item) -> Round {
    let flow = Arc::new(acts_flow());
    let start = (flow.cells.iter()).find(|cell| cell.act.as_deref() == Some("start"));
    Round {
        work: WORK,
        flow: flow.clone(),
        cell: start.expect("the flow has a start").clone(),
        run: RunId(1),
        number: 1,
        given: work::Given::Task,
        item,
        kept: Arc::new(Work::default()),
        lines: Default::default(),
        account: None,
    }
}

/// What the start act writes for the item, on the host given.
async fn start_wrote(repo: &Repo, host: Fake, item: Item) -> String {
    let acts = acts(repo, host);
    let (to, _told) = tokio::sync::mpsc::unbounded_channel();
    let teller = RoundReply::new(WORK, RunId(1), to).teller();
    let done = acts.inner.act(&starting(item), &teller, &mut Stop::never()).await;
    let Ok(Done::HandsIn(came)) = done else {
        panic!("the start hands in what it wrote");
    };
    came.writes[0].content.clone()
}

#[tokio::test]
async fn the_start_says_what_the_item_continues_and_the_start_of_one_that_continues_nothing_says_what_it_did() {
    let repo = Repo::new("true");
    let dir = &repo.project.repo;
    let main = run(dir, &["rev-parse", "main"]);
    run(dir, &["switch", "-q", "-c", "old"]);
    let old = commit(dir, "old.txt", "Old.\n");
    run(dir, &["switch", "-q", "main"]);
    let mut continuing = item("Greet the user");
    continuing.continues = Some(store::Continues::Branch("old".to_string()));
    let said = start_wrote(&repo, Fake::new(&main).continued_at(Ok(old.clone())), continuing).await;
    assert!(said.contains("It continues the branch old: "), "{said}");
    assert!(said.contains(&format!("the commit {main} of main")), "{said}");
    assert!(said.contains(&repo.place(WORK).reference()), "{said}");
    assert_eq!(run(dir, &["rev-parse", &repo.place(WORK).reference()]), old);

    // The same repository, the work of an item that continues nothing: from main, in the words it always had.
    let other = Repo::new("true");
    let main = run(&other.project.repo, &["rev-parse", "main"]);
    let said = start_wrote(&other, Fake::new(&main), item("Greet the user")).await;
    assert!(
        said.contains(&format!("which started from the commit {main} of main.")),
        "{said}"
    );
    assert!(!said.contains("continues"), "{said}");
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
        account: None,
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
        content.contains("and what fails here also fails on main. The work waits for main to pass, and is checked")
            && content.ends_with("Failed on main as well: `b`."),
        "{content}"
    );
    assert_eq!(came.said.observations, ["The test `a` may be flaky."]);
    // What it wrote waits for main to move, so that the work waits for main and not for the owner.
    assert_eq!(came.waits_for, Some(BlockId::from("main")));
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

/// The flow of the end-to-end tests with a block of the work's head, which the owner reads.
fn acts_flow_with_head() -> Compiled {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/real/flow");
    let mut source = flow::read(&dir).unwrap();
    source
        .flow
        .push_str("\n[blocks.head]\noutside = \"head\"\nshown = true\n");
    let names = |names: &[&str]| names.iter().map(|n| n.to_string()).collect();
    let has = flow::Has {
        sources: names(&["tree", "main", "head"]),
        acts: names(&["start", "check", "merge"]),
        ..Default::default()
    };
    flow::compile(&source, &has).unwrap()
}

/// The first round of the merge's run 5 of the work as it is kept, whose head's latest version says `head`.
fn merging(head: &str) -> Round {
    let flow = Arc::new(acts_flow_with_head());
    let merge = (flow.cells.iter()).find(|cell| cell.act.as_deref() == Some("merge"));
    let mut kept = Work::default();
    let at = kept.tick();
    kept.records.written.push(Written {
        block: "head".into(),
        by: Outsider::Outside,
        matters: true,
        value: None,
        said_to: None,
        empty: false,
        at,
    });
    kept.contents.insert(("head".into(), at), head.to_string());
    Round {
        work: WORK,
        flow: flow.clone(),
        cell: merge.expect("the flow has a merge").clone(),
        run: RunId(5),
        number: 1,
        given: work::Given::Task,
        item: item("Greet the user"),
        kept: Arc::new(kept),
        lines: Default::default(),
        account: None,
    }
}

/// What the outside wrote while the round was under way, as the teller said it: each block and its content.
fn written_from_outside(told: &mut tokio::sync::mpsc::UnboundedReceiver<service::Message>) -> Vec<(String, String)> {
    let mut written = Vec::new();
    while let Ok(message) = told.try_recv() {
        if let service::Message::Happened {
            event:
                engine::Event::Wrote {
                    block,
                    by: Outsider::Outside,
                    content,
                    ..
                },
            ..
        } = message
        {
            written.push((block.to_string(), content));
        }
    }
    written
}

// A commit made after the check passed and before the merge moves the work's head: the merge reads it as the check
// does, writes it from outside and fails in passing, asking the host nothing, so that the review and the check run on
// it first. Refused for the branch it moved, the merge stopped the work for the owner, and letting it on refused it
// again, as nothing ran the check on it. A head that did not move lets the merge go on to what the check passed.
#[tokio::test]
async fn a_head_moved_since_the_check_fails_the_merge_in_passing_and_is_written_from_outside() {
    let repo = Repo::new("true");
    started(&repo).await;
    let place = repo.place(WORK);
    let checked = run(&repo.project.repo, &["rev-parse", &place.reference()]);
    let main = run(&repo.project.repo, &["rev-parse", "main"]);
    let after = commit(&place.dir, "weather.txt", "Sunny.\n");
    let acts = acts(&repo, Fake::new(&main));
    // The work holds the turn, as after a check that passed: what lets it go is the merge's failure alone.
    acts.inner.turn().holder = Some((WORK, Some(std::time::Instant::now())));
    let (to, mut told) = tokio::sync::mpsc::unbounded_channel();
    let teller = RoundReply::new(WORK, RunId(5), to).teller();
    let merged = acts.inner.act(&merging(&checked), &teller, &mut Stop::never()).await;
    let Err(failed) = merged else {
        panic!("the merge went on over a head the check did not pass");
    };
    // It gives up the turn, as the review comes first and may take long.
    assert_eq!(
        acts.inner.turn().holder,
        None,
        "the merge keeps no turn over a moved head"
    );
    let said = fill(&super::words().merge.unreviewed, &[("head", &after)]);
    assert_eq!(failed, Failed::passing(said));
    assert_eq!(written_from_outside(&mut told), [("head".to_string(), after.clone())]);
    assert!(
        !(acts.inner.host.asked().iter()).any(|a| matches!(a, Asked::Merge(_))),
        "the host is asked to merge nothing"
    );
    // Its head as its latest version says it, the merge goes on to what the check passed, which here is nothing.
    let (to, mut told) = tokio::sync::mpsc::unbounded_channel();
    let teller = RoundReply::new(WORK, RunId(5), to).teller();
    let merged = acts.inner.act(&merging(&after), &teller, &mut Stop::never()).await;
    let Err(failed) = merged else {
        panic!("the merge went on with no check that passed");
    };
    assert_ne!(failed.kind, Some(decide::run::Failure::Passing), "{failed:?}");
    assert!(failed.why.contains("latest check did not pass"), "{failed:?}");
    assert_eq!(written_from_outside(&mut told), Vec::new());
}

/// The round as [`merging`] gives it, its head at the commit, where the work's latest check, a run before it, passed
/// that commit, writing in its choice what the check writes there.
fn merging_checked(checked: &str) -> Round {
    let mut round = merging(checked);
    let mut kept = (*round.kept).clone();
    let check = (round.flow.cells.iter()).find(|cell| cell.act.as_deref() == Some("check"));
    let run = RunId(4);
    let at = kept.tick();
    kept.records.started.push(decide::Started {
        run,
        cell: check.expect("the flow has a check").id.clone(),
        at,
        because: Vec::new(),
        basis: std::collections::BTreeMap::new(),
    });
    let words = super::words().check;
    let at = kept.tick();
    kept.records.finished.push(Finished {
        run,
        at,
        writes: vec![decide::Write {
            block: "passed".into(),
            meaning_unchanged: false,
            value: Some(words.values.passed.clone()),
            empty: false,
        }],
        dealt_with: None,
        waits_for: None,
    });
    let said = fill(&words.first, &[("commit", checked)]);
    kept.contents.insert(("passed".into(), at), said);
    round.kept = Arc::new(kept);
    round
}

// A merge that cannot read the work's branch fails its round with what the read found, as the check does, and asks the
// host nothing, though the work's latest check passed the commit: going on, it took a head nobody read for one that had
// not moved, and merged. Well under a second alone: a repository on disk and the fake host; no other test holds the
// merge to a read that fails.
#[tokio::test]
async fn a_merge_that_cannot_read_the_works_branch_fails_its_round_as_the_check_does() {
    let repo = Repo::new("true");
    started(&repo).await;
    let place = repo.place(WORK);
    let checked = run(&repo.project.repo, &["rev-parse", &place.reference()]);
    let main = run(&repo.project.repo, &["rev-parse", "main"]);
    let round = merging_checked(&checked);
    let passing = passed(&round.flow, &round.kept, &super::words().check);
    assert_eq!(
        passing,
        Ok(Some(checked.clone())),
        "the work's latest check passed the commit"
    );
    // The branch is gone, as where a person deleted it.
    run(&repo.project.repo, &["update-ref", "-d", &place.reference()]);
    let acts = acts(&repo, Fake::new(&main));
    // The work holds the turn, as after a check that passed: what lets it go is the merge's failure alone.
    acts.inner.turn().holder = Some((WORK, Some(std::time::Instant::now())));
    let (to, mut told) = tokio::sync::mpsc::unbounded_channel();
    let teller = RoundReply::new(WORK, RunId(5), to).teller();
    let merged = acts.inner.act(&round, &teller, &mut Stop::never()).await;
    let Err(failed) = merged else {
        panic!("the merge went on over a branch it could not read");
    };
    assert_eq!(
        acts.inner.turn().holder,
        None,
        "the merge keeps no turn over a branch it could not read"
    );
    assert!(failed.why.contains(&place.reference()), "{failed:?}");
    assert_eq!(written_from_outside(&mut told), Vec::new());
    assert!(
        !(acts.inner.host.asked().iter()).any(|a| matches!(a, Asked::Merge(_))),
        "the host is asked to merge nothing"
    );
}

// A merge that cannot read what the work's latest check found fails its round and lets the merge's turn go, as it does
// for a head that moved or a branch it cannot read: holding it, the work would keep every other work's check from
// running. Well under a second alone: a repository on disk and the fake host; no other test holds the merge to a check
// it cannot read.
#[tokio::test]
async fn a_merge_that_cannot_read_the_checks_result_fails_and_keeps_no_turn() {
    let repo = Repo::new("true");
    started(&repo).await;
    let place = repo.place(WORK);
    let head = run(&repo.project.repo, &["rev-parse", &place.reference()]);
    let main = run(&repo.project.repo, &["rev-parse", "main"]);
    // The work's head as the merge reads it, and no check that passed it.
    let round = merging(&head);
    let acts = acts(&repo, Fake::new(&main));
    acts.inner.turn().holder = Some((WORK, Some(std::time::Instant::now())));
    let (to, _told) = tokio::sync::mpsc::unbounded_channel();
    let teller = RoundReply::new(WORK, RunId(5), to).teller();
    let merged = acts.inner.act(&round, &teller, &mut Stop::never()).await;
    assert!(merged.is_err(), "the merge went on with no check it could read");
    assert!(
        !(acts.inner.host.asked().iter()).any(|a| matches!(a, Asked::Merge(_))),
        "the host is asked to merge nothing"
    );
    assert_eq!(acts.inner.turn().holder, None, "the merge keeps no turn");
}

// A merge refused for a moved main keeps the work's turn, for its check on main's new head, as after a check that
// passed; one that merges lets it go. Well under a second alone: a repository on disk and the fake host; no other test
// holds a refused or a made merge to the turn.
#[tokio::test]
async fn a_merge_refused_for_a_moved_main_keeps_the_turn_and_one_made_lets_it_go() {
    let repo = Repo::new("true");
    started(&repo).await;
    let place = repo.place(WORK);
    let checked = run(&repo.project.repo, &["rev-parse", &place.reference()]);
    let main = run(&repo.project.repo, &["rev-parse", "main"]);
    let round = merging_checked(&checked);
    let moved = NotMerged::Moved {
        main: "beef".to_string(),
        checked: checked.clone(),
    };
    let refusing = acts(&repo, Fake::new(&main).refuses(moved));
    let (to, _told) = tokio::sync::mpsc::unbounded_channel();
    let teller = RoundReply::new(WORK, RunId(5), to).teller();
    let refused = refusing.inner.act(&round, &teller, &mut Stop::never()).await;
    assert!(refused.is_err(), "refused for main having moved");
    let holder = (refusing.inner.turn().holder).map(|(work, passed)| (work, passed.is_some()));
    assert_eq!(
        holder,
        Some((WORK, true)),
        "the work keeps its turn, as after a check that passed"
    );
    // Merged, the work lets the turn go.
    let merging = acts(&repo, Fake::new(&main));
    merging.inner.turn().holder = Some((WORK, Some(std::time::Instant::now())));
    let (to, _told) = tokio::sync::mpsc::unbounded_channel();
    let teller = RoundReply::new(WORK, RunId(5), to).teller();
    let merged = merging.inner.act(&round, &teller, &mut Stop::never()).await;
    assert!(merged.is_ok(), "{:?}", merged.as_ref().err());
    assert_eq!(merging.inner.turn().holder, None, "a merge made lets the turn go");
}

// A merge waits for the work's turn and asks the host to merge only once it has it: the work whose check passed may
// have lost the turn since, paused or past its time. Well under a second alone: a repository on disk and the fake
// host; no other test holds the merge to the turn.
#[tokio::test]
async fn a_merge_takes_the_turn_before_it_merges() {
    let repo = Repo::new("true");
    started(&repo).await;
    let place = repo.place(WORK);
    let checked = run(&repo.project.repo, &["rev-parse", &place.reference()]);
    let main = run(&repo.project.repo, &["rev-parse", "main"]);
    let round = merging_checked(&checked);
    let waiting = acts(&repo, Fake::new(&main));
    let other = WorkNumber(2);
    waiting.inner.turn().holder = Some((other, None));
    let (to, _told) = tokio::sync::mpsc::unbounded_channel();
    let teller = RoundReply::new(WORK, RunId(5), to).teller();
    // Once the merge waits for the turn, nothing is merged; then the other work lets the turn go.
    let letting_go = async {
        while !waiting.inner.turn().waiting.contains(&WORK) {
            tokio::task::yield_now().await;
        }
        let asked = waiting.inner.host.asked();
        assert!(!asked.iter().any(|a| matches!(a, Asked::Merge(_))), "{asked:?}");
        waiting.inner.end_turn(other);
    };
    let mut stop = Stop::never();
    let (merged, ()) = tokio::join!(waiting.inner.act(&round, &teller, &mut stop), letting_go);
    assert!(merged.is_ok(), "{:?}", merged.as_ref().err());
    assert_eq!(waiting.inner.host.asked().last(), Some(&Asked::Merge(Some(checked))));
}
