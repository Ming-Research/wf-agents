//! GitHub for the tests of the GitHub host: a bare repository on disk that the host pushes to and fetches from, and
//! in memory the pull requests, the jobs and results each commit's runs by hand are given, the logs of the jobs, the
//! merges and the closings, and the runners each layer lists. It keeps what it was asked, and each look at the runs of a
//! workflow, and at the jobs of a run, with whether GitHub would count it against its limit on requests: an answer of
//! the runs or the jobs counts, one that they are unchanged does not.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, PoisonError};

use super::api::{Api, Check, File, Jobs, Layer, Listed, Pull, Run, Runners, State};
use crate::host::Failed;

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
    /// Why the list of every open pull request is refused, where it is.
    refuses_listing: Option<String>,
    /// How many of the next lists of pull requests leave out those opened already, as a list read just before another
    /// opened one does.
    hidden: usize,
    /// The runs by hand, in the order they were asked for.
    runs: Vec<Dispatched>,
    /// What a run by hand on each commit gives: its jobs, and the artifact it uploads with its name.
    fast: BTreeMap<String, ByHand>,
    /// What a run by hand of a workflow on a commit gives, by the workflow and the commit, in place of what the commit's
    /// runs give.
    of_workflow: BTreeMap<(String, String), ByHand>,
    /// Why a run by hand is refused, where every one is.
    refuses_dispatch: Option<String>,
    /// Why reading what a run uploaded is refused, where it is.
    refuses_artifact: Option<String>,
    /// Why reading the log of a job is refused, where it is.
    refuses_log: Option<String>,
    /// Whether GitHub lists none of the runs by hand, though it took them.
    hides_runs: bool,
    /// Whether another request runs the workflow on the same branch and commit right after each one asked here, as
    /// another check of the same work may.
    another_call: bool,
    /// Whether the runs by hand stay queued, as with no runner online.
    queued: bool,
    /// Whether the runs by hand asked for from now on have had a job run and are queued again, the next waiting for a
    /// runner.
    requeued: bool,
    /// The runners each layer lists; none listed by the repository and no organisation, where nothing was given.
    runners: Option<Runners>,
    /// Whether the runs by hand start and go on until they are finished or cancelled, as a run that takes minutes.
    going: bool,
    /// The log of each job given one, by the job's id; GitHub has no log of any other.
    logs: BTreeMap<u64, String>,
    asked: Vec<Asked>,
    /// The looks at the runs of a workflow, in order.
    looked: Vec<Looked>,
    /// The looks at the jobs of a run, in order.
    jobs_looked: Vec<Looked>,
}

/// A look at the runs of a workflow: whether it was answered with the runs, which GitHub counts against its limit on
/// requests, or as unchanged since the answer whose entity tag it sent, which it does not.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Looked {
    pub(crate) counted: bool,
}

/// What a run by hand gives: its jobs, and the artifact it uploads, with its name, where it uploads one.
type ByHand = (Vec<Check>, Option<(String, Vec<File>)>);

impl Kept {
    /// What the run by hand gives: what its workflow on its commit is given, or else what its commit is.
    fn given(&self, run: &Dispatched) -> Option<&ByHand> {
        let key = (run.workflow.clone(), run.commit.clone());
        self.of_workflow.get(&key).or_else(|| self.fast.get(&run.commit))
    }
}

/// A run by hand: of which workflow, asked for on which branch, of which commit (its input `commit`), and whether
/// another request than the host's asked for it.
struct Dispatched {
    workflow: String,
    branch: String,
    commit: String,
    another: bool,
    run: Run,
    /// Whether a runner has taken a job of it: its jobs are what its commit is given where one has, and each queued
    /// where none has, as GitHub lists the jobs of a run no runner took.
    taken: bool,
}

/// What the jobs of a run another request asked for come to, here: its tests cancelled.
pub(crate) const ANOTHER_CALLS: &str = "cancelled by another call";

/// What GitHub was asked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Asked {
    Pulls(String),
    OpenPulls,
    Open {
        branch: String,
        base: String,
        title: String,
    },
    Merge {
        number: u64,
        head: String,
        method: String,
        title: String,
        message: String,
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
    Reopen(u64),
    Cancel(u64),
    Runners,
    Log(u64),
    Pull(u64),
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

    /// Gives a run by hand of the workflow on the commit these jobs, and this artifact where one is given, whatever the
    /// commit's other runs give.
    pub(crate) fn of_workflow(
        &self,
        workflow: &str,
        commit: &str,
        jobs: Vec<Check>,
        artifact: Option<(&str, Vec<File>)>,
    ) {
        let artifact = artifact.map(|(name, files)| (name.to_string(), files));
        let key = (workflow.to_string(), commit.to_string());
        self.kept().of_workflow.insert(key, (jobs, artifact));
    }

    /// Gives the job of this id this log, as GitHub keeps it.
    pub(crate) fn logged(&self, job: u64, log: &str) {
        self.kept().logs.insert(job, log.to_string());
    }

    /// The jobs whose logs were asked for, in order.
    pub(crate) fn logs_asked(&self) -> Vec<u64> {
        (self.kept().asked.iter())
            .filter_map(|asked| match asked {
                Asked::Log(job) => Some(*job),
                _ => None,
            })
            .collect()
    }

    /// Refuses every run by hand, with these words.
    pub(crate) fn refuse_dispatch(&self, why: &str) {
        self.kept().refuses_dispatch = Some(why.to_string());
    }

    /// Refuses to give what a run uploaded, with these words, until it is told otherwise.
    pub(crate) fn refuse_artifact(&self, why: Option<&str>) {
        self.kept().refuses_artifact = why.map(str::to_string);
    }

    /// Lists none of the runs by hand from now on, though GitHub took them.
    pub(crate) fn hide_runs(&self) {
        self.kept().hides_runs = true;
    }

    /// Refuses to give the log of any job, with these words, until it is told otherwise.
    pub(crate) fn refuse_log(&self, why: Option<&str>) {
        self.kept().refuses_log = why.map(str::to_string);
    }

    /// Keeps every run by hand asked for from now on queued, never started, as with every runner busy or offline.
    pub(crate) fn queue_runs(&self) {
        self.kept().queued = true;
    }

    /// Has a runner take the runs by hand that are queued, and every one asked for from now on: each starts, and goes
    /// on where `runs_go_on` says, else completes.
    pub(crate) fn start_runs(&self) {
        let mut kept = self.kept();
        kept.queued = false;
        let going = kept.going;
        kept.requeued = false;
        for asked in &mut kept.runs {
            asked.run.started = true;
            asked.run.completed = !going;
            asked.taken = true;
        }
    }

    /// Has the runs by hand that are not complete, and every one asked for from now on, queued again after a job of
    /// each has run, as GitHub says a run is once a job has ended and the next waits for a runner: each is not started
    /// by its status, and its jobs are what its commit is given.
    pub(crate) fn requeue_runs(&self) {
        let mut kept = self.kept();
        kept.requeued = true;
        for asked in (kept.runs.iter_mut()).filter(|d| !d.run.completed) {
            asked.run.started = false;
            asked.taken = true;
        }
    }

    /// Lists these runners at the repository and, where one is given, at its organisation; a layer is read, or refused
    /// in GitHub's words.
    pub(crate) fn lists_runners(&self, repository: Layer, organisation: Option<Layer>) {
        self.kept().runners = Some(Runners {
            repository,
            organisation,
        });
    }

    /// How many times the runners were asked for.
    pub(crate) fn runners_asked(&self) -> usize {
        (self.kept().asked.iter())
            .filter(|asked| matches!(asked, Asked::Runners))
            .count()
    }

    /// Has every run by hand asked for from now on start and go on until it is finished or cancelled.
    pub(crate) fn runs_go_on(&self) {
        self.kept().going = true;
    }

    /// Completes every run by hand that is under way, as it ends with the jobs and results its commit is given.
    pub(crate) fn finish_runs(&self) {
        for asked in &mut self.kept().runs {
            asked.run.started = true;
            asked.run.completed = true;
            asked.taken = true;
        }
    }

    /// Cancels every run by hand that is under way, as a person may: it completes cancelled, with nothing uploaded and
    /// its jobs cancelled.
    pub(crate) fn cancel_runs(&self) {
        for asked in (self.kept().runs.iter_mut()).filter(|d| !d.run.completed) {
            asked.run.completed = true;
            asked.run.cancelled = true;
        }
    }

    /// Has another request run the workflow on the same branch and commit right after each one the host asks for.
    pub(crate) fn another_call_after_each(&self) {
        self.kept().another_call = true;
    }

    /// The pull requests, in the order they were opened.
    pub(crate) fn pull_requests(&self) -> Vec<Pull> {
        self.kept().pulls.iter().map(|(_, p)| p.clone()).collect()
    }

    /// Refuses the list of every open pull request, with these words.
    pub(crate) fn refuse_listing(&self, why: &str) {
        self.kept().refuses_listing = Some(why.to_string());
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

    /// A pull request of the branch into the base, in this state, that a person made before the work: at the branch's
    /// head where the bare repository has it, from a fork where `from_fork` says so. Gives its number.
    pub(crate) fn pull_of(&self, branch: &str, base: &str, state: State, from_fork: bool) -> u64 {
        let head = self.head(branch).unwrap_or_default();
        let mut kept = self.kept();
        let number = kept.pulls.len() as u64 + 1;
        let pull = Pull {
            number,
            url: format!("https://github.example/pull/{number}"),
            state,
            head,
            base: base.to_string(),
            merge_commit: None,
            title: format!("Pull request {number}"),
            body: String::new(),
            draft: false,
            node: format!("PR_{number}"),
            updated: 0,
            branch: branch.to_string(),
            from_fork,
        };
        kept.pulls.push((branch.to_string(), pull));
        number
    }

    /// What the pull requests were asked to close with, as `(number, comment)` in order.
    pub(crate) fn closed_with(&self) -> Vec<(u64, String)> {
        (self.kept().asked.iter())
            .filter_map(|asked| match asked {
                Asked::Close { number, comment } => Some((*number, comment.clone())),
                _ => None,
            })
            .collect()
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

    /// The looks at the runs of a workflow, in order.
    pub(crate) fn looked(&self) -> Vec<Looked> {
        self.kept().looked.clone()
    }

    /// The looks at the jobs of a run, in order.
    pub(crate) fn jobs_looked(&self) -> Vec<Looked> {
        self.kept().jobs_looked.clone()
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

/// What GitHub answers in these words, which begin with its status, `HTTP 422: ...`, as its refusals are said: of the
/// kind its status tells, as the host reads GitHub's own answers, a fault of GitHub's or its limit on requests passing,
/// a limit that says nothing of when for a minute. Words that begin with no status are what `gh` says where GitHub gave
/// no answer, which tell no kind.
fn answered(why: &str) -> Failed {
    let status: Option<u16> = (why.strip_prefix("HTTP "))
        .and_then(|rest| rest.split(|c: char| !c.is_ascii_digit()).next())
        .and_then(|digits| digits.parse().ok());
    match status {
        Some(status) if status >= 500 => Failed::held(why, decide::Obstacle::Fault, None),
        Some(429) => Failed::held(why, decide::Obstacle::Limit, Some(decide::Retry::In(60))),
        Some(_) => Failed::refused(why),
        None => Failed::from(why),
    }
}

/// Squashes the commit onto main in the bare repository, as GitHub's squash merge does, by a commit of this title and
/// message, and gives the new head.
fn squash(bare: &std::path::Path, head: &str, title: &str, message: &str) -> Result<String, String> {
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
    let tree = format!("{head}^{{tree}}");
    let mut args = vec!["commit-tree", &tree, "-p", &main, "-m", title];
    if !message.is_empty() {
        args.extend(["-m", message]);
    }
    let squashed = git(&args)?;
    git(&["update-ref", "refs/heads/main", &squashed])?;
    Ok(squashed)
}

impl Api for Fake {
    async fn pull(&self, number: u64) -> Result<Option<Pull>, Failed> {
        self.kept().asked.push(Asked::Pull(number));
        let found = (self.kept().pulls.iter())
            .find(|(_, p)| p.number == number)
            .map(|(branch, _)| branch.clone());
        Ok(found.and_then(|branch| self.of(&branch).into_iter().find(|p| p.number == number)))
    }

    async fn pulls(&self, branch: &str) -> Result<Vec<Pull>, Failed> {
        let mut kept = self.kept();
        kept.asked.push(Asked::Pulls(branch.to_string()));
        if kept.hidden > 0 {
            kept.hidden -= 1;
            return Ok(Vec::new());
        }
        drop(kept);
        Ok(self.of(branch))
    }

    async fn open_pulls(&self) -> Result<Vec<Pull>, Failed> {
        let mut kept = self.kept();
        kept.asked.push(Asked::OpenPulls);
        if let Some(why) = &kept.refuses_listing {
            return Err(answered(why));
        }
        Ok((kept.pulls.iter())
            .filter(|(_, pull)| pull.state == State::Open)
            .map(|(_, pull)| pull.clone())
            .collect())
    }

    async fn open(&self, branch: &str, base: &str, title: &str, body: &str) -> Result<Pull, Failed> {
        let head = self
            .head(branch)
            .ok_or_else(|| answered("HTTP 422: the head branch is not on GitHub"))?;
        let mut kept = self.kept();
        // As GitHub, which opens no second pull request of a branch into the same base.
        if (kept.pulls.iter()).any(|(of, p)| of == branch && p.state == State::Open && p.base == base) {
            return Err(answered(&format!(
                "HTTP 422: Validation Failed; A pull request already exists for {branch}."
            )));
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
            updated: 0,
            branch: branch.to_string(),
            from_fork: false,
        };
        kept.pulls.push((branch.to_string(), pull.clone()));
        Ok(pull)
    }

    async fn merge(&self, number: u64, head: &str, method: &str, title: &str, message: &str) -> Result<String, Failed> {
        let mut kept = self.kept();
        kept.asked.push(Asked::Merge {
            number,
            head: head.to_string(),
            method: method.to_string(),
            title: title.to_string(),
            message: message.to_string(),
        });
        if let Some(why) = &kept.refuses {
            return Err(answered(why));
        }
        let (_, pull) = (kept.pulls.iter_mut())
            .find(|(_, p)| p.number == number)
            .ok_or_else(|| answered("HTTP 404: Not Found"))?;
        if pull.draft {
            return Err(answered("HTTP 405: Pull Request is still a draft"));
        }
        if pull.head != head {
            return Err(answered(
                "HTTP 409: Head branch was modified. Review and try the merge again.",
            ));
        }
        let merged = squash(&self.bare, head, title, message)?;
        pull.state = State::Merged;
        pull.merge_commit = Some(merged.clone());
        Ok(merged)
    }

    async fn delete(&self, branch: &str) -> Result<(), Failed> {
        self.kept().asked.push(Asked::Delete(branch.to_string()));
        // GitHub's deletion leaves no branch of that name, as a branch it has none of is answered as deleted.
        let reference = format!("refs/heads/{branch}");
        let dropped = std::process::Command::new("git")
            .arg("-C")
            .arg(&self.bare)
            .args(["update-ref", "-d", &reference])
            .output();
        dropped
            .map(|_| ())
            .map_err(|why| Failed::refused(format!("git did not run: {why}")))
    }

    async fn runs(&self, workflow: &str, branch: &str, tag: Option<&str>) -> Result<Listed, Failed> {
        let mut kept = self.kept();
        let hidden = kept.hides_runs;
        let runs: Vec<Run> = (kept.runs.iter().rev())
            .filter(|d| !hidden && d.workflow == workflow && d.branch == branch)
            .map(|d| d.run.clone())
            .collect();
        // As GitHub's, the entity tag is the same for the same answer, and differs for any other.
        let mut hasher = std::hash::DefaultHasher::new();
        std::hash::Hash::hash(&format!("{runs:?}"), &mut hasher);
        let now = format!("W/\"{:016x}\"", std::hash::Hasher::finish(&hasher));
        let counted = tag != Some(now.as_str());
        kept.looked.push(Looked { counted });
        Ok(match counted {
            true => Listed::Runs { runs, tag: Some(now) },
            false => Listed::Unchanged,
        })
    }

    async fn dispatch(&self, workflow: &str, branch: &str, inputs: &[(String, String)]) -> Result<(), Failed> {
        let input = |name: &str| (inputs.iter()).find(|(n, _)| n == name).map(|(_, v)| v.clone());
        let mut kept = self.kept();
        kept.asked.push(Asked::Dispatch {
            workflow: workflow.to_string(),
            branch: branch.to_string(),
            inputs: inputs.to_vec(),
        });
        if let Some(why) = &kept.refuses_dispatch {
            return Err(answered(why));
        }
        drop(kept);
        self.head(branch)
            .ok_or_else(|| answered("HTTP 422: No ref found for the branch"))?;
        let commit = input("commit").ok_or_else(|| answered("HTTP 422: Required input 'commit' not provided"))?;
        let call = input("call").unwrap_or_default();
        let mut kept = self.kept();
        let mut made = vec![(call, false)];
        if kept.another_call {
            made.push(("another".to_string(), true));
        }
        let (ran, going, requeued) = (!kept.queued && !kept.requeued, kept.going, kept.requeued);
        for (call, another) in made {
            let id = 1000 + kept.runs.len() as u64;
            kept.runs.push(Dispatched {
                workflow: workflow.to_string(),
                branch: branch.to_string(),
                commit: commit.clone(),
                another,
                run: Run {
                    id,
                    title: format!("fast {call}"),
                    started: ran,
                    completed: ran && !going,
                    cancelled: false,
                    url: format!("https://github.example/actions/runs/{id}"),
                },
                taken: ran || requeued,
            });
        }
        Ok(())
    }

    async fn cancel(&self, run: u64) -> Result<(), Failed> {
        let mut kept = self.kept();
        kept.asked.push(Asked::Cancel(run));
        for asked in (kept.runs.iter_mut()).filter(|d| d.run.id == run && !d.run.completed) {
            asked.run.completed = true;
            asked.run.cancelled = true;
        }
        Ok(())
    }

    async fn jobs(&self, run: u64, tag: Option<&str>) -> Result<Jobs, Failed> {
        let mut kept = self.kept();
        let jobs = match kept.runs.iter().find(|d| d.run.id == run) {
            None => Vec::new(),
            Some(asked) if asked.another || asked.run.cancelled => {
                let how = if asked.another { ANOTHER_CALLS } else { "cancelled" };
                vec![Check {
                    id: 0,
                    name: "test".to_string(),
                    outcome: super::api::Outcome::Broken(how.to_string()),
                    url: None,
                    took: None,
                    queued: None,
                }]
            }
            Some(asked) => {
                let given = (kept.given(asked)).map_or_else(Vec::new, |(jobs, _)| jobs.clone());
                (given.into_iter())
                    .map(|job| match asked.taken {
                        true => job,
                        false => Check {
                            outcome: super::api::Outcome::Queued,
                            ..job
                        },
                    })
                    .collect()
            }
        };
        // As GitHub's, the entity tag is the same for the same answer, and differs for any other.
        let mut hasher = std::hash::DefaultHasher::new();
        std::hash::Hash::hash(&format!("{jobs:?}"), &mut hasher);
        let now = format!("W/\"{:016x}\"", std::hash::Hasher::finish(&hasher));
        let counted = tag != Some(now.as_str());
        kept.jobs_looked.push(Looked { counted });
        Ok(match counted {
            true => Jobs::Listed { jobs, tag: Some(now) },
            false => Jobs::Unchanged,
        })
    }

    async fn runners(&self) -> Runners {
        let mut kept = self.kept();
        kept.asked.push(Asked::Runners);
        kept.runners.clone().unwrap_or(Runners {
            repository: Ok(Vec::new()),
            organisation: None,
        })
    }

    async fn artifact(&self, run: u64, name: &str) -> Result<Option<Vec<File>>, Failed> {
        let kept = self.kept();
        if let Some(why) = &kept.refuses_artifact {
            return Err(answered(why));
        }
        let asked = (kept.runs.iter()).find(|d| d.run.id == run && !d.another && !d.run.cancelled);
        let artifact = asked.and_then(|d| kept.given(d).and_then(|(_, a)| a.clone()));
        Ok(artifact.filter(|(n, _)| n == name).map(|(_, files)| files))
    }

    async fn log(&self, job: u64) -> Result<String, Failed> {
        let mut kept = self.kept();
        kept.asked.push(Asked::Log(job));
        if let Some(why) = &kept.refuses_log {
            return Err(answered(why));
        }
        (kept.logs.get(&job).cloned()).ok_or_else(|| answered("HTTP 404: Not Found"))
    }

    async fn edit(&self, number: u64, title: &str, body: &str) -> Result<(), Failed> {
        let mut kept = self.kept();
        kept.asked.push(Asked::Edit {
            number,
            title: title.to_string(),
            body: body.to_string(),
        });
        let (_, pull) = (kept.pulls.iter_mut())
            .find(|(_, p)| p.number == number)
            .ok_or_else(|| answered("HTTP 404: Not Found"))?;
        pull.title = title.to_string();
        pull.body = body.to_string();
        Ok(())
    }

    async fn ready(&self, node: &str) -> Result<(), Failed> {
        let mut kept = self.kept();
        kept.asked.push(Asked::Ready(node.to_string()));
        let (_, pull) = (kept.pulls.iter_mut())
            .find(|(_, p)| p.node == node)
            .ok_or("Could not resolve to a node")?;
        pull.draft = false;
        Ok(())
    }

    async fn close(&self, number: u64, comment: &str) -> Result<(), Failed> {
        let mut kept = self.kept();
        kept.asked.push(Asked::Close {
            number,
            comment: comment.to_string(),
        });
        let (_, pull) = (kept.pulls.iter_mut())
            .find(|(_, p)| p.number == number)
            .ok_or_else(|| answered("HTTP 404: Not Found"))?;
        pull.state = State::Closed;
        Ok(())
    }

    async fn reopen(&self, number: u64) -> Result<(), Failed> {
        let mut kept = self.kept();
        kept.asked.push(Asked::Reopen(number));
        let (_, pull) = (kept.pulls.iter_mut())
            .find(|(_, p)| p.number == number)
            .ok_or_else(|| answered("HTTP 404: Not Found"))?;
        if pull.state != State::Closed {
            return Err(answered(
                "HTTP 422: Unprocessable Entity: the pull request is not closed",
            ));
        }
        pull.state = State::Open;
        Ok(())
    }
}
