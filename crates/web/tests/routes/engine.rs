//! `GET /api/v1/engine` and `POST /api/v1/engine/acts`: the engine's slots and the owner's pause of everything, which
//! holds every work back and is lifted for all at once; and a work that waits for a slot says so.

use axum::http::StatusCode;
use serde_json::{Value, json};

use crate::{START, get_json, pass, post, post_text, running, running_scripted, trial_script};

/// An item of the trial flow, going on as it is added.
fn item(title: &str) -> Value {
    json!({
        "title": title,
        "dependsOn": [],
        "flow": "trial",
        "writes": [{ "block": "request", "content": "Please.", "value": null }],
    })
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
    assert_eq!(engine["taken"], 1, "a work added goes on by itself: {engine}");
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
    assert_eq!(post(router, "/api/v1/items", &item("First")).await.0, StatusCode::OK);
    assert_eq!(
        get_json(router, "/api/v1/works/1").await["summary"]["now"],
        json!({ "kind": "paused", "by": "everything", "why": null })
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
    assert_eq!(post(router, "/api/v1/items", &item("First")).await.0, StatusCode::OK);
    assert_eq!(post(router, "/api/v1/items", &item("Second")).await.0, StatusCode::OK);
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
