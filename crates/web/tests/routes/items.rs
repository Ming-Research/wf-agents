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
            "items": [{
                "number": 1,
                "title": "Greet the user",
                "level": "2",
                "added": START,
                "dependsOn": [],
                "work": summary,
            }],
        })
    );
}

#[tokio::test(start_paused = true)]
async fn an_item_is_added_held_with_its_work_paused_and_what_it_depends_on() {
    let running = running();
    let router = &running.router;
    let item = |title: &str, on: Value| {
        json!({
            "title": title,
            "dependsOn": on,
            "flow": "trial",
            "writes": [{ "block": "request", "content": "Please.", "value": null }],
            "held": true,
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
async fn an_item_is_added_at_the_level_it_asks_for_or_at_the_default_and_never_above_the_highest() {
    let running = running();
    let router = &running.router;
    let item =
        |level: Value| json!({ "title": "Greet", "level": level, "dependsOn": [], "flow": "trial", "writes": [] });
    assert_eq!(post(router, "/api/v1/items", &item(json!("1"))).await.0, StatusCode::OK);
    assert_eq!(
        post(router, "/api/v1/items", &item(Value::Null)).await.0,
        StatusCode::OK
    );
    let unprocessable = StatusCode::UNPROCESSABLE_ENTITY;
    let error = |words: &str| json!({ "error": words });
    assert_eq!(
        post(router, "/api/v1/items", &item(json!("3"))).await,
        (
            unprocessable,
            error("The item's level: 3 is above 2, the highest level an item may be at (items.highest_level).")
        )
    );
    assert_eq!(
        post(router, "/api/v1/items", &item(json!("9"))).await,
        (
            unprocessable,
            error("The item's level: 9 is no level: the levels are 1, 2, 3 (agents.levels).")
        )
    );
    let items = get_json(router, "/api/v1/items").await;
    let levels: Vec<&Value> = items["items"].as_array().unwrap().iter().map(|i| &i["level"]).collect();
    assert_eq!(levels, [&json!("1"), &json!("2")]);
    assert_eq!(get_json(router, "/api/v1/works/2").await["level"], json!("2"));
}

#[tokio::test(start_paused = true)]
async fn an_item_is_put_at_another_level_and_refused_as_an_added_one_is() {
    let running = running();
    let router = &running.router;
    let item = json!({ "title": "Greet", "dependsOn": [], "flow": "trial", "writes": [] });
    assert_eq!(post(router, "/api/v1/items", &item).await.0, StatusCode::OK);
    let set = |level: &str| json!({ "level": level });
    assert_eq!(
        post(router, "/api/v1/items/1/level", &set("1")).await,
        (StatusCode::OK, json!({ "changed": true }))
    );
    assert_eq!(
        post(router, "/api/v1/items/1/level", &set("1")).await,
        (StatusCode::OK, json!({ "changed": false }))
    );
    let unprocessable = StatusCode::UNPROCESSABLE_ENTITY;
    assert_eq!(
        post(router, "/api/v1/items/1/level", &set("3")).await,
        (
            unprocessable,
            json!({ "error": "The item's level: 3 is above 2, the highest level an item may be at (items.highest_level)." })
        )
    );
    assert_eq!(
        post(router, "/api/v1/items/7/level", &set("1")).await,
        (unprocessable, json!({ "error": "There is no item 7." }))
    );
    assert_eq!(get_json(router, "/api/v1/items").await["items"][0]["level"], json!("1"));
}

#[tokio::test(start_paused = true)]
async fn the_levels_are_given_with_what_each_is_for_the_default_and_the_highest() {
    let running = running();
    assert_eq!(
        get_json(&running.router, "/api/v1/levels").await,
        json!({
            "levels": [
                { "name": "1", "desc": "Easy work" },
                { "name": "2", "desc": "Ordinary work" },
                { "name": "3", "desc": "Hard work" },
            ],
            "default": "2",
            "highest": "2",
        })
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
