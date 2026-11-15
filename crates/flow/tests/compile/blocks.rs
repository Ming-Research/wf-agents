//! The blocks: each is declared, each a cell reads has a writer, each a cell writes has a reader, and a choice has
//! values.

use flow::{FLOW_FILE, FaultKind::*, Has};

use crate::support::{at, compiled, edit, faults, faults_with, has, unreadable, valid};

const PLAN_INPUTS: &str = r#"inputs = ["request", "opinions"]"#;

// A block's shape is written in the words the engine holds a return to as it reads it, in the forms it reads them in:
// any other is a fault of the flow, at the block's shape. Well under a millisecond alone; no other test holds a flow to
// the shapes the engine checks.
#[test]
fn a_block_s_shape_in_a_word_the_engine_does_not_check_is_a_fault() {
    let source = edit(
        valid(),
        FLOW_FILE,
        "[blocks.design]",
        "[blocks.design]\nshape = { type = [\"string\", \"null\"], minLength = 1 }",
    );
    let what = |word: &str| ShapeNotChecked {
        block: "design".to_string(),
        word: word.to_string(),
    };
    assert_eq!(
        faults(&source),
        [
            at(FLOW_FILE, "blocks.design.shape", what("`type` written as an array")),
            at(FLOW_FILE, "blocks.design.shape", what("`minLength`")),
        ]
    );
    let checked = edit(
        valid(),
        FLOW_FILE,
        "[blocks.design]",
        "[blocks.design]\nshape = { type = \"string\" }",
    );
    let shape = compiled(&checked).blocks[&"design".into()].shape.clone();
    assert!(shape.is_some(), "a shape in the words the engine checks compiles");
}

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
        // Read by the cell that asked, and the engineer's and the testing's by the review and the testing besides, which
        // judge each new test by its runs.
        let readers: &[&str] = match result {
            "i1_tested" => &["I1", "T1", "T2"],
            "t2_tested" => &["T1", "T2"],
            _ => &[cell],
        };
        let readers: Vec<decide::CellId> = readers.iter().map(|c| decide::CellId::from(*c)).collect();
        assert_eq!(block.readers.cells, readers, "{result}");
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

// A record of what is said is written in two texts of its `writes`, the owner's entry with `{words}` and a cell's
// answer with `{reply}`: a record without either text, or with a text without its place, is refused, and its words say
// which record and what it lacks, every fault at once. Well under a second alone; the engineer's flow is the flow that
// has the record.
fn engineers() -> (flow::Source, Has) {
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
    (source, has)
}

const DISCUSSION: &str = "../data/discussion.toml";
const OWNERS: &str = "owner = \"The owner:\\n\\n{words}\"\n";
const REPLIES: &str = "reply = \"An agent:\\n\\n{reply}\"\n";

// Each of the tests below compiles the engineer's flow once or twice, well under a second alone: nothing else holds the
// compile to the texts of a record of what is said, or to it alone being refused for them.
#[test]
fn a_record_of_what_is_said_holds_both_texts_whole_in_the_compiled_flow() {
    let (source, has) = engineers();
    let compiled = flow::compile(&source, &has).expect("the engineer's flow compiles");
    let spoken = compiled.blocks[&decide::BlockId::from("discussion")]
        .spoken
        .as_ref()
        .expect("the record of what is said has its texts");
    assert_eq!(spoken.owner, "The owner:\n\n{words}");
    assert_eq!(spoken.reply, "An agent:\n\n{reply}");
    assert!(compiled.blocks[&decide::BlockId::from("q_and_a")].spoken.is_none());
}

#[test]
fn a_record_of_what_is_said_with_no_text_for_the_owner_or_for_a_cell_is_refused() {
    let (source, has) = engineers();
    let without_owner = edit(source.clone(), DISCUSSION, OWNERS, "");
    let without_reply = edit(source.clone(), DISCUSSION, REPLIES, "");
    let without_both = edit(without_owner.clone(), DISCUSSION, REPLIES, "");
    let lacks = |key: &str| SpokenWithoutText {
        block: "discussion".to_string(),
        key: key.to_string(),
    };
    for (source, lacking) in [
        (without_owner, vec!["owner"]),
        (without_reply, vec!["reply"]),
        (without_both, vec!["owner", "reply"]),
    ] {
        let refused = flow::compile(&source, &has).expect_err("the record lacks a text");
        let found: Vec<flow::Fault> = lacking
            .iter()
            .map(|key| at(DISCUSSION, &format!("writes.{key}"), lacks(key)))
            .collect();
        assert_eq!(refused, found);
    }
}

#[test]
fn a_record_of_what_is_said_with_a_text_that_has_no_place_for_the_words_is_refused() {
    let (source, has) = engineers();
    let source = edit(source, DISCUSSION, "\\n\\n{words}\"", "\"");
    let source = edit(source, DISCUSSION, "\\n\\n{reply}\"", "\"");
    let refused = flow::compile(&source, &has).expect_err("the texts have no place");
    let lacks = |key: &str, place: &str| SpokenTextWithoutPlace {
        block: "discussion".to_string(),
        key: key.to_string(),
        place: place.to_string(),
    };
    assert_eq!(
        refused,
        [
            at(DISCUSSION, "writes.owner", lacks("owner", "words")),
            at(DISCUSSION, "writes.reply", lacks("reply", "reply")),
        ]
    );
}

#[test]
fn a_block_of_the_owner_s_words_that_no_trigger_has_its_agent_read_is_no_record_and_needs_no_texts() {
    let (source, has) = engineers();
    let source = crate::support::set(source, "blocks/heard.toml", "from = \"words\"\nshown = true\n");
    let compiled = flow::compile(&source, &has).expect("a block of the words no trigger reads compiles");
    assert!(compiled.blocks[&decide::BlockId::from("heard")].spoken.is_none());
}

#[test]
fn the_faults_of_a_record_of_what_is_said_come_out_with_another_fault_of_the_flow() {
    let (source, has) = engineers();
    let source = edit(source, DISCUSSION, OWNERS, "");
    let source = crate::support::set(
        source,
        "blocks/ghost.toml",
        "from = \"owner\"\nshown = true\nvalues = []\n",
    );
    let refused = flow::compile(&source, &has).expect_err("two faults");
    let lacks = SpokenWithoutText {
        block: "discussion".to_string(),
        key: "owner".to_string(),
    };
    assert!(refused.contains(&at(DISCUSSION, "writes.owner", lacks)), "{refused:?}");
    assert!(refused.iter().any(|f| f.what == ValuesEmpty), "{refused:?}");
}
