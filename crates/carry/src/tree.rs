//! The design tree on a work's branch, as the data of the source [`TREE`], read in the step that reads the work's branch
//! (`crate::branch`): a new version of it each time the tree differs from the last version. The version is the
//! branch's commit and the id of the git tree of the design tree at it, so that the files of the tree that changed
//! between the version a cell last worked from and the latest are one `git diff --name-only` of the two commits
//! ([`changed`]), which a round is given as what changed in the datum.
//!
//! As a round ends whose return writes a block handed in only with the owner's answers in the tree
//! (`answers_in_the_tree` of its file), the engine also looks whether the work's own change against main changes the
//! tree at all, where a question of the work marked `tree` has the owner's answer ([`unwritten`]): the return is sent
//! back for each such question while it does not. The check before the merge makes the same look, and fails while it
//! does not (`crate::acts`): an answer given after the change was handed in, taking the recommended option to a
//! question that does not wait, runs no cell, so no return is held to it.
//!
//! A block whose lines name entries of the tree (`names_tree_entries` of its file) is read by the tree's format
//! (`agents/tree.toml`), which says how an entry is found and referred to: a return that writes it is sent back for
//! each line that names no entry the tree holds at the head of the work's branch ([`unknown`]), and a round that lacks
//! its latest version is given, in place of the names, each entry named whole, as the tree's latest version has it
//! ([`entries`]).
//!
//! Where the work's own change against main changes the tree, the files of the tree at the commit are held to the
//! forms of the tree's rules ([`unformed`]), as a round of a line that may change the work ends, and as the check
//! before the merge looks at the commit that would merge.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use decide::questions::{Standing, questions};
use flow::agents::{Entries, Tree, TreeFormat, fill};
use work::{Question, Said, Work};

use crate::git::{self, Place};
use crate::settings::Project;

mod forms;

/// The source outside the work that writes the block of the design tree.
pub const TREE: &str = flow::TREE;

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

/// The commit of the version of the tree a content says; none where it says none.
pub(crate) fn commit_of(content: &str) -> Option<String> {
    Version::read(content).map(|v| v.commit)
}

/// A file of the design tree, by its path, with the entries it holds, each by its kind and name, whole.
struct TreeFile {
    path: String,
    entries: Vec<(String, String, String)>,
}

/// The files of the design tree in this directory of the checkout, at the commit, that are under the directory of a
/// kind of the format, each by its kind and its path from the root of the repository: in the order of the kinds and
/// then by the byte order of their paths, which puts a concept's file before its subdirectory's, each once. Their
/// paths come from git, and no text an agent wrote is given to git.
async fn listed(dir: &Path, tree: &str, format: &TreeFormat, commit: &str) -> Result<Vec<(String, String)>, String> {
    let root = tree.trim_end_matches('/');
    let mut paths: Vec<(String, String)> = Vec::new();
    for kind in &format.kinds {
        let under = kind.dir.trim_end_matches('/');
        let under = if root.is_empty() {
            under.to_string()
        } else {
            format!("{root}/{under}")
        };
        let listed = git::git(dir, &["ls-tree", "-r", "-z", "--name-only", commit, &under]).await?;
        let mut these: Vec<String> = (listed.split('\0'))
            .filter(|p| !p.is_empty())
            .map(str::to_string)
            .collect();
        these.sort();
        for path in these {
            if !paths.iter().any(|(_, held)| *held == path) {
                paths.push((kind.kind.clone(), path));
            }
        }
    }
    Ok(paths)
}

/// The files of the design tree in this directory of the checkout, at the commit, that hold its entries, as the
/// format finds them ([`listed`]).
async fn tree_files(dir: &Path, tree: &str, format: &TreeFormat, commit: &str) -> Result<Vec<TreeFile>, String> {
    let paths = listed(dir, tree, format, commit).await?;
    let named: Vec<&str> = paths.iter().map(|(_, path)| path.as_str()).collect();
    let texts = git::blobs(dir, commit, &named).await?;
    Ok((paths.into_iter().zip(texts))
        .filter_map(|((_, path), text)| {
            let text = String::from_utf8(text?).ok()?;
            Some(TreeFile {
                entries: entries_of(format, &text),
                path,
            })
        })
        .collect())
}

/// The entries of a file of the tree, in their order: each from a line the format's `heading` fits to the line before
/// the next it fits, of any kind, or to the file's end, less the blank lines at its end.
fn entries_of(format: &TreeFormat, text: &str) -> Vec<(String, String, String)> {
    let lines: Vec<&str> = text.lines().collect();
    let begins: Vec<(usize, &str, &str)> = (lines.iter().enumerate())
        .filter_map(|(i, line)| format.heading_of(line).map(|(kind, name)| (i, kind, name)))
        .collect();
    let mut entries = Vec::new();
    for (n, (start, kind, name)) in begins.iter().enumerate() {
        let end = begins.get(n + 1).map_or(lines.len(), |(next, ..)| *next);
        let mut whole = &lines[*start..end];
        while let [rest @ .., last] = whole
            && last.trim().is_empty()
        {
            whole = rest;
        }
        entries.push((kind.to_string(), name.to_string(), whole.join("\n")));
    }
    entries
}

/// Each line written of a block whose lines name entries of the tree, trimmed, each once, in its order, but a blank one.
fn named_lines(text: &str) -> Vec<&str> {
    let mut lines: Vec<&str> = Vec::new();
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if !lines.contains(&line) {
            lines.push(line);
        }
    }
    lines
}

/// Where the entry a line refers to is among these files, by the file's place and the entry's in it; none where the
/// line is not written as the format refers to an entry, or names one the files do not hold.
fn found(format: &TreeFormat, files: &[TreeFile], line: &str) -> Option<(usize, usize)> {
    let (kind, name) = format.referred(line)?;
    (files.iter().enumerate()).find_map(|(f, file)| {
        let e = (file.entries.iter()).position(|(k, n, _)| k == kind && n == name)?;
        Some((f, e))
    })
}

/// What a round is given in place of the names a block's lines write (`names_tree_entries`), as the design tree in
/// this directory of the checkout has the entries at the commit, none where the tree has no version: each entry named,
/// whole, once, those of one file after the words of `file` with its path, the files in the order [`tree_files`] gives
/// them and the entries in their file's order; and after the files, each line that names none of them, in the words of
/// `gone`. The parts are joined by a blank line.
pub(crate) async fn entries(
    dir: &Path,
    tree: &str,
    format: &TreeFormat,
    commit: Option<&str>,
    names: &str,
    words: &Entries,
) -> Result<String, String> {
    let files = match commit {
        Some(commit) => tree_files(dir, tree, format, commit).await?,
        None => Vec::new(),
    };
    let mut chosen: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
    let mut gone = Vec::new();
    for line in named_lines(names) {
        match found(format, &files, line) {
            Some((f, e)) => {
                chosen.entry(f).or_default().insert(e);
            }
            None => gone.push(fill(&words.gone, &[("reference", line)])),
        }
    }
    let mut parts = Vec::new();
    for (f, of_file) in &chosen {
        let file = &files[*f];
        parts.push(fill(&words.file, &[("path", file.path.as_str())]));
        parts.extend(of_file.iter().map(|e| file.entries[*e].2.clone()));
    }
    parts.extend(gone);
    Ok(parts.join("\n\n"))
}

/// Each line written of a block whose lines name entries of the tree (`names_tree_entries`) that names none the design
/// tree in this directory of the checkout holds at the commit, trimmed, each once, in its order: one not written as
/// the format refers to an entry, or naming one the tree lacks.
pub(crate) async fn unknown(
    dir: &Path,
    tree: &str,
    format: &TreeFormat,
    commit: &str,
    names: &str,
) -> Result<Vec<String>, String> {
    let files = tree_files(dir, tree, format, commit).await?;
    Ok((named_lines(names).into_iter())
        .filter(|line| found(format, &files, line).is_none())
        .map(str::to_string)
        .collect())
}

/// The name of each question of the work whose answer the design tree in this directory of the checkout does not hold,
/// as a round ends whose return writes a block handed in only with the answers there (`answers_in_the_tree` of its
/// file), the return saying this beside its work, or as the check before the merge looks, with no return, which says
/// nothing: each question marked `tree` that has the owner's answer to its latest version ([`answered`]), where the
/// work's own change against main, at `main`, changes no file of the tree; none where it changes one. In the order of
/// their names. A question kept before questions said `tree` is marked by none.
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

/// Whether the work's own change against main, from where its branch left main as `main` names it, a reference or a
/// commit, to the branch's head, changes a file of the design tree in this directory of the checkout. Git is run as the
/// look at the protected paths runs it: the configuration that could make it run a program is refused first, and no
/// external diff or conversion is run.
pub(crate) async fn changed_by_the_work(
    project: &Project,
    place: &Place,
    main: &str,
    tree: &str,
) -> Result<bool, String> {
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

/// What the files of the design tree in this directory of the checkout, at the commit, are told of their forms where the
/// work's own change against main, at `main`, changes the tree: one line for each fault the check finds in the files
/// under the directory of each kind, in the flow's words, none where the change leaves the tree alone or the files
/// are in their forms. Nothing is said of a tree the work did not change, which main holds as it is.
pub(crate) async fn unformed(
    project: &Project,
    place: &Place,
    main: &str,
    tree: &str,
    rules: &Tree,
    commit: &str,
) -> Result<Vec<String>, String> {
    if !changed_by_the_work(project, place, main, tree).await? {
        return Ok(Vec::new());
    }
    formed(&place.dir, tree, rules, commit).await
}

/// One line for each fault in the forms of the files of the design tree in this directory of the checkout at the
/// commit, in the flow's words ([`forms::words`]). A file a line gives as its data is held where the commit holds a file
/// or a directory of that path; a path that is not a plain path from the root of the repository is held by none, and
/// is not given to git.
async fn formed(dir: &Path, tree: &str, rules: &Tree, commit: &str) -> Result<Vec<String>, String> {
    let paths: Vec<(String, String)> = (listed(dir, tree, &rules.format, commit).await?)
        .into_iter()
        .filter(|(_, path)| path.ends_with(".md"))
        .collect();
    let named: Vec<&str> = paths.iter().map(|(_, path)| path.as_str()).collect();
    let texts = git::blobs(dir, commit, &named).await?;
    let sources: Vec<forms::Source> = (paths.into_iter().zip(texts))
        .map(|((kind, path), text)| forms::Source {
            path,
            kind,
            text: text.and_then(|text| String::from_utf8(text).ok()),
        })
        .collect();
    let read = forms::read(&sources, &rules.format);
    let data = read.data();
    let asked: Vec<&str> = (data.iter().map(|path| path.trim_end_matches('/')))
        .filter(|path| plain(path))
        .collect();
    let held = git::holds(dir, commit, &asked).await?;
    let held: BTreeSet<&str> = asked
        .into_iter()
        .zip(held)
        .filter(|(_, held)| *held)
        .map(|(path, _)| path)
        .collect();
    let faults = read.finish(&|path| held.contains(path.trim_end_matches('/')));
    Ok(forms::words(&faults, &rules.faults))
}

/// Whether a path is a plain path from the root of a repository: not empty, not from the root of the machine, and of
/// no empty part, no part that is `.` or `..`, and no line break or NUL.
fn plain(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.contains(['\0', '\n'])
        && path
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

#[cfg(test)]
pub(crate) mod tests;
