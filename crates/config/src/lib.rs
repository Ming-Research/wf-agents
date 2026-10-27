//! The configuration: what differs by installation and by project. It lives outside this repository, in a data
//! directory the owner keeps under git and points the command at with `--data`:
//!
//! - `install.toml`: the owner, the machines and their slots, the CLIs and agent levels, the gh allow list and the
//!   limits of a query ([`Install`]);
//! - `projects/<name>.toml`: one project each ([`Project`]); its `extends` lists files under `shared/` to start
//!   from, merged in order: a later table merges into an earlier one key by key, and any other value, a list
//!   included, replaces the earlier one whole;
//! - `shared/<name>.toml`: the files that projects start from, by their `extends`;
//! - `rules/<name>.md`: the rules agents are given, by the names projects list them under;
//! - `db/`: the database, kept out of git;
//! - `agents/<cli>/`: the configuration directory each CLI runs agents with, unless the CLI names another, kept out
//!   of git; the owner logs in to the CLI there once.
//!
//! The command and the engine write more beside these, which are not committed either; README.md lists everything the
//! data directory holds.
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Owner {
    /// The language everything the owner reads is written in.
    pub language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Items {
    /// The longest an item's title may be, spaces not counted.
    pub title_max: u32,
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
    /// The levels of work, by name: an item's level picks the CLI, model and effort its agents run with; its advisor
    /// runs at the level one above.
    pub levels: BTreeMap<String, Level>,
    /// What the agents of each line may not do on this machine.
    #[serde(default)]
    pub permissions: Permissions,
    /// How many subagents a round may start for chores.
    #[serde(default)]
    pub subagents: Subagents,
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

fn read_only() -> Vec<String> {
    ["review", "advisor"].iter().map(|s| s.to_string()).collect()
}

impl Permissions {
    /// What a round on `line` may not use, as the CLI's tool rules name it: each heavy command, and for a line that
    /// only reads, the tools that edit files and the git commands that change the repository.
    pub fn refused(&self, line: &str) -> Vec<String> {
        let mut out: Vec<String> = self.heavy.iter().map(|c| format!("Bash({c} *)")).collect();
        if self.read_only.iter().any(|l| l == line) {
            out.extend(
                ["Edit", "Write", "MultiEdit", "NotebookEdit"]
                    .iter()
                    .map(|t| t.to_string()),
            );
            for g in [
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
            ] {
                out.push(format!("Bash(git {g} *)"));
            }
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Level {
    /// What work the level is for.
    pub desc: String,
    pub cli: String,
    pub model: String,
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
    /// The paths whose change is a change of design, and gives a design card.
    pub design_paths: Vec<String>,
    /// The command that checks the design tree, run in the worktree with the fork point from main as its argument.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub design_check: Option<String>,
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
}

/// The settings the flow reads from a project, as `{"config": name}` inputs name them.
pub const SETTINGS: [&str; 4] = ["design_goal", "attack_points", "full_ci", "deploy"];

impl Project {
    /// The value an input that reads a setting is given; none for a setting the flow does not read.
    pub fn setting(&self, name: &str) -> Option<serde_json::Value> {
        use serde_json::{Value, json};
        let text = |t: &Option<String>| t.as_ref().map_or(Value::Null, |s| Value::String(s.clone()));
        Some(match name {
            "design_goal" => Value::String(self.design_goal.clone()),
            "attack_points" => text(&self.attack_points),
            "full_ci" => json!({"workflows": self.ci.full, "merge_checks": self.ci.merge_checks}),
            "deploy" => text(&self.deploy),
            _ => return None,
        })
    }

    /// Whether the project has what a gate needs; none for a need the configuration does not answer.
    pub fn has(&self, need: &str) -> Option<bool> {
        match need {
            "under_test" => Some(!self.under_test.is_empty()),
            "other_projects" => Some(!self.other_projects.is_empty()),
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
    String::from_utf8(out.stdout).map_err(|e| format!("git {}: {e}", args.join(" ")))
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
    let files = git(dir, &["ls-tree", "-r", "--name-only", &commit]).map_err(fail)?;
    let files = files.lines().map(str::to_string).collect();
    let config = read(&Commit {
        dir,
        commit: commit.clone(),
        files,
    })?;
    Ok(Committed { commit, config })
}

/// Reads the configuration as the data directory's files are now, committed or not.
pub fn working(dir: &Path) -> Result<Config, Error> {
    read(&Tree(dir))
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
        for (name, p) in &self.projects {
            for r in &p.rules {
                if !self.rules.contains_key(r) {
                    problems.push(format!("projects/{name}.toml: no such rule: {r} (rules/{r}.md)"));
                }
            }
            for o in &p.other_projects {
                if !self.projects.contains_key(o) {
                    problems.push(format!("projects/{name}.toml: no such project: {o}"));
                }
            }
        }
        problems
    }

    /// What does not fit the flow the engine runs: a counter's limit the flow has no counter for, a gate's need the
    /// configuration does not answer, a setting the flow reads that no project gives.
    pub fn check(&self, compiled: &flow::Compiled) -> Vec<String> {
        let mut problems = Vec::new();
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
        let mut names: Vec<&String> = self.install.agents.levels.keys().collect();
        names.sort_by_key(|n| (n.parse::<i64>().ok(), (*n).clone()));
        match names.iter().position(|n| *n == name) {
            Some(i) => names.get(i + 1).unwrap_or(&names[i]).to_string(),
            None => name.to_string(),
        }
    }

    /// The level a line's agents run at for an item of the given level: the advisor one level above the item's, every
    /// other line at the item's.
    pub fn level_of_line(&self, item_level: &str, line: &str) -> String {
        if line == "advisor" {
            self.level_above(item_level)
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
effort = "high"

[gh]
allow = ["pr view"]

[query]
max_rows = 100
timeout_ms = 2000
"#;

    const SHARED: &str = r#"
design_goal = "correct, then fast"
design_paths = ["design/"]
test_paths = ["tests/"]
review_list = ["one", "two"]
[ci]
fast = "ci"
full = ["ci"]
merge_checks = ["check"]
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
        assert_eq!(
            p.limits,
            BTreeMap::from([("asks".to_string(), 9), ("fixes".to_string(), 5)])
        );
        assert_eq!(p.main, "main");
        assert_eq!(p.setting("full_ci").unwrap()["merge_checks"][1], "lint");
        assert_eq!(p.setting("deploy"), Some(serde_json::Value::Null));
        assert_eq!(p.has("under_test"), Some(false));
        assert!(p.setting("nope").is_none());
        // Agents run with a configuration directory of their own, inside the data directory unless one is set.
        let cli = &c.install.agents.clis["claude"];
        assert_eq!(cli.home(Path::new("/data"), "claude"), Path::new("/data/agents/claude"));
        let set = Cli {
            config_dir: Some(PathBuf::from("/elsewhere")),
            ..cli.clone()
        };
        assert_eq!(set.home(Path::new("/data"), "claude"), Path::new("/elsewhere"));
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
}
