//! The work's head, as the data of the source [`HEAD`]: the head of the work's branch without the merges on top of it
//! that bring only main's code, read in the step that reads the design tree (`crate::branch`). A merge brings only
//! main's code where one of its parents is on main and its tree is the tree git makes of a clean merge of its two
//! parents, as `git merge-tree` computes it, whoever made it and whatever its words; so a merge of the engine's that an
//! agent amended to slip in a change of its own is the work's, and moves the head. A version is the commit; between two
//! versions a round is given the command that shows what the work itself changed, without main's code ([`between`]),
//! the diff from the base it chooses ([`base`]), whose files the look at the run of a line that changes only the
//! project's tests takes too (`crate::tests_only`).

use std::path::Path;

use crate::git::{self, Place};

/// The source outside the work that writes the block of the work's head.
pub const HEAD: &str = "head";

/// The work's head on the branch of the place, main being where the reference `main` holds it: the branch's head, and
/// below it, through each merge on top of it that brings only main's code ([`brings_only_mains`]), the parent on the
/// work's side; or the first commit on main it comes to, a branch that holds no change of its own. Said with whether it
/// is such a commit.
pub(crate) async fn at(place: &Place, main: &str) -> Result<(String, bool), String> {
    let dir = &place.dir;
    let branch = format!("{}^{{commit}}", place.reference());
    let mut at = git::git(dir, &["rev-parse", "--verify", &branch]).await?;
    loop {
        if under(dir, &at, main).await? {
            return Ok((at, true));
        }
        match brings_only_mains(dir, main, &at).await? {
            Some(side) => at = side,
            None => return Ok((at, false)),
        }
    }
}

/// The content of the work's new version of its head after `last`, the latest there is: the work's head, where it is not
/// what `last` says; none where it is, or where there is no version yet and the branch holds no change of its own, the
/// head being a commit on main, as a work's branch is as it starts.
pub(crate) async fn read(place: &Place, main: &str, last: Option<&str>) -> Result<Option<String>, String> {
    let (head, on_main) = at(place, main).await?;
    let new = match last {
        Some(last) => last != head,
        None => !on_main,
    };
    Ok(new.then_some(head))
}

/// Whether the commit is `of`, or under it: on main, where `of` is main.
async fn under(dir: &Path, commit: &str, of: &str) -> Result<bool, String> {
    let said = git::said(dir, &["merge-base", "--is-ancestor", commit, of]).await?;
    match said.code {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err(format!(
            "git merge-base --is-ancestor {commit} {of} failed: {}",
            said.err
        )),
    }
}

/// The parent on the work's side of the commit, where the commit is a merge that brings only main's code: it has two
/// parents, one of them on main, and its tree is the one git makes of a clean merge of the two; the other parent is the
/// work's side, the first where the second is on main. None where the commit is no such merge.
pub(crate) async fn brings_only_mains(dir: &Path, main: &str, commit: &str) -> Result<Option<String>, String> {
    let parents = git::git(dir, &["log", "-1", "--format=%P", commit]).await?;
    let parents: Vec<&str> = parents.split_whitespace().collect();
    let [first, second] = parents.as_slice() else {
        return Ok(None);
    };
    let side = if under(dir, second, main).await? {
        first
    } else if under(dir, first, main).await? {
        second
    } else {
        return Ok(None);
    };
    Ok(clean(dir, commit, first, second).await?.then(|| side.to_string()))
}

/// Whether the merge's tree is the tree git makes of a clean merge of these, its two parents.
pub(crate) async fn clean(dir: &Path, merge: &str, first: &str, second: &str) -> Result<bool, String> {
    let Some(merged) = merged(dir, first, second).await? else {
        return Ok(false);
    };
    let tree = git::git(dir, &["rev-parse", "--verify", &format!("{merge}^{{tree}}")]).await?;
    Ok(merged == tree)
}

/// The tree git makes of a clean merge of the two commits, written into the repository's objects; none where they
/// conflict.
async fn merged(dir: &Path, one: &str, other: &str) -> Result<Option<String>, String> {
    let said = git::said(dir, &["merge-tree", "--write-tree", one, other]).await?;
    match said.code {
        Some(0) => Ok(said.out.lines().next().map(str::to_string)),
        Some(1) => Ok(None),
        _ => Err(format!(
            "git merge-tree --write-tree {one} {other} failed: {}",
            said.err
        )),
    }
}

/// The latest commit of main the commit holds; none where it shares no history with main.
async fn held_main(dir: &Path, main: &str, commit: &str) -> Result<Option<String>, String> {
    let said = git::said(dir, &["merge-base", main, commit]).await?;
    match said.code {
        Some(0) => Ok(Some(said.out)),
        Some(1) => Ok(None),
        _ => Err(format!("git merge-base {main} {commit} failed: {}", said.err)),
    }
}

/// What the diff to the work's head `now` that shows what the work itself changed since its head `was` is taken from
/// ([`base`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Base {
    /// `was`, or the tree git makes of a clean merge of `was` with the later main `now` holds: the diff from it to
    /// `now` is what the work itself changed between the two, without main's code.
    Work(String),
    /// The main `now` holds, where the two cannot be compared so: the diff from it to `now` is the whole change
    /// against main.
    Main(String),
}

impl Base {
    /// The commit or the tree the diff is taken from.
    pub(crate) fn taken_from(&self) -> &str {
        match self {
            Base::Work(from) | Base::Main(from) => from,
        }
    }
}

/// What the diff that shows what the work itself changed from its head `was` to its head `now`, without main's code, is
/// taken from: where `now` holds the main `was` held, `was`; where it holds a later main, the tree git makes of a clean
/// merge of `was` with that main; and where the two cannot be compared so, as where `was` does not merge with that main
/// without a conflict or `now` holds an older main than `was`, the main `now` holds.
pub(crate) async fn base(dir: &Path, main: &str, was: &str, now: &str) -> Result<Base, String> {
    let Some(held) = held_main(dir, main, now).await? else {
        return Ok(Base::Work(was.to_string()));
    };
    let held_before = held_main(dir, main, was).await?;
    if held_before.as_deref() == Some(held.as_str()) {
        return Ok(Base::Work(was.to_string()));
    }
    let later = match &held_before {
        Some(before) => under(dir, before, &held).await?,
        None => false,
    };
    if later && let Some(tree) = merged(dir, was, &held).await? {
        return Ok(Base::Work(tree));
    }
    Ok(Base::Main(held))
}

/// The command that shows what the work itself changed from its head `was` to its head `now`, without main's code: the
/// diff from its [`base`] to `now`, the whole change against main where the two cannot be compared so.
pub(crate) async fn between(dir: &Path, main: &str, was: &str, now: &str) -> Result<String, String> {
    let base = base(dir, main, was, now).await?;
    Ok(format!("`git diff {} {now}`", base.taken_from()))
}

#[cfg(test)]
mod tests;
