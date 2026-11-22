//! The works of a project under way, as the list the plan review reads: the source [`PARALLEL`], which the loop writes
//! into the block of that source of every work of the project as the list changes.
//!
//! A work is on the list while it has started a run, has not ended, and its flow says in what block its plan names the
//! files it will touch (`planned_in` of the block of the source): a work of a flow with no such block, as a deliverable
//! item's, has neither a branch nor a plan, and is left out. The list says of each work the files its branch has changed
//! and the files its plan names, in the words the flow gives. The files its branch has changed are read from the
//! project's repository by the carrier of the engine's acts, off the loop, so the driver only says when they are to be
//! read: [`Driver::parallel`] gives the list of each project whose works' heads or plans, or whose works under way,
//! differ from those it last gave. What it last gave is kept in memory only, and lost in a stop, when the lists are
//! given once more and written where they differ.

use std::collections::BTreeMap;

use decide::BlockId;
use flow::Compiled;
use flow::agents::fill;
use store::{Item, ItemNumber, WorkNumber};

use crate::{Driver, Held};

/// The source of the block that holds the list of a project's works under way.
pub const PARALLEL: &str = "parallel";

/// The source of the block of a work's head, which the carriers of the work's branch write.
const HEAD: &str = "head";

/// The texts the block of [`PARALLEL`] gives in its `writes`, each by its name: a work's line, the names of the two
/// parts of its entry, what stands for each where it has no files to name, and the reason a repository could not be
/// read.
pub const WORDS: [&str; 6] = ["work", "changed", "planned", "none_yet", "not_stated", "unread"];

/// The words the flow gives for the list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Words {
    /// A work's line, with `{work}` its number and `{item}` its item's.
    pub work: String,
    /// The line that names the files its branch has changed.
    pub changed: String,
    /// The line that names the files its plan names.
    pub planned: String,
    /// What stands for the files its branch has changed where it has changed none, or has no branch yet.
    pub none_yet: String,
    /// What stands for the files its plan names where it names none.
    pub not_stated: String,
    /// What stands for the files its branch has changed where the repository could not be read, with `{why}` the reason.
    pub unread: String,
}

impl Words {
    /// The words the block of [`PARALLEL`] of the flow gives; none where the flow has no such block, or does not give
    /// each ([`crate::faults`] refuses such a flow).
    pub fn of(flow: &Compiled) -> Option<Words> {
        let (_, block) = (flow.blocks.iter()).find(|(_, b)| b.writers.outside.as_deref() == Some(PARALLEL))?;
        let text = |name: &str| block.writes.get(name).map(str::to_string);
        Some(Words {
            work: text(WORDS[0])?,
            changed: text(WORDS[1])?,
            planned: text(WORDS[2])?,
            none_yet: text(WORDS[3])?,
            not_stated: text(WORDS[4])?,
            unread: text(WORDS[5])?,
        })
    }
}

/// A work on the list, with what the list of its project is made from beside what the repository says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Listed {
    pub work: WorkNumber,
    pub item: Item,
    /// The work's head, as the latest version of the block of its head says; none before it has one.
    pub head: Option<String>,
    /// The files its plan names, as the latest version of the block its flow names says; none before it is written.
    pub planned: Option<String>,
}

/// The list of a project's works under way that the carrier of its repository is asked to make, in the order of the
/// works' numbers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ask {
    pub project: String,
    pub words: Words,
    pub works: Vec<Listed>,
}

/// What the repository says of the files a work's branch has changed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Changed {
    /// Its paths, in the order given; none where it has changed none.
    Files(Vec<String>),
    /// The work has no branch yet to tell them from.
    Nothing,
    /// The repository could not be read, for this reason.
    Unread(String),
}

/// A work's entry of the list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub work: WorkNumber,
    pub item: ItemNumber,
    pub changed: Changed,
    /// The files its plan names, as it wrote them; none where it has written none.
    pub planned: Option<String>,
}

/// The list, in the flow's words: for each work its line, then the line that names the files its branch has changed and
/// the files, one to a line, then the line that names the files its plan names and the files. Where there are none to
/// name, what the flow says stands for them stands on a line of its own. An entry is not followed by a blank line.
pub fn text(words: &Words, entries: &[Entry]) -> String {
    let mut lines: Vec<String> = Vec::new();
    for entry in entries {
        let (work, item) = (entry.work.0.to_string(), entry.item.0.to_string());
        lines.push(fill(&words.work, &[("work", &work), ("item", &item)]));
        lines.push(words.changed.clone());
        match &entry.changed {
            Changed::Files(files) if !files.is_empty() => lines.extend(files.iter().cloned()),
            Changed::Files(_) | Changed::Nothing => lines.push(words.none_yet.clone()),
            Changed::Unread(why) => lines.push(fill(&words.unread, &[("why", why)])),
        }
        lines.push(words.planned.clone());
        let planned: Vec<&str> = (entry.planned.iter())
            .flat_map(|planned| planned.lines())
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect();
        match planned.is_empty() {
            true => lines.push(words.not_stated.clone()),
            false => lines.extend(planned.into_iter().map(str::to_string)),
        }
    }
    lines.join("\n")
}

/// The blocks of a flow that its list is made from: the block of [`PARALLEL`], and the block its `planned_in` names; none
/// where the flow has no block of the source or it names no block of the plan.
fn planned_block(flow: &Compiled) -> Option<BlockId> {
    let (_, block) = (flow.blocks.iter()).find(|(_, b)| b.writers.outside.as_deref() == Some(PARALLEL))?;
    block.planned_in.clone()
}

/// The block of the work's head: the block of the flow that the source of the head writes.
fn head_block(flow: &Compiled) -> Option<BlockId> {
    (flow.blocks.iter())
        .find(|(_, b)| b.writers.outside.as_deref() == Some(HEAD))
        .map(|(name, _)| name.clone())
}

/// What the latest version of each of these blocks says, in the order given, of the work's records, where it has one:
/// the records are read once for all of them.
fn latest(held: &Held, blocks: &[Option<BlockId>]) -> Vec<Option<String>> {
    let versions = decide::versions(&held.work.records);
    (blocks.iter())
        .map(|block| {
            let block = block.as_ref()?;
            let version = versions.get(block)?.last()?;
            held.work.contents.get(&(block.clone(), version.at)).cloned()
        })
        .collect()
}

impl Driver {
    /// The list of each project's works under way that the carrier of its repository is to make now: of each project
    /// whose list, with each work's head and the files its plan names, differs from the list last given for it. A
    /// project with no work under way is given none. Kept in memory only (see the module): after a stop, the first
    /// call gives every list again.
    pub fn parallel(&mut self) -> Vec<Ask> {
        if self.fault.is_some() {
            return Vec::new();
        }
        let mut lists: BTreeMap<String, (Words, Vec<Listed>)> = BTreeMap::new();
        for (number, held) in &self.works {
            if held.work.records.started.is_empty() {
                continue;
            }
            let Some(project) = held.item.project.clone() else {
                continue;
            };
            let (Some(planned_in), Some(words)) = (planned_block(&held.flow), Words::of(&held.flow)) else {
                continue;
            };
            let [head, planned] = latest(held, &[head_block(&held.flow), Some(planned_in)])
                .try_into()
                .unwrap_or_default();
            let listed = Listed {
                work: *number,
                item: held.item.clone(),
                head,
                planned,
            };
            lists
                .entry(project)
                .or_insert_with(|| (words, Vec::new()))
                .1
                .push(listed);
        }
        self.parallel_given.retain(|project, _| lists.contains_key(project));
        let mut asks = Vec::new();
        for (project, (words, works)) in lists {
            if self.parallel_given.get(&project) == Some(&works) {
                continue;
            }
            self.parallel_given.insert(project.clone(), works.clone());
            asks.push(Ask { project, words, works });
        }
        asks
    }
}

#[cfg(test)]
mod tests;
