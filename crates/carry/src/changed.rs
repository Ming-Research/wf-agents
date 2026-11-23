//! What a work changed, as its page shows it, worked out with git in the project's repository: each file between where
//! the work's branch left main and the work's latest head, together with what its worktree holds that is not committed
//! while the worktree is there; and the unified diff of one of them. Nothing here writes: git is told to take no lock
//! it can do without, and a diff against the worktree is told not to write down the stat information it finds
//! out of date ([`WRITES_NOTHING`]), so that an agent's own git in the worktree is never held up by a look.
//!
//! The work's latest head is its branch while its worktree is there, which holds what it committed since its last round
//! ended; once the worktree is gone, the reference the engine keeps each head under ([`crate::head::kept`]), or, for a
//! work whose heads were read before they were kept so, its branch. Where the branch left main is worked out from the
//! head and main alone ([`base`]), and is the same before the work merges and after; so what a work at a head changed
//! is worked out once for each head and kept in memory, as is the head of a work with no worktree, whose head nothing
//! moves until its worktree is made again ([`Known`]). What a worktree holds changes as its agent works, and is worked
//! out each time it is asked for: two git commands and a read of each file not tracked.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use service::{Changes, Diff, Diffstat, FileChanged};
use store::WorkNumber;

use crate::git::{self, Place};

/// What every diff here is given: no rename found, so that each path is a file of its own, and no program the
/// repository's configuration names for a diff run.
const PLAIN: [&str; 3] = ["--no-renames", "--no-ext-diff", "--no-textconv"];

/// What a diff against the worktree is given so that it writes nothing. `--no-optional-locks` does not stop it: with
/// `diff.autoRefreshIndex`, which is on unless it is turned off, `git diff` rewrites the index with the stat
/// information of each file whose time or size changed and whose content did not. A diff is still taken of the
/// content of such a file, which is the same, and so it lists none.
const WRITES_NOTHING: [&str; 4] = ["--no-optional-locks", "-c", "diff.autoRefreshIndex=false", "diff"];

/// The most of one file's diff that is given, in bytes: a file made by a program, such as a lock file, can change by
/// megabytes, which no reader of the page reads whole.
pub(crate) const DIFF: usize = 1 << 20;

/// What was worked out so far, kept in memory for the process: what a work at each head changed, by the head; the
/// latest head of each work with no worktree, by the work; and the pull request each merge's commit names, by the
/// commit.
#[derive(Default)]
pub(crate) struct Known {
    between: Mutex<HashMap<String, Arc<Between>>>,
    heads: Mutex<HashMap<WorkNumber, Option<String>>>,
    pulls: Mutex<HashMap<String, Option<u64>>>,
}

/// What a work at a head changed: where its branch left main, and each file changed since, by its path.
struct Between {
    base: String,
    files: BTreeMap<String, Lines>,
}

/// The lines added to a file and removed from it; none of a binary file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Lines {
    Text { added: u64, removed: u64 },
    Binary,
}

/// What a work changed, with the paths of the files of it only its worktree holds, which git does not track.
pub(crate) struct Changed {
    pub(crate) changes: Changes,
    untracked: BTreeSet<String>,
}

impl Changed {
    /// How many files, and how many lines added and removed in all.
    pub(crate) fn stat(&self) -> Diffstat {
        let files = &self.changes.files;
        Diffstat {
            files: files.len() as u64,
            added: files.iter().map(|f| f.added).sum(),
            removed: files.iter().map(|f| f.removed).sum(),
        }
    }
}

/// Takes the lock of a map of what is known, whatever a panic left in it: each entry is whole once it is in.
fn known<T>(map: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    map.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Known {
    /// What the work at its place changed, where `main` names main's head in the repository; none where it has no
    /// head to tell it from, as before its branch is made.
    pub(crate) async fn changed(
        &self,
        repo: &Path,
        main: &str,
        work: WorkNumber,
        place: &Place,
    ) -> Result<Option<Changed>, String> {
        let worktree = place.dir.is_dir();
        let Some(head) = self.head(repo, work, place, worktree).await? else {
            return Ok(None);
        };
        let between = self.between(repo, main, &head).await?;
        let (files, untracked) = if worktree {
            in_worktree(&place.dir, &between.base).await?
        } else {
            (between.files.clone(), BTreeSet::new())
        };
        let files = (files.into_iter())
            .map(|(path, lines)| {
                let (added, removed) = match lines {
                    Lines::Text { added, removed } => (added, removed),
                    Lines::Binary => (0, 0),
                };
                FileChanged {
                    committed: between.files.contains_key(&path),
                    binary: lines == Lines::Binary,
                    added,
                    removed,
                    path,
                }
            })
            .collect();
        let changes = Changes {
            base: between.base.clone(),
            head,
            worktree,
            files,
        };
        Ok(Some(Changed { changes, untracked }))
    }

    /// The unified diff of the file of this path the work changed, between where [`Known::changed`] says; none where
    /// the work changed no file of the path.
    pub(crate) async fn diff(
        &self,
        repo: &Path,
        main: &str,
        (work, place): (WorkNumber, &Place),
        path: &str,
    ) -> Result<Option<Diff>, String> {
        let Some(changed) = self.changed(repo, main, work, place).await? else {
            return Ok(None);
        };
        let changes = &changed.changes;
        if !changes.files.iter().any(|f| f.path == path) {
            return Ok(None);
        }
        let text = if changed.untracked.contains(path) {
            let (lead, tail) = (["--no-optional-locks", "diff", "--no-index"], ["--", "/dev/null", path]);
            let args = [&lead[..], &PLAIN[..], &tail[..]].concat();
            let said = git::said(&place.dir, &args).await?;
            match said.code {
                // It exits with 1 where the two differ, as a file added does.
                Some(0 | 1) => said.out,
                _ => return Err(format!("git diff --no-index of {path} failed: {}", said.err)),
            }
        } else if changes.worktree {
            let (lead, tail) = (["--literal-pathspecs"], [changes.base.as_str(), "--", path]);
            let args = [&lead[..], &WRITES_NOTHING[..], &PLAIN[..], &tail[..]].concat();
            git::git(&place.dir, &args).await?
        } else {
            let lead = ["--literal-pathspecs", "diff"];
            let tail = [changes.base.as_str(), changes.head.as_str(), "--", path];
            let args = [&lead[..], &PLAIN[..], &tail[..]].concat();
            git::git(repo, &args).await?
        };
        Ok(Some(cut(path, text)))
    }

    /// The number of the pull request the title of the commit names at its end, as GitHub titles the commit a merge of
    /// one makes, `(#N)`; none where it names none. Worked out once for each commit.
    pub(crate) async fn pull_named(&self, repo: &Path, commit: &str) -> Result<Option<u64>, String> {
        if let Some(named) = known(&self.pulls).get(commit) {
            return Ok(*named);
        }
        let title = git::git(repo, &["log", "-1", "--format=%s", commit]).await?;
        let named = (title.trim_end().strip_suffix(')'))
            .and_then(|title| title.rsplit_once("(#"))
            .and_then(|(_, number)| number.parse().ok());
        known(&self.pulls).insert(commit.to_string(), named);
        Ok(named)
    }

    /// The work's latest head: its branch while its worktree is there; else the reference the engine keeps its head
    /// under, or its branch where nothing was kept under it; none where there is neither. Of a work with no worktree it
    /// is read once, until the worktree is made again.
    async fn head(
        &self,
        repo: &Path,
        work: WorkNumber,
        place: &Place,
        worktree: bool,
    ) -> Result<Option<String>, String> {
        if worktree {
            known(&self.heads).remove(&work);
            return commit_of(repo, &place.reference()).await;
        }
        if let Some(head) = known(&self.heads).get(&work) {
            return Ok(head.clone());
        }
        let head = match commit_of(repo, &crate::head::kept(place)).await? {
            Some(head) => Some(head),
            None => commit_of(repo, &place.reference()).await?,
        };
        known(&self.heads).insert(work, head.clone());
        Ok(head)
    }

    /// What a work at this head changed, worked out once for each head.
    async fn between(&self, repo: &Path, main: &str, head: &str) -> Result<Arc<Between>, String> {
        if let Some(between) = known(&self.between).get(head) {
            return Ok(between.clone());
        }
        let base = base(repo, main, head).await?;
        let (lead, tail) = (["diff", "--numstat", "-z"], [base.as_str(), head]);
        let args = [&lead[..], &PLAIN[..], &tail[..]].concat();
        let files = numstat(&git::git(repo, &args).await?);
        let between = Arc::new(Between { base, files });
        known(&self.between).insert(head.to_string(), between.clone());
        Ok(between)
    }
}

/// The commit the reference names; none where there is no such reference.
async fn commit_of(repo: &Path, reference: &str) -> Result<Option<String>, String> {
    let said = git::said(
        repo,
        &["rev-parse", "--verify", "--quiet", &format!("{reference}^{{commit}}")],
    )
    .await?;
    match said.code {
        Some(0) => Ok(Some(said.out)),
        Some(1) => Ok(None),
        _ => Err(format!("git rev-parse {reference} failed: {}", said.err)),
    }
}

/// Where the branch whose head is `tip` left main, `main` naming main's head: the latest commit of main the head holds.
/// Of a head main holds already, as one a merge commit merged, it is the latest commit of main the head held before that
/// merge, the first parent of the first commit of main's own line that holds the head; so a work's base is the same
/// before it merges and after, and a head on main's own line is its own base.
pub(crate) async fn base(repo: &Path, main: &str, tip: &str) -> Result<String, String> {
    let held = git::said(repo, &["merge-base", "--is-ancestor", tip, main]).await?;
    let before = match held.code {
        Some(1) => main.to_string(),
        Some(0) => {
            let line = format!("{tip}..{main}");
            let after = git::git(
                repo,
                &["rev-list", "--first-parent", "--ancestry-path", "--reverse", &line],
            )
            .await?;
            match after.lines().next() {
                Some(merge) => format!("{merge}^1"),
                // The head is main's head.
                None => return Ok(tip.to_string()),
            }
        }
        _ => {
            return Err(format!(
                "git merge-base --is-ancestor {tip} {main} failed: {}",
                held.err
            ));
        }
    };
    git::git(repo, &["merge-base", tip, &before]).await
}

/// The files of `git diff --numstat -z` with no renames, by their paths.
fn numstat(out: &str) -> BTreeMap<String, Lines> {
    (out.split('\0'))
        .filter(|record| !record.is_empty())
        .filter_map(|record| {
            let mut fields = record.splitn(3, '\t');
            let (added, removed, path) = (fields.next()?, fields.next()?, fields.next()?);
            let lines = match (added.parse(), removed.parse()) {
                (Ok(added), Ok(removed)) => Lines::Text { added, removed },
                // `-` for each, of a binary file.
                _ => Lines::Binary,
            };
            Some((path.to_string(), lines))
        })
        .collect()
}

/// What the worktree holds over `base`: each tracked file whose content there differs from it, commits since the work's
/// latest head included, and each file git does not track and does not ignore, all of whose lines are added; with the
/// paths of the latter.
async fn in_worktree(dir: &Path, base: &str) -> Result<(BTreeMap<String, Lines>, BTreeSet<String>), String> {
    let args = [&WRITES_NOTHING[..], &["--numstat", "-z"][..], &PLAIN[..], &[base][..]].concat();
    let mut files = numstat(&git::git(dir, &args).await?);
    let others = [
        "--no-optional-locks",
        "ls-files",
        "--others",
        "--exclude-standard",
        "-z",
    ];
    let listed = git::git(dir, &others).await?;
    let untracked: BTreeSet<String> = (listed.split('\0'))
        .filter(|path| !path.is_empty())
        .map(str::to_string)
        .collect();
    let (dir_owned, paths): (PathBuf, Vec<String>) = (dir.to_path_buf(), untracked.iter().cloned().collect());
    let counted = tokio::task::spawn_blocking(move || {
        (paths.into_iter())
            .map(|path| {
                let lines = lines_of(&dir_owned.join(&path)).map_err(|e| format!("{path}: {e}"))?;
                Ok((path, lines))
            })
            .collect::<Result<Vec<_>, String>>()
    })
    .await
    .map_err(|e| format!("the files git does not track could not be read: {e}"))??;
    files.extend(counted);
    Ok((files, untracked))
}

/// How much of a file is looked at for a zero byte, as git looks to tell a binary file.
const BINARY_AT: usize = 8000;

/// The lines of a file git does not track, all of them added, as git counts them: a symbolic link is one line, its
/// target; a file with a zero byte in its first [`BINARY_AT`] bytes is binary; the last line counts though it has no
/// newline. The file is read in pieces, so that a large one is never held whole.
fn lines_of(path: &Path) -> std::io::Result<Lines> {
    if std::fs::symlink_metadata(path)?.file_type().is_symlink() {
        return Ok(Lines::Text { added: 1, removed: 0 });
    }
    let mut file = std::fs::File::open(path)?;
    let (mut piece, mut seen, mut added, mut last) = (vec![0u8; 64 << 10], 0usize, 0u64, b'\n');
    loop {
        let read = file.read(&mut piece)?;
        if read == 0 {
            break;
        }
        let piece = &piece[..read];
        if seen < BINARY_AT && piece[..read.min(BINARY_AT - seen)].contains(&0) {
            return Ok(Lines::Binary);
        }
        seen += read;
        added += piece.iter().filter(|b| **b == b'\n').count() as u64;
        last = piece[read - 1];
    }
    if last != b'\n' {
        added += 1;
    }
    Ok(Lines::Text { added, removed: 0 })
}

/// The diff of the file of this path, cut to [`DIFF`] bytes at a character's start where it is longer, saying its whole
/// length.
fn cut(path: &str, text: String) -> Diff {
    if text.len() <= DIFF {
        return Diff {
            path: path.to_string(),
            text,
            cut: None,
        };
    }
    let mut end = DIFF;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    Diff {
        path: path.to_string(),
        cut: Some(text.len() as u64),
        text: text[..end].to_string(),
    }
}

/// The commit a merge's words say main's head is now, read back from what the merge wrote by the words it wrote it
/// with, `text`, where `{head}` stands for that commit: what stands between the words just before `{head}` and those just
/// after. None where the words cannot be read back so, or what stands there is no commit's name.
pub(crate) fn merged_commit(content: &str, text: &str) -> Option<String> {
    let (before, after) = text.split_once("{head}")?;
    let lead = before.rsplit_once('}').map_or(before, |(_, lead)| lead);
    let trail = after.split_once('{').map_or(after, |(trail, _)| trail);
    if lead.is_empty() {
        return None;
    }
    let rest = &content[content.rfind(lead)? + lead.len()..];
    let commit = if trail.is_empty() {
        rest
    } else {
        rest.split_once(trail)?.0
    };
    let named = commit.len() >= 7 && commit.chars().all(|c| c.is_ascii_hexdigit());
    named.then(|| commit.to_string())
}

#[cfg(test)]
mod tests;
