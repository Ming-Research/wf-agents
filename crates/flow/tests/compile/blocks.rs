//! The blocks: each is declared, each a cell reads has a writer, each a cell writes has a reader, and a choice has
//! values.

use flow::{FLOW_FILE, FaultKind::*, Has};

use crate::support::{at, compiled, edit, faults, faults_with, has, unreadable, valid};

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
    assert!(plan.runs_on.contains(&"opinions".into()));
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
    let source = edit(valid(), FLOW_FILE, "outside = \"main\"", "outside = \"mirror\"");
    let what = UnknownSource {
        block: "main".to_string(),
        source: "mirror".to_string(),
    };
    assert_eq!(faults(&source), [at(FLOW_FILE, "blocks.main.outside", what)]);

    let what = UnknownSource {
        block: "main".to_string(),
        source: "main".to_string(),
    };
    let no_sources = Has {
        sources: Default::default(),
        ..has()
    };
    assert_eq!(
        faults_with(&valid(), &no_sources),
        [at(FLOW_FILE, "blocks.main.outside", what)]
    );

    // That the outside writes a block is said by naming what writes it.
    let source = edit(valid(), FLOW_FILE, "outside = \"main\"", "outside = true");
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

#[test]
fn a_cell_that_writes_the_work_s_questions_is_run_by_the_owner_s_answer_and_by_the_owner_s_words() {
    let asks = "[blocks.plan]\n\n[blocks.asks]\noutside = \"answer\"\nshown = true\n";
    let source = edit(valid(), FLOW_FILE, "[blocks.plan]\n", asks);
    let outputs = r#"outputs = ["plan", "asks"]"#;
    let source = edit(source, "plan.toml", r#"outputs = ["plan"]"#, outputs);
    let mut has = has();
    has.sources.insert("answer".to_string());
    has.sources.insert("words".to_string());
    let no_answer = AsksWithNoAnswer {
        block: "asks".to_string(),
    };
    let not_spoken_to = AsksAndCannotBeSpokenTo {
        block: "asks".to_string(),
    };
    assert_eq!(
        faults_with(&source, &has),
        [
            at("plan.toml", "outputs[1]", no_answer),
            at("plan.toml", "outputs[1]", not_spoken_to.clone()),
        ]
    );

    // Run by the answer alone, it can ask and cannot be spoken to about what it asks: refused still.
    let answered = format!("{asks}\n[blocks.answered]\noutside = \"answer\"\n");
    let source = edit(source, FLOW_FILE, asks, &answered);
    let inputs = r#"inputs = ["request", "opinions", "answered"]"#;
    let source = edit(source, "plan.toml", PLAN_INPUTS, inputs);
    assert_eq!(
        faults_with(&source, &has),
        [at("plan.toml", "outputs[1]", not_spoken_to)]
    );

    let spoken = format!("{answered}\n[blocks.spoken_to]\noutside = \"words\"\n");
    let source = edit(source, FLOW_FILE, &answered, &spoken);
    let inputs_spoken = r#"inputs = ["request", "opinions", "answered", "spoken_to"]"#;
    let source = edit(source, "plan.toml", inputs, inputs_spoken);
    let compiled = flow::compile(&source, &has);
    assert!(compiled.is_ok(), "{:?}", compiled.err());
}

// The records a source of the engine's adds entries to are the blocks of that source a trigger of it has its agent read,
// but the one it stands on: the record of what is said and that of the questions and the answers, and no other.
#[test]
fn the_records_entries_are_added_to_are_those_a_trigger_of_their_source_reads() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow/engineer");
    let source = flow::read(&dir).expect("the engineer's flow is read");
    let names = |names: &[&str]| names.iter().map(|n| n.to_string()).collect();
    let has = Has {
        sources: names(&[
            "answer",
            "asked",
            "dependencies",
            "main",
            "session",
            "start",
            "head",
            "tree",
            "words",
        ]),
        acts: names(&["start", "check", "merge", "run_tests"]),
        tools: names(&["query"]),
        details: names(&["files"]),
        ..Default::default()
    };
    let compiled = flow::compile(&source, &has).expect("the engineer's flow compiles");
    let records: Vec<&str> = (compiled.blocks.keys())
        .filter(|b| compiled.flow.adds_entries(b))
        .map(|b| b.as_str())
        .collect();
    assert_eq!(records, ["discussion", "q_and_a", "run_tests"]);
}

// The cells that write the record of the tests ask for them, each waiting for a block of its own, which the cell of the
// act that runs the tests has among its outputs, as the trigger of the result says the act writes it: the act hands the
// results in with its round's end, and each block holds what it handed in, standing for no trigger alone. Well under a
// second alone; no other test holds the compiler to where the results are written.
#[test]
fn the_cells_that_ask_for_tests_wait_for_results_the_act_that_runs_them_hands_in() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow/engineer");
    let source = flow::read(&dir).expect("the engineer's flow is read");
    let names = |names: &[&str]| names.iter().map(|n| n.to_string()).collect();
    let has = Has {
        sources: names(&[
            "answer",
            "asked",
            "dependencies",
            "main",
            "session",
            "start",
            "head",
            "tree",
            "words",
        ]),
        acts: names(&["start", "check", "merge", "run_tests"]),
        tools: names(&["query"]),
        details: names(&["files"]),
        ..Default::default()
    };
    let compiled = flow::compile(&source, &has).expect("the engineer's flow compiles");
    let asking: Vec<(&str, &str, &str)> = (compiled.flow.cells().iter())
        .flat_map(|c| {
            c.asks
                .iter()
                .map(move |a| (c.id.as_str(), a.record.as_str(), a.result.as_str()))
        })
        .collect();
    assert_eq!(
        asking,
        [
            ("R1", "run_tests", "r1_tested"),
            ("D1", "run_tests", "d1_tested"),
            ("I1", "run_tests", "i1_tested"),
            ("T2", "run_tests", "t2_tested")
        ]
    );
    let t3 = (compiled.flow.cells().iter())
        .find(|c| c.id.as_str() == "T3")
        .expect("the flow has T3");
    let mut outputs: Vec<&str> = t3.outputs.iter().map(|b| b.as_str()).collect();
    outputs.sort_unstable();
    assert_eq!(outputs, ["d1_tested", "i1_tested", "r1_tested", "t2_tested"]);
    for (cell, _, result) in asking {
        let block = &compiled.blocks[&decide::BlockId::from(result)];
        assert_eq!(block.writers.cells, [decide::CellId::from("T3")], "{result}");
        assert_eq!(block.writers.outside, None, "{result}");
        assert!(!block.trigger, "{result} holds what the act hands in");
        assert_eq!(block.readers.cells, [decide::CellId::from(cell)], "{result}");
    }
}

#[test]
fn the_engineers_flow_is_refused_where_a_cell_that_asks_cannot_be_spoken_to() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow/engineer");
    let mut source = flow::read(&dir).expect("the engineer's flow is read");
    let names = |names: &[&str]| names.iter().map(|n| n.to_string()).collect();
    let has = Has {
        sources: names(&[
            "answer",
            "asked",
            "dependencies",
            "main",
            "session",
            "start",
            "head",
            "tree",
            "words",
        ]),
        acts: names(&["start", "check", "merge", "run_tests"]),
        tools: names(&["query"]),
        details: names(&["files"]),
        ..Default::default()
    };
    let compiled = flow::compile(&source, &has);
    assert!(compiled.is_ok(), "{:?}", compiled.err());
    // R2, which may ask the owner and wait, as the work-4 case: without the owner's words among its triggers.
    let r2 = source.cells.get_mut("R2").expect("the flow has R2");
    let (with, without) = (r#""on_plan", "on_spoken", "on_answer""#, r#""on_plan", "on_answer""#);
    assert!(r2.contains(with), "{r2}");
    *r2 = (r2.lines())
        .filter(|line| !line.starts_with("on_spoken = "))
        .collect::<Vec<_>>()
        .join("\n")
        .replace(with, without);
    let refused = flow::compile(&source, &has).expect_err("R2 asks and cannot be spoken to");
    let what = AsksAndCannotBeSpokenTo {
        block: "q_and_a".to_string(),
    };
    assert!(
        refused.iter().any(|f| f.file == "cells/R2.toml" && f.what == what),
        "{refused:?}"
    );
    assert_eq!(refused.len(), 1, "{refused:?}");
}
