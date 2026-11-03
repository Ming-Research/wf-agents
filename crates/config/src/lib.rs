//! The configuration: what differs by installation and by project. It lives outside this repository, in a data
//! directory the owner keeps under git and points the command at with `--data`:
//!
//! - `install.toml`: the owner, the machines and their slots, the CLIs and agent levels, the gh allow list and the
//!   limits of a query ([`Install`]);
//! - `projects/<name>.toml`: one project each ([`Project`]); its `extends` lists files under `shared/` to start
//!   from, merged in order: a later table merges into an earlier one key by key, and any other value, a list
//!   included, replaces the earlier one whole;
//! - `rules/<name>.md`: the rules agents are given, by the names projects list them under;
//! - `db/`: the database, kept out of git; `wf-agents init` makes it;
//! - `agents/<cli>/`: the configuration directory each CLI runs agents with, unless the CLI names another, kept out
//!   of git; the owner logs in to the CLI there once.
//!
//! The engine reads only what is committed, so each round names the configuration it ran with by its commit.
//! `config check` reads the working tree instead, to check a change before it is committed. A field the types do not
//! know is an error.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

pub mod shared_git;

/// Everything wrong with a configuration.
#[derive(Debug, Clone)]
pub struct Error {
    pub problems: Vec<String>,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.problems.join("\n"))
    }
}

impl std::error::Error for Error {}

fn fail(p: impl Into<String>) -> Error {
    Error {
        problems: vec![p.into()],
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Install {
    pub owner: Owner,
    pub items: Items,
    pub machines: Machines,
    pub agents: Agents,
    pub gh: Gh,
    pub query: Query,
    #[serde(default)]
    pub reasoning: Reasoning,
    #[serde(default)]
    pub backup: Backup,
}

/// The installation's one copy of the reasoning system, which every project that keeps records is kept by.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reasoning {
    /// A clone of the reasoning system (github.com/Ming-Research/reasoning-system): relative to the data directory, or
    /// absolute. By default `reasoning-system` in the data directory. The engine runs its `check.py` on a project's
    /// records and gives its rules (`rules/`) to the cells that work on them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system: Option<PathBuf>,
}

/// The copies of the database the engine keeps, beside it under `db/backups/`: one before every migration, which the
/// database itself takes, and one a day while the engine runs.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Backup {
    /// How many daily copies are kept, the newest; 7 when not set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keep: Option<usize>,
}

impl Backup {
    /// How many daily copies are kept.
    pub fn keep(&self) -> usize {
        self.keep.unwrap_or(7)
    }
}

impl Reasoning {
    /// Where the reasoning system is, for the data directory `data`.
    pub fn system(&self, data: &Path) -> PathBuf {
        match &self.system {
            Some(d) => data.join(d),
            None => data.join("reasoning-system"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Owner {
    /// The language everything the owner reads is written in.
    pub language: String,
    /// How often, in seconds, the engine reminds the owner of the asks waiting for the owner's answer while any waits,
    /// in one notice saying how many wait and how long the oldest has; 0 sends no reminder.
    #[serde(default = "remind_interval")]
    pub remind_interval: u64,
}

fn remind_interval() -> u64 {
    3_600
}

/// How wide a title is when shown, the unit `items.title_max` is in: every character but a space counts the columns it
/// takes in a line of monospaced text. A space of any kind is not counted; a character of East Asian wide or full width
/// (a Chinese character, a kana, a hangul syllable, an emoji) counts 2, a combining mark or other character that takes
/// no column 0, and any other character 1. So the same limit holds titles of any language to about the same room on the
/// board: 24 allows 24 letters of English or 12 Chinese characters.
pub fn title_width(title: &str) -> usize {
    title
        .chars()
        .filter(|c| !c.is_whitespace())
        .map(|c| unicode_width::UnicodeWidthChar::width(c).unwrap_or(0))
        .sum()
}

/// Whether a title is within `max` ([`title_width`]); if not, why not, saying how wide it is.
pub fn title_fits(title: &str, max: usize) -> Result<(), String> {
    let width = title_width(title);
    if width > max {
        return Err(format!(
            "the title is {width} wide and a title is {max} wide at most (spaces are not counted, and a character of \
             East Asian wide or full width counts 2): write it shorter"
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Items {
    /// The widest an item's title may be, and an observation's, as [`title_width`] counts it: spaces are not counted,
    /// a character of East Asian wide or full width counts 2 and any other 1. A title is held to it when it is made or
    /// changed; a title already there is not counted again. There is no default: the installation names the width.
    pub title_max: u32,
    /// The highest level an item may be at, by its name among the levels: an item above it is refused when it is
    /// made and when a work starts on it, while the advisor may still run one level above; none, any level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub highest_level: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Machines {
    /// The owner's own machine, where no heavy work runs.
    pub workstation: String,
    /// What counts as heavy work.
    pub heavy: String,
    /// The least time, in seconds, between two polls of remote status such as CI and pull requests.
    pub poll_interval: u32,
    /// The machine set aside for precise timing; none when there is none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timing: Option<String>,
    /// The machines works run on, by name.
    pub hosts: BTreeMap<String, Host>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Host {
    /// How many works may run on it at once.
    pub slots: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Agents {
    /// The CLIs agents run in, by name.
    pub clis: BTreeMap<String, Cli>,
    /// The levels of work, the tiers, by name: an item's level picks the CLI and the model its agents run with, each
    /// cell at the reasoning effort the flow declares for it; its advisor runs at the level one above.
    pub levels: BTreeMap<String, Level>,
    /// What the agents of each line may not do on this machine.
    #[serde(default)]
    pub permissions: Permissions,
    /// How many subagents a round may start for chores.
    #[serde(default)]
    pub subagents: Subagents,
    /// The level the architect runs at, by its name among the levels, and the review of its item changes; the top level
    /// when none is named.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub architect: Option<String>,
    /// The level the search of a project's observations runs at, by its name among the levels; the lowest level when
    /// none is named.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observer: Option<String>,
    /// The level the reading of a work's history runs at (the line `librarian`), by its name among the levels; the
    /// lowest level when none is named.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub librarian: Option<String>,
    /// The level the pick of the architect's next matter runs at (the line `picker`), by its name among the levels; the
    /// lowest level when none is named.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub picker: Option<String>,
    /// The context, in tokens, past which a line's session is replaced by a new one at the line's next round: the
    /// prompt of the last call of the session's latest round, as recorded.
    #[serde(default = "context_limit")]
    pub context_limit: u64,
    /// How long, in seconds, the architect's understanding of a project (cell `U1`) holds: a new session of the
    /// architect's within it goes on from the notebook the understanding wrote, and one past it understands the project
    /// again.
    #[serde(default = "understanding_life")]
    pub understanding_life: u64,
    /// The least time, in seconds, between two matters of slots free for one project's architect (cell `S0`'s
    /// `queue_slots_free`): one that found nothing to start does not come back at once.
    #[serde(default = "slots_free_wait")]
    pub slots_free_wait: u64,
    /// The longest time, in seconds, a project's architect waits for the owner's reply in a talk with the owner (cell
    /// `H11`) before it goes on with its queue, holding the talk until the owner says more or ends it.
    #[serde(default = "owner_reply_wait")]
    pub owner_reply_wait: u64,
}

fn context_limit() -> u64 {
    500_000
}

fn understanding_life() -> u64 {
    21_600
}

fn slots_free_wait() -> u64 {
    3_600
}

fn owner_reply_wait() -> u64 {
    300
}

/// What the agents of each line may not do on this machine, besides what the CLI's own arguments say. The engine
/// gives each round's command line these as commands and tools it refuses.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Permissions {
    /// Heavy work no agent runs here, each a command with the start of its arguments: compiling, type checking,
    /// building, test suites, benchmarks and measurements run in CI.
    #[serde(default = "heavy")]
    pub heavy: Vec<String>,
    /// The lines whose agents only read: they edit no file and commit nothing.
    #[serde(default = "read_only")]
    pub read_only: Vec<String>,
}

impl Default for Permissions {
    fn default() -> Permissions {
        Permissions {
            heavy: heavy(),
            read_only: read_only(),
        }
    }
}

fn heavy() -> Vec<String> {
    [
        "cargo",
        "rustc",
        "rustup",
        "make",
        "cmake",
        "ninja",
        "bazel",
        "tsc",
        "npx",
        "pnpm",
        "yarn",
        "bun",
        "deno",
        "npm run",
        "npm test",
        "npm ci",
        "npm install",
        "npm exec",
        "node --test",
        "vitest",
        "jest",
        "go build",
        "go test",
        "go run",
        "pytest",
        "python -m pytest",
        "mvn",
        "gradle",
        "swift build",
        "swift test",
        "xcodebuild",
        "hyperfine",
        "git push",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// The review and the advisor, which look at a work's worktree, the reader of a work's history, and every line of a
/// project's architect, which reads the project in a worktree of its own at main: its work's, the setting of a new
/// policy included, the review of its item changes and the search of observations.
fn read_only() -> Vec<String> {
    [
        "review",
        "advisor",
        "librarian",
        "architect",
        "architect_review",
        "observer",
        "picker",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// The tools that edit files, which a line that only reads is refused.
pub const EDIT_TOOLS: [&str; 4] = ["Edit", "Write", "MultiEdit", "NotebookEdit"];

/// The git commands that change the repository, which a line that only reads is refused; those that only read, such
/// as show, diff and log, it may run.
pub const GIT_CHANGES: [&str; 19] = [
    "add",
    "commit",
    "reset",
    "checkout",
    "switch",
    "restore",
    "stash",
    "merge",
    "rebase",
    "rm",
    "mv",
    "cherry-pick",
    "revert",
    "tag",
    "branch",
    "worktree",
    "apply",
    "am",
    "clean",
];

impl Permissions {
    /// What a round on `line` may not use, as the CLI's tool rules name it: each heavy command, and for a line that
    /// only reads, the tools that edit files and the git commands that change the repository.
    pub fn refused(&self, line: &str) -> Vec<String> {
        let mut out: Vec<String> = self.heavy.iter().map(|c| format!("Bash({c} *)")).collect();
        if self.read_only.iter().any(|l| l == line) {
            out.extend(EDIT_TOOLS.iter().map(|t| t.to_string()));
            out.extend(GIT_CHANGES.iter().map(|g| format!("Bash(git {g} *)")));
        }
        out
    }
}

/// How many subagents a round may start for chores within it, such as reading documents or searching code.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Subagents {
    /// At most this many in one round.
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cli {
    /// Which CLI it is, which decides how a round is run in it.
    pub kind: CliKind,
    /// The command that starts it.
    pub command: String,
    /// Arguments put before the ones the engine adds.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    /// The directory the CLI keeps its own configuration in when it runs an agent (its settings, memory, plugins,
    /// tool servers and login), apart from the owner's own: relative to the data directory, or absolute. By default
    /// `agents/<the CLI's name>` in the data directory; the engine keeps it out of git.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_dir: Option<PathBuf>,
}

impl Cli {
    /// The directory the CLI named `name` runs agents with, for the data directory `data`.
    pub fn home(&self, data: &Path, name: &str) -> PathBuf {
        match &self.config_dir {
            Some(d) => data.join(d),
            None => data.join("agents").join(name),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CliKind {
    Claude,
}

/// The families of models `claude` knows, each also its alias for the family's latest model.
const CLAUDE_FAMILIES: [&str; 5] = ["haiku", "sonnet", "opus", "fable", "mythos"];

impl CliKind {
    /// How long, in milliseconds, the CLI's prompt cache keeps a session's prompt after the session's last call: a call
    /// later than that writes the whole prompt again, at many times the price of reading it. `claude` keeps it an hour.
    /// None for a CLI with no prompt cache, whose sessions lose nothing by waiting.
    pub fn cache_lifetime(self) -> Option<i64> {
        match self {
            CliKind::Claude => Some(60 * 60_000),
        }
    }

    /// Whether the CLI takes a model by this name, in the forms its `--model` takes: for `claude`, the alias of a
    /// family's latest model (`opus`) or a model's full name (`claude-opus-5-5`, `claude-3-5-sonnet-20241022`), each
    /// with `[1m]` after it or not; a family it does not know, or a name in no such form, it does not take.
    pub fn knows_model(self, model: &str) -> bool {
        match self {
            CliKind::Claude => {
                let name = model.strip_suffix("[1m]").unwrap_or(model);
                if CLAUDE_FAMILIES.contains(&name) {
                    return true;
                }
                let Some(rest) = name.strip_prefix("claude-") else {
                    return false;
                };
                let parts: Vec<&str> = rest.split('-').collect();
                let numbers = parts
                    .iter()
                    .filter(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
                    .count();
                let families = parts.iter().filter(|p| CLAUDE_FAMILIES.contains(p)).count();
                families == 1 && numbers > 0 && numbers + 1 == parts.len()
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Level {
    /// What work the level is for.
    pub desc: String,
    pub cli: String,
    pub model: String,
    /// The effort a level's rounds ran at before each cell declared its own: read in configuration commits of that time,
    /// for the rounds recorded then, and refused in the configuration the engine runs with.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Gh {
    /// The gh commands agents may have the engine run, each as the words it starts with, such as `pr view`.
    pub allow: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Query {
    /// The most rows a query hands back; it says when it cut the rest.
    pub max_rows: u32,
    /// How long a query may run, in milliseconds.
    pub timeout_ms: u64,
}

/// One project: its repository, how its CI and design are set up, and the settings the flow reads.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    /// The GitHub repository, as owner/name.
    pub repo: String,
    /// A clone of it on the machine the engine runs on; works' worktrees are made from it.
    pub checkout: PathBuf,
    /// The branch works merge into.
    #[serde(default = "default_main")]
    pub main: String,
    /// What the project's designs aim for, such as performance and correctness with no regard for migration cost.
    pub design_goal: String,
    /// The project's records, kept by the reasoning system: its works derive their designs from them, check them with
    /// the installation's copy of the reasoning system and give its rules to the cells that work on them. Their
    /// directory is the project's design paths: a change to it is a change of design, which the owner rules on decision
    /// cards. None, and the project keeps no records: its works skip every step on them and raise no card.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub records: Option<Records>,
    /// The paths that hold tests: the only ones the adversarial tester changes.
    pub test_paths: Vec<String>,
    /// The places adversarial testing attacks, beyond the acceptance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attack_points: Option<String>,
    /// What correctness is judged against: a reference implementation, a specification, conformance files.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oracles: Option<String>,
    /// What is under test, whose gaps the project's work is meant to find: a language, a compiler, a library.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub under_test: Vec<String>,
    /// The projects this one depends on or that depend on it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub other_projects: Vec<String>,
    /// What the review checks beyond the review table, one line each.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub review_list: Vec<String>,
    pub ci: Ci,
    /// How a merge to main is deployed, and how a failed deployment shows; none when nothing is deployed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deploy: Option<String>,
    /// The limits of the flow's counters, by counter. A counter with no limit is only counted.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub limits: BTreeMap<String, u64>,
    /// Rule texts particular to this project, by their names under `rules/`, given to every agent's cell beside the
    /// flow's own rules; how agents work in every project is the flow's, and is not repeated here.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<String>,
    /// The project's constitution, the lasting values its decisions come from: a path in its repository, such as
    /// `constitution.md`, spelled as the project's main has it; none when the project has none. Agents are given its
    /// text as main has it, never as the checkout's files have it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub constitution: Option<PathBuf>,
    /// The tags the project's observations may carry, each with what it covers: the architect tags an observation with
    /// these only. The owner sets them; none, and observations carry no tag.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub observation_tags: Vec<ObservationTag>,
    /// Whether every agent of the project hands in what was wrong with its round's cell, which the project's architect
    /// takes in to improve the flow: on unless set to false; off, no agent is asked for it.
    #[serde(default = "on", skip_serializing_if = "is_on")]
    pub flow_feedback: bool,
}

/// Where a project keeps its records, and how they are checked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Records {
    /// The directory in the project's repository that holds them, such as `reasoning`.
    pub root: String,
    /// Whether they are in English only, which their check then holds them to (check.py's `--ascii`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ascii: bool,
}

fn on() -> bool {
    true
}

fn is_on(b: &bool) -> bool {
    *b
}

/// A tag a project's observations may carry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservationTag {
    /// Its name, as observations carry it.
    pub name: String,
    /// What it covers, in one line.
    pub desc: String,
}

fn default_main() -> String {
    "main".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ci {
    /// The workflow of the fast CI, which the ci tool waits for: it runs on each push of a work's branch, or, with
    /// `on_demand`, when the tool runs it.
    pub fast: String,
    /// The workflows that make up the full CI, those of downstream projects included, each as owner/name:workflow
    /// or, for this project's own, its name.
    pub full: Vec<String>,
    /// The checks a merge needs to have passed.
    pub merge_checks: Vec<String>,
    /// The input of the fast CI's workflow that takes the tests to run, their names separated by spaces, when it has
    /// one: given it, the workflow runs exactly those tests, every one to its end, and the ci_before tool runs it so on
    /// its temporary ref, which the workflow's own runs on push leave alone; none when it has no such input, and
    /// ci_before then pushes the temporary ref and waits for the whole run the push makes, as
    /// `runs_on_before_branches` says it does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tests_input: Option<String>,
    /// Whether the fast CI runs when the engine pushes the temporary ref (`wf-agents-before/...`) that ci_before puts
    /// the code before the change on. Read only where `tests_input` is none: ci_before then waits for the run the push
    /// makes, and for it to end it reads each listed test's result from its logs. Set to false where the CI ignores
    /// those refs, so that ci_before refuses at once, saying so, instead of waiting for a run that does not come.
    #[serde(default = "runs_on_before_branches_default", skip_serializing_if = "is_true")]
    pub runs_on_before_branches: bool,
    /// The artifact a red run of the fast CI uploads with the files the code generates, such as types generated
    /// from other types, each at its path in the repository: the ci tool puts those that differ from the head in the
    /// work's worktree, for the engineer to commit; none when the CI uploads none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generated_artifact: Option<String>,
    /// Whether the fast CI runs only when the engine asks for it: the ci tool then runs the fast workflow by hand on
    /// the work's branch with the tests named (`tests_input`, which must be set), and the full CI, which no push of a
    /// work's branch is to run, is run by hand too, by the tools that need it, when the commit has no run of it yet.
    /// Off, the pushes of a work's branch run both, and the tools only wait for those runs.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub on_demand: bool,
    /// The workflows the measure tool may run, each with what it takes and gives; none when the project has no
    /// measurement workflow, and the measure tool then refuses and says so. The tool runs a workflow on a temporary ref
    /// under `wf-agents-measure/` and finds its run by the ref's commit and the workflow's name, so the project's
    /// workflows that run on push, the fast CI and a measurement workflow itself among them, must ignore branches under
    /// `wf-agents-measure/`; else the push starts the fast CI as well, and a push run of the measurement workflow could
    /// be taken for the one dispatched. Left out of the settings an agent is shown when empty, which they read as
    /// `none`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub measure: Vec<MeasureWorkflow>,
}

/// A workflow of a project that the measure tool runs by hand on a temporary ref of the code to measure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MeasureWorkflow {
    /// The workflow's file name, such as `measure.yml`.
    pub workflow: String,
    /// The names of the inputs it takes; the tool refuses an input not named here.
    #[serde(default)]
    pub inputs: Vec<String>,
    /// The name of the artifact it uploads with what it measured, which the tool downloads beside the jobs' logs; none
    /// when it uploads none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact: Option<String>,
}

/// A project that does not say otherwise has its fast CI run on the temporary refs, as every project's did before the
/// setting existed.
fn runs_on_before_branches_default() -> bool {
    true
}

fn is_true(b: &bool) -> bool {
    *b
}

/// The settings the flow reads from a project, as `{"config": name}` inputs name them.
pub const SETTINGS: [&str; 7] = [
    "design_goal",
    "attack_points",
    "full_ci",
    "deploy",
    "records",
    "under_test",
    "other_projects",
];

impl Project {
    /// The ref of the project's main in its checkout: the remote's, `origin/<main>`, which a fetch moves. The
    /// checkout's own `<main>` branch and its files are never read for the project's text: nothing updates them.
    pub fn main_ref(&self) -> String {
        format!("origin/{}", self.main)
    }

    /// The project's design paths: the directory of its records, with its trailing slash; none for a project that keeps
    /// no records.
    pub fn design_paths(&self) -> Vec<String> {
        self.records
            .iter()
            .map(|r| format!("{}/", r.root.trim_end_matches('/')))
            .collect()
    }

    /// The value an input that reads a setting is given, null where the project sets nothing; none for a setting the
    /// flow does not read.
    pub fn setting(&self, name: &str) -> Option<serde_json::Value> {
        use serde_json::{Value, json};
        let text = |t: &Option<String>| t.as_ref().map_or(Value::Null, |s| Value::String(s.clone()));
        let list = |l: &Vec<String>| if l.is_empty() { Value::Null } else { json!(l) };
        Some(match name {
            "design_goal" => Value::String(self.design_goal.clone()),
            "attack_points" => text(&self.attack_points),
            "full_ci" => json!({"workflows": self.ci.full, "merge_checks": self.ci.merge_checks}),
            "deploy" => text(&self.deploy),
            "records" => self.records.as_ref().map_or(Value::Null, |r| json!(r)),
            "under_test" => list(&self.under_test),
            "other_projects" => list(&self.other_projects),
            _ => return None,
        })
    }

    /// Whether the project meets what a field of every return needs; none for a need the configuration does not
    /// answer.
    pub fn has(&self, need: &str) -> Option<bool> {
        match need {
            "flow_feedback" => Some(self.flow_feedback),
            _ => None,
        }
    }
}

/// A whole configuration, as read from a commit or from the working tree.
#[derive(Debug, Clone)]
pub struct Config {
    pub install: Install,
    pub projects: BTreeMap<String, Project>,
    /// The text of each rule, by name.
    pub rules: BTreeMap<String, String>,
}

/// A configuration as committed, with the commit it was read from.
#[derive(Debug, Clone)]
pub struct Committed {
    pub commit: String,
    pub config: Config,
}

/// Where the files of a configuration are read from.
trait Files {
    /// A file's text; none when there is no such file.
    fn read(&self, path: &str) -> Result<Option<String>, String>;
    /// The names of the files directly in a directory.
    fn list(&self, dir: &str) -> Result<Vec<String>, String>;
}

/// The files as they are on disk.
struct Tree<'a>(&'a Path);

impl Files for Tree<'_> {
    fn read(&self, path: &str) -> Result<Option<String>, String> {
        match std::fs::read_to_string(self.0.join(path)) {
            Ok(t) => Ok(Some(t)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(format!("{path}: {e}")),
        }
    }

    fn list(&self, dir: &str) -> Result<Vec<String>, String> {
        let entries = match std::fs::read_dir(self.0.join(dir)) {
            Ok(es) => es,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(format!("{dir}: {e}")),
        };
        let mut out = Vec::new();
        for e in entries {
            let e = e.map_err(|e| format!("{dir}: {e}"))?;
            if e.file_type().map_err(|e| format!("{dir}: {e}"))?.is_file() {
                out.push(e.file_name().to_string_lossy().into_owned());
            }
        }
        out.sort();
        Ok(out)
    }
}

/// The files as one commit of the data directory has them.
struct Commit<'a> {
    dir: &'a Path,
    commit: String,
    files: BTreeSet<String>,
}

fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let out = git_bytes(dir, args)?;
    String::from_utf8(out).map_err(|e| format!("git {}: {e}", args.join(" ")))
}

/// What git writes to its standard output, as bytes; on failure, its command and what it says.
fn git_bytes(dir: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .map_err(|e| format!("git: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(out.stdout)
}

impl Files for Commit<'_> {
    fn read(&self, path: &str) -> Result<Option<String>, String> {
        if !self.files.contains(path) {
            return Ok(None);
        }
        git(self.dir, &["show", &format!("{}:{path}", self.commit)]).map(Some)
    }

    fn list(&self, dir: &str) -> Result<Vec<String>, String> {
        let prefix = format!("{dir}/");
        Ok(self
            .files
            .iter()
            .filter_map(|f| f.strip_prefix(&prefix))
            .filter(|f| !f.contains('/'))
            .map(str::to_string)
            .collect())
    }
}

/// Reads the configuration as the data directory's last commit has it.
pub fn committed(dir: &Path) -> Result<Committed, Error> {
    let commit = git(dir, &["rev-parse", "--verify", "HEAD"])
        .map_err(fail)?
        .trim()
        .to_string();
    let config = at(dir, &commit)?;
    Ok(Committed { commit, config })
}

/// Reads the configuration as a commit of the data directory has it: what a round that names that commit ran with.
pub fn at(dir: &Path, commit: &str) -> Result<Config, Error> {
    if commit.is_empty() || !commit.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(fail(format!("not a commit: {commit}")));
    }
    let files = git(dir, &["ls-tree", "-r", "--name-only", commit]).map_err(fail)?;
    let files = files.lines().map(str::to_string).collect();
    read(&Commit {
        dir,
        commit: commit.to_string(),
        files,
    })
}

/// Reads the configuration as the data directory's files are now, committed or not.
pub fn working(dir: &Path) -> Result<Config, Error> {
    read(&Tree(dir))
}

/// The committed files with some read from texts instead, by their paths.
struct WithFiles<'a> {
    commit: Commit<'a>,
    texts: &'a BTreeMap<String, String>,
}

impl Files for WithFiles<'_> {
    fn read(&self, path: &str) -> Result<Option<String>, String> {
        match self.texts.get(path) {
            Some(t) => Ok(Some(t.clone())),
            None => self.commit.read(path),
        }
    }

    fn list(&self, dir: &str) -> Result<Vec<String>, String> {
        self.commit.list(dir)
    }
}

/// The TOML editor the configuration's files are changed with, keeping their comments and layout, for what writes a
/// setting into them.
pub use toml_edit;

/// The installation's file in the data directory.
pub const INSTALL_FILE: &str = "install.toml";

/// A project's own file in the data directory.
pub fn project_file(name: &str) -> String {
    format!("projects/{name}.toml")
}

/// Reads the configuration as the data directory's last commit has it, with the files given (by their paths in the
/// data directory, each one the commit has) read from the texts given: what a change to them would make it, before the
/// change is written; with the commit it read.
pub fn with_files(dir: &Path, texts: &BTreeMap<String, String>) -> Result<Committed, Error> {
    let commit = git(dir, &["rev-parse", "--verify", "HEAD"])
        .map_err(fail)?
        .trim()
        .to_string();
    let files: BTreeSet<String> = git(dir, &["ls-tree", "-r", "--name-only", &commit])
        .map_err(fail)?
        .lines()
        .map(str::to_string)
        .collect();
    if let Some(p) = texts.keys().find(|p| !files.contains(*p)) {
        return Err(fail(format!("{p}: no such file in the configuration's last commit")));
    }
    let config = read(&WithFiles {
        commit: Commit {
            dir,
            commit: commit.clone(),
            files,
        },
        texts,
    })?;
    Ok(Committed { commit, config })
}

/// A TOML text with one setting set, everything else as it was, comments and layout included: the value at `path`
/// (its keys from the top of the file, through tables, inline tables and dotted keys alike) replaced, keeping the
/// spaces and the comment round it, or added at the end of its table, the tables on its path made where there are
/// none. A value of none takes the setting out. A path through a value that is no table is refused.
pub fn set_setting(text: &str, path: &[&str], value: Option<toml_edit::Value>) -> Result<String, String> {
    use toml_edit::{DocumentMut, Item, Table, TableLike};
    let Some((key, tables)) = path.split_last() else {
        return Err("no setting named".into());
    };
    let mut doc: DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| e.to_string())?;
    let mut table: &mut dyn TableLike = doc.as_table_mut();
    for (i, k) in tables.iter().enumerate() {
        if !table.contains_key(k) {
            if value.is_none() {
                return Ok(doc.to_string());
            }
            let mut t = Table::new();
            t.set_implicit(true);
            table.insert(k, Item::Table(t));
        }
        table = table
            .get_mut(k)
            .and_then(Item::as_table_like_mut)
            .ok_or_else(|| format!("{} is no table", tables[..=i].join(".")))?;
    }
    match value {
        None => {
            table.remove(key);
        }
        Some(mut v) => match table.get_mut(key) {
            Some(Item::Value(old)) => {
                *v.decor_mut() = old.decor().clone();
                *old = v;
            }
            Some(_) => return Err(format!("{} is a table, not a value", path.join("."))),
            None => {
                table.insert(key, Item::Value(v));
            }
        },
    }
    Ok(doc.to_string())
}

/// A list of inline tables, each with the keys and texts given in their order, one table to a line as a person writes
/// such a list in a file of the configuration (as `observation_tags`); `[]` when there are none.
pub fn table_list(rows: &[Vec<(&str, &str)>]) -> toml_edit::Value {
    let mut list = toml_edit::Array::new();
    for row in rows {
        let mut t = toml_edit::InlineTable::new();
        for (k, v) in row {
            t.insert(*k, toml_edit::Value::from(*v));
        }
        t.fmt();
        list.push(t);
    }
    if !list.is_empty() {
        for v in list.iter_mut() {
            v.decor_mut().set_prefix("\n  ");
            v.decor_mut().set_suffix("");
        }
        list.set_trailing_comma(true);
        list.set_trailing("\n");
    }
    toml_edit::Value::Array(list)
}

fn parse(files: &dyn Files, path: &str) -> Result<Option<toml::Table>, String> {
    match files.read(path)? {
        None => Ok(None),
        Some(t) => t.parse::<toml::Table>().map(Some).map_err(|e| format!("{path}: {e}")),
    }
}

/// Merges `over` into `base`: a table into a table key by key; any other value replaces the one before it whole.
fn merge(base: &mut toml::Table, over: toml::Table) {
    for (k, v) in over {
        match v {
            toml::Value::Table(o) => match base.get_mut(&k) {
                Some(toml::Value::Table(b)) => merge(b, o),
                _ => {
                    base.insert(k, toml::Value::Table(o));
                }
            },
            v => {
                base.insert(k, v);
            }
        }
    }
}

fn typed<T: serde::de::DeserializeOwned>(t: toml::Table, path: &str) -> Result<T, String> {
    toml::Value::Table(t).try_into().map_err(|e| format!("{path}: {e}"))
}

fn read(files: &dyn Files) -> Result<Config, Error> {
    let mut problems = Vec::new();
    let install = match parse(files, "install.toml") {
        Ok(Some(t)) => typed::<Install>(t, "install.toml").map_err(|e| problems.push(e)).ok(),
        Ok(None) => {
            problems.push("install.toml: no such file".to_string());
            None
        }
        Err(e) => {
            problems.push(e);
            None
        }
    };
    let mut projects = BTreeMap::new();
    for f in files.list("projects").map_err(fail)? {
        let Some(name) = f.strip_suffix(".toml") else { continue };
        match project(files, &format!("projects/{f}")) {
            Ok(p) => {
                projects.insert(name.to_string(), p);
            }
            Err(e) => problems.push(e),
        }
    }
    let mut rules = BTreeMap::new();
    for f in files.list("rules").map_err(fail)? {
        let Some(name) = f.strip_suffix(".md") else { continue };
        match files.read(&format!("rules/{f}")) {
            Ok(Some(t)) => {
                rules.insert(name.to_string(), t);
            }
            Ok(None) => {}
            Err(e) => problems.push(e),
        }
    }
    match install {
        Some(install) if problems.is_empty() => {
            let c = Config {
                install,
                projects,
                rules,
            };
            let problems = c.names();
            if problems.is_empty() {
                Ok(c)
            } else {
                Err(Error { problems })
            }
        }
        _ => Err(Error { problems }),
    }
}

/// One project file, merged onto the shared files it extends.
fn project(files: &dyn Files, path: &str) -> Result<Project, String> {
    let mut own = parse(files, path)?.ok_or_else(|| format!("{path}: no such file"))?;
    let extends: Vec<String> = match own.remove("extends") {
        None => Vec::new(),
        Some(v) => v.try_into().map_err(|e| format!("{path}: extends: {e}"))?,
    };
    let mut merged = toml::Table::new();
    for name in &extends {
        let shared = format!("shared/{name}.toml");
        let t = parse(files, &shared)?.ok_or_else(|| format!("{path}: extends {name}, but there is no {shared}"))?;
        if t.contains_key("extends") {
            return Err(format!("{shared}: a shared file extends nothing"));
        }
        merge(&mut merged, t);
    }
    merge(&mut merged, own);
    typed(merged, path)
}

impl Config {
    /// The project that is wf-agents itself, whose repository wf-agents is built from: its architect takes in the flow
    /// feedback of every project that belongs to wf-agents, and its faults and observations have a conversation of
    /// their own on the owner's page. None when no project is.
    pub fn wf_agents_project(&self) -> Option<String> {
        let own = env!("CARGO_PKG_REPOSITORY")
            .trim_end_matches('/')
            .trim_end_matches(".git");
        let own = own.strip_prefix("https://github.com/").unwrap_or(own);
        if own.is_empty() {
            return None;
        }
        self.projects
            .iter()
            .find(|(_, p)| p.repo.eq_ignore_ascii_case(own))
            .map(|(name, _)| name.clone())
    }

    /// Every name that points elsewhere in the configuration points at something there.
    fn names(&self) -> Vec<String> {
        let mut problems = Vec::new();
        let m = &self.install.machines;
        if !m.hosts.contains_key(&m.workstation) {
            problems.push(format!(
                "install.toml: machines.workstation names no host: {}",
                m.workstation
            ));
        }
        if let Some(t) = &m.timing
            && !m.hosts.contains_key(t)
        {
            problems.push(format!("install.toml: machines.timing names no host: {t}"));
        }
        for (name, l) in &self.install.agents.levels {
            if !self.install.agents.clis.contains_key(&l.cli) {
                problems.push(format!("install.toml: level {name} runs in no such CLI: {}", l.cli));
            }
        }
        if let Some(a) = &self.install.agents.architect
            && !self.install.agents.levels.contains_key(a)
        {
            problems.push(format!("install.toml: agents.architect names no level: {a}"));
        }
        if let Some(o) = &self.install.agents.observer
            && !self.install.agents.levels.contains_key(o)
        {
            problems.push(format!("install.toml: agents.observer names no level: {o}"));
        }
        if let Some(l) = &self.install.agents.librarian
            && !self.install.agents.levels.contains_key(l)
        {
            problems.push(format!("install.toml: agents.librarian names no level: {l}"));
        }
        if let Some(p) = &self.install.agents.picker
            && !self.install.agents.levels.contains_key(p)
        {
            problems.push(format!("install.toml: agents.picker names no level: {p}"));
        }
        if let Some(h) = &self.install.items.highest_level
            && !self.install.agents.levels.contains_key(h)
        {
            problems.push(format!("install.toml: items.highest_level names no level: {h}"));
        }
        for (name, p) in &self.projects {
            for r in &p.rules {
                if !self.rules.contains_key(r) {
                    problems.push(format!("projects/{name}.toml: no such rule: {r} (rules/{r}.md)"));
                }
            }
            // The constitution is a file of the project's repository, named by its path from the repository's root: a
            // path that goes up (`..`) or names no file (empty, `.`) is never read ([`read_at`]), so it is refused here.
            if let Some(c) = &p.constitution
                && (!c.is_relative()
                    || c.components().any(|x| matches!(x, std::path::Component::ParentDir))
                    || !c.components().any(|x| matches!(x, std::path::Component::Normal(_))))
            {
                problems.push(format!(
                    "projects/{name}.toml: constitution is not a path in the project's repository: {}",
                    c.display()
                ));
            }
            for o in &p.other_projects {
                if !self.projects.contains_key(o) {
                    problems.push(format!("projects/{name}.toml: no such project: {o}"));
                }
            }
            // A run on demand is run with the tests to run, which the workflow takes as the input `tests_input` names.
            if p.ci.on_demand && p.ci.tests_input.is_none() {
                problems.push(format!(
                    "projects/{name}.toml: ci.on_demand needs ci.tests_input, the input of the fast workflow that takes \
                     the tests to run"
                ));
            }
            // The input is passed by name to the workflow it runs: an empty name is no input.
            if p.ci.tests_input.as_deref().is_some_and(|i| i.trim().is_empty()) {
                problems.push(format!(
                    "projects/{name}.toml: ci.tests_input is empty; it names the workflow's input"
                ));
            }
            // The fast workflow is run by hand with the tests named, and its green means those tests passed. As one of
            // the full CI's workflows (named alone or with this project's repository) it would give a merge a check
            // the whole CI never ran.
            if p.ci.on_demand
                && p.ci
                    .full
                    .iter()
                    .any(|w| *w == p.ci.fast || *w == format!("{}:{}", p.repo, p.ci.fast))
            {
                problems.push(format!(
                    "projects/{name}.toml: ci.on_demand needs ci.fast to be a workflow other than the full CI's \
                     (ci.full names {})",
                    p.ci.fast
                ));
            }
            let mut tags = BTreeSet::new();
            for t in &p.observation_tags {
                if t.name.trim().is_empty() || t.name.trim() != t.name || !tags.insert(t.name.as_str()) {
                    problems.push(format!(
                        "projects/{name}.toml: observation_tags: a tag with no name, spaces around it, or a name twice: \
                         {:?}",
                        t.name
                    ));
                }
            }
        }
        problems
    }

    /// What does not fit the flow the engine runs: a counter's limit the flow has no counter for, a gate's need the
    /// configuration does not answer, a setting the flow reads that no project gives.
    pub fn check(&self, compiled: &flow::Compiled) -> Vec<String> {
        let mut problems = Vec::new();
        for (name, l) in &self.install.agents.levels {
            if l.effort.is_some() {
                problems.push(format!(
                    "install.toml: level {name} sets an effort: a level is a model, and each cell runs at the effort the \
                     flow declares for it"
                ));
            }
        }
        let counters: BTreeSet<&str> = compiled
            .graphs
            .iter()
            .flat_map(|g| g.counters.iter().map(|c| c.name.as_str()))
            .collect();
        for (name, p) in &self.projects {
            for c in p.limits.keys() {
                if !counters.contains(c.as_str()) {
                    problems.push(format!("projects/{name}.toml: limits: the flow has no counter {c}"));
                }
            }
            for s in &compiled.settings {
                if p.setting(s).is_none() {
                    problems.push(format!(
                        "projects/{name}.toml: the flow reads a setting no project has: {s}"
                    ));
                }
            }
        }
        problems
    }

    /// A level of work, with the CLI it runs in.
    pub fn level(&self, name: &str) -> Option<(&Level, &Cli)> {
        let l = self.install.agents.levels.get(name)?;
        Some((l, self.install.agents.clis.get(&l.cli)?))
    }

    /// The level one above the one named, in the order of the levels' names (by number when they are numbers); the
    /// top level is its own level above. A name that is no level is given back as it is.
    pub fn level_above(&self, name: &str) -> String {
        let names = self.levels_in_order();
        match names.iter().position(|n| *n == name) {
            Some(i) => names.get(i + 1).unwrap_or(&names[i]).to_string(),
            None => name.to_string(),
        }
    }

    /// Whether an item may be at a level: none above the configuration's highest level for items.
    pub fn item_level(&self, name: &str) -> Result<(), String> {
        let Some(highest) = &self.install.items.highest_level else {
            return Ok(());
        };
        let names = self.levels_in_order();
        let at = |n: &str| names.iter().position(|x| x.as_str() == n);
        match (at(name), at(highest)) {
            (Some(i), Some(h)) if i > h => Err(format!(
                "level {name} is above {highest}, the highest an item may be at (items.highest_level)"
            )),
            _ => Ok(()),
        }
    }

    /// The levels an item may be at, lowest first, each by its name: every level up to the highest for items.
    pub fn item_levels(&self) -> Vec<(&String, &Level)> {
        self.levels_in_order()
            .into_iter()
            .filter(|n| self.item_level(n).is_ok())
            .filter_map(|n| Some((n, self.install.agents.levels.get(n)?)))
            .collect()
    }

    /// The levels' names in their order: by number when they are numbers, by name otherwise.
    fn levels_in_order(&self) -> Vec<&String> {
        let mut names: Vec<&String> = self.install.agents.levels.keys().collect();
        names.sort_by_key(|n| (n.parse::<i64>().ok(), (*n).clone()));
        names
    }

    /// The lowest level, the cheapest the installation has: it repairs the JSON of a return the command line could not
    /// parse.
    pub fn lowest_level(&self) -> String {
        self.levels_in_order()
            .first()
            .map(|n| n.to_string())
            .unwrap_or_default()
    }

    /// The level the architect runs at, with the review of its item changes and the new architects that read a
    /// policy: the one the installation names, or else the top level.
    pub fn architect_level(&self) -> String {
        match &self.install.agents.architect {
            Some(a) => a.clone(),
            None => self.levels_in_order().last().map(|n| n.to_string()).unwrap_or_default(),
        }
    }

    /// The level the search of a project's observations runs at: the one the installation names, or else the lowest.
    pub fn observer_level(&self) -> String {
        match &self.install.agents.observer {
            Some(o) => o.clone(),
            None => self
                .levels_in_order()
                .first()
                .map(|n| n.to_string())
                .unwrap_or_default(),
        }
    }

    /// The level the pick of the architect's next matter runs at: the one the installation names, or else the lowest.
    pub fn picker_level(&self) -> String {
        match &self.install.agents.picker {
            Some(p) => p.clone(),
            None => self
                .levels_in_order()
                .first()
                .map(|n| n.to_string())
                .unwrap_or_default(),
        }
    }

    /// The level the reading of a work's history runs at: the one the installation names, or else the lowest.
    pub fn librarian_level(&self) -> String {
        match &self.install.agents.librarian {
            Some(l) => l.clone(),
            None => self
                .levels_in_order()
                .first()
                .map(|n| n.to_string())
                .unwrap_or_default(),
        }
    }

    /// The level a line's agents run at for an item of the given level: the advisor one level above the item's, the
    /// reader of a work's history at its own level whatever the item's, every other line at the item's.
    pub fn level_of_line(&self, item_level: &str, line: &str) -> String {
        if line == "advisor" {
            self.level_above(item_level)
        } else if line == "librarian" {
            self.librarian_level()
        } else {
            item_level.to_string()
        }
    }

    /// The settings a rule's text names, as `` `owner.language` `` or `` `project.under_test` ``, each with its value
    /// for the project, so the agent given the rule is given what it says.
    pub fn named_settings(&self, project: &str, text: &str) -> Vec<(String, String)> {
        let mut out = Vec::new();
        let install = serde_json::to_value(&self.install).unwrap_or_default();
        let project = self
            .projects
            .get(project)
            .and_then(|p| serde_json::to_value(p).ok())
            .unwrap_or_default();
        for name in text.split('`').skip(1).step_by(2) {
            if !name.contains('.') || name.contains(' ') || out.iter().any(|(n, _)| n == name) {
                continue;
            }
            let (root, rest) = match name.split_once('.') {
                Some(("project", rest)) => (&project, rest),
                Some((top, _)) if install.get(top).is_some() => (&install, name),
                _ => continue,
            };
            let mut v = root;
            let mut found = true;
            for part in rest.split('.') {
                match v.get(part) {
                    Some(x) => v = x,
                    None => {
                        found = false;
                        break;
                    }
                }
            }
            let value = match v {
                _ if !found => "none".to_string(),
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Null => "none".to_string(),
                x => x.to_string(),
            };
            out.push((name.to_string(), value));
        }
        out
    }
}

/// The database's file in a data directory.
pub fn database(dir: &Path) -> PathBuf {
    dir.join("db").join("wf-agents.sqlite")
}

/// The directory each work's worktree is made in: `worktrees/` in the data directory, which the data directory's git
/// leaves out.
pub fn worktrees(dir: &Path) -> PathBuf {
    dir.join("worktrees")
}

/// The commit the project's main is at in its checkout, as the last fetch left it (`origin/<main>`); an error saying
/// why when there is none: the checkout is missing, or main was never fetched into it. The checkout's own branch of
/// that name is not a fallback: no step updates it.
pub fn main_commit(p: &Project) -> Result<String, String> {
    let main = p.main_ref();
    // By the ref's full name, which a tag or branch of the checkout named like it does not shadow.
    let out = git_bytes(
        &p.checkout,
        &["rev-parse", "--verify", &format!("refs/remotes/{main}^{{commit}}")],
    )
    .map_err(|x| format!("{main} is not in the checkout {}: {x}", p.checkout.display()))?;
    Ok(String::from_utf8_lossy(&out).trim().to_string())
}

/// When a commit of the project's checkout was committed, in milliseconds since the epoch, which tells how old the text
/// read at it is.
pub fn committed_at(p: &Project, commit: &str) -> Result<i64, String> {
    let out = git(&p.checkout, &["show", "-s", "--format=%ct", commit])?;
    out.trim()
        .parse::<i64>()
        .map(|seconds| seconds * 1000)
        .map_err(|_| format!("git show {commit}: no commit time in {:?}", out.trim()))
}

/// What `git fetch origin` is given to bring the remote's branch `main` to `origin/<main>`: the full names of both
/// ends. The bare name would be looked up among the remote's tags as well, and a tag of that name would be fetched in
/// the branch's place, leaving `origin/<main>` where it was.
pub fn main_refspec(main: &str) -> String {
    format!("+refs/heads/{main}:refs/remotes/origin/{main}")
}

/// Brings the project's `origin/<main>` in its checkout to the remote's main now (`git fetch`). It moves that ref only:
/// no file of the checkout changes. Fetches in one checkout take turns ([`shared_git::git`]), so that works taking
/// their steps at the same time all succeed, as git turns a fetch down that moves a ref another has just moved.
pub fn fetch_main(p: &Project) -> Result<(), String> {
    shared_git::git(&p.checkout, &["fetch", "-q", "origin", &main_refspec(&p.main)])
        .map(|_| ())
        .map_err(|x| {
            format!(
                "main of {} could not be fetched in {}: {x}",
                p.repo,
                p.checkout.display()
            )
        })
}

/// The text of a file of the project as one commit of its checkout has it, which `path` names from the repository's
/// root. An error is one sentence that names the path and the commit and says why: the path is not in that commit, it
/// is not a file, it is not UTF-8 text, git failed.
pub fn read_at(p: &Project, commit: &str, path: &Path) -> Result<String, String> {
    use std::path::Component;
    let shown = path.display();
    let mut parts = Vec::new();
    for c in path.components() {
        match c {
            Component::Normal(x) => parts.push(x.to_string_lossy().into_owned()),
            Component::CurDir => {}
            _ => return Err(format!("{shown} at {commit} is not a path inside the repository")),
        }
    }
    let bytes = blob_object(&p.checkout, commit, &parts.join("/"))
        .and_then(|object| git_bytes(&p.checkout, &["cat-file", "blob", &object]))
        .map_err(|x| format!("{shown} cannot be read at {commit} in {}: {x}", p.checkout.display()))?;
    String::from_utf8(bytes).map_err(|_| format!("{shown} at {commit} is not UTF-8 text"))
}

/// The most links one path is followed through: a loop of links ends in an error, not a hang.
const MAX_LINKS: usize = 16;

/// What a path names at a commit.
enum Found {
    /// Nothing: this path, as far as it was walked, is not in the directory that should hold it.
    Missing(String),
    /// A file, as the git object that holds its text.
    File(String),
    /// A directory, as its git tree object, and the path it was reached at (empty for the repository's root).
    Dir { tree: String, at: String },
}

/// An entry of a directory at a commit.
struct Entry {
    mode: String,
    kind: String,
    object: String,
    name: String,
}

/// The entries of a git tree object, by their names exactly: no name is read as a pattern.
fn list_tree(repo: &Path, tree: &str) -> Result<Vec<Entry>, String> {
    let listed = git(repo, &["ls-tree", "-z", tree])?;
    let mut out = Vec::new();
    // Each entry is `<mode> <type> <object>\t<name>`.
    for entry in listed.split('\0').filter(|e| !e.is_empty()) {
        let (meta, name) = entry
            .split_once('\t')
            .ok_or_else(|| format!("git ls-tree gave {entry:?}"))?;
        let mut m = meta.split(' ');
        let mut next = || m.next().unwrap_or_default().to_string();
        out.push(Entry {
            mode: next(),
            kind: next(),
            object: next(),
            name: name.to_string(),
        });
    }
    Ok(out)
}

/// What `path` names at `commit` of the repository at `repo`. Git stores a link as a file that holds the path it points
/// to, and a read of the checkout's files followed it, at the end of the path and in the middle of it alike, so links
/// are followed here too: the path is walked one name at a time from the root, each name looked up in its directory's
/// entries by that name exactly (git's pathspecs, which give `*`, `[` and `:` a meaning, are not used), and a name that
/// is a link is replaced by the path it points to, however many links lead to the file. A target outside the
/// repository, a loop, a name in the middle that is no directory and an entry that is neither a file, a link nor a
/// directory are errors that say which.
fn resolve(repo: &Path, commit: &str, path: &str) -> Result<Found, String> {
    use std::collections::VecDeque;
    let root = git(repo, &["rev-parse", "--verify", &format!("{commit}^{{tree}}")])?;
    // The directories walked so far, real ones at the commit: their names, and their trees after the root's.
    let mut names: Vec<String> = Vec::new();
    let mut trees = vec![root.trim().to_string()];
    let mut ahead: VecDeque<String> = path.split('/').map(str::to_string).collect();
    // The last link followed and the path it points to, which a name that leaves the repository after it is blamed on.
    let mut via: Option<(String, String)> = None;
    let mut followed = 0;
    while let Some(name) = ahead.pop_front() {
        match name.as_str() {
            "" | "." => continue,
            ".." => {
                if names.pop().is_some() {
                    trees.pop();
                    continue;
                }
                return Err(match via {
                    Some((link, target)) => format!("the link {link} points outside the repository, to {target}"),
                    None => format!("{path} leads outside the repository at {commit}"),
                });
            }
            _ => {}
        }
        let at = if names.is_empty() {
            name.clone()
        } else {
            format!("{}/{name}", names.join("/"))
        };
        let here = trees.last().map(String::as_str).unwrap_or_default();
        let Some(entry) = list_tree(repo, here)?.into_iter().find(|e| e.name == name) else {
            return Ok(Found::Missing(at));
        };
        if entry.mode == "120000" {
            followed += 1;
            if followed > MAX_LINKS {
                return Err(format!(
                    "{path} is a link that leads through more than {MAX_LINKS} links at {commit}"
                ));
            }
            let target = String::from_utf8(git_bytes(repo, &["cat-file", "blob", &entry.object])?)
                .map_err(|_| format!("the link {at} does not hold a path"))?;
            if target.starts_with('/') {
                return Err(format!("the link {at} points outside the repository, to {target}"));
            }
            // The link stands where its name stood: its target is read from the directory that held it.
            for part in target.split('/').rev() {
                ahead.push_front(part.to_string());
            }
            via = Some((at, target));
        } else if entry.kind == "tree" {
            names.push(name);
            trees.push(entry.object);
        } else if entry.kind != "blob" {
            return Err(format!("{at} is not a file at {commit}"));
        } else if ahead.is_empty() {
            return Ok(Found::File(entry.object));
        } else {
            return Err(format!("{at} is a file, not a directory, at {commit}"));
        }
    }
    Ok(Found::Dir {
        tree: trees.pop().unwrap_or_default(),
        at: names.join("/"),
    })
}

/// The git object of the file `path` names at `commit`; an error that says why when it names none.
fn blob_object(repo: &Path, commit: &str, path: &str) -> Result<String, String> {
    match resolve(repo, commit, path)? {
        Found::File(object) => Ok(object),
        Found::Missing(at) => Err(format!("path '{at}' does not exist in '{commit}'")),
        Found::Dir { at, .. } if at.is_empty() => Err(format!(
            "{path} leads to the repository's root, which is not a file at {commit}"
        )),
        Found::Dir { at, .. } => Err(format!("{at} is not a file at {commit}")),
    }
}

/// What a project says of itself as one commit has it, which agents are given as the project guide: its AGENTS.md,
/// the index of its records under the design paths (each file's path and first line), and the full text of the files
/// `cites` names by their paths. It is read with git from the repository at `repo`, so nothing in a worktree's
/// files changes it: not what is uncommitted, not what the worktree's branch committed beyond that commit.
pub fn project_guide_at(
    repo: &Path,
    commit: &str,
    design: &[String],
    cites: &str,
) -> Result<serde_json::Value, String> {
    let agents = match resolve(repo, commit, "AGENTS.md")
        .map_err(|x| format!("AGENTS.md cannot be read at {commit} in {}: {x}", repo.display()))?
    {
        Found::File(object) => blob_texts(repo, &[object.as_str()])?.pop().unwrap_or_default(),
        Found::Missing(_) => String::new(),
        Found::Dir { .. } => return Err(format!("AGENTS.md is not a file at {commit} in {}", repo.display())),
    };
    let mut found = BTreeMap::new();
    for dir in design.iter().filter(|d| d.ends_with('/')) {
        records_under(repo, commit, dir, &mut found)?;
    }
    let objects: Vec<&str> = found.values().map(String::as_str).collect();
    let texts = blob_texts(repo, &objects)?;
    Ok(guide(agents, found.keys().cloned().zip(texts).collect(), cites))
}

/// Adds to `found` the Markdown files under the directory `dir` of a commit, each as its path and the git object that
/// holds its text; a directory that is not there adds none. The files are those a walk of the checkout found, so a
/// link is followed as that walk followed it: a link to a file is a file at the link's path, a link to a directory is
/// that directory at the link's path, and a link that cannot be followed is an error that names it, never left out
/// without a word.
fn records_under(repo: &Path, commit: &str, dir: &str, found: &mut BTreeMap<String, String>) -> Result<(), String> {
    let dir = dir.trim_end_matches('/');
    let at = resolve(repo, commit, dir)
        .map_err(|x| format!("{dir} cannot be read at {commit} in {}: {x}", repo.display()))?;
    if let Found::Dir { tree, .. } = at {
        let prefix = if dir.is_empty() {
            String::new()
        } else {
            format!("{dir}/")
        };
        walk_records(repo, commit, &prefix, &tree, &mut vec![tree.clone()], found)?;
    }
    Ok(())
}

/// The Markdown files of the directory `tree` stands for at the path `prefix`, added to `found`
/// ([`records_under`]); `on_path` holds the trees of the directories this walk is in, the one it started from and each
/// that a link led into, so that a link that leads back into one of them ends the walk in an error at once: it would
/// otherwise be followed again from the same place, once for every link there is, however many levels deep.
fn walk_records(
    repo: &Path,
    commit: &str,
    prefix: &str,
    tree: &str,
    on_path: &mut Vec<String>,
    found: &mut BTreeMap<String, String>,
) -> Result<(), String> {
    let listed = git(repo, &["ls-tree", "-r", "-z", tree])?;
    // Each entry is `<mode> <type> <object>\t<path>`, the path from `tree`.
    for entry in listed.split('\0') {
        let Some((meta, rel)) = entry.split_once('\t') else {
            continue;
        };
        let mut m = meta.split(' ');
        let (mode, kind, object) = (m.next(), m.next(), m.next().unwrap_or_default());
        let path = format!("{prefix}{rel}");
        if kind != Some("blob") {
            continue;
        }
        if mode != Some("120000") {
            if path.ends_with(".md") {
                found.insert(path, object.to_string());
            }
            continue;
        }
        let fail = |x: String| format!("{path} cannot be read at {commit} in {}: {x}", repo.display());
        match resolve(repo, commit, &path).map_err(fail)? {
            Found::File(object) if path.ends_with(".md") => {
                found.insert(path, object);
            }
            Found::Dir { tree, .. } => {
                if on_path.contains(&tree) {
                    return Err(fail(
                        "it leads back into a directory it is read through, a loop of links".into(),
                    ));
                }
                on_path.push(tree.clone());
                walk_records(repo, commit, &format!("{path}/"), &tree, on_path, found)?;
                on_path.pop();
            }
            Found::Missing(at) if path.ends_with(".md") => {
                return Err(fail(format!("path '{at}' does not exist in '{commit}'")));
            }
            Found::File(_) | Found::Missing(_) => {}
        }
    }
    Ok(())
}

/// The project guide of an AGENTS.md and the files of the records, each by its path: the index in the order of the
/// paths, each file's first line that is not empty, and the files `cites` names in full.
fn guide(agents: String, mut nodes: Vec<(String, String)>, cites: &str) -> serde_json::Value {
    use serde_json::{Map, Value, json};
    nodes.sort();
    let mut index = Map::new();
    let mut cited = Map::new();
    for (rel, body) in nodes {
        let first = body
            .lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or_default()
            .to_string();
        if cites.contains(&rel) {
            cited.insert(rel.clone(), Value::String(body));
        }
        index.insert(rel, Value::String(first));
    }
    json!({ "agents_md": agents, "records": index, "cited": cited })
}

/// The Markdown files under a directory of a repository as one commit has them, each by its path, in the order of the
/// paths: read with git, so that neither the worktree's files nor what it committed beyond that commit change them.
/// Links are followed as [`records_under`] says.
pub fn files_at(repo: &Path, commit: &str, dir: &str) -> Result<Vec<(String, String)>, String> {
    let mut found = BTreeMap::new();
    records_under(repo, commit, dir, &mut found)?;
    let objects: Vec<&str> = found.values().map(String::as_str).collect();
    let texts = blob_texts(repo, &objects)?;
    Ok(found.into_keys().zip(texts).collect())
}

/// The texts of git objects of a repository, in the order of their names, read by one run of git.
fn blob_texts(repo: &Path, objects: &[&str]) -> Result<Vec<String>, String> {
    use std::io::Write;
    use std::process::Stdio;
    if objects.is_empty() {
        return Ok(Vec::new());
    }
    let mut child = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["cat-file", "--batch"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("git cat-file: {e}"))?;
    let names: String = objects.iter().map(|o| format!("{o}\n")).collect();
    let mut stdin = child.stdin.take().ok_or("git cat-file: no standard input")?;
    // Written beside the reading, so that neither pipe fills while the other waits.
    let writer = std::thread::spawn(move || stdin.write_all(names.as_bytes()));
    let out = child.wait_with_output().map_err(|e| format!("git cat-file: {e}"))?;
    writer
        .join()
        .map_err(|_| "git cat-file: writing the names failed".to_string())?
        .map_err(|e| format!("git cat-file: {e}"))?;
    if !out.status.success() {
        return Err(format!("git cat-file: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    // Each object is `<name> <type> <size>\n<content>\n`; a missing one is `<name> missing\n`.
    let mut rest: &[u8] = &out.stdout;
    let mut texts = Vec::new();
    while let Some(end) = rest.iter().position(|b| *b == b'\n') {
        let header = String::from_utf8_lossy(&rest[..end]).into_owned();
        rest = &rest[end + 1..];
        let size = header
            .rsplit(' ')
            .next()
            .and_then(|s| s.parse::<usize>().ok())
            .filter(|_| !header.ends_with(" missing"));
        let Some(size) = size.filter(|s| *s <= rest.len()) else {
            return Err(format!("git cat-file: no such object: {header}"));
        };
        texts.push(String::from_utf8_lossy(&rest[..size]).into_owned());
        rest = rest.get(size + 1..).unwrap_or_default();
    }
    Ok(texts)
}

#[cfg(test)]
mod tests {
    use super::*;

    const INSTALL: &str = r#"
[owner]
language = "Chinese"

[items]
title_max = 30

[machines]
workstation = "laptop"
heavy = "builds and tests"
poll_interval = 300

[machines.hosts.laptop]
slots = 2

[agents.clis.claude]
kind = "claude"
command = "claude"

[agents.levels.senior]
desc = "New decisions"
cli = "claude"
model = "m"

[gh]
allow = ["pr view"]

[query]
max_rows = 100
timeout_ms = 2000
"#;

    const SHARED: &str = r#"
design_goal = "correct, then fast"
test_paths = ["tests/"]
review_list = ["one", "two"]
[ci]
fast = "ci"
full = ["ci"]
merge_checks = ["check"]
tests_input = "tests"
generated_artifact = "generated"
[limits]
fixes = 5
"#;

    const PROJECT: &str = r#"
extends = ["common"]
repo = "o/p"
checkout = "/tmp/p"
review_list = ["three"]
rules = ["ground"]
[ci]
merge_checks = ["check", "lint"]
[limits]
asks = 9
"#;

    struct Mem(BTreeMap<&'static str, &'static str>);

    impl Files for Mem {
        fn read(&self, path: &str) -> Result<Option<String>, String> {
            Ok(self.0.get(path).map(|s| s.to_string()))
        }

        fn list(&self, dir: &str) -> Result<Vec<String>, String> {
            let prefix = format!("{dir}/");
            Ok(self
                .0
                .keys()
                .filter_map(|k| k.strip_prefix(prefix.as_str()))
                .map(str::to_string)
                .collect())
        }
    }

    fn files(extra: &[(&'static str, &'static str)]) -> Mem {
        let mut m = BTreeMap::from([
            ("install.toml", INSTALL),
            ("shared/common.toml", SHARED),
            ("projects/p.toml", PROJECT),
            (
                "rules/ground.md",
                "Write in `owner.language`; test against `project.oracles`; `x.y` is no setting.",
            ),
        ]);
        m.extend(extra.iter().copied());
        Mem(m)
    }

    // A project starts from what it extends: tables merge key by key, and a list replaces the one before it whole.
    #[test]
    fn a_project_merges_onto_what_it_extends() {
        let c = read(&files(&[])).unwrap();
        let p = &c.projects["p"];
        assert_eq!(p.design_goal, "correct, then fast");
        assert_eq!(p.review_list, ["three"]);
        assert_eq!(p.ci.fast, "ci");
        assert_eq!(p.ci.merge_checks, ["check", "lint"]);
        assert_eq!(p.ci.tests_input.as_deref(), Some("tests"));
        assert_eq!(p.ci.generated_artifact.as_deref(), Some("generated"));
        assert_eq!(
            p.limits,
            BTreeMap::from([("asks".to_string(), 9), ("fixes".to_string(), 5)])
        );
        assert_eq!(p.main, "main");
        assert_eq!(p.setting("full_ci").unwrap()["merge_checks"][1], "lint");
        assert_eq!(p.setting("deploy"), Some(serde_json::Value::Null));
        assert_eq!(p.has("under_test"), None);
        assert_eq!(p.has("flow_feedback"), Some(true));
        // What a project does not set reads as nothing: no records, nothing under test.
        assert_eq!(p.setting("records"), Some(serde_json::Value::Null));
        assert_eq!(p.setting("under_test"), Some(serde_json::Value::Null));
        assert!(p.setting("nope").is_none());
        // The reasoning system is the installation's one copy, in the data directory unless one is named.
        assert_eq!(
            c.install.reasoning.system(Path::new("/data")),
            Path::new("/data/reasoning-system")
        );
        // Agents run with a configuration directory of their own, inside the data directory unless one is set.
        let cli = &c.install.agents.clis["claude"];
        assert_eq!(cli.home(Path::new("/data"), "claude"), Path::new("/data/agents/claude"));
        let set = Cli {
            config_dir: Some(PathBuf::from("/elsewhere")),
            ..cli.clone()
        };
        assert_eq!(set.home(Path::new("/data"), "claude"), Path::new("/elsewhere"));
    }

    // A line that only reads, as every line of an architect, is refused every tool that edits a file and every git
    // command that changes the repository, and may still read with git; the engineer is refused neither.
    #[test]
    fn a_line_that_only_reads_edits_nothing_and_changes_no_repository() {
        let p = Permissions::default();
        for line in [
            "architect",
            "architect_review",
            "observer",
            "picker",
            "review",
            "advisor",
            "librarian",
        ] {
            let refused = p.refused(line);
            for t in EDIT_TOOLS {
                assert!(refused.iter().any(|r| r == t), "{line}: {t}");
            }
            for g in GIT_CHANGES {
                assert!(refused.contains(&format!("Bash(git {g} *)")), "{line}: git {g}");
            }
            for g in ["show", "diff", "log", "fetch"] {
                assert!(!refused.contains(&format!("Bash(git {g} *)")), "{line}: git {g}");
            }
        }
        let engineer = p.refused("engineer");
        assert!(!engineer.iter().any(|r| r == "Edit" || r == "Bash(git commit *)"));
    }

    // The level above a level goes by the levels' names, by number when they are numbers; the top is its own level
    // above. The advisor runs one level above the item's, every other line at the item's.
    #[test]
    fn the_advisor_runs_one_level_up() {
        let mut c = read(&files(&[])).unwrap();
        let senior = c.install.agents.levels["senior"].clone();
        c.install.agents.levels = ["1", "2", "10"]
            .iter()
            .map(|n| (n.to_string(), senior.clone()))
            .collect();
        assert_eq!(c.level_above("1"), "2");
        assert_eq!(c.level_above("2"), "10");
        assert_eq!(c.level_above("10"), "10");
        assert_eq!(c.level_above("nine"), "nine");
        assert_eq!(c.level_of_line("1", "advisor"), "2");
        assert_eq!(c.level_of_line("1", "engineer"), "1");
        assert_eq!(c.level_of_line("10", "advisor"), "10");
        // The architect runs at the top level unless the installation names another, and the search of observations
        // at the lowest.
        assert_eq!(c.architect_level(), "10");
        c.install.agents.architect = Some("2".into());
        assert_eq!(c.architect_level(), "2");
        assert_eq!(c.observer_level(), "1");
        c.install.agents.observer = Some("2".into());
        assert_eq!(c.observer_level(), "2");
        // The reader of a work's history runs at the lowest level whatever the item's, unless the installation names
        // another.
        assert_eq!(c.librarian_level(), "1");
        assert_eq!(c.level_of_line("10", "librarian"), "1");
        c.install.agents.librarian = Some("2".into());
        assert_eq!(c.level_of_line("10", "librarian"), "2");
        // An item may be at any level unless the installation names the highest; its advisor may still run above it.
        assert_eq!(c.item_level("10"), Ok(()));
        c.install.items.highest_level = Some("2".into());
        assert_eq!(c.item_level("2"), Ok(()));
        assert_eq!(
            c.item_level("10"),
            Err("level 10 is above 2, the highest an item may be at (items.highest_level)".into())
        );
        assert_eq!(c.level_of_line("2", "advisor"), "10");
        // The levels an item may be at are those up to the highest, lowest first.
        let names: Vec<&String> = c.item_levels().into_iter().map(|l| l.0).collect();
        assert_eq!(names, ["1", "2"]);
    }

    // A setting is set where it is, keeping the spaces and the comment round it; added at the end of its table, or in
    // a table made for it; set inside an inline table and at the top of the file alike; or taken out; and every other
    // line, comments included, stays as it was.
    #[test]
    fn a_setting_is_set_where_it_is_and_the_rest_stays() {
        use toml_edit::Value;
        let text = "# The installation.\n[items]\ntitle_max = 20\n\n[agents.levels.4]\n# The top tier.\nmodel = \"a\"  # the best\n";
        assert_eq!(
            set_setting(text, &["agents", "levels", "4", "model"], Some(Value::from("b"))).unwrap(),
            "# The installation.\n[items]\ntitle_max = 20\n\n[agents.levels.4]\n# The top tier.\nmodel = \"b\"  # the best\n"
        );
        let added = set_setting(text, &["items", "highest_level"], Some(Value::from("3"))).unwrap();
        assert_eq!(
            added,
            "# The installation.\n[items]\ntitle_max = 20\nhighest_level = \"3\"\n\n[agents.levels.4]\n# The top tier.\nmodel = \"a\"  # the best\n"
        );
        assert_eq!(set_setting(&added, &["items", "highest_level"], None).unwrap(), text);
        assert_eq!(set_setting(text, &["owner", "language"], None).unwrap(), text);
        let made = set_setting(text, &["agents", "context_limit"], Some(Value::from(400_000))).unwrap();
        assert!(
            made.starts_with("# The installation.\n[items]\ntitle_max = 20\n"),
            "{made}"
        );
        let parsed: toml::Table = made.parse().unwrap();
        assert_eq!(parsed["agents"]["context_limit"].as_integer(), Some(400_000));
        assert_eq!(parsed["agents"]["levels"]["4"]["model"].as_str(), Some("a"));
        // A project's file: a key at its top, a value in an inline table, and a list replaced whole.
        let project = "# A project.\nrepo = \"o/p\"\nlimits = { asks = 9, fixes = 3 }\n\n[ci]\nfast = \"ci\"\n";
        let off = set_setting(project, &["flow_feedback"], Some(Value::from(false))).unwrap();
        assert_eq!(
            off,
            "# A project.\nrepo = \"o/p\"\nlimits = { asks = 9, fixes = 3 }\nflow_feedback = false\n\n[ci]\nfast = \"ci\"\n"
        );
        assert_eq!(
            set_setting(project, &["limits", "asks"], Some(Value::from(12))).unwrap(),
            "# A project.\nrepo = \"o/p\"\nlimits = { asks = 12, fixes = 3 }\n\n[ci]\nfast = \"ci\"\n"
        );
        let tags = table_list(&[
            vec![("name", "ui"), ("desc", "What the owner sees")],
            vec![("name", "cost"), ("desc", "Tokens \"spent\"")],
        ]);
        let tagged = set_setting(project, &["observation_tags"], Some(tags)).unwrap();
        assert!(
            tagged.ends_with(
                "observation_tags = [\n  { name = \"ui\", desc = \"What the owner sees\" },\n  \
                 { name = \"cost\", desc = 'Tokens \"spent\"' },\n]\n\n[ci]\nfast = \"ci\"\n"
            ),
            "{tagged}"
        );
        let parsed: toml::Table = tagged.parse().unwrap();
        assert_eq!(parsed["observation_tags"][1]["desc"].as_str(), Some("Tokens \"spent\""));
        let none = set_setting(&tagged, &["observation_tags"], Some(table_list(&[]))).unwrap();
        assert!(
            none.ends_with("observation_tags = []\n\n[ci]\nfast = \"ci\"\n"),
            "{none}"
        );
        assert!(set_setting(project, &["repo", "name"], Some(Value::from("x"))).is_err());
        assert!(set_setting(project, &["ci"], Some(Value::from("x"))).is_err());
        assert!(set_setting("[items\n", &["items", "title_max"], Some(Value::from(1))).is_err());
    }

    // `claude` keeps a session's prompt in its cache an hour after the session's last call.
    #[test]
    fn claudes_prompt_cache_lives_an_hour() {
        assert_eq!(CliKind::Claude.cache_lifetime(), Some(60 * 60 * 1000));
    }

    // A model is one `claude` takes: a family's alias or a model's full name, with the larger context or not; a name
    // in another form, or of a family it does not know, is not.
    #[test]
    fn a_model_is_one_the_cli_knows() {
        for m in [
            "opus",
            "sonnet[1m]",
            "claude-opus-5-5",
            "claude-fable-5-1",
            "claude-haiku-5-5",
            "claude-opus-4-5-20251101",
            "claude-3-5-sonnet-20241022",
            "claude-sonnet-4-6[1m]",
        ] {
            assert!(CliKind::Claude.knows_model(m), "{m}");
        }
        for m in [
            "",
            "opus5",
            "claude-opus",
            "claude-sonet-5-5",
            "claude-opus-5-5-sonnet",
            "claude-opus-5--5",
            "gpt-5",
            "claude-opus-5-5 ",
            "claude-opus-five",
        ] {
            assert!(!CliKind::Claude.knows_model(m), "{m}");
        }
    }

    // A field the types do not know, a missing one, or a name that points nowhere is an error, all of them at once.
    #[test]
    fn what_does_not_fit_the_types_or_points_nowhere_is_refused() {
        let e = read(&files(&[("projects/q.toml", "repo = \"o/q\"\ncolour = \"red\"")])).unwrap_err();
        assert!(
            e.problems
                .iter()
                .any(|p| p.contains("projects/q.toml") && p.contains("colour")),
            "{e}"
        );
        let e = read(&files(&[("projects/q.toml", "extends = [\"none\"]")])).unwrap_err();
        assert!(e.to_string().contains("there is no shared/none.toml"), "{e}");
        let e = read(&files(&[(
            "projects/q.toml",
            "extends = [\"common\"]\nrepo = \"o/q\"\ncheckout = \"/q\"\nrules = [\"nope\"]\nother_projects = [\"r\"]",
        )]))
        .unwrap_err();
        assert!(e.to_string().contains("no such rule: nope"), "{e}");
        assert!(e.to_string().contains("no such project: r"), "{e}");
        let mut m = files(&[]);
        m.0.insert("install.toml", "[owner]\nlanguage = 1");
        assert!(read(&m).is_err());
    }

    // The CI runs on demand only where the fast workflow takes the tests to run: off unless set, and set without the
    // input that names them, it is refused.
    #[test]
    fn a_ci_on_demand_needs_the_input_that_takes_the_tests_to_run() {
        let own = |ci: &'static str| {
            let text: &'static str = Box::leak(
                format!(
                    "repo = \"o/q\"\ncheckout = \"/q\"\ndesign_goal = \"d\"\ntest_paths = [\"tests/\"]\n[ci]\n\
                     fast = \"fast\"\nfull = [\"ci\"]\nmerge_checks = [\"check\"]\n{ci}"
                )
                .into_boxed_str(),
            );
            read(&files(&[("projects/q.toml", text)]))
        };
        // The project that extends the shared file, which names the input, does not run on demand unless it says so.
        assert!(!read(&files(&[])).unwrap().projects["p"].ci.on_demand);
        let c = own("tests_input = \"tests\"\non_demand = true\n").unwrap();
        assert!(c.projects["q"].ci.on_demand);
        assert!(!own("").unwrap().projects["q"].ci.on_demand);
        let e = own("on_demand = true\n").unwrap_err().to_string();
        assert!(e.contains("projects/q.toml: ci.on_demand needs ci.tests_input"), "{e}");
    }

    // The architect's level is one of the levels, and a project's constitution a path in its repository; anything else
    // is refused.
    #[test]
    fn the_architect_s_level_and_a_constitution_point_somewhere() {
        let mut m = files(&[(
            "projects/q.toml",
            "extends = [\"common\"]\nrepo = \"o/q\"\ncheckout = \"/q\"\nconstitution = \"CONSTITUTION.md\"",
        )]);
        let named = format!("{INSTALL}\n[agents]\narchitect = \"senior\"\n");
        m.0.insert("install.toml", Box::leak(named.into_boxed_str()));
        let c = read(&m).unwrap();
        assert_eq!(c.architect_level(), "senior");
        assert_eq!(
            c.projects["q"].constitution.as_deref(),
            Some(Path::new("CONSTITUTION.md"))
        );
        let mut m = files(&[
            (
                "projects/q.toml",
                "extends = [\"common\"]\nrepo = \"o/q\"\ncheckout = \"/q\"\nconstitution = \"/q/CONSTITUTION.md\"",
            ),
            // Paths no read of the repository can follow: one that goes up, and one that is the root itself.
            (
                "projects/up.toml",
                "extends = [\"common\"]\nrepo = \"o/up\"\ncheckout = \"/up\"\nconstitution = \"../CONSTITUTION.md\"",
            ),
            (
                "projects/root.toml",
                "extends = [\"common\"]\nrepo = \"o/root\"\ncheckout = \"/root\"\nconstitution = \".\"",
            ),
        ]);
        let unnamed = format!("{INSTALL}\n[agents]\narchitect = \"9\"\n");
        m.0.insert("install.toml", Box::leak(unnamed.into_boxed_str()));
        let e = read(&m).unwrap_err();
        assert!(e.to_string().contains("agents.architect names no level: 9"), "{e}");
        assert!(
            e.to_string().contains(
                "projects/q.toml: constitution is not a path in the project's repository: /q/CONSTITUTION.md"
            ),
            "{e}"
        );
        for bad in [
            "projects/up.toml: constitution is not a path in the project's repository: ../CONSTITUTION.md",
            "projects/root.toml: constitution is not a path in the project's repository: .",
        ] {
            assert!(e.to_string().contains(bad), "{bad}: {e}");
        }
    }

    // A project's tags for observations each have a name of their own, and the observer's level is one of the levels.
    #[test]
    fn the_observation_tags_and_the_observer_s_level_point_somewhere() {
        let tagged = "extends = [\"common\"]\nrepo = \"o/q\"\ncheckout = \"/q\"\nobservation_tags = [\
                      { name = \"perf\", desc = \"How fast it runs\" }, { name = \"docs\", desc = \"What it says\" }]";
        let mut m = files(&[("projects/q.toml", tagged)]);
        let named = format!("{INSTALL}\n[agents]\nobserver = \"senior\"\n");
        m.0.insert("install.toml", Box::leak(named.into_boxed_str()));
        let c = read(&m).unwrap();
        let names: Vec<&str> = c.projects["q"]
            .observation_tags
            .iter()
            .map(|t| t.name.as_str())
            .collect();
        assert_eq!(names, ["perf", "docs"]);
        assert_eq!(c.observer_level(), "senior");
        let twice = "extends = [\"common\"]\nrepo = \"o/q\"\ncheckout = \"/q\"\nobservation_tags = [\
                     { name = \"perf\", desc = \"a\" }, { name = \"perf\", desc = \"b\" }]";
        let mut m = files(&[("projects/q.toml", twice)]);
        let unnamed = format!("{INSTALL}\n[agents]\nobserver = \"9\"\nlibrarian = \"8\"\n");
        m.0.insert("install.toml", Box::leak(unnamed.into_boxed_str()));
        let e = read(&m).unwrap_err().to_string();
        assert!(e.contains("agents.observer names no level: 9"), "{e}");
        assert!(e.contains("agents.librarian names no level: 8"), "{e}");
        assert!(
            e.contains("projects/q.toml: observation_tags: a tag with no name"),
            "{e}"
        );
    }

    // A rule is given with the value of each setting it names.
    #[test]
    fn a_rule_comes_with_the_settings_it_names() {
        let c = read(&files(&[])).unwrap();
        let named = c.named_settings("p", &c.rules["ground"]);
        assert_eq!(
            named,
            [
                ("owner.language".to_string(), "Chinese".to_string()),
                ("project.oracles".to_string(), "none".to_string()),
            ]
        );
    }

    // A project lists the workflows its measure tool runs as `[[ci.measure]]` tables. One that lists none reads as
    // `none` in the settings an agent is shown, one that lists some as the list; a table with a field the list does
    // not know, or with no workflow, is refused.
    #[test]
    fn a_project_lists_the_workflows_the_measure_tool_runs() {
        const MEASURING: &str = "extends = [\"common\"]\nrepo = \"o/q\"\ncheckout = \"/q\"\n\
            [[ci.measure]]\nworkflow = \"measure.yml\"\ninputs = [\"command\", \"base\"]\nartifact = \"measurement\"\n\
            [[ci.measure]]\nworkflow = \"other.yml\"\n";
        const SETTING: &str = "`project.ci.measure`";
        let none = read(&files(&[])).unwrap();
        assert!(none.projects["p"].ci.measure.is_empty());
        assert_eq!(
            none.named_settings("p", SETTING),
            [("project.ci.measure".to_string(), "none".to_string())]
        );
        let c = read(&files(&[("projects/q.toml", MEASURING)])).unwrap();
        assert_eq!(
            c.projects["q"].ci.measure,
            [
                MeasureWorkflow {
                    workflow: "measure.yml".to_string(),
                    inputs: vec!["command".to_string(), "base".to_string()],
                    artifact: Some("measurement".to_string()),
                },
                MeasureWorkflow {
                    workflow: "other.yml".to_string(),
                    inputs: Vec::new(),
                    artifact: None,
                },
            ]
        );
        assert_eq!(
            c.named_settings("q", SETTING),
            [(
                "project.ci.measure".to_string(),
                "[{\"workflow\":\"measure.yml\",\"inputs\":[\"command\",\"base\"],\"artifact\":\"measurement\"},\
                 {\"workflow\":\"other.yml\",\"inputs\":[]}]"
                    .to_string()
            )]
        );
        for (table, word) in [
            ("workflow = \"m.yml\"\ncolour = \"red\"", "colour"),
            ("inputs = [\"command\"]", "workflow"),
        ] {
            let project =
                format!("extends = [\"common\"]\nrepo = \"o/q\"\ncheckout = \"/q\"\n[[ci.measure]]\n{table}\n");
            let project: &'static str = Box::leak(project.into_boxed_str());
            let e = read(&files(&[("projects/q.toml", project)])).unwrap_err();
            assert!(
                e.problems
                    .iter()
                    .any(|p| p.contains("projects/q.toml") && p.contains(word)),
                "{e}"
            );
        }
    }

    // The engine reads the last commit; the working tree's changes count only once committed.
    #[test]
    fn the_engine_reads_only_what_is_committed() {
        let dir = std::env::temp_dir().join(format!("wf-agents-config-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for (path, text) in files(&[]).0 {
            let p = dir.join(path);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
        }
        let run = |args: &[&str]| git(&dir, args).unwrap();
        run(&["init", "-q"]);
        run(&["add", "-A"]);
        run(&["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "first"]);
        std::fs::write(dir.join("projects/p.toml"), PROJECT.replace("o/p", "o/changed")).unwrap();
        let c = committed(&dir).unwrap();
        assert_eq!(c.config.projects["p"].repo, "o/p");
        assert_eq!(c.commit.len(), 40);
        assert_eq!(working(&dir).unwrap().projects["p"].repo, "o/changed");
        let _ = std::fs::remove_dir_all(&dir);
    }

    // The project guide of a commit is read from the commit alone, whatever the worktree's files say.
    #[test]
    fn the_guide_of_a_commit_is_what_the_commit_holds() {
        let dir = std::env::temp_dir().join(format!("wf-agents-guide-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let put = |path: &str, text: &str| {
            let p = dir.join(path);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
        };
        let run = |args: &[&str]| git(&dir, args).unwrap();
        put("AGENTS.md", "How to work.\n");
        put("design/a.md", "\nDecision: a.\nMore.\n");
        put("design/a/b.md", "Decision: b.\n");
        put("design/notes.txt", "not a node\n");
        put("other/c.md", "not in the design paths\n");
        run(&["init", "-q"]);
        run(&["add", "-A"]);
        run(&["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "first"]);
        let first = run(&["rev-parse", "HEAD"]).trim().to_string();
        let design = ["design/".to_string()];
        put("design/a/new.md", "Decision: new.\n");
        put("AGENTS.md", "Changed, not committed.\n");
        let g = project_guide_at(&dir, &first, &design, "it cites design/a/b.md").unwrap();
        assert_eq!(g["agents_md"], "How to work.\n");
        assert_eq!(
            g["records"],
            serde_json::json!({ "design/a.md": "Decision: a.", "design/a/b.md": "Decision: b." })
        );
        assert_eq!(g["cited"], serde_json::json!({ "design/a/b.md": "Decision: b.\n" }));
        run(&["rm", "-q", "design/a/b.md"]);
        run(&["add", "-A"]);
        run(&["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "second"]);
        let second = run(&["rev-parse", "HEAD"]).trim().to_string();
        let later = project_guide_at(&dir, &second, &design, "it cites design/a/b.md").unwrap();
        assert_eq!(later["agents_md"], "Changed, not committed.\n");
        assert_eq!(
            later["records"],
            serde_json::json!({ "design/a.md": "Decision: a.", "design/a/new.md": "Decision: new." })
        );
        assert_eq!(later["cited"], serde_json::json!({}));
        assert!(project_guide_at(&dir, "0000000", &design, "").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    // Links to directories that lead back into the directory read end the read in an error that names the link, at once:
    // with two such links (`a -> .`, `b -> .`) a read that followed each again at every level would make about
    // 2^17 walks before it gave up, which no test waits for, so this test ends only if the loop is seen on the first.
    #[cfg(unix)]
    #[test]
    fn links_that_lead_back_into_the_directory_read_end_in_an_error_at_once() {
        let dir = std::env::temp_dir().join(format!("wf-agents-link-rings-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("design/sub")).unwrap();
        let run = |args: &[&str]| git(&dir, args).unwrap();
        run(&["init", "-q"]);
        std::fs::write(dir.join("design/one.md"), "# One\n").unwrap();
        std::fs::write(dir.join("design/sub/two.md"), "# Two\n").unwrap();
        for (link, target) in [("design/a", "."), ("design/b", "."), ("design/sub/up", "..")] {
            std::os::unix::fs::symlink(target, dir.join(link)).unwrap();
        }
        run(&["add", "-A"]);
        run(&["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "rings"]);
        let commit = run(&["rev-parse", "HEAD"]).trim().to_string();
        let why = files_at(&dir, &commit, "design").unwrap_err();
        assert!(
            why.contains("design/a") && why.contains("loop of links") && why.contains(&commit),
            "{why}"
        );
        let why = project_guide_at(&dir, &commit, &["design/".to_string()], "").unwrap_err();
        assert!(why.contains("design/a") && why.contains("loop of links"), "{why}");
        // A link to a directory that is no ancestor of the walk is followed, and its files are found where it stands.
        std::fs::remove_file(dir.join("design/a")).unwrap();
        std::fs::remove_file(dir.join("design/b")).unwrap();
        std::fs::remove_file(dir.join("design/sub/up")).unwrap();
        std::os::unix::fs::symlink("sub", dir.join("design/deeper")).unwrap();
        run(&["add", "-A"]);
        run(&["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "no rings"]);
        let commit = run(&["rev-parse", "HEAD"]).trim().to_string();
        let found: Vec<String> = files_at(&dir, &commit, "design")
            .unwrap()
            .into_iter()
            .map(|f| f.0)
            .collect();
        assert_eq!(found, ["design/deeper/two.md", "design/one.md", "design/sub/two.md"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    // A link is followed to the file it points to, through any number of links inside the repository, at the end of the
    // path and in the middle of it alike (a directory that is a link, as `docs -> reasoning`); a link that leaves the
    // repository, one in a loop, one to a directory, a file taken for a directory and a name that is not there are errors
    // that name the path and the commit.
    #[cfg(unix)]
    #[test]
    fn a_link_is_followed_inside_the_repository_and_only_there() {
        let dir = std::env::temp_dir().join(format!("wf-agents-links-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::create_dir_all(dir.join("reasoning")).unwrap();
        std::fs::create_dir_all(dir.join("deep")).unwrap();
        let run = |args: &[&str]| git(&dir, args).unwrap();
        run(&["init", "-q"]);
        std::fs::write(dir.join("real.md"), "real\n").unwrap();
        std::fs::write(dir.join("reasoning/constitution.md"), "# Articles\n").unwrap();
        for (link, target) in [
            ("sub/up.md", "../real.md"),
            ("chain.md", "sub/up.md"),
            ("out.md", "../outside.md"),
            ("abs.md", "/etc/hostname"),
            ("loop-a.md", "loop-b.md"),
            ("loop-b.md", "loop-a.md"),
            ("dirlink.md", "sub"),
            // Directories that are links: at the root, through another link, one level down, and back up with `..`.
            ("docs", "reasoning"),
            ("docs2", "docs"),
            ("deep/inner", "../reasoning"),
            ("sub/root", ".."),
            // A directory link that leaves the repository, and two that lead into each other.
            ("escape", "../elsewhere"),
            ("ring-a", "ring-b"),
            ("ring-b", "ring-a"),
        ] {
            std::os::unix::fs::symlink(target, dir.join(link)).unwrap();
        }
        run(&["add", "-A"]);
        run(&["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "links"]);
        let commit = run(&["rev-parse", "HEAD"]).trim().to_string();
        let mut c = read(&files(&[])).unwrap();
        let mut p = c.projects.remove("p").unwrap();
        p.checkout = dir.clone();
        assert_eq!(read_at(&p, &commit, Path::new("chain.md")).unwrap(), "real\n");
        for path in [
            "docs/constitution.md",
            "docs2/constitution.md",
            "deep/inner/constitution.md",
            "./docs/./constitution.md",
        ] {
            assert_eq!(read_at(&p, &commit, Path::new(path)).unwrap(), "# Articles\n", "{path}");
        }
        assert_eq!(read_at(&p, &commit, Path::new("sub/root/real.md")).unwrap(), "real\n");
        for (path, why) in [
            ("out.md", "outside the repository"),
            ("abs.md", "outside the repository"),
            ("loop-a.md", "links"),
            ("dirlink.md", "not a file"),
            ("docs", "not a file"),
            ("escape/x.md", "outside the repository"),
            ("ring-a/x.md", "links"),
            ("docs/missing.md", "does not exist"),
            ("missing/constitution.md", "does not exist"),
            ("real.md/x.md", "not a directory"),
        ] {
            let e = read_at(&p, &commit, Path::new(path)).unwrap_err();
            assert!(
                e.contains(path) && e.contains(&commit) && e.contains(why),
                "{path}: {e}"
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    // A project's text is read from the commit its `origin/<main>` is at in its checkout, never from the checkout's
    // files or its own branch; a fetch moves that commit and no file; and a failure is an error that says what and where.
    #[test]
    fn a_project_s_text_is_read_from_its_fetched_main_and_a_failure_says_why() {
        let root = std::env::temp_dir().join(format!("wf-agents-main-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let origin = root.join("origin.git");
        let (checkout, other) = (root.join("checkout"), root.join("other"));
        let run = |dir: &Path, args: &[&str]| git(dir, args).unwrap();
        let commit = |dir: &Path, message: &str| {
            run(dir, &["add", "-A"]);
            run(
                dir,
                &["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", message],
            );
            run(dir, &["rev-parse", "HEAD"]).trim().to_string()
        };
        let put = |dir: &Path, path: &str, text: &[u8]| {
            let p = dir.join(path);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
        };
        run(
            &root,
            &["init", "-q", "--bare", "-b", "main", &origin.to_string_lossy()],
        );
        run(
            &root,
            &["clone", "-q", &origin.to_string_lossy(), &checkout.to_string_lossy()],
        );
        let project = |checkout: &Path| {
            let mut c = read(&files(&[])).unwrap();
            let mut p = c.projects.remove("p").unwrap();
            p.checkout = checkout.to_path_buf();
            p
        };
        let p = project(&checkout);
        // A checkout that holds no main yet has no commit of it, and says which ref it looked for.
        let none = main_commit(&p).unwrap_err();
        assert!(none.contains("origin/main"), "{none}");
        // A checkout that is not there is an error too.
        assert!(main_commit(&project(&root.join("missing"))).is_err());

        put(&checkout, "constitution.md", b"# Old\n");
        let first = commit(&checkout, "first");
        run(&checkout, &["push", "-q", "origin", "HEAD:main"]);
        assert_eq!(main_commit(&p).unwrap(), first);
        assert_eq!(read_at(&p, &first, Path::new("constitution.md")).unwrap(), "# Old\n");
        // When a commit was committed, in milliseconds; a commit the checkout lacks has none.
        let seconds: i64 = run(&checkout, &["show", "-s", "--format=%ct", &first])
            .trim()
            .parse()
            .unwrap();
        assert_eq!(committed_at(&p, &first).unwrap(), seconds * 1000);
        assert!(committed_at(&p, "0000000000000000000000000000000000000000").is_err());
        // A path may be written with a leading `./`; the same file.
        assert_eq!(read_at(&p, &first, Path::new("./constitution.md")).unwrap(), "# Old\n");

        // Main moves on the remote: nothing in the checkout moves until a fetch, and the fetch changes no file.
        run(
            &root,
            &["clone", "-q", &origin.to_string_lossy(), &other.to_string_lossy()],
        );
        put(&other, "constitution.md", b"# New\n");
        let second = commit(&other, "second");
        run(&other, &["push", "-q", "origin", "HEAD:main"]);
        assert_eq!(main_commit(&p).unwrap(), first);
        fetch_main(&p).unwrap();
        assert_eq!(main_commit(&p).unwrap(), second);
        assert_eq!(read_at(&p, &second, Path::new("constitution.md")).unwrap(), "# New\n");
        assert_eq!(
            std::fs::read_to_string(checkout.join("constitution.md")).unwrap(),
            "# Old\n"
        );
        // The commit read is the one asked for, whatever main is now.
        assert_eq!(read_at(&p, &first, Path::new("constitution.md")).unwrap(), "# Old\n");

        // What the checkout's files or its own branch hold is never read: the file there is changed, not committed,
        // and its branch is a commit main does not have.
        put(&checkout, "constitution.md", b"# Changed on disk\n");
        put(&checkout, "only-here.md", b"here\n");
        assert_eq!(read_at(&p, &second, Path::new("constitution.md")).unwrap(), "# New\n");
        let e = read_at(&p, &second, Path::new("only-here.md")).unwrap_err();
        assert!(e.contains("only-here.md") && e.contains(&second), "{e}");

        // Each failure is an error that names the path and the commit.
        put(&other, "bytes.md", &[0xff, 0xfe, 0x00]);
        put(&other, "dir/file.md", b"x\n");
        let third = commit(&other, "third");
        run(&other, &["push", "-q", "origin", "HEAD:main"]);
        fetch_main(&p).unwrap();
        let e = read_at(&p, &third, Path::new("bytes.md")).unwrap_err();
        assert!(
            e.contains("bytes.md") && e.contains(&third) && e.contains("UTF-8"),
            "{e}"
        );
        let e = read_at(&p, &third, Path::new("dir")).unwrap_err();
        assert!(e.contains("dir") && e.contains(&third), "{e}");
        let e = read_at(&p, &third, Path::new("../constitution.md")).unwrap_err();
        assert!(
            e.contains("../constitution.md") && e.contains("inside the repository"),
            "{e}"
        );
        let e = read_at(&p, &third, Path::new("/etc/hosts")).unwrap_err();
        assert!(e.contains("/etc/hosts"), "{e}");
        let e = read_at(
            &p,
            "0000000000000000000000000000000000000000",
            Path::new("constitution.md"),
        )
        .unwrap_err();
        assert!(
            e.contains("constitution.md") && e.contains("0000000000000000000000000000000000000000"),
            "{e}"
        );
        // A fetch that cannot reach the remote is an error, and leaves main where it was.
        let mut gone = project(&checkout);
        std::fs::rename(&origin, root.join("moved.git")).unwrap();
        let e = fetch_main(&gone).unwrap_err();
        assert!(e.contains("could not be fetched"), "{e}");
        assert_eq!(main_commit(&gone).unwrap(), third);
        // The checkout's own branch is no fallback: with no `origin/main`, main has no commit, though a branch `main`
        // is there.
        run(&checkout, &["update-ref", "refs/heads/main", &first]);
        run(&checkout, &["update-ref", "-d", "refs/remotes/origin/main"]);
        assert!(main_commit(&gone).is_err());
        gone.main = "master".into();
        assert!(main_commit(&gone).unwrap_err().contains("origin/master"));
        let _ = std::fs::remove_dir_all(&root);
    }
}
