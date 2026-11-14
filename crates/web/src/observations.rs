//! The observations of the installation, as the page's Observations view reads them, and the owner's reading of one,
//! as `client/src/observations-api.ts` asks for them:
//!
//! - `GET /api/v1/observations`: every observation, the newest first ([`ObservationsPage`]).
//! - `POST /api/v1/observations/{number}/read`: the owner read the observation: answers with when it was read
//!   ([`Read`]), which a reading after the first leaves as it was; 404 where there is no observation of the number.
//!
//! They are read, and a reading kept, through connections of their own to the store's file, beside the engine's loop
//! (decision `observations-read-beside-the-engines-turn`); the time of a reading is the one the loop's clock gives,
//! since the loop alone reads the clock.

use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use axum::extract::rejection::PathRejection;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use axum::routing::{get, post};
use serde::Serialize;
use service::Handle;
use store::Observations;

use crate::fail;

#[derive(Clone)]
struct Inner {
    handle: Handle,
    /// The store's file.
    store: Arc<PathBuf>,
}

/// The routes of the observations, over the store's file and the engine's loop, which gives the time.
pub fn router(handle: Handle, store: PathBuf) -> Router {
    Router::new()
        .route("/api/v1/observations", get(all))
        .route("/api/v1/observations/{number}/read", post(read))
        .with_state(Inner {
            handle,
            store: Arc::new(store),
        })
}

/// Every observation, the newest first.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ObservationsPage {
    pub observations: Vec<Observation>,
}

/// An observation: its number, what it says, where it came from, when it was kept and when the owner read it. Each
/// time is in milliseconds since the start of 1970.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Observation {
    pub number: u64,
    pub text: String,
    /// The work whose run's return carried it, and the work's item with its title as it stands.
    pub work: u64,
    pub item: u64,
    pub title: String,
    /// The cell, the run and the round of the run whose return carried it, the first round being one.
    pub cell: String,
    pub run: u64,
    pub round: usize,
    /// When it was kept: as its run finished.
    pub kept: u64,
    /// When the owner read it; none while it is unread.
    pub read: Option<u64>,
}

impl Observation {
    fn of(o: &store::Observation) -> Observation {
        Observation {
            number: o.number,
            text: o.text.clone(),
            work: o.work.0,
            item: o.item.0,
            title: o.title.clone(),
            cell: o.cell.clone(),
            run: o.run.0,
            round: o.round,
            kept: o.kept.0,
            read: o.read.map(|at| at.0),
        }
    }
}

/// When the owner read an observation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Read {
    pub read: u64,
}

async fn all(State(inner): State<Inner>) -> Response {
    let store = inner.store.clone();
    let found = tokio::task::spawn_blocking(move || {
        (Observations::open(store.as_path()))
            .and_then(|observations| observations.all())
            .map_err(|fault| service::store_fault_words(&fault))
    })
    .await;
    match found {
        Ok(Ok(all)) => Json(ObservationsPage {
            observations: all.iter().map(Observation::of).collect(),
        })
        .into_response(),
        Ok(Err(words)) => fail(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("The observations cannot be shown: {words}."),
        ),
        Err(e) => fail(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("The observations cannot be shown: {e}."),
        ),
    }
}

async fn read(State(inner): State<Inner>, path: Result<Path<u64>, PathRejection>) -> Response {
    let number = match path {
        Ok(Path(number)) => number,
        Err(rejection) => {
            return fail(
                StatusCode::BAD_REQUEST,
                format!("The path cannot be read: {}.", rejection.body_text()),
            );
        }
    };
    let Ok(now) = inner.handle.now().await else {
        return fail(StatusCode::SERVICE_UNAVAILABLE, "The engine has stopped.".to_string());
    };
    let store = inner.store.clone();
    let kept = tokio::task::spawn_blocking(move || {
        (Observations::open(store.as_path()))
            .and_then(|observations| observations.read(number, now))
            .map_err(|fault| service::store_fault_words(&fault))
    })
    .await;
    match kept {
        Ok(Ok(Some(at))) => Json(Read { read: at.0 }).into_response(),
        Ok(Ok(None)) => fail(StatusCode::NOT_FOUND, format!("There is no observation O{number}.")),
        Ok(Err(words)) => fail(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("The reading of O{number} cannot be kept: {words}."),
        ),
        Err(e) => fail(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("The reading of O{number} cannot be kept: {e}."),
        ),
    }
}
