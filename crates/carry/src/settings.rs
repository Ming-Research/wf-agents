//! The installation's settings: the project the works are done in, where its repository is hosted, and how its
//! agents are run. They are read from a TOML file, `install.toml` in the data directory unless another is named; `install.sample.toml` beside this crate's
//! manifest says what each key is. A key the file does not know is refused, as the flow's files refuse one, so that a
//! misspelt key is not silently left at nothing.

use std::path::{Path, PathBuf};

use serde::Deserialize;

/// The settings as their file is written.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub project: Project,
    pub agents: Agents,
    /// Where the project's repository is on GitHub, which then carries out the acts; with none, the local repository
    /// does.
    pub github: Option<GitHub>,
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
    /// The `gh` command line, or a path to it.
    #[serde(default = "gh")]
    pub gh: String,
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
    /// The model each round is run with.
    pub model: String,
    /// How many agents' rounds run at once.
    pub slots: usize,
    /// What is given to the program on every round after what the engine gives it.
    #[serde(default)]
    pub args: Vec<String>,
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
        let settings: Settings = toml::from_str(text).map_err(|e| e.to_string().trim_end().to_string())?;
        let mut wrong = Vec::new();
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
        if settings.project.main.is_empty() {
            wrong.push("project.main: the main branch has a name".to_string());
        }
        if let Some(github) = &settings.github {
            wrong.extend(github.wrong(&settings.agents));
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
    fn an_unknown_key_is_refused() {
        let text = SAMPLE.replace("slots = 2", "slots = 2\nslot = 3");
        let refused = Settings::read(&text).expect_err("an unknown key is refused");
        assert!(refused.contains("slot"), "{refused}");
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

    #[test]
    fn no_slots_and_a_budget_below_nothing_are_refused() {
        let text =
            (SAMPLE.replace("slots = 2", "slots = 0")).replace("work_budget_usd = 5.0", "work_budget_usd = -1.0");
        let refused = Settings::read(&text).expect_err("both are refused");
        assert_eq!(refused.lines().count(), 2, "{refused}");
    }
}
