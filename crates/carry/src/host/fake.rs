//! A host for the tests of the acts: it gives the head of main it is set to and the verdicts it is given, in order,
//! merges by saying main is where it is, and keeps what it was asked.

use std::collections::VecDeque;
use std::sync::{Mutex, MutexGuard, PoisonError};

use super::{Host, Verdict};
use crate::git::Place;
use crate::held::Stop;

/// The host's answers, and what it was asked.
pub(crate) struct Fake {
    main: Mutex<String>,
    verdicts: Mutex<VecDeque<Result<Option<Verdict>, String>>>,
    asked: Mutex<Vec<Asked>>,
}

/// What the host was asked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Asked {
    /// The head of main.
    Main,
    /// The verdict on this commit.
    Verdict(String),
    /// To merge, the check having passed this commit.
    Merge(Option<String>),
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
            asked: Mutex::new(Vec::new()),
        }
    }

    /// Gives this verdict when it is next asked for one.
    pub(crate) fn judges(self, verdict: Result<Option<Verdict>, String>) -> Fake {
        lock(&self.verdicts).push_back(verdict);
        self
    }

    /// What it was asked, in order.
    pub(crate) fn asked(&self) -> Vec<Asked> {
        lock(&self.asked).clone()
    }
}

impl Host for Fake {
    async fn main(&self) -> Result<String, String> {
        lock(&self.asked).push(Asked::Main);
        Ok(lock(&self.main).clone())
    }

    async fn verdict(&self, _: &Place, commit: &str, _: &mut Stop) -> Result<Option<Verdict>, String> {
        lock(&self.asked).push(Asked::Verdict(commit.to_string()));
        let next = lock(&self.verdicts).pop_front();
        next.unwrap_or_else(|| Err("the fake host has no verdict to give".to_string()))
    }

    async fn merge(&self, _: &Place, checked: Option<&str>) -> Result<String, String> {
        lock(&self.asked).push(Asked::Merge(checked.map(str::to_string)));
        Ok(lock(&self.main).clone())
    }
}
