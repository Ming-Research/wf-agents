//! The host of the project's repository: where the head of its main branch is, what gives the verdict on a commit of
//! a work's branch, and how the commit the check passed is merged. The rest of the acts (a work's branch and
//! worktree, the repository's configuration, a conflict with main) is done in the project's local repository whatever
//! the host, and the installation's settings say which host it is.
//!
//! - [`Local`]: the local repository is the host: its own main branch, the project's check command, and a merge in its
//!   checkout.
//! - [`GitHub`]: the repository on GitHub is the host: its main branch, the CI of the work's pull request, and the
//!   pull request's merge.
//! - `fake`: a host for the tests, which gives what it is told to.
//!
//! Each operation can be asked again after the engine started again, and finds what it did already rather than doing
//! it a second time.

pub mod github;
mod local;

#[cfg(test)]
pub(crate) mod fake;

use std::future::Future;

use store::WorkNumber;

use crate::git::Place;
use crate::held::Stop;

pub use github::GitHub;
pub use local::Local;

/// The work's branch, as a host is given it: with the work and the title of its item, which a host may show where it
/// shows the branch.
#[derive(Clone, Copy, Debug)]
pub struct Branch<'a> {
    pub work: WorkNumber,
    pub title: &'a str,
    pub place: &'a Place,
}

/// Why the commit the check passed is not merged onto main's head: it does not hold it, so it was checked against a
/// main that has since moved, and merged, it would land what nothing checked. Said with what the owner does next.
pub(crate) fn moved(name: &str, main: &str, checked: &str) -> String {
    format!(
        "{name} has moved to {main} since the check passed {checked}, which does not hold it. The engine writes \
         {name}'s new head into the work within a minute; let the work on then, and the check runs again before the \
         merge"
    )
}

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
        branch: &Branch<'_>,
        commit: &str,
        stop: &mut Stop,
    ) -> impl Future<Output = Result<Option<Verdict>, String>> + Send;

    /// Merges the work's branch into main and gives the commit the merge made main's head with. Where `checked` names
    /// the commit the check passed, only that commit is merged, and only onto a main it holds ([`moved`]); a merge
    /// made already, before the engine stopped, is found first and gives its commit. With none, the flow has no check,
    /// and what the branch holds is merged.
    fn merge(&self, branch: &Branch<'_>, checked: Option<&str>) -> impl Future<Output = Result<String, String>> + Send;
}
