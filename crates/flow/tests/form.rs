//! The form of a return of a flow, made of the flow and of what is written beside it: the engine's fields of each kind
//! in the order the return gives them, and the tools the flow names; the same whatever the cells write, and with no
//! choice at its top.

// The compile's tests use all of the support; these use what builds a definition and compiles it.
#[allow(dead_code)]
#[path = "support/mod.rs"]
mod support;

use std::path::{Path, PathBuf};

use flow::agents::{self, Agents};
use flow::{Compiled, FLOW_FILE, Has, form};
use serde_json::{Value, json};

use support::{compiled, edit, laid_out};

/// What is written beside the tests' flows.
fn beside() -> Agents {
    agents::read(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/laid_out")).expect("the files are read")
}

/// The small flow laid out one thing to a file, its flow naming these tools.
fn small_with_tools(tools: &str) -> Compiled {
    let source = edit(
        laid_out(),
        FLOW_FILE,
        "data = [\"request\"]",
        &format!("data = [\"request\"]\ntools = {tools}"),
    );
    compiled(&source)
}

/// The form, as JSON read back.
fn parsed(form: &str) -> Value {
    serde_json::from_str(form).expect("the form is JSON")
}

/// The names of an object's members in the order the text writes them.
fn in_order(form: &str, object: &Value) -> Vec<String> {
    let Value::Object(members) = object else {
        panic!("{object} is no object");
    };
    let mut names: Vec<&String> = members.keys().collect();
    names.sort_by_key(|name| form.find(&format!("\"{name}\":")).expect("the member is written"));
    names.into_iter().cloned().collect()
}

#[test]
fn a_flow_that_names_no_tool_has_one_kind_of_return_holding_the_engine_s_fields_of_it_in_their_order() {
    let form = form::form(&compiled(&laid_out()), &beside()).expect("the form is made");
    let form_value = parsed(&form);
    assert_eq!(
        form_value,
        json!({
            "type": "object",
            "properties": {
                "outcome": {
                    "type": "object",
                    "properties": {
                        "dealt_with": { "type": "array", "items": { "type": "string" } },
                        "summary": { "type": "string" },
                        "meaning_unchanged": { "type": "array", "items": { "type": "string" } },
                        "reply": { "type": "string" },
                        "outputs": { "type": "object" },
                    },
                    "required": ["dealt_with"],
                    "additionalProperties": false,
                },
            },
            "minProperties": 1,
            "maxProperties": 1,
            "additionalProperties": false,
        })
    );
    let outcome = &form_value["properties"]["outcome"]["properties"];
    assert_eq!(
        in_order(&form, outcome),
        ["dealt_with", "summary", "meaning_unchanged", "reply", "outputs"]
    );
}

#[test]
fn the_tools_a_flow_names_are_each_a_list_of_requests_in_the_kind_that_asks_for_calls() {
    let form = form::form(&small_with_tools("[\"query\"]"), &beside()).expect("the form is made");
    let form_value = parsed(&form);
    let calls = &form_value["properties"]["calls"];
    assert_eq!(
        calls,
        &json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": { "sql": { "type": "string" } },
                        "required": ["sql"],
                        "additionalProperties": false,
                    },
                },
                "summary": { "type": "string" },
            },
            "additionalProperties": false,
        })
    );
    assert_eq!(in_order(&form, &form_value["properties"]), ["calls", "outcome"]);
    assert_eq!(in_order(&form, &calls["properties"]), ["query", "summary"]);
    // A field of the kind that does the cell's work only is not in the kind that asks for calls.
    assert!(calls["properties"].get("dealt_with").is_none());
}

#[test]
fn the_form_is_two_spaces_to_a_level_and_the_same_each_time_it_is_made() {
    let flow = small_with_tools("[\"query\"]");
    let form = form::form(&flow, &beside()).expect("the form is made");
    assert!(
        form.starts_with("{\n  \"type\": \"object\",\n  \"properties\": {\n    \"calls\": {"),
        "{form}"
    );
    assert_eq!(form::form(&flow, &beside()), Ok(form));
}

#[test]
fn the_form_lists_no_block_and_is_the_same_whatever_the_cells_write() {
    let flow = compiled(&laid_out());
    let form = form::form(&flow, &beside()).expect("the form is made");
    let outputs = &parsed(&form)["properties"]["outcome"]["properties"]["outputs"];
    assert!(outputs.get("properties").is_none(), "no block is listed: {outputs}");
    // The judge writes no opinions, which people still write: what a cell writes is not the form's.
    let fewer = edit(
        laid_out(),
        "cells/judge.toml",
        "outputs = [\"needs_design\", \"opinions\"]",
        "outputs = [\"needs_design\"]",
    );
    assert_eq!(form::form(&compiled(&fewer), &beside()), Ok(form.clone()));
    // Nor is how a flow is laid out: its twin written before has the same form.
    assert_eq!(form::form(&compiled(&support::valid()), &beside()), Ok(form));
}

#[test]
fn the_form_has_no_choice_at_its_top() {
    let form = parsed(&form::form(&small_with_tools("[\"query\"]"), &beside()).expect("the form is made"));
    for word in ["oneOf", "anyOf", "allOf"] {
        assert!(form.get(word).is_none(), "{word} at the top");
    }
}

#[test]
fn a_tool_the_flow_names_that_is_not_defined_beside_the_flows_is_said() {
    let refused = form::form(&small_with_tools("[\"query\", \"run_tests\"]"), &beside()).unwrap_err();
    assert!(refused.contains("run_tests"), "{refused}");
    assert!(refused.contains("agents/tools/"), "{refused}");
}

/// The directory that holds the flows of the repository.
fn repository_flows() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow")
}

/// The engineer's flow as the repository defines it, compiled against what the engine has for it.
fn engineer() -> Compiled {
    let source = flow::read(&repository_flows().join("engineer")).expect("the engineer's flow is read");
    let names = |names: &[&str]| names.iter().map(|n| n.to_string()).collect();
    let has = Has {
        sources: names(&["answer", "asked", "dependencies", "main", "session", "start", "tree"]),
        acts: names(&["start", "check", "merge", "deliver"]),
        tools: names(&["query", "run_tests"]),
        system: names(&["guide", "constitution", "tree_rules", "how_you_work_here"]),
        fields: names(&["dealt_with", "meaning_unchanged", "observations", "questions", "reply"]),
        details: names(&["files"]),
    };
    flow::compile(&source, &has).expect("the engineer's flow compiles")
}

#[test]
fn the_engineer_s_form_is_made_of_the_files_beside_the_repository_s_flows() {
    let beside = agents::read(&repository_flows()).expect("the files beside the flows are read");
    let form = form::form(&engineer(), &beside).expect("the form is made");
    let value = parsed(&form);
    assert_eq!(in_order(&form, &value["properties"]), ["calls", "outcome"]);
    let calls = &value["properties"]["calls"]["properties"];
    assert_eq!(in_order(&form, calls), ["query", "run_tests"]);
    let outcome = &value["properties"]["outcome"];
    assert_eq!(outcome["required"], json!(["dealt_with"]));
    assert_eq!(outcome["additionalProperties"], json!(false));
    // A block is a string, or a choice's value and a line of why, and nothing else.
    let outputs = &outcome["properties"]["outputs"];
    assert_eq!(
        outputs,
        &json!({
            "type": "object",
            "additionalProperties": { "anyOf": [
                { "type": "string" },
                {
                    "type": "object",
                    "properties": { "value": { "type": "string" }, "why": { "type": "string" } },
                    "required": ["value", "why"],
                    "additionalProperties": false,
                },
            ] },
        })
    );
    assert!(
        outcome["properties"].get("summary").is_none(),
        "a return has no summary"
    );
}
