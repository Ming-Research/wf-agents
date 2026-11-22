//! Storage: what makes the items and the works of the engine outlive the process.
//!
//! The engine holds no state: everything kept of a work is its [`Work`], defined in the crate `work` beneath both the
//! engine's loop and storage, so that storage does not depend on the loop (principle `dependencies-one-way`). The loop
//! only adds to a work. After each thing that happens to a work, whatever drives the engine [saves](Storage::save) the
//! work, and on starting it [loads](Storage::load) every work. The store also keeps the items, the things to be done:
//! an item is [added](Storage::add) together with the work done for it and what is kept of that work from its start,
//! all of it or none.
//!
//! Whatever drives the engine reaches the store through [`Storage`] (principle `outside-behind-interfaces`):
//! [`Store`] is the real one, kept in SQLite, and a test gives its own to make the store fail at a step it chooses.
//!
//! Every table is only inserted into (decision `tables-only-added-to`): a work is saved by inserting the entries it has
//! that the store does not hold yet, after checking that what the store holds is where the work begins. A work changed
//! where it may only be added to is a fault of the engine, and is refused rather than written over what was kept
//! (principle `nothing-passes-for-normal`). The schema is [`SCHEMA`], kept in SQLite. A database made with an earlier
//! version of it is brought to this one as it is opened, by the migrations of `db/migrations/`, each in its turn and
//! all of them in one transaction with the opening, so that a database is either opened at this version or left as it
//! was. Before that it is copied whole, so that a migration that goes wrong loses nothing ([`before_migration`]). A
//! migration that gives the rows kept before a value makes the table anew and carries each row across whole, with the
//! value, in a step of the store after its SQL.
//!
//! The store also keeps what is the installation's and no work's: the pause of everything, set and lifted, with who set
//! it: the owner, or the engine as it started with works going on; whether the sessions of works that wait are kept
//! warm; the accounts the agents' program may run under, each added, removed and taken by the owner, by its name,
//! whose token is kept outside the store; and each version of a flow a work runs on.
//!
//! - `items`: the items and the works done for them.
//! - `flows`: the versions of the flows works run on, each kept once as its files and its outline.
//! - `tables`: a work's entries, table by table.
//! - `backups`: the copies of the database that were taken and uploaded, and what failed in keeping them. They belong
//!   to no work, and the backup's own task keeps them through a connection of its own ([`Backups`]).
//! - `observations`: what agents came across beyond their works, the observations each run kept as it finished, which
//!   are kept with the rest of the work and numbered as the installation's; and the owner's reading of each. They are
//!   read for the page, and marked read, through a connection of their own ([`Observations`]).

mod backups;
pub mod before_migration;
mod flows;
mod items;
mod observations;
mod tables;

use std::path::Path;

use decide::{BlockId, CallId, Pauser, RunId, Seq};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior};
use work::{Time, Work};

pub use backups::{Backups, Failure, Kept, Records};
pub use flows::{KeptFlow, KeptOutline};
pub use items::{Change, First, Item, ItemChange, ItemNumber, WorkHead, WorkNumber};
pub use observations::{Observation, Observations};

/// The name of the store's file in the data directory.
pub const FILE: &str = "store.sqlite";

/// The schema of the store, as SQL.
pub const SCHEMA: &str = include_str!("../../../db/schema.sql");

/// The version of [`SCHEMA`]. A database made with an earlier one is migrated to it as it is opened; one made with
/// any other is not opened.
pub const SCHEMA_VERSION: u32 = 24;

/// A step of the store after a migration's SQL.
type Step = fn(&Connection) -> Result<(), Fault>;

/// A migration: the SQL that brings a database one version further, replacing the version's one row last, and the step
/// of the store that runs after it, where it has one.
struct Migration {
    sql: &'static str,
    then: Option<Step>,
}

impl Migration {
    const fn sql(sql: &'static str) -> Self {
        Migration { sql, then: None }
    }
}

/// The migrations, in order: the first brings a database of version 1 of the schema to version 2, and each after it
/// one version further, replacing the version's one row last. Each file is named by the minute it was written and
/// what it does.
const MIGRATIONS: &[Migration] = &[
    Migration::sql(include_str!(
        "../../../db/migrations/202611101530-say-who-paused-a-work-and-why.sql"
    )),
    Migration::sql(include_str!(
        "../../../db/migrations/202611102304-give-items-a-level.sql"
    )),
    Migration::sql(include_str!(
        "../../../db/migrations/202611111400-say-when-a-work-let-go-of-what-it-held.sql"
    )),
    Migration::sql(include_str!(
        "../../../db/migrations/202611111401-keep-the-owners-pause-of-everything.sql"
    )),
    Migration::sql(include_str!(
        "../../../db/migrations/202611111600-record-the-sessions-of-lines-and-the-context-of-rounds.sql"
    )),
    Migration::sql(include_str!(
        "../../../db/migrations/202611111830-keep-each-change-of-an-item-and-why-it-was-cancelled.sql"
    )),
    Migration::sql(include_str!(
        "../../../db/migrations/202611112000-say-who-paused-everything.sql"
    )),
    Migration::sql(include_str!("../../../db/migrations/202611112100-record-backups.sql")),
    Migration::sql(include_str!(
        "../../../db/migrations/202611131500-keep-what-a-return-dealt-with-what-reached-a-round-and-what-filled-it.sql"
    )),
    Migration::sql(include_str!(
        "../../../db/migrations/202611141600-keep-the-observations-the-returns-taken-carry.sql"
    )),
    Migration::sql(include_str!(
        "../../../db/migrations/202611141700-give-items-a-project.sql"
    )),
    Migration::sql(include_str!(
        "../../../db/migrations/202611141900-keep-the-questions-put-to-the-owner-and-the-answers.sql"
    )),
    Migration::sql(include_str!(
        "../../../db/migrations/202611142000-say-why-each-change-of-an-item-was-made-tag-items-and-reopen-them.sql"
    )),
    Migration::sql(include_str!(
        "../../../db/migrations/202611142100-keep-what-the-calls-of-a-round-came-to-and-the-pings-of-sessions.sql"
    )),
    Migration::sql(include_str!(
        "../../../db/migrations/202611142300-keep-the-cell-an-entry-of-what-is-said-is-said-to.sql"
    )),
    Migration::sql(include_str!(
        "../../../db/migrations/202611151200-say-what-a-finished-run-waits-for-from-outside.sql"
    )),
    Migration {
        sql: include_str!("../../../db/migrations/202611151600-say-whether-a-version-holds-anything.sql"),
        then: Some(tables::fill_empty),
    },
    Migration::sql(include_str!(
        "../../../db/migrations/202611171900-keep-the-pull-request-of-each-work.sql"
    )),
    Migration::sql(include_str!(
        "../../../db/migrations/202611182200-keep-whether-the-sessions-of-works-that-wait-are-kept-warm.sql"
    )),
    Migration::sql(include_str!(
        "../../../db/migrations/202611191200-keep-the-accounts-the-agents-run-under.sql"
    )),
    Migration::sql(include_str!(
        "../../../db/migrations/202611211200-keep-the-versions-of-the-flows-works-run-on.sql"
    )),
    Migration::sql(include_str!(
        "../../../db/migrations/202611221000-keep-what-the-programs-of-tries-left-running.sql"
    )),
    Migration::sql(include_str!(
        "../../../db/migrations/202611221100-keep-the-engine-s-stops.sql"
    )),
];

// The migrations bring a database of every earlier version to this one.
const _: () = assert!(MIGRATIONS.len() as u32 + 1 == SCHEMA_VERSION);

/// The pause of everything, while it holds: when it began, and who set it, the owner or the engine as it started with
/// works going on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EverythingPaused {
    pub since: Time,
    pub by: Pauser,
}

/// A pause of everything set, or lifted, at this time, by this pauser: what the store keeps of it, every one in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PauseOfEverything {
    pub paused: bool,
    pub by: Pauser,
    pub at: Time,
}

/// An account the agents' program may run under, added, removed or taken by the owner as the one the agents run under,
/// by the name the owner gave it, at this time: what the store keeps of it, every one in order. Its token is kept
/// outside the store.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountRecord {
    pub name: String,
    pub what: AccountAct,
    pub at: Time,
}

/// What was done to an account.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AccountAct {
    Added,
    Removed,
    /// Taken as the one the agents run under.
    Taken,
}

/// The engine asked to stop, or started again after a stop, at this time: what the store keeps of it, every one in
/// order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StopRecord {
    pub what: StopAct,
    pub at: Time,
}

/// What a stop of the engine came to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum StopAct {
    /// Asked to stop once what is under way has ended: from then no round of any work begins.
    Asked,
    /// Asked to stop at once, cutting off what is under way.
    Now,
    /// Started again after a stop, which no longer stands.
    Over,
}

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
    /// An item was to depend on one the store does not have: an item is added depending only on items added before it.
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
    /// What a round of a run was given, came back from it or it took, or an observation kept of what came back, the
    /// first round being one.
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
    /// Adds an item as it is to be added, at its level, of its project, depending on items which must all be in the
    /// store already, of any project, and carrying its tags, together with the work done for it on the flow of this
    /// name and what is kept of that work from its start, and gives their numbers. The item, the work and what is kept
    /// of it are added in one transaction: all of them or none. Which version of the flow the work runs on is the
    /// work's first entry, a version the store keeps already.
    fn add(&mut self, item: &First, flow: &str, at: Time, first: &Work) -> Result<(ItemNumber, WorkNumber), Fault>;

    /// Keeps a version of a flow where it is not kept already, and each of its files where no version holds it.
    fn keep_flow(&mut self, kept: &KeptFlow) -> Result<(), Fault>;

    /// Every version of a flow kept, with its outline, in the order of their digests.
    fn outlines(&self) -> Result<Vec<KeptOutline>, Fault>;

    /// Keeps changes of items made together, at one time and for one reason, each after its item's earlier changes and
    /// in the order given, with what each of these works has that the store does not hold yet, in one transaction: all
    /// of it or none. Each item, and any item one comes to depend on, must be in the store, and each work saved as
    /// [`Storage::save`] saves it. Whether the changes may be made is not the store's to judge.
    fn change(
        &mut self,
        at: Time,
        why: Option<&str>,
        changes: &[(ItemNumber, ItemChange)],
        works: &[(WorkNumber, &Work)],
    ) -> Result<(), Fault>;

    /// Every item, in the order of their numbers, each as it stands after its changes.
    fn items(&self) -> Result<Vec<Item>, Fault>;

    /// Every work, in the order of their numbers.
    fn works(&self) -> Result<Vec<WorkHead>, Fault>;

    /// Everything kept of the work.
    fn load(&self, work: WorkNumber) -> Result<Work, Fault>;

    /// Keeps what the work has that the store does not hold yet, in one transaction: each observation its runs kept is
    /// numbered then, as the installation's. What the store holds of it must be where the given work begins: where it
    /// holds an entry the work lacks or holds otherwise, nothing is kept and the fault says where. Saving a work the
    /// store holds whole keeps nothing.
    fn save(&mut self, work: WorkNumber, given: &Work) -> Result<(), Fault>;

    /// The pause of everything, where it holds: when it began and who set it; none where it does not, as in a store
    /// that never kept one.
    fn everything_paused(&self) -> Result<Option<EverythingPaused>, Fault>;

    /// Keeps that everything was paused, or that pause lifted, by this pauser at this time.
    fn pause_everything(&mut self, paused: bool, by: Pauser, at: Time) -> Result<(), Fault>;

    /// Every pause of everything set or lifted, in the order they were kept.
    fn everything_pauses(&self) -> Result<Vec<PauseOfEverything>, Fault>;

    /// Whether the sessions of works that wait are kept warm, as it was last switched; they are where it was never
    /// switched, as in a store that never kept it.
    fn keeping_warm(&self) -> Result<bool, Fault>;

    /// Keeps that the sessions of works that wait are kept warm from this time, or are not.
    fn keep_warm(&mut self, warm: bool, at: Time) -> Result<(), Fault>;

    /// Every account added, removed or taken, in the order they were kept; none in a store that never kept one.
    fn accounts(&self) -> Result<Vec<AccountRecord>, Fault>;

    /// Keeps that an account was added, removed or taken. Whether it may be is not the store's to judge.
    fn keep_account(&mut self, record: &AccountRecord) -> Result<(), Fault>;

    /// Every stop of the engine asked for, and every start after one, in the order they were kept; none in a store
    /// that never kept one.
    fn stops(&self) -> Result<Vec<StopRecord>, Fault>;

    /// Keeps that the engine was asked to stop, or started again after a stop.
    fn keep_stop(&mut self, record: &StopRecord) -> Result<(), Fault>;
}

impl Store {
    /// Opens the database at this path, making it with the schema where it is new or empty, and migrating it where it
    /// was made with an earlier version of the schema; refuses one made with any other version. One that is to be
    /// migrated is copied whole first ([`before_migration`]), and is refused where the copy cannot be taken.
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
        // A database made with an earlier version is copied whole before it is migrated. SQLite takes no such copy
        // inside a transaction, so it is taken ahead of the one that migrates, which reads the version again.
        if let Some(earlier @ 1..SCHEMA_VERSION) = version(&db)? {
            before_migration::copy(&db, earlier)?;
        }
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let objects: u64 = tx.query_row("SELECT count(*) FROM sqlite_master", [], |row| row.get(0))?;
        if objects == 0 {
            tx.execute_batch(SCHEMA)?;
            tx.execute("INSERT INTO schema_version (version) VALUES (?1)", [SCHEMA_VERSION])?;
        } else {
            let found = version(&tx)?;
            match found {
                Some(SCHEMA_VERSION) => {}
                // Made with an earlier version: each migration from it on is run, in this transaction, each replacing
                // the version's one row with its own.
                Some(earlier @ 1..SCHEMA_VERSION) => {
                    migrate(&tx, earlier, SCHEMA_VERSION)?;
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

/// Brings a database of version `from` of the schema to version `to`, by the migrations between, each with its step.
/// A test makes a database of an earlier version by it.
pub fn migrate(db: &Connection, from: u32, to: u32) -> Result<(), Fault> {
    let between = (from as usize)
        .checked_sub(1)
        .zip((to as usize).checked_sub(1))
        .and_then(|(start, end)| MIGRATIONS.get(start..end))
        .ok_or_else(|| Fault::Database(format!("there are no migrations from version {from} to version {to}")))?;
    for migration in between {
        db.execute_batch(migration.sql)?;
        if let Some(then) = migration.then {
            then(db)?;
        }
    }
    Ok(())
}

/// The version of the schema a database says it was made with; none where it has no table of the version, as a new
/// one, or no row in it.
fn version(db: &Connection) -> Result<Option<u32>, Fault> {
    let has_version: bool = db.query_row(
        "SELECT count(*) > 0 FROM sqlite_master WHERE type = 'table' AND name = 'schema_version'",
        [],
        |row| row.get(0),
    )?;
    if !has_version {
        return Ok(None);
    }
    Ok(db
        .query_row("SELECT version FROM schema_version", [], |row| row.get(0))
        .optional()?)
}

impl Storage for Store {
    fn add(&mut self, item: &First, flow: &str, at: Time, first: &Work) -> Result<(ItemNumber, WorkNumber), Fault> {
        let tx = self.db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let item = items::add_item(&tx, item, at)?;
        let work = items::add_work(&tx, item, flow, at)?;
        tables::save(&tx, work, first)?;
        tx.commit()?;
        Ok((item, work))
    }

    fn keep_flow(&mut self, kept: &KeptFlow) -> Result<(), Fault> {
        let tx = self.db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        flows::keep(&tx, kept)?;
        tx.commit()?;
        Ok(())
    }

    fn outlines(&self) -> Result<Vec<KeptOutline>, Fault> {
        flows::outlines(&self.db)
    }

    fn change(
        &mut self,
        at: Time,
        why: Option<&str>,
        changes: &[(ItemNumber, ItemChange)],
        works: &[(WorkNumber, &Work)],
    ) -> Result<(), Fault> {
        let tx = self.db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        items::change(&tx, at, why, changes)?;
        for (work, given) in works {
            items::known(&tx, *work)?;
            tables::save(&tx, *work, given)?;
        }
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

    fn everything_paused(&self) -> Result<Option<EverythingPaused>, Fault> {
        let latest = (self.db)
            .query_row(
                "SELECT paused, pauser, time FROM everything_paused ORDER BY n DESC LIMIT 1",
                [],
                |row| Ok((row.get::<_, bool>(0)?, row.get::<_, String>(1)?, row.get::<_, u64>(2)?)),
            )
            .optional()?;
        let Some((true, pauser, time)) = latest else {
            return Ok(None);
        };
        Ok(Some(EverythingPaused {
            since: Time(time),
            by: tables::from_name("everything_paused", pauser)?,
        }))
    }

    fn pause_everything(&mut self, paused: bool, by: Pauser, at: Time) -> Result<(), Fault> {
        let pauser = tables::name("everything_paused", &by)?;
        (self.db).execute(
            "INSERT INTO everything_paused (paused, pauser, time) VALUES (?1, ?2, ?3)",
            rusqlite::params![paused, pauser, at.0],
        )?;
        Ok(())
    }

    fn keeping_warm(&self) -> Result<bool, Fault> {
        let latest = (self.db)
            .query_row("SELECT warm FROM keep_warm ORDER BY n DESC LIMIT 1", [], |row| {
                row.get::<_, bool>(0)
            })
            .optional()?;
        Ok(latest.unwrap_or(true))
    }

    fn keep_warm(&mut self, warm: bool, at: Time) -> Result<(), Fault> {
        (self.db).execute(
            "INSERT INTO keep_warm (warm, time) VALUES (?1, ?2)",
            rusqlite::params![warm, at.0],
        )?;
        Ok(())
    }

    fn accounts(&self) -> Result<Vec<AccountRecord>, Fault> {
        let mut query = (self.db).prepare("SELECT name, what, time FROM accounts ORDER BY n")?;
        let rows = query.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, u64>(2)?,
            ))
        })?;
        let mut records = Vec::new();
        for row in rows {
            let (name, what, time) = row?;
            records.push(AccountRecord {
                name,
                what: tables::from_name("accounts", what)?,
                at: Time(time),
            });
        }
        Ok(records)
    }

    fn keep_account(&mut self, record: &AccountRecord) -> Result<(), Fault> {
        let what = tables::name("accounts", &record.what)?;
        (self.db).execute(
            "INSERT INTO accounts (name, what, time) VALUES (?1, ?2, ?3)",
            rusqlite::params![record.name, what, record.at.0],
        )?;
        Ok(())
    }

    fn stops(&self) -> Result<Vec<StopRecord>, Fault> {
        let mut query = (self.db).prepare("SELECT what, time FROM stops ORDER BY n")?;
        let rows = query.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, u64>(1)?)))?;
        let mut records = Vec::new();
        for row in rows {
            let (what, time) = row?;
            records.push(StopRecord {
                what: tables::from_name("stops", what)?,
                at: Time(time),
            });
        }
        Ok(records)
    }

    fn keep_stop(&mut self, record: &StopRecord) -> Result<(), Fault> {
        let what = tables::name("stops", &record.what)?;
        (self.db).execute(
            "INSERT INTO stops (what, time) VALUES (?1, ?2)",
            rusqlite::params![what, record.at.0],
        )?;
        Ok(())
    }

    fn everything_pauses(&self) -> Result<Vec<PauseOfEverything>, Fault> {
        let mut query = (self.db).prepare("SELECT paused, pauser, time FROM everything_paused ORDER BY n")?;
        let rows = query.query_map([], |row| {
            Ok((row.get::<_, bool>(0)?, row.get::<_, String>(1)?, row.get::<_, u64>(2)?))
        })?;
        let mut pauses = Vec::new();
        for row in rows {
            let (paused, pauser, time) = row?;
            pauses.push(PauseOfEverything {
                paused,
                by: tables::from_name("everything_paused", pauser)?,
                at: Time(time),
            });
        }
        Ok(pauses)
    }
}
