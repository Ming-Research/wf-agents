//! What a step stands on: the records of the copies and the files they are in. An interface (principle
//! `outside-behind-interfaces`, which names storage), so that a test can end a step at any write it makes, as a stopped
//! process would, and see that the next step finishes what was left.

use std::path::{Path, PathBuf};

use store::{Backups, Records};
use work::Time;

use crate::snapshot::PARTIAL;
use crate::time_in;

/// Why a call of the ground did not do what it was asked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fail {
    /// The process is stopped here, as a test makes it. It is no failure: nothing is recorded of it.
    Stopped,
    /// It failed, in words.
    Why(String),
}

/// The records of the copies, and the directory their files are in. Each method that writes is a place a stop can be
/// made; a stop leaves the write undone.
pub trait Ground: Send + Sync {
    /// Everything recorded of the copies.
    fn records(&self) -> Result<Records, Fail>;

    /// The names of the files in the directory that are named as the engine names its copies.
    fn files(&self) -> Result<Vec<String>, Fail>;

    /// Where the copy of this name is.
    fn path(&self, name: &str) -> PathBuf;

    /// Removes the files that are not whole: the leavings of a stop in the middle of a copy.
    fn clear_partial(&self) -> Result<(), Fail>;

    /// Makes the copy of this name whole under it, by one rename of the file it was made in.
    fn finish(&self, name: &str) -> Result<(), Fail>;

    /// Removes the file of the copy of this name; whether it was there, a file that is gone being no error.
    fn remove(&self, name: &str) -> Result<bool, Fail>;

    /// Records that the copy of this name was taken.
    fn taken(&self, name: &str, at: Time) -> Result<(), Fail>;

    /// Records that the copy of this name was uploaded.
    fn uploaded(&self, name: &str, at: Time) -> Result<(), Fail>;

    /// Records a failure of a step, about the copy of this name.
    fn failed(&self, at: Time, name: &str, step: &str, why: &str) -> Result<(), Fail>;

    /// Records that a step ended with nothing failing, mending the failures up to the one at this place.
    fn ended(&self, at: Time, mended: u64) -> Result<(), Fail>;
}

/// The real ground: the store's records of the copies and a directory of the machine.
pub struct Disk {
    records: Backups,
    dir: PathBuf,
}

impl Disk {
    /// The ground of the copies in `dir`, recorded in the store at `store`, which the store has opened already.
    pub fn open(store: &Path, dir: &Path) -> Result<Disk, String> {
        let records = Backups::open(store).map_err(|e| format!("{}: {e:?}", store.display()))?;
        Ok(Disk {
            records,
            dir: dir.to_path_buf(),
        })
    }
}

fn why(e: impl std::fmt::Debug) -> Fail {
    Fail::Why(format!("{e:?}"))
}

fn io(path: &Path, e: std::io::Error) -> Fail {
    Fail::Why(format!("{}: {e}", path.display()))
}

impl Ground for Disk {
    fn records(&self) -> Result<Records, Fail> {
        self.records.records().map_err(why)
    }

    fn files(&self) -> Result<Vec<String>, Fail> {
        let entries = match std::fs::read_dir(&self.dir) {
            Ok(entries) => entries,
            // Before the first copy there is no directory.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(io(&self.dir, e)),
        };
        let mut found: Vec<String> = entries
            .flatten()
            .filter_map(|e| e.file_name().to_str().map(str::to_string))
            .filter(|name| time_in(name).is_some())
            .collect();
        found.sort();
        Ok(found)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }

    fn clear_partial(&self) -> Result<(), Fail> {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return Ok(());
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.to_str().is_some_and(|p| p.ends_with(PARTIAL)) {
                std::fs::remove_file(&path).map_err(|e| io(&path, e))?;
            }
        }
        Ok(())
    }

    fn finish(&self, name: &str) -> Result<(), Fail> {
        let to = self.path(name);
        let from = self.dir.join(format!("{name}{PARTIAL}"));
        std::fs::rename(&from, &to).map_err(|e| io(&to, e))
    }

    fn remove(&self, name: &str) -> Result<bool, Fail> {
        let path = self.path(name);
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(io(&path, e)),
        }
    }

    fn taken(&self, name: &str, at: Time) -> Result<(), Fail> {
        self.records.taken(name, at).map_err(why)
    }

    fn uploaded(&self, name: &str, at: Time) -> Result<(), Fail> {
        self.records.uploaded(name, at).map_err(why)
    }

    fn failed(&self, at: Time, name: &str, step: &str, reason: &str) -> Result<(), Fail> {
        self.records.failed(at, name, step, reason).map_err(why)
    }

    fn ended(&self, at: Time, mended: u64) -> Result<(), Fail> {
        self.records.ended(at, mended).map_err(why)
    }
}
