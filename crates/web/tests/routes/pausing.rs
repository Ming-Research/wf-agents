//! The owner's pause with its reason and the items it waits for: as the act takes it and refuses it, and as the work,
//! the candidates and the attention say it.

use axum::http::StatusCode;
use serde_json::{Value, json};

use crate::{get_json, pass, post, running};

/// An item of the trial flow, added paused, depending on none.
fn item(title: &str) -> Value {
    json!({
        "title": title,
        "dependsOn": [],
        "project": "trial",
        "flow": "trial",
        "writes": [{ "block": "request", "content": "Please.", "value": null }],
    })
}

/// The items that could be let on, by their numbers.
fn candidates(answer: &Value) -> Vec<Value> {
    (answer["rows"].as_array().unwrap().iter())
        .map(|row| row["row"]["item"].clone())
        .collect()
}

/// The lines of the attention that say a pause is over, as the item numbers they wait for.
fn over(answer: &Value) -> Vec<(u64, Vec<u64>)> {
    (answer["lines"].as_array().unwrap().iter())
        .filter(|line| line["kind"] == "pauseOver")
        .map(|line| {
            let items = line["items"].as_array().unwrap().iter();
            (
                line["of"]["work"].as_u64().unwrap(),
                items.map(|i| i.as_u64().unwrap()).collect(),
            )
        })
        .collect()
}

// A pause the owner makes may say why, and the items it waits for: the work says them, and says which have not merged
// or been cancelled; the candidates leave it out until each has, and count it; the attention gives a line once each has.
#[tokio::test(start_paused = true)]
async fn a_pause_that_waits_for_an_item_says_so_is_no_candidate_until_it_is_cancelled_and_is_a_line_then() {
    let running = running();
    let router = &running.router;
    post(router, "/api/v1/items", &item("Base")).await;
    post(router, "/api/v1/items", &item("After the base")).await;
    let pause = json!({ "kind": "pause", "why": "After the base is in.", "waitsFor": [1, 1] });
    assert_eq!(
        post(router, "/api/v1/works/2/acts", &pause).await,
        (StatusCode::NO_CONTENT, Value::Null)
    );
    let now = get_json(router, "/api/v1/works/2").await["summary"]["now"].clone();
    assert_eq!(
        (&now["kind"], &now["by"], &now["why"], &now["waitsFor"], &now["open"]),
        (
            &json!("paused"),
            &json!("owner"),
            &json!("After the base is in."),
            &json!([1]),
            &json!([1])
        )
    );
    // It is no candidate while the base has not ended, and the count says so.
    let before = get_json(router, "/api/v1/plan/candidates").await;
    assert_eq!(
        (candidates(&before), &before["total"], &before["waiting"]),
        (vec![json!(1)], &json!(1), &json!(1))
    );
    assert_eq!(over(&get_json(router, "/api/v1/attention").await), vec![]);
    // Once the base is cancelled, the work's pause says what it waited for is over, it is a candidate, and a line.
    let cancel = json!({ "why": "Not needed", "changes": [{ "kind": "cancel", "item": 1 }] });
    assert_eq!(post(router, "/api/v1/items/changes", &cancel).await.0, StatusCode::OK);
    let now = get_json(router, "/api/v1/works/2").await["summary"]["now"].clone();
    assert_eq!((&now["waitsFor"], now.get("open")), (&json!([1]), None));
    let after = get_json(router, "/api/v1/plan/candidates").await;
    assert_eq!((candidates(&after), &after["waiting"]), (vec![json!(2)], &json!(0)));
    let line = get_json(router, "/api/v1/attention").await;
    assert_eq!(over(&line), vec![(2, vec![1])]);
    // A pause with no reason and no item is as it was: it says neither.
    let plain = json!({ "kind": "pause" });
    assert_eq!(
        post(router, "/api/v1/works/2/acts", &plain).await.0,
        StatusCode::NO_CONTENT
    );
    let now = get_json(router, "/api/v1/works/2").await["summary"]["now"].clone();
    assert_eq!((now.get("waitsFor"), &now["why"]), (None, &Value::Null));
}

// An item the pause waits for is one there is, and not the item of the work's own, which it would wait for for ever; the
// refusal changes nothing.
#[tokio::test(start_paused = true)]
async fn a_pause_cannot_wait_for_an_item_there_is_none_of_or_for_its_own_work_and_changes_nothing() {
    let running = running();
    let router = &running.router;
    post(router, "/api/v1/items", &item("Base")).await;
    let unprocessable = StatusCode::UNPROCESSABLE_ENTITY;
    let nowhere = json!({ "kind": "pause", "waitsFor": [9] });
    assert_eq!(
        post(router, "/api/v1/works/1/acts", &nowhere).await,
        (unprocessable, json!({ "error": "There is no item #9." }))
    );
    let itself = json!({ "kind": "pause", "waitsFor": [1] });
    assert_eq!(
        post(router, "/api/v1/works/1/acts", &itself).await,
        (
            unprocessable,
            json!({ "error": "A pause cannot wait for item #1 of its own work, which would wait for ever." })
        )
    );
    let now = get_json(router, "/api/v1/works/1").await["summary"]["now"].clone();
    assert_eq!(now.get("waitsFor"), None);
}

// An item may be added paused with its reason and the items its pause waits for, which the work says as it does of a
// pause made later; one added paused for an item there is none of is refused.
#[tokio::test(start_paused = true)]
async fn an_item_is_added_paused_with_its_reason_and_the_items_its_pause_waits_for() {
    let running = running();
    let router = &running.router;
    post(router, "/api/v1/items", &item("Base")).await;
    let mut after = item("After the base");
    after["why"] = json!("After the base is in.");
    after["waitsFor"] = json!([1]);
    assert_eq!(
        post(router, "/api/v1/items", &after).await,
        (StatusCode::OK, json!({ "item": 2, "work": 2 }))
    );
    let now = get_json(router, "/api/v1/works/2").await["summary"]["now"].clone();
    assert_eq!(
        (&now["why"], &now["waitsFor"], &now["open"]),
        (&json!("After the base is in."), &json!([1]), &json!([1]))
    );
    after["waitsFor"] = json!([9]);
    assert_eq!(
        post(router, "/api/v1/items", &after).await,
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            json!({ "error": "There is no item #9." })
        )
    );
}

// A pause waits for an item until that item has merged or been cancelled. An item cancelled and then reopened has not
// been cancelled, so the pause waits for it again: the work says it is open, and it is no candidate.
#[tokio::test(start_paused = true)]
async fn a_pause_waits_again_for_an_item_that_was_cancelled_and_reopened() {
    let running = running();
    let router = &running.router;
    post(router, "/api/v1/items", &item("Base")).await;
    post(router, "/api/v1/items", &item("After the base")).await;
    let pause = json!({ "kind": "pause", "why": "After the base is in.", "waitsFor": [1] });
    post(router, "/api/v1/works/2/acts", &pause).await;
    let cancel = json!({ "why": "Not needed", "changes": [{ "kind": "cancel", "item": 1 }] });
    assert_eq!(post(router, "/api/v1/items/changes", &cancel).await.0, StatusCode::OK);
    let now = get_json(router, "/api/v1/works/2").await["summary"]["now"].clone();
    assert_eq!(now.get("open"), None, "{now}");
    let reopen = json!({ "why": "Needed after all", "changes": [{ "kind": "reopen", "item": 1 }] });
    assert_eq!(post(router, "/api/v1/items/changes", &reopen).await.0, StatusCode::OK);
    let now = get_json(router, "/api/v1/works/2").await["summary"]["now"].clone();
    assert_eq!((&now["waitsFor"], &now["open"]), (&json!([1]), &json!([1])), "{now}");
    let after = get_json(router, "/api/v1/plan/candidates").await;
    assert!(!candidates(&after).contains(&json!(2)), "{after}");
    assert_eq!(after["waiting"], json!(1));
}

// A pause the owner lifts waits for nothing any more, so the attention gives no line of the pause being over when the
// items it waited for end after the lift. The driver's test lifts a pause only after they ended; the order here is the
// other, at the route.
#[tokio::test(start_paused = true)]
async fn a_pause_that_is_lifted_says_no_items_and_is_no_line_when_they_end_after() {
    let running = running();
    let router = &running.router;
    post(router, "/api/v1/items", &item("Base")).await;
    post(router, "/api/v1/items", &item("After the base")).await;
    let pause = json!({ "kind": "pause", "why": "After the base is in.", "waitsFor": [1] });
    post(router, "/api/v1/works/2/acts", &pause).await;
    assert_eq!(
        post(router, "/api/v1/works/2/acts", &json!({ "kind": "resume" }))
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    let cancel = json!({ "why": "Not needed", "changes": [{ "kind": "cancel", "item": 1 }] });
    post(router, "/api/v1/items/changes", &cancel).await;
    assert_eq!(over(&get_json(router, "/api/v1/attention").await), vec![]);
}

// An item is added with its work paused and no round of it begun: the pause says so, as the first of its rounds, which the
// summary gives, is none. Once a round has begun and the work is paused, the pause says a round has.
#[tokio::test(start_paused = true)]
async fn a_pause_says_whether_a_round_of_the_work_has_begun() {
    let running = running();
    let router = &running.router;
    post(router, "/api/v1/items", &item("Greet")).await;
    let page = get_json(router, "/api/v1/works/1").await;
    assert_eq!(
        (&page["summary"]["now"]["kind"], &page["summary"]["now"]["ran"]),
        (&json!("paused"), &json!(false))
    );
    assert_eq!(page["summary"]["from"], Value::Null);
    // Let on, its first round runs for a second and a half; the owner pauses it while it does.
    assert_eq!(
        post(router, "/api/v1/works/1/acts", &json!({ "kind": "resume" })).await,
        (StatusCode::NO_CONTENT, Value::Null)
    );
    pass(500).await;
    assert_eq!(
        post(router, "/api/v1/works/1/acts", &json!({ "kind": "pause" })).await,
        (StatusCode::NO_CONTENT, Value::Null)
    );
    pass(3000).await;
    let page = get_json(router, "/api/v1/works/1").await;
    let now = &page["summary"]["now"];
    assert_eq!(
        (&now["kind"], &now["by"], &now["ran"]),
        (&json!("paused"), &json!("owner"), &json!(true))
    );
    assert!(
        page["summary"]["from"].is_u64(),
        "a round has begun: {}",
        page["summary"]["from"]
    );
}

// A round the engine would send back, which the owner's pause holds back, is a cell whose round is given again, as one
// whose round failed is; a cell whose round is under way is running, and that does not change with the pause.
#[tokio::test(start_paused = true)]
async fn a_return_the_engine_sends_back_that_a_pause_holds_is_a_cell_to_be_given_again() {
    let running = running();
    let router = &running.router;
    post(router, "/api/v1/items", &item("Greet")).await;
    post(router, "/api/v1/works/1/acts", &json!({ "kind": "resume" })).await;
    pass(500).await;
    let page = get_json(router, "/api/v1/works/1").await;
    assert_eq!(page["summary"]["cells"][0]["state"], "running");
    post(router, "/api/v1/works/1/acts", &json!({ "kind": "pause" })).await;
    // The plan's first round returns nothing, which the engine sends back; the pause holds the round it is given again.
    pass(3000).await;
    let page = get_json(router, "/api/v1/works/1").await;
    assert_eq!(page["summary"]["now"]["kind"], "paused");
    assert_eq!(
        page["summary"]["cells"][0],
        json!({ "cell": "plan", "state": "failed", "finished": 0, "record": null, "open": 1 })
    );
    // Let on, it is given the round again: it runs.
    post(router, "/api/v1/works/1/acts", &json!({ "kind": "resume" })).await;
    let page = get_json(router, "/api/v1/works/1").await;
    assert_eq!(page["summary"]["cells"][0]["state"], "running");
}
