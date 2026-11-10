//! `GET /api/v1/works/{work}/runs/{run}/rounds/{number}/told`: what a round was told and what came back of it, as
//! its carrier kept them.

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::http::StatusCode;
use decide::RunId;
use serde_json::json;
use service::{Returned, Round, RoundReply, Rounds, Scripted, Told, Try};
use store::{Store, WorkNumber};

use crate::{Running, get_json, greeted, pass, post, refused, running, running_carried, trial_script};

/// The script's agents, keeping for each round, by its work, run and number, what it is said here to have been told.
struct Keeping {
    agents: Arc<dyn Rounds>,
    kept: BTreeMap<(u64, u64, usize), Told>,
}

impl Rounds for Keeping {
    fn give(&self, round: Round, reply: RoundReply) {
        self.agents.give(round, reply);
    }

    fn stop(&self, round: Round, reply: RoundReply) {
        self.agents.stop(round, reply);
    }

    fn told(&self, work: WorkNumber, run: RunId, round: usize) -> Result<Told, String> {
        Ok(self.kept.get(&(work.0, run.0, round)).cloned().unwrap_or_default())
    }
}

/// The trial flow run with its script, its agents keeping what each round of `kept` was told.
fn keeping(kept: BTreeMap<(u64, u64, usize), Told>) -> Running {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("store.sqlite")).unwrap();
    let mut carriers = Scripted::new(trial_script()).carriers();
    carriers.agents = Arc::new(Keeping {
        agents: carriers.agents.clone(),
        kept,
    });
    running_carried(Box::new(store), dir, carriers)
}

fn tried(message: &str, returned: Option<Result<Returned, String>>) -> Try {
    Try {
        message: message.to_string(),
        returned,
    }
}

#[tokio::test(start_paused = true)]
async fn a_round_is_given_each_try_with_what_came_back() {
    let back = Returned {
        text: Some("I asked the owner.".to_string()),
        structured: Some("{\n  \"asks\": []\n}".to_string()),
        turns: Some(3),
        duration_ms: Some(1500),
        models: vec!["a-model".to_string()],
    };
    let told = Told {
        tries: vec![
            tried("# Task\n\nPlan.", Some(Err("the program printed nothing".to_string()))),
            tried("# Task\n\nPlan, again.", Some(Ok(back))),
        ],
    };
    let running = keeping(BTreeMap::from([((1, 1, 1), told)]));
    greeted(&running.router).await;
    assert_eq!(
        get_json(&running.router, "/api/v1/works/1/runs/1/rounds/1/told").await,
        json!({
            "tries": [
                {
                    "message": "# Task\n\nPlan.",
                    "returned": { "kind": "unreadable", "why": "the program printed nothing" },
                },
                {
                    "message": "# Task\n\nPlan, again.",
                    "returned": {
                        "kind": "read",
                        "text": "I asked the owner.",
                        "structured": "{\n  \"asks\": []\n}",
                        "turns": 3,
                        "durationMs": 1500,
                        "models": ["a-model"],
                    },
                },
            ],
        })
    );
}

#[tokio::test(start_paused = true)]
async fn a_round_under_way_is_given_its_message_and_nothing_back() {
    let told = Told {
        tries: vec![tried("The owner answered.", None)],
    };
    let running = keeping(BTreeMap::from([((1, 1, 2), told)]));
    let router = &running.router;
    greeted(router).await;
    let answer = json!({ "kind": "answer", "call": 2, "result": "Yes, by name." });
    assert_eq!(
        post(router, "/api/v1/works/1/acts", &answer).await.0,
        StatusCode::NO_CONTENT
    );
    // The plan's second round is given the answer, and is under way for a second and a half.
    pass(100).await;
    assert_eq!(
        get_json(router, "/api/v1/works/1").await["summary"]["now"]["kind"],
        "round"
    );
    assert_eq!(
        get_json(router, "/api/v1/works/1/runs/1/rounds/2/told").await,
        json!({ "tries": [{ "message": "The owner answered.", "returned": null }] })
    );
}

#[tokio::test(start_paused = true)]
async fn a_round_its_carrier_kept_nothing_of_is_given_no_try() {
    let running = running();
    greeted(&running.router).await;
    assert_eq!(
        get_json(&running.router, "/api/v1/works/1/runs/1/rounds/1/told").await,
        json!({ "tries": [] })
    );
}

#[tokio::test(start_paused = true)]
async fn a_round_there_is_none_of_is_refused_with_why() {
    let running = running();
    let router = &running.router;
    greeted(router).await;
    let none = StatusCode::NOT_FOUND;
    assert_eq!(
        refused(router, "/api/v1/works/9/runs/1/rounds/1/told").await,
        (none, "There is no work 9.".to_string())
    );
    assert_eq!(
        refused(router, "/api/v1/works/1/runs/1/rounds/7/told").await,
        (none, "Work 1 has no round 7 of run 1.".to_string())
    );
    assert_eq!(
        refused(router, "/api/v1/works/1/runs/5/rounds/1/told").await,
        (none, "Work 1 has no round 1 of run 5.".to_string())
    );
    let (status, words) = refused(router, "/api/v1/works/1/runs/1/rounds/one/told").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(words.starts_with("The path cannot be read: "), "{words}");
}
