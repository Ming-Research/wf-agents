//! `POST /api/v1/items/changes`: whoever plans the work changes items after they were added, several at once for one
//! reason, all of the changes or none, and the item's history says what it was before, when it changed and why; a
//! cancel says why, is refused while an item that has not ended depends on it, and a cancelled item is reopened.

use axum::http::StatusCode;
use serde_json::{Value, json};

use crate::{get_json, greeted, pass, post, running};

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

/// Changes asked together, for this reason.
fn changes(why: &str, changes: Value) -> Value {
    json!({ "why": why, "changes": changes })
}

const CHANGES: &str = "/api/v1/items/changes";

fn refused(words: &str) -> (StatusCode, Value) {
    (StatusCode::UNPROCESSABLE_ENTITY, json!({ "error": words }))
}

#[tokio::test(start_paused = true)]
async fn an_item_not_begun_is_retitled_tagged_and_comes_to_depend_on_another_and_its_history_says_why() {
    let running = running();
    let router = &running.router;
    post(router, "/api/v1/items", &item("First", json!([]))).await;
    post(router, "/api/v1/items", &item("Second", json!([]))).await;
    let changed = |changed: Value| (StatusCode::OK, json!({ "changed": changed }));
    let asked = changes(
        "The second waits for the first",
        json!([
            { "kind": "title", "item": 2, "title": "Second, narrowed" },
            { "kind": "dependsOn", "item": 2, "on": 1 },
            { "kind": "tag", "item": 2, "tag": "parser" },
        ]),
    );
    assert_eq!(post(router, CHANGES, &asked).await, changed(json!([true, true, true])));
    assert_eq!(
        post(router, CHANGES, &asked).await,
        changed(json!([false, false, false]))
    );
    let items = get_json(router, "/api/v1/items").await;
    let second = &items["items"][1];
    assert_eq!(second["title"], "Second, narrowed");
    assert_eq!(second["dependsOn"], json!([1]));
    assert_eq!(second["tags"], json!(["parser"]));
    let history = &second["history"];
    assert_eq!(
        history["first"],
        json!({ "title": "Second", "level": "2", "dependsOn": [], "tags": [] })
    );
    let changes_made = history["changes"].as_array().unwrap();
    assert_eq!(changes_made.len(), 3);
    assert_eq!(
        changes_made[0]["what"],
        json!({ "kind": "title", "title": "Second, narrowed" })
    );
    assert_eq!(changes_made[1]["what"], json!({ "kind": "dependsOn", "item": 1 }));
    assert_eq!(changes_made[2]["what"], json!({ "kind": "tagged", "tag": "parser" }));
    for change in changes_made {
        assert_eq!(change["why"], "The second waits for the first");
        assert_eq!(
            change["time"], changes_made[0]["time"],
            "changes made together have one time"
        );
    }
    // The work's page gives the same history.
    assert_eq!(get_json(router, "/api/v1/works/2").await["history"], *history);
    let undo = changes(
        "Not after all",
        json!([{ "kind": "noLongerDependsOn", "item": 2, "on": 1 }, { "kind": "untag", "item": 2, "tag": "parser" }]),
    );
    assert_eq!(post(router, CHANGES, &undo).await, changed(json!([true, true])));
    let items = get_json(router, "/api/v1/items").await;
    assert_eq!(items["items"][1]["dependsOn"], json!([]));
    assert_eq!(items["items"][1]["tags"], json!([]));
}

#[tokio::test(start_paused = true)]
async fn a_change_says_why_and_what_the_engine_refuses_says_why_and_changes_nothing() {
    let running = running();
    let router = &running.router;
    greeted(router).await;
    post(router, "/api/v1/items", &item("Second", json!([1]))).await;
    let title = |item: u64, title: &str| json!([{ "kind": "title", "item": item, "title": title }]);
    for why in ["", "  "] {
        assert_eq!(
            post(router, CHANGES, &changes(why, title(2, "Other"))).await,
            refused("A change says why it is made, and this one says nothing: give a reason in a sentence.")
        );
    }
    assert_eq!(
        post(router, CHANGES, &changes("why", json!([]))).await,
        refused("No change is asked for.")
    );
    let blank = "A title says what the item is to deliver, and one of no words says nothing: give it words.";
    for words in ["", "   "] {
        assert_eq!(
            post(router, CHANGES, &changes("why", title(2, words))).await,
            refused(blank)
        );
    }
    let depend = |item: u64, on: u64| json!([{ "kind": "dependsOn", "item": item, "on": on }]);
    assert_eq!(
        post(router, CHANGES, &changes("why", depend(1, 2))).await,
        refused(
            "The work of #1 has started, and what an item depends on changes only until its work starts: what it \
             depends on is what its work started on. What turns up for it is a new item, which may depend on it."
        )
    );
    assert_eq!(
        post(router, CHANGES, &changes("why", depend(2, 2))).await,
        refused("#2 cannot depend on itself.")
    );
    assert_eq!(
        post(router, CHANGES, &changes("why", title(3, "x"))).await,
        refused("There is no item #3.")
    );
    let level = json!([{ "kind": "level", "item": 2, "level": "9" }]);
    assert_eq!(
        post(router, CHANGES, &changes("why", level)).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let unknown = json!([{ "kind": "join", "item": 2, "on": 1 }]);
    assert_eq!(
        post(router, CHANGES, &changes("why", unknown)).await.0,
        StatusCode::BAD_REQUEST
    );
    // Several asked together: each refused is named by its place, and none is made, the good ones neither.
    let together = json!([
        { "kind": "title", "item": 2, "title": "Kept?" },
        { "kind": "cancel", "item": 1 },
        { "kind": "tag", "item": 2, "tag": "two words" },
    ]);
    assert_eq!(
        post(router, CHANGES, &changes("why", together)).await,
        refused(
            "Change 2: #2 depends on #1 and has not ended, and would wait for it for good: cancel it first, in the \
             same change before it, or take that dependency away; change 3: \"two words\" is no tag: a tag is a name \
             of 1 to 40 characters with no space and no comma; so none of the changes was made."
        )
    );
    assert_eq!(get_json(router, "/api/v1/items").await["items"][1]["title"], "Second");
    // A begun item is still given another title.
    assert_eq!(
        post(router, CHANGES, &changes("why", title(1, "Greet the user by name"))).await,
        (StatusCode::OK, json!({ "changed": [true] }))
    );
}

#[tokio::test(start_paused = true)]
async fn a_cancel_says_why_is_refused_under_a_dependant_and_a_cancelled_item_is_reopened() {
    let running = running();
    let router = &running.router;
    post(router, "/api/v1/items", &item("First", json!([]))).await;
    post(router, "/api/v1/items", &item("Second", json!([1]))).await;
    // The owner's cancel on the page is judged as every change is.
    let refused_cancel = post(router, "/api/v1/works/1/acts", &json!({ "kind": "cancel" })).await;
    assert_eq!(
        refused_cancel,
        refused(
            "#2 depends on #1 and has not ended, and would wait for it for good: cancel it first, in the same change \
             before it, or take that dependency away."
        )
    );
    let acts = "/api/v1/works/2/acts";
    assert_eq!(
        post(router, acts, &json!({ "kind": "cancel", "why": "  " })).await,
        refused("A cancel's reason says nothing: give one that says why, or none.")
    );
    let cancel = json!({ "kind": "cancel", "why": "Joined into item 1." });
    assert_eq!(post(router, acts, &cancel).await, (StatusCode::NO_CONTENT, Value::Null));
    pass(100).await;
    let page = get_json(router, "/api/v1/works/2").await;
    let steps = page["steps"].as_array().unwrap();
    let cancelled = steps.iter().find(|s| s["kind"] == "cancelled").expect("a cancel");
    assert_eq!(cancelled["why"], "Joined into item 1.");
    assert_eq!(
        post(
            router,
            CHANGES,
            &changes("why", json!([{ "kind": "title", "item": 2, "title": "Other" }]))
        )
        .await,
        refused("#2 is cancelled: an item that has ended takes no change, and is what it ended as; reopen it first.")
    );
    // Reopened, it is paused, and its history says why.
    let reopen = changes("Item 1 turned out too big", json!([{ "kind": "reopen", "item": 2 }]));
    assert_eq!(
        post(router, CHANGES, &reopen).await,
        (StatusCode::OK, json!({ "changed": [true] }))
    );
    let page = get_json(router, "/api/v1/works/2").await;
    let steps = page["steps"].as_array().unwrap();
    let reopened = steps.iter().find(|s| s["kind"] == "reopened").expect("a reopen");
    assert_eq!(reopened["why"], "Item 1 turned out too big");
    // Paused, it waits besides for the item it depends on.
    assert_eq!(page["summary"]["paused"], true);
    assert_eq!(page["summary"]["now"]["kind"], "waits");
    assert_eq!(
        post(router, CHANGES, &reopen).await,
        refused("#2 is not cancelled: only a cancelled item is reopened.")
    );
}
