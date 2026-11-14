//! A flow laid out one thing to a file: read with what it may use beside the flows, it compiles to what its twin
//! written before compiles to. A thing is named by its file, and a second file of one name is refused; the engine's own
//! data a flow names are its blocks; a line names only its sets of rules; and each fault is in the file of the thing it
//! is about.

use decide::BlockId;
use flow::FaultKind::*;
use flow::{Compiled, FLOW_FILE, Fault};

use crate::support::{at, compiled, edit, faults, laid_out, set, unreadable, valid};

/// The definition compiled, with no cell's triggers: a cell written before triggers names them by its blocks.
fn without_triggers(compiled: Compiled) -> Compiled {
    let mut compiled = compiled;
    for cell in &mut compiled.cells {
        cell.triggers.clear();
    }
    compiled
}

fn twice(name: &str) -> flow::FaultKind {
    Twice { name: name.to_string() }
}

#[test]
fn a_flow_laid_out_one_thing_to_a_file_compiles_to_what_its_twin_written_before_compiles_to() {
    let mut twin = without_triggers(compiled(&laid_out()));
    // Every block is as it was, but that `main` stands for the trigger `main_moved`, which declares it.
    let main = twin.blocks.get_mut(&BlockId::from("main")).expect("main is declared");
    assert!(main.trigger, "main stands only for a trigger");
    main.trigger = false;
    assert_eq!(twin, without_triggers(compiled(&valid())));
}

#[test]
fn the_engine_s_own_data_a_flow_names_are_its_blocks_as_defined_beside_the_flows() {
    let source = edit(
        laid_out(),
        FLOW_FILE,
        "data = [\"request\"]",
        "data = [\"request\", \"discussion\"]",
    );
    let compiled = compiled(&source);
    let discussion = &compiled.blocks[&BlockId::from("discussion")];
    assert_eq!(discussion.field.as_deref(), Some("reply"));
    assert_eq!(
        discussion.what.as_deref(),
        Some("What the owner says to the work and what its cells answer.")
    );
    assert!(discussion.writers.people && discussion.readers.shown && !discussion.trigger);
    // A datum beside the flows that a flow does not name is none of its blocks.
    assert!(
        !crate::support::compiled(&laid_out())
            .blocks
            .contains_key(&BlockId::from("discussion"))
    );
}

#[test]
fn a_datum_the_flow_names_that_is_not_beside_the_flows_named_twice_or_defined_by_the_flow_as_well_is_refused() {
    let source = edit(
        laid_out(),
        FLOW_FILE,
        "data = [\"request\"]",
        "data = [\"request\", \"ghost\", \"request\"]",
    );
    assert_eq!(
        faults(&source),
        [
            at(FLOW_FILE, "data[1]", NoSuchData { name: "ghost".into() }),
            at(FLOW_FILE, "data[2]", twice("request")),
        ]
    );
    let source = set(laid_out(), "blocks/request.toml", "people = true");
    assert_eq!(faults(&source), [at(FLOW_FILE, "data[0]", twice("request"))]);
}

#[test]
fn a_second_file_of_one_name_is_refused_on_the_second_by_its_path() {
    // A block, wherever in `blocks/` it is.
    let source = set(laid_out(), "blocks/other/plan.toml", "");
    assert_eq!(faults(&source), [at("blocks/plan.toml", "", twice("plan"))]);
    // A datum of the engine's.
    let source = set(laid_out(), "../data/more/request.toml", "people = true");
    assert_eq!(faults(&source), [at("../data/request.toml", "", twice("request"))]);
    // A trigger, of its own or one each cell has its own of.
    let source = set(
        laid_out(),
        "../triggers/of_a_cell/main_moved.toml",
        "block = \"{cell}_main\"\npeople = true",
    );
    assert_eq!(
        faults(&source),
        [at("../triggers/of_a_cell/main_moved.toml", "", twice("main_moved"))]
    );
}

#[test]
fn a_line_names_only_the_sets_of_rules_beside_the_flows_each_once_and_none_of_the_engine_s_parts() {
    let source = set(
        laid_out(),
        "lines/engineer.toml",
        "rules = [\"engineering\", \"general\"]",
    );
    let compiled = compiled(&source);
    let system = |id: &str| {
        let cell = compiled.cells.iter().find(|c| c.id.as_str() == id).expect("the cell");
        cell.system.clone()
    };
    for id in ["plan", "design", "answer"] {
        assert_eq!(system(id), ["engineering", "general"], "{id}");
    }
    assert_eq!(system("judge"), Vec::<String>::new());
    // The texts are what the engine says to every agent, not the flow's.
    assert!(compiled.rules.is_empty());

    let source = set(
        laid_out(),
        "lines/review.toml",
        "rules = [\"style\", \"guide\", \"general\", \"general\"]",
    );
    let at_rules = |i: usize, what| at("lines/review.toml", &format!("rules[{i}]"), what);
    assert_eq!(
        faults(&source),
        [
            at_rules(0, NoSuchRules { name: "style".into() }),
            at_rules(1, EnginePart { name: "guide".into() }),
            at_rules(3, twice("general")),
        ]
    );
}

#[test]
fn each_fault_is_in_the_file_of_the_thing_it_is_about_flow_toml_first_then_the_cells_in_order_then_the_rest() {
    let source = edit(laid_out(), "tests/may_merge.toml", "reads = [\"ci\"]", "reads = []");
    let source = edit(
        source,
        "cells/judge.toml",
        "context = [\"plan\"]",
        "context = [\"plan\", \"ghost\"]",
    );
    let source = edit(source, FLOW_FILE, "calls = 40", "calls = 0");
    let source = edit(
        source,
        "blocks/judging/opinions.toml",
        "people = true",
        "people = true\nnote = true\nvalues = [\"a\"]",
    );
    let source = set(
        source,
        "cells/extra.toml",
        "title = \"Extra\"\nkind = \"engine\"\nact = \"check\"",
    );
    assert_eq!(
        faults(&source),
        [
            at(FLOW_FILE, "limits.calls", LimitOutOfRange { value: 0 }),
            at("cells/judge.toml", "context[1]", NoSuchBlock { block: "ghost".into() }),
            at(
                "blocks/judging/opinions.toml",
                "note",
                NoteIsAChoice {
                    block: "opinions".into()
                }
            ),
            at("cells/extra.toml", "", NotInOrder { cell: "extra".into() }),
            at(
                "tests/may_merge.toml",
                "reads",
                ReadsEmpty {
                    test: "may_merge".into()
                }
            ),
        ]
    );
}

#[test]
fn a_file_laid_out_one_thing_to_a_file_that_cannot_be_read_is_reported_by_its_path() {
    unreadable(&set(laid_out(), "cells/judge.toml", "title = "), "cells/judge.toml");
    let message = unreadable(
        &edit(
            laid_out(),
            "blocks/check/ci.toml",
            "values",
            "shape = \"prose\"\nvalues",
        ),
        "blocks/check/ci.toml",
    );
    assert!(message.contains("shape"), "{message}");
    unreadable(
        &set(
            laid_out(),
            "../triggers/main_moved.toml",
            "block = \"main\"\nwhen = \"now\"",
        ),
        "../triggers/main_moved.toml",
    );
    // A line laid out so names its rules, not the parts of its system prompt.
    unreadable(
        &set(laid_out(), "lines/review.toml", "system = [\"general\"]"),
        "lines/review.toml",
    );
    // The `flow.toml` of a flow laid out so holds no line, block or test, and that of one written before names no data
    // of the engine's, which it cannot use.
    unreadable(
        &edit(laid_out(), FLOW_FILE, "[limits]", "[blocks.plan]\n\n[limits]"),
        FLOW_FILE,
    );
    unreadable(
        &edit(valid(), FLOW_FILE, "[limits]", "data = [\"request\"]\n\n[limits]"),
        FLOW_FILE,
    );
}

#[test]
fn only_the_files_that_cannot_be_read_are_reported_when_one_cannot() {
    let source = set(laid_out(), "tests/may_merge.toml", "says = ");
    let source = edit(
        source,
        "cells/judge.toml",
        "context = [\"plan\"]",
        "context = [\"ghost\"]",
    );
    let files: Vec<String> = faults(&source).into_iter().map(|f: Fault| f.file).collect();
    assert_eq!(files, ["tests/may_merge.toml"]);
}
