//! The installation's settings: the project the works are done in, where its repository is hosted, and how its
//! agents are run. They are read from a TOML file, `install.toml` in the data directory unless another is named; `install.sample.toml` beside this crate's
//! manifest says what each key is. A key the file does not know is refused, as the flow's files refuse one, so that a
//! misspelt key is not silently left at nothing.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use driver::Policy;
use flow::CellSpec;
use serde::Deserialize;
use service::{Levels, LevelsFault};
use work::RanOn;

use crate::permissions::{HEAVY, NEVER_A_LOCK, falls_under};

/// The settings as their file is written.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub project: Project,
    pub agents: Agents,
    pub items: Items,
    /// Where the project's repository is on GitHub, which then carries out the acts; with none, the local repository
    /// does.
    pub github: Option<GitHub>,
    /// Where the copies of the database are kept off the machine, and how many are kept; with none, the copies stay
    /// on the machine alone, which shares the disk they are kept against losing.
    pub backup: Option<Backup>,
    /// When a line of a work begins a new session.
    #[serde(default)]
    pub sessions: Sessions,
}

/// The copies of the database kept off the machine: in a private repository on GitHub, as assets of a release.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
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
    fn wrong(&self, agents: &Agents, github: Option<&GitHub>) -> Vec<String> {
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
        if github.is_some_and(|g| g.token_env == self.token_env) {
            wrong.push(
                "backup.token_env: the copies have a token of their own, not the one of the project's repository"
                    .to_string(),
            );
        }
        wrong
    }
}

/// When a line of a work begins a new session, beyond its first, a lost one, and one the owner asks for.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sessions {
    /// The context in tokens past which a line's session is replaced by a new one: once a round of the line ends with
    /// more, the line's next round begins a new session.
    #[serde(default = "context_limit")]
    pub context_limit: u64,
}

impl Default for Sessions {
    fn default() -> Self {
        Sessions {
            context_limit: context_limit(),
        }
    }
}

fn context_limit() -> u64 {
    500_000
}

/// The project whose works are done: a local git repository, its main branch and the command that checks it.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    /// The repository's own checkout: works branch from its main branch, and are merged into it here.
    pub repo: PathBuf,
    /// The name of the main branch.
    pub main: String,
    /// A shell command run in a work's worktree; its exit with 0 means the work passes.
    pub check: String,
    /// The keys of the repository's own configuration that could make git run a program and that the acts are to let
    /// be, each a whole key or a beginning followed by `*`: `filter.lfs.*` for a repository that needs Git LFS.
    #[serde(default)]
    pub allowed_config: Vec<String>,
    /// Who the engine's commits are by, and the words of each: not in the settings, but in `agents/commits.toml`
    /// beside the flows, put here as the carriers are made ([`crate::carry`]).
    #[serde(skip)]
    pub commits: flow::agents::Commits,
}

/// The project's repository on GitHub: the works' branches are pushed there, checked by its CI through a pull request
/// each, and merged by GitHub.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GitHub {
    /// The repository on github.com, as `owner/name`; `project.repo` is a clone of it.
    pub repository: String,
    /// The clone's name for the repository on GitHub: main's head is fetched to `refs/remotes/<remote>/<main>`.
    #[serde(default = "origin")]
    pub remote: String,
    /// The variable of the engine's environment that holds the engine's token for GitHub.
    #[serde(default = "token_env")]
    pub token_env: String,
    /// The checks of the CI that count, by the name of their check run or commit status.
    pub checks: Vec<String>,
    /// How many minutes a named check may have no run on a pushed commit before the CI is taken never to have
    /// started.
    #[serde(default = "start_minutes")]
    pub start_minutes: u64,
    /// How many minutes the CI of a commit may take before it is given up.
    #[serde(default = "timeout_minutes")]
    pub timeout_minutes: u64,
    /// How a pull request is merged: `squash`, `merge` or `rebase`.
    #[serde(default = "squash")]
    pub merge_method: String,
    /// Whether a work's branch on GitHub is deleted once it is merged.
    #[serde(default)]
    pub delete_branch: bool,
    /// Whether the project's CI runs on a push to the branches the agents' `check_before` pushes to; without it, the
    /// tool is refused before anything is pushed.
    #[serde(default)]
    pub runs_on_before_branches: bool,
    /// The `gh` command line, or a path to it.
    #[serde(default = "gh")]
    pub gh: String,
    /// The project's fast CI, which runs only the tests and checks an agent names; with none, an agent's check is the
    /// whole CI of its commit.
    pub fast: Option<Fast>,
}

/// A project's fast CI: a workflow GitHub runs by hand, always by its file on the main branch, which checks out the
/// commit it is given, runs only the tests and the checks it is given and reports what became of each test. The engine
/// reads its results by the conventions its reading of them holds to (JUnit XML, a unit not built as a testsuite with
/// no testcase and an error, and at least every test a name names run).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fast {
    /// The workflow, by its file's name, such as `fast.yml`.
    pub workflow: String,
    /// The input that takes the commit to check out and test.
    #[serde(default = "commit_input")]
    pub commit_input: String,
    /// The input that takes a value only one request knows, which the run's name must hold: by it the engine finds the
    /// run it asked for among the others.
    #[serde(default = "call_input")]
    pub call_input: String,
    /// The input that takes the tests to run: each name followed by a line break, the last too.
    #[serde(default = "tests_input")]
    pub tests_input: String,
    /// The input that takes the checks to run beside the tests, as a JSON array of the names of their jobs, given only
    /// when one is named.
    #[serde(default = "checks_input")]
    pub checks_input: String,
    /// The checks, each the name of a job of the workflow, that it runs when named, such as the lint.
    #[serde(default)]
    pub named_checks: Vec<String>,
    /// The artifact a run uploads with its JUnit files.
    #[serde(default = "results")]
    pub results: String,
    /// What the project's test runners put between the parts of a test's name, such as `::` and ` > `.
    #[serde(default)]
    pub separators: Vec<String>,
}

fn commit_input() -> String {
    "commit".to_string()
}

fn call_input() -> String {
    "call".to_string()
}

fn tests_input() -> String {
    "tests".to_string()
}

fn checks_input() -> String {
    "checks".to_string()
}

fn results() -> String {
    "test-results".to_string()
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

fn timeout_minutes() -> u64 {
    120
}

fn squash() -> String {
    "squash".to_string()
}

fn gh() -> String {
    "gh".to_string()
}

/// The ways a pull request may be merged.
const MERGE_METHODS: [&str; 3] = ["squash", "merge", "rebase"];

/// How the agents are run.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Agents {
    /// The command line program that runs an agent's round: `claude`, or a path to it.
    pub command: String,
    /// The levels of work, by name: an item's level chooses the model its agents' rounds are run with.
    pub levels: BTreeMap<String, Level>,
    /// How many agents' rounds run at once.
    pub slots: usize,
    /// What is given to the program on every round after what the engine gives it, before the round's line's
    /// permissions and its session.
    #[serde(default)]
    pub args: Vec<String>,
    /// What the installation adds to the engine's lists of what the agents may run.
    #[serde(default)]
    pub permissions: PermissionsAdded,
    /// The names of the variables of the engine's environment the program is given beside those it is always given
    /// (`PATH`, `HOME`, `USER`, `LANG` and `LC_*`, `TMPDIR`, `TERM`, `SHELL`): an API key, for one.
    #[serde(default)]
    pub env: Vec<String>,
    /// Which of the program's settings files it reads, as `--setting-sources`; `user` where none is given, so that a
    /// settings file the worktree holds, which an agent may write, is not read.
    #[serde(default = "user")]
    pub setting_sources: String,
    /// The program's own directory of settings and sign-in, as `CLAUDE_CONFIG_DIR`; with none, the program's own
    /// default.
    pub config_dir: Option<PathBuf>,
    /// The language the blocks the owner reads are written in.
    pub owner_language: String,
    /// What a work's rounds may spend in all, in dollars, before the work waits for the owner; with none, no limit.
    pub work_budget_usd: Option<f64>,
}

fn user() -> String {
    "user".to_string()
}

/// What an installation adds to the engine's lists of what the agents may run (`permissions`): what its project's
/// toolchain counts as heavy, and the commands that update its lock files, with the uses of them that would build,
/// run or install after all. It takes nothing away from the engine's lists.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PermissionsAdded {
    /// Heavy commands no agent runs, each a command with the start of its arguments.
    #[serde(default)]
    pub heavy: Vec<String>,
    /// Commands that update a lock file and build, run and install nothing, each a command with the start of its
    /// arguments: the agents of a line that writes may run them, though a heavy command covers them.
    #[serde(default)]
    pub locks: Vec<String>,
    /// Uses of the lock commands refused all the same, each a command whose arguments may hold `*` for any text.
    #[serde(default)]
    pub locks_except: Vec<String>,
}

/// The program's flags, as its own help lists them (claude 2.1.321), that take no value: the only ones `agents.args`
/// may end with, since any other, written last without its value, could take the first of the engine's arguments
/// after them for its value.
const TAKES_NO_VALUE: [&str; 28] = [
    "--allow-dangerously-skip-permissions",
    "--ax-screen-reader",
    "--bg",
    "--background",
    "--bare",
    "--brief",
    "--chrome",
    "-c",
    "--continue",
    "--dangerously-skip-permissions",
    "--desktop",
    "--disable-slash-commands",
    "--exclude-dynamic-system-prompt-sections",
    "--fork-session",
    "--forward-subagent-text",
    "--ide",
    "--include-hook-events",
    "--include-partial-messages",
    "--no-chrome",
    "--no-session-persistence",
    "-p",
    "--print",
    "--replay-user-messages",
    "--restricted",
    "--safe-mode",
    "--strict-mcp-config",
    "--tmux",
    "--verbose",
];

/// The sources of settings files the program may be told to read, besides the user's: those of the working
/// directory, which is the work's worktree, where an agent can write a settings file.
const SOURCES_IN_THE_WORKTREE: [&str; 2] = ["project", "local"];

impl Agents {
    /// What is wrong with these settings of the agents, each in words with its key: arguments that would undo or
    /// swallow those the engine gives after them, settings files read from the worktree, and a command of the added
    /// permissions that names nothing, could be read as more than one rule, or would allow a heavy command whole.
    fn wrong(&self) -> Vec<String> {
        let mut wrong = Vec::new();
        let given = |flag: &str| (self.args.iter()).any(|a| a == flag || a.starts_with(&format!("{flag}=")));
        if given("--settings") {
            wrong.push(
                "agents.args: --settings would give the program rules the engine does not see; put them in the \
                 program's own directory of settings"
                    .to_string(),
            );
        }
        if given("--permission-prompt-tool") {
            wrong.push(
                "agents.args: --permission-prompt-tool would have a tool approve what no rule refuses".to_string(),
            );
        }
        if given("--setting-sources") {
            wrong.push("agents.args: --setting-sources is set by agents.setting_sources".to_string());
        }
        if self.args.iter().any(|a| a == "--") {
            wrong.push("agents.args: -- would make the engine's arguments after it no flags".to_string());
        }
        let open = |a: &String| a.starts_with('-') && !a.contains('=') && !TAKES_NO_VALUE.contains(&a.as_str());
        if let Some(last) = self.args.last().filter(|a| open(a)) {
            wrong.push(format!(
                "agents.args: {last}, last, is no flag known to take no value, and could take the engine's next \
                 argument for its value"
            ));
        }
        let sources = self.setting_sources.split(',').map(str::trim);
        if let Some(source) = sources.into_iter().find(|s| SOURCES_IN_THE_WORKTREE.contains(s)) {
            wrong.push(format!(
                "agents.setting_sources: {source} would have the program read settings files in the worktree, \
                 which an agent may write"
            ));
        }
        let added = &self.permissions;
        for (key, list) in [
            ("heavy", &added.heavy),
            ("locks", &added.locks),
            ("locks_except", &added.locks_except),
        ] {
            if list.iter().any(|c| c.trim().is_empty()) {
                wrong.push(format!("agents.permissions.{key}: each entry names a command"));
            }
            // A parenthesis would end the rule the entry is written into and begin another; a `*` in a command
            // that is allowed or refused from its start on would stand for any text.
            let star = key != "locks_except";
            if let Some(c) = list
                .iter()
                .find(|c| c.contains(['(', ')']) || (star && c.contains('*')))
            {
                let what = if star {
                    "a `*` or a parenthesis"
                } else {
                    "a parenthesis"
                };
                wrong.push(format!("agents.permissions.{key}: {c:?} holds {what}"));
            }
        }
        let heavy: Vec<&str> = HEAVY
            .iter()
            .copied()
            .chain(added.heavy.iter().map(String::as_str))
            .collect();
        for lock in &added.locks {
            if let Some(h) = NEVER_A_LOCK.iter().find(|h| falls_under(lock, h)) {
                wrong.push(format!(
                    "agents.permissions.locks: {lock:?} would push, which is the engine's alone ({h:?})"
                ));
            }
            if let Some(h) = heavy.iter().find(|h| falls_under(h, lock)) {
                wrong.push(format!(
                    "agents.permissions.locks: {lock:?} would allow all of the heavy command {h:?}"
                ));
            }
        }
        wrong
    }
}

/// A level of work: what work it is for, and the model its rounds are run with.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Level {
    /// What work the level is for, in words for whoever gives an item its level.
    pub desc: String,
    /// The model, as the command line's `--model` takes it.
    pub model: String,
}

/// How items are given their level.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Items {
    /// The level an item is added at where it is given none, by its name among the levels.
    pub default_level: String,
    /// The highest level an item may be at, by its name among the levels: an item above it is refused as it is added,
    /// and no line runs above it; with none, the top level.
    pub highest_level: Option<String>,
}

/// What the settings are told where they name `agents.model`, the one model every round once ran on.
const ONE_MODEL: &str = "agents.model: rounds no longer run on one model: each item has a level, and each level names \
     its model. Take out agents.model; add a table [agents.levels.<name>] with desc and model for each level, and a \
     table [items] with default_level, the level an item is added at where it is given none, and, if you like, \
     highest_level. install.sample.toml shows them.";

/// The settings file's name in the data directory.
pub const FILE: &str = "install.toml";

impl GitHub {
    /// What is wrong with these settings, each in words with its key.
    fn wrong(&self, agents: &Agents) -> Vec<String> {
        let mut wrong = Vec::new();
        let parts: Vec<&str> = self.repository.split('/').collect();
        if !(parts.len() == 2 && parts.iter().all(|p| !p.is_empty())) {
            wrong.push(format!(
                "github.repository: a repository is named as owner/name, not {:?}",
                self.repository
            ));
        }
        if self.checks.is_empty() || self.checks.iter().any(|c| c.trim().is_empty()) {
            wrong.push("github.checks: at least one check counts, each named".to_string());
        }
        if self.start_minutes == 0 || self.timeout_minutes < self.start_minutes {
            wrong.push(
                "github.start_minutes, github.timeout_minutes: the CI is given at least a minute to start, and no \
                 less time to end"
                    .to_string(),
            );
        }
        if !MERGE_METHODS.contains(&self.merge_method.as_str()) {
            wrong.push(format!(
                "github.merge_method: one of {}, not {:?}",
                MERGE_METHODS.join(", "),
                self.merge_method
            ));
        }
        if let Some(fast) = &self.fast {
            let blank = [
                &fast.workflow,
                &fast.commit_input,
                &fast.call_input,
                &fast.tests_input,
                &fast.checks_input,
                &fast.results,
            ];
            if blank.iter().any(|v| v.trim().is_empty()) || fast.named_checks.iter().any(|c| c.trim().is_empty()) {
                wrong.push(
                    "github.fast: the workflow, its inputs, its results and each check it runs by name are named"
                        .to_string(),
                );
            }
        }
        if self.token_env.is_empty() {
            wrong.push("github.token_env: the token's variable has a name".to_string());
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
        if settings.sessions.context_limit == 0 {
            wrong.push("sessions.context_limit: a session may hold some context".to_string());
        }
        if settings.project.main.is_empty() {
            wrong.push("project.main: the main branch has a name".to_string());
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
        if let Some(github) = &settings.github {
            wrong.extend(github.wrong(&settings.agents));
        }
        if let Some(backup) = &settings.backup {
            wrong.extend(backup.wrong(&settings.agents, settings.github.as_ref()));
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

    /// What the owner is to be told of the settings as the engine starts, each a sentence: where an agent's sign-in
    /// and settings are not the installation's own.
    pub fn warnings(&self) -> Vec<String> {
        match self.agents.config_dir {
            Some(_) => Vec::new(),
            None => vec![format!(
                "The agents use the default sign-in and settings directory of {}, which an agent can write to; \
                 agents.config_dir gives them one of their own.",
                self.agents.command
            )],
        }
    }

    /// The levels items are given, as these settings name them; or what is wrong with them, in words with their keys.
    pub fn levels(&self) -> Result<Levels, String> {
        let items = &self.items;
        let levels = (self.agents.levels.iter()).map(|(name, level)| service::Level {
            name: name.clone(),
            desc: level.desc.clone(),
        });
        Levels::new(levels, &items.default_level, items.highest_level.as_deref()).map_err(|fault| match fault {
            LevelsFault::None => "agents.levels: there is at least one level".to_string(),
            LevelsFault::NoSuchDefault(name) => format!("items.default_level: there is no level {name:?}"),
            LevelsFault::NoSuchHighest(name) => format!("items.highest_level: there is no level {name:?}"),
            LevelsFault::DefaultAboveHighest { default, highest } => {
                format!("items.default_level: {default:?} is above {highest:?}, the highest level an item may be at")
            }
        })
    }

    /// What a round of this cell, for an item at this level, runs on: the level of its item, or above it as far as the
    /// cell's line says, with that level's model and the cell's effort; or why it cannot run, in words. An item added
    /// before items had levels is at the default.
    pub fn ran_on(&self, item_level: Option<&str>, cell: &CellSpec) -> Result<RanOn, String> {
        let level = self.levels()?.of_round(item_level, cell.levels_above)?;
        let model = (self.agents.levels.get(&level))
            .map(|level| level.model.clone())
            .ok_or_else(|| format!("agents.levels: there is no level {level:?}"))?;
        Ok(RanOn {
            level,
            model,
            effort: cell.effort.map(|effort| effort.name().to_string()),
            account: (self.agents.config_dir.as_ref()).map(|dir| dir.to_string_lossy().into_owned()),
        })
    }

    /// When a line begins a new session: past the context limit, and where the model or the account its session's
    /// rounds ran on is not the one its rounds run on now.
    pub fn policy(&self) -> Policy {
        let settings = self.clone();
        Policy {
            limit: self.sessions.context_limit,
            on: Box::new(move |item, cell| settings.ran_on(item.level.as_deref(), cell).ok()),
        }
    }

    /// The budget of a work in millionths of a dollar, where there is one.
    pub fn budget_micros(&self) -> Option<u64> {
        // The budget was checked to be a finite number of no less than nothing; a larger one than a u64 holds is none.
        (self.agents.work_budget_usd).map(|usd| (usd * 1_000_000.0).round() as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = include_str!("../install.sample.toml");

    #[test]
    fn the_sample_reads() {
        let settings = Settings::read(SAMPLE).expect("the sample reads");
        assert_eq!(settings.project.main, "main");
        assert_eq!(settings.agents.command, "claude");
        assert_eq!(settings.agents.slots, 2);
        assert_eq!(settings.agents.config_dir, None);
        assert_eq!(settings.budget_micros(), Some(5_000_000));
        assert!(
            !settings.agents.args.iter().any(|a| a == "--setting-sources"),
            "the code gives it"
        );
        assert_eq!(settings.agents.setting_sources, "user");
        assert!(settings.agents.env.is_empty());
        assert_eq!(settings.sessions.context_limit, 500_000);
    }

    #[test]
    fn a_session_is_given_a_context_limit_of_500000_tokens_where_none_is_set() {
        let without = SAMPLE.replace("context_limit = 500000\n", "");
        assert_ne!(without, SAMPLE, "the sample sets it");
        let settings = Settings::read(&without).expect("it reads without");
        assert_eq!(settings.policy().limit, 500_000);
        let set = Settings::read(&SAMPLE.replace("context_limit = 500000", "context_limit = 200000")).unwrap();
        assert_eq!(set.policy().limit, 200_000);
        let none = SAMPLE.replace("context_limit = 500000", "context_limit = 0");
        assert!(
            Settings::read(&none)
                .expect_err("refused")
                .contains("sessions.context_limit")
        );
    }

    #[test]
    fn a_fast_ci_is_read_with_its_defaults_and_its_names_must_be_given() {
        let github =
            "\n[github]\nrepository = \"owner/name\"\nchecks = [\"check\"]\n[github.fast]\nworkflow = \"fast.yml\"\n";
        let settings = Settings::read(&format!("{SAMPLE}{github}")).expect("it reads");
        let fast = settings.github.and_then(|g| g.fast).expect("a fast CI");
        assert_eq!(
            fast,
            Fast {
                workflow: "fast.yml".to_string(),
                commit_input: "commit".to_string(),
                call_input: "call".to_string(),
                tests_input: "tests".to_string(),
                checks_input: "checks".to_string(),
                named_checks: Vec::new(),
                results: "test-results".to_string(),
                separators: Vec::new(),
            }
        );
        let blank = format!("{SAMPLE}{}", github.replace("\"fast.yml\"", "\" \""));
        let refused = Settings::read(&blank).expect_err("refused");
        assert!(refused.contains("github.fast"), "{refused}");
    }

    #[test]
    fn the_owner_is_told_when_the_agents_have_no_directory_of_their_own() {
        let mut settings = Settings::read(SAMPLE).unwrap();
        let warned = settings.warnings();
        assert_eq!(warned.len(), 1);
        assert!(warned[0].contains("agents.config_dir"), "{}", warned[0]);
        settings.agents.config_dir = Some("agents".into());
        assert!(settings.warnings().is_empty());
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
            task: Some("Research.".to_string()),
            act: None,
            must_write: Vec::new(),
            describes: Vec::new(),
            runs_on: Vec::new(),
            given: Vec::new(),
            tools: Vec::new(),
            system: Vec::new(),
            returns: 3,
            calls: 40,
            effort,
            levels_above,
            takes_main: false,
            triggers: Vec::new(),
        }
    }

    #[test]
    fn a_round_runs_on_the_model_of_its_item_s_level_raised_as_its_line_says_and_at_its_cell_s_effort() {
        let settings = Settings::read(SAMPLE).unwrap();
        let ran = |item: Option<&str>, cell: &CellSpec| settings.ran_on(item, cell).unwrap();
        let at = ran(Some("1"), &cell(0, None));
        assert_eq!(
            (at.level.as_str(), at.model.as_str(), at.effort),
            ("1", "claude-haiku-5-5", None)
        );
        let raised = ran(Some("1"), &cell(1, Some(flow::Effort::Xhigh)));
        assert_eq!(raised.level, "2");
        assert_eq!(raised.model, "claude-sonnet-5-5");
        assert_eq!(raised.effort.as_deref(), Some("xhigh"));
        // No line runs above the highest level an item may be at, and an item of no level is at the default.
        assert_eq!(ran(Some("3"), &cell(1, None)).model, "claude-opus-5-5");
        assert_eq!(ran(None, &cell(0, None)).level, "2");
        let gone = settings
            .ran_on(Some("7"), &cell(0, None))
            .expect_err("there is no level 7");
        assert!(gone.contains("7 is no level"), "{gone}");
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
    fn an_unknown_key_is_refused() {
        let text = SAMPLE.replace("slots = 2", "slots = 2\nslot = 3");
        let refused = Settings::read(&text).expect_err("an unknown key is refused");
        assert!(refused.contains("slot"), "{refused}");
    }

    #[test]
    fn the_samples_permissions_read_and_add_to_the_engines() {
        let text = SAMPLE.replace("# permissions = {", "permissions = {");
        let added = Settings::read(&text).expect("it reads").agents.permissions;
        assert_eq!(added.heavy, ["uv"]);
        assert_eq!(added.locks, ["uv lock"]);
        assert_eq!(added.locks_except, ["uv lock *--script*"]);
        assert_eq!(
            Settings::read(SAMPLE).unwrap().agents.permissions,
            PermissionsAdded::default()
        );
        let empty = SAMPLE.replace("# permissions = {", "permissions = { heavy = [\" \"] } #");
        let refused = Settings::read(&empty).expect_err("refused");
        assert!(refused.contains("agents.permissions.heavy"), "{refused}");
    }

    #[test]
    fn arguments_that_would_undo_or_take_the_engines_own_are_refused() {
        let with = |args: &str| {
            SAMPLE.replace(
                "\"--disable-slash-commands\"]",
                &format!("\"--disable-slash-commands\", {args}]"),
            )
        };
        for (args, says) in [
            ("\"--settings\", \"{}\"", "--settings"),
            ("\"--settings={}\"", "--settings"),
            ("\"--\"", "-- would"),
            ("\"--permission-mode\"", "--permission-mode, last"),
            ("\"--tools\"", "--tools, last"),
            ("\"--permission-prompt-tool\"", "--permission-prompt-tool, last"),
            ("\"--setting-sources\", \"user,project\"", "--setting-sources"),
            ("\"--setting-sources=local\"", "--setting-sources"),
            (
                "\"--permission-prompt-tool\", \"mcp__ok__approve\"",
                "--permission-prompt-tool would",
            ),
            (
                "\"--permission-prompt-tool=mcp__ok__approve\"",
                "--permission-prompt-tool would",
            ),
        ] {
            let refused = Settings::read(&with(args)).expect_err(args);
            assert!(refused.contains(says), "{args}: {refused}");
        }
        // A flag that takes no value may come last, and one that takes a value may come before its value.
        for args in [
            "\"--verbose\"",
            "\"--model\", \"claude-haiku-5-5\"",
            "\"--model=claude-haiku-5-5\"",
        ] {
            assert!(Settings::read(&with(args)).is_ok(), "{args}");
        }
    }

    #[test]
    fn settings_files_in_the_worktree_are_not_read() {
        for sources in ["project", "user,local", "user, project"] {
            let text = SAMPLE.replace(
                "# setting_sources = \"user\"",
                &format!("setting_sources = \"{sources}\""),
            );
            let refused = Settings::read(&text).expect_err(sources);
            assert!(refused.contains("agents.setting_sources"), "{sources}: {refused}");
        }
        let user = SAMPLE.replace("# setting_sources = \"user\"", "setting_sources = \"user\"");
        assert!(Settings::read(&user).is_ok());
    }

    #[test]
    fn an_added_command_cannot_take_an_engine_refusal_away() {
        let with = |permissions: &str| SAMPLE.replace("# permissions = {", &format!("permissions = {permissions} #"));
        for (permissions, says) in [
            (
                "{ locks = [\"git push\"] }",
                "\"git push\" would allow all of the heavy command \"git push\"",
            ),
            (
                "{ locks = [\"cargo\"] }",
                "would allow all of the heavy command \"cargo\"",
            ),
            ("{ locks = [\"npm\"] }", "would allow all of the heavy command \"npm\""),
            ("{ locks = [\"git push origin\"] }", "\"git push origin\" would push"),
            (
                "{ heavy = [\"uv\"], locks = [\"uv\"] }",
                "would allow all of the heavy command \"uv\"",
            ),
            ("{ locks = [\"cargo *\"] }", "locks: \"cargo *\" holds a `*`"),
            (
                "{ locks = [\"cargo update) Bash(cargo test\"] }",
                "holds a `*` or a parenthesis",
            ),
            ("{ heavy = [\"uv*\"] }", "heavy: \"uv*\" holds a `*`"),
            (
                "{ locks_except = [\"uv lock (x)\"] }",
                "locks_except: \"uv lock (x)\" holds a parenthesis",
            ),
        ] {
            let refused = Settings::read(&with(permissions)).expect_err(permissions);
            assert!(refused.contains(says), "{permissions}: {refused}");
        }
        // A lock command under a heavy one, and an exception with `*`, are what the lists are for.
        let fine = with("{ heavy = [\"uv\"], locks = [\"uv lock\"], locks_except = [\"uv lock *--script*\"] }");
        assert!(Settings::read(&fine).is_ok());
    }

    /// The sample with its table for GitHub written in.
    fn on_github() -> String {
        let table = (SAMPLE.lines())
            .skip_while(|line| *line != "# [github]")
            .take_while(|line| line.starts_with('#'))
            .map(|line| line.strip_prefix("# ").unwrap_or(line))
            .collect::<Vec<_>>()
            .join("\n");
        format!("{SAMPLE}\n{table}\n")
    }

    #[test]
    fn the_local_repository_is_the_host_where_no_github_is_given() {
        assert_eq!(Settings::read(SAMPLE).unwrap().github, None);
    }

    #[test]
    fn the_samples_table_for_github_reads() {
        let github = Settings::read(&on_github())
            .expect("it reads")
            .github
            .expect("GitHub is the host");
        assert_eq!(github.repository, "owner/name");
        assert_eq!(github.url(), "https://github.com/owner/name.git");
        assert_eq!(github.remote, "origin");
        assert_eq!(github.token_env, "WF_GITHUB_TOKEN");
        assert_eq!(github.checks, ["check"]);
        assert_eq!((github.start_minutes, github.timeout_minutes), (10, 120));
        assert_eq!(github.merge_method, "squash");
        assert!(!github.delete_branch);
        assert_eq!(github.gh, "gh");
    }

    #[test]
    fn what_github_cannot_be_given_is_refused_all_at_once() {
        let text = (on_github().replace("repository = \"owner/name\"", "repository = \"name\""))
            .replace("checks = [\"check\"]", "checks = []")
            .replace("merge_method = \"squash\"", "merge_method = \"fast-forward\"")
            .replace("# env = [\"ANTHROPIC_API_KEY\"]", "env = [\"WF_GITHUB_TOKEN\"]");
        let refused = Settings::read(&text).expect_err("refused");
        assert_eq!(refused.lines().count(), 4, "{refused}");
        assert!(refused.contains("github.repository"), "{refused}");
        assert!(refused.contains("github.checks"), "{refused}");
        assert!(refused.contains("github.merge_method"), "{refused}");
        assert!(refused.contains("agents.env: WF_GITHUB_TOKEN"), "{refused}");
        let unknown = on_github().replace("delete_branch = false", "delete_branches = false");
        assert!(
            Settings::read(&unknown)
                .expect_err("refused")
                .contains("delete_branches")
        );
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
            .replace("# env = [\"ANTHROPIC_API_KEY\"]", "env = [\"WF_BACKUP_TOKEN\"]");
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
        let shared = with_backup().replace("# env = [\"ANTHROPIC_API_KEY\"]", "env = [\"WF_BACKUP_TOKEN\"]");
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
}
