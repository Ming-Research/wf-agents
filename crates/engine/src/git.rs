//! What the engine does with git in a project's clone and a work's worktree. Everything here is local; pushing and
//! everything on GitHub go through [`crate::hub::GitHub`]; setting a worktree up and removing it are the session
//! manager's ([`sessions::worktree`]).

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

/// A work's worktree is the session manager's: it sets it up, removes it, and says what its head is and whether it is
/// clean.
pub use sessions::worktree::{create_worktree, git, head, is_clean, remove_worktree, resolve, take_over_worktree};

/// The file names git wrote with `-z`: separated by NUL, as written, with no quoting or escapes (without `-z` git
/// quotes a name that holds a tab, a quote, a backslash or a character outside ASCII).
fn names(s: &str) -> Vec<String> {
    s.split('\0').filter(|n| !n.is_empty()).map(str::to_string).collect()
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
pub fn main_merged_into(dir: &Path, rev: &str, main: &str) -> Result<String, String> {
    Ok(git(dir, &["merge-base", rev, &format!("origin/{main}")])?
        .trim()
        .to_string())
}

/// The files changed from one commit to another.
pub fn changed(dir: &Path, from: &str, to: &str) -> Result<Vec<String>, String> {
    Ok(names(&git(
        dir,
        &["diff", "--name-only", "-z", &format!("{from}..{to}")],
    )?))
}

/// The files among `paths` that differ between two commits, a file taken away included, and a file moved to another
/// name counted at the name it had: the paths are taken as they are written, with no pattern in them, and told as
/// they were written (git's `-z` output holds no quoting or escapes). No path names no file, where git itself reads
/// an empty list as every path; a commit the clone does not have is an error.
pub fn changed_among(dir: &Path, from: &str, to: &str, paths: &[String]) -> Result<Vec<String>, String> {
    let range = format!("{from}..{to}");
    if paths.is_empty() {
        // Still ask git about the range, so that a commit it lacks is told as an error.
        git(dir, &["rev-list", "--max-count=0", range.as_str()])?;
        return Ok(Vec::new());
    }
    let mut args = vec![
        "--literal-pathspecs",
        "diff",
        "--name-only",
        "-z",
        "--no-renames",
        range.as_str(),
        "--",
    ];
    args.extend(paths.iter().map(String::as_str));
    Ok(names(&git(dir, &args)?))
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

/// The lines the work itself adds and removes at `rev`, against main as merged into `rev`, as the text of a diff with
/// nothing in it that main's changes elsewhere move: no blob ids, no line numbers and no function names, but one line
/// of context around each change, which says where it is. What main does in another place of a file the work changed,
/// or in a file the work did not, does not show in it. Equal text means the same lines written or removed with the
/// same neighbouring line on each side; main changing those neighbours makes the text differ. A change moved between
/// two places whose neighbours are equal (a line added after the first of four equal lines, then after the third) is
/// not told apart, which can skip a retest; the full CI that follows catching up with main is the gate for it. The
/// text is bytes, since a line that is not UTF-8 would otherwise be read as another that is not.
pub fn own_lines(dir: &Path, main: &str, rev: &str) -> Result<Vec<u8>, String> {
    let fork = main_merged_into(dir, rev, main)?;
    let range = format!("{fork}..{rev}");
    // The options are fixed, so that no setting of the user's git changes the text. `--binary` carries the change of a
    // binary file, which its blob ids alone would otherwise stand for; `--no-renames` makes a renamed file its removal
    // and its addition, so that a rename is a change.
    let args = [
        "diff",
        "--no-color",
        "--no-ext-diff",
        "--no-textconv",
        "--no-renames",
        "--binary",
        // One line of context on each side says where in the file a line is written or removed, once the hunk's
        // numbers are gone: the same line in another place has other neighbours.
        "-U1",
        range.as_str(),
    ];
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .map_err(|e| format!("git: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let mut text = Vec::with_capacity(out.stdout.len());
    // Every line of git's output ends with a newline, so a diff with no change is an empty text.
    for line in out.stdout.split_inclusive(|b| *b == b'\n') {
        // The blob ids on both sides change whenever main changes the file.
        if line.starts_with(b"index ") {
            continue;
        }
        // A hunk's line numbers change with what main adds above, and so does the function name git prints after
        // them (the nearest line above that begins with a letter); the context lines say where the hunk is.
        text.extend_from_slice(if line.starts_with(b"@@") { &b"@@\n"[..] } else { line });
    }
    Ok(text)
}

/// The files changed in the worktree against a commit, committed or not, untracked files included.
pub fn changed_since(dir: &Path, from: &str) -> Result<Vec<String>, String> {
    let mut out = names(&git(dir, &["diff", "--name-only", "-z", from])?);
    out.extend(names(&git(dir, &["ls-files", "-z", "--others", "--exclude-standard"])?));
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
            let files = names(&git(dir, &["diff", "--name-only", "-z", "--diff-filter=U"]).unwrap_or_default());
            let _ = git(dir, &["merge", "--abort"]);
            Ok(Err(files))
        }
    }
}

/// A commit on `base` that has the given files as they are at `from`: the tests of a change put on the code before
/// it, its message naming `tag`, so that two made at once from the same files are two commits and a run found by its
/// commit is the run made for it. Nothing changes in the worktree.
pub fn files_onto(dir: &Path, files: &[String], from: &str, base: &str, tag: &str) -> Result<String, String> {
    // The index is the worktree's git directory's, named by `tag`: never a file in the worktree, which a push from it
    // at the same time would find uncommitted, and one for each run, so that two runs at once do not share it.
    let gitdir = git(dir, &["rev-parse", "--absolute-git-dir"])?;
    let tmp = Path::new(gitdir.trim()).join(format!("wf-agents-index-{tag}"));
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
    let result = commit_with(&run, files, from, base, tag);
    let _ = std::fs::remove_file(&tmp);
    result
}

/// A commit on top of `from` that has the same files, with the message given: a commit of its own, which no workflow
/// has run on, so that a run found by its commit is the run made for it. Nothing changes in the worktree.
pub fn commit_over(dir: &Path, from: &str, message: &str) -> Result<String, String> {
    let tree = git(dir, &["rev-parse", &format!("{from}^{{tree}}")])?;
    let who = ["-c", "user.name=wf-agents", "-c", "user.email=wf-agents@localhost"];
    let made = git(
        dir,
        &[&who[..], &["commit-tree", tree.trim(), "-p", from, "-m", message]].concat(),
    )?;
    Ok(made.trim().to_string())
}

fn commit_with(
    run: &impl Fn(&[&str]) -> Result<String, String>,
    files: &[String],
    from: &str,
    base: &str,
    tag: &str,
) -> Result<String, String> {
    run(&["read-tree", base])?;
    for f in files {
        let blob = run(&["rev-parse", &format!("{from}:{f}")])?;
        run(&["update-index", "--add", "--cacheinfo", &format!("100644,{blob},{f}")])?;
    }
    let tree = run(&["write-tree"])?;
    let who = ["-c", "user.name=wf-agents", "-c", "user.email=wf-agents@localhost"];
    let msg = format!("Tests of the change on the code before it (wf-agents {tag})");
    run(&[&who[..], &["commit-tree", tree.as_str(), "-p", base, "-m", &msg]].concat())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

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

    // The commit made for a measurement has the head's files and the head as its parent, and is another commit each time
    // its message is another, so that a run found by its commit is not an earlier one's.
    #[test]
    fn a_commit_over_a_head_has_its_files_and_is_its_own() {
        let dir = std::env::temp_dir().join(format!("wf-agents-commit-over-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        run(&dir, &["init", "-q"]);
        let head = commit(&dir, &[("src/x.txt", "x\n")], "Start");
        let first = commit_over(&dir, &head, "Measurement 1").unwrap();
        let again = commit_over(&dir, &head, "Measurement 2").unwrap();
        assert_ne!(first, again);
        assert_ne!(first, head);
        assert_eq!(run(&dir, &["rev-parse", &format!("{first}^")]), head);
        assert_eq!(
            run(&dir, &["rev-parse", &format!("{first}^{{tree}}")]),
            run(&dir, &["rev-parse", &format!("{head}^{{tree}}")])
        );
        assert_eq!(run(&dir, &["log", "-1", "--format=%s", &first]), "Measurement 1");
        assert!(commit_over(&dir, "no-such-commit", "x").is_err());
        let _ = std::fs::remove_dir_all(&dir);
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

    // Item #217, R241-41: the files a run of ci_before put on the code before the change are compared with the head by
    // name: one edited, one taken away and one moved to another name count at the name they had, one left as it was
    // does not, a name with a pattern in it is a name, and a file the run did not use counts for nothing.
    #[test]
    fn the_files_among_some_paths_that_changed_are_named_as_they_were_written() {
        let dir = std::env::temp_dir().join(format!("wf-agents-changed-among-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        run(&dir, &["init", "-q"]);
        let start = commit(
            &dir,
            &[
                ("t/kept.rs", "kept\n"),
                ("t/edited.rs", "one\n"),
                ("t/removed.rs", "gone\n"),
                ("t/moved.rs", "moved\nmoved\nmoved\n"),
                ("t/star*.rs", "star\n"),
                ("t/starry.rs", "starry\n"),
                ("src/other.rs", "other\n"),
            ],
            "Start",
        );
        std::fs::remove_file(dir.join("t/removed.rs")).unwrap();
        std::fs::rename(dir.join("t/moved.rs"), dir.join("t/renamed.rs")).unwrap();
        commit(
            &dir,
            &[
                ("t/edited.rs", "two\n"),
                ("t/starry.rs", "starry, edited\n"),
                ("src/other.rs", "other, edited\n"),
            ],
            "Change",
        );
        let ps = |xs: &[&str]| xs.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let among = |paths: &[&str]| changed_among(&dir, &start, "HEAD", &ps(paths)).unwrap();
        assert_eq!(
            among(&["t/kept.rs", "t/edited.rs", "t/removed.rs", "t/moved.rs"]),
            ["t/edited.rs", "t/moved.rs", "t/removed.rs"]
        );
        // The pattern in a name is no pattern: `t/star*.rs` is a file that did not change, and `t/starry.rs` is not it.
        assert!(among(&["t/star*.rs"]).is_empty());
        // Nothing changed in the files left alone, and the other file of the commit is not asked about.
        assert!(among(&["t/kept.rs"]).is_empty());
        assert!(
            changed_among(&dir, "HEAD", "HEAD", &ps(&["t/edited.rs"]))
                .unwrap()
                .is_empty()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A repository with `files` committed on main, and `origin/main` standing at main, on branch `work` forked from it.
    fn forked(name: &str, files: &[(&str, &str)]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("wf-agents-own-lines-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        run(&dir, &["init", "-q"]);
        commit(&dir, files, "Start");
        run(&dir, &["update-ref", "refs/remotes/origin/main", "main"]);
        run(&dir, &["checkout", "-q", "-b", "work"]);
        dir
    }

    /// Main moves on by a commit that writes `files`, and the fetched main with it; the work stays on its branch.
    fn main_moves_on(dir: &Path, files: &[(&str, &str)]) {
        run(dir, &["checkout", "-q", "main"]);
        commit(dir, files, "Main's change");
        run(dir, &["update-ref", "refs/remotes/origin/main", "main"]);
        run(dir, &["checkout", "-q", "work"]);
    }

    /// Merges the fetched main where it conflicts, leaving the merge to be finished by the commit that resolves it.
    fn merge_in_conflict(dir: &Path) {
        let who = ["-c", "user.name=t", "-c", "user.email=t@t"];
        assert!(git(dir, &[&who[..], &["merge", "-q", "--no-edit", "origin/main"]].concat()).is_err());
    }

    // What main does elsewhere in a file the work changed, or in files the work did not, is not the work's own change:
    // the work's own lines are the same before and after a clean merge of main, though the diff from the fork point is
    // not, since its blob ids and line numbers move with main.
    #[test]
    fn a_clean_merge_of_main_leaves_the_works_own_lines_as_they_were() {
        let text: String = (1..=30).map(|n| format!("line {n}\n")).collect();
        let worked = text.replace("line 20\n", "line 20, the work's\n");
        let moved = format!("new 1\nnew 2\nnew 3\n{text}");
        let dir = forked("clean", &[("a.txt", text.as_str()), ("b.txt", "b\n")]);
        let start = run(&dir, &["rev-parse", "HEAD"]);
        let before = commit(&dir, &[("a.txt", worked.as_str())], "The work");
        // Main adds lines above the work's, in the same file, and changes a file the work did not.
        main_moves_on(&dir, &[("a.txt", moved.as_str()), ("b.txt", "main's b\n")]);
        let main = run(&dir, &["rev-parse", "main"]);
        run(&dir, &["merge", "-q", "--no-edit", "origin/main"]);

        let lines_before = own_lines(&dir, "main", &before).unwrap();
        let lines_after = own_lines(&dir, "main", "HEAD").unwrap();
        let shown = String::from_utf8(lines_before.clone()).unwrap();
        assert!(
            shown.contains("+line 20, the work's") && !shown.contains("b.txt"),
            "{shown}"
        );
        assert_eq!(lines_before, lines_after);
        // The diff from the fork point, which the engine hashed before, is not the same.
        assert_ne!(
            git(&dir, &["diff", &format!("{start}..{before}")]).unwrap(),
            git(&dir, &["diff", &format!("{main}..HEAD")]).unwrap()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    // Whatever changes a line the work adds or removes, a file it adds or removes, or a binary file it changes is a
    // change of its own lines, however small and by whatever means: a conflict resolved by writing another line, a
    // line's whitespace, a file's name, a binary file's bytes, and a file that main deleted, with the work's change.
    #[test]
    fn a_change_of_what_the_work_itself_adds_or_removes_changes_its_own_lines() {
        let differ = |case: &str, dir: &Path, before: &str| {
            let was = own_lines(dir, "main", before).unwrap();
            let now = own_lines(dir, "main", "HEAD").unwrap();
            assert!(!was.is_empty(), "{case}: the work has no lines of its own at the start");
            assert_ne!(was, now, "{case}");
            let _ = std::fs::remove_dir_all(dir);
        };

        // A conflict resolved by writing another line.
        let dir = forked("conflict", &[("a.txt", "1\n2\n3\n")]);
        let before = commit(&dir, &[("a.txt", "1\nthe work's\n3\n")], "The work");
        main_moves_on(&dir, &[("a.txt", "1\nmain's\n3\n")]);
        merge_in_conflict(&dir);
        commit(&dir, &[("a.txt", "1\nresolved\n3\n")], "Merge main");
        differ("conflict resolved", &dir, &before);

        // A change of whitespace only.
        let dir = forked("blank", &[("a.txt", "1\n2\n")]);
        let before = commit(&dir, &[("a.txt", "1\nx = 1\n2\n")], "The work");
        commit(&dir, &[("a.txt", "1\nx  = 1\n2\n")], "Whitespace");
        differ("whitespace", &dir, &before);

        // A file the work added, renamed with its content as it was.
        let dir = forked("added", &[("a.txt", "a\n")]);
        let before = commit(&dir, &[("added.txt", "content\n")], "The work");
        run(&dir, &["mv", "added.txt", "renamed.txt"]);
        commit(&dir, &[], "Rename");
        differ("added file renamed", &dir, &before);

        // A file of main's, renamed by the work.
        let dir = forked("moved", &[("a.txt", "a\n"), ("old.txt", "old content\n")]);
        run(&dir, &["mv", "old.txt", "moved.txt"]);
        let before = commit(&dir, &[], "The work moves it");
        run(&dir, &["mv", "moved.txt", "moved-again.txt"]);
        commit(&dir, &[], "Rename");
        differ("renamed file renamed", &dir, &before);

        // A binary file whose bytes changed; git shows only blob ids for it without `--binary`.
        let dir = forked("binary", &[("a.txt", "a\n")]);
        let bytes = |tail: u8| std::fs::write(dir.join("img.bin"), [0u8, 1, 2, tail]).unwrap();
        run(&dir, &["checkout", "-q", "main"]);
        bytes(9);
        run(&dir, &["add", "-A"]);
        run(&dir, &["commit", "-q", "-m", "A binary file"]);
        run(&dir, &["update-ref", "refs/remotes/origin/main", "main"]);
        run(&dir, &["checkout", "-q", "work"]);
        run(&dir, &["merge", "-q", "--ff-only", "main"]);
        bytes(3);
        run(&dir, &["add", "-A"]);
        run(&dir, &["commit", "-q", "-m", "The work"]);
        let before = run(&dir, &["rev-parse", "HEAD"]);
        bytes(4);
        run(&dir, &["add", "-A"]);
        run(&dir, &["commit", "-q", "-m", "Other bytes"]);
        differ("binary file", &dir, &before);

        // A file the work changed, which main deleted: the work's change goes with it.
        let dir = forked("deleted", &[("a.txt", "a\n"), ("b.txt", "b\n")]);
        let before = commit(&dir, &[("b.txt", "b, the work's\n")], "The work");
        run(&dir, &["checkout", "-q", "main"]);
        run(&dir, &["rm", "-q", "b.txt"]);
        run(&dir, &["commit", "-q", "-m", "Main deletes b"]);
        run(&dir, &["update-ref", "refs/remotes/origin/main", "main"]);
        run(&dir, &["checkout", "-q", "work"]);
        merge_in_conflict(&dir);
        run(&dir, &["rm", "-q", "b.txt"]);
        run(&dir, &["commit", "-q", "-m", "Merge main"]);
        differ("file deleted by main", &dir, &before);
    }

    // A commit that cannot be found has no own lines to give: the caller is told, not given an empty text that equals
    // that of a work with no change.
    #[test]
    fn the_own_lines_of_a_commit_that_is_not_there_are_an_error() {
        let dir = forked("missing", &[("a.txt", "a\n")]);
        commit(&dir, &[("a.txt", "a\nb\n")], "The work");
        assert!(own_lines(&dir, "main", "0123456789abcdef0123456789abcdef01234567").is_err());
        assert!(own_lines(&dir, "elsewhere", "HEAD").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
