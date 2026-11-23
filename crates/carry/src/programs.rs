//! The programs the engine starts as children, and what they are given of the engine's environment.
//!
//! A child given an environment of its own and named without a path is started by `fork` and `exec`, since the
//! standard library must look the name up on the child's own `PATH`; on macOS each `fork` takes the allocator's locks in
//! every thread of the engine. So a program named without a path is started by the path its name has on the engine's
//! `PATH`, found once, which the children are given unchanged ([`cli::KEPT`]); the standard library then starts it with
//! `posix_spawn`. The child is given the name as its first argument, as before. The engine's environment is read once,
//! as it is first asked for: nothing in the engine changes it.

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock, PoisonError};

use tokio::process::Command;

use crate::cli;

/// The engine's environment, read once.
pub(crate) fn engine() -> &'static [(String, String)] {
    static ENGINE: OnceLock<Vec<(String, String)>> = OnceLock::new();
    ENGINE.get_or_init(|| std::env::vars().collect())
}

/// What a child is given of the engine's environment where it is given no more: [`cli::KEPT`] and every `LC_*`.
pub(crate) fn kept() -> &'static [(String, String)] {
    static KEPT: OnceLock<Vec<(String, String)>> = OnceLock::new();
    KEPT.get_or_init(|| {
        (engine().iter())
            .filter(|(name, _)| cli::KEPT.contains(&name.as_str()) || name.starts_with("LC_"))
            .cloned()
            .collect()
    })
}

/// The program of this name as a child, started by its path ([`path`]) and given the name as its first argument.
pub(crate) fn command(name: &str) -> Command {
    let mut command = Command::new(path(name));
    command.arg0(name);
    command
}

/// The paths found so far, by the names they were found for.
static FOUND: Mutex<BTreeMap<String, PathBuf>> = Mutex::new(BTreeMap::new());

/// The path the program of this name is started by: the name itself where it is a path; else the first file of the
/// name on the engine's `PATH` that may be run, found once; else the name, which the system looks up as it starts the
/// child, as where `PATH` names a directory by a relative path.
pub(crate) fn path(name: &str) -> PathBuf {
    let mut found = FOUND.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(path) = found.get(name) {
        return path.clone();
    }
    let on = engine().iter().find(|(variable, _)| variable == "PATH");
    match find(name, on.map(|(_, value)| value.as_str())) {
        Some(path) => {
            found.insert(name.to_string(), path.clone());
            path
        }
        None => PathBuf::from(name),
    }
}

/// The name a child's program was asked for by, to say in words: the name a path was found for ([`path`]), or the
/// program as it is.
pub(crate) fn named(program: &OsStr) -> String {
    let found = FOUND.lock().unwrap_or_else(PoisonError::into_inner);
    match found.iter().find(|(_, path)| path.as_os_str() == program) {
        Some((name, _)) => name.clone(),
        None => program.to_string_lossy().into_owned(),
    }
}

/// The first file of the name that may be run in the directories `path` names in turn, as the system looks a program
/// up; none where the name is a path, where none is found, or where a directory named by a relative path or by
/// nothing comes before it, which the system reads against the child's own directory.
fn find(name: &str, path: Option<&str>) -> Option<PathBuf> {
    if name.is_empty() || name.contains('/') {
        return None;
    }
    for dir in path?.split(':') {
        let dir = Path::new(dir);
        if !dir.is_absolute() {
            return None;
        }
        let file = dir.join(name);
        if runnable(&file) {
            return Some(file);
        }
    }
    None
}

/// Whether the path leads to a file someone may run.
fn runnable(file: &Path) -> bool {
    std::fs::metadata(file).is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

#[cfg(test)]
mod tests;
