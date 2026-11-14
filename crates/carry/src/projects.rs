//! The projects of the installation, as the carriers carry their works out. An item belongs to one project, whose
//! repository its works run in: each project has its own settings, its own host, and its own declaration,
//! `flow/project.toml` on its main as its host last gave it ([`flow::agents::DECLARATION`]), which takes the place of
//! the one beside the flows in what its works' agents are given: its design tree, its files, its settings, its tests and
//! its workflows. What is beside the flows is otherwise the same for every project, and read once.
//!
//! The acts and the tools of each project are its own, on its host; what the loop gives them reaches the project of the
//! work's item ([`ByProject`]). A work whose item has no project these settings name has its round fail, saying so, and
//! its call answered in the words beside the flows: nothing of it is carried out in a repository that is not its own.
//! A work whose item has no project at all, as one added before items had projects, made nothing in any repository the
//! engine knows of now, and is let go of as it ends.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use flow::agents::{NoProject, fill};
use service::{CallReply, Release, ReleaseReply, Round, RoundReply, Rounds, ToolCall, Tools};
use store::Item;
use work::Spent;

use crate::settings::{Project, Settings};
use crate::tools;

/// A project as the carriers carry its works out.
#[derive(Clone, Debug)]
pub struct Declared {
    /// Its settings.
    pub settings: Project,
    /// What is written beside the flows, with the project's declaration in place of the one beside them and, where its
    /// host is its local repository, the words of `run_tests` for that host.
    pub beside: flow::agents::Agents,
}

/// Each project the settings name whose declaration can be read, by its name, with its declaration; and every fault
/// found, each said with the projects' keys: a declaration that cannot be read from its project's main, and two
/// projects that name one repository, whose main, branches and merges they would share.
pub fn declared(settings: &Settings, beside: &flow::agents::Agents) -> (BTreeMap<String, Declared>, Vec<String>) {
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
    for (name, project) in &settings.projects {
        let mut beside = match declaration(project, beside) {
            Ok(beside) => beside,
            Err(why) => {
                faults.push(format!("projects.{name}: {why}"));
                continue;
            }
        };
        if project.github.is_none() {
            local_words(&mut beside);
        }
        declared.insert(
            name.clone(),
            Declared {
                settings: project.clone(),
                beside,
            },
        );
    }
    (declared, faults)
}

/// What is beside the flows with the project's declaration in place of the one beside them, the declaration read from
/// the project's main as its host last gave it, where works branch from, and never from its checkout's files, which the
/// engine does not move for a project hosted elsewhere; or why it cannot be read.
fn declaration(project: &Project, beside: &flow::agents::Agents) -> Result<flow::agents::Agents, String> {
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
    beside.declared(&text, &at)
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
/// owner, who reads them where the work is paused.
pub(crate) fn of<'a, T>(of: &'a BTreeMap<String, T>, item: &Item) -> Result<&'a T, String> {
    let number = item.number.0;
    match &item.project {
        Some(name) => of.get(name).ok_or_else(|| {
            format!("the item #{number} belongs to the project {name}, which the installation's settings do not name")
        }),
        None => Err(format!(
            "the item #{number} belongs to no project: it was added before items had projects, and its work is carried \
             out in no repository"
        )),
    }
}

/// What each project carries out, by the project's name: the rounds or the tools of each, given what the loop gives
/// them for the works of that project's items.
pub struct ByProject<T: ?Sized> {
    pub of: BTreeMap<String, Arc<T>>,
    /// What a tool's result says where the work's item belongs to no project of these, as the files beside the flows
    /// word it.
    pub said: NoProject,
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
}

impl Tools for ByProject<dyn Tools> {
    fn call(&self, call: ToolCall, reply: CallReply) {
        match of(&self.of, &call.item) {
            Ok(tools) => tools.call(call, reply),
            // No round of such a work is carried out, so a call of it was asked before its item's project was lost.
            Err(_) => reply.done(match &call.item.project {
                Some(project) => fill(&self.said.unnamed, &[("project", project.as_str())]),
                None => self.said.none.clone(),
            }),
        }
    }

    fn withdraw(&self, call: ToolCall, reply: CallReply) {
        match of(&self.of, &call.item) {
            Ok(tools) => tools.withdraw(call, reply),
            Err(_) => reply.done(service::WITHDRAWN.to_string()),
        }
    }
}

#[cfg(test)]
mod tests;
