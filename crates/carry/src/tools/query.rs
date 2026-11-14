//! The tool `query`: one read-only SQL query on the engine's store, run here by the engine, which holds the store; the
//! agent only asks for it in its return and is answered in its next message. The connection refuses every write itself
//! (it is opened read-only, its `query_only` is on, and its authorizer allows nothing but a select of what agents may
//! read), and a query is bounded in rows, in time and in bytes. The words an agent is told, and each place the tool
//! speaks of its own answers, are the tool's file in the flow (`agents/tools/query.toml`); what the code makes is JSON
//! and a query.
//!
//! The answer is `{"rows": [...]}`, each row an object by the column names. Where the rows past a bound were not kept,
//! it holds `"cut": {"count": n, "rest": "<query>"}` too; and a message gives a result longer than the tool's file
//! says cut to its first rows, with `"cut": {"count": n, "bytes": <the whole's size>, "rest": "<query>"}`, and
//! `"trimmed": true` where the first row alone was too long and its long values were shortened. The whole is kept in
//! the records as it is answered here.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use flow::agents::{Tool, fill};
use rusqlite::hooks::{AuthAction, AuthContext, Authorization};
use rusqlite::limits::Limit;
use rusqlite::types::ValueRef;
use rusqlite::{Connection, ErrorCode, InterruptHandle, OpenFlags};
use serde_json::{Map, Value, json};

use crate::settings;

/// The name of the tool.
pub const QUERY: &str = "query";

/// What the comment of a table or of a column says to keep it from the agents: it is not described to them and a query
/// of theirs may not read it. Matched without regard to case.
pub const HIDDEN: &str = "hidden from agents";

/// A table of the store, as `db/schema.sql` says it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Table {
    pub name: String,
    /// Its comment, the lines joined by a space.
    pub comment: String,
    pub columns: Vec<Column>,
}

/// A column of a table of the store.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Column {
    pub name: String,
    /// Its type as the schema writes it, empty where it has none.
    pub kind: String,
    /// The comment above it and the comment after it on its line, joined by a space.
    pub comment: String,
}

impl Table {
    /// Whether its comment keeps it from the agents.
    pub fn hidden(&self) -> bool {
        hides(&self.comment)
    }
}

impl Column {
    /// Whether its comment keeps it from the agents.
    pub fn hidden(&self) -> bool {
        hides(&self.comment)
    }
}

fn hides(comment: &str) -> bool {
    comment.to_lowercase().contains(HIDDEN)
}

/// The tables of a schema, with the comment above each and above and after each of its columns. A comment is the
/// lines of `--` directly above what it is of, with no blank line between.
pub fn tables(schema: &str) -> Vec<Table> {
    let mut found = Vec::new();
    let mut comment: Vec<String> = Vec::new();
    let mut open: Option<Table> = None;
    for line in schema.lines() {
        let line = line.trim();
        if let Some(table) = open.as_mut() {
            if line.starts_with(')') {
                found.extend(open.take());
            } else if let Some(said) = line.strip_prefix("--") {
                comment.push(said.trim().to_string());
            } else {
                let (code, after) = line
                    .split_once("--")
                    .map_or((line, None), |(c, a)| (c.trim(), Some(a.trim())));
                let mut words = code.split_whitespace();
                let name = words.next().unwrap_or_default();
                let constraint = ["PRIMARY", "UNIQUE", "FOREIGN", "CHECK", "CONSTRAINT"].contains(&name);
                if !name.is_empty() && !constraint {
                    comment.extend(after.map(str::to_string));
                    table.columns.push(Column {
                        name: name.to_string(),
                        kind: words.next().unwrap_or_default().trim_end_matches(',').to_string(),
                        comment: comment.join(" "),
                    });
                }
                comment.clear();
            }
            continue;
        }
        if let Some(said) = line.strip_prefix("--") {
            comment.push(said.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("CREATE TABLE ") {
            let name = rest
                .split(|c: char| c.is_whitespace() || c == '(')
                .next()
                .unwrap_or_default();
            open = Some(Table {
                name: name.to_string(),
                comment: comment.join(" "),
                columns: Vec::new(),
            });
            comment.clear();
        } else {
            comment.clear();
        }
    }
    found
}

/// What is given for `{store}` where the store is open: each table agents may read, with each of its columns they may
/// read, as the tool's file lays it out (`store_table`, with its columns each as `store_column`, or
/// `store_column_commented` where the schema has a comment on it), from the comments of the schema.
pub fn describe(schema: &str, texts: &BTreeMap<String, String>) -> String {
    let text = |name: &str| texts.get(name).map_or("", String::as_str);
    let described: Vec<String> = (tables(schema).iter().filter(|t| !t.hidden()))
        .map(|table| {
            let columns: Vec<String> = (table.columns.iter().filter(|c| !c.hidden()))
                .map(|column| {
                    let which = if column.comment.is_empty() {
                        "store_column"
                    } else {
                        "store_column_commented"
                    };
                    let slots = [
                        ("column", column.name.as_str()),
                        ("type", column.kind.as_str()),
                        ("comment", column.comment.as_str()),
                    ];
                    fill(text(which), &slots)
                })
                .collect();
            let slots = [
                ("table", table.name.as_str()),
                ("comment", table.comment.as_str()),
                ("columns", &columns.join("\n")),
            ];
            fill(text("store_table"), &slots)
        })
        .collect();
    described.join("\n\n")
}

/// The bounds of a query.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub max_rows: usize,
    pub timeout: Duration,
    /// The most bytes a result is kept with, and the most one row may take.
    pub max_bytes: usize,
    /// The longest a string or a blob may be, whatever makes it. A function's cost grows with the length of what it is
    /// given, and the progress handler that holds a query to its time runs between the VM's instructions, so that one
    /// call over very long values is not stopped by it: no value is longer than this, which holds every call to what
    /// such values cost, however it is made, with no function named.
    pub max_value_bytes: usize,
}

impl Limits {
    fn of(settings: &settings::Query) -> Self {
        Limits {
            max_rows: settings.max_rows,
            timeout: Duration::from_millis(settings.timeout_ms),
            max_bytes: settings.max_bytes,
            max_value_bytes: settings.max_value_bytes,
        }
    }
}

/// Stops a query that runs: told, it interrupts the connection, which stops it at the VM's next instruction.
#[derive(Default)]
pub struct Cancel {
    told: AtomicBool,
    handle: Mutex<Option<InterruptHandle>>,
}

impl Cancel {
    /// The query is to stop.
    pub fn tell(&self) {
        self.told.store(true, Ordering::SeqCst);
        let handle = self.handle.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(handle) = handle.as_ref() {
            handle.interrupt();
        }
    }

    fn told(&self) -> bool {
        self.told.load(Ordering::SeqCst)
    }

    /// The connection the query runs on, which is interrupted at once where it was told to stop already.
    fn on(&self, handle: InterruptHandle) {
        let mut held = self.handle.lock().unwrap_or_else(PoisonError::into_inner);
        if self.told() {
            handle.interrupt();
        }
        *held = Some(handle);
    }
}

/// The store as it is open to queries.
#[derive(Clone, Debug)]
struct Open {
    store: PathBuf,
    limits: Limits,
    /// The tables a query may not read.
    hidden: Vec<String>,
    /// The columns a query may not read, each of a table.
    hidden_columns: Vec<(String, String)>,
}

/// The tool `query` on this installation: its file in the flow, and the store where the settings open it.
#[derive(Clone, Debug)]
pub struct Querying {
    words: String,
    texts: BTreeMap<String, String>,
    schema: String,
    open: Option<Open>,
}

impl Querying {
    /// The tool as it is before the settings are known: closed, and saying nothing of its answers.
    pub fn closed() -> Self {
        Querying {
            words: String::new(),
            texts: BTreeMap::new(),
            schema: String::new(),
            open: None,
        }
    }

    /// The tool as the settings and the flow's file for it say, on the store in this data directory, which `schema`
    /// describes. Open to the agents it is refused, in the words of the file, while the schema hides no table and no
    /// column from them: what they may see is the owner's to mark, and none is not that. Without the tool's file
    /// nothing is told, and nothing is said of an answer.
    pub fn new(settings: &settings::Query, tool: Option<&Tool>, data: &Path, schema: &str) -> Result<Self, String> {
        let texts = tool.map(|t| t.texts.clone()).unwrap_or_default();
        let mut querying = Querying {
            words: tool.map(|t| t.words.clone()).unwrap_or_default(),
            texts,
            schema: schema.to_string(),
            open: None,
        };
        if settings.open {
            let all = tables(schema);
            let hidden_columns: Vec<(String, String)> = (all.iter())
                .flat_map(|t| {
                    t.columns
                        .iter()
                        .filter(|c| c.hidden())
                        .map(|c| (t.name.clone(), c.name.clone()))
                })
                .collect();
            let hidden: Vec<String> = all.iter().filter(|t| t.hidden()).map(|t| t.name.clone()).collect();
            if hidden.is_empty() && hidden_columns.is_empty() {
                return Err(querying.text("hides_nothing", &[]));
            }
            querying.open = Some(Open {
                store: data.join(store::FILE),
                limits: Limits::of(settings),
                hidden,
                hidden_columns,
            });
        }
        Ok(querying)
    }

    /// What fills `{store}` in the tool's words: what the schema's comments say of the store where it is open, and the
    /// file's own answer that it is not where it is not.
    pub fn store(&self) -> String {
        match &self.open {
            Some(_) => describe(&self.schema, &self.texts),
            None => self.text("closed", &[]),
        }
    }

    /// The tool's words, `{store}` filled.
    pub fn words(&self) -> String {
        fill(&self.words, &[("store", &self.store())])
    }

    fn text(&self, name: &str, slots: &[(&str, &str)]) -> String {
        fill(self.texts.get(name).map_or("", String::as_str), slots)
    }

    /// What is said where the query could not be run at all, which the library's own text is never.
    pub fn unavailable(&self) -> String {
        self.text("unavailable", &[])
    }

    /// The answer to a request, as `{"sql": "..."}`. It blocks while the query runs, within its time limit, or until
    /// `cancel` is told; where the store is not open it touches nothing.
    pub fn answer(&self, input: &str, cancel: &Arc<Cancel>) -> String {
        let Some(open) = &self.open else {
            return self.text("closed", &[]);
        };
        let sql = match serde_json::from_str::<Value>(input) {
            Ok(Value::Object(request)) => match request.get("sql") {
                Some(Value::String(sql)) => sql.clone(),
                _ => return self.text("unreadable", &[]),
            },
            _ => return self.text("unreadable", &[]),
        };
        match run(open, &sql, cancel) {
            Ok(result) => result.to_string(),
            Err(Failed::Timeout) => self.text("timeout", &[("ms", &open.limits.timeout.as_millis().to_string())]),
            Err(Failed::Store) => self.unavailable(),
            Err(Failed::Row) => self.text("row_too_long", &[("bytes", &open.limits.max_bytes.to_string())]),
            Err(Failed::Sql(why)) => self.text("error", &[("why", &why)]),
        }
    }

    /// What a message gives of a result: the result, or where it is longer than the tool's file says a message gives,
    /// the first of its rows that fit with the size of the whole and the query that reads the rest. Where the first
    /// row alone does not fit, it is given with its long values shortened, so that the rest is the query from the
    /// row after it and each cut makes progress.
    pub fn cut(result: &str, input: &str, given_bytes: usize) -> String {
        if result.len() <= given_bytes {
            return result.to_string();
        }
        let Ok(Value::Object(mut whole)) = serde_json::from_str::<Value>(result) else {
            return result.to_string();
        };
        let Some(Value::Array(rows)) = whole.remove("rows") else {
            return result.to_string();
        };
        let Some(sql) = (serde_json::from_str::<Value>(input).ok()).and_then(|r| r["sql"].as_str().map(str::to_string))
        else {
            return result.to_string();
        };
        let mut room = given_bytes;
        let mut first: Vec<Value> = (rows.iter())
            .take_while(|row| {
                let size = row.to_string().len() + 1;
                let fits = size <= room;
                room = room.saturating_sub(size);
                fits
            })
            .cloned()
            .collect();
        let trimmed = first.is_empty() && !rows.is_empty();
        if trimmed {
            first.push(shortened(&rows[0], given_bytes));
        }
        let count = first.len();
        let mut cut = json!({ "count": count, "bytes": result.len(), "rest": rest(&sql, count) });
        if trimmed {
            cut["trimmed"] = json!(true);
        }
        json!({ "rows": first, "cut": cut }).to_string()
    }
}

/// A row with each of its strings no longer than its share of these bytes, at least a few characters.
fn shortened(row: &Value, bytes: usize) -> Value {
    let Value::Object(fields) = row else {
        return row.clone();
    };
    let share = (bytes / fields.len().max(1)).saturating_sub(32).max(16);
    let short = |text: &str| {
        let mut end = share.min(text.len());
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text[..end].to_string()
    };
    let cut: Map<String, Value> = (fields.iter())
        .map(|(name, value)| match value {
            Value::String(text) => (short(name), Value::String(short(text))),
            other => (short(name), other.clone()),
        })
        .collect();
    Value::Object(cut)
}

/// Why a query gave no rows.
enum Failed {
    Timeout,
    /// The store could not be read: not the query's fault, and not the library's words to tell.
    Store,
    /// A row was longer than the bytes a result is kept with.
    Row,
    /// The SQL's own failure, in SQLite's words.
    Sql(String),
}

/// The query that reads the rows of this one from this many on.
fn rest(sql: &str, from: usize) -> String {
    format!("SELECT * FROM (\n{}\n) LIMIT -1 OFFSET {from}", statement(sql))
}

/// The text of a statement without what follows its last token: the whitespace, the semicolons and the comments there,
/// so that it can stand in a subquery.
fn statement(sql: &str) -> &str {
    let b = sql.as_bytes();
    let (mut at, mut end) = (0, 0);
    while at < b.len() {
        let next = b.get(at + 1).copied();
        match b[at] {
            quote @ (b'\'' | b'"' | b'`') => {
                at += 1;
                while at < b.len() {
                    if b[at] == quote {
                        if next_is(b, at + 1, quote) {
                            at += 2;
                            continue;
                        }
                        break;
                    }
                    at += 1;
                }
                at += 1;
                end = at.min(b.len());
            }
            b'[' => {
                while at < b.len() && b[at] != b']' {
                    at += 1;
                }
                at += 1;
                end = at.min(b.len());
            }
            b'-' if next == Some(b'-') => {
                while at < b.len() && b[at] != b'\n' {
                    at += 1;
                }
            }
            b'/' if next == Some(b'*') => {
                at += 2;
                while at < b.len() && !(b[at] == b'*' && next_is(b, at + 1, b'/')) {
                    at += 1;
                }
                at += 2;
            }
            b';' => at += 1,
            c if c.is_ascii_whitespace() => at += 1,
            _ => {
                at += 1;
                end = at;
            }
        }
    }
    sql[..end].trim_start()
}

fn next_is(b: &[u8], at: usize, byte: u8) -> bool {
    b.get(at) == Some(&byte)
}

/// The store's file opened for a query, read-only as SQLite opens a file, and with `query_only` on, which refuses every
/// write by itself; with nothing attached, and no value longer than the bytes a value may be.
fn open_read_only(open: &Open) -> rusqlite::Result<Connection> {
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let conn = Connection::open_with_flags(&open.store, flags)?;
    conn.busy_timeout(open.limits.timeout)?;
    conn.pragma_update(None, "query_only", true)?;
    conn.set_limit(Limit::SQLITE_LIMIT_ATTACHED, 0)?;
    let value = i32::try_from(open.limits.max_value_bytes).unwrap_or(i32::MAX);
    conn.set_limit(Limit::SQLITE_LIMIT_LENGTH, value)?;
    // A pattern of LIKE and GLOB costs the length of the text it is matched to for every character of itself.
    conn.set_limit(Limit::SQLITE_LIMIT_LIKE_PATTERN_LENGTH, 100)?;
    Ok(conn)
}

/// The connection of a query: [`open_read_only`], and an authorizer that allows only a select, the reading of what is
/// not hidden from agents, and functions; and the time and the stop the query is held to.
fn connect(open: &Open, cancel: &Arc<Cancel>) -> rusqlite::Result<Connection> {
    let mut conn = open_read_only(open)?;
    let (hidden, hidden_columns) = (open.hidden.clone(), open.hidden_columns.clone());
    conn.authorizer(Some(move |ctx: AuthContext<'_>| match ctx.action {
        AuthAction::Select | AuthAction::Recursive | AuthAction::Function { .. } => Authorization::Allow,
        AuthAction::Read {
            table_name,
            column_name,
        } => {
            let is = |name: &str, other: &str| name.eq_ignore_ascii_case(other);
            let refused = table_name.starts_with("sqlite_")
                || hidden.iter().any(|h| is(h, table_name))
                || hidden_columns
                    .iter()
                    .any(|(t, c)| is(t, table_name) && is(c, column_name));
            if refused {
                Authorization::Deny
            } else {
                Authorization::Allow
            }
        }
        _ => Authorization::Deny,
    }))?;
    let (deadline, told) = (Instant::now() + open.limits.timeout, cancel.clone());
    conn.progress_handler(1000, Some(move || Instant::now() > deadline || told.told()))?;
    cancel.on(conn.get_interrupt_handle());
    Ok(conn)
}

/// Why a failure of the library is the query's or not: only SQL's own failures are told in SQLite's words.
fn failed(error: rusqlite::Error) -> Failed {
    let Some(code) = error.sqlite_error_code() else {
        return Failed::Sql(error.to_string());
    };
    match code {
        ErrorCode::OperationInterrupted => Failed::Timeout,
        ErrorCode::CannotOpen
        | ErrorCode::SystemIoFailure
        | ErrorCode::DatabaseBusy
        | ErrorCode::DatabaseLocked
        | ErrorCode::DatabaseCorrupt
        | ErrorCode::NotADatabase
        | ErrorCode::DiskFull
        | ErrorCode::OutOfMemory
        | ErrorCode::NotFound => Failed::Store,
        _ => Failed::Sql(error.to_string()),
    }
}

/// Runs the query and gives its rows within the bounds.
fn run(open: &Open, sql: &str, cancel: &Arc<Cancel>) -> Result<Value, Failed> {
    // Whatever goes wrong in opening the connection is not the query's.
    let conn = connect(open, cancel).map_err(|_| Failed::Store)?;
    let mut statement = conn.prepare(sql).map_err(failed)?;
    let names = columns(&statement);
    let mut found = statement.query([]).map_err(failed)?;
    let mut rows: Vec<Value> = Vec::new();
    let mut bytes = 0;
    let mut more = false;
    while let Some(row) = found.next().map_err(failed)? {
        let mut object = Map::new();
        let mut size = 2;
        for (at, name) in names.iter().enumerate() {
            let value = json_of(row.get_ref(at).map_err(failed)?);
            size += name.len() + value.to_string().len() + 4;
            // A row is bounded as it is built, so that columns past the bound are not read.
            if size > open.limits.max_bytes {
                return Err(Failed::Row);
            }
            object.insert(name.clone(), value);
        }
        if rows.len() == open.limits.max_rows || bytes + size > open.limits.max_bytes {
            more = true;
            break;
        }
        bytes += size;
        rows.push(Value::Object(object));
    }
    let count = rows.len();
    let mut result = json!({ "rows": rows });
    if more {
        result["cut"] = json!({ "count": count, "rest": rest(sql, count) });
    }
    Ok(result)
}

/// The names of a statement's columns as SQLite names them in a subquery: a name that comes again is followed by `:`
/// and how many of its name came before it, so that the rows of a query and those of the query for its rest are keyed
/// alike.
fn columns(statement: &rusqlite::Statement<'_>) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for name in statement.column_names() {
        let before = names
            .iter()
            .filter(|n| n.as_str() == name || n.strip_prefix(name).is_some_and(|r| r.starts_with(':')))
            .count();
        names.push(if before == 0 {
            name.to_string()
        } else {
            format!("{name}:{before}")
        });
    }
    names
}

fn json_of(value: ValueRef<'_>) -> Value {
    match value {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(i) => json!(i),
        ValueRef::Real(f) => json!(f),
        ValueRef::Text(t) => Value::String(String::from_utf8_lossy(t).into_owned()),
        ValueRef::Blob(b) => {
            let hex: String = b.iter().map(|byte| format!("{byte:02x}")).collect();
            Value::String(format!("x'{hex}'"))
        }
    }
}

#[cfg(test)]
mod tests;
