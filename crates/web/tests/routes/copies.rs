//! `GET /api/v1/backup`: how the copies of the database stand, from the records of the copies and the loop's time.

use std::sync::Arc;

use serde_json::json;
use service::{Scripted, Service, TokioClock};
use store::{Backups, Store};
use web::{Copies, Flows};

use crate::{START, get_json, levels, pass, trial_dir, trial_has, trial_script};

/// The interface over the engine's loop, with the route of the copies over the store's file.
fn serving(off_machine: bool, keep: usize) -> (axum::Router, Backups, tempfile::TempDir) {
    let (router, backups, dir, _handle) = serving_with_handle(off_machine, keep);
    (router, backups, dir)
}

/// The same, and the way into the loop.
pub(crate) fn serving_with_handle(
    off_machine: bool,
    keep: usize,
) -> (axum::Router, Backups, tempfile::TempDir, service::Handle) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.sqlite");
    let store = Store::open(&path).unwrap();
    let flow = flow::compile(
        &flow::read(&trial_dir().join("flow")).unwrap(),
        &trial_has(&flow::Source::default()),
    )
    .unwrap();
    let clock = Arc::new(TokioClock::new(work::Time(START)));
    let (service, inbox) = Service::open(
        Box::new(store),
        vec![flow],
        Scripted::new(trial_script()).carriers(),
        clock,
    )
    .unwrap();
    let handle = service.handle();
    tokio::spawn(service.run(inbox));
    let flows = Flows::dirs(trial_dir().join("flow"), None, Box::new(trial_has));
    let copies = Copies {
        store: path.clone(),
        keep,
        off_machine,
    };
    let router = web::with_works_and_copies(flows, handle.clone(), levels(), copies);
    (router, Backups::open(&path).unwrap(), dir, handle)
}

#[tokio::test(start_paused = true)]
async fn with_no_place_off_the_machine_the_copies_are_said_to_stay_on_it() {
    let (router, records, _dir) = serving(false, 7);
    let report = get_json(&router, "/api/v1/backup").await;
    assert_eq!(report["state"], json!({ "kind": "stale", "since": null }));
    assert_eq!(report["now"], START);
    records
        .taken(&backup::name_at(work::Time(START - 1000)), work::Time(START - 1000))
        .unwrap();
    let report = get_json(&router, "/api/v1/backup").await;
    assert_eq!(report["state"], json!({ "kind": "localOnly" }));
    assert_eq!(report["lastTaken"], START - 1000);
    assert_eq!(report["lastUploaded"], json!(null));
    assert_eq!(report["pending"], 0);
}

#[tokio::test(start_paused = true)]
async fn the_state_is_worked_out_with_the_time_of_the_loop_and_a_failure_is_given_with_its_reason() {
    let (router, records, _dir) = serving(true, 7);
    let taken = work::Time(START - 1000);
    let name = backup::name_at(taken);
    records.taken(&name, taken).unwrap();
    let report = get_json(&router, "/api/v1/backup").await;
    assert_eq!(report["state"], json!({ "kind": "pending", "n": 1 }));
    assert_eq!(report["pending"], 1);
    records
        .failed(work::Time(START - 500), &name, "upload", "the place cannot be reached")
        .unwrap();
    let report = get_json(&router, "/api/v1/backup").await;
    assert_eq!(
        report["state"],
        json!({ "kind": "failed", "at": START - 500, "step": "upload", "why": "the place cannot be reached" })
    );
    assert_eq!(report["lastFailure"]["step"], "upload");
    // An upload alone does not mend a failure: a step that ends with nothing failing does.
    records.uploaded(&name, work::Time(START - 100)).unwrap();
    let report = get_json(&router, "/api/v1/backup").await;
    assert_eq!(report["state"]["kind"], "failed");
    records.ended(work::Time(START - 100), 1).unwrap();
    let report = get_json(&router, "/api/v1/backup").await;
    assert_eq!(report["state"], json!({ "kind": "ok" }));
    assert_eq!(report["lastUploaded"], START - 100);
    // A day and an hour later by the loop's clock, the copy is too old.
    pass(25 * 60 * 60 * 1000 + 1).await;
    let report = get_json(&router, "/api/v1/backup").await;
    assert_eq!(report["state"], json!({ "kind": "stale", "since": START - 1000 }));
    assert_eq!(report["now"], START + 25 * 60 * 60 * 1000 + 1);
}
