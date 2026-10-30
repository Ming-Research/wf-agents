//! What the engine does on GitHub: pull requests, CI runs, merges and the gh commands agents ask for. It is a trait so
//! that the engine's tests can stand in for GitHub; [`Gh`] does it with the gh CLI and the engine's own credentials.

use std::path::{Path, PathBuf};
use std::process::Command;
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_tail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_path: Option<String>,
}

/// A tool that did not run, in the kinds the flow names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    /// network_or_api, ci_did_not_start, timed_out, machine_down or other.
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

    pub fn other(what: impl Into<String>) -> Failure {
        Failure {
            kind: "other",
            what: what.into(),
        }
    }
}

pub trait GitHub {
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
    /// on the branch's commit then waits for that run.
    fn dispatch(&self, _repo: &str, _workflow: &str, _branch: &str, _inputs: &[(&str, &str)]) -> Result<(), Failure> {
        Err(Failure::other("no workflow runs by hand here"))
    }
    /// Runs the red checks of the workflow's run on a commit once more, waits, and gives each check's result.
    fn rerun(&self, repo: &str, sha: &str, workflow: &str) -> Result<Vec<CheckRun>, Failure>;
    /// The branch a pull request merges into: its base.
    fn base(&self, repo: &str, pr: i64) -> Result<String, Failure>;
    /// Sets the branch a pull request merges into.
    fn set_base(&self, repo: &str, pr: i64, base: &str) -> Result<(), Failure>;
    /// Marks the pull request ready and merges it into its base, whichever branch that is; gives the commit it made
    /// there. The engine merges only through [`merge_into`], which makes the base the project's main first.
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

/// Merges a pull request into `main`, the project's main, and nowhere else: makes `main` its base first, and merges
/// nothing when it cannot; gives the merge commit.
pub fn merge_into(github: &dyn GitHub, repo: &str, pr: i64, main: &str) -> Result<String, Failure> {
    ensure_base(github, repo, pr, main)?;
    github.merge(repo, pr)
}

/// GitHub through the gh CLI.
pub struct Gh {
    /// Where logs of red checks are kept.
    pub logs: PathBuf,
    /// How long to wait between two looks at a run.
    pub poll: Duration,
    /// How long a run may take to appear, and to finish.
    pub start_timeout: Duration,
    pub run_timeout: Duration,
}

impl Gh {
    pub fn new(logs: PathBuf) -> Gh {
        Gh {
            logs,
            poll: Duration::from_secs(20),
            start_timeout: Duration::from_secs(600),
            run_timeout: Duration::from_secs(3 * 3600),
        }
    }

    fn run(&self, args: &[&str]) -> Result<String, Failure> {
        let out = Command::new("gh")
            .args(args)
            .output()
            .map_err(|e| Failure::other(format!("gh: {e}")))?;
        if !out.status.success() {
            return Err(Failure::network(format!(
                "gh {}: {}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr).trim()
            )));
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }

    fn json(&self, args: &[&str]) -> Result<Value, Failure> {
        let out = self.run(args)?;
        serde_json::from_str(&out).map_err(|e| Failure::other(format!("gh {}: {e}", args.join(" "))))
    }

    /// The workflow's latest run on a commit, once it has finished.
    fn finished_run(&self, repo: &str, sha: &str, workflow: &str, after: Option<i64>) -> Result<Value, Failure> {
        let started = Instant::now();
        loop {
            let runs = self.json(&[
                "run",
                "list",
                "-R",
                repo,
                "--workflow",
                workflow,
                "--commit",
                sha,
                "--json",
                "databaseId,status,conclusion,url,attempt",
                "-L",
                "5",
            ])?;
            let latest = runs.as_array().and_then(|rs| rs.first()).cloned();
            match latest {
                None if started.elapsed() > self.start_timeout => {
                    return Err(Failure {
                        kind: "ci_did_not_start",
                        what: format!("no run of {workflow} on {sha} after {} s", self.start_timeout.as_secs()),
                    });
                }
                Some(r)
                    if r["status"] == "completed" && after.is_none_or(|a| r["attempt"].as_i64().unwrap_or(0) > a) =>
                {
                    return Ok(r);
                }
                _ if started.elapsed() > self.run_timeout => {
                    return Err(Failure {
                        kind: "timed_out",
                        what: format!(
                            "the run of {workflow} on {sha} did not finish in {} s",
                            self.run_timeout.as_secs()
                        ),
                    });
                }
                _ => std::thread::sleep(self.poll),
            }
        }
    }

    fn jobs(&self, repo: &str, run: &Value) -> Result<Vec<CheckRun>, Failure> {
        let id = run["databaseId"].as_i64().unwrap_or_default().to_string();
        let v = self.json(&["run", "view", &id, "-R", repo, "--json", "jobs"])?;
        let mut out = Vec::new();
        for j in v["jobs"].as_array().cloned().unwrap_or_default() {
            let result = match j["conclusion"].as_str().unwrap_or_default() {
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
            };
            if result == "red" {
                let job = j["databaseId"].as_i64().unwrap_or_default().to_string();
                if let Ok(log) = self.run(&["run", "view", &id, "-R", repo, "--log-failed", "--job", &job]) {
                    let path = self.logs.join(format!("run-{id}-job-{job}.log"));
                    if std::fs::create_dir_all(&self.logs).is_ok() && std::fs::write(&path, &log).is_ok() {
                        c.log_path = Some(path.display().to_string());
                    }
                    c.log_tail = Some(tail(&clean_log(&log), 60));
                }
            }
            out.push(c);
        }
        Ok(out)
    }
}

/// The last `n` lines of a text.
pub fn tail(text: &str, n: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    lines[lines.len().saturating_sub(n)..].join("\n")
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
        let run = self.finished_run(repo, sha, workflow, None)?;
        self.jobs(repo, &run)
    }

    fn dispatch(&self, repo: &str, workflow: &str, branch: &str, inputs: &[(&str, &str)]) -> Result<(), Failure> {
        let fields: Vec<String> = inputs.iter().map(|(k, v)| format!("{k}={v}")).collect();
        let mut args = vec!["workflow", "run", workflow, "-R", repo, "--ref", branch];
        for f in &fields {
            args.extend(["-f", f.as_str()]);
        }
        self.run(&args).map(|_| ())
    }

    fn job_log(&self, repo: &str, check: &CheckRun) -> Result<String, Failure> {
        let (run, job) = run_and_job(&check.run)
            .ok_or_else(|| Failure::other(format!("no run and job in the check's link: {}", check.run)))?;
        self.run(&["run", "view", &run, "-R", repo, "--log", "--job", &job])
    }

    fn rerun(&self, repo: &str, sha: &str, workflow: &str) -> Result<Vec<CheckRun>, Failure> {
        let run = self.finished_run(repo, sha, workflow, None)?;
        let id = run["databaseId"].as_i64().unwrap_or_default().to_string();
        let attempt = run["attempt"].as_i64().unwrap_or(1);
        self.run(&["run", "rerun", &id, "-R", repo, "--failed"])?;
        let run = self.finished_run(repo, sha, workflow, Some(attempt))?;
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

    fn merge(&self, repo: &str, pr: i64) -> Result<String, Failure> {
        let n = pr.to_string();
        self.run(&["pr", "ready", &n, "-R", repo])?;
        self.run(&["pr", "merge", &n, "-R", repo, "--squash"])?;
        let v = self.json(&["pr", "view", &n, "-R", repo, "--json", "mergeCommit"])?;
        v["mergeCommit"]["oid"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| Failure::other(format!("PR {n} merged but gh gave no merge commit")))
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

    /// What setting a pull request's base does on [`OnePr`].
    #[derive(Clone, Copy)]
    enum Setting {
        Takes,
        Fails,
        /// gh says it was set, and the base is the same.
        Ignored,
    }

    /// GitHub with one pull request: its base, what setting it does, and which branch it merged into.
    struct OnePr {
        base: std::cell::RefCell<String>,
        setting: Setting,
        merged_into: std::cell::RefCell<Option<String>>,
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
            Ok(self.base.borrow().clone())
        }

        fn set_base(&self, _repo: &str, _pr: i64, base: &str) -> Result<(), Failure> {
            match self.setting {
                Setting::Takes => {
                    *self.base.borrow_mut() = base.to_string();
                    Ok(())
                }
                Setting::Fails => Err(Failure::network("HTTP 422: the base cannot be changed")),
                Setting::Ignored => Ok(()),
            }
        }

        fn merge(&self, _repo: &str, _pr: i64) -> Result<String, Failure> {
            *self.merged_into.borrow_mut() = Some(self.base.borrow().clone());
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
        let hub = |base: &str, setting| OnePr {
            base: std::cell::RefCell::new(base.to_string()),
            setting,
            merged_into: std::cell::RefCell::new(None),
        };
        // Setting the base would fail, and is not asked.
        let on_main = hub("trial-main", Setting::Fails);
        assert_eq!(merge_into(&on_main, "o/r", 4, "trial-main"), Ok("merged".to_string()));
        assert_eq!(on_main.merged_into.take().as_deref(), Some("trial-main"));
        let elsewhere = hub("first-version", Setting::Takes);
        assert_eq!(merge_into(&elsewhere, "o/r", 4, "trial-main"), Ok("merged".to_string()));
        assert_eq!(elsewhere.merged_into.take().as_deref(), Some("trial-main"));
        for setting in [Setting::Fails, Setting::Ignored] {
            let stuck = hub("first-version", setting);
            let f = merge_into(&stuck, "o/r", 4, "trial-main").unwrap_err();
            assert!(
                f.what
                    .starts_with("PR 4 merges into first-version, not the project's main trial-main: "),
                "{}",
                f.what
            );
            assert_eq!(stuck.merged_into.take(), None);
        }
    }
}
