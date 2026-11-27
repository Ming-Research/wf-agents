//! GitHub asked with the `gh` command line: each question is one `gh api` call of GitHub's REST API, its answer read
//! with the status GitHub gave it.
//!
//! `gh` is given, of the engine's environment, only what git is given of it, with a directory of its own for its
//! configuration, so that no login or setting the owner keeps for their own use of `gh` is read; and each call is
//! given, as `GH_TOKEN`, the token its credential gives for it, the engine's GitHub App's (the module `app`). Where the
//! credential gives none, `gh` is not run, and the call fails saying why. The token is given in the environment alone,
//! never as an argument, and nothing `gh` is given is said where it fails.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::AsyncWriteExt;

use super::Credential;
use super::api::{Api, Check, File, Jobs, Layer, Listed, OPEN_PULLS, Outcome, Pull, Run, Runner, Runners, State};
use crate::acts::failing::told_by_its_words;
use crate::host::Failed;
use decide::{Obstacle, Retry};

/// GitHub's REST API, through `gh`, in one repository.
#[derive(Clone)]
pub struct Gh {
    /// The `gh` command line: `gh`, or a path to it.
    command: String,
    /// The repository, as `owner/name`.
    repository: String,
    /// What `gh` is given of the engine's environment, and its own variables, the token apart.
    env: Vec<(String, String)>,
    /// What gives each call its token.
    credential: Credential,
    /// How long a call may take before it is given up.
    limit: Duration,
}

/// How long a call of `gh` may take before it is given up: GitHub answers in seconds, and a job's log of some
/// megabytes comes in well under this.
pub const LIMIT: Duration = Duration::from_secs(120);

impl std::fmt::Debug for Gh {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The credential is not shown.
        f.debug_struct("Gh")
            .field("command", &self.command)
            .field("repository", &self.repository)
            .finish_non_exhaustive()
    }
}

/// What GitHub answered: the status, and the body; whether it says it limits the requests it takes, by its status (429),
/// its headers (none remaining of them, or a time given to try again after) or its words (a secondary limit gives
/// neither); when it may be asked again where its headers say (any answer may say it, a 503's `Retry-After` as well);
/// and the entity tag its headers give the answer, where they give one.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Answer {
    pub(super) status: u16,
    pub(super) body: String,
    limited: bool,
    retry: Option<Retry>,
    tag: Option<String>,
}

/// How long a limit is waited out when GitHub says nothing of when it may be asked again, in seconds: at least a
/// minute, as GitHub's guide to its limits says of a limit that gives neither `retry-after` nor a reset.
const LIMIT_WAIT: u64 = 60;

impl Gh {
    /// `gh` as this command, in the repository `owner/name`, each call given the token this credential gives for it,
    /// keeping its configuration in this directory.
    pub fn new(command: &str, repository: &str, credential: Credential, config_dir: &Path) -> Gh {
        Gh {
            command: command.to_string(),
            repository: repository.to_string(),
            env: environment(crate::programs::engine().iter().cloned(), config_dir),
            credential,
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
    /// gave no answer, or why the credential gave no token, in which case `gh` is not run.
    async fn ask(&self, method: &str, path: &str, body: Option<Value>) -> Result<Answer, Failed> {
        self.ask_with(method, path, body, &[]).await
    }

    /// Asks GitHub as [`Gh::ask`] does, `gh` given these flags beside.
    async fn ask_with(&self, method: &str, path: &str, body: Option<Value>, flags: &[&str]) -> Result<Answer, Failed> {
        let token = self.credential.token().await?;
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
        let mut child = crate::programs::command(&self.command)
            .args(&args)
            .env_clear()
            .envs(self.env.iter().cloned())
            .env(GH_TOKEN, &token)
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
            None => Err(Failed::from(format!(
                "{} api {method} {path} gave no answer ({}): {}",
                self.command,
                output.status,
                String::from_utf8_lossy(&output.stderr).trim_end()
            ))),
        }
    }

    /// Runs `gh` with these arguments, other than `api`, given up as an ask is; says what it said where it failed.
    async fn run(&self, args: &[&str]) -> Result<(), Failed> {
        self.output(args, self.limit).await.map(|_| ())
    }

    /// Runs `gh` with these arguments, other than `api`, given up after `limit`; gives what it printed, or what it said
    /// where it failed, or why the credential gave no token, in which case `gh` is not run.
    async fn output(&self, args: &[&str], limit: Duration) -> Result<String, Failed> {
        let token = self.credential.token().await?;
        let child = crate::programs::command(&self.command)
            .args(args)
            .env_clear()
            .envs(self.env.iter().cloned())
            .env(GH_TOKEN, &token)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| format!("{} cannot be started: {e}", self.command))?;
        let waited = tokio::time::timeout(limit, child.wait_with_output()).await;
        let output = (waited.map_err(|_| {
            format!(
                "{} {} did not end in time, and was stopped",
                self.command,
                args[..2].join(" ")
            )
        }))?
        .map_err(|e| format!("{} could not be waited for: {e}", self.command))?;
        if output.status.success() {
            return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
        }
        Err(Failed::from(format!(
            "{} {} failed: {}",
            self.command,
            args[..2].join(" "),
            String::from_utf8_lossy(&output.stderr).trim_end()
        )))
    }

    /// Whether the repository's owner is an organisation, which has runners of its own to list.
    async fn owner_is_an_organisation(&self) -> Result<bool, String> {
        let path = format!("repos/{}", self.repository);
        let repository = self
            .json("GET", &path, None, &[200])
            .await
            .map_err(|failed| failed.why)?;
        Ok(repository["owner"]["type"].as_str() == Some("Organization"))
    }

    /// The runners one layer lists, each page of GitHub's list read; or GitHub's words for why it could not be read.
    async fn layer(&self, path: &str) -> Layer {
        let mut runners = Vec::new();
        for page in 1.. {
            let paged = format!("{path}?per_page=100&page={page}");
            let listed = self
                .json("GET", &paged, None, &[200])
                .await
                .map_err(|failed| failed.why)?;
            let listed = listed["runners"].as_array().cloned().unwrap_or_default();
            runners.extend(listed.iter().map(runner));
            if listed.len() < 100 {
                break;
            }
        }
        Ok(runners)
    }

    /// Asks GitHub, and gives the body of an answer whose status is one of `ok`; or GitHub's words for why not.
    async fn want(&self, method: &str, path: &str, body: Option<Value>, ok: &[u16]) -> Result<String, Failed> {
        let answer = self.ask(method, path, body).await?;
        if ok.contains(&answer.status) {
            Ok(answer.body)
        } else {
            Err(refused(&answer))
        }
    }

    /// As [`Gh::want`], the body read as JSON.
    pub(super) async fn json(
        &self,
        method: &str,
        path: &str,
        body: Option<Value>,
        ok: &[u16],
    ) -> Result<Value, Failed> {
        let body = self.want(method, path, body, ok).await?;
        serde_json::from_str(&body).map_err(|e| format!("GitHub's answer to {method} {path} is not JSON: {e}").into())
    }
}

impl Api for Gh {
    async fn pull(&self, number: u64) -> Result<Option<Pull>, Failed> {
        let path = format!("repos/{}/pulls/{number}", self.repository);
        let answer = self.ask("GET", &path, None).await?;
        match answer.status {
            200 => {
                let value: Value = serde_json::from_str(&answer.body)
                    .map_err(|e| format!("GitHub's answer to GET {path} is not JSON: {e}"))?;
                Ok(Some(pull(&value)?))
            }
            404 => Ok(None),
            _ => Err(refused(&answer)),
        }
    }

    async fn pulls(&self, branch: &str) -> Result<Vec<Pull>, Failed> {
        let owner = self.repository.split('/').next().unwrap_or_default();
        let path = format!(
            "repos/{}/pulls?head={owner}:{branch}&state=all&per_page=100",
            self.repository
        );
        let listed = self.json("GET", &path, None, &[200]).await?;
        let pulls = listed
            .as_array()
            .ok_or("GitHub's list of pull requests is not a list")?;
        Ok(pulls.iter().map(pull).collect::<Result<_, String>>()?)
    }

    async fn open_pulls(&self) -> Result<Vec<Pull>, Failed> {
        let mut open = Vec::new();
        for page in 1.. {
            let path = format!("repos/{}/pulls?state=open&per_page=100&page={page}", self.repository);
            let listed = self.json("GET", &path, None, &[200]).await?;
            let listed = listed
                .as_array()
                .ok_or("GitHub's list of pull requests is not a list")?;
            open.extend(listed.iter().map(pull).collect::<Result<Vec<_>, String>>()?);
            if open.len() > OPEN_PULLS {
                return Err(format!("the repository holds more than {OPEN_PULLS} open pull requests").into());
            }
            if listed.len() < 100 {
                break;
            }
        }
        Ok(open)
    }

    async fn open(&self, branch: &str, base: &str, title: &str, body: &str) -> Result<Pull, Failed> {
        let (method, path, asked) = opening(&self.repository, branch, base, title, body);
        Ok(pull(&self.json(method, &path, Some(asked), &[201]).await?)?)
    }

    async fn merge(&self, number: u64, head: &str, method: &str, title: &str, message: &str) -> Result<String, Failed> {
        let (put, path, asked) = merging(&self.repository, number, head, method, title, message);
        let merged = self.json(put, &path, Some(asked), &[200]).await?;
        (merged["sha"].as_str())
            .map(str::to_string)
            .ok_or_else(|| "GitHub's answer to the merge names no commit".into())
    }

    async fn runs(&self, workflow: &str, branch: &str, tag: Option<&str>) -> Result<Listed, Failed> {
        let path = format!(
            "repos/{}/actions/workflows/{workflow}/runs?branch={branch}&event=workflow_dispatch&per_page=50",
            self.repository
        );
        // GitHub answers 304 to a request that names the entity tag of the answer it would give again, and does not
        // count it against the limit; `gh` exits with 1 on it, and its status line says what GitHub answered.
        let condition = tag.map(|tag| format!("If-None-Match: {tag}"));
        let flags: Vec<&str> = condition.iter().flat_map(|header| ["-H", header.as_str()]).collect();
        let answer = self.ask_with("GET", &path, None, &flags).await?;
        match answer.status {
            200 => {
                let listed: Value = serde_json::from_str(&answer.body)
                    .map_err(|e| format!("GitHub's answer to GET {path} is not JSON: {e}"))?;
                let runs = listed["workflow_runs"].as_array().cloned().unwrap_or_default();
                Ok(Listed::Runs {
                    runs: runs.iter().filter_map(run).collect(),
                    tag: answer.tag,
                })
            }
            304 if tag.is_some() => Ok(Listed::Unchanged),
            304 => {
                Err(format!("GitHub answered GET {path} with 304, unchanged, though it was sent no entity tag").into())
            }
            _ => Err(refused(&answer)),
        }
    }

    async fn dispatch(&self, workflow: &str, branch: &str, inputs: &[(String, String)]) -> Result<(), Failed> {
        let path = format!("repos/{}/actions/workflows/{workflow}/dispatches", self.repository);
        let inputs: serde_json::Map<String, Value> = (inputs.iter())
            .map(|(name, value)| (name.clone(), Value::String(value.clone())))
            .collect();
        let asked = json!({ "ref": branch, "inputs": inputs });
        self.want("POST", &path, Some(asked), &[200, 204]).await.map(|_| ())
    }

    async fn cancel(&self, run: u64) -> Result<(), Failed> {
        let path = format!("repos/{}/actions/runs/{run}/cancel", self.repository);
        // A run that has completed already is answered 409, "Cannot cancel a workflow run that is completed".
        self.want("POST", &path, None, &[202, 409]).await.map(|_| ())
    }

    async fn jobs(&self, run: u64, tag: Option<&str>) -> Result<Jobs, Failed> {
        let path = format!("repos/{}/actions/runs/{run}/jobs?per_page=100", self.repository);
        // As the look at the runs of a workflow does, it names the entity tag of the answer it would give again.
        let condition = tag.map(|tag| format!("If-None-Match: {tag}"));
        let flags: Vec<&str> = condition.iter().flat_map(|header| ["-H", header.as_str()]).collect();
        let answer = self.ask_with("GET", &path, None, &flags).await?;
        match answer.status {
            200 => {
                let listed: Value = serde_json::from_str(&answer.body)
                    .map_err(|e| format!("GitHub's answer to GET {path} is not JSON: {e}"))?;
                let jobs = listed["jobs"].as_array().cloned().unwrap_or_default();
                Ok(Jobs::Listed {
                    jobs: jobs.iter().filter_map(job).collect(),
                    tag: answer.tag,
                })
            }
            304 if tag.is_some() => Ok(Jobs::Unchanged),
            304 => {
                Err(format!("GitHub answered GET {path} with 304, unchanged, though it was sent no entity tag").into())
            }
            _ => Err(refused(&answer)),
        }
    }

    async fn runners(&self) -> Runners {
        let repository = self.layer(&format!("repos/{}/actions/runners", self.repository)).await;
        let owner = self.repository.split('/').next().unwrap_or_default();
        // Only an organisation has runners of its own beside the repository's; a person's repository has the one layer.
        let organisation = match self.owner_is_an_organisation().await {
            Ok(false) => None,
            Ok(true) => Some(self.layer(&format!("orgs/{owner}/actions/runners")).await),
            Err(why) => Some(Err(why)),
        };
        Runners {
            repository,
            organisation,
        }
    }

    async fn artifact(&self, run: u64, name: &str) -> Result<Option<Vec<File>>, Failed> {
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
            Ok(()) => files_under(&dir, "").map_err(Failed::from),
            Err(why) if why.why.contains("no artifact") || why.why.contains("no valid artifact") => Ok(Vec::new()),
            // The credential gave no token, and said of what kind its failure is.
            Err(why) if why.kind.is_some() => Err(why),
            Err(why) => Err(told_by_the_command(why.why)),
        };
        let _ = tokio::fs::remove_dir_all(&dir).await;
        let files = files?;
        Ok((!files.is_empty()).then_some(files))
    }

    async fn log(&self, job: u64) -> Result<String, Failed> {
        let path = format!("repos/{}/actions/jobs/{job}/logs", self.repository);
        // GitHub answers with a redirect to the log's text, which `gh` follows. The log holds the escapes that colour
        // what the job printed, and `gh` refuses an answer that holds any unless told to allow them: the reader of the
        // log takes them out.
        let answer = (self.ask_with("GET", &path, None, &["--allow-escape-sequences"])).await?;
        if answer.status == 200 {
            Ok(answer.body)
        } else {
            Err(refused(&answer))
        }
    }

    async fn delete(&self, branch: &str) -> Result<(), Failed> {
        let path = format!("repos/{}/git/refs/heads/{branch}", self.repository);
        // A branch that is not there is answered 422, "Reference does not exist".
        self.want("DELETE", &path, None, &[204, 404, 422]).await.map(|_| ())
    }

    async fn edit(&self, number: u64, title: &str, body: &str) -> Result<(), Failed> {
        let (method, path, asked) = editing(&self.repository, number, title, body);
        self.want(method, &path, Some(asked), &[200]).await.map(|_| ())
    }

    async fn ready(&self, node: &str) -> Result<(), Failed> {
        let (method, path, asked) = readying(node);
        let answer = self.json(method, &path, Some(asked), &[200]).await?;
        // GraphQL says what it refused with a status of 200.
        match answer["errors"].as_array() {
            Some(errors) if !errors.is_empty() => {
                let words: Vec<&str> = errors.iter().filter_map(|e| e["message"].as_str()).collect();
                Err(Failed::refused(format!(
                    "GitHub did not make the pull request ready: {}",
                    words.join("; ")
                )))
            }
            _ => Ok(()),
        }
    }

    async fn close(&self, number: u64, comment: &str) -> Result<(), Failed> {
        let [(method, path, said), (closing, at, closed)] = closing(&self.repository, number, comment);
        self.want(method, &path, Some(said), &[201]).await?;
        self.want(closing, &at, Some(closed), &[200]).await.map(|_| ())
    }

    async fn reopen(&self, number: u64) -> Result<(), Failed> {
        let (method, path, asked) = reopening(&self.repository, number);
        self.want(method, &path, Some(asked), &[200]).await.map(|_| ())
    }
}

/// The request that opens the closed pull request again.
pub(crate) fn reopening(repository: &str, number: u64) -> Request {
    let path = format!("repos/{repository}/pulls/{number}");
    ("PATCH", path, json!({ "state": "open" }))
}

/// A request of GitHub's API: its method, its path and its body.
type Request = (&'static str, String, Value);

/// The request that opens a pull request of the branch into the base, as a draft.
pub(crate) fn opening(repository: &str, branch: &str, base: &str, title: &str, body: &str) -> Request {
    let path = format!("repos/{repository}/pulls");
    let asked = json!({ "title": title, "body": body, "head": branch, "base": base, "draft": true });
    ("POST", path, asked)
}

/// The request that merges the pull request by the method, provided its head is the commit, the commit the merge makes
/// titled so and with this message under its title: given no title or message, GitHub would take its own, which the
/// repository's settings choose.
pub(crate) fn merging(repository: &str, number: u64, head: &str, method: &str, title: &str, message: &str) -> Request {
    let path = format!("repos/{repository}/pulls/{number}/merge");
    let asked = json!({ "sha": head, "merge_method": method, "commit_title": title, "commit_message": message });
    ("PUT", path, asked)
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

/// The variable `gh` takes its token from.
const GH_TOKEN: &str = "GH_TOKEN";

/// The environment `gh` is given beside the token of each call: of the engine's, what git is given of it; then a
/// directory of its own for its configuration, and no prompt, no notice of a newer version and no colour.
pub(crate) fn environment(
    engine: impl IntoIterator<Item = (String, String)>,
    config_dir: &Path,
) -> Vec<(String, String)> {
    let kept = |name: &str| crate::cli::KEPT.contains(&name) || name.starts_with("LC_");
    let mut env: Vec<(String, String)> = engine.into_iter().filter(|(name, _)| kept(name)).collect();
    env.sort();
    let dir: PathBuf = config_dir.to_path_buf();
    env.extend([
        ("GH_CONFIG_DIR".to_string(), dir.to_string_lossy().into_owned()),
        ("GH_PROMPT_DISABLED".to_string(), "1".to_string()),
        ("GH_NO_UPDATE_NOTIFIER".to_string(), "1".to_string()),
        ("NO_COLOR".to_string(), "1".to_string()),
    ]);
    env
}

/// What `gh api --include` printed, read as GitHub's answer: the status line, the headers, an empty line, the body.
/// `curl --include` prints an answer in the same form.
pub(super) fn included(out: &str) -> Option<Answer> {
    let (head, body) = (out.split_once("\r\n\r\n")).or_else(|| out.split_once("\n\n"))?;
    let mut lines = head.lines();
    let status: u16 = lines.next()?.split_whitespace().nth(1)?.parse().ok()?;
    let headers: Vec<(String, &str)> = (lines.filter_map(|line| line.split_once(':')))
        .map(|(name, value)| (name.trim().to_ascii_lowercase(), value.trim()))
        .collect();
    let header = |wanted: &str| headers.iter().find(|(name, _)| name == wanted).map(|(_, value)| *value);
    let after = header("retry-after");
    let exhausted = header("x-ratelimit-remaining") == Some("0");
    // A secondary limit answers 403 or 429 and says so in its words, with neither header.
    let named = (status == 403 || status == 429) && body.to_ascii_lowercase().contains("rate limit");
    let retry = match (after.and_then(|seconds| seconds.parse().ok()), exhausted) {
        (Some(seconds), _) => Some(Retry::In(seconds)),
        (None, true) => header("x-ratelimit-reset")
            .and_then(|epoch| epoch.parse().ok())
            .map(Retry::At),
        (None, false) => None,
    };
    let limited = status == 429 || after.is_some() || exhausted || named;
    let tag = (headers.iter())
        .find(|(name, value)| name == "etag" && !value.is_empty())
        .map(|(_, value)| value.to_string());
    Some(Answer {
        status,
        body: body.to_string(),
        limited,
        retry,
        tag,
    })
}

/// GitHub's refusal, of the kind its status tells: a fault of GitHub's own (5xx) or its limit on the requests it takes
/// (429, or another refusal that says it limits them) passes by itself, and only keeps what asked from being done for
/// now, with when it may be asked again where GitHub says; a limit that says nothing of when is not asked again for at
/// least a minute ([`LIMIT_WAIT`]); any other is a refusal no wait mends, whatever its words.
pub(super) fn refused(answer: &Answer) -> Failed {
    let why = refusal(answer);
    if answer.status >= 500 {
        Failed::held(why, Obstacle::Fault, answer.retry)
    } else if answer.limited {
        Failed::held(
            why,
            Obstacle::Limit,
            Some(answer.retry.unwrap_or(Retry::In(LIMIT_WAIT))),
        )
    } else {
        Failed::refused(why)
    }
}

/// A failure of a `gh` command other than `gh api`: it says what GitHub refused as `HTTP <status>: <words>` on its
/// standard error and gives no headers. A fault of GitHub's own (5xx) and its limit (429, or a 403 whose words name a
/// rate limit) are held as `refused` holds them, a limit for a minute as nothing of when is given; any other failure is
/// the network's where its words say so ([`told_by_its_words`]), and is otherwise as it is.
fn told_by_the_command(why: String) -> Failed {
    let status = (why.find("HTTP ")).and_then(|at| why[at + "HTTP ".len()..].get(..3)?.parse::<u16>().ok());
    match status {
        Some(500..=599) => Failed::held(why, Obstacle::Fault, None),
        Some(429) => Failed::held(why, Obstacle::Limit, Some(Retry::In(LIMIT_WAIT))),
        Some(403) if why.to_ascii_lowercase().contains("rate limit") => {
            Failed::held(why, Obstacle::Limit, Some(Retry::In(LIMIT_WAIT)))
        }
        _ => told_by_its_words(why),
    }
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
        updated: value["updated_at"]
            .as_str()
            .and_then(epoch)
            .and_then(|seconds| u64::try_from(seconds).ok())
            .map(|seconds| seconds * 1000)
            .ok_or("a pull request has no time it was updated")?,
        branch: text(&value["head"]["ref"], "branch")?,
        // The head's repository is null where a fork was deleted.
        from_fork: value["head"]["repo"]["full_name"] != value["base"]["repo"]["full_name"],
    })
}

/// A run of a workflow, as GitHub's REST API lists it; none for one with no number.
fn run(run: &Value) -> Option<Run> {
    let status = run["status"].as_str();
    let completed = status == Some("completed");
    let cancelled = completed && run["conclusion"].as_str() == Some("cancelled");
    let took = match completed && !cancelled {
        true => (run["run_started_at"].as_str())
            .zip(run["updated_at"].as_str())
            .and_then(|(from, to)| seconds_between(from, to)),
        false => None,
    };
    Some(Run {
        id: run["id"].as_u64()?,
        title: run["display_title"].as_str().unwrap_or_default().to_string(),
        started: matches!(status, Some("in_progress" | "completed")),
        completed,
        cancelled,
        url: run["html_url"].as_str().unwrap_or_default().to_string(),
        took,
    })
}

/// A self-hosted runner as GitHub's REST API lists it.
fn runner(runner: &Value) -> Runner {
    Runner {
        online: runner["status"].as_str() == Some("online"),
        labels: (runner["labels"].as_array().into_iter().flatten())
            .filter_map(|label| label["name"].as_str().map(str::to_string))
            .collect(),
    }
}

/// A job of a run as a check; none for a job that has no id or no name. A job that did not run is a check that was
/// skipped; one that no runner has taken is queued.
fn job(job: &Value) -> Option<Check> {
    let outcome = match (job["status"].as_str(), job["conclusion"].as_str()) {
        (Some("completed"), Some("skipped")) => Outcome::Skipped,
        (Some("completed"), Some("success" | "neutral")) => Outcome::Passed,
        (Some("completed"), Some(how @ ("failure" | "timed_out"))) => Outcome::Failed(how.to_string()),
        (Some("completed"), Some(how)) => Outcome::Broken(how.to_string()),
        (Some("completed"), None) => Outcome::Broken("no conclusion".to_string()),
        (Some("in_progress"), _) => Outcome::Pending,
        _ => Outcome::Queued,
    };
    // Only a job that ran to a conclusion has a time to give; one cancelled or not complete did not finish.
    let took = match outcome {
        Outcome::Passed | Outcome::Failed(_) => job["started_at"]
            .as_str()
            .zip(job["completed_at"].as_str())
            .and_then(|(from, to)| seconds_between(from, to)),
        _ => None,
    };
    Some(Check {
        id: job["id"].as_u64()?,
        name: job["name"].as_str()?.to_string(),
        outcome,
        url: job["html_url"].as_str().map(str::to_string),
        took,
    })
}

/// The whole seconds from one timestamp to a later one, each in the form GitHub gives, `YYYY-MM-DDTHH:MM:SSZ` in UTC;
/// none where either is in another form or the second is before the first.
fn seconds_between(from: &str, to: &str) -> Option<u64> {
    u64::try_from(epoch(to)? - epoch(from)?).ok()
}

/// The seconds from 1970-01-01T00:00:00Z to a timestamp in the form GitHub gives, `YYYY-MM-DDTHH:MM:SSZ`; none for a
/// timestamp in any other form or of a day that does not exist.
pub(super) fn epoch(stamp: &str) -> Option<i64> {
    let bytes = stamp.as_bytes();
    let shaped = bytes.len() == 20
        && [(4, b'-'), (7, b'-'), (10, b'T'), (13, b':'), (16, b':'), (19, b'Z')]
            .iter()
            .all(|&(at, sign)| bytes[at] == sign);
    if !shaped {
        return None;
    }
    let number = |from: usize, to: usize| -> Option<i64> {
        let digits = stamp.get(from..to)?;
        digits
            .bytes()
            .all(|b| b.is_ascii_digit())
            .then(|| digits.parse().ok())
            .flatten()
    };
    let (year, month, day) = (number(0, 4)?, number(5, 7)?, number(8, 10)?);
    let (hour, minute, second) = (number(11, 13)?, number(14, 16)?, number(17, 19)?);
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days_in_month = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return None,
    };
    if !(1..=days_in_month).contains(&day) || hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    // Days from the civil date to 1970-01-01, counting the year from March so that a leap day ends it.
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let day_of_year = (153 * ((month + 9) % 12) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    let days = era * 146_097 + day_of_era - 719_468;
    Some(days * 86_400 + hour * 3600 + minute * 60 + second)
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

mod release;

pub use release::{GhOffsite, UPLOAD_RATE};

#[cfg(test)]
mod tests;
