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

use super::api::{Api, Check, File, Outcome, Pull, Run, State};

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
        self.ask_with(method, path, body, &[]).await
    }

    /// Asks GitHub as [`Gh::ask`] does, `gh` given these flags beside.
    async fn ask_with(&self, method: &str, path: &str, body: Option<Value>, flags: &[&str]) -> Result<Answer, String> {
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
        args.extend(flags);
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

    /// Runs `gh` with these arguments, other than `api`, given up as an ask is; says what it said where it failed.
    async fn run(&self, args: &[&str]) -> Result<(), String> {
        let child = Command::new(&self.command)
            .args(args)
            .env_clear()
            .envs(self.env.iter().cloned())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| format!("{} cannot be started: {e}", self.command))?;
        let waited = tokio::time::timeout(self.limit, child.wait_with_output()).await;
        let output = (waited.map_err(|_| {
            format!(
                "{} {} did not end in time, and was stopped",
                self.command,
                args[..2].join(" ")
            )
        }))?
        .map_err(|e| format!("{} could not be waited for: {e}", self.command))?;
        if output.status.success() {
            return Ok(());
        }
        Err(format!(
            "{} {} failed: {}",
            self.command,
            args[..2].join(" "),
            String::from_utf8_lossy(&output.stderr).trim_end()
        ))
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
        let (method, path, asked) = opening(&self.repository, branch, base, title, body);
        pull(&self.json(method, &path, Some(asked), &[201]).await?)
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
        // A log holds the escapes that colour its text, which `gh` otherwise refuses to print, giving the status and
        // no body: the engine reads them as data and takes them out itself.
        let answer = self.ask_with("GET", &path, None, &["--allow-escape-sequences"]).await?;
        if answer.status == 200 {
            Ok(answer.body)
        } else {
            Err(refusal(&answer))
        }
    }

    async fn merge(&self, number: u64, head: &str, method: &str) -> Result<String, String> {
        let path = format!("repos/{}/pulls/{number}/merge", self.repository);
        let asked = json!({ "sha": head, "merge_method": method });
        let merged = self.json("PUT", &path, Some(asked), &[200]).await?;
        (merged["sha"].as_str())
            .map(str::to_string)
            .ok_or_else(|| "GitHub's answer to the merge names no commit".to_string())
    }

    async fn runs(&self, workflow: &str, commit: &str) -> Result<Vec<Run>, String> {
        let path = format!(
            "repos/{}/actions/workflows/{workflow}/runs?head_sha={commit}&event=workflow_dispatch&per_page=30",
            self.repository
        );
        let listed = self.json("GET", &path, None, &[200]).await?;
        let runs = listed["workflow_runs"].as_array().cloned().unwrap_or_default();
        Ok((runs.iter())
            .filter_map(|run| {
                Some(Run {
                    id: run["id"].as_u64()?,
                    completed: run["status"].as_str() == Some("completed"),
                    url: run["html_url"].as_str().unwrap_or_default().to_string(),
                })
            })
            .collect())
    }

    async fn dispatch(&self, workflow: &str, branch: &str, inputs: &[(String, String)]) -> Result<(), String> {
        let path = format!("repos/{}/actions/workflows/{workflow}/dispatches", self.repository);
        let inputs: serde_json::Map<String, Value> = (inputs.iter())
            .map(|(name, value)| (name.clone(), Value::String(value.clone())))
            .collect();
        let asked = json!({ "ref": branch, "inputs": inputs });
        self.want("POST", &path, Some(asked), &[200, 204]).await.map(|_| ())
    }

    async fn jobs(&self, run: u64) -> Result<Vec<Check>, String> {
        let path = format!("repos/{}/actions/runs/{run}/jobs?per_page=100", self.repository);
        let listed = self.json("GET", &path, None, &[200]).await?;
        let jobs = listed["jobs"].as_array().cloned().unwrap_or_default();
        Ok(jobs.iter().filter_map(job).collect())
    }

    async fn artifact(&self, run: u64, name: &str) -> Result<Option<Vec<File>>, String> {
        let dir = std::env::temp_dir().join(format!("wf-agents-artifact-{}", uuid::Uuid::new_v4().simple()));
        let (run, dir_text) = (run.to_string(), dir.to_string_lossy().into_owned());
        let args = [
            "run",
            "download",
            run.as_str(),
            "-R",
            self.repository.as_str(),
            "-n",
            name,
            "-D",
            &dir_text,
        ];
        let ran = self.run(&args).await;
        let files = match ran {
            Ok(()) => files_under(&dir, ""),
            Err(why) if why.contains("no artifact") || why.contains("no valid artifact") => Ok(Vec::new()),
            Err(why) => Err(why),
        };
        let _ = tokio::fs::remove_dir_all(&dir).await;
        let files = files?;
        Ok((!files.is_empty()).then_some(files))
    }

    async fn delete(&self, branch: &str) -> Result<(), String> {
        let path = format!("repos/{}/git/refs/heads/{branch}", self.repository);
        // A branch that is not there is answered 422, "Reference does not exist".
        self.want("DELETE", &path, None, &[204, 404, 422]).await.map(|_| ())
    }

    async fn edit(&self, number: u64, title: &str, body: &str) -> Result<(), String> {
        let (method, path, asked) = editing(&self.repository, number, title, body);
        self.want(method, &path, Some(asked), &[200]).await.map(|_| ())
    }

    async fn ready(&self, node: &str) -> Result<(), String> {
        let (method, path, asked) = readying(node);
        let answer = self.json(method, &path, Some(asked), &[200]).await?;
        // GraphQL says what it refused with a status of 200.
        match answer["errors"].as_array() {
            Some(errors) if !errors.is_empty() => {
                let words: Vec<&str> = errors.iter().filter_map(|e| e["message"].as_str()).collect();
                Err(format!(
                    "GitHub did not make the pull request ready: {}",
                    words.join("; ")
                ))
            }
            _ => Ok(()),
        }
    }

    async fn close(&self, number: u64, comment: &str) -> Result<(), String> {
        let [(method, path, said), (closing, at, closed)] = closing(&self.repository, number, comment);
        self.want(method, &path, Some(said), &[201]).await?;
        self.want(closing, &at, Some(closed), &[200]).await.map(|_| ())
    }
}

/// A request of GitHub's API: its method, its path and its body.
type Request = (&'static str, String, Value);

/// The request that opens a pull request of the branch into the base, as a draft.
pub(crate) fn opening(repository: &str, branch: &str, base: &str, title: &str, body: &str) -> Request {
    let path = format!("repos/{repository}/pulls");
    let asked = json!({ "title": title, "body": body, "head": branch, "base": base, "draft": true });
    ("POST", path, asked)
}

/// The request that gives the pull request this title and body.
pub(crate) fn editing(repository: &str, number: u64, title: &str, body: &str) -> Request {
    let path = format!("repos/{repository}/pulls/{number}");
    ("PATCH", path, json!({ "title": title, "body": body }))
}

/// The request that makes the draft pull request of this id ready: the REST API cannot, GitHub's GraphQL API can.
pub(crate) fn readying(node: &str) -> Request {
    let query = "mutation($id: ID!) { markPullRequestReadyForReview(input: {pullRequestId: $id}) { pullRequest { \
                 isDraft } } }";
    (
        "POST",
        "graphql".to_string(),
        json!({ "query": query, "variables": { "id": node } }),
    )
}

/// The requests that say the comment on the pull request and then close it.
pub(crate) fn closing(repository: &str, number: u64, comment: &str) -> [Request; 2] {
    [
        (
            "POST",
            format!("repos/{repository}/issues/{number}/comments"),
            json!({ "body": comment }),
        ),
        (
            "PATCH",
            format!("repos/{repository}/pulls/{number}"),
            json!({ "state": "closed" }),
        ),
    ]
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
        title: text(&value["title"], "title")?,
        // A pull request that says nothing has a body of null.
        body: value["body"].as_str().unwrap_or_default().to_string(),
        draft: value["draft"].as_bool().unwrap_or(false),
        node: text(&value["node_id"], "id")?,
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
    let mut checks: Vec<Check> = (latest.into_iter())
        .map(|(name, kept)| {
            let mut check = check_run(name, kept);
            // The earlier runs of the same check, said as they are, oldest first.
            let mut earlier: Vec<&Value> = (runs.iter())
                .filter(|run| run["name"].as_str() == Some(name) && run["id"].as_u64() != kept["id"].as_u64())
                .collect();
            earlier.sort_by_key(|run| run["id"].as_u64());
            check.earlier = (earlier.into_iter())
                .map(|run| {
                    let how = (run["conclusion"].as_str())
                        .or_else(|| run["status"].as_str())
                        .unwrap_or("unknown");
                    let at = (run["html_url"].as_str()).or_else(|| run["details_url"].as_str());
                    at.map_or(how.to_string(), |at| format!("{how} {at}"))
                })
                .collect();
            check
        })
        .collect();
    checks.extend(statuses.iter().filter_map(status));
    checks
}

/// A check run as a check.
fn check_run(name: &str, run: &Value) -> Check {
    let outcome = match (run["status"].as_str(), run["conclusion"].as_str()) {
        (Some("completed"), Some("success" | "neutral" | "skipped")) => Outcome::Passed,
        // A job stopped at its time limit, or cancelled, judged the code as far as it ran: its log says what ran and
        // what was still running, which a check asked again on the same commit would not run again to say.
        (Some("completed"), Some(how @ ("failure" | "timed_out" | "cancelled"))) => {
            match (how, lasted(&run["started_at"], &run["completed_at"])) {
                ("failure", _) | (_, None) => Outcome::Failed(how.to_string()),
                (_, Some(lasted)) => Outcome::Failed(format!("{how} after {lasted}")),
            }
        }
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
        earlier: Vec::new(),
    }
}

/// A job of a run as a check; none for a job that did not run, its condition false.
fn job(job: &Value) -> Option<Check> {
    let outcome = match (job["status"].as_str(), job["conclusion"].as_str()) {
        (Some("completed"), Some("skipped")) => return None,
        (Some("completed"), Some("success" | "neutral")) => Outcome::Passed,
        (Some("completed"), Some(how @ ("failure" | "timed_out"))) => Outcome::Failed(how.to_string()),
        (Some("completed"), Some(how)) => Outcome::Broken(how.to_string()),
        (Some("completed"), None) => Outcome::Broken("no conclusion".to_string()),
        _ => Outcome::Pending,
    };
    Some(Check {
        name: job["name"].as_str()?.to_string(),
        outcome,
        url: job["html_url"].as_str().map(str::to_string),
        summary: None,
        job: job["id"].as_u64(),
        earlier: Vec::new(),
    })
}

/// Every file under a directory, each as its path under it with `/` between its parts, after `prefix`, and its bytes.
fn files_under(dir: &Path, prefix: &str) -> Result<Vec<File>, String> {
    let mut out = Vec::new();
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("{}: {e}", dir.display()))?;
        let path = format!("{prefix}{}", entry.file_name().to_string_lossy());
        if entry.path().is_dir() {
            out.extend(files_under(&entry.path(), &format!("{path}/"))?);
        } else {
            let bytes = std::fs::read(entry.path()).map_err(|e| format!("{path}: {e}"))?;
            out.push((path, bytes));
        }
    }
    Ok(out)
}

/// How long a check ran, in words, from the times GitHub gives as `2026-11-11T07:48:56Z`; none where it gives none.
fn lasted(started: &Value, completed: &Value) -> Option<String> {
    let seconds = seconds_of(completed.as_str()?)? - seconds_of(started.as_str()?)?;
    let (minutes, seconds) = (seconds / 60, seconds % 60);
    let unit = |n: i64, one: &str| format!("{n} {one}{}", if n == 1 { "" } else { "s" });
    Some(match (minutes, seconds) {
        (0, s) => unit(s, "second"),
        (m, 0) => unit(m, "minute"),
        (m, s) => format!("{} and {}", unit(m, "minute"), unit(s, "second")),
    })
}

/// The seconds since 1970 of a time as GitHub writes one, in UTC: `2026-11-11T07:48:56Z`, fractions of a second left
/// out; none for anything else.
fn seconds_of(time: &str) -> Option<i64> {
    let time = time.strip_suffix('Z')?;
    let (date, clock) = time.split_once('T')?;
    let clock = clock.split('.').next()?;
    let numbers = |text: &str, sep: char| -> Option<Vec<i64>> { text.split(sep).map(|n| n.parse().ok()).collect() };
    let (d, c) = (numbers(date, '-')?, numbers(clock, ':')?);
    let [year, month, day] = <[i64; 3]>::try_from(d).ok()?;
    let [hour, minute, second] = <[i64; 3]>::try_from(c).ok()?;
    // Days from the civil date, by the proleptic Gregorian calendar.
    let (y, m) = if month <= 2 {
        (year - 1, month + 9)
    } else {
        (year, month - 3)
    };
    let era = y.div_euclid(400);
    let of_era = y - era * 400;
    let of_year = (153 * m + 2) / 5 + day - 1;
    let of_cycle = of_era * 365 + of_era / 4 - of_era / 100 + of_year;
    let days = era * 146_097 + of_cycle - 719_468;
    Some(days * 86_400 + hour * 3600 + minute * 60 + second)
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
        earlier: Vec::new(),
    })
}

#[cfg(test)]
mod tests;
