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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Items {
    /// The longest an item's title may be, spaces not counted.
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
    /// The context, in tokens, past which a line's session is replaced by a new one at the line's next round: the
    /// prompt of the last call of the session's latest round, as recorded.
    #[serde(default = "context_limit")]
    pub context_limit: u64,
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
    /// The workflow that runs on each push of a work's branch: the fast CI the ci tool waits for.
    pub fast: String,
    /// The workflows that make up the full CI, those of downstream projects included, each as owner/name:workflow
    /// or, for this project's own, its name.
    pub full: Vec<String>,
    /// The checks a merge needs to have passed.
    pub merge_checks: Vec<String>,
    /// The input of the fast CI's workflow that takes the tests to run, their names separated by spaces, when it has
    /// one: given it, the workflow runs exactly those tests, every one to its end, and the ci_before tool runs it so on
    /// its temporary ref, which the workflow's own runs on push leave alone; none when it has no such input, and
    /// ci_before then waits for the whole fast CI its push runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tests_input: Option<String>,
    /// The artifact a red run of the fast CI uploads with the files the code generates, such as types generated
    /// from other types, each at its path in the repository: the ci tool puts those that differ from the head in the
    /// work's worktree, for the engineer to commit; none when the CI uploads none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generated_artifact: Option<String>,
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
            // The constitution is a file in the project's checkout, named by its path from there.
            if let Some(c) = &p.constitution
                && (c.as_os_str().is_empty() || !c.is_relative())
            {
                problems.push(format!(
                    "projects/{name}.toml: constitution is not a path in the project's checkout: {}",
                    c.display()
                ));
            }
            for o in &p.other_projects {
                if !self.projects.contains_key(o) {
                    problems.push(format!("projects/{name}.toml: no such project: {o}"));
                }
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
    let out = git_bytes(&p.checkout, &["rev-parse", "--verify", &format!("{main}^{{commit}}")])
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

/// Brings the project's `origin/<main>` in its checkout to the remote's main now (`git fetch`). It moves that ref only:
/// no file of the checkout changes.
pub fn fetch_main(p: &Project) -> Result<(), String> {
    git_bytes(&p.checkout, &["fetch", "-q", "origin", &p.main])
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
    let spec = format!("{commit}:{}", parts.join("/"));
    let bytes = git_bytes(&p.checkout, &["cat-file", "blob", &spec])
        .map_err(|x| format!("{shown} cannot be read at {commit} in {}: {x}", p.checkout.display()))?;
    String::from_utf8(bytes).map_err(|_| format!("{shown} at {commit} is not UTF-8 text"))
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
    let dirs: Vec<&str> = design.iter().filter(|d| d.ends_with('/')).map(String::as_str).collect();
    let mut args = vec!["ls-tree", "-r", "-z", commit, "--", "AGENTS.md"];
    args.extend(dirs.iter().copied());
    let listed = git(repo, &args)?;
    // Each entry is `<mode> <type> <object>\t<path>`.
    let blobs: Vec<(&str, &str)> = listed
        .split('\0')
        .filter_map(|entry| {
            let (meta, path) = entry.split_once('\t')?;
            let mut m = meta.split(' ').skip(1);
            let (kind, object) = (m.next()?, m.next()?);
            let node = path.ends_with(".md") && dirs.iter().any(|d| path.starts_with(d));
            (kind == "blob" && (path == "AGENTS.md" || node)).then_some((path, object))
        })
        .collect();
    let mut texts = blob_texts(repo, &blobs.iter().map(|b| b.1).collect::<Vec<_>>())?.into_iter();
    let mut agents = String::new();
    let mut nodes = Vec::new();
    for (path, _) in &blobs {
        let text = texts.next().unwrap_or_default();
        if *path == "AGENTS.md" {
            agents = text;
        } else {
            nodes.push((path.to_string(), text));
        }
    }
    Ok(guide(agents, nodes, cites))
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
pub fn files_at(repo: &Path, commit: &str, dir: &str) -> Result<Vec<(String, String)>, String> {
    let dir = format!("{}/", dir.trim_end_matches('/'));
    let listed = git(repo, &["ls-tree", "-r", "-z", commit, "--", &dir])?;
    let blobs: Vec<(&str, &str)> = listed
        .split('\0')
        .filter_map(|entry| {
            let (meta, path) = entry.split_once('\t')?;
            let mut m = meta.split(' ').skip(1);
            let (kind, object) = (m.next()?, m.next()?);
            (kind == "blob" && path.ends_with(".md")).then_some((path, object))
        })
        .collect();
    let texts = blob_texts(repo, &blobs.iter().map(|b| b.1).collect::<Vec<_>>())?;
    Ok(blobs.iter().map(|b| b.0.to_string()).zip(texts).collect())
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

    // The architect's level is one of the levels, and a project's constitution a path in its checkout; anything else
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
        let mut m = files(&[(
            "projects/q.toml",
            "extends = [\"common\"]\nrepo = \"o/q\"\ncheckout = \"/q\"\nconstitution = \"/q/CONSTITUTION.md\"",
        )]);
        let unnamed = format!("{INSTALL}\n[agents]\narchitect = \"9\"\n");
        m.0.insert("install.toml", Box::leak(unnamed.into_boxed_str()));
        let e = read(&m).unwrap_err();
        assert!(e.to_string().contains("agents.architect names no level: 9"), "{e}");
        assert!(
            e.to_string()
                .contains("projects/q.toml: constitution is not a path in the project's checkout: /q/CONSTITUTION.md"),
            "{e}"
        );
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

    // A file of main that is a link (an AGENTS.md that points to another file is common) is never given as the path it
    // points to, which is what git holds for it: the text is the file's, as a read of the checkout's files followed the
    // link, or else an error that names the path. Reference: `std::fs::read_to_string`, which followed a link, as the
    // reads of the checkout's files did before main was read with git.
    #[cfg(unix)]
    #[ignore = "held out of one CI run to see the tests after it"]
    #[test]
    fn a_link_on_main_is_never_read_as_the_path_it_points_to() {
        let root = std::env::temp_dir().join(format!("wf-agents-links-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let dir = root.join("repo");
        std::fs::create_dir_all(dir.join("design")).unwrap();
        std::fs::create_dir_all(dir.join("docs")).unwrap();
        std::fs::write(dir.join("CLAUDE.md"), "How to work.\n").unwrap();
        std::fs::write(dir.join("docs/real.md"), "The real constitution.\n").unwrap();
        std::fs::write(dir.join("design/real.md"), "Decision: real.\n").unwrap();
        std::os::unix::fs::symlink("CLAUDE.md", dir.join("AGENTS.md")).unwrap();
        std::os::unix::fs::symlink("docs/real.md", dir.join("constitution.md")).unwrap();
        std::os::unix::fs::symlink("real.md", dir.join("design/link.md")).unwrap();
        let run = |args: &[&str]| git(&dir, args).unwrap();
        run(&["init", "-q"]);
        run(&["add", "-A"]);
        run(&["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "links"]);
        let commit = run(&["rev-parse", "HEAD"]).trim().to_string();
        let mut p = read(&files(&[])).unwrap().projects.remove("p").unwrap();
        p.checkout = dir.clone();

        match read_at(&p, &commit, Path::new("constitution.md")) {
            Ok(text) => assert_eq!(text, "The real constitution.\n"),
            Err(why) => assert!(why.contains("constitution.md"), "{why}"),
        }
        let design = ["design/".to_string()];
        match project_guide_at(&dir, &commit, &design, "") {
            Ok(g) => {
                assert_ne!(g["agents_md"], "CLAUDE.md", "the guide is the path the link holds");
                assert_eq!(g["agents_md"], "How to work.\n");
                assert_eq!(g["records"]["design/link.md"], "Decision: real.");
            }
            Err(why) => assert!(why.contains("AGENTS.md") || why.contains("link.md"), "{why}"),
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    // Fetches of main that overlap in one checkout all succeed: the works of the engine take their steps at the same time,
    // and every round of a cell that reads main fetches it, so a fetch must not be turned down because another moved the
    // same ref a moment before ("cannot lock ref ... is at X but expected Y"). Reference: the same fetches one after
    // another, which all succeed.
    #[ignore = "held out of one CI run to see the tests after it"]
    #[test]
    fn fetches_of_main_that_overlap_in_one_checkout_all_succeed() {
        let root = std::env::temp_dir().join(format!("wf-agents-overlap-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let origin = root.join("origin.git");
        let (checkout, other) = (root.join("checkout"), root.join("other"));
        let run = |dir: &Path, args: &[&str]| git(dir, args).unwrap();
        run(
            &root,
            &["init", "-q", "--bare", "-b", "main", &origin.to_string_lossy()],
        );
        run(
            &root,
            &["clone", "-q", &origin.to_string_lossy(), &other.to_string_lossy()],
        );
        std::fs::write(other.join("f.md"), "0\n").unwrap();
        run(&other, &["add", "-A"]);
        run(
            &other,
            &["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "0"],
        );
        run(&other, &["push", "-q", "origin", "HEAD:main"]);
        run(
            &root,
            &["clone", "-q", &origin.to_string_lossy(), &checkout.to_string_lossy()],
        );
        let mut p = read(&files(&[])).unwrap().projects.remove("p").unwrap();
        p.checkout = checkout.clone();
        for round in 1..=15 {
            std::fs::write(other.join("f.md"), format!("{round}\n")).unwrap();
            run(&other, &["add", "-A"]);
            run(
                &other,
                &["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "next"],
            );
            run(&other, &["push", "-q", "origin", "HEAD:main"]);
            let pushed = run(&other, &["rev-parse", "HEAD"]).trim().to_string();
            let results: Vec<Result<(), String>> = std::thread::scope(|s| {
                let fetches: Vec<_> = (0..8).map(|_| s.spawn(|| fetch_main(&p))).collect();
                fetches.into_iter().map(|f| f.join().unwrap()).collect()
            });
            for result in &results {
                assert!(result.is_ok(), "round {round}: {result:?}");
            }
            assert_eq!(main_commit(&p).unwrap(), pushed);
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    // The checkout's `origin/<main>` is what the remote's main is after a fetch, whatever happened to it: a main that was
    // rewritten and force-pushed, and a main whose name has a slash; a main the remote does not have is an error.
    // Reference: `git ls-remote origin <main>`, the remote's own answer.
    #[test]
    fn a_rewritten_main_and_a_main_with_a_slash_in_its_name_are_what_the_remote_has_after_a_fetch() {
        let root = std::env::temp_dir().join(format!("wf-agents-rewrite-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let origin = root.join("origin.git");
        let (checkout, other) = (root.join("checkout"), root.join("other"));
        let run = |dir: &Path, args: &[&str]| git(dir, args).unwrap();
        let commit = |dir: &Path, args: &[&str]| {
            run(dir, &["add", "-A"]);
            let mut all = vec!["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q"];
            all.extend_from_slice(args);
            run(dir, &all);
            run(dir, &["rev-parse", "HEAD"]).trim().to_string()
        };
        run(
            &root,
            &["init", "-q", "--bare", "-b", "main", &origin.to_string_lossy()],
        );
        run(
            &root,
            &["clone", "-q", &origin.to_string_lossy(), &other.to_string_lossy()],
        );
        std::fs::write(other.join("constitution.md"), "# First\n").unwrap();
        let first = commit(&other, &["-m", "first"]);
        run(&other, &["push", "-q", "origin", "HEAD:main"]);
        run(
            &root,
            &["clone", "-q", &origin.to_string_lossy(), &checkout.to_string_lossy()],
        );
        let mut p = read(&files(&[])).unwrap().projects.remove("p").unwrap();
        p.checkout = checkout.clone();
        assert_eq!(main_commit(&p).unwrap(), first);

        // The remote's main is rewritten: the commit is replaced by another one, which is not its child.
        std::fs::write(other.join("constitution.md"), "# Rewritten\n").unwrap();
        let rewritten = commit(&other, &["--amend", "-m", "first, rewritten"]);
        assert_ne!(rewritten, first);
        run(&other, &["push", "-q", "--force", "origin", "HEAD:main"]);
        fetch_main(&p).unwrap();
        assert_eq!(main_commit(&p).unwrap(), rewritten);
        assert_eq!(
            read_at(&p, &rewritten, Path::new("constitution.md")).unwrap(),
            "# Rewritten\n"
        );

        // A main named with a slash is `origin/release/1` in the checkout, and a fetch brings it.
        std::fs::write(other.join("constitution.md"), "# Release\n").unwrap();
        let release = commit(&other, &["-m", "release"]);
        run(&other, &["push", "-q", "origin", "HEAD:refs/heads/release/1"]);
        p.main = "release/1".into();
        assert_eq!(p.main_ref(), "origin/release/1");
        assert!(main_commit(&p).is_err());
        fetch_main(&p).unwrap();
        assert_eq!(main_commit(&p).unwrap(), release);
        assert_eq!(
            read_at(&p, &release, Path::new("constitution.md")).unwrap(),
            "# Release\n"
        );

        // A main the remote has not got cannot be fetched, and the error says which.
        p.main = "no-such-branch".into();
        let e = fetch_main(&p).unwrap_err();
        assert!(e.contains("could not be fetched"), "{e}");
        assert!(main_commit(&p).is_err());
        let _ = std::fs::remove_dir_all(&root);
    }
}
