//! `GET /api/v1/works/{work}` and the content it leaves out: a version, a round, a call.

use axum::http::StatusCode;
use serde_json::{Value, json};

use crate::{START, get_json, greeted, pass, post, refused, running};

const WHY: &str = "The cell's run had 2 returns sent back, its limit.";

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

/// An agent's cell of the trial flow as a work's outline has it: given what it watches, each its last and its latest.
fn outlined(id: &str, title: &str, line: &str, watches: &[&str]) -> Value {
    let given: Vec<Value> = (watches.iter())
        .map(|block| json!({ "block": block, "kind": "lastAndLatest", "list": "watches" }))
        .collect();
    let watched: Vec<Value> = (watches.iter())
        .map(|block| json!({ "name": block, "blocks": [block], "do": null }))
        .collect();
    json!({
        "id": id, "title": title, "line": line, "agent": true, "given": given, "watches": watched, "reads": [],
        "words": null,
    })
}

/// The call the plan's run makes at its limit of returns.
fn at_limit() -> Value {
    json!({ "kind": "atLimit", "limit": { "of": "returns", "sentBack": 2 } })
}

#[tokio::test(start_paused = true)]
async fn a_work_is_given_as_it_is_now_and_step_by_step_with_the_flow_it_runs_on() {
    let running = running();
    greeted(&running.router).await;
    let (resumed, asked) = (START + 1000, START + 5500);
    let not_yet = |cell: &str| json!({ "cell": cell, "state": "notYet", "finished": 0, "record": null, "open": null });
    let input =
        |block: &str, current: Value| json!({ "block": block, "current": current, "known": null, "changed": false });
    // The request is new to the plan, whose run has dealt with nothing yet.
    let for_the_request = json!([{ "kind": "changed", "block": "request", "workedFrom": null, "behind": false }]);
    let flow = get_json(&running.router, "/api/v1/flow").await["flow"].clone();
    // Each of the plan's three rounds hands in nothing and asks for nothing, so that the next is given why it was sent
    // back.
    let rounds: Vec<Value> = (1..=3_u64)
        .map(|number| {
            let given = match number {
                1 => json!({ "kind": "task" }),
                _ => json!({ "kind": "sentBack", "unmet": [{ "key": "dealt_with_nothing", "values": [] }] }),
            };
            json!({
                "number": number,
                "began": resumed + 1500 * (number - 1),
                "ended": resumed + 1500 * number,
                "given": given,
                "back": { "kind": "handedIn", "blocks": [] },
                "spent": spent(1),
                "context": null,
                "compactions": 0,
                // The engineer's first session, which the records count as none; the script records no try.
                "session": { "line": "engineer", "number": 0 },
                "tries": [],
                "missed": null,
                "estimated": false,
                // The engine's loop runs with no policy, which a line is worked out by: nothing is judged.
                "judged": { "cost": null, "time": null },
            })
        })
        .collect();
    assert_eq!(
        get_json(&running.router, "/api/v1/works/1").await,
        json!({
            "summary": {
                "number": 1,
                "item": 1,
                "flow": "trial",
                "delivers": false,
                "added": START,
                "paused": null,
                "now": {
                    "kind": "waits",
                    "at": "plan",
                    "on": [{ "kind": "call", "call": 2, "who": "owner", "what": at_limit(), "made": asked, "at": null }],
                },
                "cells": [
                    { "cell": "plan", "state": "resting", "finished": 0, "record": null, "open": 1 },
                    not_yet("build"),
                    not_yet("review"),
                    not_yet("merge"),
                ],
                "spent": spent(3),
                // No ping of its sessions was sent.
                "pinged": { "usdMicros": 0, "inputTokens": 0, "cachedTokens": 0, "outputTokens": 0, "workingMs": 0, "rounds": 0 },
                // No fork of a round's session was asked.
                "forked": { "usdMicros": 0, "inputTokens": 0, "cachedTokens": 0, "outputTokens": 0, "workingMs": 0, "rounds": 0 },
                "last": asked,
                // Its first round began as it was resumed, and an agent has run the whole of its time since.
                "from": resumed,
                "bar": "m".repeat(96),
                // The plan's second and third rounds were given what the round before handed in, sent back.
                "sentBack": 2,
                "waits": [{ "key": "limit", "why": WHY, "ends": { "kind": "goOn", "call": 2 } }],
                // The call holds the work, and no question of its own stands to be answered ahead.
                "ahead": 0,
                // Its rounds run at its item's level, and the plan's line raises it no higher.
                "level": "2",
                "cellLevel": "2",
                "raise": 0,
                // The trial's host keeps no pull request, and its acts' carrier no repository to say what it changed.
                "pull": null,
                "changed": null,
            },
            "title": "Greet the user",
            "level": "2",
            "tags": [],
            "history": {
                "first": { "title": "Greet the user", "level": "2", "dependsOn": [], "tags": [] },
                "changes": [],
            },
            "project": "trial",
            // The version of the flow the work runs on, as its outline has it: what its history and its card are
            // drawn from.
            "outline": {
                "kept": true,
                "cells": [
                    outlined("plan", "Plan", "engineer", &["request"]),
                    outlined("build", "Build", "engineer", &["plan", "opinions"]),
                    outlined("review", "Review", "reviewer", &["code"]),
                    {
                        "id": "merge", "title": "Merge", "line": null, "agent": false, "given": [],
                        "watches": [{ "name": "code", "blocks": ["code"], "do": null }], "reads": [], "words": null,
                    },
                ],
                "lines": [{ "name": "engineer", "cells": ["plan", "build"] }, { "name": "reviewer", "cells": ["review"] }],
                "groups": [],
                "card": [],
            },
            "flow": flow,
            "cells": [
                {
                    "cell": "plan",
                    "needs": for_the_request,
                    // Each version by its number among its block's versions: the request's first.
                    "inputs": [json!({ "block": "request", "current": 1, "known": null, "changed": true })],
                },
                {
                    "cell": "build",
                    "needs": [],
                    "inputs": [input("plan", Value::Null), input("opinions", Value::Null)],
                },
                {
                    "cell": "review",
                    "needs": [],
                    "inputs": [input("code", Value::Null)],
                },
                { "cell": "merge", "needs": [], "inputs": [input("code", Value::Null)] },
            ],
            "blocks": [
                {
                    "name": "dependencies",
                    "versions": [{
                        "at": 2,
                        "number": 1,
                        "time": START,
                        "by": { "kind": "outside" },
                        "meaning": 1,
                        "value": "merged",
                        "saidTo": null,
                        "chars": 42,
                        "empty": false,
                    }],
                },
                {
                    "name": "request",
                    "versions": [{
                        "at": 3,
                        "number": 1,
                        "time": START,
                        "by": { "kind": "owner" },
                        "meaning": 1,
                        "value": null,
                        "saidTo": null,
                        "chars": 23,
                        "empty": false,
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
                    "by": { "kind": "owner" },
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
                    "basis": [
                        { "block": "plan", "version": null },
                        { "block": "request", "version": 1 },
                    ],
                    "rounds": rounds,
                    // A call at a limit is no call of a tool, whose time and the size of its result are judged.
                    "calls": [{ "call": 2, "what": at_limit(), "made": asked, "done": null, "chars": null, "time": null, "size": null }],
                    // The run is open, at the owner's call.
                    "end": null,
                    "spent": spent(3),
                },
            ],
            // No question of the work's own is asked.
            "questions": [],
            // No cell of the trial flow has words of its own.
            "speaking": [
                {
                    "to": { "kind": "line", "line": "engineer" },
                    "cell": null,
                    "refused": "no cell of the line engineer has words of its own",
                },
                {
                    "to": { "kind": "line", "line": "reviewer" },
                    "cell": null,
                    "refused": "no cell of the line reviewer has words of its own",
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
        get_json(router, "/api/v1/works/1/versions/request/1").await,
        json!({ "block": "request", "number": 1, "content": "Greet the user, please." })
    );
    assert_eq!(
        get_json(router, "/api/v1/works/1/runs/1/rounds/1").await,
        json!({ "run": 1, "number": 1, "results": [], "handed": [] })
    );
    assert_eq!(
        get_json(router, "/api/v1/works/1/calls/2").await,
        json!({ "call": 2, "what": at_limit(), "result": null })
    );

    let answer = json!({ "kind": "result", "call": 2, "result": "Yes, by name." });
    assert_eq!(
        post(router, "/api/v1/works/1/acts", &answer).await.0,
        StatusCode::NO_CONTENT
    );
    let answered = json!({ "call": 2, "what": at_limit(), "result": "Yes, by name." });
    assert_eq!(get_json(router, "/api/v1/works/1/calls/2").await, answered);
    // The plan's run goes on, its fourth round given why the third was sent back, and hands in the plan a second and a
    // half later.
    pass(1600).await;
    assert_eq!(
        get_json(router, "/api/v1/works/1/runs/1/rounds/4").await,
        json!({
            "run": 1,
            "number": 4,
            "results": [],
            "handed": [{ "block": "plan", "content": "The plan as plan wrote it in its run 1, round 4." }],
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
        refused(router, "/api/v1/works/1/versions/request/2").await,
        (none, "Work 1 has no version 2 of request.".to_string())
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

// A version says whether it holds nothing as the engine recorded it, and the route works out nothing of its own: words
// of only whitespace hold nothing though their length is not nought, and the owner's request holds what it says. Well
// under a second alone, the engine's loop in memory; no other test holds the route to what the engine recorded.
#[tokio::test(start_paused = true)]
async fn a_version_says_whether_it_holds_nothing_as_the_engine_recorded_it() {
    let running = running();
    let router = &running.router;
    greeted(router).await;
    let blank = json!({ "kind": "write", "block": "opinions", "content": " \n  ", "value": null });
    assert_eq!(
        post(router, "/api/v1/works/1/acts", &blank).await,
        (StatusCode::NO_CONTENT, Value::Null)
    );
    let page = get_json(router, "/api/v1/works/1").await;
    let first = |name: &str| {
        let blocks = page["blocks"].as_array().unwrap();
        let block = blocks.iter().find(|b| b["name"] == name).unwrap();
        let version = &block["versions"][0];
        (version["chars"].clone(), version["empty"].clone())
    };
    assert_eq!(first("opinions"), (json!(4), json!(true)));
    assert_eq!(first("request"), (json!(23), json!(false)));
}
