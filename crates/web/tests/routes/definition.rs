//! `GET /api/v1/flow`: the definition shown, compiled or with its faults.

use std::path::PathBuf;

use serde_json::json;

use crate::{Shared, edit, get_json, has, serve, tiny};

#[tokio::test]
async fn a_definition_that_compiles_is_given_whole_as_the_page_reads_it() {
    let router = serve(&Shared::new(Ok(tiny())), None);
    let limits = |runs| json!({ "runs": runs, "returns": 2, "calls": 40 });
    let may_merge = json!({
        "name": "may_merge",
        "words": "The owner let the work merge",
        "kind": "all",
        "reads": ["go"],
        "is": "yes",
    });
    assert_eq!(
        get_json(&router, "/api/v1/flow").await,
        json!({
            "state": "compiled",
            "flow": {
                "name": "tiny",
                "limits": limits(3),
                "lines": [{ "name": "engineer", "cells": ["plan", "review"], "levelsAbove": 0 }],
                "tests": [may_merge],
                "cells": [
                    {
                        "id": "plan",
                        "title": "Plan",
                        "kind": "agent",
                        "line": "engineer",
                        "task": "Write the plan from the request.",
                        "act": null,
                        "effort": "high",
                        "condition": null,
                        "preconditions": [],
                        "inputs": ["request", "opinions"],
                        "input": {},
                        "outputs": ["plan"],
                        "mustWrite": [],
                        "limits": limits(3),
                    },
                    {
                        "id": "review",
                        "title": "Review",
                        "kind": "agent",
                        "line": "engineer",
                        "task": "Review the plan.\nSay whether it is content.",
                        "act": null,
                        "effort": "high",
                        "condition": null,
                        "preconditions": [],
                        "inputs": ["plan"],
                        "input": {},
                        "outputs": ["opinions", "content"],
                        "mustWrite": ["content"],
                        "limits": limits(5),
                    },
                    {
                        "id": "merge",
                        "title": "Merge",
                        "kind": "engine",
                        "line": null,
                        "task": null,
                        "act": "merge",
                        "effort": null,
                        "condition": null,
                        "preconditions": [may_merge],
                        "inputs": ["content"],
                        "input": {},
                        "outputs": [],
                        "mustWrite": [],
                        "limits": limits(3),
                    },
                ],
                "blocks": [
                    {
                        "name": "content",
                        "values": ["yes", "no"],
                        "what": null,
                        "writtenBy": { "cells": ["review"], "people": false, "note": false, "outside": null },
                        "readBy": { "cells": ["merge"], "conditions": [], "tests": [], "shown": false },
                    },
                    {
                        "name": "go",
                        "values": ["yes", "no"],
                        "what": null,
                        "writtenBy": { "cells": [], "people": true, "note": false, "outside": null },
                        "readBy": { "cells": [], "conditions": [], "tests": ["may_merge"], "shown": false },
                    },
                    {
                        "name": "opinions",
                        "values": null,
                        "what": null,
                        "writtenBy": { "cells": ["review"], "people": true, "note": false, "outside": null },
                        "readBy": { "cells": ["plan"], "conditions": [], "tests": [], "shown": false },
                    },
                    {
                        "name": "plan",
                        "values": null,
                        "what": null,
                        "writtenBy": { "cells": ["plan"], "people": false, "note": false, "outside": null },
                        "readBy": { "cells": ["review"], "conditions": [], "tests": [], "shown": false },
                    },
                    {
                        "name": "request",
                        "values": null,
                        "what": null,
                        "writtenBy": { "cells": [], "people": true, "note": false, "outside": null },
                        "readBy": { "cells": ["plan"], "conditions": [], "tests": [], "shown": false },
                    },
                ],
            },
        })
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
    let redo = "title = \"Redo\"\nkind = \"engine\"\nact = \"check\"\nwhen = { block = \"content\", is = \"no\" }\n";
    source.cells.insert("redo".to_string(), redo.to_string());
    let router = serve(&Shared::new(Ok(source)), None);
    let answer = get_json(&router, "/api/v1/flow").await;
    assert_eq!(
        answer["flow"]["cells"][1]["condition"],
        json!({
            "on": "test",
            "test": {
                "name": "may_merge",
                "words": "The owner let the work merge",
                "kind": "all",
                "reads": ["go"],
                "is": "yes",
            },
        })
    );
    assert_eq!(
        answer["flow"]["cells"][2]["condition"],
        json!({ "on": "block", "block": "content", "value": "no" })
    );
    assert_eq!(
        answer["flow"]["blocks"][0]["readBy"],
        json!({ "cells": ["merge"], "conditions": ["redo"], "tests": [], "shown": true })
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
    let router = serve(&Shared::new(Ok(source)), None);
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
        "kind = \"agent\"\nrequires = [\"may_merge\"]",
    );
    let source = edit(
        source,
        Some("review"),
        "kind = \"agent\"",
        "kind = \"agent\"\nrequires = [\"may_merge\"]",
    );
    let router = serve(&Shared::new(Ok(source)), None);
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
                { "file": "plan.toml", "field": "requires[0]", "what": not_before },
                { "file": "review.toml", "field": "requires[0]", "what": own },
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
    let router = serve(&Shared::new(Ok(source)), None);
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
    let router = serve(&Shared::new(Ok(source)), None);
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
    let router = serve(&Shared::new(Ok(source)), None);
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
    let router = serve(&Shared::new(Ok(source)), None);
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
    let router = serve(&Shared::new(Ok(source)), None);
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
    let router = serve(&Shared::new(Ok(source)), None);
    let answer = get_json(&router, "/api/v1/flow").await;
    assert_eq!(
        answer["flow"]["tests"][0],
        json!({
            "name": "may_merge",
            "words": "The owner let the work merge",
            "kind": "noneStanding",
            "reads": ["go", "content"],
            "is": "no",
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
    let router = serve(&Shared::new(Ok(source)), None);
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
                    "field": "inputs[2]",
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
    let router = serve(&Shared::new(Ok(source)), None);
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
    let router = web::router(web::Flows::dirs(missing.clone(), None, Box::new(has)));
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
    let router = web::router(web::Flows::dirs(PathBuf::from(&dir), None, Box::new(has)));
    assert_eq!(get_json(&router, "/api/v1/flow").await["flow"]["name"], "tiny");
    write(&edit(tiny(), None, "name = \"tiny\"", "name = \"small\""));
    let answer = get_json(&router, "/api/v1/flow").await;
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(answer["flow"]["name"], "small");
}

#[tokio::test]
async fn what_the_reader_gives_is_read_again_for_each_request() {
    let shown = Shared::new(Ok(tiny()));
    let router = serve(&shown, None);
    assert_eq!(get_json(&router, "/api/v1/flow").await["state"], "compiled");
    shown.set(Err("gone".to_string()));
    assert_eq!(
        get_json(&router, "/api/v1/flow").await,
        json!({ "state": "unreadable", "reason": "gone" })
    );
}
