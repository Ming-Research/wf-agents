//! How long `GET /api/v1/system` takes as the page reads it: the first read of an engine, which works every work's part
//! out, and the median of twenty-one reads after it, each in a new engine over the same works. The first test has a
//! hundred merged works and five that have not ended. The second has a hundred cancelled works, five not begun and ten
//! that have begun and are still going, each with two hundred waits, and times a second series of reads, each after every
//! work going on got a record. The time of each is taken in the test and given in the failure it ends in, which the CI
//! gives back as what the test said; the test is put into `crates/web/tests/routes/` of each commit measured, beside a
//! line `mod system_read;` in its `main.rs`, and is in no commit of the repository's history
//! (research/system-read-measured.md).

use std::time::{Duration, Instant};

use axum::http::StatusCode;
use serde_json::{Value, json};
use store::Store;

use crate::{START, get_json, pass, post, running_scripted, trial_script};

const WORKS: usize = 100;
/// Works added and not let on: they have not ended, so that what is read of them is worked out from the flow installed.
const WAITING: usize = 5;
const ENGINES: usize = 5;
const READS: usize = 21;
const WEEK: u64 = 7 * 24 * 60 * 60 * 1000;

#[tokio::test(start_paused = true)]
async fn the_system_is_read_over_a_hundred_merged_works() {
    let (mut first, mut median) = (Vec::new(), Vec::new());
    for _ in 0..ENGINES {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path().join("store.sqlite")).unwrap();
        let mut script = trial_script();
        script.flows.get_mut("trial").unwrap().rounds[0].asks = None;
        script.slots = Some(WORKS);
        let running = running_scripted(Box::new(store), dir, script);
        let router = &running.router;
        for work in 1..=WORKS {
            let item = json!({
                "title": format!("Greet the user {work}"),
                "dependsOn": [],
                "project": "trial",
                "flow": "trial",
                "writes": [{ "block": "request", "content": "Greet the user, please.", "value": null }],
            });
            assert_eq!(post(router, "/api/v1/items", &item).await.0, StatusCode::OK);
            let resumed = post(
                router,
                &format!("/api/v1/works/{work}/acts"),
                &json!({ "kind": "resume" }),
            )
            .await;
            assert_eq!(resumed, (StatusCode::NO_CONTENT, Value::Null));
        }
        for work in WORKS + 1..=WORKS + WAITING {
            let item = json!({
                "title": format!("Greet the user {work}"),
                "dependsOn": [],
                "project": "trial",
                "flow": "trial",
                "writes": [{ "block": "request", "content": "Greet the user, please.", "value": null }],
            });
            assert_eq!(post(router, "/api/v1/items", &item).await.0, StatusCode::OK);
        }
        pass(10 * 60_000).await;
        // The span cut into the weeks the page gives.
        let path = format!("/api/v1/system?weeks={},{},{}", START - 2 * WEEK, START - WEEK, START);
        let began = Instant::now();
        let system = get_json(router, &path).await;
        first.push(began.elapsed());
        assert_eq!(
            system["merged"].as_array().map(Vec::len),
            Some(WORKS),
            "every work merged"
        );
        let mut reads: Vec<Duration> = Vec::new();
        for _ in 0..READS {
            let began = Instant::now();
            get_json(router, &path).await;
            reads.push(began.elapsed());
        }
        reads.sort();
        median.push(reads[READS / 2]);
    }
    let ms = |times: &[Duration]| {
        times
            .iter()
            .map(|t| format!("{:.1}", t.as_secs_f64() * 1e3))
            .collect::<Vec<_>>()
    };
    panic!(
        "MEASURED first read, ms: {:?}; median of {READS} reads, ms: {:?}",
        ms(&first),
        ms(&median)
    );
}

/// Works that have begun and not ended, each with many waits: they are worked out from the flow installed, as every work
/// still going on is, and again whenever one of them gets a record.
const LIVE: usize = 10;
/// Each round of a live work fails and the engine holds the work for the owner to let it on, so that each cycle has one
/// wait, for the owner, and one round: two hundred waits are those of the largest work in the owner's store
/// (research/flow-versions/bar-cost.md).
const CYCLES: usize = 200;

async fn add_works(router: &axum::Router, from: usize, to: usize, resume: bool) {
    for work in from..=to {
        let item = json!({
            "title": format!("Greet the user {work}"),
            "dependsOn": [],
            "project": "trial",
            "flow": "trial",
            "writes": [{ "block": "request", "content": "Greet the user, please.", "value": null }],
        });
        assert_eq!(post(router, "/api/v1/items", &item).await.0, StatusCode::OK);
        if resume {
            let resumed = post(
                router,
                &format!("/api/v1/works/{work}/acts"),
                &json!({ "kind": "resume" }),
            )
            .await;
            assert_eq!(resumed, (StatusCode::NO_CONTENT, Value::Null));
        }
    }
}

/// The owner's act on each of the works, `from` to `to`.
async fn act_on(router: &axum::Router, from: usize, to: usize, act: Value) {
    for work in from..=to {
        let done = post(router, &format!("/api/v1/works/{work}/acts"), &act).await;
        assert_eq!(done, (StatusCode::NO_CONTENT, Value::Null));
    }
}

/// How long `GET /api/v1/system` takes over a hundred works that were cancelled, five that have not begun and ten that
/// have begun and are still going, each with two hundred waits: the first read of an engine; the median of twenty-one
/// reads after it, nothing having happened between them, so that every part stands as it was kept; and the median of
/// twenty-one reads each after every work going on got a record, so that the part of each is worked out again. Each of
/// the plan's rounds after the third fails, and the engine holds the work until the owner lets it on.
#[tokio::test(start_paused = true)]
async fn the_system_is_read_over_works_going_on_with_many_waits() {
    let (mut first, mut warm, mut again) = (Vec::new(), Vec::new(), Vec::new());
    for _ in 0..ENGINES {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path().join("store.sqlite")).unwrap();
        let mut script = trial_script();
        script.slots = Some(WORKS + LIVE);
        let plan = script.flows.get_mut("trial").unwrap();
        for round in 4..=4 + CYCLES + 2 * READS + 10 {
            let mut failing = plan.rounds[0].clone();
            failing.round = round;
            failing.asks = None;
            failing.took = Default::default();
            failing.fails = Some("The agent's program could not start.".to_string());
            plan.rounds.push(failing);
        }
        let running = running_scripted(Box::new(store), dir, script);
        let router = &running.router;
        add_works(router, 1, WORKS, true).await;
        pass(6_000).await;
        // The first hundred ended.
        act_on(router, 1, WORKS, json!({ "kind": "cancel" })).await;
        add_works(router, WORKS + 1, WORKS + WAITING, false).await;
        let (live_from, live_to) = (WORKS + WAITING + 1, WORKS + WAITING + LIVE);
        add_works(router, live_from, live_to, true).await;
        pass(6_000).await;
        // The plan of each is at its limit: the owner lets it go on, and its fourth round fails; then, again and again, the
        // owner lets it on and the round fails.
        for work in live_from..=live_to {
            let done = post(
                router,
                &format!("/api/v1/works/{work}/acts"),
                &json!({ "kind": "goOn", "call": 2 }),
            )
            .await;
            assert_eq!(done, (StatusCode::NO_CONTENT, Value::Null), "work {work}");
        }
        for _ in 0..CYCLES {
            pass(2_000).await;
            act_on(router, live_from, live_to, json!({ "kind": "resume" })).await;
        }
        pass(2_000).await;
        let path = format!("/api/v1/system?weeks={},{},{}", START - 2 * WEEK, START - WEEK, START);
        let began = Instant::now();
        let system = get_json(router, &path).await;
        first.push(began.elapsed());
        assert_eq!(
            system["cancelled"].as_array().map(Vec::len),
            Some(WORKS),
            "every work but those going on was cancelled"
        );
        // What the change counts: where it does, the waits of the works going on are in it, whatever cause each is counted
        // under, so the read has the stretches of each to work out; the code before the change has no such part.
        if let Some(by) = system["waited"]["by"].as_array() {
            let stops: u64 = by.iter().map(|by| by["stops"].as_u64().unwrap()).sum();
            assert!(
                stops >= (LIVE * CYCLES * 9 / 10) as u64,
                "the works going on had their waits counted: {by:?}"
            );
        }
        let mut reads: Vec<Duration> = Vec::new();
        for _ in 0..READS {
            let began = Instant::now();
            get_json(router, &path).await;
            reads.push(began.elapsed());
        }
        reads.sort();
        warm.push(reads[READS / 2]);
        let mut reads: Vec<Duration> = Vec::new();
        for _ in 0..READS {
            // Each work going on gets a record, as a record of it would come while the page is read.
            act_on(router, live_from, live_to, json!({ "kind": "resume" })).await;
            let began = Instant::now();
            get_json(router, &path).await;
            reads.push(began.elapsed());
            pass(2_000).await;
        }
        reads.sort();
        again.push(reads[READS / 2]);
    }
    let ms = |times: &[Duration]| {
        times
            .iter()
            .map(|t| format!("{:.1}", t.as_secs_f64() * 1e3))
            .collect::<Vec<_>>()
    };
    panic!(
        "MEASURED first read, ms: {:?}; median of {READS} reads with nothing changed, ms: {:?}; median of {READS} reads each \
         after the works going on got a record, ms: {:?}",
        ms(&first),
        ms(&warm),
        ms(&again)
    );
}
