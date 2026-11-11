//! The host of the project's repository: where the head of its main branch is, what gives the verdict on a commit of
//! a work's branch, how the commit the check passed is merged, how the agents' tools `check` and `check_before`
//! check their work, and what it lets go of once a work has ended. The rest of the acts (a work's branch and worktree, the repository's configuration, a conflict
//! with main) is done in the project's local repository whatever the host, and the installation's settings say which
//! host it is.
//!
//! - [`Local`]: the local repository is the host: its own main branch, the project's check command, and a merge in its
//!   checkout; an agent's check runs the command on the worktree as it stands.
//! - [`GitHub`]: the repository on GitHub is the host: its main branch, the CI of the work's pull request, and the
//!   pull request's merge; an agent's check is the CI of the last commit of its work's branch, which it pushes.
//! - `fake`: a host for the tests, which gives what it is told to.
//!
//! Each operation can be asked again after the engine started again, and finds what it did already rather than doing
//! it a second time.

pub mod github;
mod local;

#[cfg(test)]
pub(crate) mod fake;

use std::collections::BTreeMap;
use std::future::Future;
use std::sync::Arc;

use store::WorkNumber;

use crate::git::{Before, Place};
use crate::held::Stop;

pub use github::GitHub;
pub use local::Local;

/// The work's branch, as a host is given it: with the work, the title of its item and what the work says of itself,
/// which a host may show where it shows the branch.
#[derive(Clone, Copy, Debug)]
pub struct Branch<'a> {
    pub work: WorkNumber,
    pub title: &'a str,
    /// What the work wrote of itself where the flow's check names it, in the flow's order; empty where it names nothing
    /// or nothing is written yet.
    pub about: &'a str,
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
    /// What the host says of the checks beside the verdict, each in words and with no judgment of its own: a check run
    /// more than once on the commit, with each run's result.
    pub notes: Vec<String>,
}

/// Where the project's repository is hosted. A failure of the host itself (it cannot be reached, it refuses, it does
/// not answer in time) is an `Err` with its reason, which fails the act's round: it says nothing of the work's code.
pub trait Host: Send + Sync + 'static {
    /// The head of the main branch, brought into the local repository where it is kept elsewhere: what a work starts
    /// from and is checked against.
    fn main(&self) -> impl Future<Output = Result<String, String>> + Send;

    /// The verdict on the commit, the head of the work's branch, which is committed and merges into main without a
    /// conflict; none where it was told to stop. A named check red on the commit fails it, whatever else is red, and
    /// nothing is run again.
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

    /// The reference of the local repository that holds the head of main as the host last gave it, read without asking
    /// the host again: where a work's change begins is worked out from it.
    fn kept_main(&self) -> String;

    /// What the agents' tool `check` finds of the work, in words for the agent that asked; none where it was told to
    /// stop. An `Err` is why it could not check, for the agent too. What failed is said as [`Host::verdict`] says it.
    fn check(
        &self,
        branch: &Branch<'_>,
        stop: &mut Stop,
    ) -> impl Future<Output = Result<Option<String>, String>> + Send;

    /// What the agents' tool `check_before` finds of the commit made of the code before the work's change with some
    /// of its files put on it, in words for the agent that asked, as [`Host::check`] gives them.
    fn check_before(
        &self,
        branch: &Branch<'_>,
        before: &Before,
        stop: &mut Stop,
    ) -> impl Future<Output = Result<Option<String>, String>> + Send;

    /// Lets go of what the host holds for the work, which has ended with nothing of it under way: the branches of its
    /// checks before the change, and its pull request where it did not merge, closed with a word that its work ended
    /// ([`CLOSED`]). What is gone, closed or merged already is left as it is, and the work's own branch stays.
    fn end(&self, branch: &Branch<'_>, merged: bool) -> impl Future<Output = Result<(), String>> + Send;
}

/// What the host says on the pull request of a work that ended without merging. It says not why: why is the owner's,
/// in the owner's language, and on the owner's page.
pub const CLOSED: &str = "Its work ended without merging.";

/// The names a host's checks before the change are running under in this process: a worktree or a branch each, named
/// by the work and the commit. A check holds its name while it runs, so that another asked at once for the same commit
/// waits for it, and what a check finds of the work's names that none holds was left by a process of the engine that
/// ended, and is taken away.
#[derive(Clone, Default)]
pub(crate) struct InUse(Arc<std::sync::Mutex<BTreeMap<String, Arc<tokio::sync::Mutex<()>>>>>);

impl std::fmt::Debug for InUse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("InUse")
    }
}

impl InUse {
    fn names(&self) -> std::sync::MutexGuard<'_, BTreeMap<String, Arc<tokio::sync::Mutex<()>>>> {
        self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Holds the name, once no other check holds it, until what is given back is dropped.
    pub(crate) async fn hold(&self, name: &str) -> tokio::sync::OwnedMutexGuard<()> {
        let lock = self.names().entry(name.to_string()).or_default().clone();
        lock.lock_owned().await
    }

    /// Whether a check of this process holds the name.
    pub(crate) fn held(&self, name: &str) -> bool {
        self.names().get(name).is_some_and(|lock| lock.try_lock().is_err())
    }
}

/// A host shared, as the acts and the tools share it.
impl<H: Host> Host for Arc<H> {
    async fn main(&self) -> Result<String, String> {
        (**self).main().await
    }

    async fn verdict(&self, branch: &Branch<'_>, commit: &str, stop: &mut Stop) -> Result<Option<Verdict>, String> {
        (**self).verdict(branch, commit, stop).await
    }

    async fn merge(&self, branch: &Branch<'_>, checked: Option<&str>) -> Result<String, String> {
        (**self).merge(branch, checked).await
    }

    fn kept_main(&self) -> String {
        (**self).kept_main()
    }

    async fn check(&self, branch: &Branch<'_>, stop: &mut Stop) -> Result<Option<String>, String> {
        (**self).check(branch, stop).await
    }

    async fn end(&self, branch: &Branch<'_>, merged: bool) -> Result<(), String> {
        (**self).end(branch, merged).await
    }

    async fn check_before(
        &self,
        branch: &Branch<'_>,
        before: &Before,
        stop: &mut Stop,
    ) -> Result<Option<String>, String> {
        (**self).check_before(branch, before, stop).await
    }
}
