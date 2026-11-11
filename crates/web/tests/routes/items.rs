//! `GET /api/v1/items` and `POST /api/v1/items`: the items with their works, and adding one.

use axum::http::StatusCode;
use serde_json::{Value, json};

use crate::{START, get_json, greeted, post, post_text, running};

#[tokio::test(start_paused = true)]
async fn the_items_are_given_each_with_its_work_in_brief() {
    let running = running();
    greeted(&running.router).await;
    let summary = get_json(&running.router, "/api/v1/works/1").await["summary"].clone();
    assert_eq!(summary["now"]["kind"], "waits");
    assert_eq!(
        get_json(&running.router, "/api/v1/items").await,
        json!({
            "items": [{ "number": 1, "title": "Greet the user", "added": START, "dependsOn": [], "work": summary }],
        })
    );
}

#[tokio::test(start_paused = true)]
async fn an_item_is_added_with_its_work_paused_and_what_it_depends_on() {
    let running = running();
    let router = &running.router;
    let item = |title: &str, on: Value| {
        json!({
            "title": title,
            "dependsOn": on,
            "flow": "trial",
            "writes": [{ "block": "request", "content": "Please.", "value": null }],
        })
    };
    assert_eq!(
        post(router, "/api/v1/items", &item("First", json!([]))).await,
        (StatusCode::OK, json!({ "item": 1, "work": 1 }))
    );
    assert_eq!(
        post(router, "/api/v1/items", &item("Second", json!([1]))).await,
        (StatusCode::OK, json!({ "item": 2, "work": 2 }))
    );
    let items = get_json(router, "/api/v1/items").await;
    let second = &items["items"][1];
    assert_eq!(second["dependsOn"], json!([1]));
    assert_eq!(second["work"]["paused"], true);
    // A precondition holds the work before its cell whether or not it is paused.
    assert_eq!(
        second["work"]["now"],
        json!({
            "kind": "waits",
            "at": "plan",
            "on": [{
                "kind": "precondition",
                "test": "ready",
                "words": "Every item this one depends on has merged",
                "by": [{ "kind": "outside", "source": "dependencies" }],
            }],
        })
    );
    assert_eq!(
        items["items"][0]["work"]["now"],
        json!({ "kind": "paused", "by": "owner", "why": null })
    );
}

#[tokio::test(start_paused = true)]
async fn an_item_that_cannot_be_added_is_refused_with_why() {
    let running = running();
    let router = &running.router;
    let item = |flow: &str, on: Value, block: &str| {
        json!({
            "title": "Greet the user",
            "dependsOn": on,
            "flow": flow,
            "writes": [{ "block": block, "content": "Please.", "value": null }],
        })
    };
    let unprocessable = StatusCode::UNPROCESSABLE_ENTITY;
    let error = |words: &str| json!({ "error": words });
    assert_eq!(
        post(router, "/api/v1/items", &item("other", json!([]), "request")).await,
        (unprocessable, error("There is no flow named other."))
    );
    assert_eq!(
        post(router, "/api/v1/items", &item("trial", json!([5]), "request")).await,
        (unprocessable, error("There is no item 5."))
    );
    assert_eq!(
        post(router, "/api/v1/items", &item("trial", json!([]), "plan")).await,
        (unprocessable, error("People do not write the block plan."))
    );
    for body in [
        "not json",
        r#"{ "title": "Greet the user", "flow": "trial", "dependsOn": [], "writes": [], "paused": true }"#,
    ] {
        let (status, json) = post_text(router, "/api/v1/items", body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert!(
            json["error"].as_str().unwrap().starts_with("The item cannot be read: "),
            "{json}"
        );
    }
    assert_eq!(get_json(router, "/api/v1/items").await, json!({ "items": [] }));
}
