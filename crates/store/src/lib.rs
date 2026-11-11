//! Storage: what makes the items and the works of the engine outlive the process.
//!
//! The engine holds no state: everything kept of a work is its [`Work`], defined in the crate `work` beneath both the
//! engine's loop and storage, so that storage does not depend on the loop (entry `engine-in-layers`). The loop only
//! adds to a work. After each thing that happens to a work, whatever drives the engine [saves](Storage::save) the
//! work, and on starting it [loads](Storage::load) every work. The store also keeps the items, the things to be done:
//! an item is [added](Storage::add) together with the work done for it and what is kept of that work from its start,
//! all of it or none.
//!
//! Whatever drives the engine reaches the store through [`Storage`] (principle `outside-behind-interfaces`):
//! [`Store`] is the real one, kept in SQLite, and a test gives its own to make the store fail at a step it chooses.
//!
//! Every table is only inserted into (entry `tables-only-added-to`): a work is saved by inserting the entries it has
//! that the store does not hold yet, after checking that what the store holds is where the work begins. A work changed
//! where it may only be added to is a fault of the engine, and is refused rather than written over what was kept
//! (principle `nothing-passes-for-normal`). The schema is [`SCHEMA`], kept in SQLite. A database made with an earlier
//! version of it is brought to this one as it is opened, by the migrations of `migrations/`, each in its turn and all
//! of them in one transaction with the opening, so that a database is either opened at this version or left as it was.
//!
//! The store also keeps what is the installation's and no work's: the owner's pause of everything, set and lifted.
//!
//! - `items`: the items and the works done for them.
//! - `tables`: a work's entries, table by table.

mod items;
mod tables;

use std::path::Path;

use decide::{BlockId, CallId, RunId, Seq};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior};
use work::{Time, Work};

pub use items::{Item, ItemNumber, WorkHead, WorkNumber};

/// The schema of the store, as SQL.
pub const SCHEMA: &str = include_str!("../schema.sql");

/// The version of [`SCHEMA`]. A database made with an earlier one is migrated to it as it is opened; one made with
/// any other is not opened.
pub const SCHEMA_VERSION: u32 = 5;

/// The migrations, in order: the first brings a database of version 1 of the schema to version 2, and each after it
/// one version further, replacing the version's one row last. Each file is named by the minute it was written and
/// what it does.
const MIGRATIONS: &[&str] = &[
    include_str!("../migrations/202611101530-say-who-paused-a-work-and-why.sql"),
    include_str!("../migrations/202611102304-give-items-a-level.sql"),
    include_str!("../migrations/202611111400-say-when-a-work-let-go-of-what-it-held.sql"),
    include_str!("../migrations/202611111401-keep-the-owners-pause-of-everything.sql"),
];

// The migrations bring a database of every earlier version to this one.
const _: () = assert!(MIGRATIONS.len() as u32 + 1 == SCHEMA_VERSION);

/// The items and the works, kept in one SQLite database.
pub struct Store {
    db: Connection,
}

/// Why the store did not do what it was asked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fault {
    /// The database failed, in its own words.
    Database(String),
    /// The database was made with another version of the schema than this code's; `found` is none where it has
    /// tables but no version at all.
    Schema { found: Option<u32>, expected: u32 },
    /// The store has no work of this number.
    NoSuchWork(WorkNumber),
    /// An item was to depend on one the store does not have: an item depends only on items added before it.
    NoSuchDependency(ItemNumber),
    /// The store has no item of this number.
    NoSuchItem(ItemNumber),
    /// The work given to be saved is not what the store holds with more added: at this entry of this table it holds
    /// something else, or lacks what the store holds. Nothing was saved.
    Diverged {
        table: &'static str,
        entry: Entry,
        how: Divergence,
    },
    /// A value of this table could not be written as JSON, or what the table holds could not be read back as one;
    /// in the words of the reader or the writer.
    Value { table: &'static str, message: String },
}

/// An entry of a work, as a table keys it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Entry {
    /// An entry of the records, or a moment, by its place in the order of what happened to the work.
    At(Seq),
    /// The content of a version of a block, the version named by the place of the entry that wrote it.
    Version { block: BlockId, at: Seq },
    /// What a round of a run was given, came back from it or it took, the first round being one.
    Round { run: RunId, round: usize },
    /// What a call is, or its result.
    Call(CallId),
}

/// How a work given to be saved differs from what the store holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Divergence {
    /// The work holds something else at the entry.
    Changed,
    /// The work lacks the entry.
    Missing,
}

impl From<rusqlite::Error> for Fault {
    fn from(e: rusqlite::Error) -> Self {
        Fault::Database(e.to_string())
    }
}

/// The store, as whatever drives the engine reaches it.
pub trait Storage {
    /// Adds an item at this level, depending on these items, which must all be in the store already, together with the
    /// work done for it on the flow of this name and what is kept of that work from its start, and gives their numbers.
    /// The
    /// item, the work and what is kept of it are added in one transaction: all of them or none. Only the flow's name
    /// is kept: which version of the flow the work runs on is not kept yet.
    fn add(
        &mut self,
        title: &str,
        level: &str,
        depends_on: &[ItemNumber],
        flow: &str,
        at: Time,
        first: &Work,
    ) -> Result<(ItemNumber, WorkNumber), Fault>;

    /// Puts the item at this level from now on, keeping the change with its time.
    fn set_level(&mut self, item: ItemNumber, level: &str, at: Time) -> Result<(), Fault>;

    /// Every item, in the order of their numbers, each at its latest level.
    fn items(&self) -> Result<Vec<Item>, Fault>;

    /// Every work, in the order of their numbers.
    fn works(&self) -> Result<Vec<WorkHead>, Fault>;

    /// Everything kept of the work.
    fn load(&self, work: WorkNumber) -> Result<Work, Fault>;

    /// Keeps what the work has that the store does not hold yet, in one transaction. What the store holds of it must
    /// be where the given work begins: where it holds an entry the work lacks or holds otherwise, nothing is kept and
    /// the fault says where. Saving a work the store holds whole keeps nothing.
    fn save(&mut self, work: WorkNumber, given: &Work) -> Result<(), Fault>;

    /// When the owner's pause of everything began, where it holds; none where it does not, as in a store that never
    /// kept one.
    fn everything_paused(&self) -> Result<Option<Time>, Fault>;

    /// Keeps that the owner paused everything, or lifted that pause, at this time.
    fn pause_everything(&mut self, paused: bool, at: Time) -> Result<(), Fault>;
}

impl Store {
    /// Opens the database at this path, making it with the schema where it is new or empty, and migrating it where it
    /// was made with an earlier version of the schema; refuses one made with any other version.
    pub fn open(path: impl AsRef<Path>) -> Result<Store, Fault> {
        let store = Store::start(Connection::open(path)?)?;
        // A write-ahead log, so that what reads the database is not held up by what writes it. It is set once the
        // database is known to be the store's, so that a file refused is left as it was.
        (store.db).pragma_update_and_check(None, "journal_mode", "wal", |row| row.get::<_, String>(0))?;
        Ok(store)
    }

    /// A store kept in memory alone, for tests.
    pub fn in_memory() -> Result<Store, Fault> {
        Store::start(Connection::open_in_memory()?)
    }

    fn start(mut db: Connection) -> Result<Store, Fault> {
        db.pragma_update(None, "foreign_keys", "ON")?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let objects: u64 = tx.query_row("SELECT count(*) FROM sqlite_master", [], |row| row.get(0))?;
        if objects == 0 {
            tx.execute_batch(SCHEMA)?;
            tx.execute("INSERT INTO schema_version (version) VALUES (?1)", [SCHEMA_VERSION])?;
        } else {
            let has_version: bool = tx.query_row(
                "SELECT count(*) > 0 FROM sqlite_master WHERE type = 'table' AND name = 'schema_version'",
                [],
                |row| row.get(0),
            )?;
            let found = if has_version {
                tx.query_row("SELECT version FROM schema_version", [], |row| row.get(0))
                    .optional()?
            } else {
                None
            };
            match found {
                Some(SCHEMA_VERSION) => {}
                // Made with an earlier version: each migration from it on is run, in this transaction, each replacing
                // the version's one row with its own.
                Some(earlier @ 1..SCHEMA_VERSION) => {
                    for migration in &MIGRATIONS[earlier as usize - 1..] {
                        tx.execute_batch(migration)?;
                    }
                }
                _ => {
                    return Err(Fault::Schema {
                        found,
                        expected: SCHEMA_VERSION,
                    });
                }
            }
        }
        tx.commit()?;
        Ok(Store { db })
    }
}

impl Storage for Store {
    fn add(
        &mut self,
        title: &str,
        level: &str,
        depends_on: &[ItemNumber],
        flow: &str,
        at: Time,
        first: &Work,
    ) -> Result<(ItemNumber, WorkNumber), Fault> {
        let tx = self.db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let item = items::add_item(&tx, title, level, depends_on, at)?;
        let work = items::add_work(&tx, item, flow, at)?;
        tables::save(&tx, work, first)?;
        tx.commit()?;
        Ok((item, work))
    }

    fn set_level(&mut self, item: ItemNumber, level: &str, at: Time) -> Result<(), Fault> {
        let tx = self.db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        items::set_level(&tx, item, level, at)?;
        tx.commit()?;
        Ok(())
    }

    fn items(&self) -> Result<Vec<Item>, Fault> {
        items::items(&self.db)
    }

    fn works(&self) -> Result<Vec<WorkHead>, Fault> {
        items::works(&self.db)
    }

    fn load(&self, work: WorkNumber) -> Result<Work, Fault> {
        items::known(&self.db, work)?;
        tables::load(&self.db, work)
    }

    /// Each call reads the whole of what the store holds of the work, to compare it with the given work.
    fn save(&mut self, work: WorkNumber, given: &Work) -> Result<(), Fault> {
        let tx = self.db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        items::known(&tx, work)?;
        tables::save(&tx, work, given)?;
        tx.commit()?;
        Ok(())
    }

    fn everything_paused(&self) -> Result<Option<Time>, Fault> {
        let latest = (self.db)
            .query_row(
                "SELECT paused, time FROM everything_paused ORDER BY n DESC LIMIT 1",
                [],
                |row| Ok((row.get::<_, bool>(0)?, row.get::<_, u64>(1)?)),
            )
            .optional()?;
        Ok(latest.and_then(|(paused, time)| paused.then_some(Time(time))))
    }

    fn pause_everything(&mut self, paused: bool, at: Time) -> Result<(), Fault> {
        (self.db).execute(
            "INSERT INTO everything_paused (paused, time) VALUES (?1, ?2)",
            rusqlite::params![paused, at.0],
        )?;
        Ok(())
    }
}
