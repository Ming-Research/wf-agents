//! GitHub as the host: main is the repository's main branch on GitHub, fetched into the local repository; the verdict
//! on a commit is the CI GitHub runs on it, reached through the work's pull request, which the check pushes the
//! commit to and opens where the branch has none, as a draft whose title and body it keeps to what the work says of
//! itself; and the merge is the pull request's, made by GitHub once it is made ready, and only onto a main whose CI is
//! not red. A work that has ended has the branches of its checks before the change taken away and, where it did not
//! merge, its pull request closed.
//!
//! Of the named checks, the check before the merge and an agent's check alike, one that fails on main's head as well is
//! not the change's and holds no work; one that failed in a run the work read before is run again, and passing then it
//! fails now and then. A check that is not named does not count.
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
mod fast;
mod gh;
mod log;
mod results;
mod test_name;

#[cfg(test)]
mod fake;
#[cfg(test)]
mod tests;

use std::path::Path;
use std::time::Duration;

pub use api::{Api, Check, Outcome, Pull, State};
pub use gh::{Gh, GhOffsite, UPLOAD_RATE};

use super::{Branch, Host, InUse, Named, Verdict, WHOLE};
use crate::git::{self, Before, With};
use crate::held::Stop;
use crate::process;
use crate::settings::{self, Project};

/// How often GitHub is asked how a CI goes: two calls a time, so a few hundred an hour for each work whose check
/// waits, of the 5,000 GitHub allows a token; and a CI of minutes is seen done half a minute late at most.
pub const POLL: Duration = Duration::from_secs(30);

/// How often GitHub is asked how a run of the fast CI goes: a run takes from half a minute to a minute or so, and is
/// waited for while an agent waits, so it is seen done within a few seconds, for a call a time.
pub const FAST_POLL: Duration = Duration::from_secs(5);

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
    /// The project's fast CI, which runs the tests and checks an agent names; none where it has none.
    pub fast: Option<settings::Fast>,
    /// How the lines of a red job's log are told apart, as the project's tools print them.
    pub log: settings::Log,
    /// How often GitHub is asked how a run of the fast CI goes.
    pub fast_poll: Duration,
    /// The engine's token, which git's pushes and fetches are given in their environment; none where the address
    /// needs none.
    pub token: Option<String>,
    /// The paths of this machine that nothing the engine publishes on GitHub names: the data directory, the project's
    /// local repository and the home directory, each said by what it is instead.
    pub withheld: Vec<(String, &'static str)>,
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
            .field("fast", &self.fast)
            .field("log", &self.log)
            .field("fast_poll", &self.fast_poll)
            .field("token", &self.token.as_ref().map(|_| "(given)"))
            .field("withheld", &self.withheld)
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
    /// The branches of the checks before the change running.
    in_use: InUse,
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
            fast: github.fast.clone(),
            log: github.log.clone(),
            fast_poll: FAST_POLL,
            token: Some(token.clone()),
            withheld: withheld(data, &project.repo, env("HOME")),
        };
        let gh = Gh::new(&github.gh, &github.repository, &token, &dir);
        Ok(GitHub::new(gh, project.clone(), config))
    }
}

impl<A: Api> GitHub<A> {
    pub fn new(api: A, project: Project, config: Config) -> GitHub<A> {
        GitHub {
            api,
            project,
            config,
            in_use: InUse::default(),
        }
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
    /// that passed by its name, what failed as the check before the merge says it, and the runs of a check run more
    /// than once on the commit.
    async fn told(&self, commit: &str, through: &str, judged: &Judged) -> String {
        let passed: Vec<&str> = (judged.checks.iter())
            .filter(|c| c.outcome == Outcome::Passed)
            .map(|c| c.name.as_str())
            .collect();
        let notes = judged.notes();
        let also = if notes.is_empty() {
            String::new()
        } else {
            format!("\n\nAlso:\n- {}", notes.join("\n- "))
        };
        match self.failures(judged).await {
            None => format!(
                "The CI passed on the commit {commit}, {through}: {}.{also}",
                passed.join(", ")
            ),
            Some(failed) => {
                let it_passed = if passed.is_empty() {
                    String::new()
                } else {
                    format!(" It passed {}.", passed.join(", "))
                };
                format!("The CI of the commit {commit}, {through}, did not pass.{it_passed}\n\n{failed}{also}")
            }
        }
    }

    /// The named checks of the commit once each has completed, with main's head as last brought in and the runs there
    /// of those that failed, so that what is red on main as well is said; none where it was told to stop. Nothing is
    /// run again.
    async fn judged(&self, commit: &str, stop: &mut Stop) -> Result<Option<Judged>, String> {
        let Some(checks) = self.wait(commit, stop).await? else {
            return Ok(None);
        };
        let mut on_main = Vec::new();
        if checks.iter().any(failed) {
            let main = git::git(&self.project.repo, &["rev-parse", "--verify", &self.kept_main()]).await?;
            let there = self.api.checks(&main).await?;
            for check in checks.iter().filter(|c| failed(c)) {
                if let Some(red) = there.iter().find(|m| m.name == check.name && failed(m)) {
                    on_main.push((check.name.clone(), main.clone(), red.url.clone()));
                }
            }
        }
        Ok(Some(Judged { checks, on_main }))
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
            let body = self.body(branch);
            if open.title != branch.title || open.body != body {
                self.api.edit(open.number, branch.title, &body).await?;
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
        let body = self.body(branch);
        match self.api.open(name, main, branch.title, &body).await {
            Ok(opened) => Ok(opened),
            // Another check of the work, which found none either, opened it meanwhile: that one is the work's.
            Err(why) => {
                let pulls = self.api.pulls(name).await?;
                let open = pulls.into_iter().find(|p| p.state == State::Open && p.base == *main);
                open.ok_or(why)
            }
        }
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
    /// whether it is red on main's head as well, and what its log says of the failure where GitHub keeps one, the logs sharing the bytes a check's output is
    /// given ([`process::TAIL`]); none where none failed.
    async fn failures(&self, judged: &Judged) -> Option<String> {
        let failed: Vec<&Check> = (judged.checks.iter()).filter(|c| failed(c)).collect();
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
            if let Some((_, main, link)) = judged.on_main.iter().find(|(name, ..)| *name == check.name) {
                let link = link.as_ref().map_or(String::new(), |link| format!(" ({link})"));
                lines.push(format!(
                    "It is also red on {} at {main}{link}; whether the change adds a failure of its own is not known.",
                    self.project.main
                ));
            }
            lines.extend(check.summary.clone());
            if let Some(job) = check.job {
                match self.api.log(job).await {
                    Ok(log) => lines.push(format!(
                        "What its log says of the failure, as the CI printed it:\n{}",
                        log::fenced(&log::what_failed(&log, budget, &self.config.log))
                    )),
                    Err(why) => lines.push(format!("Its log could not be read: {why}")),
                }
            }
            said.push(lines.join("\n"));
        }
        Some(said.join("\n\n"))
    }

    /// Why no work merges onto main's head, where a named check of its CI failed there: a red main would take the
    /// work's merge into what is broken, and the work's own check could not tell what it broke. Said with what the
    /// owner does next. A CI of main not complete holds nothing.
    async fn red(&self, main: &str) -> Result<Option<String>, String> {
        let checks = self.api.checks(main).await?;
        let red: Vec<String> = (checks.iter())
            .filter(|c| self.config.checks.contains(&c.name) && failed(c))
            .map(|c| format!("{}{}", c.name, at(c)))
            .collect();
        if red.is_empty() {
            return Ok(None);
        }
        Ok(Some(format!(
            "{} is red at {main}: {}. No work merges onto a red {0}; once it is green again, let the work on",
            self.project.main,
            red.join(", ")
        )))
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
        let Some(judged) = self.judged(commit, stop).await? else {
            return Ok(None);
        };
        Ok(Some(Verdict {
            failed: self.failures(&judged).await,
            by: Some(format!("the CI of the pull request {}", pull.url)),
            notes: judged.notes(),
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
        // A named check red on the commit merges nothing, whatever made it red and whatever main is: as when it was run
        // again on GitHub after the check passed it.
        let red: Vec<String> = (self.api.checks(checked).await?.iter())
            .filter(|c| self.config.checks.contains(&c.name) && failed(c))
            .map(|c| format!("{} failed on {checked}{}", c.name, at(c)))
            .collect();
        if !red.is_empty() {
            return Err(format!(
                "{}, in its latest run since the check passed it: a commit with a named check red is not merged. Write \
                 to the check before the merge to have it run again, or to the implementation to fix it",
                red.join("; ")
            ));
        }
        let now = self.main().await?;
        let holds = git::said(&self.project.repo, &["merge-base", "--is-ancestor", &now, checked]).await?;
        if !holds.ok {
            return Err(super::moved(main, &now, checked));
        }
        if let Some(red) = self.red(&now).await? {
            return Err(red);
        }
        if open.draft {
            (self.api.ready(&open.node).await)
                .map_err(|why| format!("GitHub did not make the pull request {} ready: {why}", open.url))?;
        }
        let commit = (self.api.merge(open.number, checked, &self.config.merge_method).await)
            .map_err(|why| format!("GitHub did not merge the pull request {}: {why}", open.url))?;
        self.delete(branch).await?;
        Ok(commit)
    }

    fn kept_main(&self) -> String {
        kept_main(&self.config.remote, &self.project.main)
    }

    async fn check(&self, branch: &Branch<'_>, named: &Named, stop: &mut Stop) -> Result<Option<String>, String> {
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
        let fast = self.config.fast.as_ref().filter(|_| !named.is_empty());
        if let Some(why) = fast.and_then(|fast| fast::refused(fast, named)) {
            return Ok(Some(why));
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
        if let Some(fast) = fast {
            return self.fast(fast, &commit, named, stop).await;
        }
        let Some(judged) = self.judged(&commit, stop).await? else {
            return Ok(None);
        };
        let through = format!("through the pull request {}", pull.url);
        Ok(Some(whole(named, self.told(&commit, &through, &judged).await)))
    }

    async fn check_before(
        &self,
        branch: &Branch<'_>,
        before: &Before,
        named: &Named,
        stop: &mut Stop,
    ) -> Result<Option<String>, String> {
        let fast = self.config.fast.as_ref().filter(|_| !named.tests.is_empty());
        if let Some(fast) = fast {
            if !named.checks.is_empty() {
                return Ok(Some(format!(
                    "Nothing was checked: the check before the change runs the tests named alone, and no check ({}): \
                     a check would judge the code before the change, which is not yours.",
                    named.checks.join(", ")
                )));
            }
            if let Some(why) = fast::refused(fast, named) {
                return Ok(Some(why));
            }
        } else if self.config.fast.is_some() && !self.config.before_branches {
            // The whole CI does not run where the code before the change is pushed: only tests named are run, by the
            // fast CI. Said as a result, as what the agent can mend.
            return Ok(Some(NO_TESTS_BEFORE.to_string()));
        } else if !self.config.before_branches {
            return Err(format!(
                "the installation's settings do not say that the project's CI runs on a push to the branches under \
                 {BEFORE}/ (github.runs_on_before_branches), so nothing was pushed: the CI would never start there"
            ));
        }
        // A branch of its own for each commit, taken away once its CI is read: the commit is no commit of the work's
        // branch, and is the same commit whenever it is made of the same files.
        let commit = &before.commit;
        let mine = format!("refs/heads/{BEFORE}/{}-{}", branch.place.branch, short(commit));
        let name = mine.trim_start_matches("refs/heads/");
        let _held = self.in_use.hold(&mine).await;
        self.sweep(branch).await?;
        if fast.is_none() {
            // A commit checked already is answered from its CI, and pushed again to no branch, where a new run would
            // start. One GitHub has not been given has no checks, and GitHub answers so with a refusal.
            let now = match self.api.checks(commit).await {
                Ok(now) => now,
                Err(why) if why.starts_with("HTTP 422") => Vec::new(),
                Err(why) => return Err(why),
            };
            if let Ok(Some(done)) = self.judge(commit, &now, Duration::ZERO) {
                let done = Judged::as_they_are(done);
                let said = self.told(commit, &format!("on the branch {name}"), &done).await;
                return Ok(Some(whole(named, said)));
            }
        }
        self.push_to(commit, &mine, true).await?;
        // The code before the change is to fail where its tests do: what fails is said as it is.
        let said = match fast {
            Some(fast) => self.fast(fast, commit, named, stop).await,
            None => match self.wait(commit, stop).await {
                Ok(Some(checks)) => {
                    let checks = Judged::as_they_are(checks);
                    let said = self.told(commit, &format!("on the branch {name}"), &checks).await;
                    Ok(Some(whole(named, said)))
                }
                other => other.map(|_| None),
            },
        };
        let taken = self.push_to("", &mine, false).await;
        let said = said?;
        taken?;
        Ok(said)
    }

    async fn end(&self, branch: &Branch<'_>, merged: bool) -> Result<(), String> {
        self.sweep(branch).await?;
        if merged {
            return Ok(());
        }
        // Only the work's pull request into main is the work's: one into another branch is a person's.
        let pulls = self.api.pulls(&branch.place.branch).await?;
        for open in (pulls.iter()).filter(|p| p.state == State::Open && p.base == self.project.main) {
            (self.api.close(open.number, super::CLOSED).await)
                .map_err(|why| format!("GitHub did not close the pull request {}: {why}", open.url))?;
        }
        Ok(())
    }
}

impl<A: Api> GitHub<A> {
    /// Deletes on GitHub the branches of the work's checks before the change that no check of this process holds: an
    /// engine that ended while such a check waited left them.
    async fn sweep(&self, branch: &Branch<'_>) -> Result<(), String> {
        let args = ["ls-remote", "--heads", &self.config.url];
        let listed = git::said_with(&self.project.repo, &self.network().await?, &args).await?;
        if !listed.ok {
            return Err(format!("the branches on GitHub could not be listed: {}", listed.err));
        }
        let beginning = format!("refs/heads/{BEFORE}/{}-", branch.place.branch);
        let left: Vec<&str> = (listed.out.lines())
            .filter_map(|line| line.split_once('\t').map(|(_, reference)| reference))
            .filter(|reference| reference.starts_with(&beginning) && !self.in_use.held(reference))
            .collect();
        for reference in left {
            self.push_to("", reference, false).await?;
        }
        Ok(())
    }
}

/// What the check before the change says where it names no test and only the project's fast CI runs on the code before
/// the change.
pub(super) const NO_TESTS_BEFORE: &str = "Nothing was checked: the check before the change runs the tests you name on \
                                          the project's fast CI, and its whole CI does not run on the code before the \
                                          change. Name the tests to run, a line `test: <name>` each, beside the \
                                          `file: <path>` lines.";

/// The beginning of a commit by which a branch of one is named.
fn short(commit: &str) -> &str {
    &commit[..commit.len().min(12)]
}

/// The reference that holds main's head as GitHub last gave it, fetched there from the remote of this name, main of
/// this name.
pub(crate) fn kept_main(remote: &str, main: &str) -> String {
    format!("refs/remotes/{remote}/{main}")
}

/// What a check that ran whole found, said after [`WHOLE`] where the agent named what it does not run apart.
fn whole(named: &Named, said: String) -> String {
    if named.is_empty() {
        said
    } else {
        format!("{WHOLE}\n\n{said}")
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

/// How much of what the work wrote of itself its pull request shows, in characters; the rest is on the engine's page.
/// GitHub keeps a body of 65,536 characters, and a reader of the pull request needs far less than that.
const ABOUT: usize = 16_000;

/// The words that close an issue a pull request names once it merges, as GitHub reads them.
const CLOSING: [&str; 9] = [
    "close", "closes", "closed", "fix", "fixes", "fixed", "resolve", "resolves", "resolved",
];

/// What breaks a mention, a reference or a closing word as GitHub reads them, and shows as nothing.
const UNSEEN: char = '\u{200B}';

/// What an agent wrote, as it may be published on the repository: no mention notifies anyone, no reference names an
/// issue or a pull request, and no closing word closes one, each broken by an unseen space after its first character
/// (`@`, `#`, the word's first letter); every path withheld is said as what it is; and the text is cut to [`ABOUT`]
/// characters, saying so. GitHub's own reading of a fenced block is not relied on for any of it.
pub(crate) fn neutral(text: &str, withheld: &[(String, &'static str)]) -> String {
    let mut text = text.to_string();
    for (path, said) in withheld {
        text = text.replace(path.as_str(), said);
    }
    if text.chars().count() > ABOUT {
        text = text.chars().take(ABOUT).collect();
        text.push_str("\n(cut: the whole is on the engine's page)");
    }
    let mut out = String::with_capacity(text.len() + 16);
    let mut word = String::new();
    let flush = |word: &mut String, out: &mut String| {
        if CLOSING.contains(&word.to_ascii_lowercase().as_str()) {
            let mut chars = word.chars();
            out.extend(chars.next());
            out.push(UNSEEN);
            out.extend(chars);
        } else {
            out.push_str(word);
        }
        word.clear();
    };
    for c in text.chars() {
        if c.is_alphanumeric() {
            word.push(c);
            continue;
        }
        flush(&mut word, &mut out);
        out.push(c);
        if c == '@' || c == '#' {
            out.push(UNSEEN);
        }
    }
    flush(&mut word, &mut out);
    out
}

/// A fence of backticks longer than any run of them in the text, so that the text cannot end the block it is in.
fn fence(text: &str) -> String {
    let mut longest = 0;
    let mut run = 0;
    for c in text.chars() {
        run = if c == '`' { run + 1 } else { 0 };
        longest = longest.max(run);
    }
    "`".repeat((longest + 1).max(3))
}

impl<A: Api> GitHub<A> {
    /// What the pull request of the work says of itself: what the work wrote of itself, as a block of plain text made
    /// [`neutral`], then what the pull request is. A person's edit of the body is written over at the next check.
    fn body(&self, branch: &Branch<'_>) -> String {
        let about = neutral(branch.about.trim(), &self.config.withheld);
        let what = format!(
            "This pull request carries work {} of wf-agents.\n\nThe engine pushes here each commit of the work's \
             branch that it checks, and reads the CI of that commit. It merges the pull request once the check passed \
             on the commit it merges and the work's flow lets it go. What was decided in the work, and why, is on the \
             engine's page for the work. The text above is written over at each check.\n",
            branch.work.0
        );
        if about.is_empty() {
            return what;
        }
        let fence = fence(&about);
        format!("What the work's agents wrote of the change:\n\n{fence}text\n{about}\n{fence}\n\n---\n\n{what}")
    }
}

/// The named checks of a commit, each by its latest run, and of those that failed, the runs that failed on main's head
/// as well, by the check's name, main's head and the run's link.
struct Judged {
    checks: Vec<Check>,
    on_main: Vec<(String, String, Option<String>)>,
}

impl Judged {
    /// The checks as they are, with nothing looked for on main: the code before a change is to fail where its tests do.
    fn as_they_are(checks: Vec<Check>) -> Judged {
        Judged {
            checks,
            on_main: Vec::new(),
        }
    }

    /// The runs of each check run more than once on the commit, said as they are.
    fn notes(&self) -> Vec<String> {
        (self.checks.iter())
            .filter(|c| !c.earlier.is_empty())
            .map(|c| {
                let latest = match &c.outcome {
                    Outcome::Passed => "passed".to_string(),
                    Outcome::Failed(how) | Outcome::Broken(how) => format!("ended in {how}"),
                    Outcome::Pending => "is not complete".to_string(),
                };
                let link = c.url.as_ref().map_or(String::new(), |url| format!(" ({url})"));
                format!(
                    "{}: its latest run on this commit {latest}{link}; its earlier runs on it: {}.",
                    c.name,
                    c.earlier.join(", ")
                )
            })
            .collect()
    }
}

/// The paths nothing published names, each with what is said in its place: the longest first, so that one inside
/// another is said as the one it is in.
pub(crate) fn withheld(data: &Path, repo: &Path, home: Option<String>) -> Vec<(String, &'static str)> {
    let mut withheld = vec![
        (data.display().to_string(), "<the data directory>"),
        (repo.display().to_string(), "<the repository>"),
    ];
    withheld.extend(
        home.filter(|home| home.len() > 1)
            .map(|home| (home, "<the home directory>")),
    );
    withheld.sort_by_key(|(path, _)| std::cmp::Reverse(path.len()));
    withheld
}

/// Whether the check judged the code and found it wanting.
fn failed(check: &Check) -> bool {
    matches!(check.outcome, Outcome::Failed(_))
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
