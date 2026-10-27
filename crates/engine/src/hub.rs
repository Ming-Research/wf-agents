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
    /// new ones are given; gives its number.
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
    /// Runs the red checks of the workflow's run on a commit once more, waits, and gives each check's result.
    fn rerun(&self, repo: &str, sha: &str, workflow: &str) -> Result<Vec<CheckRun>, Failure>;
    /// Marks the pull request ready and merges it; gives the commit it made on the base branch.
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
    let text = match (parts.next(), parts.next(), parts.next()) {
        (Some(_), Some(_), Some(rest)) => rest,
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

/// A text without its terminal escapes: ESC [ ... a letter.
fn without_escapes(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                chars.next();
                for x in chars.by_ref() {
                    if ('@'..='~').contains(&x) {
                        break;
                    }
                }
            }
            continue;
        }
        out.push(c);
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
        let link = "https://github.com/o/r/actions/runs/40483126002/job/118000000001";
        assert_eq!(
            run_and_job(link),
            Some(("40483126002".to_string(), "118000000001".to_string()))
        );
        assert_eq!(run_and_job("https://github.com/o/r/pull/3"), None);
    }
}
