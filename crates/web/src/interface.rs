//! The routes of the items and the works, as `client/src/works-api.ts` reads them, and the reads of the plan that
//! whoever plans the work makes with the commands (module `plan`). Each read and each act goes to the engine's loop
//! through its [`Handle`], which takes them in order with everything else that happens; what a round was told and
//! returned, and what git says a work changed, are read off the loop, from their carriers.
//!
//! A refusal answers with a status of 4xx and [`Refusal`]: 404 for a work, version, round or call there is none of,
//! 400 for what cannot be read, and 422 for what the engine refused. A work that cannot be shown, which the engine never
//! makes, and what was kept of a round that cannot be read answer 500, and an engine that stopped 503, each saying why.

use std::collections::VecDeque;
use std::time::Duration;

use axum::Router;
use axum::body::Bytes;
use axum::extract::rejection::{PathRejection, QueryRejection};
use axum::extract::{Path, Query, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Json, Response};
use axum::routing::{get, post};
use decide::RunId;
use driver::Failed;
use serde::{Deserialize, Serialize};
use service::{Handle, Levels, Mark, Projects};
use store::WorkNumber;
use tokio::sync::broadcast::error::RecvError;

use crate::attention::{Times, Wanted};
use crate::items_cache::{Failure, ItemsCache};
use crate::plan::{Candidates, Graph, HITS, Hits, PlanChanges, PlanMark, Tags, Way};
use crate::pulls::Pulls;
use crate::works::{
    Act, Added, Broken, CallPage, Changed, ChangesPage, DiffPage, DoingPage, Done, Engine, EngineAct, ForkAsked,
    ForkPage, ItemChanges, ItemsChanged, LevelsPage, NewItem, ProjectsPage, Refusal, RoundPage, Said, SystemPage,
    ToldPage, VersionPage, WorkPage,
};

/// The routes of the items and the works, over the engine's loop; an item is added at one of these levels, to one of
/// these projects, and a work's rounds run at them; `wf-agents wait` goes by these times.
pub fn router(handle: Handle, levels: Levels, projects: Projects, times: Times) -> Router {
    let cache = std::sync::Arc::new(ItemsCache::new(handle.clone(), levels.clone()));
    router_over(cache, handle, levels, projects, times)
}

/// [`router`], reading the items through this cache, which the caller keeps to see what it has worked out.
pub fn router_over(
    cache: std::sync::Arc<ItemsCache>,
    handle: Handle,
    levels: Levels,
    projects: Projects,
    times: Times,
) -> Router {
    let levels = std::sync::Arc::new(levels);
    let shown = LevelsPage::of(&levels);
    let named = ProjectsPage::of(&projects);
    let (to_add, projects) = (levels.clone(), std::sync::Arc::new(projects));
    let to_show = levels.clone();
    let items = {
        let cache = cache.clone();
        move |query: Query<Since>| items(cache.clone(), query)
    };
    let pulls = {
        let (cache, projects) = (cache.clone(), projects.clone());
        move |handle: State<Handle>, query: Result<Query<Pulled>, QueryRejection>| {
            pulls(handle, cache.clone(), projects.clone(), query)
        }
    };
    let attention = {
        let cache = cache.clone();
        move |handle: State<Handle>, asked: Result<Query<Wanted>, QueryRejection>| {
            crate::attention::attention(handle, cache.clone(), times, asked)
        }
    };
    let work = move |handle: State<Handle>, path: Result<Path<u64>, PathRejection>| {
        work(handle, cache.clone(), to_show.clone(), path)
    };
    let add = move |handle: State<Handle>, body: Bytes| add(handle, to_add.clone(), projects.clone(), body);
    let change = move |handle: State<Handle>, body: Bytes| change(handle, levels.clone(), body);
    let parts = std::sync::Arc::new(std::sync::Mutex::new(crate::system::Parts::default()));
    let system_now = move |handle: State<Handle>, span: Result<Query<crate::system::Span>, QueryRejection>| {
        crate::system::system(handle, parts.clone(), span)
    };
    Router::new()
        .route("/api/v1/items", get(items).post(add))
        .route("/api/v1/items/changes", post(change))
        .route("/api/v1/plan/changes", get(plan_changes))
        .route("/api/v1/plan/candidates", get(candidates))
        .route("/api/v1/plan/search", get(search))
        .route("/api/v1/plan/graph/{item}", get(graph))
        .route("/api/v1/plan/tags", get(tags))
        .route("/api/v1/pulls", get(pulls))
        .route("/api/v1/levels", get(move || async move { Json(shown.clone()) }))
        .route("/api/v1/projects", get(move || async move { Json(named.clone()) }))
        .route("/api/v1/works/{work}", get(work))
        .route("/api/v1/works/{work}/versions/{block}/{version}", get(version))
        .route("/api/v1/works/{work}/runs/{run}/rounds/{number}", get(round))
        .route("/api/v1/works/{work}/runs/{run}/rounds/{number}/told", get(told))
        .route("/api/v1/works/{work}/runs/{run}/rounds/{number}/doing", get(doing))
        .route(
            "/api/v1/works/{work}/runs/{run}/rounds/{number}/doing/stream",
            get(following),
        )
        .route("/api/v1/works/{work}/changes", get(changes_of))
        .route("/api/v1/works/{work}/changes/diff", get(diff))
        .route(
            "/api/v1/works/{work}/lines/{line}/sessions/{session}/system",
            get(system),
        )
        .route("/api/v1/works/{work}/calls/{call}", get(call))
        .route("/api/v1/works/{work}/acts", post(act))
        .route("/api/v1/works/{work}/runs/{run}/forks", post(fork))
        .route("/api/v1/attention", get(attention))
        .route("/api/v1/system", get(system_now))
        .route("/api/v1/system/under-way", get(crate::system::under_way))
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
    let done = match act {
        EngineAct::Pause {} => handle.pause_everything(true).await,
        EngineAct::Resume {} => handle.pause_everything(false).await,
        EngineAct::KeepWarm { on } => handle.keep_warm(on).await,
        EngineAct::Stop { now } => handle.stop(now).await,
    };
    match done {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(refusal) => refused(&refusal),
    }
}

/// The items: all of them, or, where the page names the cursor it was last given, those that changed since.
async fn items(cache: std::sync::Arc<ItemsCache>, Query(query): Query<Since>) -> Response {
    match cache.read(query.since.as_deref()).await {
        Ok(body) => (
            [(header::CONTENT_TYPE, HeaderValue::from_static("application/json"))],
            body,
        )
            .into_response(),
        Err(failure) => failed(failure),
    }
}

/// The answer to what the items could not give.
fn failed(failure: Failure) -> Response {
    match failure {
        Failure::Refused(refusal) => refused(&refusal),
        Failure::Broken(broken) => fail(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("The items cannot be shown: {}.", broken.0),
        ),
    }
}

/// The query of the open pull requests: the one project they are asked of, where one is named.
#[derive(Deserialize)]
struct Pulled {
    project: Option<String>,
}

/// The pull requests open on the host of each project's repository, or of the one project asked, as each host says
/// them off the loop, each with the work it belongs to as the records say. A project whose host could not say is said
/// by itself, with why.
async fn pulls(
    State(handle): State<Handle>,
    cache: std::sync::Arc<ItemsCache>,
    projects: std::sync::Arc<Projects>,
    query: Result<Query<Pulled>, QueryRejection>,
) -> Response {
    let Query(query) = match query {
        Ok(query) => query,
        Err(rejection) => {
            return fail(
                StatusCode::BAD_REQUEST,
                format!("The pull requests cannot be asked for: {}.", rejection.body_text()),
            );
        }
    };
    let asked = match &query.project {
        Some(project) => match projects.of_item(project) {
            Ok(project) => vec![project],
            Err(why) => return fail(StatusCode::UNPROCESSABLE_ENTITY, format!("{why}.")),
        },
        None => projects.names().to_vec(),
    };
    let items = match cache.all().await {
        Ok(items) => items,
        Err(failure) => return failed(failure),
    };
    Json(Pulls::of(&items, handle.open_pulls(asked).await)).into_response()
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
                return fail(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    format!("{}{}.", why[..1].to_uppercase(), &why[1..]),
                );
            }
            Err(why) => unlevelled.push(format!("change {}: {}", at + 1, why)),
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
        Ok(None) => fail(
            StatusCode::NOT_FOUND,
            format!("There is no item {item}.", item = store::ItemNumber(item)),
        ),
        Err(refusal) => refused(&refusal),
    }
}

async fn work(
    State(handle): State<Handle>,
    cache: std::sync::Arc<ItemsCache>,
    levels: std::sync::Arc<Levels>,
    path: Result<Path<u64>, PathRejection>,
) -> Response {
    let number = match path {
        Ok(Path(number)) => WorkNumber(number),
        Err(rejection) => return unreadable(&rejection),
    };
    let missing = format!("There is no work {number}.");
    let checking = cache.clone();
    let page = handle
        .read_at(move |driver, now| {
            driver.work(number).map(|_| {
                let page = WorkPage::of(driver, &levels, number, now)?;
                if page.is_some() {
                    checking.check(driver, now)?;
                }
                Ok::<_, Broken>(page)
            })
        })
        .await;
    let page = match page {
        // The summary is the one the items give, so that the page and the list never say two things of a work.
        Ok(Some(Ok(Some(mut page)))) => {
            if let Some(summary) = cache.summary(page.summary.item).await {
                page.summary = summary;
            }
            Ok(Some(Ok(Some(page))))
        }
        Ok(Some(Ok(None))) => Ok(Some(Ok(None))),
        Ok(Some(Err(broken))) => Ok(Some(Err(broken))),
        Ok(None) => Ok(None),
        Err(refusal) => Err(refusal),
    };
    found(page, number, missing)
}

/// The content of a version of a block, the version by its number among the block's versions.
async fn version(State(handle): State<Handle>, path: Result<Path<(u64, String, u64)>, PathRejection>) -> Response {
    let (number, block, version) = match path {
        Ok(Path((number, block, version))) => (WorkNumber(number), block, version),
        Err(rejection) => return unreadable(&rejection),
    };
    let missing = format!("{number} has no version {version} of {block}.");
    let page = handle.read(move |driver| {
        driver
            .work(number)
            .map(|_| Ok(VersionPage::of(driver, number, &block, version)))
    });
    found(page.await, number, missing)
}

async fn round(State(handle): State<Handle>, path: Result<Path<(u64, u64, usize)>, PathRejection>) -> Response {
    let (number, run, round) = match path {
        Ok(Path((number, run, round))) => (WorkNumber(number), run, round),
        Err(rejection) => return unreadable(&rejection),
    };
    let missing = format!("{number} has no round {round} of run {run}.");
    let page = handle.read(move |driver| driver.work(number).map(|_| RoundPage::of(driver, number, run, round)));
    found(page.await, number, missing)
}

async fn told(State(handle): State<Handle>, path: Result<Path<(u64, u64, usize)>, PathRejection>) -> Response {
    let (number, run, round) = match path {
        Ok(Path((number, run, round))) => (WorkNumber(number), run, round),
        Err(rejection) => return unreadable(&rejection),
    };
    let missing = format!("{number} has no round {round} of run {run}.");
    let told = handle.told(number, RunId(run), round).await;
    let page = told.map(|work| work.map(|told| told.map(|round| round.map(ToldPage::of)).map_err(Broken)));
    found(page, number, missing)
}

/// What the agent of a round did from its start, every try in order, read off the loop from its carrier as [`told`]
/// is: the whole of a round's history, as against [`following`], which sends it as it comes.
async fn doing(State(handle): State<Handle>, path: Result<Path<(u64, u64, usize)>, PathRejection>) -> Response {
    let (number, run, round) = match path {
        Ok(Path((number, run, round))) => (WorkNumber(number), run, round),
        Err(rejection) => return unreadable(&rejection),
    };
    let missing = format!("{number} has no round {round} of run {run}.");
    let doing = handle.doing((number, RunId(run), round), Mark::default()).await;
    let page = doing.map(|work| {
        work.map(|doing| (doing.map(|round| round.map(|(doings, _)| DoingPage::of(doings.done)))).map_err(Broken))
    });
    found(page, number, missing)
}

/// How often a round followed is read again for what its agent did since: its program writes a line for each thing as
/// it does it, and a read that finds nothing new looks at the length of a file or two and asks the loop whether the
/// round has ended, which takes well under a millisecond, so a page sees each thing within half a second.
const FOLLOW: Duration = Duration::from_millis(500);

/// What the agent of a round does, as it comes, to the page that has the round open alone, as server-sent events: one
/// `done` for each thing, every try in order, from the round's start; then, once the round has ended and all it did was
/// sent, one `end`, and the stream ends. Where what was kept of the round cannot be read, one `failed` says why, and the
/// stream ends. The engine's stream of changes says nothing of it: the round's stream is followed, by a read every
/// [`FOLLOW`], only while a page has it open.
async fn following(State(handle): State<Handle>, path: Result<Path<(u64, u64, usize)>, PathRejection>) -> Response {
    let key = match path {
        Ok(Path((number, run, round))) => (WorkNumber(number), RunId(run), round),
        Err(rejection) => return unreadable(&rejection),
    };
    let (number, run, round) = key;
    let (doings, ended) = match handle.doing(key, Mark::default()).await {
        Ok(Some(Ok(Some(found)))) => found,
        Ok(None) => return fail(StatusCode::NOT_FOUND, format!("There is no work {number}.")),
        Ok(Some(Ok(None))) => {
            let missing = format!("{number} has no round {round} of run {}.", run.0);
            return fail(StatusCode::NOT_FOUND, missing);
        }
        Ok(Some(Err(why))) => {
            return fail(StatusCode::INTERNAL_SERVER_ERROR, format!("It cannot be shown: {why}."));
        }
        Err(refusal) => return refused(&refusal),
    };
    let mut first = Follow {
        handle,
        key,
        next: doings.next,
        queue: VecDeque::new(),
        over: false,
    };
    first.took(doings.done, ended);
    let stream = futures_util::stream::unfold(first, |mut follow| async move {
        loop {
            if let Some(event) = follow.queue.pop_front() {
                return Some((event, follow));
            }
            if follow.over {
                return None;
            }
            tokio::time::sleep(FOLLOW).await;
            match follow.handle.doing(follow.key, follow.next).await {
                Ok(Some(Ok(Some((doings, ended))))) => {
                    follow.next = doings.next;
                    follow.took(doings.done, ended);
                }
                Ok(Some(Err(why))) => follow.failed(why),
                Err(refusal) => follow.failed(refusal.to_string()),
                // The work and its round were there as the stream opened, and records are only added to.
                Ok(None | Some(Ok(None))) => follow.failed("the round is no longer there".to_string()),
            }
        }
    });
    Sse::new(stream).keep_alive(KeepAlive::default()).into_response()
}

/// A round followed, as [`following`] sends it: where the next read of it goes on from, and the events read and not
/// yet sent.
struct Follow {
    handle: Handle,
    key: (WorkNumber, RunId, usize),
    next: Mark,
    queue: VecDeque<Result<Event, axum::Error>>,
    /// Whether nothing more is to be read: the round ended and all it did was read, or a read failed.
    over: bool,
}

impl Follow {
    /// Takes what a read found the agent did, and, where the round had ended before it was read, the end.
    fn took(&mut self, done: Vec<(usize, service::Doing)>, ended: bool) {
        for done in done {
            self.queue
                .push_back(Event::default().event("done").json_data(Done::of(done)));
        }
        if ended {
            self.queue
                .push_back(Event::default().event("end").json_data(serde_json::json!({})));
            self.over = true;
        }
    }

    /// Takes why a read failed, after which nothing more is read.
    fn failed(&mut self, why: String) {
        let error = serde_json::json!({ "error": why });
        self.queue.push_back(Event::default().event("failed").json_data(error));
        self.over = true;
    }
}

/// The query of a read of one file's diff: the file's path, as the work's changes give it.
#[derive(Deserialize)]
struct Of {
    path: String,
}

/// Each file a work changed, as git in its project's repository says, worked out off the loop.
async fn changes_of(State(handle): State<Handle>, path: Result<Path<u64>, PathRejection>) -> Response {
    let number = match path {
        Ok(Path(number)) => WorkNumber(number),
        Err(rejection) => return unreadable(&rejection),
    };
    let missing = format!("{number} has no change to tell yet: its branch is not made.");
    let changes = handle.changes_of(number).await;
    let page = changes.map(|work| work.map(|changes| changes.map(|c| c.map(ChangesPage::of)).map_err(Broken)));
    found(page, number, missing)
}

/// The unified diff of one file a work changed, worked out off the loop.
async fn diff(
    State(handle): State<Handle>,
    path: Result<Path<u64>, PathRejection>,
    query: Result<Query<Of>, QueryRejection>,
) -> Response {
    let number = match path {
        Ok(Path(number)) => WorkNumber(number),
        Err(rejection) => return unreadable(&rejection),
    };
    let Query(Of { path: file }) = match query {
        Ok(query) => query,
        Err(rejection) => {
            return fail(
                StatusCode::BAD_REQUEST,
                format!("The file cannot be read: {}.", rejection.body_text()),
            );
        }
    };
    let missing = format!("{number} changed no file {file}.");
    let diff = handle.diff(number, file).await;
    let page = diff.map(|work| work.map(|diff| diff.map(|d| d.map(DiffPage::of)).map_err(Broken)));
    found(page, number, missing)
}

/// The system prompt the line's session of this number, the line's first being none, began with, as its carrier kept
/// it.
async fn system(State(handle): State<Handle>, path: Result<Path<(u64, String, u32)>, PathRejection>) -> Response {
    let (number, line, session) = match path {
        Ok(Path((number, line, session))) => (WorkNumber(number), line, session),
        Err(rejection) => return unreadable(&rejection),
    };
    let missing = format!("{number} kept no system prompt of session {session} of {line}.",);
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
    let missing = format!("{number} has no call {call}.");
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
        // Words said answer with the cell the engine gave them to; every other act with nothing.
        Ok(service::Acted { reached: Some(cell) }) => Json(Said {
            reached: cell.to_string(),
        })
        .into_response(),
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(refusal) => refused(&refusal),
    }
}

/// A fork of a round's session asked a question, which the engine's agents' carrier asks off the loop: what it
/// answered, or why it could not be asked.
async fn fork(State(handle): State<Handle>, path: Result<Path<(u64, u64)>, PathRejection>, body: Bytes) -> Response {
    let (number, run) = match path {
        Ok(Path((number, run))) => (WorkNumber(number), RunId(run)),
        Err(rejection) => return unreadable(&rejection),
    };
    let asked: ForkAsked = match serde_json::from_slice(&body) {
        Ok(asked) => asked,
        Err(e) => return fail(StatusCode::BAD_REQUEST, format!("The fork asked cannot be read: {e}.")),
    };
    if asked.question.trim().is_empty() {
        return fail(
            StatusCode::UNPROCESSABLE_ENTITY,
            "The question says nothing.".to_string(),
        );
    }
    match handle.fork(number, run, asked.round, asked.question).await {
        Ok(Some(Ok(forked))) => Json(ForkPage::of(forked)).into_response(),
        Ok(Some(Err(why))) => fail(StatusCode::UNPROCESSABLE_ENTITY, why),
        Ok(None) => fail(StatusCode::NOT_FOUND, format!("There is no work {number}.")),
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

/// That every work, the items, the copies, the accounts and the observations may have changed.
async fn everything(handle: &Handle) -> Result<Changed, service::Refusal> {
    let works = handle
        .read(|driver| driver.works().map(|kept| kept.head.number.0).collect())
        .await?;
    Ok(Changed {
        works,
        items: true,
        copies: true,
        accounts: true,
        observations: true,
        going: true,
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
        Ok(None) => fail(StatusCode::NOT_FOUND, format!("There is no work {number}.")),
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
