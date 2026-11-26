//! How long `GET /api/v1/system` takes over a hundred merged works and five that have not ended, as the page reads it:
//! the first read of an engine, which works every work's part out, and the median of twenty-one reads after it, each in
//! a new engine over the same works. The time of each is taken in the test and given in the failure it ends in, which
//! the CI gives back as what the test said; the test is put into `crates/web/tests/routes/` of each commit measured,
//! beside a line `mod system_read;` in its `main.rs`, and is in no commit of the repository's history
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
