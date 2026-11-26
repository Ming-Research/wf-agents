//! What the GitHub host asks of GitHub, apart from what `git` pushes and fetches: the pull requests of a branch and
//! their text, the merge and the closing of a pull request, and a workflow run by
//! hand with its runs, their jobs, what they uploaded and the log of a job, and the runners the repository's workflows
//! may run on. [`Gh`](super::gh::Gh) asks GitHub with the `gh` command line; the tests ask a fake.

use std::future::Future;

use crate::host::Failed;

/// A pull request, as far as the host reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pull {
    pub number: u64,
    /// Where a person sees it.
    pub url: String,
    pub state: State,
    /// The commit at the head of its branch.
    pub head: String,
    /// The branch it would be merged into.
    pub base: String,
    /// The commit its merge made, once it is merged.
    pub merge_commit: Option<String>,
    pub title: String,
    /// What it says of itself below its title; empty where it says nothing.
    pub body: String,
    /// Whether it is a draft, which is not yet for anyone to review or merge.
    pub draft: bool,
    /// GitHub's id of it among all its objects, by which a draft is made ready.
    pub node: String,
    /// When GitHub last updated it, in milliseconds since 1970.
    pub updated: u64,
    /// The branch its head is on.
    pub branch: String,
    /// Whether its head is on a branch of another repository, a fork, or of none that GitHub still has.
    pub from_fork: bool,
}

/// The most open pull requests of a repository [`Api::open_pulls`] reads.
pub const OPEN_PULLS: usize = 1000;

/// Whether a pull request is open, or was closed with or without its merge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Open,
    Closed,
    Merged,
}

/// One job of a run of a workflow, by its name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Check {
    /// GitHub's id of the job, by which its log is read.
    pub id: u64,
    pub name: String,
    pub outcome: Outcome,
    /// Where a person sees it, where it says.
    pub url: Option<String>,
    /// How long it took, in whole seconds, from its start to its end as GitHub records them; only for a job that ran to
    /// a conclusion of passed or failed, and none where GitHub gives no usable start or end.
    pub took: Option<u64>,
}

/// What came of a check.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// It is under way: a runner has taken it, and it is not complete.
    Pending,
    /// It has not started: no runner has taken it yet.
    Queued,
    /// It did not run: its condition was false, or a job it needs did not succeed.
    Skipped,
    /// It passed.
    Passed,
    /// It judged the code and found it wanting, with GitHub's word for how.
    Failed(String),
    /// It ended without judging the code, with GitHub's word for how: it was cancelled, say.
    Broken(String),
}

/// A run of a workflow, as far as the host reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Run {
    /// Its number.
    pub id: u64,
    /// Its name, as the workflow's `run-name` made it.
    pub title: String,
    /// Whether a runner took a job of it: one queued, waiting or pending has not started. A run that has started may
    /// have jobs that wait for a runner still (`Outcome::Queued`).
    pub started: bool,
    /// Whether it has completed.
    pub completed: bool,
    /// Whether it completed cancelled, having judged nothing.
    pub cancelled: bool,
    /// Where a person sees it.
    pub url: String,
}

/// What GitHub answered to a look at the runs of a workflow.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Listed {
    /// The runs, and the entity tag GitHub gave this answer, which a later look sends back; none where it gave none.
    Runs { runs: Vec<Run>, tag: Option<String> },
    /// The runs are as the answer whose entity tag the look sent gave them: GitHub answered 304, which costs nothing
    /// against its limit on requests.
    Unchanged,
}

/// What GitHub answered to a look at the jobs of a run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Jobs {
    /// The jobs, and the entity tag GitHub gave this answer, which a later look sends back; none where it gave none.
    Listed { jobs: Vec<Check>, tag: Option<String> },
    /// The jobs are as the answer whose entity tag the look sent gave them: GitHub answered 304.
    Unchanged,
}

/// A self-hosted runner, as far as the host reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Runner {
    /// Whether it is connected to GitHub, busy or not.
    pub online: bool,
    /// The labels a workflow's `runs-on` names it by.
    pub labels: Vec<String>,
}

/// The runners one layer lists, the repository's or its organisation's, or GitHub's words for why it could not be read:
/// a token that may not read them is refused.
pub type Layer = Result<Vec<Runner>, String>;

/// The runners a repository's workflows may run on, as each layer they are registered at lists them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Runners {
    /// The runners registered for the repository itself.
    pub repository: Layer,
    /// The runners registered for the organisation that owns the repository; none where its owner is no organisation.
    pub organisation: Option<Layer>,
}

/// A file a run uploaded: its path in the artifact, with `/` between its parts, and its bytes.
pub type File = (String, Vec<u8>);

/// What the GitHub host asks of GitHub, in the repository it is given. An `Err` is GitHub's refusal or failure, in
/// words that say which, of the kind the status of GitHub's answer tells: a fault of GitHub's or its limit on requests
/// passes, any other refusal does not; none where GitHub gave no answer.
pub trait Api: Send + Sync + 'static {
    /// The pull requests whose head is this branch of the repository, open or not.
    fn pulls(&self, branch: &str) -> impl Future<Output = Result<Vec<Pull>, Failed>> + Send;

    /// Every pull request open in the repository, whatever its branch, each page of GitHub's list read; it is an `Err`
    /// where the repository holds more than [`OPEN_PULLS`] of them, which a list cut short would hide.
    fn open_pulls(&self) -> impl Future<Output = Result<Vec<Pull>, Failed>> + Send;
    /// The pull request of this number; none where the repository has none so numbered.
    fn pull(&self, number: u64) -> impl Future<Output = Result<Option<Pull>, Failed>> + Send;

    /// Opens a pull request of the branch into the base, with this title and body, as a draft.
    fn open(
        &self,
        branch: &str,
        base: &str,
        title: &str,
        body: &str,
    ) -> impl Future<Output = Result<Pull, Failed>> + Send;

    /// Merges the pull request by the method (`squash`, `merge` or `rebase`), provided its head is the commit, the
    /// commit the merge makes titled so and with this message under its title; gives the commit the merge made.
    fn merge(
        &self,
        number: u64,
        head: &str,
        method: &str,
        title: &str,
        message: &str,
    ) -> impl Future<Output = Result<String, Failed>> + Send;

    /// Deletes the branch of the repository; a branch that is not there is no fault.
    fn delete(&self, branch: &str) -> impl Future<Output = Result<(), Failed>> + Send;

    /// The latest runs by hand of the workflow asked for on the branch. Given the entity tag of an earlier answer, it is
    /// [`Listed::Unchanged`] where they are as that answer gave them; given none, it is never that.
    fn runs(
        &self,
        workflow: &str,
        branch: &str,
        tag: Option<&str>,
    ) -> impl Future<Output = Result<Listed, Failed>> + Send;

    /// Runs the workflow by hand on the branch, with these inputs.
    fn dispatch(
        &self,
        workflow: &str,
        branch: &str,
        inputs: &[(String, String)],
    ) -> impl Future<Output = Result<(), Failed>> + Send;

    /// Cancels the run, where it is not complete; one complete already is left as it is.
    fn cancel(&self, run: u64) -> impl Future<Output = Result<(), Failed>> + Send;

    /// The jobs of the run, each as a check. Given the entity tag of an earlier answer, it is [`Jobs::Unchanged`] where
    /// they are as that answer gave them; given none, it is never that.
    fn jobs(&self, run: u64, tag: Option<&str>) -> impl Future<Output = Result<Jobs, Failed>> + Send;

    /// The runners the repository's workflows may run on, as the repository and, where an organisation owns it, the
    /// organisation list them; a layer that cannot be read says why, and does not fail the other.
    fn runners(&self) -> impl Future<Output = Runners> + Send;

    /// The files of the artifact of this name that the run uploaded; none where it uploaded none so named.
    fn artifact(&self, run: u64, name: &str) -> impl Future<Output = Result<Option<Vec<File>>, Failed>> + Send;

    /// The log of the job of this id, as GitHub keeps it: plain text, each line after the time it was written.
    fn log(&self, job: u64) -> impl Future<Output = Result<String, Failed>> + Send;

    /// Gives the pull request this title and body.
    fn edit(&self, number: u64, title: &str, body: &str) -> impl Future<Output = Result<(), Failed>> + Send;

    /// Makes the draft pull request of this id ready to be reviewed and merged.
    fn ready(&self, node: &str) -> impl Future<Output = Result<(), Failed>> + Send;

    /// Closes the pull request, saying this on it first.
    fn close(&self, number: u64, comment: &str) -> impl Future<Output = Result<(), Failed>> + Send;

    /// Opens the closed pull request again.
    fn reopen(&self, number: u64) -> impl Future<Output = Result<(), Failed>> + Send;
}
