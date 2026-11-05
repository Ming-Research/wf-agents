//! What the engine does on GitHub: pull requests, CI runs, merges and the gh commands agents ask for. It is a trait so
//! that the engine's tests can stand in for GitHub; [`Gh`] does it with the gh CLI and the engine's own credentials.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The latest run of one check on a commit, as tools give it to agents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckRun {
    pub name: String,
    /// green, red or cancelled.
    pub result: String,
    /// The link to the run.
    pub run: String,
    /// What of a red run's log tells why it is red ([`what_failed`]).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_tail: Option<String>,
    /// Where the red run's whole log is kept, for an agent to read with its tools.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_path: Option<String>,
    /// Whether the job did not run, its condition false, as a job of the fast CI that runs only when named: GitHub
    /// concludes it skipped, which counts as green. Read from GitHub only, and never written: a tool that gives the
    /// checks of a run gives such a job as green, or leaves it out ([`CheckRun::ran`]).
    #[serde(skip)]
    pub skipped: bool,
}

impl CheckRun {
    /// The checks of a run that ran: a job GitHub skipped is none of them, though it counts as green, so that a check
    /// the run was not asked for is not read as passed.
    pub fn ran(checks: Vec<CheckRun>) -> Vec<CheckRun> {
        checks.into_iter().filter(|c| !c.skipped).collect()
    }
}

/// A tool that did not run, in the kinds the flow names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    /// network_or_api, ci_did_not_start, timed_out, machine_down or other; or `rejected`, which a tool turns into a
    /// refusal its agent sees and never gives as a failed tool: GitHub reached, and refused the request itself.
    pub kind: &'static str,
    pub what: String,
}

impl Failure {
    pub fn network(what: impl Into<String>) -> Failure {
        Failure {
            kind: "network_or_api",
            what: what.into(),
        }
    }

    pub fn rejected(what: impl Into<String>) -> Failure {
        Failure {
            kind: "rejected",
            what: what.into(),
        }
    }

    pub fn other(what: impl Into<String>) -> Failure {
        Failure {
            kind: "other",
            what: what.into(),
        }
    }
}

/// A file of a run's artifact: its path in the artifact, with `/` between its parts, and its bytes.
pub type ArtifactFile = (String, Vec<u8>);

/// GitHub as the engine's threads share it.
pub trait GitHub: Send + Sync {
    /// Opens a draft pull request from `branch` into `base`, or, given its number, updates its title and body when
    /// new ones are given; gives its number. A pull request that was open already keeps its base: the engine sets it
    /// with [`ensure_base`].
    fn pull_request(
        &self,
        repo: &str,
        branch: &str,
        base: &str,
        title_body: Option<(&str, &str)>,
        existing: Option<i64>,
    ) -> Result<i64, Failure>;
    /// Waits for the workflow's run on a commit to finish, and gives each check's result.
    fn checks(&self, repo: &str, sha: &str, workflow: &str) -> Result<Vec<CheckRun>, Failure>;
    /// Runs a workflow on a branch, by hand, with the inputs given, each as its name and its value; [`GitHub::checks`]
    /// on the branch's commit then waits for that run, or [`GitHub::checks_after`] for the run this made.
    fn dispatch(&self, _repo: &str, _workflow: &str, _branch: &str, _inputs: &[(&str, &str)]) -> Result<(), Failure> {
        Err(Failure::other("no workflow runs by hand here"))
    }
    /// The number of the workflow's latest run on a commit, finished or not; none when it has none. Runs are numbered
    /// in the order they are made, so that a run with a larger number than this one is made after it.
    fn latest_run_id(&self, _repo: &str, _sha: &str, _workflow: &str) -> Result<Option<i64>, Failure> {
        Err(Failure::other("no runs are listed here"))
    }
    /// Waits for the workflow's run on a commit that is numbered above `after` (any run when none is given) to finish,
    /// and gives each check's result: a run numbered at or below `after` was made before, and is never the one.
    fn checks_after(
        &self,
        _repo: &str,
        _sha: &str,
        _workflow: &str,
        _after: Option<i64>,
    ) -> Result<Vec<CheckRun>, Failure> {
        Err(Failure::other("no runs are waited for here"))
    }
    /// Runs the red checks of the workflow's run on a commit once more, waits, and gives each check's result.
    fn rerun(&self, repo: &str, sha: &str, workflow: &str) -> Result<Vec<CheckRun>, Failure>;
    /// The commit a branch of a repository is at, as GitHub has it.
    fn branch_head(&self, _repo: &str, _branch: &str) -> Result<String, Failure> {
        Err(Failure::other("no branches are read here"))
    }
    /// Each check's result of the workflow's latest run on a commit once that run has finished, without waiting for
    /// it: none while it runs, and none when the commit has no run of it.
    fn finished_checks(&self, _repo: &str, _sha: &str, _workflow: &str) -> Result<Option<Vec<CheckRun>>, Failure> {
        Err(Failure::other("no runs are read here"))
    }
    /// The branch a pull request merges into: its base.
    fn base(&self, repo: &str, pr: i64) -> Result<String, Failure>;
    /// Sets the branch a pull request merges into.
    fn set_base(&self, repo: &str, pr: i64, base: &str) -> Result<(), Failure>;
    /// The commit a pull request merged as; none when it has not merged, whether it is open or closed.
    fn merge_commit(&self, repo: &str, pr: i64) -> Result<Option<String>, Failure>;
    /// Marks the pull request ready and merges it into its base, whichever branch that is; gives the commit it made
    /// there. It is called only for a pull request that has not merged, which marking ready refuses; the engine merges
    /// only through [`merge_into`], which gives the commit of one that merged already, and makes the base the
    /// project's main first.
    fn merge(&self, repo: &str, pr: i64) -> Result<String, Failure>;
    /// Closes a pull request with a comment.
    fn close(&self, repo: &str, pr: i64, comment: &str) -> Result<(), Failure>;
    /// Runs a gh command, as agents hand it in, with the engine's credentials, in the work's worktree so that it is
    /// about the project's repository; gives its exit code and output.
    fn gh(&self, dir: &Path, args: &[String]) -> Result<(i32, String), Failure>;
    /// The whole log of a check's job, as gh gives it.
    fn job_log(&self, _repo: &str, _check: &CheckRun) -> Result<String, Failure> {
        Err(Failure::other("no job logs here"))
    }
    /// The files of the artifact of that name of the run a check is of; none when the run has no such artifact.
    fn artifact(&self, _repo: &str, _check: &CheckRun, _name: &str) -> Result<Option<Vec<ArtifactFile>>, Failure> {
        Err(Failure::other("no artifacts here"))
    }
}

/// Makes a pull request merge into `main`, the project's main: when its base is another branch, sets it to `main` and
/// reads it again; fails, naming both branches, when the base is not `main` after that. A pull request keeps the base
/// it was opened with, while a work taken over keeps the pull request of the work before it and a project's main may
/// change, so its base may be a branch that is not, or no longer, the project's main.
pub fn ensure_base(github: &dyn GitHub, repo: &str, pr: i64, main: &str) -> Result<(), Failure> {
    let was = github.base(repo, pr)?;
    if was == main {
        return Ok(());
    }
    let refused = |why: String| format!("PR {pr} merges into {was}, not the project's main {main}: {why}");
    github.set_base(repo, pr, main).map_err(|f| Failure {
        kind: f.kind,
        what: refused(format!("setting its base failed: {}", f.what)),
    })?;
    let now = github.base(repo, pr)?;
    if now != main {
        return Err(Failure::other(refused(format!(
            "setting its base to {main} did not take; it is {now}"
        ))));
    }
    Ok(())
}

/// The commit a pull request merged as into `main`, the project's main; none when it has not merged. Fails, naming both
/// branches, when it merged into another branch: that is no merge into the project's main. The merge of a pull request
/// is GitHub's write and the engine's record of it another, so that a merge whose answer was lost (a 502 after GitHub
/// merged it) leaves a pull request that merged and a step that does not know it.
pub fn merged_on(github: &dyn GitHub, repo: &str, pr: i64, main: &str) -> Result<Option<String>, Failure> {
    // One read in the common case, a pull request that is not merged.
    let Some(sha) = github.merge_commit(repo, pr)? else {
        return Ok(None);
    };
    let base = github.base(repo, pr)?;
    if base != main {
        return Err(Failure::other(format!(
            "PR {pr} merged into {base}, not the project's main {main}"
        )));
    }
    Ok(Some(sha))
}

/// Merges a pull request into `main`, the project's main, and nowhere else: makes `main` its base first, and merges
/// nothing when it cannot; gives the merge commit. A pull request that merged into `main` already, as when the answer
/// of the merge was lost, is not merged again, which GitHub refuses: its merge commit is the answer.
pub fn merge_into(github: &dyn GitHub, repo: &str, pr: i64, main: &str) -> Result<String, Failure> {
    if let Some(sha) = merged_on(github, repo, pr, main)? {
        return Ok(sha);
    }
    ensure_base(github, repo, pr, main)?;
    github.merge(repo, pr)
}

/// GitHub through the gh CLI.
pub struct Gh {
    /// Where logs of red checks are kept.
    pub logs: PathBuf,
    /// How long from the start of one look at a run to the start of the next.
    pub poll: Duration,
    /// How long a run may take to appear, and to finish.
    pub start_timeout: Duration,
    pub run_timeout: Duration,
    /// The ids of the workflows found by their names, by repository and name: a workflow keeps its id.
    workflow_ids: Mutex<BTreeMap<(String, String), String>>,
}

impl Gh {
    pub fn new(logs: PathBuf) -> Gh {
        Gh {
            logs,
            poll: Duration::from_secs(5),
            start_timeout: Duration::from_secs(600),
            run_timeout: Duration::from_secs(3 * 3600),
            workflow_ids: Mutex::new(BTreeMap::new()),
        }
    }

    fn run(&self, args: &[&str]) -> Result<String, Failure> {
        self.run_said(args).map_err(|(failure, _)| failure)
    }

    /// Runs gh; when it fails, gives the failure and, apart from the command line the failure's text begins with, what
    /// gh said, which is all that tells why: the command line holds the inputs an engineer gave.
    fn run_said(&self, args: &[&str]) -> Result<String, (Failure, String)> {
        let out = Command::new("gh")
            .args(args)
            .output()
            .map_err(|e| (Failure::other(format!("gh: {e}")), String::new()))?;
        if !out.status.success() {
            let said = String::from_utf8_lossy(&out.stderr).trim().to_string();
            return Err((Failure::network(format!("gh {}: {said}", args.join(" "))), said));
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }

    fn json(&self, args: &[&str]) -> Result<Value, Failure> {
        let out = self.run(args)?;
        serde_json::from_str(&out).map_err(|e| Failure::other(format!("gh {}: {e}", args.join(" "))))
    }

    /// The workflow's runs on a commit, as one look finds them, under the names gh's own listing of runs uses.
    fn runs(&self, repo: &str, sha: &str, workflow: &str) -> Result<Value, Failure> {
        match self.look(repo, sha, workflow, None)? {
            Look::Changed { runs, .. } => Ok(runs),
            Look::Unchanged => Err(Failure::other(NOT_MODIFIED_WITHOUT_TAG)),
        }
    }

    /// One look at the workflow's runs on a commit, one request to GitHub. Given the entity tag of the answer to the
    /// last look, GitHub answers 304 when nothing changed since, which costs nothing against the rate limit.
    fn look(&self, repo: &str, sha: &str, workflow: &str, etag: Option<&str>) -> Result<Look, Failure> {
        let workflow = self.workflow_in_path(repo, workflow)?;
        let path = format!("repos/{repo}/actions/workflows/{workflow}/runs?head_sha={sha}&{RUNS_QUERY}");
        let header = etag.map(|e| format!("If-None-Match: {e}"));
        let mut gh = Command::new("gh");
        gh.args(["api", path.as_str(), "-i"]);
        if let Some(h) = &header {
            gh.args(["-H", h.as_str()]);
        }
        // The exit status is not read: gh exits 1 on a 304, and the status line says what GitHub answered.
        let out = gh.output().map_err(|e| Failure::other(format!("gh: {e}")))?;
        parse_look(
            &String::from_utf8_lossy(&out.stdout),
            &String::from_utf8_lossy(&out.stderr),
        )
        .map_err(|f| Failure {
            kind: f.kind,
            what: format!("gh api {path}: {}", f.what),
        })
    }

    /// How a workflow is written in the path of the API: its file name or its id as they are; any other name, as a
    /// project's configuration may write it, by the id of the workflow of that name, found once in the repository's list
    /// of workflows and remembered.
    fn workflow_in_path(&self, repo: &str, workflow: &str) -> Result<String, Failure> {
        if named_by_file_or_id(workflow) {
            return Ok(workflow.to_string());
        }
        let key = (repo.to_string(), workflow.to_string());
        if let Some(id) = self.workflow_ids.lock().ok().and_then(|ids| ids.get(&key).cloned()) {
            return Ok(id);
        }
        let listed = self.json(&["api", &format!("repos/{repo}/actions/workflows?per_page=100")])?;
        let id = workflow_named(&listed, workflow)?;
        if let Ok(mut ids) = self.workflow_ids.lock() {
            ids.insert(key, id.clone());
        }
        Ok(id)
    }

    /// The workflow's latest run on a commit, once it has finished: one at an attempt above `after` when that is
    /// given, which is how a run run again is told from the run it was, and one numbered above `run_above` when that
    /// is given, which is how a run made after a point is told from the runs there were.
    fn finished_run(
        &self,
        repo: &str,
        sha: &str,
        workflow: &str,
        after: Option<i64>,
        run_above: Option<i64>,
    ) -> Result<Value, Failure> {
        let waits = Waits {
            poll: self.poll,
            start_timeout: self.start_timeout,
            run_timeout: self.run_timeout,
        };
        let wanted = Wanted {
            workflow,
            sha,
            after,
            run_above,
        };
        wait_for_run(&waits, &Wall(Instant::now()), &wanted, |etag| {
            self.look(repo, sha, workflow, etag)
        })
    }

    fn jobs(&self, repo: &str, run: &Value) -> Result<Vec<CheckRun>, Failure> {
        let id = run["databaseId"].as_i64().unwrap_or_default().to_string();
        let v = self.json(&["run", "view", &id, "-R", repo, "--json", "jobs"])?;
        let mut out = Vec::new();
        for j in v["jobs"].as_array().cloned().unwrap_or_default() {
            let conclusion = j["conclusion"].as_str().unwrap_or_default();
            let result = match conclusion {
                "success" | "skipped" | "neutral" => "green",
                "cancelled" => "cancelled",
                _ => "red",
            };
            let mut c = CheckRun {
                name: j["name"].as_str().unwrap_or_default().to_string(),
                result: result.to_string(),
                run: j["url"].as_str().unwrap_or_default().to_string(),
                log_tail: None,
                log_path: None,
                skipped: conclusion == "skipped",
            };
            if result == "red" {
                let job = j["databaseId"].as_i64().unwrap_or_default().to_string();
                if let Ok(log) = self.run(&["run", "view", &id, "-R", repo, "--log-failed", "--job", &job]) {
                    let path = self.logs.join(format!("run-{id}-job-{job}.log"));
                    if std::fs::create_dir_all(&self.logs).is_ok() && std::fs::write(&path, &log).is_ok() {
                        c.log_path = Some(path.display().to_string());
                    }
                    c.log_tail = Some(what_failed(&log));
                }
            }
            out.push(c);
        }
        Ok(out)
    }
}

/// The newest of the runs gh lists that is numbered above `above` (any when none is given): runs are numbered in the
/// order they are made, so that the newest is the one with the largest number, whatever order they are listed in. A run
/// numbered at or below `above` was made before it, however recent its last attempt, and is never the one.
fn newest_above(runs: &Value, above: Option<i64>) -> Option<Value> {
    runs.as_array()?
        .iter()
        .filter_map(|r| r["databaseId"].as_i64().map(|id| (id, r)))
        .filter(|(id, _)| above.is_none_or(|a| *id > a))
        .max_by_key(|(id, _)| *id)
        .map(|(_, r)| r.clone())
}

/// What one look at a workflow's runs on a commit found.
#[derive(Debug, Clone, PartialEq)]
enum Look {
    /// GitHub gave the runs, as [`run_fields`] names them, with the entity tag of its answer when it gave one: the next
    /// look sends it back.
    Changed { etag: Option<String>, runs: Value },
    /// GitHub answered 304 to a look that sent the entity tag of the last answer: nothing changed since.
    Unchanged,
}

/// What a look asks of the API of a workflow's runs besides the commit: up to 20 runs, without the pull requests each
/// lists, which a look does not read.
const RUNS_QUERY: &str = "per_page=20&exclude_pull_requests=true";

/// What a look that sent no entity tag is told when GitHub answers that nothing changed, which it cannot.
const NOT_MODIFIED_WITHOUT_TAG: &str = "GitHub answered that nothing changed to a look that sent no entity tag";

/// The look in what `gh api <path> -i` printed: on stdout the status line, the headers, a blank line and the body; on
/// stderr what gh says of a failure, which is also what it says of a 304. The exit status is no part of it: gh exits 1
/// on a 304. A 200 gives the runs, a 304 `Unchanged`; any other status, and no status at all, a failure of the kind
/// `network_or_api` with what gh or GitHub said.
fn parse_look(stdout: &str, stderr: &str) -> Result<Look, Failure> {
    let (head, body) = split_answer(stdout);
    let status = head
        .lines()
        .next()
        .filter(|l| l.starts_with("HTTP/"))
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|code| code.parse::<u16>().ok());
    match status {
        Some(304) => Ok(Look::Unchanged),
        Some(200) => {
            let etag = head.lines().skip(1).find_map(|l| {
                let (name, value) = l.split_once(':')?;
                name.trim()
                    .eq_ignore_ascii_case("etag")
                    .then(|| value.trim().to_string())
            });
            let answer: Value = serde_json::from_str(body)
                .map_err(|e| Failure::other(format!("the answer is not JSON ({e}): {}", tail(body, 3))))?;
            let runs = answer["workflow_runs"]
                .as_array()
                .ok_or_else(|| Failure::other("the answer has no workflow_runs"))?;
            Ok(Look::Changed {
                etag,
                runs: Value::Array(runs.iter().map(run_fields).collect()),
            })
        }
        Some(code) => {
            let said = if stderr.trim().is_empty() { body } else { stderr };
            Err(Failure::network(format!("HTTP {code}: {}", said.trim())))
        }
        None => Err(Failure::network(format!("no HTTP answer: {}", stderr.trim()))),
    }
}

/// The head and the body of what `gh api -i` printed on stdout, cut at the first blank line after the status line. The
/// status line ends in a line feed alone and every header in a carriage return and a line feed.
fn split_answer(text: &str) -> (&str, &str) {
    let mut at = 0;
    for line in text.split_inclusive('\n') {
        if at > 0 && line.trim_end_matches(['\r', '\n']).is_empty() {
            return (&text[..at], &text[at + line.len()..]);
        }
        at += line.len();
    }
    (text, "")
}

/// A run as the API of runs gives it, under the names gh's own listing of runs uses (`databaseId`, `url`, `attempt`),
/// which the rest of the engine reads.
fn run_fields(run: &Value) -> Value {
    serde_json::json!({
        "databaseId": run["id"],
        "status": run["status"],
        "conclusion": run["conclusion"],
        "url": run["html_url"],
        "attempt": run["run_attempt"],
    })
}

/// Whether the API takes a workflow written so in its path as it is: its file name or its id, as gh does.
fn named_by_file_or_id(workflow: &str) -> bool {
    workflow.parse::<u64>().is_ok() || workflow.ends_with(".yml") || workflow.ends_with(".yaml")
}

/// The id of the workflow a name names in a repository's list of workflows (`GET .../actions/workflows`): the one whose
/// name or file name is that, compared as gh does, without regard to case. A failure, naming it, when none is, when
/// several are, or when the list is cut short, since the one wanted may be past its end.
fn workflow_named(listed: &Value, name: &str) -> Result<String, Failure> {
    let all = listed["workflows"]
        .as_array()
        .ok_or_else(|| Failure::other("GitHub gave no list of workflows"))?;
    let named: Vec<&Value> = all
        .iter()
        .filter(|w| {
            let file = w["path"].as_str().and_then(|p| p.rsplit('/').next());
            w["name"].as_str().is_some_and(|n| n.eq_ignore_ascii_case(name))
                || file.is_some_and(|f| f.eq_ignore_ascii_case(name))
        })
        .collect();
    match named.as_slice() {
        [one] => one["id"]
            .as_i64()
            .map(|id| id.to_string())
            .ok_or_else(|| Failure::other(format!("the workflow {name} has no id in GitHub's list"))),
        [] if listed["total_count"].as_u64().unwrap_or(0) > all.len() as u64 => Err(Failure::other(format!(
            "no workflow named {name} among the first {} of the repository's; name it by its file name",
            all.len()
        ))),
        [] => Err(Failure::other(format!("the repository has no workflow named {name}"))),
        many => Err(Failure::other(format!(
            "{} workflows are named {name}; name it by its file name",
            many.len()
        ))),
    }
}

/// How long a wait for a run lasts.
struct Waits {
    /// From the start of one look to the start of the next.
    poll: Duration,
    /// How long the run may take to appear, and to finish.
    start_timeout: Duration,
    run_timeout: Duration,
}

/// The run a wait is for: the latest of the workflow's runs on a commit that is numbered above `run_above` and, once
/// finished, at an attempt above `after`, when those are given.
struct Wanted<'a> {
    workflow: &'a str,
    sha: &'a str,
    after: Option<i64>,
    run_above: Option<i64>,
}

/// What a wait for a run needs of time: how long it has lasted, and a way to let time pass. The tests keep the time.
trait Clock {
    fn elapsed(&self) -> Duration;
    fn sleep(&self, how_long: Duration);
}

/// The clock of the wall, from the start of the wait.
struct Wall(Instant);

impl Clock for Wall {
    fn elapsed(&self) -> Duration {
        self.0.elapsed()
    }

    fn sleep(&self, how_long: Duration) {
        std::thread::sleep(how_long);
    }
}

/// Looks at a workflow's runs on a commit until the run wanted has finished, and gives it. A look starts every
/// `waits.poll`, counted from the start of the one before, so that the time a look takes is not added to the wait. Each
/// look sends the entity tag of the last answer that gave runs, and an answer that nothing changed leaves those runs as
/// they were. Fails when the run has not appeared in `waits.start_timeout` or has not finished in `waits.run_timeout`.
fn wait_for_run(
    waits: &Waits,
    clock: &dyn Clock,
    wanted: &Wanted,
    mut look: impl FnMut(Option<&str>) -> Result<Look, Failure>,
) -> Result<Value, Failure> {
    let (workflow, sha) = (wanted.workflow, wanted.sha);
    // The entity tag of the last answer, and the runs it gave.
    let mut seen: Option<(Option<String>, Value)> = None;
    loop {
        let began = clock.elapsed();
        let sent = seen.as_ref().and_then(|(etag, _)| etag.clone());
        match look(sent.as_deref())? {
            Look::Changed { etag, runs } => seen = Some((etag, runs)),
            Look::Unchanged if sent.is_some() => {}
            Look::Unchanged => return Err(Failure::other(NOT_MODIFIED_WITHOUT_TAG)),
        }
        let latest = seen.as_ref().and_then(|(_, runs)| newest_above(runs, wanted.run_above));
        let waited = clock.elapsed();
        match latest {
            None if waited > waits.start_timeout => {
                return Err(Failure {
                    kind: "ci_did_not_start",
                    what: format!(
                        "no run of {workflow} on {sha} after {} s",
                        waits.start_timeout.as_secs()
                    ),
                });
            }
            Some(r)
                if r["status"] == "completed"
                    && wanted.after.is_none_or(|a| r["attempt"].as_i64().unwrap_or(0) > a) =>
            {
                return Ok(r);
            }
            _ if waited > waits.run_timeout => {
                return Err(Failure {
                    kind: "timed_out",
                    what: format!(
                        "the run of {workflow} on {sha} did not finish in {} s",
                        waits.run_timeout.as_secs()
                    ),
                });
            }
            _ => clock.sleep(waits.poll.saturating_sub(clock.elapsed().saturating_sub(began))),
        }
    }
}

/// The last `n` lines of a text.
pub fn tail(text: &str, n: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    lines[lines.len().saturating_sub(n)..].join("\n")
}

/// The most lines of a red job's log given whole once the steps that passed, the runner's set-up and clean-up and the
/// lines that only show progress are left out; a longer one is given as its failures, its summary and its errors.
const WHOLE: usize = 80;

/// The most lines given of a red job's log, however many failures and errors it has: the first of them and its end.
const MOST: usize = 120;

/// The lines given at the end of a red job's log cut to [`MOST`]: its summary and the error that ended its step.
const END: usize = 30;

/// The most lines an error is given with, its own included, up to the blank line that ends it.
const ERROR_LINES: usize = 30;

/// The words a line of cargo or nextest begins with, right-aligned to twelve columns, that only show progress: a crate
/// fetched or built, a test started, passed, skipped or slow.
const PROGRESS: [&str; 22] = [
    "Adding",
    "Archiving",
    "Blocking",
    "Checking",
    "Compiling",
    "Documenting",
    "Downloaded",
    "Downloading",
    "Finished",
    "Fresh",
    "Installed",
    "Installing",
    "Locking",
    "Packaging",
    "PASS",
    "Removed",
    "SKIP",
    "SLOW",
    "START",
    "Unpacking",
    "Updating",
    "Verifying",
];

/// The word a line of cargo or nextest begins with, right-aligned to twelve columns and followed by a space, such as
/// `   Compiling`, `        PASS` or `     Summary`; none for any other line.
fn status(line: &str) -> Option<&str> {
    let head = line.get(..12)?;
    let word = head.trim_start();
    let right_aligned = !word.is_empty() && !word.contains(' ') && line[12..].starts_with(' ');
    right_aligned.then_some(word)
}

/// Whether a line only shows progress: cargo fetching or building a crate, nextest starting, passing, skipping or
/// waiting on a test, or libtest's line of a test that passed.
fn progress(line: &str) -> bool {
    status(line).is_some_and(|w| PROGRESS.contains(&w))
        || (line.starts_with("test ") && (line.ends_with(" ... ok") || line.ends_with(" ... ignored")))
}

/// Whether a line is nextest's of a test that did not pass, as `        FAIL [   0.466s] (139/146) bin name`: failed,
/// timed out, leaked, aborted or killed by a signal.
fn failure(line: &str) -> bool {
    status(line).is_some_and(|w| !PROGRESS.contains(&w) && w.chars().all(|c| c.is_ascii_uppercase() || c == '-'))
        && line[12..].trim_start().starts_with('[')
}

/// Whether a line begins an error: rustc's, cargo's, clippy's and nextest's errors and warnings, a diff of rustfmt,
/// libtest's failures and result, and the runner's `##[error]`.
fn error_begins(line: &str) -> bool {
    let rustc = ["error", "warning"].iter().any(|w| {
        line.strip_prefix(w)
            .is_some_and(|rest| rest.starts_with(':') || rest.starts_with('['))
    });
    rustc
        || line.starts_with("Diff in ")
        || line.starts_with("##[error]")
        || line.starts_with("failures:")
        || line.starts_with("test result: FAILED")
        || (line.starts_with("---- ") && line.ends_with(" ----"))
}

/// What of a red job's log tells why it is red, from the log of its failed steps as gh gives it: the steps that
/// failed, those with an `##[error]` line, each from its command, without the step's set-up (the shell and the
/// environment it ran with), the lines that only show progress and repeated blank lines; with no step that failed,
/// every step but the runner's set-up and clean-up. Up to [`WHOLE`] lines it is given whole; a longer one is given as
/// the failures (each test that did not pass with its output), the summary and each error with the lines under it,
/// what is left out between them said as how many lines; and at most [`MOST`] lines, the first and the end, since the
/// whole log is kept at the check's `log_path`. Instead of the log's last lines, which were the runner's clean-up and
/// tests that passed, and missed the start of a failure.
pub fn what_failed(log: &str) -> String {
    let lines: Vec<String> = clean_log(log).lines().map(str::to_string).collect();
    let kept = failed_steps(&lines);
    let kept = if kept.len() > WHOLE { picked(&kept) } else { kept };
    capped(kept, MOST).join("\n")
}

/// The lines of the steps of a job's log that failed, as [`what_failed`] takes them.
fn failed_steps(lines: &[String]) -> Vec<String> {
    // A step begins with the group that shows its command, `##[group]Run …`; the runner's clean-up with `Post job
    // cleanup.`; what comes before the first step is the runner's set-up.
    let mut steps: Vec<Vec<&str>> = vec![Vec::new()];
    for l in lines {
        if l.starts_with("##[group]Run ") || l.starts_with("Post job cleanup") {
            steps.push(Vec::new());
        }
        if let Some(s) = steps.last_mut() {
            s.push(l.as_str());
        }
    }
    let cleanup = |s: &&Vec<&str>| s.first().is_some_and(|l| l.starts_with("Post job cleanup"));
    let ran: Vec<&Vec<&str>> = steps.iter().skip(1).filter(|s| !cleanup(s)).collect();
    let failed: Vec<&Vec<&str>> = ran
        .iter()
        .copied()
        .filter(|s| s.iter().any(|l| l.starts_with("##[error]")))
        .collect();
    let chosen = match (failed.is_empty(), ran.is_empty()) {
        (false, _) => failed,
        (true, false) => ran,
        // A log with no step in it is all one.
        (true, true) => steps.iter().collect(),
    };
    let mut out: Vec<String> = Vec::new();
    for s in chosen {
        let mut header = s.first().is_some_and(|l| l.starts_with("##[group]Run "));
        for (i, l) in s.iter().enumerate() {
            if header {
                if i == 0 {
                    out.push(l.trim_start_matches("##[group]").to_string());
                } else if *l == "##[endgroup]" {
                    header = false;
                }
                continue;
            }
            if progress(l) {
                continue;
            }
            if l.trim().is_empty() && out.last().is_some_and(|x| x.trim().is_empty()) {
                continue;
            }
            out.push(l.to_string());
        }
    }
    out
}

/// The lines of a long red log that tell why it is red: each step's command, each failure with the lines under it up
/// to the next test's line, the summary to the end, and each error with the lines under it up to a blank line; the
/// rest said as how many lines were left out, but for blank lines between what is given.
fn picked(lines: &[String]) -> Vec<String> {
    let n = lines.len();
    let mut keep = vec![false; n];
    // Nextest's summary: from the rule before it to the end.
    let summary = lines
        .iter()
        .rposition(|l| l.trim_start().starts_with("Summary ["))
        .map(|i| {
            if i > 0 && lines[i - 1].starts_with('\u{2500}') {
                i - 1
            } else {
                i
            }
        });
    let mut i = 0;
    while i < n {
        if summary == Some(i) {
            keep[i..].iter_mut().for_each(|k| *k = true);
            break;
        }
        let l = &lines[i];
        if l.starts_with("Run ") {
            keep[i] = true;
        }
        if failure(l) {
            keep[i] = true;
            i += 1;
            while i < n && status(&lines[i]).is_none() && !lines[i].starts_with('\u{2500}') && summary != Some(i) {
                keep[i] = true;
                i += 1;
            }
            continue;
        }
        if error_begins(l) {
            let from = i;
            while i < n && i - from < ERROR_LINES && (i == from || !lines[i].trim().is_empty()) && summary != Some(i) {
                keep[i] = true;
                i += 1;
            }
            continue;
        }
        i += 1;
    }
    let mut out = Vec::new();
    let mut left: Vec<&str> = Vec::new();
    for (k, l) in keep.iter().zip(lines) {
        if *k {
            if left.iter().any(|x| !x.trim().is_empty()) {
                out.push(format!("[{} lines left out]", left.len()));
            } else {
                out.extend(left.iter().map(|x| x.to_string()));
            }
            left.clear();
            out.push(l.clone());
        } else {
            left.push(l);
        }
    }
    if left.iter().any(|x| !x.trim().is_empty()) {
        out.push(format!("[{} lines left out]", left.len()));
    }
    out
}

/// At most `most` lines: the first and the [`END`] last, with how many were left out between them.
fn capped(lines: Vec<String>, most: usize) -> Vec<String> {
    if lines.len() <= most {
        return lines;
    }
    let head = most - END - 1;
    let left = lines.len() - head - END;
    let mut out: Vec<String> = lines[..head].to_vec();
    out.push(format!(
        "[{left} lines left out: the whole log is at the check's log_path]"
    ));
    out.extend_from_slice(&lines[lines.len() - END..]);
    out
}

/// A job's log as gh gives it, as a reader wants it: without the job, the step and the time gh puts before every
/// line ("job<TAB>step<TAB>2026-10-27T09:47:49.0473549Z "), and without the escapes that colour the text.
pub fn clean_log(log: &str) -> String {
    log.lines().map(clean_line).collect::<Vec<_>>().join("\n")
}

fn clean_line(line: &str) -> String {
    let line = line.trim_start_matches('\u{feff}');
    let mut parts = line.splitn(3, '\t');
    // The log's first line has a byte order mark before its time.
    let text = match (parts.next(), parts.next(), parts.next()) {
        (Some(_), Some(_), Some(rest)) => rest.trim_start_matches('\u{feff}'),
        _ => line,
    };
    let text = match text.split_once(' ') {
        Some((stamp, rest)) if is_timestamp(stamp) => rest,
        _ if is_timestamp(text) => "",
        _ => text,
    };
    without_escapes(text)
}

/// Whether a word is a time as GitHub writes one in logs: 2026-10-27T09:47:49.0473549Z.
fn is_timestamp(w: &str) -> bool {
    let b = w.as_bytes();
    b.len() >= 20
        && b[..4].iter().all(u8::is_ascii_digit)
        && b[4] == b'-'
        && b[7] == b'-'
        && b[10] == b'T'
        && b[13] == b':'
        && w.ends_with('Z')
}

/// A text without its terminal escapes: ESC [ ... a letter. gh writes the escape character of a job's log as the two
/// characters "^[", so "^[[1m" is an escape too.
fn without_escapes(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        let escape = match c {
            '\u{1b}' => true,
            '^' if chars.peek() == Some(&'[') => {
                chars.next();
                true
            }
            _ => false,
        };
        if !escape {
            out.push(c);
            continue;
        }
        if chars.peek() == Some(&'[') {
            chars.next();
            for x in chars.by_ref() {
                if ('@'..='~').contains(&x) {
                    break;
                }
            }
        }
    }
    out
}

/// Every file under a directory, each as its path under it with `/` between its parts, after `prefix`, and its bytes.
fn files_under(dir: &Path, prefix: &str, out: &mut Vec<ArtifactFile>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = format!("{prefix}{}", entry.file_name().to_string_lossy());
        if entry.file_type()?.is_dir() {
            files_under(&entry.path(), &format!("{path}/"), out)?;
        } else {
            out.push((path, std::fs::read(entry.path())?));
        }
    }
    Ok(())
}

/// The run and the job a check's link names: .../actions/runs/<run>/job/<job>.
fn run_and_job(link: &str) -> Option<(String, String)> {
    let (_, rest) = link.split_once("/actions/runs/")?;
    let mut parts = rest.split('/');
    let run = parts.next()?.to_string();
    (parts.next()? == "job").then_some(())?;
    let job = parts.next()?.split(['?', '#']).next()?.to_string();
    Some((run, job))
}

impl GitHub for Gh {
    fn pull_request(
        &self,
        repo: &str,
        branch: &str,
        base: &str,
        title_body: Option<(&str, &str)>,
        existing: Option<i64>,
    ) -> Result<i64, Failure> {
        // A PR opened before the engine could record it, by a run that stopped, is the work's PR still.
        let existing = match existing {
            Some(n) => Some(n),
            None => {
                let open = self.json(&[
                    "pr", "list", "-R", repo, "--head", branch, "--state", "open", "--json", "number",
                ])?;
                open.as_array()
                    .and_then(|ps| ps.first())
                    .and_then(|p| p["number"].as_i64())
            }
        };
        if let Some(n) = existing {
            if let Some((title, body)) = title_body {
                self.run(&[
                    "pr",
                    "edit",
                    &n.to_string(),
                    "-R",
                    repo,
                    "--title",
                    title,
                    "--body",
                    body,
                ])?;
            }
            return Ok(n);
        }
        let (title, body) = title_body.ok_or_else(|| Failure::other("a first push needs the PR's title and body"))?;
        let url = self.run(&[
            "pr", "create", "-R", repo, "--draft", "--head", branch, "--base", base, "--title", title, "--body", body,
        ])?;
        url.trim()
            .rsplit('/')
            .next()
            .and_then(|n| n.parse().ok())
            .ok_or_else(|| Failure::other(format!("gh pr create gave no PR number: {url}")))
    }

    fn checks(&self, repo: &str, sha: &str, workflow: &str) -> Result<Vec<CheckRun>, Failure> {
        self.checks_after(repo, sha, workflow, None)
    }

    fn checks_after(
        &self,
        repo: &str,
        sha: &str,
        workflow: &str,
        after: Option<i64>,
    ) -> Result<Vec<CheckRun>, Failure> {
        let run = self.finished_run(repo, sha, workflow, None, after)?;
        self.jobs(repo, &run)
    }

    fn latest_run_id(&self, repo: &str, sha: &str, workflow: &str) -> Result<Option<i64>, Failure> {
        Ok(newest_above(&self.runs(repo, sha, workflow)?, None).and_then(|r| r["databaseId"].as_i64()))
    }

    fn branch_head(&self, repo: &str, branch: &str) -> Result<String, Failure> {
        let out = self.run(&["api", &format!("repos/{repo}/branches/{branch}"), "--jq", ".commit.sha"])?;
        let sha = out.trim();
        if sha.is_empty() {
            return Err(Failure::other(format!("GitHub gave no commit for {repo}'s {branch}")));
        }
        Ok(sha.to_string())
    }

    fn finished_checks(&self, repo: &str, sha: &str, workflow: &str) -> Result<Option<Vec<CheckRun>>, Failure> {
        match newest_above(&self.runs(repo, sha, workflow)?, None) {
            Some(run) if run["status"] == "completed" => self.jobs(repo, &run).map(Some),
            _ => Ok(None),
        }
    }

    fn dispatch(&self, repo: &str, workflow: &str, branch: &str, inputs: &[(&str, &str)]) -> Result<(), Failure> {
        let fields: Vec<String> = inputs.iter().map(|(k, v)| format!("{k}={v}")).collect();
        let mut args = vec!["workflow", "run", workflow, "-R", repo, "--ref", branch];
        for f in &fields {
            args.extend(["-f", f.as_str()]);
        }
        self.run_said(&args).map(|_| ()).map_err(|(failure, said)| {
            if dispatch_rejected(&said) {
                Failure::rejected(failure.what)
            } else {
                failure
            }
        })
    }

    fn job_log(&self, repo: &str, check: &CheckRun) -> Result<String, Failure> {
        let (run, job) = run_and_job(&check.run)
            .ok_or_else(|| Failure::other(format!("no run and job in the check's link: {}", check.run)))?;
        self.run(&["run", "view", &run, "-R", repo, "--log", "--job", &job])
    }

    fn artifact(&self, repo: &str, check: &CheckRun, name: &str) -> Result<Option<Vec<ArtifactFile>>, Failure> {
        let (run, _) = run_and_job(&check.run)
            .ok_or_else(|| Failure::other(format!("no run in the check's link: {}", check.run)))?;
        let listed = self.json(&[
            "api",
            &format!("repos/{repo}/actions/runs/{run}/artifacts?per_page=100"),
        ])?;
        let there = listed["artifacts"]
            .as_array()
            .is_some_and(|xs| xs.iter().any(|x| x["name"] == name && x["expired"] != true));
        if !there {
            return Ok(None);
        }
        let dir = std::env::temp_dir().join(format!("wf-agents-artifact-{run}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let got = self
            .run(&[
                "run",
                "download",
                &run,
                "-R",
                repo,
                "-n",
                name,
                "-D",
                &dir.to_string_lossy(),
            ])
            .and_then(|_| {
                let mut files = Vec::new();
                files_under(&dir, "", &mut files)
                    .map_err(|e| Failure::other(format!("reading the artifact {name}: {e}")))?;
                Ok(files)
            });
        let _ = std::fs::remove_dir_all(&dir);
        got.map(Some)
    }

    fn rerun(&self, repo: &str, sha: &str, workflow: &str) -> Result<Vec<CheckRun>, Failure> {
        let run = self.finished_run(repo, sha, workflow, None, None)?;
        let id = run["databaseId"].as_i64().unwrap_or_default().to_string();
        let attempt = run["attempt"].as_i64().unwrap_or(1);
        self.run(&["run", "rerun", &id, "-R", repo, "--failed"])?;
        let run = self.finished_run(repo, sha, workflow, Some(attempt), None)?;
        self.jobs(repo, &run)
    }

    fn base(&self, repo: &str, pr: i64) -> Result<String, Failure> {
        let n = pr.to_string();
        let v = self.json(&["pr", "view", &n, "-R", repo, "--json", "baseRefName"])?;
        v["baseRefName"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| Failure::other(format!("gh gave no base for PR {n}")))
    }

    fn set_base(&self, repo: &str, pr: i64, base: &str) -> Result<(), Failure> {
        self.run(&["pr", "edit", &pr.to_string(), "-R", repo, "--base", base])
            .map(|_| ())
    }

    fn merge_commit(&self, repo: &str, pr: i64) -> Result<Option<String>, Failure> {
        let n = pr.to_string();
        // `state` tells a pull request that merged from one that was closed; `mergeCommit` is null until it merges.
        let v = self.json(&["pr", "view", &n, "-R", repo, "--json", "state,mergeCommit"])?;
        merged_oid(&v, &n)
    }

    fn merge(&self, repo: &str, pr: i64) -> Result<String, Failure> {
        let n = pr.to_string();
        self.run(&["pr", "ready", &n, "-R", repo])?;
        self.run(&["pr", "merge", &n, "-R", repo, "--squash"])?;
        self.merge_commit(repo, pr)?
            .ok_or_else(|| Failure::other(format!("PR {n} is not merged after gh pr merge")))
    }

    fn close(&self, repo: &str, pr: i64, comment: &str) -> Result<(), Failure> {
        self.run(&["pr", "close", &pr.to_string(), "-R", repo, "--comment", comment])
            .map(|_| ())
    }

    fn gh(&self, dir: &Path, args: &[String]) -> Result<(i32, String), Failure> {
        let out = Command::new("gh")
            .args(args)
            .current_dir(dir)
            .output()
            .map_err(|e| Failure::other(format!("gh: {e}")))?;
        let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
        let err = String::from_utf8_lossy(&out.stderr);
        if !err.trim().is_empty() {
            text.push_str(&err);
        }
        Ok((out.status.code().unwrap_or(-1), text))
    }
}

/// The merge commit in what `gh pr view N --json state,mergeCommit` gave, `n` the pull request's number: only a pull
/// request in the state `MERGED` has one. One that is merged and has none is a failure, not a pull request that did
/// not merge.
fn merged_oid(v: &Value, n: &str) -> Result<Option<String>, Failure> {
    if v["state"] != "MERGED" {
        return Ok(None);
    }
    v["mergeCommit"]["oid"]
        .as_str()
        .map(|s| Some(s.to_string()))
        .ok_or_else(|| Failure::other(format!("PR {n} merged but gh gave no merge commit")))
}

/// Whether what `gh workflow run` said (its stderr, not the command line) is GitHub refusing the request itself (HTTP 422: an input the workflow requires
/// is missing, one it does not take is given, a value is not allowed, the workflow cannot run by hand), which running
/// it again cannot change; not a failure to reach GitHub, nor a ref GitHub has not seen yet after the push, which a
/// later try gets through.
fn dispatch_rejected(said: &str) -> bool {
    said.contains("HTTP 422") && !said.contains("No ref found")
}

/// Pushes a commit to a branch or ref of the worktree's origin; it must fast-forward unless forced.
pub fn push(worktree: &Path, sha: &str, refname: &str, force: bool) -> Result<(), Failure> {
    let spec = format!("{}{sha}:{refname}", if force { "+" } else { "" });
    crate::git::git(worktree, &["push", "-q", "origin", &spec])
        .map(|_| ())
        .map_err(Failure::network)
}

/// Deletes a ref on the worktree's origin.
pub fn delete_ref(worktree: &Path, refname: &str) -> Result<(), Failure> {
    crate::git::git(worktree, &["push", "-q", "origin", &format!(":{refname}")])
        .map(|_| ())
        .map_err(Failure::network)
}

#[cfg(test)]
mod tests {
    use super::*;

    // A wait for a run made after a point takes only a run numbered above it: one at the point, or below, was made
    // before, however recent its last attempt, and the newest above it is the one with the largest number, in
    // whatever order gh lists them. With nothing above the point there is no run yet, and the wait goes on.
    #[test]
    fn a_run_numbered_at_or_below_the_point_is_never_the_run_waited_for() {
        let runs = serde_json::json!([
            { "databaseId": 7, "status": "completed", "attempt": 3 },
            { "databaseId": 9, "status": "in_progress", "attempt": 1 },
            { "databaseId": 8, "status": "completed", "attempt": 1 },
        ]);
        let id = |above| newest_above(&runs, above).map(|r| r["databaseId"].as_i64().unwrap());
        assert_eq!(id(None), Some(9));
        assert_eq!(id(Some(7)), Some(9));
        assert_eq!(id(Some(8)), Some(9));
        assert_eq!(id(Some(9)), None);
        assert_eq!(id(Some(100)), None);
        assert_eq!(newest_above(&serde_json::json!([]), None), None);
        assert_eq!(newest_above(&serde_json::json!({}), None), None);
    }

    /// What `gh api <path> -i` prints: the status line ends in a line feed alone, every header in a carriage return and
    /// a line feed, and a blank line comes before the body.
    fn printed(status: &str, headers: &[&str], body: &str) -> String {
        let headers: String = headers.iter().map(|h| format!("{h}\r\n")).collect();
        format!("{status}\n{headers}\r\n{body}")
    }

    // The answers gh printed on 2026-11-04 UTC to a look at the runs of fast.yml of Ming-Research/wf-agents, their headers
    // cut to those the look reads and a few others, and the run's fields to those a look reads and a few others.
    const SPENT: &str = "X-Ratelimit-Used: 1335";
    const NO_RUNS: &str = "{\"total_count\":0,\"workflow_runs\":[]}";
    const NO_RUNS_TAG: &str = "W/\"85b3e85b6d9542f753944c2b8e2861a56e4e32e247e249cf6e7e67cec4c8b2bf\"";
    const ONE_RUN: &str = "{\"total_count\":356,\"workflow_runs\":[{\"id\":41932573157,\"name\":\"fast\",\
        \"head_branch\":\"wf-agents/288-322\",\"head_sha\":\"4aa1516e75cfea2bad0eb94191a089c99d21db51\",\
        \"path\":\".github/workflows/fast.yml\",\"run_number\":356,\"event\":\"workflow_dispatch\",\
        \"status\":\"in_progress\",\"conclusion\":null,\"workflow_id\":405492466,\
        \"url\":\"https://api.github.com/repos/Ming-Research/wf-agents/actions/runs/41932573157\",\
        \"html_url\":\"https://github.com/Ming-Research/wf-agents/actions/runs/41932573157\",\
        \"pull_requests\":[],\"created_at\":\"2026-11-04T23:17:48Z\",\"updated_at\":\"2026-11-04T23:17:53Z\",\
        \"run_attempt\":1,\"run_started_at\":\"2026-11-04T23:17:48Z\"}]}";
    const ONE_RUN_TAG: &str = "W/\"954e1d0912784720ad503b1b0fc68962566d434b632251e80968f5b78fbbac4c\"";

    // A look reads what gh printed: a 200 gives the runs under the names the engine reads, not the API's own, and the
    // entity tag as GitHub wrote it, to be sent back whole; a 304, which gh exits 1 on, gives that nothing changed; any
    // other status, or none, is a failure carrying what gh or GitHub said.
    #[test]
    fn a_look_reads_what_gh_api_printed() {
        fn headers(tag: &str) -> [&str; 5] {
            [
                "Cache-Control: private, max-age=60, s-maxage=60",
                "Content-Type: application/json; charset=utf-8",
                tag,
                SPENT,
                "X-Xss-Protection: 0",
            ]
        }
        let none = printed("HTTP/2.0 200 OK", &headers(&format!("Etag: {NO_RUNS_TAG}")), NO_RUNS);
        assert_eq!(
            parse_look(&none, ""),
            Ok(Look::Changed {
                etag: Some(NO_RUNS_TAG.to_string()),
                runs: serde_json::json!([])
            })
        );

        let one = printed("HTTP/2.0 200 OK", &headers(&format!("Etag: {ONE_RUN_TAG}")), ONE_RUN);
        let Ok(Look::Changed { etag, runs }) = parse_look(&one, "") else {
            panic!("a 200 gives the runs");
        };
        assert_eq!(etag.as_deref(), Some(ONE_RUN_TAG));
        // `url` is the API's address of the run and `html_url` the page's, which is what the engine's `url` is.
        assert_eq!(
            runs,
            serde_json::json!([{
                "databaseId": 41932573157_i64,
                "status": "in_progress",
                "conclusion": null,
                "url": "https://github.com/Ming-Research/wf-agents/actions/runs/41932573157",
                "attempt": 1,
            }])
        );
        // The header's name is read in any case, and an answer with none sends none back.
        let lower = printed("HTTP/1.1 200 OK", &["etag: W/\"x\""], NO_RUNS);
        assert_eq!(
            parse_look(&lower, ""),
            Ok(Look::Changed {
                etag: Some("W/\"x\"".to_string()),
                runs: serde_json::json!([])
            })
        );
        let untagged = printed("HTTP/2.0 200 OK", &[SPENT], NO_RUNS);
        assert!(matches!(
            parse_look(&untagged, ""),
            Ok(Look::Changed { etag: None, .. })
        ));

        // The 304 as gh printed it: the entity tag in its strong form, no body, and `gh: HTTP 304` on stderr.
        let strong = format!("Etag: {}", NO_RUNS_TAG.trim_start_matches("W/"));
        let kept = printed("HTTP/2.0 304 Not Modified", &headers(&strong), "");
        assert_eq!(parse_look(&kept, "gh: HTTP 304\n"), Ok(Look::Unchanged));

        let refused = printed("HTTP/2.0 404 Not Found", &[SPENT], "{\"message\":\"Not Found\"}");
        let f = parse_look(&refused, "gh: it is not there (HTTP 404)\n").unwrap_err();
        assert_eq!(
            (f.kind, f.what.as_str()),
            ("network_or_api", "HTTP 404: gh: it is not there (HTTP 404)")
        );
        let f = parse_look(&refused, "").unwrap_err();
        assert_eq!(f.what, "HTTP 404: {\"message\":\"Not Found\"}");
        let f = parse_look("", "error connecting to api.github.com\n").unwrap_err();
        assert_eq!(
            (f.kind, f.what.as_str()),
            ("network_or_api", "no HTTP answer: error connecting to api.github.com")
        );
        assert_eq!(
            parse_look(&printed("HTTP/2.0 200 OK", &[SPENT], "not json"), "")
                .unwrap_err()
                .kind,
            "other"
        );
    }

    // A workflow written as a file name or an id goes into the path of the API as it is; any other name, as a project's
    // configuration writes it (`fast`), is the one workflow of the repository's list that has it as its name or its
    // file name, without regard to case; none, several, or a list cut short is a failure that names it.
    #[test]
    fn a_workflow_is_found_by_its_file_name_its_id_or_its_name() {
        for as_is in ["fast.yml", "ci.yaml", "405492466"] {
            assert!(named_by_file_or_id(as_is), "{as_is}");
        }
        for by_name in ["fast", "ci", "Fast CI"] {
            assert!(!named_by_file_or_id(by_name), "{by_name}");
        }
        let workflow = |id: i64, name: &str, file: &str| serde_json::json!({ "id": id, "name": name, "path": format!(".github/workflows/{file}"), "state": "active" });
        let listed = serde_json::json!({
            "total_count": 4,
            "workflows": [
                workflow(405492466, "fast", "fast.yml"),
                workflow(11, "ci", "ci.yml"),
                workflow(12, "measure", "measure.yml"),
                workflow(13, "ci", "ci-again.yml"),
            ],
        });
        assert_eq!(workflow_named(&listed, "fast"), Ok("405492466".to_string()));
        assert_eq!(workflow_named(&listed, "FAST"), Ok("405492466".to_string()));
        assert_eq!(workflow_named(&listed, "measure.yml"), Ok("12".to_string()));
        assert_eq!(
            workflow_named(&listed, "ci").unwrap_err().what,
            "2 workflows are named ci; name it by its file name"
        );
        assert_eq!(
            workflow_named(&listed, "nightly").unwrap_err().what,
            "the repository has no workflow named nightly"
        );
        let first: Vec<Value> = (0..100)
            .map(|i| workflow(i, &format!("w{i}"), &format!("w{i}.yml")))
            .collect();
        let cut = serde_json::json!({ "total_count": 130, "workflows": first });
        assert_eq!(workflow_named(&cut, "w7"), Ok("7".to_string()));
        assert_eq!(
            workflow_named(&cut, "w120").unwrap_err().what,
            "no workflow named w120 among the first 100 of the repository's; name it by its file name"
        );
    }

    /// The time of a wait kept by the test: letting time pass moves it, and so does each look, by what it takes.
    struct Fake(std::cell::Cell<Duration>);

    impl Clock for Fake {
        fn elapsed(&self) -> Duration {
            self.0.get()
        }

        fn sleep(&self, how_long: Duration) {
            self.0.set(self.0.get() + how_long);
        }
    }

    const LOOK_TAKES: Duration = Duration::from_millis(600);

    /// For each look of a wait, when it started and the entity tag it sent.
    type Looks = Vec<(Duration, Option<String>)>;

    /// A wait for a run whose n-th look (from 0) finds what `script` says, each look taking [`LOOK_TAKES`]: what the
    /// wait gave, and the looks it made.
    fn waited(
        waits: &Waits,
        wanted: &Wanted,
        mut script: impl FnMut(usize) -> Look,
    ) -> (Result<Value, Failure>, Looks) {
        let clock = Fake(std::cell::Cell::new(Duration::ZERO));
        let mut looks = Vec::new();
        let run = wait_for_run(waits, &clock, wanted, |etag| {
            looks.push((clock.elapsed(), etag.map(str::to_string)));
            clock.sleep(LOOK_TAKES);
            Ok(script(looks.len() - 1))
        });
        (run, looks)
    }

    fn waits() -> Waits {
        Waits {
            poll: Duration::from_secs(5),
            start_timeout: Duration::from_secs(60),
            run_timeout: Duration::from_secs(120),
        }
    }

    fn run_of(id: i64, status: &str, attempt: i64) -> Value {
        serde_json::json!({ "databaseId": id, "status": status, "attempt": attempt })
    }

    fn changed(tag: &str, runs: Vec<Value>) -> Look {
        Look::Changed {
            etag: Some(tag.to_string()),
            runs: Value::Array(runs),
        }
    }

    // The engine looks at a run every 5 s, counted from the start of the look before; each look sends the entity tag of
    // the last answer that gave runs, and an answer that nothing changed keeps those runs, so that it is neither a run
    // that is not there nor a reason to stop. The wait ends with the look that shows the run finished.
    #[test]
    fn a_run_is_looked_at_every_five_seconds_with_the_tag_of_the_last_answer() {
        assert_eq!(Gh::new(PathBuf::new()).poll, Duration::from_secs(5));
        let wanted = Wanted {
            workflow: "fast",
            sha: "abc",
            after: None,
            run_above: None,
        };
        let (run, looks) = waited(&waits(), &wanted, |n| match n {
            0 => changed("a", vec![]),
            1 | 2 => Look::Unchanged,
            3 => changed("b", vec![run_of(10, "queued", 1)]),
            4 => Look::Unchanged,
            5 => changed("c", vec![run_of(10, "in_progress", 1)]),
            6 => Look::Unchanged,
            _ => changed("d", vec![run_of(10, "completed", 1)]),
        });
        assert_eq!(run.unwrap(), run_of(10, "completed", 1));
        let tags: Vec<Option<&str>> = looks.iter().map(|(_, t)| t.as_deref()).collect();
        assert_eq!(
            tags,
            [
                None,
                Some("a"),
                Some("a"),
                Some("a"),
                Some("b"),
                Some("b"),
                Some("c"),
                Some("c")
            ]
        );
        let starts: Vec<Duration> = looks.iter().map(|(at, _)| *at).collect();
        let every_five: Vec<Duration> = (0..8).map(|n| Duration::from_secs(5 * n)).collect();
        assert_eq!(starts, every_five);
    }

    // The rules of the wait that the loop kept when it was taken out into `wait_for_run`: a run numbered at or below the
    // point is never the one; a finished run is the one only at an attempt above `after`; and a run that does not appear
    // in the first, or finish in the second, of the times allowed ends the wait with the failure of that kind. The
    // function is new, so this test cannot run before the change; the old wait as a whole is held by the tests of
    // `tests/gh_runs.rs`, which pass before it too.
    #[test]
    fn a_wait_for_a_run_takes_the_run_asked_for_and_ends_at_its_times() {
        let wait = |after, run_above| Wanted {
            workflow: "fast",
            sha: "abc",
            after,
            run_above,
        };
        let (run, _) = waited(&waits(), &wait(None, Some(7)), |_| {
            changed("a", vec![run_of(7, "completed", 3)])
        });
        let f = run.unwrap_err();
        assert_eq!(
            (f.kind, f.what.as_str()),
            ("ci_did_not_start", "no run of fast on abc after 60 s")
        );

        let (run, looks) = waited(&waits(), &wait(Some(1), None), |n| {
            changed("a", vec![run_of(9, "completed", if n < 3 { 1 } else { 2 })])
        });
        assert_eq!(run.unwrap(), run_of(9, "completed", 2));
        assert_eq!(looks.len(), 4);

        let (run, _) = waited(&waits(), &wait(None, None), |_| {
            changed("a", vec![run_of(9, "in_progress", 1)])
        });
        let f = run.unwrap_err();
        assert_eq!(
            (f.kind, f.what.as_str()),
            ("timed_out", "the run of fast on abc did not finish in 120 s")
        );
    }

    // GitHub answers 304 only to a look that sent an entity tag; if it says nothing changed to one that sent none, there
    // are no runs to keep and the wait fails instead of going on with none.
    #[test]
    fn an_answer_that_nothing_changed_to_a_look_that_sent_no_tag_is_a_failure() {
        let wanted = Wanted {
            workflow: "fast",
            sha: "abc",
            after: None,
            run_above: None,
        };
        let (run, looks) = waited(&waits(), &wanted, |_| Look::Unchanged);
        assert_eq!(run.unwrap_err().what, NOT_MODIFIED_WITHOUT_TAG);
        assert_eq!(looks.len(), 1);
        // A tag-less 200 sends none back, and then a 304 cannot be told from this one: the same failure.
        let (run, looks) = waited(&waits(), &wanted, |n| {
            if n == 0 {
                Look::Changed {
                    etag: None,
                    runs: serde_json::json!([]),
                }
            } else {
                Look::Unchanged
            }
        });
        assert_eq!(run.unwrap_err().what, NOT_MODIFIED_WITHOUT_TAG);
        assert_eq!(looks.len(), 2);
    }

    // gh's messages for a workflow run by hand that GitHub refuses are told from a failure to reach GitHub and from a
    // ref GitHub has not seen yet: only the first is a request no later try can fix.
    #[test]
    fn a_refused_dispatch_is_told_from_one_that_could_not_be_made() {
        assert!(dispatch_rejected(
            "could not create workflow dispatch event: HTTP 422: Required input 'base' not provided"
        ));
        assert!(dispatch_rejected(
            "could not create workflow dispatch event: HTTP 422: Unexpected inputs provided: [\"x\"]"
        ));
        assert!(!dispatch_rejected("HTTP 422: No ref found for: b"));
        assert!(!dispatch_rejected("error connecting to api.github.com"));
        assert!(!dispatch_rejected("HTTP 502: Bad Gateway"));
    }

    #[test]
    fn a_job_log_loses_what_gh_puts_before_each_line_and_its_colours() {
        let log = "\u{feff}check\tTest\t2026-10-27T09:47:49.0473549Z \u{1b}[1m\u{1b}[91merror\u{1b}[0m: test failed\n\
                   check\tTest\t2026-10-27T09:47:49.0475850Z \n\
                   plain line";
        assert_eq!(clean_log(log), "error: test failed\n\nplain line");
        // As gh gives a job's log, with the escape character written "^[".
        let real = "test 1 of 3\tTest\t2026-10-30T00:57:30.8321137Z ^[[32;1m        PASS^[[0m [   0.007s] ( 41/141) ^[[35;1mengine^[[0m ^[[36mtools::tests^[[0m^[[36m::^[[0m^[[34;1ma_command_line_is_split_as_a_shell_would^[[0m";
        assert_eq!(
            clean_log(real),
            "        PASS [   0.007s] ( 41/141) engine tools::tests::a_command_line_is_split_as_a_shell_would"
        );
        let link = "https://github.com/o/r/actions/runs/40483126002/job/118000000001";
        assert_eq!(
            run_and_job(link),
            Some(("40483126002".to_string(), "118000000001".to_string()))
        );
        assert_eq!(run_and_job("https://github.com/o/r/pull/3"), None);
    }

    // A red test partition, its log cut from a real one: what is given is the test step's command, the test that
    // failed with its output, the summary and the error that ended the step; not the runner's set-up and clean-up, the
    // steps that passed, the step's environment, cargo's progress or the tests that passed.
    #[test]
    fn a_red_test_is_given_with_its_output_and_the_summary() {
        let log = include_str!("../tests/job-logs/nextest-fail.log");
        let given = what_failed(log);
        for kept in [
            "Run cargo nextest run --workspace --locked --cargo-profile ci --profile ci --partition count:1/3",
            "        FAIL [   0.466s] (139/146) wf-agents::cli a_table_over_the_row_limit_says_how_many_rows_it_leaves_out",
            "    test a_table_over_the_row_limit_says_how_many_rows_it_leaves_out ... FAILED",
            "panicked at crates/wf-agents/tests/cli.rs:466:13:",
            "     Summary [ 105.999s] 142/146 tests run: 141 passed, 1 failed, 279 skipped",
            "error: test run failed",
            "##[error]Process completed with exit code 100.",
        ] {
            assert!(given.contains(kept), "{kept}\n{given}");
        }
        for gone in [
            "Current runner version",
            "actions/checkout",
            "rustup toolchain install",
            "CARGO_TERM_COLOR",
            "Downloaded",
            "Finished",
            "PASS [",
            "Post job cleanup",
            "git version",
        ] {
            assert!(!given.contains(gone), "{gone}\n{given}");
        }
    }

    // A partition that did not compile, its log cut from a real one with nine errors: too long to give whole, it is
    // given as each error with the lines under it, up to the most lines given, the first errors and the end.
    #[test]
    fn errors_are_given_with_the_lines_under_them_up_to_the_most_lines() {
        let given = what_failed(include_str!("../tests/job-logs/compile-errors.log"));
        let lines: Vec<&str> = given.lines().collect();
        assert!(lines.len() <= MOST, "{}", lines.len());
        assert_eq!(
            lines[..3],
            [
                "Run cargo nextest run --workspace --locked --cargo-profile ci --profile ci --partition count:2/3",
                "error[E0432]: unresolved import `engine::ending::EndKind`",
                "    --> crates/engine/tests/e2e.rs:5680:31",
            ]
        );
        assert!(!given.contains("Compiling"), "{given}");
        assert!(
            given.contains("lines left out: the whole log is at the check's log_path]"),
            "{given}"
        );
        assert_eq!(
            lines[lines.len() - 4..],
            [
                "error: could not compile `engine` (test \"e2e\") due to 9 previous errors",
                "warning: build failed, waiting for other jobs to finish...",
                "error: command `/home/runner/.rustup/toolchains/1.99.1-x86_64-unknown-linux-gnu/bin/cargo '--color=always' test --no-run --message-format json-render-diagnostics --workspace --profile ci --locked` exited with code 101",
                "##[error]Process completed with exit code 101.",
            ]
        );
    }

    // The job that only says which jobs failed is short, and given whole but for its step's set-up.
    #[test]
    fn a_short_red_log_is_given_whole_but_for_its_set_up() {
        assert_eq!(
            what_failed(include_str!("../tests/job-logs/check.log")),
            "Run echo \"lint and test: $RESULTS\"\nlint and test: success failure\n##[error]Process completed with \
             exit code 1."
        );
    }

    // Cargo's and nextest's lines that only show progress are known by their word, right-aligned to twelve columns;
    // a test that did not pass, and an error, are not progress.
    #[test]
    fn progress_is_known_by_its_word() {
        assert!(progress("   Compiling engine v0.1.0 (/w/crates/engine)"));
        assert!(progress("        PASS [   0.006s] (  1/146) api control::tests::a"));
        assert!(progress("test tools::tests::a ... ok"));
        assert!(!progress("        FAIL [   0.466s] (139/146) wf-agents::cli a"));
        assert!(failure("        FAIL [   0.466s] (139/146) wf-agents::cli a"));
        assert!(failure("     SIGSEGV [   0.466s] (139/146) wf-agents::cli a"));
        assert!(!failure("     Summary [ 105.999s] 142/146 tests run"));
        assert!(!progress("error[E0432]: unresolved import"));
        assert!(error_begins("error[E0432]: unresolved import"));
        assert!(!error_begins("errors: none"));
    }

    /// What setting a pull request's base does on [`OnePr`].
    #[derive(Clone, Copy)]
    enum Setting {
        Takes,
        Fails,
        /// gh says it was set, and the base is the same.
        Ignored,
    }

    /// GitHub with one pull request: its base, what setting it does, which branch it merged into, whether it was closed
    /// without merging, and how many times [`GitHub::merge`] was called. Marking ready refuses a pull request that is
    /// not open, as gh does, whether it merged or was closed.
    struct OnePr {
        base: std::sync::Mutex<String>,
        setting: Setting,
        merged_into: std::sync::Mutex<Option<String>>,
        closed: bool,
        merges: std::sync::atomic::AtomicUsize,
    }

    /// An open pull request on `base`.
    fn one_pr(base: &str, setting: Setting) -> OnePr {
        OnePr {
            base: std::sync::Mutex::new(base.to_string()),
            setting,
            merged_into: std::sync::Mutex::new(None),
            closed: false,
            merges: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    impl OnePr {
        fn merges(&self) -> usize {
            self.merges.load(std::sync::atomic::Ordering::SeqCst)
        }
    }

    impl GitHub for OnePr {
        fn pull_request(
            &self,
            _repo: &str,
            _branch: &str,
            _base: &str,
            _title_body: Option<(&str, &str)>,
            _existing: Option<i64>,
        ) -> Result<i64, Failure> {
            unreachable!("no pull request is opened here")
        }

        fn checks(&self, _repo: &str, _sha: &str, _workflow: &str) -> Result<Vec<CheckRun>, Failure> {
            unreachable!("no check runs here")
        }

        fn rerun(&self, _repo: &str, _sha: &str, _workflow: &str) -> Result<Vec<CheckRun>, Failure> {
            unreachable!("no check runs here")
        }

        fn base(&self, _repo: &str, _pr: i64) -> Result<String, Failure> {
            Ok(self.base.lock().unwrap().clone())
        }

        fn set_base(&self, _repo: &str, _pr: i64, base: &str) -> Result<(), Failure> {
            match self.setting {
                Setting::Takes => {
                    *self.base.lock().unwrap() = base.to_string();
                    Ok(())
                }
                Setting::Fails => Err(Failure::network("HTTP 422: the base cannot be changed")),
                Setting::Ignored => Ok(()),
            }
        }

        fn merge_commit(&self, _repo: &str, _pr: i64) -> Result<Option<String>, Failure> {
            Ok(self.merged_into.lock().unwrap().as_ref().map(|_| "merged".to_string()))
        }

        fn merge(&self, _repo: &str, pr: i64) -> Result<String, Failure> {
            self.merges.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if self.closed || self.merged_into.lock().unwrap().is_some() {
                return Err(Failure::network(format!(
                    "gh pr ready {pr} -R o/r: X Pull request o/r#{pr} is closed. Only draft pull requests can be \
                     marked as \"ready for review\""
                )));
            }
            *self.merged_into.lock().unwrap() = Some(self.base.lock().unwrap().clone());
            Ok("merged".to_string())
        }

        fn close(&self, _repo: &str, _pr: i64, _comment: &str) -> Result<(), Failure> {
            unreachable!("no pull request is closed here")
        }

        fn gh(&self, _dir: &Path, _args: &[String]) -> Result<(i32, String), Failure> {
            unreachable!("no gh command runs here")
        }
    }

    // A pull request opened against another branch than the project's main, as a work taken over may hold, is set to
    // merge into main before it merges, and one already on main is left as it is; when its base cannot be set, nothing
    // merges, and the failure names both branches.
    #[test]
    fn a_pull_request_merges_into_the_project_s_main_and_nowhere_else() {
        // Setting the base would fail, and is not asked.
        let on_main = one_pr("trial-main", Setting::Fails);
        assert_eq!(merge_into(&on_main, "o/r", 4, "trial-main"), Ok("merged".to_string()));
        assert_eq!(
            on_main.merged_into.lock().unwrap().take().as_deref(),
            Some("trial-main")
        );
        let elsewhere = one_pr("first-version", Setting::Takes);
        assert_eq!(merge_into(&elsewhere, "o/r", 4, "trial-main"), Ok("merged".to_string()));
        assert_eq!(
            elsewhere.merged_into.lock().unwrap().take().as_deref(),
            Some("trial-main")
        );
        for setting in [Setting::Fails, Setting::Ignored] {
            let stuck = one_pr("first-version", setting);
            let f = merge_into(&stuck, "o/r", 4, "trial-main").unwrap_err();
            assert!(
                f.what
                    .starts_with("PR 4 merges into first-version, not the project's main trial-main: "),
                "{}",
                f.what
            );
            assert_eq!(stuck.merged_into.lock().unwrap().take(), None);
        }
    }

    // A pull request that merged into the project's main already, as when the answer of the merge was lost, gives the
    // commit it merged as and is not merged again, which gh refuses; one that merged into another branch is no merge
    // into the project's main and fails naming both, setting nothing; one that was closed without merging is merged as
    // before, and fails as gh does.
    #[test]
    fn a_pull_request_that_merged_already_gives_its_merge_commit_and_is_not_merged_again() {
        let lost = one_pr("first-version", Setting::Takes);
        assert_eq!(merge_into(&lost, "o/r", 4, "trial-main"), Ok("merged".to_string()));
        // The second call finds it merged, and the merge is not called again.
        assert_eq!(merge_into(&lost, "o/r", 4, "trial-main"), Ok("merged".to_string()));
        assert_eq!(lost.merges(), 1);

        let elsewhere = OnePr {
            merged_into: std::sync::Mutex::new(Some("first-version".to_string())),
            ..one_pr("first-version", Setting::Takes)
        };
        let f = merge_into(&elsewhere, "o/r", 4, "trial-main").unwrap_err();
        assert_eq!(
            f.what,
            "PR 4 merged into first-version, not the project's main trial-main"
        );
        assert_eq!(elsewhere.merges(), 0);
        assert_eq!(*elsewhere.base.lock().unwrap(), "first-version");

        let closed = OnePr {
            closed: true,
            ..one_pr("trial-main", Setting::Takes)
        };
        let f = merge_into(&closed, "o/r", 4, "trial-main").unwrap_err();
        assert!(f.what.contains("is closed. Only draft pull requests"), "{}", f.what);
        assert_eq!(closed.merges(), 1);
    }

    // Only a pull request in the state MERGED has a merge commit: an open one and a closed one have none, and one that
    // merged without a commit in gh's answer is a failure, not a pull request that did not merge.
    #[test]
    fn a_pull_request_is_merged_only_when_gh_says_state_merged_with_a_commit() {
        let merged = serde_json::json!({ "state": "MERGED", "mergeCommit": { "oid": "abc" } });
        assert_eq!(merged_oid(&merged, "4"), Ok(Some("abc".to_string())));
        for state in ["OPEN", "CLOSED"] {
            let v = serde_json::json!({ "state": state, "mergeCommit": null });
            assert_eq!(merged_oid(&v, "4"), Ok(None), "{state}");
        }
        let lost = serde_json::json!({ "state": "MERGED", "mergeCommit": null });
        assert_eq!(
            merged_oid(&lost, "4").unwrap_err().what,
            "PR 4 merged but gh gave no merge commit"
        );
    }
}
