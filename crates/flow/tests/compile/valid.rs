//! What a valid flow compiles to.

use std::collections::BTreeMap;

use decide::{BlockId, Cell, Condition, TestName};
use flow::{Block, CellSpec, Effort, FLOW_FILE, FaultKind::*, Kind, Limits, Readers, Test, TestKind, Writers};

use crate::support::{at, compiled, edit, faults, valid};

fn blocks(names: &[&str]) -> Vec<BlockId> {
    names.iter().map(|&n| n.into()).collect()
}

#[test]
fn a_valid_flow_compiles_to_its_cells_in_order_with_the_blocks_each_reads_and_writes() {
    let compiled = compiled(&valid());
    let mut plan = Cell::new("plan");
    plan.inputs = blocks(&["request", "opinions"]);
    plan.outputs = blocks(&["plan"]);
    let mut judge = Cell::new("judge");
    judge.inputs = blocks(&["plan"]);
    judge.outputs = blocks(&["needs_design", "opinions"]);
    let mut design = Cell::new("design");
    design.applies = Some(Condition::Is {
        block: "needs_design".into(),
        value: "yes".to_string(),
    });
    design.inputs = blocks(&["plan"]);
    design.outputs = blocks(&["design"]);
    let mut answer = Cell::new("answer");
    answer.applies = Some(Condition::Test("has_comment".into()));
    answer.requires = vec![TestName::from("plan_confirmed")];
    answer.inputs = blocks(&["comments", "plan"]);
    answer.outputs = blocks(&["reply"]);
    answer.limit = 5;
    let mut check = Cell::new("check");
    check.inputs = blocks(&["main", "design"]);
    check.outputs = blocks(&["ci"]);
    let mut merge = Cell::new("merge");
    merge.requires = vec![TestName::from("may_merge")];
    merge.inputs = blocks(&["ci"]);
    let expected = [plan, judge, design, answer, check, merge];

    let cells = compiled.flow.cells();
    assert_eq!(cells.len(), expected.len());
    for (cell, expected) in cells.iter().zip(&expected) {
        assert_eq!(cell.id, expected.id);
        assert_eq!(cell.applies, expected.applies, "{}", cell.id);
        assert_eq!(cell.requires, expected.requires, "{}", cell.id);
        assert_eq!(cell.inputs, expected.inputs, "{}", cell.id);
        assert_eq!(cell.outputs, expected.outputs, "{}", cell.id);
        assert_eq!(cell.limit, expected.limit, "{}", cell.id);
    }
}

#[test]
fn a_valid_flow_compiles_to_what_the_engine_needs_of_its_cells_blocks_tests_and_limits() {
    let compiled = compiled(&valid());
    assert_eq!(compiled.name, "small");
    let limits = Limits {
        runs: 3,
        returns: 2,
        calls: 40,
    };
    assert_eq!(compiled.limits, limits);

    let spec = |id: &str, title: &str, kind, line: Option<&str>| CellSpec {
        id: id.into(),
        title: title.to_string(),
        kind,
        line: line.map(str::to_string),
        reads_only: false,
        task: None,
        act: None,
        must_write: Vec::new(),
        describes: Vec::new(),
        input: BTreeMap::new(),
        returns: 2,
        calls: 40,
        effort: None,
        levels_above: 0,
        takes_main: false,
    };
    let agent = |id: &str, title: &str, line: &str, task: &str| CellSpec {
        task: Some(task.to_string()),
        effort: Some(Effort::High),
        ..spec(id, title, Kind::Agent, Some(line))
    };
    let engine = |id: &str, title: &str, act: &str| CellSpec {
        act: Some(act.to_string()),
        ..spec(id, title, Kind::Engine, None)
    };
    assert_eq!(
        compiled.cells,
        [
            agent("plan", "Plan", "engineer", "Write the plan from the request."),
            agent(
                "judge",
                "Judge the plan",
                "review",
                "Judge the plan, and say whether it needs a design.",
            ),
            agent("design", "Design", "engineer", "Design what the plan asks for."),
            // A task of several lines keeps its line breaks.
            agent(
                "answer",
                "Answer comments",
                "engineer",
                "Answer the comments on the plan.\nSay what you changed for each.\n",
            ),
            engine("check", "Check with the latest main", "check"),
            engine("merge", "Merge", "merge"),
        ]
    );

    let ids = |cells: &[&str]| cells.iter().map(|&c| c.into()).collect();
    let block = |values: Option<&[&str]>,
                 writers: &[&str],
                 who: (bool, Option<&str>),
                 readers: &[&str],
                 conditions: &[&str]| {
        Block {
            values: values.map(|v| v.iter().map(|s| s.to_string()).collect()),
            what: None,
            writers: Writers {
                cells: ids(writers),
                people: who.0,
                note: false,
                outside: who.1.map(str::to_string),
            },
            readers: Readers {
                cells: ids(readers),
                conditions: ids(conditions),
                tests: Vec::new(),
                shown: false,
            },
        }
    };
    let none = (false, None);
    let people = (true, None);
    let yes_no: Option<&[&str]> = Some(&["yes", "no"]);
    let mut reply = block(None, &["answer"], none, &[], &[]);
    reply.readers.shown = true;
    let tested = |mut block: Block, test: &str| {
        block.readers.tests = vec![test.into()];
        block
    };
    let expected: BTreeMap<BlockId, Block> = [
        ("request", block(None, &[], people, &["plan"], &[])),
        (
            "plan",
            block(None, &["plan"], none, &["judge", "design", "answer"], &[]),
        ),
        ("opinions", block(None, &["judge"], people, &["plan"], &[])),
        (
            "needs_design",
            block(Some(&["yes", "no"]), &["judge"], none, &[], &["design"]),
        ),
        ("design", block(None, &["design"], none, &["check"], &[])),
        ("comments", block(None, &[], people, &["answer"], &[])),
        ("reply", reply),
        ("main", block(None, &[], (false, Some("main")), &["check"], &[])),
        (
            "ci",
            tested(
                block(Some(&["green", "red"]), &["check"], none, &["merge"], &[]),
                "may_merge",
            ),
        ),
        ("commented", tested(block(yes_no, &[], people, &[], &[]), "has_comment")),
        (
            "confirmed",
            tested(block(yes_no, &[], people, &[], &[]), "plan_confirmed"),
        ),
    ]
    .into_iter()
    .map(|(name, b)| (name.into(), b))
    .collect();
    assert_eq!(compiled.blocks, expected);

    let words: Vec<(&str, &str)> = compiled.tests.iter().map(|(t, w)| (t.as_str(), w.as_str())).collect();
    assert_eq!(
        words,
        [
            ("has_comment", "The item has a comment"),
            ("may_merge", "The work may merge"),
            ("plan_confirmed", "The owner confirmed the plan"),
        ]
    );
    let test = |kind, reads: &str, is: &str| Test {
        kind,
        reads: vec![reads.into()],
        is: is.to_string(),
    };
    let expected: BTreeMap<TestName, Test> = [
        ("has_comment", test(TestKind::Any, "commented", "yes")),
        ("may_merge", test(TestKind::None, "ci", "red")),
        ("plan_confirmed", test(TestKind::All, "confirmed", "yes")),
    ]
    .into_iter()
    .map(|(name, t)| (name.into(), t))
    .collect();
    assert_eq!(*compiled.flow.tests(), expected);
}

#[test]
fn faults_in_several_files_are_all_reported_flow_toml_first_then_the_cells_in_the_order_of_the_walk() {
    let source = edit(
        valid(),
        "check.toml",
        "kind = \"engine\"",
        "kind = \"engine\"\nline = \"engineer\"",
    );
    let source = edit(source, "plan.toml", "line = \"engineer\"", "line = \"designer\"");
    let source = edit(source, FLOW_FILE, "calls = 40", "calls = 0");
    let no_line = NoSuchLine {
        line: "designer".to_string(),
    };
    let engine = EngineHasLine {
        line: "engineer".to_string(),
    };
    assert_eq!(
        faults(&source),
        [
            at(FLOW_FILE, "limits.calls", LimitOutOfRange { value: 0 }),
            at("plan.toml", "line", no_line),
            at("check.toml", "line", engine),
        ]
    );
}
