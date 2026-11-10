//! The named tests: each a cell uses is declared, each declared is used and reads choices that offer the value it
//! looks for, and a cell uses none that reads what the cell writes or what nothing before it writes.

use flow::{FLOW_FILE, FaultKind::*};

use crate::support::{at, compiled, edit, faults, unreadable, valid};

const REQUIRES: &str = r#"requires = ["may_merge"]"#;

const MAY_MERGE_READS: &str = r#"reads = ["ci"]"#;

#[test]
fn every_test_a_cell_requires_is_declared_and_named_once() {
    let source = edit(
        valid(),
        "merge.toml",
        REQUIRES,
        r#"requires = ["may_merge", "nobody", "may_merge"]"#,
    );
    let undeclared = UndeclaredTest {
        test: "nobody".to_string(),
    };
    let twice = Twice {
        name: "may_merge".to_string(),
    };
    assert_eq!(
        faults(&source),
        [
            at("merge.toml", "requires[1]", undeclared),
            at("merge.toml", "requires[2]", twice),
        ]
    );
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
fn a_test_reads_at_least_one_block() {
    let source = edit(valid(), FLOW_FILE, MAY_MERGE_READS, "reads = []");
    assert_eq!(faults(&source), [at(FLOW_FILE, "tests.may_merge.reads", ReadsEmpty)]);
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
fn a_cell_s_condition_or_precondition_is_no_test_that_reads_what_the_cell_writes() {
    let both = format!("kind = \"engine\"\nwhen = {{ test = \"may_merge\" }}\n{REQUIRES}");
    let source = edit(valid(), "check.toml", "kind = \"engine\"", &both);
    let what = || TestOnOwnOutput {
        test: "may_merge".into(),
        block: "ci".into(),
    };
    assert_eq!(
        faults(&source),
        [
            at("check.toml", "when.test", what()),
            at("check.toml", "requires[0]", what()),
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
            at("plan.toml", "requires[0]", what()),
            at("judge.toml", "when.test", what()),
        ]
    );
}

#[test]
fn a_test_may_read_choices_people_or_the_outside_write_before_any_cell() {
    let checks = "[blocks.checks]\nvalues = [\"pass\", \"fail\"]\noutside = \"repository\"\n\n[blocks.plan]";
    let source = edit(valid(), FLOW_FILE, "[blocks.plan]", checks);
    let checked =
        "\n[tests.checked]\nsays = \"The checks passed\"\nkind = \"all\"\nreads = [\"checks\"]\nis = \"pass\"\n";
    let source = edit(source, FLOW_FILE, "is = \"red\"\n", &format!("is = \"red\"\n{checked}"));
    let source = edit(
        source,
        "plan.toml",
        "line = \"engineer\"",
        "line = \"engineer\"\nrequires = [\"plan_confirmed\", \"checked\"]",
    );
    let compiled = compiled(&source);
    assert_eq!(
        compiled.flow.cells()[0].requires,
        ["plan_confirmed".into(), "checked".into()]
    );
    assert_eq!(compiled.blocks[&"checks".into()].readers.tests, ["checked".into()]);
}
