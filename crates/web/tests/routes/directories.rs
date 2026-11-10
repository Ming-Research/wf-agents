//! The small flow kept in `tests/flows/`, which the page is shown with: read from its directories as the command reads
//! them, so that it stays a definition that compiles and its earlier version one it is compared against.

use std::path::PathBuf;

use flow::Source;
use serde_json::json;

use crate::get_json;

fn dir(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/flows").join(name)
}

#[tokio::test]
async fn the_small_flow_compiles_and_what_changed_from_its_earlier_version_is_listed() {
    let router = web::router(web::Flows::dirs(
        dir("small"),
        Some(dir("small-earlier")),
        Box::new(|_: &Source| ["has_comment".into(), "plan_confirmed".into(), "may_merge".into()].into()),
    ));
    let flow = get_json(&router, "/api/v1/flow").await;
    assert_eq!(flow["state"], "compiled", "{flow}");
    let answer = get_json(&router, "/api/v1/flow/changes").await;
    assert_eq!(answer["against"]["state"], "compiled", "{answer}");
    assert_eq!(
        answer["changes"],
        json!([
            { "kind": "flowLimit", "limit": "runs", "before": 2, "after": 3 },
            { "kind": "testAdded", "test": "has_comment", "words": "The item has a comment" },
            { "kind": "testReworded", "test": "may_merge", "before": "The CI is green", "after": "The work may merge" },
            { "kind": "testAdded", "test": "plan_confirmed", "words": "The owner confirmed the plan" },
            { "kind": "title", "cell": "judge", "before": "Review the plan", "after": "Judge the plan" },
            { "kind": "outputAdded", "cell": "judge", "block": "opinions" },
            { "kind": "cellAdded", "cell": "answer", "place": 3 },
            { "kind": "inputAdded", "cell": "check", "block": "design" },
            { "kind": "blockAdded", "block": "comments" },
            { "kind": "shown", "block": "design", "before": true, "after": false },
            { "kind": "blockAdded", "block": "reply" },
        ])
    );
}
