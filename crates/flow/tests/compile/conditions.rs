//! The conditions, `when` and `requires`: tables of one key or more, each on a choice written before the cell or on a
//! declared test, every one of which holds; a choice that has a default counts as it until it is written; and no
//! `when` on the last cell.

use decide::Condition;
use flow::{FLOW_FILE, FaultKind::*, Source};

use crate::support::{at, compiled, edit, faults, valid};

const WHEN: &str = "when = { needs_design = \"yes\" }";

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
        "when = { needs_design = ",
        &format!("when = {{ {block} = "),
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
    assert_eq!(faults(&source), [at("design.toml", "when.ghost", what)]);
}

#[test]
fn a_condition_reads_a_choice() {
    let source = design_reads("plan", valid());
    let what = NotAChoice {
        block: "plan".to_string(),
    };
    assert_eq!(faults(&source), [at("design.toml", "when.plan", what)]);
}

#[test]
fn a_condition_waits_for_a_value_the_choice_offers() {
    let source = edit(valid(), "design.toml", WHEN, "when = { needs_design = \"maybe\" }");
    let what = NotAValue {
        block: "needs_design".to_string(),
        is: "maybe".to_string(),
    };
    assert_eq!(faults(&source), [at("design.toml", "when.needs_design", what)]);
}

#[test]
fn a_condition_of_no_keys_is_refused() {
    let source = edit(valid(), "design.toml", WHEN, "when = {}");
    // The judge's choice that the design's condition read then has no reader.
    let unread = NoReader {
        block: "needs_design".to_string(),
    };
    assert_eq!(
        faults(&source),
        [
            at(FLOW_FILE, "blocks.needs_design", unread),
            at("design.toml", "when", EmptyCondition),
        ]
    );
}

#[test]
fn a_cell_runs_when_every_key_of_its_condition_holds_in_the_order_written() {
    let both = "when = { needs_design = \"yes\", test = \"has_comment\" }";
    let source = edit(valid(), "design.toml", WHEN, both);
    let compiled = compiled(&source);
    // The test is used by the design too now, and the answer still.
    assert_eq!(
        compiled.flow.cells()[2].applies,
        [
            Condition::Is {
                block: "needs_design".into(),
                value: "yes".to_string(),
            },
            Condition::Test("has_comment".into()),
        ]
    );
}

#[test]
fn a_precondition_may_wait_on_the_value_of_a_choice() {
    let source = with_go("people = true");
    let source = edit(
        source,
        "merge.toml",
        "requires = { test = \"may_merge\" }",
        "requires = { test = \"may_merge\", go = \"yes\" }",
    );
    let compiled = compiled(&source);
    let merge = compiled.flow.cells().last().expect("the merge");
    assert_eq!(
        merge.requires,
        [
            Condition::Test("may_merge".into()),
            Condition::Is {
                block: "go".into(),
                value: "yes".to_string(),
            },
        ]
    );
    assert_eq!(compiled.blocks[&"go".into()].readers.conditions, ["merge".into()]);
}

#[test]
fn a_condition_may_read_a_choice_its_own_cell_alone_writes() {
    let source = edit(
        valid(),
        "judge.toml",
        "line = \"review\"",
        &format!("line = \"review\"\n{WHEN}"),
    );
    let condition = Condition::Is {
        block: "needs_design".into(),
        value: "yes".to_string(),
    };
    assert_eq!(compiled(&source).flow.cells()[1].applies, [condition]);
}

#[test]
fn a_condition_reads_no_block_its_own_cell_writes_and_another_writer_writes_too() {
    let source = edit(
        valid(),
        "judge.toml",
        "line = \"review\"",
        &format!("line = \"review\"\n{WHEN}"),
    );
    let source = edit(
        source,
        FLOW_FILE,
        "[blocks.needs_design]\n",
        "[blocks.needs_design]\npeople = true\n",
    );
    let what = ConditionOnOwnOutput {
        block: "needs_design".to_string(),
    };
    assert_eq!(faults(&source), [at("judge.toml", "when.needs_design", what)]);
}

#[test]
fn a_precondition_reads_no_block_its_own_cell_writes() {
    let source = edit(
        valid(),
        "judge.toml",
        "line = \"review\"",
        "line = \"review\"\nrequires = { needs_design = \"yes\" }",
    );
    let what = ConditionOnOwnOutput {
        block: "needs_design".to_string(),
    };
    assert_eq!(faults(&source), [at("judge.toml", "requires.needs_design", what)]);
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
    assert_eq!(faults(&source), [at("plan.toml", "when.needs_design", what)]);
}

#[test]
fn a_condition_may_read_a_choice_people_or_the_outside_write() {
    for writer in ["people = true", "outside = \"main\""] {
        let compiled = compiled(&design_reads("go", with_go(writer)));
        let condition = Condition::Is {
            block: "go".into(),
            value: "yes".to_string(),
        };
        assert_eq!(compiled.flow.cells()[2].applies, [condition], "{writer}");
    }
}

#[test]
fn a_choice_that_has_a_default_counts_as_it_until_it_is_written() {
    let source = edit(
        valid(),
        FLOW_FILE,
        "[blocks.needs_design]\nvalues = [\"yes\", \"no\"]",
        "[blocks.needs_design]\nvalues = [\"yes\", \"no\"]\ndefault = \"no\"",
    );
    let compiled = compiled(&source);
    assert_eq!(compiled.flow.default_of(&"needs_design".into()), Some("no"));
    assert_eq!(compiled.blocks[&"needs_design".into()].default.as_deref(), Some("no"));
    assert_eq!(compiled.flow.default_of(&"commented".into()), None);
}

#[test]
fn a_default_is_one_of_the_choice_s_values_and_only_a_choice_has_one() {
    let source = edit(
        valid(),
        FLOW_FILE,
        "[blocks.needs_design]\nvalues = [\"yes\", \"no\"]",
        "[blocks.needs_design]\nvalues = [\"yes\", \"no\"]\ndefault = \"maybe\"",
    );
    let what = DefaultNotAValue {
        block: "needs_design".to_string(),
        default: "maybe".to_string(),
    };
    assert_eq!(faults(&source), [at(FLOW_FILE, "blocks.needs_design.default", what)]);
    let source = edit(
        valid(),
        FLOW_FILE,
        "[blocks.design]",
        "[blocks.design]\ndefault = \"yes\"",
    );
    let what = DefaultNotAChoice {
        block: "design".to_string(),
    };
    assert_eq!(faults(&source), [at(FLOW_FILE, "blocks.design.default", what)]);
}

#[test]
fn no_block_is_named_test_which_a_condition_takes_for_a_named_test() {
    let source = edit(
        valid(),
        FLOW_FILE,
        "[blocks.design]",
        "[blocks.test]\nvalues = [\"yes\", \"no\"]\npeople = true\nshown = true\n\n[blocks.design]",
    );
    assert_eq!(faults(&source), [at(FLOW_FILE, "blocks.test", BlockNamedTest)]);
}

/// The small flow with the design's condition reading `go` as a block, or through a named test `go_on` that reads it
/// alone, as this line of the condition says; `go_on` is declared where the line names it.
fn design_runs_when(when: &str, writer: &str) -> Source {
    let mut source = with_go(writer);
    if when.contains("go_on") {
        let test =
            "[tests.go_on]\nsays = \"Go on\"\nkind = \"all\"\nreads = [\"go\"]\nis = \"yes\"\n\n[tests.has_comment]";
        source = edit(source, FLOW_FILE, "[tests.has_comment]", test);
    }
    let source = edit(
        source,
        FLOW_FILE,
        "[blocks.needs_design]",
        "[blocks.needs_design]\nshown = true",
    );
    edit(source, "design.toml", WHEN, when)
}

#[test]
fn a_condition_on_a_block_and_a_condition_by_a_test_of_that_block_alone_are_held_to_the_same_rule() {
    let by_block = "when = { go = \"yes\" }";
    let by_test = "when = { test = \"go_on\" }";
    // Written before the cell by people or by the outside: both compile.
    for writer in ["people = true", "outside = \"main\""] {
        let source = design_runs_when(by_test, writer);
        compiled(&source);
        let source = edit(
            source,
            "design.toml",
            by_test,
            &format!("{by_block}\nrequires = {{ test = \"go_on\" }}"),
        );
        compiled(&source);
    }
    // Written by a cell before it and by one after it: both compile, the later write deciding again whether it runs.
    let before_and_after = |source: Source| {
        let source = edit(
            source,
            "judge.toml",
            r#"outputs = ["needs_design", "opinions"]"#,
            r#"outputs = ["needs_design", "opinions", "go"]"#,
        );
        edit(source, "check.toml", "outputs = [\"ci\"]", "outputs = [\"ci\", \"go\"]")
    };
    compiled(&before_and_after(design_runs_when(by_test, "")));
    compiled(&before_and_after(design_runs_when(by_block, "")));
    // Written only by a cell after it: both are refused, each with its own fault.
    let after = |source: Source| edit(source, "check.toml", "outputs = [\"ci\"]", "outputs = [\"ci\", \"go\"]");
    let go = || "go".to_string();
    let source = after(design_runs_when(by_block, ""));
    let not_before = ConditionNotWrittenBefore { block: go() };
    assert_eq!(faults(&source), [at("design.toml", "when.go", not_before)]);
    let source = after(design_runs_when(by_test, ""));
    let not_before = TestNotWrittenBefore {
        test: "go_on".to_string(),
        block: go(),
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
