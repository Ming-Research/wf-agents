//! What the engine reads of a work's branch as data of the work, in one step for each: the design tree (`crate::tree`)
//! and the work's head (`crate::head`), each the block of its source, read at the work's branch and not at what its
//! worktree has checked out. A version is written only where what is read differs from the latest. The step is taken
//! as the branch takes main in for a run, at the end of each round of the work, by the check before it judges and by
//! the merge before it merges (decision a-version-read-at-a-rounds-end-is-that-rounds): read at the end of an agent's round on a line that may
//! change the work, a version is what the round committed, which its run hands in; read anywhere else, it is no round's
//! doing and is written from outside, put into the round where the round's own act read it, as taking main in. A round
//! that failed hands in nothing: what it committed is read at the end of the round that takes it up. A new version of
//! the head is kept under a reference of the engine's own as it is read (`crate::head::keep`).

use std::collections::BTreeSet;
use std::path::Path;

use decide::{BlockId, RunId};
use flow::Compiled;
use service::Teller;
use work::{Handed, Work};

use crate::git::Place;
use crate::{head, tree};

/// What a datum of the branch is read with: the directory of the checkout that holds the design tree, and the reference
/// that holds main's head as its host last gave it.
#[derive(Clone, Debug)]
pub(crate) struct Reading {
    pub(crate) tree: String,
    pub(crate) main: String,
}

/// A datum the engine reads of the work's branch, by the source that writes its block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Datum {
    Tree,
    Head,
}

impl Datum {
    /// The datum of the source; none where the source is not the branch's.
    fn of(source: &str) -> Option<Datum> {
        match source {
            tree::TREE => Some(Datum::Tree),
            head::HEAD => Some(Datum::Head),
            _ => None,
        }
    }

    /// The content of the new version the branch makes of the datum after the latest, which says `last`, where it makes
    /// one.
    async fn read(self, place: &Place, reading: &Reading, last: Option<&str>) -> Result<Option<String>, String> {
        match self {
            Datum::Tree => tree::read(place, &reading.tree, last).await,
            Datum::Head => head::read(place, &reading.main, last).await,
        }
    }

    /// What changed from the version that says `was` to the one that says `now`, as a round is given it: the files of
    /// the design tree, one to a line; of the head, the command that shows what the work itself changed.
    async fn changed(self, dir: &Path, reading: &Reading, was: &str, now: &str) -> Result<String, String> {
        match self {
            Datum::Tree => tree::changed(dir, &reading.tree, was, now).await,
            Datum::Head => head::between(dir, &reading.main, was, now).await,
        }
    }
}

/// A block of the branch's data, with what its latest version says.
#[derive(Clone, Debug)]
struct Kept {
    block: BlockId,
    datum: Datum,
    last: Option<String>,
}

/// What reads the work's branch for the blocks of its data, and says what it finds.
pub(crate) struct Looking {
    reading: Reading,
    kept: Vec<Kept>,
}

impl Looking {
    /// The look for the work as it is kept, each block of the flow that the branch's data are written into holding what
    /// its latest version says; none where the flow has no such block.
    pub(crate) fn of(flow: &Compiled, work: &Work, reading: Reading) -> Looking {
        let versions = decide::versions(&work.records);
        let kept = (flow.blocks.iter())
            .filter_map(|(block, declared)| {
                let datum = Datum::of(declared.writers.outside.as_deref()?)?;
                let latest = versions.get(block).and_then(|versions| versions.last());
                let last = latest.and_then(|v| work.contents.get(&(block.clone(), v.at))).cloned();
                Some(Kept {
                    block: block.clone(),
                    datum,
                    last,
                })
            })
            .collect();
        Looking { reading, kept }
    }

    /// Reads the work's branch, and gives each block of which it makes a new version with that version's content; the
    /// latest of each is then what was read. Where one cannot be read, none is taken, and the next look is made against
    /// the last versions there are, so that what this one missed is found by it.
    pub(crate) async fn read(&mut self, place: &Place) -> Result<Vec<(BlockId, String)>, String> {
        let mut new = Vec::new();
        for kept in &self.kept {
            if let Some(content) = kept.datum.read(place, &self.reading, kept.last.as_deref()).await? {
                // A new head is kept as it is read, so that what the work committed outlives its branch.
                if kept.datum == Datum::Head {
                    head::keep(place, &content).await?;
                }
                new.push((kept.block.clone(), content));
            }
        }
        for (block, content) in &new {
            if let Some(kept) = self.kept.iter_mut().find(|k| &k.block == block) {
                kept.last = Some(content.clone());
            }
        }
        Ok(new)
    }

    /// Reads the branch, and gives each new version as what the round hands in, which its run writes as it finishes; or
    /// why the branch could not be read, nothing being handed in then.
    pub(crate) async fn handed_in(&mut self, place: &Place) -> Result<Vec<Handed>, String> {
        let new = self.read(place).await?;
        let handed = (new.into_iter())
            .map(|(block, content)| Handed {
                block,
                content,
                matters: true,
                value: None,
            })
            .collect();
        Ok(handed)
    }

    /// Reads the branch, and says each new version to the loop as written from outside while the round is under way:
    /// put into the round where `for_round`, as what the round's own act read, as it took main in. A read that fails is
    /// said on the standard error and makes no version.
    pub(crate) async fn tell(&mut self, place: &Place, teller: &Teller, for_round: bool) {
        match self.read(place).await {
            Ok(new) => {
                for (block, content) in new {
                    if for_round {
                        teller.outside_wrote_for_round(block, content);
                    } else {
                        teller.outside_wrote(block, content);
                    }
                }
            }
            Err(why) => eprintln!("the work's branch could not be read: {why}"),
        }
    }

    /// The work's head among these new versions, where one of them is of the head.
    pub(crate) fn head_in<'n>(&self, new: &'n [(BlockId, String)]) -> Option<&'n str> {
        let head = self.kept.iter().find(|k| k.datum == Datum::Head)?;
        (new.iter())
            .find(|(block, _)| *block == head.block)
            .map(|(_, content)| content.as_str())
    }

    /// What each block of the branch's data says at its latest version, as the last read left it, where it has one.
    pub(crate) fn latest(&self) -> Vec<(BlockId, String)> {
        (self.kept.iter())
            .filter_map(|kept| Some((kept.block.clone(), kept.last.clone()?)))
            .collect()
    }

    /// The look of a flow with no block of the branch's data, which reads nothing.
    #[cfg(test)]
    pub(crate) fn nothing() -> Looking {
        let reading = Reading {
            tree: String::new(),
            main: String::new(),
        };
        Looking {
            reading,
            kept: Vec::new(),
        }
    }

    /// The look of a flow whose one block of the branch's data is the work's head, named after its source, whose latest
    /// version says `last`.
    #[cfg(test)]
    pub(crate) fn head_only(reading: Reading, last: Option<&str>) -> Looking {
        let kept = vec![Kept {
            block: BlockId::from(head::HEAD),
            datum: Datum::Head,
            last: last.map(str::to_string),
        }];
        Looking { reading, kept }
    }

    /// What the latest version of the work's head says, as the last read left it: the work's head where a read since
    /// the look began went well; none where the flow has no head, or it has no version, the branch holding no change of
    /// its own.
    pub(crate) fn head(&self) -> Option<&str> {
        (self.kept.iter())
            .find(|k| k.datum == Datum::Head)
            .and_then(|k| k.last.as_deref())
    }
}

/// The blocks of the flow whose versions the engine compares itself, as git compares them: each a source of the work's
/// branch writes (`context::Carried::compared`).
pub fn compared(flow: &Compiled) -> BTreeSet<BlockId> {
    (flow.blocks.iter())
        .filter(|(_, declared)| declared.writers.outside.as_deref().and_then(Datum::of).is_some())
        .map(|(block, _)| block.clone())
        .collect()
}

/// What changed in the branch's data as the round of this number of the run began, as the context manager is given it
/// (`context::Changes`): of each block of them, from each earlier version to the latest, what its datum says changed.
pub(crate) async fn changes(
    flow: &Compiled,
    work: &Work,
    run: RunId,
    number: usize,
    dir: &Path,
    reading: &Reading,
) -> Result<context::Changes, String> {
    let mut changes = context::Changes::new();
    // The versions as the round began, which the manager reads the records at.
    let began = work::sessions::began_at(work, run, number).unwrap_or_else(|| work.tick());
    let versions = decide::versions(&work.records.before(began));
    for (block, declared) in &flow.blocks {
        let Some(datum) = declared.writers.outside.as_deref().and_then(Datum::of) else {
            continue;
        };
        let Some((latest, earlier)) = versions.get(block).and_then(|all| all.split_last()) else {
            continue;
        };
        let content = |at| work.contents.get(&(block.clone(), at));
        let Some(now) = content(latest.at) else {
            continue;
        };
        for version in earlier {
            let Some(was) = content(version.at) else {
                continue;
            };
            let changed = datum.changed(dir, reading, was, now).await?;
            changes.insert((block.clone(), version.at), changed);
        }
    }
    Ok(changes)
}

/// The judges' heads the check holds the work to: of each cell that writes a block the check's preconditions read and
/// that the block of the work's head runs, as the flow's graph says, what the version of the head it consumed says;
/// none where it consumed none. None where the flow has no block of the head.
pub(crate) fn reviewed(flow: &Compiled, work: &Work, check: &decide::CellId) -> Option<Vec<Option<String>>> {
    let (block, _) = (flow.blocks.iter()).find(|(_, b)| b.writers.outside.as_deref() == Some(head::HEAD))?;
    let judges = decide::judges(&flow.flow, &work.records, check, block);
    let said = |at| work.contents.get(&(block.clone(), at)).cloned();
    Some(judges.into_iter().map(|judge| judge.consumed.and_then(said)).collect())
}

#[cfg(test)]
mod tests;
