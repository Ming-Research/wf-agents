//! What the GitHub host asks of GitHub, apart from what `git` pushes and fetches: the pull requests of a branch and
//! their text, the merge and the closing of a pull request, and a workflow run by
//! hand with its runs, their jobs and what they uploaded. [`Gh`](super::gh::Gh) asks GitHub with the
//! `gh` command line; the tests ask a fake.

use std::future::Future;

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
}

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
    /// It is not complete.
    Pending,
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
    /// Whether a runner took it: one queued, waiting or pending has not started.
    pub started: bool,
    /// Whether it has completed.
    pub completed: bool,
    /// Where a person sees it.
    pub url: String,
}

/// A file a run uploaded: its path in the artifact, with `/` between its parts, and its bytes.
pub type File = (String, Vec<u8>);

/// What the GitHub host asks of GitHub, in the repository it is given. An `Err` is GitHub's refusal or failure, in
/// words that say which.
pub trait Api: Send + Sync + 'static {
    /// The pull requests whose head is this branch of the repository, open or not.
    fn pulls(&self, branch: &str) -> impl Future<Output = Result<Vec<Pull>, String>> + Send;

    /// Opens a pull request of the branch into the base, with this title and body, as a draft.
    fn open(
        &self,
        branch: &str,
        base: &str,
        title: &str,
        body: &str,
    ) -> impl Future<Output = Result<Pull, String>> + Send;

    /// Merges the pull request by the method (`squash`, `merge` or `rebase`), provided its head is the commit; gives
    /// the commit the merge made.
    fn merge(&self, number: u64, head: &str, method: &str) -> impl Future<Output = Result<String, String>> + Send;

    /// Deletes the branch of the repository; a branch that is not there is no fault.
    fn delete(&self, branch: &str) -> impl Future<Output = Result<(), String>> + Send;

    /// The latest runs by hand of the workflow asked for on the branch.
    fn runs(&self, workflow: &str, branch: &str) -> impl Future<Output = Result<Vec<Run>, String>> + Send;

    /// Runs the workflow by hand on the branch, with these inputs.
    fn dispatch(
        &self,
        workflow: &str,
        branch: &str,
        inputs: &[(String, String)],
    ) -> impl Future<Output = Result<(), String>> + Send;

    /// The jobs of the run, each as a check.
    fn jobs(&self, run: u64) -> impl Future<Output = Result<Vec<Check>, String>> + Send;

    /// The files of the artifact of this name that the run uploaded; none where it uploaded none so named.
    fn artifact(&self, run: u64, name: &str) -> impl Future<Output = Result<Option<Vec<File>>, String>> + Send;

    /// Gives the pull request this title and body.
    fn edit(&self, number: u64, title: &str, body: &str) -> impl Future<Output = Result<(), String>> + Send;

    /// Makes the draft pull request of this id ready to be reviewed and merged.
    fn ready(&self, node: &str) -> impl Future<Output = Result<(), String>> + Send;

    /// Closes the pull request, saying this on it first.
    fn close(&self, number: u64, comment: &str) -> impl Future<Output = Result<(), String>> + Send;

    /// Opens the closed pull request again.
    fn reopen(&self, number: u64) -> impl Future<Output = Result<(), String>> + Send;
}
