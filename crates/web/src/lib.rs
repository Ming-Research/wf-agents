//! The owner's pages: a JSON interface made for them, and the pages themselves, built into the binary.
//!
//! The owner reads the flow on this page and has no other way to see it (Article 7), so the interface gives the
//! definition as the page shows it: compiled, with every cell and block and who writes and reads each, or the faults
//! that keep it from compiling. The definition is read anew for every request, so that an edit of its files shows when
//! the page is loaded again.
//!
//! - `GET /api/v1/flow`: the definition shown ([`view::Definition`]).
//! - `GET /api/v1/flow/given`: what an agent is sent as the engine enters each agent's cell of the definition shown,
//!   part by part in the order sent ([`given::Given`]): the system prompt and the message as the engine makes them,
//!   every form of each part.
//! - With the engine running, the items and the works ([`interface`], whose types are in [`works`]), and what needs
//!   whoever plans the work since a mark ([`attention`]).
//! - `GET /api/v1/backup`: how the copies of the database stand ([`copies`]).
//! - The observations of the installation, and the owner's reading of each ([`observations`]).
//! - `GET /api/v1/system`: how the whole system went over a span of time ([`system`]).
//! - `GET` and `POST /api/v1/settings`, and its accounts: the installation's settings as their file writes them, and
//!   what the owner changes of them ([`settings`]), where the engine runs on settings.
//! - Any other path that is not under `/api/` is a file of the page, `index.html` when there is no such file.
//!
//! Wherever they are served, a request that could come from a page of another site is refused before it reaches a
//! route, by the module `guard`.

pub mod attention;
pub mod copies;
pub mod given;
mod guard;
pub mod interface;
pub mod observations;
pub mod settings;
pub mod system;
pub mod view;
pub mod works;

use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use axum::extract::State;
use axum::http::{HeaderValue, Method, StatusCode, Uri, header};
use axum::middleware;
use axum::response::{IntoResponse, Json, Response};
use axum::routing::{any, get};
use flow::{Compiled, Source};
use serde_json::json;

use crate::view::{Definition, Fault, Flow};

pub use crate::copies::Copies;

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
    /// What each definition is compiled against.
    pub has: Has,
    /// What every agent is sent alike, as the program that carries the rounds out has it.
    pub alike: given::Alike,
}

impl Flows {
    /// The definition in this directory, read with [`flow::read`], with what every agent is sent alike read from
    /// beside it: the directory that holds the flows is the one that holds this one. No project's checkout is known.
    pub fn dirs(shown: PathBuf, has: Has) -> Self {
        let alike = given::Alike {
            root: shown.parent().map(PathBuf::from),
            flow: Some(shown.clone()),
            project: None,
        };
        Flows {
            shown: Box::new(move || flow::read(&shown).map_err(|e| format!("{}: {e}", shown.display()))),
            has,
            alike,
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

    fn given(&self) -> given::Given {
        match self.compile(&self.shown).1 {
            Some(compiled) => given::given(&compiled, &self.alike),
            None => given::Given::NotCompiled,
        }
    }
}

/// The interface and the page, over these definitions, with the items and the works of the engine's loop, whose items
/// are added at these levels.
pub fn with_works(flows: Flows, handle: service::Handle, levels: service::Levels) -> Router {
    router(flows).merge(interface::router(handle, levels))
}

/// The same, with the route of how the copies of the database stand.
pub fn with_works_and_copies(flows: Flows, handle: service::Handle, levels: service::Levels, copies: Copies) -> Router {
    with_works(flows, handle.clone(), levels).merge(copies::router(handle, copies))
}

/// The interface and the page, over these definitions.
pub fn router(flows: Flows) -> Router {
    Router::new()
        .route("/api/v1/flow", get(definition))
        .route("/api/v1/flow/given", get(given))
        .route("/api/{*rest}", any(no_such))
        .fallback(page)
        .with_state(Arc::new(flows))
}

/// Serves the interface and the page on this listener, until the process ends. Every request passes the guard first,
/// whatever route it is for, so that no route has to remember it.
pub async fn serve(listener: tokio::net::TcpListener, router: Router) -> std::io::Result<()> {
    let own = guard::Own::at(listener.local_addr()?.port());
    axum::serve(
        listener,
        router.layer(middleware::from_fn_with_state(own, guard::guard)),
    )
    .await
}

async fn definition(State(flows): State<Arc<Flows>>) -> Response {
    off_the_server(move || flows.definition()).await
}

async fn given(State(flows): State<Arc<Flows>>) -> Response {
    off_the_server(move || flows.given()).await
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
