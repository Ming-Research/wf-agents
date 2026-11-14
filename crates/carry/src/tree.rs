//! The design tree on a work's branch, as the data of the source [`TREE`]: a new version of it each time the tree
//! differs from the last version, looked at where the branch may have changed: after the branch took main in and after
//! a round of the work ended. The version is the branch's commit and the id of the git tree of the design tree at it, so
//! that the files of the tree that changed between the version a cell last worked from and the latest are one
//! `git diff --name-only` of the two commits ([`files_changed`]), which a round is given as what changed in the datum
//! ([`changes`]).

use std::path::Path;

use decide::{BlockId, RunId, Seq};
use flow::Compiled;
use service::Teller;
use work::Work;

use crate::git::{self, Place};

/// The source outside the works that writes the block of the design tree.
pub const TREE: &str = "tree";

/// What the version holds for the id of the tree where the branch has no design tree.
const ABSENT: &str = "-";

/// A version of the tree: the commit of the branch, and the id of the git tree of the design tree at it, or [`ABSENT`].
#[derive(Clone, Debug, PartialEq, Eq)]
struct Version {
    commit: String,
    tree: String,
}

impl Version {
    /// The version a content says.
    fn read(content: &str) -> Option<Version> {
        let mut words = content.split_whitespace();
        let (commit, tree) = (words.next()?, words.next()?);
        words.next().is_none().then(|| Version {
            commit: commit.to_string(),
            tree: tree.to_string(),
        })
    }

    fn content(&self) -> String {
        format!("{} {}", self.commit, self.tree)
    }
}

/// Whether the branch at `now` makes a new version of the tree after `last`, the latest there is, none where there is
/// none yet. This is the one place that says which commits make a version: any commit at which the design tree is not
/// what the last version holds, whoever changed it, main in what it brought in or the work in a commit of its own, and
/// one that removes it; a branch that has none and has had none makes none.
fn makes_a_version(last: Option<&Version>, now: &Version) -> bool {
    match last {
        Some(last) => last.tree != now.tree,
        None => now.tree != ABSENT,
    }
}

/// The version at the head of the work's branch, whatever the worktree has checked out, with the design tree in this
/// directory of the checkout.
async fn head(place: &Place, tree: &str) -> Result<Version, String> {
    let branch = place.reference();
    let commit = git::git(&place.dir, &["rev-parse", "--verify", &branch]).await?;
    let spec = format!("{branch}:{}", tree.trim_end_matches('/'));
    let id = git::said(&place.dir, &["rev-parse", "--verify", "--quiet", &spec]).await?;
    Ok(Version {
        commit,
        tree: if id.ok { id.out } else { ABSENT.to_string() },
    })
}

/// What looks at the tree on a work's branch for the block of the source [`TREE`], and says what it finds.
pub(crate) struct Looking {
    block: BlockId,
    /// The directory of the checkout that holds the tree.
    tree: String,
    /// The latest version, as the work's records have it and as what the looks since found.
    last: Option<Version>,
}

impl Looking {
    /// The look for the work as it is kept, at the design tree in this directory of the checkout; none where its flow
    /// has no block of the source.
    pub(crate) fn of(flow: &Compiled, work: &Work, tree: &str) -> Option<Looking> {
        let (block, _) = flow
            .blocks
            .iter()
            .find(|(_, block)| block.writers.outside.as_deref() == Some(TREE))?;
        let versions = decide::versions(&work.records);
        let latest = versions.get(block).and_then(|versions| versions.last());
        let last = latest.and_then(|v| work.contents.get(&(block.clone(), v.at)));
        Some(Looking {
            block: block.clone(),
            tree: tree.to_string(),
            last: last.and_then(|content| Version::read(content)),
        })
    }

    /// Looks at the work's branch, and gives the content of the new version it makes, where it makes one.
    pub(crate) async fn next(&mut self, place: &Place) -> Result<Option<String>, String> {
        let now = head(place, &self.tree).await?;
        if !makes_a_version(self.last.as_ref(), &now) {
            return Ok(None);
        }
        let content = now.content();
        self.last = Some(now);
        Ok(Some(content))
    }

    /// Looks as [`Looking::next`] does, and says the new version to the loop, while the round is under way, put into
    /// the round where `for_round`: what the carrier did before the round's program, as taking main in. A look
    /// that fails is said on the standard error and makes no version: the next look is made against the last version
    /// there is, so that what it missed is found by it.
    pub(crate) async fn tell(&mut self, place: &Place, teller: &Teller, for_round: bool) {
        match self.next(place).await {
            Ok(Some(content)) if for_round => teller.outside_wrote_for_round(self.block.clone(), content),
            Ok(Some(content)) => teller.outside_wrote(self.block.clone(), content),
            Ok(None) => {}
            Err(why) => eprintln!("the design tree on the work's branch could not be looked at: {why}"),
        }
    }
}

/// Looks once at the work's branch, for the work as it is kept, and says a new version where there is one.
pub(crate) async fn look(flow: &Compiled, work: &Work, tree: &str, place: &Place, teller: &Teller) {
    if let Some(mut looking) = Looking::of(flow, work, tree) {
        looking.tell(place, teller, false).await;
    }
}

/// The files of the design tree that changed between two versions of its block, as `git diff --name-only` gives them for
/// their commits, in the directory of the checkout that holds the tree; none where a version's content is not one of the
/// tree's. What a cell is given of the tree is the files that changed since the version its session last read.
pub async fn files_changed(
    work: &Work,
    block: &BlockId,
    was: Seq,
    now: Seq,
    dir: &Path,
    tree: &str,
) -> Result<Vec<String>, String> {
    let version = |at| {
        work.contents
            .get(&(block.clone(), at))
            .and_then(|content| Version::read(content))
    };
    let (Some(was), Some(now)) = (version(was), version(now)) else {
        return Ok(Vec::new());
    };
    let names = git::git(
        dir,
        &[
            "diff",
            "--name-only",
            "--no-renames",
            &was.commit,
            &now.commit,
            "--",
            tree.trim_end_matches('/'),
        ],
    )
    .await?;
    Ok(names.lines().map(str::to_string).collect())
}

/// What changed in the design tree as the round of this number of the run began, as the context manager is given it
/// (`context::Changes`): from each earlier version of its block to the latest, the files that changed, one to a line.
/// None where the flow has no block of the source [`TREE`].
pub(crate) async fn changes(
    flow: &Compiled,
    work: &Work,
    run: RunId,
    number: usize,
    dir: &Path,
    tree: &str,
) -> Result<context::Changes, String> {
    let mut changes = context::Changes::new();
    let Some((block, _)) = (flow.blocks.iter()).find(|(_, b)| b.writers.outside.as_deref() == Some(TREE)) else {
        return Ok(changes);
    };
    // The versions as the round began, which the manager reads the records at.
    let began = work::sessions::began_at(work, run, number).unwrap_or_else(|| work.tick());
    let versions = decide::versions(&work.records.before(began));
    let Some((latest, earlier)) = versions.get(block).and_then(|all| all.split_last()) else {
        return Ok(changes);
    };
    for version in earlier {
        let files = files_changed(work, block, version.at, latest.at, dir, tree).await?;
        changes.insert((block.clone(), version.at), files.join("\n"));
    }
    Ok(changes)
}

#[cfg(test)]
mod tests;
