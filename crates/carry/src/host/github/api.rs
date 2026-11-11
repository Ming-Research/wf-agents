//! What the GitHub host asks of GitHub, apart from what `git` pushes and fetches: the pull requests of a branch and
//! their text, the checks of a commit, a check's log, and the merge and the closing of a pull
//! request. [`Gh`](super::gh::Gh) asks GitHub with the
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

/// One check of a commit: a check run, or a commit status, by its name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Check {
    pub name: String,
    pub outcome: Outcome,
    /// Where a person sees it, where it says.
    pub url: Option<String>,
    /// What it said of itself, its title and summary, where it said anything.
    pub summary: Option<String>,
    /// The job whose log GitHub keeps, for a check that is a job of GitHub Actions.
    pub job: Option<u64>,
    /// The earlier runs of the check on the same commit, oldest first, each as GitHub says how it ended and where a
    /// person sees it.
    pub earlier: Vec<String>,
}

/// What came of a check.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// It is not complete.
    Pending,
    /// It passed, or had nothing to do.
    Passed,
    /// It judged the code and found it wanting, with GitHub's word for how.
    Failed(String),
    /// It ended without judging the code, with GitHub's word for how: it was cancelled, say.
    Broken(String),
}

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

    /// The checks of the commit, the latest run of each.
    fn checks(&self, commit: &str) -> impl Future<Output = Result<Vec<Check>, String>> + Send;

    /// The log of the job of GitHub Actions.
    fn log(&self, job: u64) -> impl Future<Output = Result<String, String>> + Send;

    /// Merges the pull request by the method (`squash`, `merge` or `rebase`), provided its head is the commit; gives
    /// the commit the merge made.
    fn merge(&self, number: u64, head: &str, method: &str) -> impl Future<Output = Result<String, String>> + Send;

    /// Deletes the branch of the repository; a branch that is not there is no fault.
    fn delete(&self, branch: &str) -> impl Future<Output = Result<(), String>> + Send;

    /// Gives the pull request this title and body.
    fn edit(&self, number: u64, title: &str, body: &str) -> impl Future<Output = Result<(), String>> + Send;

    /// Makes the draft pull request of this id ready to be reviewed and merged.
    fn ready(&self, node: &str) -> impl Future<Output = Result<(), String>> + Send;

    /// Closes the pull request, saying this on it first.
    fn close(&self, number: u64, comment: &str) -> impl Future<Output = Result<(), String>> + Send;
}
