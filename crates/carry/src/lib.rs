//! The real carriers of what the engine gives to be done: the engine's own acts with plain `git` in the project's
//! repository and its host, the agents' rounds with the `claude` command line in each work's worktree, and the
//! tools agents ask for. They are what `wf-agents run` carries the works out with where no script is given, as the
//! installation's settings say.
//!
//! - [`settings`]: the installation's settings, read from `install.toml`, and the settings a text names given with
//!   their values.
//! - [`projects`]: the projects the settings name, each with its declaration from its repository, and what the loop
//!   gives the acts and the tools reaching the project of the work's item.
//! - [`system`]: the system prompt of a line's session, made as the session begins from the files beside the flows
//!   (`flow::system`) and kept with it, and what is checked of it as the engine starts.
//! - [`git`]: a work's branch and worktree, what the check before its merge finds there, and the merge into the
//!   repository's own main branch.
//! - `host`: where the project's repository is hosted: the head of main, the verdict on a work's commit, the merge.
//! - `acts`: the engine's acts, as the rounds of the cells they carry out on the host.
//! - `changed`: what a work changed, worked out with git for its page.
//! - [`returns`]: what came back, read in the form of return of its flow against the cell it was entered for.
//! - [`cli`]: the command line of a round, and what it printed read into what came back and what it took.
//! - [`permissions`]: what the agents of a line may run, and what they are told of it.
//! - [`failure`]: the kind of cause of a round that failed, told apart by the fields the command line names the
//!   service's errors in and by the words of what it reported, and the looks at the network.
//! - [`outside`]: the check of the outside the agents' rounds need, which the engine takes by itself while they wait
//!   for it, and as the owner looks at the accounts' usage: an account in use, the network and a short request.
//! - `agents`: the agents' rounds, run with the command line, a few at a time.
//! - [`catch_up`]: the work's branch taking main in as a run of a cell that changes the code starts.
//! - `kept`: what each round of an agent was told and returned, where it is kept and how it is read back.
//! - `tools`: the tools, `query` and `run_tests`.
//! - `branch`: what the engine reads of a work's branch as data of the work, in one step: [`tree`], the design tree on
//!   it, and [`head`], the work's head, each the data of a source outside the work.
//! - [`session`]: the sessions of each line of each work, worked out from the installation's id.
//! - `held` and `process`: what a carrier holds while it carries something out, and the programs it runs to do so.
//! - [`account`]: the account the agents' program makes its calls under: the variables it is given of it, the token
//!   of the account in use, the program's directory, and the usage its stream says.
//! - [`keychain`]: the tokens of the accounts, kept in the login keychain.

pub mod account;
mod acts;
mod agents;
mod branch;
pub mod catch_up;
mod changed;
pub mod cli;
pub mod commit_lines;
mod cost;
pub mod failure;
pub mod git;
pub mod guard;
pub mod head;
mod held;
mod host;
mod kept;
pub mod keychain;
pub mod outside;
pub mod permissions;
mod process;
mod programs;
pub mod projects;
mod protected;
pub mod repair;
pub mod returns;
pub mod session;
pub mod settings;
pub mod system;
mod tests_only;
mod tools;
pub mod tree;

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use acts::Acts;
use host::Host;
use projects::ByProject;
use service::{Carriers, Rounds, Tools as ToolsTrait};
use settings::Project;
use tools::Tools;

pub use acts::{MAIN, WATCH, Watch, Words, faults, main_faults, word_faults};
pub use agents::{Agents, Of, ROUNDS};
pub use branch::compared;
pub use host::github::{GhOffsite, UPLOAD_RATE};
pub use process::{STOP_WAIT, TAIL};
pub use settings::Settings;
pub use tools::{QUERY, Querying, RUN_TESTS};

/// The carriers of these settings, keeping what they keep in the data directory: the installation's id, the works'
/// worktrees, and what each round was told and returned. The agents' rounds are told, and given the form of return,
/// as what is written beside the flows says, and their system prompts are made from it with the declaration of the
/// project of the work's item ([`system`]). A project whose declaration cannot be used is held, and its works are
/// carried out in no way but a failed round that says why ([`projects::declared`]).
///
/// Each project's acts and tools are carried out on the host its settings say: GitHub where they have a table for it,
/// with the engine's token from its environment, and its local repository where they have none. The tokens of the
/// accounts the agents run under are kept in the login keychain ([`keychain`]).
pub fn carriers(
    settings: Settings,
    data: &Path,
    beside: flow::agents::Agents,
    flows: &[flow::Compiled],
) -> Result<Carriers, String> {
    carry(settings, data, beside, flows).map(|(carriers, _)| carriers)
}

/// The carriers [`carriers`] gives, the tokens of the accounts kept by what is given here in place of the login
/// keychain, as a test keeps them.
pub fn carriers_keeping(
    settings: Settings,
    data: &Path,
    beside: flow::agents::Agents,
    flows: &[flow::Compiled],
    tokens: Arc<dyn service::Tokens>,
) -> Result<Carriers, String> {
    carry_keeping(settings, data, beside, flows, Some(tokens)).map(|(carriers, _)| carriers)
}

/// The watch of main of a project's acts, with the project's name and the name of its main branch.
pub struct ProjectWatch {
    pub project: String,
    pub main: String,
    pub watch: Watch,
}

/// The carriers of these settings, as [`carriers`] gives them, and the watch of main of each project's acts, which
/// whoever runs the loop runs beside it, saying what it finds to the loop as the source [`MAIN`] of that project. The
/// acts and the commits the engine makes say what is written beside the flows says, as the agents' rounds do.
pub fn carry(
    settings: Settings,
    data: &Path,
    beside: flow::agents::Agents,
    flows: &[flow::Compiled],
) -> Result<(Carriers, Vec<ProjectWatch>), String> {
    carry_keeping(settings, data, beside, flows, None)
}

/// The carriers and the watches [`carry`] gives, the tokens of the accounts kept by what is given, or, with none, in
/// the login keychain under the installation's id; the agents' program's directory, `profile` in the data directory,
/// made where it is missing, whose settings file may name no credential and no permission mode.
fn carry_keeping(
    settings: Settings,
    data: &Path,
    beside: flow::agents::Agents,
    flows: &[flow::Compiled],
    tokens: Option<Arc<dyn service::Tokens>>,
) -> Result<(Carriers, Vec<ProjectWatch>), String> {
    let words = beside.acts.clone();
    // The worktrees are given to git by their path, which git would read against the repository's directory were it
    // relative.
    let data = std::path::absolute(data).map_err(|e| format!("{}: {e}", data.display()))?;
    let installation = session::installation(&data)?;
    let tokens: Arc<dyn service::Tokens> = tokens.unwrap_or_else(|| {
        let making = format!("{} {}", settings.agents.command, account::MAKE_TOKEN);
        Arc::new(keychain::Keychain::new(
            Path::new(keychain::SECURITY),
            &installation,
            making,
        ))
    });
    let profile = account::profile(&data);
    std::fs::create_dir_all(&profile).map_err(|e| format!("{}: {e}", profile.display()))?;
    // The hook that ends the agents' commits with the item and the work, which each round's git is given.
    commit_lines::install(&data)?;
    // The program reads its own settings file there with every command: one that gives it a credential of no
    // account's, or names a permission mode it would ignore, keeps the engine from starting (decisions
    // the-programs-directory-is-the-engines and permissions-are-rules-not-a-mode).
    let faults = account::settings_file_faults(&profile);
    if !faults.is_empty() {
        return Err(faults.join("; "));
    }
    let tag = git::tag(&installation);
    let query = querying(&beside, &data);
    let (mut acts, mut tools, mut watches, mut of) = (BTreeMap::new(), BTreeMap::new(), Vec::new(), BTreeMap::new());
    // The network reaches a project's host on GitHub, one look for every such project, and no network a local one.
    let mut host_look: Option<Arc<dyn service::Network>> = None;
    let (declared, faults) = projects::declared(&settings, &beside, flows);
    if !faults.is_empty() {
        return Err(faults.join("; "));
    }
    for (name, declared) in declared {
        // A project held for its declaration carries nothing out: its works fail with why, each round.
        let declared = match declared {
            Ok(declared) => declared,
            Err(why) => {
                acts.insert(name.clone(), Err(why.clone()));
                tools.insert(name.clone(), Err(why.clone()));
                of.insert(name, Err(why));
                continue;
            }
        };
        let declaration = declared.declaration;
        let mut project = declared.settings;
        project.commits = beside.commits.clone();
        let beside = declared.beside;
        project.tree = declaration.tree.clone();
        let tool = beside.tools.get(tools::RUN_TESTS).cloned();
        let said = tools::Said::of(&beside).map_err(|why| format!("projects.{name}: {why}"))?;
        // The check before the merge holds a change to what the project protects until the owner lets it merge.
        let protecting = acts::Protecting {
            paths: declaration.protected.paths.clone(),
            words: Some(beside.asks.protected.clone()),
        };
        let (rounds, answers, watch, main) = match &project.github {
            None => {
                let check = (declaration.local.as_ref())
                    .map(|local| local.check.clone())
                    .ok_or_else(|| format!("projects.{name}: {} has no [local] check", flow::agents::DECLARATION))?;
                let host = host::Local::new(project.clone(), check)
                    .giving(settings.agents.env.clone())
                    .saying(tool.as_ref());
                on(
                    host,
                    &project,
                    &data,
                    &tag,
                    (words.clone(), protecting, None),
                    query.clone(),
                    said,
                )
            }
            Some(github) => {
                let env = |name: &str| std::env::var(name).ok();
                let ci = declaration.ci.clone();
                let merging = (declaration.pull_request.as_ref())
                    .ok_or_else(|| format!("projects.{name}: {} has no [pull_request]", flow::agents::DECLARATION))?;
                let hosting = (tool.clone(), Some(words.check.clone()), beside.pull_request.clone());
                let host =
                    host::GitHub::from_settings(github, &project, &data, installation, env, (ci, merging), hosting)
                        .map_err(|why| format!("projects.{name}: {why}"))?;
                let look = (host_look.get_or_insert_with(|| Arc::new(failure::LookAt(host::github::LOOK.to_string()))))
                    .clone();
                on(
                    host,
                    &project,
                    &data,
                    &tag,
                    (words.clone(), protecting, Some(look)),
                    query.clone(),
                    said,
                )
            }
        };
        acts.insert(name.clone(), Ok(rounds));
        tools.insert(name.clone(), Ok(answers));
        watches.push(ProjectWatch {
            project: name.clone(),
            main: project.main.clone(),
            watch,
        });
        of.insert(name, Ok(agents::Of::new(&settings, project, beside, declaration, main)));
    }
    let slots = settings.agents.slots;
    let outside = outside::Check::of(&settings, &beside.cost, tokens.clone(), &profile)?;
    let carriers = Carriers {
        slots,
        agents: Arc::new(Agents::new(settings, data.clone(), installation, of, tokens.clone())),
        acts: Arc::new(ByProject {
            of: acts,
            said: beside.installation.no_project.clone(),
            withdrawn: beside.calls.withdrawn.clone(),
        }),
        tools: Arc::new(ByProject {
            of: tools,
            said: beside.installation.no_project.clone(),
            withdrawn: beside.calls.withdrawn.clone(),
        }),
        outside: Arc::new(outside),
        host: host_look,
        tokens,
        calls: beside.calls.clone(),
    };
    Ok((carriers, watches))
}

/// The tool `query` as the files beside the flows say, on the store in `data`: always open, held to the bounds every
/// installation has ([`tools::Limits::FIXED`]).
fn querying(beside: &flow::agents::Agents, data: &Path) -> Querying {
    Querying::new(tools::Limits::FIXED, beside.tools.get(QUERY), data, store::SCHEMA)
}

/// What fills `{store}` in the words of the tool `query`, which the line's system prompt holds: the store as the
/// comments of its schema describe it. It is made outside the tools, which the loop holds as one trait object, so that
/// the prompt, made from the same files, can be given it.
pub fn store(beside: &flow::agents::Agents) -> String {
    querying(beside, Path::new("")).store()
}

/// The acts, with their words and what their check holds to the owner's yes, the tools and the watch of main, on one
/// host they share, and the reference that holds main's head as the host last gave it.
fn on<H: Host>(
    host: H,
    project: &Project,
    data: &Path,
    tag: &str,
    (words, protecting, look): (Words, acts::Protecting, Option<Arc<dyn service::Network>>),
    query: Querying,
    said: tools::Said,
) -> (Arc<dyn Rounds>, Arc<dyn ToolsTrait>, Watch, String) {
    let main = host.kept_main();
    let host = Arc::new(host);
    let acts = Acts::new(
        host.clone(),
        project.clone(),
        data.to_path_buf(),
        tag.to_string(),
        words,
    )
    .protecting(protecting);
    let acts = match look {
        Some(look) => acts.looking(look),
        None => acts,
    };
    let watch = acts.watch(WATCH);
    let tools = Tools::new(host, project.clone(), data.to_path_buf(), tag.to_string(), query, said);
    (Arc::new(acts), Arc::new(tools), watch, main)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    #[test]
    fn what_a_request_that_cannot_be_read_is_told_is_what_the_repositorys_file_of_the_tool_says() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow");
        let beside = flow::agents::read(&root).expect("what is written beside the flows is read");
        let told = super::tools::Said::of(&beside)
            .expect("the file of run_tests says what it tells")
            .unread
            .expect("the file of run_tests says it");
        assert!(
            told.starts_with("Nothing was run: ") && told.contains("{error}"),
            "{told}"
        );
    }

    #[test]
    fn the_project_declares_its_two_workflows_how_it_merges_and_where_its_tests_are() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let beside = flow::agents::read(&root.join("flow")).expect("what is written beside the flows is read");
        let text = std::fs::read_to_string(root.join(flow::agents::DECLARATION)).expect("the declaration is read");
        let project = flow::agents::Declaration::read(&text, flow::agents::DECLARATION, &beside)
            .expect("the repository's own declaration is one");
        let ci = project.ci.expect("it declares its workflows");
        assert_eq!(
            (ci.fast.workflow.as_str(), ci.fast.too_long.as_str()),
            ("fast.yml", "2h")
        );
        let full = ci.full;
        assert_eq!(
            (full.workflows.as_slice(), full.too_long.as_str()),
            (&["ci.yml".to_string()][..], "2h")
        );
        let pull = project
            .pull_request
            .expect("it declares how its pull requests are merged");
        assert_eq!((pull.merge_method.as_str(), pull.delete_branch), ("squash", true));
        let tests = project.tests.expect("it declares its tests");
        assert!(!tests.paths.is_empty() && !tests.expected.is_empty());
    }

    #[test]
    fn the_local_host_is_told_its_own_words_for_run_tests_and_github_the_others() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../flow");
        let mut beside = flow::agents::read(&root).expect("what is written beside the flows is read");
        let github = beside.tools["run_tests"].words.clone();
        assert!(github.contains("the last commit of the work's branch"), "{github}");
        crate::projects::local_words(&mut beside);
        let local = &beside.tools["run_tests"].words;
        assert!(
            local.contains("your worktree as it is, including what is not committed"),
            "{local}"
        );
        assert_ne!(local, &github);
    }
}
