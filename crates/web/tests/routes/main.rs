//! The interface's routes, each asked as the page asks it, over small definitions written here. The tests are kept in
//! modules by subject, so that a new test goes into the module of what it tests.

mod changes;
mod definition;
mod directories;
mod page;

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use flow::Source;
use http_body_util::BodyExt;
use tower::ServiceExt;
use web::Flows;

/// A small flow: a plan that people ask for and a review can send back, the review, and the merge, which waits for the
/// owner's word.
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
line = "engineer"
inputs = ["request", "opinions"]
outputs = ["plan"]
"#;

pub const REVIEW: &str = r#"
title = "Review"
kind = "agent"
line = "engineer"
limit = 5
inputs = ["plan"]
outputs = ["opinions", "content"]
"#;

pub const MERGE: &str = r#"
title = "Merge"
kind = "engine"
requires = ["may_merge"]
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

/// The interface over a definition shown and, when given, one compared against; the engine is taken to have one
/// source outside a work, `repository`.
pub fn serve(shown: &Shared, against: Option<&Shared>) -> Router {
    web::router(Flows {
        shown: shown.read(),
        against: against.map(Shared::read),
        sources: Box::new(|_: &Source| BTreeSet::from(["repository".to_string()])),
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
