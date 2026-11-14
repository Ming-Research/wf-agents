//! The interface's routes, each asked as the page asks it: the flow's over small definitions written here, and the
//! works' over the engine's loop running the trial flow with its script (the service crate's `tests/trial`), the
//! runtime's time paused so that each round ends when the script says. The tests are kept in modules by subject, so
//! that a new test goes into the module of what it tests.

mod acts;
mod attention;
mod changing;
mod copies;
mod definition;
mod directories;
mod engine;
mod failing;
mod given;
mod items;
mod observations;
mod other_sites;
mod page;
mod sessions;
mod stopped;
mod stream;
mod system;
mod told;
mod works;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use flow::Source;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use service::{Scripted, Service, TokioClock};
use store::Store;
use tower::ServiceExt;
use web::Flows;

/// A small flow: a plan that people ask for and a review can send back, the review, which must say each time whether it
/// is content, and the merge, which waits for the owner's word.
pub const FLOW: &str = r#"
name = "tiny"
order = ["plan", "review", "merge"]

[limits]
runs = 3
returns = 2
calls = 40

[lines.engineer]

[blocks.request]
people = true

[blocks.plan]

[blocks.opinions]
people = true

[blocks.content]
values = ["yes", "no"]

[blocks.go]
values = ["yes", "no"]
people = true

[tests.may_merge]
says = "The owner let the work merge"
kind = "all"
reads = ["go"]
is = "yes"
"#;

pub const PLAN: &str = r#"
title = "Plan"
kind = "agent"
effort = "high"
line = "engineer"
task = "Write the plan from the request."
inputs = ["request", "opinions"]
outputs = ["plan"]
"#;

pub const REVIEW: &str = r#"
title = "Review"
kind = "agent"
effort = "high"
line = "engineer"
task = """
Review the plan.
Say whether it is content."""
limit = 5
inputs = ["plan"]
outputs = ["opinions", "content"]
must_write = ["content"]
"#;

pub const MERGE: &str = r#"
title = "Merge"
kind = "engine"
act = "merge"
requires = { test = "may_merge" }
inputs = ["content"]
"#;

/// The small flow's files.
pub fn tiny() -> Source {
    let mut source = Source {
        flow: FLOW.to_string(),
        ..Source::default()
    };
    for (id, text) in [("plan", PLAN), ("review", REVIEW), ("merge", MERGE)] {
        source.cells.insert(id.to_string(), text.to_string());
    }
    source
}

/// The small flow with `from` replaced by `to` in the file of this cell, or in `flow.toml` for `None`.
pub fn edit(source: Source, cell: Option<&str>, from: &str, to: &str) -> Source {
    let mut source = source;
    let text = match cell {
        None => &mut source.flow,
        Some(id) => source.cells.get_mut(id).expect("the small flow has the cell"),
    };
    assert!(text.contains(from), "{cell:?} has no {from:?}");
    *text = text.replacen(from, to, 1);
    source
}

/// A definition that is read as it stands when a request comes, so that a test can change it between requests.
#[derive(Clone)]
pub struct Shared(Arc<Mutex<Result<Source, String>>>);

impl Shared {
    pub fn new(source: Result<Source, String>) -> Self {
        Shared(Arc::new(Mutex::new(source)))
    }

    pub fn set(&self, source: Result<Source, String>) {
        *self.0.lock().unwrap() = source;
    }

    fn read(&self) -> web::Read {
        let shared = self.clone();
        Box::new(move || shared.0.lock().unwrap().clone())
    }
}

/// What the engine is taken to have for the small flows: one source outside a work, `main`, and two acts, `check` and
/// `merge`.
pub fn has(_: &Source) -> flow::Has {
    flow::Has {
        sources: BTreeSet::from(["main".to_string()]),
        acts: BTreeSet::from(["check".to_string(), "merge".to_string()]),
        ..Default::default()
    }
}

/// The interface over a definition shown, compiled against [`has`].
pub fn serve(shown: &Shared) -> Router {
    web::router(Flows {
        shown: shown.read(),
        has: Box::new(has),
        alike: web::given::Alike::default(),
    })
}

/// The answer to a request: its status, its content type, and its body.
pub async fn ask(router: &Router, method: Method, path: &str) -> (StatusCode, String, Vec<u8>) {
    let request = Request::builder().method(method).uri(path).body(Body::empty()).unwrap();
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .map(|v| v.to_str().unwrap().to_string())
        .unwrap_or_default();
    let body = response.into_body().collect().await.unwrap().to_bytes().to_vec();
    (status, content_type, body)
}

/// The JSON a GET of the interface answers with, which must succeed.
pub async fn get_json(router: &Router, path: &str) -> serde_json::Value {
    let (status, content_type, body) = ask(router, Method::GET, path).await;
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    assert_eq!(content_type, "application/json");
    serde_json::from_slice(&body).unwrap()
}

/// When the clock of the engine's loop starts: 2026-11-10, at midnight.
pub const START: u64 = 1_794_268_800_000;

/// The directory of the trial flow, `flow`, and its script, `script.toml`.
pub fn trial_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../service/tests/trial")
}

/// What the trial flow is compiled against: the driver's source outside a work, and the act `merge`.
pub(crate) fn trial_has(_: &Source) -> flow::Has {
    flow::Has {
        sources: BTreeSet::from([driver::DEPENDENCIES.to_string()]),
        acts: BTreeSet::from(["merge".to_string()]),
        ..Default::default()
    }
}

/// The interface over the engine's loop on the trial flow, with a store in a directory of its own.
pub struct Running {
    pub router: Router,
    _dir: tempfile::TempDir,
}

/// Starts the engine's loop on the trial flow with its script, and serves the interface over it.
pub fn running() -> Running {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("store.sqlite")).unwrap();
    running_on(Box::new(store), dir)
}

/// Starts the engine's loop on the trial flow with its script over this store, kept in this directory, and serves
/// the interface over it. A loop that stops shows in the answers of the interface.
pub fn running_on(store: Box<dyn store::Storage + Send>, dir: tempfile::TempDir) -> Running {
    running_scripted(store, dir, trial_script())
}

/// The trial flow's script.
pub fn trial_script() -> service::Script {
    service::Script::read(&std::fs::read_to_string(trial_dir().join("script.toml")).unwrap()).unwrap()
}

/// Starts the engine's loop on the trial flow with this script over this store, kept in this directory, and serves
/// the interface over it.
pub fn running_scripted(
    store: Box<dyn store::Storage + Send>,
    dir: tempfile::TempDir,
    script: service::Script,
) -> Running {
    running_carried(store, dir, Scripted::new(script).carriers())
}

/// Starts the engine's loop on the trial flow with these carriers over this store, kept in this directory, and serves
/// the interface over it.
pub fn running_carried(
    store: Box<dyn store::Storage + Send>,
    dir: tempfile::TempDir,
    carriers: service::Carriers,
) -> Running {
    let flow = flow::compile(
        &flow::read(&trial_dir().join("flow")).unwrap(),
        &trial_has(&Source::default()),
    )
    .unwrap();
    let clock = Arc::new(TokioClock::new(work::Time(START)));
    let (service, inbox) = Service::open(store, vec![flow], carriers, clock).unwrap();
    let handle = service.handle();
    tokio::spawn(service.run(inbox));
    let flows = Flows::dirs(trial_dir().join("flow"), Box::new(trial_has));
    // The observations are read from the store's file, as the engine's program serves them.
    let observations = web::observations::router(handle.clone(), dir.path().join("store.sqlite"));
    Running {
        router: web::with_works(flows, handle, levels(), projects()).merge(observations),
        _dir: dir,
    }
}

/// The levels an item is added at: `1` and `2`, the default, and `3`, above the highest an item may be at.
pub fn levels() -> service::Levels {
    let levels = [("1", "Easy work"), ("2", "Ordinary work"), ("3", "Hard work")].map(|(name, desc)| service::Level {
        name: name.to_string(),
        desc: desc.to_string(),
    });
    service::Levels::new(levels, "2", Some("2")).expect("the levels are made")
}

/// The projects an item is added to: `trial`, and `other`.
pub fn projects() -> service::Projects {
    service::Projects::new(["trial".to_string(), "other".to_string()])
}

/// The answer to a POST of this JSON: its status, and its body as JSON, or null where it has none.
pub async fn post(router: &Router, path: &str, body: &Value) -> (StatusCode, Value) {
    post_text(router, path, &body.to_string()).await
}

/// The answer to a POST of this text: its status, and its body as JSON, or null where it has none.
pub async fn post_text(router: &Router, path: &str, body: &str) -> (StatusCode, Value) {
    let request = Request::builder()
        .method(Method::POST)
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json = if body.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&body).unwrap()
    };
    (status, json)
}

/// The answer to a GET that must be refused: its status and its words.
pub async fn refused(router: &Router, path: &str) -> (StatusCode, String) {
    let (status, content_type, body) = ask(router, Method::GET, path).await;
    assert_eq!(content_type, "application/json");
    let json: Value = serde_json::from_slice(&body).unwrap();
    (status, json["error"].as_str().unwrap().to_string())
}

/// Lets this many milliseconds of the paused clock pass, the loop and the carriers doing what falls in them.
pub async fn pass(ms: u64) {
    tokio::time::sleep(Duration::from_millis(ms)).await;
}

/// The trial item, added held at the start and let on a second later: by 2.5 seconds its plan has asked the owner its
/// question (call 2), and the work waits for the answer.
pub async fn greeted(router: &Router) {
    let item = json!({
        "title": "Greet the user",
        "dependsOn": [],
        "project": "trial",
        "flow": "trial",
        "writes": [{ "block": "request", "content": "Greet the user, please.", "value": null }],
    });
    assert_eq!(
        post(router, "/api/v1/items", &item).await,
        (StatusCode::OK, json!({ "item": 1, "work": 1 }))
    );
    pass(1000).await;
    assert_eq!(
        post(router, "/api/v1/works/1/acts", &json!({ "kind": "resume" })).await,
        (StatusCode::NO_CONTENT, Value::Null)
    );
    pass(2000).await;
}
