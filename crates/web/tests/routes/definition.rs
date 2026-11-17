//! `GET /api/v1/flow`: the definition shown, compiled or with its faults.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};

use crate::given::the_engineer_s_flow;
use crate::{Shared, ask, edit, get_json, has, serve, tiny};

#[tokio::test]
async fn a_definition_that_compiles_is_given_whole_as_the_page_reads_it() {
    let router = serve(&Shared::new(Ok(tiny())));
    let limits = |runs| json!({ "runs": runs, "returns": 2, "calls": 40 });
    let may_merge = json!({
        "name": "may_merge",
        "words": "The owner let the work merge",
        "file": "flow.toml",
        "kind": "all",
        "reads": ["go"],
        "is": "yes",
    });
    // Each cell is given as its file writes it: what it watches and what it reads, each a datum or a group with the
    // blocks it stands for, what it does when one it watches is new, and each block it is given, with its kind and the
    // list that gives it. An engine's cell is given nothing.
    let watched = |name: &str| json!({ "name": name, "blocks": [name], "do": null, "list": "watches" });
    let given = |block: &str| json!({ "block": block, "kind": "lastAndLatest", "list": "watches" });
    assert_eq!(
        get_json(&router, "/api/v1/flow").await,
        json!({
            "state": "compiled",
            "flow": {
                "name": "tiny",
                "limits": limits(3),
                "lines": [{
                    "name": "engineer",
                    "cells": ["plan", "review"],
                    "levelsAbove": 0,
                    "file": "flow.toml",
                    "readsOnly": false,
                    "testsOnly": false,
                    "system": [],
                    "rules": [],
                }],
                "tests": [may_merge],
                "tools": [],
                "groups": [],
                "card": [],
                "everyAgent": { "watches": [], "reads": [], "kinds": {} },
                "cells": [
                    {
                        "id": "plan",
                        "title": "Plan",
                        "kind": "agent",
                        "line": "engineer",
                        "task": "Write the plan from the request.",
                        "taskFile": null,
                        "act": null,
                        "effort": "high",
                        "takesMain": false,
                        "when": [],
                        "requires": [],
                        "watches": [watched("request"), watched("opinions")],
                        "reads": [],
                        "given": [given("request"), given("opinions")],
                        "outputs": ["plan"],
                        "mustWrite": [],
                        "tools": [],
                        "limits": limits(3),
                    },
                    {
                        "id": "review",
                        "title": "Review",
                        "kind": "agent",
                        "line": "engineer",
                        "task": "Review the plan.\nSay whether it is content.",
                        "taskFile": null,
                        "act": null,
                        "effort": "high",
                        "takesMain": false,
                        "when": [],
                        "requires": [],
                        "watches": [watched("plan")],
                        "reads": [],
                        "given": [given("plan")],
                        "outputs": ["opinions", "content"],
                        "mustWrite": ["content"],
                        "tools": [],
                        "limits": limits(5),
                    },
                    {
                        "id": "merge",
                        "title": "Merge",
                        "kind": "engine",
                        "line": null,
                        "task": null,
                        "taskFile": null,
                        "act": "merge",
                        "effort": null,
                        "takesMain": false,
                        "when": [],
                        "requires": [{ "on": "test", "test": may_merge }],
                        "watches": [watched("content")],
                        "reads": [],
                        "given": [],
                        "outputs": [],
                        "mustWrite": [],
                        "tools": [],
                        "limits": limits(3),
                    },
                ],
                "blocks": [
                    {
                        "name": "content",
                        "values": ["yes", "no"],
                        "default": null,
                        "what": null,
                        "writes": [],
                        "writtenBy": { "cells": ["review"], "people": false, "note": false, "outside": null },
                        "readBy": { "cells": ["merge"], "conditions": [], "tests": [], "shown": false },
                    },
                    {
                        "name": "go",
                        "values": ["yes", "no"],
                        "default": null,
                        "what": null,
                        "writes": [],
                        "writtenBy": { "cells": [], "people": true, "note": false, "outside": null },
                        "readBy": { "cells": [], "conditions": [], "tests": ["may_merge"], "shown": false },
                    },
                    {
                        "name": "opinions",
                        "values": null,
                        "default": null,
                        "what": null,
                        "writes": [],
                        "writtenBy": { "cells": ["review"], "people": true, "note": false, "outside": null },
                        "readBy": { "cells": ["plan"], "conditions": [], "tests": [], "shown": false },
                    },
                    {
                        "name": "plan",
                        "values": null,
                        "default": null,
                        "what": null,
                        "writes": [],
                        "writtenBy": { "cells": ["plan"], "people": false, "note": false, "outside": null },
                        "readBy": { "cells": ["review"], "conditions": [], "tests": [], "shown": false },
                    },
                    {
                        "name": "request",
                        "values": null,
                        "default": null,
                        "what": null,
                        "writes": [],
                        "writtenBy": { "cells": [], "people": true, "note": false, "outside": null },
                        "readBy": { "cells": ["plan"], "conditions": [], "tests": [], "shown": false },
                    },
                ],
            },
        })
    );
}

// A line whose file says its agents change only the project's tests is given so, as written; the definition above holds
// a line that says neither kind.
#[tokio::test]
async fn a_line_that_changes_only_the_tests_is_given_so() {
    let source = edit(tiny(), None, "[lines.engineer]", "[lines.engineer]\ntests_only = true");
    let router = serve(&Shared::new(Ok(source)));
    let answer = get_json(&router, "/api/v1/flow").await;
    let line = &answer["flow"]["lines"][0];
    assert_eq!(
        (&line["name"], &line["testsOnly"], &line["readsOnly"]),
        (&json!("engineer"), &json!(true), &json!(false))
    );
}

#[tokio::test]
async fn a_condition_is_given_with_its_block_and_value_or_with_its_test_as_declared() {
    let source = edit(tiny(), None, "[blocks.content]", "[blocks.content]\nshown = true");
    let source = edit(
        source,
        Some("review"),
        "limit = 5",
        "limit = 5\nwhen = { test = \"may_merge\" }",
    );
    let mut source = edit(
        source,
        None,
        r#"order = ["plan", "review", "merge"]"#,
        r#"order = ["plan", "review", "redo", "merge"]"#,
    );
    let redo = "title = \"Redo\"\nkind = \"engine\"\nact = \"check\"\nwhen = { content = \"no\" }\n";
    source.cells.insert("redo".to_string(), redo.to_string());
    let router = serve(&Shared::new(Ok(source)));
    let answer = get_json(&router, "/api/v1/flow").await;
    assert_eq!(
        answer["flow"]["cells"][1]["when"],
        json!([{
            "on": "test",
            "test": {
                "name": "may_merge",
                "words": "The owner let the work merge",
                "file": "flow.toml",
                "kind": "all",
                "reads": ["go"],
                "is": "yes",
            },
        }])
    );
    assert_eq!(
        answer["flow"]["cells"][2]["when"],
        json!([{ "on": "block", "block": "content", "value": "no" }])
    );
    assert_eq!(
        answer["flow"]["blocks"][0]["readBy"],
        json!({ "cells": ["merge"], "conditions": ["redo"], "tests": [], "shown": true })
    );
}

#[tokio::test]
async fn how_a_cell_takes_a_block_and_what_a_block_is_are_given_as_written() {
    let source = edit(
        tiny(),
        None,
        "[blocks.plan]",
        "[blocks.plan]\nwhat = \"What will be done.\"",
    );
    let source = edit(
        source,
        Some("review"),
        "must_write = [\"content\"]",
        "must_write = [\"content\"]\n\n[do]\nplan = \"Review what changed in it.\"",
    );
    let source = edit(
        source,
        Some("plan"),
        "request = \"last_and_latest\"",
        "request = \"all\"",
    );
    let router = serve(&Shared::new(Ok(source)));
    let answer = get_json(&router, "/api/v1/flow").await;
    assert_eq!(
        answer["flow"]["cells"][1]["watches"],
        json!([{ "name": "plan", "blocks": ["plan"], "do": "Review what changed in it.", "list": "watches" }])
    );
    // Each block an agent's cell watches is given to it in the kind it names; what it does for one it leaves unsaid
    // is none; an engine's cell is given nothing.
    assert_eq!(
        answer["flow"]["cells"][0]["given"],
        json!([
            { "block": "request", "kind": "all", "list": "watches" },
            { "block": "opinions", "kind": "lastAndLatest", "list": "watches" },
        ])
    );
    assert_eq!(answer["flow"]["cells"][0]["watches"][0]["do"], json!(null));
    assert_eq!(answer["flow"]["cells"][2]["given"], json!([]));
    let plan = &answer["flow"]["blocks"][3];
    assert_eq!(
        (&plan["name"], &plan["what"]),
        (&json!("plan"), &json!("What will be done."))
    );
}

#[tokio::test]
async fn what_a_cell_does_for_a_datum_it_does_not_watch_is_a_fault_named_by_watching() {
    let source = edit(
        tiny(),
        Some("review"),
        "must_write = [\"content\"]",
        "must_write = [\"content\"]\n\n[do]\nopinions = \"Answer them.\"",
    );
    let router = serve(&Shared::new(Ok(source)));
    assert_eq!(
        get_json(&router, "/api/v1/flow").await,
        json!({
            "state": "faults",
            "faults": [
                { "file": "review.toml", "field": "do.opinions", "what": { "kind": "notWatched", "block": "opinions" } },
            ],
        })
    );
}

#[tokio::test]
async fn a_block_the_outside_writes_is_given_with_the_name_of_its_source_and_a_block_a_test_reads_with_the_test() {
    let source = edit(
        tiny(),
        None,
        "[blocks.request]\npeople = true",
        "[blocks.request]\noutside = \"main\"",
    );
    let source = edit(source, None, "reads = [\"go\"]", "reads = [\"go\", \"content\"]");
    let router = serve(&Shared::new(Ok(source)));
    let answer = get_json(&router, "/api/v1/flow").await;
    let blocks = &answer["flow"]["blocks"];
    assert_eq!(
        blocks[4]["writtenBy"],
        json!({ "cells": [], "people": false, "note": false, "outside": "main" })
    );
    assert_eq!(blocks[0]["readBy"]["tests"], json!(["may_merge"]));
    assert_eq!(answer["flow"]["tests"][0]["reads"], json!(["go", "content"]));
}

#[tokio::test]
async fn the_faults_of_named_tests_and_of_sources_are_given_with_their_blocks() {
    let source = edit(
        tiny(),
        None,
        "[blocks.request]\npeople = true",
        "[blocks.request]\noutside = \"mirror\"",
    );
    let source = edit(source, None, "reads = [\"go\"]", "reads = [\"content\"]");
    let source = edit(
        source,
        Some("plan"),
        "kind = \"agent\"",
        "kind = \"agent\"\nrequires = { test = \"may_merge\" }",
    );
    let source = edit(
        source,
        Some("review"),
        "kind = \"agent\"",
        "kind = \"agent\"\nrequires = { test = \"may_merge\" }",
    );
    let router = serve(&Shared::new(Ok(source)));
    let read = |block: &str| json!({ "test": "may_merge", "block": block });
    let mut not_before = read("content");
    not_before["kind"] = json!("testNotWrittenBefore");
    let mut own = read("content");
    own["kind"] = json!("testOnOwnOutput");
    assert_eq!(
        get_json(&router, "/api/v1/flow").await,
        json!({
            "state": "faults",
            "faults": [
                { "file": "flow.toml", "field": "blocks.go", "what": { "kind": "noReader", "block": "go" } },
                {
                    "file": "flow.toml",
                    "field": "blocks.request.outside",
                    "what": { "kind": "unknownSource", "block": "request", "source": "mirror" },
                },
                { "file": "plan.toml", "field": "requires.test", "what": not_before },
                { "file": "review.toml", "field": "requires.test", "what": own },
            ],
        })
    );

    let source = edit(
        tiny(),
        None,
        "kind = \"all\"\nreads = [\"go\"]\nis = \"yes\"",
        "kind = \"none_standing\"\nreads = [\"content\"]\nis = \"no\"",
    );
    let source = edit(
        source,
        Some("plan"),
        "outputs = [\"plan\"]",
        "outputs = [\"plan\", \"content\"]",
    );
    let router = serve(&Shared::new(Ok(source)));
    let faults = &get_json(&router, "/api/v1/flow").await["faults"];
    assert_eq!(
        faults[1],
        json!({
            "file": "flow.toml",
            "field": "tests.may_merge.reads[0]",
            "what": { "kind": "standingWrittenByTwo", "test": "may_merge", "block": "content", "cell": "review" },
        })
    );

    let source = edit(tiny(), None, "reads = [\"go\"]", "reads = []");
    let router = serve(&Shared::new(Ok(source)));
    let faults = &get_json(&router, "/api/v1/flow").await["faults"];
    assert_eq!(
        faults[1],
        json!({ "file": "flow.toml", "field": "tests.may_merge.reads", "what": { "kind": "readsEmpty", "test": "may_merge" } })
    );
}

#[tokio::test]
async fn a_block_a_cell_must_write_that_is_not_among_its_outputs_is_a_fault_given_with_the_block() {
    let source = edit(
        tiny(),
        Some("review"),
        "must_write = [\"content\"]",
        "must_write = [\"content\", \"plan\"]",
    );
    let router = serve(&Shared::new(Ok(source)));
    assert_eq!(
        get_json(&router, "/api/v1/flow").await,
        json!({
            "state": "faults",
            "faults": [
                { "file": "review.toml", "field": "must_write[1]", "what": { "kind": "notAnOutput", "block": "plan" } },
            ],
        })
    );
}

#[tokio::test]
async fn the_faults_of_a_cell_s_task_and_act_are_given_with_the_act_it_names() {
    let source = edit(
        tiny(),
        Some("plan"),
        "task = \"Write the plan from the request.\"\n",
        "",
    );
    let source = edit(
        source,
        Some("review"),
        "kind = \"agent\"",
        "kind = \"agent\"\nact = \"merge\"",
    );
    let source = edit(
        source,
        Some("merge"),
        "act = \"merge\"",
        "act = \"deploy\"\ntask = \"Merge it.\"",
    );
    let router = serve(&Shared::new(Ok(source)));
    assert_eq!(
        get_json(&router, "/api/v1/flow").await,
        json!({
            "state": "faults",
            "faults": [
                { "file": "plan.toml", "field": "task", "what": { "kind": "noTask" } },
                { "file": "review.toml", "field": "act", "what": { "kind": "agentHasAct", "act": "merge" } },
                { "file": "merge.toml", "field": "task", "what": { "kind": "engineHasTask" } },
                { "file": "merge.toml", "field": "act", "what": { "kind": "unknownAct", "act": "deploy" } },
            ],
        })
    );

    let source = edit(tiny(), Some("merge"), "act = \"merge\"\n", "");
    let router = serve(&Shared::new(Ok(source)));
    assert_eq!(
        get_json(&router, "/api/v1/flow").await["faults"],
        json!([{ "file": "merge.toml", "field": "act", "what": { "kind": "noAct" } }])
    );
}

#[tokio::test]
async fn a_test_of_what_stands_is_given_with_its_kind() {
    let source = edit(
        tiny(),
        None,
        "kind = \"all\"\nreads = [\"go\"]",
        "kind = \"none_standing\"\nreads = [\"go\", \"content\"]",
    );
    let source = edit(source, None, "is = \"yes\"", "is = \"no\"");
    // The review, whose content the test reads, is a judge: it holds its dialogue with the plan in the opinions.
    let source = edit(
        source,
        Some("review"),
        "watches = [\"plan\"]",
        "watches = [\"plan\", \"opinions\"]",
    );
    let source = edit(
        source,
        Some("review"),
        "plan = \"last_and_latest\"",
        "plan = \"last_and_latest\"\nopinions = \"all\"",
    );
    let source = edit(
        source,
        Some("plan"),
        "outputs = [\"plan\"]",
        "outputs = [\"plan\", \"opinions\"]",
    );
    let router = serve(&Shared::new(Ok(source)));
    let answer = get_json(&router, "/api/v1/flow").await;
    assert_eq!(
        answer["flow"]["tests"][0],
        json!({
            "name": "may_merge",
            "words": "The owner let the work merge",
            "file": "flow.toml",
            "kind": "noneStanding",
            "reads": ["go", "content"],
            "is": "no",
        })
    );
}

#[tokio::test]
async fn a_test_of_content_is_given_with_its_kind_and_no_value() {
    let source = edit(
        tiny(),
        None,
        "kind = \"all\"\nreads = [\"go\"]\nis = \"yes\"",
        "kind = \"any_content\"\nreads = [\"go\", \"plan\"]",
    );
    let router = serve(&Shared::new(Ok(source)));
    let answer = get_json(&router, "/api/v1/flow").await;
    assert_eq!(
        answer["flow"]["tests"][0],
        json!({
            "name": "may_merge",
            "words": "The owner let the work merge",
            "file": "flow.toml",
            "kind": "anyContent",
            "reads": ["go", "plan"],
            "is": null,
        })
    );
}

#[tokio::test]
async fn a_definition_that_does_not_compile_is_given_as_its_faults_each_with_its_file_and_field() {
    let source = edit(
        tiny(),
        Some("plan"),
        "[\"request\", \"opinions\"]",
        "[\"request\", \"opinions\", \"ghost\"]",
    );
    let source = edit(source, None, "calls = 40", "calls = 0");
    let router = serve(&Shared::new(Ok(source)));
    assert_eq!(
        get_json(&router, "/api/v1/flow").await,
        json!({
            "state": "faults",
            "faults": [
                {
                    "file": "flow.toml",
                    "field": "limits.calls",
                    "what": { "kind": "limitOutOfRange", "value": 0 },
                },
                {
                    "file": "plan.toml",
                    "field": "watches[2]",
                    "what": { "kind": "noSuchBlock", "block": "ghost" },
                },
            ],
        })
    );
}

#[tokio::test]
async fn a_file_that_cannot_be_read_is_a_fault_of_the_whole_file_with_the_parser_s_words() {
    let mut source = tiny();
    source.cells.insert("review".to_string(), "title = ".to_string());
    let router = serve(&Shared::new(Ok(source)));
    let answer = get_json(&router, "/api/v1/flow").await;
    let fault = &answer["faults"][0];
    assert_eq!((&fault["file"], &fault["field"]), (&json!("review.toml"), &json!("")));
    assert_eq!(fault["what"]["kind"], "unreadable");
    assert!(
        fault["what"]["message"].as_str().is_some_and(|m| !m.is_empty()),
        "{fault}"
    );
}

#[tokio::test]
async fn a_directory_that_cannot_be_read_is_said_so_with_its_path() {
    let missing = std::env::temp_dir().join(format!("web-no-flow-{}", std::process::id()));
    let router = web::router(web::Flows::dirs(missing.clone(), Box::new(has)));
    let answer = get_json(&router, "/api/v1/flow").await;
    assert_eq!(answer["state"], "unreadable");
    let reason = answer["reason"].as_str().unwrap();
    assert!(reason.starts_with(&format!("{}: ", missing.display())), "{reason}");
}

#[tokio::test]
async fn a_directory_is_read_again_for_each_request_so_that_an_edit_shows() {
    let dir = std::env::temp_dir().join(format!("web-flow-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let write = |source: &flow::Source| {
        std::fs::write(dir.join("flow.toml"), &source.flow).unwrap();
        for (id, text) in &source.cells {
            std::fs::write(dir.join(format!("{id}.toml")), text).unwrap();
        }
    };
    write(&tiny());
    let router = web::router(web::Flows::dirs(PathBuf::from(&dir), Box::new(has)));
    assert_eq!(get_json(&router, "/api/v1/flow").await["flow"]["name"], "tiny");
    write(&edit(tiny(), None, "name = \"tiny\"", "name = \"small\""));
    let answer = get_json(&router, "/api/v1/flow").await;
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(answer["flow"]["name"], "small");
}

#[tokio::test]
async fn what_the_reader_gives_is_read_again_for_each_request() {
    let shown = Shared::new(Ok(tiny()));
    let router = serve(&shown);
    assert_eq!(get_json(&router, "/api/v1/flow").await["state"], "compiled");
    shown.set(Err("gone".to_string()));
    assert_eq!(
        get_json(&router, "/api/v1/flow").await,
        json!({ "state": "unreadable", "reason": "gone" })
    );
}

/// A TOML file of the engineer flow, as written.
fn engineer_file(path: &str) -> toml::Table {
    let at = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../flow/engineer")
        .join(path);
    toml::from_str(&std::fs::read_to_string(&at).unwrap()).unwrap()
}

/// The strings of a list a TOML table holds under this key; none where it holds none.
fn written(table: &toml::Table, key: &str) -> Vec<String> {
    (table.get(key).and_then(|v| v.as_array()).into_iter().flatten())
        .map(|v| v.as_str().unwrap().to_string())
        .collect()
}

/// The strings of a table a TOML table holds under this key, by their keys; none where it holds none.
fn by_name(table: &toml::Table, key: &str) -> BTreeMap<String, String> {
    (table.get(key).and_then(|v| v.as_table()).into_iter().flatten())
        .map(|(k, v)| (k.clone(), v.as_str().unwrap().to_string()))
        .collect()
}

/// The strings of a JSON list.
fn names_of(list: &Value) -> Vec<String> {
    (list.as_array().unwrap().iter())
        .map(|v| v.as_str().unwrap().to_string())
        .collect()
}

// What every cell of the repository's engineer flow watches and reads, and what every agent cell is given alike, are
// given as the files write them (principle definitions-shown-as-written): each name as written, a group's with its
// blocks as `flow.toml` names them, a name with `{each}` with the datum of each cell that has one (decision
// a-datum-of-a-cell-s-own-is-made-where-a-cell-names-it), and each block a cell is given by the list that gives it,
// what every agent cell is given first and none of it a block the cell's own file names.
#[tokio::test]
async fn the_engineer_flow_s_cells_are_given_as_their_files_write_what_they_watch_and_read() {
    let answer = get_json(&the_engineer_s_flow(), "/api/v1/flow").await;
    let flow = &answer["flow"];
    let file = engineer_file("flow.toml");
    let groups: BTreeMap<String, Vec<String>> = (file["groups"].as_table().unwrap().iter())
        .map(|(name, blocks)| {
            (
                name.clone(),
                (blocks.as_array().unwrap().iter())
                    .map(|b| b.as_str().unwrap().to_string())
                    .collect(),
            )
        })
        .collect();
    let order = written(&file, "order");
    let cells: Vec<(String, toml::Table)> = order
        .iter()
        .map(|id| (id.clone(), engineer_file(&format!("cells/{id}.toml"))))
        .collect();
    // The blocks a name a cell writes stands for: a group's, or the datum alone.
    let blocks_of = |name: &str| groups.get(name).cloned().unwrap_or_else(|| vec![name.to_string()]);
    // The blocks a name of what every agent cell takes stands for: with `{each}`, the datum of each cell, in the order
    // of the walk, that some cell's file names.
    let named_anywhere = |block: &str| {
        (cells.iter()).any(|(_, f)| {
            ["watches", "reads", "outputs"]
                .iter()
                .any(|k| written(f, k).iter().any(|n| n == block))
        })
    };
    let every_blocks = |name: &str| -> Vec<String> {
        match name.contains("{each}") {
            true => (order.iter())
                .map(|id| name.replace("{each}", &id.to_lowercase()))
                .filter(|block| named_anywhere(block))
                .collect(),
            false => vec![name.to_string()],
        }
    };
    let every = file["every_agent"].as_table().unwrap();
    let every_does = by_name(every, "do");
    let taken = |list: &str| -> Value {
        Value::Array(
            (written(every, list).iter())
                .map(|name| {
                    let mut entry = json!({ "name": name, "blocks": every_blocks(name) });
                    if list == "watches" {
                        entry["do"] = json!(every_does.get(name));
                    }
                    entry
                })
                .collect(),
        )
    };
    let kinds: BTreeMap<String, String> = by_name(every, "kinds");
    assert_eq!(
        flow["everyAgent"],
        json!({ "watches": taken("watches"), "reads": taken("reads"), "kinds": kinds_json(&kinds) })
    );

    let every_watched: Vec<String> = written(every, "watches").iter().flat_map(|n| every_blocks(n)).collect();
    let every_read: Vec<String> = written(every, "reads").iter().flat_map(|n| every_blocks(n)).collect();
    for ((id, own), cell) in cells.iter().zip(flow["cells"].as_array().unwrap()) {
        assert_eq!(cell["id"], json!(id));
        let agent = cell["kind"] == json!("agent");
        let does = by_name(own, "do");
        let mine: Vec<String> = ["reads", "watches"]
            .iter()
            .flat_map(|k| written(own, k))
            .flat_map(|n| blocks_of(&n))
            .collect();
        // What runs it: of an agent's cell, each datum every agent cell watches that its own file does not name, by
        // the datum's name, what `[every_agent]` says to do of it given once, with `[every_agent]` (principle
        // say-a-thing-once); then what its own file watches, each with what its file says to do.
        let mut watches: Vec<Value> = Vec::new();
        for written_name in written(every, "watches").iter().filter(|_| agent) {
            for block in every_blocks(written_name).into_iter().filter(|b| !mine.contains(b)) {
                watches.push(json!({ "name": block, "blocks": [block], "list": "every_agent" }));
            }
        }
        for name in written(own, "watches") {
            watches.push(json!({ "name": name, "blocks": blocks_of(&name), "do": does.get(&name), "list": "watches" }));
        }
        let reads: Vec<Value> = (written(own, "reads").iter())
            .map(|name| json!({ "name": name, "blocks": blocks_of(name) }))
            .collect();
        assert_eq!(cell["watches"], json!(watches), "{id}");
        assert_eq!(cell["reads"], json!(reads), "{id}");
        // What it is given, by list: what every agent cell is given that its own file does not name, then what it
        // reads, then what it watches, each name's blocks in order; nothing for an engine's cell.
        let lists: Vec<(String, String)> = (cell["given"].as_array().unwrap().iter())
            .map(|g| {
                (
                    g["block"].as_str().unwrap().to_string(),
                    g["list"].as_str().unwrap().to_string(),
                )
            })
            .collect();
        let mut expected: Vec<(String, String)> = Vec::new();
        if agent {
            for block in every_watched.iter().chain(&every_read).filter(|b| !mine.contains(b)) {
                expected.push((block.clone(), "every_agent".to_string()));
            }
            for list in ["reads", "watches"] {
                for block in written(own, list).iter().flat_map(|n| blocks_of(n)) {
                    expected.push((block, list.to_string()));
                }
            }
        }
        assert_eq!(lists, expected, "{id}");
        assert_eq!(names_of(&cell["outputs"]), written(own, "outputs"), "{id}");
    }
}

/// The kinds of what every agent cell takes as the page reads them, by the name written.
fn kinds_json(kinds: &BTreeMap<String, String>) -> Value {
    let page = |kind: &str| match kind {
        "latest" => "latest",
        "last_and_latest" => "lastAndLatest",
        "all" => "all",
        other => panic!("no kind {other}"),
    };
    Value::Object(
        kinds
            .iter()
            .map(|(name, kind)| (name.clone(), json!(page(kind))))
            .collect(),
    )
}

// The page's tests of the flowchart read the engineer flow from a copy of what this route gives for it
// (`client/src/graph/engineer-flow.json`), so that what the chart draws of the real flow is known. The copy must be
// what the route gives now: kept from an earlier flow, those tests would test a flow that is not there.
#[tokio::test]
async fn the_page_s_copy_of_the_engineer_flow_is_what_the_route_gives_for_it() {
    let (status, _, body) = ask(&the_engineer_s_flow(), Method::GET, "/api/v1/flow").await;
    assert_eq!(status, StatusCode::OK);
    let copy = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../client/src/graph/engineer-flow.json");
    let kept = std::fs::read(&copy).unwrap();
    assert!(
        kept == body,
        "client/src/graph/engineer-flow.json is not what the route gives for flow/engineer; it must hold this one line \
         (between the markers):\n>>>\n{}\n<<<",
        String::from_utf8_lossy(&body)
    );
}
