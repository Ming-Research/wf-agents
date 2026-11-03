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

pub mod attachments;
pub mod backup;
mod cells;
mod holds;
mod matters;
mod questions;
mod start_over;

pub use cells::{cells_sql, code_of, is_cell};
pub use holds::{HoldReason, hold_starts, holds_of, release_starts};
pub use matters::{
    Declared, FAULT, Forwarder, Matter, Speaker, fault_forwarding, forward_fault, forward_new_fault, lift_pause,
    one_sentence, project_of_item, queue_matter, say_to_architect, set_fault_forwarding, without_final_terminal,
};
pub use questions::{QUESTIONS_PENDING, question_waited_on};
pub use start_over::{
    CANCEL_ITEM, RESUME_WORK, START_OVER, ask_cancel, ask_resume, cancel_asked_too, cannot_start_over, merge_stands,
    resume_asked, resume_asker, start_over, stop_asked, stopping, withdraw_resume,
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
    include_str!("../../../db/migrations/0042.sql"),
    include_str!("../../../db/migrations/0043.sql"),
    include_str!("../../../db/migrations/0044.sql"),
    include_str!("../../../db/migrations/0045.sql"),
    include_str!("../../../db/migrations/0046.sql"),
    include_str!("../../../db/migrations/0047.sql"),
    include_str!("../../../db/migrations/0048.sql"),
    include_str!("../../../db/migrations/0049.sql"),
    include_str!("../../../db/migrations/0050.sql"),
    include_str!("../../../db/migrations/0051.sql"),
    include_str!("../../../db/migrations/0052.sql"),
    include_str!("../../../db/migrations/0053.sql"),
    include_str!("../../../db/migrations/0054.sql"),
    include_str!("../../../db/migrations/0055.sql"),
    include_str!("../../../db/migrations/0056.sql"),
    include_str!("../../../db/migrations/0057.sql"),
    include_str!("../../../db/migrations/0058.sql"),
    include_str!("../../../db/migrations/0059.sql"),
    include_str!("../../../db/migrations/0060.sql"),
    include_str!("../../../db/migrations/0061.sql"),
    include_str!("../../../db/migrations/0062.sql"),
    include_str!("../../../db/migrations/0063.sql"),
    include_str!("../../../db/migrations/0064.sql"),
    include_str!("../../../db/migrations/0065.sql"),
    include_str!("../../../db/migrations/0066.sql"),
    include_str!("../../../db/migrations/0067.sql"),
    include_str!("../../../db/migrations/0068.sql"),
    include_str!("../../../db/migrations/0069.sql"),
    include_str!("../../../db/migrations/0070.sql"),
    include_str!("../../../db/migrations/0071.sql"),
    include_str!("../../../db/migrations/0072.sql"),
    include_str!("../../../db/migrations/0073.sql"),
    include_str!("../../../db/migrations/0074.sql"),
    include_str!("../../../db/migrations/0075.sql"),
    include_str!("../../../db/migrations/0076.sql"),
    include_str!("../../../db/migrations/0077.sql"),
    include_str!("../../../db/migrations/0078.sql"),
];

/// The version [`SCHEMA`] defines, kept in the database's `user_version`.
pub const VERSION: i64 = 1 + MIGRATIONS.len() as i64;

/// Tables whose rows are history themselves, or the engine's bookkeeping: changes to them are not tracked.
const UNTRACKED: [&str; 11] = [
    "rounds",
    "tool_runs",
    "pings",
    "cache_misses",
    "round_calls",
    "round_measures",
    "guard_events",
    "changes",
    "outbox",
    "contents",
    "attachments",
];

/// The line a table's description has when agents may not read it.
const HIDDEN: &str = "Hidden from agents.";

/// The text the owner's choice of another option on a decision card, or disagreement with it, is interjected with; the
/// owner's ruling writes it and the engine's checks read it. A confirmation is a word that needs no answer, worded
/// otherwise.
pub const CARD_RULING: &str = "Ruling on decision card";

/// What a line is given of an interjection: who said it and, for the owner's question about a decision card, which card
/// by its number and key, then the words.
pub fn interjection_words(speaker: &str, text: &str, card: Option<i64>, key: Option<&str>) -> String {
    match (card, key) {
        (Some(c), Some(k)) => format!("From the {speaker}, about decision card {c} ({k}): {text}"),
        (Some(c), None) => format!("From the {speaker}, about decision card {c}: {text}"),
        _ => format!("From the {speaker}: {text}"),
    }
}

/// The text a ruling on a design card of a flow version before decision cards was interjected with; a disagreement
/// said so after it.
pub const DESIGN_CARD_RULING: &str = "Ruling on design card";

/// The text a return was sent back with, as `rounds.rejected` holds it: under `check` for a return that failed the
/// engine's checks, under `format` for one that does not fit its cell's schema. Read by name, not as the first key:
/// `problems` follows it. None for a round that failed, was cut off or ended with an error.
pub fn sent_back_text(rejected: &Value) -> Option<&str> {
    ["check", "format"].iter().find_map(|k| rejected.get(k)?.as_str())
}

/// `rounds.rejected` without `problems`, the same reason in the form for counting by check and by field. A text that
/// quotes what a round was sent back with quotes this, so that it says the reason once.
pub fn without_problems(rejected: &Value) -> Value {
    let mut quoted = rejected.clone();
    if let Some(o) = quoted.as_object_mut() {
        o.shift_remove("problems");
    }
    quoted
}

/// The verdicts of a ruling as `design_cards.rulings` holds it. A ruling on a decision card is one verdict on the card,
/// with decision 0: confirm, choose (another option) or disagree, with the option chosen. A ruling on a design card of a
/// flow version before decision cards held one for each decision in the order given, each with its verdict and any
/// approach chosen instead, or, made before rulings were per decision, one verdict on the card as a whole.
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

/// Whether a ruling confirms its card: a decision card, which is one decision; a design card of before of so many
/// decisions, every decision confirmed, or, for a ruling made before rulings were per decision, the card.
pub fn ruling_confirms(ruling: &serde_json::Value, decisions: usize) -> bool {
    let verdicts = ruling_verdicts(ruling);
    let whole = verdicts.len() == 1 && verdicts[0].0 == 0;
    let all = whole || (1..=decisions as i64).all(|n| verdicts.iter().any(|(d, _, _)| *d == n));
    all && verdicts.iter().all(|(_, v, _)| v == "confirm")
}

/// Whether a ruling chooses another option than the one its card recommends, or disagrees with it: with any decision of
/// a design card of before, or with the card.
pub fn ruling_disagrees(ruling: &serde_json::Value) -> bool {
    ruling_verdicts(ruling)
        .iter()
        .any(|(_, v, _)| v == "disagree" || v == "choose")
}

/// Why a work is paused when the owner paused it without saying why: its Pause on the page, and the round it asked
/// for of a work the engine had paused, which takes the work into the owner's hands.
pub const PAUSED_BY_OWNER: &str = "the owner paused it";

/// What the reason of a work the architect stopped starts with: the engine pauses it as it pauses one for a problem,
/// and the reason says the architect stopped it, then why. Only the architect's resume, or the owner's Continue of
/// that work, lets it go on.
pub const STOPPED_BY_ARCHITECT: &str = "stopped by the architect";

/// Whether a work's pause, by why and who paused it as its row has them, is the architect's stop.
pub fn stopped_by_architect(paused: Option<&str>, by: Option<&str>) -> bool {
    by == Some("engine") && paused.is_some_and(|p| p.starts_with(STOPPED_BY_ARCHITECT))
}

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

/// What is told the rows each committed write touched, given the connection that made it to look them up. One listener
/// may be registered on several connections, as each thread of the engine writes on a connection of its own.
pub type Listener = Arc<dyn Fn(&Connection, &[Changed]) + Send + Sync>;

/// The rows changed on a connection, kept until they can be told, and the rounds it wrote.
#[derive(Default)]
struct Watch {
    /// Whether a listener is told the rows changed: only then are they kept.
    listening: bool,
    /// Changed by the transaction still open.
    pending: Vec<Changed>,
    /// Changed by transactions that committed, not yet told.
    committed: Vec<Changed>,
    /// The rounds the transaction still open changed, by rowid.
    pending_rounds: Vec<i64>,
    /// How many committed transactions on this connection changed a round.
    written: u64,
    /// Each round a committed transaction on this connection changed, by rowid, with the number of the last such
    /// transaction (`written` as it counted it).
    rounds: BTreeMap<i64, u64>,
}

/// Where a connection stood among the rounds it wrote: [`Db::rounds_since`] gives those it has changed since.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoundsMark(u64);

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

    fn set_up(mut conn: Connection) -> Result<Db> {
        conn.busy_timeout(Duration::from_secs(10))?;
        conn.pragma_update(None, "foreign_keys", true)?;
        let writer: Current = Arc::new(Mutex::new(None));
        register(&conn, "wf_tx", &writer, |w| w.tx)?;
        register(&conn, "wf_at", &writer, |w| w.at)?;
        register(&conn, "wf_actor", &writer, |w| w.actor.name())?;
        let watch: Arc<Mutex<Watch>> = Arc::default();
        watch_changes(&mut conn, &watch)?;
        Ok(Db {
            conn,
            writer,
            watch,
            listener: None,
        })
    }

    /// From now on tells `f` the rows each write on this connection touched, every table's, once its transaction
    /// has committed: a reader that looks them up then sees them. Rows a statement outside [`Db::write`] changed are
    /// told after the next write commits.
    pub fn on_commit(&mut self, f: Listener) -> Result<()> {
        lock(&self.watch).listening = true;
        self.listener = Some(f);
        Ok(())
    }

    /// Where this connection stands among the rounds it wrote, for [`Db::rounds_since`].
    pub fn rounds_mark(&self) -> RoundsMark {
        RoundsMark(lock(&self.watch).written)
    }

    /// The rounds this connection started or changed in transactions committed since `mark`, as `(work, n, cell)`, in
    /// the order they started: what another connection wrote meanwhile is not among them, nor what a transaction
    /// rolled back changed. A round is known by its rowid, which `rounds` keeps since its key is not an integer.
    pub fn rounds_since(&self, mark: RoundsMark) -> Result<Vec<(i64, i64, Option<String>)>> {
        let ids: Vec<String> = lock(&self.watch)
            .rounds
            .iter()
            .filter(|(_, n)| **n > mark.0)
            .map(|(id, _)| id.to_string())
            .collect();
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut st = self.conn.prepare(
            "SELECT work, n, cell FROM rounds WHERE rowid IN (SELECT value FROM json_each(?1))
             ORDER BY started_at, work, n",
        )?;
        let rows = st
            .query_map([format!("[{}]", ids.join(","))], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
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
        // A copy of the database as it stands is kept before it is migrated, so that a migration that goes wrong
        // loses nothing.
        backup::before_migration(&self.conn, at)?;
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

/// Keeps, as each transaction on `conn` commits, the rounds it changed, and the rows of every table it changed while a
/// listener is told them; what a transaction rolled back changed is dropped.
fn watch_changes(conn: &mut Connection, watch: &Arc<Mutex<Watch>>) -> Result<()> {
    let w = Arc::clone(watch);
    conn.update_hook(Some(move |_: Action, _: &str, table: &str, rowid: i64| {
        let mut x = lock(&w);
        if table == "rounds" {
            x.pending_rounds.push(rowid);
        }
        if x.listening {
            x.pending.push(Changed {
                table: table.to_string(),
                rowid,
            });
        }
    }))?;
    let w = Arc::clone(watch);
    conn.commit_hook(Some(move || {
        let mut x = lock(&w);
        let done = std::mem::take(&mut x.pending);
        x.committed.extend(done);
        let rounds = std::mem::take(&mut x.pending_rounds);
        if !rounds.is_empty() {
            x.written += 1;
            let n = x.written;
            for id in rounds {
                x.rounds.insert(id, n);
            }
        }
        // Lets the commit go on.
        false
    }))?;
    let w = Arc::clone(watch);
    conn.rollback_hook(Some(move || {
        let mut x = lock(&w);
        x.pending.clear();
        x.pending_rounds.clear();
    }))?;
    Ok(())
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

    // The text a return was sent back with is read by the name of its key, `check` or `format`, whatever else the column
    // holds and in whatever order, and quoted without `problems`; a column of a run that failed, was cut off or ended in
    // an error has no such text, and is quoted as it is.
    #[test]
    fn a_return_sent_back_is_read_by_the_key_of_its_text_and_quoted_without_its_problems() {
        let problems = serde_json::json!([{ "check": "c", "field": "f", "text": "t" }]);
        let check = serde_json::json!({ "check": "c (f): t", "problems": problems });
        let format = serde_json::json!({ "format": "x", "problems": problems });
        let before = serde_json::json!({ "problems": problems, "check": "c (f): t" });
        assert_eq!(sent_back_text(&check), Some("c (f): t"));
        assert_eq!(sent_back_text(&format), Some("x"));
        assert_eq!(sent_back_text(&before), Some("c (f): t"));
        assert_eq!(
            sent_back_text(&serde_json::json!({ "check": "c (f): t" })),
            Some("c (f): t")
        );
        for other in [
            serde_json::json!({ "failed": { "kind": "k", "what": "w" } }),
            serde_json::json!({ "interrupted": "i" }),
            serde_json::json!({ "error": "e" }),
            serde_json::json!({ "problems": problems }),
            serde_json::json!({ "check": 3 }),
            serde_json::json!("text"),
        ] {
            assert_eq!(sent_back_text(&other), None, "{other}");
            assert_eq!(without_problems(&other).get("problems"), None, "{other}");
        }
        // The same text and the same order of keys as a column recorded without problems.
        assert_eq!(
            without_problems(&check).to_string(),
            serde_json::json!({ "check": "c (f): t" }).to_string()
        );
        assert_eq!(
            without_problems(&format).to_string(),
            serde_json::json!({ "format": "x" }).to_string()
        );
        let failed = serde_json::json!({ "failed": { "kind": "k", "what": "w" } });
        assert_eq!(without_problems(&failed), failed);
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
        db.on_commit(Arc::new(move |_: &Connection, rows: &[Changed]| {
            t.lock().unwrap().extend(rows.iter().cloned());
        }))
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

    // A connection tells the rounds it started or changed since a mark, in the order they started: one it started
    // before and ended after is among them; one it wrote before and left alone is not, nor one another connection wrote
    // meanwhile, nor one a write that failed inserted.
    #[test]
    fn a_connection_tells_the_rounds_it_wrote_since_a_mark() {
        let path = std::env::temp_dir().join(format!("wf-agents-db-rounds-{}.db", std::process::id()));
        let files = |p: &Path| ["", "-wal", "-shm"].map(|x| format!("{}{x}", p.display()));
        for f in files(&path) {
            let _ = std::fs::remove_file(f);
        }
        let mut own = Db::create(&path).unwrap();
        let mut other = Db::open(&path).unwrap();
        own.write(Actor::Engine, |tx| {
            tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
            for _ in 0..4 {
                tx.execute(
                    "INSERT INTO works (flow_version, graph, at) VALUES (1, 'engineer', 'R1')",
                    [],
                )?;
            }
            Ok(())
        })
        .unwrap();
        let start = |db: &mut Db, work: i64, at: i64| {
            db.write(Actor::Engine, |tx| {
                tx.execute(
                    "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES (?1, 1, 'R1', '{}', 'c', ?2)",
                    params![work, at],
                )?;
                Ok(())
            })
        };
        start(&mut own, 1, 10).unwrap();
        start(&mut own, 2, 15).unwrap();
        let mark = own.rounds_mark();
        let others = other.rounds_mark();
        assert!(own.rounds_since(mark).unwrap().is_empty());
        start(&mut other, 3, 20).unwrap();
        start(&mut own, 4, 30).unwrap();
        own.write(Actor::Engine, |tx| {
            tx.execute("UPDATE rounds SET ended_at = 40 WHERE work = 1 AND n = 1", [])?;
            Ok(())
        })
        .unwrap();
        let failed = own.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES (4, 2, 'R2', '{}', 'c', 50)",
                [],
            )?;
            Err::<(), _>(Error::Abort("given up".into()))
        });
        assert!(failed.is_err());
        let r1 = Some("R1".to_string());
        assert_eq!(
            own.rounds_since(mark).unwrap(),
            [(1, 1, r1.clone()), (4, 1, r1.clone())]
        );
        assert_eq!(other.rounds_since(others).unwrap(), [(3, 1, r1)]);
        drop((own, other));
        for f in files(&path) {
            let _ = std::fs::remove_file(f);
        }
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

    /// Three works of project `p`, each of an item of its own, in the order of their numbers, all at `T6`; the
    /// database they are in has the one flow version they run on.
    fn three_works() -> (Db, [i64; 3]) {
        let mut db = Db::in_memory().unwrap();
        let works = db
            .write(Actor::Engine, |tx| {
                tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
                let mut works = [0; 3];
                for w in &mut works {
                    let item = item(tx, "an item")?;
                    tx.execute(
                        "INSERT INTO works (item, flow_version, graph, at) VALUES (?1, 1, 'engineer', 'T6')",
                        [item],
                    )?;
                    *w = tx.last_insert_rowid();
                }
                Ok(works)
            })
            .unwrap();
        (db, works)
    }

    /// The work goes to the cell, in a write of its own.
    fn goes_to(db: &mut Db, work: i64, cell: &str) {
        db.write(Actor::Engine, |tx| {
            tx.execute("UPDATE works SET at = ?2 WHERE id = ?1", params![work, cell])?;
            Ok(())
        })
        .unwrap();
    }

    /// The works in project `p`'s merge queue, in its order.
    fn merge_queue(db: &Db) -> Vec<i64> {
        let mut st = db
            .conn()
            .prepare("SELECT work FROM merge_queue WHERE project = 'p'")
            .unwrap();
        st.query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect()
    }

    // The merge queue is in the order the works came to the merge cell, whatever their numbers: the last begun that
    // came first is first; a work that left the cell to be fixed, or to ask the architect, and came back is last.
    // The expected orders here are the orders of the writes below, not what the view says.
    #[test]
    fn the_merge_queue_is_in_the_order_the_works_came_to_the_cell() {
        let (mut db, [a, b, c]) = three_works();
        goes_to(&mut db, c, "M2");
        goes_to(&mut db, a, "M2");
        goes_to(&mut db, b, "M2");
        assert_eq!(merge_queue(&db), [c, a, b]);
        // The head leaves to be fixed and comes back after the fix: behind the others, though its number is largest.
        goes_to(&mut db, c, "T6");
        assert_eq!(merge_queue(&db), [a, b]);
        goes_to(&mut db, c, "M1");
        goes_to(&mut db, c, "M2");
        assert_eq!(merge_queue(&db), [a, b, c]);
        // The head leaves to ask the architect and comes back: last again, though its number is the smallest.
        goes_to(&mut db, a, "A2");
        goes_to(&mut db, a, "M2");
        assert_eq!(merge_queue(&db), [b, c, a]);
        // One that ended is no longer in the queue.
        db.write(Actor::Engine, |tx| {
            tx.execute("UPDATE works SET ended = 'cancelled' WHERE id = ?1", [b])?;
            Ok(())
        })
        .unwrap();
        assert_eq!(merge_queue(&db), [c, a]);
    }

    // A change to a work in the merge queue that leaves it at the merge cell (what it waits for, the values it
    // recorded, its pull request, the head it pushed) does not move it: the head of the queue stays the head, as a
    // work that waits its turn goes round the cell again and again.
    #[test]
    fn a_change_that_leaves_a_work_at_the_merge_cell_keeps_its_place_in_the_queue() {
        let (mut db, [a, b, _]) = three_works();
        goes_to(&mut db, b, "M2");
        goes_to(&mut db, a, "M2");
        assert_eq!(merge_queue(&db), [b, a]);
        for change in [
            "waiting = '[\"M2\"]'",
            "recorded = '{\"turn\":\"first\"}'",
            "pr = 7",
            "pushed_head = 'abc'",
            "at = 'M2'",
        ] {
            db.write(Actor::Engine, |tx| {
                tx.execute(&format!("UPDATE works SET {change} WHERE id = ?1"), [b])?;
                Ok(())
            })
            .unwrap();
            assert_eq!(merge_queue(&db), [b, a], "{change}");
        }
    }

    // The order of the merge queue finds when each work came to the cell by the index of the changes of its row,
    // scanned from the newest back with no sort of them, so that it costs the few changes since the work came to the
    // cell and not all it has had: the plan of that subquery uses the index and has no temporary sort.
    #[test]
    fn the_merge_queue_finds_when_a_work_came_to_the_cell_by_the_index_of_its_changes() {
        let db = Db::in_memory().unwrap();
        let mut st = db
            .conn()
            .prepare("EXPLAIN QUERY PLAN SELECT work FROM merge_queue WHERE project = 'p'")
            .unwrap();
        // Each step of the plan: its number, the number of the step it is part of, and what it does.
        let plan: Vec<(i64, i64, String)> = st
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(3)?)))
            .unwrap()
            .map(|x| x.unwrap())
            .collect();
        let sub = plan
            .iter()
            .find(|(_, _, detail)| detail.contains("SUBQUERY"))
            .unwrap_or_else(|| panic!("no subquery in the plan: {plan:#?}"));
        let mut inside = BTreeSet::from([sub.0]);
        loop {
            let before = inside.len();
            for (id, parent, _) in &plan {
                if inside.contains(parent) {
                    inside.insert(*id);
                }
            }
            if inside.len() == before {
                break;
            }
        }
        let steps: Vec<&str> = plan
            .iter()
            .filter(|(id, _, _)| *id != sub.0 && inside.contains(id))
            .map(|(_, _, detail)| detail.as_str())
            .collect();
        assert!(
            steps.iter().any(|d| d.contains("USING INDEX changes_by_row")),
            "{plan:#?}"
        );
        assert!(!steps.iter().any(|d| d.contains("TEMP B-TREE")), "{plan:#?}");
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

    // Item #217: the comment of an observation's title is part of the table's text, which an agent reads with
    // `SELECT sql FROM sqlite_master`, and the shape comparison of a migrated and a new database does not look at it:
    // a database from version 73 comes out with the comment of the new schema and its rows, and the index.
    #[test]
    fn observations_from_version_73_keep_their_rows_and_get_the_comment_of_the_new_schema() {
        let mut old = at_version(73);
        let sql = |db: &Db| -> String {
            db.conn()
                .query_row("SELECT sql FROM sqlite_master WHERE name = 'observations'", [], |r| {
                    r.get(0)
                })
                .unwrap()
        };
        assert!(sql(&old).contains("in English of at most 20 characters"));
        old.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO observations (id, project, title, what, source) VALUES (7, 'p', 'Old title', 'w', 's')",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        old.migrate(MIGRATIONS).unwrap();
        assert!(sql(&old).contains("in the owner's language, at most `items.title_max` wide"));
        assert!(!sql(&old).contains("in English of at most 20 characters"));
        let row: (i64, String) = old
            .conn()
            .query_row("SELECT id, title FROM observations", [], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap();
        assert_eq!(row, (7, "Old title".to_string()));
        let index: i64 = old
            .conn()
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE name = 'observations_by_project'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(index, 1);
        assert_eq!(sql(&old), sql(&Db::in_memory().unwrap()));
    }

    // Item #239: taken from version 77 to 78, the holds on new starts keep their rows as they were (time and words
    // included), the table is the new one letter for letter and takes the reason owner, and no change of its rows is
    // recorded by the migration; the triggers that record changes are made afresh, so that a hold written after it is.
    #[test]
    fn holds_from_version_77_keep_their_rows_and_take_the_owners_reason() {
        let mut old = at_version(77);
        let rejected = old.write(Actor::Owner, |tx| {
            tx.execute(
                "INSERT INTO dispatch_holds (project, reason, since, why) VALUES ('p', 'owner', 5, 'w')",
                [],
            )?;
            Ok(())
        });
        assert!(rejected.is_err(), "version 77 does not know the owner's hold");
        old.write(Actor::Architect, |tx| {
            tx.execute(
                "INSERT INTO dispatch_holds (project, reason, since, why) VALUES
                   ('p', 'architect', 1000, 'Wait for the review'), ('p', 'set_straight', 2000, NULL),
                   ('q', 'architect', 3000, NULL)",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        let changes = |db: &Db| -> i64 {
            db.conn()
                .query_row(
                    "SELECT count(*) FROM changes WHERE table_name = 'dispatch_holds'",
                    [],
                    |r| r.get(0),
                )
                .unwrap()
        };
        assert_eq!(changes(&old), 3);
        old.migrate(MIGRATIONS).unwrap();
        let held: Vec<(String, String, i64, Option<String>)> = {
            let mut st = old
                .conn()
                .prepare("SELECT project, reason, since, why FROM dispatch_holds ORDER BY project, reason")
                .unwrap();
            st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
                .unwrap()
                .map(|x| x.unwrap())
                .collect()
        };
        let row = |p: &str, r: &str, since: i64, why: Option<&str>| {
            (p.to_string(), r.to_string(), since, why.map(str::to_string))
        };
        assert_eq!(
            held,
            [
                row("p", "architect", 1000, Some("Wait for the review")),
                row("p", "set_straight", 2000, None),
                row("q", "architect", 3000, None),
            ]
        );
        assert_eq!(changes(&old), 3, "the migration records no change of its own");
        let sql = |db: &Db| -> String {
            db.conn()
                .query_row("SELECT sql FROM sqlite_master WHERE name = 'dispatch_holds'", [], |r| {
                    r.get(0)
                })
                .unwrap()
        };
        let new = Db::in_memory().unwrap();
        assert_eq!(sql(&old), sql(&new));
        assert_eq!(shape(old.conn()).unwrap(), shape(new.conn()).unwrap());
        old.write(Actor::Owner, |tx| {
            hold_starts(tx, "p", HoldReason::Owner, Some("Leaving"))
        })
        .unwrap();
        assert_eq!(changes(&old), 4);
        let actor: String = old
            .conn()
            .query_row(
                "SELECT actor FROM changes WHERE table_name = 'dispatch_holds' ORDER BY id DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(actor, "owner");
        let other = old.write(Actor::Owner, |tx| {
            tx.execute(
                "INSERT INTO dispatch_holds (project, reason, since) VALUES ('p', 'policy', 1)",
                [],
            )?;
            Ok(())
        });
        assert!(other.is_err(), "a reason that is not one of the three is still refused");
    }

    // A database at version 50 that holds what the migrations after it change (an open setting of a policy with its
    // study waiting for the owner, the hold on new starts it kept, and a talk held with words of the owner's it did not
    // take yet) migrates to now, with no change outside a write, and comes out as those migrations say; a database
    // that holds none of it, as a new one, never fires the triggers that record changes, and so cannot tell.
    #[test]
    fn a_database_with_rows_from_version_50_migrates_to_now() {
        let mut old = at_version(50);
        old.write(Actor::Engine, |tx| {
            tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
            tx.execute(
                "INSERT INTO works (flow_version, graph, at, project) VALUES (1, 'architect', 'B1', 'p')",
                [],
            )?;
            tx.execute(
                "INSERT INTO rounds (work, n, cell, input, config_commit, started_at, ended_at, transition)
                 VALUES (1, 1, 'P1', '{}', 'c', 1, 2, 'set_again'), (1, 2, 'P3', '{}', 'c', 3, 4, 'held')",
                [],
            )?;
            tx.execute(
                "INSERT INTO owner_asks (work, round, body) VALUES (1, 2, '{\"from\":\"architect\",\"kind\":\"study\"}')",
                [],
            )?;
            tx.execute(
                "INSERT INTO architect_matters (project, kind, what, body, work, round, at, asked, resume_cell)
                 VALUES ('p', 'set_policy', 'Set the policy', '{}', 1, 1, 1, 1, 'P3')",
                [],
            )?;
            tx.execute(
                "INSERT INTO dispatch_holds (project, reason, since) VALUES ('p', 'policy', 1)",
                [],
            )?;
            tx.execute(
                "INSERT INTO architect_matters (project, kind, detail, what, body, at, talk_held)
                 VALUES ('p', 'owner_words', 'said', 'The owner says: hi', '{\"text\":\"hi\",\"where\":\"page\"}', 5, 6)",
                [],
            )?;
            tx.execute("INSERT INTO talk_entries (matter, text, at) VALUES (2, 'And more.', 7)", [])?;
            Ok(())
        })
        .unwrap();
        old.migrate(MIGRATIONS).unwrap();
        let new = Db::in_memory().unwrap();
        assert_eq!(shape(old.conn()).unwrap(), shape(new.conn()).unwrap());
        let one = |q: &str| -> String { old.conn().query_row(q, [], |r| r.get(0)).unwrap() };
        assert!(one("SELECT withdrawn FROM architect_matters WHERE id = 1").contains("no longer runs so"));
        assert_eq!(
            one("SELECT json_extract(answer, '$.by') FROM owner_asks WHERE id = 1"),
            "engine"
        );
        assert_eq!(one("SELECT count(*) || '' FROM dispatch_holds"), "0");
        assert_eq!(
            one("SELECT json_extract(body, '$.text') FROM architect_matters WHERE id = 3"),
            "And more."
        );
        // The triggers are made afresh: a change outside a write is refused again.
        assert!(
            old.conn()
                .execute("UPDATE architect_matters SET what = 'x' WHERE id = 3", [])
                .is_err()
        );
    }

    // Taken to version 69, the head `Owner's decisions:` of an item's approach is `Decisions:`, wherever the head
    // stands (first line, middle or last), and every other byte of the text stays: a line that holds the words but is
    // not the head, an indented one and an approach with no head are as they were, and an item with no approach has
    // none; the view of what the owner said that concerns a work reads the words and who was declared to speak them.
    #[test]
    fn the_head_of_the_owner_s_decisions_is_renamed_and_nothing_else_changes() {
        let mut old = at_version(68);
        let texts: [Option<&str>; 6] = [
            Some("Approach:\n- Do it.\nOwner's decisions:\nDecision (2026-11-02, owner): A.\n\nLearned:\nCommits: abc"),
            Some("Owner's decisions:\nDecision (2026-11-02, owner): B."),
            Some("Approach:\n- Do it.\nOwner's decisions:"),
            Some("Approach:\nThe Owner's decisions: stay.\n  Owner's decisions:\nowner's decisions:\n"),
            Some("Approach:\n- Do it.\nOwner's decisions:\n\n\nLearned:\n"),
            None,
        ];
        old.write(Actor::Engine, |tx| {
            for t in texts {
                tx.execute(
                    "INSERT INTO items (project, title, problem, why, approach, deliver, level)
                     VALUES ('p', 't', 'x', 'y', ?1, 'change', '1')",
                    [t],
                )?;
            }
            Ok(())
        })
        .unwrap();
        // An interjection, and two open questions of which one was answered by a later change.
        old.write(Actor::Engine, |tx| {
            tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
            tx.execute(
                "INSERT INTO works (flow_version, graph, at) VALUES (1, 'engineer', 'A3')",
                [],
            )?;
            tx.execute(
                "INSERT INTO interjections (work, speaker, text) VALUES (1, 'owner', 'Said.')",
                [],
            )?;
            tx.execute(
                "INSERT INTO architect_matters (project, kind, what, body, at) VALUES
                 ('p', 'open_question', 'Which?', '{\"question\":\"Which?\"}', 5),
                 ('p', 'open_question', 'Which?', '{\"question\":\"Which?\"}', 5)",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        old.write(Actor::Engine, |tx| {
            tx.execute(
                "UPDATE architect_matters SET body = json_set(body, '$.answer', 'One.') WHERE id = 1",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        old.migrate(MIGRATIONS).unwrap();
        // The interjection stays, with no declared speaker; the answered question has the time of the change that wrote
        // its answer, not the time it was raised (5); the one with no answer has none.
        let answered = |matter: i64| -> Option<i64> {
            old.conn()
                .query_row(
                    "SELECT json_extract(body, '$.answered_at') FROM architect_matters WHERE id = ?1",
                    [matter],
                    |r| r.get(0),
                )
                .unwrap()
        };
        let answer_change: i64 = old
            .conn()
            .query_row(
                "SELECT max(at) FROM changes WHERE table_name = 'architect_matters'
                   AND json_extract(row_key, '$.id') = 1 AND json_extract(new_row, '$.body.answer') = 'One.'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(answer_change > 5);
        assert_eq!(answered(1), Some(answer_change));
        assert_eq!(answered(2), None);
        let said: (String, Option<String>) = old
            .conn()
            .query_row("SELECT text, declared FROM interjections", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(said, ("Said.".to_string(), None));
        let approaches: Vec<Option<String>> = old
            .conn()
            .prepare("SELECT approach FROM items ORDER BY id")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        let renamed = |t: &str| t.replace("Owner's decisions:\n", "Decisions:\n");
        assert_eq!(
            approaches,
            [
                Some(renamed(texts[0].unwrap())),
                Some(renamed(texts[1].unwrap())),
                Some("Approach:\n- Do it.\nDecisions:".to_string()),
                Some(texts[3].unwrap().to_string()),
                Some(renamed(texts[4].unwrap())),
                None,
            ]
        );
        assert_eq!(
            shape(old.conn()).unwrap(),
            shape(Db::in_memory().unwrap().conn()).unwrap()
        );
    }

    // The view of what the owner said that concerns a work gives the words said in the work's group, the owner's answers
    // to its engineer's questions, those said to its engineer alone and the answers to its asks, each with who was
    // declared to speak it; the words of a matter that is no work's, an interjection a matter holds and the
    // architect's own are left out.
    #[test]
    fn what_the_owner_said_that_concerns_a_work_is_read_from_its_records() {
        let mut db = Db::in_memory().unwrap();
        db.write(Actor::Engine, |tx| {
            tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
            tx.execute("INSERT INTO works (flow_version, graph, at) VALUES (1, 'engineer', 'A3')", [])?;
            tx.execute(
                "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES (1, 1, 'A3', '{}', 'c', 1)",
                [],
            )?;
            for (body, at) in [
                (
                    r#"{"text":"In the group.","where":"work","work":1,"speaker":"delegate","relayed":["In the"]}"#,
                    5,
                ),
                (r#"{"text":"On the page.","where":"page","speaker":"owner"}"#, 6),
                (r#"{"text":"Kept before speakers.","where":"work","work":1}"#, 7),
            ] {
                tx.execute(
                    "INSERT INTO architect_matters (project, kind, what, body, at) VALUES ('p', 'owner_words', 'w', ?1, ?2)",
                    params![body, at],
                )?;
            }
            tx.execute(
                "INSERT INTO interjections (work, speaker, text, matter) VALUES (1, 'owner', 'Held by a matter.', 1)",
                [],
            )?;
            tx.execute(
                "INSERT INTO interjections (work, speaker, declared, relayed, text)
                 VALUES (1, 'owner', 'delegate', '[\"To the\"]', 'To the engineer.')",
                [],
            )?;
            // The owner's answer to an engineer's question is the reply of its matter; the architect's reply is not.
            tx.execute(
                "INSERT INTO architect_matters (project, kind, what, body, work, round, at, reply) VALUES
                 ('p', 'engineer_question', 'w', '{}', 1, 1, 8,
                  '{\"by\":\"owner\",\"at\":9,\"text\":\"Split it.\",\"speaker\":\"delegate\",\"relayed\":[\"Split\"]}'),
                 ('p', 'engineer_question', 'w', '{}', 1, 1, 8, '{\"by\":\"architect\",\"at\":9,\"text\":\"No.\"}')",
                [],
            )?;
            tx.execute("INSERT INTO interjections (work, speaker, text) VALUES (1, 'architect', 'Not the owner.')", [])?;
            tx.execute(
                "INSERT INTO owner_asks (work, round, body, answer) VALUES
                 (1, 1, '{}', '{\"by\":\"owner\",\"at\":9,\"text\":\"Yes.\",\"speaker\":\"owner\"}'),
                 (1, 1, '{}', '{\"by\":\"engine\",\"at\":9,\"text\":\"Closed.\"}'),
                 (1, 1, '{}', NULL)",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        type Row = (i64, String, i64, Option<String>, String, Option<String>);
        let rows: Vec<Row> = db
            .conn()
            .prepare("SELECT work, source, id, speaker, words, relayed FROM work_owner_words ORDER BY source, id")
            .unwrap()
            .query_map([], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))
            })
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        // The parts a delegate declared to be the owner's own come with the row, a JSON array; none when it declared none.
        let said = |source: &str, id: i64, speaker: Option<&str>, words: &str, relayed: Option<&str>| {
            (
                1,
                source.to_string(),
                id,
                speaker.map(str::to_string),
                words.to_string(),
                relayed.map(str::to_string),
            )
        };
        assert_eq!(
            rows,
            [
                said("ask", 1, Some("owner"), "Yes.", None),
                said(
                    "interjection",
                    2,
                    Some("delegate"),
                    "To the engineer.",
                    Some(r#"["To the"]"#),
                ),
                said("matter", 1, Some("delegate"), "In the group.", Some(r#"["In the"]"#)),
                said("matter", 3, None, "Kept before speakers.", None),
                said("matter", 4, Some("delegate"), "Split it.", Some(r#"["Split"]"#)),
            ]
        );
    }

    // Taken to version 67, a database keeps its sessions as they were, with the rows that point to them, takes a session
    // that started because the cache expired and none that started for a reason made up, has the view of the
    // conversations shown, and comes out as a new one.
    #[test]
    fn a_database_taken_to_version_67_keeps_its_sessions_and_takes_the_expired_reason() {
        let mut old = at_version(66);
        old.write(Actor::Engine, |tx| {
            tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
            tx.execute(
                "INSERT INTO works (flow_version, graph, at, project) VALUES (1, 'architect', 'B1', 'p')",
                [],
            )?;
            tx.execute(
                "INSERT INTO sessions (work, line, cli, cli_session, started)
                 VALUES (1, 'architect', 'claude', 's1', 'account')",
                [],
            )?;
            tx.execute(
                "INSERT INTO rounds (work, n, cell, session, input, config_commit, started_at)
                 VALUES (1, 1, 'U1', 1, '{}', 'c', 1)",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        old.migrate(MIGRATIONS).unwrap();
        let one = |q: &str| -> String { old.conn().query_row(q, [], |r| r.get(0)).unwrap() };
        assert_eq!(
            one("SELECT id || ' ' || line || ' ' || cli_session || ' ' || started FROM sessions"),
            "1 architect s1 account"
        );
        assert_eq!(one("SELECT session || '' FROM rounds WHERE work = 1 AND n = 1"), "1");
        assert_eq!(
            one("SELECT count(*) || '' FROM sqlite_schema WHERE type = 'view' AND name = 'conversation_shown'"),
            "1"
        );
        let insert = |db: &mut Db, started: &str| {
            db.write(Actor::Engine, |tx| {
                tx.execute(
                    "INSERT INTO sessions (work, line, cli, cli_session, ended, started)
                     VALUES (1, 'architect', 'claude', 's2', 5, ?1)",
                    [started],
                )?;
                Ok(())
            })
        };
        assert!(insert(&mut old, "expired").is_ok());
        assert!(insert(&mut old, "made up").is_err());
        // The triggers that record changes are made afresh: the new session's insertion was recorded.
        let recorded = old
            .conn()
            .query_row(
                "SELECT count(*) FROM changes WHERE table_name = 'sessions' AND json_extract(new_row, '$.started') = 'expired'",
                [],
                |r| r.get::<_, i64>(0),
            )
            .unwrap();
        assert_eq!(recorded, 1);
        let new = Db::in_memory().unwrap();
        assert_eq!(shape(old.conn()).unwrap(), shape(new.conn()).unwrap());
    }

    // Taken to version 72, a database keeps each matter of the architects as it was, none of them in a group, with the
    // views over them, takes a matter into a group and refuses a group led by a matter that is not there; the table made
    // afresh is tracked again, and comes out as a new one.
    #[test]
    fn a_database_taken_to_version_72_keeps_its_matters_and_takes_groups() {
        let mut old = at_version(71);
        old.write(Actor::Engine, |tx| {
            for (what, goes_on_at) in [("first", None), ("second", Some("H5"))] {
                tx.execute(
                    "INSERT INTO architect_matters (project, kind, detail, what, body, at, goes_on_at)
                     VALUES ('p', 'report', 'other', ?1, '{}', 7, ?2)",
                    params![what, goes_on_at],
                )?;
            }
            Ok(())
        })
        .unwrap();
        old.migrate(MIGRATIONS).unwrap();
        let one = |db: &Db, q: &str| -> String { db.conn().query_row(q, [], |r| r.get(0)).unwrap() };
        assert_eq!(
            one(
                &old,
                "SELECT group_concat(id || what || coalesce(goes_on_at, '-') || coalesce(group_lead, '-'), ' ')
                 FROM architect_matters"
            ),
            "1first-- 2secondH5-"
        );
        // The view over the matters still reads them.
        assert_eq!(one(&old, "SELECT count(*) || '' FROM architect_queue"), "2");
        // A matter is put in the group the other leads, the change recorded by the triggers made afresh; a lead that is no
        // matter is refused.
        old.write(Actor::Engine, |tx| {
            tx.execute("UPDATE architect_matters SET group_lead = 1 WHERE id = 2", [])?;
            Ok(())
        })
        .unwrap();
        assert_eq!(
            one(
                &old,
                "SELECT count(*) || '' FROM changes
                 WHERE table_name = 'architect_matters' AND json_extract(new_row, '$.group_lead') = 1"
            ),
            "1"
        );
        let refused = old.write(Actor::Engine, |tx| {
            tx.execute("UPDATE architect_matters SET group_lead = 99 WHERE id = 1", [])?;
            Ok(())
        });
        assert!(refused.is_err());
        let new = Db::in_memory().unwrap();
        assert_eq!(shape(old.conn()).unwrap(), shape(new.conn()).unwrap());
    }

    // Taken to version 61, a database keeps each observation as it was, and one recorded before observations had titles
    // has none, rather than one made up for it; the table made afresh is tracked again.
    #[test]
    fn an_observation_recorded_before_titles_is_kept_with_no_title() {
        let mut old = at_version(60);
        old.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO observations (project, what, tags, source, times_reported)
                 VALUES ('p', 'Parsing is slow.', '[\"perf\"]', 'A report.', 2)",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        old.migrate(MIGRATIONS).unwrap();
        let new = Db::in_memory().unwrap();
        assert_eq!(shape(old.conn()).unwrap(), shape(new.conn()).unwrap());
        let row: (Option<String>, String, String, i64, bool) = old
            .conn()
            .query_row(
                "SELECT title, what, tags, times_reported, created IS NOT NULL FROM observations_all WHERE id = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .unwrap();
        assert_eq!(
            row,
            (None, "Parsing is slow.".to_string(), "[\"perf\"]".to_string(), 2, true)
        );
        assert!(
            old.conn()
                .execute("UPDATE observations SET title = 'Slow parse' WHERE id = 1", [])
                .is_err()
        );
    }

    // Taken to version 63, a database keeps each design card of before as it was, readable, keyed by its number, with
    // no records it revises and no flow; its rulings and what replaced it stay.
    #[test]
    fn a_design_card_of_before_keeps_its_body_keyed_by_its_number() {
        let mut old = at_version(62);
        old.write(Actor::Engine, |tx| {
            item(tx, "t")?;
            tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
            tx.execute(
                "INSERT INTO works (item, flow_version, graph, at) VALUES (1, 1, 'engineer', 'M1')",
                [],
            )?;
            tx.execute(
                "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES (1, 1, 'D4', '{}', 'c', 1)",
                [],
            )?;
            tx.execute(
                "INSERT INTO design_cards (work, round, body, status, rulings) VALUES (1, 1, '[{\"node\":\"a\"}]', 'replaced',
                   '[{\"by\":\"owner\",\"verdict\":\"confirm\"}]')",
                [],
            )?;
            tx.execute(
                "INSERT INTO design_cards (work, round, body, status, changed) VALUES (1, 1, '[]', 'open', 'Kept one.')",
                [],
            )?;
            tx.execute("UPDATE design_cards SET replaced_by = 2 WHERE id = 1", [])?;
            Ok(())
        })
        .unwrap();
        old.migrate(MIGRATIONS).unwrap();
        let mut st = old
            .conn()
            .prepare("SELECT id, key, body, status, rulings, replaced_by, changed, revision, flow_snapshot FROM design_cards")
            .unwrap();
        type Row = (
            i64,
            String,
            String,
            String,
            String,
            Option<i64>,
            Option<String>,
            String,
            Option<i64>,
        );
        let rows: Vec<Row> = st
            .query_map([], |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                    r.get(7)?,
                    r.get(8)?,
                ))
            })
            .unwrap()
            .map(|x| x.unwrap())
            .collect();
        assert_eq!(rows[0].1, "card-1");
        assert_eq!(rows[0].2, "[{\"node\":\"a\"}]");
        assert_eq!((rows[0].3.as_str(), rows[0].5), ("replaced", Some(2)));
        assert!(rows[0].4.contains("confirm"));
        assert_eq!(
            (rows[1].1.as_str(), rows[1].6.as_deref()),
            ("card-2", Some("Kept one."))
        );
        assert_eq!((rows[1].7.as_str(), rows[1].8), ("[]", None));
    }

    // Taken to version 56, a database withdraws each look of the architect's at a design card the owner ruled or another
    // replaced that is still in its queue, saying why; taken on to version 63, the look at a card that still waits too,
    // the design's coherence review judging each card since.
    #[test]
    fn a_look_at_a_design_card_ruled_or_replaced_is_withdrawn() {
        let mut old = at_version(55);
        old.write(Actor::Engine, |tx| {
            item(tx, "t")?;
            tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
            tx.execute(
                "INSERT INTO works (item, flow_version, graph, at) VALUES (1, 1, 'engineer', 'I1')",
                [],
            )?;
            tx.execute(
                "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES (1, 1, 'I1', '{}', 'c', 1)",
                [],
            )?;
            let cards = [("ruled", None), ("open", None), ("replaced", Some(2)), ("replaced", None)];
            for (status, by) in cards {
                tx.execute(
                    "INSERT INTO design_cards (work, round, body, status, replaced_by) VALUES (1, 1, '[]', ?1, ?2)",
                    params![status, by],
                )?;
            }
            for card in 1..=4 {
                tx.execute(
                    "INSERT INTO architect_matters (project, kind, what, body, item, at)
                     VALUES ('p', 'design_card', 'Design card to look at', json_object('design_card', ?1), 1, 1)",
                    [card],
                )?;
            }
            Ok(())
        })
        .unwrap();
        old.migrate(MIGRATIONS).unwrap();
        let mut st = old
            .conn()
            .prepare("SELECT withdrawn FROM architect_matters ORDER BY id")
            .unwrap();
        let withdrawn: Vec<Option<String>> = st.query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect();
        assert_eq!(
            withdrawn,
            [
                Some("the owner ruled design card 1".to_string()),
                Some(
                    "the architect no longer looks at decision cards: the design's coherence review judges each before \
                     it is posted"
                        .to_string()
                ),
                Some("design card 3 was replaced by design card 2".to_string()),
                Some("design card 4 was replaced".to_string()),
            ]
        );
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
        for table in [
            "items",
            "architect_matters",
            "dispatch_queue",
            "interjections",
            "rounds",
        ] {
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

    // A database opened older than this wf-agents is copied as it stood before it is migrated: the copy, in `backups/`
    // beside it, holds the version and the rows it had; opened again at its version, it is not copied again.
    #[test]
    fn a_database_is_copied_before_it_is_migrated() {
        let home = std::env::temp_dir().join(format!("wf-agents-db-{}-before-migration", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        let path = home.join("wf-agents.sqlite");
        let mut old = at_version(VERSION - 1);
        old.write(Actor::Owner, |tx| item(tx, "kept")).unwrap();
        old.conn.execute("VACUUM INTO ?1", [path.to_string_lossy()]).unwrap();
        let db = Db::open(&path).unwrap();
        assert_eq!(version_of(db.conn()).unwrap(), VERSION);
        let dir = backup::dir(&path);
        let copies = backup::copies(&dir, backup::BEFORE_MIGRATION);
        assert_eq!(copies.len(), 1);
        assert!(
            copies[0]
                .to_string_lossy()
                .ends_with(&format!("-v{}.sqlite", VERSION - 1))
        );
        let copy = Connection::open(&copies[0]).unwrap();
        assert_eq!(version_of(&copy).unwrap(), VERSION - 1);
        assert_eq!(names(&copy, "SELECT title FROM items").unwrap(), ["kept"]);
        drop(db);
        Db::open(&path).unwrap();
        assert_eq!(backup::copies(&dir, backup::BEFORE_MIGRATION).len(), 1);
        std::fs::remove_dir_all(&home).unwrap();
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

    // Rounds from before the ledger enter what the run guard's lines are computed from with what their records tell,
    // marked so: the first prompt and the last, the turns, the jump from the session's round before and what was given,
    // across a session its row carried on and over a run that made no call. Nothing is kept of a step no agent ran.
    #[test]
    fn rounds_from_before_the_ledger_are_measured_from_their_records() {
        let usage = |first: i64, last: i64| {
            let prompt = |p: i64| serde_json::json!({ "input_tokens": 2, "cache_read_input_tokens": p - 2 });
            serde_json::json!({
                "first_call": prompt(first), "usage": { "iterations": [prompt(1), prompt(last)] },
                "turns": 3, "cost_usd": 1.5,
            })
            .to_string()
        };
        let mut old = at_version(65);
        old.write(Actor::Engine, |tx| {
            tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
            tx.execute("INSERT INTO works (flow_version, graph, at) VALUES (1, 'engineer', 'I1')", [])?;
            tx.execute(
                "INSERT INTO works (flow_version, graph, at, project) VALUES (1, 'architect', 'B1', 'p')",
                [],
            )?;
            tx.execute(
                "INSERT INTO sessions (id, work, line, cli, cli_session, ended) VALUES
                 (1, 1, 'engineer', 'c', 's', NULL), (2, 2, 'architect', 'c', 'carried', 1),
                 (3, 2, 'architect', 'c', 'carried', NULL)",
                [],
            )?;
            tx.execute("INSERT INTO contents (hash, text) VALUES ('h', ?1)", ["x".repeat(3000)])?;
            let round = |work: i64, n: i64, session: Option<i64>, input: &str, output: Option<&str>, usage: Option<&str>| {
                tx.execute(
                    "INSERT INTO rounds (work, n, cell, session, input, output, model, config_commit, usage, compactions,
                                         started_at)
                     VALUES (?1, ?2, 'I1', ?3, ?4, ?5, 'm', 'c', ?6, 1, ?2)",
                    params![work, n, session, input, output, usage],
                )
            };
            round(1, 1, None, "{}", None, None)?;
            round(1, 2, Some(1), r#"{"message":"abcd"}"#, Some(r#"{"a":1}"#), Some(&usage(100, 300)))?;
            round(1, 3, Some(1), r#"{"message":"failed"}"#, None, Some(r#"{"started":true}"#))?;
            let parts = r#"{"message":{"parts":["ab",{"content":"h"}]}}"#;
            round(1, 4, Some(1), parts, None, Some(&usage(1000, 1200)))?;
            round(2, 1, Some(2), r#"{"message":"ab"}"#, None, Some(&usage(100, 500)))?;
            round(2, 2, Some(3), r#"{"message":"ab"}"#, None, Some(&usage(450, 450)))?;
            Ok(())
        })
        .unwrap();
        old.migrate(MIGRATIONS).unwrap();
        let mut st = old
            .conn()
            .prepare(
                "SELECT work, round, graph, cell, calls, added, jump, given, repeats, compactions, cost_usd, ledger
                 FROM round_measures ORDER BY work, round",
            )
            .unwrap();
        type Row = (
            i64,
            i64,
            String,
            String,
            i64,
            i64,
            Option<i64>,
            Option<i64>,
            Option<i64>,
            i64,
            f64,
            i64,
        );
        let rows: Vec<Row> = st
            .query_map([], |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                    r.get(7)?,
                    r.get(8)?,
                    r.get(9)?,
                    r.get(10)?,
                    r.get(11)?,
                ))
            })
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        let engineer = |n, added, jump, given| {
            (
                1,
                n,
                "engineer".to_string(),
                "I1".to_string(),
                3,
                added,
                jump,
                given,
                None,
                1,
                1.5,
                0,
            )
        };
        let architect = |n, added, jump, given| {
            (
                2,
                n,
                "architect".to_string(),
                "I1".to_string(),
                3,
                added,
                jump,
                given,
                None,
                1,
                1.5,
                0,
            )
        };
        assert_eq!(
            rows,
            [
                engineer(2, 200, None, Some(4)),
                engineer(4, 200, Some(700), Some(3009)),
                architect(1, 400, None, Some(2)),
                architect(2, 0, Some(0), Some(2)),
            ]
        );
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
        // A ruling on a decision card is one verdict: a confirmation confirms it, another option chosen disagrees.
        let confirm = serde_json::json!({ "by": "owner", "verdict": "confirm" });
        let choose = serde_json::json!({ "by": "owner", "verdict": "choose", "option": "B" });
        assert!(ruling_confirms(&confirm, 1) && !ruling_disagrees(&confirm));
        assert!(!ruling_confirms(&choose, 1) && ruling_disagrees(&choose));
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

    // Taken to version 20, a database of one project gives its architect's works that project, ends the work still open
    // at a cell of the old graph with its round and answers the asks only that work waited for or that asked to approve
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
            // Each matter's kind, the engineer's work that raised it, how it is held and by which ask, and the
            // architect's work that handled it.
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

    // A session a work of the architect went on in from the project's previous architect work, recorded before version
    // 43 under the name its start reason had then, says carried; the others keep why they started.
    #[test]
    fn a_session_carried_over_from_the_previous_architect_work_says_so() {
        let mut old = at_version(42);
        old.write(Actor::Engine, |tx| {
            tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
            for at in ["1", "2"] {
                tx.execute(
                    "INSERT INTO works (flow_version, graph, at, project) VALUES (1, 'architect', ?1, 'p')",
                    [at],
                )?;
            }
            for (work, started) in [(1, "line"), (2, "wake")] {
                tx.execute(
                    "INSERT INTO sessions (work, line, cli, cli_session, started) VALUES (?1, 'architect', 'c', 's', ?2)",
                    params![work, started],
                )?;
            }
            Ok(())
        })
        .unwrap();
        old.migrate(MIGRATIONS).unwrap();
        let mut st = old.conn().prepare("SELECT started FROM sessions ORDER BY id").unwrap();
        let started: Vec<String> = st.query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect();
        assert_eq!(started, ["line", "carried"]);
    }

    /// The cells of a graph in the flows of the counters tests: each with its id and its rows, as `(name, counts)`.
    type Cells<'a> = &'a [(&'a str, &'a [(&'a str, Option<&'a str>)])];

    /// A compiled flow of an engineer's graph and an architect's, with the cells given.
    fn counting_flow(engineer: Cells<'_>, architect: Cells<'_>) -> String {
        let graph = |name: &str, cells: Cells<'_>| {
            let cells: Vec<serde_json::Value> = cells
                .iter()
                .map(|(id, rows)| {
                    let rows: Vec<serde_json::Value> = rows
                        .iter()
                        .map(|(row, counts)| match counts {
                            Some(c) => serde_json::json!({ "name": row, "counts": c }),
                            None => serde_json::json!({ "name": row }),
                        })
                        .collect();
                    serde_json::json!({ "id": id, "rows": rows })
                })
                .collect();
            serde_json::json!({ "name": name, "cells": cells })
        };
        serde_json::json!({ "graphs": [graph("engineer", engineer), graph("architect", architect)] }).to_string()
    }

    /// What the counters views say: each work's counters with their times, and each work's asks.
    type Counters = (Vec<(i64, String, i64)>, Vec<(i64, i64)>);

    fn counters(db: &Db) -> Counters {
        let mut st = db
            .conn()
            .prepare("SELECT work, counter, times FROM work_counters ORDER BY work, counter")
            .unwrap();
        let counted = st
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap()
            .map(|x| x.unwrap())
            .collect();
        let mut st = db
            .conn()
            .prepare("SELECT work, asks FROM work_asking ORDER BY work")
            .unwrap();
        let asking = st
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(|x| x.unwrap())
            .collect();
        (counted, asking)
    }

    // The counters of a database migrated from version 69, where the views parsed the compiled flow of the version
    // each work is on now for every round, are what those views gave before the migration, for every work: rows that
    // count nothing, a row counted as total time, rounds that took no row, a cell or a row the version does not
    // have, two graphs with a cell of one id, a work that moved to a version that counts the same row otherwise, a
    // version no work is on, a name two rows of a cell share when only one counts, and rounds that ended in no
    // return (a return sent back, a failed run, a failed run past its bound, a round cut off, a round that ended in
    // an error), whose shared rows count by the flow's rounds section. The counts written to the rounds are those of
    // the rows by that rule. The expected lists are worked out from the rows written.
    #[test]
    fn counters_migrated_from_version_69_are_those_the_old_views_gave() {
        let mut old = at_version(69);
        // Version 1: asks and fixes count; `pause` counts the total time; a cell has two rows named `again`, the
        // second of which counts. Version 2: the same row `ask` counts another counter, and `fix` counts none.
        let v1 = counting_flow(
            &[
                ("A4", &[("ask", Some("asks")), ("go", None)]),
                (
                    "I1",
                    &[
                        ("fix", Some("fixes")),
                        ("pause", Some("total_time")),
                        ("again", None),
                        ("again", Some("retries")),
                        ("sent_back", Some("sent_back")),
                        ("transient", Some("retries")),
                        ("transient_too_often", None),
                        ("cut_off", None),
                    ],
                ),
            ],
            &[("A4", &[("ask", Some("architect_asks"))])],
        );
        let v2 = counting_flow(
            &[
                ("A4", &[("ask", Some("questions")), ("go", None)]),
                (
                    "I1",
                    &[
                        ("fix", None),
                        ("pause", Some("total_time")),
                        ("sent_back", Some("sent_backs")),
                    ],
                ),
            ],
            &[("A4", &[("ask", Some("architect_asks"))])],
        );
        // Version 3 is no work's, and has two rows of a name that both count: nothing reads it.
        let v3 = counting_flow(&[("A4", &[("ask", Some("asks")), ("ask", Some("other"))])], &[]);
        old.write(Actor::Engine, |tx| {
            for (digest, compiled) in [("one", &v1), ("two", &v2), ("three", &v3)] {
                tx.execute(
                    "INSERT INTO flow_versions (digest, compiled) VALUES (?1, ?2)",
                    params![digest, compiled],
                )?;
            }
            for (graph, version, moved_from) in [
                ("engineer", 1, None),
                ("engineer", 2, Some(1)),
                ("architect", 1, None),
                ("engineer", 2, None),
                ("engineer", 1, None),
            ] {
                tx.execute(
                    "INSERT INTO works (flow_version, moved_from, graph, at) VALUES (?1, ?2, ?3, 'A4')",
                    params![version, moved_from, graph],
                )?;
            }
            // Work 1 on version 1; work 2 on version 2 after version 1; work 3 the architect's on version 1; work 4
            // on version 2 with no round; work 5 on version 1. Each round: work, number, cell, row taken.
            let rounds = [
                (1, 1, Some("A4"), Some("ask")),
                (1, 2, Some("A4"), Some("ask")),
                (1, 3, Some("A4"), Some("ask")),
                (1, 4, Some("A4"), Some("go")),
                (1, 5, Some("I1"), Some("fix")),
                (1, 6, Some("I1"), Some("fix")),
                (1, 7, Some("I1"), Some("pause")),
                (1, 8, Some("ZZ"), Some("ask")),
                (1, 9, Some("A4"), Some("nothing")),
                (1, 10, Some("A4"), None),
                (1, 11, Some("I1"), Some("again")),
                (2, 1, Some("A4"), Some("ask")),
                (2, 2, Some("A4"), Some("ask")),
                (2, 3, Some("I1"), Some("fix")),
                (3, 1, Some("A4"), Some("ask")),
                (3, 2, Some("I1"), Some("fix")),
                (5, 1, Some("A4"), Some("ask")),
            ];
            for (work, n, cell, row) in rounds {
                tx.execute(
                    "INSERT INTO rounds (work, n, cell, input, transition, config_commit, started_at, ended_at)
                     VALUES (?1, ?2, ?3, '{}', ?4, 'c', 0, 1)",
                    params![work, n, cell, row],
                )?;
            }
            // A round off the graph, which has no cell; and a round of the tools cell that lasted two minutes, which
            // counts the total time.
            tx.execute(
                "INSERT INTO rounds (work, n, cell, input, config_commit, started_at, ended_at)
                 VALUES (1, 12, NULL, '{}', 'c', 0, 1), (1, 13, 'X1', '{}', 'c', 0, 120000)",
                [],
            )?;
            // Rounds that ended in no return its cell takes: a return sent back, a failed run, one past its bound, a
            // round cut off, a round that ended in an error, and a return sent back at a cell the version does not
            // have. Work 2 is on version 2, where a return sent back counts another counter.
            for (work, n, cell, rejected) in [
                (1, 14, "I1", r#"{"format":"x"}"#),
                (1, 15, "I1", r#"{"failed":{"kind":"transient","what":"w"}}"#),
                (1, 16, "I1", r#"{"failed":{"kind":"transient","what":"w","over":3}}"#),
                (1, 17, "I1", r#"{"interrupted":"stopped"}"#),
                (1, 18, "I1", r#"{"error":"e"}"#),
                (1, 19, "ZZ", r#"{"format":"x"}"#),
                (2, 4, "I1", r#"{"format":"x"}"#),
            ] {
                tx.execute(
                    "INSERT INTO rounds (work, n, cell, input, rejected, config_commit, started_at, ended_at)
                     VALUES (?1, ?2, ?3, '{}', ?4, 'c', 0, 1)",
                    params![work, n, cell, rejected],
                )?;
            }
            Ok(())
        })
        .unwrap();
        let before = counters(&old);
        // Worked out by hand: work 1 took `ask` 3 times (asks), `fix` 2 times (fixes) and the second `again` once
        // (retries); `pause` adds no counter of its own, and the total time is the two minutes of its round at `X1`.
        // Its one return sent back counts `sent_back`, and its failed run `transient` counts retries a second time;
        // the one past its bound, the round cut off, the one that ended in an error and the one at a cell the version
        // has not count nothing. Work 2 is on version 2: its `ask` is a question, its `fix` counts nothing, and its
        // return sent back counts another counter. Work 3 is the architect's: its one `ask` counts architect_asks, and
        // it has no `I1`.
        let counted = |work: i64, counter: &str, times: i64| (work, counter.to_string(), times);
        assert_eq!(
            before.0,
            [
                counted(1, "asks", 3),
                counted(1, "fixes", 2),
                counted(1, "retries", 2),
                counted(1, "sent_back", 1),
                counted(1, "total_time", 2),
                counted(2, "questions", 2),
                counted(2, "sent_backs", 1),
                counted(3, "architect_asks", 1),
                counted(5, "asks", 1),
            ]
        );
        assert_eq!(before.1, [(1, 3), (2, 0), (3, 0), (4, 0), (5, 1)]);
        old.migrate(MIGRATIONS).unwrap();
        assert_eq!(counters(&old), before);
        // The rounds hold the counter of the row they took, as the version their work is on says it.
        let mut st = old
            .conn()
            .prepare("SELECT work, n, counts FROM rounds WHERE counts IS NOT NULL ORDER BY work, n")
            .unwrap();
        let kept: Vec<(i64, i64, String)> = st
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap()
            .map(|x| x.unwrap())
            .collect();
        let kept_as = |work: i64, n: i64, counts: &str| (work, n, counts.to_string());
        assert_eq!(
            kept,
            [
                kept_as(1, 1, "asks"),
                kept_as(1, 2, "asks"),
                kept_as(1, 3, "asks"),
                kept_as(1, 5, "fixes"),
                kept_as(1, 6, "fixes"),
                kept_as(1, 7, "total_time"),
                kept_as(1, 11, "retries"),
                kept_as(1, 14, "sent_back"),
                kept_as(1, 15, "retries"),
                kept_as(2, 1, "questions"),
                kept_as(2, 2, "questions"),
                kept_as(2, 4, "sent_backs"),
                kept_as(3, 1, "architect_asks"),
                kept_as(5, 1, "asks"),
            ]
        );
        let new = Db::in_memory().unwrap();
        assert_eq!(shape(old.conn()).unwrap(), shape(new.conn()).unwrap());
    }

    // A flow version a work is on that has two rows of one name in a cell which both count something cannot be told
    // apart by the rule the old views used, which counted such a round twice: the migration fails and keeps the
    // database as it was.
    #[test]
    fn a_flow_version_with_two_counting_rows_of_a_name_stops_the_migration() {
        let mut old = at_version(69);
        let flow = counting_flow(&[("A4", &[("ask", Some("asks")), ("ask", Some("other"))])], &[]);
        old.write(Actor::Engine, |tx| {
            tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', ?1)", [&flow])?;
            tx.execute(
                "INSERT INTO works (flow_version, graph, at) VALUES (1, 'engineer', 'A4')",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        assert!(old.migrate(MIGRATIONS).is_err());
        assert_eq!(version_of(old.conn()).unwrap(), 69);
        let has_counts: i64 = old
            .conn()
            .query_row(
                "SELECT count(*) FROM pragma_table_info('rounds') WHERE name = 'counts'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(has_counts, 0);
    }

    // A round counts something only if it took a row, by its return (`transition`) or by ending in no return its cell
    // takes (`rejected`): the database refuses a counter on a round that took none, when it is written and when it is
    // ended; a round that took a row may count nothing or something.
    #[test]
    fn a_round_that_took_no_row_counts_nothing() {
        let mut db = Db::in_memory().unwrap();
        db.write(Actor::Engine, |tx| {
            tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
            tx.execute(
                "INSERT INTO works (flow_version, graph, at) VALUES (1, 'engineer', 'A4')",
                [],
            )?;
            tx.execute(
                "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES (1, 1, 'A4', '{}', 'c', 0)",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        let refused = |db: &mut Db, sql: &str| db.write(Actor::Engine, |tx| Ok(tx.execute(sql, [])?)).is_err();
        assert!(refused(
            &mut db,
            "INSERT INTO rounds (work, n, cell, input, counts, config_commit, started_at)
             VALUES (1, 2, 'A4', '{}', 'asks', 'c', 0)"
        ));
        assert!(refused(
            &mut db,
            "UPDATE rounds SET counts = 'asks' WHERE work = 1 AND n = 1"
        ));
        assert!(!refused(
            &mut db,
            "UPDATE rounds SET transition = 'ask', counts = 'asks' WHERE work = 1 AND n = 1"
        ));
        assert!(!refused(
            &mut db,
            "INSERT INTO rounds (work, n, cell, input, transition, config_commit, started_at)
             VALUES (1, 2, 'A4', '{}', 'go', 'c', 0)"
        ));
        assert!(!refused(
            &mut db,
            "INSERT INTO rounds (work, n, cell, input, rejected, counts, config_commit, started_at)
             VALUES (1, 3, 'A4', '{}', '{\"format\":\"x\"}', 'sent_back', 'c', 0)"
        ));
    }

    // The counters of one work are read from the index on its counted rounds alone: the plans of the two views for
    // one work search `rounds_counts`, which holds the counter without the round's inputs and outputs, and parse no
    // JSON and scan no table of rounds. The total time in `work_counters` is its other part, found by the work's
    // primary key. The same judgment fails on the views of version 69, which parsed the compiled flow for every
    // round with `json_each` and scanned the rounds: without that, a plan whose words SQLite changes would pass this
    // test for want of anything to find.
    #[test]
    fn the_counters_of_a_work_are_read_from_the_index_of_its_counted_rounds() {
        let faults = counter_plan_faults(&Db::in_memory().unwrap());
        assert!(faults.is_empty(), "{}", faults.join("\n\n"));
        let old = counter_plan_faults(&at_version(69));
        assert!(
            old.iter().any(|f| f.contains("parses JSON")),
            "the views of version 69 are not found to parse JSON: {old:?}"
        );
        assert!(
            old.iter().any(|f| f.contains("scans a table of rounds")),
            "the views of version 69 are not found to scan the rounds: {old:?}"
        );
    }

    /// What is wrong with how the two counters views are read for one work in `db`, as their query plans show: each
    /// fault names the view and what it does. Nothing is wrong when the list is empty.
    fn counter_plan_faults(db: &Db) -> Vec<String> {
        let plan = |sql: &str| -> Vec<String> {
            let mut st = db.conn().prepare(&format!("EXPLAIN QUERY PLAN {sql}")).unwrap();
            st.query_map([], |r| r.get::<_, String>(3))
                .unwrap()
                .map(|x| x.unwrap())
                .collect()
        };
        let mut faults = Vec::new();
        for view in ["work_asking", "work_counters"] {
            let steps = plan(&format!("SELECT * FROM {view} WHERE work = 1"));
            let text = steps.join("\n");
            if !steps.iter().any(|s| s.contains("COVERING INDEX rounds_counts")) {
                faults.push(format!("{view} does not read rounds_counts alone:\n{text}"));
            }
            if text.contains("json_each") || text.contains("VIRTUAL TABLE") {
                faults.push(format!("{view} parses JSON:\n{text}"));
            }
            if steps.iter().any(|s| s.starts_with("SCAN r")) {
                faults.push(format!("{view} scans a table of rounds:\n{text}"));
            }
        }
        // How asks are counted reads rounds in that one way and no other.
        let steps = plan("SELECT * FROM work_asking WHERE work = 1");
        let rounds: Vec<&String> = steps.iter().filter(|s| s.contains(" r ")).collect();
        if !(rounds.len() == 1 && rounds[0].contains("COVERING INDEX rounds_counts (work=? AND counts=?)")) {
            faults.push(format!(
                "work_asking reads rounds other than by rounds_counts:\n{}",
                steps.join("\n")
            ));
        }
        // The counters read rounds by the work only: the counted rounds through their index, and the total time
        // through the primary key.
        let steps = plan("SELECT * FROM work_counters WHERE work = 1");
        for s in steps.iter().filter(|s| s.contains(" r ")) {
            let by_work = s.starts_with("SEARCH r USING")
                && (s.contains("COVERING INDEX rounds_counts (work=?")
                    || s.contains("INDEX sqlite_autoindex_rounds_1 (work=?)"));
            if !by_work {
                faults.push(format!(
                    "work_counters reads rounds other than by the work:\n{}",
                    steps.join("\n")
                ));
            }
        }
        faults
    }
}
