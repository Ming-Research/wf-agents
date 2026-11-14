//! GitHub for the tests of the GitHub host: a bare repository on disk that the host pushes to and fetches from, and
//! in memory the pull requests, the jobs and results each commit's runs by hand are given, the merges and the
//! closings. It keeps what it was asked.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, PoisonError};

use super::api::{Api, Check, File, Pull, Run, State};

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
    /// Why a merge is refused, where it is.
    refuses: Option<String>,
    /// How many of the next lists of pull requests leave out those opened already, as a list read just before another
    /// opened one does.
    hidden: usize,
    /// The runs by hand, in the order they were asked for.
    runs: Vec<Dispatched>,
    /// What a run by hand on each commit gives: its jobs, and the artifact it uploads with its name.
    fast: BTreeMap<String, ByHand>,
    /// Why a run by hand is refused, where every one is.
    refuses_dispatch: Option<String>,
    /// Whether another request runs the workflow on the same branch and commit right after each one asked here, as
    /// another check of the same work may.
    another_call: bool,
    /// Whether the runs by hand stay queued, as with no runner online.
    queued: bool,
    asked: Vec<Asked>,
}

/// What a run by hand gives: its jobs, and the artifact it uploads, with its name, where it uploads one.
type ByHand = (Vec<Check>, Option<(String, Vec<File>)>);

/// A run by hand: of which workflow, asked for on which branch, of which commit (its input `commit`), and whether
/// another request than the host's asked for it.
struct Dispatched {
    workflow: String,
    branch: String,
    commit: String,
    another: bool,
    run: Run,
}

/// What the jobs of a run another request asked for come to, here: its tests cancelled.
pub(crate) const ANOTHER_CALLS: &str = "cancelled by another call";

/// What GitHub was asked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Asked {
    Pulls(String),
    Open {
        branch: String,
        base: String,
        title: String,
    },
    Merge {
        number: u64,
        head: String,
        method: String,
    },
    Delete(String),
    Dispatch {
        workflow: String,
        branch: String,
        inputs: Vec<(String, String)>,
    },
    Edit {
        number: u64,
        title: String,
        body: String,
    },
    Ready(String),
    Close {
        number: u64,
        comment: String,
    },
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

    /// Gives a run by hand on the commit these jobs, and this artifact where one is given.
    pub(crate) fn fast(&self, commit: &str, jobs: Vec<Check>, artifact: Option<(&str, Vec<File>)>) {
        let artifact = artifact.map(|(name, files)| (name.to_string(), files));
        self.kept().fast.insert(commit.to_string(), (jobs, artifact));
    }

    /// Refuses every run by hand, with these words.
    pub(crate) fn refuse_dispatch(&self, why: &str) {
        self.kept().refuses_dispatch = Some(why.to_string());
    }

    /// Keeps every run by hand queued, never started, as with no runner online.
    pub(crate) fn no_runner(&self) {
        self.kept().queued = true;
    }

    /// Has another request run the workflow on the same branch and commit right after each one the host asks for.
    pub(crate) fn another_call_after_each(&self) {
        self.kept().another_call = true;
    }

    /// The pull requests, in the order they were opened.
    pub(crate) fn pull_requests(&self) -> Vec<Pull> {
        self.kept().pulls.iter().map(|(_, p)| p.clone()).collect()
    }

    /// Refuses every merge, with these words.
    pub(crate) fn refuse_merges(&self, why: &str) {
        self.kept().refuses = Some(why.to_string());
    }

    /// Leaves the pull requests opened already out of the next list of them, as one read just before another check
    /// opened its pull request would.
    pub(crate) fn hide_pulls_once(&self) {
        self.kept().hidden += 1;
    }

    /// Closes the pull request, as a person may.
    pub(crate) fn close(&self, number: u64) {
        for (_, pull) in &mut self.kept().pulls {
            if pull.number == number {
                pull.state = State::Closed;
            }
        }
    }

    /// Makes the pull request go into another branch, as a person may.
    pub(crate) fn set_base(&self, number: u64, base: &str) {
        for (_, pull) in &mut self.kept().pulls {
            if pull.number == number {
                pull.base = base.to_string();
            }
        }
    }

    /// Merges the pull request at this head, as a person may.
    pub(crate) fn merged_by_a_person(&self, number: u64, head: &str) {
        for (_, pull) in &mut self.kept().pulls {
            if pull.number == number {
                pull.state = State::Merged;
                pull.head = head.to_string();
                pull.merge_commit = Some(format!("theirs-{head}"));
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

/// Squashes the commit onto main in the bare repository, as GitHub's squash merge does, and gives the new head.
fn squash(bare: &std::path::Path, head: &str) -> Result<String, String> {
    let git = |args: &[&str]| -> Result<String, String> {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(bare)
            .args(args)
            .env("GIT_AUTHOR_NAME", "GitHub")
            .env("GIT_AUTHOR_EMAIL", "noreply@github.example")
            .env("GIT_COMMITTER_NAME", "GitHub")
            .env("GIT_COMMITTER_EMAIL", "noreply@github.example")
            .output()
            .map_err(|e| e.to_string())?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).into_owned());
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    };
    let main = git(&["rev-parse", "refs/heads/main"])?;
    let squashed = git(&[
        "commit-tree",
        &format!("{head}^{{tree}}"),
        "-p",
        &main,
        "-m",
        "Squashed",
    ])?;
    git(&["update-ref", "refs/heads/main", &squashed])?;
    Ok(squashed)
}

impl Api for Fake {
    async fn pulls(&self, branch: &str) -> Result<Vec<Pull>, String> {
        let mut kept = self.kept();
        kept.asked.push(Asked::Pulls(branch.to_string()));
        if kept.hidden > 0 {
            kept.hidden -= 1;
            return Ok(Vec::new());
        }
        drop(kept);
        Ok(self.of(branch))
    }

    async fn open(&self, branch: &str, base: &str, title: &str, body: &str) -> Result<Pull, String> {
        let head = self.head(branch).ok_or("HTTP 422: the head branch is not on GitHub")?;
        let mut kept = self.kept();
        // As GitHub, which opens no second pull request of a branch into the same base.
        if (kept.pulls.iter()).any(|(of, p)| of == branch && p.state == State::Open && p.base == base) {
            return Err(format!(
                "HTTP 422: Validation Failed; A pull request already exists for {branch}."
            ));
        }
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
            title: title.to_string(),
            body: body.to_string(),
            draft: true,
            node: format!("PR_{number}"),
        };
        kept.pulls.push((branch.to_string(), pull.clone()));
        Ok(pull)
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
        if pull.draft {
            return Err("HTTP 405: Pull Request is still a draft".to_string());
        }
        if pull.head != head {
            return Err("HTTP 409: Head branch was modified. Review and try the merge again.".to_string());
        }
        let merged = squash(&self.bare, head)?;
        pull.state = State::Merged;
        pull.merge_commit = Some(merged.clone());
        Ok(merged)
    }

    async fn delete(&self, branch: &str) -> Result<(), String> {
        self.kept().asked.push(Asked::Delete(branch.to_string()));
        Ok(())
    }

    async fn runs(&self, workflow: &str, branch: &str) -> Result<Vec<Run>, String> {
        let kept = self.kept();
        Ok((kept.runs.iter().rev())
            .filter(|d| d.workflow == workflow && d.branch == branch)
            .map(|d| d.run.clone())
            .collect())
    }

    async fn dispatch(&self, workflow: &str, branch: &str, inputs: &[(String, String)]) -> Result<(), String> {
        let input = |name: &str| (inputs.iter()).find(|(n, _)| n == name).map(|(_, v)| v.clone());
        let mut kept = self.kept();
        kept.asked.push(Asked::Dispatch {
            workflow: workflow.to_string(),
            branch: branch.to_string(),
            inputs: inputs.to_vec(),
        });
        if let Some(why) = &kept.refuses_dispatch {
            return Err(why.clone());
        }
        drop(kept);
        self.head(branch).ok_or("HTTP 422: No ref found for the branch")?;
        let commit = input("commit").ok_or("HTTP 422: Required input 'commit' not provided")?;
        let call = input("call").unwrap_or_default();
        let mut kept = self.kept();
        let mut made = vec![(call, false)];
        if kept.another_call {
            made.push(("another".to_string(), true));
        }
        for (call, another) in made {
            let id = 1000 + kept.runs.len() as u64;
            let ran = !kept.queued;
            kept.runs.push(Dispatched {
                workflow: workflow.to_string(),
                branch: branch.to_string(),
                commit: commit.clone(),
                another,
                run: Run {
                    id,
                    title: format!("fast {call}"),
                    started: ran,
                    completed: ran,
                    url: format!("https://github.example/actions/runs/{id}"),
                },
            });
        }
        Ok(())
    }

    async fn jobs(&self, run: u64) -> Result<Vec<Check>, String> {
        let kept = self.kept();
        let Some(asked) = kept.runs.iter().find(|d| d.run.id == run) else {
            return Ok(Vec::new());
        };
        if asked.another {
            return Ok(vec![Check {
                name: "test".to_string(),
                outcome: super::api::Outcome::Broken(ANOTHER_CALLS.to_string()),
                url: None,
            }]);
        }
        Ok((kept.fast.get(&asked.commit)).map_or_else(Vec::new, |(jobs, _)| jobs.clone()))
    }

    async fn artifact(&self, run: u64, name: &str) -> Result<Option<Vec<File>>, String> {
        let kept = self.kept();
        let asked = (kept.runs.iter()).find(|d| d.run.id == run && !d.another);
        let artifact = asked.and_then(|d| kept.fast.get(&d.commit).and_then(|(_, a)| a.clone()));
        Ok(artifact.filter(|(n, _)| n == name).map(|(_, files)| files))
    }

    async fn edit(&self, number: u64, title: &str, body: &str) -> Result<(), String> {
        let mut kept = self.kept();
        kept.asked.push(Asked::Edit {
            number,
            title: title.to_string(),
            body: body.to_string(),
        });
        let (_, pull) = (kept.pulls.iter_mut())
            .find(|(_, p)| p.number == number)
            .ok_or("HTTP 404: Not Found")?;
        pull.title = title.to_string();
        pull.body = body.to_string();
        Ok(())
    }

    async fn ready(&self, node: &str) -> Result<(), String> {
        let mut kept = self.kept();
        kept.asked.push(Asked::Ready(node.to_string()));
        let (_, pull) = (kept.pulls.iter_mut())
            .find(|(_, p)| p.node == node)
            .ok_or("Could not resolve to a node")?;
        pull.draft = false;
        Ok(())
    }

    async fn close(&self, number: u64, comment: &str) -> Result<(), String> {
        let mut kept = self.kept();
        kept.asked.push(Asked::Close {
            number,
            comment: comment.to_string(),
        });
        let (_, pull) = (kept.pulls.iter_mut())
            .find(|(_, p)| p.number == number)
            .ok_or("HTTP 404: Not Found")?;
        pull.state = State::Closed;
        Ok(())
    }
}
