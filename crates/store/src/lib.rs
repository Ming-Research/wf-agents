//! Storage: what makes the items and the works of the engine outlive the process.
//!
//! The engine holds no state: everything kept of a work is its [`Work`], defined in the crate `work` beneath both the
//! engine's loop and storage, so that storage does not depend on the loop (entry `engine-in-layers`). The loop only
//! adds to a work. After
//! each thing that happens to a work, whatever drives the engine [saves](Store::save) the work, and on starting it
//! [loads](Store::load) every work. The store also keeps the items, the things to be done: a work is done for one item,
//! and an item may have several works over time.
//!
//! Every table is only inserted into (entry `tables-only-added-to`): a work is saved by inserting the entries it has
//! that the store does not hold yet, after checking that what the store holds is where the work begins. A work changed
//! where it may only be added to is a fault of the engine, and is refused rather than written over what was kept
//! (principle `nothing-passes-for-normal`). The schema is [`SCHEMA`], kept in SQLite.
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

/// The version of [`SCHEMA`]. A database made with another is not opened: there is no migration yet.
pub const SCHEMA_VERSION: u32 = 1;

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
    /// The store has no item of this number.
    NoSuchItem(ItemNumber),
    /// The store has no work of this number.
    NoSuchWork(WorkNumber),
    /// An item was to depend on one the store does not have: an item depends only on items added before it.
    NoSuchDependency(ItemNumber),
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

impl Store {
    /// Opens the database at this path, making it with the schema where it is new or empty; refuses one made with
    /// another version of the schema.
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
            if found != Some(SCHEMA_VERSION) {
                return Err(Fault::Schema {
                    found,
                    expected: SCHEMA_VERSION,
                });
            }
        }
        tx.commit()?;
        Ok(Store { db })
    }

    /// Adds an item, depending on these items, which must all be in the store already, and gives its number.
    pub fn add_item(&mut self, title: &str, depends_on: &[ItemNumber], at: Time) -> Result<ItemNumber, Fault> {
        let tx = self.db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let number = items::add_item(&tx, title, depends_on, at)?;
        tx.commit()?;
        Ok(number)
    }

    /// Every item, in the order of their numbers.
    pub fn items(&self) -> Result<Vec<Item>, Fault> {
        items::items(&self.db)
    }

    /// Adds a work for the item, on the flow of this name, and gives its number. The work has nothing kept of it yet.
    /// Only the flow's name is kept: which version of the flow the work runs on is not kept yet.
    pub fn add_work(&mut self, item: ItemNumber, flow: &str, at: Time) -> Result<WorkNumber, Fault> {
        let tx = self.db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let number = items::add_work(&tx, item, flow, at)?;
        tx.commit()?;
        Ok(number)
    }

    /// Every work, in the order of their numbers.
    pub fn works(&self) -> Result<Vec<WorkHead>, Fault> {
        items::works(&self.db)
    }

    /// Everything kept of the work.
    pub fn load(&self, work: WorkNumber) -> Result<Work, Fault> {
        items::known(&self.db, work)?;
        tables::load(&self.db, work)
    }

    /// Keeps what the work has that the store does not hold yet, in one transaction. What the store holds of it must
    /// be where the given work begins: where it holds an entry the work lacks or holds otherwise, nothing is kept and
    /// the fault says where. Saving a work the store holds whole keeps nothing. Each call reads the whole of what the
    /// store holds of the work, to compare it with the given work.
    pub fn save(&mut self, work: WorkNumber, given: &Work) -> Result<(), Fault> {
        let tx = self.db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        items::known(&tx, work)?;
        tables::save(&tx, work, given)?;
        tx.commit()?;
        Ok(())
    }
}
