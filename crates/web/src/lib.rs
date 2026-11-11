//! The owner's pages: a JSON interface made for them, and the pages themselves, built into the binary.
//!
//! The owner reviews every change to the flow on this page and has no other way to see it (Article 7), so the
//! interface gives the definition as the page shows it: compiled, with every cell and block and who writes and reads
//! each, or the faults that keep it from compiling; and, when there is a definition to compare against, what changed
//! from that one. Both definitions are read anew for every request, so that an edit of their files shows when the page
//! is loaded again.
//!
//! - `GET /api/v1/flow`: the definition shown ([`view::Definition`]).
//! - `GET /api/v1/flow/changes`: what changed from the definition compared against ([`view::Comparison`]).
//! - With the engine running, the items and the works ([`interface`], whose types are in [`works`]).
//! - Any other path that is not under `/api/` is a file of the page, `index.html` when there is no such file.

pub mod interface;
pub mod view;
pub mod works;

use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use axum::extract::State;
use axum::http::{HeaderValue, Method, StatusCode, Uri, header};
use axum::response::{IntoResponse, Json, Response};
use axum::routing::{any, get};
use flow::{Compiled, Source};
use serde_json::json;

use crate::view::{Change, Comparison, Definition, Fault, Flow};

mod built {
    include!(concat!(env!("OUT_DIR"), "/page.rs"));
}

/// Reads a definition's files, or says why they cannot be read.
pub type Read = Box<dyn Fn() -> Result<Source, String> + Send + Sync>;

/// What the engine has, by name, for a definition: the sources outside a work and the engine's own acts.
pub type Has = Box<dyn Fn(&Source) -> flow::Has + Send + Sync>;

/// Where the page's definitions come from.
pub struct Flows {
    /// The definition shown.
    pub shown: Read,
    /// The definition compared against, if any.
    pub against: Option<Read>,
    /// What each definition is compiled against.
    pub has: Has,
}

impl Flows {
    /// The definitions in these directories, read with [`flow::read`].
    pub fn dirs(shown: PathBuf, against: Option<PathBuf>, has: Has) -> Self {
        let reader = |dir: PathBuf| -> Read {
            Box::new(move || flow::read(&dir).map_err(|e| format!("{}: {e}", dir.display())))
        };
        Flows {
            shown: reader(shown),
            against: against.map(reader),
            has,
        }
    }

    /// A definition read and compiled: as the page shows it, and what it compiles to when it does.
    fn compile(&self, read: &Read) -> (Definition, Option<Compiled>) {
        let source = match read() {
            Ok(source) => source,
            Err(reason) => return (Definition::Unreadable { reason }, None),
        };
        match flow::compile(&source, &(self.has)(&source)) {
            Ok(compiled) => (
                Definition::Compiled {
                    flow: Flow::of(&compiled),
                },
                Some(compiled),
            ),
            Err(faults) => (
                Definition::Faults {
                    faults: faults.iter().map(Fault::of).collect(),
                },
                None,
            ),
        }
    }

    fn definition(&self) -> Definition {
        self.compile(&self.shown).0
    }

    fn comparison(&self) -> Comparison {
        let Some(against) = &self.against else {
            return Comparison::NothingToCompare;
        };
        let (view, before) = self.compile(against);
        let (_, after) = self.compile(&self.shown);
        let changes = match (&before, &after) {
            (Some(before), Some(after)) => Some(
                flow::compare(before, after)
                    .iter()
                    .map(|c| Change::of(c, before, after))
                    .collect(),
            ),
            _ => None,
        };
        Comparison::Compared { against: view, changes }
    }
}

/// The interface and the page, over these definitions, with the items and the works of the engine's loop, whose items
/// are added at these levels.
pub fn with_works(flows: Flows, handle: service::Handle, levels: service::Levels) -> Router {
    router(flows).merge(interface::router(handle, levels))
}

/// The interface and the page, over these definitions.
pub fn router(flows: Flows) -> Router {
    Router::new()
        .route("/api/v1/flow", get(definition))
        .route("/api/v1/flow/changes", get(changes))
        .route("/api/{*rest}", any(no_such))
        .fallback(page)
        .with_state(Arc::new(flows))
}

/// Serves the interface and the page on this listener, until the process ends.
pub async fn serve(listener: tokio::net::TcpListener, router: Router) -> std::io::Result<()> {
    axum::serve(listener, router).await
}

async fn definition(State(flows): State<Arc<Flows>>) -> Response {
    off_the_server(move || flows.definition()).await
}

async fn changes(State(flows): State<Arc<Flows>>) -> Response {
    off_the_server(move || flows.comparison()).await
}

/// Reads the disk on a thread of its own, off the threads that serve requests, and answers with what it made.
async fn off_the_server<T, F>(f: F) -> Response
where
    T: serde::Serialize + Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    match tokio::task::spawn_blocking(f).await {
        Ok(view) => Json(view).into_response(),
        Err(e) => fail(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("the definition could not be read: {e}"),
        ),
    }
}

async fn no_such(uri: Uri) -> Response {
    fail(StatusCode::NOT_FOUND, format!("no such path: {}", uri.path()))
}

fn fail(status: StatusCode, error: String) -> Response {
    (status, Json(json!({ "error": error }))).into_response()
}

/// The page's files. A path that is no file of the page is a view of the page's own, which `index.html` draws; a
/// missing asset is not.
async fn page(method: Method, uri: Uri) -> Response {
    if method != Method::GET && method != Method::HEAD {
        return fail(StatusCode::METHOD_NOT_ALLOWED, "the page is only read".to_string());
    }
    let path = uri.path().trim_start_matches('/');
    let find = |p: &str| built::FILES.iter().find(|(name, _)| *name == p).copied();
    let found = match find(path) {
        Some(file) => Some(file),
        None if path.starts_with("assets/") => None,
        None => find("index.html"),
    };
    let Some((name, bytes)) = found else {
        return fail(StatusCode::NOT_FOUND, format!("no such file: {path}"));
    };
    // Assets are named by their content, so they never change; the rest is checked each time.
    let cache = if name.starts_with("assets/") {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    };
    (
        [
            (header::CONTENT_TYPE, HeaderValue::from_static(content_type(name))),
            (header::CACHE_CONTROL, HeaderValue::from_static(cache)),
        ],
        bytes,
    )
        .into_response()
}

fn content_type(name: &str) -> &'static str {
    match name.rsplit_once('.').map(|(_, ext)| ext) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json" | "map") => "application/json",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("ico") => "image/x-icon",
        Some("woff2") => "font/woff2",
        Some("txt") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}
