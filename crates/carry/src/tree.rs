//! The design tree on a work's branch, as the data of the source [`TREE`], read in the step that reads the work's branch
//! (`crate::branch`): a new version of it each time the tree differs from the last version. The version is the
//! branch's commit and the id of the git tree of the design tree at it, so that the files of the tree that changed
//! between the version a cell last worked from and the latest are one `git diff --name-only` of the two commits
//! ([`changed`]), which a round is given as what changed in the datum.
//!
//! As a round ends whose return writes a block handed in only with the owner's answers in the tree
//! (`answers_in_the_tree` of its file), the engine also looks whether the work's own change against main changes the
//! tree at all, where a question of the work marked `tree` has the owner's answer ([`unwritten`]): the return is sent
//! back for each such question while it does not.

use std::path::Path;

use decide::questions::{Standing, questions};
use work::{Question, Said, Work};

use crate::git::{self, Place};
use crate::settings::Project;

/// The source outside the work that writes the block of the design tree.
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

/// The content of the new version the work's branch makes of the design tree in this directory of the checkout after
/// `last`, the latest there is, where it makes one ([`makes_a_version`]).
pub(crate) async fn read(place: &Place, tree: &str, last: Option<&str>) -> Result<Option<String>, String> {
    let now = head(place, tree).await?;
    let last = last.and_then(Version::read);
    Ok(makes_a_version(last.as_ref(), &now).then(|| now.content()))
}

/// The files of the design tree that changed from the version that says `was` to the one that says `now`, one to a line
/// ([`between`]). What a cell is given of the tree is the files that changed since the version its session last read.
pub(crate) async fn changed(dir: &Path, tree: &str, was: &str, now: &str) -> Result<String, String> {
    Ok(between(dir, tree, was, now).await?.join("\n"))
}

/// The files of the design tree that changed between the versions these contents say, as `git diff --name-only` gives
/// them for their commits, in the directory of the checkout that holds the tree; none where a content is not one of the
/// tree's.
async fn between(dir: &Path, tree: &str, was: &str, now: &str) -> Result<Vec<String>, String> {
    let (Some(was), Some(now)) = (Version::read(was), Version::read(now)) else {
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

/// The name of each question of the work whose answer the design tree in this directory of the checkout does not hold,
/// as a round ends whose return writes a block handed in only with the answers there (`answers_in_the_tree` of its
/// file), the return saying this beside its work: each question marked `tree` that has the owner's answer to its latest
/// version ([`answered`]), where the work's own change against main changes no file of the tree; none where it changes
/// one. In the order of their names. A question kept before questions said `tree` is marked by none.
pub(crate) async fn unwritten(
    project: &Project,
    place: &Place,
    main: &str,
    tree: &str,
    work: &Work,
    said: &Said,
) -> Result<Vec<String>, String> {
    let answered = answered(work, said);
    if answered.is_empty() || changed_by_the_work(project, place, main, tree).await? {
        return Ok(Vec::new());
    }
    Ok(answered)
}

/// Each question of the work marked `tree` that has the owner's answer to its latest version, by its name, in the order
/// of the names: but one that the return saying this asks anew or withdraws, which has no answer to its latest version
/// once the return is taken.
fn answered(work: &Work, said: &Said) -> Vec<String> {
    let marked = |q: &Standing<'_>| matches!(work.question(q.latest), Some(Question::Asked { tree: Some(true), .. }));
    let rewritten = |name: &str| said.questions.iter().any(|q| q.name() == name);
    (questions(&work.records).into_iter())
        .filter(|(name, q)| q.stands() && q.answer.is_some() && marked(q) && !rewritten(name))
        .map(|(name, _)| name.to_string())
        .collect()
}

/// Whether the work's own change against main, from where its branch left main as the reference `main` holds it to
/// the branch's head, changes a file of the design tree in this directory of the checkout. Git is run as the look at
/// the protected paths runs it: the configuration that could make it run a program is refused first, and no external
/// diff or conversion is run.
async fn changed_by_the_work(project: &Project, place: &Place, main: &str, tree: &str) -> Result<bool, String> {
    git::configuration(project, place).await?;
    let head = place.reference();
    let base = git::git(&place.dir, &["merge-base", main, head.as_str()]).await?;
    let names = git::git(
        &place.dir,
        &[
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "--no-renames",
            "--name-only",
            "-z",
            base.as_str(),
            head.as_str(),
            "--",
            tree.trim_end_matches('/'),
        ],
    )
    .await?;
    Ok(!names.is_empty())
}

#[cfg(test)]
mod tests;
