//! The routes of the items and the works, as `client/src/works-api.ts` reads them, and the reads of the plan that
//! whoever plans the work makes with the commands (module `plan`). Each read and each act goes to the engine's loop
//! through its [`Handle`], which takes them in order with everything else that happens; what a round was told and
//! returned is read off the loop, from its carrier.
//!
//! A refusal answers with a status of 4xx and [`Refusal`]: 404 for a work, version, round or call there is none of,
//! 400 for what cannot be read, and 422 for what the engine refused. A work that cannot be shown, which the engine never
//! makes, and what was kept of a round that cannot be read answer 500, and an engine that stopped 503, each saying why.

use axum::Router;
use axum::body::Bytes;
use axum::extract::rejection::{PathRejection, QueryRejection};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Json, Response};
use axum::routing::{get, post};
use decide::RunId;
use driver::Failed;
use serde::{Deserialize, Serialize};
use service::{Handle, Levels, Projects};
use store::WorkNumber;
use tokio::sync::broadcast::error::RecvError;

use crate::plan::{Candidates, Graph, HITS, Hits, PlanChanges, PlanMark, Tags, Way};
use crate::works::{
    Act, Added, Broken, CallPage, Changed, DoingPage, Engine, EngineAct, ItemChanges, Items, ItemsChanged, LevelsPage,
    NewItem, ProjectsPage, Refusal, RoundPage, SystemPage, ToldPage, VersionPage, WorkPage,
};

/// The routes of the items and the works, over the engine's loop; an item is added at one of these levels, to one of
/// these projects.
pub fn router(handle: Handle, levels: Levels, projects: Projects) -> Router {
    let levels = std::sync::Arc::new(levels);
    let shown = LevelsPage::of(&levels);
    let named = ProjectsPage::of(&projects);
    let (to_add, projects) = (levels.clone(), std::sync::Arc::new(projects));
    let add = move |handle: State<Handle>, body: Bytes| add(handle, to_add.clone(), projects.clone(), body);
    let change = move |handle: State<Handle>, body: Bytes| change(handle, levels.clone(), body);
    Router::new()
        .route("/api/v1/items", get(items).post(add))
        .route("/api/v1/items/changes", post(change))
        .route("/api/v1/plan/changes", get(plan_changes))
        .route("/api/v1/plan/candidates", get(candidates))
        .route("/api/v1/plan/search", get(search))
        .route("/api/v1/plan/graph/{item}", get(graph))
        .route("/api/v1/plan/tags", get(tags))
        .route("/api/v1/levels", get(move || async move { Json(shown.clone()) }))
        .route("/api/v1/projects", get(move || async move { Json(named.clone()) }))
        .route("/api/v1/works/{work}", get(work))
        .route("/api/v1/works/{work}/versions/{block}/{at}", get(version))
        .route("/api/v1/works/{work}/runs/{run}/rounds/{number}", get(round))
        .route("/api/v1/works/{work}/runs/{run}/rounds/{number}/told", get(told))
        .route("/api/v1/works/{work}/runs/{run}/rounds/{number}/doing", get(doing))
        .route(
            "/api/v1/works/{work}/lines/{line}/sessions/{session}/system",
            get(system),
        )
        .route("/api/v1/works/{work}/calls/{call}", get(call))
        .route("/api/v1/works/{work}/acts", post(act))
        .route("/api/v1/attention", get(crate::attention::attention))
        .route("/api/v1/system", get(crate::system::system))
        .route("/api/v1/engine", get(engine))
        .route("/api/v1/engine/acts", post(engine_act))
        .route("/api/v1/changes", get(changes))
        .with_state(handle)
}

async fn engine(State(handle): State<Handle>) -> Response {
    match handle.read(Engine::of).await {
        Ok(engine) => Json(engine).into_response(),
        Err(refusal) => refused(&refusal),
    }
}

async fn engine_act(State(handle): State<Handle>, body: Bytes) -> Response {
    let act: EngineAct = match serde_json::from_slice(&body) {
        Ok(act) => act,
        Err(e) => return fail(StatusCode::BAD_REQUEST, format!("The act cannot be read: {e}.")),
    };
    match handle.pause_everything(act.pauses()).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(refusal) => refused(&refusal),
    }
}

async fn items(State(handle): State<Handle>) -> Response {
    match handle.read_at(Items::of).await {
        Ok(Ok(items)) => Json(items).into_response(),
        Ok(Err(broken)) => fail(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("The items cannot be shown: {}.", broken.0),
        ),
        Err(refusal) => refused(&refusal),
    }
}

async fn add(
    State(handle): State<Handle>,
    levels: std::sync::Arc<Levels>,
    projects: std::sync::Arc<Projects>,
    body: Bytes,
) -> Response {
    let item: NewItem = match serde_json::from_slice(&body) {
        Ok(item) => item,
        Err(e) => return fail(StatusCode::BAD_REQUEST, format!("The item cannot be read: {e}.")),
    };
    let item = match item.taken(&levels, &projects) {
        Ok(item) => item,
        Err(why) => return fail(StatusCode::UNPROCESSABLE_ENTITY, format!("{why}.")),
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

/// Changes items, as whoever plans the work asks, for the one reason given, all of the changes or none: a level is
/// one of the levels, at most the highest an item may be at. Answers whether each changed its item, or why they were
/// refused, each change refused named by its place among them.
async fn change(State(handle): State<Handle>, levels: std::sync::Arc<Levels>, body: Bytes) -> Response {
    let asked: ItemChanges = match serde_json::from_slice(&body) {
        Ok(asked) => asked,
        Err(e) => return fail(StatusCode::BAD_REQUEST, format!("The changes cannot be read: {e}.")),
    };
    if asked.why.trim().is_empty() {
        return fail(
            StatusCode::UNPROCESSABLE_ENTITY,
            "A change says why it is made, and this one says nothing: give a reason in a sentence.".to_string(),
        );
    }
    if asked.changes.is_empty() {
        return fail(StatusCode::UNPROCESSABLE_ENTITY, "No change is asked for.".to_string());
    }
    let (mut changes, mut unlevelled) = (Vec::new(), Vec::new());
    let asked_alone = asked.changes.len() == 1;
    for (at, change) in asked.changes.into_iter().enumerate() {
        match change.taken(&levels) {
            Ok(change) => changes.push(change),
            Err(why) if asked_alone => {
                return fail(StatusCode::UNPROCESSABLE_ENTITY, format!("The item's level: {why}."));
            }
            Err(why) => unlevelled.push(format!("change {}: the item's level: {why}", at + 1)),
        }
    }
    if !unlevelled.is_empty() {
        let said = unlevelled.join("; ");
        return fail(
            StatusCode::UNPROCESSABLE_ENTITY,
            format!(
                "{}{}; so none of the changes was made.",
                said[..1].to_uppercase(),
                &said[1..]
            ),
        );
    }
    match handle.change(asked.why, changes).await {
        Ok(changed) => Json(ItemsChanged { changed }).into_response(),
        Err(refusal) => refused(&refusal),
    }
}

/// The query of a read of the plan's changes: the mark the last read gave, where there was one.
#[derive(Deserialize)]
struct Since {
    since: Option<String>,
}

async fn plan_changes(State(handle): State<Handle>, Query(query): Query<Since>) -> Response {
    let since = match query.since.as_deref().map(PlanMark::read).transpose() {
        Ok(since) => since,
        Err(unreadable) => return fail(StatusCode::BAD_REQUEST, format!("{}.", unreadable.0)),
    };
    match handle.read(move |driver| PlanChanges::of(driver, since.as_ref())).await {
        Ok(Ok(changes)) => Json(changes).into_response(),
        Ok(Err(unreadable)) => fail(StatusCode::UNPROCESSABLE_ENTITY, format!("{}.", unreadable.0)),
        Err(refusal) => refused(&refusal),
    }
}

async fn candidates(State(handle): State<Handle>) -> Response {
    match handle.read(Candidates::of).await {
        Ok(candidates) => Json(candidates).into_response(),
        Err(refusal) => refused(&refusal),
    }
}

async fn tags(State(handle): State<Handle>) -> Response {
    match handle.read(Tags::of).await {
        Ok(tags) => Json(tags).into_response(),
        Err(refusal) => refused(&refusal),
    }
}

/// The query of a search: the text, and the most hits to give.
#[derive(Deserialize)]
struct Searched {
    text: String,
    limit: Option<usize>,
}

async fn search(State(handle): State<Handle>, query: Result<Query<Searched>, QueryRejection>) -> Response {
    let Query(query) = match query {
        Ok(query) => query,
        Err(rejection) => {
            return fail(
                StatusCode::BAD_REQUEST,
                format!("The search cannot be read: {}.", rejection.body_text()),
            );
        }
    };
    if query.text.trim().is_empty() {
        return fail(
            StatusCode::UNPROCESSABLE_ENTITY,
            "A search looks for a text, and this one has none.".to_string(),
        );
    }
    let limit = query.limit.unwrap_or(HITS).max(1);
    match handle.read(move |driver| Hits::of(driver, &query.text, limit)).await {
        Ok(hits) => Json(hits).into_response(),
        Err(refusal) => refused(&refusal),
    }
}

/// The query of a walk of dependencies: which way, where only one, up to what the item depends on and down to what
/// depends on it, and how deep.
#[derive(Deserialize)]
struct Walked {
    way: Option<String>,
    depth: Option<usize>,
}

async fn graph(
    State(handle): State<Handle>,
    path: Result<Path<u64>, PathRejection>,
    Query(query): Query<Walked>,
) -> Response {
    let item = match path {
        Ok(Path(item)) => item,
        Err(rejection) => return unreadable(&rejection),
    };
    let way = match query.way.as_deref() {
        None => None,
        Some("down") => Some(Way::Down),
        Some("up") => Some(Way::Up),
        Some(other) => {
            return fail(
                StatusCode::BAD_REQUEST,
                format!("A walk goes up or down, and {other:?} is neither."),
            );
        }
    };
    let depth = query.depth.unwrap_or(3).max(1);
    match handle.read(move |driver| Graph::of(driver, item, way, depth)).await {
        Ok(Some(graph)) => Json(graph).into_response(),
        Ok(None) => fail(StatusCode::NOT_FOUND, format!("There is no item #{item}.")),
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
        .read_at(move |driver, now| driver.work(number).map(|_| WorkPage::of(driver, number, now)))
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

/// What the agent of a round was last seen doing, read off the loop from its carrier as [`told`] is: the page asks
/// again every few seconds while the round is under way, since the stream it is read from grows with nothing that
/// happens to the work.
async fn doing(State(handle): State<Handle>, path: Result<Path<(u64, u64, usize)>, PathRejection>) -> Response {
    let (number, run, round) = match path {
        Ok(Path((number, run, round))) => (WorkNumber(number), run, round),
        Err(rejection) => return unreadable(&rejection),
    };
    let missing = format!("Work {} has no round {round} of run {run}.", number.0);
    let doing = handle.doing(number, RunId(run), round).await;
    let page = doing.map(|work| work.map(|doing| doing.map(|round| round.map(DoingPage::of)).map_err(Broken)));
    found(page, number, missing)
}

/// The system prompt the line's session of this number, the line's first being none, began with, as its carrier kept
/// it.
async fn system(State(handle): State<Handle>, path: Result<Path<(u64, String, u32)>, PathRejection>) -> Response {
    let (number, line, session) = match path {
        Ok(Path((number, line, session))) => (WorkNumber(number), line, session),
        Err(rejection) => return unreadable(&rejection),
    };
    let missing = format!(
        "Work {} kept no system prompt of session {session} of {line}.",
        number.0
    );
    let system = handle.system(number, line.clone(), session).await;
    let page = system.map(|work| {
        work.map(|system| {
            system
                .map(|text| text.map(|text| SystemPage { line, session, text }))
                .map_err(Broken)
        })
    });
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
    if let Act::Cancel { why: Some(why) } = &act
        && why.trim().is_empty()
    {
        return fail(
            StatusCode::UNPROCESSABLE_ENTITY,
            "A cancel's reason says nothing: give one that says why, or none.".to_string(),
        );
    }
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
    Ok(Changed {
        works,
        items: true,
        copies: true,
    })
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
pub(crate) fn refused(refusal: &service::Refusal) -> Response {
    let (status, kept) = match refusal {
        service::Refusal::Failed(Failed::NoSuchWork(_)) => (StatusCode::NOT_FOUND, None),
        service::Refusal::Failed(Failed::Stopped { kept, .. }) => (StatusCode::SERVICE_UNAVAILABLE, Some(*kept)),
        service::Refusal::Gone => (StatusCode::SERVICE_UNAVAILABLE, None),
        _ => (StatusCode::UNPROCESSABLE_ENTITY, None),
    };
    let error = refusal.to_string();
    (status, Json(Refusal { error, kept })).into_response()
}

pub(crate) fn fail(status: StatusCode, error: String) -> Response {
    (status, Json(Refusal { error, kept: None })).into_response()
}
