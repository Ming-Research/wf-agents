//! The named tests: each a cell uses is declared, each declared is used and reads choices that offer the value it
//! looks for, or, of a test of content, any declared block and no value, and a cell uses none that reads what the cell
//! writes or what nothing before it writes.

use decide::{BlockId, TestName};
use flow::{FLOW_FILE, FaultKind::*, TestKind};

use crate::support::{at, compiled, edit, faults, unreadable, valid};

const REQUIRES: &str = r#"requires = { test = "may_merge" }"#;

const MAY_MERGE_READS: &str = r#"reads = ["ci"]"#;

#[test]
fn every_test_a_cell_requires_is_declared() {
    let source = edit(valid(), "merge.toml", REQUIRES, r#"requires = { test = "nobody" }"#);
    let undeclared = UndeclaredTest {
        test: "nobody".to_string(),
    };
    // The test the merge named before is then used by no cell.
    let unused = UnusedTest {
        test: "may_merge".to_string(),
    };
    assert_eq!(
        faults(&source),
        [
            at(FLOW_FILE, "tests.may_merge", unused),
            at("merge.toml", "requires.test", undeclared),
        ]
    );
}

#[test]
fn a_requires_written_as_a_list_is_a_file_the_parser_refuses() {
    let source = edit(valid(), "merge.toml", REQUIRES, r#"requires = ["may_merge"]"#);
    assert!(!unreadable(&source, "merge.toml").is_empty());
}

#[test]
fn a_test_of_a_kind_there_is_no_function_for_is_a_file_the_parser_refuses() {
    let source = edit(valid(), FLOW_FILE, "kind = \"none\"", "kind = \"most\"");
    assert!(unreadable(&source, FLOW_FILE).contains("most"));
}

#[test]
fn every_declared_test_is_used_by_a_cell() {
    let extra = "\n[tests.extra]\nsays = \"Something else\"\nkind = \"all\"\nreads = [\"ci\"]\nis = \"green\"\n";
    let source = edit(valid(), FLOW_FILE, "is = \"red\"\n", &format!("is = \"red\"\n{extra}"));
    let what = UnusedTest {
        test: "extra".to_string(),
    };
    assert_eq!(faults(&source), [at(FLOW_FILE, "tests.extra", what)]);
}

#[test]
fn a_test_no_cell_uses_makes_nothing_it_reads_read() {
    let extra = "\n[tests.extra]\nsays = \"Spare\"\nkind = \"all\"\nreads = [\"spare\"]\nis = \"yes\"\n";
    let source = edit(valid(), FLOW_FILE, "is = \"red\"\n", &format!("is = \"red\"\n{extra}"));
    let spare = "[blocks.spare]\nvalues = [\"yes\", \"no\"]\npeople = true\n\n[blocks.plan]";
    let source = edit(source, FLOW_FILE, "[blocks.plan]", spare);
    let unread = NoReader {
        block: "spare".to_string(),
    };
    let unused = UnusedTest {
        test: "extra".to_string(),
    };
    assert_eq!(
        faults(&source),
        [
            at(FLOW_FILE, "blocks.spare", unread),
            at(FLOW_FILE, "tests.extra", unused),
        ]
    );
}

/// The test the answer to comments runs by, made one of content reading the choice and the comments people write.
const HAS_COMMENT: &str = "kind = \"any\"\nreads = [\"commented\"]\nis = \"yes\"";
const OF_CONTENT: &str = "kind = \"any_content\"\nreads = [\"commented\", \"comments\"]";

#[test]
fn a_test_of_content_reads_any_declared_block_and_looks_for_no_value() {
    let source = edit(valid(), FLOW_FILE, HAS_COMMENT, OF_CONTENT);
    let flow = compiled(&source);
    let test = &flow.flow.tests()[&TestName::from("has_comment")];
    assert_eq!(test.kind, TestKind::AnyContent);
    assert_eq!(test.reads, [BlockId::from("commented"), BlockId::from("comments")]);
    assert_eq!(test.is, None);
    // One that names a value is refused there, and so is one that reads a block the flow does not declare.
    let named = edit(
        source.clone(),
        FLOW_FILE,
        OF_CONTENT,
        &format!("{OF_CONTENT}\nis = \"yes\""),
    );
    let what = ContentTestNamesValue {
        test: "has_comment".to_string(),
    };
    assert_eq!(faults(&named), [at(FLOW_FILE, "tests.has_comment.is", what)]);
    let ghost = edit(source, FLOW_FILE, "\"comments\"]", "\"comments\", \"ghost\"]");
    let what = NoSuchBlock {
        block: "ghost".to_string(),
    };
    assert_eq!(faults(&ghost), [at(FLOW_FILE, "tests.has_comment.reads[2]", what)]);
}

#[test]
fn a_test_of_a_value_names_the_value_it_looks_for() {
    let source = edit(valid(), FLOW_FILE, "reads = [\"ci\"]\nis = \"red\"", "reads = [\"ci\"]");
    let what = TestNamesNoValue {
        test: "may_merge".to_string(),
    };
    assert_eq!(faults(&source), [at(FLOW_FILE, "tests.may_merge.is", what)]);
}

#[test]
fn a_test_reads_at_least_one_block() {
    let source = edit(valid(), FLOW_FILE, MAY_MERGE_READS, "reads = []");
    let what = ReadsEmpty {
        test: "may_merge".to_string(),
    };
    assert_eq!(faults(&source), [at(FLOW_FILE, "tests.may_merge.reads", what)]);
}

#[test]
fn a_test_reads_declared_choices_each_once_that_offer_the_value_it_looks_for() {
    let reads = r#"reads = ["ghost", "plan", "ci", "needs_design", "ci"]"#;
    let source = edit(valid(), FLOW_FILE, MAY_MERGE_READS, reads);
    let field = |i: usize| format!("tests.may_merge.reads[{i}]");
    let not_a_value = NotAValue {
        block: "needs_design".into(),
        is: "red".into(),
    };
    assert_eq!(
        faults(&source),
        [
            at(FLOW_FILE, &field(0), NoSuchBlock { block: "ghost".into() }),
            at(FLOW_FILE, &field(1), NotAChoice { block: "plan".into() }),
            at(FLOW_FILE, &field(3), not_a_value),
            at(FLOW_FILE, &field(4), Twice { name: "ci".into() }),
        ]
    );
}

#[test]
fn a_cell_s_precondition_is_no_test_that_reads_what_the_cell_writes_and_its_condition_may_be_where_it_alone_writes() {
    let both = format!("kind = \"engine\"\nwhen = {{ test = \"may_merge\" }}\n{REQUIRES}");
    let source = edit(valid(), "check.toml", "kind = \"engine\"", &both);
    let what = || TestOnOwnOutput {
        test: "may_merge".into(),
        block: "ci".into(),
    };
    assert_eq!(faults(&source), [at("check.toml", "requires.test", what())]);
    let source = edit(source, FLOW_FILE, "[blocks.ci]\n", "[blocks.ci]\npeople = true\n");
    assert_eq!(
        faults(&source),
        [
            at("check.toml", "when.test", what()),
            at("check.toml", "requires.test", what()),
        ]
    );
}

#[test]
fn a_cell_s_condition_or_precondition_is_no_test_that_reads_what_only_a_cell_after_it_writes() {
    let source = edit(
        valid(),
        "plan.toml",
        "line = \"engineer\"",
        &format!("line = \"engineer\"\n{REQUIRES}"),
    );
    let source = edit(
        source,
        "judge.toml",
        "line = \"review\"",
        "line = \"review\"\nwhen = { test = \"may_merge\" }",
    );
    let what = || TestNotWrittenBefore {
        test: "may_merge".into(),
        block: "ci".into(),
    };
    assert_eq!(
        faults(&source),
        [
            at("plan.toml", "requires.test", what()),
            at("judge.toml", "when.test", what()),
        ]
    );
}

#[test]
fn a_test_may_read_choices_people_or_the_outside_write_before_any_cell() {
    let checks = "[blocks.checks]\nvalues = [\"pass\", \"fail\"]\noutside = \"main\"\n\n[blocks.plan]";
    let source = edit(valid(), FLOW_FILE, "[blocks.plan]", checks);
    let checked =
        "\n[tests.checked]\nsays = \"The checks passed\"\nkind = \"all\"\nreads = [\"checks\"]\nis = \"pass\"\n";
    let source = edit(source, FLOW_FILE, "is = \"red\"\n", &format!("is = \"red\"\n{checked}"));
    let source = edit(
        source,
        "plan.toml",
        "line = \"engineer\"",
        "line = \"engineer\"\nrequires = { test = \"checked\" }",
    );
    // The answer still requires plan_confirmed.
    let compiled = compiled(&source);
    assert_eq!(compiled.flow.cells()[0].requires, ["checked".into()]);
    assert_eq!(compiled.blocks[&"checks".into()].readers.tests, ["checked".into()]);
}

#[test]
fn a_test_of_what_stands_reads_no_block_a_cell_after_its_cell_writes_and_another_kind_may() {
    // The judge, before the answer, writes needs_design; the check, after it, writes it too.
    let source = edit(
        valid(),
        "check.toml",
        r#"outputs = ["ci"]"#,
        r#"outputs = ["ci", "needs_design"]"#,
    );
    let reads = r#"reads = ["confirmed", "needs_design"]"#;
    let source = edit(source, FLOW_FILE, r#"reads = ["confirmed"]"#, reads);
    // A test that every block has the value is held to the rule of a condition on a block: written before is enough.
    compiled(&source);
    let all = "kind = \"all\"\nreads = [\"confirmed\", \"needs_design\"]";
    let standing = "kind = \"none_standing\"\nreads = [\"confirmed\", \"needs_design\"]";
    let source = edit(source, FLOW_FILE, all, standing);
    let after = TestWrittenAfter {
        test: "plan_confirmed".into(),
        block: "needs_design".into(),
        cell: "check".into(),
    };
    let two = StandingWrittenByTwo {
        test: "plan_confirmed".into(),
        block: "needs_design".into(),
        cell: "check".into(),
    };
    assert_eq!(
        faults(&source),
        [
            at(FLOW_FILE, "tests.plan_confirmed.reads[1]", two),
            at("answer.toml", "requires.test", after),
        ]
    );
}

#[test]
fn a_test_of_what_stands_reads_only_blocks_one_cell_writes_and_people_or_the_outside_may_write_them_too() {
    let standing = "kind = \"none_standing\"\nreads = [\"ci\"]";
    let source = edit(valid(), FLOW_FILE, "kind = \"none\"\nreads = [\"ci\"]", standing);
    // The check alone writes ci, and people too.
    compiled(&source);
    let source = edit(
        source,
        FLOW_FILE,
        "values = [\"green\", \"red\"]",
        "values = [\"green\", \"red\"]\npeople = true",
    );
    compiled(&source);
    // The judge writes it too: a skipped judge's write could hide the other's word, and is refused.
    let source = edit(
        source,
        "judge.toml",
        r#"outputs = ["needs_design", "opinions"]"#,
        r#"outputs = ["needs_design", "opinions", "ci"]"#,
    );
    let what = StandingWrittenByTwo {
        test: "may_merge".into(),
        block: "ci".into(),
        cell: "check".into(),
    };
    assert_eq!(faults(&source), [at(FLOW_FILE, "tests.may_merge.reads[0]", what)]);
}

#[test]
fn a_test_without_a_writer_before_or_after_its_cell_is_not_written_before_it() {
    let source = edit(
        valid(),
        FLOW_FILE,
        "[blocks.plan]",
        "[blocks.never]\nvalues = [\"yes\", \"no\"]\n\n[blocks.plan]",
    );
    let source = edit(
        source,
        FLOW_FILE,
        r#"reads = ["confirmed"]"#,
        r#"reads = ["confirmed", "never"]"#,
    );
    let not_written = NoWriter {
        block: "never".to_string(),
    };
    let not_before = TestNotWrittenBefore {
        test: "plan_confirmed".into(),
        block: "never".into(),
    };
    assert_eq!(
        faults(&source),
        [
            at(FLOW_FILE, "blocks.never", not_written),
            at("answer.toml", "requires.test", not_before),
        ]
    );
}
