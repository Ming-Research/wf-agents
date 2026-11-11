//! The place off the machine the copies go to: an interface (principle `outside-behind-interfaces`). The real one, on
//! GitHub with the `gh` command line, is in the crate `carry`, beside the runner of `gh` it shares with the host; this
//! crate holds no process code.

use std::future::Future;
use std::path::Path;

/// The release of the private repository whose assets the copies are.
pub const RELEASE: &str = "database-backups";

/// A place off the machine. A call that fails says why in words; none of them panics, and each one ends, as an answer or
/// as a failure, whatever the place does. Its methods are used as type parameters, never as objects.
pub trait Offsite: Send + Sync {
    /// Whether the place is private. Asked before every upload: a repository can be made public after it was set.
    fn private(&self) -> impl Future<Output = Result<bool, String>> + Send;

    /// Puts the file there, as an asset named as the file is, over one of the same name.
    fn upload(&self, file: &Path) -> impl Future<Output = Result<(), String>> + Send;

    /// The names of the assets there; a place with no release yet holds none, and one that cannot be reached is an error
    /// and never an empty place.
    fn list(&self) -> impl Future<Output = Result<Vec<String>, String>> + Send;

    /// Takes the asset of this name away.
    fn remove(&self, name: &str) -> impl Future<Output = Result<(), String>> + Send;
}

/// The place of an installation that gives none: the copies stay on the machine, and this is never asked.
pub struct Nowhere;

impl Offsite for Nowhere {
    async fn private(&self) -> Result<bool, String> {
        Err(NONE.to_string())
    }

    async fn upload(&self, _file: &Path) -> Result<(), String> {
        Err(NONE.to_string())
    }

    async fn list(&self) -> Result<Vec<String>, String> {
        Err(NONE.to_string())
    }

    async fn remove(&self, _name: &str) -> Result<(), String> {
        Err(NONE.to_string())
    }
}

const NONE: &str = "no place off the machine is given";
