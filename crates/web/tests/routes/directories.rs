//! The small flow kept in `tests/flows/`, which the page is shown with: read from its directories as the command reads
//! them, so that it stays a definition that compiles and its earlier version one it is compared against.

use std::path::PathBuf;

use serde_json::json;

use crate::{get_json, has};

fn dir(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/flows").join(name)
}

#[tokio::test]
async fn the_small_flow_compiles_and_what_changed_from_its_earlier_version_is_listed() {
    let router = web::router(web::Flows::dirs(
        dir("small"),
        Some(dir("small-earlier")),
        Box::new(has),
    ));
    let flow = get_json(&router, "/api/v1/flow").await;
    assert_eq!(flow["state"], "compiled", "{flow}");
    let answer = get_json(&router, "/api/v1/flow/changes").await;
    assert_eq!(answer["against"]["state"], "compiled", "{answer}");
    let test = |name: &str, words: &str, kind: &str, reads: &str, is: &str| json!({ "name": name, "words": words, "kind": kind, "reads": [reads], "is": is });
    assert_eq!(
        answer["changes"],
        json!([
            { "kind": "flowLimit", "limit": "runs", "before": 2, "after": 3 },
            {
                "kind": "testAdded",
                "test": test("has_comment", "The item has a comment", "any", "commented", "yes"),
            },
            { "kind": "testReworded", "test": "may_merge", "before": "The CI is green", "after": "The work may merge" },
            {
                "kind": "testChanged",
                "test": "may_merge",
                "before": test("may_merge", "The CI is green", "all", "ci", "green"),
                "after": test("may_merge", "The work may merge", "none", "ci", "red"),
            },
            {
                "kind": "testAdded",
                "test": test("plan_confirmed", "The owner confirmed the plan", "all", "confirmed", "yes"),
            },
            { "kind": "title", "cell": "judge", "before": "Review the plan", "after": "Judge the plan" },
            {
                "kind": "task",
                "cell": "judge",
                "before": "Review the plan, and say whether it needs a design.",
                "after": "Judge the plan, and say whether it needs a design.",
            },
            { "kind": "outputAdded", "cell": "judge", "block": "opinions" },
            { "kind": "cellAdded", "cell": "answer", "place": 3 },
            { "kind": "inputAdded", "cell": "check", "block": "design" },
            { "kind": "blockAdded", "block": "commented" },
            { "kind": "blockAdded", "block": "comments" },
            { "kind": "blockAdded", "block": "confirmed" },
            { "kind": "shown", "block": "design", "before": true, "after": false },
            { "kind": "people", "block": "main", "before": true, "after": false },
            { "kind": "outside", "block": "main", "before": null, "after": "main" },
            { "kind": "blockAdded", "block": "reply" },
        ])
    );
}
