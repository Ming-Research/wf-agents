//! How long `GET /api/v1/system` takes over many merged works, which the measurements `system-read` and
//! `system-read-build` (`measure/`) time: the first runs this test with its reads, the second without them, and the
//! time of the reads is the difference of the two. The test is not run with the project's tests, which have no use for
//! its time: it asserts only that the works merged, so that what is read is what is meant.

use axum::http::StatusCode;
use serde_json::{Value, json};
use store::Store;

use crate::{START, get_json, pass, post, running_scripted, trial_script};

/// How many works the system is read over, each with the trial plan's one round, merged.
const WORKS: usize = 100;
const WEEK: u64 = 7 * 24 * 60 * 60 * 1000;

#[tokio::test(start_paused = true)]
#[ignore = "timed by the measurements system-read and system-read-build, which give it how many reads in SYSTEM_READS"]
async fn the_system_is_read_over_a_hundred_merged_works() {
    let reads: usize = std::env::var("SYSTEM_READS")
        .ok()
        .and_then(|reads| reads.parse().ok())
        .unwrap_or(0);
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
    pass(10 * 60_000).await;
    // As the page reads it: the span cut into the weeks it gives. The first read works every work's part out, the others
    // find them kept.
    let path = format!("/api/v1/system?weeks={},{},{}", START - 2 * WEEK, START - WEEK, START);
    let system = get_json(router, &path).await;
    assert_eq!(
        system["merged"].as_array().map(Vec::len),
        Some(WORKS),
        "every work merged"
    );
    for _ in 0..reads {
        get_json(router, &path).await;
    }
}
