//! The wf-agents database. `db/schema.sql` is its only definition; this crate carries it into the binary, creates a
//! database from it or brings an older one up to it, writes every change to a state table into `changes`, and opens
//! the read-only connection agents query through.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fmt;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::functions::FunctionFlags;
use rusqlite::hooks::{Action, AuthAction, AuthContext, Authorization};
use rusqlite::{Connection, OpenFlags, ToSql, Transaction, TransactionBehavior, params};

pub use rusqlite;

/// The database's definition, as `db/schema.sql` writes it.
pub const SCHEMA: &str = include_str!("../../../db/schema.sql");

/// The steps that bring an existing database up to [`SCHEMA`]: the first takes version 1 to 2, the next 2 to 3, and
/// so on. A change to `db/schema.sql` adds one in the same pull request. The first one also adds
/// `db/migrations/0001.sql`, the schema as it was at version 1, and a test that a database migrated from it has the
/// shape of one created from [`SCHEMA`].
pub const MIGRATIONS: &[&str] = &[];

/// The version [`SCHEMA`] defines, kept in the database's `user_version`.
pub const VERSION: i64 = 1 + MIGRATIONS.len() as i64;

/// Tables whose rows are history themselves, or the engine's bookkeeping: changes to them are not tracked.
const UNTRACKED: [&str; 4] = ["rounds", "tool_runs", "changes", "outbox"];

/// The line a table's description has when agents may not read it.
const HIDDEN: &str = "Hidden from agents.";

#[derive(Debug)]
pub enum Error {
    Sql(rusqlite::Error),
    /// The database was made by a newer wf-agents: its version.
    Newer(i64),
    /// The file holds tables but is not a wf-agents database.
    Foreign,
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
    /// Opens the database at `path`: creates it from the schema when the file is new, and migrates it when it is
    /// older than this wf-agents.
    pub fn open(path: &Path) -> Result<Db> {
        let db = Db::set_up(Connection::open(path)?)?;
        db.conn.pragma_update(None, "journal_mode", "WAL")?;
        Ok(db)
    }

    /// A new database in memory.
    pub fn in_memory() -> Result<Db> {
        Db::set_up(Connection::open_in_memory()?)
    }

    fn set_up(conn: Connection) -> Result<Db> {
        conn.busy_timeout(Duration::from_secs(10))?;
        conn.pragma_update(None, "foreign_keys", true)?;
        let writer: Current = Arc::new(Mutex::new(None));
        register(&conn, "wf_tx", &writer, |w| w.tx)?;
        register(&conn, "wf_at", &writer, |w| w.at)?;
        register(&conn, "wf_actor", &writer, |w| w.actor.name())?;
        let mut db = Db {
            conn,
            writer,
            watch: Arc::default(),
            listener: None,
        };
        db.upgrade(SCHEMA, MIGRATIONS)?;
        Ok(db)
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

    /// Brings the database to the version `schema` defines: creates it from `schema` when it is empty, or runs the
    /// migrations from its version on; then makes the triggers that track changes afresh.
    fn upgrade(&mut self, schema: &str, migrations: &[&str]) -> Result<()> {
        let version = 1 + migrations.len() as i64;
        let at: i64 = self.conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if at > version {
            return Err(Error::Newer(at));
        }
        if at == version {
            return Ok(());
        }
        let empty: bool = self
            .conn
            .query_row("SELECT count(*) = 0 FROM sqlite_schema", [], |r| r.get(0))?;
        let tx = self.conn.transaction_with_behavior(TransactionBehavior::Exclusive)?;
        if at == 0 {
            if !empty {
                return Err(Error::Foreign);
            }
            tx.execute_batch(schema)?;
        } else {
            for m in &migrations[at as usize - 1..] {
                tx.execute_batch(m)?;
            }
        }
        track(&tx)?;
        tx.pragma_update(None, "user_version", version)?;
        tx.commit()?;
        Ok(())
    }
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

/// What agents are told about the database: the definition of every table and view they can read, with the
/// comments that say what each holds and who reads it, in the order the schema defines them.
pub fn described(conn: &Connection) -> rusqlite::Result<String> {
    let mut st = conn.prepare(
        "SELECT sql FROM sqlite_schema WHERE type IN ('table', 'view') AND name NOT LIKE 'sqlite\\_%' ESCAPE '\\'
         AND instr(sql, ?1) = 0 ORDER BY rowid",
    )?;
    let defs: Vec<String> = st.query_map([HIDDEN], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
    Ok(defs.iter().map(|d| format!("{d};\n")).collect::<Vec<_>>().join("\n"))
}

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
        let mut db = Db::open(&path).unwrap();
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
        let d = described(db.conn()).unwrap();
        assert!(d.contains("CREATE TABLE items") && d.contains("CREATE VIEW ready_items"));
        assert!(!d.contains("CREATE TABLE sessions"));
        assert!(d.contains("-- Item number, one sequence across all projects."));
    }

    // A database from a newer wf-agents, or a file that is not one, is not touched.
    #[test]
    fn a_newer_or_foreign_database_is_refused() {
        let path = temp("newer");
        Db::open(&path).unwrap();
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
            };
            db.upgrade(schema, migrations).unwrap();
            db
        };
        let mut old = fresh(v1, &[]);
        old.upgrade(v2, &[step]).unwrap();
        let new = fresh(v2, &[step]);
        assert_eq!(shape(old.conn()).unwrap(), shape(new.conn()).unwrap());
        let mut wrong = fresh(v1, &[]);
        wrong
            .upgrade(v2, &["ALTER TABLE things ADD COLUMN size INTEGER;"])
            .unwrap();
        assert_ne!(shape(wrong.conn()).unwrap(), shape(new.conn()).unwrap());
        assert_eq!(VERSION, 1 + MIGRATIONS.len() as i64);
    }
}
