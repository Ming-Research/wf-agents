//! The wf-agents database. `db/schema.sql` is its only definition; this crate carries it into the binary, creates a
//! database from it on request only, opens an existing one and brings it up to the schema when it is older, writes
//! every change to a state table into `changes`, and opens the read-only connection agents query through.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fmt;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::functions::FunctionFlags;
use rusqlite::hooks::{Action, AuthAction, AuthContext, Authorization};
use rusqlite::{Connection, OpenFlags, OptionalExtension, ToSql, Transaction, TransactionBehavior, params};
use serde_json::Value;

pub use rusqlite;

/// The database's definition, as `db/schema.sql` writes it.
pub const SCHEMA: &str = include_str!("../../../db/schema.sql");

/// The steps that bring an existing database up to [`SCHEMA`]: `db/migrations/0002.sql` takes version 1 to 2, the
/// next 2 to 3, and so on. A change to `db/schema.sql` adds one in the same pull request. `db/migrations/0001.sql` is
/// the schema as it was at version 1, from which a test migrates a database and compares it with one created from
/// [`SCHEMA`].
pub const MIGRATIONS: &[&str] = &[
    include_str!("../../../db/migrations/0002.sql"),
    include_str!("../../../db/migrations/0003.sql"),
    include_str!("../../../db/migrations/0004.sql"),
    include_str!("../../../db/migrations/0005.sql"),
    include_str!("../../../db/migrations/0006.sql"),
    include_str!("../../../db/migrations/0007.sql"),
];

/// The version [`SCHEMA`] defines, kept in the database's `user_version`.
pub const VERSION: i64 = 1 + MIGRATIONS.len() as i64;

/// Tables whose rows are history themselves, or the engine's bookkeeping: changes to them are not tracked.
const UNTRACKED: [&str; 5] = ["rounds", "tool_runs", "changes", "outbox", "contents"];

/// The line a table's description has when agents may not read it.
const HIDDEN: &str = "Hidden from agents.";

#[derive(Debug)]
pub enum Error {
    Sql(rusqlite::Error),
    /// The database was made by a newer wf-agents: its version.
    Newer(i64),
    /// The file holds tables but is not a wf-agents database.
    Foreign,
    /// There is no database: the file does not exist, or is empty.
    Missing,
    /// A database is already there, and creating one would overwrite it.
    Exists,
    /// The dependency would close a cycle.
    Cycle {
        item: i64,
        depends_on: i64,
    },
    /// The write was given up, for the reason given; nothing of it was kept.
    Abort(String),
}

impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Error {
        Error::Sql(e)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Sql(e) => write!(f, "{e}"),
            Error::Newer(v) => write!(
                f,
                "the database is at version {v}, newer than this wf-agents ({VERSION})"
            ),
            Error::Foreign => write!(f, "the file holds tables but is not a wf-agents database"),
            Error::Missing => write!(f, "there is no database"),
            Error::Exists => write!(f, "a database is already there"),
            Error::Cycle { item, depends_on } => {
                write!(
                    f,
                    "item {item} cannot depend on {depends_on}: {depends_on} already depends on {item}"
                )
            }
            Error::Abort(why) => write!(f, "{why}"),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

/// Who makes a write, as `changes` records it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Actor {
    Owner,
    Architect,
    Engine,
}

impl Actor {
    pub fn name(self) -> &'static str {
        match self {
            Actor::Owner => "owner",
            Actor::Architect => "architect",
            Actor::Engine => "engine",
        }
    }
}

/// Now, in UTC milliseconds since the Unix epoch, as every time in the database is.
pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

/// The write in progress: what the triggers record each change under.
#[derive(Clone, Copy)]
struct Writer {
    tx: i64,
    at: i64,
    actor: Actor,
}

type Current = Arc<Mutex<Option<Writer>>>;

/// A row that a committed change touched: its table, and its rowid.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Changed {
    pub table: String,
    pub rowid: i64,
}

/// What is told the rows each committed write touched, given the connection that made it to look them up.
pub type Listener = Box<dyn Fn(&Connection, &[Changed]) + Send>;

/// The rows changed on a connection, kept until they can be told.
#[derive(Default)]
struct Watch {
    /// Changed by the transaction still open.
    pending: Vec<Changed>,
    /// Changed by transactions that committed, not yet told.
    committed: Vec<Changed>,
}

/// The database as the engine and the owner's commands use it. Every write goes through [`Db::write`]: a state
/// table's triggers refuse a change made any other way, so none escapes `changes`.
pub struct Db {
    conn: Connection,
    writer: Current,
    watch: Arc<Mutex<Watch>>,
    listener: Option<Listener>,
}

impl Db {
    /// Opens the database at `path`, and migrates it when it is older than this wf-agents. It makes nothing: when the
    /// file does not exist or is empty, there is no database, and the error says so.
    pub fn open(path: &Path) -> Result<Db> {
        let flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        let conn = match Connection::open_with_flags(path, flags) {
            Ok(conn) => conn,
            Err(_) if !path.exists() => return Err(Error::Missing),
            Err(e) => return Err(e.into()),
        };
        let mut db = Db::set_up(conn)?;
        db.migrate(MIGRATIONS)?;
        db.conn.pragma_update(None, "journal_mode", "WAL")?;
        Ok(db)
    }

    /// Makes the database at `path` from the schema. The place must hold no database: a file that does not exist or
    /// is empty is taken, anything in it is refused. A failure leaves an empty file at most, which a retry takes.
    pub fn create(path: &Path) -> Result<Db> {
        let mut db = Db::set_up(Connection::open(path)?)?;
        db.create_schema(SCHEMA, MIGRATIONS)?;
        db.conn.pragma_update(None, "journal_mode", "WAL")?;
        Ok(db)
    }

    /// A new database in memory.
    pub fn in_memory() -> Result<Db> {
        let mut db = Db::set_up(Connection::open_in_memory()?)?;
        db.create_schema(SCHEMA, MIGRATIONS)?;
        Ok(db)
    }

    fn set_up(conn: Connection) -> Result<Db> {
        conn.busy_timeout(Duration::from_secs(10))?;
        conn.pragma_update(None, "foreign_keys", true)?;
        let writer: Current = Arc::new(Mutex::new(None));
        register(&conn, "wf_tx", &writer, |w| w.tx)?;
        register(&conn, "wf_at", &writer, |w| w.at)?;
        register(&conn, "wf_actor", &writer, |w| w.actor.name())?;
        Ok(Db {
            conn,
            writer,
            watch: Arc::default(),
            listener: None,
        })
    }

    /// From now on tells `f` the rows each write on this connection touched, every table's, once its transaction
    /// has committed: a reader that looks them up then sees them. Rows a statement outside [`Db::write`] changed are
    /// told after the next write commits.
    pub fn on_commit(&mut self, f: Listener) -> Result<()> {
        let w = Arc::clone(&self.watch);
        self.conn
            .update_hook(Some(move |_: Action, _: &str, table: &str, rowid: i64| {
                lock(&w).pending.push(Changed {
                    table: table.to_string(),
                    rowid,
                });
            }))?;
        let w = Arc::clone(&self.watch);
        self.conn.commit_hook(Some(move || {
            let mut x = lock(&w);
            let done = std::mem::take(&mut x.pending);
            x.committed.extend(done);
            // Lets the commit go on.
            false
        }))?;
        let w = Arc::clone(&self.watch);
        self.conn.rollback_hook(Some(move || lock(&w).pending.clear()))?;
        self.listener = Some(f);
        Ok(())
    }

    /// Tells the listener the rows committed since it was last told, each once.
    fn tell(&self) {
        let Some(f) = &self.listener else { return };
        let rows = std::mem::take(&mut lock(&self.watch).committed);
        if rows.is_empty() {
            return;
        }
        let mut seen = HashSet::new();
        let rows: Vec<Changed> = rows.into_iter().filter(|c| seen.insert(c.clone())).collect();
        f(&self.conn, &rows);
    }

    /// The connection, for reads.
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    /// Runs `f` in one immediate transaction, its changes recorded as made by `actor`, and commits it when `f`
    /// succeeds.
    pub fn write<T>(&mut self, actor: Actor, f: impl FnOnce(&Transaction<'_>) -> Result<T>) -> Result<T> {
        let tx = self.conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let id: i64 = tx.query_row("SELECT coalesce(max(tx), 0) + 1 FROM changes", [], |r| r.get(0))?;
        set(
            &self.writer,
            Some(Writer {
                tx: id,
                at: now(),
                actor,
            }),
        );
        let out = f(&tx);
        set(&self.writer, None);
        let out = out?;
        tx.commit()?;
        self.tell();
        Ok(out)
    }

    /// Builds `schema` into a database that holds nothing, at the version `migrations` lead to; then makes the
    /// triggers that track changes. What is there is judged under the lock, so of two creations only one builds.
    fn create_schema(&mut self, schema: &str, migrations: &[&str]) -> Result<()> {
        let version = 1 + migrations.len() as i64;
        let tx = self.conn.transaction_with_behavior(TransactionBehavior::Exclusive)?;
        if version_of(&tx)? != 0 {
            return Err(Error::Exists);
        }
        if !is_empty(&tx)? {
            return Err(Error::Foreign);
        }
        tx.execute_batch(schema)?;
        track(&tx)?;
        tx.pragma_update(None, "user_version", version)?;
        tx.commit()?;
        Ok(())
    }

    /// Brings an existing database to the version `migrations` lead to: runs the migrations from its version on, then
    /// makes the triggers that track changes afresh. A file with nothing in it holds no database.
    fn migrate(&mut self, migrations: &[&str]) -> Result<()> {
        let version = 1 + migrations.len() as i64;
        if version_of(&self.conn)? == version {
            return Ok(());
        }
        let tx = self.conn.transaction_with_behavior(TransactionBehavior::Exclusive)?;
        // Judged again under the lock: another process may have migrated it while this one waited.
        let at = version_of(&tx)?;
        if at > version {
            return Err(Error::Newer(at));
        }
        if at == version {
            return Ok(());
        }
        if at == 0 {
            if is_empty(&tx)? {
                return Err(Error::Missing);
            }
            return Err(Error::Foreign);
        }
        for m in &migrations[at as usize - 1..] {
            tx.execute_batch(m)?;
        }
        track(&tx)?;
        tx.pragma_update(None, "user_version", version)?;
        tx.commit()?;
        Ok(())
    }
}

/// The version a database is at, which is 0 for a file with nothing made in it.
fn version_of(conn: &Connection) -> rusqlite::Result<i64> {
    conn.pragma_query_value(None, "user_version", |r| r.get(0))
}

/// Whether the database holds no table, view or index.
fn is_empty(conn: &Connection) -> rusqlite::Result<bool> {
    conn.query_row("SELECT count(*) = 0 FROM sqlite_schema", [], |r| r.get(0))
}

fn set(current: &Current, w: Option<Writer>) {
    *current.lock().unwrap_or_else(|e| e.into_inner()) = w;
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// A function the triggers call for the write in progress; outside one it fails, and so does the change.
fn register<T: ToSql + 'static>(
    conn: &Connection,
    name: &str,
    current: &Current,
    get: fn(&Writer) -> T,
) -> rusqlite::Result<()> {
    let current = Arc::clone(current);
    let flags = FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_INNOCUOUS;
    conn.create_scalar_function(name, 0, flags, move |_| {
        let w = *current.lock().unwrap_or_else(|e| e.into_inner());
        w.as_ref().map(get).ok_or_else(|| {
            rusqlite::Error::UserFunctionError("a change to a state table outside wf-agents' own writes".into())
        })
    })
}

/// Makes afresh, for every state table, the triggers that write each change to its rows into `changes`.
fn track(tx: &Transaction<'_>) -> Result<()> {
    let old: Vec<String> = names(
        tx,
        "SELECT name FROM sqlite_schema WHERE type = 'trigger' AND name LIKE 'track\\_%' ESCAPE '\\'",
    )?;
    for t in old {
        tx.execute_batch(&format!("DROP TRIGGER {t};"))?;
    }
    for table in names(
        tx,
        "SELECT name FROM sqlite_schema WHERE type = 'table' AND name NOT LIKE 'sqlite\\_%' ESCAPE '\\'",
    )? {
        if UNTRACKED.contains(&table.as_str()) {
            continue;
        }
        tx.execute_batch(&triggers(tx, &table)?)?;
    }
    Ok(())
}

fn names(conn: &Connection, sql: &str) -> rusqlite::Result<Vec<String>> {
    let mut st = conn.prepare(sql)?;
    st.query_map([], |r| r.get(0))?.collect()
}

/// The insert, update and delete triggers of one table. A row is kept as a JSON object, with JSON columns nested;
/// an update that changes nothing is not recorded.
fn triggers(conn: &Connection, table: &str) -> rusqlite::Result<String> {
    let sql: String = conn.query_row("SELECT sql FROM sqlite_schema WHERE name = ?1", [table], |r| r.get(0))?;
    let mut st = conn.prepare("SELECT name, pk FROM pragma_table_info(?1) ORDER BY cid")?;
    let cols: Vec<(String, i64)> = st
        .query_map([table], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let row = |of: &str| {
        let parts: Vec<String> = cols
            .iter()
            .map(|(c, _)| {
                let v = format!("{of}.\"{c}\"");
                let v = if sql.contains(&format!("json_valid({c})")) {
                    format!("json({v})")
                } else {
                    v
                };
                format!("'{c}', {v}")
            })
            .collect();
        format!("json_object({})", parts.join(", "))
    };
    let mut pk: Vec<&(String, i64)> = cols.iter().filter(|(_, k)| *k > 0).collect();
    pk.sort_by_key(|(_, k)| *k);
    let key = |of: &str| {
        let parts: Vec<String> = pk.iter().map(|(c, _)| format!("'{c}', {of}.\"{c}\"")).collect();
        if parts.is_empty() {
            format!("json_object('rowid', {of}.rowid)")
        } else {
            format!("json_object({})", parts.join(", "))
        }
    };
    let changed: Vec<String> = cols
        .iter()
        .map(|(c, _)| format!("OLD.\"{c}\" IS NOT NEW.\"{c}\""))
        .collect();
    let insert = "INSERT INTO changes (tx, at, actor, table_name, row_key, old_row, new_row)\n    \
                  VALUES (wf_tx(), wf_at(), wf_actor()";
    Ok(format!(
        "CREATE TRIGGER track_{table}_insert AFTER INSERT ON \"{table}\" BEGIN\n  {insert}, '{table}', {nk}, NULL, {nr});\nEND;\n\
         CREATE TRIGGER track_{table}_update AFTER UPDATE ON \"{table}\" WHEN {changed} BEGIN\n  {insert}, '{table}', {nk}, {or}, {nr});\nEND;\n\
         CREATE TRIGGER track_{table}_delete AFTER DELETE ON \"{table}\" BEGIN\n  {insert}, '{table}', {ok}, {or}, NULL);\nEND;\n",
        nk = key("NEW"),
        ok = key("OLD"),
        nr = row("NEW"),
        or = row("OLD"),
        changed = changed.join(" OR "),
    ))
}

/// Makes `item` depend on `depends_on`, unless `depends_on` already depends on `item`, directly or not.
pub fn add_dependency(tx: &Transaction<'_>, item: i64, depends_on: i64, needs: &str) -> Result<()> {
    let cycle: bool = tx.query_row(
        "WITH RECURSIVE reach(id) AS (SELECT ?1 UNION SELECT l.depends_on FROM item_links l JOIN reach ON l.item = reach.id)
         SELECT count(*) > 0 FROM reach WHERE id = ?2",
        params![depends_on, item],
        |r| r.get(0),
    )?;
    if cycle {
        return Err(Error::Cycle { item, depends_on });
    }
    tx.execute(
        "INSERT INTO item_links (item, depends_on, needs) VALUES (?1, ?2, ?3)",
        params![item, depends_on, needs],
    )?;
    Ok(())
}

/// The tables agents may not read: those whose description says so.
fn hidden(conn: &Connection) -> rusqlite::Result<HashSet<String>> {
    let mut st = conn.prepare("SELECT name FROM sqlite_schema WHERE type = 'table' AND instr(sql, ?1) > 0")?;
    st.query_map([HIDDEN], |r| r.get(0))?.collect()
}

/// Opens the database at `path` the way agents query it, for one query: read-only, with no value of a table hidden
/// from agents readable, through a view either, and stopped once it has run for `limit`.
pub fn open_for_agents(path: &Path, limit: Duration) -> Result<Connection> {
    let mut conn =
        Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)?;
    conn.busy_timeout(Duration::from_secs(10))?;
    conn.pragma_update(None, "query_only", true)?;
    let hidden = hidden(&conn)?;
    conn.authorizer(Some(move |cx: AuthContext<'_>| match cx.action {
        AuthAction::Select | AuthAction::Function { .. } | AuthAction::Recursive => Authorization::Allow,
        AuthAction::Read { table_name, .. } if !hidden.contains(table_name) => Authorization::Allow,
        _ => Authorization::Deny,
    }))?;
    // The handler runs every few thousand steps; the clock starts at the connection's first.
    let started: Arc<Mutex<Option<std::time::Instant>>> = Arc::new(Mutex::new(None));
    conn.progress_handler(
        10_000,
        Some(move || {
            let mut s = started.lock().unwrap_or_else(|e| e.into_inner());
            let t = *s.get_or_insert_with(std::time::Instant::now);
            t.elapsed() > limit
        }),
    )?;
    Ok(conn)
}

/// Opens the database at `path` for the owner's interface to read: read-only, every table readable, waiting while a
/// write holds the database.
pub fn open_read(path: &Path) -> Result<Connection> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)?;
    conn.busy_timeout(Duration::from_secs(10))?;
    conn.pragma_update(None, "query_only", true)?;
    Ok(conn)
}

/// What agents are told about the database: each table and view they can read, by its name, with the first line of
/// the comment that says what it holds, in the order the schema defines them. An agent reads a definition in full,
/// its comments included, with [`DEFINITION`] through the query tool.
pub fn index(conn: &Connection) -> rusqlite::Result<Vec<(String, String)>> {
    let mut st = conn.prepare(
        "SELECT name, sql FROM sqlite_schema WHERE type IN ('table', 'view') AND name NOT LIKE 'sqlite\\_%' ESCAPE '\\'
         AND instr(sql, ?1) = 0 ORDER BY rowid",
    )?;
    let rows = st.query_map([HIDDEN], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    let mut out = Vec::new();
    for row in rows {
        let (name, sql) = row?;
        let first = sql
            .lines()
            .map(str::trim)
            .find_map(|l| l.strip_prefix("--"))
            .unwrap_or_default()
            .trim()
            .to_string();
        out.push((name, first));
    }
    Ok(out)
}

/// What [`index`] gives for a schema, from the schema loaded into a database in memory.
pub fn index_of(schema: &str) -> rusqlite::Result<Vec<(String, String)>> {
    let db = Connection::open_in_memory()?;
    db.execute_batch(schema)?;
    index(&db)
}

/// The query that gives one table's or view's definition in full, with its comments, for `<name>`.
pub const DEFINITION: &str = "SELECT sql FROM sqlite_master WHERE name = '<name>'";

/// Every table and view a schema defines, with its columns, from the schema loaded into a database in memory.
pub fn relations(schema: &str) -> rusqlite::Result<BTreeMap<String, BTreeSet<String>>> {
    let db = Connection::open_in_memory()?;
    db.execute_batch(schema)?;
    let mut out = BTreeMap::new();
    for n in names(
        &db,
        "SELECT name FROM sqlite_schema WHERE type IN ('table', 'view') AND name NOT LIKE 'sqlite_%'",
    )? {
        let mut cols = db.prepare("SELECT name FROM pragma_table_info(?1)")?;
        let cols: BTreeSet<String> = cols.query_map([&n], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
        out.insert(n, cols);
    }
    Ok(out)
}

/// What a database is made of, to compare one migrated with one created afresh: each table's columns, foreign keys
/// and indexes, and the definition of each view, index and trigger.
pub fn shape(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    let mut out = Vec::new();
    let objects: Vec<(String, String, Option<String>)> = {
        let mut st = conn.prepare(
            "SELECT type, name, sql FROM sqlite_schema WHERE name NOT LIKE 'sqlite\\_%' ESCAPE '\\' ORDER BY type, name",
        )?;
        st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<rusqlite::Result<_>>()?
    };
    for (kind, name, sql) in objects {
        if kind != "table" {
            out.push(format!("{kind} {name}: {}", sql.unwrap_or_default()));
            continue;
        }
        let mut st =
            conn.prepare("SELECT name, type, \"notnull\", dflt_value, pk, hidden FROM pragma_table_xinfo(?1)")?;
        let cols = st.query_map([&name], |r| {
            Ok(format!(
                "{} {} notnull={} default={:?} pk={} hidden={}",
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, i64>(4)?,
                r.get::<_, i64>(5)?
            ))
        })?;
        for c in cols {
            out.push(format!("table {name} column {}", c?));
        }
        let mut st = conn.prepare("SELECT \"table\", \"from\", \"to\", on_delete FROM pragma_foreign_key_list(?1)")?;
        let fks = st.query_map([&name], |r| {
            Ok(format!(
                "{} -> {}({:?}) on delete {}",
                r.get::<_, String>(1)?,
                r.get::<_, String>(0)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, String>(3)?
            ))
        })?;
        for f in fks {
            out.push(format!("table {name} key {}", f?));
        }
        let strict: bool = conn.query_row("SELECT strict FROM pragma_table_list(?1)", [&name], |r| r.get(0))?;
        out.push(format!("table {name} strict={strict}"));
    }
    Ok(out)
}

/// How long a text must be for a round's record to keep it in `contents` and refer to it.
pub const STORED: usize = 256;

/// The key a text is kept under in `contents`: the SHA-256 of the text, in hex.
pub fn content_hash(text: &str) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(text.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Keeps texts in `contents`, each once.
pub fn keep(tx: &Transaction<'_>, texts: &[(String, String)]) -> rusqlite::Result<()> {
    for (hash, text) in texts {
        tx.execute(
            "INSERT INTO contents (hash, text) VALUES (?1, ?2) ON CONFLICT (hash) DO NOTHING",
            params![hash, text],
        )?;
    }
    Ok(())
}

/// A round's record as it was: each `{"content": hash}` put back as the text kept under that hash, and each
/// `{"parts": [...]}` (texts and such references) joined into one text. A record that refers to nothing, as every round
/// recorded before `contents` was, comes back as it is.
pub fn put_back(conn: &Connection, v: &Value) -> Value {
    let text = |h: &str| -> Option<String> {
        conn.query_row("SELECT text FROM contents WHERE hash = ?1", [h], |r| r.get(0))
            .optional()
            .ok()
            .flatten()
    };
    let only = |v: &Value, key: &str| -> Option<Value> {
        let o = v.as_object()?;
        if o.len() == 1 { o.get(key).cloned() } else { None }
    };
    if let Some(h) = only(v, "content").and_then(|h| h.as_str().map(str::to_string)) {
        return Value::String(text(&h).unwrap_or_else(|| format!("(content {h} is not kept)")));
    }
    if let Some(Value::Array(parts)) = only(v, "parts") {
        let mut out = String::new();
        for p in &parts {
            if let Value::String(s) = put_back(conn, p) {
                out.push_str(&s);
            }
        }
        return Value::String(out);
    }
    match v {
        Value::Array(xs) => Value::Array(xs.iter().map(|x| put_back(conn, x)).collect()),
        Value::Object(o) => Value::Object(o.iter().map(|(k, x)| (k.clone(), put_back(conn, x))).collect()),
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("wf-agents-db-{}-{name}.sqlite", std::process::id()));
        for ext in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{ext}", p.display()));
        }
        p
    }

    fn item(tx: &Transaction<'_>, title: &str) -> Result<i64> {
        tx.execute(
            "INSERT INTO items (project, title, problem, why, deliver, level) VALUES ('p', ?1, 'x', 'y', 'change', 'l')",
            [title],
        )?;
        Ok(tx.last_insert_rowid())
    }

    /// A row of changes: the write, who made it, the table, the row's key, the row before and after.
    type Change = (i64, String, String, String, Option<String>, Option<String>);

    fn changes(db: &Db) -> Vec<Change> {
        let mut st = db
            .conn()
            .prepare("SELECT tx, actor, table_name, row_key, old_row, new_row FROM changes ORDER BY id")
            .unwrap();
        st.query_map([], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))
        })
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
    }

    // Every view loads and can be read, and each lists its columns.
    #[test]
    fn the_schema_loads_and_every_view_reads() {
        let rels = relations(SCHEMA).unwrap();
        assert!(rels["items"].contains("problem"));
        assert!(rels["work_state"].contains("work"));
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(SCHEMA).unwrap();
        for name in rels.keys() {
            db.prepare(&format!("SELECT * FROM {name} LIMIT 0")).unwrap();
        }
    }

    // A new database is at the schema's version, with the triggers of every state table and of no history table.
    #[test]
    fn a_new_database_is_at_the_schema_version_and_tracks_its_state_tables() {
        let db = Db::in_memory().unwrap();
        let v: i64 = db
            .conn()
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(v, VERSION);
        let tracked = names(
            db.conn(),
            "SELECT DISTINCT tbl_name FROM sqlite_schema WHERE type = 'trigger' ORDER BY 1",
        )
        .unwrap();
        assert!(tracked.contains(&"items".to_string()) && tracked.contains(&"works".to_string()));
        for t in UNTRACKED {
            assert!(!tracked.contains(&t.to_string()), "{t}");
        }
    }

    // Each change is kept with who made it, the write it was part of, and the row before and after.
    #[test]
    fn every_change_to_a_state_table_is_recorded_with_who_made_it() {
        let mut db = Db::in_memory().unwrap();
        let id = db
            .write(Actor::Owner, |tx| {
                let id = item(tx, "first")?;
                tx.execute("INSERT INTO dispatch_queue (item, position) VALUES (?1, 1.0)", [id])?;
                Ok(id)
            })
            .unwrap();
        db.write(Actor::Engine, |tx| {
            tx.execute("UPDATE items SET title = title WHERE id = ?1", [id])?;
            tx.execute("UPDATE items SET title = 'second' WHERE id = ?1", [id])?;
            tx.execute("DELETE FROM dispatch_queue WHERE item = ?1", [id])?;
            Ok(())
        })
        .unwrap();
        let c = changes(&db);
        assert_eq!(c.len(), 4, "{c:?}");
        assert_eq!((c[0].0, c[0].1.as_str(), c[0].2.as_str()), (1, "owner", "items"));
        assert_eq!(c[0].3, format!("{{\"id\":{id}}}"));
        assert!(c[0].4.is_none() && c[0].5.as_deref().unwrap().contains("\"title\":\"first\""));
        assert_eq!((c[1].0, c[1].2.as_str()), (1, "dispatch_queue"));
        assert_eq!((c[2].0, c[2].1.as_str()), (2, "engine"));
        assert!(c[2].4.as_deref().unwrap().contains("\"first\"") && c[2].5.as_deref().unwrap().contains("\"second\""));
        assert!(c[3].4.is_some() && c[3].5.is_none());
    }

    // A JSON column is kept as JSON inside the row, not as a string.
    #[test]
    fn a_json_column_is_nested_in_the_recorded_row() {
        let mut db = Db::in_memory().unwrap();
        db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{\"a\":[1]}')",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        let c = changes(&db);
        assert!(c[0].5.as_deref().unwrap().contains("\"compiled\":{\"a\":[1]}"), "{c:?}");
    }

    // A change made around wf-agents' writes fails, so changes misses none.
    #[test]
    fn a_change_outside_a_write_is_refused() {
        let db = Db::in_memory().unwrap();
        let e = db
            .conn()
            .execute("INSERT INTO items (project, title, problem, why, deliver, level) VALUES ('p', 't', 'x', 'y', 'change', 'l')", [])
            .unwrap_err();
        assert!(e.to_string().contains("outside wf-agents' own writes"), "{e}");
        let n: i64 = db
            .conn()
            .query_row("SELECT count(*) FROM items", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0);
    }

    // A write that fails leaves nothing behind, and the next one works.
    #[test]
    fn a_failed_write_changes_nothing() {
        let mut db = Db::in_memory().unwrap();
        let e = db.write(Actor::Owner, |tx| {
            item(tx, "kept?")?;
            tx.execute("INSERT INTO items (project) VALUES ('p')", [])?;
            Ok(())
        });
        assert!(e.is_err());
        db.write(Actor::Owner, |tx| item(tx, "kept")).unwrap();
        let titles = names(db.conn(), "SELECT title FROM items").unwrap();
        assert_eq!(titles, ["kept"]);
        assert_eq!(changes(&db).len(), 1);
    }

    #[test]
    fn a_committed_write_tells_the_rows_it_touched_and_a_failed_one_nothing() {
        let mut db = Db::in_memory().unwrap();
        let told: Arc<Mutex<Vec<Changed>>> = Arc::default();
        let t = Arc::clone(&told);
        db.on_commit(Box::new(move |_, rows| t.lock().unwrap().extend(rows.iter().cloned())))
            .unwrap();
        let take = || -> Vec<Changed> {
            told.lock()
                .unwrap()
                .drain(..)
                .filter(|c| c.table != "changes")
                .collect()
        };
        let id = db.write(Actor::Owner, |tx| item(tx, "a")).unwrap();
        let items = vec![Changed {
            table: "items".into(),
            rowid: id,
        }];
        assert_eq!(take(), items);
        let failed = db.write(Actor::Owner, |tx| {
            item(tx, "b")?;
            Err::<(), _>(Error::Abort("given up".into()))
        });
        assert!(failed.is_err());
        db.write(Actor::Owner, |tx| {
            tx.execute("UPDATE items SET title = 'c' WHERE id = ?1", [id])?;
            tx.execute("UPDATE items SET title = 'd' WHERE id = ?1", [id])?;
            Ok(())
        })
        .unwrap();
        assert_eq!(take(), items);
    }

    // Dependencies stay a directed acyclic graph.
    #[test]
    fn a_dependency_that_closes_a_cycle_is_refused() {
        let mut db = Db::in_memory().unwrap();
        let e = db.write(Actor::Owner, |tx| {
            let a = item(tx, "a")?;
            let b = item(tx, "b")?;
            let c = item(tx, "c")?;
            add_dependency(tx, a, b, "b's part")?;
            add_dependency(tx, b, c, "c's part")?;
            add_dependency(tx, c, a, "a's part")
        });
        assert!(matches!(e, Err(Error::Cycle { item: 3, depends_on: 1 })), "{e:?}");
        let n: i64 = db
            .conn()
            .query_row("SELECT count(*) FROM item_links", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0);
    }

    // Agents read the tables and views they may, and cannot write or read a hidden table.
    #[test]
    fn agents_read_what_they_may_and_nothing_else() {
        let path = temp("agents");
        let mut db = Db::create(&path).unwrap();
        db.write(Actor::Owner, |tx| {
            let id = item(tx, "visible")?;
            tx.execute("INSERT INTO dispatch_queue (item, position) VALUES (?1, 1.0)", [id])?;
            Ok(())
        })
        .unwrap();
        let agent = open_for_agents(&path, Duration::from_secs(5)).unwrap();
        let t: String = agent
            .query_row("SELECT title FROM ready_items", [], |r| r.get(0))
            .unwrap();
        assert_eq!(t, "visible");
        assert!(agent.prepare("SELECT cli_session FROM sessions").is_err());
        assert!(agent.execute("DELETE FROM dispatch_queue", []).is_err());
        assert!(agent.execute_batch("ATTACH ':memory:' AS other").is_err());
        let forever = "WITH RECURSIVE c(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM c) SELECT count(*) FROM c";
        let slow = open_for_agents(&path, Duration::from_millis(50)).unwrap();
        assert!(slow.query_row(forever, [], |r| r.get::<_, i64>(0)).is_err());
        // The index names each table and view with the first line of its description; the query it points to gives
        // the whole definition with its comments.
        let index = index(db.conn()).unwrap();
        let names: Vec<&str> = index.iter().map(|(n, _)| n.as_str()).collect();
        assert!(names.contains(&"items") && names.contains(&"ready_items"));
        assert!(!names.contains(&"sessions"));
        let (_, first) = index.iter().find(|(n, _)| n == "items").unwrap();
        assert!(
            first.starts_with("A piece of work to do, as its creator wrote it."),
            "{first}"
        );
        let full: String = agent
            .query_row(&DEFINITION.replace("<name>", "items"), [], |r| r.get(0))
            .unwrap();
        assert!(
            full.contains("-- Item number, one sequence across all projects."),
            "{full}"
        );
    }

    // A database made at version 1 and migrated has the shape of one created from the schema, and keeps its rounds
    // and every row that points to them.
    #[test]
    fn a_database_migrated_from_version_1_has_the_shape_of_a_new_one() {
        let v1 = include_str!("../../../db/migrations/0001.sql");
        let mut old = Db {
            conn: Connection::open_in_memory().unwrap(),
            writer: Arc::new(Mutex::new(None)),
            watch: Arc::default(),
            listener: None,
        };
        old.conn.pragma_update(None, "foreign_keys", true).unwrap();
        register(&old.conn, "wf_tx", &old.writer, |w| w.tx).unwrap();
        register(&old.conn, "wf_at", &old.writer, |w| w.at).unwrap();
        register(&old.conn, "wf_actor", &old.writer, |w| w.actor.name()).unwrap();
        old.create_schema(v1, &[]).unwrap();
        old.write(Actor::Engine, |tx| {
            let id = item(tx, "kept")?;
            tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
            tx.execute("INSERT INTO works (item, flow_version, graph, at) VALUES (?1, 1, 'engineer', '1')", [id])?;
            tx.execute(
                "INSERT INTO rounds (work, cell, input, config_commit, started_at) VALUES (1, '1', '{\"prompt\":\"p\"}', 'c', 1)",
                [],
            )?;
            tx.execute(
                "INSERT INTO cards (round, kind, body, status) VALUES (1, 'ask', '{}', 'open')",
                [],
            )?;
            // A design card, and an ask a failed run raised, whose work it paused.
            tx.execute(
                "INSERT INTO cards (round, kind, body, status) VALUES (1, 'design', '[]', 'open')",
                [],
            )?;
            tx.execute(
                "INSERT INTO cards (round, kind, body, status) VALUES (1, 'ask', '{\"failed\":{}}', 'open')",
                [],
            )?;
            tx.execute("UPDATE works SET paused = 'asked the owner in card 3' WHERE id = 1", [])?;
            tx.execute(
                "INSERT INTO tool_runs (round, tool, args, attempt, queued_at) VALUES (1, 'gh', '{}', 1, 1)",
                [],
            )?;
            tx.execute(
                "UPDATE items SET acceptance = '[]', acceptance_round = 1 WHERE id = ?1",
                [id],
            )?;
            Ok(())
        })
        .unwrap();
        old.migrate(MIGRATIONS).unwrap();
        let new = Db::in_memory().unwrap();
        assert_eq!(shape(old.conn()).unwrap(), shape(new.conn()).unwrap());
        let input: String = old
            .conn()
            .query_row("SELECT input FROM rounds WHERE id = 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(input, "{\"prompt\":\"p\"}");
        // Design cards and asks of the owner are apart, with their ids; the ask a failed run raised is closed, and
        // its work goes on.
        let ids = |q: &str| -> Vec<i64> {
            let mut st = old.conn().prepare(q).unwrap();
            st.query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect()
        };
        assert_eq!(ids("SELECT id FROM design_cards"), [2]);
        assert_eq!(ids("SELECT id FROM owner_asks WHERE answer IS NULL"), [1]);
        assert_eq!(ids("SELECT id FROM owner_asks WHERE answer IS NOT NULL"), [3]);
        assert_eq!(ids("SELECT id FROM works WHERE paused IS NOT NULL"), Vec::<i64>::new());
        let broken: i64 = old
            .conn()
            .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| r.get(0))
            .unwrap();
        assert_eq!(broken, 0);
        let sql = |db: &Db| -> String {
            db.conn()
                .query_row("SELECT sql FROM sqlite_schema WHERE name = 'rounds'", [], |r| r.get(0))
                .unwrap()
        };
        assert_eq!(sql(&old), sql(&new));
        let v: i64 = old
            .conn()
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(v, VERSION);
    }

    // A round's record keeps a large text once and refers to it; put back, it is the text it was. A record that
    // refers to nothing comes back as it is.
    #[test]
    fn a_kept_text_is_put_back_where_the_record_refers_to_it() {
        let mut db = Db::in_memory().unwrap();
        let big = "x".repeat(STORED);
        let h = content_hash(&big);
        db.write(Actor::Engine, |tx| {
            keep(tx, &[(h.clone(), big.clone()), (h.clone(), big.clone())])?;
            Ok(())
        })
        .unwrap();
        let stored = serde_json::json!({
            "message": { "parts": ["# R1\n", { "content": h }, "\nend\n"] },
            "call": { "command": ["claude", "--json-schema", { "content": h }] },
            "old": "a whole text",
        });
        let back = put_back(db.conn(), &stored);
        assert_eq!(back["message"], format!("# R1\n{big}\nend\n"));
        assert_eq!(
            back["call"]["command"],
            serde_json::json!(["claude", "--json-schema", big])
        );
        assert_eq!(back["old"], "a whole text");
        let n: i64 = db
            .conn()
            .query_row("SELECT count(*) FROM contents", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1);
    }

    // A database from a newer wf-agents, or a file that is not one, is not touched.
    #[test]
    fn a_newer_or_foreign_database_is_refused() {
        let path = temp("newer");
        Db::create(&path).unwrap();
        Connection::open(&path)
            .unwrap()
            .pragma_update(None, "user_version", VERSION + 1)
            .unwrap();
        assert!(matches!(Db::open(&path), Err(Error::Newer(v)) if v == VERSION + 1));
        let path = temp("foreign");
        Connection::open(&path)
            .unwrap()
            .execute_batch("CREATE TABLE t (x)")
            .unwrap();
        assert!(matches!(Db::open(&path), Err(Error::Foreign)));
    }

    // Opening makes nothing: a path with no file gets none, and a file with nothing in it is no database and stays as
    // it was.
    #[test]
    fn opening_a_missing_or_empty_file_is_refused_and_makes_nothing() {
        let path = temp("missing");
        assert!(matches!(Db::open(&path), Err(Error::Missing)));
        assert!(!path.exists());
        std::fs::write(&path, b"").unwrap();
        assert!(matches!(Db::open(&path), Err(Error::Missing)));
        assert_eq!(std::fs::metadata(&path).unwrap().len(), 0);
        assert!(!Path::new(&format!("{}-wal", path.display())).exists());
    }

    // A database made at a path has the shape of a new one in memory, with its version and triggers, and opens again;
    // a place with a database or other tables in it is refused and what is there stays; an empty file is taken.
    #[test]
    fn creating_makes_a_new_database_and_refuses_a_place_with_anything_in_it() {
        let path = temp("create");
        let mut made = Db::create(&path).unwrap();
        let new = Db::in_memory().unwrap();
        assert_eq!(shape(made.conn()).unwrap(), shape(new.conn()).unwrap());
        assert_eq!(version_of(made.conn()).unwrap(), VERSION);
        made.write(Actor::Owner, |tx| item(tx, "kept")).unwrap();
        drop(made);
        assert!(matches!(Db::create(&path), Err(Error::Exists)));
        let reopened = Db::open(&path).unwrap();
        let titles = names(reopened.conn(), "SELECT title FROM items").unwrap();
        assert_eq!(titles, ["kept"]);

        let other = temp("create-foreign");
        let conn = Connection::open(&other).unwrap();
        conn.execute_batch("CREATE TABLE t (x)").unwrap();
        assert!(matches!(Db::create(&other), Err(Error::Foreign)));
        let tables = names(&conn, "SELECT name FROM sqlite_schema").unwrap();
        assert_eq!(tables, ["t"]);

        let empty = temp("create-empty");
        std::fs::write(&empty, b"").unwrap();
        Db::create(&empty).unwrap();
        Db::open(&empty).unwrap();
    }

    // A database migrated from an older version has the shape of one created afresh; a migration that forgets a
    // change is found.
    #[test]
    fn a_migrated_database_has_the_shape_of_a_new_one() {
        let v1 = "CREATE TABLE things (id INTEGER PRIMARY KEY, name TEXT NOT NULL) STRICT;";
        let v2 = "CREATE TABLE things (id INTEGER PRIMARY KEY, name TEXT NOT NULL, size INTEGER) STRICT;
                  CREATE INDEX things_by_size ON things (size);";
        let step = "ALTER TABLE things ADD COLUMN size INTEGER; CREATE INDEX things_by_size ON things (size);";
        let fresh = |schema: &str, migrations: &[&str]| {
            let mut db = Db {
                conn: Connection::open_in_memory().unwrap(),
                writer: Arc::new(Mutex::new(None)),
                watch: Arc::default(),
                listener: None,
            };
            db.create_schema(schema, migrations).unwrap();
            db
        };
        let mut old = fresh(v1, &[]);
        old.migrate(&[step]).unwrap();
        let new = fresh(v2, &[step]);
        assert_eq!(shape(old.conn()).unwrap(), shape(new.conn()).unwrap());
        let mut wrong = fresh(v1, &[]);
        wrong.migrate(&["ALTER TABLE things ADD COLUMN size INTEGER;"]).unwrap();
        assert_ne!(shape(wrong.conn()).unwrap(), shape(new.conn()).unwrap());
        assert_eq!(VERSION, 1 + MIGRATIONS.len() as i64);
    }
}
