//! `POST /api/v1/works/{work}/acts`: what the owner does to a work, each as the engine takes it, or refused with why.

use axum::http::StatusCode;
use serde_json::{Value, json};
use store::Store;

use crate::{get_json, greeted, pass, post, post_text, running, running_scripted, trial_script};

const ACTS: &str = "/api/v1/works/1/acts";

#[tokio::test(start_paused = true)]
async fn each_act_is_taken_as_what_happened_to_the_work() {
    let running = running();
    let router = &running.router;
    greeted(router).await;
    let done = (StatusCode::NO_CONTENT, Value::Null);
    assert_eq!(post(router, ACTS, &json!({ "kind": "pause" })).await, done);
    assert_eq!(get_json(router, "/api/v1/works/1").await["summary"]["paused"], "owner");
    assert_eq!(post(router, ACTS, &json!({ "kind": "resume" })).await, done);
    let opinion = json!({ "kind": "write", "block": "opinions", "content": "Keep it short.", "value": null });
    assert_eq!(post(router, ACTS, &opinion).await, done);
    let page = get_json(router, "/api/v1/works/1").await;
    let kinds: Vec<&str> = (page["steps"].as_array().unwrap().iter())
        .map(|s| s["kind"].as_str().unwrap())
        .collect();
    assert_eq!(
        kinds,
        [
            "paused", "wrote", "wrote", "resumed", "run", "paused", "resumed", "wrote"
        ]
    );
    let opinions = &page["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["name"] == "opinions")
        .unwrap()["versions"];
    assert_eq!(opinions[0]["by"], json!({ "kind": "owner" }));

    assert_eq!(post(router, ACTS, &json!({ "kind": "cancel" })).await, done);
    assert_eq!(
        get_json(router, "/api/v1/works/1").await["summary"]["now"],
        json!({ "kind": "cancelled" })
    );
}

#[tokio::test(start_paused = true)]
async fn an_act_that_cannot_be_taken_is_refused_with_why() {
    let running = running();
    let router = &running.router;
    greeted(router).await;
    let error = |words: &str| json!({ "error": words });
    let unprocessable = StatusCode::UNPROCESSABLE_ENTITY;
    let answer = |call: u64| json!({ "kind": "result", "call": call, "result": "Yes." });
    let go_on = |call: u64| json!({ "kind": "goOn", "call": call });
    assert_eq!(
        post(router, "/api/v1/works/9/acts", &json!({ "kind": "pause" })).await,
        (StatusCode::NOT_FOUND, error("There is no work W9."))
    );
    assert_eq!(
        post(router, ACTS, &answer(9)).await,
        (unprocessable, error("The work has no call 9."))
    );
    let write = json!({ "kind": "write", "block": "code", "content": "fn main() {}", "value": null });
    assert_eq!(
        post(router, ACTS, &write).await,
        (unprocessable, error("People do not write the block code."))
    );
    assert_eq!(
        post(router, ACTS, &json!({ "kind": "letGo", "cell": "deploy" })).await,
        (unprocessable, error("The flow has no cell deploy."))
    );
    // The work waits for the owner's call at the plan's run, and at no cell past its limit: no cell is let go on.
    assert_eq!(
        post(router, ACTS, &json!({ "kind": "letGo", "cell": "build" })).await,
        (
            unprocessable,
            error(
                "The cell build is not at its limit: a cell is let go on only where the work waits at it for the \
                 owner past its limit."
            )
        )
    );
    for act in [
        r#"{ "kind": "jump" }"#,
        r#"{ "kind": "pause", "at": 3 }"#,
        r#"{ "kind": "write", "block": "opinions", "content": "Short.", "value": null, "for": "build" }"#,
    ] {
        let (status, json) = post_text(router, ACTS, act).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{act}");
        assert!(
            json["error"].as_str().unwrap().starts_with("The act cannot be read: "),
            "{json}"
        );
    }
    let (status, json) = post_text(router, ACTS, r#"{ "kind": "jump" }"#).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        json["error"].as_str().unwrap().starts_with("The act cannot be read: "),
        "{json}"
    );

    // The plan's run at its limit is let go on, and takes no words of a person as its result.
    assert_eq!(
        post(router, ACTS, &answer(2)).await,
        (
            unprocessable,
            error("Call 2 is a run at its limit: it is let go on and takes no words, and words are said to its cell.")
        )
    );
    assert_eq!(post(router, ACTS, &go_on(2)).await.0, StatusCode::NO_CONTENT);
    assert_eq!(
        post(router, ACTS, &go_on(2)).await,
        (unprocessable, error("Call 2 has its result already."))
    );
    // The plan hands in and the build asks for the CI: a tool's call, which no person answers and is no run at a limit.
    pass(4000).await;
    assert_eq!(
        post(router, ACTS, &answer(4)).await,
        (
            unprocessable,
            error("Call 4 is a tool's: the tool gives its result, not a person.")
        )
    );
    assert_eq!(
        post(router, ACTS, &go_on(4)).await,
        (
            unprocessable,
            error("Call 4 is no run at its limit, which is all that is let go on.")
        )
    );
}

/// The trial's script with the review objecting to each of the first four builds: the build goes round three times,
/// which is its limit, and the work waits at it for the owner.
fn objecting() -> service::Script {
    let mut script = trial_script();
    let rounds = &mut script.flows.get_mut("trial").unwrap().rounds;
    let objects = (rounds.iter())
        .find(|r| r.cell == "review" && r.run == 1)
        .unwrap()
        .clone();
    rounds.retain(|r| r.cell != "review" || r.run == 1);
    for run in 2..=4 {
        let mut again = objects.clone();
        again.run = run;
        for hand_in in again.hands_in.iter_mut().flatten() {
            if hand_in.block == "opinions" {
                hand_in.content = Some(format!("Not yet, as review {run} finds."));
            }
        }
        rounds.push(again);
    }
    script
}

// A cell the work waits at past its limit is let go on, as the owner's wait at it says, and the let-go is one of the
// work's steps.
#[tokio::test(start_paused = true)]
async fn a_cell_at_its_limit_is_let_go_on_and_shown_among_the_steps() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("store.sqlite")).unwrap();
    let running = running_scripted(Box::new(store), dir, objecting());
    let router = &running.router;
    greeted(router).await;
    let done = (StatusCode::NO_CONTENT, Value::Null);
    assert_eq!(post(router, ACTS, &json!({ "kind": "goOn", "call": 2 })).await, done);
    let let_go = json!({ "kind": "letGo", "cell": "build" });
    let waits_at_the_build =
        |page: &Value| (page["summary"]["waits"].as_array().unwrap().iter()).any(|w| w["ends"] == let_go);
    let mut waited = false;
    for _ in 0..240 {
        if waits_at_the_build(&get_json(router, "/api/v1/works/1").await) {
            waited = true;
            break;
        }
        pass(250).await;
    }
    assert!(
        waited,
        "the work did not come to wait at the build past its limit within a minute"
    );
    assert_eq!(post(router, ACTS, &let_go).await, done);
    let page = get_json(router, "/api/v1/works/1").await;
    assert!(!waits_at_the_build(&page), "{}", page["summary"]["waits"]);
    let kinds: Vec<&str> = (page["steps"].as_array().unwrap().iter())
        .map(|s| s["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds.iter().filter(|k| **k == "letGo").count(), 1, "{kinds:?}");
}

// What the owner says to a cell or a line, and the option the owner takes for a question, are acts of the route as the
// others are: each is read, handed to the engine with what it names, and refused in words that name it where nothing
// takes it. The trial flow has no question and no cell with words of its own, so each is refused here, and each refusal
// says the line, the cell or the question the act named. An answer is an option and nothing else, a reason with it is
// not an act.
#[tokio::test(start_paused = true)]
async fn words_said_and_an_answer_given_are_taken_as_acts_and_refused_naming_what_they_were_given_to() {
    let running = running();
    let router = &running.router;
    greeted(router).await;
    let unprocessable = StatusCode::UNPROCESSABLE_ENTITY;
    let say = |to: Value| json!({ "kind": "say", "to": to, "words": "Keep it short." });
    let refused = |words: &str| (unprocessable, json!({ "error": words }));
    assert_eq!(
        post(router, ACTS, &say(json!({ "kind": "line", "line": "drafter" }))).await,
        refused("The flow has no cell on the line drafter.")
    );
    assert_eq!(
        post(router, ACTS, &say(json!({ "kind": "cell", "cell": "deploy" }))).await,
        refused("The flow has no cell deploy.")
    );
    assert_eq!(
        post(router, ACTS, &say(json!({ "kind": "line", "line": "engineer" }))).await,
        refused("No cell of the line engineer has words of its own.")
    );
    assert_eq!(
        post(router, ACTS, &say(json!({ "kind": "cell", "cell": "build" }))).await,
        refused("The words would reach build, which has no words of its own.")
    );
    let answer = json!({ "kind": "answer", "question": "check", "version": 1, "option": 2 });
    assert_eq!(
        post(router, ACTS, &answer).await,
        refused("The work has no question check.")
    );
    let with_a_reason =
        json!({ "kind": "answer", "question": "check", "version": 1, "option": 2, "why": "It is short." });
    let (status, json) = post(router, ACTS, &with_a_reason).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{json}");
    assert!(
        json["error"].as_str().unwrap().starts_with("The act cannot be read: "),
        "{json}"
    );
}
