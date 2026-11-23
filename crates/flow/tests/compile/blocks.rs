//! The blocks: each is declared, each a cell reads has a writer, each a cell writes has a reader, and a choice has
//! values.

use flow::{FLOW_FILE, FaultKind::*, Has};

use crate::support::{at, compiled, edit, faults, faults_with, has, names, unreadable, valid};

const PLAN_WATCHES: &str = r#"watches = ["request", "opinions"]"#;

// A block whose return is held to the owner's answers marked `tree` being in the design tree says so in its file
// (`answers_in_the_tree`), and is compiled so; a block that says nothing of it is not held.
#[test]
fn a_block_handed_in_only_with_the_answers_in_the_tree_is_compiled_so() {
    let source = edit(
        valid(),
        FLOW_FILE,
        "[blocks.design]\n",
        "[blocks.design]\nanswers_in_the_tree = true\n",
    );
    let compiled = compiled(&source);
    assert!(compiled.blocks[&"design".into()].answers_in_the_tree);
    assert!(!compiled.blocks[&"plan".into()].answers_in_the_tree);
}

// The block the source of the works under way writes says in what block of the flow a work's plan names the files it will
// touch (`planned_in`), which the engine reads of each work by that name and not by one of its own; a name the flow
// does not declare is refused where it is written, and a block that says nothing of it names none.
#[test]
fn a_block_says_in_what_block_a_plan_names_its_files_and_the_block_must_be_declared() {
    let has = Has {
        sources: names(&["main", "parallel"]),
        ..has()
    };
    let listed = |planned_in: &str| {
        edit(
            valid(),
            FLOW_FILE,
            "[blocks.main]\noutside = \"main\"",
            &format!("[blocks.main]\noutside = \"parallel\"\nplanned_in = \"{planned_in}\""),
        )
    };
    let compiled = match flow::compile(&listed("plan"), &has) {
        Ok(compiled) => compiled,
        Err(faults) => panic!("the definition should compile: {faults:#?}"),
    };
    assert_eq!(compiled.blocks[&"main".into()].planned_in, Some("plan".into()));
    assert_eq!(compiled.blocks[&"plan".into()].planned_in, None);

    let what = NoSuchBlock {
        block: "nowhere".to_string(),
    };
    assert_eq!(
        faults_with(&listed("nowhere"), &has),
        [at(FLOW_FILE, "blocks.main.planned_in", what)]
    );
}

// A block whose lines name entries of the design tree says so in its file (`names_tree_entries`), and is compiled so.
// The engine gives it as the tree has the entries it names: a flow with no datum of the tree is refused it, and a cell
// is given it only as its latest version, since what changed in the names is not what changed in the entries' text.
#[test]
fn a_block_naming_entries_of_the_tree_needs_the_tree_and_is_given_only_as_its_latest_version() {
    let names = edit(
        valid(),
        FLOW_FILE,
        "[blocks.design]\n",
        "[blocks.design]\nnames_tree_entries = true\n",
    );
    let what = EntriesWithoutTree {
        block: "design".to_string(),
    };
    assert_eq!(
        faults(&names),
        [at(FLOW_FILE, "blocks.design.names_tree_entries", what)]
    );
    let with_tree = edit(
        names,
        FLOW_FILE,
        "[blocks.main]\n",
        "[blocks.tree]\noutside = \"tree\"\nshown = true\n\n[blocks.main]\n",
    );
    let mut has = has();
    has.sources.insert("tree".to_string());
    let compiled = flow::compile(&with_tree, &has).expect("the flow compiles");
    assert!(compiled.blocks[&"design".into()].names_tree_entries);
    assert!(!compiled.blocks[&"plan".into()].names_tree_entries);
    let read = edit(
        with_tree,
        "answer.toml",
        "outputs = [\"reply\"]",
        "reads = [\"design\"]\noutputs = [\"reply\"]",
    );
    let latest = edit(
        read.clone(),
        "answer.toml",
        "plan = \"last_and_latest\"",
        "plan = \"last_and_latest\"\ndesign = \"latest\"",
    );
    assert!(flow::compile(&latest, &has).is_ok(), "given as its latest");
    for kind in ["last_and_latest", "all"] {
        let taken = edit(
            read.clone(),
            "answer.toml",
            "plan = \"last_and_latest\"",
            &format!("plan = \"last_and_latest\"\ndesign = \"{kind}\""),
        );
        let what = EntriesNotLatest {
            block: "design".to_string(),
        };
        assert_eq!(
            faults_with(&taken, &has),
            [at("answer.toml", "kinds.design", what)],
            "{kind}"
        );
    }
}

#[test]
fn every_block_a_cell_reads_or_writes_is_declared() {
    let source = edit(
        valid(),
        "plan.toml",
        PLAN_WATCHES,
        r#"watches = ["request", "opinions", "nothing"]"#,
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
            at("plan.toml", "watches[2]", no_such("nothing")),
            at("plan.toml", "outputs[1]", no_such("nowhere")),
        ]
    );
}

#[test]
fn a_cell_names_a_block_once_among_what_it_watches_and_once_among_its_outputs() {
    let source = edit(
        valid(),
        "plan.toml",
        PLAN_WATCHES,
        r#"watches = ["request", "opinions", "request"]"#,
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
            at("plan.toml", "watches[2]", twice("request")),
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
        PLAN_WATCHES,
        r#"watches = ["request", "opinions", "notes"]"#,
    );
    let source = edit(source, "plan.toml", "[kinds]", "[kinds]\nnotes = \"latest\"");
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
fn a_cell_that_writes_its_questions_watches_them_and_its_own_words() {
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

    // Watching its questions alone, it can ask and cannot be spoken to about what it asks: refused still.
    let watches = r#"watches = ["request", "opinions", "asks"]"#;
    let source = edit(source, "plan.toml", PLAN_WATCHES, watches);
    let source = edit(source, "plan.toml", "[kinds]", "[kinds]\nasks = \"all\"");
    assert_eq!(
        faults_with(&source, &has),
        [at("plan.toml", "outputs[1]", not_spoken_to)]
    );

    let words = format!("{asks}\n[blocks.words]\noutside = \"words\"\n");
    let source = edit(source, FLOW_FILE, asks, &words);
    let source = edit(source, "plan.toml", outputs, r#"outputs = ["plan", "asks", "words"]"#);
    let watches_words = r#"watches = ["request", "opinions", "asks", "words"]"#;
    let source = edit(source, "plan.toml", watches, watches_words);
    let source = edit(source, "plan.toml", "[kinds]", "[kinds]\nwords = \"all\"");
    let compiled = flow::compile(&source, &has);
    assert!(compiled.is_ok(), "{:?}", compiled.err());
}

// The blocks each version of which is an entry beside those before it are those of the sources of the owner's words and
// of the answers: each cell's own words and questions, and no other.
#[test]
fn the_blocks_entries_are_added_to_are_each_cell_s_own_questions_and_words() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow/engineer");
    let source = flow::read(&dir).expect("the engineer's flow is read");
    let names = |names: &[&str]| names.iter().map(|n| n.to_string()).collect();
    let has = Has {
        sources: names(&[
            "answer",
            "asked",
            "dependencies",
            "handed_in",
            "parallel",
            "main",
            "start",
            "head",
            "tree",
            "words",
        ]),
        acts: names(&["start", "check", "merge"]),
        tools: names(&["query", "run_tests"]),
        ..Default::default()
    };
    let compiled = flow::compile(&source, &has).expect("the engineer's flow compiles");
    let records: Vec<&str> = (compiled.blocks.keys())
        .filter(|b| compiled.flow.adds_entries(b))
        .map(|b| b.as_str())
        .collect();
    assert_eq!(
        records,
        [
            "d1_questions",
            "d1_words",
            "i1_questions",
            "i1_words",
            "r1_questions",
            "r1_words",
        ]
    );
}

// A datum of a cell's own that a cell names only in its condition is made for that cell (decision
// a-datum-of-a-cell-s-own-is-made-where-a-cell-names-it): the better-way cells' `handed_in`, which the engine writes and
// no cell watches, reads or writes, and of no other cell.
#[test]
fn a_datum_of_a_cell_s_own_named_only_in_a_condition_is_made_for_that_cell() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow/engineer");
    let source = flow::read(&dir).expect("the engineer's flow is read");
    let names = |names: &[&str]| names.iter().map(|n| n.to_string()).collect();
    let has = Has {
        sources: names(&[
            "answer",
            "asked",
            "dependencies",
            "handed_in",
            "parallel",
            "main",
            "start",
            "head",
            "tree",
            "words",
        ]),
        acts: names(&["start", "check", "merge"]),
        tools: names(&["query", "run_tests"]),
        ..Default::default()
    };
    let compiled = flow::compile(&source, &has).expect("the engineer's flow compiles");
    let made: Vec<(String, Option<String>, Option<String>, bool)> = (compiled.blocks.iter())
        .filter(|(_, block)| block.writers.outside.as_deref() == Some("handed_in"))
        .map(|(name, block)| {
            let whose = block.of_cell.as_ref().map(|c| c.0.clone());
            let named = !block.readers.cells.is_empty() || !block.writers.cells.is_empty();
            (name.to_string(), whose, block.default.clone(), named)
        })
        .collect();
    let made_for = |cell: &str| {
        let name = format!("{}_handed_in", cell.to_lowercase());
        (name, Some(cell.to_string()), Some("no".to_string()), false)
    };
    assert_eq!(made, [made_for("D2"), made_for("R2")]);
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
            "handed_in",
            "parallel",
            "main",
            "start",
            "head",
            "tree",
            "words",
        ]),
        acts: names(&["start", "check", "merge"]),
        tools: names(&["query", "run_tests"]),
        ..Default::default()
    };
    let compiled = flow::compile(&source, &has);
    assert!(compiled.is_ok(), "{:?}", compiled.err());
    // R1, which may ask the owner and wait, as the work-4 case: not watching its own words.
    let r1 = source.cells.get_mut("R1").expect("the flow has R1");
    let (with, without) = ("  \"r1_words\",\n]\nreads", "]\nreads");
    assert!(r1.contains(with), "{r1}");
    *r1 = (r1.lines())
        .filter(|line| !line.starts_with("r1_words = "))
        .collect::<Vec<_>>()
        .join("\n")
        .replace(with, without);
    let refused = flow::compile(&source, &has).expect_err("R1 asks and cannot be spoken to");
    let what = AsksAndCannotBeSpokenTo {
        block: "r1_questions".to_string(),
    };
    assert!(
        refused.iter().any(|f| f.file == "cells/R1.toml" && f.what == what),
        "{refused:?}"
    );
    assert_eq!(refused.len(), 1, "{refused:?}");
}
