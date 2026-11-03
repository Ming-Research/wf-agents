//! What the engine does with git in a project's clone and a work's worktree. Everything here is local; pushing and
//! everything on GitHub go through [`crate::hub::GitHub`]; setting a worktree up and removing it are the session
//! manager's ([`sessions::worktree`]).

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

/// A work's worktree is the session manager's: it sets it up, removes it, and says what its head is and whether it is
/// clean.
pub use sessions::worktree::{create_worktree, git, head, is_clean, remove_worktree, resolve, take_over_worktree};

/// Git's setting that prints a path beyond ASCII as it is, not as the quoted, escaped form (`"caf\303\251.txt"`) that
/// no agent could open or compare with a path. Every listing of file names goes through it.
const NAMES_AS_THEY_ARE: &str = "core.quotepath=false";

fn lines(s: &str) -> Vec<String> {
    s.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect()
}

/// Whether `a` is `b` or an ancestor of it.
pub fn is_ancestor(dir: &Path, a: &str, b: &str) -> bool {
    git(dir, &["merge-base", "--is-ancestor", a, b]).is_ok()
}

/// Where the work's branch forked from main: the merge base of HEAD and the fetched main.
pub fn fork_point(dir: &Path, main: &str) -> Result<String, String> {
    main_merged_into(dir, "HEAD", main)
}

/// Main as merged into a commit: the merge base of the commit and the fetched main, the latest commit of main it
/// contains.
fn main_merged_into(dir: &Path, rev: &str, main: &str) -> Result<String, String> {
    Ok(git(dir, &["merge-base", rev, &format!("origin/{main}")])?
        .trim()
        .to_string())
}

/// The files changed from one commit to another.
pub fn changed(dir: &Path, from: &str, to: &str) -> Result<Vec<String>, String> {
    Ok(lines(&git(
        dir,
        &["-c", NAMES_AS_THEY_ARE, "diff", "--name-only", &format!("{from}..{to}")],
    )?))
}

/// The files the work itself changed from one commit to another: those changed between them that `to` also changes
/// against main as merged into it. A file that only main changed, brought in by merging main, drops out; a file both
/// main and the work changed stays.
pub fn own_changes(dir: &Path, main: &str, from: &str, to: &str) -> Result<Vec<String>, String> {
    let all = changed(dir, from, to)?;
    if all.is_empty() {
        return Ok(all);
    }
    let merged = main_merged_into(dir, to, main)?;
    let own: BTreeSet<String> = changed(dir, &merged, to)?.into_iter().collect();
    Ok(all.into_iter().filter(|f| own.contains(f)).collect())
}

/// One commit of a branch, as the log of its first parents gives it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commit {
    pub id: String,
    /// The first line of its message.
    pub subject: String,
    /// Whether it has more than one parent: a merge, such as the work's branch taking main in.
    pub merge: bool,
}

/// What happened on a branch from `base` to `head`, read from git alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Changes {
    /// The commits of the branch after `base`, following first parents only, newest first; a merge shows as one
    /// commit, not as the commits it brought in.
    pub commits: Vec<Commit>,
    /// The merges among them, newest first, so that the reader can set aside what main brought.
    pub merges: Vec<Commit>,
    /// The files the work itself changed from `base` to `head`; what merging main brought is left out.
    pub own_files: Vec<String>,
    /// Whether `base` is not an ancestor of `head` (the branch was rewritten, or `base` is main, which went on): the
    /// commits are those of `head` that `base` lacks, and the files are counted from the commit they share.
    pub diverged: bool,
}

/// The commits of a branch from `base` to `head`, without the files: what an input needs that lists commits only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Log {
    /// As [`Changes::commits`].
    pub commits: Vec<Commit>,
    /// As [`Changes::merges`].
    pub merges: Vec<Commit>,
    /// As [`Changes::diverged`].
    pub diverged: bool,
}

/// The merges of main into the branch from `base` to `head`, newest first, and whether `base` is no ancestor of
/// `head`: only the merges that took in a commit of the fetched main, which is what a reader setting aside what main
/// brought needs, not a merge of anything else. Only the log is read, not the files.
pub fn main_merges_since(dir: &Path, main: &str, base: &str, head: &str) -> Result<(Vec<Commit>, bool), String> {
    let log = log_since(dir, base, head)?;
    let tip = format!("origin/{main}");
    let mut took = Vec::new();
    for merge in log.merges {
        // A merge's first parent is the branch; main is in what the others bring.
        let parents = git(dir, &["rev-list", "--parents", "-n", "1", &merge.id])?;
        if parents.split_whitespace().skip(2).any(|p| is_ancestor(dir, p, &tip)) {
            took.push(merge);
        }
    }
    Ok((took, log.diverged))
}

/// The commits from `base` to `head`, following first parents, newest first, with the merges among them marked.
pub fn log_since(dir: &Path, base: &str, head: &str) -> Result<Log, String> {
    let log = git(
        dir,
        &[
            "log",
            "--first-parent",
            "--format=%H%x09%P%x09%s",
            &format!("{base}..{head}"),
        ],
    )?;
    let commits: Vec<Commit> = log
        .lines()
        .filter_map(|l| {
            let mut parts = l.splitn(3, '\t');
            let id = parts.next().filter(|id| !id.is_empty())?;
            let parents = parts.next()?;
            Some(Commit {
                id: id.to_string(),
                subject: parts.next().unwrap_or_default().trim().to_string(),
                merge: parents.split_whitespace().count() > 1,
            })
        })
        .collect();
    let merges = commits.iter().filter(|c| c.merge).cloned().collect();
    Ok(Log {
        commits,
        merges,
        diverged: !is_ancestor(dir, base, head),
    })
}

/// What happened on the branch from `base` to `head`: the single definition of "since" that the cells' inputs share.
/// `main` is the name of the project's main branch, which the work's own files are told from main's by.
pub fn changes_since(dir: &Path, main: &str, base: &str, head: &str) -> Result<Changes, String> {
    let Log {
        commits,
        merges,
        diverged,
    } = log_since(dir, base, head)?;
    let from = if diverged {
        git(dir, &["merge-base", base, head])?.trim().to_string()
    } else {
        base.to_string()
    };
    Ok(Changes {
        commits,
        merges,
        own_files: own_changes(dir, main, &from, head)?,
        diverged,
    })
}

/// The files changed in the worktree against a commit, committed or not, untracked files included.
pub fn changed_since(dir: &Path, from: &str) -> Result<Vec<String>, String> {
    let mut out = lines(&git(dir, &["-c", NAMES_AS_THEY_ARE, "diff", "--name-only", from])?);
    out.extend(lines(&git(
        dir,
        &["-c", NAMES_AS_THEY_ARE, "ls-files", "--others", "--exclude-standard"],
    )?));
    out.sort();
    out.dedup();
    Ok(out)
}

/// The diff from one commit to another, of some paths or of all.
pub fn diff(dir: &Path, from: &str, to: &str, paths: &[String]) -> Result<String, String> {
    let range = format!("{from}..{to}");
    let mut args = vec!["diff", range.as_str()];
    if !paths.is_empty() {
        args.push("--");
        args.extend(paths.iter().map(String::as_str));
    }
    git(dir, &args)
}

/// Whether a path is under one of the given paths: a directory given with its trailing slash, a file by its name,
/// and `*` matching one segment.
pub fn under(path: &str, paths: &[String]) -> bool {
    let got: Vec<&str> = path.split('/').collect();
    paths.iter().any(|p| {
        let dir = p.ends_with('/');
        let pat: Vec<&str> = p.trim_end_matches('/').split('/').collect();
        let long_enough = if dir {
            got.len() > pat.len()
        } else {
            got.len() >= pat.len()
        };
        long_enough && pat.iter().zip(&got).all(|(a, b)| *a == "*" || a == b)
    })
}

/// Merges the fetched main into the worktree's branch. On a conflict the merge is undone and the files in conflict
/// are given.
pub fn catch_up(dir: &Path, main: &str) -> Result<Result<String, Vec<String>>, String> {
    git(dir, &["fetch", "-q", "origin", &config::main_refspec(main)])?;
    let theirs = format!("origin/{main}");
    if is_ancestor(dir, &theirs, "HEAD") {
        return Ok(Ok(head(dir)?));
    }
    match git(
        dir,
        &[
            "-c",
            "user.name=wf-agents",
            "-c",
            "user.email=wf-agents@localhost",
            "merge",
            "--no-edit",
            "-q",
            &theirs,
        ],
    ) {
        Ok(_) => Ok(Ok(head(dir)?)),
        Err(_) => {
            let files = lines(
                &git(
                    dir,
                    &["-c", NAMES_AS_THEY_ARE, "diff", "--name-only", "--diff-filter=U"],
                )
                .unwrap_or_default(),
            );
            let _ = git(dir, &["merge", "--abort"]);
            Ok(Err(files))
        }
    }
}

/// A commit on `base` that has the given files as they are at `from`: the tests of a change put on the code before
/// it. Nothing changes in the worktree.
pub fn files_onto(dir: &Path, files: &[String], from: &str, base: &str) -> Result<String, String> {
    let tmp = dir.join(".git-wf-agents-index");
    let index = tmp.to_string_lossy().into_owned();
    let run = |args: &[&str]| -> Result<String, String> {
        let out = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .env("GIT_INDEX_FILE", &index)
            .output()
            .map_err(|e| format!("git: {e}"))?;
        if !out.status.success() {
            return Err(format!(
                "git {}: {}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    };
    let result = commit_with(&run, files, from, base);
    let _ = std::fs::remove_file(&tmp);
    result
}

fn commit_with(
    run: &impl Fn(&[&str]) -> Result<String, String>,
    files: &[String],
    from: &str,
    base: &str,
) -> Result<String, String> {
    run(&["read-tree", base])?;
    for f in files {
        let blob = run(&["rev-parse", &format!("{from}:{f}")])?;
        run(&["update-index", "--add", "--cacheinfo", &format!("100644,{blob},{f}")])?;
    }
    let tree = run(&["write-tree"])?;
    let who = ["-c", "user.name=wf-agents", "-c", "user.email=wf-agents@localhost"];
    let msg = "Tests of the change on the code before it";
    run(&[&who[..], &["commit-tree", tree.as_str(), "-p", base, "-m", msg]].concat())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_are_matched_by_directory_name_and_segment() {
        let ps = |xs: &[&str]| xs.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(under("design/a.md", &ps(&["design/"])));
        assert!(!under("designs/a.md", &ps(&["design/"])));
        assert!(under("flow/flow.json", &ps(&["flow/flow.json"])));
        assert!(under("crates/db/tests/x.rs", &ps(&["crates/*/tests/"])));
        assert!(!under("crates/db/src/x.rs", &ps(&["crates/*/tests/"])));
        assert!(!under("crates/tests", &ps(&["crates/*/tests/"])));
    }

    fn run(dir: &Path, args: &[&str]) -> String {
        let who = [
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "init.defaultBranch=main",
        ];
        git(dir, &[&who[..], args].concat()).unwrap().trim().to_string()
    }

    fn commit(dir: &Path, files: &[(&str, &str)], message: &str) -> String {
        for (f, text) in files {
            let p = dir.join(f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
        }
        run(dir, &["add", "-A"]);
        run(dir, &["commit", "-q", "-m", message]);
        run(dir, &["rev-parse", "HEAD"])
    }

    // What a merge of main brings in is not the work's own change, unless the work changed that file too.
    #[test]
    fn a_merge_of_main_brings_in_no_change_of_the_works_own() {
        let dir = std::env::temp_dir().join(format!("wf-agents-own-changes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        run(&dir, &["init", "-q"]);
        let start = commit(&dir, &[("design/a.md", "# A\nOld.\n"), ("src/x.txt", "old\n")], "Start");
        run(&dir, &["checkout", "-q", "-b", "work"]);
        let own = commit(
            &dir,
            &[("design/a.md", "# A\nOld.\nThe work's.\n"), ("src/x.txt", "new\n")],
            "The work's change",
        );
        run(&dir, &["checkout", "-q", "main"]);
        commit(
            &dir,
            &[("design/a.md", "# The A\nOld.\n"), ("design/b.md", "Main's.\n")],
            "Main's change",
        );
        run(&dir, &["update-ref", "refs/remotes/origin/main", "main"]);
        run(&dir, &["checkout", "-q", "work"]);
        run(&dir, &["merge", "-q", "--no-edit", "origin/main"]);
        // Since the work's own commit, the merge changed both files; main's new file drops out, and the file both
        // changed stays.
        assert_eq!(changed(&dir, &own, "HEAD").unwrap(), vec!["design/a.md", "design/b.md"]);
        assert_eq!(own_changes(&dir, "main", &own, "HEAD").unwrap(), vec!["design/a.md"]);
        // Across the whole branch, the work's own files.
        assert_eq!(
            own_changes(&dir, "main", &start, "HEAD").unwrap(),
            vec!["design/a.md", "src/x.txt"]
        );
        assert!(own_changes(&dir, "main", "HEAD", "HEAD").unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A new repository whose first branch is main, in a directory of its own to the test.
    fn fresh(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("wf-agents-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        run(&dir, &["init", "-q"]);
        dir
    }

    // What happened on a branch since a commit is told from git alone: the commits by first parent with the merges
    // marked, those merges apart, and the work's own files without what a merge of main brought. Where main is the
    // base, the merges are the branch's taking main in, and none when it never did.
    #[test]
    fn what_happened_since_a_commit_marks_the_merges_and_leaves_out_what_main_brought() {
        let dir = fresh("changes-since");
        let start = commit(&dir, &[("a.txt", "a\n")], "Start");
        run(&dir, &["checkout", "-q", "-b", "plain"]);
        let plain = commit(&dir, &[("plain.txt", "p\n")], "A branch that never took main in");
        run(&dir, &["checkout", "-q", "-b", "work", &start]);
        let w1 = commit(&dir, &[("own1.txt", "1\n")], "First");
        run(&dir, &["checkout", "-q", "main"]);
        commit(&dir, &[("main1.txt", "m\n")], "Main's change");
        run(&dir, &["update-ref", "refs/remotes/origin/main", "main"]);
        run(&dir, &["checkout", "-q", "work"]);
        let w2 = commit(&dir, &[("own2.txt", "2\n")], "Second");
        run(&dir, &["merge", "-q", "--no-edit", "origin/main"]);
        let merge = run(&dir, &["rev-parse", "HEAD"]);
        let w3 = commit(&dir, &[("own3.txt", "3\n")], "Third");

        // Since the first commit: the second, the merge and the third, newest first, the merge marked; main's file is
        // not the work's own.
        let since = changes_since(&dir, "main", &w1, "HEAD").unwrap();
        let ids: Vec<(&str, bool)> = since.commits.iter().map(|c| (c.id.as_str(), c.merge)).collect();
        assert_eq!(
            ids,
            [(w3.as_str(), false), (merge.as_str(), true), (w2.as_str(), false)]
        );
        assert_eq!(since.commits[0].subject, "Third");
        assert_eq!(since.merges.len(), 1);
        assert_eq!(since.merges[0].id, merge);
        assert_eq!(since.own_files, ["own2.txt", "own3.txt"]);
        assert!(!since.diverged);

        // Against main, the merge is the branch taking main in, and every commit of the branch is there.
        let from_main = changes_since(&dir, "main", "origin/main", "HEAD").unwrap();
        assert_eq!(from_main.merges.len(), 1);
        assert_eq!(from_main.merges[0].id, merge);
        assert_eq!(from_main.commits.len(), 4);
        assert_eq!(from_main.own_files, ["own1.txt", "own2.txt", "own3.txt"]);

        // A branch that never took main in has no merge, though main went on past where it was cut.
        let never = changes_since(&dir, "main", "origin/main", &plain).unwrap();
        assert!(never.merges.is_empty(), "{never:?}");
        assert_eq!(never.own_files, ["plain.txt"]);

        // Nothing since the head itself.
        let none = changes_since(&dir, "main", "HEAD", "HEAD").unwrap();
        assert!(none.commits.is_empty() && none.merges.is_empty() && none.own_files.is_empty() && !none.diverged);
        let _ = std::fs::remove_dir_all(&dir);
    }

    // A base that is no ancestor of the head, as after the branch was rebuilt on main, still gives what the head has
    // that the base lacks, with the files counted from the commit they share, and says so; a head the base is ahead
    // of gives nothing new.
    #[test]
    fn a_base_that_is_not_an_ancestor_of_the_head_is_said_and_counted_from_the_shared_commit() {
        let dir = fresh("changes-diverged");
        commit(&dir, &[("a.txt", "a\n")], "Start");
        run(&dir, &["checkout", "-q", "-b", "work"]);
        let w1 = commit(&dir, &[("own1.txt", "1\n")], "First");
        let w2 = commit(&dir, &[("own2.txt", "2\n")], "Second");
        run(&dir, &["checkout", "-q", "main"]);
        run(&dir, &["update-ref", "refs/remotes/origin/main", "main"]);
        // The branch rebuilt: one commit on main in place of the two, which has another id.
        run(&dir, &["checkout", "-q", "-b", "rebuilt"]);
        let r1 = commit(&dir, &[("own1.txt", "1\n"), ("own2.txt", "2\n")], "Both");
        let rebuilt = changes_since(&dir, "main", &w2, &r1).unwrap();
        assert!(rebuilt.diverged);
        assert_eq!(rebuilt.commits.len(), 1);
        assert_eq!(rebuilt.commits[0].id, r1);
        assert!(rebuilt.merges.is_empty());
        assert_eq!(rebuilt.own_files, ["own1.txt", "own2.txt"]);
        // The base ahead of the head: nothing of the head's is new.
        let behind = changes_since(&dir, "main", &w2, &w1).unwrap();
        assert!(
            behind.diverged && behind.commits.is_empty() && behind.own_files.is_empty(),
            "{behind:?}"
        );
        // A base that is no commit is an error, not an empty answer.
        assert!(changes_since(&dir, "main", "no-such-commit", &r1).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
