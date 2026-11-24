//! A host for the tests of the acts: it gives the head of main it is set to and the verdicts it is given, in order,
//! merges by saying main is where it is, and keeps what it was asked, the title of each branch it was given, and what
//! asked for each verdict and each run of the tests. It runs no test for the agents' tool, and keeps what it was asked
//! to run, giving the words it is set to. Its pull requests are at addresses of its own.

use std::collections::VecDeque;
use std::sync::{Mutex, MutexGuard, PoisonError};

use service::OpenPull;
use store::Continues;

use super::{Asking, Branch, Failed, Host, Named, NotMerged, Verdict};
use crate::held::Stop;

/// The host's answers, and what it was asked.
pub(crate) struct Fake {
    main: Mutex<String>,
    verdicts: Mutex<VecDeque<Result<Option<Verdict>, Failed>>>,
    /// Why letting go fails, where it does.
    ending: Mutex<Option<String>>,
    /// The refusal the merge gives, where it gives one.
    refusing: Mutex<Option<NotMerged>>,
    asked: Mutex<Vec<Asked>>,
    /// What it says of the tests it is asked to run; its reason for running none where there are none.
    tests: Mutex<Option<String>>,
    /// The title of each branch it was given, in order.
    titles: Mutex<Vec<String>>,
    /// What asked for each verdict and each run of the tests, in order.
    askings: Mutex<Vec<Asking>>,
    /// What it gives of what an item continues; a refusal where it is given none.
    continuing: Mutex<Option<Result<String, Failed>>>,
}

/// What the host was asked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Asked {
    /// The head of main.
    Main,
    /// The verdict on this commit.
    Verdict(String),
    /// The agents' request to run these tests and checks, as they named them.
    RunTests(Named),
    /// To merge, the check having passed this commit.
    Merge(Option<String>),
    /// To let go of what it holds for the branch, the work having merged or not.
    End(String, bool),
    /// To take up again what it held for the branch, the work having been reopened.
    Reopen(String),
    /// The head of what an item continues.
    Continued(Continues),
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Fake {
    /// A host whose main is at this commit, with no verdict to give.
    pub(crate) fn new(main: &str) -> Fake {
        Fake {
            main: Mutex::new(main.to_string()),
            verdicts: Mutex::new(VecDeque::new()),
            ending: Mutex::new(None),
            refusing: Mutex::new(None),
            asked: Mutex::new(Vec::new()),
            tests: Mutex::new(None),
            titles: Mutex::new(Vec::new()),
            askings: Mutex::new(Vec::new()),
            continuing: Mutex::new(None),
        }
    }

    /// Gives this when it is asked for the head of what an item continues.
    pub(crate) fn continued_at(self, answer: Result<String, Failed>) -> Fake {
        *lock(&self.continuing) = Some(answer);
        self
    }

    /// Fails to let go, with these words, until it is told otherwise.
    pub(crate) fn ending(&self, refused: Option<&str>) {
        *lock(&self.ending) = refused.map(str::to_string);
    }

    /// Refuses to merge, for this reason, instead of merging.
    pub(crate) fn refuses(self, refusal: NotMerged) -> Fake {
        *lock(&self.refusing) = Some(refusal);
        self
    }

    /// Gives this verdict when it is next asked for one.
    pub(crate) fn judges(self, verdict: Result<Option<Verdict>, Failed>) -> Fake {
        lock(&self.verdicts).push_back(verdict);
        self
    }

    /// Says these words of the tests it is asked to run.
    pub(crate) fn runs(self, said: &str) -> Fake {
        *lock(&self.tests) = Some(said.to_string());
        self
    }

    /// What it was asked, in order.
    pub(crate) fn asked(&self) -> Vec<Asked> {
        lock(&self.asked).clone()
    }

    /// The title of each branch it was given, in order.
    pub(crate) fn titles(&self) -> Vec<String> {
        lock(&self.titles).clone()
    }

    /// What asked for each verdict and each run of the tests, in order.
    pub(crate) fn askings(&self) -> Vec<Asking> {
        lock(&self.askings).clone()
    }

    /// Keeps the title of the branch it is given.
    fn given(&self, branch: &Branch<'_>) {
        lock(&self.titles).push(branch.title.to_string());
    }
}

impl Host for Fake {
    async fn main(&self) -> Result<String, String> {
        lock(&self.asked).push(Asked::Main);
        Ok(lock(&self.main).clone())
    }

    async fn verdict(
        &self,
        branch: &Branch<'_>,
        commit: &str,
        _: &str,
        asking: Asking,
        _: &tokio::sync::Mutex<()>,
        _: &mut Stop,
    ) -> Result<Option<Verdict>, Failed> {
        self.given(branch);
        lock(&self.askings).push(asking);
        lock(&self.asked).push(Asked::Verdict(commit.to_string()));
        let next = lock(&self.verdicts).pop_front();
        next.unwrap_or_else(|| Err("the fake host has no verdict to give".into()))
    }

    async fn merge(&self, branch: &Branch<'_>, checked: Option<&str>) -> Result<String, NotMerged> {
        self.given(branch);
        lock(&self.asked).push(Asked::Merge(checked.map(str::to_string)));
        if let Some(refusal) = lock(&self.refusing).clone() {
            return Err(refusal);
        }
        Ok(lock(&self.main).clone())
    }

    async fn continued(&self, what: &Continues) -> Result<String, Failed> {
        lock(&self.asked).push(Asked::Continued(what.clone()));
        let answer = lock(&self.continuing).clone();
        answer.unwrap_or_else(|| Err(Failed::refused("the fake host has nothing an item continues to give")))
    }

    fn kept_main(&self) -> String {
        "refs/heads/main".to_string()
    }

    async fn run_tests(
        &self,
        branch: &Branch<'_>,
        named: &Named,
        asking: Asking,
        _: &mut Stop,
    ) -> Result<Option<String>, String> {
        self.given(branch);
        lock(&self.askings).push(asking);
        lock(&self.asked).push(Asked::RunTests(named.clone()));
        let said = lock(&self.tests).clone();
        said.map(Some).ok_or_else(|| NO_TOOLS.to_string())
    }

    async fn end(&self, branch: &Branch<'_>, merged: bool) -> Result<(), String> {
        self.given(branch);
        let refused = lock(&self.ending).clone();
        lock(&self.asked).push(Asked::End(branch.place.branch.clone(), merged));
        refused.map_or(Ok(()), Err)
    }

    async fn reopen(&self, branch: &Branch<'_>) -> Result<(), String> {
        self.given(branch);
        lock(&self.asked).push(Asked::Reopen(branch.place.branch.clone()));
        Ok(())
    }

    fn pull_at(&self, number: u64) -> Option<String> {
        Some(format!("https://host.example/pull/{number}"))
    }

    async fn open_pulls(&self) -> Result<Option<Vec<OpenPull>>, String> {
        Ok(None)
    }
}

/// What the fake host says where the agents' tool asks it to run tests and it is set to say nothing.
pub(crate) const NO_TOOLS: &str = "the fake host runs no test for the agents' tool";
