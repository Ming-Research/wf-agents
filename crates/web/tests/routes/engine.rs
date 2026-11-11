//! `GET /api/v1/engine` and `POST /api/v1/engine/acts`: the engine's slots and the owner's pause of everything, which
//! holds every work back and is lifted for all at once; and a work that waits for a slot says so.

use axum::http::StatusCode;
use serde_json::{Value, json};

use crate::{START, get_json, pass, post, post_text, running, running_scripted, trial_script};

/// An item of the trial flow.
fn item(title: &str) -> Value {
    json!({
        "title": title,
        "dependsOn": [],
        "flow": "trial",
        "writes": [{ "block": "request", "content": "Please.", "value": null }],
    })
}

/// Adds an item of the trial flow, its work being the next by number, and has the owner let its work on.
async fn add_let_on(router: &axum::Router, title: &str, work: u64) {
    assert_eq!(post(router, "/api/v1/items", &item(title)).await.0, StatusCode::OK);
    assert_eq!(
        post(
            router,
            &format!("/api/v1/works/{work}/acts"),
            &json!({ "kind": "resume" })
        )
        .await,
        (StatusCode::NO_CONTENT, Value::Null)
    );
}

#[tokio::test(start_paused = true)]
async fn the_engine_says_its_slots_and_whether_everything_is_paused() {
    let running = running();
    let router = &running.router;
    assert_eq!(
        get_json(router, "/api/v1/engine").await,
        json!({ "slots": 2, "taken": 0, "paused": null })
    );
    assert_eq!(post(router, "/api/v1/items", &item("First")).await.0, StatusCode::OK);
    let engine = get_json(router, "/api/v1/engine").await;
    assert_eq!(engine["taken"], 0, "a work added paused takes no slot: {engine}");
    assert_eq!(
        post(router, "/api/v1/works/1/acts", &json!({ "kind": "resume" })).await,
        (StatusCode::NO_CONTENT, Value::Null)
    );
    let engine = get_json(router, "/api/v1/engine").await;
    assert_eq!(engine["taken"], 1, "a work let on goes on: {engine}");
    assert_eq!(
        get_json(router, "/api/v1/works/1").await["summary"]["now"]["kind"],
        "round"
    );
}

#[tokio::test(start_paused = true)]
async fn the_pause_of_everything_holds_back_a_work_added_and_lifted_lets_it_on() {
    let running = running();
    let router = &running.router;
    assert_eq!(
        post(router, "/api/v1/engine/acts", &json!({ "kind": "pause" })).await,
        (StatusCode::NO_CONTENT, Value::Null)
    );
    assert!(get_json(router, "/api/v1/engine").await["paused"].as_u64() >= Some(START));
    add_let_on(router, "First", 1).await;
    assert_eq!(
        get_json(router, "/api/v1/works/1").await["summary"]["now"],
        json!({ "kind": "everything" })
    );
    pass(1000).await;
    assert_eq!(
        post(router, "/api/v1/engine/acts", &json!({ "kind": "resume" })).await,
        (StatusCode::NO_CONTENT, Value::Null)
    );
    assert_eq!(get_json(router, "/api/v1/engine").await["paused"], Value::Null);
    assert_eq!(
        get_json(router, "/api/v1/works/1").await["summary"]["now"]["kind"],
        "round"
    );
}

#[tokio::test(start_paused = true)]
async fn a_work_that_waits_for_a_slot_says_so() {
    let dir = tempfile::tempdir().unwrap();
    let store = store::Store::open(dir.path().join("store.sqlite")).unwrap();
    let mut script = trial_script();
    script.slots = Some(1);
    let running = running_scripted(Box::new(store), dir, script);
    let router = &running.router;
    add_let_on(router, "First", 1).await;
    add_let_on(router, "Second", 2).await;
    assert_eq!(
        get_json(router, "/api/v1/works/2").await["summary"]["now"],
        json!({ "kind": "waits", "at": "plan", "on": [{ "kind": "slot" }] })
    );
}

#[tokio::test(start_paused = true)]
async fn an_act_on_the_engine_that_cannot_be_read_is_refused() {
    let running = running();
    let (status, json) = post_text(&running.router, "/api/v1/engine/acts", r#"{ "kind": "stop" }"#).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        json["error"].as_str().unwrap().starts_with("The act cannot be read: "),
        "{json}"
    );
}
