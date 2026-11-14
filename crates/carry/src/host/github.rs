//! GitHub as the host: main is the repository's main branch on GitHub, fetched into the local repository; the verdict
//! on a commit is the project's full workflows run by hand on it, once, and judged by the JUnit they upload and by
//! their jobs' conclusions, never by the pull request's checks or a log; the work's pull request, which the check
//! pushes the commit to and opens where the branch has none, as a draft whose title and body it keeps to what the work
//! says of itself, is what is merged, by GitHub once it is made ready. A work that has ended has, where it did not
//! merge, its pull request closed.
//!
//! What it does is found again on GitHub by the work's branch and the commit, never kept by the engine: a check given
//! again after the engine started again pushes nothing GitHub has, finds the pull request it opened and runs the
//! workflows on the same commit again; a merge given again finds the pull request merged.
//!
//! An agent's `run_tests` is run on the last commit of its work's branch, pushed and put in the same pull request:
//! by the fast workflow where it names tests or checks, by the full ones where it names none (the module `fast`); what
//! the worktree holds that is not committed is said, and nothing is pushed.
//!
//! - `api`: what is asked of GitHub besides pushing and fetching, and `gh`, which asks it with the `gh` command line.
//! - `output`: what a test said, as a reader wants it.
//! - `fake`: GitHub for the tests, with a bare repository on disk to push to.

mod api;
mod fast;
mod gh;
mod output;
mod results;

#[cfg(test)]
mod fake;
#[cfg(test)]
mod tests;

use std::path::Path;

use flow::agents::fill;
use std::time::Duration;

pub use api::{Api, Pull, State};
pub use gh::{Gh, GhOffsite, UPLOAD_RATE};

use super::{Branch, Host, Named, Verdict};
use crate::git::{self, With};
use crate::held::Stop;
use crate::settings::{self, Project};

/// How often GitHub is asked how a run of a workflow goes: a run takes from half a minute to some minutes, and is waited
/// for while an agent or the check before a merge waits, so it is seen done within a few seconds, for a call a time.
pub const FAST_POLL: Duration = Duration::from_secs(5);

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
    /// How often GitHub is asked how a run of a workflow goes.
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
            .field("start", &self.start)
            .field("timeout", &self.timeout)
            .field("merge_method", &self.merge_method)
            .field("delete_branch", &self.delete_branch)
            .field("ci", &self.ci)
            .field("runners", &self.runners)
            .field("fast_poll", &self.fast_poll)
            .field("token", &self.token.as_ref().map(|_| "(given)"))
            .field("withheld", &self.withheld)
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
    /// GitHub as the installation's settings and the project's declaration say, reached with `gh` and the engine's
    /// token, which `env` gives from the engine's environment; `gh` keeps its configuration in the data directory, which is an absolute path. Says
    /// why where the token is not there.
    pub fn from_settings(
        github: &settings::GitHub,
        project: &Project,
        data: &Path,
        env: impl Fn(&str) -> Option<String>,
        ci: Option<flow::agents::Ci>,
        tool: Option<flow::agents::Tool>,
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
        self.pull_request(branch).await?;
        self.judge_full(commit, stop).await
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

    async fn run_tests(&self, branch: &Branch<'_>, named: &Named, stop: &mut Stop) -> Result<Option<String>, String> {
        // What the check before the merge refuses, this refuses the same way: the repository's configuration, a
        // worktree off the work's branch, and in the push, connections sent elsewhere and submodules.
        let place = branch.place;
        let words = (self.config.tool.as_ref())
            .and_then(|tool| tool.github.as_ref())
            .ok_or("the words of run_tests (agents/tools/run_tests.toml) have no [github]")?;
        let dir = git::worktree(place)?;
        git::configuration(&self.project, place).await?;
        git::on_branch(place).await?;
        if git::merging(&dir).await? {
            let changes = git::uncommitted(&dir).await?;
            return Ok(Some(fill(&words.merge_under_way, &[("changes", &changes)])));
        }
        let changes = git::uncommitted(&dir).await?;
        if !changes.is_empty() {
            return Ok(Some(fill(&words.uncommitted, &[("changes", &changes)])));
        }
        let ci = self.config.ci.as_ref();
        let role = match self.role(ci, named)? {
            Ok(role) => role,
            Err(refused) => return Ok(Some(refused)),
        };
        let repo = &self.project.repo;
        let commit = git::git(repo, &["rev-parse", &place.reference()]).await?;
        // GitHub opens no pull request for a branch with nothing of its own.
        let main = self.kept_main();
        if git::said(repo, &["merge-base", "--is-ancestor", &commit, &main])
            .await?
            .ok
        {
            return Ok(Some(fill(&words.no_own_commit, &[("main", &self.project.main)])));
        }
        self.push(branch, &commit).await?;
        self.pull_request(branch).await?;
        self.run_role(role, &commit, named, stop).await
    }

    async fn end(&self, branch: &Branch<'_>, merged: bool) -> Result<(), String> {
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

/// A length of time in words: whole minutes from two minutes on, seconds below.
fn span(time: Duration) -> String {
    let seconds = time.as_secs();
    if seconds >= 120 && seconds.is_multiple_of(60) {
        format!("{} minutes", seconds / 60)
    } else {
        format!("{seconds} seconds")
    }
}
