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
fn a_condition_reads_a_choice_a_cell_before_it_writes_and_not_one_only_a_cell_after_it_writes() {
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
}

#[test]
fn a_condition_may_read_a_choice_people_or_the_outside_write() {
    for writer in ["people = true", "outside = \"main\""] {
        let compiled = compiled(&design_reads("go", with_go(writer)));
        let condition = Condition::Is {
            block: "go".into(),
            value: "yes".to_string(),
        };
        assert_eq!(compiled.flow.cells()[2].applies, Some(condition), "{writer}");
    }
}

/// The small flow with the design's condition reading `go` as a block, or through a named test `go_on` that reads it
/// alone, as this line of the condition says.
fn design_runs_when(when: &str, writer: &str) -> Source {
    let test = "[tests.go_on]\nsays = \"Go on\"\nkind = \"all\"\nreads = [\"go\"]\nis = \"yes\"\n\n[tests.has_comment]";
    let source = edit(with_go(writer), FLOW_FILE, "[tests.has_comment]", test);
    let source = edit(
        source,
        FLOW_FILE,
        "[blocks.needs_design]",
        "[blocks.needs_design]\nshown = true",
    );
    edit(
        source,
        "design.toml",
        "when = { block = \"needs_design\", is = \"yes\" }",
        when,
    )
}

#[test]
fn a_condition_on_a_block_and_one_by_a_test_of_that_block_alone_agree_on_who_writes_it_before() {
    let by_block = "when = { block = \"go\", is = \"yes\" }";
    let by_test = "when = { test = \"go_on\" }";
    // Written before the cell by people or by the outside: both compile.
    for writer in ["people = true", "outside = \"main\""] {
        let source = design_runs_when(by_test, writer);
        compiled(&source);
        let source = edit(
            source,
            "design.toml",
            by_test,
            &format!("{by_block}\nrequires = [\"go_on\"]"),
        );
        compiled(&source);
    }
    // Written only by a cell after it: both are refused, each with its own fault.
    let after = |source: Source| edit(source, "check.toml", "outputs = [\"ci\"]", "outputs = [\"ci\", \"go\"]");
    let go = || "go".to_string();
    let source = after(design_runs_when(by_block, ""));
    // The test stays in use, by the merge, after the cell that writes go.
    let source = edit(
        source,
        "merge.toml",
        "requires = [\"may_merge\"]",
        "requires = [\"may_merge\", \"go_on\"]",
    );
    let not_before = ConditionNotWrittenBefore { block: go() };
    assert_eq!(faults(&source), [at("design.toml", "when.block", not_before)]);
    let source = after(design_runs_when(by_test, ""));
    let not_before = TestWrittenAfter {
        test: "go_on".to_string(),
        block: go(),
        cell: "check".to_string(),
    };
    assert_eq!(faults(&source), [at("design.toml", "when.test", not_before)]);
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
    // The test no cell uses then reads nothing, and the choice only it read has no reader.
    let unread = NoReader {
        block: "commented".to_string(),
    };
    assert_eq!(
        faults(&source),
        [
            at(FLOW_FILE, "blocks.commented", unread),
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
