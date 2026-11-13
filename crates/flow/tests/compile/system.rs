//! The flow's sets of rules for the agents, which the system prompt of a line's session holds: a line names only its
//! own, each once, each a file the flow has, and never a part the engine places in every system prompt itself; each file
//! is named by a line and has no such part's name. They are compiled into each cell of the line in the order written,
//! and their texts into the flow.

use flow::FaultKind::*;
use flow::{FLOW_FILE, Has};

use crate::support::{at, compiled, edit, faults, faults_with, has, valid};

/// The small flow whose engineer line names these sets of rules.
fn engineer_system(list: &str) -> flow::Source {
    edit(
        valid(),
        FLOW_FILE,
        "[lines.engineer]",
        &format!("[lines.engineer]\nsystem = {list}"),
    )
}

/// The flow with these sets of rules beside it, each saying its name.
fn with_rules(source: flow::Source, names: &[&str]) -> flow::Source {
    let mut source = source;
    for name in names {
        source.rules.insert(name.to_string(), format!("The rules {name}."));
    }
    source
}

fn system(compiled: &flow::Compiled, id: &str) -> Vec<String> {
    let cell = (compiled.cells.iter()).find(|c| c.id.as_str() == id).expect("the cell");
    cell.system.clone()
}

#[test]
fn each_cell_of_a_line_has_the_sets_of_rules_its_line_names_in_their_order_and_the_flow_their_texts() {
    let source = with_rules(
        engineer_system(r#"["engineering", "general"]"#),
        &["general", "engineering"],
    );
    let compiled = compiled(&source);
    for cell in ["plan", "design", "answer"] {
        assert_eq!(system(&compiled, cell), ["engineering", "general"], "{cell}");
    }
    // A line that names none has none, and an engine cell, in no line, has none.
    assert_eq!(system(&compiled, "judge"), Vec::<String>::new());
    assert_eq!(system(&compiled, "check"), Vec::<String>::new());
    assert_eq!(
        compiled.rules.into_iter().collect::<Vec<_>>(),
        [
            ("engineering".to_string(), "The rules engineering.".to_string()),
            ("general".to_string(), "The rules general.".to_string()),
        ]
    );
}

#[test]
fn a_line_names_only_sets_of_rules_the_flow_has_each_once_and_none_of_the_engine_s_parts() {
    let source = with_rules(
        engineer_system(r#"["general", "style", "general", "guide"]"#),
        &["general"],
    );
    let source = edit(
        source,
        FLOW_FILE,
        "[lines.review]",
        "[lines.review]\nsystem = [\"tree_rules\"]",
    );
    assert_eq!(
        faults(&source),
        [
            at(
                FLOW_FILE,
                "lines.engineer.system[1]",
                NoSuchRules {
                    name: "style".to_string(),
                },
            ),
            at(
                FLOW_FILE,
                "lines.engineer.system[2]",
                Twice {
                    name: "general".to_string(),
                },
            ),
            at(
                FLOW_FILE,
                "lines.engineer.system[3]",
                EnginePart {
                    name: "guide".to_string(),
                },
            ),
            at(
                FLOW_FILE,
                "lines.review.system[0]",
                EnginePart {
                    name: "tree_rules".to_string(),
                },
            ),
        ]
    );
    // Which parts are the engine's is the engine's to say.
    let places_none = Has {
        system: Default::default(),
        ..has()
    };
    let source = with_rules(engineer_system(r#"["general", "guide"]"#), &["general", "guide"]);
    assert!(flow::compile(&source, &places_none).is_ok());
}

#[test]
fn a_set_of_rules_no_line_names_or_one_named_as_an_engine_part_is_refused_on_its_file() {
    let source = with_rules(engineer_system(r#"["general"]"#), &["general", "style", "guide"]);
    assert_eq!(
        faults(&source),
        [
            at(
                "rules/guide.md",
                "",
                RulesNamedAsEnginePart {
                    name: "guide".to_string(),
                },
            ),
            at(
                "rules/style.md",
                "",
                UnusedRules {
                    name: "style".to_string(),
                },
            ),
        ]
    );
    // With no line, no set of rules is named.
    let unused = |name: &str| UnusedRules { name: name.to_string() };
    assert_eq!(
        faults_with(&with_rules(valid(), &["general"]), &has()),
        [at("rules/general.md", "", unused("general"))]
    );
}
