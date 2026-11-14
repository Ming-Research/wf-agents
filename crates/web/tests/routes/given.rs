//! `GET /api/v1/flow/given`: what an agent is sent as the engine enters each agent's cell of the definition shown, part
//! by part, read from the definition and from what the engine says to every agent beside the flows.

use std::path::Path;

use serde_json::json;
use web::Flows;
use web::given::Alike;

use crate::{Shared, get_json, has, serve, tiny};

/// The interface over the small flow, with what every agent is sent read from the repository's own files beside its
/// flows.
fn beside_the_repository_s_flows() -> axum::Router {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow");
    web::router(Flows {
        shown: Shared::new(Ok(tiny())).read(),
        against: None,
        has: Box::new(has),
        alike: Alike {
            root: Some(root),
            flow: None,
            project: None,
        },
    })
}

#[tokio::test]
async fn where_the_page_was_not_told_where_the_flows_are_nothing_is_read() {
    let router = serve(&Shared::new(Ok(tiny())), None);
    assert_eq!(get_json(&router, "/api/v1/flow/given").await["state"], json!("unread"));
}

#[tokio::test]
async fn each_agent_s_cell_is_given_its_parts_in_the_order_sent_and_each_act_what_it_writes() {
    let given = get_json(&beside_the_repository_s_flows(), "/api/v1/flow/given").await;
    assert_eq!(given["state"], json!("compiled"), "{given}");
    let cells = given["cells"].as_object().expect("the cells");
    assert_eq!(cells.keys().collect::<Vec<_>>(), ["plan", "review"]);
    let plan = given["cells"]["plan"].as_array().expect("the plan's parts");
    // The program's own part, then the form, given with every call, then the system prompt and the message.
    assert_eq!(
        (
            &plan[0]["name"],
            &plan[0]["carried"],
            &plan[1]["name"],
            &plan[1]["carried"]
        ),
        (&json!("program"), &json!("program"), &json!("form"), &json!("call"))
    );
    let task = plan.iter().find(|p| p["what"] == json!("task")).expect("the task");
    assert_eq!(task["carried"], json!("message"));
    assert_eq!(given["tools"], json!(["query", "run_tests"]));
    // The merge is an act of the engine's: what it writes is read from its file beside the flows, key by key.
    assert_eq!(given["acts"]["merge"]["source"], json!("flow/agents/acts/merge.toml"));
    assert_eq!(given["acts"]["merge"]["words"][0][0], json!("text"));
}
