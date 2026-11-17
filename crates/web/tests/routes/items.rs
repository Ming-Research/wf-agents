//! `GET /api/v1/items` and `POST /api/v1/items`: the items with their works, and adding one.

use axum::http::StatusCode;
use serde_json::{Value, json};

use flow::Source;
use service::Scripted;
use store::Store;

use crate::{START, get_json, greeted, pass, post, post_text, running, running_flow, trial_flow, trial_script};

#[tokio::test(start_paused = true)]
async fn the_items_are_given_each_with_its_work_in_brief() {
    let running = running();
    greeted(&running.router).await;
    let summary = get_json(&running.router, "/api/v1/works/1").await["summary"].clone();
    assert_eq!(summary["now"]["kind"], "waits");
    assert_eq!(
        get_json(&running.router, "/api/v1/items").await,
        json!({
            "items": [{
                "number": 1,
                "title": "Greet the user",
                "level": "2",
                "project": "trial",
                "added": START,
                "dependsOn": [],
                "tags": [],
                "history": {
                    "first": { "title": "Greet the user", "level": "2", "dependsOn": [], "tags": [] },
                    "changes": [],
                },
                "work": summary,
            }],
            "outage": null,
        })
    );
}

#[tokio::test(start_paused = true)]
async fn an_item_is_added_with_its_work_paused_and_what_it_depends_on() {
    let running = running();
    let router = &running.router;
    let item = |title: &str, on: Value| {
        json!({
            "title": title,
            "dependsOn": on,
            "project": "trial",
            "flow": "trial",
            "writes": [{ "block": "request", "content": "Please.", "value": null }],
        })
    };
    assert_eq!(
        post(router, "/api/v1/items", &item("First", json!([]))).await,
        (StatusCode::OK, json!({ "item": 1, "work": 1 }))
    );
    assert_eq!(
        post(router, "/api/v1/items", &item("Second", json!([1]))).await,
        (StatusCode::OK, json!({ "item": 2, "work": 2 }))
    );
    let items = get_json(router, "/api/v1/items").await;
    let second = &items["items"][1];
    assert_eq!(second["dependsOn"], json!([1]));
    assert_eq!(second["work"]["paused"], "owner");
    // A precondition only the items it depends on make hold holds the work before its cell whether or not it is
    // paused, and says which of them have not merged.
    assert_eq!(
        second["work"]["now"],
        json!({
            "kind": "waits",
            "at": "plan",
            "on": [{ "kind": "items", "items": [1] }],
        })
    );
    assert_eq!(
        items["items"][0]["work"]["now"],
        json!({ "kind": "paused", "at": "plan", "by": "owner", "why": null })
    );
}

#[tokio::test(start_paused = true)]
async fn an_item_is_added_at_the_level_it_asks_for_or_at_the_default_and_never_above_the_highest() {
    let running = running();
    let router = &running.router;
    let item = |level: Value| json!({ "title": "Greet", "level": level, "dependsOn": [], "project": "trial", "flow": "trial", "writes": [] });
    assert_eq!(post(router, "/api/v1/items", &item(json!("1"))).await.0, StatusCode::OK);
    assert_eq!(
        post(router, "/api/v1/items", &item(Value::Null)).await.0,
        StatusCode::OK
    );
    let unprocessable = StatusCode::UNPROCESSABLE_ENTITY;
    let error = |words: &str| json!({ "error": words });
    assert_eq!(
        post(router, "/api/v1/items", &item(json!("3"))).await,
        (
            unprocessable,
            error("The item's level: 3 is above 2, the highest level an item may be at (items.highest_level).")
        )
    );
    assert_eq!(
        post(router, "/api/v1/items", &item(json!("9"))).await,
        (
            unprocessable,
            error("The item's level: 9 is no level: the levels are 1, 2, 3 (agents.levels).")
        )
    );
    let items = get_json(router, "/api/v1/items").await;
    let levels: Vec<&Value> = items["items"].as_array().unwrap().iter().map(|i| &i["level"]).collect();
    assert_eq!(levels, [&json!("1"), &json!("2")]);
    assert_eq!(get_json(router, "/api/v1/works/2").await["level"], json!("2"));
}

// An item is added to one of the projects the installation names, which the page reads, and keeps it; it may depend on
// an item of another project. One added to a project the installation does not name is refused, and one that names no
// project cannot be read.
#[tokio::test(start_paused = true)]
async fn an_item_is_added_to_a_project_the_installation_names_and_may_depend_on_one_of_another() {
    let running = running();
    let router = &running.router;
    assert_eq!(
        get_json(router, "/api/v1/projects").await,
        json!({ "names": ["other", "trial"] })
    );
    let item = |project: &str, on: Value| json!({ "title": "Greet", "dependsOn": on, "project": project, "flow": "trial", "writes": [] });
    assert_eq!(
        post(router, "/api/v1/items", &item("other", json!([]))).await.0,
        StatusCode::OK
    );
    assert_eq!(
        post(router, "/api/v1/items", &item("trial", json!([1]))).await.0,
        StatusCode::OK
    );
    assert_eq!(
        post(router, "/api/v1/items", &item("elsewhere", json!([]))).await,
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            json!({
                "error": "The item's project: projects: the installation names no project \"elsewhere\"; it names other, \
                          trial."
            })
        )
    );
    let items = get_json(router, "/api/v1/items").await;
    let projects: Vec<&Value> = items["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| &i["project"])
        .collect();
    assert_eq!(projects, [&json!("other"), &json!("trial")]);
    assert_eq!(items["items"][1]["dependsOn"], json!([1]));
    assert_eq!(get_json(router, "/api/v1/works/2").await["project"], json!("trial"));
    let unnamed = json!({ "title": "Greet", "dependsOn": [], "flow": "trial", "writes": [] });
    assert_eq!(post(router, "/api/v1/items", &unnamed).await.0, StatusCode::BAD_REQUEST);
}

#[tokio::test(start_paused = true)]
async fn an_item_is_put_at_another_level_and_refused_as_an_added_one_is() {
    let running = running();
    let router = &running.router;
    let item = json!({ "title": "Greet", "dependsOn": [], "project": "trial", "flow": "trial", "writes": [] });
    assert_eq!(post(router, "/api/v1/items", &item).await.0, StatusCode::OK);
    let set = |item: u64, level: &str| json!({ "why": "It is easier than it looked.", "changes": [{ "kind": "level", "item": item, "level": level }] });
    let changes = "/api/v1/items/changes";
    assert_eq!(
        post(router, changes, &set(1, "1")).await,
        (StatusCode::OK, json!({ "changed": [true] }))
    );
    assert_eq!(
        post(router, changes, &set(1, "1")).await,
        (StatusCode::OK, json!({ "changed": [false] }))
    );
    let unprocessable = StatusCode::UNPROCESSABLE_ENTITY;
    assert_eq!(
        post(router, changes, &set(1, "3")).await,
        (
            unprocessable,
            json!({ "error": "The item's level: 3 is above 2, the highest level an item may be at (items.highest_level)." })
        )
    );
    assert_eq!(
        post(router, changes, &set(7, "1")).await,
        (unprocessable, json!({ "error": "There is no item #7." }))
    );
    assert_eq!(get_json(router, "/api/v1/items").await["items"][0]["level"], json!("1"));
}

#[tokio::test(start_paused = true)]
async fn the_levels_are_given_with_what_each_is_for_the_default_and_the_highest() {
    let running = running();
    assert_eq!(
        get_json(&running.router, "/api/v1/levels").await,
        json!({
            "levels": [
                { "name": "1", "desc": "Easy work" },
                { "name": "2", "desc": "Ordinary work" },
                { "name": "3", "desc": "Hard work" },
            ],
            "default": "2",
            "highest": "2",
        })
    );
}

#[tokio::test(start_paused = true)]
async fn an_item_that_cannot_be_added_is_refused_with_why() {
    let running = running();
    let router = &running.router;
    let item = |flow: &str, on: Value, block: &str| {
        json!({
            "title": "Greet the user",
            "dependsOn": on,
            "project": "trial",
            "flow": flow,
            "writes": [{ "block": block, "content": "Please.", "value": null }],
        })
    };
    let unprocessable = StatusCode::UNPROCESSABLE_ENTITY;
    let error = |words: &str| json!({ "error": words });
    assert_eq!(
        post(router, "/api/v1/items", &item("other", json!([]), "request")).await,
        (unprocessable, error("There is no flow named other."))
    );
    assert_eq!(
        post(router, "/api/v1/items", &item("trial", json!([5]), "request")).await,
        (unprocessable, error("There is no item #5."))
    );
    assert_eq!(
        post(router, "/api/v1/items", &item("trial", json!([]), "plan")).await,
        (unprocessable, error("People do not write the block plan."))
    );
    for body in [
        "not json",
        r#"{ "title": "Greet the user", "flow": "trial", "dependsOn": [], "writes": [], "paused": true }"#,
    ] {
        let (status, json) = post_text(router, "/api/v1/items", body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert!(
            json["error"].as_str().unwrap().starts_with("The item cannot be read: "),
            "{json}"
        );
    }
    assert_eq!(
        get_json(router, "/api/v1/items").await,
        json!({ "items": [], "outage": null })
    );
}

/// The summary of the first item's work in the list of items once this holds of it, the paused clock let on a quarter
/// of a second at a time; it fails where it does not hold within a minute.
async fn listed_until(router: &axum::Router, holds: impl Fn(&Value) -> bool) -> Value {
    for _ in 0..240 {
        let summary = get_json(router, "/api/v1/items").await["items"][0]["work"].clone();
        if holds(&summary) {
            return summary;
        }
        pass(250).await;
    }
    panic!("the work did not come to it within a minute");
}

// A work's summary gives the level its rounds run at, its item's, and the level of the cell it stands at, which its
// line raises above the item's, by the engine's one rule: here a reviewer's line one level up, an item at level 1. An
// engine cell's act runs on no level, and a work that ended stands at no cell.
#[tokio::test(start_paused = true)]
async fn a_works_summary_gives_its_level_and_that_of_the_cell_it_stands_at() {
    let trial = trial_flow();
    let raised = trial
        .flow
        .replacen("[lines.reviewer]\n", "[lines.reviewer]\nlevels_above = 1\n", 1);
    assert_ne!(raised, trial.flow, "the trial flow has the reviewer's line");
    let source = Source { flow: raised, ..trial };
    // The merge's act takes four seconds, so that the work is seen standing at it.
    let mut script = trial_script();
    let rounds = &mut script.flows.get_mut("trial").unwrap().rounds;
    rounds.iter_mut().find(|r| r.cell == "merge").unwrap().took.ms = Some(4000);
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("store.sqlite")).unwrap();
    let running = running_flow(Box::new(store), dir, Scripted::new(script).carriers(), &source);
    let router = &running.router;
    let item = json!({
        "title": "Greet the user",
        "level": "1",
        "dependsOn": [],
        "project": "trial",
        "flow": "trial",
        "writes": [{ "block": "request", "content": "Greet the user, please.", "value": null }],
    });
    assert_eq!(post(router, "/api/v1/items", &item).await.0, StatusCode::OK);
    let resumed = post(router, "/api/v1/works/1/acts", &json!({ "kind": "resume" })).await;
    assert_eq!(resumed, (StatusCode::NO_CONTENT, Value::Null));
    let levels = |s: &Value| (s["level"].clone(), s["cellLevel"].clone());
    let at = |s: &Value, cell: &str| s["now"]["cell"] == cell || s["now"]["at"] == cell;

    // The plan's question waits for the owner at the plan, whose line runs at the item's level.
    let asked = listed_until(router, |s| s["waits"].as_array().is_some_and(|w| !w.is_empty())).await;
    assert!(at(&asked, "plan"), "{asked}");
    assert_eq!(levels(&asked), (json!("1"), json!("1")));
    let answer = json!({ "kind": "result", "call": 2, "result": "Yes." });
    assert_eq!(
        post(router, "/api/v1/works/1/acts", &answer).await.0,
        StatusCode::NO_CONTENT
    );

    let reviewed = listed_until(router, |s| at(s, "review")).await;
    assert_eq!(levels(&reviewed), (json!("1"), json!("2")), "{reviewed}");
    let merging = listed_until(router, |s| at(s, "merge")).await;
    assert_eq!(levels(&merging), (json!("1"), Value::Null), "{merging}");
    let merged = listed_until(router, |s| s["now"]["kind"] == "merged").await;
    assert_eq!(levels(&merged), (json!("1"), Value::Null));
    // A work's page gives the same summary.
    let page = get_json(router, "/api/v1/works/1").await;
    assert_eq!(levels(&page["summary"]), (json!("1"), Value::Null));
}
