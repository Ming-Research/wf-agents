//! What the engine reads of a work's branch, against a repository made for each test: the design tree and the work's
//! head read in one step, each a version only where it changed, and what a round is given between versions. Each test
//! makes a repository of a few commits and runs a few dozen git commands, in well under a second.

use std::path::Path;

use decide::{
    BlockId, CallId, Called, Cause, Finished, Outsider, Records, Returned, Round, RunId, Seq, Started, Written,
};
use store::WorkNumber;

use super::*;
use crate::git::tests::{Repo, commit, run};
use crate::git::{self, Place};

const WORK: WorkNumber = WorkNumber(1);

/// A flow of one cell, given the design tree and the work's head and run by a request.
fn reading_the_branch() -> Compiled {
    reading_the_branch_on("[lines.engineer]")
}

/// The same flow, its cell's line declared as this.
fn reading_the_branch_on(line: &str) -> Compiled {
    reading_cells(line, &["read"])
}

/// The same flow with these cells, each the cell above, in this order.
fn reading_cells(line: &str, cells: &[&str]) -> Compiled {
    let order = cells.iter().map(|c| format!("\"{c}\"")).collect::<Vec<_>>().join(", ");
    let flow = format!(
        r#"
name = "tiny"
order = [{order}]

[limits]
runs = 10
returns = 10
calls = 10

{line}

[blocks.tree]
outside = "tree"

[blocks.head]
outside = "head"

[blocks.request]
people = true
"#
    );
    let cell = r#"
title = "Read"
kind = "agent"
effort = "high"
line = "engineer"
task = "Read the tree and the change."
watches = ["request"]
reads = ["tree", "head"]
outputs = []

[kinds]
request = "latest"
tree = "last_and_latest"
head = "last_and_latest"
"#;
    let source = flow::Source {
        flow,
        cells: cells.iter().map(|c| (c.to_string(), cell.to_string())).collect(),
        ..Default::default()
    };
    let has = flow::Has {
        sources: [tree::TREE.to_string(), head::HEAD.to_string()].into_iter().collect(),
        ..Default::default()
    };
    flow::compile(&source, &has).expect("the flow compiles")
}

/// What the branch's data are read with in the tests' repository.
fn reading() -> Reading {
    Reading {
        tree: "design/".to_string(),
        main: "refs/heads/main".to_string(),
    }
}

/// A started work.
async fn started() -> (Repo, Place) {
    let repo = Repo::new("true");
    let started = git::start(&repo.project, &repo.place(WORK), "main").await.unwrap();
    (repo, started.place)
}

/// Writes the file in a directory of the worktree, commits it, and gives the commit.
fn write(dir: &Path, file: &str, text: &str) -> String {
    let path = dir.join(file);
    std::fs::create_dir_all(path.parent().expect("a directory")).expect("the directory is made");
    std::fs::write(&path, text).expect("the file is written");
    run(dir, &["add", file]);
    run(dir, &["commit", "-q", "-m", &format!("Write {file}")]);
    run(dir, &["rev-parse", "HEAD"])
}

/// The blocks of the new versions a read gives.
fn blocks(new: &[(BlockId, String)]) -> Vec<&str> {
    new.iter().map(|(block, _)| block.as_str()).collect()
}

// One step reads the design tree and the work's head: a commit of the tree makes a version of each, one of another
// file of the head alone, and a branch that holds no change of its own, or none since, makes none.
#[tokio::test]
async fn the_tree_and_the_head_are_read_in_one_step_each_a_version_only_where_it_changed() {
    let (_repo, place) = started().await;
    let mut looking = Looking::of(&reading_the_branch(), &Work::default(), reading(), None);
    assert_eq!(looking.read(&place).await.unwrap(), Vec::new());
    assert_eq!(looking.head(), None, "the branch holds no change of its own");
    let designed = write(&place.dir, "design/a.md", "One.\n");
    let new = looking.read(&place).await.unwrap();
    assert_eq!(blocks(&new), ["head", "tree"]);
    assert_eq!(new[0].1, designed);
    assert_eq!(looking.head(), Some(designed.as_str()));
    let built = commit(&place.dir, "farewell.txt", "Bye.\n");
    assert_eq!(
        looking.read(&place).await.unwrap(),
        [(BlockId::from("head"), built.clone())]
    );
    assert_eq!(looking.read(&place).await.unwrap(), Vec::new(), "nothing changed since");
    // A look begun from the work as it is kept starts from what its latest versions say.
    let mut work = Work::default();
    for (at, (block, content)) in [(1, ("head", built.clone())), (2, ("tree", String::from("x -")))] {
        work.contents.insert((BlockId::from(block), Seq(at)), content);
        work.records.written.push(Written {
            block: BlockId::from(block),
            by: Outsider::Outside,
            matters: true,
            value: None,
            said_to: None,
            empty: false,
            at: Seq(at),
        });
    }
    let mut again = Looking::of(&reading_the_branch(), &work, reading(), None);
    assert_eq!(blocks(&again.read(&place).await.unwrap()), ["tree"]);
    assert_eq!(again.head(), Some(built.as_str()));
}

// Each new head is kept under the engine's own reference as it is read, so that what the work committed outlives its
// worktree and its branch, and git's collecting of what no reference holds.
#[tokio::test]
async fn each_new_head_is_kept_under_the_engines_own_reference_and_outlives_the_branch() {
    let (repo, place) = started().await;
    let kept = format!("refs/wf-agents/{}", place.branch);
    let resolves = |reference: &str| {
        let verify = ["rev-parse", "--verify", "--quiet", reference];
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(&repo.project.repo)
            .args(verify)
            .output()
            .expect("git runs");
        output
            .status
            .success()
            .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
    };
    let mut looking = Looking::of(&reading_the_branch(), &Work::default(), reading(), None);
    looking.read(&place).await.unwrap();
    assert_eq!(
        resolves(&kept),
        None,
        "a branch with no change of its own keeps nothing"
    );
    let first = commit(&place.dir, "farewell.txt", "Bye.\n");
    looking.read(&place).await.unwrap();
    assert_eq!(resolves(&kept), Some(first));
    let second = commit(&place.dir, "farewell.txt", "Goodbye.\n");
    looking.read(&place).await.unwrap();
    assert_eq!(resolves(&kept).as_deref(), Some(second.as_str()));
    git::remove(&repo.project, &place).await.unwrap();
    run(&repo.project.repo, &["branch", "-D", &place.branch]);
    run(&repo.project.repo, &["gc", "-q", "--prune=now"]);
    let object = format!("{second}^{{commit}}");
    assert_eq!(
        resolves(&object).as_deref(),
        Some(second.as_str()),
        "the commit outlives the branch"
    );
}

#[tokio::test]
async fn a_round_is_given_what_changed_from_each_earlier_version_of_the_tree_and_the_head_to_the_latest() {
    let (_repo, place) = started().await;
    let dir = &place.dir;
    let first = write(dir, "design/a.md", "One.\n");
    let second = write(dir, "design/b.md", "Two.\n");
    let third = write(dir, "design/c.md", "Three.\n");
    let mut work = Work::default();
    let of_tree = |commit: &str| format!("{commit} {}", run(dir, &["rev-parse", &format!("{commit}:design")]));
    let mut at = 0;
    for commit in [&first, &second, &third] {
        for (block, content) in [("tree", of_tree(commit)), ("head", commit.to_string())] {
            at += 1;
            work.contents.insert((BlockId::from(block), Seq(at)), content);
            work.records.written.push(Written {
                block: BlockId::from(block),
                by: Outsider::Outside,
                matters: true,
                value: None,
                said_to: None,
                empty: false,
                at: Seq(at),
            });
        }
    }
    let changes = changes(&reading_the_branch(), &work, RunId(1), 1, dir, &reading())
        .await
        .unwrap();
    let (tree, head) = (BlockId::from("tree"), BlockId::from("head"));
    let expected = [
        ((tree.clone(), Seq(1)), "design/b.md\ndesign/c.md".to_string()),
        ((head.clone(), Seq(2)), format!("`git diff {first} {third}`")),
        ((tree, Seq(3)), "design/c.md".to_string()),
        ((head, Seq(4)), format!("`git diff {second} {third}`")),
    ];
    assert_eq!(changes, expected.into_iter().collect());
}

// The engine compares the versions of each datum of the branch itself, as git compares them: the tree's and the head's,
// and no text's.
#[test]
fn each_datum_of_the_branch_is_compared_by_the_engine_itself() {
    let expected = [BlockId::from("head"), BlockId::from("tree")];
    assert_eq!(compared(&reading_the_branch()), expected.into_iter().collect());
}

/// The records of a work whose one cell, `read`, has a run that asked for a call, 7, as its round ended; the call has
/// no result. With `entered`, the cell was entered again for the result, and that run finished asking for no call.
fn asked_for_a_call(entered: bool) -> Work {
    let (run, call) = (RunId(1), CallId(7));
    let mut records = Records::default();
    records.started.push(Started {
        run,
        cell: "read".into(),
        at: Seq(1),
        because: Vec::new(),
        basis: Default::default(),
    });
    records.rounds.push(Round { run, at: Seq(2) });
    records.returned.push(Returned { run, at: Seq(3) });
    records.called.push(Called {
        call,
        run,
        on: None,
        at: Seq(4),
    });
    records.finished.push(Finished {
        run,
        at: Seq(5),
        writes: Vec::new(),
        dealt_with: Some(Vec::new()),
        waits_for: None,
    });
    if entered {
        let run = RunId(2);
        records.done.push(decide::Done { call, at: Seq(6) });
        records.started.push(Started {
            run,
            cell: "read".into(),
            at: Seq(7),
            because: vec![Cause::Results { calls: vec![call] }],
            basis: Default::default(),
        });
        records.rounds.push(Round { run, at: Seq(8) });
        records.returned.push(Returned { run, at: Seq(9) });
        records.finished.push(Finished {
            run,
            at: Seq(10),
            writes: Vec::new(),
            dealt_with: Some(vec!["request".into()]),
            waits_for: None,
        });
    }
    Work {
        records,
        ..Work::default()
    }
}

/// `work` with a run, `run`, of this cell, entered for the results of its calls (`results`) or for nothing, that has
/// not finished.
fn entered(mut work: Work, run: u64, cell: &str, results: &[CallId]) -> Work {
    let run = RunId(run);
    let because = if results.is_empty() {
        Vec::new()
    } else {
        vec![Cause::Results {
            calls: results.to_vec(),
        }]
    };
    work.records.started.push(Started {
        run,
        cell: cell.into(),
        at: Seq(20),
        because,
        basis: Default::default(),
    });
    work.records.rounds.push(Round { run, at: Seq(21) });
    work
}

/// `work` with the cell `cell` having asked for the call `call` in its run `run`, which finished, the call having no
/// result.
fn asked_by(mut work: Work, run: u64, cell: &str, call: u64) -> Work {
    let (run, call) = (RunId(run), CallId(call));
    work.records.started.push(Started {
        run,
        cell: cell.into(),
        at: Seq(11),
        because: Vec::new(),
        basis: Default::default(),
    });
    work.records.rounds.push(Round { run, at: Seq(12) });
    work.records.returned.push(Returned { run, at: Seq(13) });
    work.records.called.push(Called {
        call,
        run,
        on: None,
        at: Seq(14),
    });
    work.records.finished.push(Finished {
        run,
        at: Seq(15),
        writes: Vec::new(),
        dealt_with: Some(Vec::new()),
        waits_for: None,
    });
    work
}

// What the branch holds beyond the records, the head and the design tree alike, is the cell's that may change the work
// while it waits for the results of the calls its round asked for: a read that writes from outside, a round that
// hands in what it committed and any other round reads none of it, whichever it is (decision
// a-version-read-at-a-rounds-end-is-that-rounds).
#[tokio::test]
async fn while_a_cell_that_may_change_the_work_waits_for_its_calls_no_read_takes_the_head_or_the_tree() {
    let (_repo, place) = started().await;
    let committed = write(&place.dir, "design/a.md", "One.\n");
    // A read that writes from outside, as a round that only reads ends with, one after taking main in, an act's.
    let waiting = asked_for_a_call(false);
    let mut looking = Looking::of(&reading_the_branch(), &waiting, reading(), None);
    assert!(looking.awaiting && looking.held);
    assert_eq!(looking.read_of(&place, looking.held).await.unwrap(), Vec::new());
    // A round of a line that may change the work, on the work of another cell, hands in nothing; and neither does the
    // waiting cell's own round, entered for the owner's words.
    let for_words = entered(waiting, 3, "read", &[]);
    let mut other = Looking::of(&reading_the_branch(), &for_words, reading(), Some(RunId(3)));
    assert!(other.awaiting);
    assert_eq!(other.handed_in(&place).await.unwrap(), Vec::new());
    assert_eq!(other.head(), None);
    // What the branch held is still all there for the run that ends the waiting, and it is told of nothing before.
    let results = [CallId(7)];
    let mut done = asked_for_a_call(false);
    done.records.done.push(decide::Done {
        call: CallId(7),
        at: Seq(6),
    });
    let mut run = Looking::of(
        &reading_the_branch(),
        &entered(done, 4, "read", &results),
        reading(),
        Some(RunId(4)),
    );
    assert!(!run.awaiting, "the run entered for the results is not held back");
    assert!(run.held, "but a read that writes from outside still reads nothing");
    assert_eq!(run.read_of(&place, run.held).await.unwrap(), Vec::new());
    let handed = run.handed_in(&place).await.unwrap();
    let handed: Vec<_> = handed
        .into_iter()
        .map(|h| (h.block.as_str().to_string(), h.content))
        .collect();
    assert_eq!(handed.len(), 2, "the head and the tree");
    assert_eq!(handed[0], ("head".to_string(), committed));
    assert_eq!(handed[1].0, "tree");
}

// A tree change committed before the call is the waiting cell's as much as the head is: a round that only reads and ends
// meanwhile does not tell it as the outside's, and the run that ends the waiting hands it in.
#[tokio::test]
async fn a_tree_change_committed_before_the_call_is_not_told_as_the_outsides_meanwhile() {
    let (_repo, place) = started().await;
    write(&place.dir, "design/a.md", "One.\n");
    let mut meanwhile = Looking::of(&reading_the_branch(), &asked_for_a_call(false), reading(), None);
    assert_eq!(meanwhile.read_of(&place, meanwhile.held).await.unwrap(), Vec::new());
    let mut ending = Looking::of(&reading_the_branch(), &asked_for_a_call(true), reading(), None);
    assert!(!ending.awaiting && !ending.held);
    assert_eq!(
        blocks(&ending.read_of(&place, ending.held).await.unwrap()),
        ["head", "tree"]
    );
}

// Where two cells that may change the work wait at once, the run of one entered for its results sees the other waiting
// and hands in nothing; the last to end hands in everything the branch holds.
#[tokio::test]
async fn where_two_cells_wait_the_first_run_to_end_hands_in_nothing_and_the_last_hands_in_the_head() {
    let (_repo, place) = started().await;
    let committed = write(&place.dir, "design/a.md", "One.\n");
    let flow = reading_cells("[lines.engineer]", &["read", "other"]);
    let both = asked_by(asked_for_a_call(false), 5, "other", 8);
    let first = entered(both.clone(), 6, "read", &[CallId(7)]);
    let mut looking = Looking::of(&flow, &first, reading(), Some(RunId(6)));
    assert!(looking.awaiting, "the other cell still waits");
    assert_eq!(looking.handed_in(&place).await.unwrap(), Vec::new());
    // The first cell's run finished, and the other's is entered for its results: no cell but its own waits.
    let mut finished = first;
    finished.records.finished.push(Finished {
        run: RunId(6),
        at: Seq(22),
        writes: Vec::new(),
        dealt_with: Some(Vec::new()),
        waits_for: None,
    });
    finished.records.done.push(decide::Done {
        call: CallId(8),
        at: Seq(23),
    });
    let last = entered(finished, 9, "other", &[CallId(8)]);
    let mut looking = Looking::of(&flow, &last, reading(), Some(RunId(9)));
    assert!(!looking.awaiting);
    let handed = looking.handed_in(&place).await.unwrap();
    assert_eq!(handed[0].content, committed);
}

// A cell on a line that only reads commits nothing, so what its calls leave open holds nothing back.
#[tokio::test]
async fn a_cell_that_only_reads_holds_nothing_back_while_it_waits_for_its_calls() {
    let reading_only = reading_the_branch_on("[lines.engineer]\nreads_only = true");
    let looking = Looking::of(&reading_only, &asked_for_a_call(false), reading(), None);
    assert!(!looking.awaiting && !looking.held);
}
