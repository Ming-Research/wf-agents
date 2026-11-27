//! How many open children an act leaves an item with, as the interface answers it: an add, changes asked together and
//! a cancel each answer, where they left an item past the warning, with what they say of it, and are refused past the
//! most, naming the item and the count; where nothing is past the warning they answer as they always do.

use axum::http::StatusCode;
use serde_json::{Value, json};
use service::Scripted;
use store::Store;

use crate::{post, running_carried, trial_script};

/// An item of the trial flow, held, depending on these.
fn item(title: &str, on: Value) -> Value {
    json!({
        "title": title,
        "dependsOn": on,
        "project": "trial",
        "flow": "trial",
        "writes": [{ "block": "request", "content": "Please.", "value": null }],
    })
}

/// Changes asked together, for a reason.
fn changes(changes: Value) -> Value {
    json!({ "why": "The plan moved", "changes": changes })
}

/// What is said of an item left with this many open children, past one.
fn warning(item: u64, children: usize) -> String {
    format!(
        "#{item} has {children} open children now, more than 1 (items.children_warn): group some of them under a \
         deliver item."
    )
}

/// What a refusal says of an item, or of the new one, that would have this many open children, past two.
fn refusal(item: &str, children: usize) -> (StatusCode, Value) {
    let error = format!(
        "{item} would have {children} open children, more than 2 (items.children_max): group some of them under a \
         deliver item first; so nothing was changed."
    );
    (StatusCode::UNPROCESSABLE_ENTITY, json!({ "error": error }))
}

#[tokio::test(start_paused = true)]
async fn an_add_a_change_and_a_cancel_answer_with_their_warnings_and_are_refused_past_the_most() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("store.sqlite")).unwrap();
    let mut carriers = Scripted::new(trial_script()).carriers();
    carriers.children = Some(service::Children { warn: 1, most: 2 });
    let running = running_carried(Box::new(store), dir, carriers);
    let router = &running.router;
    for title in ["a1", "a2", "a3"] {
        post(router, "/api/v1/items", &item(title, json!([]))).await;
    }
    // p, #4, has one open child, and q, #5, two: q's add is warned of.
    assert_eq!(
        post(router, "/api/v1/items", &item("p", json!([1]))).await,
        (StatusCode::OK, json!({ "item": 4, "work": 4 }))
    );
    assert_eq!(
        post(router, "/api/v1/items", &item("q", json!([2, 3]))).await,
        (
            StatusCode::OK,
            json!({ "item": 5, "work": 5, "warnings": [warning(5, 2)] })
        )
    );
    // An item on three free ones would have three: refused, and nothing added.
    for title in ["b1", "b2", "b3"] {
        post(router, "/api/v1/items", &item(title, json!([]))).await;
    }
    assert_eq!(
        post(router, "/api/v1/items", &item("r", json!([6, 7, 8]))).await,
        refusal("The new item", 3)
    );
    // p comes to depend on b1, its second: warned of. q depends on a1 too, which stays under p.
    let depends = |item: u64, on: u64| changes(json!([{ "kind": "dependsOn", "item": item, "on": on }]));
    assert_eq!(
        post(router, "/api/v1/items/changes", &depends(4, 6)).await,
        (
            StatusCode::OK,
            json!({ "changed": [true], "warnings": [warning(4, 2)] })
        )
    );
    assert_eq!(
        post(router, "/api/v1/items/changes", &depends(5, 1)).await,
        (StatusCode::OK, json!({ "changed": [true] }))
    );
    // Cancelled, p would leave a1 to q, its third: refused.
    let cancel = json!({ "kind": "cancel", "why": "Joined into q" });
    assert_eq!(post(router, "/api/v1/works/4/acts", &cancel).await, refusal("#5", 3));
    // With one child fewer, q takes a1 as its second, and the cancel answers with the warning.
    let leave = changes(json!([{ "kind": "noLongerDependsOn", "item": 5, "on": 2 }]));
    assert_eq!(
        post(router, "/api/v1/items/changes", &leave).await,
        (StatusCode::OK, json!({ "changed": [true] }))
    );
    assert_eq!(
        post(router, "/api/v1/works/4/acts", &cancel).await,
        (StatusCode::OK, json!({ "warnings": [warning(5, 2)] }))
    );
}
