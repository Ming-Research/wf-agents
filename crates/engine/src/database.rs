//! The database of a data directory: where it is, how it is opened and how it is made. The owner's commands and the
//! engine all get it from here, so a missing database is told the same way everywhere, and only [`create`] ever
//! makes one: a place that has none is a mistake until someone says it is the first use.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use db::{Db, Error};

/// Opens the database in the data directory `data`. It makes nothing: when there is no database, the error names
/// where it looked and the command that makes one.
pub fn open(data: &Path) -> Result<Db, String> {
    let path = config::database(data);
    Db::open(&path).map_err(|e| told(&path, e))
}

/// Opens the database in the data directory `data` only to read it, as it is. It makes nothing, and a missing
/// database is told as [`open`] tells it.
pub fn open_read_only(data: &Path) -> Result<Db, String> {
    let path = config::database(data);
    Db::open_read_only(&path).map_err(|e| told(&path, e))
}

/// What a failure to open the database at `path` is told as: a missing database says where it was looked for and
/// which command makes one.
fn told(path: &Path, e: Error) -> String {
    match e {
        Error::Missing => format!(
            "{}: no database here; if this is the right data directory, `wf-agents init` makes one",
            path.display()
        ),
        e => format!("{}: {e}", path.display()),
    }
}

/// Whether the place of the data directory `data`'s database is one [`create`] will refuse: a database is there, or
/// other tables. For a caller that has checks to make before it creates, and can leave them out where it is refused.
pub fn exists(data: &Path) -> bool {
    db::is_taken(&config::database(data))
}

/// Makes the database in the data directory `data`, with its `db/` directory when that is missing, and gives its
/// path. The data directory itself must exist, and a database that is already there is not touched.
pub fn create(data: &Path) -> Result<PathBuf, String> {
    let path = config::database(data);
    if let Some(dir) = path.parent() {
        match std::fs::create_dir(dir) {
            Ok(()) => {}
            Err(e) if e.kind() == ErrorKind::AlreadyExists => {}
            Err(e) => return Err(format!("{}: {e}", dir.display())),
        }
    }
    Db::create(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(path)
}
