//! The installation's settings: the project the works are done in, and how its agents are run. They are read from a
//! TOML file, `install.toml` in the data directory unless another is named; `install.sample.toml` beside this crate's
//! manifest says what each key is. A key the file does not know is refused, as the flow's files refuse one, so that a
//! misspelt key is not silently left at nothing.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::cache;

/// The settings as their file is written.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub project: Project,
    pub agents: Agents,
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
    /// Whether a round that read less from the prompt cache than its session's previous round left there makes the
    /// work wait for the owner; true where it is not given.
    #[serde(default = "yes")]
    pub cache_check: bool,
    /// How long, in minutes, what a round left in the cache is taken to be there still; 55 where it is not given.
    #[serde(default = "cache_life")]
    pub cache_life_minutes: u64,
    /// The share, in hundredths, of what a round should have read from the cache that it may read less and still hit;
    /// 2 where it is not given.
    #[serde(default = "cache_shortfall_percent")]
    pub cache_shortfall_percent: f64,
    /// The tokens a round may read less than it should have and still hit, where that is more than the share; 512
    /// where it is not given.
    #[serde(default = "cache_shortfall_tokens")]
    pub cache_shortfall_tokens: u64,
}

fn user() -> String {
    "user".to_string()
}

fn yes() -> bool {
    true
}

fn cache_life() -> u64 {
    55
}

fn cache_shortfall_percent() -> f64 {
    2.0
}

fn cache_shortfall_tokens() -> u64 {
    512
}

/// The settings file's name in the data directory.
pub const FILE: &str = "install.toml";

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
        let percent = settings.agents.cache_shortfall_percent;
        if !(percent.is_finite() && (0.0..=100.0).contains(&percent)) {
            wrong.push("agents.cache_shortfall_percent: a share is from 0 to 100 hundredths".to_string());
        }
        if settings.project.main.is_empty() {
            wrong.push("project.main: the main branch has a name".to_string());
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

    /// How a round's read from the prompt cache is judged.
    pub fn cache(&self) -> cache::Check {
        cache::Check {
            life_minutes: self.agents.cache_life_minutes,
            shortfall_percent: self.agents.cache_shortfall_percent,
            shortfall_tokens: self.agents.cache_shortfall_tokens,
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
        assert!(settings.agents.cache_check);
        assert_eq!(
            settings.cache(),
            cache::Check {
                life_minutes: 55,
                shortfall_percent: 2.0,
                shortfall_tokens: 512,
            }
        );
    }

    #[test]
    fn the_cache_check_is_on_unless_turned_off_and_a_share_beyond_the_whole_is_refused() {
        let off = SAMPLE.replace("[agents]\n", "[agents]\ncache_check = false\n");
        assert!(!Settings::read(&off).expect("it reads").agents.cache_check);
        let beyond = SAMPLE.replace("[agents]\n", "[agents]\ncache_shortfall_percent = 101.0\n");
        let refused = Settings::read(&beyond).expect_err("it is refused");
        assert!(refused.contains("cache_shortfall_percent"), "{refused}");
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

    #[test]
    fn no_slots_and_a_budget_below_nothing_are_refused() {
        let text =
            (SAMPLE.replace("slots = 2", "slots = 0")).replace("work_budget_usd = 5.0", "work_budget_usd = -1.0");
        let refused = Settings::read(&text).expect_err("both are refused");
        assert_eq!(refused.lines().count(), 2, "{refused}");
    }
}
