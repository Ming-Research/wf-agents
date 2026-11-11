//! GitHub for the tests of the GitHub host: a bare repository on disk that the host pushes to and fetches from, and
//! in memory the pull requests, the checks each commit is given, poll by poll, the jobs' logs and the merges. It keeps
//! what it was asked.

use std::collections::{BTreeMap, VecDeque};
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, PoisonError};

use super::api::{Api, Check, Pull, State};

/// GitHub, as far as the host reaches it.
pub(crate) struct Fake {
    /// The repository on GitHub.
    pub(crate) bare: PathBuf,
    state: Mutex<Kept>,
}

#[derive(Default)]
struct Kept {
    /// The pull requests, each with its branch.
    pulls: Vec<(String, Pull)>,
    /// What each commit's checks are at each ask: the first, then the next, the last staying.
    checks: BTreeMap<String, VecDeque<Vec<Check>>>,
    logs: BTreeMap<u64, String>,
    /// Why a merge is refused, where it is.
    refuses: Option<String>,
    asked: Vec<Asked>,
}

/// What GitHub was asked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Asked {
    Pulls(String),
    Open {
        branch: String,
        base: String,
        title: String,
    },
    Checks(String),
    Log(u64),
    Merge {
        number: u64,
        head: String,
        method: String,
    },
    Delete(String),
}

impl Fake {
    /// GitHub with a bare repository holding this repository's main branch.
    pub(crate) fn new(bare: PathBuf) -> Fake {
        Fake {
            bare,
            state: Mutex::new(Kept::default()),
        }
    }

    fn kept(&self) -> MutexGuard<'_, Kept> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Gives the commit these checks, one list at each ask, the last staying.
    pub(crate) fn checks(&self, commit: &str, polls: Vec<Vec<Check>>) {
        self.kept().checks.insert(commit.to_string(), polls.into());
    }

    /// Keeps this log for the job.
    pub(crate) fn log(&self, job: u64, log: &str) {
        self.kept().logs.insert(job, log.to_string());
    }

    /// Refuses every merge, with these words.
    pub(crate) fn refuse_merges(&self, why: &str) {
        self.kept().refuses = Some(why.to_string());
    }

    /// Closes the pull request, as a person may.
    pub(crate) fn close(&self, number: u64) {
        for (_, pull) in &mut self.kept().pulls {
            if pull.number == number {
                pull.state = State::Closed;
            }
        }
    }

    /// What it was asked, in order.
    pub(crate) fn asked(&self) -> Vec<Asked> {
        self.kept().asked.clone()
    }

    /// The commit the branch is at in the bare repository, where it is there.
    fn head(&self, branch: &str) -> Option<String> {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(&self.bare)
            .args(["rev-parse", "--verify", "--quiet", &format!("refs/heads/{branch}")])
            .output()
            .ok()?;
        let head = String::from_utf8_lossy(&output.stdout).trim().to_string();
        (output.status.success() && !head.is_empty()).then_some(head)
    }

    /// The pull requests of the branch, each open one at the branch's head.
    fn of(&self, branch: &str) -> Vec<Pull> {
        let head = self.head(branch);
        let mut kept = self.kept();
        for (of, pull) in &mut kept.pulls {
            if of == branch
                && pull.state == State::Open
                && let Some(head) = &head
            {
                pull.head = head.clone();
            }
        }
        (kept.pulls.iter())
            .filter(|(of, _)| of == branch)
            .map(|(_, p)| p.clone())
            .collect()
    }
}

impl Api for Fake {
    async fn pulls(&self, branch: &str) -> Result<Vec<Pull>, String> {
        self.kept().asked.push(Asked::Pulls(branch.to_string()));
        Ok(self.of(branch))
    }

    async fn open(&self, branch: &str, base: &str, title: &str, _: &str) -> Result<Pull, String> {
        let head = self.head(branch).ok_or("HTTP 422: the head branch is not on GitHub")?;
        let mut kept = self.kept();
        kept.asked.push(Asked::Open {
            branch: branch.to_string(),
            base: base.to_string(),
            title: title.to_string(),
        });
        let number = kept.pulls.len() as u64 + 1;
        let pull = Pull {
            number,
            url: format!("https://github.example/pull/{number}"),
            state: State::Open,
            head,
            base: base.to_string(),
            merge_commit: None,
        };
        kept.pulls.push((branch.to_string(), pull.clone()));
        Ok(pull)
    }

    async fn checks(&self, commit: &str) -> Result<Vec<Check>, String> {
        let mut kept = self.kept();
        kept.asked.push(Asked::Checks(commit.to_string()));
        let Some(polls) = kept.checks.get_mut(commit) else {
            return Ok(Vec::new());
        };
        let now = if polls.len() > 1 {
            polls.pop_front()
        } else {
            polls.front().cloned()
        };
        Ok(now.unwrap_or_default())
    }

    async fn log(&self, job: u64) -> Result<String, String> {
        let mut kept = self.kept();
        kept.asked.push(Asked::Log(job));
        (kept.logs.get(&job).cloned()).ok_or_else(|| "HTTP 410: the log has expired".to_string())
    }

    async fn merge(&self, number: u64, head: &str, method: &str) -> Result<String, String> {
        let mut kept = self.kept();
        kept.asked.push(Asked::Merge {
            number,
            head: head.to_string(),
            method: method.to_string(),
        });
        if let Some(why) = &kept.refuses {
            return Err(why.clone());
        }
        let (_, pull) = (kept.pulls.iter_mut())
            .find(|(_, p)| p.number == number)
            .ok_or("HTTP 404: Not Found")?;
        if pull.head != head {
            return Err("HTTP 409: Head branch was modified. Review and try the merge again.".to_string());
        }
        let merged = format!("squashed-{head}");
        pull.state = State::Merged;
        pull.merge_commit = Some(merged.clone());
        Ok(merged)
    }

    async fn delete(&self, branch: &str) -> Result<(), String> {
        self.kept().asked.push(Asked::Delete(branch.to_string()));
        Ok(())
    }
}
