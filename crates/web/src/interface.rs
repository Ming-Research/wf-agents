//! The routes of the items and the works, as `client/src/works-api.ts` reads them. Each read and each act goes to the
//! engine's loop through its [`Handle`], which takes them in order with everything else that happens; what a round was
//! told and returned is read off the loop, from its carrier.
//!
//! A refusal answers with a status of 4xx and [`Refusal`]: 404 for a work, version, round or call there is none of,
//! 400 for what cannot be read, and 422 for what the engine refused. A work that cannot be shown, which the engine never
//! makes, and what was kept of a round that cannot be read answer 500, and an engine that stopped 503, each saying why.

use axum::Router;
use axum::body::Bytes;
use axum::extract::rejection::PathRejection;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Json, Response};
use axum::routing::{get, post};
use decide::RunId;
use driver::Failed;
use serde::Serialize;
use service::{Handle, Levels};
use store::{ItemNumber, WorkNumber};
use tokio::sync::broadcast::error::RecvError;

use crate::works::{
    Act, Added, Broken, CallPage, Changed, Items, LevelSet, LevelsPage, NewItem, NewLevel, Refusal, RoundPage,
    ToldPage, VersionPage, WorkPage,
};

/// The routes of the items and the works, over the engine's loop; an item is added at one of these levels.
pub fn router(handle: Handle, levels: Levels) -> Router {
    let levels = std::sync::Arc::new(levels);
    let shown = LevelsPage::of(&levels);
    let to_add = levels.clone();
    let add = move |handle: State<Handle>, body: Bytes| add(handle, to_add.clone(), body);
    let set = move |handle: State<Handle>, path: Result<Path<u64>, PathRejection>, body: Bytes| {
        set_level(handle, levels.clone(), path, body)
    };
    Router::new()
        .route("/api/v1/items", get(items).post(add))
        .route("/api/v1/items/{item}/level", post(set))
        .route("/api/v1/levels", get(move || async move { Json(shown.clone()) }))
        .route("/api/v1/works/{work}", get(work))
        .route("/api/v1/works/{work}/versions/{block}/{at}", get(version))
        .route("/api/v1/works/{work}/runs/{run}/rounds/{number}", get(round))
        .route("/api/v1/works/{work}/runs/{run}/rounds/{number}/told", get(told))
        .route("/api/v1/works/{work}/calls/{call}", get(call))
        .route("/api/v1/works/{work}/acts", post(act))
        .route("/api/v1/changes", get(changes))
        .with_state(handle)
}

async fn items(State(handle): State<Handle>) -> Response {
    match handle.read(Items::of).await {
        Ok(Ok(items)) => Json(items).into_response(),
        Ok(Err(broken)) => fail(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("The items cannot be shown: {}.", broken.0),
        ),
        Err(refusal) => refused(&refusal),
    }
}

async fn add(State(handle): State<Handle>, levels: std::sync::Arc<Levels>, body: Bytes) -> Response {
    let item: NewItem = match serde_json::from_slice(&body) {
        Ok(item) => item,
        Err(e) => return fail(StatusCode::BAD_REQUEST, format!("The item cannot be read: {e}.")),
    };
    let item = match item.taken(&levels) {
        Ok(item) => item,
        Err(why) => return fail(StatusCode::UNPROCESSABLE_ENTITY, format!("The item's level: {why}.")),
    };
    match handle.add(item).await {
        Ok((item, work)) => Json(Added {
            item: item.0,
            work: work.0,
        })
        .into_response(),
        Err(refusal) => refused(&refusal),
    }
}

/// Puts an item at another level, as the owner: one of the levels, at most the highest an item may be at.
async fn set_level(
    State(handle): State<Handle>,
    levels: std::sync::Arc<Levels>,
    path: Result<Path<u64>, PathRejection>,
    body: Bytes,
) -> Response {
    let item = match path {
        Ok(Path(item)) => ItemNumber(item),
        Err(rejection) => return unreadable(&rejection),
    };
    let asked: NewLevel = match serde_json::from_slice(&body) {
        Ok(asked) => asked,
        Err(e) => return fail(StatusCode::BAD_REQUEST, format!("The level cannot be read: {e}.")),
    };
    let level = match levels.of_item(Some(&asked.level)) {
        Ok(level) => level,
        Err(why) => return fail(StatusCode::UNPROCESSABLE_ENTITY, format!("The item's level: {why}.")),
    };
    match handle.set_level(item, &level).await {
        Ok(changed) => Json(LevelSet { changed }).into_response(),
        Err(refusal) => refused(&refusal),
    }
}

async fn work(State(handle): State<Handle>, path: Result<Path<u64>, PathRejection>) -> Response {
    let number = match path {
        Ok(Path(number)) => WorkNumber(number),
        Err(rejection) => return unreadable(&rejection),
    };
    let missing = format!("There is no work {}.", number.0);
    let page = handle
        .read(move |driver| driver.work(number).map(|_| WorkPage::of(driver, number)))
        .await;
    found(page, number, missing)
}

async fn version(State(handle): State<Handle>, path: Result<Path<(u64, String, u64)>, PathRejection>) -> Response {
    let (number, block, at) = match path {
        Ok(Path((number, block, at))) => (WorkNumber(number), block, at),
        Err(rejection) => return unreadable(&rejection),
    };
    let missing = format!("Work {} has no version of {block} at {at}.", number.0);
    let page = handle.read(move |driver| {
        driver
            .work(number)
            .map(|_| Ok(VersionPage::of(driver, number, &block, at)))
    });
    found(page.await, number, missing)
}

async fn round(State(handle): State<Handle>, path: Result<Path<(u64, u64, usize)>, PathRejection>) -> Response {
    let (number, run, round) = match path {
        Ok(Path((number, run, round))) => (WorkNumber(number), run, round),
        Err(rejection) => return unreadable(&rejection),
    };
    let missing = format!("Work {} has no round {round} of run {run}.", number.0);
    let page = handle.read(move |driver| driver.work(number).map(|_| RoundPage::of(driver, number, run, round)));
    found(page.await, number, missing)
}

async fn told(State(handle): State<Handle>, path: Result<Path<(u64, u64, usize)>, PathRejection>) -> Response {
    let (number, run, round) = match path {
        Ok(Path((number, run, round))) => (WorkNumber(number), run, round),
        Err(rejection) => return unreadable(&rejection),
    };
    let missing = format!("Work {} has no round {round} of run {run}.", number.0);
    let told = handle.told(number, RunId(run), round).await;
    let page = told.map(|work| work.map(|told| told.map(|round| round.map(ToldPage::of)).map_err(Broken)));
    found(page, number, missing)
}

async fn call(State(handle): State<Handle>, path: Result<Path<(u64, u64)>, PathRejection>) -> Response {
    let (number, call) = match path {
        Ok(Path((number, call))) => (WorkNumber(number), call),
        Err(rejection) => return unreadable(&rejection),
    };
    let missing = format!("Work {} has no call {call}.", number.0);
    let page = handle.read(move |driver| driver.work(number).map(|_| CallPage::of(driver, number, call)));
    found(page.await, number, missing)
}

async fn act(State(handle): State<Handle>, path: Result<Path<u64>, PathRejection>, body: Bytes) -> Response {
    let number = match path {
        Ok(Path(number)) => WorkNumber(number),
        Err(rejection) => return unreadable(&rejection),
    };
    let act: Act = match serde_json::from_slice(&body) {
        Ok(act) => act,
        Err(e) => return fail(StatusCode::BAD_REQUEST, format!("The act cannot be read: {e}.")),
    };
    match handle.act(number, act.taken()).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(refusal) => refused(&refusal),
    }
}

/// The stream of changes: one event as it opens, saying that every work and the items may have changed, so that a
/// page reads everything again each time it opens it; then one each time something happened. A page that falls
/// behind is told that everything may have changed.
async fn changes(State(handle): State<Handle>) -> Response {
    // Listened to before the works are read, so that nothing that happens between is missed.
    let listener = handle.changes();
    let first = match everything(&handle).await {
        Ok(first) => first,
        Err(refusal) => return refused(&refusal),
    };
    let stream = futures_util::stream::unfold((Some(first), listener, handle), |(first, mut listener, handle)| async {
        if let Some(first) = first {
            return Some((event(&first), (None, listener, handle)));
        }
        let changed = match listener.recv().await {
            Ok(changed) => Changed::of(&changed),
            Err(RecvError::Lagged(_)) => everything(&handle).await.ok()?,
            Err(RecvError::Closed) => return None,
        };
        Some((event(&changed), (None, listener, handle)))
    });
    Sse::new(stream).keep_alive(KeepAlive::default()).into_response()
}

/// That every work and the items may have changed.
async fn everything(handle: &Handle) -> Result<Changed, service::Refusal> {
    let works = handle
        .read(|driver| driver.works().map(|kept| kept.head.number.0).collect())
        .await?;
    Ok(Changed { works, items: true })
}

fn event(changed: &Changed) -> Result<Event, axum::Error> {
    Event::default().json_data(changed)
}

/// The answer to a read of a work or of something of it: what was read; or that there is no such work, or no such
/// thing of it, as `missing` says; or why it cannot be shown.
fn found<T: Serialize>(
    page: Result<Option<Result<Option<T>, Broken>>, service::Refusal>,
    number: WorkNumber,
    missing: String,
) -> Response {
    match page {
        Ok(Some(Ok(Some(page)))) => Json(page).into_response(),
        Ok(None) => fail(StatusCode::NOT_FOUND, format!("There is no work {}.", number.0)),
        Ok(Some(Ok(None))) => fail(StatusCode::NOT_FOUND, missing),
        Ok(Some(Err(broken))) => fail(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("It cannot be shown: {}.", broken.0),
        ),
        Err(refusal) => refused(&refusal),
    }
}

fn unreadable(rejection: &PathRejection) -> Response {
    fail(
        StatusCode::BAD_REQUEST,
        format!("The path cannot be read: {}.", rejection.body_text()),
    )
}

/// The answer to what the loop refused. Where the engine stopped as it was asked, the answer says whether what was
/// asked was kept.
fn refused(refusal: &service::Refusal) -> Response {
    let (status, kept) = match refusal {
        service::Refusal::Failed(Failed::NoSuchWork(_)) => (StatusCode::NOT_FOUND, None),
        service::Refusal::Failed(Failed::Stopped { kept, .. }) => (StatusCode::SERVICE_UNAVAILABLE, Some(*kept)),
        service::Refusal::Gone => (StatusCode::SERVICE_UNAVAILABLE, None),
        _ => (StatusCode::UNPROCESSABLE_ENTITY, None),
    };
    let error = refusal.to_string();
    (status, Json(Refusal { error, kept })).into_response()
}

fn fail(status: StatusCode, error: String) -> Response {
    (status, Json(Refusal { error, kept: None })).into_response()
}
