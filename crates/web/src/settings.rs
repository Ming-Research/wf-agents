//! The installation's settings on the page: its file shown as it writes them, table by table, each key with its value,
//! beside what the engine runs with where that differs; and what the owner changes there written back into the file.
//!
//! - `GET /api/v1/settings`: the file ([`SettingsPage`]).
//! - `POST /api/v1/settings`: the owner's change of a few keys ([`SettingsChange`]), answered with what was changed in
//!   words ([`Saved`]).
//! - `GET /api/v1/settings/accounts`: the accounts the agents' program may run under ([`AccountsPage`]).
//! - `POST /api/v1/settings/accounts`: the owner's choice of another of them ([`AccountChoice`]), answered as a change.
//!
//! The owner changes on the page what is tuned while works go on: the language the agents write in for the owner, the
//! model of each level, the highest level an item may be at, the context past which a line begins a new session, how
//! many rounds run at once, and the account the agents' program runs under. The rest only the file changes. A value
//! is checked here only for its kind; what the engine refuses of it, the engine's own reading of the file refuses. A
//! change is written into the file with its comments and its layout kept, only where the file it makes reads as
//! settings, holds each value as asked, and has not changed since the page read it; where the directory the file is in
//! keeps it in git, the change is committed there, saying what changed, and nothing is written onto a file whose
//! changes are not committed. The engine reads the file as it starts: until it starts again, each key it runs at
//! another value than the file gives, as the engine reads the file, is shown with that value beside it.
//!
//! The settings hold no secret: a token is given to the engine in a variable of its environment, which the file names.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use axum::routing::get;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use toml_edit::{DocumentMut, Item, Table as TomlTable, TableLike, Value};

use crate::interface::fail;

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
    /// The agents' program, for its accounts.
    pub program: Program,
}

/// What the page needs of the agents' program to show its accounts and switch between them.
pub struct Program {
    /// The variable of its environment that names its directory to it.
    pub variable: &'static str,
    /// What the owner types in it to sign a directory in.
    pub sign_in: &'static str,
    /// The account a directory is signed in as, where it is.
    pub signed_in_as: fn(&Path) -> Option<String>,
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

/// The accounts the agents' program may run under: the directories beside the one the file names, named as it is
/// or as it is with a dash and a name after it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountsPage {
    /// The program's command, as the file names it.
    pub command: String,
    /// The variable that names a directory to it, and what is typed in it to sign the directory in.
    pub variable: String,
    pub sign_in: String,
    /// The directory the accounts' directories are in; none where the file names no directory of the program's, when
    /// it runs with its own default.
    pub dir: Option<String>,
    /// The name every account's directory begins with.
    pub stem: String,
    pub accounts: Vec<Account>,
    /// The file's text as it was read, by a digest of it, for a choice made against it.
    pub base: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    /// The directory's name.
    pub name: String,
    pub path: String,
    /// The account it is signed in as; none where it is not signed in.
    pub signed_in_as: Option<String>,
    /// Whether the file names it.
    pub named: bool,
}

/// The owner's choice of the account the agents' program runs under, by its directory's name.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountChoice {
    pub base: String,
    pub name: String,
}

/// The keys the page may change, by their tables and themselves joined by dots, a level's model by any level.
fn change_of(path: &[String]) -> Option<Change> {
    let parts: Vec<&str> = path.iter().map(String::as_str).collect();
    match parts.as_slice() {
        ["owner", "language"] | ["agents", "levels", _, "model"] => Some(Change::Name),
        ["agents", "slots"] | ["sessions", "context_limit"] => Some(Change::Count),
        ["items", "highest_level"] => Some(Change::Level),
        _ => None,
    }
}

/// The key the account the agents' program runs under is written at.
const CONFIG_DIR: [&str; 2] = ["agents", "config_dir"];

/// One writer of the file at a time: two changes at once would each be made on the text the other replaces.
static WRITING: Mutex<()> = Mutex::new(());

/// The routes of the settings.
pub fn router(install: Install) -> Router {
    let install = Arc::new(install);
    Router::new()
        .route("/api/v1/settings", get(page).post(save))
        .route("/api/v1/settings/accounts", get(accounts).post(choose))
        .with_state(install)
}

async fn page(State(install): State<Arc<Install>>) -> Response {
    off_the_loop(move || shown(&install)).await
}

async fn save(State(install): State<Arc<Install>>, body: Bytes) -> Response {
    let change: SettingsChange = match serde_json::from_slice(&body) {
        Ok(change) => change,
        Err(e) => return fail(StatusCode::BAD_REQUEST, format!("The change cannot be read: {e}.")),
    };
    off_the_loop(move || changed(&install, &change)).await
}

async fn accounts(State(install): State<Arc<Install>>) -> Response {
    off_the_loop(move || accounts_of(&install)).await
}

async fn choose(State(install): State<Arc<Install>>, body: Bytes) -> Response {
    let choice: AccountChoice = match serde_json::from_slice(&body) {
        Ok(choice) => choice,
        Err(e) => return fail(StatusCode::BAD_REQUEST, format!("The choice cannot be read: {e}.")),
    };
    off_the_loop(move || chosen(&install, &choice)).await
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
    match tokio::task::spawn_blocking(f).await {
        Ok(Ok(made)) => Json(made).into_response(),
        Ok(Err(Refused(status, why))) => fail(status, why),
        Err(e) => fail(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("The settings cannot be read: {e}."),
        ),
    }
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

/// The file as it writes the settings, and what the engine runs with beside it; or why it cannot be shown.
pub fn page_of(install: &Install) -> Result<SettingsPage, String> {
    shown(install).map_err(|Refused(_, why)| why)
}

/// The file as it writes the settings, and what the engine runs with beside it.
fn shown(install: &Install) -> Result<SettingsPage, Refused> {
    let text = text_of(&install.file)?;
    let doc: DocumentMut = text.parse().map_err(|e| {
        Refused(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("{} is not TOML: {e}", install.file.display()),
        )
    })?;
    let read = (install.read)(&text);
    let mut tables = written(&doc);
    // Each value written beside the engine's, where they differ as the engine reads them: `5` and `5.0` are one value
    // to it, and a table written in part is filled with what the engine takes for the rest. Where the file does not
    // read, its values are taken as TOML writes them.
    for table in &mut tables {
        let base = path_of(&table.name);
        for key in &mut table.keys {
            let path: Vec<String> = base.iter().cloned().chain(path_of(&key.key)).collect();
            let running = at(&install.running, &path);
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
    left_out(&install.running, &mut Vec::new(), &doc, &mut tables);
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

/// Writes the owner's change into the file and commits it, saying what changed; or says why it is refused.
fn changed(install: &Install, change: &SettingsChange) -> Result<Saved, Refused> {
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
    write(
        install,
        &text,
        &mut doc,
        &change.base,
        &sets,
        "The owner changed the settings on the page",
    )
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
/// no change of it not committed; commits it there, saying what was set. A commit that fails puts the file back.
fn write(
    install: &Install,
    text: &str,
    doc: &mut DocumentMut,
    base: &str,
    sets: &[(Vec<String>, Value)],
    saying: &str,
) -> Result<Saved, Refused> {
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
        return Ok(Saved {
            said: "nothing changed".to_string(),
            committed: false,
        });
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
    Ok(Saved { said, committed })
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

/// The accounts the agents' program may run under.
fn accounts_of(install: &Install) -> Result<AccountsPage, Refused> {
    let text = text_of(&install.file)?;
    let doc: DocumentMut = text.parse().map_err(|e| {
        Refused(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("{} is not TOML: {e}", install.file.display()),
        )
    })?;
    let command = (doc.get("agents").and_then(|agents| agents.get("command")))
        .and_then(Item::as_str)
        .or_else(|| install.running["agents"]["command"].as_str())
        .unwrap_or_default()
        .to_string();
    let named = (doc.get("agents").and_then(|agents| agents.get("config_dir")))
        .and_then(Item::as_str)
        .map(PathBuf::from);
    let stem = Path::new(&command)
        .file_name()
        .map_or_else(|| command.clone(), |name| name.to_string_lossy().into_owned());
    let page = |dir: Option<&Path>, accounts: Vec<Account>| AccountsPage {
        command: command.clone(),
        variable: install.program.variable.to_string(),
        sign_in: install.program.sign_in.to_string(),
        dir: dir.map(|dir| dir.display().to_string()),
        stem: stem.clone(),
        accounts,
        base: digest(&text),
    };
    let Some(named) = named else {
        return Ok(page(None, Vec::new()));
    };
    let Some(dir) = named.parent() else {
        return Ok(page(None, Vec::new()));
    };
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let ours = name == stem
                || name
                    .strip_prefix(&stem)
                    .is_some_and(|rest| rest.len() > 1 && rest.starts_with('-'));
            if ours && entry.path().is_dir() {
                dirs.push(entry.path());
            }
        }
    }
    if !dirs.iter().any(|d| same(d, &named)) {
        dirs.push(named.clone());
    }
    dirs.sort();
    let accounts = (dirs.iter())
        .map(|d| Account {
            name: (d.file_name()).map_or_else(String::new, |name| name.to_string_lossy().into_owned()),
            path: d.display().to_string(),
            signed_in_as: (install.program.signed_in_as)(d),
            named: same(d, &named),
        })
        .collect();
    Ok(page(Some(dir), accounts))
}

/// Whether two paths are of one directory.
fn same(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

/// Writes the account's directory as the one the agents' program runs under, where it is signed in.
fn chosen(install: &Install, choice: &AccountChoice) -> Result<Saved, Refused> {
    let refused = |why: String| Refused(StatusCode::UNPROCESSABLE_ENTITY, why);
    let _writing = WRITING.lock().unwrap_or_else(|e| e.into_inner());
    let all = accounts_of(install)?;
    let Some(dir) = &all.dir else {
        return Err(refused(
            "agents.config_dir names no directory: the program runs with its own, and has no accounts beside it"
                .to_string(),
        ));
    };
    let Some(to) = all.accounts.iter().find(|a| a.name == choice.name) else {
        return Err(refused(format!("there is no account {} in {dir}", choice.name)));
    };
    if to.named {
        return Ok(Saved {
            said: format!("the agents run under {} already", to.name),
            committed: false,
        });
    }
    if to.signed_in_as.is_none() {
        return Err(refused(format!(
            "{} is not signed in: run `{}={} {}` and type {} there first",
            to.name, all.variable, to.path, all.command, all.sign_in
        )));
    }
    let text = text_of(&install.file)?;
    let mut doc: DocumentMut = text
        .parse()
        .map_err(|e| refused(format!("the file is not TOML: {e}")))?;
    let path: Vec<String> = CONFIG_DIR.iter().map(|part| part.to_string()).collect();
    write(
        install,
        &text,
        &mut doc,
        &choice.base,
        &[(path, Value::from(to.path.as_str()))],
        "The owner switched the account the agents run under on the page",
    )
}

#[cfg(test)]
mod tests;
