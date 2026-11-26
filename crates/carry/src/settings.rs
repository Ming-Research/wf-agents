//! The installation's settings: the owner, the projects the works are done in, each by its name with its repository
//! and where it is hosted, and how the agents are run. They are read from a TOML file, `install.toml` in the data
//! directory unless another is named; `install.sample.toml` beside this crate's manifest says what each key is. A key
//! the file does not know is refused, as the flow's files refuse one, so that a misspelt key is not silently left at
//! nothing.
//!
//! A text the agents are given names a setting by its key between backticks, as `` `owner.language` ``, and is given
//! with the setting's value written beside the name: `agents/installation.toml` beside the flows names the
//! installation's settings the texts may name, and the installation holds each one's value ([`Settings::value`]).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use driver::{Budget, Policy, Warm};
use flow::CellSpec;
use serde::{Deserialize, Serialize};
use service::{Levels, LevelsFault};
use work::RanOn;

/// The settings as their file is written.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    /// The owner, for whom the agents write what the owner reads.
    pub owner: Owner,
    /// The projects the works are done in, by the name the installation gives each: an item belongs to one of them,
    /// and its works run in that project's repository. Left out, there is none, which is refused with words.
    #[serde(default)]
    pub projects: BTreeMap<String, Project>,
    pub agents: Agents,
    pub items: Items,
    /// Where the copies of the database are kept off the machine, and how many are kept; with none, the copies stay
    /// on the machine alone, which shares the disk they are kept against losing.
    pub backup: Option<Backup>,
    /// How a line's session is kept: its program's prompt cache, and the pings that keep it warm while the line waits.
    #[serde(default)]
    pub sessions: Sessions,
    /// When `wf-agents wait` gives again what stands, and how long work may be held up before that is something to
    /// handle.
    #[serde(default)]
    pub wait: Wait,
}

/// The owner of the installation.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Owner {
    /// The language the agents write in what the owner reads.
    pub language: String,
}

/// The copies of the database kept off the machine: in a private repository on GitHub, as assets of a release.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Backup {
    /// The private repository on github.com, as `owner/name`. It is not the project's repository nor the data
    /// directory's remote: the database holds everything said to every agent.
    pub repository: String,
    /// The variable of the engine's environment that holds the token for this repository, which is not the one the
    /// project's repository has.
    #[serde(default = "backup_token_env")]
    pub token_env: String,
    /// How many copies are kept, the newest, on the machine and off it.
    #[serde(default = "backup_keep")]
    pub keep: usize,
    /// The `gh` command line, or a path to it.
    #[serde(default = "gh")]
    pub gh: String,
}

fn backup_token_env() -> String {
    "WF_BACKUP_TOKEN".to_string()
}

/// How many copies are kept where the settings do not say.
pub const KEEP: usize = 7;

fn backup_keep() -> usize {
    KEEP
}

impl Backup {
    /// What is wrong with these settings, each in words with its key.
    fn wrong(&self, agents: &Agents, projects: &BTreeMap<String, Project>) -> Vec<String> {
        let mut wrong = Vec::new();
        let parts: Vec<&str> = self.repository.split('/').collect();
        if !(parts.len() == 2 && parts.iter().all(|p| !p.is_empty())) {
            wrong.push(format!(
                "backup.repository: a repository is named as owner/name, not {:?}",
                self.repository
            ));
        }
        if self.keep == 0 {
            wrong.push("backup.keep: at least one copy is kept".to_string());
        }
        if self.token_env.is_empty() {
            wrong.push("backup.token_env: the token's variable has a name".to_string());
        }
        if agents.env.contains(&self.token_env) {
            wrong.push(format!(
                "agents.env: {} holds the engine's token for the copies, which no agent is given",
                self.token_env
            ));
        }
        for (name, project) in projects {
            if project.github.as_ref().is_some_and(|g| g.token_env == self.token_env) {
                wrong.push(format!(
                    "backup.token_env: the copies have a token of their own, not the one of the repository of the \
                     project {name}"
                ));
            }
        }
        wrong
    }
}

/// How a line's session is kept between its rounds. The agents' program compacts a session at `compact_at`, so that
/// its line goes on in it; a session on another model is replaced at the line's next round, and one under another
/// account goes on (decision a-line-goes-on-in-its-session-under-another-account). Whether the sessions of works that
/// wait are kept warm is no setting: it is the engine's own state, which the owner switches.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Sessions {
    /// The context at which the agents' program compacts a line's session, in tokens, given it on every round's
    /// command line; no line drawn from the line's history lowers it, and the engine begins no new session for a
    /// session's size (decision `a-session-is-compacted-at-the-installations-number`). Between 100,000 and 1,000,000,
    /// the range the program takes.
    #[serde(default = "compact_at")]
    pub compact_at: u64,
    /// How long the agents' program keeps a session in its prompt cache after its last call, in minutes: an hour for
    /// `claude`, whose usage says every write to the cache is one kept an hour (`ephemeral_1h_input_tokens`); 0 for a
    /// program that keeps none, whose sessions are not kept warm. How many keep-warm pings a wait is worth is not set
    /// here: it follows from the prices of the session's model, as the work's own reports of its calls tell them.
    #[serde(default = "cache_minutes")]
    pub cache_minutes: u64,
    /// How long after a session's last call a line that waits has it sent a keep-warm ping, in minutes: five before the
    /// cache lets it go, time for the ping and four tries again a minute apart.
    #[serde(default = "warm_minutes")]
    pub warm_minutes: u64,
}

impl Default for Sessions {
    fn default() -> Self {
        Sessions {
            compact_at: compact_at(),
            cache_minutes: cache_minutes(),
            warm_minutes: warm_minutes(),
        }
    }
}

/// The least and the most context the agents' program compacts a session at, as its own help gives the range of
/// `--autocompact` (`claude` 2.1.326: "Auto-compact window size (auto, or 100k–1M tokens)").
const COMPACTS_AT_LEAST: u64 = 100_000;
const COMPACTS_AT_MOST: u64 = 1_000_000;

fn compact_at() -> u64 {
    500_000
}

fn cache_minutes() -> u64 {
    60
}

fn warm_minutes() -> u64 {
    55
}

/// A minute, in milliseconds.
const MINUTE: u64 = 60_000;

/// What `wf-agents wait` goes by, in minutes: how long after it gave what stands to be handled it gives it whole again,
/// until each thing is gone; how long a work may hold its project's turn to check and merge, its round or call under
/// way, while other works wait for the turn, before that is something to handle; how long a round or call may wait for
/// a runner before that is something to handle; and how long a measurement may wait for its turn on its machine before
/// that is something to handle.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Wait {
    #[serde(default = "again_minutes")]
    pub again_minutes: u64,
    #[serde(default = "turn_minutes")]
    pub turn_minutes: u64,
    #[serde(default = "queued_minutes")]
    pub queued_minutes: u64,
    #[serde(default = "machine_minutes")]
    pub machine_minutes: u64,
}

impl Default for Wait {
    fn default() -> Self {
        Wait {
            again_minutes: again_minutes(),
            turn_minutes: turn_minutes(),
            queued_minutes: queued_minutes(),
            machine_minutes: machine_minutes(),
        }
    }
}

fn again_minutes() -> u64 {
    5
}

fn turn_minutes() -> u64 {
    30
}

fn queued_minutes() -> u64 {
    15
}

fn machine_minutes() -> u64 {
    60
}

impl Wait {
    /// How long after what stands was given whole it is given again, how long a turn may be held so, how long a round
    /// may wait for a runner, and how long a measurement may wait for its machine, in milliseconds.
    pub fn ms(&self) -> (u64, u64, u64, u64) {
        let ms = |minutes: u64| minutes.saturating_mul(MINUTE);
        (
            ms(self.again_minutes),
            ms(self.turn_minutes),
            ms(self.queued_minutes),
            ms(self.machine_minutes),
        )
    }
}

/// A project whose works are done, as this machine has it: a local git repository, its main branch, and where it is
/// hosted. What is the project's own, such as the command that checks it, is in its declaration.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    /// The repository's own checkout: works branch from its main branch, and are merged into it here.
    pub repo: PathBuf,
    /// The name of the main branch.
    pub main: String,
    /// The keys of the repository's own configuration that could make git run a program and that the acts are to let
    /// be, each a whole key or a beginning followed by `*`: `filter.lfs.*` for a repository that needs Git LFS.
    #[serde(default)]
    pub allowed_config: Vec<String>,
    /// Where the repository is on GitHub, which then carries out the acts of the project's works; with none, the local
    /// repository does.
    pub github: Option<GitHub>,
    /// Who the engine's commits are by, and the words of each: not in the settings, but in `agents/commits.toml`
    /// beside the flows, put here as the carriers are made ([`crate::carry`]).
    #[serde(skip)]
    pub commits: flow::agents::Commits,
    /// The directory of the repository that holds the project's design tree: not in the settings, but in the
    /// project's declaration, `wf-agents.toml` on its main, put here as the carriers are made.
    #[serde(skip)]
    pub tree: String,
}

/// The project's repository on GitHub: the works' branches are pushed there, checked by its CI, which the engine runs
/// by hand on the commit to merge, and merged by GitHub through a pull request each.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GitHub {
    /// The repository on github.com, as `owner/name`; the project's `repo` is a clone of it.
    pub repository: String,
    /// The clone's name for the repository on GitHub: main's head is fetched to `refs/remotes/<remote>/<main>`.
    #[serde(default = "origin")]
    pub remote: String,
    /// The variable of the engine's environment that holds the engine's token for GitHub.
    #[serde(default = "token_env")]
    pub token_env: String,
    /// How many minutes a run of a workflow may be unlisted, or wait for a runner (the run not taken, or the jobs of a
    /// started run not taken), before the engine asks whether a runner of the workflow's kind is online; it asks again
    /// after each further such time while one is. A run unlisted so long is taken never to have started. A run that
    /// waits where none is online, or where that cannot be told, is cancelled where no job of it has started, and the
    /// check fails its round in passing, or the agents' tool says so; one a job of which has started is not, and its
    /// wait counts in the time it may take until one is online. GitHub's own machines count as online.
    #[serde(default = "start_minutes")]
    pub start_minutes: u64,
    /// The `gh` command line, or a path to it.
    #[serde(default = "gh")]
    pub gh: String,
    /// The runners the repository has, by the name a workflow's `runner` in the project's declaration gives them:
    /// `github` for GitHub's own machines, and the label of each self-hosted runner registered for it. A workflow that
    /// runs on another is not run.
    #[serde(default = "runners")]
    pub runners: Vec<String>,
}

fn runners() -> Vec<String> {
    vec!["github".to_string()]
}

fn origin() -> String {
    "origin".to_string()
}

fn token_env() -> String {
    "WF_GITHUB_TOKEN".to_string()
}

fn start_minutes() -> u64 {
    10
}

fn gh() -> String {
    "gh".to_string()
}

/// How the agents are run.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Agents {
    /// The command line program that runs an agent's round: `claude`, or a path to it.
    pub command: String,
    /// The levels of work, by name: an item's level chooses the model its agents' rounds are run with.
    pub levels: BTreeMap<String, Level>,
    /// The highest level one round may run at, by its name among the levels: a round of a line that runs above its
    /// item's level goes no higher. It is no lower than `items.highest_level`, so that a round of an item at its
    /// highest level can run at it; with none, the top level.
    pub highest_level: Option<String>,
    /// How many agents' rounds run at once.
    pub slots: usize,
    /// The names of the variables of the engine's environment the program is given beside those it is always given
    /// (`PATH`, `HOME`, `USER`, `LANG` and `LC_*`, `TMPDIR`, `TERM`, `SHELL`). The carrier's own variables are refused,
    /// and so is a credential the program takes above an account's token, an API key among them: the program runs
    /// under no credential but the owner's accounts.
    #[serde(default)]
    pub env: Vec<String>,
    /// The subagents an agent may start within its round; given where a text names `agents.subagents.most`.
    #[serde(default)]
    pub subagents: Subagents,
    /// What a work's rounds may spend in all, in dollars, before the work waits for the owner; with none, no limit.
    pub work_budget_usd: Option<f64>,
    /// What each letting-on of a work the engine paused for its budget allows it beyond what it had spent by then, in
    /// dollars, so that a work let on has room to go on and does not wait for the owner again at its next round.
    #[serde(default = "let_on_allowance")]
    pub let_on_allowance_usd: f64,
}

/// What a letting-on allows a work beyond what it had spent where the settings name no number, in dollars.
fn let_on_allowance() -> f64 {
    20.0
}

/// The subagents an agent may start within its round for chores, such as reading documents or searching code, which
/// the agent is told.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Subagents {
    /// How many at most in one round; 3 where none is given.
    #[serde(default = "most_subagents")]
    pub most: u32,
}

impl Default for Subagents {
    fn default() -> Subagents {
        Subagents { most: most_subagents() }
    }
}

fn most_subagents() -> u32 {
    3
}

impl Agents {
    /// What is wrong with these settings of the agents, each in words with its key: a variable of the program's
    /// environment that is the carrier's own, or a credential the program takes above an account's token.
    fn wrong(&self) -> Vec<String> {
        let mut wrong = Vec::new();
        // The carrier's own variables are given by the carrier alone, and a credential the program takes above the
        // token by none (decision no-default-credential).
        let own = [crate::account::TOKEN, crate::account::VARIABLE, crate::cli::SCRUB];
        for name in self.env.iter().filter(|name| own.contains(&name.as_str())) {
            wrong.push(format!(
                "agents.env: {name} is the carrier's own, which it gives the program itself"
            ));
        }
        for name in (self.env.iter()).filter(|name| crate::account::ABOVE_TOKEN.contains(&name.as_str())) {
            wrong.push(format!(
                "agents.env: {name} gives the agents' program a credential it takes above an account's token, and it \
                 runs under no credential but the owner's accounts: take it out, and add the account on the Settings \
                 view"
            ));
        }
        // What every agent is refused, and the heavy commands of each project's declaration, are checked as the
        // engine starts, beside the flows and the declarations it reads (`permissions::faults`,
        // `permissions::heavy_faults`).
        wrong
    }
}

/// A level of work: what work it is for, and the model its rounds are run with.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Level {
    /// What work the level is for, in words for whoever gives an item its level.
    pub desc: String,
    /// The model, as the command line's `--model` takes it.
    pub model: String,
}

/// How items are given their level.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Items {
    /// The level an item is added at where it is given none, by its name among the levels.
    pub default_level: String,
    /// The highest level an item may be at, by its name among the levels: an item above it is refused as it is added;
    /// with none, the top level. A round's own highest is `agents.highest_level`.
    pub highest_level: Option<String>,
    /// The most words an item's title may have, as `service::title_words` counts them: a Chinese character counts as a
    /// word. A title is held to it when it is made or changed; a title already there is not counted again. There is no
    /// default: the installation names the number.
    pub title_max: u32,
}

/// What the settings are told where they name `github.runs_on_before_branches`, which the engine no longer reads.
const BEFORE_BRANCHES: &str = "github.runs_on_before_branches: this setting was removed, as the engine no longer pushes \
     the code before a change to branches of its own; delete the line";

/// What the settings are told where they name `github.fast`, which the project declares now.
const FAST_MOVED: &str = "github.fast: the fast workflow is the project's, not the installation's: it is declared in the \
     project's wf-agents.toml as [ci.fast] (workflow, runner, the inputs, results, and `checks` for named_checks), \
     and its separators are gone, for a test's name is the one its results give it; delete the table, and name in \
     github.runners the runners this installation has";

/// What the settings are told where they name `github.log`, which the engine no longer reads.
const LOG_GONE: &str = "github.log: the engine reads no log by a project's patterns: a test's result and what it said \
     come from the JUnit its workflow uploads, and a job that failed with no test of its own to say why is given the end \
     of its failed step, read in the same way for every project; delete the table";

/// What the settings are told where they name `github.checks`, which the project's full workflows are now.
const CHECKS_MOVED: &str = "github.checks: the checks that count are the jobs of the project's full workflows, declared in \
     the project's wf-agents.toml as [ci.full] (workflows, runner, too_long), which the check before a merge runs \
     on the commit to merge; delete the line";

/// What the settings are told where they hold `[project]` or `[github]`, the tables of the one project an installation
/// once had, `{key}` the one they hold.
const ONE_PROJECT: &str = "{key}: an installation names each of its projects now, and [{key}] is a project's: write \
     [project] as [projects.<name>] and [github] as [projects.<name>.github], <name> the name the project is known \
     by; install.sample.toml shows them.";

/// What the settings are told where they name `agents.model`, the one model every round once ran on.
const ONE_MODEL: &str = "agents.model: rounds no longer run on one model: each item has a level, and each level names \
     its model. Take out agents.model; add a table [agents.levels.<name>] with desc and model for each level, and a \
     table [items] with default_level, the level an item is added at where it is given none, and, if you like, \
     highest_level. install.sample.toml shows them.";

/// What the settings are told where they name `sessions.keep_warm` or `sessions.warm_while_paused`, the keys that once
/// said whether the sessions of works that wait are kept warm, `{key}` the one they hold.
const KEEPING_WARM: &str = "sessions.{key}: whether the sessions of works that wait are kept warm is the engine's own \
     state now, which the owner switches on their page and which takes effect at once, not a setting read as the \
     engine starts: take the key out of [sessions].";

/// What the settings are told where they name `sessions.context_limit`, where the context at which a session is
/// compacted once was.
const CONTEXT_LIMIT: &str = "sessions.context_limit: the number is the context at which the agents' program compacts a \
     line's session, and nothing is limited by it: write it as compact_at in [sessions].";

/// What the settings are told where they name `agents.owner_language`, where the owner's language once was.
const OWNER_LANGUAGE: &str = "agents.owner_language: the owner's language is the owner's, not the agents': take it \
     out of [agents] and write it as language in a table [owner], as install.sample.toml shows.";

/// What the settings are told where they name `agents.config_dir`, where the program's directory of settings and
/// sessions once was named.
const CONFIG_DIR_MOVED: &str = "agents.config_dir: the engine keeps the agents' program's directory of settings and \
     sessions itself, at profile/ in the data directory, and gives it as CLAUDE_CONFIG_DIR: move the directory there, \
     so that the works' sessions go on, and take agents.config_dir out of [agents].";

/// The settings file's name in the data directory.
pub const FILE: &str = "install.toml";

impl GitHub {
    /// What is wrong with these settings of the project of this name, each in words with its key.
    fn wrong(&self, project: &str, agents: &Agents) -> Vec<String> {
        let mut wrong = Vec::new();
        let parts: Vec<&str> = self.repository.split('/').collect();
        if !(parts.len() == 2 && parts.iter().all(|p| !p.is_empty())) {
            wrong.push(format!(
                "projects.{project}.github.repository: a repository is named as owner/name, not {:?}",
                self.repository
            ));
        }
        if self.start_minutes == 0 {
            wrong.push(format!(
                "projects.{project}.github.start_minutes: the CI is given at least a minute to start"
            ));
        }
        if self.runners.iter().any(|r| r.trim().is_empty()) {
            wrong.push(format!("projects.{project}.github.runners: each runner is named"));
        }
        if self.token_env.is_empty() {
            wrong.push(format!(
                "projects.{project}.github.token_env: the token's variable has a name"
            ));
        }
        if agents.env.contains(&self.token_env) {
            wrong.push(format!(
                "agents.env: {} holds the engine's token for GitHub, which no agent is given",
                self.token_env
            ));
        }
        wrong
    }

    /// The repository's address, which git pushes to and fetches from.
    pub fn url(&self) -> String {
        format!("https://github.com/{}.git", self.repository)
    }

    /// The address where a person sees the repository's pull request of this number.
    pub fn pull_at(&self, number: u64) -> String {
        format!("https://github.com/{}/pull/{number}", self.repository)
    }
}

impl Settings {
    /// Reads the settings from the text of their file, or says everything wrong with it.
    pub fn read(text: &str) -> Result<Settings, String> {
        // Settings written when every round ran on one model would be refused for that key, then for each table they
        // lack: what replaces it is said at once.
        let table: Option<toml::Table> = toml::from_str(text).ok();
        let agents = table.as_ref().and_then(|t| t.get("agents")).and_then(|a| a.as_table());
        if agents.is_some_and(|agents| agents.contains_key("model")) {
            return Err(ONE_MODEL.to_string());
        }
        if agents.is_some_and(|agents| agents.contains_key("owner_language")) {
            return Err(OWNER_LANGUAGE.to_string());
        }
        if agents.is_some_and(|agents| agents.contains_key("config_dir")) {
            return Err(CONFIG_DIR_MOVED.to_string());
        }
        let sessions = table
            .as_ref()
            .and_then(|t| t.get("sessions"))
            .and_then(|s| s.as_table());
        let warm: Vec<&str> = (["keep_warm", "warm_while_paused"].into_iter())
            .filter(|key| sessions.is_some_and(|sessions| sessions.contains_key(*key)))
            .collect();
        if let Some(key) = warm.first() {
            return Err(KEEPING_WARM.replace("{key}", key));
        }
        if sessions.is_some_and(|sessions| sessions.contains_key("context_limit")) {
            return Err(CONTEXT_LIMIT.to_string());
        }
        // Settings of the one project an installation once had are told where its tables went.
        let moved: Vec<&str> = (["project", "github"].into_iter())
            .filter(|key| table.as_ref().is_some_and(|t| t.contains_key(*key)))
            .collect();
        if !moved.is_empty() {
            return Err(moved
                .iter()
                .map(|key| ONE_PROJECT.replace("{key}", key))
                .collect::<Vec<_>>()
                .join("\n"));
        }
        let projects = table
            .as_ref()
            .and_then(|t| t.get("projects"))
            .and_then(|p| p.as_table());
        for (name, project) in projects.into_iter().flatten() {
            let github = project.get("github").and_then(|g| g.as_table());
            for (key, words) in [
                ("runs_on_before_branches", BEFORE_BRANCHES),
                ("fast", FAST_MOVED),
                ("log", LOG_GONE),
                ("checks", CHECKS_MOVED),
            ] {
                if github.is_some_and(|github| github.contains_key(key)) {
                    return Err(format!("projects.{name}.{words}"));
                }
            }
        }
        let settings: Settings = toml::from_str(text).map_err(|e| e.to_string().trim_end().to_string())?;
        let mut wrong = Vec::new();
        wrong.extend(settings.agents.wrong());
        if settings.agents.slots == 0 {
            wrong.push("agents.slots: at least one round must be able to run".to_string());
        }
        if settings
            .agents
            .work_budget_usd
            .is_some_and(|usd| !(usd.is_finite() && usd >= 0.0))
        {
            wrong.push("agents.work_budget_usd: a budget is a number of dollars, none below nothing".to_string());
        }
        let allowance = settings.agents.let_on_allowance_usd;
        if !(allowance.is_finite() && allowance >= 0.0) {
            wrong.push(
                "agents.let_on_allowance_usd: what a letting-on allows is a number of dollars, none below nothing"
                    .to_string(),
            );
        }
        let sessions = &settings.sessions;
        if !(COMPACTS_AT_LEAST..=COMPACTS_AT_MOST).contains(&sessions.compact_at) {
            wrong.push(format!(
                "sessions.compact_at: the agents' program compacts a session within {COMPACTS_AT_LEAST} and \
                 {COMPACTS_AT_MOST} tokens"
            ));
        }
        if sessions.cache_minutes > 0 && sessions.warm_minutes >= sessions.cache_minutes {
            wrong.push(
                "sessions.warm_minutes: a session is pinged before the cache lets it go, within sessions.cache_minutes"
                    .to_string(),
            );
        }
        if settings.wait.again_minutes == 0 {
            wrong.push("wait.again_minutes: what stands is given again at least a minute after it was".to_string());
        }
        if settings.projects.is_empty() {
            wrong.push(
                "projects: the installation names at least one project, as a table [projects.<name>]".to_string(),
            );
        }
        for (name, project) in &settings.projects {
            if name.trim() != name || name.is_empty() || name.chars().any(char::is_control) {
                wrong.push(format!(
                    "projects.{name:?}: a project's name is some text, with no space around it and no control \
                     character"
                ));
            }
            if project.main.is_empty() {
                wrong.push(format!("projects.{name}.main: the main branch has a name"));
            }
            if let Some(github) = &project.github {
                wrong.extend(github.wrong(name, &settings.agents));
            }
        }
        if settings.owner.language.trim().is_empty() {
            wrong.push("owner.language: the owner's language has a name".to_string());
        }
        if let Err(why) = settings.levels() {
            wrong.push(why);
        }
        for (name, level) in &settings.agents.levels {
            if !crate::cli::knows_model(&level.model) {
                wrong.push(format!(
                    "agents.levels.{name}.model: the command line takes no model named {:?}",
                    level.model
                ));
            }
        }
        if let Some(backup) = &settings.backup {
            wrong.extend(backup.wrong(&settings.agents, &settings.projects));
        }
        if wrong.is_empty() {
            Ok(settings)
        } else {
            Err(wrong.join("\n"))
        }
    }

    /// Reads the settings from their file.
    pub fn load(path: &Path) -> Result<Settings, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Settings::read(&text).map_err(|why| format!("{}: {why}", path.display()))
    }

    /// The levels items are given, as these settings name them; or what is wrong with them, in words with their keys.
    pub fn levels(&self) -> Result<Levels, String> {
        let items = &self.items;
        let levels = (self.agents.levels.iter()).map(|(name, level)| service::Level {
            name: name.clone(),
            desc: level.desc.clone(),
        });
        let made = Levels::new(
            levels,
            &items.default_level,
            items.highest_level.as_deref(),
            self.agents.highest_level.as_deref(),
        );
        made.map(|levels| levels.with_title_max(items.title_max as usize))
            .map_err(|fault| match fault {
                LevelsFault::None => "agents.levels: there is at least one level".to_string(),
                LevelsFault::NoSuchDefault(name) => format!("items.default_level: there is no level {name:?}"),
                LevelsFault::NoSuchItemsHighest(name) => format!("items.highest_level: there is no level {name:?}"),
                LevelsFault::NoSuchRoundsHighest(name) => format!("agents.highest_level: there is no level {name:?}"),
                LevelsFault::DefaultAboveItemsHighest { default, highest } => {
                    format!(
                        "items.default_level: {default:?} is above {highest:?}, the highest level an item may be at"
                    )
                }
                LevelsFault::ItemsHighestAboveRounds { items, rounds } => format!(
                    "items.highest_level: {items:?} is above {rounds:?}, the highest level one round may run at \
                     (agents.highest_level): an item at it could not run at its own level"
                ),
            })
    }

    /// What a round of this cell, for an item at this level, runs on, under the account it was handed out with: the
    /// level of its item, or above it as far as the cell's line says, with that level's model and the cell's effort,
    /// the program's directory of settings and sessions, `profile`, the engine's, and the account by its name; or why
    /// it cannot run, in words. An item added before items had levels is at the default.
    pub fn ran_on(
        &self,
        item_level: Option<&str>,
        cell: &CellSpec,
        profile: &Path,
        account: Option<&str>,
    ) -> Result<RanOn, String> {
        let (level, model) = self.model_of(item_level, cell)?;
        Ok(RanOn {
            level,
            model,
            effort: cell.effort.map(|effort| effort.name().to_string()),
            directory: Some(profile.to_string_lossy().into_owned()),
            account: account.map(str::to_string),
        })
    }

    /// The level a round of this cell, for an item at this level, runs at, and that level's model; or why it cannot
    /// run, in words.
    fn model_of(&self, item_level: Option<&str>, cell: &CellSpec) -> Result<(String, String), String> {
        let level = self.levels()?.of_round(item_level, cell.levels_above)?.level;
        let model = (self.agents.levels.get(&level))
            .map(|level| level.model.clone())
            .ok_or_else(|| format!("agents.levels: there is no level {level:?}"))?;
        Ok((level, model))
    }

    /// When a line begins a new session, where the model its session's rounds ran on is not the one its rounds run on
    /// now; and how its sessions are kept warm while it waits. A session past the installation's context is compacted
    /// by the agents' program, not replaced, so the number is no part of the policy. Whether a new session would begin
    /// with another system prompt or form is what the agents that keep the sessions say ([`crate::Agents`]): none here.
    pub fn policy(&self) -> Policy {
        let settings = self.clone();
        let sessions = &self.sessions;
        let warm = self.cache_ms().map(|life_ms| Warm {
            life_ms,
            after_ms: sessions.warm_minutes.saturating_mul(MINUTE),
            unmeasured_pings: driver::prices::UNMEASURED_PINGS,
            budget: self.budget(),
        });
        Policy {
            samples: driver::HISTORY_SAMPLES,
            model: Box::new(move |item, cell| {
                settings
                    .model_of(item.level.as_deref(), cell)
                    .ok()
                    .map(|(_, model)| model)
            }),
            changed: None,
            warm,
        }
    }

    /// How long the agents' program keeps a session in its prompt cache after its last call, in milliseconds; none
    /// where it keeps none.
    pub fn cache_ms(&self) -> Option<u64> {
        (self.sessions.cache_minutes > 0).then(|| self.sessions.cache_minutes.saturating_mul(MINUTE))
    }

    /// The value of the setting this key names, its parts joined by dots, as an agent is given it beside the key: a
    /// text as it is, any other value as JSON writes it; none where these settings have none, as a key they do not
    /// have, a value left out or a table left out. `owner.language` gives `Chinese`; `github.checks`, `["check"]`.
    pub fn value(&self, key: &str) -> Option<String> {
        let all = serde_json::to_value(self).ok()?;
        let mut value = &all;
        for part in key.split('.') {
            value = value.get(part)?;
        }
        match value {
            serde_json::Value::Null => None,
            serde_json::Value::String(text) => Some(text.clone()),
            other => Some(other.to_string()),
        }
    }

    /// The project of this name, where these settings name one so.
    pub fn project(&self, name: &str) -> Option<&Project> {
        self.projects.get(name)
    }

    /// The most a work may spend, where the installation gives a work a budget: that budget and what each letting-on
    /// of a work the engine paused for it allows beyond it, in millionths of a dollar ([`Budget::bar`]).
    pub fn budget(&self) -> Option<Budget> {
        // Each was checked to be a finite number of no less than nothing; a larger one than a u64 holds is none.
        let micros = |usd: f64| (usd * 1_000_000.0).round() as u64;
        let work = (self.agents.work_budget_usd).map(micros)?;
        Some(Budget {
            work,
            let_on: micros(self.agents.let_on_allowance_usd),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = include_str!("../install.sample.toml");

    #[test]
    fn the_sample_reads() {
        let settings = Settings::read(SAMPLE).expect("the sample reads");
        assert_eq!(settings.projects.keys().collect::<Vec<_>>(), ["example"]);
        assert_eq!(settings.projects["example"].main, "main");
        assert_eq!(settings.agents.command, "claude");
        assert_eq!(settings.agents.slots, 2);
        let budget = settings.budget().expect("a budget");
        assert_eq!((budget.work, budget.let_on), (5_000_000, 2_000_000));
        assert!(settings.agents.env.is_empty());
        assert_eq!(settings.sessions, Sessions::default());
        assert_eq!(
            settings.policy().samples,
            19,
            "the history needs 19 values, whatever the file says"
        );
    }

    // Whatever model a level names, a wait of its session is kept warm as often as the work's own reports of the model
    // make worth (crate `driver`, module `prices`), and before they tell it as often as the dearest read makes worth,
    // never not at all. Fails where the pings a wait is worth come from a table of models by their names, as when
    // Fable 5.2, which the table did not name, was never pinged.
    #[test]
    fn a_session_is_kept_warm_from_55_minutes_after_its_last_call_to_the_hour_as_often_as_its_models_prices_make_worth()
    {
        let without = SAMPLE
            .replace("cache_minutes = 60\n", "")
            .replace("warm_minutes = 55\n", "");
        assert_ne!(without, SAMPLE, "the sample sets them");
        let settings = Settings::read(&without).expect("it reads without");
        let warm = settings.policy().warm.expect("kept warm");
        let budget = Budget {
            work: 5_000_000,
            let_on: 2_000_000,
        };
        assert_eq!(
            (warm.life_ms, warm.after_ms, warm.budget),
            (3_600_000, 3_300_000, Some(budget))
        );
        assert_eq!(warm.unmeasured_pings, driver::prices::UNMEASURED_PINGS);
        assert_ne!(
            warm.unmeasured_pings, 0,
            "a session on a model not yet reported is kept warm"
        );
        let none = Settings::read(&SAMPLE.replace("cache_minutes = 60", "cache_minutes = 0")).unwrap();
        assert_eq!(none.policy().warm, None, "a program that keeps no cache");
        let late = SAMPLE.replace("warm_minutes = 55", "warm_minutes = 60");
        assert!(
            Settings::read(&late)
                .expect_err("refused")
                .contains("sessions.warm_minutes")
        );
    }

    // What `wf-agents wait` goes by is read from its table, each time left out at its default, and a time of nought
    // between two givings of what stands is refused, by its key. Fails where a default is not the one the sample says,
    // a key is not read, or a key the table does not know is.
    #[test]
    fn what_wait_goes_by_is_read_each_left_out_at_its_default_and_no_time_between_givings_is_refused() {
        let settings = Settings::read(SAMPLE).expect("the sample reads");
        assert_eq!(settings.wait.ms(), (5 * MINUTE, 30 * MINUTE, 15 * MINUTE, 60 * MINUTE));
        let without = (SAMPLE.replace("again_minutes = 5\n", ""))
            .replace("turn_minutes = 30\n", "")
            .replace("queued_minutes = 15\n", "")
            .replace("machine_minutes = 60\n", "");
        assert_ne!(without, SAMPLE, "the sample sets them");
        assert_eq!(
            Settings::read(&without).expect("it reads without").wait,
            Wait::default()
        );
        assert_eq!(Wait::default(), settings.wait, "the sample sets each at its default");
        let set = (SAMPLE.replace("again_minutes = 5", "again_minutes = 2"))
            .replace("queued_minutes = 15", "queued_minutes = 40")
            .replace("machine_minutes = 60", "machine_minutes = 90");
        let read = Settings::read(&set).expect("it reads");
        assert_eq!(read.wait.ms(), (2 * MINUTE, 30 * MINUTE, 40 * MINUTE, 90 * MINUTE));
        let never = SAMPLE.replace("again_minutes = 5", "again_minutes = 0");
        assert!(
            Settings::read(&never)
                .expect_err("refused")
                .contains("wait.again_minutes")
        );
        let unknown = SAMPLE.replace("again_minutes = 5", "again_seconds = 30");
        assert!(
            Settings::read(&unknown).is_err(),
            "a key the table does not know is read"
        );
    }

    #[test]
    fn a_session_is_compacted_at_500000_tokens_where_none_is_set_and_only_what_the_program_takes_is_read() {
        let of = |at: &str| SAMPLE.replace("compact_at = 500000", &format!("compact_at = {at}"));
        let without = SAMPLE.replace("compact_at = 500000\n", "");
        assert_ne!(without, SAMPLE, "the sample sets it");
        let settings = Settings::read(&without).expect("it reads without");
        assert_eq!(settings.sessions.compact_at, 500_000);
        // The program compacts within 100,000 and 1,000,000 tokens: outside that it is refused, by its key.
        for (at, read) in [
            ("99999", false),
            ("100000", true),
            ("1000000", true),
            ("1000001", false),
            ("0", false),
        ] {
            let result = Settings::read(&of(at));
            match result {
                Ok(settings) if read => assert_eq!(settings.sessions.compact_at.to_string(), at),
                Ok(_) => panic!("{at} is no context the program compacts at"),
                Err(why) if read => panic!("{at}: {why}"),
                Err(why) => assert!(why.contains("sessions.compact_at"), "{at}: {why}"),
            }
        }
    }

    #[test]
    fn the_fast_ci_of_the_installation_is_refused_in_words_that_say_where_it_went_and_the_runners_are_read() {
        let github = "\n[projects.example.github]\nrepository = \"owner/name\"\n";
        let settings = Settings::read(&format!("{SAMPLE}{github}")).expect("it reads");
        let of = |settings: Settings| settings.projects["example"].github.clone().expect("github");
        assert_eq!(of(settings).runners, ["github"]);
        let runners = format!("{SAMPLE}{github}runners = [\"github\", \"14900K\"]\n");
        let both = of(Settings::read(&runners).expect("it reads")).runners;
        assert_eq!(both, ["github", "14900K"]);
        let checks = format!("{SAMPLE}{github}checks = [\"check\"]\n");
        let refused = Settings::read(&checks).expect_err("refused");
        assert!(
            refused.starts_with("projects.example.github.checks: the checks that count are the jobs of the project's"),
            "{refused}"
        );
        assert!(
            refused.contains("[ci.full]") && refused.ends_with("delete the line"),
            "{refused}"
        );
        let fast = format!("{SAMPLE}{github}[projects.example.github.fast]\nworkflow = \"fast.yml\"\n");
        let refused = Settings::read(&fast).expect_err("refused");
        assert!(
            refused.starts_with("projects.example.github.fast: the fast workflow is the project's"),
            "{refused}"
        );
        assert!(
            refused.contains("[ci.fast]") && refused.contains("github.runners"),
            "{refused}"
        );
    }

    #[test]
    fn a_table_for_the_log_a_failed_job_was_read_by_is_refused_in_words_that_say_what_is_read_now() {
        let github = "\n[projects.example.github]\nrepository = \"owner/name\"\n";
        let log = format!("{SAMPLE}{github}[projects.example.github.log]\n");
        let refused = Settings::read(&log).expect_err("refused");
        assert!(
            refused.starts_with("projects.example.github.log: the engine reads no log by a project's patterns"),
            "{refused}"
        );
        assert!(
            refused.contains("the end of its failed step") && refused.ends_with("delete the table"),
            "{refused}"
        );
    }

    #[test]
    fn what_a_letting_on_allows_is_20_dollars_where_none_is_named_and_a_number_of_dollars_none_below_nothing() {
        let without = SAMPLE.replace("let_on_allowance_usd = 2.0\n", "");
        assert_ne!(without, SAMPLE, "the sample sets it");
        let budget = Settings::read(&without).expect("it reads without").budget();
        assert_eq!(budget.expect("a budget").let_on, 20_000_000);
        for bad in ["-1.0", "nan", "inf"] {
            let text = SAMPLE.replace("let_on_allowance_usd = 2.0", &format!("let_on_allowance_usd = {bad}"));
            let refused = Settings::read(&text).expect_err(bad);
            assert!(refused.contains("agents.let_on_allowance_usd"), "{bad}: {refused}");
        }
    }

    #[test]
    fn the_samples_levels_read_with_their_default_and_highest() {
        let settings = Settings::read(SAMPLE).expect("the sample reads");
        let levels = settings.levels().expect("its levels are made");
        let names: Vec<&str> = levels.levels().iter().map(|l| l.name.as_str()).collect();
        assert_eq!(names, ["1", "2", "3", "4"]);
        assert_eq!(
            levels.levels()[0].desc,
            "Easy work: small and clear, with little to work out"
        );
        assert_eq!(levels.of_item(None).unwrap(), "2");
        assert!(
            levels.of_item(Some("4")).is_err(),
            "4 is above the highest an item may be at"
        );
        assert_eq!(settings.agents.levels["1"].model, "claude-haiku-5-5");
    }

    /// A cell of the sample flow's kind, on a line that runs this many levels above its item's, at this effort.
    fn cell(levels_above: u32, effort: Option<flow::Effort>) -> CellSpec {
        CellSpec {
            id: "D1".into(),
            title: "Deep research".to_string(),
            kind: flow::Kind::Agent,
            line: Some("deep_research".to_string()),
            reads_only: false,
            tests_only: false,
            task: Some("Research.".to_string()),
            task_file: None,
            act: None,
            must_write: Vec::new(),
            describes: Vec::new(),
            named_by: None,
            reads: Vec::new(),
            given: Vec::new(),
            tools: Vec::new(),
            system: Vec::new(),
            returns: 3,
            calls: 40,
            effort,
            levels_above,
            takes_main: false,
            takes_turn: false,
            watches: Vec::new(),
        }
    }

    #[test]
    fn a_round_runs_on_the_model_of_its_item_s_level_raised_as_its_line_says_and_at_its_cell_s_effort() {
        let settings = Settings::read(SAMPLE).unwrap();
        let profile = Path::new("/data/profile");
        let ran = |item: Option<&str>, cell: &CellSpec| settings.ran_on(item, cell, profile, None).unwrap();
        let at = ran(Some("1"), &cell(0, None));
        assert_eq!(
            (at.level.as_str(), at.model.as_str(), at.effort),
            ("1", "claude-haiku-5-5", None)
        );
        let raised = ran(Some("1"), &cell(1, Some(flow::Effort::Xhigh)));
        assert_eq!(raised.level, "2");
        assert_eq!(raised.model, "claude-sonnet-5-5");
        assert_eq!(raised.effort.as_deref(), Some("xhigh"));
        // No line runs above the highest level one round may run at, which the sample leaves above the highest an item
        // may be at, and an item of no level is at the default.
        assert_eq!(ran(Some("3"), &cell(1, None)).model, "claude-fable-5-1");
        assert_eq!(ran(None, &cell(0, None)).level, "2");
        let gone = settings
            .ran_on(Some("7"), &cell(0, None), profile, None)
            .expect_err("there is no level 7");
        assert!(gone.contains("7 is no level"), "{gone}");
    }

    #[test]
    fn a_line_above_its_item_s_level_runs_up_to_the_highest_level_a_round_may_run_at() {
        let profile = Path::new("/data/profile");
        let ran = |text: &str, item: &str| {
            let settings = Settings::read(text).unwrap();
            settings
                .ran_on(Some(item), &cell(1, None), profile, None)
                .unwrap()
                .level
        };
        // The sample: items at most at 3, rounds at most at 4: an item at 3 has a line one above run at 4.
        assert_eq!(ran(SAMPLE, "3"), "4");
        // The highest for rounds left out is the top level, as the sample says.
        let open = SAMPLE.replace("highest_level = \"4\"\n", "");
        assert_eq!(Settings::read(&open).unwrap().agents.highest_level, None);
        assert_eq!(ran(&open, "3"), "4");
        // Held at 3, the line of an item at 3 runs at 3, and an item below it still has its line one above.
        let held = SAMPLE.replace("highest_level = \"4\"", "highest_level = \"3\"");
        assert_eq!(ran(&held, "3"), "3");
        assert_eq!(ran(&held, "2"), "3");
    }

    #[test]
    fn a_highest_level_for_items_above_the_one_for_rounds_is_refused_naming_both_keys() {
        let below = SAMPLE.replace("highest_level = \"4\"", "highest_level = \"2\"");
        let refused = Settings::read(&below)
            .and_then(|s| s.levels().map(|_| ()))
            .expect_err("refused");
        assert!(
            refused.contains("items.highest_level") && refused.contains("agents.highest_level"),
            "{refused}"
        );
        let none = SAMPLE.replace("highest_level = \"4\"", "highest_level = \"9\"");
        let refused = Settings::read(&none)
            .and_then(|s| s.levels().map(|_| ()))
            .expect_err("refused");
        assert_eq!(refused, "agents.highest_level: there is no level \"9\"");
    }

    #[test]
    fn a_round_records_the_engines_directory_it_ran_with_and_the_account_it_was_handed_out_with_apart() {
        let settings = Settings::read(SAMPLE).unwrap();
        let under = (settings.ran_on(None, &cell(0, None), Path::new("/data/profile"), Some("work"))).unwrap();
        assert_eq!(
            (under.directory.as_deref(), under.account.as_deref()),
            (Some("/data/profile"), Some("work"))
        );
    }

    // Microseconds, the sample read: an installation that names another credential for the agents does not start.
    #[test]
    fn the_carriers_own_variables_and_a_credential_above_the_token_named_in_agents_env_are_refused() {
        let named = |names: &str| SAMPLE.replace("# env = [\"HTTPS_PROXY\"]", &format!("env = [{names}]"));
        let refused = Settings::read(&named(
            "\"CLAUDE_CODE_OAUTH_TOKEN\", \"CLAUDE_CONFIG_DIR\", \"CLAUDE_CODE_SUBPROCESS_ENV_SCRUB\"",
        ))
        .expect_err("refused");
        for name in [
            "CLAUDE_CODE_OAUTH_TOKEN",
            "CLAUDE_CONFIG_DIR",
            "CLAUDE_CODE_SUBPROCESS_ENV_SCRUB",
        ] {
            assert!(
                refused.contains(&format!("agents.env: {name} is the carrier's own")),
                "{refused}"
            );
        }
        // An installation that names a credential the program takes above the token does not start.
        let refused = Settings::read(&named("\"ANTHROPIC_API_KEY\", \"ANTHROPIC_AUTH_TOKEN\"")).expect_err("refused");
        assert_eq!(
            refused,
            "agents.env: ANTHROPIC_API_KEY gives the agents' program a credential it takes above an account's token, \
             and it runs under no credential but the owner's accounts: take it out, and add the account on the \
             Settings view\n\
             agents.env: ANTHROPIC_AUTH_TOKEN gives the agents' program a credential it takes above an account's \
             token, and it runs under no credential but the owner's accounts: take it out, and add the account on the \
             Settings view"
        );
        assert!(Settings::read(&named("\"LANGUAGE\"")).is_ok());
    }

    // Microseconds, the sample read: the owner is told where the directory went, in the words the engine stops with.
    #[test]
    fn the_directory_of_the_programs_settings_is_refused_with_where_it_went() {
        let named = SAMPLE.replace("[agents]\n", "[agents]\nconfig_dir = \"/data/agents/claude\"\n");
        assert_ne!(named, SAMPLE);
        assert_eq!(
            Settings::read(&named).expect_err("refused"),
            "agents.config_dir: the engine keeps the agents' program's directory of settings and sessions itself, at \
             profile/ in the data directory, and gives it as CLAUDE_CONFIG_DIR: move the directory there, so that the \
             works' sessions go on, and take agents.config_dir out of [agents]."
        );
    }

    #[test]
    fn the_words_of_a_title_are_the_settings_and_have_no_default() {
        let levels = Settings::read(SAMPLE).unwrap().levels().unwrap();
        assert_eq!(levels.title_fits("word ".repeat(20).trim()), Ok(()));
        assert_eq!(levels.title_fits(&"\u{4e2d}".repeat(20)), Ok(()));
        let why = levels.title_fits(&"\u{4e2d}".repeat(21)).unwrap_err();
        assert!(
            why.starts_with("the title has 21 words and a title has 20 at most"),
            "{why}"
        );
        let without = SAMPLE.replace("title_max = 20\n", "");
        let refused = Settings::read(&without).expect_err("refused");
        assert!(refused.contains("title_max"), "{refused}");
    }

    #[test]
    fn levels_that_cannot_be_and_models_the_command_line_does_not_take_are_refused_all_at_once() {
        let text = (SAMPLE.replace("model = \"claude-haiku-5-5\"", "model = \"gpt-5\""))
            .replace("default_level = \"2\"", "default_level = \"5\"");
        let refused = Settings::read(&text).expect_err("refused");
        assert_eq!(refused.lines().count(), 2, "{refused}");
        assert!(
            refused.contains("items.default_level: there is no level \"5\""),
            "{refused}"
        );
        assert!(refused.contains("agents.levels.1.model"), "{refused}");
        let above = SAMPLE.replace("default_level = \"2\"", "default_level = \"4\"");
        let refused = Settings::read(&above).expect_err("refused");
        assert!(refused.contains("is above \"3\""), "{refused}");
        // The one model of every round, as settings had it before levels, is refused with what replaces it.
        let before = SAMPLE.replace("slots = 2", "slots = 2\nmodel = \"claude-haiku-5-5\"");
        let refused = Settings::read(&before).expect_err("refused");
        assert!(refused.starts_with("agents.model: "), "{refused}");
        for key in ["[agents.levels.<name>]", "[items]", "default_level"] {
            assert!(refused.contains(key), "{refused}");
        }
    }

    #[test]
    fn the_removed_setting_of_the_branches_before_a_change_is_refused_in_words_that_say_it_can_go() {
        let text =
            format!("{SAMPLE}\n[projects.example.github]\nrepository = \"o/n\"\nruns_on_before_branches = true\n");
        let refused = Settings::read(&text).expect_err("refused");
        assert!(
            refused.starts_with("projects.example.github.runs_on_before_branches: this setting was removed")
                && refused.ends_with("delete the line"),
            "{refused}"
        );
    }

    #[test]
    fn an_unknown_key_is_refused() {
        let text = SAMPLE.replace("slots = 2", "slots = 2\nslot = 3");
        let refused = Settings::read(&text).expect_err("an unknown key is refused");
        assert!(refused.contains("slot"), "{refused}");
    }

    // What is a project's own is in its declaration, not in the installation's settings: each key the settings once held
    // of it is refused, by its name, as any key the settings do not know is.
    #[test]
    fn what_is_a_projects_own_is_refused_in_the_installation_by_its_name() {
        let on_github = on_github();
        let at = |text: &str, from: &str, line: &str| text.replacen(from, &format!("{from}\n{line}"), 1);
        for (key, text) in [
            ("check", at(SAMPLE, "main = \"main\"", "check = \"./test.sh\"")),
            (
                "permissions",
                at(
                    SAMPLE,
                    "command = \"claude\"",
                    "permissions = { locks = [\"uv lock\"] }",
                ),
            ),
            (
                "merge_method",
                at(&on_github, "repository = \"owner/name\"", "merge_method = \"squash\""),
            ),
            (
                "delete_branch",
                at(&on_github, "repository = \"owner/name\"", "delete_branch = false"),
            ),
            (
                "timeout_minutes",
                at(&on_github, "repository = \"owner/name\"", "timeout_minutes = 120"),
            ),
        ] {
            let refused = Settings::read(&text).expect_err(key);
            assert!(refused.contains(&format!("unknown field `{key}`")), "{key}: {refused}");
        }
    }

    // What wf-agents gives every installation the same is no setting: the arguments the program is given, the settings
    // files it reads, the number of values a history needs, and whether `query` is open and its bounds. A file that names
    // one is refused by its key, as a key the settings do not know is, rather than taken to be in force. Milliseconds.
    #[test]
    fn what_wf_agents_gives_is_not_a_setting() {
        let at = |from: &str, line: &str| SAMPLE.replacen(from, &format!("{from}\n{line}"), 1);
        for (key, text) in [
            ("args", at("slots = 2", "args = [\"--strict-mcp-config\"]")),
            ("setting_sources", at("slots = 2", "setting_sources = \"user\"")),
            (
                "permissions",
                at("slots = 2", "permissions = { locks = [\"uv lock\"] }"),
            ),
            ("history", format!("{SAMPLE}\n[history]\nsamples = 19\n")),
            ("query", format!("{SAMPLE}\n[query]\nopen = true\n")),
        ] {
            let refused = Settings::read(&text).expect_err(key);
            assert!(refused.contains(&format!("unknown field `{key}`")), "{key}: {refused}");
        }
    }

    /// The sample with its table for GitHub written in.
    fn on_github() -> String {
        let table = (SAMPLE.lines())
            .skip_while(|line| *line != "# [projects.example.github]")
            .take_while(|line| line.starts_with('#'))
            .map(|line| line.strip_prefix("# ").unwrap_or(line))
            .collect::<Vec<_>>()
            .join("\n");
        format!("{SAMPLE}\n{table}\n")
    }

    #[test]
    fn the_local_repository_is_the_host_where_no_github_is_given() {
        assert_eq!(Settings::read(SAMPLE).unwrap().projects["example"].github, None);
    }

    #[test]
    fn the_samples_table_for_github_reads() {
        let github = (Settings::read(&on_github()).expect("it reads").projects["example"]
            .github
            .clone())
        .expect("GitHub is the host");
        assert_eq!(github.repository, "owner/name");
        assert_eq!(github.url(), "https://github.com/owner/name.git");
        assert_eq!(github.remote, "origin");
        assert_eq!(github.token_env, "WF_GITHUB_TOKEN");
        assert_eq!(github.start_minutes, 10);
        assert_eq!(github.gh, "gh");
    }

    #[test]
    fn a_work_s_pull_request_is_at_github_by_the_repository_and_its_number() {
        let github = Settings::read(&on_github()).expect("it reads").projects["example"]
            .github
            .clone()
            .expect("GitHub is the host");
        assert_eq!(github.pull_at(12), "https://github.com/owner/name/pull/12");
    }

    #[test]
    fn what_github_cannot_be_given_is_refused_all_at_once() {
        let text = (on_github().replace("repository = \"owner/name\"", "repository = \"name\""))
            .replace("# env = [\"HTTPS_PROXY\"]", "env = [\"WF_GITHUB_TOKEN\"]");
        let refused = Settings::read(&text).expect_err("refused");
        assert_eq!(refused.lines().count(), 2, "{refused}");
        assert!(refused.contains("projects.example.github.repository"), "{refused}");
        assert!(refused.contains("agents.env: WF_GITHUB_TOKEN"), "{refused}");
        let unknown = on_github().replace("remote = \"origin\"", "remotes = \"origin\"");
        assert!(Settings::read(&unknown).expect_err("refused").contains("remotes"));
    }

    /// The sample with its table for the copies written in.
    fn with_backup() -> String {
        let table = (SAMPLE.lines())
            .skip_while(|line| *line != "# [backup]")
            .take_while(|line| line.starts_with('#'))
            .map(|line| line.strip_prefix("# ").unwrap_or(line))
            .collect::<Vec<_>>()
            .join("\n");
        format!("{SAMPLE}\n{table}\n")
    }

    #[test]
    fn the_copies_stay_on_the_machine_where_no_backup_is_given() {
        assert_eq!(Settings::read(SAMPLE).unwrap().backup, None);
    }

    #[test]
    fn the_samples_table_for_the_copies_reads_with_its_defaults() {
        let backup = Settings::read(&with_backup())
            .expect("it reads")
            .backup
            .expect("the copies go off the machine");
        assert_eq!(backup.repository, "owner/name");
        assert_eq!(backup.token_env, "WF_BACKUP_TOKEN");
        assert_eq!(backup.keep, 7);
        assert_eq!(backup.gh, "gh");
        // Only the repository is needed.
        let short = format!("{SAMPLE}\n[backup]\nrepository = \"owner/name\"\n");
        let backup = Settings::read(&short).unwrap().backup.unwrap();
        assert_eq!(
            (backup.token_env.as_str(), backup.keep, backup.gh.as_str()),
            ("WF_BACKUP_TOKEN", 7, "gh")
        );
    }

    #[test]
    fn what_the_copies_cannot_be_given_is_refused_all_at_once() {
        let text = (with_backup().replace("repository = \"owner/name\"", "repository = \"name\""))
            .replace("keep = 7", "keep = 0")
            .replace("# env = [\"HTTPS_PROXY\"]", "env = [\"WF_BACKUP_TOKEN\"]");
        let refused = Settings::read(&text).expect_err("refused");
        assert_eq!(refused.lines().count(), 3, "{refused}");
        assert!(refused.contains("backup.repository"), "{refused}");
        assert!(refused.contains("backup.keep"), "{refused}");
        assert!(refused.contains("agents.env: WF_BACKUP_TOKEN"), "{refused}");
        let unknown = with_backup().replace("keep = 7", "kept = 7");
        assert!(Settings::read(&unknown).expect_err("refused").contains("kept"));
        // The project's token is not the copies'.
        let same = format!(
            "{}\n{}",
            on_github(),
            "[backup]\nrepository = \"o/n\"\ntoken_env = \"WF_GITHUB_TOKEN\"\n"
        );
        let refused = Settings::read(&same).expect_err("refused");
        assert!(refused.contains("backup.token_env"), "{refused}");
    }

    // The token writes a release that holds the whole database and every conversation: it has a name, and no agent is
    // given it, as with the token for the project's repository.
    #[test]
    fn the_copies_token_has_a_name_and_is_given_to_no_agent() {
        let empty = with_backup().replace("token_env = \"WF_BACKUP_TOKEN\"", "token_env = \"\"");
        let refused = Settings::read(&empty).expect_err("an empty name is refused");
        assert_eq!(refused.lines().count(), 1, "{refused}");
        assert!(
            refused.contains("backup.token_env: the token's variable has a name"),
            "{refused}"
        );
        let shared = with_backup().replace("# env = [\"HTTPS_PROXY\"]", "env = [\"WF_BACKUP_TOKEN\"]");
        let refused = Settings::read(&shared).expect_err("a token an agent is given is refused");
        assert_eq!(refused.lines().count(), 1, "{refused}");
        assert!(refused.contains("agents.env: WF_BACKUP_TOKEN"), "{refused}");
        assert!(refused.contains("which no agent is given"), "{refused}");
    }

    #[test]
    fn no_slots_and_a_budget_below_nothing_are_refused() {
        let text =
            (SAMPLE.replace("slots = 2", "slots = 0")).replace("work_budget_usd = 5.0", "work_budget_usd = -1.0");
        let refused = Settings::read(&text).expect_err("both are refused");
        assert_eq!(refused.lines().count(), 2, "{refused}");
    }

    // A setting's value is what an agent is given beside its key: a text as it is, any other value as JSON writes it;
    // a key the settings do not have, a value left out and a table left out have none.
    #[test]
    fn a_settings_value_is_given_as_an_agent_is_told_it() {
        let settings = Settings::read(SAMPLE).unwrap();
        assert_eq!(settings.value("owner.language").as_deref(), Some("Chinese"));
        assert_eq!(settings.value("agents.slots").as_deref(), Some("2"));
        assert_eq!(
            settings.value("agents.levels.2.model").as_deref(),
            Some("claude-sonnet-5-5")
        );
        for none in [
            "github.checks",
            "github.repository",
            "agents.config_dir",
            "owner.voice",
            "agents.slots.more",
        ] {
            assert_eq!(settings.value(none), None, "{none}");
        }
        let listed = Settings::read(&on_github()).unwrap();
        assert_eq!(
            listed.value("projects.example.github.runners").as_deref(),
            Some("[\"github\"]")
        );
    }

    /// The sample with a second project, `other`, in a repository of its own on GitHub.
    fn two_projects() -> String {
        let other = "[projects.other]\nrepo = \"/path/to/the/other\"\nmain = \"trunk\"\n\n\
                     [projects.other.github]\nrepository = \"owner/other\"\nrunners = [\"github\", \"14900k\"]\n";
        format!("{SAMPLE}\n{other}")
    }

    // An installation names its projects, each with its own repository, main branch, check and host.
    #[test]
    fn an_installation_names_several_projects_each_with_its_repository_and_host() {
        let settings = Settings::read(&two_projects()).expect("it reads");
        assert_eq!(settings.projects.keys().collect::<Vec<_>>(), ["example", "other"]);
        let (example, other) = (&settings.projects["example"], &settings.projects["other"]);
        assert_eq!(example.repo, Path::new("/path/to/the/repository"));
        assert_eq!(example.github, None, "the local repository is its host");
        assert_eq!(
            (other.repo.as_path(), other.main.as_str()),
            (Path::new("/path/to/the/other"), "trunk")
        );
        let github = other.github.as_ref().expect("GitHub is its host");
        assert_eq!(github.repository, "owner/other");
        assert_eq!(github.runners, ["github", "14900k"]);
        assert_eq!(settings.project("other"), Some(other));
        assert_eq!(settings.project("elsewhere"), None);
    }

    // What is wrong with a project's settings is said with the project's key, every project's at once; an installation
    // with no project is refused.
    #[test]
    fn what_a_project_cannot_be_given_is_said_with_its_name() {
        let text = (two_projects().replace("main = \"trunk\"", "main = \"\""))
            .replace("repository = \"owner/other\"", "repository = \"other\"")
            .replace("main = \"main\"", "main = \"\"");
        let refused = Settings::read(&text).expect_err("refused");
        assert_eq!(refused.lines().count(), 3, "{refused}");
        assert!(
            refused.contains("projects.example.main: the main branch has a name"),
            "{refused}"
        );
        assert!(
            refused.contains("projects.other.main: the main branch has a name"),
            "{refused}"
        );
        assert!(refused.contains("projects.other.github.repository"), "{refused}");
        let start = SAMPLE.find("[projects.example]").unwrap();
        let end = SAMPLE.find("[agents]").unwrap();
        let none = format!("{}{}", &SAMPLE[..start], &SAMPLE[end..]);
        let refused = Settings::read(&none).expect_err("refused");
        assert!(
            refused.starts_with("projects: the installation names at least one project"),
            "{refused}"
        );
        let spaced = SAMPLE.replace("[projects.example]", "[projects.\" example\"]");
        assert!(
            Settings::read(&spaced)
                .expect_err("refused")
                .contains("a project's name is some text")
        );
    }

    // Settings written when an installation had one project, in [project] and [github], are told where each went,
    // both at once: the one edit an installation of one project needs.
    #[test]
    fn the_tables_of_the_one_project_are_told_where_they_went() {
        let one = SAMPLE.replace("[projects.example]", "[project]");
        let refused = Settings::read(&one).expect_err("refused");
        assert!(
            refused.starts_with("project: an installation names each of its projects now"),
            "{refused}"
        );
        assert!(refused.contains("write [project] as [projects.<name>]"), "{refused}");
        let both = format!("{one}\n[github]\nrepository = \"owner/name\"\n");
        let refused = Settings::read(&both).expect_err("refused");
        assert_eq!(refused.lines().count(), 2, "{refused}");
        assert!(
            refused.lines().nth(1).is_some_and(|l| l.starts_with("github: ")),
            "{refused}"
        );
        // Moved as it says, it reads.
        let moved = both
            .replace("[project]", "[projects.example]")
            .replace("[github]", "[projects.example.github]");
        assert!(
            Settings::read(&moved).expect("it reads").projects["example"]
                .github
                .is_some()
        );
    }

    // The copies' token is none of the projects' tokens.
    #[test]
    fn the_copies_token_is_no_projects_token() {
        let same = format!(
            "{}\n[backup]\nrepository = \"o/n\"\ntoken_env = \"WF_GITHUB_TOKEN\"\n",
            two_projects()
        );
        let refused = Settings::read(&same).expect_err("refused");
        assert!(refused.contains("the repository of the project other"), "{refused}");
    }

    // The subagents an agent may start are three where the settings give none, or as they say.
    #[test]
    fn the_subagents_are_three_where_the_settings_give_none() {
        let settings = Settings::read(SAMPLE).unwrap();
        assert_eq!(settings.value("agents.subagents.most").as_deref(), Some("3"));
        let given = SAMPLE.replace(
            "work_budget_usd = 5.0",
            "work_budget_usd = 5.0\nsubagents = { most = 1 }",
        );
        assert_eq!(Settings::read(&given).unwrap().agents.subagents.most, 1);
    }

    // Keeping sessions warm was a key of these settings, under two names: settings that still name either are told
    // that it is the engine's own state now, which the owner switches on their page.
    #[test]
    fn the_keys_that_once_said_whether_sessions_are_kept_warm_are_told_where_it_went() {
        for key in ["keep_warm", "warm_while_paused"] {
            let old = SAMPLE.replace("warm_minutes = 55", &format!("warm_minutes = 55\n{key} = true"));
            assert_ne!(old, SAMPLE, "the sample sets the minutes");
            let refused = Settings::read(&old).expect_err("refused");
            assert!(refused.contains(&format!("sessions.{key}")), "{refused}");
            assert!(refused.contains("page"), "{refused}");
        }
    }

    // The context at which the agents' program compacts a session was `context_limit`, which said "limit" of a number
    // that ends no session: settings that still name it are told where it went.
    #[test]
    fn the_context_a_session_is_compacted_at_where_it_once_was_is_told_where_it_moved() {
        let old = SAMPLE.replace("compact_at = 500000", "context_limit = 500000");
        assert_ne!(old, SAMPLE, "the sample sets it");
        let refused = Settings::read(&old).expect_err("refused");
        assert!(refused.contains("sessions.context_limit"), "{refused}");
        assert!(refused.contains("compact_at"), "{refused}");
    }

    // The owner's language was once a setting of the agents: settings that still say it there are told where it went.
    #[test]
    fn the_owners_language_where_it_once_was_is_told_where_it_moved() {
        let old = SAMPLE.replace(
            "work_budget_usd = 5.0",
            "work_budget_usd = 5.0\nowner_language = \"Chinese\"",
        );
        let refused = Settings::read(&old).expect_err("refused");
        assert!(refused.contains("agents.owner_language"), "{refused}");
        assert!(refused.contains("[owner]"), "{refused}");
    }
}
