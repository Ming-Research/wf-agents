//! `GET /api/v1/system`: how the system went over a span, worked out from the works' records: what was produced, what
//! was spent and on what, and the rounds that failed; the span cut into the weeks the page gives.

use axum::http::StatusCode;
use serde_json::{Value, json};
use store::Store;

use crate::{START, get_json, greeted, pass, post, refused, running_scripted, trial_script};

const DAY: u64 = 24 * 60 * 60 * 1000;

/// The trial item, its plan asking nothing, added and let on: within a minute it has merged.
async fn merged() -> crate::Running {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("store.sqlite")).unwrap();
    let mut script = trial_script();
    script.flows.get_mut("trial").unwrap().rounds[0].asks = None;
    let running = running_scripted(Box::new(store), dir, script);
    let router = &running.router;
    let item = json!({
        "title": "Greet the user",
        "dependsOn": [],
        "project": "trial",
        "flow": "trial",
        "writes": [{ "block": "request", "content": "Greet the user, please.", "value": null }],
    });
    assert_eq!(post(router, "/api/v1/items", &item).await.0, StatusCode::OK);
    let resumed = post(router, "/api/v1/works/1/acts", &json!({ "kind": "resume" })).await;
    assert_eq!(resumed, (StatusCode::NO_CONTENT, Value::Null));
    pass(60_000).await;
    assert_eq!(
        get_json(router, "/api/v1/works/1").await["summary"]["now"],
        json!({ "kind": "merged" })
    );
    running
}

#[tokio::test(start_paused = true)]
async fn all_time_gives_the_merge_and_what_was_spent_on_each_cell_and_model() {
    let running = merged().await;
    let router = &running.router;
    let system = get_json(router, "/api/v1/system").await;
    let work = get_json(router, "/api/v1/works/1").await;
    let spent = work["summary"]["spent"].clone();
    assert_eq!(system["from"], Value::Null);
    assert_eq!(system["spent"], spent, "every round ended in all time: {system}");
    assert_eq!(system["byWork"], json!([{ "work": 1, "item": 1, "spent": spent }]));
    let merged = &system["merged"].as_array().unwrap()[..];
    assert_eq!(merged.len(), 1, "{system}");
    assert_eq!(
        (merged[0]["work"].clone(), merged[0]["item"].clone()),
        (json!(1), json!(1))
    );
    assert_eq!(merged[0]["from"], work["summary"]["from"]);
    assert_eq!(merged[0]["spent"], spent);
    assert!(merged[0]["at"].as_u64().unwrap() > START, "{system}");
    assert_eq!(system["added"], json!([{ "work": 1, "item": 1, "at": START }]));
    assert_eq!(system["cancelled"], json!([]));
    assert_eq!(system["failed"], json!([]));
    let cells: Vec<&str> = (system["byCell"].as_array().unwrap().iter())
        .map(|c| c["cell"].as_str().unwrap())
        .collect();
    for cell in ["plan", "build", "review", "merge"] {
        assert!(cells.contains(&cell), "{cell} spent nothing: {system}");
    }
    let rounds = |list: &Value| -> u64 {
        (list.as_array().unwrap().iter())
            .map(|x| x["spent"]["rounds"].as_u64().unwrap())
            .sum()
    };
    assert_eq!(rounds(&system["byCell"]), spent["rounds"].as_u64().unwrap());
    assert_eq!(rounds(&system["byModel"]), spent["rounds"].as_u64().unwrap());
    // The merge is the engine's own act, its one round on no model, apart from the agents' rounds.
    let acts = (system["byModel"].as_array().unwrap().iter())
        .find(|m| m["on"]["kind"] == "act")
        .expect("the merge's round");
    assert_eq!(acts["spent"]["rounds"], 1, "{system}");
    assert_eq!(system["weeks"], json!([]), "all time is not cut into weeks");
    // 2026-11-10 is a Tuesday: everything happened in the week that began on the Monday before, and nothing in the
    // week before it.
    let (week, before) = (START - DAY, START - 8 * DAY);
    let weeks = get_json(router, &format!("/api/v1/system?weeks={week},{before}")).await;
    assert_eq!(weeks["from"], before, "the span begins with its earliest week");
    let nothing =
        json!({ "usdMicros": 0, "inputTokens": 0, "cachedTokens": 0, "outputTokens": 0, "workingMs": 0, "rounds": 0 });
    assert_eq!(
        weeks["weeks"],
        json!([
            {
                "start": week, "spent": spent, "pinged": nothing, "merged": 1, "cancelled": 0, "added": 1, "sentBack": 0,
                "failed": 0
            },
            {
                "start": before, "spent": nothing, "pinged": nothing, "merged": 0, "cancelled": 0, "added": 0,
                "sentBack": 0, "failed": 0
            },
        ])
    );
}

#[tokio::test(start_paused = true)]
async fn a_span_counts_only_what_happened_in_it() {
    let running = merged().await;
    let router = &running.router;
    let later = START + 120_000;
    let system = get_json(router, &format!("/api/v1/system?weeks={later}")).await;
    assert_eq!(system["from"], later);
    assert_eq!(system["spent"]["rounds"], 0, "{system}");
    assert_eq!(system["merged"], json!([]));
    assert_eq!(system["added"], json!([]));
    assert_eq!(system["byWork"], json!([]));
    assert_eq!(
        system["weeks"].as_array().unwrap().len(),
        1,
        "the span's one week, with nothing: {system}"
    );
    let (status, words) = refused(router, "/api/v1/system?weeks=1,yesterday").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(words.starts_with("The span cannot be read"), "{words}");
}

#[tokio::test(start_paused = true)]
async fn a_round_that_failed_is_given_with_its_cause() {
    const WHY: &str = "The agent's program could not start.";
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("store.sqlite")).unwrap();
    let mut script = trial_script();
    let plan = script.flows.get_mut("trial").unwrap();
    let mut failing = plan.rounds[0].clone();
    failing.run = 2;
    failing.asks = None;
    failing.fails = Some(WHY.to_string());
    plan.rounds.push(failing);
    let running = running_scripted(Box::new(store), dir, script);
    let router = &running.router;
    greeted(router).await;
    let answer = json!({ "kind": "result", "call": 2, "result": "Yes, by name." });
    assert_eq!(
        post(router, "/api/v1/works/1/acts", &answer).await.0,
        StatusCode::NO_CONTENT
    );
    pass(2000).await;
    let system = get_json(router, &format!("/api/v1/system?weeks={}", START - DAY)).await;
    let failed = &system["failed"].as_array().unwrap()[..];
    assert_eq!(failed.len(), 1, "{system}");
    assert_eq!(failed[0]["cell"], "plan");
    assert_eq!(failed[0]["failure"], "other");
    assert_eq!(failed[0]["why"], WHY);
    assert_eq!(system["weeks"][0]["failed"], 1);
}
