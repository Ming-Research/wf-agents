//! `GET /api/v1/observations` and `POST /api/v1/observations/{number}/read`: the observations of the returns the
//! engine took, the newest first, each unread until the owner reads it.

use axum::http::StatusCode;
use serde_json::{Value, json};
use store::Store;

use crate::{Running, START, get_json, pass, post, running_scripted};

/// The trial flow's script with the plan's first round handing in its plan and saying two observations.
const SCRIPT: &str = r#"
[[flows.trial.rounds]]
cell = "plan"
hands_in = [{ block = "plan" }]
observations = ["The greeting has no test.", "The request names no language."]
"#;

/// The engine's loop over the trial flow with that script, and the trial item added and let on at a second: by 1.2
/// seconds its plan's return has been taken.
async fn planned() -> Running {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("store.sqlite")).unwrap();
    let running = running_scripted(Box::new(store), dir, service::Script::read(SCRIPT).unwrap());
    let item = json!({
        "title": "Greet the user",
        "dependsOn": [],
        "project": "trial",
        "flow": "trial",
        "writes": [{ "block": "request", "content": "Greet the user, please.", "value": null }],
    });
    assert_eq!(post(&running.router, "/api/v1/items", &item).await.0, StatusCode::OK);
    pass(1000).await;
    let resumed = post(&running.router, "/api/v1/works/1/acts", &json!({ "kind": "resume" })).await;
    assert_eq!(resumed, (StatusCode::NO_CONTENT, Value::Null));
    pass(2000).await;
    running
}

/// An observation of the plan's first round, as the route gives it.
fn of_the_plan(number: u64, text: &str, read: Value) -> Value {
    json!({
        "number": number,
        "text": text,
        "work": 1,
        "item": 1,
        "title": "Greet the user",
        "cell": "plan",
        "run": 1,
        "round": 1,
        "kept": START + 1200,
        "read": read,
    })
}

#[tokio::test(start_paused = true)]
async fn the_observations_of_a_return_taken_are_listed_newest_first_and_unread() {
    let running = planned().await;
    let listed = get_json(&running.router, "/api/v1/observations").await;
    assert_eq!(
        listed,
        json!({ "observations": [
            of_the_plan(2, "The request names no language.", Value::Null),
            of_the_plan(1, "The greeting has no test.", Value::Null),
        ] })
    );
}

#[tokio::test(start_paused = true)]
async fn an_observation_the_owner_reads_is_read_from_then_on_and_one_there_is_none_of_is_refused() {
    let running = planned().await;
    let router = &running.router;
    let now = START + 3000;
    assert_eq!(
        post(router, "/api/v1/observations/1/read", &json!({})).await,
        (StatusCode::OK, json!({ "read": now }))
    );
    // Read again later, it keeps the time it was first read.
    pass(1000).await;
    assert_eq!(
        post(router, "/api/v1/observations/1/read", &json!({})).await,
        (StatusCode::OK, json!({ "read": now }))
    );
    let listed = get_json(router, "/api/v1/observations").await;
    assert_eq!(
        listed["observations"],
        json!([
            of_the_plan(2, "The request names no language.", Value::Null),
            of_the_plan(1, "The greeting has no test.", json!(now)),
        ])
    );
    assert_eq!(
        post(router, "/api/v1/observations/3/read", &json!({})).await,
        (StatusCode::NOT_FOUND, json!({ "error": "There is no observation O3." }))
    );
    let (status, _) = post(router, "/api/v1/observations/O1/read", &json!({})).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test(start_paused = true)]
async fn a_number_past_what_the_store_can_number_names_no_observation() {
    let running = planned().await;
    // One past the largest the store gives, the largest a path's number reads as, and one past that.
    for number in ["9223372036854775808", "18446744073709551615", "18446744073709551616"] {
        assert_eq!(
            post(
                &running.router,
                &format!("/api/v1/observations/{number}/read"),
                &json!({})
            )
            .await,
            (
                StatusCode::NOT_FOUND,
                json!({ "error": format!("There is no observation O{number}.") })
            ),
            "{number}"
        );
    }
    // Nothing was read.
    let listed = get_json(&running.router, "/api/v1/observations").await;
    let read: Vec<&Value> = (listed["observations"].as_array().unwrap().iter())
        .map(|o| &o["read"])
        .collect();
    assert_eq!(read, [&Value::Null, &Value::Null]);
}
