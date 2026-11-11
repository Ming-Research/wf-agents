//! `GET /api/v1/works/{work}/runs/{run}/rounds/{number}/told`: what a round was told and what came back of it, as
//! its carrier kept them.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier};

use axum::http::StatusCode;
use decide::RunId;
use serde_json::json;
use service::{Back, Returned, Round, RoundReply, Rounds, Scripted, Told, Try};
use store::{Store, WorkNumber};
use work::RanOn;

use crate::{Running, get_json, greeted, pass, post, refused, running, running_carried, trial_script};

/// The script's agents, keeping for each round, by its work, run and number, what it is said here to have been told.
struct Keeping {
    agents: Arc<dyn Rounds>,
    kept: BTreeMap<(u64, u64, usize), Told>,
    /// The number of the try of the first round that says what it ran on.
    attempt: usize,
}

impl Rounds for Keeping {
    fn give(&self, round: Round, reply: RoundReply) {
        // A try of the first round says what it ran on, as a real carrier does.
        if (round.work.0, round.run.0, round.number) == (1, 1, 1) {
            reply.teller().ran_on(
                self.attempt,
                RanOn {
                    level: "3".to_string(),
                    model: "claude-opus-5-5".to_string(),
                    effort: Some("high".to_string()),
                    account: None,
                },
            );
        }
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
    keeping_at(kept, 1)
}

/// As [`keeping`], the first round's try of this number saying what it ran on.
fn keeping_at(kept: BTreeMap<(u64, u64, usize), Told>, attempt: usize) -> Running {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("store.sqlite")).unwrap();
    let mut carriers = Scripted::new(trial_script()).carriers();
    carriers.agents = Arc::new(Keeping {
        agents: carriers.agents.clone(),
        kept,
        attempt,
    });
    running_carried(Box::new(store), dir, carriers)
}

fn tried(message: &str, back: Back) -> Try {
    Try {
        message: message.to_string(),
        cut: None,
        back,
        ran: None,
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
            Try {
                message: "# Task".to_string(),
                cut: Some(2_000_000),
                back: Back::Unreadable("it is not a result: EOF".to_string()),
                ran: None,
            },
            tried(
                "# Task\n\nPlan, again.",
                Back::Failed("API Error: 529 Overloaded".to_string()),
            ),
            tried("# Task\n\nPlan, a third time.", Back::Returned(back)),
        ],
        left_out: 2,
    };
    let running = keeping(BTreeMap::from([((1, 1, 1), told)]));
    greeted(&running.router).await;
    assert_eq!(
        get_json(&running.router, "/api/v1/works/1/runs/1/rounds/1/told").await,
        json!({
            "tries": [
                {
                    "message": "# Task",
                    "cut": 2_000_000,
                    "back": { "kind": "unreadable", "why": "it is not a result: EOF" },
                    "ran": { "level": "3", "model": "claude-opus-5-5", "effort": "high" },
                },
                {
                    "message": "# Task\n\nPlan, again.",
                    "cut": null,
                    "back": { "kind": "failed", "why": "API Error: 529 Overloaded" },
                    "ran": null,
                },
                {
                    "message": "# Task\n\nPlan, a third time.",
                    "cut": null,
                    "back": {
                        "kind": "returned",
                        "text": "I asked the owner.",
                        "structured": "{\n  \"asks\": []\n}",
                        "turns": 3,
                        "durationMs": 1500,
                        "models": ["a-model"],
                    },
                    "ran": null,
                },
            ],
            "leftOut": 2,
        })
    );
}

#[tokio::test(start_paused = true)]
async fn a_try_is_given_what_the_record_of_its_number_says_it_ran_on_or_else_what_its_carrier_kept() {
    // The first try was kept before tries were recorded, and its carrier read what it ran on from beside it; the
    // second is recorded.
    let before = RanOn {
        level: "2".to_string(),
        model: "claude-sonnet-5-5".to_string(),
        effort: None,
        account: None,
    };
    let told = Told {
        tries: vec![
            Try {
                ran: Some(before),
                ..tried("# Task", Back::Failed("API Error: 529 Overloaded".to_string()))
            },
            tried("# Task, again", Back::Nothing),
        ],
        left_out: 0,
    };
    let running = keeping_at(BTreeMap::from([((1, 1, 1), told)]), 2);
    greeted(&running.router).await;
    let got = get_json(&running.router, "/api/v1/works/1/runs/1/rounds/1/told").await;
    assert_eq!(
        got["tries"][0]["ran"],
        json!({ "level": "2", "model": "claude-sonnet-5-5", "effort": null })
    );
    assert_eq!(
        got["tries"][1]["ran"],
        json!({ "level": "3", "model": "claude-opus-5-5", "effort": "high" })
    );
}

#[tokio::test(start_paused = true)]
async fn a_round_under_way_is_given_its_message_and_nothing_back() {
    let told = Told {
        tries: vec![tried("The owner answered.", Back::Nothing)],
        left_out: 0,
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
        json!({
            "tries": [{ "message": "The owner answered.", "cut": null, "back": { "kind": "nothing" }, "ran": null }],
            "leftOut": 0,
        })
    );
}

/// The script's agents, whose `told` counts its calls and waits at the barrier before it answers.
struct Waiting {
    agents: Arc<dyn Rounds>,
    calls: Arc<AtomicUsize>,
    barrier: Arc<Barrier>,
}

impl Rounds for Waiting {
    fn give(&self, round: Round, reply: RoundReply) {
        self.agents.give(round, reply);
    }

    fn stop(&self, round: Round, reply: RoundReply) {
        self.agents.stop(round, reply);
    }

    fn told(&self, _: WorkNumber, _: RunId, _: usize) -> Result<Told, String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.barrier.wait();
        Ok(Told::default())
    }
}

#[tokio::test(start_paused = true)]
async fn what_a_round_was_told_is_read_off_the_loop_and_only_for_a_round_there_is() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("store.sqlite")).unwrap();
    let mut carriers = Scripted::new(trial_script()).carriers();
    let (calls, barrier) = (Arc::new(AtomicUsize::new(0)), Arc::new(Barrier::new(2)));
    carriers.agents = Arc::new(Waiting {
        agents: carriers.agents.clone(),
        calls: calls.clone(),
        barrier: barrier.clone(),
    });
    let running = running_carried(Box::new(store), dir, carriers);
    let router = running.router.clone();
    greeted(&router).await;
    let reading = tokio::spawn({
        let router = router.clone();
        async move { get_json(&router, "/api/v1/works/1/runs/1/rounds/1/told").await }
    });
    while calls.load(Ordering::SeqCst) == 0 {
        tokio::task::yield_now().await;
    }
    // While the carrier reads, the loop answers.
    assert_eq!(get_json(&router, "/api/v1/works/1").await["summary"]["number"], 1);
    // A round the records lack never reaches the carrier.
    assert_eq!(
        refused(&router, "/api/v1/works/1/runs/1/rounds/7/told").await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    barrier.wait();
    assert_eq!(reading.await.unwrap(), json!({ "tries": [], "leftOut": 0 }));
}

#[tokio::test(start_paused = true)]
async fn a_round_its_carrier_kept_nothing_of_is_given_no_try() {
    let running = running();
    greeted(&running.router).await;
    assert_eq!(
        get_json(&running.router, "/api/v1/works/1/runs/1/rounds/1/told").await,
        json!({ "tries": [], "leftOut": 0 })
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
