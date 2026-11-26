//! What is kept of a version of a flow for reading a work on it later: its outline, made from the compiled flow, read
//! back as kept, a part an outline kept before lacks reading as absent; and how a work's records read on a later
//! version, through the renumberings after the one it was recorded on (decisions
//! `a-version-is-kept-as-its-files-and-its-outline` and `a-datum-made-for-a-cell-follows-its-cell`).

use std::collections::BTreeMap;
use std::path::Path;

use decide::{BlockId, CellId};
use flow::outline::{Outline, OutlinedCell, OutlinedLine, taken_out};
use flow::{Compiled, Has, Renumbering, reading};

/// The engineer's flow as the repository defines it, compiled against what the engine has for it.
fn engineer() -> Compiled {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow/engineer");
    let source = flow::read(&dir).expect("the engineer's flow is read");
    let names = |names: &[&str]| names.iter().map(|n| n.to_string()).collect();
    let has = Has {
        sources: names(&[
            "answer",
            "asked",
            "dependencies",
            "handed_in",
            "main",
            "start",
            "head",
            "test_times",
            "test_runs",
            "tree",
            "words",
        ]),
        acts: names(&["start", "check", "merge", "deliver"]),
        tools: names(&["query", "run_tests"]),
        system: names(&["guide", "constitution", "tree_rules", "how_you_work_here"]),
    };
    flow::compile(&source, &has).expect("the engineer's flow compiles")
}

/// An outline of no cells, with this table of earlier ids and the data each cell has its own of the engineer's flow
/// makes.
fn tabled(earlier_ids: Vec<Renumbering>) -> Outline {
    Outline {
        name: "engineer".into(),
        cells: Vec::new(),
        lines: Vec::new(),
        groups: Vec::new(),
        card: Vec::new(),
        earlier_ids,
        own: vec!["{cell}_questions".into(), "{cell}_words".into()],
        from: BTreeMap::new(),
    }
}

fn renumbering(ids: &[(&str, &str)], taken: &[&str]) -> Renumbering {
    Renumbering {
        ids: (ids.iter())
            .map(|(a, b)| (CellId::from(*a), CellId::from(*b)))
            .collect(),
        taken_out: taken.iter().map(|id| CellId::from(*id)).collect(),
    }
}

fn pairs<T: From<&'static str> + Ord>(pairs: &[(&'static str, &'static str)]) -> BTreeMap<T, T> {
    pairs.iter().map(|(a, b)| (T::from(*a), T::from(*b))).collect()
}

#[test]
fn the_engineer_s_outline_holds_each_cell_in_order_with_what_reading_a_work_on_it_needs() {
    let flow = engineer();
    let outline = Outline::of(&flow);
    let ids: Vec<&str> = outline.cells.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "S1", "R0", "R1", "R2", "D1", "D2", "P1", "P2", "I1", "T1", "T2", "T3", "T4", "M1", "M2"
        ]
    );
    let cell = |id: &str| outline.cell(&CellId::from(id)).expect("the cell is outlined");
    assert_eq!(cell("T2").title, "Test review");
    assert_eq!(cell("T4").title, "Adversarial testing");
    assert!(cell("I1").agent && cell("I1").line.as_deref() == Some("engineer"));
    assert!(!cell("M1").agent && cell("M1").line.is_none());
    // What a cell is given, watches and reads is the compiled flow's.
    let spec = flow.cells.iter().find(|s| s.id.as_str() == "T2").expect("T2");
    let t2: &OutlinedCell = cell("T2");
    assert_eq!(t2.given.len(), spec.given.len());
    assert_eq!(
        t2.watches.iter().map(|w| w.name.as_str()).collect::<Vec<_>>(),
        ["change", "test_review"]
    );
    assert_eq!(
        t2.reads.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(),
        ["plan", "background", "tree", "tree_entries", "test_times", "test_runs"]
    );
    // The tests' times are read where the change and its tests are written and judged, and watched by no cell: what the
    // engine writes of them as a run reports them runs none.
    let reads_times = |id: &str| cell(id).reads.iter().any(|r| r.name == "test_times");
    assert!(reads_times("I1") && reads_times("T2"));
    assert!(
        (outline.cells.iter()).all(|c| c.watches.iter().all(|w| w.name != "test_times")),
        "no cell watches the tests' times"
    );
    // The calls of `run_tests` are read by the test review alone, and watched by no cell: what the engine writes of
    // them as a call ends runs none.
    let reads_runs = |id: &str| cell(id).reads.iter().any(|r| r.name == "test_runs");
    assert!(reads_runs("T2"));
    let runs_kind = t2
        .given
        .iter()
        .find(|g| g.block.as_str() == "test_runs")
        .map(|g| g.kind);
    assert_eq!(
        runs_kind,
        Some(flow::InputKind::All),
        "a version for each call, each given once"
    );
    assert!(
        (outline.cells.iter())
            .filter(|c| c.id.as_str() != "T2")
            .all(|c| !reads_runs(c.id.as_str())),
        "no other cell reads the calls of `run_tests`"
    );
    assert!(
        (outline.cells.iter()).all(|c| c.watches.iter().all(|w| w.name != "test_runs")),
        "no cell watches the calls of `run_tests`"
    );
    // A cell that may ask is spoken to in its own words, whose texts the page reads the talk by.
    let words = cell("I1").words.as_ref().expect("I1 has words of its own");
    assert_eq!(words.block, BlockId::from("i1_words"));
    let texts: Vec<&str> = words.writes.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(texts, ["owner", "reply"]);
    assert!(
        cell("T1").words.is_none(),
        "a review asks nothing and is spoken to in nothing"
    );
    // A cell that may ask has questions of its own, of which the owner's answers are versions; a review has none.
    assert_eq!(cell("I1").questions, Some(BlockId::from("i1_questions")));
    assert!(cell("T1").questions.is_none());
    // Where each datum written from outside the work comes from, as its file says, and none for what a cell writes.
    let from = |block: &str| outline.from.get(&BlockId::from(block)).map(String::as_str);
    assert_eq!(from("background"), Some("owner"));
    assert_eq!(from("i1_words"), Some("words"));
    assert_eq!(from("i1_questions"), Some("answer"));
    assert_eq!(from("test_times"), Some("test_times"));
    assert_eq!(from("test_runs"), Some("test_runs"));
    assert_eq!(from("plan_owner"), None);
    // Each line by its name; the groups, the card, the table and the data made for a cell.
    let lines: Vec<&str> = outline.lines.iter().map(|l| l.name.as_str()).collect();
    assert_eq!(
        lines,
        [
            "code_reviewer",
            "deep_researcher",
            "engineer",
            "plan_reviewer",
            "tester"
        ]
    );
    // A line's agents only read where its file says so, as a fork of one of its sessions must know.
    let reads_only = |name: &str| {
        (outline.lines.iter())
            .find(|l| l.name == name)
            .and_then(|l| l.reads_only)
    };
    assert_eq!(
        (reads_only("code_reviewer"), reads_only("engineer")),
        (Some(true), Some(false))
    );
    let groups: Vec<&str> = outline.groups.iter().map(|g| g.name.as_str()).collect();
    assert_eq!(groups, ["change", "plan"]);
    assert_eq!(outline.card, [BlockId::from("background"), BlockId::from("plan_owner")]);
    assert_eq!(outline.earlier_ids, Vec::new());
    assert_eq!(outline.own, ["{cell}_handed_in", "{cell}_questions", "{cell}_words"]);
    assert_eq!(outline.last(), Some(&CellId::from("M2")));
    // Kept as JSON, it reads back as it was.
    let kept = serde_json::to_string(&outline).expect("the outline is written");
    assert_eq!(serde_json::from_str::<Outline>(&kept).expect("and read"), outline);
}

#[test]
fn a_work_s_records_read_through_each_renumbering_after_the_one_they_were_recorded_on_in_turn() {
    // T2 moved to T4, a new T2 standing where it was; then T4 moved to T5 and T3 was taken out.
    let to = tabled(vec![
        renumbering(&[("T2", "T4")], &[]),
        renumbering(&[("T4", "T5")], &["T3"]),
    ]);
    // Recorded before both: T2 reads as T5, T3 as the cell taken out at the second, and their data follow.
    let read = reading(&to, 0);
    assert_eq!(read.cells, pairs(&[("T2", "T5"), ("T3", "T3@2"), ("T4", "T5")]));
    assert_eq!(read.cell(&CellId::from("T3")), taken_out(&CellId::from("T3"), 2));
    assert_eq!(read.block(&BlockId::from("t2_words")), BlockId::from("t5_words"));
    assert_eq!(
        read.block(&BlockId::from("t3_questions")),
        BlockId::from("t3@2_questions")
    );
    assert!(read.renames(&BlockId::from("t2_questions")));
    // A datum made for no cell, and a cell no renumbering moved, read as recorded.
    assert!(!read.renames(&BlockId::from("plan_owner")));
    assert_eq!(read.cell(&CellId::from("I1")), CellId::from("I1"));
    // Recorded on the version that held the first renumbering already: only the second is read through.
    let read = reading(&to, 1);
    assert_eq!(read.cells, pairs(&[("T3", "T3@2"), ("T4", "T5")]));
    assert_eq!(read.blocks.get(&BlockId::from("t2_words")), None);
    // Recorded on the version installed: nothing reads otherwise.
    assert!(reading(&to, 2).is_empty());
}

#[test]
fn an_outline_kept_before_a_part_was_added_reads_with_that_part_absent() {
    // The outline kept as the first version of its form, checked in so that a change of the form that would not read it
    // fails here.
    let kept = include_str!("outline/kept.json");
    let outline: Outline = serde_json::from_str(kept).expect("the outline kept reads with today's reader");
    assert_eq!(outline.last(), Some(&CellId::from("M1")));
    let cell = outline.cell(&CellId::from("R1")).expect("R1");
    assert_eq!(cell.title, "Initial research");
    assert_eq!(cell.words.as_ref().map(|w| w.block.as_str()), Some("r1_words"));
    // A cell with no part but its id, title, line and whether an agent runs it reads with each other part absent.
    let bare: OutlinedCell =
        serde_json::from_str(r#"{"id": "M1", "title": "Merge", "line": null, "agent": false}"#).expect("it reads");
    assert!(bare.given.is_empty() && bare.watches.is_empty() && bare.reads.is_empty() && bare.words.is_none());
    assert!(bare.questions.is_none());
    let bare: Outline = serde_json::from_str(r#"{"name": "engineer", "cells": []}"#).expect("it reads");
    assert!(bare.lines.is_empty() && bare.earlier_ids.is_empty() && bare.own.is_empty() && bare.card.is_empty());
    assert!(bare.from.is_empty());
    // A line kept with its name alone reads with whether its agents only read not known.
    let line: OutlinedLine = serde_json::from_str(r#"{"name": "engineer"}"#).expect("it reads");
    assert_eq!(line.reads_only, None);
}

// The two reviews of the plan think at the effort high, the owner having chosen it over xhigh for them, as the
// reviews of the change do; the research and the implementation keep their own.
#[test]
fn the_engineer_s_reviews_of_the_plan_run_at_the_effort_high() {
    let compiled = engineer();
    for id in ["P1", "P2"] {
        let cell = compiled.cells.iter().find(|c| c.id.as_str() == id).expect("the cell");
        assert_eq!(cell.effort, Some(flow::Effort::High), "{id}");
    }
}

// The three reviews of the change are cells of one line that only reads, so that they share a session and none of them
// can change the work; the adversarial testing runs on a line of its own.
#[test]
fn the_three_reviews_of_the_change_share_one_line_that_only_reads_and_the_adversarial_testing_has_its_own() {
    let compiled = engineer();
    let cell =
        |id: &str| (compiled.cells.iter().find(|c| c.id.as_str() == id)).unwrap_or_else(|| panic!("the cell {id}"));
    for id in ["T1", "T2", "T3"] {
        assert_eq!(cell(id).line.as_deref(), Some("code_reviewer"), "{id}");
        assert!(cell(id).reads_only, "{id} may change the work");
    }
    let testing = cell("T4");
    assert!(testing.line.is_some(), "the adversarial testing has a line");
    assert_ne!(testing.line.as_deref(), Some("code_reviewer"));
}
