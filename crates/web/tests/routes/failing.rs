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
    failing.round = 4;
    failing.asks = None;
    failing.took = Default::default();
    failing.fails = Some(WHY.to_string());
    plan.rounds.push(failing);
    let running = running_scripted(Box::new(store), dir, script);
    let router = &running.router;
    greeted(router).await;
    let answer = json!({ "kind": "goOn", "call": 2 });
    assert_eq!(
        post(router, "/api/v1/works/1/acts", &answer).await,
        (StatusCode::NO_CONTENT, Value::Null)
    );
    // The plan's fourth round, begun as the owner let its run go on at 6 seconds, fails a second and a half later.
    pass(2000).await;
    let failed_at = START + 7500;
    let page = get_json(router, "/api/v1/works/1").await;
    let reason = format!("The round failed. The cause: {WHY}");
    let paused = json!({ "kind": "paused", "at": "plan", "by": "engine", "ran": true, "why": reason });
    assert_eq!(page["summary"]["now"], paused);
    assert_eq!(page["summary"]["paused"], "engine");
    assert_eq!(
        page["summary"]["cells"][0],
        json!({ "cell": "plan", "state": "failed", "finished": 0, "record": null, "open": 1 })
    );
    let steps = page["steps"].as_array().unwrap();
    let run = steps.iter().find(|s| s["kind"] == "run" && s["run"] == 1).unwrap();
    assert_eq!(run["end"], Value::Null);
    let rounds = run["rounds"].as_array().unwrap();
    assert_eq!(rounds.len(), 4);
    assert_eq!(
        rounds[3]["back"],
        json!({ "kind": "failed", "why": WHY, "failure": "other", "resets": null })
    );
    assert_eq!(rounds[3]["ended"], failed_at);
    assert_eq!(rounds[3]["spent"]["rounds"], 1);
    let pause = steps.iter().rev().find(|s| s["kind"] == "paused").unwrap();
    assert_eq!(pause["by"], "engine");
    assert_eq!(pause["why"], reason);
    assert_eq!(pause["time"], failed_at);
    // The list of works says it as well.
    let items = get_json(router, "/api/v1/items").await;
    assert_eq!(items["items"][0]["work"]["now"], paused);
    // The round's own page has nothing handed in.
    assert_eq!(
        get_json(router, "/api/v1/works/1/runs/1/rounds/4").await["handed"],
        json!([])
    );

    // Let on, the plan's next round is given what the one that failed was given, and hands in the plan.
    assert_eq!(
        post(router, "/api/v1/works/1/acts", &json!({ "kind": "resume" })).await,
        (StatusCode::NO_CONTENT, Value::Null)
    );
    let page = get_json(router, "/api/v1/works/1").await;
    assert_eq!(page["summary"]["now"]["kind"], "round");
    assert_eq!(page["summary"]["now"]["round"], 5);
    assert_eq!(page["summary"]["cells"][0]["state"], "running");
    pass(1600).await;
    let round = get_json(router, "/api/v1/works/1/runs/1/rounds/5").await;
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
    failing.round = 4;
    failing.asks = None;
    failing.took = Default::default();
    failing.fails = Some("You've hit your limit".to_string());
    failing.failure = Some(Failure::UsageLimit);
    failing.resets = Some(resets);
    plan.rounds.push(failing);
    let running = running_scripted(Box::new(store), dir, script);
    let router = &running.router;
    greeted(router).await;
    let answer = json!({ "kind": "goOn", "call": 2 });
    post(router, "/api/v1/works/1/acts", &answer).await;
    pass(2000).await;
    let outage = json!({
        "service": "agents",
        "failure": "usageLimit",
        "why": "You've hit your limit",
        "work": 1,
        "run": 1,
        "round": 4,
        "since": START + 7500,
        "checked": null,
        "until": { "kind": "checked", "at": resets },
        "holds": true,
        "first": null,
        "mayMove": false,
    });
    let items = get_json(router, "/api/v1/items").await;
    assert_eq!(items["outage"], outage);
    // A usage limit is no command line that cannot run: no work waits on the owner for it.
    let waits = items["items"][0]["work"]["waits"].as_array().unwrap().clone();
    assert!(!waits.iter().any(|w| w["key"] == "agents"), "{waits:?}");
    let page = get_json(router, "/api/v1/works/1").await;
    assert_eq!(
        page["summary"]["now"],
        json!({ "kind": "waits", "at": "plan", "on": [{ "kind": "agents", "outage": outage }] })
    );
    assert_eq!(page["summary"]["paused"], Value::Null);
    // Past the reset the engine's check finds the outside well, and the wait no longer holds, though it stands until a
    // round that began after it comes back: the pages are not told it holds "until" a time gone by.
    pass(3_600_000 - 5000 + 100).await;
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

#[tokio::test(start_paused = true)]
async fn a_command_line_that_cannot_run_is_what_each_work_waits_on_the_owner_for_while_it_holds() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("store.sqlite")).unwrap();
    let mut script = trial_script();
    let plan = script.flows.get_mut("trial").unwrap();
    let mut failing = plan.rounds[0].clone();
    failing.round = 4;
    failing.asks = None;
    failing.took = Default::default();
    failing.fails = Some("no account is in use".to_string());
    failing.failure = Some(Failure::CannotRun);
    plan.rounds.push(failing);
    let running = running_scripted(Box::new(store), dir, script);
    let router = &running.router;
    greeted(router).await;
    let answer = json!({ "kind": "goOn", "call": 2 });
    post(router, "/api/v1/works/1/acts", &answer).await;
    // Within the half minute before the engine's first check of the outside, the wait holds.
    pass(5000).await;
    let items = get_json(router, "/api/v1/items").await;
    assert_eq!(items["outage"]["failure"], "cannotRun", "{}", items["outage"]);
    assert_eq!(items["outage"]["holds"], true);
    let work = &items["items"][0]["work"];
    let waits = work["waits"].as_array().unwrap();
    assert!(waits.iter().any(|w| w["key"] == "agents"), "{waits:?}");
    // The check finds the outside well: the wait no longer holds, and no work waits on the owner for it.
    pass(40_000).await;
    let items = get_json(router, "/api/v1/items").await;
    assert!(
        items["outage"].is_null() || items["outage"]["holds"] == false,
        "{}",
        items["outage"]
    );
    let waits = items["items"][0]["work"]["waits"].as_array().unwrap().clone();
    assert!(!waits.iter().any(|w| w["key"] == "agents"), "{waits:?}");
}
