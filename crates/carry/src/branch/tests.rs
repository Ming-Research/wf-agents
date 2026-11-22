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
    let flow = format!(
        r#"
name = "tiny"
order = ["read"]

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
        cells: [("read".to_string(), cell.to_string())].into_iter().collect(),
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
    let mut looking = Looking::of(&reading_the_branch(), &Work::default(), reading());
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
    let mut again = Looking::of(&reading_the_branch(), &work, reading());
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
    let mut looking = Looking::of(&reading_the_branch(), &Work::default(), reading());
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

// What the branch holds beyond the head the records say, while the cell that may change the work waits for the results
// of the calls its round asked for, is its commit, which its run hands in: a read that writes from outside leaves the
// head alone then and reads the rest, and the read that hands in reads the head (decision
// a-version-read-at-a-rounds-end-is-that-rounds).
#[tokio::test]
async fn a_read_that_writes_from_outside_leaves_the_head_while_the_cell_that_committed_waits_for_its_calls() {
    let (_repo, place) = started().await;
    let committed = write(&place.dir, "design/a.md", "One.\n");
    let mut looking = Looking::of(&reading_the_branch(), &asked_for_a_call(false), reading());
    assert!(looking.awaiting);
    let new = looking.read_of(&place, looking.awaiting).await.unwrap();
    assert_eq!(blocks(&new), ["tree"], "the head is left for the run that hands it in");
    assert_eq!(looking.head(), None);
    let handed = looking.read(&place).await.unwrap();
    assert_eq!(handed, [(BlockId::from("head"), committed)]);
}

#[tokio::test]
async fn a_read_that_writes_from_outside_reads_the_head_once_the_results_are_handed_back_and_the_cell_has_finished() {
    let (_repo, place) = started().await;
    let committed = write(&place.dir, "design/a.md", "One.\n");
    let mut looking = Looking::of(&reading_the_branch(), &asked_for_a_call(true), reading());
    assert!(!looking.awaiting);
    let new = looking.read_of(&place, looking.awaiting).await.unwrap();
    assert_eq!(blocks(&new), ["head", "tree"]);
    assert_eq!(looking.head(), Some(committed.as_str()));
}

// A cell on a line that only reads commits nothing, so what its calls leave open holds no head back.
#[tokio::test]
async fn a_cell_that_only_reads_holds_no_head_back_while_it_waits_for_its_calls() {
    let reading_only = reading_the_branch_on("[lines.engineer]\nreads_only = true");
    let looking = Looking::of(&reading_only, &asked_for_a_call(false), reading());
    assert!(!looking.awaiting);
}
