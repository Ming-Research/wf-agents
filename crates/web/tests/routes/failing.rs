//! A round that fails, as the pages are given it: the work paused by the engine with the reason, the cell's round
//! failed, the round's end and the pause in its steps; and the work going on once the owner lets it on.

use axum::http::StatusCode;
use serde_json::{Value, json};
use store::Store;

use crate::{START, get_json, greeted, pass, post, running_scripted, trial_script};
use decide::run::Failure;

const WHY: &str = "The agent's program could not start.";

#[tokio::test(start_paused = true)]
async fn a_failed_round_and_the_engines_pause_are_given_with_the_reason() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("store.sqlite")).unwrap();
    let mut script = trial_script();
    let plan = script.flows.get_mut("trial").unwrap();
    let mut failing = plan.rounds[0].clone();
    failing.run = 2;
    failing.asks = None;
    failing.took = Default::default();
    failing.fails = Some(WHY.to_string());
    plan.rounds.push(failing);
    let running = running_scripted(Box::new(store), dir, script);
    let router = &running.router;
    greeted(router).await;
    let answer = json!({ "kind": "result", "call": 2, "result": "Yes, by name." });
    assert_eq!(
        post(router, "/api/v1/works/1/acts", &answer).await,
        (StatusCode::NO_CONTENT, Value::Null)
    );
    // The plan's second run, entered for the answer at 3 seconds, has its round fail a second and a half later.
    pass(2000).await;
    let failed_at = START + 4500;
    let page = get_json(router, "/api/v1/works/1").await;
    let paused = json!({ "kind": "paused", "by": "engine", "why": WHY });
    assert_eq!(page["summary"]["now"], paused);
    assert_eq!(page["summary"]["paused"], true);
    assert_eq!(
        page["summary"]["cells"][0],
        json!({ "cell": "plan", "state": "failed", "finished": 1, "record": 1, "open": 3 })
    );
    let steps = page["steps"].as_array().unwrap();
    let run = steps.iter().find(|s| s["kind"] == "run" && s["run"] == 3).unwrap();
    assert_eq!(run["end"], Value::Null);
    let rounds = run["rounds"].as_array().unwrap();
    assert_eq!(rounds.len(), 1);
    assert_eq!(
        rounds[0]["back"],
        json!({ "kind": "failed", "why": WHY, "failure": "other", "resets": null })
    );
    assert_eq!(rounds[0]["ended"], failed_at);
    assert_eq!(rounds[0]["spent"]["rounds"], 1);
    let pause = steps.iter().rev().find(|s| s["kind"] == "paused").unwrap();
    assert_eq!(pause["by"], "engine");
    assert_eq!(pause["why"], WHY);
    assert_eq!(pause["time"], failed_at);
    // The list of works says it as well.
    let items = get_json(router, "/api/v1/items").await;
    assert_eq!(items["items"][0]["work"]["now"], paused);
    // The round's own page has nothing handed in.
    assert_eq!(
        get_json(router, "/api/v1/works/1/runs/3/rounds/1").await["handed"],
        json!([])
    );

    // Let on, the plan's next round is given the answer again, and hands in the plan.
    assert_eq!(
        post(router, "/api/v1/works/1/acts", &json!({ "kind": "resume" })).await,
        (StatusCode::NO_CONTENT, Value::Null)
    );
    let page = get_json(router, "/api/v1/works/1").await;
    assert_eq!(page["summary"]["now"]["kind"], "round");
    assert_eq!(page["summary"]["now"]["round"], 2);
    assert_eq!(page["summary"]["cells"][0]["state"], "running");
    pass(1600).await;
    let round = get_json(router, "/api/v1/works/1/runs/3/rounds/2").await;
    assert_eq!(round["results"][0]["result"], "Yes, by name.");
    assert_eq!(round["handed"][0]["block"], "plan");
}

#[tokio::test(start_paused = true)]
async fn a_wait_every_work_shares_is_given_once_for_all_and_as_what_each_work_it_holds_waits_for() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("store.sqlite")).unwrap();
    let resets = START + 3_600_000;
    let mut script = trial_script();
    let plan = script.flows.get_mut("trial").unwrap();
    let mut failing = plan.rounds[0].clone();
    failing.run = 2;
    failing.asks = None;
    failing.took = Default::default();
    failing.fails = Some("You've hit your limit".to_string());
    failing.failure = Some(Failure::UsageLimit);
    failing.resets = Some(resets);
    plan.rounds.push(failing);
    let running = running_scripted(Box::new(store), dir, script);
    let router = &running.router;
    greeted(router).await;
    let answer = json!({ "kind": "result", "call": 2, "result": "Yes, by name." });
    post(router, "/api/v1/works/1/acts", &answer).await;
    pass(2000).await;
    let outage = json!({
        "failure": "usageLimit",
        "why": "You've hit your limit",
        "work": 1,
        "run": 3,
        "round": 1,
        "since": START + 4500,
        "until": { "kind": "at", "at": resets },
        "holds": true,
    });
    let items = get_json(router, "/api/v1/items").await;
    assert_eq!(items["outage"], outage);
    let page = get_json(router, "/api/v1/works/1").await;
    assert_eq!(
        page["summary"]["now"],
        json!({ "kind": "waits", "at": "plan", "on": [{ "kind": "agents", "outage": outage }] })
    );
    assert_eq!(page["summary"]["paused"], false);
    // Past the reset the wait no longer holds, though it stands until a round that began after it comes back: the
    // pages are not told it holds "until" a time gone by.
    pass(3_600_000 - 2000 + 100).await;
    let items = get_json(router, "/api/v1/items").await;
    assert!(
        items["outage"].is_null() || items["outage"]["holds"] == false,
        "{}",
        items["outage"]
    );
    let page = get_json(router, "/api/v1/works/1").await;
    let now = &page["summary"]["now"];
    let held = now["kind"] == "waits" && now["on"].as_array().unwrap().iter().any(|w| w["kind"] == "agents");
    assert!(!held, "{now}");
}
