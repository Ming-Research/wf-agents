//! What the engine reads of a work's branch, against a repository made for each test: the design tree and the work's
//! head read in one step, each a version only where it changed, and what a round is given between versions. Each test
//! makes a repository of a few commits and runs a few dozen git commands, in well under a second.

use std::path::Path;

use decide::{BlockId, Outsider, RunId, Seq, Written};
use store::WorkNumber;

use super::*;
use crate::git::tests::{Repo, commit, run};
use crate::git::{self, Place};

const WORK: WorkNumber = WorkNumber(1);

/// A flow of one cell, given the design tree and the work's head and run by a request.
fn reading_the_branch() -> Compiled {
    let flow = r#"
name = "tiny"
order = ["read"]

[limits]
runs = 10
returns = 10
calls = 10

[lines.engineer]

[blocks.tree]
outside = "tree"

[blocks.head]
outside = "head"

[blocks.request]
people = true
"#;
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
        flow: flow.to_string(),
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
        guide: "AGENTS.md".to_string(),
    }
}

/// The flow of one cell that reads the project's guide as well.
fn reading_the_guide() -> Compiled {
    let flow = r#"
name = "guided"
order = ["read"]

[limits]
runs = 10
returns = 10
calls = 10

[lines.engineer]

[blocks.tree]
outside = "tree"

[blocks.guide]
outside = "guide"

[blocks.request]
people = true
"#;
    let cell = r#"
title = "Read"
kind = "agent"
effort = "high"
line = "engineer"
task = "Read the guide and the tree."
watches = ["request", "guide"]
reads = ["tree"]
outputs = []

[kinds]
request = "latest"
guide = "last_and_latest"
tree = "last_and_latest"
"#;
    let source = flow::Source {
        flow: flow.to_string(),
        cells: [("read".to_string(), cell.to_string())].into_iter().collect(),
        ..Default::default()
    };
    let has = flow::Has {
        sources: [tree::TREE, "guide"].map(str::to_string).into_iter().collect(),
        ..Default::default()
    };
    flow::compile(&source, &has).expect("the flow compiles")
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

// The project's guide is read in the same step as the tree and the head (design document `data-and-context`, 24): its
// text as the work's branch has it, a version only where the text changed, and none where the branch has no guide.
#[tokio::test]
async fn the_guide_is_read_in_the_same_step_its_text_a_version_only_where_it_changed() {
    let (_repo, place) = started().await;
    let flow = reading_the_guide();
    let mut looking = Looking::of(&flow, &Work::default(), reading());
    assert_eq!(
        looking.read(&place).await.unwrap(),
        Vec::new(),
        "the branch has no guide"
    );
    write(&place.dir, "AGENTS.md", "Write in English.\n");
    assert_eq!(
        looking.read(&place).await.unwrap(),
        [(BlockId::from("guide"), "Write in English.\n".to_string())]
    );
    commit(&place.dir, "farewell.txt", "Bye.\n");
    assert_eq!(
        looking.read(&place).await.unwrap(),
        Vec::new(),
        "the guide did not change"
    );
    write(&place.dir, "AGENTS.md", "Write in plain English.\n");
    assert_eq!(
        looking.read(&place).await.unwrap(),
        [(BlockId::from("guide"), "Write in plain English.\n".to_string())]
    );
    // The engine compares the tree's versions and the head's itself, and no guide's: what changed in it is its text.
    assert_eq!(compared(&flow), [BlockId::from("tree")].into_iter().collect());
}
