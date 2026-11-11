//! GitHub as the host: main is the repository's main branch on GitHub, fetched into the local repository; the verdict
//! on a commit is the CI GitHub runs on it, reached through the work's pull request, which the check pushes the
//! commit to and opens where the branch has none; and the merge is the pull request's, made by GitHub.
//!
//! What it does is found again on GitHub by the work's branch and the commit, never kept by the engine: a check given
//! again after the engine started again pushes nothing GitHub has, finds the pull request it opened and reads the CI
//! of the same commit; a merge given again finds the pull request merged.
//!
//! An agent's check is the CI of the last commit of its work's branch, pushed and checked through the same pull
//! request; what the worktree holds that is not committed is said, and nothing is pushed. Its check before the change
//! pushes the commit made for it to a branch of its own, [`BEFORE`], which the project's CI is to run on, and takes
//! the branch away once its CI is read.
//!
//! - `api`: what is asked of GitHub besides pushing and fetching, and `gh`, which asks it with the `gh` command line.
//! - `log`: what of a red job's log tells why it is red.
//! - `fake`: GitHub for the tests, with a bare repository on disk to push to.

mod api;
mod gh;
mod log;

#[cfg(test)]
mod fake;
#[cfg(test)]
mod tests;

use std::path::Path;
use std::time::Duration;

pub use api::{Api, Check, Outcome, Pull, State};
pub use gh::Gh;

use super::{Branch, Host, Verdict};
use crate::git::{self, Before, With};
use crate::held::Stop;
use crate::process;
use crate::settings::{self, Project};

/// How often GitHub is asked how a CI goes: two calls a time, so a few hundred an hour for each work whose check
/// waits, of the 5,000 GitHub allows a token; and a CI of minutes is seen done half a minute late at most.
pub const POLL: Duration = Duration::from_secs(30);

/// How the GitHub host reaches the repository and judges its CI.
#[derive(Clone, PartialEq, Eq)]
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
    /// Whether the project's CI runs on a push to a branch under [`BEFORE`], which the check before the change needs.
    pub before_branches: bool,
    /// The engine's token, which git's pushes and fetches are given in their environment; none where the address
    /// needs none.
    pub token: Option<String>,
}

impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The token is not shown.
        f.debug_struct("Config")
            .field("url", &self.url)
            .field("remote", &self.remote)
            .field("checks", &self.checks)
            .field("start", &self.start)
            .field("timeout", &self.timeout)
            .field("poll", &self.poll)
            .field("merge_method", &self.merge_method)
            .field("delete_branch", &self.delete_branch)
            .field("before_branches", &self.before_branches)
            .field("token", &self.token.as_ref().map(|_| "(given)"))
            .finish()
    }
}

/// Where on GitHub the commits the agents' `check_before` makes are pushed, each to a branch of its own under this
/// name, which the project's CI must run on when it is pushed.
pub const BEFORE: &str = "wf-agents-before";

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
            before_branches: github.runs_on_before_branches,
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

    /// What git is given to push and fetch: neither the user's nor the system's configuration, no credential helper of
    /// the owner's, a connection that stalls given up, and, with a token, a helper that gives it for GitHub alone over
    /// HTTPS, from the command's environment.
    fn credentials(&self) -> With {
        // A connection slower than a kilobyte a second for a minute is given up, as a stalled one.
        let settings = ["credential.helper=", "http.lowSpeedLimit=1000", "http.lowSpeedTime=60"];
        let mut with = With {
            settings: settings.map(str::to_string).to_vec(),
            env: vec![
                ("GIT_CONFIG_GLOBAL".to_string(), "/dev/null".to_string()),
                ("GIT_CONFIG_NOSYSTEM".to_string(), "1".to_string()),
            ],
        };
        if let Some(token) = &self.config.token {
            with.settings.push(format!(
                "credential.helper=!f() {{ test \"$1\" = get || exit 0; host=; https=; while IFS= read -r line && test \
                 -n \"$line\"; do case \"$line\" in host={GITHUB}) host=1 ;; protocol=https) https=1 ;; esac; done; if \
                 test -n \"$host\" && test -n \"$https\"; then printf 'username=x-access-token\\npassword=%s\\n' \
                 \"${TOKEN}\"; fi; }}; f"
            ));
            with.env.push((TOKEN.to_string(), token.clone()));
        }
        with
    }

    /// What git is given to push to and fetch from GitHub, once the local repository's configuration, with what it
    /// includes, is found to send nothing elsewhere: a key of its own that rewrites an address (`url.*`), sets how
    /// HTTP goes (`http.*`: a proxy, the checking of certificates), or is set for an address itself (`remote.<address>.*`:
    /// another address, a program to run for the transfer) could send the token to another server or run a program
    /// with it, and is refused.
    async fn network(&self) -> Result<With, String> {
        let listed = git::git(&self.project.repo, &["config", "--list", "--show-scope", "--includes"]).await?;
        let found: Vec<String> = (listed.lines())
            .filter_map(|line| line.split_once('\t'))
            .filter(|(scope, _)| *scope == "local" || *scope == "worktree")
            .map(|(scope, entry)| {
                let key = entry.split_once('=').map_or(entry, |(key, _)| key).to_ascii_lowercase();
                (scope, key)
            })
            .filter(|(_, key)| key.starts_with("url.") || key.starts_with("http.") || for_an_address(key))
            .map(|(scope, key)| format!("{key} ({scope})"))
            .collect();
        if !found.is_empty() {
            return Err(format!(
                "the configuration of {} says where git's connections to GitHub go, which could send the engine's \
                 token elsewhere: {}. Take each out",
                self.project.repo.display(),
                found.join(", ")
            ));
        }
        Ok(self.credentials())
    }

    /// Pushes the commit to the work's branch on GitHub, never forcing it: a branch there that holds a commit this one
    /// does not is refused. A commit the branch is at already is pushed as nothing.
    async fn push(&self, branch: &Branch<'_>, commit: &str) -> Result<(), String> {
        self.push_to(commit, &branch.place.reference(), false).await
    }

    /// Pushes the commit to the branch on GitHub that the reference names, forcing it where `force` says so, or, with
    /// no commit, deletes that branch.
    async fn push_to(&self, commit: &str, reference: &str, force: bool) -> Result<(), String> {
        let target = format!("{}{commit}:{reference}", if force { "+" } else { "" });
        let args = [
            "push",
            "--quiet",
            "--no-verify",
            "--recurse-submodules=no",
            &self.config.url,
            &target,
        ];
        let pushed = git::said_with(&self.project.repo, &self.network().await?, &args).await?;
        if pushed.ok {
            return Ok(());
        }
        let name = reference.strip_prefix("refs/heads/").unwrap_or(reference);
        let what = if commit.is_empty() {
            format!("the deletion of the branch {name}")
        } else {
            format!("the push of {commit} to the branch {name}")
        };
        Err(format!(
            "{what} on GitHub was refused: {}",
            if pushed.err.is_empty() { pushed.out } else { pushed.err }
        ))
    }

    /// The CI of the commit in words for the agent that asked, `through` saying where it was read: each named check
    /// that passed by its name, and what failed as the check before the merge says it.
    async fn told(&self, commit: &str, through: &str, checks: &[Check]) -> String {
        let passed: Vec<&str> = (checks.iter())
            .filter(|c| c.outcome == Outcome::Passed)
            .map(|c| c.name.as_str())
            .collect();
        match self.failures(checks).await {
            None => format!(
                "The CI passed on the commit {commit}, {through}: {}.",
                passed.join(", ")
            ),
            Some(failed) => {
                let also = if passed.is_empty() {
                    String::new()
                } else {
                    format!(" It passed {}.", passed.join(", "))
                };
                format!("The CI of the commit {commit}, {through}, did not pass.{also}\n\n{failed}")
            }
        }
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
    /// and what its log says of the failure where GitHub keeps one, the logs sharing the bytes a check's output is
    /// given ([`process::TAIL`]); none where none failed.
    async fn failures(&self, checks: &[Check]) -> Option<String> {
        let failed: Vec<&Check> = (checks.iter())
            .filter(|c| matches!(c.outcome, Outcome::Failed(_)))
            .collect();
        if failed.is_empty() {
            return None;
        }
        let budget = process::TAIL / failed.len();
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
                        "What its log says of the failure:\n{}",
                        log::what_failed(&log, budget)
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
        let tracking = self.kept_main();
        let spec = format!("+refs/heads/{main}:{tracking}");
        let args = [
            "fetch",
            "--quiet",
            "--no-tags",
            "--no-recurse-submodules",
            &self.config.url,
            &spec,
        ];
        let fetched = git::said_with(&self.project.repo, &self.network().await?, &args).await?;
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
        // Only a pull request into main can be the work's merge: one merged into another branch put nothing on main.
        if let Some(merged) = pulls.iter().find(|p| p.state == State::Merged && p.base == *main) {
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
        let now = self.main().await?;
        let holds = git::said(&self.project.repo, &["merge-base", "--is-ancestor", &now, checked]).await?;
        if !holds.ok {
            return Err(super::moved(main, &now, checked));
        }
        let commit = (self.api.merge(open.number, checked, &self.config.merge_method).await)
            .map_err(|why| format!("GitHub did not merge the pull request {}: {why}", open.url))?;
        self.delete(branch).await?;
        Ok(commit)
    }

    fn kept_main(&self) -> String {
        format!("refs/remotes/{}/{}", self.config.remote, self.project.main)
    }

    async fn check(&self, branch: &Branch<'_>, stop: &mut Stop) -> Result<Option<String>, String> {
        // What the check before the merge refuses, this refuses the same way: the repository's configuration, a
        // worktree off the work's branch, and in the push, connections sent elsewhere and submodules.
        let place = branch.place;
        let dir = git::worktree(place)?;
        git::configuration(&self.project, place).await?;
        git::on_branch(place).await?;
        if git::merging(&dir).await? {
            let changes = git::uncommitted(&dir).await?;
            return Ok(Some(format!(
                "A merge is under way in the worktree and is not committed: finish it and commit it, or take it back. \
                 Nothing was pushed.\n{changes}"
            )));
        }
        let changes = git::uncommitted(&dir).await?;
        if !changes.is_empty() {
            return Ok(Some(format!(
                "The worktree has changes that are not committed, and only what is committed is checked: commit them, \
                 and ask again. Nothing was pushed.\n{changes}"
            )));
        }
        let repo = &self.project.repo;
        let commit = git::git(repo, &["rev-parse", &place.reference()]).await?;
        // GitHub opens no pull request for a branch with nothing of its own.
        let main = self.kept_main();
        if git::said(repo, &["merge-base", "--is-ancestor", &commit, &main])
            .await?
            .ok
        {
            return Ok(Some(format!(
                "The work's branch has no commit of its own beyond {}: commit your change, and ask again. Nothing was \
                 pushed.",
                self.project.main
            )));
        }
        self.push(branch, &commit).await?;
        let pull = self.pull_request(branch).await?;
        let Some(checks) = self.wait(&commit, stop).await? else {
            return Ok(None);
        };
        let through = format!("through the pull request {}", pull.url);
        Ok(Some(self.told(&commit, &through, &checks).await))
    }

    async fn check_before(
        &self,
        branch: &Branch<'_>,
        before: &Before,
        stop: &mut Stop,
    ) -> Result<Option<String>, String> {
        // A branch of its own for each commit, taken away once its CI is read: the commit is no commit of the work's
        // branch, and is the same commit whenever it is made of the same files.
        let _ = self.config.before_branches;
        let commit = &before.commit;
        let name = format!("{BEFORE}/{}-{}", branch.place.branch, &commit[..commit.len().min(12)]);
        let reference = format!("refs/heads/{name}");
        self.push_to(commit, &reference, true).await?;
        let waited = self.wait(commit, stop).await;
        let said = match waited {
            Ok(Some(checks)) => Ok(Some(self.told(commit, &format!("on the branch {name}"), &checks).await)),
            other => other.map(|_| None),
        };
        let taken = self.push_to("", &reference, false).await;
        let said = said?;
        taken?;
        Ok(said)
    }
}

/// Whether the key is one of a remote named by an address rather than a name, `remote.<address>.<name>`, which git
/// applies to every push and fetch given that address: a path, or what has a scheme or a host before a colon.
fn for_an_address(key: &str) -> bool {
    let Some(rest) = key.strip_prefix("remote.") else {
        return false;
    };
    let Some((sub, _)) = rest.rsplit_once('.') else {
        return false;
    };
    sub.contains(':') || sub.starts_with('/') || sub.starts_with('.') || sub.starts_with('~')
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
