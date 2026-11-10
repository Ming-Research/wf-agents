//! The conditions: on a choice written before the cell, or on a declared test, and none on the last cell.

use decide::Condition;
use flow::{FLOW_FILE, FaultKind::*, Source};

use crate::support::{at, compiled, edit, faults, valid};

const WHEN: &str = "when = { block = \"needs_design\", is = \"yes\" }";

/// The small flow with the design's condition reading another block, the judge's choice being shown on the page so
/// that it keeps a reader.
fn design_reads(block: &str, source: Source) -> Source {
    let source = edit(
        source,
        FLOW_FILE,
        "[blocks.needs_design]",
        "[blocks.needs_design]\nshown = true",
    );
    edit(
        source,
        "design.toml",
        "block = \"needs_design\"",
        &format!("block = \"{block}\""),
    )
}

/// The small flow with a choice `go` that people or the outside write, as this line of its declaration says.
fn with_go(writer: &str) -> Source {
    let go = format!("[blocks.go]\nvalues = [\"yes\", \"no\"]\n{writer}\n\n[blocks.plan]");
    edit(valid(), FLOW_FILE, "[blocks.plan]", &go)
}

#[test]
fn a_condition_reads_a_declared_block() {
    let source = design_reads("ghost", valid());
    let what = NoSuchBlock {
        block: "ghost".to_string(),
    };
    assert_eq!(faults(&source), [at("design.toml", "when.block", what)]);
}

#[test]
fn a_condition_reads_a_choice() {
    let source = design_reads("plan", valid());
    let what = NotAChoice {
        block: "plan".to_string(),
    };
    assert_eq!(faults(&source), [at("design.toml", "when.block", what)]);
}

#[test]
fn a_condition_waits_for_a_value_the_choice_offers() {
    let source = edit(valid(), "design.toml", "is = \"yes\"", "is = \"maybe\"");
    let what = NotAValue {
        block: "needs_design".to_string(),
        is: "maybe".to_string(),
    };
    assert_eq!(faults(&source), [at("design.toml", "when.is", what)]);
}

#[test]
fn a_condition_reads_no_block_its_own_cell_writes() {
    let source = edit(
        valid(),
        "judge.toml",
        "line = \"review\"",
        &format!("line = \"review\"\n{WHEN}"),
    );
    let what = ConditionOnOwnOutput {
        block: "needs_design".to_string(),
    };
    assert_eq!(faults(&source), [at("judge.toml", "when.block", what)]);
}

#[test]
fn a_condition_reads_a_choice_a_cell_before_it_writes_and_not_one_only_a_cell_after_it_or_the_outside_writes() {
    let source = edit(
        valid(),
        "plan.toml",
        "line = \"engineer\"",
        &format!("line = \"engineer\"\n{WHEN}"),
    );
    let what = ConditionNotWrittenBefore {
        block: "needs_design".to_string(),
    };
    assert_eq!(faults(&source), [at("plan.toml", "when.block", what)]);

    let source = design_reads("go", with_go("outside = \"repository\""));
    let what = ConditionNotWrittenBefore {
        block: "go".to_string(),
    };
    assert_eq!(faults(&source), [at("design.toml", "when.block", what)]);
}

#[test]
fn a_condition_may_read_a_choice_people_write() {
    let compiled = compiled(&design_reads("go", with_go("people = true")));
    let condition = Condition::Is {
        block: "go".into(),
        value: "yes".to_string(),
    };
    assert_eq!(compiled.flow.cells()[2].applies, Some(condition));
}

#[test]
fn a_condition_on_a_test_names_a_declared_test() {
    let source = edit(valid(), "answer.toml", "test = \"has_comment\"", "test = \"nothing\"");
    let unused = UnusedTest {
        test: "has_comment".to_string(),
    };
    let undeclared = UndeclaredTest {
        test: "nothing".to_string(),
    };
    assert_eq!(
        faults(&source),
        [
            at(FLOW_FILE, "tests.has_comment", unused),
            at("answer.toml", "when.test", undeclared),
        ]
    );
}

#[test]
fn the_last_cell_has_no_condition() {
    let when = "kind = \"engine\"\nwhen = { test = \"has_comment\" }";
    let source = edit(valid(), "merge.toml", "kind = \"engine\"", when);
    assert_eq!(faults(&source), [at("merge.toml", "when", LastHasCondition)]);
}
