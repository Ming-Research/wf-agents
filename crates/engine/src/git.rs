//! What the engine does with git in a project's clone and a work's worktree. Everything here is local; pushing and
//! everything on GitHub go through [`crate::hub::GitHub`]; setting a worktree up and removing it are the session
//! manager's ([`sessions::worktree`]).

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

/// A work's worktree is the session manager's: it sets it up, removes it, and says what its head is and whether it is
/// clean.
pub use sessions::worktree::{create_worktree, git, head, is_clean, remove_worktree, resolve, take_over_worktree};

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
    Ok(lines(&git(dir, &["diff", "--name-only", &format!("{from}..{to}")])?))
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

/// The lines the work itself adds and removes at `rev`, against main as merged into `rev`, as the text of a diff with
/// nothing in it that main's changes elsewhere move: no blob ids and no line numbers, but the function name and one line
/// of context around each change, which say where it is. What main does in another place of a file the work changed,
/// or in a file the work did not, does not show in it; two commits have the same text exactly when the work's own
/// lines are the same in both, in the same place (main changing the neighbours of the work's lines also counts). The text is bytes, since a line that is not UTF-8 would
/// otherwise be read as another that is not.
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
    for line in out.stdout.split(|b| *b == b'\n') {
        // The blob ids on both sides change whenever main changes the file.
        if line.starts_with(b"index ") {
            continue;
        }
        // A hunk's line numbers change with what main adds above; the function name after them stays, since it names
        // the place the hunk is in, and it changes only if main changes that place (then the checks run again).
        match hunk_function(line) {
            Some(function) => {
                text.extend_from_slice(b"@@");
                text.extend_from_slice(function);
            }
            None => text.extend_from_slice(line),
        }
        text.push(b'\n');
    }
    Ok(text)
}

/// What follows the line numbers of a hunk header (`@@ -1,2 +3,4 @@ fn name`): the space and the function name, or
/// nothing. None for a line that is not a hunk header.
fn hunk_function(line: &[u8]) -> Option<&[u8]> {
    let numbers = line.strip_prefix(b"@@ -")?;
    let end = numbers.windows(3).position(|w| w == b" @@")?;
    Some(&numbers[end + 3..])
}

/// The files changed in the worktree against a commit, committed or not, untracked files included.
pub fn changed_since(dir: &Path, from: &str) -> Result<Vec<String>, String> {
    let mut out = lines(&git(dir, &["diff", "--name-only", from])?);
    out.extend(lines(&git(dir, &["ls-files", "--others", "--exclude-standard"])?));
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
            let files = lines(&git(dir, &["diff", "--name-only", "--diff-filter=U"]).unwrap_or_default());
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
