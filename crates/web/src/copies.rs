//! `GET /api/v1/backup`: how the copies of the database stand, for the System view. It is worked out from the records
//! of the copies and the time now, which the engine's loop gives: the loop alone reads the clock. An engine that keeps
//! no copies says so, rather than having no such path, which the page would take for a failure.

use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use axum::routing::get;
use service::Handle;

use crate::fail;

/// What the route is told of the copies.
#[derive(Clone, Debug)]
pub struct Copies {
    /// The store's file, whose records of the copies are read.
    pub store: PathBuf,
    /// How many copies are kept.
    pub keep: usize,
    /// Whether a place off the machine is given.
    pub off_machine: bool,
}

#[derive(Clone)]
struct Inner {
    handle: Handle,
    /// What is told of the copies, or none where the engine keeps none.
    copies: Option<Arc<Copies>>,
}

/// The route of the copies, over the engine's loop: of the copies the engine keeps, or of none.
pub fn router(handle: Handle, copies: Option<Copies>) -> Router {
    Router::new().route("/api/v1/backup", get(report)).with_state(Inner {
        handle,
        copies: copies.map(Arc::new),
    })
}

async fn report(State(inner): State<Inner>) -> Response {
    let Ok(now) = inner.handle.now().await else {
        return fail(StatusCode::SERVICE_UNAVAILABLE, "The engine has stopped.".to_string());
    };
    let Some(copies) = inner.copies.clone() else {
        return Json(backup::not_kept(now)).into_response();
    };
    let read = tokio::task::spawn_blocking(move || {
        let records = store::Backups::open(&copies.store)
            .and_then(|backups| backups.records())
            .map_err(|fault| service::store_fault_words(&fault))?;
        Ok::<_, String>(backup::report(&records, now, copies.off_machine, copies.keep))
    })
    .await;
    match read {
        Ok(Ok(report)) => Json(report).into_response(),
        Ok(Err(words)) => fail(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("The copies cannot be shown: {words}."),
        ),
        Err(e) => fail(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("The copies cannot be shown: {e}."),
        ),
    }
}
