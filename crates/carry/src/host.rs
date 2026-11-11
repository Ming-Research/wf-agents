//! The host of the project's repository: where the head of its main branch is, what gives the verdict on a commit of
//! a work's branch, and how the commit the check passed is merged. The rest of the acts (a work's branch and
//! worktree, the repository's configuration, a conflict with main) is done in the project's local repository whatever
//! the host, and the installation's settings say which host it is.
//!
//! - [`Local`]: the local repository is the host: its own main branch, the project's check command, and a merge in its
//!   checkout.
//! - `fake`: a host for the tests, which gives what it is told to.
//!
//! Each operation can be asked again after the engine started again, and finds what it did already rather than doing
//! it a second time.

mod github;
mod local;

#[cfg(test)]
pub(crate) mod fake;

use std::future::Future;

use crate::git::Place;
use crate::held::Stop;

pub use local::Local;

/// What a host found of a commit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Verdict {
    /// What failed, in words the work's implementing agent can act on; none where everything passed.
    pub failed: Option<String>,
    /// What judged the commit, as words that follow "checked by", where the host says it; with none, the check is told
    /// by what it was checked against alone.
    pub by: Option<String>,
}

/// Where the project's repository is hosted. A failure of the host itself (it cannot be reached, it refuses, it does
/// not answer in time) is an `Err` with its reason, which fails the act's round: it says nothing of the work's code.
pub trait Host: Send + Sync + 'static {
    /// The head of the main branch, brought into the local repository where it is kept elsewhere: what a work starts
    /// from and is checked against.
    fn main(&self) -> impl Future<Output = Result<String, String>> + Send;

    /// The verdict on the commit, the head of the work's branch, which is committed and merges into main without a
    /// conflict; none where it was told to stop.
    fn verdict(
        &self,
        place: &Place,
        commit: &str,
        stop: &mut Stop,
    ) -> impl Future<Output = Result<Option<Verdict>, String>> + Send;

    /// Merges the work's branch into main and gives the head of main after it. Where `checked` names the commit the
    /// check passed, only that commit is merged; with none, the flow has no check, and what the branch holds is.
    fn merge(&self, place: &Place, checked: Option<&str>) -> impl Future<Output = Result<String, String>> + Send;
}
