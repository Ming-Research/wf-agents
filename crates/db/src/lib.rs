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

mod cells;
mod matters;

pub use cells::{cells_sql, code_of, is_cell};
pub use matters::{
    FAULT, Forwarder, Matter, fault_forwarding, forward_fault, forward_new_fault, lift_pause, one_sentence,
    project_of_item, queue_matter, say_to_architect, set_fault_forwarding,
};

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
    include_str!("../../../db/migrations/0008.sql"),
    include_str!("../../../db/migrations/0009.sql"),
    include_str!("../../../db/migrations/0010.sql"),
    include_str!("../../../db/migrations/0011.sql"),
    include_str!("../../../db/migrations/0012.sql"),
    include_str!("../../../db/migrations/0013.sql"),
    include_str!("../../../db/migrations/0014.sql"),
    include_str!("../../../db/migrations/0015.sql"),
    include_str!("../../../db/migrations/0016.sql"),
    include_str!("../../../db/migrations/0017.sql"),
    include_str!("../../../db/migrations/0018.sql"),
    include_str!("../../../db/migrations/0019.sql"),
    include_str!("../../../db/migrations/0020.sql"),
    include_str!("../../../db/migrations/0021.sql"),
    include_str!("../../../db/migrations/0022.sql"),
    include_str!("../../../db/migrations/0023.sql"),
    include_str!("../../../db/migrations/0024.sql"),
    include_str!("../../../db/migrations/0025.sql"),
    include_str!("../../../db/migrations/0026.sql"),
    include_str!("../../../db/migrations/0027.sql"),
    include_str!("../../../db/migrations/0028.sql"),
    include_str!("../../../db/migrations/0029.sql"),
    include_str!("../../../db/migrations/0030.sql"),
    include_str!("../../../db/migrations/0031.sql"),
    include_str!("../../../db/migrations/0032.sql"),
    include_str!("../../../db/migrations/0033.sql"),
    include_str!("../../../db/migrations/0034.sql"),
    include_str!("../../../db/migrations/0035.sql"),
    include_str!("../../../db/migrations/0036.sql"),
    include_str!("../../../db/migrations/0037.sql"),
    include_str!("../../../db/migrations/0038.sql"),
    include_str!("../../../db/migrations/0039.sql"),
    include_str!("../../../db/migrations/0040.sql"),
    include_str!("../../../db/migrations/0041.sql"),
];

/// The version [`SCHEMA`] defines, kept in the database's `user_version`.
pub const VERSION: i64 = 1 + MIGRATIONS.len() as i64;

/// Tables whose rows are history themselves, or the engine's bookkeeping: changes to them are not tracked.
const UNTRACKED: [&str; 7] = [
    "rounds",
    "tool_runs",
    "pings",
    "cache_misses",
    "changes",
    "outbox",
    "contents",
];

/// The line a table's description has when agents may not read it.
const HIDDEN: &str = "Hidden from agents.";

/// The text a ruling on a design card is interjected with; a disagreement says so after it. The owner's ruling writes
/// it and the engine's checks read it.
pub const DESIGN_CARD_RULING: &str = "Ruling on design card";

/// The verdicts of a ruling on a design card as `design_cards.rulings` holds it, one for each decision in the order
/// given, each with its verdict and any approach chosen instead; a ruling made before rulings were per decision is one
/// verdict on the card as a whole, with decision 0.
pub fn ruling_verdicts(ruling: &serde_json::Value) -> Vec<(i64, String, Option<String>)> {
    let text = |v: &serde_json::Value, k: &str| v.get(k).and_then(serde_json::Value::as_str).map(str::to_string);
    match ruling.get("decisions").and_then(serde_json::Value::as_array) {
        Some(ds) => ds
            .iter()
            .map(|d| {
                let n = d
                    .get("decision")
                    .and_then(serde_json::Value::as_i64)
                    .unwrap_or_default();
                (n, text(d, "verdict").unwrap_or_default(), text(d, "option"))
            })
            .collect(),
        None => vec![(0, text(ruling, "verdict").unwrap_or_default(), text(ruling, "option"))],
    }
}

/// Whether a ruling confirms a design card of so many decisions: every decision confirmed, or, for a ruling made
/// before rulings were per decision, the card.
pub fn ruling_confirms(ruling: &serde_json::Value, decisions: usize) -> bool {
    let verdicts = ruling_verdicts(ruling);
    let whole = verdicts.len() == 1 && verdicts[0].0 == 0;
    let all = whole || (1..=decisions as i64).all(|n| verdicts.iter().any(|(d, _, _)| *d == n));
    all && verdicts.iter().all(|(_, v, _)| v == "confirm")
}

/// Whether a ruling disagrees with any decision of its design card, or with the card.
pub fn ruling_disagrees(ruling: &serde_json::Value) -> bool {
    ruling_verdicts(ruling).iter().any(|(_, v, _)| v == "disagree")
}

/// Why a work is paused when the owner paused it without saying why: its Pause on the page, and the round it asked
/// for of a work the engine had paused, which takes the work into the owner's hands.
pub const PAUSED_BY_OWNER: &str = "the owner paused it";

/// What the reason of a work the architect stopped starts with: the engine pauses it as it pauses one for a problem,
/// and the reason says the architect stopped it, then why. Only the architect's resume, or the owner's Continue of
/// that work, lets it go on.
pub const STOPPED_BY_ARCHITECT: &str = "stopped by the architect";

#[derive(Debug)]
pub enum Error {
    Sql(rusqlite::Error),
    /// The database was made by a newer wf-agents: its version.
    Newer(i64),
    /// The database is older than this wf-agents and was opened only to read, so it was not migrated: its version.
    Older(i64),
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
            Error::Older(v) => write!(
                f,
                "the database is at version {v}, older than this wf-agents ({VERSION}): run the engine once to migrate it"
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
        let mut db = Db::set_up(connect(path, flags)?)?;
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

    /// Opens the database at `path` only to read it, as it is: nothing is created or migrated, and a write on it
    /// fails. A file that does not exist or is empty is no database, as for [`Db::open`]; one not at this wf-agents'
    /// version is refused, since its shape is not the one read.
    pub fn open_read_only(path: &Path) -> Result<Db> {
        let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        let conn = connect(path, flags)?;
        conn.busy_timeout(Duration::from_secs(10))?;
        conn.pragma_update(None, "query_only", true)?;
        let at = version_of(&conn)?;
        if at == 0 {
            return Err(refusal(&conn)?);
        }
        if !up_to_date(at, VERSION)? {
            return Err(Error::Older(at));
        }
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
        if let Some(e) = in_the_way(&tx)? {
            return Err(e);
        }
        tx.execute_batch(schema)?;
        track(&tx)?;
        tx.pragma_update(None, "user_version", version)?;
        tx.commit()?;
        Ok(())
    }

    /// Brings an existing database to the version `migrations` lead to: runs the migrations from its version on, then
    /// makes the triggers that track changes afresh. A file with nothing in it holds no database. A migration may make
    /// a table afresh, which SQLite does with the foreign keys off: they are off while the migrations run, and every
    /// row that points to another is checked before they commit.
    fn migrate(&mut self, migrations: &[&str]) -> Result<()> {
        let version = 1 + migrations.len() as i64;
        // Settled on reads that take no lock, so that a database which needs nothing, or is refused, does not wait
        // for a writer.
        let at = version_of(&self.conn)?;
        if up_to_date(at, version)? {
            return Ok(());
        }
        if at == 0 {
            return Err(refusal(&self.conn)?);
        }
        // The setting cannot change within a transaction, so it is set around it, and put back whatever comes.
        let keys: bool = self.conn.pragma_query_value(None, "foreign_keys", |r| r.get(0))?;
        self.conn.pragma_update(None, "foreign_keys", false)?;
        let done = self.run_migrations(migrations, version);
        self.conn.pragma_update(None, "foreign_keys", keys)?;
        done
    }

    fn run_migrations(&mut self, migrations: &[&str], version: i64) -> Result<()> {
        let tx = self.conn.transaction_with_behavior(TransactionBehavior::Exclusive)?;
        // Judged again under the lock: another process may have migrated it while this one waited.
        let at = version_of(&tx)?;
        if up_to_date(at, version)? {
            return Ok(());
        }
        for m in &migrations[at as usize - 1..] {
            tx.execute_batch(m)?;
        }
        let broken: i64 = tx.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| r.get(0))?;
        if broken > 0 {
            return Err(Error::Abort(format!(
                "the migration to version {version} leaves {broken} rows pointing to rows that do not exist"
            )));
        }
        track(&tx)?;
        tx.pragma_update(None, "user_version", version)?;
        tx.commit()?;
        Ok(())
    }
}

/// What stops a creation in the database `conn` is open on: a database is there, or other tables are. None when it
/// holds nothing, which a creation takes.
fn in_the_way(conn: &Connection) -> Result<Option<Error>> {
    if version_of(conn)? != 0 {
        return Ok(Some(Error::Exists));
    }
    if !is_empty(conn)? {
        return Ok(Some(Error::Foreign));
    }
    Ok(None)
}

/// Whether a creation at `path` would be refused, judged the way the creation judges it: the file holds a database or
/// other tables. No file, an empty one, and one that cannot be read at all are not told as taken, so that a caller
/// who leaves out checks where a creation will be refused leaves them out only when it is sure.
pub fn is_taken(path: &Path) -> bool {
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let Ok(conn) = Connection::open_with_flags(path, flags) else {
        return false;
    };
    in_the_way(&conn).is_ok_and(|e| e.is_some())
}

/// Whether a database at version `at` is already at `version`; one from a newer wf-agents is refused.
fn up_to_date(at: i64, version: i64) -> Result<bool> {
    if at > version {
        return Err(Error::Newer(at));
    }
    Ok(at == version)
}

/// Why a file at version 0 is no database of this program: it holds nothing, or it holds other tables.
fn refusal(conn: &Connection) -> Result<Error> {
    if is_empty(conn)? {
        return Ok(Error::Missing);
    }
    Ok(Error::Foreign)
}

/// Opens the file at `path` with `flags`, which must leave out the one that makes a file. A file that is not there is
/// no database, whichever way it is opened; any other failure to open is told as SQLite tells it.
fn connect(path: &Path, flags: OpenFlags) -> Result<Connection> {
    match Connection::open_with_flags(path, flags) {
        Ok(conn) => Ok(conn),
        // Only the file system tells a file that is not there from one that cannot be opened.
        Err(_) if matches!(path.try_exists(), Ok(false)) => Err(Error::Missing),
        Err(e) => Err(e.into()),
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

/// Makes `item` depend on `depends_on` by a dependency of the kind given, `blocking` or `parallel`, unless `depends_on`
/// already depends on `item`, directly or not.
pub fn add_dependency(tx: &Transaction<'_>, item: i64, depends_on: i64, needs: &str, kind: &str) -> Result<()> {
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
        "INSERT INTO item_links (item, depends_on, needs, kind) VALUES (?1, ?2, ?3, ?4)",
        params![item, depends_on, needs, kind],
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

/// What agents are told about the database: each table and view they can read, by its name, with its columns' names
/// and the first sentence of the comment that says what it holds, in the order the schema defines them. An agent
/// reads a definition in full, its comments included, with [`DEFINITION`] through the query tool.
pub fn index(conn: &Connection) -> rusqlite::Result<Vec<(String, Vec<String>, String)>> {
    let mut cols = conn.prepare("SELECT name FROM pragma_table_info(?1) ORDER BY cid")?;
    let mut out = Vec::new();
    for (name, first) in described(conn, first_sentence)? {
        let columns = cols
            .query_map([&name], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        out.push((name, columns, first));
    }
    Ok(out)
}

/// Each table and view agents can read, by its name, with the whole first comment of its definition: what it holds,
/// why it exists and who reads it, in the order the schema defines them.
pub fn descriptions(conn: &Connection) -> rusqlite::Result<Vec<(String, String)>> {
    described(conn, description)
}

fn described(conn: &Connection, say: fn(&str) -> String) -> rusqlite::Result<Vec<(String, String)>> {
    let mut st = conn.prepare(
        "SELECT name, sql FROM sqlite_schema WHERE type IN ('table', 'view') AND name NOT LIKE 'sqlite\\_%' ESCAPE '\\'
         AND instr(sql, ?1) = 0 ORDER BY rowid",
    )?;
    let rows = st.query_map([HIDDEN], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    let mut out = Vec::new();
    for row in rows {
        let (name, sql) = row?;
        out.push((name, say(&sql)));
    }
    Ok(out)
}

/// The first comment of a definition, its lines joined.
fn description(sql: &str) -> String {
    let comment: Vec<&str> = sql
        .lines()
        .map(str::trim)
        .skip_while(|l| !l.starts_with("--"))
        .map_while(|l| l.strip_prefix("--"))
        .map(str::trim)
        .collect();
    comment.join(" ")
}

/// The first sentence of the first comment of a definition: its lines joined, up to the first full stop that ends a
/// sentence, so that a sentence that goes on to the next line is not cut short.
fn first_sentence(sql: &str) -> String {
    let text = description(sql);
    match text.find(". ") {
        Some(i) => text[..=i].to_string(),
        None => text,
    }
}

/// What [`index`] gives for a schema, from the schema loaded into a database in memory.
pub fn index_of(schema: &str) -> rusqlite::Result<Vec<(String, Vec<String>, String)>> {
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
            add_dependency(tx, a, b, "b's part", "blocking")?;
            add_dependency(tx, b, c, "c's part", "parallel")?;
            add_dependency(tx, c, a, "a's part", "parallel")
        });
        assert!(matches!(e, Err(Error::Cycle { item: 3, depends_on: 1 })), "{e:?}");
        let n: i64 = db
            .conn()
            .query_row("SELECT count(*) FROM item_links", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0);
    }

    // An item that depends on another by a blocking dependency is not ready to start, queued by the owner or not, until
    // the other is merged; one that depends on it by a parallel dependency is ready at once.
    #[test]
    fn a_blocking_dependency_keeps_an_item_from_starting_until_the_other_merges() {
        let mut db = Db::in_memory().unwrap();
        let ready = |db: &Db| names(db.conn(), "SELECT title FROM ready_items ORDER BY item").unwrap();
        let (_, _, library) = db
            .write(Actor::Owner, |tx| {
                let parser = item(tx, "parser")?;
                let checker = item(tx, "checker")?;
                let library = item(tx, "library")?;
                add_dependency(tx, parser, library, "its lexer", "blocking")?;
                add_dependency(tx, checker, library, "its types", "parallel")?;
                tx.execute("INSERT INTO dispatch_queue (item, position) VALUES (?1, 1.0)", [parser])?;
                Ok((parser, checker, library))
            })
            .unwrap();
        assert_eq!(ready(&db), ["checker", "library"]);
        assert_eq!(
            names(db.conn(), "SELECT needs FROM item_blocked_by").unwrap(),
            ["its lexer"]
        );
        db.write(Actor::Engine, |tx| {
            tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
            tx.execute(
                "INSERT INTO works (item, flow_version, graph, at) VALUES (?1, 1, 'engineer', 'E1')",
                [library],
            )?;
            tx.execute(
                "INSERT INTO rounds (work, n, cell, input, output, transition, config_commit, started_at, ended_at)
                 VALUES (1, 1, 'M2', '{}', '{}', 'merged', 'c', 1, 2)",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        assert_eq!(ready(&db), ["parser", "checker"]);
        assert!(
            names(db.conn(), "SELECT needs FROM item_blocked_by")
                .unwrap()
                .is_empty()
        );
    }

    // A work of a flow version before cells had names keeps the numbers its cells had then: the merge it recorded at the
    // merge cell, then 9, lets the items that depend on it start, and it is done at the end of its graph, then 20.
    #[test]
    fn a_work_of_a_version_before_cells_had_names_is_read_by_its_numbers() {
        let mut db = Db::in_memory().unwrap();
        db.write(Actor::Engine, |tx| {
            let parser = item(tx, "parser")?;
            let library = item(tx, "library")?;
            add_dependency(tx, parser, library, "its lexer", "blocking")?;
            tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
            tx.execute(
                "INSERT INTO works (item, flow_version, graph, at) VALUES (?1, 1, 'engineer', '20')",
                [library],
            )?;
            tx.execute(
                "INSERT INTO rounds (work, n, cell, input, output, transition, config_commit, started_at, ended_at)
                 VALUES (1, 1, '9', '{}', '{}', 'merged', 'c', 1, 2)",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        assert_eq!(
            names(db.conn(), "SELECT title FROM ready_items ORDER BY item").unwrap(),
            ["parser"]
        );
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
        // The index names each table and view with its columns and the first sentence of its description, whole
        // however many lines it takes; the query it points to gives the whole definition with its comments.
        let index = index(db.conn()).unwrap();
        let names: Vec<&str> = index.iter().map(|(n, _, _)| n.as_str()).collect();
        assert!(names.contains(&"items") && names.contains(&"ready_items"));
        assert!(!names.contains(&"sessions"));
        let (_, columns, first) = index.iter().find(|(n, _, _)| n == "items").unwrap();
        assert_eq!(first, "A piece of work to do, as its creator wrote it.");
        assert_eq!(columns[..3], ["id", "project", "title"]);
        let (_, columns, _) = index.iter().find(|(n, _, _)| n == "ready_items").unwrap();
        assert!(columns.contains(&"position".to_string()), "{columns:?}");
        for (n, _, first) in &index {
            assert!(first.ends_with('.') && !first.contains(". "), "{n}: {first}");
        }
        assert_eq!(
            first_sentence(
                "CREATE VIEW v AS\n  -- Rows the work holds: its findings, as the\n  -- review wrote them. Read by: cell `T6`.\n  SELECT 1"
            ),
            "Rows the work holds: its findings, as the review wrote them."
        );
        let full: String = agent
            .query_row(&DEFINITION.replace("<name>", "items"), [], |r| r.get(0))
            .unwrap();
        assert!(
            full.contains("-- Item number, one sequence across all projects."),
            "{full}"
        );
    }

    /// A database as it was at an earlier version: made at version 1 and migrated up to `version`.
    fn at_version(version: i64) -> Db {
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
        old.migrate(&MIGRATIONS[..version as usize - 1]).unwrap();
        old
    }

    // A database made at version 1 and migrated has the shape of one created from the schema, and keeps its rounds
    // and every row that points to them; each work's rounds are numbered from 1 in the order they were made.
    #[test]
    fn a_database_migrated_from_version_1_has_the_shape_of_a_new_one() {
        let mut old = at_version(1);
        old.write(Actor::Engine, |tx| {
            let id = item(tx, "kept")?;
            tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
            for _ in [1, 2] {
                tx.execute(
                    "INSERT INTO works (item, flow_version, graph, at) VALUES (?1, 1, 'engineer', '1')",
                    [id],
                )?;
            }
            // Rounds 1 and 3 are work 1's, round 2 is work 2's.
            for (work, input) in [(1, "{\"prompt\":\"p\"}"), (2, "{}"), (1, "{\"prompt\":\"q\"}")] {
                tx.execute(
                    "INSERT INTO rounds (work, cell, input, config_commit, started_at) VALUES (?1, '1', ?2, 'c', 1)",
                    params![work, input],
                )?;
            }
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
                "INSERT INTO tool_runs (round, tool, args, attempt, queued_at) VALUES (3, 'gh', '{}', 1, 1)",
                [],
            )?;
            tx.execute(
                "INSERT INTO interjections (work, speaker, text, delivered_round) VALUES (2, 'owner', 'x', 2)",
                [],
            )?;
            tx.execute(
                "UPDATE items SET acceptance = '[]', acceptance_round = 3 WHERE id = ?1",
                [id],
            )?;
            Ok(())
        })
        .unwrap();
        old.migrate(MIGRATIONS).unwrap();
        let new = Db::in_memory().unwrap();
        assert_eq!(shape(old.conn()).unwrap(), shape(new.conn()).unwrap());
        // A table made afresh for its description has the description the schema writes, which agents read.
        for table in ["items", "architect_matters"] {
            let definition = |db: &Db| -> String {
                db.conn()
                    .query_row("SELECT sql FROM sqlite_schema WHERE name = ?1", [table], |r| r.get(0))
                    .unwrap()
            };
            assert_eq!(definition(&old), definition(&new), "{table}");
        }
        let input = |work: i64, n: i64| -> String {
            old.conn()
                .query_row("SELECT input FROM rounds WHERE work = ?1 AND n = ?2", [work, n], |r| {
                    r.get(0)
                })
                .unwrap()
        };
        assert_eq!(input(1, 1), "{\"prompt\":\"p\"}");
        assert_eq!(input(1, 2), "{\"prompt\":\"q\"}");
        assert_eq!(input(2, 1), "{}");
        let pair = |q: &str| -> (i64, i64) { old.conn().query_row(q, [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap() };
        assert_eq!(pair("SELECT work, round FROM tool_runs"), (1, 2));
        assert_eq!(pair("SELECT acceptance_work, acceptance_round FROM items"), (1, 2));
        assert_eq!(pair("SELECT work, delivered_round FROM interjections"), (2, 1));
        assert_eq!(pair("SELECT work, round FROM design_cards"), (1, 1));
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

    /// A way to open a database that exists.
    type Open = fn(&Path) -> Result<Db>;

    /// The ways a database is opened to be used, each with a name for the failure message.
    const OPENS: [(&str, Open); 2] = [("open", Db::open), ("open_read_only", Db::open_read_only)];

    // A database from a newer wf-agents, or a file that is not one, is not touched, however it is opened; one older
    // than this wf-agents is not migrated by an open that only reads.
    #[test]
    fn a_newer_or_foreign_database_is_refused() {
        let path = temp("newer");
        Db::create(&path).unwrap();
        Connection::open(&path)
            .unwrap()
            .pragma_update(None, "user_version", VERSION + 1)
            .unwrap();
        let foreign = temp("foreign");
        Connection::open(&foreign)
            .unwrap()
            .execute_batch("CREATE TABLE t (x)")
            .unwrap();
        for (how, open) in OPENS {
            assert!(matches!(open(&path), Err(Error::Newer(v)) if v == VERSION + 1), "{how}");
            assert!(matches!(open(&foreign), Err(Error::Foreign)), "{how}");
        }
        let older = temp("older");
        Db::create(&older).unwrap();
        Connection::open(&older)
            .unwrap()
            .pragma_update(None, "user_version", VERSION - 1)
            .unwrap();
        assert!(matches!(Db::open_read_only(&older), Err(Error::Older(v)) if v == VERSION - 1));
    }

    // Opening makes nothing, however it is done: a path with no file gets none, and a file with nothing in it is no
    // database and stays as it was.
    #[test]
    fn opening_a_missing_or_empty_file_is_refused_and_makes_nothing() {
        for (how, open) in OPENS {
            let path = temp(&format!("missing-{how}"));
            assert!(matches!(open(&path), Err(Error::Missing)), "{how}");
            assert!(!path.exists(), "{how}");
            std::fs::write(&path, b"").unwrap();
            assert!(matches!(open(&path), Err(Error::Missing)), "{how}");
            assert_eq!(std::fs::metadata(&path).unwrap().len(), 0, "{how}");
            assert!(!Path::new(&format!("{}-wal", path.display())).exists(), "{how}");
        }
    }

    /// The database has the shape of a new one in memory, triggers and version included.
    fn assert_like_new(db: &Db) {
        let new = Db::in_memory().unwrap();
        assert_eq!(shape(db.conn()).unwrap(), shape(new.conn()).unwrap());
        assert_eq!(version_of(db.conn()).unwrap(), VERSION);
    }

    // A database made at a path, whether there was no file or an empty one, has the shape of a new one in memory, with
    // its version and triggers, and opens again; a place with a database or other tables in it is refused and what is
    // there stays.
    #[test]
    fn creating_makes_a_new_database_and_refuses_a_place_with_anything_in_it() {
        let path = temp("create");
        let mut made = Db::create(&path).unwrap();
        assert_like_new(&made);
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
        let created = Db::create(&empty).unwrap();
        assert_like_new(&created);
        Db::open(&empty).unwrap();
    }

    // Before a creation, a place it would refuse is told by the rule it refuses by: a database, or other tables. No
    // file, an empty file, and a file SQLite has written a header to that holds nothing are not in the way.
    #[test]
    fn a_place_a_creation_would_refuse_is_told_beforehand() {
        let none = temp("taken-none");
        assert!(!is_taken(&none));
        let empty = temp("taken-empty");
        std::fs::write(&empty, b"").unwrap();
        assert!(!is_taken(&empty));
        let header = temp("taken-header");
        Connection::open(&header)
            .unwrap()
            .pragma_update(None, "journal_mode", "WAL")
            .unwrap();
        assert!(std::fs::metadata(&header).unwrap().len() > 0);
        assert!(!is_taken(&header));
        Db::create(&header).unwrap();
        assert!(is_taken(&header));
        let foreign = temp("taken-foreign");
        Connection::open(&foreign)
            .unwrap()
            .execute_batch("CREATE TABLE t (x)")
            .unwrap();
        assert!(is_taken(&foreign));
    }

    // Of two creations at the same path at once only one builds; the other is told a database is there, and the
    // database opens.
    #[test]
    fn of_two_creations_at_once_only_one_builds() {
        for round in 0..8 {
            let path = temp(&format!("race-{round}"));
            let start = std::sync::Barrier::new(2);
            let racer = || {
                start.wait();
                Db::create(&path).map(|_| ())
            };
            let made = std::thread::scope(|s| {
                let (a, b) = (s.spawn(racer), s.spawn(racer));
                [a.join().unwrap(), b.join().unwrap()]
            });
            let built = made.iter().filter(|r| r.is_ok()).count();
            let told = made.iter().filter(|r| matches!(r, Err(Error::Exists))).count();
            assert_eq!((built, told), (1, 1), "{made:?}");
            Db::open(&path).unwrap();
        }
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

    // A design card an earlier engine left open beside a newer card of its work is replaced by the next one; the
    // newest stays, and so does a card of another work.
    #[test]
    fn a_design_card_left_open_beside_a_newer_one_is_replaced() {
        // The database as it was at version 8, before the repair, and the repair alone run on it.
        let mut db = at_version(8);
        db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO items (project, title, problem, why, deliver, level) VALUES ('p', 't', 'x', 'y', 'change', '1')",
                [],
            )?;
            tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
            for w in [1, 2] {
                tx.execute(
                    "INSERT INTO works (item, flow_version, graph, at) VALUES (1, 1, 'engineer', '4')",
                    [],
                )?;
                tx.execute(
                    "INSERT INTO rounds (work, cell, input, config_commit, started_at) VALUES (?1, '2a', '{}', 'c', 1)",
                    [w],
                )?;
            }
            // Work 1: cards 1 and 2 left open beside card 3. Work 2: card 4 alone.
            for (round, status) in [(1, "open"), (1, "ruled"), (1, "open"), (2, "open")] {
                tx.execute(
                    "INSERT INTO design_cards (round, body, status) VALUES (?1, '[]', ?2)",
                    params![round, status],
                )?;
            }
            Ok(())
        })
        .unwrap();
        db.migrate(&MIGRATIONS[..8]).unwrap();
        let mut st = db
            .conn()
            .prepare("SELECT status, replaced_by FROM design_cards ORDER BY id")
            .unwrap();
        let cards: Vec<(String, Option<i64>)> = st
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(|x| x.unwrap())
            .collect();
        let want = [
            ("replaced", Some(2)),
            ("replaced", Some(3)),
            ("open", None),
            ("open", None),
        ];
        let want: Vec<(String, Option<i64>)> = want.iter().map(|(s, r)| (s.to_string(), *r)).collect();
        assert_eq!(cards, want);
    }

    // Taken to version 23, a database keeps what an agent told the owner once: each notice of the engine's that repeats
    // an agent's message of its work word for word is removed, and every other notice stays.
    #[test]
    fn a_notice_that_repeats_an_agents_message_is_removed() {
        let mut db = at_version(22);
        db.write(Actor::Engine, |tx| {
            item(tx, "t")?;
            tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
            tx.execute(
                "INSERT INTO works (item, flow_version, graph, at) VALUES (1, 1, 'engineer', '1')",
                [],
            )?;
            tx.execute(
                "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES (1, 1, '1', '{}', 'c', 1)",
                [],
            )?;
            tx.execute(
                "INSERT INTO owner_messages (work, round, text) VALUES (1, 1, 'Look at the log.')",
                [],
            )?;
            tx.execute(
                "INSERT INTO engine_notices (work, text, at) VALUES
                   (1, 'The engineer of work 1 (item 1: t) at cell 1 tells you: Look at the log.', 2),
                   (1, 'Work 1 stopped.', 3),
                   (1, 'The engineer of work 1 (item 1: t) at cell 1 tells you: Not told as a message.', 4)",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        db.migrate(&MIGRATIONS[..22]).unwrap();
        let mut st = db
            .conn()
            .prepare("SELECT text FROM engine_notices ORDER BY id")
            .unwrap();
        let left: Vec<String> = st.query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect();
        assert_eq!(
            left,
            [
                "Work 1 stopped.",
                "The engineer of work 1 (item 1: t) at cell 1 tells you: Not told as a message."
            ]
        );
    }

    // Taken to version 14, a database gives the architect a look at each design card not replaced that the owner has
    // not confirmed, and one matter for the side reports handed in before, to look through them all, a report since
    // version 20, which holds each text once; a database with no side report gets no such matter.
    #[test]
    fn the_architect_is_given_the_unconfirmed_cards_and_the_side_reports_handed_in_before() {
        let mut old = at_version(13);
        old.write(Actor::Engine, |tx| {
            item(tx, "t")?;
            tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
            tx.execute(
                "INSERT INTO works (item, flow_version, graph, at) VALUES (1, 1, 'engineer', '4')",
                [],
            )?;
            let reports = r#"{"side_reports":[{"kind":"duplicate","what":"Item 7 does the same.","urgent":true},
                {"kind":"other","what":"It needs a machine."}]}"#;
            let again = r#"{"side_reports":[{"kind":"other","what":" It needs a machine.\n"}]}"#;
            for (n, output) in [(1, reports), (2, r#"{"side_reports":[]}"#), (3, "{}"), (4, again)] {
                tx.execute(
                    "INSERT INTO rounds (work, n, cell, input, output, config_commit, started_at)
                     VALUES (1, ?1, '4', '{}', ?2, 'c', 1)",
                    params![n, output],
                )?;
            }
            let two = r#"[{"node":"a"},{"node":"b"}]"#;
            let confirmed = r#"[{"by":"owner","at":1,"decisions":[{"decision":1,"verdict":"confirm"},
                {"decision":2,"verdict":"confirm"}]}]"#;
            let whole = r#"[{"by":"owner","verdict":"confirm","text":""}]"#;
            let disagreed = r#"[{"by":"owner","at":1,"decisions":[{"decision":1,"verdict":"confirm"},
                {"decision":2,"verdict":"disagree","text":"Keep it."}]}]"#;
            let cards = [
                (two, "open", "[]"),
                (two, "ruled", confirmed),
                ("[]", "ruled", whole),
                (two, "ruled", disagreed),
                (two, "replaced", "[]"),
            ];
            for (body, status, rulings) in cards {
                tx.execute(
                    "INSERT INTO design_cards (work, round, body, status, rulings) VALUES (1, 1, ?1, ?2, ?3)",
                    params![body, status, rulings],
                )?;
            }
            Ok(())
        })
        .unwrap();
        old.migrate(MIGRATIONS).unwrap();
        let mut st = old
            .conn()
            .prepare("SELECT kind, what, body FROM architect_matters ORDER BY id")
            .unwrap();
        let matters: Vec<(String, String, String)> = st
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap()
            .map(|x| x.unwrap())
            .collect();
        let kinds: Vec<(&str, &str)> = matters.iter().map(|m| (m.0.as_str(), m.1.as_str())).collect();
        assert_eq!(
            kinds,
            [
                ("design_card", "Design card 1 of work 1 to look at"),
                ("design_card", "Design card 4 of work 1 to look at"),
                ("report", "Look through the 2 side reports handed in before you ran"),
            ]
        );
        let body: serde_json::Value = serde_json::from_str(&matters[2].2).unwrap();
        assert_eq!(
            body,
            serde_json::json!({ "side_reports": [
                { "work": 1, "round": 1, "cell": "4", "index": 0,
                  "report": { "kind": "duplicate", "what": "Item 7 does the same.", "urgent": true } },
                { "work": 1, "round": 1, "cell": "4", "index": 1,
                  "report": { "kind": "other", "what": "It needs a machine." } }
            ] })
        );
        let mut none = at_version(13);
        none.migrate(MIGRATIONS).unwrap();
        let n: i64 = none
            .conn()
            .query_row("SELECT count(*) FROM architect_matters", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0);
    }

    // A ruling with a verdict on each decision confirms its design card only when every decision is confirmed; one
    // made before rulings were per decision confirms or disagrees with the card as a whole.
    #[test]
    fn a_design_card_is_confirmed_only_when_every_decision_is() {
        let ruling = |ds: serde_json::Value| serde_json::json!({ "by": "owner", "at": 1, "decisions": ds });
        let all = ruling(serde_json::json!([
            { "decision": 1, "verdict": "confirm" },
            { "decision": 2, "verdict": "confirm" }
        ]));
        assert!(ruling_confirms(&all, 2) && !ruling_disagrees(&all));
        assert!(!ruling_confirms(&all, 3));
        let one_not = ruling(serde_json::json!([
            { "decision": 1, "verdict": "confirm" },
            { "decision": 2, "verdict": "disagree", "option": "B", "text": "Keep it." },
            { "decision": 3, "verdict": "confirm" }
        ]));
        assert!(!ruling_confirms(&one_not, 3) && ruling_disagrees(&one_not));
        assert_eq!(
            ruling_verdicts(&one_not)[1],
            (2, "disagree".to_string(), Some("B".to_string()))
        );
        let whole = serde_json::json!({ "by": "owner", "verdict": "confirm", "text": "" });
        assert!(ruling_confirms(&whole, 3) && !ruling_disagrees(&whole));
        assert!(ruling_disagrees(
            &serde_json::json!({ "verdict": "disagree", "text": "no" })
        ));
    }

    // A work paused before its pause said who paused it is the owner's when the owner's change paused it, and the
    // engine's otherwise; a work that goes on says no one.
    #[test]
    fn a_work_paused_before_says_who_paused_it() {
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
        old.create_schema(include_str!("../../../db/migrations/0001.sql"), &[])
            .unwrap();
        // The database as it was at version 12, the version before works said who paused them.
        old.migrate(&MIGRATIONS[..11]).unwrap();
        old.write(Actor::Engine, |tx| {
            let id = item(tx, "paused")?;
            tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
            for _ in 0..3 {
                tx.execute(
                    "INSERT INTO works (item, flow_version, graph, at) VALUES (?1, 1, 'engineer', '1')",
                    [id],
                )?;
            }
            tx.execute("UPDATE works SET paused = 'cell 1: no' WHERE id = 1", [])?;
            Ok(())
        })
        .unwrap();
        old.write(Actor::Owner, |tx| {
            tx.execute("UPDATE works SET paused = 'later' WHERE id = 2", [])?;
            Ok(())
        })
        .unwrap();
        old.migrate(MIGRATIONS).unwrap();
        let mut st = old.conn().prepare("SELECT paused_by FROM works ORDER BY id").unwrap();
        let by: Vec<Option<String>> = st.query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect();
        assert_eq!(by, [Some("engine".to_string()), Some("owner".to_string()), None]);
        // The migration's own change is not recorded as a writer's.
        let n: i64 = old
            .conn()
            .query_row(
                "SELECT count(*) FROM changes WHERE json_extract(new_row, '$.paused_by') IS NOT NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 0);
    }

    // Taken to version 20, a database of one project gives its architect's wakes that project, ends the wake still open
    // at a cell of the old graph with its round and answers the asks only that wake waited for or that asked to approve
    // changes; a matter not handled takes the kind of the new graph with its old kind as its detail, one held for
    // approval is back in the queue, one held by an ask still waiting stays held, to go back to the cell that decides
    // it (by its name, once cells have names), and one handled stays as it was; the architect's hold on new starts is
    // the project's.
    #[test]
    fn each_project_has_its_architect_on_the_new_graph() {
        let mut old = at_version(19);
        old.write(Actor::Engine, |tx| {
            item(tx, "t")?;
            tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
            tx.execute(
                "INSERT INTO works (item, flow_version, graph, at) VALUES (1, 1, 'engineer', '15')",
                [],
            )?;
            tx.execute(
                "INSERT INTO works (flow_version, graph, at) VALUES (1, 'architect', 'A5')",
                [],
            )?;
            tx.execute(
                "INSERT INTO works (flow_version, graph, at, ended) VALUES (1, 'architect', 'A12', 'done')",
                [],
            )?;
            for (work, cell, ended) in [(1, "15", Some(1)), (2, "A5", None), (3, "A3", Some(1))] {
                tx.execute(
                    "INSERT INTO rounds (work, n, cell, input, config_commit, started_at, ended_at)
                     VALUES (?1, 1, ?2, '{}', 'c', 1, ?3)",
                    params![work, cell, ended],
                )?;
            }
            for body in [
                r#"{"from":"architect","kind":"changes"}"#,
                r#"{"from":"architect","holds":[3]}"#,
                r#"{"from":"architect"}"#,
            ] {
                tx.execute("INSERT INTO owner_asks (work, round, body) VALUES (2, 1, ?1)", [body])?;
            }
            // Each matter's kind, the engineer's work that raised it, how it is held and by which ask, and the wake
            // that handled it.
            type Old<'a> = (&'a str, Option<i64>, Option<&'a str>, Option<i64>, Option<i64>);
            let matters: [Old; 4] = [
                ("split", Some(1), None, None, None),
                ("tidy_board", None, Some("approval"), None, Some(3)),
                ("tool_failed", None, Some("owner_ask"), Some(2), Some(2)),
                ("advance", None, None, None, Some(3)),
            ];
            for (kind, work, held, held_by, handled) in matters {
                tx.execute(
                    "INSERT INTO architect_matters (kind, what, body, item, work, round, at, held, held_by,
                       handled_work, handled_round)
                     VALUES (?1, 'w', '{}', 1, ?2, ?2, 1, ?3, ?4, ?5, ?6)",
                    params![kind, work, held, held_by, handled, handled.map(|_| 1)],
                )?;
            }
            tx.execute("INSERT INTO dispatch_holds (reason, since) VALUES ('architect', 1)", [])?;
            tx.execute("INSERT INTO dispatch_holds (reason, since) VALUES ('tidying', 1)", [])?;
            Ok(())
        })
        .unwrap();
        old.migrate(MIGRATIONS).unwrap();
        let conn = old.conn();
        let works: Vec<(Option<String>, Option<String>)> = {
            let mut st = conn.prepare("SELECT project, ended FROM works ORDER BY id").unwrap();
            st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
                .unwrap()
                .map(|x| x.unwrap())
                .collect()
        };
        assert_eq!(works[0], (None, None));
        assert!(works[1].0.as_deref() == Some("p") && works[1].1.as_deref().is_some_and(|e| e.starts_with("stopped:")));
        assert_eq!(works[2], (Some("p".into()), Some("done".into())));
        let ended: Option<i64> = conn
            .query_row("SELECT ended_at FROM rounds WHERE work = 2", [], |r| r.get(0))
            .unwrap();
        assert!(ended.is_some());
        let answered: Vec<bool> = {
            let mut st = conn
                .prepare("SELECT answer IS NOT NULL FROM owner_asks ORDER BY id")
                .unwrap();
            st.query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect()
        };
        assert_eq!(answered, [true, false, true]);
        type Matter = (String, Option<String>, Option<i64>, Option<String>, Option<i64>);
        let matters: Vec<Matter> = {
            let mut st = conn
                .prepare("SELECT kind, detail, asked, resume_cell, handled_round FROM architect_matters ORDER BY id")
                .unwrap();
            st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))
                .unwrap()
                .map(|x| x.unwrap())
                .collect()
        };
        let m = |kind: &str, detail: Option<&str>, asked: Option<i64>, cell: Option<&str>, handled: Option<i64>| {
            (
                kind.to_string(),
                detail.map(str::to_string),
                asked,
                cell.map(str::to_string),
                handled,
            )
        };
        assert_eq!(
            matters,
            [
                m("engineer_question", Some("split"), None, None, None),
                m("set_straight", Some("tidy_board"), None, None, None),
                m("failure", Some("tool_failed"), Some(2), Some("H7"), None),
                m("advance", None, None, None, Some(1)),
            ]
        );
        let queue: Vec<i64> = {
            let mut st = conn
                .prepare("SELECT matter FROM architect_queue WHERE project = 'p'")
                .unwrap();
            st.query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect()
        };
        assert_eq!(queue, [1, 2]);
        let holds: Vec<(String, String)> = {
            let mut st = conn.prepare("SELECT project, reason FROM dispatch_holds").unwrap();
            st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
                .unwrap()
                .map(|x| x.unwrap())
                .collect()
        };
        assert_eq!(holds, [("p".to_string(), "architect".to_string())]);
    }

    // Taken to version 19, a database has no plan: every item is ranked among its project's items by the order it was
    // made, the owner's queue keeps its rows, and the items to start come from the queue first, then by rank.
    #[test]
    fn the_items_are_ranked_in_each_project_by_the_order_they_were_made() {
        let mut old = at_version(18);
        old.write(Actor::Owner, |tx| {
            for (project, title) in [("p", "a"), ("q", "b"), ("p", "c"), ("p", "d")] {
                tx.execute(
                    "INSERT INTO items (project, title, problem, why, deliver, level) VALUES (?1, ?2, 'x', 'y', 'change', 'l')",
                    [project, title],
                )?;
            }
            tx.execute(
                "INSERT INTO plan_segments (key, project, title, stage, delivers, done_when, assumptions, indicators, size)
                 VALUES ('s1', 'p', 't', '1', 'd', 'w', '[]', '[]', 'small')",
                [],
            )?;
            tx.execute("UPDATE items SET segment = 's1', touches = '[]' WHERE id = 1", [])?;
            tx.execute("UPDATE items SET held = 'later' WHERE id = 4", [])?;
            tx.execute("INSERT INTO dispatch_queue (item, position) VALUES (3, 1.0)", [])?;
            Ok(())
        })
        .unwrap();
        old.migrate(MIGRATIONS).unwrap();
        let rows = |q: &str| -> Vec<(i64, Option<i64>)> {
            let mut st = old.conn().prepare(q).unwrap();
            st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
                .unwrap()
                .map(|x| x.unwrap())
                .collect()
        };
        assert_eq!(
            rows("SELECT id, rank FROM items ORDER BY id"),
            [(1, Some(1)), (2, Some(1)), (3, Some(2)), (4, Some(3))]
        );
        assert_eq!(
            rows("SELECT item, rank FROM ready_items"),
            [(3, Some(2)), (1, Some(1)), (2, Some(1))]
        );
        let queued: Vec<(i64, f64)> = {
            let mut st = old.conn().prepare("SELECT item, position FROM dispatch_queue").unwrap();
            st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
                .unwrap()
                .map(|x| x.unwrap())
                .collect()
        };
        assert_eq!(queued, [(3, 1.0)]);
        let plan: i64 = old
            .conn()
            .query_row("SELECT count(*) FROM sqlite_schema WHERE name LIKE 'plan%'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(plan, 0);
    }

    // What a stopped task waits on becomes one record: each ask of the owner, each engineer's question with the
    // architect's reply, the wait at `A6` and the merge gate's wait are blockers, those still open the open blocker of
    // their work, held by the owner, the project's architect, or the items not merged yet.
    #[test]
    fn what_stopped_tasks_wait_on_becomes_their_blockers() {
        let mut old = at_version(35);
        old.write(Actor::Engine, |tx| {
            for t in ["asks", "asks again", "waits", "merges", "needed"] {
                item(tx, t)?;
            }
            tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
            // Work 1 asked the owner, who answered, then asked again; work 2 asked the architect, which replied;
            // work 3 waits at A6 for item 5; work 4 waits at the merge gate for it.
            for (item, at) in [(1, "A3"), (2, "A4"), (3, "A6"), (4, "M1")] {
                tx.execute(
                    "INSERT INTO works (item, flow_version, graph, at) VALUES (?1, 1, 'engineer', ?2)",
                    params![item, at],
                )?;
            }
            let rounds: [(i64, i64, &str, Option<&str>); 9] = [
                (1, 1, "R1", Some("ask_owner")),
                (1, 2, "A3", Some("answered")),
                (1, 3, "D1", Some("ask_owner")),
                (1, 4, "A3", None),
                (2, 1, "D1", Some("ask_architect")),
                (2, 2, "A2", Some("answered")),
                (3, 1, "A4", Some("wait_for_items")),
                (4, 1, "T4", Some("done")),
                (4, 2, "M1", Some("wait")),
            ];
            for (work, n, cell, t) in rounds {
                tx.execute(
                    "INSERT INTO rounds (work, n, cell, input, transition, config_commit, started_at, ended_at)
                     VALUES (?1, ?2, ?3, '{}', ?4, 'c', ?2, ?2)",
                    params![work, n, cell, t],
                )?;
            }
            tx.execute(
                "INSERT INTO owner_asks (work, round, body, answer) VALUES (1, 2, '{\"to\":\"owner\"}', '{\"text\":\"yes\"}')",
                [],
            )?;
            tx.execute("INSERT INTO owner_asks (work, round, body) VALUES (1, 4, '{\"to\":\"owner\"}')", [])?;
            tx.execute(
                "INSERT INTO architect_matters (project, kind, what, body, item, work, round, at, reply)
                 VALUES ('p', 'engineer_question', 'which', '{}', 2, 2, 2, 5, '{\"text\":\"this\"}')",
                [],
            )?;
            for i in [3, 4] {
                tx.execute(
                    "INSERT INTO item_links (item, depends_on, needs, kind) VALUES (?1, 5, 'it', 'parallel')",
                    [i],
                )?;
            }
            Ok(())
        })
        .unwrap();
        old.migrate(MIGRATIONS).unwrap();
        type Row = (i64, String, String, String, Option<String>, Option<String>);
        let rows: Vec<Row> = {
            let mut st = old
                .conn()
                .prepare("SELECT work, cell, kind, holder, items, ended FROM blockers ORDER BY id")
                .unwrap();
            st.query_map([], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))
            })
            .unwrap()
            .map(|x| x.unwrap())
            .collect()
        };
        let waited = Some(r#"[{"item":5,"project":"p"}]"#.to_string());
        assert_eq!(
            rows,
            [
                (
                    1,
                    "R1".into(),
                    "owner_ask".into(),
                    "owner".into(),
                    None,
                    Some("resolved".into())
                ),
                (1, "D1".into(), "owner_ask".into(), "owner".into(), None, None),
                (
                    2,
                    "D1".into(),
                    "question".into(),
                    "architect".into(),
                    None,
                    Some("resolved".into())
                ),
                (3, "A6".into(), "items".into(), "items".into(), waited.clone(), None),
                (4, "M1".into(), "items".into(), "items".into(), waited, None),
            ]
        );
    }
}
