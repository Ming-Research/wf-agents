//! `GET /api/v1/works/{work}` and the content it leaves out: a version, a round, a call.

use axum::http::StatusCode;
use serde_json::{Value, json};

use crate::{START, get_json, greeted, pass, post, refused, running};

const QUESTION: &str = "Should the greeting name the user?";

fn spent(rounds: u64) -> Value {
    json!({
        "usdMicros": 21000 * rounds,
        "inputTokens": 3200 * rounds,
        "cachedTokens": 14000 * rounds,
        "outputTokens": 450 * rounds,
        "workingMs": 1500 * rounds,
        "rounds": rounds,
    })
}

fn question() -> Value {
    json!({ "kind": "question", "who": "owner", "question": QUESTION })
}

#[tokio::test(start_paused = true)]
async fn a_work_is_given_as_it_is_now_and_step_by_step_with_the_flow_it_runs_on() {
    let running = running();
    greeted(&running.router).await;
    let (resumed, asked) = (START + 1000, START + 2500);
    let not_yet = |cell: &str| json!({ "cell": cell, "state": "notYet", "finished": 0, "record": null, "open": null });
    let input =
        |block: &str, current: Value| json!({ "block": block, "current": current, "known": null, "changed": false });
    // The request is new to the plan, whose run that asked dealt with nothing.
    let for_the_request = json!([{ "kind": "changed", "block": "request", "was": null, "behind": false }]);
    let flow = get_json(&running.router, "/api/v1/flow").await["flow"].clone();
    assert_eq!(
        get_json(&running.router, "/api/v1/works/1").await,
        json!({
            "summary": {
                "number": 1,
                "item": 1,
                "flow": "trial",
                "added": START,
                "paused": false,
                "now": {
                    "kind": "waits",
                    "at": "plan",
                    "on": [{ "kind": "call", "call": 2, "who": "owner", "what": question() }],
                },
                "cells": [
                    { "cell": "plan", "state": "again", "finished": 1, "record": 1, "open": null },
                    not_yet("build"),
                    not_yet("review"),
                    not_yet("merge"),
                ],
                "spent": spent(1),
                "last": asked,
                // Its first round began as it was resumed, and an agent has run the whole of its time since.
                "from": resumed,
                "bar": "m".repeat(96),
                "sentBack": 0,
            },
            "title": "Greet the user",
            "level": "2",
            "history": {
                "first": { "title": "Greet the user", "level": "2", "dependsOn": [] },
                "changes": [],
            },
            "flow": flow,
            "cells": [
                {
                    "cell": "plan",
                    "needs": for_the_request,
                    "inputs": [{ "block": "request", "current": 3, "known": null, "changed": true }],
                },
                {
                    "cell": "build",
                    "needs": [],
                    "inputs": [input("plan", Value::Null), input("opinions", Value::Null)],
                },
                { "cell": "review", "needs": [], "inputs": [input("code", Value::Null)] },
                { "cell": "merge", "needs": [], "inputs": [input("code", Value::Null)] },
            ],
            "blocks": [
                {
                    "name": "dependencies",
                    "versions": [{
                        "at": 2,
                        "time": START,
                        "by": { "kind": "outside" },
                        "meaning": 2,
                        "value": "merged",
                        "chars": 42,
                    }],
                },
                {
                    "name": "request",
                    "versions": [{
                        "at": 3,
                        "time": START,
                        "by": { "kind": "person" },
                        "meaning": 3,
                        "value": null,
                        "chars": 23,
                    }],
                },
            ],
            "steps": [
                { "kind": "paused", "at": 1, "time": START, "by": "owner", "why": null },
                {
                    "kind": "wrote",
                    "at": 2,
                    "time": START,
                    "block": "dependencies",
                    "by": { "kind": "outside" },
                    "changed": true,
                    "value": "merged",
                    "chars": 42,
                },
                {
                    "kind": "wrote",
                    "at": 3,
                    "time": START,
                    "block": "request",
                    "by": { "kind": "person" },
                    "changed": true,
                    "value": null,
                    "chars": 23,
                },
                { "kind": "resumed", "at": 4, "time": resumed },
                {
                    "kind": "run",
                    "run": 1,
                    "cell": "plan",
                    "at": 5,
                    "time": resumed,
                    "because": for_the_request,
                    "basis": [{ "block": "plan", "version": null }, { "block": "request", "version": 3 }],
                    "rounds": [{
                        "number": 1,
                        "began": resumed,
                        "ended": asked,
                        "given": { "kind": "task" },
                        "back": { "kind": "asked", "asks": [question()] },
                        "spent": spent(1),
                        "context": null,
                        "compactions": 0,
                    }],
                    "calls": [{ "call": 2, "what": question(), "made": asked, "done": null, "chars": null }],
                    // A return that asks ends its run, having dealt with nothing.
                    "end": { "kind": "finished", "at": 9, "time": asked, "writes": [] },
                    "spent": spent(1),
                },
            ],
        })
    );
}

#[tokio::test(start_paused = true)]
async fn a_version_a_round_and_a_call_are_given_with_their_content() {
    let running = running();
    let router = &running.router;
    greeted(router).await;
    assert_eq!(
        get_json(router, "/api/v1/works/1/versions/request/3").await,
        json!({ "block": "request", "at": 3, "content": "Greet the user, please." })
    );
    assert_eq!(
        get_json(router, "/api/v1/works/1/runs/1/rounds/1").await,
        json!({ "run": 1, "number": 1, "results": [], "handed": [] })
    );
    assert_eq!(
        get_json(router, "/api/v1/works/1/calls/2").await,
        json!({ "call": 2, "what": question(), "result": null })
    );

    let answer = json!({ "kind": "answer", "call": 2, "result": "Yes, by name." });
    assert_eq!(
        post(router, "/api/v1/works/1/acts", &answer).await.0,
        StatusCode::NO_CONTENT
    );
    let answered = json!({ "call": 2, "what": question(), "result": "Yes, by name." });
    assert_eq!(get_json(router, "/api/v1/works/1/calls/2").await, answered);
    // The plan, entered again as its second run, is given the answer, and hands in the plan a second and a half later.
    pass(1600).await;
    assert_eq!(
        get_json(router, "/api/v1/works/1/runs/3/rounds/1").await,
        json!({
            "run": 3,
            "number": 1,
            "results": [answered],
            "handed": [{ "block": "plan", "content": "The plan as plan wrote it in its run 2, round 1." }],
        })
    );
}

#[tokio::test(start_paused = true)]
async fn what_there_is_none_of_is_refused_with_why() {
    let running = running();
    let router = &running.router;
    greeted(router).await;
    let none = StatusCode::NOT_FOUND;
    assert_eq!(
        refused(router, "/api/v1/works/9").await,
        (none, "There is no work 9.".to_string())
    );
    assert_eq!(
        refused(router, "/api/v1/works/9/calls/2").await,
        (none, "There is no work 9.".to_string())
    );
    assert_eq!(
        refused(router, "/api/v1/works/1/versions/request/4").await,
        (none, "Work 1 has no version of request at 4.".to_string())
    );
    assert_eq!(
        refused(router, "/api/v1/works/1/runs/1/rounds/7").await,
        (none, "Work 1 has no round 7 of run 1.".to_string())
    );
    assert_eq!(
        refused(router, "/api/v1/works/1/calls/3").await,
        (none, "Work 1 has no call 3.".to_string())
    );
    let (status, words) = refused(router, "/api/v1/works/one").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(words.starts_with("The path cannot be read: "), "{words}");
}
