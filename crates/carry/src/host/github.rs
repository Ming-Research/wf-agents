//! GitHub as the host: main is the repository's main branch on GitHub, fetched into the local repository; the verdict
//! on a commit is the project's full workflows run by hand on it, once, and judged by the JUnit they upload and by
//! their jobs' conclusions, never by the pull request's checks or a log, where only tests failed the fast workflow
//! running them again, on the commit and on main's head; the work's pull request, which the check
//! pushes the commit to and opens where the branch has none, as a draft whose title it keeps to the name of the work's
//! merge and whose body to what the work says of itself, is what is merged, by GitHub once it is made ready, in a
//! commit of that name. A work that has ended has, where it did not merge, its pull request closed.
//!
//! What it does is found again on GitHub, never by what the engine kept: the pull request by the work's branch, and a
//! run of a workflow by the value its name holds, worked out from what asked for it. A check given again after the
//! engine started again pushes nothing GitHub has, finds the pull request it opened, and waits for the runs it asked
//! for on the same commit, or reads them where they have ended; a merge given again finds the pull request merged. The
//! pull request each check finds is said, through what it was given to say it with, so that the work keeps its number
//! and address for its page.
//!
//! An agent's `run_tests` runs the tests and checks it names by the fast workflow (the module `fast`), on the commit it
//! names or else the last commit of its work's branch, which is pushed to a branch of its own for the call and deleted
//! once the run's result is read; the work's own branch and its pull request are the check's. Where no commit is
//! named, what the worktree holds that is not committed is said, and nothing is pushed. A request that names neither a
//! test nor a check runs nothing.
//!
//! - `api`: what is asked of GitHub besides pushing and fetching, and `gh`, which asks it with the `gh` command line.
//! - `output`: what a test said, as a reader wants it.
//! - `log`: what a job's log says of the step that failed it, which is said of a job that failed with no failed test
//!   of its own to say why.
//! - `fake`: GitHub for the tests, with a bare repository on disk to push to.

mod api;
mod fast;
mod gh;
mod log;
mod output;
mod results;
mod workflow;

#[cfg(test)]
mod fake;
#[cfg(test)]
mod tests;

use std::path::Path;

use flow::agents::fill;
use std::time::Duration;
use uuid::Uuid;

pub use api::{Api, Pull, State};
pub use gh::{Gh, GhOffsite, UPLOAD_RATE};

use super::{Asking, Branch, Failed, Host, Named, NotMerged, Verdict};
use crate::git::{self, With};
use crate::held::Stop;
use crate::settings::{self, Project};

/// How often GitHub is asked how a run of a workflow goes: a run takes from half a minute to some minutes, and is waited
/// for while an agent or the check before a merge waits, so it is seen done within a few seconds, for a call a time.
pub const FAST_POLL: Duration = Duration::from_secs(5);

/// What GitHub says where it refuses a merge because main moved as it was to merge (`HTTP 405`).
const BASE_MODIFIED: &str = "Base branch was modified";

/// Where a look at the network to GitHub connects: whether it resolves and takes a connection tells whether the acts'
/// host can be reached.
pub const LOOK: &str = "github.com:443";

/// How the GitHub host reaches the repository and judges its CI.
#[derive(Clone)]
pub struct Config {
    /// Where git pushes the works' branches and fetches main from: the repository's address on GitHub. It is given
    /// here, never read from the local repository's configuration, which an agent can write.
    pub url: String,
    /// The local repository's name for the repository on GitHub: main's head is fetched to
    /// `refs/remotes/<remote>/<main>`.
    pub remote: String,
    /// How long a workflow's run may be unlisted or queued before the CI is taken never to have started.
    pub start: Duration,
    /// How long the CI may take before it is given up.
    pub timeout: Duration,
    /// How a pull request is merged: `squash`, `merge` or `rebase`.
    pub merge_method: String,
    /// Whether the work's branch on GitHub is deleted once it is merged.
    pub delete_branch: bool,
    /// The project's two workflows, as its declaration gives them; none where it declares none.
    pub ci: Option<flow::agents::Ci>,
    /// The runners this installation has, by the name a workflow of the project's declaration gives them.
    pub runners: Vec<String>,
    /// The words of the tool `run_tests`, from its file beside the flows.
    pub tool: Option<flow::agents::Tool>,
    /// The words of the check before a merge, from its file beside the flows, which say what it ran again.
    pub check: Option<flow::agents::Check>,
    /// How often GitHub is asked how a run of a workflow goes.
    pub fast_poll: Duration,
    /// The engine's token, which git's pushes and fetches are given in their environment; none where the address
    /// needs none.
    pub token: Option<String>,
    /// The paths of this machine that nothing the engine publishes on GitHub names: the data directory, the project's
    /// local repository and the home directory, each said by what it is instead.
    pub withheld: Vec<(String, String)>,
    /// What the engine writes on a work's pull request, from its file beside the flows.
    pub words: flow::agents::PullRequest,
    /// The installation's id, in whose namespace the value that finds a run of a workflow is worked out.
    pub installation: Uuid,
}

impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The token is not shown.
        f.debug_struct("Config")
            .field("url", &self.url)
            .field("remote", &self.remote)
            .field("start", &self.start)
            .field("timeout", &self.timeout)
            .field("merge_method", &self.merge_method)
            .field("delete_branch", &self.delete_branch)
            .field("ci", &self.ci)
            .field("runners", &self.runners)
            .field("fast_poll", &self.fast_poll)
            .field("token", &self.token.as_ref().map(|_| "(given)"))
            .field("withheld", &self.withheld)
            .field("words", &self.words)
            .field("installation", &self.installation)
            .finish()
    }
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
    /// GitHub as the installation's settings and the project's declaration say, for the installation of this id,
    /// reached with `gh` and the engine's token, which `env` gives from the engine's environment; `gh` keeps its
    /// configuration in the data directory, which is an absolute path. Says why where the token is not there.
    pub fn from_settings(
        github: &settings::GitHub,
        project: &Project,
        data: &Path,
        installation: Uuid,
        env: impl Fn(&str) -> Option<String>,
        ci: Option<flow::agents::Ci>,
        (tool, check, words): (
            Option<flow::agents::Tool>,
            Option<flow::agents::Check>,
            flow::agents::PullRequest,
        ),
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
            start: Duration::from_secs(github.start_minutes * 60),
            timeout: Duration::from_secs(github.timeout_minutes * 60),
            merge_method: github.merge_method.clone(),
            delete_branch: github.delete_branch,
            ci,
            runners: github.runners.clone(),
            tool,
            check,
            fast_poll: FAST_POLL,
            token: Some(token.clone()),
            withheld: withheld(data, &project.repo, env("HOME"), &words.withheld),
            words,
            installation,
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

    /// The work's open pull request into main, opened where the branch has none; or why the work cannot be checked
    /// through one: its pull request was closed or merged by someone, or goes into another branch.
    async fn pull_request(&self, branch: &Branch<'_>) -> Result<Pull, Failed> {
        let main = &self.project.main;
        let name = &branch.place.branch;
        let pulls = self.api.pulls(name).await?;
        if let Some(open) = pulls.iter().find(|p| p.state == State::Open) {
            if open.base != *main {
                return Err(Failed::refused(format!(
                    "the pull request {} of the branch {name} goes into {}, not into {main}",
                    open.url, open.base
                )));
            }
            self.follow(open, branch).await?;
            return Ok(open.clone());
        }
        if let Some(ended) = pulls.first() {
            let how = if ended.state == State::Merged {
                "merged"
            } else {
                "closed"
            };
            return Err(Failed::refused(format!(
                "the pull request {} of the branch {name} was {how} on GitHub: the work is checked through no other; \
                 reopen it to go on, or cancel the work",
                ended.url
            )));
        }
        let (title, body) = (self.title(branch), self.body(branch));
        match self.api.open(name, main, &title, &body).await {
            Ok(opened) => Ok(opened),
            // Another check of the work, which found none either, opened it meanwhile: that one is the work's.
            Err(why) => {
                let pulls = self.api.pulls(name).await?;
                let open = pulls.into_iter().find(|p| p.state == State::Open && p.base == *main);
                open.ok_or(why)
            }
        }
    }

    /// Gives the work's open pull request the title and the body the work's branch has it say now, where it says
    /// other.
    async fn follow(&self, open: &Pull, branch: &Branch<'_>) -> Result<(), Failed> {
        let (title, body) = (self.title(branch), self.body(branch));
        if open.title != title || open.body != body {
            self.api.edit(open.number, &title, &body).await?;
        }
        Ok(())
    }

    /// The commit the name gives in the worktree, in full; or why git finds none by it.
    async fn named_commit(&self, dir: &Path, name: &str) -> Result<Result<String, String>, String> {
        // A name that begins with a dash would be read as an option by git, whatever follows it.
        if name.starts_with('-') {
            return Ok(Err(format!("{name}: a name does not begin with a dash")));
        }
        let spec = format!("{name}^{{commit}}");
        let found = git::said(dir, &["rev-parse", "--verify", "--quiet", "--end-of-options", &spec]).await?;
        if found.ok && !found.out.is_empty() {
            return Ok(Ok(found.out));
        }
        let said = if found.err.is_empty() {
            format!("{name} is no commit")
        } else {
            found.err
        };
        Ok(Err(said))
    }

    /// The branches on GitHub that calls of `run_tests` of the work pushed commits to, as their names there; those
    /// the call deleted already are not among them.
    async fn trials(&self, branch: &Branch<'_>) -> Result<Vec<String>, String> {
        let pattern = format!("refs/heads/{}", trial_pattern(branch));
        let args = ["ls-remote", "--heads", &self.config.url, &pattern];
        let listed = git::said_with(&self.project.repo, &self.network().await?, &args).await?;
        if !listed.ok {
            return Err(format!(
                "the branches of the work's calls of run_tests could not be listed on GitHub: {}",
                if listed.err.is_empty() { listed.out } else { listed.err }
            ));
        }
        Ok((listed.out.lines())
            .filter_map(|line| line.split_once('\t'))
            .filter_map(|(_, reference)| reference.strip_prefix("refs/heads/"))
            .map(str::to_string)
            .collect())
    }

    /// Closes the work's open pull request into main, saying its work ended. Only that one is the work's: one into
    /// another branch is a person's.
    async fn close_pulls(&self, branch: &Branch<'_>) -> Result<(), String> {
        let pulls = self.api.pulls(&branch.place.branch).await?;
        for open in (pulls.iter()).filter(|p| p.state == State::Open && p.base == self.project.main) {
            (self.api.close(open.number, &self.config.words.closed).await)
                .map_err(|why| format!("GitHub did not close the pull request {}: {why}", open.url))?;
        }
        Ok(())
    }

    /// Deletes what the calls of `run_tests` left of the branches they pushed commits to, where the engine stopped
    /// before it read the result: they are the work's, whether or not it merged.
    async fn sweep_trials(&self, branch: &Branch<'_>) -> Result<(), String> {
        for left in self.trials(branch).await? {
            (self.api.delete(&left).await).map_err(|why| format!("GitHub did not delete the branch {left}: {why}"))?;
        }
        Ok(())
    }

    /// Deletes the work's branch on GitHub, where the settings say to.
    async fn delete(&self, branch: &Branch<'_>) -> Result<(), Failed> {
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

    async fn verdict(
        &self,
        branch: &Branch<'_>,
        commit: &str,
        main: &str,
        asking: Asking,
        _: &tokio::sync::Mutex<()>,
        stop: &mut Stop,
    ) -> Result<Option<Verdict>, Failed> {
        self.push(branch, commit).await?;
        let pull = self.pull_request(branch).await?;
        stop.pulled(pull.number, &pull.url);
        let request = fast::Request {
            work: branch.work,
            commit,
            asking,
        };
        self.judge_full(request, main, stop).await
    }

    async fn merge(&self, branch: &Branch<'_>, checked: Option<&str>) -> Result<String, NotMerged> {
        let Some(checked) = checked else {
            return Err(NotMerged::refused(
                "the flow has no check before its merge: on GitHub, what is merged is the pull request the check opens"
                    .to_string(),
            ));
        };
        let (main, name) = (&self.project.main, &branch.place.branch);
        let pulls = self.api.pulls(name).await?;
        // Only a pull request into main can be the work's merge: one merged into another branch put nothing on main.
        if let Some(merged) = pulls.iter().find(|p| p.state == State::Merged && p.base == *main) {
            if merged.head != checked {
                return Err(NotMerged::refused(format!(
                    "the pull request {} was merged at {}, not at {checked}, the commit the check passed",
                    merged.url, merged.head
                )));
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
            return Err(NotMerged::refused(format!(
                "the branch {name} has no open pull request on GitHub: the check opens it, and the merge merges it"
            )));
        };
        if open.base != *main {
            return Err(NotMerged::refused(format!(
                "the pull request {} goes into {}, not into {main}",
                open.url, open.base
            )));
        }
        if open.head != checked {
            return Err(NotMerged::refused(format!(
                "the pull request {} is at {}, not at {checked}, the commit the check passed: the work is checked \
                 again before it merges",
                open.url, open.head
            )));
        }
        let now = self.main().await?;
        let holds = git::said(&self.project.repo, &["merge-base", "--is-ancestor", &now, checked]).await?;
        if !holds.ok {
            return Err(NotMerged::Moved {
                main: now,
                checked: checked.to_string(),
            });
        }
        (self.follow(open, branch).await).map_err(|failed| {
            failed.said(|why| format!("GitHub did not name the pull request {} as its merge: {why}", open.url))
        })?;
        if open.draft {
            (self.api.ready(&open.node).await).map_err(|failed| {
                failed.said(|why| format!("GitHub did not make the pull request {} ready: {why}", open.url))
            })?;
        }
        // Titled as GitHub titles a squash by default, with the pull request's number, whatever the method.
        let number = open.number.to_string();
        let title = fill(
            &self.config.words.merged,
            &[("name", &self.title(branch)), ("number", &number)],
        );
        let method = &self.config.merge_method;
        let commit = match self.api.merge(open.number, checked, method, &title).await {
            Ok(commit) => commit,
            // GitHub's own word that main moved as it was to merge.
            Err(failed) if failed.why.contains(BASE_MODIFIED) => {
                let main = self.main().await?;
                return Err(NotMerged::Moved {
                    main,
                    checked: checked.to_string(),
                });
            }
            Err(failed) => {
                let failed = failed.said(|why| format!("GitHub did not merge the pull request {}: {why}", open.url));
                return Err(NotMerged::Failed(failed));
            }
        };
        self.delete(branch).await?;
        Ok(commit)
    }

    fn kept_main(&self) -> String {
        kept_main(&self.config.remote, &self.project.main)
    }

    async fn run_tests(
        &self,
        branch: &Branch<'_>,
        named: &Named,
        asking: Asking,
        stop: &mut Stop,
    ) -> Result<Option<String>, String> {
        // What the check before the merge refuses, this refuses the same way: the repository's configuration and a
        // worktree off the work's branch.
        let place = branch.place;
        let words = (self.config.tool.as_ref())
            .and_then(|tool| tool.github.as_ref())
            .ok_or("the words of run_tests (agents/tools/run_tests.toml) have no [github]")?;
        let dir = git::worktree(place)?;
        git::configuration(&self.project, place).await?;
        git::on_branch(place).await?;
        // Only what is named is run: a request that names nothing is the agent's to ask again, and the full workflows
        // are the check before the merge's.
        if named.is_empty() {
            return Ok(Some(words.none_named.clone()));
        }
        let role = match self.role(self.config.ci.as_ref(), named)? {
            Ok(role) => role,
            Err(refused) => return Ok(Some(refused)),
        };
        let repo = &self.project.repo;
        let (commit, untracked) = match &named.commit {
            // The agent says which commit to run, so what the worktree holds besides it is of no concern.
            Some(name) => {
                let found = match self.named_commit(&dir, name).await? {
                    Ok(found) => found,
                    Err(why) => return Ok(Some(fill(&words.commit_unknown, &[("why", &why)]))),
                };
                (found, String::new())
            }
            None => {
                if git::merging(&dir).await? {
                    let changes = git::uncommitted(&dir).await?;
                    return Ok(Some(fill(&words.merge_under_way, &[("changes", &changes)])));
                }
                let changes = git::uncommitted(&dir).await?;
                if !changes.is_empty() {
                    return Ok(Some(fill(&words.uncommitted, &[("changes", &changes)])));
                }
                // What git does not track is in no commit, so it is not in the run: it is said with the result, where a
                // test an agent forgot to commit would otherwise be missing from it unseen.
                let untracked = git::untracked(&dir).await?;
                (git::git(repo, &["rev-parse", &place.reference()]).await?, untracked)
            }
        };
        // The commit is pushed to a branch of its own for this call, never to the work's branch, which the check before
        // the merge pushes: a push there would make GitHub run the full workflows on the pull request each time.
        let trying = trial_branch(branch, asking);
        self.push_to(&commit, &format!("refs/heads/{trying}"), false).await?;
        let request = fast::Request {
            work: branch.work,
            commit: &commit,
            asking,
        };
        let said = self.run_role(role, request, named, stop).await?;
        // The branch is kept until the run has ended and its result is read: a run the engine stopped waiting for is
        // found again by the same call, and checks the commit out when it starts. One that could not be deleted is
        // deleted as the work ends ([`Host::end`]).
        if said.is_some() {
            let _ = self.api.delete(&trying).await;
        }
        Ok(said.map(|said| match untracked.is_empty() {
            true => said,
            false => format!("{}\n\n{said}", fill(&words.untracked, &[("files", &untracked)])),
        }))
    }

    async fn end(&self, branch: &Branch<'_>, merged: bool) -> Result<(), String> {
        // The pull request is closed first, and the branches the calls left are swept whether or not that went well: they
        // are harmless leftovers, and what fails is said, not hidden by what comes before it.
        let closed = match merged {
            true => Ok(()),
            false => self.close_pulls(branch).await,
        };
        let swept = self.sweep_trials(branch).await;
        match (closed, swept) {
            (Err(closing), Err(sweeping)) => Err(format!("{closing}; {sweeping}")),
            (Err(why), Ok(())) | (Ok(()), Err(why)) => Err(why),
            (Ok(()), Ok(())) => Ok(()),
        }
    }

    fn pull_at(&self, number: u64) -> Option<String> {
        (self.project.github.as_ref()).map(|github| github.pull_at(number))
    }

    async fn reopen(&self, branch: &Branch<'_>) -> Result<(), String> {
        let pulls = self.api.pulls(&branch.place.branch).await?;
        if pulls.iter().any(|p| p.state == State::Open) {
            return Ok(());
        }
        // The newest of the work's pull requests into main, as GitHub lists them, the newest first.
        let latest = (pulls.iter()).find(|p| p.base == self.project.main);
        match latest {
            Some(closed) if closed.state == State::Closed => (self.api.reopen(closed.number).await)
                .map_err(|why| format!("GitHub did not open the pull request {} again: {why}", closed.url)),
            _ => Ok(()),
        }
    }
}

/// The pattern of git's that takes the name of every branch a call of `run_tests` of the work pushes a commit to, a place
/// no work's own branch is in: `wf-try/<the work's branch>/` and anything after it.
fn trial_pattern(branch: &Branch<'_>) -> String {
    format!("wf-try/{}/*", branch.place.branch)
}

/// The branch on GitHub that the commit a call of `run_tests` runs is pushed to, which the workflow checks out: one for
/// each call, so that calls asked together do not take each other's branch.
fn trial_branch(branch: &Branch<'_>, asking: Asking) -> String {
    format!("wf-try/{}/{}", branch.place.branch, asking.name())
}

/// The reference that holds main's head as GitHub last gave it, fetched there from the remote of this name, main of
/// this name.
pub(crate) fn kept_main(remote: &str, main: &str) -> String {
    format!("refs/remotes/{remote}/{main}")
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
pub(crate) fn neutral(text: &str, withheld: &[(String, String)], cut: &str) -> String {
    let mut text = text.to_string();
    for (path, said) in withheld {
        text = text.replace(path.as_str(), said.as_str());
    }
    if text.chars().count() > ABOUT {
        text = text.chars().take(ABOUT).collect();
        text.push('\n');
        text.push_str(cut);
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

/// How many characters of the name of a work's merge its pull request and its merge's commit take: GitHub refuses a
/// title of more than 256, and the name is one line.
const TITLE: usize = 200;

impl<A: Api> GitHub<A> {
    /// The title of the work's pull request: the name of its merge, made [`neutral`] as what the work writes is, cut
    /// to [`TITLE`] characters, saying so.
    fn title(&self, branch: &Branch<'_>) -> String {
        let title = neutral(branch.title.trim(), &self.config.withheld, &self.config.words.cut);
        if title.chars().count() <= TITLE {
            return title;
        }
        let mut cut: String = title.chars().take(TITLE - 1).collect();
        cut.push('…');
        cut
    }

    /// What the pull request of the work says of itself: what the work wrote of itself, as a block of plain text made
    /// [`neutral`], then what the pull request is. A person's edit of the body is written over at the next check.
    fn body(&self, branch: &Branch<'_>) -> String {
        let words = &self.config.words;
        let about = neutral(branch.about.trim(), &self.config.withheld, &words.cut);
        let work = branch.work.to_string();
        let what = fill(&words.body, &[("work", &work)]);
        if about.is_empty() {
            return what;
        }
        let fence = fence(&about);
        let block = format!("{fence}text\n{about}\n{fence}");
        format!("{}{what}", fill(&words.about, &[("about", &block)]))
    }
}

/// The paths nothing published names, each with what is said in its place: the longest first, so that one inside
/// another is said as the one it is in.
pub(crate) fn withheld(
    data: &Path,
    repo: &Path,
    home: Option<String>,
    said: &flow::agents::Withheld,
) -> Vec<(String, String)> {
    let mut withheld = vec![
        (data.display().to_string(), said.data.clone()),
        (repo.display().to_string(), said.repository.clone()),
    ];
    withheld.extend(home.filter(|home| home.len() > 1).map(|home| (home, said.home.clone())));
    withheld.sort_by_key(|(path, _)| std::cmp::Reverse(path.len()));
    withheld
}

/// A length of time in words: whole minutes from two minutes on, seconds below.
fn span(time: Duration) -> String {
    let seconds = time.as_secs();
    if seconds >= 120 && seconds.is_multiple_of(60) {
        format!("{} minutes", seconds / 60)
    } else if seconds == 1 {
        "1 second".to_string()
    } else {
        format!("{seconds} seconds")
    }
}
