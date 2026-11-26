//! How long `GET /api/v1/system` takes as the page reads it: the first read of an engine, which works every work's part
//! out, and the median of twenty-one reads after it, each in a new engine over the same works. The first test has a
//! hundred merged works and five that have not ended. The second adds ten works that have begun and are still going, each
//! with two hundred waits, and times a second series of reads, each after every work going on got a record. The time of
//! each is taken in the test and given in the failure it ends in, which the CI gives back as what the test said; the test
//! is put into `crates/web/tests/routes/` of each commit measured, beside a line `mod system_read;` in its `main.rs`, and
//! is in no commit of the repository's history (research/system-read-measured.md).

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

/// Works that have begun and not ended, each with many waits: they are worked out from the flow installed, as the work
/// still going on are, and again whenever one of them gets a record.
const LIVE: usize = 10;
/// Each cycle holds the work with the owner's pause and lets it on again, so that it has two waits and two moments:
/// a hundred cycles make the two hundred waits of the largest work in the owner's store (research/flow-versions/bar-cost.md).
const CYCLES: usize = 100;

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

/// The owner's act `kind` on each of the works, `from` to `to`.
async fn act_on(router: &axum::Router, from: usize, to: usize, act: Value) {
    for work in from..=to {
        let done = post(router, &format!("/api/v1/works/{work}/acts"), &act).await;
        assert_eq!(done, (StatusCode::NO_CONTENT, Value::Null));
    }
}

/// How long `GET /api/v1/system` takes over a hundred merged works, five that have not begun and ten that have begun and
/// are still going, each with two hundred waits: the first read of an engine; the median of twenty-one reads after it,
/// nothing having happened between them, so that every part stands as it was kept; and the median of twenty-one reads
/// each after every work going on got a record, so that the part of each is worked out again. The trial flow's plan is
/// sent back three times, so that a work let on and not let go on waits at it for the owner.
#[tokio::test(start_paused = true)]
async fn the_system_is_read_over_works_going_on_with_many_waits() {
    let (mut first, mut warm, mut again) = (Vec::new(), Vec::new(), Vec::new());
    for _ in 0..ENGINES {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path().join("store.sqlite")).unwrap();
        let mut script = trial_script();
        script.slots = Some(WORKS + LIVE);
        let running = running_scripted(Box::new(store), dir, script);
        let router = &running.router;
        add_works(router, 1, WORKS, true).await;
        // Each plan is sent back three times and reaches its limit, where its work waits for the owner at call 2: the owner
        // lets each go on, as soon as it is there, and it merges.
        for work in 1..=WORKS {
            let mut tries = 0;
            loop {
                let done = post(
                    router,
                    &format!("/api/v1/works/{work}/acts"),
                    &json!({ "kind": "goOn", "call": 2 }),
                )
                .await;
                if done == (StatusCode::NO_CONTENT, Value::Null) {
                    break;
                }
                tries += 1;
                assert!(
                    tries < 30,
                    "work {work} was let go on at the limit of its plan: {done:?}"
                );
                pass(2_000).await;
            }
        }
        pass(10 * 60_000).await;
        add_works(router, WORKS + 1, WORKS + WAITING, false).await;
        let (live_from, live_to) = (WORKS + WAITING + 1, WORKS + WAITING + LIVE);
        add_works(router, live_from, live_to, true).await;
        pass(60_000).await;
        for _ in 0..CYCLES {
            act_on(router, live_from, live_to, json!({ "kind": "pause" })).await;
            pass(1_000).await;
            act_on(router, live_from, live_to, json!({ "kind": "resume" })).await;
            pass(1_000).await;
        }
        let path = format!("/api/v1/system?weeks={},{},{}", START - 2 * WEEK, START - WEEK, START);
        let began = Instant::now();
        let system = get_json(router, &path).await;
        first.push(began.elapsed());
        assert_eq!(
            system["merged"].as_array().map(Vec::len),
            Some(WORKS),
            "every work but those going on merged"
        );
        // What the change counts: where it does, the pauses of the works going on are in it, so the read has the stretches
        // of each to work out; the code before the change has no such part.
        if let Some(by) = system["waited"]["by"].as_array() {
            let stops: u64 = by.iter().map(|by| by["stops"].as_u64().unwrap()).sum();
            assert!(
                stops >= (LIVE * CYCLES) as u64,
                "the works going on had their waits counted: {stops}"
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
            act_on(router, live_from, live_to, json!({ "kind": "pause" })).await;
            let began = Instant::now();
            get_json(router, &path).await;
            reads.push(began.elapsed());
            act_on(router, live_from, live_to, json!({ "kind": "resume" })).await;
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
