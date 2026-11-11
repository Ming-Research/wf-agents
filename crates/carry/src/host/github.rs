//! GitHub as the host: main is the repository's main branch on GitHub, fetched into the local repository; the verdict
//! on a commit is the CI GitHub runs on it, reached through the work's pull request, which the check pushes the
//! commit to and opens where the branch has none; and the merge is the pull request's, made by GitHub.
//!
//! What it does is found again on GitHub by the work's branch and the commit, never kept by the engine: a check given
//! again after the engine started again pushes nothing GitHub has, finds the pull request it opened and reads the CI
//! of the same commit; a merge given again finds the pull request merged.
//!
//! - `api`: what is asked of GitHub besides pushing and fetching, and `gh`, which asks it with the `gh` command line.
//! - `fake`: GitHub for the tests, with a bare repository on disk to push to.

mod api;
mod gh;

#[cfg(test)]
mod fake;
#[cfg(test)]
mod tests;

use std::path::Path;
use std::time::Duration;

pub use api::{Api, Check, Outcome, Pull, State};
pub use gh::Gh;

use super::{Branch, Host, Verdict};
use crate::git::{self, With};
use crate::held::Stop;
use crate::process;
use crate::settings::{self, Project};

/// How often GitHub is asked how a CI goes: two calls a time, so a few hundred an hour for each work whose check
/// waits, of the 5,000 GitHub allows a token; and a CI of minutes is seen done half a minute late at most.
pub const POLL: Duration = Duration::from_secs(30);

/// How the GitHub host reaches the repository and judges its CI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    /// Where git pushes the works' branches and fetches main from: the repository's address on GitHub. It is given
    /// here, never read from the local repository's configuration, which an agent can write.
    pub url: String,
    /// The local repository's name for the repository on GitHub: main's head is fetched to
    /// `refs/remotes/<remote>/<main>`.
    pub remote: String,
    /// The checks that count, by the name of their check run or commit status.
    pub checks: Vec<String>,
    /// How long a named check may have no run on the commit before the CI is taken never to have started.
    pub start: Duration,
    /// How long the CI may take before it is given up.
    pub timeout: Duration,
    /// How often GitHub is asked how the CI goes.
    pub poll: Duration,
    /// How a pull request is merged: `squash`, `merge` or `rebase`.
    pub merge_method: String,
    /// Whether the work's branch on GitHub is deleted once it is merged.
    pub delete_branch: bool,
    /// The engine's token, which git's pushes and fetches are given in their environment; none where the address
    /// needs none.
    pub token: Option<String>,
}

/// GitHub, reached through the API given.
pub struct GitHub<A> {
    api: A,
    project: Project,
    config: Config,
}

/// The variable of a git command's environment that holds the token for its credential helper.
const TOKEN: &str = "WF_AGENTS_GITHUB_TOKEN";

/// The only host the credential helper gives the token for.
const GITHUB: &str = "github.com";

impl GitHub<Gh> {
    /// GitHub as the installation's settings say, reached with `gh` and the engine's token, which `env` gives from
    /// the engine's environment; `gh` keeps its configuration in the data directory, which is an absolute path. Says
    /// why where the token is not there.
    pub fn from_settings(
        github: &settings::GitHub,
        project: &Project,
        data: &Path,
        env: impl Fn(&str) -> Option<String>,
    ) -> Result<GitHub<Gh>, String> {
        let token = (env(&github.token_env).filter(|token| !token.is_empty())).ok_or_else(|| {
            format!(
                "github.token_env: the engine's environment has no {}, which is to hold its token for GitHub",
                github.token_env
            )
        })?;
        let dir = data.join("gh");
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let config = Config {
            url: github.url(),
            remote: github.remote.clone(),
            checks: github.checks.clone(),
            start: Duration::from_secs(github.start_minutes * 60),
            timeout: Duration::from_secs(github.timeout_minutes * 60),
            poll: POLL,
            merge_method: github.merge_method.clone(),
            delete_branch: github.delete_branch,
            token: Some(token.clone()),
        };
        let gh = Gh::new(&github.gh, &github.repository, &token, &dir);
        Ok(GitHub::new(gh, project.clone(), config))
    }
}

impl<A: Api> GitHub<A> {
    pub fn new(api: A, project: Project, config: Config) -> GitHub<A> {
        GitHub { api, project, config }
    }

    /// What git is given to push and fetch: no credential helper of the owner's, and, with a token, one that gives it
    /// for GitHub alone, from the command's environment.
    fn credentials(&self) -> With {
        // A connection slower than a kilobyte a second for a minute is given up, as a stalled one.
        let settings = ["credential.helper=", "http.lowSpeedLimit=1000", "http.lowSpeedTime=60"];
        let mut with = With {
            settings: settings.map(str::to_string).to_vec(),
            env: Vec::new(),
        };
        if let Some(token) = &self.config.token {
            with.settings.push(format!(
                "credential.helper=!f() {{ test \"$1\" = get || exit 0; ok=; while IFS= read -r line && test -n \
                 \"$line\"; do test \"$line\" = host={GITHUB} && ok=1; done; if test -n \"$ok\"; then printf \
                 'username=x-access-token\\npassword=%s\\n' \"${TOKEN}\"; fi; }}; f"
            ));
            with.env.push((TOKEN.to_string(), token.clone()));
        }
        with
    }

    /// Pushes the commit to the work's branch on GitHub, never forcing it: a branch there that holds a commit this one
    /// does not is refused. A commit the branch is at already is pushed as nothing.
    async fn push(&self, branch: &Branch<'_>, commit: &str) -> Result<(), String> {
        let target = format!("{commit}:{}", branch.place.reference());
        let args = ["push", "--quiet", "--no-verify", &self.config.url, &target];
        let pushed = git::said_with(&self.project.repo, &self.credentials(), &args).await?;
        if pushed.ok {
            return Ok(());
        }
        Err(format!(
            "the push of {commit} to the branch {} on GitHub was refused: {}",
            branch.place.branch,
            if pushed.err.is_empty() { pushed.out } else { pushed.err }
        ))
    }

    /// The work's open pull request into main, opened where the branch has none; or why the work cannot be checked
    /// through one: its pull request was closed or merged by someone, or goes into another branch.
    async fn pull_request(&self, branch: &Branch<'_>) -> Result<Pull, String> {
        let main = &self.project.main;
        let name = &branch.place.branch;
        let pulls = self.api.pulls(name).await?;
        if let Some(open) = pulls.iter().find(|p| p.state == State::Open) {
            if open.base != *main {
                return Err(format!(
                    "the pull request {} of the branch {name} goes into {}, not into {main}",
                    open.url, open.base
                ));
            }
            return Ok(open.clone());
        }
        if let Some(ended) = pulls.first() {
            let how = if ended.state == State::Merged {
                "merged"
            } else {
                "closed"
            };
            return Err(format!(
                "the pull request {} of the branch {name} was {how} on GitHub: the work is checked through no other; \
                 reopen it to go on, or cancel the work",
                ended.url
            ));
        }
        let body = body(branch);
        self.api.open(name, main, branch.title, &body).await
    }

    /// Waits for every named check of the commit to complete, and gives them; none where it was told to stop.
    async fn wait(&self, commit: &str, stop: &mut Stop) -> Result<Option<Vec<Check>>, String> {
        let began = tokio::time::Instant::now();
        loop {
            let checks = tokio::select! {
                checks = self.api.checks(commit) => checks?,
                () = stop.told() => return Ok(None),
            };
            if let Some(done) = self.judge(commit, &checks, began.elapsed())? {
                return Ok(Some(done));
            }
            tokio::select! {
                () = tokio::time::sleep(self.config.poll) => {}
                () = stop.told() => return Ok(None),
            }
        }
    }

    /// The named checks of the commit, once each has completed; none while one has not and may yet; or why the CI
    /// says nothing of the code: a check ended without judging it, one never started, or the CI took too long.
    fn judge(&self, commit: &str, checks: &[Check], waited: Duration) -> Result<Option<Vec<Check>>, String> {
        let (mut done, mut missing, mut pending, mut broken) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for name in &self.config.checks {
            let runs: Vec<&Check> = checks.iter().filter(|c| c.name == *name).collect();
            if runs.is_empty() {
                missing.push(name.as_str());
            }
            for check in runs {
                match &check.outcome {
                    Outcome::Pending => pending.push(name.as_str()),
                    Outcome::Broken(how) => broken.push(format!("{name} ({how}){}", at(check))),
                    Outcome::Passed | Outcome::Failed(_) => done.push(check.clone()),
                }
            }
        }
        if !broken.is_empty() {
            return Err(format!(
                "the CI of {commit} ended without judging it: {}",
                broken.join("; ")
            ));
        }
        if !missing.is_empty() && waited >= self.config.start {
            return Err(format!(
                "the CI never started on {commit}: no run of {} after {}",
                missing.join(", "),
                span(self.config.start)
            ));
        }
        if missing.is_empty() && pending.is_empty() {
            return Ok(Some(done));
        }
        if waited >= self.config.timeout {
            let waiting: Vec<&str> = missing.into_iter().chain(pending).collect();
            return Err(format!(
                "the CI of {commit} is not complete after {}: {} not done",
                span(self.config.timeout),
                waiting.join(", ")
            ));
        }
        Ok(None)
    }

    /// What failed of the checks, for the implementing agent: each red check with its link, what it said of itself,
    /// and the end of its log where GitHub keeps one; none where none failed.
    async fn failures(&self, checks: &[Check]) -> Option<String> {
        let failed: Vec<&Check> = (checks.iter())
            .filter(|c| matches!(c.outcome, Outcome::Failed(_)))
            .collect();
        if failed.is_empty() {
            return None;
        }
        let mut said = vec![format!(
            "The CI failed on this commit, in {} of its checks.",
            failed.len()
        )];
        for check in failed {
            let Outcome::Failed(how) = &check.outcome else {
                continue;
            };
            let mut lines = vec![format!("{} ({how}){}", check.name, at(check))];
            lines.extend(check.summary.clone());
            if let Some(job) = check.job {
                match self.api.log(job).await {
                    Ok(log) => lines.push(format!(
                        "The end of its log, up to its error:\n{}",
                        tail_of_log(&log).trim_end()
                    )),
                    Err(why) => lines.push(format!("Its log could not be read: {why}")),
                }
            }
            said.push(lines.join("\n"));
        }
        Some(said.join("\n\n"))
    }

    /// Deletes the work's branch on GitHub, where the settings say to.
    async fn delete(&self, branch: &Branch<'_>) -> Result<(), String> {
        if self.config.delete_branch {
            self.api.delete(&branch.place.branch).await?;
        }
        Ok(())
    }
}

impl<A: Api> Host for GitHub<A> {
    async fn main(&self) -> Result<String, String> {
        let main = &self.project.main;
        let tracking = format!("refs/remotes/{}/{main}", self.config.remote);
        let spec = format!("+refs/heads/{main}:{tracking}");
        let args = ["fetch", "--quiet", "--no-tags", &self.config.url, &spec];
        let fetched = git::said_with(&self.project.repo, &self.credentials(), &args).await?;
        if !fetched.ok {
            return Err(format!(
                "{main} could not be fetched from GitHub: {}",
                if fetched.err.is_empty() {
                    fetched.out
                } else {
                    fetched.err
                }
            ));
        }
        git::git(&self.project.repo, &["rev-parse", "--verify", &tracking]).await
    }

    async fn verdict(&self, branch: &Branch<'_>, commit: &str, stop: &mut Stop) -> Result<Option<Verdict>, String> {
        self.push(branch, commit).await?;
        let pull = self.pull_request(branch).await?;
        let Some(checks) = self.wait(commit, stop).await? else {
            return Ok(None);
        };
        Ok(Some(Verdict {
            failed: self.failures(&checks).await,
            by: Some(format!("the CI of the pull request {}", pull.url)),
        }))
    }

    async fn merge(&self, branch: &Branch<'_>, checked: Option<&str>) -> Result<String, String> {
        let Some(checked) = checked else {
            return Err(
                "the flow has no check before its merge: on GitHub, what is merged is the pull request the check opens"
                    .to_string(),
            );
        };
        let (main, name) = (&self.project.main, &branch.place.branch);
        let pulls = self.api.pulls(name).await?;
        if let Some(merged) = pulls.iter().find(|p| p.state == State::Merged) {
            if merged.head != checked {
                return Err(format!(
                    "the pull request {} was merged at {}, not at {checked}, the commit the check passed",
                    merged.url, merged.head
                ));
            }
            let commit = (merged.merge_commit.clone()).ok_or_else(|| {
                format!(
                    "GitHub says the pull request {} is merged, and by no commit",
                    merged.url
                )
            })?;
            self.delete(branch).await?;
            return Ok(commit);
        }
        let Some(open) = pulls.iter().find(|p| p.state == State::Open) else {
            return Err(format!(
                "the branch {name} has no open pull request on GitHub: the check opens it, and the merge merges it"
            ));
        };
        if open.base != *main {
            return Err(format!(
                "the pull request {} goes into {}, not into {main}",
                open.url, open.base
            ));
        }
        if open.head != checked {
            return Err(format!(
                "the pull request {} is at {}, not at {checked}, the commit the check passed: the work is checked \
                 again before it merges",
                open.url, open.head
            ));
        }
        let commit = (self.api.merge(open.number, checked, &self.config.merge_method).await)
            .map_err(|why| format!("GitHub did not merge the pull request {}: {why}", open.url))?;
        self.delete(branch).await?;
        Ok(commit)
    }
}

/// What the pull request of the work says of itself.
fn body(branch: &Branch<'_>) -> String {
    format!(
        "This pull request carries work {} of wf-agents.\n\nThe engine pushes here each commit of the work's branch \
         that it checks, and reads the CI of that commit. It merges the pull request once the check passed on the \
         commit it merges and the work's flow lets it go. What was decided in the work, and why, is on the engine's \
         page for the work.\n",
        branch.work.0
    )
}

/// The check's link, after a space, where it has one.
fn at(check: &Check) -> String {
    (check.url.as_ref()).map_or(String::new(), |url| format!(" {url}"))
}

/// A length of time in words: whole minutes from two minutes on, seconds below.
fn span(time: Duration) -> String {
    let seconds = time.as_secs();
    if seconds >= 120 && seconds.is_multiple_of(60) {
        format!("{} minutes", seconds / 60)
    } else {
        format!("{seconds} seconds")
    }
}

/// The end of a job's log as GitHub Actions keeps it, up to the last line where a step reported its error: the steps
/// that clean up after it would otherwise push the cause out of what is kept. Each line is read without the time
/// GitHub writes before it and without the codes that colour it; what is kept is as much as a local check's output.
pub(crate) fn tail_of_log(log: &str) -> String {
    let lines: Vec<String> = log.lines().map(plain).collect();
    let end = (lines.iter())
        .rposition(|line| line.contains("##[error]"))
        .map_or(lines.len(), |at| at + 1);
    let mut kept = lines[..end].join("\n");
    kept.push('\n');
    process::tail(kept.as_bytes())
}

/// A line of a job's log without its time and its colour codes.
fn plain(line: &str) -> String {
    let line = line.trim_start_matches('\u{feff}');
    let line = match line.split_once(' ') {
        Some((time, rest)) if time.len() >= 20 && time.ends_with('Z') && time.as_bytes()[10] == b'T' => rest,
        _ => line,
    };
    let mut plain = String::with_capacity(line.len());
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            // An escape: `ESC [`, then numbers and semicolons, up to the letter that ends it.
            for c in chars.by_ref() {
                if c.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            plain.push(c);
        }
    }
    plain
}
