//! A new session the owner asks for: taken as what happened to the work, shown as a step with why, and refused for a
//! line the flow has no cell on.

use axum::http::StatusCode;
use serde_json::{Value, json};

use crate::{get_json, greeted, post, running};

const ACTS: &str = "/api/v1/works/1/acts";

#[tokio::test(start_paused = true)]
async fn the_owners_new_session_is_a_step_of_the_work_with_why() {
    let running = running();
    let router = &running.router;
    greeted(router).await;
    let done = (StatusCode::NO_CONTENT, Value::Null);
    assert_eq!(
        post(router, ACTS, &json!({ "kind": "newSession", "line": "engineer" })).await,
        done
    );
    let page = get_json(router, "/api/v1/works/1").await;
    let session = (page["steps"].as_array().unwrap().iter())
        .find(|s| s["kind"] == "session")
        .expect("a step of the session")
        .clone();
    assert_eq!(session["line"], "engineer");
    assert_eq!(session["why"], json!({ "kind": "owner" }));
    let refused = post(router, ACTS, &json!({ "kind": "newSession", "line": "elsewhere" })).await;
    assert_eq!(refused.0, StatusCode::UNPROCESSABLE_ENTITY, "{refused:?}");
}
