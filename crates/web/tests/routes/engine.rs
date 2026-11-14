//! `GET /api/v1/engine` and `POST /api/v1/engine/acts`: the engine's slots and the pause of everything, the owner's or
//! the engine's own as it starts with works going on, which holds every work back and is lifted for all at once; and a
//! work that waits for a slot says so.

use axum::http::StatusCode;
use serde_json::{Value, json};

use crate::{START, get_json, pass, post, post_text, running, running_scripted, trial_script};

/// An item of the trial flow.
fn item(title: &str) -> Value {
    json!({
        "title": title,
        "dependsOn": [],
        "project": "trial",
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
    let paused = get_json(router, "/api/v1/engine").await["paused"].clone();
    assert!(paused["since"].as_u64() >= Some(START), "{paused}");
    assert_eq!(paused["by"], "owner");
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

#[tokio::test(start_paused = true)]
async fn started_on_a_work_going_on_the_engine_says_it_holds_everything_until_the_owner_lifts_it() {
    // A store with a work let on, its round under way when the engine before stopped.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.sqlite");
    {
        let flow = flow::compile(
            &flow::read(&crate::trial_dir().join("flow")).unwrap(),
            &crate::trial_has(&flow::Source::default()),
        )
        .unwrap();
        let store = store::Store::open(&path).unwrap();
        let (mut driver, _) = driver::Driver::open(Box::new(store), vec![flow], 2, None, work::Time(START)).unwrap();
        let new = driver::NewItem {
            title: "First".to_string(),
            level: "2".to_string(),
            project: "trial".to_string(),
            depends_on: Vec::new(),
            tags: Vec::new(),
            flow: "trial".to_string(),
            writes: vec![driver::FirstWrite {
                block: decide::BlockId("request".to_string()),
                content: "Please.".to_string(),
                value: None,
            }],
        };
        let (_, work, _) = driver.add(work::Time(START), new).unwrap();
        driver
            .happened(work::Time(START), work, engine::Event::Resumed)
            .unwrap();
    }
    let running = crate::running_on(Box::new(store::Store::open(&path).unwrap()), dir);
    let router = &running.router;
    assert_eq!(
        get_json(router, "/api/v1/engine").await["paused"],
        json!({ "since": START, "by": "engine" })
    );
    let attention = get_json(router, "/api/v1/attention").await;
    assert_eq!(
        attention["lines"][0],
        json!({ "kind": "everything", "since": START, "by": "engine" }),
        "{attention}"
    );
    assert_eq!(
        post(router, "/api/v1/engine/acts", &json!({ "kind": "resume" })).await,
        (StatusCode::NO_CONTENT, Value::Null)
    );
    assert_eq!(get_json(router, "/api/v1/engine").await["paused"], Value::Null);
}
