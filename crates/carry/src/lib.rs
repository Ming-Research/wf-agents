//! The real carriers of what the engine gives to be done: the engine's own acts with plain `git` in the project's
//! repository and its host, the agents' rounds with the `claude` command line in each work's worktree, and the
//! tools agents ask for. They are what `wf-agents run` carries the works out with where no script is given, as the
//! installation's settings say.
//!
//! - [`settings`]: the installation's settings, read from `install.toml`, and the settings a text names given with
//!   their values.
//! - [`system`]: the system prompt of a line's session, made as the session begins from the files beside the flows
//!   (`flow::system`) and kept with it, and what is checked of it as the engine starts.
//! - [`git`]: a work's branch and worktree, what the check before its merge finds there, and the merge into the
//!   repository's own main branch.
//! - `host`: where the project's repository is hosted: the head of main, the verdict on a work's commit, the merge.
//! - `acts`: the engine's acts, as the rounds of the cells they carry out on the host.
//! - [`message`]: what an agent is told at the start of a round, a function of the records alone.
//! - [`read`]: the tool `read`, the blocks of a work as an agent reads them.
//! - [`returns`]: what came back, read in the form of return of its flow against the cell it was entered for.
//! - [`cli`]: the command line of a round, and what it printed read into what came back and what it took.
//! - [`permissions`]: what the agents of a line may run, and what they are told of it.
//! - [`failure`]: the kind of cause of a round that failed, told apart by what the command line reported, and the look
//!   at the network while the agents' rounds wait for it.
//! - `agents`: the agents' rounds, run with the command line, a few at a time.
//! - [`catch_up`]: the work's branch taking main in as a run of a cell that changes the code starts.
//! - `kept`: what each round of an agent was told and returned, where it is kept and how it is read back.
//! - `tools`: the tools, `read`, `check` and `check_before`.
//! - [`session`]: the sessions of each line of each work, worked out from the installation's id.
//! - `held` and `process`: what a carrier holds while it carries something out, and the programs it runs to do so.

mod acts;
mod agents;
pub mod catch_up;
pub mod cli;
pub mod failure;
pub mod git;
mod held;
mod host;
mod kept;
pub mod message;
pub mod permissions;
mod process;
pub mod read;
pub mod returns;
pub mod session;
pub mod settings;
pub mod system;
mod tools;

use std::path::Path;
use std::sync::Arc;

use acts::Acts;
use host::Host;
use service::{Carriers, Rounds, Tools as ToolsTrait};
use settings::Project;
use tools::Tools;

pub use acts::{MAIN, WATCH, Watch, Words, faults, main_faults, word_faults};
pub use agents::{Agents, ROUNDS};
pub use host::github::{GhOffsite, UPLOAD_RATE};
pub use process::{STOP_WAIT, TAIL};
pub use settings::Settings;
pub use tools::{CHECK, CHECK_BEFORE};

/// The carriers of these settings, keeping what they keep in the data directory: the installation's id, the works'
/// worktrees, and what each round was told and returned. The agents' rounds are told, and given the form of return,
/// as what is written beside the flows says, and their system prompts are made from it ([`system`]).
///
/// The acts and the tools are carried out on the host the settings say: GitHub where they have a table for it, with
/// the engine's token from its environment, and the local repository where they have none.
pub fn carriers(settings: Settings, data: &Path, beside: flow::agents::Agents) -> Result<Carriers, String> {
    carry(settings, data, beside).map(|(carriers, _)| carriers)
}

/// The carriers of these settings, as [`carriers`] gives them, and the watch of main of their acts, which whoever
/// runs the loop runs beside it, saying what it finds to the loop as the source [`MAIN`]. The acts and the commits the
/// engine makes say what is written beside the flows says, as the agents' rounds do.
pub fn carry(mut settings: Settings, data: &Path, beside: flow::agents::Agents) -> Result<(Carriers, Watch), String> {
    settings.project.commits = beside.commits.clone();
    let words = beside.acts.clone();
    // The worktrees are given to git by their path, which git would read against the repository's directory were it
    // relative.
    let data = std::path::absolute(data).map_err(|e| format!("{}: {e}", data.display()))?;
    let installation = session::installation(&data)?;
    let project = settings.project.clone();
    let tag = git::tag(&installation);
    let (acts, tools, watch, main) = match &settings.github {
        None => {
            let host = host::Local::new(project.clone()).giving(settings.agents.env.clone());
            on(host, &project, &data, &tag, words)
        }
        Some(github) => {
            let host = host::GitHub::from_settings(github, &project, &data, |name| std::env::var(name).ok())?;
            on(host, &project, &data, &tag, words)
        }
    };
    let slots = settings.agents.slots;
    let carriers = Carriers {
        slots,
        agents: Arc::new(Agents::new(settings, data.clone(), installation, main, beside)),
        acts,
        tools,
        network: Arc::new(failure::Look),
    };
    Ok((carriers, watch))
}

/// The acts, the tools and the watch of main, on one host they share, and the reference that holds main's head as the
/// host last gave it.
fn on<H: Host>(
    host: H,
    project: &Project,
    data: &Path,
    tag: &str,
    words: Words,
) -> (Arc<dyn Rounds>, Arc<dyn ToolsTrait>, Watch, String) {
    let main = host.kept_main();
    let host = Arc::new(host);
    let acts = Acts::new(
        host.clone(),
        project.clone(),
        data.to_path_buf(),
        tag.to_string(),
        words,
    );
    let watch = acts.watch(WATCH);
    let tools = Tools::new(host, project.clone(), data.to_path_buf(), tag.to_string());
    (Arc::new(acts), Arc::new(tools), watch, main)
}
