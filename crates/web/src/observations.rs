//! The observations of the installation, as the page's Observations view reads them, and what the commands and the
//! owner do with them, as `client/src/observations-api.ts` and the commands `wf-agents observations`,
//! `wf-agents add-observation`, `wf-agents change-observations` and `wf-agents tags` ask for them:
//!
//! - `GET /api/v1/observations`: every observation not deleted, the newest first ([`ObservationsPage`]).
//! - `POST /api/v1/observations`: an architect adds one ([`NewObservation`]): answers with its number
//!   ([`ObservationAdded`]); 422 where the project it is of or the one that reported it is none the settings name,
//!   where its text has no words, or where its title or a tag is none that may be given.
//! - `POST /api/v1/observations/changes`: changes of observations, made together for one reason
//!   ([`ObservationChanges`]): answers whether each changed its observation ([`ObservationsChanged`]), or, with 422,
//!   every change refused, each named by its place among them, none having been made.
//! - `GET /api/v1/observations/tags`: each tag an observation carries, with how many carry it ([`TagsPage`]).
//! - `POST /api/v1/observations/{number}/read`: the owner read the observation: answers with when it was read
//!   ([`Read`]), which a reading after the first leaves as it was; 404 where there is no observation of the number,
//!   however large.
//!
//! They are read, added to, changed and read by the owner through connections of their own to the store's file, beside
//! the engine's loop; the time of each is the one the loop's clock gives, since the loop alone reads the clock. What
//! is added or changed is told to whoever listens to the stream of changes, so that the page reads again.

use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use axum::body::Bytes;
use axum::extract::rejection::PathRejection;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use axum::routing::{get, post};
use serde::{Deserialize, Serialize};
use service::{Handle, Levels, Projects};
use store::{ItemNumber, ObservationChange, Observations, Refusal, tag_is_bad};

use crate::fail;

#[derive(Clone)]
struct Inner {
    handle: Handle,
    /// The store's file.
    store: Arc<PathBuf>,
    levels: Arc<Levels>,
    projects: Arc<Projects>,
}

/// The routes of the observations, over the store's file and the engine's loop, which gives the time; the title of an
/// observation is within the words these levels allow, and the projects are among these.
pub fn router(handle: Handle, store: PathBuf, levels: Levels, projects: Projects) -> Router {
    Router::new()
        .route("/api/v1/observations", get(all).post(add))
        .route("/api/v1/observations/changes", post(change))
        .route("/api/v1/observations/tags", get(tags))
        .route("/api/v1/observations/{number}/read", post(read))
        .with_state(Inner {
            handle,
            store: Arc::new(store),
            levels: Arc::new(levels),
            projects: Arc::new(projects),
        })
}

/// Every observation not deleted, the newest first.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservationsPage {
    pub observations: Vec<Observation>,
}

/// An observation: its number, what it says, its title and tags, the projects it is of and was reported by, where a
/// run kept it where one did, when it was kept, when the owner read it, the item that took it in and whether it is
/// handled. Each time is in milliseconds since the start of 1970.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Observation {
    pub number: u64,
    pub text: String,
    /// The latest title it was given or was added with; none while it has none.
    pub title: Option<String>,
    pub tags: Vec<String>,
    /// The project it is of; none for one a run kept for an item kept before items had projects.
    pub project: Option<String>,
    /// The project that reported it; for one a run kept, the project of its work's item.
    pub reported_by: Option<String>,
    /// Where a run kept it; none for one an architect added.
    pub round: Option<Round>,
    /// When it was kept: as its run finished, or as it was added.
    pub kept: u64,
    /// When the owner read it; none while it is unread.
    pub read: Option<u64>,
    /// The item that took it in, the latest to have.
    pub taken_in_by: Option<u64>,
    /// Whether an item whose work is not cancelled has taken it in.
    pub handled: bool,
}

/// Where a run kept an observation: the work whose run's return carried it, the work's item with its title as it
/// stands, and the cell, the run and the round whose return carried it, the first round being one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Round {
    pub work: u64,
    pub item: u64,
    pub item_title: String,
    pub cell: String,
    pub run: u64,
    pub round: usize,
}

impl Observation {
    fn of(o: &store::Observation) -> Observation {
        Observation {
            number: o.number,
            text: o.text.clone(),
            title: o.title.clone(),
            tags: o.tags.clone(),
            project: o.project.clone(),
            reported_by: o.reported_by.clone(),
            round: (o.round.as_ref()).map(|r| Round {
                work: r.work.0,
                item: r.item.0,
                item_title: r.item_title.clone(),
                cell: r.cell.clone(),
                run: r.run.0,
                round: r.round,
            }),
            kept: o.kept.0,
            read: o.read.map(|at| at.0),
            taken_in_by: o.taken_in_by.map(|item| item.0),
            handled: o.handled,
        }
    }
}

/// An observation an architect adds: what it says, its title if it has one, its tags, the project it is of and the
/// project that reported it, both among those the settings name.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NewObservation {
    pub text: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub project: String,
    pub reported_by: String,
}

/// The number of the observation that was added.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservationAdded {
    pub number: u64,
}

/// Changes of observations asked together, for one reason: made in the order given, each judged against what those
/// before it make, all of them or none.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ObservationChanges {
    /// Why they are made, in a sentence: kept with each.
    pub why: String,
    pub changes: Vec<ObservationChangeAsked>,
}

/// A change of an observation, as an architect asks for it. Each kind is a struct, as an item's change is.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ObservationChangeAsked {
    /// The observation is given this title, in the owner's language.
    Title { observation: u64, title: String },
    /// The observation comes to carry this tag.
    Tag { observation: u64, tag: String },
    /// The observation no longer carries this tag.
    Untag { observation: u64, tag: String },
    /// This item takes the observation in: its work is not cancelled.
    TakenIn { observation: u64, item: u64 },
    /// The observation is deleted: it is listed no more, and its text and why stay in the records.
    Delete { observation: u64 },
}

impl ObservationChangeAsked {
    /// The observation it changes.
    pub fn observation(&self) -> u64 {
        match self {
            ObservationChangeAsked::Title { observation, .. }
            | ObservationChangeAsked::Tag { observation, .. }
            | ObservationChangeAsked::Untag { observation, .. }
            | ObservationChangeAsked::TakenIn { observation, .. }
            | ObservationChangeAsked::Delete { observation } => *observation,
        }
    }

    /// What the store is asked to make of it.
    fn taken(&self) -> ObservationChange {
        match self {
            ObservationChangeAsked::Title { title, .. } => ObservationChange::Titled(title.clone()),
            ObservationChangeAsked::Tag { tag, .. } => ObservationChange::Tagged(tag.clone()),
            ObservationChangeAsked::Untag { tag, .. } => ObservationChange::Untagged(tag.clone()),
            ObservationChangeAsked::TakenIn { item, .. } => ObservationChange::TakenIn(ItemNumber(*item)),
            ObservationChangeAsked::Delete { .. } => ObservationChange::Deleted,
        }
    }
}

/// Whether each change of those asked together changed its observation, in the order asked: it was so already where
/// not, as when the observation had that title already.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObservationsChanged {
    pub changed: Vec<bool>,
}

/// Each tag an observation not deleted carries, in the order of the names.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TagsPage {
    pub tags: Vec<TagCount>,
}

/// A tag, how many observations carry it and how many of those are not handled.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TagCount {
    pub tag: String,
    pub carrying: usize,
    pub unhandled: usize,
}

/// When the owner read an observation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Read {
    pub read: u64,
}

/// What came of work on the store's file off the loop's thread: what it gave, or why it failed, in words.
async fn on_store<T: Send + 'static>(
    inner: &Inner,
    work: impl FnOnce(Observations) -> Result<T, store::Fault> + Send + 'static,
) -> Result<T, String> {
    let store = inner.store.clone();
    let done = tokio::task::spawn_blocking(move || {
        (Observations::open(store.as_path()))
            .and_then(work)
            .map_err(|fault| service::store_fault_words(&fault))
    })
    .await;
    match done {
        Ok(done) => done,
        Err(e) => Err(e.to_string()),
    }
}

async fn all(State(inner): State<Inner>) -> Response {
    match on_store(&inner, |observations| observations.all()).await {
        Ok(all) => Json(ObservationsPage {
            observations: all.iter().map(Observation::of).collect(),
        })
        .into_response(),
        Err(words) => fail(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("The observations cannot be shown: {words}."),
        ),
    }
}

async fn tags(State(inner): State<Inner>) -> Response {
    match on_store(&inner, |observations| observations.tags()).await {
        Ok(tags) => Json(TagsPage {
            tags: (tags.into_iter())
                .map(|tag| TagCount {
                    tag: tag.tag,
                    carrying: tag.carrying,
                    unhandled: tag.unhandled,
                })
                .collect(),
        })
        .into_response(),
        Err(words) => fail(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("The tags of the observations cannot be shown: {words}."),
        ),
    }
}

/// The words with their first letter a capital.
fn capital(words: &str) -> String {
    let mut chars = words.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// Why a change was refused, as a clause.
fn refusal_words(why: &Refusal) -> String {
    match why {
        Refusal::NoSuchObservation => "there is no such observation".to_string(),
        Refusal::Deleted => "it is deleted, and takes no change".to_string(),
        Refusal::NoSuchItem(item) => format!("there is no item #{}", item.0),
        Refusal::ItemCancelled(item) => {
            format!(
                "#{} is cancelled, and an observation it took in would be lost with it",
                item.0
            )
        }
        Refusal::BadTag(tag) => format!(
            "{tag:?} is no tag: a tag is a name of 1 to {} characters with no space and no comma",
            store::TAG_CHARS
        ),
        Refusal::BlankTitle => "a title of no words says nothing: give it words".to_string(),
    }
}

/// An observation is added: its text has words, its title is within the words the settings allow, its tags are names
/// a tag may be, and both projects are among the settings'. The time is the loop's.
async fn add(State(inner): State<Inner>, body: Bytes) -> Response {
    let new: NewObservation = match serde_json::from_slice(&body) {
        Ok(new) => new,
        Err(e) => return fail(StatusCode::BAD_REQUEST, format!("The observation cannot be read: {e}.")),
    };
    let refused = |why: String| fail(StatusCode::UNPROCESSABLE_ENTITY, format!("{why}."));
    if new.text.trim().is_empty() {
        return refused("An observation says what was come across, and this one says nothing: give it words".into());
    }
    if let Some(title) = &new.title {
        if title.trim().is_empty() {
            return refused("The observation's title has no words: give it words or none".into());
        }
        if let Err(why) = inner.levels.title_fits(title) {
            return refused(format!("The observation's title: {why}"));
        }
    }
    if let Some(tag) = new.tags.iter().find(|tag| tag_is_bad(tag)) {
        return refused(refusal_words(&Refusal::BadTag(tag.clone())));
    }
    let project = match inner.projects.of_item(&new.project) {
        Ok(project) => project,
        Err(why) => return refused(format!("The observation's project: {why}")),
    };
    let reported_by = match inner.projects.of_item(&new.reported_by) {
        Ok(project) => project,
        Err(why) => return refused(format!("The project that reported the observation: {why}")),
    };
    let Ok(now) = inner.handle.now().await else {
        return fail(StatusCode::SERVICE_UNAVAILABLE, "The engine has stopped.".to_string());
    };
    let added = store::NewObservation {
        text: new.text,
        title: new.title,
        tags: new.tags,
        project,
        reported_by,
    };
    match on_store(&inner, move |observations| observations.add(&added, now)).await {
        Ok(number) => {
            inner.handle.observations_changed();
            Json(ObservationAdded { number }).into_response()
        }
        Err(words) => fail(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("The observation cannot be kept: {words}."),
        ),
    }
}

/// Observations are changed, for the one reason given, all of the changes or none. Answers whether each changed its
/// observation, or why they were refused, each change refused named by its place among them.
async fn change(State(inner): State<Inner>, body: Bytes) -> Response {
    let asked: ObservationChanges = match serde_json::from_slice(&body) {
        Ok(asked) => asked,
        Err(e) => return fail(StatusCode::BAD_REQUEST, format!("The changes cannot be read: {e}.")),
    };
    let refused = |why: String| fail(StatusCode::UNPROCESSABLE_ENTITY, why);
    if asked.why.trim().is_empty() {
        return refused(
            "A change says why it is made, and this one says nothing: give a reason in a sentence.".to_string(),
        );
    }
    if asked.changes.is_empty() {
        return refused("No change is asked for.".to_string());
    }
    let mut unfit = Vec::new();
    for (at, change) in asked.changes.iter().enumerate() {
        if let ObservationChangeAsked::Title { title, .. } = change
            && let Err(why) = inner.levels.title_fits(title)
        {
            unfit.push(format!("change {}: the observation's title: {why}", at + 1));
        }
    }
    if !unfit.is_empty() {
        return refused(capital(&format!(
            "{}; so none of the changes was made.",
            unfit.join("; ")
        )));
    }
    let changes: Vec<(u64, ObservationChange)> = (asked.changes.iter())
        .map(|change| (change.observation(), change.taken()))
        .collect();
    let Ok(now) = inner.handle.now().await else {
        return fail(StatusCode::SERVICE_UNAVAILABLE, "The engine has stopped.".to_string());
    };
    let why = asked.why;
    match on_store(&inner, move |observations| observations.change(&why, &changes, now)).await {
        Ok(Ok(changed)) => {
            if changed.contains(&true) {
                inner.handle.observations_changed();
            }
            Json(ObservationsChanged { changed }).into_response()
        }
        Ok(Err(not_made)) => {
            let said: Vec<String> = (not_made.iter())
                .map(|n| format!("change {} (O{}): {}", n.at + 1, n.observation, refusal_words(&n.why)))
                .collect();
            refused(capital(&format!(
                "{}; so none of the changes was made.",
                said.join("; ")
            )))
        }
        Err(words) => fail(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("The changes cannot be kept: {words}."),
        ),
    }
}

/// The owner read the observation the path names. A number past what the store can number names none, as any other
/// number of none does; what is no number at all cannot be read.
async fn read(State(inner): State<Inner>, path: Result<Path<String>, PathRejection>) -> Response {
    let named = match path {
        Ok(Path(named)) => named,
        Err(rejection) => {
            return fail(
                StatusCode::BAD_REQUEST,
                format!("The path cannot be read: {}.", rejection.body_text()),
            );
        }
    };
    if named.is_empty() || !named.bytes().all(|b| b.is_ascii_digit()) {
        return fail(
            StatusCode::BAD_REQUEST,
            format!("The path cannot be read: {named} is no number of an observation."),
        );
    }
    let Ok(number) = named.parse::<u64>() else {
        return fail(StatusCode::NOT_FOUND, format!("There is no observation O{named}."));
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
