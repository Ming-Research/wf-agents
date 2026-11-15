//! `POST /api/v1/works/{work}/acts`: what the owner does to a work, each as the engine takes it, or refused with why.

use axum::http::StatusCode;
use serde_json::{Value, json};

use crate::{NAMED, get_json, greeted, named, pass, post, post_text, running};

const ACTS: &str = "/api/v1/works/1/acts";

#[tokio::test(start_paused = true)]
async fn each_act_is_taken_as_what_happened_to_the_work() {
    let running = running();
    let router = &running.router;
    greeted(router).await;
    let done = (StatusCode::NO_CONTENT, Value::Null);
    assert_eq!(post(router, ACTS, &json!({ "kind": "pause" })).await, done);
    assert_eq!(get_json(router, "/api/v1/works/1").await["summary"]["paused"], "owner");
    assert_eq!(post(router, ACTS, &json!({ "kind": "resume" })).await, done);
    let opinion = json!({ "kind": "write", "block": "opinions", "content": "Keep it short.", "value": null });
    assert_eq!(post(router, ACTS, &opinion).await, done);
    assert_eq!(
        post(router, ACTS, &json!({ "kind": "letGo", "cell": "build" })).await,
        done
    );
    let page = get_json(router, "/api/v1/works/1").await;
    let kinds: Vec<&str> = (page["steps"].as_array().unwrap().iter())
        .map(|s| s["kind"].as_str().unwrap())
        .collect();
    assert_eq!(
        kinds,
        [
            "paused", "wrote", "wrote", "resumed", "run", "paused", "resumed", "wrote", "letGo"
        ]
    );
    let opinions = &page["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["name"] == "opinions")
        .unwrap()["versions"];
    assert_eq!(opinions[0]["by"], json!({ "kind": "person" }));

    assert_eq!(post(router, ACTS, &json!({ "kind": "cancel" })).await, done);
    assert_eq!(
        get_json(router, "/api/v1/works/1").await["summary"]["now"],
        json!({ "kind": "cancelled" })
    );
}

#[tokio::test(start_paused = true)]
async fn an_act_that_cannot_be_taken_is_refused_with_why() {
    let running = running();
    let router = &running.router;
    greeted(router).await;
    let error = |words: &str| json!({ "error": words });
    let unprocessable = StatusCode::UNPROCESSABLE_ENTITY;
    let answer = |call: u64| json!({ "kind": "result", "call": call, "result": "Yes." });
    assert_eq!(
        post(router, "/api/v1/works/9/acts", &json!({ "kind": "pause" })).await,
        (StatusCode::NOT_FOUND, error("There is no work 9."))
    );
    assert_eq!(
        post(router, ACTS, &answer(9)).await,
        (unprocessable, error("The work has no call 9."))
    );
    let write = json!({ "kind": "write", "block": "code", "content": "fn main() {}", "value": null });
    assert_eq!(
        post(router, ACTS, &write).await,
        (unprocessable, error("People do not write the block code."))
    );
    assert_eq!(
        post(router, ACTS, &json!({ "kind": "letGo", "cell": "deploy" })).await,
        (unprocessable, error("The flow has no cell deploy."))
    );
    for act in [
        r#"{ "kind": "jump" }"#,
        r#"{ "kind": "pause", "at": 3 }"#,
        r#"{ "kind": "write", "block": "opinions", "content": "Short.", "value": null, "for": "build" }"#,
    ] {
        let (status, json) = post_text(router, ACTS, act).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{act}");
        assert!(
            json["error"].as_str().unwrap().starts_with("The act cannot be read: "),
            "{json}"
        );
    }
    let (status, json) = post_text(router, ACTS, r#"{ "kind": "jump" }"#).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        json["error"].as_str().unwrap().starts_with("The act cannot be read: "),
        "{json}"
    );

    // The question answered by an option it does not have, or a version that is not one of it, is refused; answered,
    // the plan hands in and the build asks for a query: a tool's call, which no person answers.
    let by = |version: u64, option: u64| json!({ "kind": "answer", "question": "named", "version": version, "option": option });
    assert_eq!(
        post(router, ACTS, &by(NAMED, 3)).await,
        (
            unprocessable,
            error("The version of the question named answered has no option 3.")
        )
    );
    assert_eq!(
        post(router, ACTS, &by(NAMED - 1, 1)).await,
        (
            unprocessable,
            error(&format!(
                "The question named has no version {} that asks it.",
                NAMED - 1
            ))
        )
    );
    assert_eq!(post(router, ACTS, &named()).await.0, StatusCode::NO_CONTENT);
    pass(4000).await;
    assert_eq!(
        post(router, ACTS, &answer(4)).await,
        (
            unprocessable,
            error("Call 4 is a tool's: the tool gives its result, not a person.")
        )
    );
}
