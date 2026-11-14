//! The projects of the installation, as the carriers carry their works out. An item belongs to one project, whose
//! repository its works run in: each project has its own settings, its own host, and its own declaration,
//! `flow/project.toml` in its repository ([`flow::agents::DECLARATION`]), which takes the place of the one beside the
//! flows in what its works' agents are given: its files, its settings, its tests and its workflows. What is beside the
//! flows is otherwise the same for every project.
//!
//! The acts and the tools of each project are its own, on its host; what the loop gives them reaches the project of the
//! work's item ([`ByProject`]). A work whose item has no project these settings name, as an item kept before items had
//! projects, has its round fail and its call answered saying so: nothing of it is carried out in a repository that is
//! not its own.

use std::collections::BTreeMap;
use std::sync::Arc;

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

/// Each project the settings name, by its name, with its declaration read from its repository; or why one cannot be
/// read, said with the project's key.
pub fn declared(settings: &Settings, beside: &flow::agents::Agents) -> Result<BTreeMap<String, Declared>, String> {
    let mut declared = BTreeMap::new();
    for (name, project) in &settings.projects {
        let path = project.repo.join(flow::agents::DECLARATION);
        let mut beside = beside
            .declared(&path)
            .map_err(|why| format!("projects.{name}: {why}"))?;
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
    Ok(declared)
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
pub struct ByProject<T: ?Sized>(pub BTreeMap<String, Arc<T>>);

impl Rounds for ByProject<dyn Rounds> {
    fn give(&self, round: Round, reply: RoundReply) {
        match of(&self.0, &round.item) {
            Ok(rounds) => rounds.give(round, reply),
            Err(why) => reply.failed(why, Spent::default()),
        }
    }

    fn stop(&self, round: Round, reply: RoundReply) {
        match of(&self.0, &round.item) {
            Ok(rounds) => rounds.stop(round, reply),
            // Nothing was carried out for it: the round ends with nothing.
            Err(_) => reply.ended(engine::Came::hands_in(Vec::new()), Spent::default()),
        }
    }

    fn release(&self, release: Release, reply: ReleaseReply) {
        match of(&self.0, &release.item) {
            Ok(rounds) => rounds.release(release, reply),
            Err(why) => reply.failed(why),
        }
    }
}

impl Tools for ByProject<dyn Tools> {
    fn call(&self, call: ToolCall, reply: CallReply) {
        match of(&self.0, &call.item) {
            Ok(tools) => tools.call(call, reply),
            // No round of such a work is carried out, so a call of it was asked before its item's project was known.
            Err(why) => reply.done(why),
        }
    }

    fn withdraw(&self, call: ToolCall, reply: CallReply) {
        match of(&self.0, &call.item) {
            Ok(tools) => tools.withdraw(call, reply),
            Err(_) => reply.done(service::WITHDRAWN.to_string()),
        }
    }
}
