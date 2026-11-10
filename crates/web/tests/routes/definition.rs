//! `GET /api/v1/flow`: the definition shown, compiled or with its faults.

use std::path::PathBuf;

use serde_json::json;

use crate::{Shared, edit, get_json, serve, tiny};

#[tokio::test]
async fn a_definition_that_compiles_is_given_whole_as_the_page_reads_it() {
    let router = serve(&Shared::new(Ok(tiny())), None);
    let limits = |runs| json!({ "runs": runs, "returns": 2, "calls": 40 });
    assert_eq!(
        get_json(&router, "/api/v1/flow").await,
        json!({
            "state": "compiled",
            "flow": {
                "name": "tiny",
                "limits": limits(3),
                "lines": [{ "name": "engineer", "cells": ["plan", "review"] }],
                "tests": [{ "name": "may_merge", "words": "The owner let the work merge" }],
                "cells": [
                    {
                        "id": "plan",
                        "title": "Plan",
                        "kind": "agent",
                        "line": "engineer",
                        "condition": null,
                        "preconditions": [],
                        "inputs": ["request", "opinions"],
                        "outputs": ["plan"],
                        "limits": limits(3),
                    },
                    {
                        "id": "review",
                        "title": "Review",
                        "kind": "agent",
                        "line": "engineer",
                        "condition": null,
                        "preconditions": [],
                        "inputs": ["plan"],
                        "outputs": ["opinions", "content"],
                        "limits": limits(5),
                    },
                    {
                        "id": "merge",
                        "title": "Merge",
                        "kind": "engine",
                        "line": null,
                        "condition": null,
                        "preconditions": [{ "name": "may_merge", "words": "The owner let the work merge" }],
                        "inputs": ["content"],
                        "outputs": [],
                        "limits": limits(3),
                    },
                ],
                "blocks": [
                    {
                        "name": "content",
                        "values": ["yes", "no"],
                        "writtenBy": { "cells": ["review"], "people": false, "outside": false },
                        "readBy": { "cells": ["merge"], "conditions": [], "shown": false },
                    },
                    {
                        "name": "opinions",
                        "values": null,
                        "writtenBy": { "cells": ["review"], "people": true, "outside": false },
                        "readBy": { "cells": ["plan"], "conditions": [], "shown": false },
                    },
                    {
                        "name": "plan",
                        "values": null,
                        "writtenBy": { "cells": ["plan"], "people": false, "outside": false },
                        "readBy": { "cells": ["review"], "conditions": [], "shown": false },
                    },
                    {
                        "name": "request",
                        "values": null,
                        "writtenBy": { "cells": [], "people": true, "outside": false },
                        "readBy": { "cells": ["plan"], "conditions": [], "shown": false },
                    },
                ],
            },
        })
    );
}

#[tokio::test]
async fn a_condition_is_given_with_its_block_and_value_or_with_its_test_and_the_test_s_words() {
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
    let redo = "title = \"Redo\"\nkind = \"engine\"\nwhen = { block = \"content\", is = \"no\" }\n";
    source.cells.insert("redo".to_string(), redo.to_string());
    let router = serve(&Shared::new(Ok(source)), None);
    let answer = get_json(&router, "/api/v1/flow").await;
    assert_eq!(
        answer["flow"]["cells"][1]["condition"],
        json!({ "on": "test", "test": "may_merge", "words": "The owner let the work merge" })
    );
    assert_eq!(
        answer["flow"]["cells"][2]["condition"],
        json!({ "on": "block", "block": "content", "value": "no" })
    );
    assert_eq!(
        answer["flow"]["blocks"][0]["readBy"],
        json!({ "cells": ["merge"], "conditions": ["redo"], "shown": true })
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
    let router = web::router(web::Flows::dirs(
        missing.clone(),
        None,
        Box::new(|_: &flow::Source| Default::default()),
    ));
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
    let router = web::router(web::Flows::dirs(
        PathBuf::from(&dir),
        None,
        Box::new(|_: &flow::Source| ["may_merge".into()].into()),
    ));
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
