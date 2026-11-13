//! The tools of the engine's a cell may ask for in its return: each a tool the engine has, named once, and only by an
//! agent cell; compiled in the order written.

use flow::FaultKind::*;
use flow::Has;

use crate::support::{at, compiled, edit, faults, faults_with, has, valid};

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
