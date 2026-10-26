//! The lock that keeps one engine on a data directory. A second engine takes the same works, so a round could run
//! twice: two agents in one worktree, two returns for one cell. The lock is the operating system's, on a file in the
//! data directory, so it goes with the process however the process ends and a crash leaves nothing to judge stale.
//! The file also says who took the lock, for the message that refuses a second engine.

use std::fmt;
use std::fs::{File, OpenOptions, TryLockError};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

/// The most of the lock file that is read to see who holds it.
const READ_LIMIT: u64 = 256;

/// The lock of a data directory, held for as long as the value lives. An [`crate::Engine`] needs one and takes its
/// data directory from it, so an engine cannot work on a directory whose lock it does not hold.
#[derive(Debug)]
pub struct Lock {
    dir: PathBuf,
    // Never read: the open file is what holds the lock.
    _file: File,
}

/// Who a lock file says holds the lock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Holder {
    pub pid: u32,
    /// When the holder took the lock, in UTC milliseconds since the Unix epoch.
    pub since: i64,
}

/// Why a lock was not taken.
#[derive(Debug)]
pub enum Refused {
    /// Another process holds it. The holder is what the lock file says; none when it says nothing that reads as one,
    /// which is also the case in the moment between the holder taking the lock and writing it.
    Held { path: PathBuf, holder: Option<Holder> },
    /// The lock file cannot be opened or locked, or written once locked; no engine runs without the lock.
    Unusable { path: PathBuf, error: io::Error },
}

impl fmt::Display for Refused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refused::Held { path, holder: Some(h) } => write!(
                f,
                "engine already running: process {} holds {}, taken {} seconds ago",
                h.pid,
                path.display(),
                (db::now() - h.since).max(0) / 1000
            ),
            Refused::Held { path, holder: None } => write!(
                f,
                "another process holds {} and has not recorded who it is",
                path.display()
            ),
            Refused::Unusable { path, error } => write!(f, "{}: {error}", path.display()),
        }
    }
}

impl std::error::Error for Refused {}

impl Lock {
    /// Takes the lock of the data directory, or says why not. The file is written with this process and the time
    /// once the lock is held.
    pub fn acquire(dir: &Path) -> Result<Lock, Refused> {
        let path = config::engine_lock(dir);
        let unusable = |error| Refused::Unusable {
            path: path.clone(),
            error,
        };
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .map_err(unusable)?;
        match file.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => {
                return Err(Refused::Held {
                    holder: holder(&mut file),
                    path: path.clone(),
                });
            }
            Err(TryLockError::Error(error)) => return Err(unusable(error)),
        }
        file.set_len(0).map_err(unusable)?;
        file.write_all(format!("{} {}\n", std::process::id(), db::now()).as_bytes())
            .map_err(unusable)?;
        Ok(Lock {
            dir: dir.to_path_buf(),
            _file: file,
        })
    }

    /// The data directory the lock is on.
    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

/// The holder a lock file names: exactly a process number and a time, on one line.
fn holder(file: &mut File) -> Option<Holder> {
    let mut text = Vec::new();
    Read::take(file, READ_LIMIT).read_to_end(&mut text).ok()?;
    let text = std::str::from_utf8(&text).ok()?;
    let mut words = text.strip_suffix('\n')?.split(' ');
    let pid = words.next()?.parse().ok()?;
    let since = words.next()?.parse().ok()?;
    words.next().is_none().then_some(Holder { pid, since })
}
