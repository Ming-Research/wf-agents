//! Copies of the database, kept beside it under `backups/`: one before every migration, so that a migration that goes
//! wrong loses nothing, and the engine's daily ones. Each is the whole database, consistent as it stood (SQLite's
//! `VACUUM INTO`, which reads it as one snapshot while writers go on), named by its kind and when it was taken, so that
//! the names sort oldest first.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags};

use crate::{Error, Result, now};

/// The kind of copy taken before a migration.
pub const BEFORE_MIGRATION: &str = "before-migration";

/// The kind of copy the engine takes once a day while it runs.
pub const DAILY: &str = "daily";

/// How many copies taken before a migration are kept, the newest.
pub const KEEP_BEFORE_MIGRATION: usize = 7;

/// The directory a database's copies are kept in: `backups/` beside it.
pub fn dir(database: &Path) -> PathBuf {
    database.parent().unwrap_or_else(|| Path::new(".")).join("backups")
}

/// Copies the database `conn` has open, whole and as one snapshot, to a new file of the kind `kind` in `dir`, which is
/// made when missing; `note` goes into its name after when it was taken. Gives the copy's path.
pub fn copy(conn: &Connection, dir: &Path, kind: &str, note: &str) -> Result<PathBuf> {
    std::fs::create_dir_all(dir).map_err(|e| Error::Abort(format!("{}: {e}", dir.display())))?;
    let to = dir.join(format!("{kind}-{}{note}.sqlite", now()));
    conn.execute("VACUUM INTO ?1", [to.to_string_lossy()])?;
    Ok(to)
}

/// Copies the database at `database` as [`copy`] does, on a connection of its own that only reads it, so that the
/// engine's own connection is not held while the copy is written.
pub fn copy_file(database: &Path, kind: &str) -> Result<PathBuf> {
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let conn = Connection::open_with_flags(database, flags)?;
    copy(&conn, &dir(database), kind, "")
}

/// The copies of the kind `kind` in `dir`, oldest first.
pub fn copies(dir: &Path, kind: &str) -> Vec<PathBuf> {
    let prefix = format!("{kind}-");
    let mut found: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n.starts_with(&prefix) && n.ends_with(".sqlite"))
                })
                .collect()
        })
        .unwrap_or_default();
    found.sort();
    found
}

/// When the newest copy of the kind `kind` in `dir` was taken, in milliseconds; none when there is none.
pub fn newest(dir: &Path, kind: &str) -> Option<i64> {
    let last = copies(dir, kind).pop()?;
    let name = last.file_name()?.to_str()?;
    name.strip_prefix(&format!("{kind}-"))?
        .split(|c: char| !c.is_ascii_digit())
        .next()?
        .parse()
        .ok()
}

/// Keeps the `keep` newest copies of the kind `kind` in `dir`, removing the older ones.
pub fn prune(dir: &Path, kind: &str, keep: usize) -> Result<()> {
    let all = copies(dir, kind);
    let gone = all.len().saturating_sub(keep);
    for p in &all[..gone] {
        std::fs::remove_file(p).map_err(|e| Error::Abort(format!("{}: {e}", p.display())))?;
    }
    Ok(())
}

/// Copies the database `conn` has open before it is migrated from the version `at`, keeping the newest such copies;
/// nothing for a database with no file, such as one in memory.
pub(crate) fn before_migration(conn: &Connection, at: i64) -> Result<()> {
    let Some(path) = conn.path().filter(|p| !p.is_empty()).map(PathBuf::from) else {
        return Ok(());
    };
    let dir = dir(&path);
    copy(conn, &dir, BEFORE_MIGRATION, &format!("-v{at}"))?;
    prune(&dir, BEFORE_MIGRATION, KEEP_BEFORE_MIGRATION)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copies_are_listed_oldest_first_and_pruned_to_the_newest() {
        let dir = std::env::temp_dir().join(format!("wf-agents-backups-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for (kind, at) in [(DAILY, 3), (DAILY, 1), (DAILY, 2), (BEFORE_MIGRATION, 5)] {
            std::fs::write(dir.join(format!("{kind}-100000000000{at}.sqlite")), "").unwrap();
        }
        std::fs::write(dir.join("daily-notes.txt"), "").unwrap();
        let names = |kind: &str| -> Vec<String> {
            copies(&dir, kind)
                .iter()
                .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
                .collect()
        };
        assert_eq!(
            names(DAILY),
            [
                "daily-1000000000001.sqlite",
                "daily-1000000000002.sqlite",
                "daily-1000000000003.sqlite"
            ]
        );
        assert_eq!(newest(&dir, DAILY), Some(1_000_000_000_003));
        prune(&dir, DAILY, 2).unwrap();
        assert_eq!(
            names(DAILY),
            ["daily-1000000000002.sqlite", "daily-1000000000003.sqlite"]
        );
        assert_eq!(names(BEFORE_MIGRATION).len(), 1);
        assert_eq!(newest(&dir, "none"), None);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    // A daily copy of a database file is the whole database as it stood, read on a connection of its own while the
    // database stays open for writing.
    #[test]
    fn a_daily_copy_holds_the_database_as_it_stood() {
        let home = std::env::temp_dir().join(format!("wf-agents-daily-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        let database = home.join("wf-agents.sqlite");
        let writer = Connection::open(&database).unwrap();
        writer
            .execute_batch("PRAGMA journal_mode = WAL; CREATE TABLE t (x); INSERT INTO t VALUES (1), (2);")
            .unwrap();
        let copy = copy_file(&database, DAILY).unwrap();
        assert_eq!(copies(&dir(&database), DAILY), [copy.clone()]);
        let read = Connection::open(&copy).unwrap();
        let n: i64 = read.query_row("SELECT count(*) FROM t", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 2);
        drop((read, writer));
        std::fs::remove_dir_all(&home).unwrap();
    }
}
