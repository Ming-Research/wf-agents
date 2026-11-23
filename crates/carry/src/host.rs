//! The host of the project's repository: where the head of its main branch is, what gives the verdict on a commit of
//! a work's branch, how the commit the check passed is merged, how the agents' tool `run_tests` runs the tests of
//! their work, and what it lets go of once a work has ended. The rest of the acts (a work's branch and worktree, the
//! repository's configuration, a conflict with main) is done in the project's local repository whatever the host, and the installation's settings say which
//! host it is.
//!
//! - [`Local`]: the local repository is the host: its own main branch, the project's check command, and a merge in its
//!   checkout; an agent's check runs the command on the worktree as it stands.
//! - [`GitHub`]: the repository on GitHub is the host: its main branch, the CI of the work's pull request, and the
//!   pull request's merge; an agent's `run_tests` runs the tests it names by the fast workflow, on the commit it names or
//!   else the last commit of its work's branch, which the engine pushes to a branch of its own.
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

use decide::run::Failure;
use decide::{CallId, RunId};
use store::WorkNumber;
use tokio::sync::Mutex;

use crate::git::Place;
use crate::held::Stop;

pub use github::GitHub;
pub use local::Local;

/// The work's branch, as a host is given it: with the work, the name of its merge and what the work says of itself,
/// which a host may show where it shows the branch. A host is given nothing of the work's item.
#[derive(Clone, Copy, Debug)]
pub struct Branch<'a> {
    pub work: WorkNumber,
    /// The name of the work's merge, which its pull request and the commit its merge makes take
    /// ([`crate::acts::name`]).
    pub title: &'a str,
    /// What the work wrote of itself where the flow's check names it, in the flow's order; empty where it names nothing
    /// or nothing is written yet.
    pub about: &'a str,
    pub place: &'a Place,
}

/// What asks a host to run the project's checks, as the work's records name it: a call of `run_tests`, or a round of
/// the check before the merge. Given again after the engine started again it is the same, and a host that runs the
/// checks elsewhere finds by it what it began for it there rather than beginning it again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Asking {
    /// A call of `run_tests`.
    Call(CallId),
    /// A round of the run of the check, by its run and which round of it it is, the first being one.
    Round(RunId, usize),
}

impl Asking {
    /// Its name among what the work asks: `call/<call>`, or `run/<run>/round/<round>`.
    pub fn name(&self) -> String {
        match self {
            Asking::Call(call) => format!("call/{}", call.0),
            Asking::Round(run, round) => format!("run/{}/round/{round}", run.0),
        }
    }
}

/// Why the host did not do what it was asked, with the kind of its cause where the host's answer, or what the host
/// found, tells it: a fault of the host that passes by itself, or a refusal no wait mends. Where nothing tells it, as
/// where a command gave no answer, the act's words are read for the network's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failed {
    pub why: String,
    pub kind: Option<Failure>,
}

impl Failed {
    /// A failure that passes by itself: what failed is tried again after a wait.
    pub fn passing(why: impl Into<String>) -> Failed {
        Failed {
            why: why.into(),
            kind: Some(Failure::Passing),
        }
    }

    /// A refusal that no wait mends, as a person's doing or what the configuration says: the work waits, with why, for
    /// the owner.
    pub fn refused(why: impl Into<String>) -> Failed {
        Failed {
            why: why.into(),
            kind: Some(Failure::Other),
        }
    }

    /// The same failure, of the same kind, in the words these make of its own.
    pub fn said(self, words: impl FnOnce(String) -> String) -> Failed {
        Failed {
            why: words(self.why),
            kind: self.kind,
        }
    }
}

impl From<String> for Failed {
    fn from(why: String) -> Self {
        Failed { why, kind: None }
    }
}

impl From<&str> for Failed {
    fn from(why: &str) -> Self {
        why.to_string().into()
    }
}

impl From<Failed> for String {
    fn from(failed: Failed) -> Self {
        failed.why
    }
}

impl std::fmt::Display for Failed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.why)
    }
}

/// Why a work's branch was not merged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NotMerged {
    /// Main has moved to this head since the check passed the commit, which does not hold it: merged, it would land
    /// what nothing checked. The check is to be taken again on main's new head.
    Moved { main: String, checked: String },
    /// Anything else, as git or the host said it.
    Failed(Failed),
}

impl NotMerged {
    /// Not merged for a refusal no wait mends ([`Failed::refused`]).
    pub fn refused(why: String) -> NotMerged {
        NotMerged::Failed(Failed::refused(why))
    }
}

impl From<String> for NotMerged {
    fn from(why: String) -> Self {
        NotMerged::Failed(why.into())
    }
}

impl From<Failed> for NotMerged {
    fn from(failed: Failed) -> Self {
        NotMerged::Failed(failed)
    }
}

/// What a host found of a commit.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Verdict {
    /// What failed, in words the work's implementing agent can act on; none where everything passed, or where what
    /// failed fails on the head of main as well ([`Verdict::held`]).
    pub failed: Option<String>,
    /// What failed on the commit and fails on the head of main it was checked against as well, in words: the commit
    /// neither passes nor fails by its own change, and waits for main to pass it. None where nothing does.
    pub held: Option<String>,
    /// What judged the commit, as words that follow "checked by", where the host says it; with none, the check is told
    /// by what it was checked against alone.
    pub by: Option<String>,
    /// What the host says of the checks beside the verdict, each in words and with no judgment of its own: a check run
    /// more than once on the commit, with each run's result; and the jobs of the runs that are not said as what failed,
    /// each with how it ended and its wall time.
    pub notes: Vec<String>,
    /// What the host found beyond the work, each a text worth an item of its own: a test that failed on the commit and
    /// passed when it was run on it again.
    pub observations: Vec<String>,
}

/// Where the project's repository is hosted. A failure of the host itself (it cannot be reached, it refuses, it does
/// not answer in time) is an `Err` with its reason, which fails the act's round: it says nothing of the work's code.
pub trait Host: Send + Sync + 'static {
    /// The head of the main branch, brought into the local repository where it is kept elsewhere: what a work starts
    /// from and is checked against.
    fn main(&self) -> impl Future<Output = Result<String, String>> + Send;

    /// The verdict on the commit, the head of the work's branch, which is committed and merges into main without a
    /// conflict and holds the head of main `main`, as the round of the check that asks finds it; none where it was told
    /// to stop. A failure fails it, whatever else passes, but where the host runs tests by name and only tests failed:
    /// those are run on the commit again, a test that passes there no longer failing it and being observed, and what
    /// fails again is run on `main`, a commit whose every such test fails there too being held
    /// ([`Verdict::held`]). Checks that could not be run fail the round, with the kind of the cause where the host
    /// tells it ([`Failed`]). `repository` is the lock the acts hold while they work in the project's repository: a
    /// host whose verdict works there holds it for that work alone, never while it waits for the verdict.
    fn verdict(
        &self,
        branch: &Branch<'_>,
        commit: &str,
        main: &str,
        asking: Asking,
        repository: &Mutex<()>,
        stop: &mut Stop,
    ) -> impl Future<Output = Result<Option<Verdict>, Failed>> + Send;

    /// Merges the work's branch into main, by a commit named by the branch's title, and gives the commit the merge made
    /// main's head with. Where `checked` names the commit the check passed, only that commit is merged, and only onto
    /// a main it holds ([`NotMerged::Moved`]); a merge made already, before the engine stopped, is found first and
    /// gives its commit. With none, the flow has no check, and what the branch holds is merged.
    fn merge(
        &self,
        branch: &Branch<'_>,
        checked: Option<&str>,
    ) -> impl Future<Output = Result<String, NotMerged>> + Send;

    /// The reference of the local repository that holds the head of main as the host last gave it, read without asking
    /// the host again: where a work's change begins is worked out from it.
    fn kept_main(&self) -> String;

    /// What the agents' tool `run_tests` finds of the work, with the tests and checks the agent named, on the commit it
    /// named or else the work's last commit, in words for the agent whose call asks, whole; none where it was told to
    /// stop. A request that names neither a test nor a check is run as nothing, and said so. An `Err` is why it could not
    /// run them, for the agent too. What failed is said as [`Host::verdict`] says it.
    fn run_tests(
        &self,
        branch: &Branch<'_>,
        named: &Named,
        asking: Asking,
        stop: &mut Stop,
    ) -> impl Future<Output = Result<Option<String>, String>> + Send;

    /// Lets go of what the host holds for the work, which has ended with nothing of it under way: its pull request
    /// where it did not merge, closed with a word that its work ended, as the words beside the flows say it
    /// (`agents/pull_request.toml`), and, merged or not, the branches that calls of [`Host::run_tests`] left
    /// (`wf-try/<the work's branch>/…`), deleted after the pull request is closed, whether or not that went well. What
    /// is gone, closed or merged already is left as it is, and the work's own branch stays.
    fn end(&self, branch: &Branch<'_>, merged: bool) -> impl Future<Output = Result<(), String>> + Send;

    /// Takes up again what the host held for a work that was cancelled and reopened, as the work makes its place
    /// again on the branch it kept: its pull request into main, which its cancel closed, is opened again, where it has
    /// one closed and not merged and none open.
    fn reopen(&self, branch: &Branch<'_>) -> impl Future<Output = Result<(), String>> + Send;

    /// The address of the pull request of this number on the host, worked out without asking it; none where the host
    /// has no pull requests, as the local repository.
    fn pull_at(&self, number: u64) -> Option<String>;
}

/// The tests and the checks an agent names to `run_tests`, each as it wrote it, and the commit it names to run them on.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Named {
    pub tests: Vec<String>,
    pub checks: Vec<String>,
    /// The commit to run on, as git names one in the agent's worktree; none for the last commit of the work's branch.
    pub commit: Option<String>,
}

impl Named {
    /// Whether neither a test nor a check is named, which a commit alone does not make a request to run anything.
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

/// A host shared, as the acts and the tools share it.
impl<H: Host> Host for Arc<H> {
    async fn main(&self) -> Result<String, String> {
        (**self).main().await
    }

    async fn verdict(
        &self,
        branch: &Branch<'_>,
        commit: &str,
        main: &str,
        asking: Asking,
        repository: &Mutex<()>,
        stop: &mut Stop,
    ) -> Result<Option<Verdict>, Failed> {
        (**self).verdict(branch, commit, main, asking, repository, stop).await
    }

    async fn merge(&self, branch: &Branch<'_>, checked: Option<&str>) -> Result<String, NotMerged> {
        (**self).merge(branch, checked).await
    }

    fn kept_main(&self) -> String {
        (**self).kept_main()
    }

    async fn run_tests(
        &self,
        branch: &Branch<'_>,
        named: &Named,
        asking: Asking,
        stop: &mut Stop,
    ) -> Result<Option<String>, String> {
        (**self).run_tests(branch, named, asking, stop).await
    }

    async fn end(&self, branch: &Branch<'_>, merged: bool) -> Result<(), String> {
        (**self).end(branch, merged).await
    }

    async fn reopen(&self, branch: &Branch<'_>) -> Result<(), String> {
        (**self).reopen(branch).await
    }

    fn pull_at(&self, number: u64) -> Option<String> {
        (**self).pull_at(number)
    }
}
