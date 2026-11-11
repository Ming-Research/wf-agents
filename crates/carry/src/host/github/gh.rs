//! GitHub asked with the `gh` command line: each question is one `gh api` call of GitHub's REST API, its answer read
//! with the status GitHub gave it.
//!
//! `gh` is given, of the engine's environment, only what git is given of it, with the engine's token as `GH_TOKEN`
//! and a directory of its own for its configuration, so that no login or setting the owner keeps for their own use of
//! `gh` is read. The token is given in the environment alone, never as an argument, and nothing `gh` is given is said
//! where it fails.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use super::api::{Api, Check, Outcome, Pull, State};

/// GitHub's REST API, through `gh`, in one repository.
#[derive(Clone)]
pub struct Gh {
    /// The `gh` command line: `gh`, or a path to it.
    command: String,
    /// The repository, as `owner/name`.
    repository: String,
    env: Vec<(String, String)>,
    /// How long a call may take before it is given up.
    limit: Duration,
}

/// How long a call of `gh` may take before it is given up: GitHub answers in seconds, and a job's log of some
/// megabytes comes in well under this.
pub const LIMIT: Duration = Duration::from_secs(120);

impl std::fmt::Debug for Gh {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The environment holds the token.
        f.debug_struct("Gh")
            .field("command", &self.command)
            .field("repository", &self.repository)
            .finish_non_exhaustive()
    }
}

/// What GitHub answered: the status, and the body.
#[derive(Debug, PartialEq, Eq)]
struct Answer {
    status: u16,
    body: String,
}

impl Gh {
    /// `gh` as this command, in the repository `owner/name`, with the engine's token, keeping its configuration in
    /// this directory.
    pub fn new(command: &str, repository: &str, token: &str, config_dir: &Path) -> Gh {
        Gh {
            command: command.to_string(),
            repository: repository.to_string(),
            env: environment(std::env::vars(), token, config_dir),
            limit: LIMIT,
        }
    }

    /// `gh` giving a call up after this long.
    #[cfg(test)]
    pub(crate) fn within(mut self, limit: Duration) -> Gh {
        self.limit = limit;
        self
    }

    /// Asks GitHub, with this body where one is given, and gives what it answered whatever its status; or why `gh`
    /// gave no answer.
    async fn ask(&self, method: &str, path: &str, body: Option<Value>) -> Result<Answer, String> {
        let mut args = vec![
            "api",
            "--include",
            "--method",
            method,
            "-H",
            "Accept: application/vnd.github+json",
            "-H",
            "X-GitHub-Api-Version: 2022-11-28",
            path,
        ];
        if body.is_some() {
            args.extend(["--input", "-"]);
        }
        let mut child = Command::new(&self.command)
            .args(&args)
            .env_clear()
            .envs(self.env.iter().cloned())
            .stdin(if body.is_some() { Stdio::piped() } else { Stdio::null() })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| format!("{} cannot be started: {e}", self.command))?;
        if let (Some(body), Some(mut pipe)) = (body, child.stdin.take()) {
            let bytes = body.to_string().into_bytes();
            tokio::spawn(async move {
                let _ = pipe.write_all(&bytes).await;
            });
        }
        let waited = tokio::time::timeout(self.limit, child.wait_with_output()).await;
        // Given up, the child is dropped with the wait, and killed with it.
        let waited = waited.map_err(|_| {
            format!(
                "{} api {method} {path} did not end within {} seconds, and was stopped",
                self.command,
                self.limit.as_secs()
            )
        })?;
        let output = waited.map_err(|e| format!("{} could not be waited for: {e}", self.command))?;
        let out = String::from_utf8_lossy(&output.stdout);
        match included(&out) {
            Some(answer) => Ok(answer),
            None => Err(format!(
                "{} api {method} {path} gave no answer ({}): {}",
                self.command,
                output.status,
                String::from_utf8_lossy(&output.stderr).trim_end()
            )),
        }
    }

    /// Asks GitHub, and gives the body of an answer whose status is one of `ok`; or GitHub's words for why not.
    async fn want(&self, method: &str, path: &str, body: Option<Value>, ok: &[u16]) -> Result<String, String> {
        let answer = self.ask(method, path, body).await?;
        if ok.contains(&answer.status) {
            Ok(answer.body)
        } else {
            Err(refusal(&answer))
        }
    }

    /// As [`Gh::want`], the body read as JSON.
    async fn json(&self, method: &str, path: &str, body: Option<Value>, ok: &[u16]) -> Result<Value, String> {
        let body = self.want(method, path, body, ok).await?;
        serde_json::from_str(&body).map_err(|e| format!("GitHub's answer to {method} {path} is not JSON: {e}"))
    }
}

impl Api for Gh {
    async fn pulls(&self, branch: &str) -> Result<Vec<Pull>, String> {
        let owner = self.repository.split('/').next().unwrap_or_default();
        let path = format!(
            "repos/{}/pulls?head={owner}:{branch}&state=all&per_page=100",
            self.repository
        );
        let listed = self.json("GET", &path, None, &[200]).await?;
        let pulls = listed
            .as_array()
            .ok_or("GitHub's list of pull requests is not a list")?;
        pulls.iter().map(pull).collect()
    }

    async fn open(&self, branch: &str, base: &str, title: &str, body: &str) -> Result<Pull, String> {
        let path = format!("repos/{}/pulls", self.repository);
        let asked = json!({ "title": title, "body": body, "head": branch, "base": base });
        pull(&self.json("POST", &path, Some(asked), &[201]).await?)
    }

    async fn checks(&self, commit: &str) -> Result<Vec<Check>, String> {
        let mut runs = Vec::new();
        for page in 1.. {
            let path = format!(
                "repos/{}/commits/{commit}/check-runs?per_page=100&page={page}",
                self.repository
            );
            let listed = self.json("GET", &path, None, &[200]).await?;
            let total = listed["total_count"].as_u64().unwrap_or_default();
            let these = listed["check_runs"].as_array().cloned().unwrap_or_default();
            let last = these.is_empty();
            runs.extend(these);
            if last || runs.len() as u64 >= total {
                break;
            }
        }
        let path = format!("repos/{}/commits/{commit}/status?per_page=100", self.repository);
        let status = self.json("GET", &path, None, &[200]).await?;
        let statuses = status["statuses"].as_array().cloned().unwrap_or_default();
        Ok(checks(&runs, &statuses))
    }

    async fn log(&self, job: u64) -> Result<String, String> {
        let path = format!("repos/{}/actions/jobs/{job}/logs", self.repository);
        self.want("GET", &path, None, &[200]).await
    }

    async fn merge(&self, number: u64, head: &str, method: &str) -> Result<String, String> {
        let path = format!("repos/{}/pulls/{number}/merge", self.repository);
        let asked = json!({ "sha": head, "merge_method": method });
        let merged = self.json("PUT", &path, Some(asked), &[200]).await?;
        (merged["sha"].as_str())
            .map(str::to_string)
            .ok_or_else(|| "GitHub's answer to the merge names no commit".to_string())
    }

    async fn delete(&self, branch: &str) -> Result<(), String> {
        let path = format!("repos/{}/git/refs/heads/{branch}", self.repository);
        // A branch that is not there is answered 422, "Reference does not exist".
        self.want("DELETE", &path, None, &[204, 404, 422]).await.map(|_| ())
    }
}

/// The environment `gh` is given: of the engine's, what git is given of it; then the token, a directory of its own
/// for its configuration, and no prompt, no notice of a newer version and no colour.
pub(crate) fn environment(
    engine: impl IntoIterator<Item = (String, String)>,
    token: &str,
    config_dir: &Path,
) -> Vec<(String, String)> {
    let kept = |name: &str| crate::cli::KEPT.contains(&name) || name.starts_with("LC_");
    let mut env: Vec<(String, String)> = engine.into_iter().filter(|(name, _)| kept(name)).collect();
    env.sort();
    let dir: PathBuf = config_dir.to_path_buf();
    env.extend([
        ("GH_TOKEN".to_string(), token.to_string()),
        ("GH_CONFIG_DIR".to_string(), dir.to_string_lossy().into_owned()),
        ("GH_PROMPT_DISABLED".to_string(), "1".to_string()),
        ("GH_NO_UPDATE_NOTIFIER".to_string(), "1".to_string()),
        ("NO_COLOR".to_string(), "1".to_string()),
    ]);
    env
}

/// What `gh api --include` printed, read as GitHub's answer: the status line, the headers, an empty line, the body.
fn included(out: &str) -> Option<Answer> {
    let (head, body) = (out.split_once("\r\n\r\n")).or_else(|| out.split_once("\n\n"))?;
    let status = head.lines().next()?.split_whitespace().nth(1)?.parse().ok()?;
    Some(Answer {
        status,
        body: body.to_string(),
    })
}

/// GitHub's words for why it did not do what it was asked, with the status.
fn refusal(answer: &Answer) -> String {
    let said: Value = serde_json::from_str(&answer.body).unwrap_or(Value::Null);
    let mut words: Vec<String> = said["message"].as_str().map(str::to_string).into_iter().collect();
    for error in said["errors"].as_array().into_iter().flatten() {
        if let Some(message) = error["message"].as_str().or_else(|| error.as_str()) {
            words.push(message.to_string());
        }
    }
    if words.is_empty() {
        words.push(answer.body.trim().chars().take(300).collect());
    }
    format!("HTTP {}: {}", answer.status, words.join("; "))
}

/// A pull request, as GitHub's REST API gives it.
fn pull(value: &Value) -> Result<Pull, String> {
    let text = |at: &Value, what: &str| {
        at.as_str()
            .map(str::to_string)
            .ok_or(format!("a pull request has no {what}"))
    };
    let merged = value["merged_at"].is_string();
    let state = match (value["state"].as_str(), merged) {
        (_, true) => State::Merged,
        (Some("open"), false) => State::Open,
        (Some("closed"), false) => State::Closed,
        (other, _) => return Err(format!("a pull request is in a state GitHub does not give: {other:?}")),
    };
    Ok(Pull {
        number: value["number"].as_u64().ok_or("a pull request has no number")?,
        url: text(&value["html_url"], "link")?,
        state,
        head: text(&value["head"]["sha"], "head")?,
        base: text(&value["base"]["ref"], "base")?,
        // GitHub gives an open pull request's test merge here as well: only a merged one's counts.
        merge_commit: if merged {
            value["merge_commit_sha"].as_str().map(str::to_string)
        } else {
            None
        },
    })
}

/// The checks of a commit, from its check runs and its commit statuses as GitHub's REST API gives them. Of each name
/// among the check runs, the latest run counts, so that a check run again, as a person may on GitHub, is judged by
/// its new run; GitHub gives each status's latest itself.
pub(crate) fn checks(runs: &[Value], statuses: &[Value]) -> Vec<Check> {
    let mut latest: std::collections::BTreeMap<&str, &Value> = std::collections::BTreeMap::new();
    for run in runs {
        let Some(name) = run["name"].as_str() else {
            continue;
        };
        let newer = latest
            .get(name)
            .is_none_or(|kept| run["id"].as_u64() > kept["id"].as_u64());
        if newer {
            latest.insert(name, run);
        }
    }
    let mut checks: Vec<Check> = latest.into_iter().map(|(name, run)| check_run(name, run)).collect();
    checks.extend(statuses.iter().filter_map(status));
    checks
}

/// A check run as a check.
fn check_run(name: &str, run: &Value) -> Check {
    let outcome = match (run["status"].as_str(), run["conclusion"].as_str()) {
        (Some("completed"), Some("success" | "neutral" | "skipped")) => Outcome::Passed,
        (Some("completed"), Some(how @ ("failure" | "timed_out"))) => Outcome::Failed(how.to_string()),
        (Some("completed"), Some(how)) => Outcome::Broken(how.to_string()),
        (Some("completed"), None) => Outcome::Broken("no conclusion".to_string()),
        _ => Outcome::Pending,
    };
    let said: Vec<&str> = [&run["output"]["title"], &run["output"]["summary"]]
        .into_iter()
        .filter_map(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .collect();
    let actions = run["app"]["slug"].as_str() == Some("github-actions");
    Check {
        name: name.to_string(),
        outcome,
        url: (run["html_url"].as_str())
            .or_else(|| run["details_url"].as_str())
            .map(str::to_string),
        summary: (!said.is_empty()).then(|| said.join("\n")),
        job: if actions { run["id"].as_u64() } else { None },
    }
}

/// A commit status as a check.
fn status(status: &Value) -> Option<Check> {
    let name = status["context"].as_str()?;
    let outcome = match status["state"].as_str()? {
        "success" => Outcome::Passed,
        "pending" => Outcome::Pending,
        "failure" => Outcome::Failed("failure".to_string()),
        other => Outcome::Broken(other.to_string()),
    };
    Some(Check {
        name: name.to_string(),
        outcome,
        url: status["target_url"].as_str().map(str::to_string),
        summary: (status["description"].as_str())
            .filter(|s| !s.trim().is_empty())
            .map(str::to_string),
        job: None,
    })
}

#[cfg(test)]
mod tests;
