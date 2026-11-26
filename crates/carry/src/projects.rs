//! The projects of the installation, as the carriers carry their works out. An item belongs to one project, whose
//! repository its works run in: each project has its own settings, its own host, and its own declaration,
//! `wf-agents.toml` at the root of its repository, on its main as its host last gave it ([`flow::agents::DECLARATION`]):
//! its design tree, its files, its settings, its tests, its workflows, its heavy commands, the command that
//! checks it where its repository is the host, and how its pull requests are merged where GitHub is. What is beside the
//! flows is the same for every project, and read once, what every agent is refused whatever its project
//! (`agents/refused.toml`) among it.
//!
//! The acts and the tools of each project are its own, on its host; what the loop gives them reaches the project of the
//! work's item ([`ByProject`]). A project whose declaration cannot be used is held, and says why: nothing of its works
//! is carried out, each round of them fails with the reason, which pauses the work with it, and the other projects go
//! on. The declaration is read as the engine starts, so that a held project is carried again by a start after it is
//! mended. A work whose item has no project these settings name has its round fail, saying so, and its call answered
//! in the words beside the flows: nothing of it is carried out in a repository that is not its own. A work whose item
//! has no project at all, as one added before items had projects, made nothing in any repository the engine knows of
//! now, and is let go of as it ends.

use std::collections::BTreeMap;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

use flow::Compiled;
use flow::agents::{Declaration, NoProject, fill};
use service::{
    CallReply, Changes, Diff, OfWork, OpenPull, Release, ReleaseReply, Round, RoundReply, Rounds, Seen, ToolCall, Tools,
};
use store::Item;
use work::Spent;

use crate::settings::{Project, Settings};
use crate::tools;

/// A project as the carriers carry its works out.
#[derive(Clone, Debug)]
pub struct Declared {
    /// Its settings.
    pub settings: Project,
    /// What it declares.
    pub declaration: Declaration,
    /// Its declaration as it was read, which the owner's page shows.
    pub text: DeclarationText,
    /// What is written beside the flows, with, where its host is its local repository, the words of `run_tests` for
    /// that host.
    pub beside: flow::agents::Agents,
}

/// A project's declaration as the engine read it: where, as `<main>:wf-agents.toml`, and its text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeclarationText {
    pub at: String,
    pub text: String,
}

/// What the engine holds of each project's declaration, by the project's name: the declaration as it read it, or why
/// the project is held.
pub type Declarations = BTreeMap<String, Result<DeclarationText, String>>;

/// Each project the settings name, by its name, with its declaration, or with why it is held; and every fault that
/// keeps the engine from starting, each said with the projects' keys: two projects that name one repository, whose
/// main, branches and merges they would share, and what is wrong with the flows' texts whoever's declaration they are
/// read with.
///
/// A project is held, with every reason at once, where its own declaration is the cause: one that cannot be read from
/// its project's main (a time limit of its workflows that is no time among it), or lacks what its host needs
/// ([`needs_of_its_host`]), or does not protect itself, a heavy command of it that names nothing or holds a `*`
/// or a parenthesis ([`crate::permissions::heavy_faults`]), a variable it names for the agents' command line that the
/// engine, the installation or the program owns ([`crate::cli::environment_faults`]), a fast workflow that is given less
/// time than the CI is given to start, a setting the flows' texts name that it lacks, no paths of its tests where a
/// line changes only them, and a file it names that main does not hold ([`crate::system::project_files`]).
pub fn declared(
    settings: &Settings,
    beside: &flow::agents::Agents,
    flows: &[Compiled],
) -> (BTreeMap<String, Result<Declared, String>>, Vec<String>) {
    let mut faults = Vec::new();
    let mut by_repository: BTreeMap<PathBuf, Vec<&str>> = BTreeMap::new();
    for (name, project) in &settings.projects {
        let repository = std::fs::canonicalize(&project.repo).unwrap_or_else(|_| project.repo.clone());
        by_repository.entry(repository).or_default().push(name);
    }
    for (repository, names) in by_repository.iter().filter(|(_, names)| names.len() > 1) {
        let keys: Vec<String> = names.iter().map(|name| format!("projects.{name}")).collect();
        faults.push(format!(
            "{}: name one repository, {}, which is one project's",
            keys.join(", "),
            repository.display()
        ));
    }
    let mut declared = BTreeMap::new();
    let mut of_flow: Vec<String> = Vec::new();
    for (name, project) in &settings.projects {
        let (declaration, text) = match declaration(project, beside) {
            Ok(read) => read,
            Err(why) => {
                declared.insert(name.clone(), Err(why));
                continue;
            }
        };
        // What the project's host makes of the words beside the flows, as its agents are sent them.
        let mut beside = beside.clone();
        if project.github.is_none() {
            local_words(&mut beside);
        }
        let mut why = needs_of_its_host(project, &declaration);
        why.extend(protects_itself(&declaration));
        why.extend(crate::permissions::heavy_faults(&declaration));
        why.extend(crate::cli::environment_faults(
            &declaration.environment,
            &settings.agents.env,
            &beside.refused.environment,
        ));
        for flow in flows {
            let texts = crate::system::faults(settings, &beside, &declaration, flow);
            why.extend(texts.of_project);
            for fault in texts.of_flow {
                if !of_flow.contains(&fault) {
                    of_flow.push(fault);
                }
            }
        }
        why.extend(crate::system::project_files(project, &beside, &declaration));
        if !why.is_empty() {
            declared.insert(name.clone(), Err(why.join("; ")));
            continue;
        }
        declared.insert(
            name.clone(),
            Ok(Declared {
                settings: project.clone(),
                declaration,
                text,
                beside,
            }),
        );
    }
    faults.extend(of_flow);
    (declared, faults)
}

/// The declaration of the project, read from its main as its host last gave it, where works branch from, and never from
/// its checkout's files, which the engine does not move for a project hosted elsewhere, with its text as read there; or
/// why it cannot be read.
fn declaration(project: &Project, beside: &flow::agents::Agents) -> Result<(Declaration, DeclarationText), String> {
    let at = format!("{}:{}", crate::host::kept_main(project), flow::agents::DECLARATION);
    let read = std::process::Command::new("git")
        .arg("-C")
        .arg(&project.repo)
        .args(["cat-file", "blob", &at])
        .output()
        .map_err(|e| format!("git could not be run to read {at}: {e}"))?;
    if !read.status.success() {
        let said = String::from_utf8_lossy(&read.stderr).trim().to_string();
        return Err(format!(
            "{at}, the project's declaration on main where works branch from, cannot be read: {said}"
        ));
    }
    let text = String::from_utf8(read.stdout).map_err(|_| format!("{at}: the declaration is not UTF-8"))?;
    let declaration = Declaration::read(&text, &at, beside)?;
    Ok((declaration, DeclarationText { at, text }))
}

/// What the project's host needs of its declaration and does not find: the command that checks it where its own
/// repository is the host, and how its pull requests are merged where GitHub is, and a fast workflow given at least as
/// long as the CI is given to start. Each is said with where it is written.
fn needs_of_its_host(project: &Project, declaration: &Declaration) -> Vec<String> {
    let at = flow::agents::DECLARATION;
    let mut wrong = Vec::new();
    match &project.github {
        None if declaration.local.is_none() => wrong.push(format!(
            "{at} has no [local] with the `check` command, which the project's host, its own repository, runs"
        )),
        Some(_) if declaration.pull_request.is_none() => wrong.push(format!(
            "{at} has no [pull_request] with how a pull request is merged, which the project's host, GitHub, needs"
        )),
        Some(github) => {
            let limit = (declaration.ci.as_ref()).and_then(|ci| flow::agents::span(&ci.fast.too_long).ok());
            if limit.is_some_and(|limit| limit.as_secs() < github.start_minutes * 60) {
                wrong.push(format!(
                    "{at} [ci.fast] too_long is less than the {} minutes the settings give the CI to start \
                     (github.start_minutes)",
                    github.start_minutes
                ));
            }
        }
        None => {}
    }
    wrong
}

/// Why the declaration does not protect itself: it names the commands the engine runs for the project, so a work's
/// change to it is the owner's to let merge (principle protected-files-are-the-owners), and `[protected]` must hold it.
fn protects_itself(declaration: &Declaration) -> Option<String> {
    let file = flow::agents::DECLARATION;
    let covered = (declaration.protected.paths.iter())
        .any(|path| path == file || (path.ends_with('/') && file.starts_with(path.as_str())));
    (!covered)
        .then(|| format!("{file}: [protected] paths must hold {file} itself, which names commands the engine runs"))
}

/// What `run_tests` is told to do where the host is the local repository: its own words for it, in place of those for
/// GitHub.
pub(crate) fn local_words(beside: &mut flow::agents::Agents) {
    if let Some(tool) = beside.tools.get_mut(tools::RUN_TESTS)
        && let Some(local) = tool.words_local.take()
    {
        tool.words = local;
    }
}

/// What of these, by the name of a project, is the item's project's; or why the item has none of them, in words for the
/// owner, who reads them where the work is paused: a project is held, and says why its declaration cannot be used.
pub(crate) fn of<'a, T>(of: &'a BTreeMap<String, Result<T, String>>, item: &Item) -> Result<&'a T, String> {
    let number = item.number;
    match &item.project {
        Some(name) => match of.get(name) {
            Some(Ok(carried)) => Ok(carried),
            Some(Err(why)) => Err(format!(
                "the item {number} belongs to the project {name}, which is held: {why}"
            )),
            None => Err(format!(
                "the item {number} belongs to the project {name}, which the installation's settings do not name"
            )),
        },
        None => Err(format!(
            "the item {number} belongs to no project: it was added before items had projects, and its work is carried \
             out in no repository"
        )),
    }
}

/// Why the item's project is held, where it is; none where it is carried, not named, or the item has none.
fn held<T>(of: &BTreeMap<String, Result<T, String>>, item: &Item) -> Option<String> {
    let name = item.project.as_ref()?;
    of.get(name)?.as_ref().err()?;
    crate::projects::of(of, item).err()
}

/// What each project carries out, by the project's name: the rounds or the tools of each, given what the loop gives
/// them for the works of that project's items; or why the project is held.
pub struct ByProject<T: ?Sized> {
    pub of: BTreeMap<String, Result<Arc<T>, String>>,
    /// What a tool's result says where the work's item belongs to no project of these, as the files beside the flows
    /// word it.
    pub said: NoProject,
    /// The result of a call withdrawn, as the files beside the flows word it.
    pub withdrawn: String,
}

impl Rounds for ByProject<dyn Rounds> {
    fn give(&self, round: Round, reply: RoundReply) {
        match of(&self.of, &round.item) {
            Ok(rounds) => rounds.give(round, reply),
            Err(why) => reply.failed(why, Spent::default()),
        }
    }

    fn stop(&self, round: Round, reply: RoundReply) {
        match of(&self.of, &round.item) {
            Ok(rounds) => rounds.stop(round, reply),
            // Nothing was carried out for it: the round ends with nothing.
            Err(_) => reply.ended(engine::Came::hands_in(Vec::new()), Spent::default()),
        }
    }

    fn release(&self, release: Release, reply: ReleaseReply) {
        match of(&self.of, &release.item) {
            Ok(rounds) => rounds.release(release, reply),
            // An item of no project had its work carried out in no repository: there is nothing to let go of.
            Err(_) if release.item.project.is_none() => reply.released(),
            Err(why) => reply.failed(why),
        }
    }

    // A work whose item belongs to no project these settings name was carried out in no repository they name: there
    // is nothing of it to tell. A work of a held project may have a worktree and changes that cannot be read while the
    // project is held: that is said, never shown as nothing.
    fn seen(&self, asked: OfWork) -> Pin<Box<dyn Future<Output = Result<Seen, String>> + Send + '_>> {
        match of(&self.of, &asked.item) {
            Ok(rounds) => rounds.seen(asked),
            Err(_) => match held(&self.of, &asked.item) {
                Some(why) => Box::pin(async move { Err::<Seen, String>(why) }),
                None => Box::pin(async { Ok(Seen::default()) }),
            },
        }
    }

    fn changes(&self, asked: OfWork) -> Pin<Box<dyn Future<Output = Result<Option<Changes>, String>> + Send + '_>> {
        match of(&self.of, &asked.item) {
            Ok(rounds) => rounds.changes(asked),
            Err(_) => match held(&self.of, &asked.item) {
                Some(why) => Box::pin(async move { Err::<Option<Changes>, String>(why) }),
                None => Box::pin(async { Ok(None) }),
            },
        }
    }

    fn diff(
        &self,
        asked: OfWork,
        path: String,
    ) -> Pin<Box<dyn Future<Output = Result<Option<Diff>, String>> + Send + '_>> {
        match of(&self.of, &asked.item) {
            Ok(rounds) => rounds.diff(asked, path),
            Err(_) => match held(&self.of, &asked.item) {
                Some(why) => Box::pin(async move { Err::<Option<Diff>, String>(why) }),
                None => Box::pin(async { Ok(None) }),
            },
        }
    }

    // A project the settings do not name, or one held for its declaration, has no host to ask, which is not a host with
    // no pull requests.
    fn open_pulls(
        &self,
        project: &str,
    ) -> Pin<Box<dyn Future<Output = Result<Option<Vec<OpenPull>>, String>> + Send + '_>> {
        match self.of.get(project) {
            Some(Ok(rounds)) => rounds.open_pulls(project),
            Some(Err(why)) => {
                let why = format!("the project {project} is held: {why}");
                Box::pin(async move { Err(why) })
            }
            None => {
                let why = format!("the installation's settings do not name the project {project}");
                Box::pin(async move { Err(why) })
            }
        }
    }
}

impl Tools for ByProject<dyn Tools> {
    fn call(&self, call: ToolCall, reply: CallReply) {
        match of(&self.of, &call.item) {
            Ok(tools) => tools.call(call, reply),
            // No round of such a work is carried out, so a call of it was asked before its item's project was lost.
            Err(_) => reply.done(match (held(&self.of, &call.item), &call.item.project) {
                (Some(why), _) => why,
                (None, Some(project)) => fill(&self.said.unnamed, &[("project", project.as_str())]),
                (None, None) => self.said.none.clone(),
            }),
        }
    }

    fn withdraw(&self, call: ToolCall, reply: CallReply) {
        match of(&self.of, &call.item) {
            Ok(tools) => tools.withdraw(call, reply),
            Err(_) => reply.done(self.withdrawn.clone()),
        }
    }
}

#[cfg(test)]
mod tests;
