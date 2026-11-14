//! A flow laid out one thing to a file: read with what it may use beside the flows, it compiles to what its twin
//! written before compiles to. A thing is named by its file, and a second file of one name is refused; the engine's own
//! data a flow names are its blocks; a line names only its sets of rules; and each fault is in the file of the thing it
//! is about.

use decide::BlockId;
use flow::FaultKind::*;
use flow::{FLOW_FILE, Fault, FaultKind, Texts};

use crate::support::{at, compiled, edit, faults, laid_out, set, unreadable, valid};

fn twice(name: &str) -> FaultKind {
    Twice { name: name.to_string() }
}

fn defined_twice(name: &str, also: &str) -> FaultKind {
    DefinedTwice {
        name: name.to_string(),
        also: also.to_string(),
    }
}

/// The name of the twin's trigger that stands on each block that runs a cell of the small flow: `on_main` on `main`,
/// which stands only for it, and on each other block, a datum, the trigger the rule `on_{datum}` makes of it.
fn trigger_of(block: &str) -> String {
    format!("on_{block}")
}

#[test]
fn a_flow_laid_out_one_thing_to_a_file_compiles_to_what_its_twin_written_before_compiles_to() {
    let mut before = compiled(&valid());
    // A cell written before triggers has a trigger of each block that runs it, by the block's name. The twin's are
    // named as events and stand on the same blocks, and each has its agent read and do the same.
    for cell in &mut before.cells {
        for trigger in &mut cell.triggers {
            trigger.name = trigger_of(trigger.block.as_str());
        }
    }
    let mut twin = compiled(&laid_out());
    // Every block is as it was, but that `main` stands for the trigger `on_main`, which declares it.
    let main = twin.blocks.get_mut(&BlockId::from("main")).expect("main is declared");
    assert!(main.trigger, "main stands only for a trigger");
    main.trigger = false;
    // Each thing of the twin is in a file of its own, where the flow written before has them all in `flow.toml`; and
    // a line laid out so names its `rules` where one written before names its `system`.
    for block in twin.blocks.values_mut() {
        block.file = FLOW_FILE.to_string();
    }
    for file in twin.test_files.values_mut() {
        *file = FLOW_FILE.to_string();
    }
    for line in twin.lines.values_mut() {
        line.file = FLOW_FILE.to_string();
        line.system = std::mem::take(&mut line.rules);
    }
    assert_eq!(twin, before);
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
    let source = set(laid_out(), "blocks/request.toml", "from = \"owner\"");
    assert_eq!(
        faults(&source),
        [at(
            FLOW_FILE,
            "data[0]",
            defined_twice("request", "blocks/request.toml")
        )]
    );
}

#[test]
fn whether_a_datum_of_the_engine_s_is_written_and_read_is_the_flow_s_and_its_shape_is_its_own_file_s() {
    let source = edit(
        laid_out(),
        FLOW_FILE,
        "data = [\"request\"]",
        "data = [\"request\", \"unused\"]",
    );
    let source = set(source, "../data/unused.toml", "from = \"owner\"\nvalues = []");
    assert_eq!(
        faults(&source),
        [
            at(FLOW_FILE, "data[1]", NoReader { block: "unused".into() }),
            at("../data/unused.toml", "values", ValuesEmpty),
        ]
    );
}

#[test]
fn who_writes_a_block_from_outside_the_cells_is_said_by_from_the_owner_or_a_source_the_engine_has() {
    let compiled = compiled(&laid_out());
    let opinions = &compiled.blocks[&BlockId::from("opinions")];
    assert!(opinions.writers.people && opinions.writers.outside.is_none());
    let main = &compiled.blocks[&BlockId::from("main")];
    assert!(!main.writers.people);
    assert_eq!(main.writers.outside.as_deref(), Some("main"));
    // A source the engine does not have is refused where `from` names it.
    let source = edit(
        laid_out(),
        "blocks/check/ci.toml",
        "values",
        "from = \"mirror\"\nvalues",
    );
    let what = UnknownSource {
        block: "ci".into(),
        source: "mirror".into(),
    };
    assert_eq!(faults(&source), [at("blocks/check/ci.toml", "from", what)]);
    // `people` and `outside` are said in a flow written before it was laid out one thing to a file only.
    unreadable(
        &edit(laid_out(), "blocks/comments.toml", "from = \"owner\"", "people = true"),
        "blocks/comments.toml",
    );
    unreadable(
        &set(
            laid_out(),
            "../triggers/on_main.toml",
            "block = \"main\"\noutside = \"main\"",
        ),
        "../triggers/on_main.toml",
    );
}

#[test]
fn a_datum_is_written_only_through_a_field_of_a_return_the_engine_has() {
    let source = edit(
        laid_out(),
        FLOW_FILE,
        "data = [\"request\"]",
        "data = [\"request\", \"discussion\"]",
    );
    let source = edit(
        source,
        "../data/discussion.toml",
        "field = \"reply\"",
        "field = \"words\"",
    );
    let what = UnknownReturnField { field: "words".into() };
    assert_eq!(faults(&source), [at("../data/discussion.toml", "field", what)]);
}

#[test]
fn a_second_file_of_one_name_is_refused_on_the_second_naming_the_first() {
    // A block, wherever in `blocks/` it is.
    let source = set(laid_out(), "blocks/other/plan.toml", "");
    assert_eq!(
        faults(&source),
        [at(
            "blocks/plan.toml",
            "",
            defined_twice("plan", "blocks/other/plan.toml")
        )]
    );
    // A datum of the engine's.
    let source = set(laid_out(), "../data/more/request.toml", "from = \"owner\"");
    assert_eq!(
        faults(&source),
        [at(
            "../data/request.toml",
            "",
            defined_twice("request", "../data/more/request.toml")
        )]
    );
    // A trigger, of its own or one each cell has its own of: the two are named alike.
    let source = set(
        laid_out(),
        "../triggers/on_spoken.toml",
        "block = \"spoken\"\nfrom = \"owner\"",
    );
    assert_eq!(
        faults(&source),
        [at(
            "../triggers/on_spoken.toml",
            "",
            defined_twice("on_spoken", "../triggers/of_a_cell/on_spoken.toml")
        )]
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
        "from = \"owner\"",
        "from = \"owner\"\nnote = true\nvalues = [\"a\"]",
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
            "../triggers/on_main.toml",
            "block = \"main\"\nwhen = \"now\"",
        ),
        "../triggers/on_main.toml",
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

#[test]
fn what_a_source_of_the_engine_s_writes_into_a_block_is_compiled_text_by_text_in_the_order_written() {
    let on_main = "block = \"main\"\nfrom = \"main\"\n\n[writes]\ntext = \"Main is at {head}.\"\nafter = \"then\"\n";
    let source = set(laid_out(), "../triggers/on_main.toml", on_main);
    let request = "from = \"owner\"\n\n[writes]\nz = \"last\"\na = \"first\"\n";
    let source = set(source, "../data/request.toml", request);
    let compiled = compiled(&source);
    let texts = |pairs: &[(&str, &str)]| Texts(pairs.iter().map(|(n, t)| (n.to_string(), t.to_string())).collect());
    assert_eq!(
        compiled.blocks[&BlockId::from("main")].writes,
        texts(&[("text", "Main is at {head}."), ("after", "then")])
    );
    // The order is the file's, not that of the names.
    assert_eq!(
        compiled.blocks[&BlockId::from("request")].writes,
        texts(&[("z", "last"), ("a", "first")])
    );
    assert_eq!(compiled.blocks[&BlockId::from("plan")].writes, Texts::default());
}

#[test]
fn what_a_trigger_is_is_compiled_with_each_cell_s_trigger_and_a_datum_s_with_its_name_filled() {
    let on_main = "what = \"Fires when main has moved.\"\nblock = \"main\"\nfrom = \"main\"\n";
    let source = set(laid_out(), "../triggers/on_main.toml", on_main);
    let changed = "name = \"on_{datum}\"\nwhat = \"Fires when `{datum}` is new.\"\nread = [\"{datum}\"]\n";
    let source = set(source, "../triggers/of_a_datum/changed.toml", changed);
    let compiled = compiled(&source);
    let trigger = |cell: &str, name: &str| {
        let cell = (compiled.cells.iter())
            .find(|c| c.id.as_str() == cell)
            .expect("the cell");
        let trigger = (cell.triggers.iter()).find(|t| t.name == name).expect("the trigger");
        trigger.what.clone()
    };
    assert_eq!(
        trigger("check", "on_main").as_deref(),
        Some("Fires when main has moved.")
    );
    assert_eq!(
        trigger("plan", "on_request").as_deref(),
        Some("Fires when `request` is new.")
    );
    // What a trigger is, where its file says nothing, is none.
    let compiled = crate::support::compiled(&laid_out());
    let check = (compiled.cells.iter())
        .find(|c| c.id.as_str() == "check")
        .expect("the check");
    assert!(check.triggers.iter().all(|t| t.what.is_none()));
}
