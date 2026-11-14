//! The host of the project's repository: where the head of its main branch is, what gives the verdict on a commit of
//! a work's branch, how the commit the check passed is merged, how the agents' tool `run_tests` runs the tests of
//! their work, and what it lets go of once a work has ended. The rest of the acts (a work's branch and worktree, the
//! repository's configuration, a conflict with main) is done in the project's local repository whatever the host, and the installation's settings say which
//! host it is.
//!
//! - [`Local`]: the local repository is the host: its own main branch, the project's check command, and a merge in its
//!   checkout; an agent's check runs the command on the worktree as it stands.
//! - [`GitHub`]: the repository on GitHub is the host: its main branch, the CI of the work's pull request, and the
//!   pull request's merge; an agent's `run_tests` is the CI of the last commit of its work's branch, which the engine pushes.
//! - `fake`: a host for the tests, which gives what it is told to.
//!
//! Each operation can be asked again after the engine started again, and finds what it did already rather than doing
//! it a second time.

pub mod github;
mod local;

#[cfg(test)]
pub(crate) mod fake;

use std::future::Future;
use std::sync::Arc;

use store::WorkNumber;

use crate::git::Place;
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

/// Why a work's branch was not merged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NotMerged {
    /// Main has moved to this head since the check passed the commit, which does not hold it: merged, it would land
    /// what nothing checked. The check is to be taken again on main's new head.
    Moved { main: String, checked: String },
    /// Anything else, as git or the host said it.
    Failed(String),
}

impl From<String> for NotMerged {
    fn from(why: String) -> Self {
        NotMerged::Failed(why)
    }
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
    /// more than once on the commit, with each run's result; and the jobs of the runs that are not said as what failed,
    /// each with how it ended and its wall time.
    pub notes: Vec<String>,
}

/// Where the project's repository is hosted. A failure of the host itself (it cannot be reached, it refuses, it does
/// not answer in time) is an `Err` with its reason, which fails the act's round: it says nothing of the work's code.
pub trait Host: Send + Sync + 'static {
    /// The head of the main branch, brought into the local repository where it is kept elsewhere: what a work starts
    /// from and is checked against.
    fn main(&self) -> impl Future<Output = Result<String, String>> + Send;

    /// The verdict on the commit, the head of the work's branch, which is committed and merges into main without a
    /// conflict; none where it was told to stop. A failure fails it, whatever else passes, and
    /// nothing is run again.
    fn verdict(
        &self,
        branch: &Branch<'_>,
        commit: &str,
        stop: &mut Stop,
    ) -> impl Future<Output = Result<Option<Verdict>, String>> + Send;

    /// Merges the work's branch into main and gives the commit the merge made main's head with. Where `checked` names
    /// the commit the check passed, only that commit is merged, and only onto a main it holds ([`NotMerged::Moved`]);
    /// a merge made already, before the engine stopped, is found first and gives its commit. With none, the flow has
    /// no check, and what the branch holds is merged.
    fn merge(
        &self,
        branch: &Branch<'_>,
        checked: Option<&str>,
    ) -> impl Future<Output = Result<String, NotMerged>> + Send;

    /// The reference of the local repository that holds the head of main as the host last gave it, read without asking
    /// the host again: where a work's change begins is worked out from it.
    fn kept_main(&self) -> String;

    /// What the agents' tool `run_tests` finds of the work, with the tests and checks the agent named, in words for the
    /// agent that asked, whole; none where it was told to stop. An `Err` is why it could not run them, for the agent
    /// too. What failed is said as [`Host::verdict`] says it.
    fn run_tests(
        &self,
        branch: &Branch<'_>,
        named: &Named,
        stop: &mut Stop,
    ) -> impl Future<Output = Result<Option<String>, String>> + Send;

    /// Lets go of what the host holds for the work, which has ended with nothing of it under way: its pull request
    /// where it did not merge, closed with a word that its work ended ([`CLOSED`]). What is gone, closed or merged
    /// already is left as it is, and the work's own branch stays.
    fn end(&self, branch: &Branch<'_>, merged: bool) -> impl Future<Output = Result<(), String>> + Send;

    /// Takes up again what the host held for a work that was cancelled and reopened, as the work makes its place
    /// again on the branch it kept: its pull request into main, which its cancel closed, is opened again, where it has
    /// one closed and not merged and none open.
    fn reopen(&self, branch: &Branch<'_>) -> impl Future<Output = Result<(), String>> + Send;
}

/// The tests and the checks an agent names to `run_tests`, each as it wrote it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Named {
    pub tests: Vec<String>,
    pub checks: Vec<String>,
}

impl Named {
    /// Whether nothing is named.
    pub fn is_empty(&self) -> bool {
        self.tests.is_empty() && self.checks.is_empty()
    }
}

/// The reference that holds main's head as the host the settings say last gave it, where works branch from: as the
/// host's own [`Host::kept_main`] gives it.
pub(crate) fn kept_main(project: &crate::settings::Project) -> String {
    let main = &project.main;
    match &project.github {
        None => local::kept_main(main),
        Some(github) => github::kept_main(&github.remote, main),
    }
}

/// What the host says on the pull request of a work that ended without merging. It says not why: why is the owner's,
/// in the owner's language, and on the owner's page.
pub const CLOSED: &str = "Its work ended without merging.";

/// A host shared, as the acts and the tools share it.
impl<H: Host> Host for Arc<H> {
    async fn main(&self) -> Result<String, String> {
        (**self).main().await
    }

    async fn verdict(&self, branch: &Branch<'_>, commit: &str, stop: &mut Stop) -> Result<Option<Verdict>, String> {
        (**self).verdict(branch, commit, stop).await
    }

    async fn merge(&self, branch: &Branch<'_>, checked: Option<&str>) -> Result<String, NotMerged> {
        (**self).merge(branch, checked).await
    }

    fn kept_main(&self) -> String {
        (**self).kept_main()
    }

    async fn run_tests(&self, branch: &Branch<'_>, named: &Named, stop: &mut Stop) -> Result<Option<String>, String> {
        (**self).run_tests(branch, named, stop).await
    }

    async fn end(&self, branch: &Branch<'_>, merged: bool) -> Result<(), String> {
        (**self).end(branch, merged).await
    }

    async fn reopen(&self, branch: &Branch<'_>) -> Result<(), String> {
        (**self).reopen(branch).await
    }
}
