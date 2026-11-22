//! The installation's settings on the page: its file shown as it writes them, table by table, each key with its value,
//! beside what the engine runs with where that differs; and what the owner changes there written back into the file.
//!
//! - `GET /api/v1/settings`: the file ([`SettingsPage`]).
//! - `POST /api/v1/settings`: the owner's change of a few keys ([`SettingsChange`]), answered with what was changed in
//!   words ([`Saved`]).
//! - `GET /api/v1/settings/accounts`: the accounts the agents' program may run under, each with what the service last
//!   said of its usage, the one in use, and the command that makes a token ([`AccountsPage`]). Reading spends nothing.
//! - `POST /api/v1/settings/accounts`: the owner adds, takes or removes an account ([`AccountAsked`]), through the
//!   engine's loop, answered with what came of it ([`AccountDone`]).
//! - `POST /api/v1/settings/accounts/look`: the owner opens the accounts: the engine looks under each whose usage it last
//!   heard of long enough ago, off the loop, and answers at once with the names it looks under ([`Looking`]); the
//!   accounts are said changed as each look ends (crate `driver`, module `usage`).
//!
//! The owner changes on the page what is tuned while works go on: the language the agents write in for the owner, the
//! model of each level, the highest level an item may be at, the context at which the agents' program compacts a line's
//! session, and how many rounds run at once. The rest only the file changes. The accounts the agents' program runs
//! under are no key of the file: they are the engine's own state, which takes a change at once (decision
//! the-page-changes-what-is-tuned-while-the-engine-runs). A value is checked here only for its kind; what the engine
//! refuses of it, the engine's own reading of the file refuses. A change is written into the file with its comments and
//! its layout kept, only where the file it makes reads as settings, holds each value as asked, and has not changed
//! since the page read it; where the directory the file is in keeps it in git, the change is committed there, saying
//! what changed, and nothing is written onto a file whose changes are not committed. The engine reads the file as it
//! starts, and runs with what it read until it starts again, but for the slots: once a change of them is written, the
//! engine takes them at once, as the file then gives them (decision the-slots-change-at-once). Each key the engine
//! runs at another value than the file gives, as the engine reads the file, is shown with that value beside it, the
//! slots with those the engine holds now.
//!
//! The settings hold no secret: a token is given to the engine in a variable of its environment, which the file names.
//! An account's token is taken in once, as it is added, and kept outside the engine; no answer of these routes holds
//! it, nor do the words of a refusal, a body that cannot be read among them.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use axum::routing::{get, post};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use toml_edit::{DocumentMut, Item, Table as TomlTable, TableLike, Value};

use crate::interface::{fail, refused};

/// Reads a text as the installation's settings: every key's value, those the text leaves out at what the engine takes
/// for them, as JSON writes it; or why the text does not read, in words with the keys.
pub type ReadSettings = Box<dyn Fn(&str) -> Result<JsonValue, String> + Send + Sync>;

/// What the route is told of the installation's settings.
pub struct Install {
    /// The settings' file.
    pub file: PathBuf,
    /// How a text is read as settings, as the engine reads it.
    pub read: ReadSettings,
    /// The settings the engine runs with: those it read as it started.
    pub running: JsonValue,
    /// What every git command is given before its own arguments, so that nothing the repository's configuration names
    /// is run.
    pub git: &'static [&'static str],
}

/// The file as it writes the settings.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsPage {
    /// The settings' file.
    pub file: String,
    /// The file's text as it was read, by a digest of it: a change is made against it.
    pub base: String,
    /// Why the file as it is now does not read as settings; none where it does.
    pub unreadable: Option<String>,
    /// The file's tables in the order it writes them, the keys of its own first where it has any; then the tables it
    /// leaves out of which the engine runs with values.
    pub tables: Vec<SettingsTable>,
    /// The names of the levels the file gives, for the highest level an item may be at.
    pub levels: Vec<String>,
    /// How the directory the file is in keeps it in git; none where it does not.
    pub git: Option<Kept>,
}

/// A table of the settings.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsTable {
    /// Its name as its header writes it; empty for the keys before the first header.
    pub name: String,
    /// Whether the file leaves it out.
    pub left_out: bool,
    pub keys: Vec<SettingsKey>,
}

/// A key of a table, as the file writes it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsKey {
    /// The key within its table.
    pub key: String,
    /// Its value as the file writes it; none where the file leaves it out.
    pub written: Option<String>,
    /// The value the engine runs with, where it is not the one written: the engine read the file as it started, and
    /// takes a value of its own for a key left out. None where it is the one written.
    pub running: Option<String>,
    /// What the page may change it to; none where only the file changes it.
    pub change: Option<Change>,
}

/// What the page may change a key to.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Change {
    /// A name, on one line.
    Name,
    /// A whole number.
    Count,
    /// One of the levels.
    Level,
}

/// How git keeps the file: whether its changes are all committed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Kept {
    pub committed: bool,
}

/// The owner's change: each key, its tables and itself joined by dots, with its new value, against the file as it was
/// read.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsChange {
    pub base: String,
    pub set: Vec<SetKey>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetKey {
    pub key: String,
    pub value: JsonValue,
}

/// What was changed, in words, and whether it was committed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Saved {
    pub said: String,
    pub committed: bool,
}

/// The accounts the agents' program may run under, as the engine holds them, with what the page says beside them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountsPage {
    /// The time now, by the engine's clock.
    pub now: u64,
    /// The accounts added and not removed, in the order they were added.
    pub accounts: Vec<Account>,
    /// The account the agents run under, by its name; none where none is in use, and no agent's round runs.
    pub in_use: Option<String>,
    /// The command the owner runs, signed in to an account in the browser, to make a token of it.
    pub command: String,
    /// Why no account can be added now, as where what keeps the tokens is missing; none where one can.
    pub closed: Option<String>,
}

/// An account added.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub name: String,
    /// When it was added.
    pub added: u64,
    /// Until when its token lasts: the life the agents' program gives a token, from when it was added.
    pub lasts: u64,
    /// The usage limit the engine holds the account's rounds at, as its records say, where it stands: since when, and
    /// the reset it waits for, where one was said.
    pub limit: Option<AccountLimit>,
    /// What the agents' service last said of its usage.
    pub usage: AccountUsage,
}

/// What the agents' service last said of an account's usage, and when: each window it named, or, where it refused the
/// account's calls naming none, that the account is at its limit; whether a look under it is under way; and why the
/// latest look under it failed, where it did.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountUsage {
    pub seen: Option<u64>,
    pub windows: Vec<UsageWindow>,
    pub at_limit: Option<AtLimit>,
    pub looking: bool,
    pub failed: Option<String>,
}

/// That the service refused the account's calls and named no window: the account is at its limit, which resets then,
/// where it said.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AtLimit {
    pub resets: Option<u64>,
}

/// A window of an account's usage, by the name the service gives it: how much of it is used, from 0 to 1, and when it
/// resets, where the service says; and whether the service lets the account's calls through in it without a warning.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageWindow {
    pub name: String,
    pub utilization: Option<f64>,
    pub resets: Option<u64>,
    pub allowed: bool,
}

/// What the owner's opening of the accounts looks under: the accounts whose usage the engine looks at now.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Looking {
    pub looking: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountLimit {
    pub since: u64,
    pub resets: Option<u64>,
}

/// What the owner does to an account: adds one under a name with its token, takes one as the one the agents run under,
/// or removes one.
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum AccountAsked {
    Add { name: String, token: String },
    Take { name: String },
    Remove { name: String },
}

/// What came of an act on an account: of one added, whether its token met its usage limit as it was checked, and when
/// that resets; of one removed, why its token could not be let go of, where it could not.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountDone {
    pub at_limit: bool,
    pub resets: Option<u64>,
    pub not_let_go: Option<String>,
}

/// The keys the page may change, by their tables and themselves joined by dots, a level's model by any level.
fn change_of(path: &[String]) -> Option<Change> {
    let parts: Vec<&str> = path.iter().map(String::as_str).collect();
    match parts.as_slice() {
        ["owner", "language"] | ["agents", "levels", _, "model"] => Some(Change::Name),
        ["agents", "slots"] | ["sessions", "compact_at"] => Some(Change::Count),
        ["items", "highest_level"] => Some(Change::Level),
        _ => None,
    }
}

/// One writer of the file at a time: two changes at once would each be made on the text the other replaces.
static WRITING: Mutex<()> = Mutex::new(());

/// The key of the slots, by its table and itself: of the keys the page changes, the one the engine takes at once.
const SLOTS: [&str; 2] = ["agents", "slots"];

/// What the routes of the file reach: the file, and the engine's loop, which takes the slots at once.
#[derive(Clone)]
struct Installed {
    install: Arc<Install>,
    handle: service::Handle,
    /// One change at a time, from the file's write to the engine taking the slots it gives, so that the engine runs
    /// with the slots of the file written last.
    saving: Arc<tokio::sync::Mutex<()>>,
}

/// What the routes of the accounts reach: the engine's loop.
#[derive(Clone)]
struct Accounted {
    handle: service::Handle,
}

/// The routes of the settings, and of the accounts, through the engine's loop.
pub fn router(install: Install, handle: service::Handle) -> Router {
    let installed = Installed {
        install: Arc::new(install),
        handle: handle.clone(),
        saving: Arc::default(),
    };
    let accounts = Router::new()
        .route("/api/v1/settings/accounts", get(accounts).post(account))
        .route("/api/v1/settings/accounts/look", post(look))
        .with_state(Accounted { handle });
    Router::new()
        .route("/api/v1/settings", get(page).post(save))
        .with_state(installed)
        .merge(accounts)
}

/// The file as it writes the settings, beside what the engine runs with, the slots as it holds them now.
async fn page(State(at): State<Installed>) -> Response {
    let slots = match at.handle.read(|driver| driver.slots().slots).await {
        Ok(slots) => slots,
        Err(refusal) => return refused(&refusal),
    };
    off_the_loop(move || shown(&at.install, slots)).await
}

/// Writes the owner's change into the file; where it sets the slots, the engine then takes them as the file gives
/// them. Where the engine cannot take them, the file stays as written, and the answer says so.
async fn save(State(at): State<Installed>, body: Bytes) -> Response {
    let change: SettingsChange = match serde_json::from_slice(&body) {
        Ok(change) => change,
        Err(e) => return fail(StatusCode::BAD_REQUEST, format!("The change cannot be read: {e}.")),
    };
    let _one = at.saving.lock().await;
    let install = at.install.clone();
    let (saved, slots) = match on_a_thread(move || changed(&install, &change)).await {
        Ok(made) => made,
        Err(Refused(status, why)) => return fail(status, why),
    };
    if let Some(slots) = slots
        && let Err(refusal) = at.handle.set_slots(slots).await
    {
        let committed = if saved.committed { " and committed" } else { "" };
        return fail(
            StatusCode::SERVICE_UNAVAILABLE,
            format!(
                "{} was written{committed}, but the engine did not take the slots: {refusal}. It takes them as it \
                 starts again.",
                saved.said
            ),
        );
    }
    Json(saved).into_response()
}

/// The accounts as the engine holds them, each with when its token lasts until, the usage limit the engine holds its
/// rounds at where one stands, and what the service last said of its usage. Reading spends nothing: a look is the
/// owner's act ([`look`]).
async fn accounts(State(at): State<Accounted>) -> Response {
    let read = at.handle.read_at(|driver, now| {
        let accounts = driver.accounts();
        let in_use = accounts.in_use().map(str::to_string);
        let usage = driver.usage().clone();
        (now, accounts.live(), in_use, driver.limits(), usage)
    });
    let (now, live, in_use, limits, usage) = match read.await {
        Ok(read) => read,
        Err(refusal) => return refused(&refusal),
    };
    let tokens = at.handle.tokens();
    let (life, command, closed) = (tokens.life_ms(), tokens.making(), tokens.closed());
    let accounts = (live.into_iter())
        .map(|live| Account {
            limit: limits.get(&live.name).map(|limit| AccountLimit {
                since: limit.since.0,
                resets: limit.resets.map(|at| at.0),
            }),
            added: live.added.0,
            lasts: live.added.0.saturating_add(life),
            usage: usage_of(usage.of(&live.name)),
            name: live.name,
        })
        .collect();
    Json(AccountsPage {
        now: now.0,
        accounts,
        in_use,
        command,
        closed,
    })
    .into_response()
}

/// What the service last said of an account's usage, as the page is given it; nothing where nothing is known.
fn usage_of(usage: Option<&driver::Usage>) -> AccountUsage {
    let usage = usage.cloned().unwrap_or_default();
    let (seen, said) = usage.seen.unzip();
    let (windows, at_limit) = match said {
        Some(driver::Said::Windows(windows)) => (windows, None),
        Some(driver::Said::Limit { resets }) => (
            Vec::new(),
            Some(AtLimit {
                resets: resets.map(|at| at.0),
            }),
        ),
        None => (Vec::new(), None),
    };
    AccountUsage {
        seen: seen.map(|at| at.0),
        windows: (windows.into_iter())
            .map(|window| UsageWindow {
                name: window.name,
                utilization: window.utilization,
                resets: window.resets.map(|at| at.0),
                allowed: window.allowed,
            })
            .collect(),
        at_limit,
        looking: usage.looking,
        failed: usage.failed,
    }
}

/// The owner opens the accounts: the engine looks under each whose usage it last heard of long enough ago, and answers
/// at once with the names it looks under.
async fn look(State(at): State<Accounted>) -> Response {
    match at.handle.look().await {
        Ok(looking) => Json(Looking { looking }).into_response(),
        Err(refusal) => refused(&refusal),
    }
}

/// Adds, takes or removes an account through the engine's loop. A body that cannot be read is refused in words of
/// the route's own, which hold nothing of the body: it may hold a token.
async fn account(State(at): State<Accounted>, body: Bytes) -> Response {
    let Ok(asked) = serde_json::from_slice::<AccountAsked>(&body) else {
        return fail(
            StatusCode::BAD_REQUEST,
            "The act cannot be read: it is no adding, taking or removing of an account.".to_string(),
        );
    };
    let done = match asked {
        AccountAsked::Add { name, token } => {
            let added = at.handle.add_account(name, service::Token::new(token)).await;
            added.map(|added| AccountDone {
                at_limit: added.at_limit,
                resets: added.resets.map(|at| at.0),
                not_let_go: None,
            })
        }
        AccountAsked::Take { name } => (at.handle.take_account(name).await).map(|()| AccountDone::default()),
        AccountAsked::Remove { name } => (at.handle.remove_account(name).await).map(|left| AccountDone {
            not_let_go: left,
            ..AccountDone::default()
        }),
    };
    match done {
        Ok(done) => Json(done).into_response(),
        Err(refusal) => refused(&refusal),
    }
}

/// What a request was refused for: its status and why.
struct Refused(StatusCode, String);

/// Reads and writes the disk on a thread of its own, off the threads that serve requests, and answers with what it
/// made or why it was refused.
async fn off_the_loop<T, F>(f: F) -> Response
where
    T: Serialize + Send + 'static,
    F: FnOnce() -> Result<T, Refused> + Send + 'static,
{
    match on_a_thread(f).await {
        Ok(made) => Json(made).into_response(),
        Err(Refused(status, why)) => fail(status, why),
    }
}

/// Reads and writes the disk on a thread of its own, off the threads that serve requests: what it made, or why it was
/// refused.
async fn on_a_thread<T, F>(f: F) -> Result<T, Refused>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, Refused> + Send + 'static,
{
    tokio::task::spawn_blocking(f).await.unwrap_or_else(|e| {
        Err(Refused(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("The settings cannot be read: {e}."),
        ))
    })
}

/// The file's text, or why it cannot be read.
fn text_of(file: &Path) -> Result<String, Refused> {
    std::fs::read_to_string(file).map_err(|e| {
        Refused(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("{} cannot be read: {e}.", file.display()),
        )
    })
}

/// A digest of a text, which changes whenever the text does.
fn digest(text: &str) -> String {
    use std::hash::{Hash, Hasher};
    // Two digests of the standard hasher, of the text and of its length, so that a change is not missed by chance.
    let mut first = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut first);
    let mut second = std::collections::hash_map::DefaultHasher::new();
    (text.len(), text).hash(&mut second);
    format!("{:016x}{:016x}", first.finish(), second.finish())
}

/// The file as it writes the settings, and what the engine runs with beside it, holding these slots; or why it cannot
/// be shown.
pub fn page_of(install: &Install, slots: usize) -> Result<SettingsPage, String> {
    shown(install, slots).map_err(|Refused(_, why)| why)
}

/// The settings the engine runs with now: those it read as it started, but for the slots, which it takes at once as the
/// page changes them, and holds as these.
fn running(install: &Install, slots: usize) -> JsonValue {
    let mut running = install.running.clone();
    let [table, key] = SLOTS;
    if let Some(all) = running.as_object_mut() {
        let table = all
            .entry(table)
            .or_insert_with(|| JsonValue::Object(Default::default()));
        if let Some(table) = table.as_object_mut() {
            table.insert(key.to_string(), JsonValue::from(slots));
        }
    }
    running
}

/// The file as it writes the settings, and what the engine runs with beside it, holding these slots.
fn shown(install: &Install, slots: usize) -> Result<SettingsPage, Refused> {
    let text = text_of(&install.file)?;
    let doc: DocumentMut = text.parse().map_err(|e| {
        Refused(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("{} is not TOML: {e}", install.file.display()),
        )
    })?;
    let read = (install.read)(&text);
    let running = running(install, slots);
    let mut tables = written(&doc);
    // Each value written beside the engine's, where they differ as the engine reads them: `5` and `5.0` are one value
    // to it, and a table written in part is filled with what the engine takes for the rest. Where the file does not
    // read, its values are taken as TOML writes them.
    for table in &mut tables {
        let base = path_of(&table.name);
        for key in &mut table.keys {
            let path: Vec<String> = base.iter().cloned().chain(path_of(&key.key)).collect();
            let running = at(&running, &path);
            let written = match &read {
                Ok(read) => at(read, &path).cloned(),
                Err(_) => key.written.as_deref().and_then(json_of),
            };
            if running != written.as_ref() {
                key.running = Some(running.and_then(toml_text).unwrap_or_else(|| "none".to_string()));
            }
            key.change = change_of(&path);
        }
    }
    left_out(&running, &mut Vec::new(), &doc, &mut tables);
    let levels = (doc.get("agents").and_then(|agents| agents.get("levels")))
        .and_then(Item::as_table_like)
        .map(|levels| levels.iter().map(|(name, _)| name.to_string()).collect())
        .unwrap_or_default();
    Ok(SettingsPage {
        file: install.file.display().to_string(),
        base: digest(&text),
        unreadable: read.err(),
        tables,
        levels,
        git: kept(install).map_err(|why| Refused(StatusCode::INTERNAL_SERVER_ERROR, why))?,
    })
}

/// A key's parts as written: split at the dots that are not inside quotes.
fn path_of(written: &str) -> Vec<String> {
    if written.is_empty() {
        return Vec::new();
    }
    let mut parts = vec![String::new()];
    let mut quoted: Option<char> = None;
    for c in written.chars() {
        match (quoted, c) {
            (None, '"' | '\'') => quoted = Some(c),
            (Some(q), c) if c == q => quoted = None,
            (None, '.') => parts.push(String::new()),
            (None, ' ') => {}
            _ => parts.last_mut().expect("there is a part").push(c),
        }
    }
    parts
}

/// A key as TOML writes it: bare where it may be, else quoted.
fn key_text(key: &str) -> String {
    if !key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        key.to_string()
    } else {
        Value::from(key).to_string().trim().to_string()
    }
}

/// The file's tables, in the order it writes them, each with the keys it writes in it.
fn written(doc: &DocumentMut) -> Vec<SettingsTable> {
    let mut found: Vec<(isize, SettingsTable)> = Vec::new();
    gather(doc.as_table(), &mut Vec::new(), &mut found);
    found.sort_by_key(|(position, _)| *position);
    found.into_iter().map(|(_, table)| table).collect()
}

/// Gathers a table of the file with its keys, and each table under it with its own; a table with no header of its
/// own, as one only named by the headers under it, is gathered only where it holds keys itself.
fn gather(table: &TomlTable, path: &mut Vec<String>, found: &mut Vec<(isize, SettingsTable)>) {
    let mut keys = Vec::new();
    values(table, &mut Vec::new(), &mut keys);
    if !keys.is_empty() || (!table.is_implicit() && !path.is_empty()) {
        found.push((
            table
                .position()
                .unwrap_or(if path.is_empty() { -1 } else { isize::MAX }),
            SettingsTable {
                name: path.iter().map(|part| key_text(part)).collect::<Vec<_>>().join("."),
                left_out: false,
                keys,
            },
        ));
    }
    for (name, item) in table.iter() {
        if let Some(inner) = item.as_table()
            && !inner.is_dotted()
        {
            path.push(name.to_string());
            gather(inner, path, found);
            path.pop();
        }
    }
}

/// The values a table writes, those written under a dotted key with the key's parts before them.
fn values(table: &TomlTable, prefix: &mut Vec<String>, keys: &mut Vec<SettingsKey>) {
    for (name, item) in table.iter() {
        match item {
            Item::Value(value) => {
                let mut bare = value.clone();
                bare.decor_mut().clear();
                let key: Vec<String> = prefix.iter().cloned().chain([name.to_string()]).collect();
                keys.push(SettingsKey {
                    key: key.iter().map(|part| key_text(part)).collect::<Vec<_>>().join("."),
                    written: Some(bare.to_string().trim().to_string()),
                    running: None,
                    change: None,
                });
            }
            Item::Table(inner) if inner.is_dotted() => {
                prefix.push(name.to_string());
                values(inner, prefix, keys);
                prefix.pop();
            }
            _ => {}
        }
    }
}

/// The value of a key, its text as TOML writes it, as JSON writes it.
fn json_of(text: &str) -> Option<JsonValue> {
    let table: toml::Table = toml::from_str(&format!("v = {text}")).ok()?;
    serde_json::to_value(table.get("v")?).ok()
}

/// The value at this path of a value as JSON writes it; none where there is none, or it is null.
fn at<'a>(value: &'a JsonValue, path: &[String]) -> Option<&'a JsonValue> {
    let mut value = value;
    for part in path {
        value = value.get(part)?;
    }
    (!value.is_null()).then_some(value)
}

/// A value as JSON writes it, as TOML writes it; none for null.
fn toml_text(value: &JsonValue) -> Option<String> {
    Some(match value {
        JsonValue::Null => return None,
        JsonValue::Bool(b) => b.to_string(),
        JsonValue::Number(n) => n.to_string(),
        JsonValue::String(s) => Value::from(s.as_str()).to_string().trim().to_string(),
        JsonValue::Array(items) => format!(
            "[{}]",
            items.iter().filter_map(toml_text).collect::<Vec<_>>().join(", ")
        ),
        JsonValue::Object(keys) => {
            let keys: Vec<String> = (keys.iter())
                .filter_map(|(key, value)| toml_text(value).map(|text| format!("{} = {text}", key_text(key))))
                .collect();
            if keys.is_empty() {
                "{}".to_string()
            } else {
                format!("{{ {} }}", keys.join(", "))
            }
        }
    })
}

/// Adds the keys the engine runs with a value for that the file leaves out, each to the table of its path, which is
/// added after the file's own where the file leaves it out too.
fn left_out(running: &JsonValue, path: &mut Vec<String>, doc: &DocumentMut, tables: &mut Vec<SettingsTable>) {
    let JsonValue::Object(keys) = running else {
        return;
    };
    for (name, value) in keys {
        if value.is_null() {
            continue;
        }
        path.push(name.clone());
        let written = written_at(doc, path);
        match (written, value) {
            // Written as a value, whole: compared with it already.
            (Some(Written::Value), _) => {}
            // A table the file writes or leaves out: its keys one by one.
            (Some(Written::Table) | None, JsonValue::Object(_)) => left_out(value, path, doc, tables),
            (None, _) => {
                let (key, table) = path.split_last().expect("the path holds the key");
                let name = table.iter().map(|part| key_text(part)).collect::<Vec<_>>().join(".");
                let row = SettingsKey {
                    key: key_text(key),
                    written: None,
                    running: toml_text(value),
                    change: change_of(path),
                };
                match tables.iter_mut().find(|t| t.name == name) {
                    Some(t) => t.keys.push(row),
                    None => tables.push(SettingsTable {
                        name,
                        left_out: true,
                        keys: vec![row],
                    }),
                }
            }
            (Some(Written::Table), _) => {}
        }
        path.pop();
    }
}

enum Written {
    Value,
    Table,
}

/// What the file writes at a path: a value, a table, or nothing.
fn written_at(doc: &DocumentMut, path: &[String]) -> Option<Written> {
    let mut item = doc.as_item();
    for part in path {
        item = item.as_table_like()?.get(part)?;
    }
    Some(match item {
        Item::Table(_) => Written::Table,
        _ => Written::Value,
    })
}

/// Runs git in the directory, with the engine's settings for it; gives what it printed, or why it failed.
fn git(install: &Install, dir: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(install.git)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|e| format!("git cannot be run: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

/// The directory the file is in, and its name there.
fn place(install: &Install) -> Result<(PathBuf, String), String> {
    let dir = (install.file.parent())
        .map(|dir| {
            if dir.as_os_str().is_empty() {
                Path::new(".")
            } else {
                dir
            }
        })
        .ok_or_else(|| format!("{} is in no directory", install.file.display()))?;
    let name = (install.file.file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .ok_or_else(|| format!("{} names no file", install.file.display()))?;
    Ok((dir.to_path_buf(), name))
}

/// How git keeps the file; none where the directory it is in is no work tree of git, or does not keep it.
fn kept(install: &Install) -> Result<Option<Kept>, String> {
    let (dir, name) = place(install)?;
    if git(install, &dir, &["rev-parse", "--is-inside-work-tree"]).is_err()
        || git(install, &dir, &["ls-files", "--error-unmatch", "--", &name]).is_err()
    {
        return Ok(None);
    }
    let status = git(install, &dir, &["status", "--porcelain", "--", &name])?;
    Ok(Some(Kept {
        committed: status.trim().is_empty(),
    }))
}

/// Writes the owner's change into the file and commits it, saying what changed, with the slots the engine is to take at
/// once: where the change sets them and the file was written, as the file then gives them. Or says why it is refused.
fn changed(install: &Install, change: &SettingsChange) -> Result<(Saved, Option<usize>), Refused> {
    let refused = |why: String| Refused(StatusCode::UNPROCESSABLE_ENTITY, why);
    let _writing = WRITING.lock().unwrap_or_else(|e| e.into_inner());
    let text = text_of(&install.file)?;
    let mut doc: DocumentMut = text
        .parse()
        .map_err(|e| refused(format!("the file is not TOML: {e}")))?;
    let levels: Vec<String> = (doc.get("agents").and_then(|agents| agents.get("levels")))
        .and_then(Item::as_table_like)
        .map(|levels| levels.iter().map(|(name, _)| name.to_string()).collect())
        .unwrap_or_default();
    let mut sets = Vec::new();
    for set in &change.set {
        let path = path_of(&set.key);
        let Some(kind) = change_of(&path) else {
            return Err(refused(format!("{} is changed only in the file", set.key)));
        };
        let value = valued(&set.key, &kind, &set.value, &levels).map_err(refused)?;
        sets.push((path, value));
    }
    let (saved, read) = write(
        install,
        &text,
        &mut doc,
        &change.base,
        &sets,
        "The owner changed the settings on the page",
    )?;
    let slots = (read.filter(|_| sets.iter().any(|(path, _)| *path == SLOTS)))
        .and_then(|read| at(&read, &SLOTS.map(String::from)).and_then(JsonValue::as_u64))
        .and_then(|slots| usize::try_from(slots).ok());
    Ok((saved, slots))
}

/// The value asked for a key, checked against what the key may be; or why it may not be that.
fn valued(key: &str, kind: &Change, value: &JsonValue, levels: &[String]) -> Result<Value, String> {
    match (kind, value) {
        (Change::Name, JsonValue::String(name)) if !name.trim().is_empty() && !name.contains(['\n', '\r']) => {
            Ok(Value::from(name.as_str()))
        }
        (Change::Name, _) => Err(format!("{key} is a name on one line")),
        (Change::Count, JsonValue::Number(n)) if n.as_i64().is_some_and(|n| n >= 0) => {
            Ok(Value::from(n.as_i64().unwrap_or_default()))
        }
        (Change::Count, _) => Err(format!("{key} is a whole number")),
        (Change::Level, JsonValue::String(level)) if levels.contains(level) => Ok(Value::from(level.as_str())),
        (Change::Level, _) => Err(format!("{key} is one of the levels: {}", levels.join(", "))),
    }
}

/// Sets each key to its value in the document, its comments and its layout kept, and writes it as the file where the
/// file has not changed since `base`, it then reads as settings with each value as set, and its directory's git has
/// no change of it not committed; commits it there, saying what was set, and gives the settings the file then gives as
/// the engine reads them, none where nothing was set. A commit that fails puts the file back.
fn write(
    install: &Install,
    text: &str,
    doc: &mut DocumentMut,
    base: &str,
    sets: &[(Vec<String>, Value)],
    saying: &str,
) -> Result<(Saved, Option<JsonValue>), Refused> {
    let conflict = |why: String| Refused(StatusCode::CONFLICT, why);
    let refused = |why: String| Refused(StatusCode::UNPROCESSABLE_ENTITY, why);
    if digest(text) != base {
        return Err(conflict(format!(
            "{} changed since the page read it: read it again, and change it then",
            install.file.display()
        )));
    }
    let git_kept = kept(install).map_err(refused)?;
    if git_kept.is_some_and(|kept| !kept.committed) {
        return Err(conflict(format!(
            "{} has changes not committed in its directory: commit them or drop them first",
            install.file.display()
        )));
    }
    let mut said = Vec::new();
    for (path, value) in sets {
        let was = written_value(doc, path);
        let now = value.to_string().trim().to_string();
        if was.as_deref() == Some(now.as_str()) {
            continue;
        }
        set(doc, path, value.clone()).map_err(refused)?;
        let key = path.iter().map(|part| key_text(part)).collect::<Vec<_>>().join(".");
        said.push(match was {
            Some(was) => format!("{key} = {now} instead of {was}"),
            None => format!("{key} = {now}"),
        });
    }
    if said.is_empty() {
        let saved = Saved {
            said: "nothing changed".to_string(),
            committed: false,
        };
        return Ok((saved, None));
    }
    let new = doc.to_string();
    let read = (install.read)(&new).map_err(|why| refused(format!("the settings would not read: {why}")))?;
    for (path, value) in sets {
        let asked = json_of(value.to_string().trim());
        if at(&read, path) != asked.as_ref() {
            return Err(refused(format!(
                "{} would not be as asked",
                path.iter().map(|part| key_text(part)).collect::<Vec<_>>().join(".")
            )));
        }
    }
    replace(&install.file, &new).map_err(|why| Refused(StatusCode::INTERNAL_SERVER_ERROR, why))?;
    let said = said.join("; ");
    let committed = match git_kept {
        None => false,
        Some(_) => {
            let (dir, name) = place(install).map_err(refused)?;
            let message = format!("{saying}: {said}");
            let commit = git(install, &dir, &["add", "--", &name])
                .and_then(|_| git(install, &dir, &["commit", "-q", "-m", &message, "--", &name]));
            if let Err(why) = commit {
                // Nothing half done is left: the file goes back to what it was, in the index too.
                let _ = replace(&install.file, text);
                let _ = git(install, &dir, &["reset", "-q", "--", &name]);
                return Err(Refused(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("the change could not be committed, and the file is as it was: {why}"),
                ));
            }
            true
        }
    };
    Ok((Saved { said, committed }, Some(read)))
}

/// The value written at a path, as TOML writes it; none where nothing is.
fn written_value(doc: &DocumentMut, path: &[String]) -> Option<String> {
    let mut item = doc.as_item();
    for part in path {
        item = item.as_table_like()?.get(part)?;
    }
    let mut value = item.as_value()?.clone();
    value.decor_mut().clear();
    Some(value.to_string().trim().to_string())
}

/// Sets the value at a path, keeping what is written round the value there; a table on the path the file leaves out
/// is added with a header of its own.
fn set(doc: &mut DocumentMut, path: &[String], value: Value) -> Result<(), String> {
    let (key, tables) = path.split_last().ok_or("no key is named")?;
    let mut table: &mut dyn TableLike = doc.as_table_mut();
    for part in tables {
        let item = table.entry(part).or_insert(toml_edit::table());
        table = item
            .as_table_like_mut()
            .ok_or_else(|| format!("{part} is written as a value, not a table"))?;
    }
    match table.get_mut(key) {
        Some(Item::Value(old)) => {
            let decor = old.decor().clone();
            *old = value;
            *old.decor_mut() = decor;
        }
        Some(_) => return Err(format!("{key} is written as a table, not a value")),
        None => {
            table.insert(key, Item::Value(value));
        }
    }
    Ok(())
}

/// Writes the text as the file: into a file beside it first, which then takes its place, so that the file is never
/// half written.
fn replace(file: &Path, text: &str) -> Result<(), String> {
    let beside = file.with_extension("toml.writing");
    std::fs::write(&beside, text).map_err(|e| format!("{}: {e}", beside.display()))?;
    std::fs::rename(&beside, file).map_err(|e| format!("{}: {e}", file.display()))
}

#[cfg(test)]
mod tests;
