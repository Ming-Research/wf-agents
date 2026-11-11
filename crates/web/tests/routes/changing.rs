//! `POST /api/v1/items/{item}/title` and `POST /api/v1/items/{item}/dependencies`: whoever plans the work changes an
//! item after it was added, and the item's history says what it was before and when it changed; a cancel says why.

use axum::http::StatusCode;
use serde_json::{Value, json};

use crate::{get_json, greeted, pass, post, running};

/// An item of the trial flow, held, depending on these.
fn item(title: &str, on: Value) -> Value {
    json!({
        "title": title,
        "dependsOn": on,
        "flow": "trial",
        "writes": [{ "block": "request", "content": "Please.", "value": null }],
    })
}

#[tokio::test(start_paused = true)]
async fn an_item_not_begun_is_retitled_and_comes_to_depend_on_another_and_its_history_says_so() {
    let running = running();
    let router = &running.router;
    post(router, "/api/v1/items", &item("First", json!([]))).await;
    post(router, "/api/v1/items", &item("Second", json!([]))).await;
    let changed = |changed: bool| (StatusCode::OK, json!({ "changed": changed }));
    let title = json!({ "title": "Second, narrowed" });
    assert_eq!(post(router, "/api/v1/items/2/title", &title).await, changed(true));
    assert_eq!(post(router, "/api/v1/items/2/title", &title).await, changed(false));
    let add = json!({ "kind": "add", "item": 1 });
    assert_eq!(post(router, "/api/v1/items/2/dependencies", &add).await, changed(true));
    assert_eq!(post(router, "/api/v1/items/2/dependencies", &add).await, changed(false));
    let items = get_json(router, "/api/v1/items").await;
    let second = &items["items"][1];
    assert_eq!(second["title"], "Second, narrowed");
    assert_eq!(second["dependsOn"], json!([1]));
    let changes = &second["history"]["changes"];
    assert_eq!(
        second["history"]["first"],
        json!({ "title": "Second", "level": "2", "dependsOn": [] })
    );
    assert_eq!(changes[0]["kind"], "title");
    assert_eq!(changes[0]["title"], "Second, narrowed");
    assert_eq!(changes[1]["kind"], "dependsOn");
    assert_eq!(changes[1]["item"], 1);
    assert!(changes[0]["time"].as_u64() <= changes[1]["time"].as_u64());
    // The work's page gives the same history.
    assert_eq!(get_json(router, "/api/v1/works/2").await["history"], second["history"]);
    let remove = json!({ "kind": "remove", "item": 1 });
    assert_eq!(
        post(router, "/api/v1/items/2/dependencies", &remove).await,
        changed(true)
    );
    let items = get_json(router, "/api/v1/items").await;
    assert_eq!(items["items"][1]["dependsOn"], json!([]));
    assert_eq!(items["items"][1]["history"]["changes"][2]["kind"], "noLongerDependsOn");
}

#[tokio::test(start_paused = true)]
async fn a_change_the_engine_refuses_says_why() {
    let running = running();
    let router = &running.router;
    greeted(router).await;
    post(router, "/api/v1/items", &item("Second", json!([1]))).await;
    let refused = |words: &str| (StatusCode::UNPROCESSABLE_ENTITY, json!({ "error": words }));
    assert_eq!(
        post(
            router,
            "/api/v1/items/1/dependencies",
            &json!({ "kind": "add", "item": 2 })
        )
        .await,
        refused(
            "The work of item 1 has begun, and what an item depends on changes only until its work begins: what it \
             depends on is what its work began on. What turns up for it is a new item, which may depend on it."
        )
    );
    assert_eq!(
        post(
            router,
            "/api/v1/items/2/dependencies",
            &json!({ "kind": "add", "item": 2 })
        )
        .await,
        refused("Item 2 cannot depend on itself.")
    );
    assert_eq!(
        post(
            router,
            "/api/v1/items/2/dependencies",
            &json!({ "kind": "remove", "item": 3 })
        )
        .await,
        refused("There is no item 3.")
    );
    assert_eq!(
        post(router, "/api/v1/items/3/title", &json!({ "title": "x" })).await,
        refused("There is no item 3.")
    );
    assert_eq!(
        post(
            router,
            "/api/v1/items/2/dependencies",
            &json!({ "kind": "join", "item": 1 })
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    // A begun item is still given another title.
    assert_eq!(
        post(
            router,
            "/api/v1/items/1/title",
            &json!({ "title": "Greet the user by name" })
        )
        .await,
        (StatusCode::OK, json!({ "changed": true }))
    );
}

#[tokio::test(start_paused = true)]
async fn a_cancel_says_why_and_its_item_then_takes_no_change() {
    let running = running();
    let router = &running.router;
    post(router, "/api/v1/items", &item("First", json!([]))).await;
    post(router, "/api/v1/items", &item("Second", json!([]))).await;
    let acts = "/api/v1/works/2/acts";
    assert_eq!(
        post(router, acts, &json!({ "kind": "cancel", "why": "  " })).await,
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            json!({ "error": "A cancel's reason says nothing: give one that says why, or none." })
        )
    );
    let cancel = json!({ "kind": "cancel", "why": "Joined into item 1." });
    assert_eq!(post(router, acts, &cancel).await, (StatusCode::NO_CONTENT, Value::Null));
    pass(100).await;
    let page = get_json(router, "/api/v1/works/2").await;
    let steps = page["steps"].as_array().unwrap();
    let cancelled = steps.iter().find(|s| s["kind"] == "cancelled").expect("a cancel");
    assert_eq!(cancelled["why"], "Joined into item 1.");
    assert_eq!(
        post(router, "/api/v1/items/2/title", &json!({ "title": "Other" })).await,
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            json!({ "error": "Item 2 is cancelled: an item that has ended takes no change, and is what it ended as." })
        )
    );
}
