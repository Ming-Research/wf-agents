//! The blocks: each is declared, each a cell reads has a writer, each a cell writes has a reader, and a choice has
//! values.

use flow::{FLOW_FILE, FaultKind::*};

use crate::support::{at, compiled, edit, faults, faults_with, unreadable, valid};

const PLAN_INPUTS: &str = r#"inputs = ["request", "opinions"]"#;

#[test]
fn every_block_a_cell_reads_or_writes_is_declared() {
    let source = edit(
        valid(),
        "plan.toml",
        PLAN_INPUTS,
        r#"inputs = ["request", "opinions", "nothing"]"#,
    );
    let source = edit(
        source,
        "plan.toml",
        r#"outputs = ["plan"]"#,
        r#"outputs = ["plan", "nowhere"]"#,
    );
    let no_such = |block: &str| NoSuchBlock {
        block: block.to_string(),
    };
    assert_eq!(
        faults(&source),
        [
            at("plan.toml", "inputs[2]", no_such("nothing")),
            at("plan.toml", "outputs[1]", no_such("nowhere")),
        ]
    );
}

#[test]
fn a_cell_names_a_block_once_among_its_inputs_and_once_among_its_outputs() {
    let source = edit(
        valid(),
        "plan.toml",
        PLAN_INPUTS,
        r#"inputs = ["request", "opinions", "request"]"#,
    );
    let source = edit(
        source,
        "plan.toml",
        r#"outputs = ["plan"]"#,
        r#"outputs = ["plan", "plan"]"#,
    );
    let twice = |name: &str| Twice { name: name.to_string() };
    assert_eq!(
        faults(&source),
        [
            at("plan.toml", "inputs[2]", twice("request")),
            at("plan.toml", "outputs[1]", twice("plan")),
        ]
    );
}

#[test]
fn every_block_is_written_by_a_cell_people_or_the_outside() {
    let source = edit(valid(), FLOW_FILE, "[blocks.plan]", "[blocks.notes]\n\n[blocks.plan]");
    let source = edit(
        source,
        "plan.toml",
        PLAN_INPUTS,
        r#"inputs = ["request", "opinions", "notes"]"#,
    );
    let what = NoWriter {
        block: "notes".to_string(),
    };
    assert_eq!(faults(&source), [at(FLOW_FILE, "blocks.notes", what)]);
}

#[test]
fn a_block_shown_on_the_page_still_has_a_writer() {
    let source = edit(
        valid(),
        FLOW_FILE,
        "[blocks.plan]",
        "[blocks.notes]\nshown = true\n\n[blocks.plan]",
    );
    let what = NoWriter {
        block: "notes".to_string(),
    };
    assert_eq!(faults(&source), [at(FLOW_FILE, "blocks.notes", what)]);
}

#[test]
fn a_block_a_cell_reads_may_be_written_by_a_cell_after_it() {
    let compiled = compiled(&valid());
    let plan = &compiled.flow.cells()[0];
    assert!(plan.inputs.contains(&"opinions".into()));
    assert_eq!(compiled.blocks[&"opinions".into()].writers.cells, ["judge".into()]);
}

#[test]
fn every_block_is_read_by_a_cell_a_condition_the_page_or_a_named_test() {
    let source = edit(valid(), FLOW_FILE, "[blocks.plan]", "[blocks.log]\n\n[blocks.plan]");
    let outputs = r#"outputs = ["needs_design", "opinions"]"#;
    let with_log = r#"outputs = ["needs_design", "opinions", "log"]"#;
    let source = edit(source, "judge.toml", outputs, with_log);
    let what = NoReader {
        block: "log".to_string(),
    };
    assert_eq!(faults(&source), [at(FLOW_FILE, "blocks.log", what)]);

    let shown = edit(
        valid(),
        FLOW_FILE,
        "[blocks.plan]",
        "[blocks.log]\nshown = true\n\n[blocks.plan]",
    );
    compiled(&edit(shown, "judge.toml", outputs, with_log));

    // A choice that only a named test reads has a reader too.
    let commented = &compiled(&valid()).blocks[&"commented".into()];
    assert_eq!(
        (commented.readers.cells.len(), commented.readers.conditions.len()),
        (0, 0)
    );
    assert_eq!(commented.readers.tests, ["has_comment".into()]);
    assert!(!commented.readers.shown);
}

#[test]
fn a_block_the_outside_writes_names_a_source_the_engine_has() {
    let source = edit(valid(), FLOW_FILE, "outside = \"repository\"", "outside = \"mirror\"");
    let what = UnknownSource {
        block: "main".to_string(),
        source: "mirror".to_string(),
    };
    assert_eq!(faults(&source), [at(FLOW_FILE, "blocks.main.outside", what)]);

    let what = UnknownSource {
        block: "main".to_string(),
        source: "repository".to_string(),
    };
    assert_eq!(
        faults_with(&valid(), &Default::default()),
        [at(FLOW_FILE, "blocks.main.outside", what)]
    );

    // That the outside writes a block is said by naming what writes it.
    let source = edit(valid(), FLOW_FILE, "outside = \"repository\"", "outside = true");
    assert!(!unreadable(&source, FLOW_FILE).is_empty());
}

#[test]
fn a_block_people_write_still_has_a_reader() {
    let source = edit(
        valid(),
        FLOW_FILE,
        "[blocks.plan]",
        "[blocks.notes]\npeople = true\n\n[blocks.plan]",
    );
    let what = NoReader {
        block: "notes".to_string(),
    };
    assert_eq!(faults(&source), [at(FLOW_FILE, "blocks.notes", what)]);
}

#[test]
fn a_block_that_nothing_writes_and_nothing_reads_has_both_faults_the_writer_first() {
    let source = edit(valid(), FLOW_FILE, "[blocks.plan]", "[blocks.spare]\n\n[blocks.plan]");
    let block = || "spare".to_string();
    assert_eq!(
        faults(&source),
        [
            at(FLOW_FILE, "blocks.spare", NoWriter { block: block() }),
            at(FLOW_FILE, "blocks.spare", NoReader { block: block() }),
        ]
    );
}

#[test]
fn a_choice_has_values_each_given_once() {
    let ci = "values = [\"green\", \"red\"]";
    let source = edit(valid(), FLOW_FILE, ci, "values = []");
    // The test that reads ci then looks for a value it does not offer.
    let red = NotAValue {
        block: "ci".to_string(),
        is: "red".to_string(),
    };
    assert_eq!(
        faults(&source),
        [
            at(FLOW_FILE, "blocks.ci.values", ValuesEmpty),
            at(FLOW_FILE, "tests.may_merge.reads[0]", red),
        ]
    );

    let source = edit(valid(), FLOW_FILE, ci, "values = [\"green\", \"red\", \"green\"]");
    let what = Twice {
        name: "green".to_string(),
    };
    assert_eq!(faults(&source), [at(FLOW_FILE, "blocks.ci.values[2]", what)]);
}
