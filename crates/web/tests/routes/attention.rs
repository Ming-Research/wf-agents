//! `GET /api/v1/attention`: what needs whoever plans the work since a mark, answered at once or held open until there
//! is something or the time asked for has passed.

use axum::http::StatusCode;
use serde_json::{Value, json};

use crate::{START, get_json, greeted, pass, post, refused, running};

/// The trial item's line once its plan asked the owner its question, call 2, two and a half seconds in.
fn asked() -> Value {
    json!({
        "kind": "forAPerson",
        "of": { "work": 1, "item": 1, "title": "Greet the user" },
        "since": START + 2500,
        "now": {
            "kind": "waits",
            "at": "plan",
            "on": [{
                "kind": "call",
                "call": 2,
                "who": "owner",
                "what": { "kind": "question", "who": "owner", "question": "Should the greeting name the user?" },
            }],
        },
    })
}

#[tokio::test(start_paused = true)]
async fn a_question_for_the_owner_is_given_since_it_was_asked_and_not_again_from_the_mark_given_with_it() {
    let running = running();
    let router = &running.router;
    greeted(router).await;
    let answer = get_json(router, "/api/v1/attention").await;
    assert_eq!(answer["lines"], json!([asked()]));
    assert_eq!(answer["leftOut"], json!(0));
    let mark = answer["mark"].as_str().unwrap().to_string();
    let again = get_json(router, &format!("/api/v1/attention?since={mark}")).await;
    assert_eq!(again["lines"], json!([]));
    assert_eq!(
        again["mark"],
        json!(mark),
        "nothing happened, and the mark stays where it was"
    );
}

#[tokio::test(start_paused = true)]
async fn a_request_held_open_is_answered_as_soon_as_something_comes() {
    let running = running();
    let router = running.router.clone();
    let item = json!({
        "title": "Greet the user",
        "dependsOn": [],
        "flow": "trial",
        "writes": [{ "block": "request", "content": "Greet the user, please.", "value": null }],
    });
    assert_eq!(post(&router, "/api/v1/items", &item).await.0, StatusCode::OK);
    pass(1000).await;
    let resumed = post(&router, "/api/v1/works/1/acts", &json!({ "kind": "resume" })).await;
    assert_eq!(resumed.0, StatusCode::NO_CONTENT);
    let mark = get_json(&router, "/api/v1/attention").await["mark"]
        .as_str()
        .unwrap()
        .to_string();
    let held =
        tokio::spawn(async move { get_json(&router, &format!("/api/v1/attention?since={mark}&wait=600")).await });
    pass(1000).await;
    assert!(!held.is_finished(), "answered while the plan's round was under way");
    pass(1000).await;
    let answer = held.await.unwrap();
    assert_eq!(answer["lines"], json!([asked()]));
}

#[tokio::test(start_paused = true)]
async fn a_request_held_open_with_nothing_is_answered_with_no_line_once_its_time_has_passed() {
    let running = running();
    let router = running.router.clone();
    let held = tokio::spawn(async move { get_json(&router, "/api/v1/attention?wait=5").await });
    pass(4900).await;
    assert!(!held.is_finished(), "answered before its time");
    pass(200).await;
    assert_eq!(held.await.unwrap(), json!({ "lines": [], "leftOut": 0, "mark": "0" }));
}

#[tokio::test(start_paused = true)]
async fn a_mark_that_cannot_be_read_or_is_not_of_these_records_is_refused() {
    let running = running();
    let router = &running.router;
    greeted(router).await;
    let (status, words) = refused(router, "/api/v1/attention?since=nonsense").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{words}");
    let (status, words) = refused(router, "/api/v1/attention?since=7").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{words}");
    assert!(words.contains("work 7"), "{words}");
    let (status, words) = refused(router, "/api/v1/attention?after=1").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{words}");
}
