//! The real carriers of what the engine gives to be done: the engine's own acts with plain `git` in the project's
//! repository and its host, the agents' rounds with the `claude` command line in each work's worktree, and the
//! tools agents ask for. They are what `wf-agents run` carries the works out with where no script is given, as the
//! installation's settings say.
//!
//! - [`settings`]: the installation's settings, read from `install.toml`.
//! - [`git`]: a work's branch and worktree, what the check before its merge finds there, and the merge into the
//!   repository's own main branch.
//! - `host`: where the project's repository is hosted: the head of main, the verdict on a work's commit, the merge.
//! - `acts`: the engine's acts, as the rounds of the cells they carry out on the host.
//! - [`message`]: what an agent is told at the start of a round, a function of the records alone.
//! - [`read`]: the tool `read`, the blocks of a work as an agent reads them.
//! - [`returns`]: the one form of every agent's return, and what came back read from it.
//! - [`cli`]: the command line of a round, and what it printed read into what came back and what it took.
//! - `agents`: the agents' rounds, run with the command line, a few at a time.
//! - `kept`: what each round of an agent was told and returned, where it is kept and how it is read back.
//! - `tools`: the tools, `read` and `check`.
//! - [`session`]: the session of each line of each work, worked out from the installation's id.
//! - `held` and `process`: what a carrier holds while it carries something out, and the programs it runs to do so.

mod acts;
mod agents;
pub mod cli;
pub mod git;
mod held;
mod host;
mod kept;
pub mod message;
mod process;
pub mod read;
pub mod returns;
pub mod session;
pub mod settings;
mod tools;

use std::path::Path;
use std::sync::Arc;

use acts::Acts;
use service::{Carriers, Rounds};

pub use acts::{FAILED, PASSED, faults};
pub use agents::{Agents, PROTOCOL, ROUNDS};
pub use process::{STOP_WAIT, TAIL};
pub use settings::Settings;
pub use tools::{CHECK, Tools};

/// The carriers of these settings, keeping what they keep in the data directory: the installation's id, the works'
/// worktrees, and what each round was told and returned.
///
/// The acts are carried out on the host the settings say: GitHub where they have a table for it, with the engine's
/// token from its environment, and the local repository where they have none.
pub fn carriers(settings: Settings, data: &Path) -> Result<Carriers, String> {
    // The worktrees are given to git by their path, which git would read against the repository's directory were it
    // relative.
    let data = std::path::absolute(data).map_err(|e| format!("{}: {e}", data.display()))?;
    let installation = session::installation(&data)?;
    let project = settings.project.clone();
    let tag = git::tag(&installation);
    let settings_env = settings.agents.env.clone();
    let acts: Arc<dyn Rounds> = match &settings.github {
        None => {
            let host = host::Local::new(project.clone()).giving(settings.agents.env.clone());
            Arc::new(Acts::new(host, project.clone(), data.clone(), tag.clone()))
        }
        Some(github) => {
            let host = host::GitHub::from_settings(github, &project, &data, |name| std::env::var(name).ok())?;
            Arc::new(Acts::new(host, project.clone(), data.clone(), tag.clone()))
        }
    };
    Ok(Carriers {
        agents: Arc::new(Agents::new(settings, data.clone(), installation)),
        acts,
        tools: Arc::new(Tools::new(project, settings_env, data, tag)),
    })
}
