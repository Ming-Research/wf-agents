//! `POST /api/v1/items/changes`: whoever plans the work changes items after they were added, several at once for one
//! reason, all of the changes or none, and the item's history says what it was before, when it changed and why; a
//! cancel says why and takes away no dependency on its item, and a cancelled item is reopened.

use axum::http::StatusCode;
use serde_json::{Value, json};

use crate::{get_json, greeted, pass, post, running, running_placing};

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

/// An item of the flow `placing`, whose first cell starts a work, held.
fn placing_item(title: &str) -> Value {
    json!({
        "title": title,
        "dependsOn": [],
        "project": "trial",
        "flow": "placing",
        "writes": [],
    })
}

#[tokio::test(start_paused = true)]
async fn an_item_comes_to_continue_a_branch_or_a_pull_request_and_to_continue_nothing_again_and_its_history_says_why() {
    let running = running_placing();
    let router = &running.router;
    assert_eq!(
        post(router, "/api/v1/items", &placing_item("First")).await,
        (StatusCode::OK, json!({ "item": 1, "work": 1 }))
    );
    let ask = |continues: Value| {
        changes(
            "Continue the old work",
            json!([{ "kind": "continues", "item": 1, "continues": continues }]),
        )
    };
    let changed = |changed: bool| (StatusCode::OK, json!({ "changed": [changed] }));
    let branch = json!({ "kind": "branch", "name": "old-work" });
    let pull = json!({ "kind": "pull", "number": 226 });
    assert_eq!(post(router, CHANGES, &ask(branch.clone())).await, changed(true));
    assert_eq!(get_json(router, "/api/v1/items").await["items"][0]["continues"], branch);
    assert_eq!(post(router, CHANGES, &ask(branch.clone())).await, changed(false));
    assert_eq!(post(router, CHANGES, &ask(pull.clone())).await, changed(true));
    assert_eq!(get_json(router, "/api/v1/items").await["items"][0]["continues"], pull);
    assert_eq!(post(router, CHANGES, &ask(Value::Null)).await, changed(true));
    let items = get_json(router, "/api/v1/items").await;
    let first = &items["items"][0];
    assert!(first.get("continues").is_none(), "{first}");
    let history = &first["history"];
    assert!(history["first"].get("continues").is_none(), "{history}");
    let made = history["changes"].as_array().unwrap();
    assert_eq!(made.len(), 3);
    for (change, continues) in made.iter().zip([branch, pull, Value::Null]) {
        assert_eq!(change["what"], json!({ "kind": "continues", "continues": continues }));
        assert_eq!(change["why"], "Continue the old work");
    }
    // The work's page gives the same history.
    assert_eq!(get_json(router, "/api/v1/works/1").await["history"], *history);
}

#[tokio::test(start_paused = true)]
async fn an_item_is_refused_continuing_what_no_branch_is_called_and_on_a_flow_that_starts_no_work() {
    let placing = running_placing();
    let router = &placing.router;
    post(router, "/api/v1/items", &placing_item("First")).await;
    let ask = |item: u64, continues: Value| {
        changes(
            "Continue the old work",
            json!([{ "kind": "continues", "item": item, "continues": continues }]),
        )
    };
    let (status, body) = post(router, CHANGES, &ask(1, json!({ "kind": "branch", "name": "-x" }))).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .contains("is no branch an item may continue"),
        "{body}"
    );
    assert!(
        get_json(router, "/api/v1/items").await["items"][0]
            .get("continues")
            .is_none()
    );
    // The trial flow has no cell that starts a work: nothing would read what the item continues.
    let trial = running();
    let router = &trial.router;
    post(router, "/api/v1/items", &item("First", json!([]))).await;
    let (status, body) = post(router, CHANGES, &ask(1, json!({ "kind": "pull", "number": 7 }))).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .contains("has no cell that starts a work"),
        "{body}"
    );
    assert_eq!(
        post(router, CHANGES, &ask(1, Value::Null)).await,
        (StatusCode::OK, json!({ "changed": [false] }))
    );
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
        refused("#1 cannot depend on #2, which depends on it already: #2 depends on #1.")
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
    // Several asked together: each refused is named by its place, and none is made, the good ones neither: neither
    // the title nor the cancel of the item another depends on, which is no reason to refuse it.
    let together = json!([
        { "kind": "title", "item": 2, "title": "Kept?" },
        { "kind": "cancel", "item": 1 },
        { "kind": "tag", "item": 2, "tag": "two words" },
    ]);
    assert_eq!(
        post(router, CHANGES, &changes("why", together)).await,
        refused(
            "Change 3: \"two words\" is no tag: a tag is a name of 1 to 40 characters with no space and no comma; so \
             none of the changes was made."
        )
    );
    let items = get_json(router, "/api/v1/items").await;
    assert_eq!(items["items"][1]["title"], "Second");
    assert_ne!(items["items"][0]["work"]["now"]["kind"], "cancelled");
    // A begun item is still given another title.
    assert_eq!(
        post(router, CHANGES, &changes("why", title(1, "Greet the user by name"))).await,
        (StatusCode::OK, json!({ "changed": [true] }))
    );
}

#[tokio::test(start_paused = true)]
async fn an_item_whose_work_has_started_comes_to_depend_on_another_item_and_gives_it_up_again() {
    let running = running();
    let router = &running.router;
    greeted(router).await;
    post(router, "/api/v1/items", &item("Second", json!([]))).await;
    let depend = json!([{ "kind": "dependsOn", "item": 1, "on": 2 }]);
    assert_eq!(
        post(router, CHANGES, &changes("why", depend)).await,
        (StatusCode::OK, json!({ "changed": [true] }))
    );
    // The work, which has run, says it waits for the item at the cell where it stands, beside what it waited for.
    let items = get_json(router, "/api/v1/items").await;
    assert_eq!(items["items"][0]["dependsOn"], json!([2]));
    let now = &items["items"][0]["work"]["now"];
    assert_eq!(now["kind"], "waits", "{now}");
    let waited = now["on"].as_array().expect("what it waits for");
    assert!(waited.contains(&json!({ "kind": "items", "items": [2] })), "{now}");
    let leave = json!([{ "kind": "noLongerDependsOn", "item": 1, "on": 2 }]);
    assert_eq!(
        post(router, CHANGES, &changes("why", leave)).await,
        (StatusCode::OK, json!({ "changed": [true] }))
    );
    let items = get_json(router, "/api/v1/items").await;
    assert_eq!(items["items"][0]["dependsOn"], json!([]));
    let now = &items["items"][0]["work"]["now"];
    let waited = now["on"].as_array().map_or(&[][..], Vec::as_slice);
    assert!(!waited.contains(&json!({ "kind": "items", "items": [2] })), "{now}");
}

#[tokio::test(start_paused = true)]
async fn a_dependency_is_made_the_main_one_and_the_store_moves_the_main_one_once_its_item_is_cancelled() {
    let running = running();
    let router = &running.router;
    post(router, "/api/v1/items", &item("First", json!([]))).await;
    post(router, "/api/v1/items", &item("Second", json!([1]))).await;
    post(router, "/api/v1/items", &item("Third", json!([1]))).await;
    let main_of = |items: &Value, item: usize| items["items"][item - 1]["mainDependent"].clone();
    // The first dependency on an item is its main one.
    let items = get_json(router, "/api/v1/items").await;
    assert_eq!(main_of(&items, 1), json!(2));
    assert_eq!(main_of(&items, 3), Value::Null);
    // Made the main one, the third's takes its place, and made so again it is no change; an item that does not depend
    // on the other is refused.
    let main = changes(
        "It stands under the third",
        json!([{ "kind": "main", "item": 3, "on": 1 }]),
    );
    let changed = |changed: Value| (StatusCode::OK, json!({ "changed": changed }));
    assert_eq!(post(router, CHANGES, &main).await, changed(json!([true])));
    assert_eq!(post(router, CHANGES, &main).await, changed(json!([false])));
    assert_eq!(
        post(
            router,
            CHANGES,
            &changes("why", json!([{ "kind": "main", "item": 1, "on": 2 }]))
        )
        .await,
        refused("#1 does not depend on #2.")
    );
    let items = get_json(router, "/api/v1/items").await;
    assert_eq!(main_of(&items, 1), json!(3));
    let history = items["items"][2]["history"]["changes"].as_array().unwrap().clone();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0]["what"], json!({ "kind": "main", "item": 1 }));
    // A work's page says each dependency of its item, both ways, and which is the main one.
    assert_eq!(
        get_json(router, "/api/v1/works/1").await["dependencies"],
        json!({ "on": [], "by": [{ "item": 2, "main": false }, { "item": 3, "main": true }] })
    );
    assert_eq!(
        get_json(router, "/api/v1/works/3").await["dependencies"],
        json!({ "on": [{ "item": 1, "main": true }], "by": [] })
    );
    // Its item cancelled, the main one goes to the earliest made of those left whose item is not cancelled.
    let cancel = changes("Joined into the second", json!([{ "kind": "cancel", "item": 3 }]));
    assert_eq!(post(router, CHANGES, &cancel).await, changed(json!([true])));
    let items = get_json(router, "/api/v1/items").await;
    assert_eq!(main_of(&items, 1), json!(2));
}

#[tokio::test(start_paused = true)]
async fn a_cancel_says_why_and_keeps_what_depends_on_its_item_and_a_cancelled_item_is_reopened() {
    let running = running();
    let router = &running.router;
    post(router, "/api/v1/items", &item("First", json!([]))).await;
    post(router, "/api/v1/items", &item("Second", json!([1]))).await;
    // The owner's cancel on the page is judged as every change is, and takes away no dependency on the item: the
    // item that depends on it still does, and the page places the cancelled item under it.
    let cancel_first = post(router, "/api/v1/works/1/acts", &json!({ "kind": "cancel" })).await;
    assert_eq!(cancel_first, (StatusCode::NO_CONTENT, Value::Null));
    let items = get_json(router, "/api/v1/items").await;
    assert_eq!(items["items"][0]["work"]["now"]["kind"], "cancelled");
    assert_eq!(items["items"][1]["dependsOn"], json!([1]));
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
    // Reopened, the item 2 depends on holds it again.
    let reopen_first = changes("Wanted after all", json!([{ "kind": "reopen", "item": 1 }]));
    assert_eq!(
        post(router, CHANGES, &reopen_first).await,
        (StatusCode::OK, json!({ "changed": [true] }))
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
    // Paused by the owner, it waits besides for the item it depends on, reopened before it.
    assert_eq!(page["summary"]["paused"], "owner");
    assert_eq!(page["summary"]["now"]["kind"], "waits");
    assert_eq!(
        post(router, CHANGES, &reopen).await,
        refused("#2 is not cancelled: only a cancelled item is reopened.")
    );
}

#[tokio::test(start_paused = true)]
async fn a_title_longer_than_the_installation_allows_is_refused_as_added_and_as_changed_and_nothing_is_made() {
    let running = running();
    let router = &running.router;
    greeted(router).await;
    let wide = "\u{4e2d}".repeat(21);
    let why = "the title has 21 words and a title has 20 at most (a Chinese character counts as a word): write it \
               shorter";
    // Twenty Chinese characters, or twenty words, fit.
    let fits = format!("{} {}", "a".repeat(20), "b ".repeat(19));
    assert_eq!(
        post(router, "/api/v1/items", &item(&fits, json!([]))).await.0,
        StatusCode::OK
    );
    assert_eq!(
        post(router, "/api/v1/items", &item(&wide, json!([]))).await,
        refused(&format!("The item's title: {why}."))
    );
    let title = |title: &str| json!([{ "kind": "title", "item": 2, "title": title }]);
    assert_eq!(
        post(router, CHANGES, &changes("why", title(&wide))).await,
        refused(&format!("The item's title: {why}."))
    );
    // Of several changes, the refused one is named by its place and none is made.
    let several = json!([
        { "kind": "tag", "item": 2, "tag": "x" },
        { "kind": "title", "item": 2, "title": wide },
    ]);
    assert_eq!(
        post(router, CHANGES, &changes("why", several)).await,
        refused(&format!(
            "Change 2: the item's title: {why}; so none of the changes was made."
        ))
    );
    let items = get_json(router, "/api/v1/items").await;
    assert_eq!(items["items"].as_array().unwrap().len(), 2);
    assert_eq!(items["items"][1]["title"], fits);
    assert_eq!(
        post(router, CHANGES, &changes("why", title(&"\u{4e2d}".repeat(20))))
            .await
            .0,
        StatusCode::OK
    );
}
