//! The copy of the database taken before it is migrated, so that a migration that goes wrong loses nothing. Each is
//! the whole database as it stood, read as one snapshot (SQLite's `VACUUM INTO`), kept beside it under `backups/`,
//! where the daily copies are kept too. Its name says the version it had after a number one more than the last such
//! copy's, so that the copies are told oldest first without a clock. The newest [`KEEP`] are kept.

use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::Fault;

/// What the name of a copy taken before a migration begins with.
pub const BEFORE_MIGRATION: &str = "before-migration-";

/// How many copies taken before a migration are kept, the newest.
pub const KEEP: usize = 7;

/// The directory a database's copies are kept in: `backups/` beside it.
pub fn dir(database: &Path) -> PathBuf {
    database.parent().unwrap_or_else(|| Path::new(".")).join("backups")
}

/// The copies taken before a migration that `dir` holds, oldest first; none where there is no such directory. A file
/// of any other name is not one of them.
pub fn copies(dir: &Path) -> Vec<PathBuf> {
    let mut found: Vec<(u64, PathBuf)> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter_map(|p| Some((number(&p)?, p)))
                .collect()
        })
        .unwrap_or_default();
    found.sort();
    found.into_iter().map(|(_, p)| p).collect()
}

/// The number in the name of a copy taken before a migration; none for any other name.
fn number(copy: &Path) -> Option<u64> {
    let name = copy.file_name()?.to_str()?;
    let (number, version) = (name.strip_prefix(BEFORE_MIGRATION)?.strip_suffix(".sqlite")?).split_once("-v")?;
    let digits = |text: &str| !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit());
    if !digits(number) || !digits(version) {
        return None;
    }
    number.parse().ok()
}

/// How often the copy is tried, each time under the name after the last, where the name it chose was taken meanwhile.
const TRIES: u32 = 3;

/// Copies the database `db` has open, made with the version `from` of the schema, before it is migrated, and keeps
/// the newest such copies; nothing for a database with no file, such as one in memory.
pub(crate) fn copy(db: &Connection, from: u32) -> Result<(), Fault> {
    let Some(database) = db.path().filter(|p| !p.is_empty()).map(PathBuf::from) else {
        return Ok(());
    };
    let dir = dir(&database);
    std::fs::create_dir_all(&dir).map_err(|e| failed(&dir, &e))?;
    let mut tries = 0;
    loop {
        let next = copies(&dir).last().and_then(|last| number(last)).map_or(1, |n| n + 1);
        let to = dir.join(format!("{BEFORE_MIGRATION}{next:06}-v{from}.sqlite"));
        let Err(refused) = db.execute("VACUUM INTO ?1", [to.to_string_lossy()]) else {
            break;
        };
        // Another process that opens the same database may have migrated it meanwhile, and then no copy is needed; or
        // it took this name for its own copy, which SQLite does not write over, and the name after it is tried.
        tries += 1;
        if crate::version(db)? != Some(from) {
            return Ok(());
        }
        if tries == TRIES || !to.exists() {
            return Err(refused.into());
        }
    }
    let all = copies(&dir);
    for old in &all[..all.len().saturating_sub(KEEP)] {
        std::fs::remove_file(old).map_err(|e| failed(old, &e))?;
    }
    Ok(())
}

fn failed(at: &Path, e: &std::io::Error) -> Fault {
    Fault::Database(format!("the copy before the migration, {}: {e}", at.display()))
}
