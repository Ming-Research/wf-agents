//! The reads of the plan, under `/api/v1/plan/`: what changed since a mark, the items that could be let on now, where
//! a text is mentioned, what an item waits for and what waits for it, and the tags.

use axum::http::{Method, StatusCode};
use serde_json::{Value, json};

use crate::{ask, get_json, post, running};

/// An item of the trial flow, held, depending on these, carrying these tags, its request saying this.
fn item(title: &str, on: Value, tags: Value, request: &str) -> Value {
    json!({
        "title": title,
        "dependsOn": on,
        "tags": tags,
        "project": "trial",
        "flow": "trial",
        "writes": [{ "block": "request", "content": request, "value": null }],
    })
}

#[tokio::test(start_paused = true)]
async fn the_candidates_are_the_paused_items_whose_dependencies_merged_those_more_wait_for_first() {
    let running = running();
    let router = &running.router;
    post(router, "/api/v1/items", &item("Lone", json!([]), json!([]), "Please.")).await;
    post(router, "/api/v1/items", &item("Base", json!([]), json!([]), "Please.")).await;
    post(
        router,
        "/api/v1/items",
        &item("On base", json!([2]), json!([]), "Please."),
    )
    .await;
    post(
        router,
        "/api/v1/items",
        &item("On that", json!([3]), json!([]), "Please."),
    )
    .await;
    let candidates = get_json(router, "/api/v1/plan/candidates").await;
    assert_eq!(candidates["total"], 2);
    let rows = candidates["rows"].as_array().unwrap();
    // The base comes first: two items wait for it, by way of the one between.
    assert_eq!(rows[0]["row"]["item"], 2);
    assert_eq!(rows[0]["waitedBy"], 2);
    assert_eq!(rows[0]["started"], false);
    assert_eq!(rows[1]["row"]["item"], 1);
    assert_eq!(rows[1]["waitedBy"], 0);
    assert_eq!(rows[0]["row"]["standing"], "paused");
}

#[tokio::test(start_paused = true)]
async fn the_candidates_and_the_changes_are_bounded_and_what_is_left_out_of_the_changes_comes_again() {
    let running = running();
    let router = &running.router;
    for n in 1..=12 {
        post(
            router,
            "/api/v1/items",
            &item(&format!("Item {n}"), json!([]), json!([]), "Please."),
        )
        .await;
    }
    let candidates = get_json(router, "/api/v1/plan/candidates").await;
    assert_eq!(candidates["total"], 12);
    assert_eq!(candidates["rows"].as_array().unwrap().len(), 10);
    for n in 0..25 {
        let title =
            json!({ "why": "Again", "changes": [{ "kind": "title", "item": 1, "title": format!("Title {n}") }] });
        post(router, "/api/v1/items/changes", &title).await;
    }
    // 12 added and 25 changes: 30 given, and the 7 left out given from the mark, once.
    let first = get_json(router, "/api/v1/plan/changes").await;
    assert_eq!(first["lines"].as_array().unwrap().len(), 30);
    assert_eq!(first["leftOut"], 7);
    let mark = first["mark"].as_str().unwrap().to_string();
    let rest = get_json(router, &format!("/api/v1/plan/changes?since={mark}")).await;
    assert_eq!(rest["lines"].as_array().unwrap().len(), 7);
    assert_eq!(rest["leftOut"], 0);
    let mark = rest["mark"].as_str().unwrap().to_string();
    let none = get_json(router, &format!("/api/v1/plan/changes?since={mark}")).await;
    assert_eq!(none["lines"], json!([]));
}

#[tokio::test(start_paused = true)]
async fn a_walk_goes_up_to_what_an_item_depends_on_and_down_to_what_depends_on_it_to_a_depth() {
    let running = running();
    let router = &running.router;
    for (title, on) in [
        ("A", json!([])),
        ("B", json!([1])),
        ("C", json!([2])),
        ("D", json!([3])),
    ] {
        post(router, "/api/v1/items", &item(title, on, json!([]), "Please.")).await;
    }
    let graph = get_json(router, "/api/v1/plan/graph/2").await;
    assert_eq!(graph["item"]["item"], 2);
    assert_eq!(graph["up"][0]["row"]["item"], 1);
    assert_eq!(graph["down"][0]["row"]["item"], 3);
    assert_eq!(graph["down"][0]["next"][0]["row"]["item"], 4);
    // One step down: what is past it is counted, not shown.
    let graph = get_json(router, "/api/v1/plan/graph/2?way=down&depth=1").await;
    assert_eq!(graph["up"], json!([]));
    assert_eq!(graph["down"][0]["next"], json!([]));
    assert_eq!(graph["down"][0]["beyond"], 1);
    assert_eq!(graph["leftOut"], 1);
    let (status, _, body) = ask(router, Method::GET, "/api/v1/plan/graph/9").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let body: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body, json!({ "error": "There is no item #9." }));
    assert_eq!(
        ask(router, Method::GET, "/api/v1/plan/graph/2?way=sideways").await.0,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test(start_paused = true)]
async fn a_search_finds_a_text_in_titles_in_what_people_wrote_and_in_why_a_change_was_made() {
    let running = running();
    let router = &running.router;
    post(
        router,
        "/api/v1/items",
        &item("Parse the header", json!([]), json!([]), "Please."),
    )
    .await;
    post(
        router,
        "/api/v1/items",
        &item("Other", json!([]), json!([]), "The header parser drops a byte."),
    )
    .await;
    let why = json!({ "why": "The HEADER work is split", "changes": [{ "kind": "tag", "item": 2, "tag": "parser" }] });
    post(router, "/api/v1/items/changes", &why).await;
    let hits = get_json(router, "/api/v1/plan/search?text=header").await;
    let hits = hits["hits"].as_array().unwrap();
    assert_eq!(hits.len(), 3);
    assert_eq!(hits[0]["item"], 1);
    assert_eq!(hits[0]["field"], json!({ "kind": "title" }));
    assert_eq!(hits[0]["snippet"], "Parse the «header»");
    assert_eq!(hits[1]["field"], json!({ "kind": "block", "block": "request" }));
    assert_eq!(hits[1]["snippet"], "The «header» parser drops a byte.");
    assert_eq!(hits[2]["field"]["kind"], "why");
    assert_eq!(hits[2]["snippet"], "The «HEADER» work is split");
    let one = get_json(router, "/api/v1/plan/search?text=header&limit=1").await;
    assert_eq!(one["hits"].as_array().unwrap().len(), 1);
    assert_eq!(one["more"], 2);
    assert_eq!(
        ask(router, Method::GET, "/api/v1/plan/search?text=%20").await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
}

#[tokio::test(start_paused = true)]
async fn the_tags_are_counted_with_the_items_that_have_not_ended() {
    let running = running();
    let router = &running.router;
    post(
        router,
        "/api/v1/items",
        &item("A", json!([]), json!(["page", "engine"]), "Please."),
    )
    .await;
    post(
        router,
        "/api/v1/items",
        &item("B", json!([]), json!(["page"]), "Please."),
    )
    .await;
    let cancel = json!({ "why": "Not needed", "changes": [{ "kind": "cancel", "item": 2 }] });
    post(router, "/api/v1/items/changes", &cancel).await;
    assert_eq!(
        get_json(router, "/api/v1/plan/tags").await,
        json!({ "tags": [
            { "tag": "engine", "items": 1, "open": 1 },
            { "tag": "page", "items": 2, "open": 1 },
        ] })
    );
}

#[tokio::test(start_paused = true)]
async fn what_changed_since_a_mark_is_each_item_added_and_each_change_once() {
    let running = running();
    let router = &running.router;
    post(router, "/api/v1/items", &item("A", json!([]), json!([]), "Please.")).await;
    post(
        router,
        "/api/v1/items",
        &item("B", json!([1]), json!(["page"]), "Please."),
    )
    .await;
    let first = get_json(router, "/api/v1/plan/changes").await;
    let lines = first["lines"].as_array().unwrap();
    assert_eq!(lines.len(), 2);
    assert_eq!(
        lines[1]["what"],
        json!({ "kind": "added", "level": "2", "dependsOn": [1], "tags": ["page"] })
    );
    let mark = first["mark"].as_str().unwrap().to_string();
    assert_eq!(mark, "2");
    // Nothing since.
    let again = get_json(router, &format!("/api/v1/plan/changes?since={mark}")).await;
    assert_eq!(again["lines"], json!([]));
    assert_eq!(again["mark"], mark);
    let changes = json!({ "why": "B is the page's", "changes": [
        { "kind": "title", "item": 2, "title": "B, the page" },
        { "kind": "cancel", "item": 2 },
    ] });
    post(router, "/api/v1/items/changes", &changes).await;
    post(router, "/api/v1/items", &item("C", json!([]), json!([]), "Please.")).await;
    let since = get_json(router, &format!("/api/v1/plan/changes?since={mark}")).await;
    let lines = since["lines"].as_array().unwrap();
    let what: Vec<&Value> = lines.iter().map(|line| &line["what"]["kind"]).collect();
    assert_eq!(what, [&json!("changed"), &json!("cancelled"), &json!("added")]);
    assert_eq!(lines[0]["why"], "B is the page's");
    assert_eq!(lines[1]["why"], "B is the page's");
    assert_eq!(lines[2]["item"], 3);
    assert_eq!(since["mark"], "3,2.1.1");
    let after = get_json(router, "/api/v1/plan/changes?since=3,2.1.1").await;
    assert_eq!(after["lines"], json!([]));
    assert_eq!(
        ask(router, Method::GET, "/api/v1/plan/changes?since=x").await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        ask(router, Method::GET, "/api/v1/plan/changes?since=9").await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
}
