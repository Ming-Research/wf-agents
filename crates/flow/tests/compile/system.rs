//! The system prompt of a line's session: the parts its line names, each one the engine fills in itself or a set of
//! the flow's rules for the agents, named once; compiled into each cell of the line in the order written.

use flow::FaultKind::*;
use flow::{FLOW_FILE, Has};

use crate::support::{at, compiled, edit, faults, faults_with, has, valid};

/// The small flow whose engineer line names these parts of its system prompt.
fn engineer_system(list: &str) -> flow::Source {
    edit(
        valid(),
        FLOW_FILE,
        "[lines.engineer]",
        &format!("[lines.engineer]\nsystem = {list}"),
    )
}

/// The small flow with this set of rules beside it.
fn with_rules(source: flow::Source, name: &str) -> flow::Source {
    let mut source = source;
    source
        .rules
        .insert(name.to_string(), "Say one thing per sentence.".to_string());
    source
}

fn system(compiled: &flow::Compiled, id: &str) -> Vec<String> {
    let cell = (compiled.cells.iter()).find(|c| c.id.as_str() == id).expect("the cell");
    cell.system.clone()
}

#[test]
fn each_cell_of_a_line_has_the_parts_its_line_names_in_their_order() {
    let source = with_rules(engineer_system(r#"["tree_rules", "general", "guide"]"#), "general");
    let compiled = compiled(&source);
    for cell in ["plan", "design", "answer"] {
        assert_eq!(system(&compiled, cell), ["tree_rules", "general", "guide"], "{cell}");
    }
    // A line that names none has none, and an engine cell, in no line, has none.
    assert_eq!(system(&compiled, "judge"), Vec::<String>::new());
    assert_eq!(system(&compiled, "check"), Vec::<String>::new());
}

#[test]
fn a_part_that_is_neither_the_engine_s_nor_a_set_of_the_flow_s_rules_or_one_named_twice_is_refused() {
    let source = engineer_system(r#"["guide", "general", "guide"]"#);
    let source = edit(
        source,
        FLOW_FILE,
        "[lines.review]",
        "[lines.review]\nsystem = [\"style\"]",
    );
    let unknown = |part: &str| UnknownPart { part: part.to_string() };
    assert_eq!(
        faults(&source),
        [
            at(FLOW_FILE, "lines.engineer.system[1]", unknown("general")),
            at(
                FLOW_FILE,
                "lines.engineer.system[2]",
                Twice {
                    name: "guide".to_string(),
                },
            ),
            at(FLOW_FILE, "lines.review.system[0]", unknown("style")),
        ]
    );
    // Which parts the engine fills in is its to say.
    let fills_none = Has {
        system: Default::default(),
        ..has()
    };
    assert_eq!(
        faults_with(
            &with_rules(engineer_system(r#"["general", "guide"]"#), "general"),
            &fills_none
        ),
        [at(FLOW_FILE, "lines.engineer.system[1]", unknown("guide"))]
    );
}
