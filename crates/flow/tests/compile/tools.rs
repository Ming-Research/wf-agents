//! The tools of the engine's a cell may ask for in its return: each a tool the engine has, named once, and only by an
//! agent cell; compiled in the order written. A flow laid out one thing to a file may name them once for every agent
//! cell of it, since the form of a return is the same for the whole flow; a cell of it then names none of its own.

use flow::FaultKind::*;
use flow::{FLOW_FILE, Has};

use crate::support::{at, compiled, edit, faults, faults_with, has, laid_out, valid};

const JUDGE_INPUTS: &str = r#"inputs = ["plan"]"#;

/// The small flow whose judge may ask for the tools this list names.
fn judge_tools(list: &str) -> flow::Source {
    edit(
        valid(),
        "judge.toml",
        JUDGE_INPUTS,
        &format!("{JUDGE_INPUTS}\ntools = {list}"),
    )
}

fn tools(compiled: &flow::Compiled, id: &str) -> Vec<String> {
    let cell = (compiled.cells.iter()).find(|c| c.id.as_str() == id).expect("the cell");
    cell.tools.clone()
}

#[test]
fn the_tools_a_cell_may_ask_for_are_compiled_in_the_order_written_and_a_cell_that_names_none_has_none() {
    let compiled = compiled(&judge_tools(r#"["run_tests", "query"]"#));
    assert_eq!(tools(&compiled, "judge"), ["run_tests", "query"]);
    assert_eq!(tools(&compiled, "plan"), Vec::<String>::new());
}

#[test]
fn a_tool_the_engine_does_not_have_or_one_named_twice_is_refused() {
    let source = judge_tools(r#"["query", "deploy", "query"]"#);
    assert_eq!(
        faults(&source),
        [
            at(
                "judge.toml",
                "tools[1]",
                UnknownTool {
                    tool: "deploy".to_string(),
                },
            ),
            at(
                "judge.toml",
                "tools[2]",
                Twice {
                    name: "query".to_string(),
                },
            ),
        ]
    );
    // Which tools there are is the engine's to say.
    let no_tools = Has {
        tools: Default::default(),
        ..has()
    };
    let unknown = |tool: &str| UnknownTool { tool: tool.to_string() };
    assert_eq!(
        faults_with(&judge_tools(r#"["query"]"#), &no_tools),
        [at("judge.toml", "tools[0]", unknown("query"))]
    );
}

#[test]
fn an_engine_cell_asks_for_no_tool() {
    let source = edit(
        valid(),
        "check.toml",
        "act = \"check\"",
        "act = \"check\"\ntools = [\"query\"]",
    );
    assert_eq!(faults(&source), [at("check.toml", "tools", EngineHasTools)]);
}

/// The small flow laid out one thing to a file, its `flow.toml` naming the tools of this list for every agent cell.
fn flow_tools(list: &str) -> flow::Source {
    edit(
        laid_out(),
        FLOW_FILE,
        "data = [\"request\"]",
        &format!("data = [\"request\"]\ntools = {list}"),
    )
}

#[test]
fn the_tools_a_flow_names_are_every_agent_cell_s_and_no_engine_cell_s() {
    let compiled = compiled(&flow_tools(r#"["run_tests", "query"]"#));
    for cell in ["plan", "judge", "design", "answer"] {
        assert_eq!(tools(&compiled, cell), ["run_tests", "query"], "{cell}");
    }
    for cell in ["check", "merge"] {
        assert_eq!(tools(&compiled, cell), Vec::<String>::new(), "{cell}");
    }
}

#[test]
fn a_cell_that_names_tools_of_its_own_beside_the_flow_s_names_them_twice() {
    let source = edit(
        flow_tools(r#"["query"]"#),
        "cells/judge.toml",
        "line = \"review\"",
        "line = \"review\"\ntools = [\"query\"]",
    );
    assert_eq!(
        faults(&source),
        [at(
            "cells/judge.toml",
            "tools",
            Twice {
                name: "tools".to_string(),
            },
        )]
    );
}

#[test]
fn a_tool_the_flow_names_that_the_engine_does_not_have_or_names_twice_is_refused_in_flow_toml() {
    assert_eq!(
        faults(&flow_tools(r#"["query", "deploy", "query"]"#)),
        [
            at(
                FLOW_FILE,
                "tools[1]",
                UnknownTool {
                    tool: "deploy".to_string(),
                },
            ),
            at(
                FLOW_FILE,
                "tools[2]",
                Twice {
                    name: "query".to_string(),
                },
            ),
        ]
    );
}
