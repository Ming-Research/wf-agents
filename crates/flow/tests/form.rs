//! The form of a return of a line, made of the flow and of what is written beside it: the engine's fields of the kind
//! that does the cell's work, in the order the return gives them, `outputs` holding exactly the blocks the cells of the
//! line write, each in its shape; and the tools the flow names, in the kind that asks for tools and nothing beside them;
//! with no choice at its top, and the same for every cell of the line.

// The compile's tests use all of the support; these use what builds a definition and compiles it.
#[allow(dead_code)]
#[path = "support/mod.rs"]
mod support;

use std::path::{Path, PathBuf};

use decide::BlockId;
use flow::agents::{self, Agents};
use flow::{Compiled, FLOW_FILE, Has, Kind, form};
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

/// The names of an object's members in the order the text writes them, from where `from` is first written in it.
fn in_order(form: &str, from: &str, object: &Value) -> Vec<String> {
    let Value::Object(members) = object else {
        panic!("{object} is no object");
    };
    let start = form.find(from).expect("it is written");
    let mut names: Vec<&String> = members.keys().collect();
    names.sort_by_key(|name| {
        form[start..]
            .find(&format!("\"{name}\":"))
            .expect("the member is written")
    });
    names.into_iter().cloned().collect()
}

/// The blocks the form lists in `outputs`, in the order it writes them.
fn listed(form: &str) -> Vec<String> {
    let outputs = &parsed(form)["properties"]["outcome"]["properties"]["outputs"];
    in_order(form, "\"outputs\": {", &outputs["properties"])
}

/// The blocks the agent cells of the line write, each once, in the order of the walk and of each cell's outputs.
fn written_by(flow: &Compiled, line: &str) -> Vec<String> {
    let mut blocks: Vec<String> = Vec::new();
    for cell in (flow.flow.cells().iter()).filter(|c| {
        (flow.cells.iter()).any(|s| s.id == c.id && s.kind == Kind::Agent && s.line.as_deref() == Some(line))
    }) {
        for block in &cell.outputs {
            if !blocks.iter().any(|b| b == block.as_str()) {
                blocks.push(block.as_str().to_string());
            }
        }
    }
    blocks
}

/// The shape of a choice of these values, as the form holds it.
fn choice(values: &[&str]) -> Value {
    json!({
        "type": "object",
        "properties": { "value": { "type": "string", "enum": values }, "why": { "type": "string" } },
        "required": ["value", "why"],
        "additionalProperties": false,
    })
}

#[test]
fn a_line_s_form_lists_in_outputs_exactly_the_blocks_its_cells_write_each_in_its_shape() {
    let flow = compiled(&laid_out());
    let review = form::form(&flow, &beside(), "review").expect("the form is made");
    assert_eq!(
        parsed(&review),
        json!({
            "type": "object",
            "properties": {
                "outcome": {
                    "type": "object",
                    "properties": {
                        "dealt_with": { "type": "array", "items": { "type": "string" } },
                        "summary": { "type": "string" },
                        "meaning_unchanged": { "type": "array", "items": { "type": "string" } },
                        "outputs": {
                            "type": "object",
                            "properties": { "needs_design": choice(&["yes", "no"]), "opinions": { "type": "string" } },
                            "additionalProperties": false,
                        },
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
    let outcome = &parsed(&review)["properties"]["outcome"]["properties"];
    assert_eq!(
        in_order(&review, "\"outcome\"", outcome),
        ["dealt_with", "summary", "meaning_unchanged", "outputs"]
    );
    // Each line's form lists the blocks of its own cells, in the order of the walk, and no other.
    for line in flow.lines.keys() {
        let form = form::form(&flow, &beside(), line).expect("the form is made");
        assert_eq!(listed(&form), written_by(&flow, line), "{line}");
    }
    let engineer = form::form(&flow, &beside(), "engineer").expect("the form is made");
    assert_eq!(listed(&engineer), ["plan", "design", "reply"]);
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
        acts: names(&["start", "check", "merge", "deliver", "run_tests"]),
        tools: names(&["query"]),
        system: names(&["guide", "constitution", "tree_rules", "how_you_work_here"]),
        details: names(&["files"]),
    };
    flow::compile(&source, &has).expect("the engineer's flow compiles")
}

// The work's questions are in the form of a line one of whose cells writes them, in the shape their own file gives,
// and in no other line's: a cell of a line that writes none is not told by its form that it may ask. So is the record
// of what is said, a text, where a cell of the line writes it.
#[test]
fn the_engineer_s_lines_each_list_their_cells_blocks_and_only_those_that_write_questions_have_them() {
    let flow = engineer();
    let beside = agents::read(&repository_flows()).expect("the files beside the flows are read");
    let questions = flow.blocks[&BlockId::from("q_and_a")]
        .shape
        .clone()
        .expect("the questions' shape")
        .0;
    let questions = serde_json::to_value(&questions).expect("the shape is JSON");
    for line in flow.lines.keys() {
        let form = form::form(&flow, &beside, line).expect("the form is made");
        let blocks = written_by(&flow, line);
        assert_eq!(listed(&form), blocks, "{line}");
        let outputs = &parsed(&form)["properties"]["outcome"]["properties"]["outputs"];
        assert_eq!(outputs["additionalProperties"], json!(false), "{line}");
        let asks = blocks.iter().any(|b| b == "q_and_a");
        assert_eq!(
            outputs["properties"].get("q_and_a"),
            asks.then_some(&questions),
            "{line}"
        );
        if blocks.iter().any(|b| b == "discussion") {
            assert_eq!(
                outputs["properties"]["discussion"],
                json!({ "type": "string" }),
                "{line}"
            );
        }
        for block in &blocks {
            let declared = &flow.blocks[&BlockId::from(block.as_str())];
            if let Some(values) = &declared.values {
                let values: Vec<&str> = values.iter().map(String::as_str).collect();
                assert_eq!(outputs["properties"][block], choice(&values), "{line} {block}");
            }
        }
    }
    let asking: Vec<&str> = (flow.lines.keys().map(String::as_str))
        .filter(|line| written_by(&flow, line).iter().any(|b| b == "q_and_a"))
        .collect();
    let quiet: Vec<&str> = (flow.lines.keys().map(String::as_str))
        .filter(|line| !asking.contains(line))
        .collect();
    assert!(!asking.is_empty() && !quiet.is_empty(), "{asking:?} {quiet:?}");
}

#[test]
fn the_tools_a_flow_names_are_each_a_list_of_requests_in_the_kind_that_asks_for_tools_and_nothing_else() {
    let form = form::form(&small_with_tools("[\"query\"]"), &beside(), "engineer").expect("the form is made");
    let form_value = parsed(&form);
    let tools = &form_value["properties"]["tools"];
    assert_eq!(
        tools,
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
            },
            "additionalProperties": false,
        })
    );
    assert_eq!(in_order(&form, "{", &form_value["properties"]), ["tools", "outcome"]);
}

#[test]
fn the_form_is_two_spaces_to_a_level_and_the_same_each_time_it_is_made() {
    let flow = small_with_tools("[\"query\"]");
    let form = form::form(&flow, &beside(), "engineer").expect("the form is made");
    assert!(
        form.starts_with("{\n  \"type\": \"object\",\n  \"properties\": {\n    \"tools\": {"),
        "{form}"
    );
    assert_eq!(form::form(&flow, &beside(), "engineer"), Ok(form));
}

// The form a session is given is a function of its line alone: a cell of another line writing other blocks leaves it
// as it is, and one of the line's own cells writing another block changes it. Nor does how a flow is laid out change it.
#[test]
fn a_line_s_form_is_the_same_whatever_the_cells_of_other_lines_write() {
    let flow = compiled(&laid_out());
    let engineer = form::form(&flow, &beside(), "engineer").expect("the form is made");
    let review = form::form(&flow, &beside(), "review").expect("the form is made");
    // The judge, of the review's line, writes no opinions, which people still write.
    let fewer = compiled(&edit(
        laid_out(),
        "cells/judge.toml",
        "outputs = [\"needs_design\", \"opinions\"]",
        "outputs = [\"needs_design\"]",
    ));
    assert_eq!(form::form(&fewer, &beside(), "engineer"), Ok(engineer.clone()));
    let narrower = form::form(&fewer, &beside(), "review").expect("the form is made");
    assert_ne!(narrower, review);
    assert_eq!(listed(&narrower), ["needs_design"]);
    // Its twin written before it was laid out has the same forms.
    let twin = compiled(&support::valid());
    assert_eq!(form::form(&twin, &beside(), "engineer"), Ok(engineer));
    assert_eq!(form::form(&twin, &beside(), "review"), Ok(review));
}

// The form is made in three parts, which together are the form as it is sent: the engine's fields, with the kind that
// asks for tools holding none and `outputs` no block; the line's blocks, as `outputs` holds them; and the flow's tools,
// as that kind holds them. Nothing is in two of them.
#[test]
fn the_three_parts_of_a_line_s_form_together_are_the_form_and_none_holds_what_another_does() {
    let flow = small_with_tools("[\"query\"]");
    for line in ["engineer", "review"] {
        let form = parsed(&form::form(&flow, &beside(), line).expect("the form is made"));
        let parts = form::parts(&flow, &beside(), line).expect("the parts are made");
        let fields = parsed(&parts.fields);
        let blocks = parsed(parts.blocks.as_deref().expect("the form has outputs"));
        let tools = parsed(parts.tools.as_deref().expect("the flow names a tool"));
        assert_eq!(fields["properties"]["tools"]["properties"], json!({}), "{line}");
        let outputs = &fields["properties"]["outcome"]["properties"]["outputs"];
        assert!(outputs.get("properties").is_none(), "{line}: {outputs}");
        assert_eq!(tools, form["properties"]["tools"], "{line}");
        assert_eq!(blocks, form["properties"]["outcome"]["properties"]["outputs"], "{line}");
        let mut whole = fields.clone();
        whole["properties"]["tools"] = tools;
        whole["properties"]["outcome"]["properties"]["outputs"] = blocks;
        assert_eq!(whole, form, "{line}");
    }
    // A flow that names no tool has no part of tools, and no kind that asks for them.
    let parts = form::parts(&compiled(&laid_out()), &beside(), "review").expect("the parts are made");
    assert_eq!(parts.tools, None);
    assert!(parsed(&parts.fields)["properties"].get("tools").is_none());
}

#[test]
fn the_form_has_no_choice_at_its_top() {
    let form = parsed(&form::form(&small_with_tools("[\"query\"]"), &beside(), "engineer").expect("the form is made"));
    for word in ["oneOf", "anyOf", "allOf"] {
        assert!(form.get(word).is_none(), "{word} at the top");
    }
}

#[test]
fn a_tool_the_flow_names_that_is_not_defined_beside_the_flows_is_said() {
    let refused = form::form(&small_with_tools("[\"query\", \"run_tests\"]"), &beside(), "engineer").unwrap_err();
    assert!(refused.contains("run_tests"), "{refused}");
    assert!(refused.contains("agents/tools/"), "{refused}");
}

#[test]
fn the_engineer_s_form_is_made_of_the_files_beside_the_repository_s_flows() {
    let beside = agents::read(&repository_flows()).expect("the files beside the flows are read");
    let form = form::form(&engineer(), &beside, "engineer").expect("the form is made");
    let value = parsed(&form);
    assert_eq!(in_order(&form, "{", &value["properties"]), ["tools", "outcome"]);
    let tools = &value["properties"]["tools"]["properties"];
    assert_eq!(in_order(&form, "\"tools\": {", tools), ["query"]);
    let outcome = &value["properties"]["outcome"];
    assert_eq!(outcome["required"], json!(["dealt_with"]));
    assert_eq!(outcome["additionalProperties"], json!(false));
    assert_eq!(
        in_order(&form, "\"outcome\": {", &outcome["properties"]),
        [
            "dealt_with",
            "meaning_unchanged",
            "observations",
            "protected_changes",
            "outputs"
        ]
    );
    // Why the work changes each file the project protects: a text under each file's path.
    assert_eq!(
        outcome["properties"]["protected_changes"],
        json!({ "type": "object", "additionalProperties": { "type": "string" } })
    );
}

/// A shape, written as a block's file writes it.
fn shape_of(text: &str) -> flow::Shape {
    flow::Shape(toml::from_str(text).expect("a shape"))
}

// A value is of a shape where it is of the type it says, each property of the shape it gives with those it requires and
// no other where it allows none, each item of the shape it gives, of one of the shapes `anyOf` gives, and one of the
// values `enum` gives; as the record of the questions and a choice are written. Well under a millisecond alone; no
// other test holds a value to a shape.
#[test]
fn a_value_is_of_a_shape_by_each_word_the_shape_is_written_in() {
    let text = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow/data/q_and_a.toml"))
        .expect("the record of the questions");
    let table: toml::Table = toml::from_str(&text).expect("TOML");
    let questions = flow::Shape(table["shape"].clone());
    let asked = json!({"name": "a", "waits": true, "recommends": 1, "ask": "Which?", "options": [
        {"does": "this", "follows": "that"}
    ]});
    let withdrawn = json!({"name": "a", "withdrawn": "no longer"});
    assert!(form::fits(&json!([asked.clone(), withdrawn.clone()]), &questions));
    assert!(form::fits(&json!([]), &questions));
    let mut wrong = vec![json!({"name": "a"}), json!(asked.clone())];
    wrong[1]["recommends"] = json!("1");
    for item in wrong {
        assert!(!form::fits(&json!([item]), &questions), "{item}");
    }
    let mut more = asked.clone();
    more["also"] = json!(true);
    assert!(!form::fits(&json!([more]), &questions));
    let mut option = asked.clone();
    option["options"][0]["does"] = json!(1);
    assert!(!form::fits(&json!([option]), &questions));
    assert!(!form::fits(&json!(asked), &questions), "an array");
    let choice = shape_of(
        "type = \"object\"\nrequired = [\"value\"]\nproperties = { value = { type = \"string\", enum = [\"yes\", \"no\"] } }\n",
    );
    assert!(form::fits(&json!({"value": "yes", "why": "it holds"}), &choice));
    assert!(!form::fits(&json!({"value": "maybe"}), &choice));
    assert!(!form::fits(&json!({"why": "it holds"}), &choice));
    let open = shape_of("type = \"object\"\nadditionalProperties = { type = \"integer\" }\n");
    assert!(form::fits(&json!({"a": 1, "b": 2}), &open));
    assert!(
        form::fits(&json!({"a": 1.0}), &open),
        "a whole number written as a float"
    );
    assert!(!form::fits(&json!({"a": 1.5}), &open));
}

// What of a shape the engine holds no return to as it says is found at every depth: a word it does not check, a word it
// checks written in a form it does not read, and a type it does not know; a shape in the words it checks has none.
// Well under a millisecond alone; no other test holds the words of a shape to those the engine checks.
#[test]
fn a_word_of_a_shape_the_engine_does_not_check_is_found_at_any_depth() {
    let checked = shape_of(
        "type = \"array\"\nitems = { anyOf = [{ type = \"object\", properties = { a = { type = \"string\" } }, \
         required = [\"a\"], additionalProperties = false }, { enum = [\"b\"] }] }\n",
    );
    assert_eq!(form::unchecked(&checked), Vec::<String>::new());
    let unchecked = shape_of(
        "type = \"array\"\nminItems = 1\nitems = { anyOf = [{ type = \"date\" }, { type = \"object\", properties = \
         { a = { type = \"string\", pattern = \"^a\" } } }] }\n",
    );
    assert_eq!(
        form::unchecked(&unchecked),
        ["`minItems`", "the type `date`", "`pattern`"]
    );
    let forms = shape_of(
        "type = \"object\"\nproperties = { a = { type = [\"string\", \"null\"] }, b = { type = \"array\", items = [{ \
         type = \"string\" }] }, c = { enum = \"x\" }, d = { type = \"object\", required = \"a\" }, e = { type = \
         \"object\", required = [1] }, f = { anyOf = { type = \"string\" } }, g = { anyOf = [] }, h = { type = \
         \"object\", properties = [\"a\"] }, i = { type = \"object\", additionalProperties = \"no\" }, j = { enum = [] } \
         }\n",
    );
    assert_eq!(
        form::unchecked(&forms),
        [
            "`type` written as an array",
            "`items` written as an array",
            "`enum` written as a string",
            "`required` written as a string",
            "`required` holding other than the names of properties",
            "`anyOf` written as a table",
            "`anyOf` written as an empty array",
            "`properties` written as an array",
            "`additionalProperties` written as a string",
            "`enum` written as an empty array",
        ]
    );
}

// The record a cell asks for tests in takes the commit an entry names, as a string, and nothing else beside the tests and
// the checks. Well under a millisecond alone; no other test holds the record's shape to what an entry may name.
#[test]
fn an_entry_of_the_tests_record_may_name_its_commit_as_a_string() {
    let text = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow/data/run_tests.toml"))
        .expect("the record of the tests");
    let table: toml::Table = toml::from_str(&text).expect("TOML");
    let entries = flow::Shape(table["shape"].clone());
    let full = "0123456789abcdef0123456789abcdef01234567";
    assert!(form::fits(
        &json!({"tests": ["a"], "checks": [], "commit": full}),
        &entries
    ));
    assert!(form::fits(&json!({"tests": ["a"]}), &entries));
    assert!(!form::fits(&json!({"tests": ["a"], "commit": 1}), &entries));
    assert!(!form::fits(&json!({"tests": ["a"], "at": full}), &entries));
}
