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
//! - [`returns`]: what came back, read in the form of return of its flow against the cell it was entered for.
//! - [`cli`]: the command line of a round, and what it printed read into what came back and what it took.
//! - [`permissions`]: what the agents of a line may run, and what they are told of it.
//! - [`failure`]: the kind of cause of a round that failed, told apart by what the command line reported, and the look
//!   at the network while the agents' rounds wait for it.
//! - `agents`: the agents' rounds, run with the command line, a few at a time.
//! - [`catch_up`]: the work's branch taking main in as a run of a cell that changes the code starts.
//! - `kept`: what each round of an agent was told and returned, where it is kept and how it is read back.
//! - `tools`: the tools, `query` and `run_tests`.
//! - [`tree`]: the design tree on a work's branch, as the data a source outside the work writes.
//! - [`session`]: the sessions of each line of each work, worked out from the installation's id.
//! - `held` and `process`: what a carrier holds while it carries something out, and the programs it runs to do so.
//! - [`account`]: the account the agents' program makes its calls under, a directory of its own signed in by hand.

pub mod account;
mod acts;
mod agents;
pub mod catch_up;
pub mod cli;
pub mod failure;
pub mod git;
mod held;
mod host;
mod kept;
pub mod permissions;
mod process;
pub mod returns;
pub mod session;
pub mod settings;
pub mod system;
mod tools;
pub mod tree;

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
pub use tools::{QUERY, Querying, RUN_TESTS};

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
pub fn carry(
    mut settings: Settings,
    data: &Path,
    mut beside: flow::agents::Agents,
) -> Result<(Carriers, Watch), String> {
    settings.project.commits = beside.commits.clone();
    settings.project.tree = beside.project.tree.clone();
    let words = beside.acts.clone();
    // Where the host is the local repository, the agents are told what `run_tests` does there.
    if settings.github.is_none() {
        local_words(&mut beside);
    }
    let tool = beside.tools.get(tools::RUN_TESTS).cloned();
    let unread = unread(&beside);
    // The worktrees are given to git by their path, which git would read against the repository's directory were it
    // relative.
    let data = std::path::absolute(data).map_err(|e| format!("{}: {e}", data.display()))?;
    let installation = session::installation(&data)?;
    let project = settings.project.clone();
    let tag = git::tag(&installation);
    let query = querying(&settings, &beside, &data)?;
    let (acts, tools, watch, main) = match &settings.github {
        None => {
            let host = host::Local::new(project.clone())
                .giving(settings.agents.env.clone())
                .saying(tool.as_ref());
            on(host, &project, &data, &tag, words, query.clone(), unread)
        }
        Some(github) => {
            let env = |name: &str| std::env::var(name).ok();
            let ci = beside.project.ci.clone();
            let host = host::GitHub::from_settings(github, &project, &data, env, ci, tool.clone())?;
            on(host, &project, &data, &tag, words, query.clone(), unread)
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

/// The tool `query` as these settings and the files beside the flows say; or why the settings cannot open it.
fn querying(settings: &Settings, beside: &flow::agents::Agents, data: &Path) -> Result<Querying, String> {
    let tool = beside.tools.get(QUERY);
    Querying::new(&settings.query, tool, data, store::SCHEMA).map_err(|why| format!("query.open: {why}"))
}

/// What fills `{store}` in the words of the tool `query`, which the line's system prompt holds: the store as the
/// comments of its schema describe it where the settings open it to the agents, and the file's words that it is not
/// open where they do not; or why the settings cannot open it. It is made outside the tools, which the loop holds as
/// one trait object, so that the prompt, made from the same settings and files, can be given it.
pub fn store(settings: &Settings, beside: &flow::agents::Agents) -> Result<String, String> {
    querying(settings, beside, Path::new("")).map(|query| query.store())
}

/// What `run_tests` is told to do where the host is the local repository: its own words for it, in place of those for
/// GitHub.
fn local_words(beside: &mut flow::agents::Agents) {
    if let Some(tool) = beside.tools.get_mut(tools::RUN_TESTS)
        && let Some(local) = tool.words_local.take()
    {
        tool.words = local;
    }
}

/// What an agent is told of a request of `run_tests` that cannot be read, from the file of the tool.
fn unread(beside: &flow::agents::Agents) -> Option<String> {
    beside.tools.get(tools::RUN_TESTS).and_then(|tool| tool.unread.clone())
}

/// The acts, the tools and the watch of main, on one host they share, and the reference that holds main's head as the
/// host last gave it.
fn on<H: Host>(
    host: H,
    project: &Project,
    data: &Path,
    tag: &str,
    words: Words,
    query: Querying,
    unread: Option<String>,
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
    let tools = Tools::new(host, project.clone(), data.to_path_buf(), tag.to_string(), unread).querying(query);
    (Arc::new(acts), Arc::new(tools), watch, main)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    #[test]
    fn what_a_request_that_cannot_be_read_is_told_is_what_the_repositorys_file_of_the_tool_says() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow");
        let beside = flow::agents::read(&root).expect("what is written beside the flows is read");
        let told = super::unread(&beside).expect("the file of run_tests says it");
        assert!(
            told.starts_with("Nothing was run: ") && told.contains("{error}"),
            "{told}"
        );
    }

    #[test]
    fn the_project_declares_its_two_workflows_and_where_its_tests_are() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow");
        let project = flow::agents::read(&root)
            .expect("what is written beside the flows is read")
            .project;
        let ci = project.ci.expect("it declares its workflows");
        assert_eq!(ci.fast.expect("fast").workflow, "fast.yml");
        let full = ci.full.expect("full");
        assert_eq!(
            (full.workflows.as_slice(), full.too_long.as_str()),
            (&["ci.yml".to_string()][..], "2h")
        );
        let tests = project.tests.expect("it declares its tests");
        assert!(!tests.paths.is_empty() && !tests.expected.is_empty());
    }

    #[test]
    fn the_local_host_is_told_its_own_words_for_run_tests_and_github_the_others() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow");
        let mut beside = flow::agents::read(&root).expect("what is written beside the flows is read");
        let github = beside.tools["run_tests"].words.clone();
        assert!(github.contains("last commit of the work's branch"), "{github}");
        super::local_words(&mut beside);
        let local = &beside.tools["run_tests"].words;
        assert!(
            local.contains("your worktree as it stands, what is not committed included"),
            "{local}"
        );
        assert_ne!(local, &github);
    }
}
