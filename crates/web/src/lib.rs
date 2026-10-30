//! The owner's interface as `wf-agents run` serves it, on its own threads beside the engine's loop: HTTP and JSON
//! under `/api/v1` (GET to read, POST for what the owner asks of the engine and for marking messages read), the event
//! stream at `/api/v1/events`, and the page built into the binary at every other path.
//!
//! It listens on the loopback address only, and refuses a request whose Host is not that address with its port (a
//! domain name rebound to the loopback address), whose Origin is another site's, or that the browser says came from
//! another site. Each read opens its own read-only connection to the database; marking messages read opens one that
//! writes as the owner.

use std::collections::BTreeMap;
use std::convert::Infallible;
use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::sync::Arc;

use api::{ApiError, Bus, Control, Envelope, Event, Flows, Sources};
use axum::extract::{DefaultBodyLimit, Path, Query, Request, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, Uri, header};
use axum::middleware::{self, Next};
use axum::response::sse::{self, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get, post};
use axum::{Json, Router};
use db::rusqlite::Connection;
use serde::Deserialize;
use tokio::sync::broadcast;
use tokio_stream::StreamExt as _;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;

mod built {
    include!(concat!(env!("OUT_DIR"), "/page.rs"));
}

/// The most of a kept file the interface hands out.
const FILE_MAX: u64 = 4 * 1024 * 1024;

/// The most a request that says or answers something may hold: ten images of 5 MB, a third larger as base64, and the
/// words; any other request keeps the server's own limit.
const SAID_MAX: usize = 72 * 1024 * 1024;

/// What the interface serves.
pub struct Options {
    /// The data directory, under which the files tools kept are found.
    pub data: PathBuf,
    /// The database.
    pub db: PathBuf,
    /// The port on the loopback address; 0 takes any free one.
    pub port: u16,
    pub bus: Arc<Bus>,
    pub control: Arc<Control>,
}

#[derive(Clone)]
struct App {
    data: Arc<PathBuf>,
    db: Arc<PathBuf>,
    flows: Arc<Flows>,
    /// The flows compiled at refs of the projects' checkouts.
    sources: Arc<Sources>,
    bus: Arc<Bus>,
    control: Arc<Control>,
    events: broadcast::Sender<Envelope>,
    /// The Host and Origin values a request may have.
    hosts: Arc<Vec<String>>,
    origins: Arc<Vec<String>>,
}

/// Starts serving on threads of its own, and gives the port it listens on. The port is taken before it returns, so
/// one already in use fails here.
pub fn start(o: Options) -> Result<u16, String> {
    let listener = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, o.port))
        .map_err(|e| format!("listening on 127.0.0.1:{}: {e}", o.port))?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let (events, _) = broadcast::channel(1024);
    let tx = events.clone();
    o.bus.listen(Box::new(move |e: &Envelope| {
        // No stream open is no failure.
        let _ = tx.send(e.clone());
    }));
    let app = App {
        data: Arc::new(o.data),
        db: Arc::new(o.db),
        flows: Arc::default(),
        sources: Arc::default(),
        bus: o.bus,
        control: o.control,
        events,
        hosts: Arc::new(vec![format!("127.0.0.1:{port}"), format!("localhost:{port}")]),
        origins: Arc::new(vec![
            format!("http://127.0.0.1:{port}"),
            format!("http://localhost:{port}"),
        ]),
    };
    let router = router(app);
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .thread_name("wf-agents-web")
        .enable_all()
        .build()
        .map_err(|e| format!("the interface's runtime: {e}"))?;
    std::thread::Builder::new()
        .name("wf-agents-web".into())
        .spawn(move || {
            rt.block_on(async move {
                let served = match tokio::net::TcpListener::from_std(listener) {
                    Ok(l) => axum::serve(l, router).await,
                    Err(e) => Err(e),
                };
                if let Err(e) = served {
                    eprintln!("the interface stopped: {e}");
                }
            });
        })
        .map_err(|e| format!("the interface's thread: {e}"))?;
    Ok(port)
}

fn router(app: App) -> Router {
    Router::new()
        .route("/api/v1/status", get(status))
        .route("/api/v1/engine/pause", post(pause_all))
        .route("/api/v1/engine/start", post(start_all))
        .route("/api/v1/engine/keep-warm", post(keep_warm))
        .route("/api/v1/events", get(events))
        .route("/api/v1/works", get(works))
        .route("/api/v1/works/{id}", get(work))
        .route("/api/v1/works/{id}/step", post(step))
        .route("/api/v1/works/{id}/start", post(start_work))
        .route("/api/v1/works/{id}/pause", post(pause))
        .route("/api/v1/works/{id}/start-over", post(start_over))
        .route(
            "/api/v1/works/{id}/say",
            post(say_to_work).layer(DefaultBodyLimit::max(SAID_MAX)),
        )
        .route("/api/v1/works/{id}/rounds/{n}", get(round))
        .route("/api/v1/works/{id}/contents/{hash}", get(content))
        .route("/api/v1/flows", get(flows))
        .route("/api/v1/flows/sources", get(flow_sources))
        .route("/api/v1/flows/at", get(flow_at))
        .route("/api/v1/flows/diff", get(flow_diff))
        .route("/api/v1/flows/{id}", get(flow))
        .route("/api/v1/flows/{id}/view", get(flow_view))
        .route("/api/v1/inputs-now", get(inputs_now))
        .route("/api/v1/items", get(items))
        .route("/api/v1/items/{id}", get(item))
        .route("/api/v1/architect", get(architect))
        .route("/api/v1/conversations", get(conversations))
        .route("/api/v1/conversation", get(conversation))
        .route("/api/v1/system", get(system))
        .route("/api/v1/settings", get(settings).post(save_settings))
        .route("/api/v1/settings/accounts", get(accounts).post(switch_account))
        .route("/api/v1/items/{id}/level", post(set_level))
        .route("/api/v1/messages/read", post(mark_read))
        .route("/api/v1/faults/{id}/forward", post(forward_fault))
        .route("/api/v1/faults/forward-new", post(forward_new))
        .route("/api/v1/conversations/read", post(read_to))
        .route(
            "/api/v1/projects/say",
            post(say_to_projects).layer(DefaultBodyLimit::max(SAID_MAX)),
        )
        .route(
            "/api/v1/asks/{id}/answer",
            post(answer).layer(DefaultBodyLimit::max(SAID_MAX)),
        )
        .route("/api/v1/design-cards/{id}/rule", post(rule))
        .route(
            "/api/v1/architect/say",
            post(say_to_architect).layer(DefaultBodyLimit::max(SAID_MAX)),
        )
        .route("/api/v1/architect/step", post(step_architect))
        .route("/api/v1/architect/start", post(start_architect))
        .route("/api/v1/architect/pause", post(pause_architect))
        .route("/api/v1/observations/{id}/close", post(close_observation))
        .route("/api/v1/files/{dir}/{name}", get(file))
        .route("/api/v1/attachments/{id}", get(attachment))
        .route("/api/{*rest}", any(no_such))
        .fallback(page)
        .layer(middleware::from_fn_with_state(app.clone(), guard))
        .with_state(app)
}

/// Why a request was refused or failed, as JSON.
struct Fail {
    status: StatusCode,
    error: String,
}

impl Fail {
    fn new(status: StatusCode, error: impl Into<String>) -> Fail {
        Fail {
            status,
            error: error.into(),
        }
    }
}

impl From<api::Error> for Fail {
    fn from(e: api::Error) -> Fail {
        match e {
            api::Error::NotFound(s) => Fail::new(StatusCode::NOT_FOUND, s),
            api::Error::Invalid(s) => Fail::new(StatusCode::UNPROCESSABLE_ENTITY, s),
            api::Error::Failed(s) => Fail::new(StatusCode::INTERNAL_SERVER_ERROR, s),
        }
    }
}

impl IntoResponse for Fail {
    fn into_response(self) -> Response {
        (self.status, Json(ApiError { error: self.error })).into_response()
    }
}

/// Refuses what could come from another site: a Host that is not the loopback address with this port, an Origin of
/// another site, an API request the browser says another site made, and a POST that is not JSON, which a page of
/// another site cannot send without asking first.
async fn guard(State(app): State<App>, req: Request, next: Next) -> Response {
    let headers = req.headers();
    let value = |h: &str| headers.get(h).and_then(|v| v.to_str().ok());
    let one_of = |v: &str, allowed: &[String]| allowed.iter().any(|a| a.eq_ignore_ascii_case(v));
    if !value(header::HOST.as_str()).is_some_and(|h| one_of(h, &app.hosts)) {
        return Fail::new(StatusCode::FORBIDDEN, "the Host is not this interface's address").into_response();
    }
    if headers.contains_key(header::ORIGIN) && !value(header::ORIGIN.as_str()).is_some_and(|o| one_of(o, &app.origins))
    {
        return Fail::new(StatusCode::FORBIDDEN, "the request comes from another site").into_response();
    }
    let api = req.uri().path().starts_with("/api/");
    if api && value("sec-fetch-site").is_some_and(|s| !matches!(s, "same-origin" | "none")) {
        return Fail::new(StatusCode::FORBIDDEN, "the request comes from another site").into_response();
    }
    let json = value(header::CONTENT_TYPE.as_str()).is_some_and(|t| t.starts_with("application/json"));
    if *req.method() == Method::POST && !json {
        return Fail::new(StatusCode::UNSUPPORTED_MEDIA_TYPE, "a POST carries JSON").into_response();
    }
    next.run(req).await
}

/// Runs a read on a connection of its own, off the threads that serve requests.
async fn read<T, F>(app: &App, f: F) -> Result<T, Fail>
where
    T: Send + 'static,
    F: FnOnce(&Connection, &App) -> Result<T, api::Error> + Send + 'static,
{
    let app = app.clone();
    tokio::task::spawn_blocking(move || {
        let conn = db::open_read(&app.db).map_err(|e| api::Error::Failed(format!("{}: {e}", app.db.display())))?;
        f(&conn, &app)
    })
    .await
    .map_err(|e| Fail::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .map_err(Fail::from)
}

async fn status(State(app): State<App>) -> Result<Json<api::Status>, Fail> {
    app.bus.status().map(|s| Json(app.control.status(s))).ok_or_else(|| {
        Fail::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "the engine has not said how it stands yet",
        )
    })
}

/// Tells every open page how the engine stands now that the owner asked something of it: before the engine's loop
/// does, which it does only between two steps.
fn tell_status(app: &App) {
    if let Some(s) = app.bus.status() {
        app.bus.publish(Event::Status {
            status: Box::new(app.control.status(s)),
        });
    }
}

/// Pauses every work: a round under way ends first, and nothing starts after it. The works and the architect the owner
/// started follow the global pause again; those the owner paused stay paused.
async fn pause_all(State(app): State<App>) -> StatusCode {
    app.control.set_paused(true);
    tell_status(&app);
    StatusCode::NO_CONTENT
}

/// Lets every work go on that follows the global state, and the engine starts works on ready items as slots free; a
/// work the owner paused, or the engine or the architect stopped, stays paused.
async fn start_all(State(app): State<App>) -> Json<api::Done> {
    app.control.set_paused(false);
    tell_status(&app);
    Json(api::Done {
        done: "every work goes on but those paused by themselves".into(),
    })
}

/// Keeps the sessions of waiting works warm while every work is paused, or not, as the owner's switch says: written as
/// the owner, so that it holds across restarts.
async fn keep_warm(State(app): State<App>, Json(k): Json<api::KeepWarm>) -> Result<Json<api::Done>, Fail> {
    let on = k.on;
    let done = decide(&app, move |db| api::set_keep_warm_paused(db, on)).await?;
    app.control.set_keep_warm(on);
    tell_status(&app);
    Ok(done)
}

/// Runs one round of one work, whatever the global state, resuming its line's session or in a new one as the owner
/// chose: one real round, after which the work is paused by the owner.
async fn step(State(app): State<App>, Path(id): Path<i64>, Json(r): Json<api::RunAsk>) -> Result<StatusCode, Fail> {
    app.control
        .ask_step(id, r.new_session)
        .map(|_| StatusCode::ACCEPTED)
        .map_err(|e| Fail::new(StatusCode::CONFLICT, e))
}

/// Starts a work, as the owner: it goes on whatever the global state, its own pause lifted by the engine once it may
/// hold a slot (a work the architect stopped is the owner's release of it, which the architect is told), and its next
/// round resumes its line's session or starts a new one as the owner chose.
async fn start_work(
    State(app): State<App>,
    Path(id): Path<i64>,
    Json(r): Json<api::RunAsk>,
) -> Result<Json<api::Done>, Fail> {
    let done = read(&app, move |c, _| {
        api::can_start(c, id).map_err(|e| {
            if e.starts_with("no work") {
                api::Error::NotFound(e)
            } else {
                api::Error::Invalid(e)
            }
        })
    })
    .await?;
    app.control.start(api::Fresh::Work(id), r.new_session);
    tell_status(&app);
    Ok(Json(api::Done { done }))
}

async fn works(State(app): State<App>) -> Result<Json<Vec<api::WorkSummary>>, Fail> {
    read(&app, |c, a| api::works(c, &a.flows)).await.map(Json)
}

async fn work(State(app): State<App>, Path(id): Path<i64>) -> Result<Json<api::WorkDetail>, Fail> {
    read(&app, move |c, a| api::work(c, &a.flows, id)).await.map(Json)
}

/// A round, by its work and its number in the work.
async fn round(State(app): State<App>, Path((work, n)): Path<(i64, i64)>) -> Result<Json<api::RoundDetail>, Fail> {
    read(&app, move |c, a| api::round(c, &a.flows, &a.data, work, n))
        .await
        .map(Json)
}

#[derive(Deserialize)]
struct Addressed {
    address: String,
}

/// What a conditional input of one of a work's rounds holds, by the hash the round's record names it with and the
/// address it was given or referred to under.
async fn content(
    State(app): State<App>,
    Path((work, hash)): Path<(i64, String)>,
    Query(q): Query<Addressed>,
) -> Result<Json<serde_json::Value>, Fail> {
    read(&app, move |c, a| api::content(c, &a.flows, work, &hash, &q.address))
        .await
        .map(Json)
}

async fn flows(State(app): State<App>) -> Result<Json<Vec<api::FlowVersion>>, Fail> {
    read(&app, |c, _| api::flows(c)).await.map(Json)
}

/// A flow version's compiled flow, as the database keeps it.
async fn flow(State(app): State<App>, Path(id): Path<i64>) -> Result<Response, Fail> {
    let text = read(&app, move |c, _| api::compiled_flow(c, id)).await?;
    Ok(([(header::CONTENT_TYPE, "application/json")], text).into_response())
}

async fn flow_view(State(app): State<App>, Path(id): Path<i64>) -> Result<Json<api::FlowView>, Fail> {
    read(&app, move |c, a| api::flow_view(c, &a.flows, id)).await.map(Json)
}

/// What the inputs from outside the flow hold now, by the committed configuration and the projects' checkouts.
async fn inputs_now(State(app): State<App>) -> Result<Json<api::InputsNow>, Fail> {
    read(&app, move |c, a| {
        let committed = config::committed(&a.data).map_err(|e| api::Error::Failed(e.to_string()))?;
        api::inputs_now(c, &committed.config)
    })
    .await
    .map(Json)
}

/// The checkout of each project the committed configuration names, as it is now.
fn checkouts(data: &std::path::Path) -> Result<BTreeMap<String, PathBuf>, api::Error> {
    let committed = config::committed(data).map_err(|e| api::Error::Failed(e.to_string()))?;
    Ok(committed
        .config
        .projects
        .into_iter()
        .map(|(name, p)| (name, p.checkout))
        .collect())
}

fn checkout_of(data: &std::path::Path, project: &str) -> Result<PathBuf, api::Error> {
    checkouts(data)?
        .remove(project)
        .ok_or_else(|| api::Error::NotFound(format!("no project {project} in the configuration")))
}

/// Each project's checkout, with its branches, where the flow can be read at a ref.
async fn flow_sources(State(app): State<App>) -> Result<Json<Vec<api::FlowSource>>, Fail> {
    read(&app, |_, a| {
        Ok(checkouts(&a.data)?
            .into_iter()
            .map(|(project, checkout)| api::FlowSource {
                project,
                refs: api::refs(&checkout).unwrap_or_default(),
                checkout: checkout.display().to_string(),
            })
            .collect())
    })
    .await
    .map(Json)
}

#[derive(Deserialize)]
struct At {
    project: String,
    #[serde(rename = "ref")]
    rev: String,
}

/// The flow as `flow/flow.json` is at a git ref of a project's checkout, compiled, with what the page shows of it.
async fn flow_at(State(app): State<App>, Query(q): Query<At>) -> Result<Json<api::FlowAt>, Fail> {
    read(&app, move |c, a| {
        let checkout = checkout_of(&a.data, &q.project)?;
        let (commit, compiled) = a.sources.at(&checkout, &q.rev)?;
        let digest = compiled.digest();
        Ok(api::FlowAt {
            project: q.project,
            rev: q.rev,
            commit,
            version: api::version_of(c, &digest)?,
            digest,
            view: api::view(&compiled),
            compiled: (*compiled).clone(),
        })
    })
    .await
    .map(Json)
}

#[derive(Deserialize)]
struct Between {
    /// The older flow: a stored version.
    from: i64,
    /// The newer flow: a stored version, or the flow at a ref of a project's checkout.
    to: Option<i64>,
    project: Option<String>,
    #[serde(rename = "ref")]
    rev: Option<String>,
}

/// What changed from a stored flow version to another, or to the flow at a git ref of a project's checkout.
async fn flow_diff(State(app): State<App>, Query(q): Query<Between>) -> Result<Json<api::FlowDiff>, Fail> {
    read(&app, move |c, a| {
        let from = a.flows.get(c, q.from)?;
        let to = match (q.to, q.project, q.rev) {
            (Some(v), None, None) => a.flows.get(c, v)?,
            (None, Some(p), Some(r)) => a.sources.at(&checkout_of(&a.data, &p)?, &r)?.1,
            _ => {
                return Err(api::Error::Invalid(
                    "compare with another version (to) or with a project's ref (project and ref)".into(),
                ));
            }
        };
        Ok(api::diff(&from, &to))
    })
    .await
    .map(Json)
}

async fn items(State(app): State<App>) -> Result<Json<api::Items>, Fail> {
    read(&app, |c, a| api::items(c, &a.flows)).await.map(Json)
}

async fn item(State(app): State<App>, Path(id): Path<i64>) -> Result<Json<api::ItemDetail>, Fail> {
    read(&app, move |c, a| api::item(c, &a.flows, id)).await.map(Json)
}

/// Which project's architect a request is about: the one it names, or the configuration's one project.
#[derive(Deserialize)]
struct OfProject {
    project: Option<String>,
}

/// The project a request names, or the configuration's one project, with the configuration as committed.
fn project_of(app: &App, q: &OfProject) -> Result<(String, config::Config), Fail> {
    let c = config::committed(&app.data)
        .map_err(|e| Fail::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .config;
    let project = api::project_named(&c, q.project.as_deref()).map_err(|e| Fail::new(StatusCode::NOT_FOUND, e))?;
    Ok((project, c))
}

/// A project's architect, as its tab shows it; with no project named, the configuration's first, so that the page
/// opens on one and offers the others.
async fn architect(State(app): State<App>, Query(q): Query<OfProject>) -> Result<Json<api::Architect>, Fail> {
    let first = OfProject {
        project: q.project.clone().or_else(|| {
            config::committed(&app.data)
                .ok()
                .and_then(|c| c.config.projects.keys().next().cloned())
        }),
    };
    let (project, c) = project_of(&app, &first)?;
    let projects: Vec<String> = c.projects.keys().cloned().collect();
    let tags: Vec<api::Tag> = c
        .projects
        .get(&project)
        .map(|p| {
            p.observation_tags
                .iter()
                .map(|t| api::Tag {
                    name: t.name.clone(),
                    desc: t.desc.clone(),
                })
                .collect()
        })
        .unwrap_or_default();
    let own = c.wf_agents_project().as_deref() == Some(project.as_str());
    // The project's constitution, the file its configuration names in its checkout, as the architect reads it.
    let constitution = c.projects.get(&project).and_then(|p| {
        p.constitution.as_ref().map(|path| api::NamedText {
            name: path.display().to_string(),
            text: std::fs::read_to_string(p.checkout.join(path)).unwrap_or_default(),
        })
    });
    read(&app, move |conn, a| {
        api::architect(conn, &a.flows, &project, projects, tags, own, constitution)
    })
    .await
    .map(Json)
}

/// What the owner's conversations need of the configuration as committed: its projects, the one that is wf-agents
/// itself, and the groups of projects it connects.
fn talks(data: &std::path::Path) -> Result<api::Talks, api::Error> {
    let c = config::committed(data).map_err(|e| api::Error::Failed(e.to_string()))?;
    Ok(api::Talks::of(&c.config))
}

/// The owner's conversations in the list's order, each with what waits for the owner in it and what is unread.
async fn conversations(State(app): State<App>) -> Result<Json<Vec<api::Conversation>>, Fail> {
    read(&app, |c, a| api::conversations(c, &talks(&a.data)?))
        .await
        .map(Json)
}

/// Which conversation a request is about, by its key.
#[derive(Deserialize)]
struct OfConversation {
    key: String,
}

/// One of the owner's conversations in full.
async fn conversation(
    State(app): State<App>,
    Query(q): Query<OfConversation>,
) -> Result<Json<api::ConversationDetail>, Fail> {
    read(&app, move |c, a| {
        api::conversation(c, &a.flows, &talks(&a.data)?, &q.key)
    })
    .await
    .map(Json)
}

/// What the owner says in a work's group: to its engineer, to its project's architect or to both, or an answer to a
/// question of the engineer's that the architect holds.
async fn say_to_work(
    State(app): State<App>,
    Path(id): Path<i64>,
    Json(s): Json<api::WorkSaid>,
) -> Result<Json<api::Done>, Fail> {
    decide(&app, move |db| api::say_to_work(db, id, &s)).await
}

/// The settings the owner keeps on the page, as the configuration's last commit has them.
async fn settings(State(app): State<App>) -> Result<Json<api::Settings>, Fail> {
    read(&app, |_, a| api::settings(&a.data).map_err(api::Error::Failed))
        .await
        .map(Json)
}

/// Writes a change to the settings into the installation's file and commits it, as the owner; the engine takes it from
/// its next round. A change the configuration would refuse is refused before anything is written.
async fn save_settings(State(app): State<App>, Json(c): Json<api::SettingsChange>) -> Result<Json<api::Done>, Fail> {
    let data = Arc::clone(&app.data);
    decide(&app, move |_| api::save_settings(&data, &c)).await
}

/// The login accounts the agents may run under, each with the account it is signed in as, and the one they run under.
async fn accounts(State(app): State<App>) -> Result<Json<Vec<api::CliAccounts>>, Fail> {
    read(&app, |_, a| api::accounts(&a.data).map_err(api::Error::Failed))
        .await
        .map(Json)
}

/// Has the agents run under another of their login accounts, as the owner: writes it into the installation's file and
/// commits it; every line starts a new session at its next round. One not signed in is refused.
async fn switch_account(State(app): State<App>, Json(c): Json<api::AccountChoice>) -> Result<Json<api::Done>, Fail> {
    let data = Arc::clone(&app.data);
    decide(&app, move |_| api::switch_account(&data, &c)).await
}

/// Puts an item at another tier, as the owner.
async fn set_level(
    State(app): State<App>,
    Path(id): Path<i64>,
    Json(l): Json<api::NewLevel>,
) -> Result<Json<api::Done>, Fail> {
    let data = Arc::clone(&app.data);
    decide(&app, move |db| api::set_level(db, &data, id, &l.level)).await
}

/// The period the System view counts over, and the owner's time zone its weeks start in.
#[derive(Deserialize)]
struct Period {
    /// How many weeks, the week under way the last; none for all time.
    weeks: Option<i64>,
    /// Minutes east of UTC.
    #[serde(default)]
    tz: i64,
}

/// How the whole system stands now, and what it did over the period asked for: the architects of the projects the
/// committed configuration names, and the anomalies its counters' limits find.
async fn system(State(app): State<App>, Query(p): Query<Period>) -> Result<Json<api::System>, Fail> {
    read(&app, move |conn, a| {
        let c = config::committed(&a.data)
            .map_err(|e| api::Error::Failed(e.to_string()))?
            .config;
        let projects: Vec<String> = c.projects.keys().cloned().collect();
        let limits: BTreeMap<String, BTreeMap<String, u64>> = c
            .projects
            .into_iter()
            .map(|(name, project)| (name, project.limits))
            .collect();
        let ask = api::SystemAsk {
            weeks: p.weeks,
            tz: p.tz.clamp(-14 * 60, 14 * 60),
            projects: &projects,
            limits: &limits,
            now: db::now(),
        };
        api::system(conn, &a.flows, &ask)
    })
    .await
    .map(Json)
}

/// Answers an ask of the owner, as the owner, as the command line does; the engine is woken to go on.
async fn answer(
    State(app): State<App>,
    Path(id): Path<i64>,
    Json(a): Json<api::Answer>,
) -> Result<Json<api::Done>, Fail> {
    decide(&app, move |db| api::answer(db, id, &a)).await
}

/// Rules on a design card, as the owner, as the command line does; the engine is woken to go on.
async fn rule(
    State(app): State<App>,
    Path(id): Path<i64>,
    Json(r): Json<api::Ruling>,
) -> Result<Json<api::Done>, Fail> {
    decide(&app, move |db| api::rule(db, id, &r)).await
}

/// Says something to a project's architect, as the owner, as the command line does: it comes to its queue, and the
/// engine is woken, so that a work of the architect starts when works go on.
async fn say_to_architect(
    State(app): State<App>,
    Query(q): Query<OfProject>,
    Json(s): Json<api::ArchitectSaid>,
) -> Result<Json<api::Done>, Fail> {
    let (project, _) = project_of(&app, &q)?;
    decide(&app, move |db| {
        let images = api::images(&s.images)?;
        match s.reply_to {
            Some(ask) => api::reply_to_question(db, &project, ask, &s.text, &images),
            None => db::say_to_architect(db, &project, s.item, None, &s.text, &images),
        }
    })
    .await
}

/// Runs one round of a project's architect: the next round of its work that runs, or, when none runs and its queue
/// would start one, a work started paused by the owner and its first round; in a new session for its work as the owner
/// chose.
async fn step_architect(
    State(app): State<App>,
    Query(q): Query<OfProject>,
    Json(r): Json<api::RunAsk>,
) -> Result<StatusCode, Fail> {
    let (project, _) = project_of(&app, &q)?;
    app.control
        .ask_step_architect(&project, r.new_session)
        .map(|_| StatusCode::ACCEPTED)
        .map_err(|e| Fail::new(StatusCode::CONFLICT, e))
}

/// Starts a project's architect, as the owner, as a work is started: its work goes on, and one starts when its queue
/// has a matter, whatever the global state; its next round resumes its session or starts a new one for its work as the
/// owner chose.
async fn start_architect(
    State(app): State<App>,
    Query(q): Query<OfProject>,
    Json(r): Json<api::RunAsk>,
) -> Result<Json<api::Done>, Fail> {
    let (project, _) = project_of(&app, &q)?;
    app.control.start(api::Fresh::Architect(project.clone()), r.new_session);
    tell_status(&app);
    Ok(Json(api::Done {
        done: format!("the architect of {project} goes on"),
    }))
}

/// Closes an observation, as the owner, saying why.
async fn close_observation(
    State(app): State<App>,
    Path(id): Path<i64>,
    Json(c): Json<api::CloseObservation>,
) -> Result<Json<api::Done>, Fail> {
    decide(&app, move |db| api::close_observation(db, id, &c.why)).await
}

/// Pauses a work, as the owner, as the command line does: a round of it under way ends first, and it stays paused,
/// whatever the global state, until the owner starts it. The owner's Start of it is over, and with it the lifting of
/// its pause that Start asked, so a work still paused stays so.
async fn pause(State(app): State<App>, Path(id): Path<i64>) -> Result<Json<api::Done>, Fail> {
    app.control.pause(&api::Fresh::Work(id));
    tell_status(&app);
    decide(&app, move |db| match api::pause(db, id, None) {
        Err(e) if e.ends_with("is paused already") => Ok(format!("work {id} stays paused")),
        r => r,
    })
    .await
}

/// Starts a work over, as the owner: the engine takes it over at once, whatever the global state, as it takes over a
/// work on an older flow version, a round of it under way cut off once its agent's process has ended; the owner's Start
/// of it goes on in the new work.
async fn start_over(State(app): State<App>, Path(id): Path<i64>) -> Result<Json<api::Done>, Fail> {
    decide(&app, move |db| api::start_over(db, id)).await
}

/// Pauses a project's architect, as the owner: its work that runs is paused as a work is, and the owner's Start of the
/// architect is over.
async fn pause_architect(State(app): State<App>, Query(q): Query<OfProject>) -> Result<Json<api::Done>, Fail> {
    let (project, _) = project_of(&app, &q)?;
    app.control.pause(&api::Fresh::Architect(project.clone()));
    tell_status(&app);
    decide(&app, move |db| api::pause_architect(db, &project)).await
}

/// Writes what the owner decided, then wakes the engine's loop.
async fn decide(
    app: &App,
    f: impl FnOnce(&mut db::Db) -> Result<String, String> + Send + 'static,
) -> Result<Json<api::Done>, Fail> {
    let path = Arc::clone(&app.db);
    let done = tokio::task::spawn_blocking(move || {
        let mut conn = db::Db::open(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        f(&mut conn)
    })
    .await
    .map_err(|e| Fail::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .map_err(|e| Fail::new(StatusCode::UNPROCESSABLE_ENTITY, e))?;
    app.control.wake();
    Ok(Json(api::Done { done }))
}

/// What the owner says in a group of connected projects' architects: to one of them, or to every one.
async fn say_to_projects(State(app): State<App>, Json(s): Json<api::ProjectsSaid>) -> Result<Json<api::Done>, Fail> {
    let data = Arc::clone(&app.data);
    decide(&app, move |db| {
        let projects = talks(&data)
            .map_err(|e| e.to_string())?
            .set_of(&s.key)
            .ok_or_else(|| format!("no group of projects {}", s.key))?;
        api::say_to_projects(db, &projects, &s)
    })
    .await
}

/// Forwards a fault of wf-agents to a project's architect, as the owner: a project of the configuration, whose
/// architect takes it as a matter of its queue once; the engine is woken, so that the architect takes it up.
async fn forward_fault(
    State(app): State<App>,
    Path(id): Path<i64>,
    Json(f): Json<api::ForwardFault>,
) -> Result<Json<api::Done>, Fail> {
    let (project, _) = project_of(
        &app,
        &OfProject {
            project: Some(f.project),
        },
    )?;
    decide(&app, move |db| api::forward_fault(db, id, &project)).await
}

/// Sets, as the owner, where each new fault of wf-agents goes as it comes: to the architect of a project of the
/// configuration, or to nobody.
async fn forward_new(State(app): State<App>, Json(f): Json<api::ForwardNew>) -> Result<Json<api::Done>, Fail> {
    let project = match f.project {
        Some(p) => Some(project_of(&app, &OfProject { project: Some(p) })?.0),
        None => None,
    };
    decide(&app, move |db| api::set_fault_forwarding(db, project.as_deref())).await
}

/// Marks how far the owner has read a conversation, as the owner.
async fn read_to(State(app): State<App>, Json(r): Json<api::ReadTo>) -> Result<Json<api::Marked>, Fail> {
    let db = Arc::clone(&app.db);
    tokio::task::spawn_blocking(move || api::read_to(&db, &r))
        .await
        .map_err(|e| Fail::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map(Json)
        .map_err(Fail::from)
}

/// Marks messages to the owner read, as the owner.
async fn mark_read(State(app): State<App>, Json(m): Json<api::MarkRead>) -> Result<Json<api::Marked>, Fail> {
    let db = Arc::clone(&app.db);
    tokio::task::spawn_blocking(move || api::mark_read(&db, &m))
        .await
        .map_err(|e| Fail::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map(Json)
        .map_err(Fail::from)
}

async fn file(State(app): State<App>, Path((dir, name)): Path<(String, String)>) -> Result<Response, Fail> {
    let data = Arc::clone(&app.data);
    let text = tokio::task::spawn_blocking(move || api::file(&data, &dir, &name, FILE_MAX))
        .await
        .map_err(|e| Fail::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))??;
    Ok(([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], text).into_response())
}

/// An image the owner sent, by its id, as it was sent, with its media type; it never changes, so the browser keeps it.
async fn attachment(State(app): State<App>, Path(id): Path<i64>) -> Result<Response, Fail> {
    let (media_type, bytes) = read(&app, move |c, _| api::attachment(c, id)).await?;
    Ok((
        [
            (header::CONTENT_TYPE, media_type),
            (
                header::CACHE_CONTROL,
                "private, max-age=31536000, immutable".to_string(),
            ),
        ],
        bytes,
    )
        .into_response())
}

async fn no_such(uri: Uri) -> Fail {
    Fail::new(StatusCode::NOT_FOUND, format!("no such request: {}", uri.path()))
}

#[derive(Deserialize)]
struct After {
    /// The id of the last event the client saw, for a first connection, which cannot send Last-Event-ID.
    after: Option<String>,
}

/// The event stream: what the client missed since the id it gives (or that the stream starts here), then every event
/// as it is published.
async fn events(State(app): State<App>, headers: HeaderMap, Query(q): Query<After>) -> impl IntoResponse {
    let last = headers
        .get("last-event-id")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
        .or(q.after);
    // Listening first, then reading what was missed: an event published in between is in both, and the number
    // tells which to drop.
    let live = BroadcastStream::new(app.events.subscribe());
    let (first, upto) = app.bus.since(last.as_deref());
    let live = live.filter_map(move |e| match e {
        Ok(e) if e.seq > upto => Some(e),
        Ok(_) => None,
        // Events were dropped on the way: the client reads everything again.
        Err(BroadcastStreamRecvError::Lagged(_)) => Some(Envelope {
            id: String::new(),
            seq: 0,
            event: Event::Resync,
        }),
    });
    let stream = tokio_stream::iter(first)
        .chain(live)
        .map(|e| Ok::<_, Infallible>(sse_event(&e)));
    Sse::new(stream).keep_alive(KeepAlive::default())
}

fn sse_event(e: &Envelope) -> sse::Event {
    let data = serde_json::to_string(&e.event).unwrap_or_default();
    let event = sse::Event::default().data(data);
    if e.id.is_empty() { event } else { event.id(&e.id) }
}

/// The page's files. A path that is not a file is a page of the client's own, which `index.html` routes; a missing
/// asset is not.
async fn page(method: Method, uri: Uri) -> Response {
    if method != Method::GET && method != Method::HEAD {
        return Fail::new(StatusCode::METHOD_NOT_ALLOWED, "the page is only read").into_response();
    }
    let path = uri.path().trim_start_matches('/');
    let find = |p: &str| built::FILES.iter().find(|(name, _)| *name == p).copied();
    let found = match find(path) {
        Some(f) => Some(f),
        None if path.starts_with("assets/") => None,
        None => find("index.html"),
    };
    let Some((name, bytes)) = found else {
        return Fail::new(StatusCode::NOT_FOUND, format!("no such file: {path}")).into_response();
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
