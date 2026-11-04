//! A project's decisions in force, as its repository keeps them: one file for each decision in the directory its
//! configuration names ([`Project::decisions`]), read at one commit of its main as the constitution is; and the
//! history of one on its main, each version main had with the commit that made it. The directory holds only what is in
//! force: a decision replaced or withdrawn lives on in the repository's history alone, which is why the history is read
//! from there.

use std::collections::BTreeMap;
use std::path::{Component, Path};

use serde::Serialize;

use super::{Found, Project, blob_texts, git, main_commit, resolve, walk_files};

/// The name of the file that says what a directory of decisions holds: no decision, and never given as one.
pub const README: &str = "README.md";

/// The project's directory of decisions as a path from its repository's root, its parts joined by `/`, with no `./`
/// and no slash at its end, such as `decisions`.
pub fn dir(p: &Project) -> String {
    p.decisions
        .components()
        .filter_map(|c| match c {
            Component::Normal(x) => Some(x.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// Whether a file of the directory of decisions, by its path, is a decision: every file is but a [`README`].
fn is_decision(path: &str) -> bool {
    Path::new(path).file_name().is_none_or(|n| n != README)
}

/// The project's decisions in force as the commit `commit` of its checkout has them: each file under its directory of
/// decisions but a [`README`], by its path from the repository's root, with its text, in the order of the paths; none
/// when the directory is not there. They are read with git, so that no file of a worktree and nothing a branch
/// committed beyond that commit changes them, and a link is followed as the records' links are. A directory that is a
/// file, a link that cannot be followed and a failure of git are errors that name the directory and the commit.
pub fn in_force(p: &Project, commit: &str) -> Result<Vec<(String, String)>, String> {
    let repo = &p.checkout;
    let dir = dir(p);
    let fail = |x: String| format!("{dir} cannot be read at {commit} in {}: {x}", repo.display());
    let tree = match resolve(repo, commit, &dir).map_err(fail)? {
        Found::Missing(_) => return Ok(Vec::new()),
        Found::File(_) => return Err(fail("it is a file, not a directory".into())),
        Found::Dir { tree, .. } => tree,
    };
    let mut found = BTreeMap::new();
    walk_files(
        repo,
        commit,
        &format!("{dir}/"),
        &tree,
        &mut vec![tree.clone()],
        &mut found,
        &is_decision,
    )?;
    let objects: Vec<&str> = found.values().map(String::as_str).collect();
    let texts = blob_texts(repo, &objects)?;
    Ok(found.into_keys().zip(texts).collect())
}

/// The path from the repository's root of the decision's file an agent names: by that path, as the decisions in force
/// are given (`decisions/one.md`), or by its path in the directory of decisions (`one.md`), which is put under it. A
/// path that leaves the repository or starts from outside it, and one that names no file of the directory or its
/// [`README`], name no decision: the error says so.
pub fn named(p: &Project, path: &str) -> Result<String, String> {
    let dir = dir(p);
    let mut parts = Vec::new();
    for c in Path::new(path.trim()).components() {
        match c {
            Component::Normal(x) => parts.push(x.to_string_lossy().into_owned()),
            Component::CurDir => {}
            _ => {
                return Err(format!(
                    "{path} is not a path inside the repository, from its root or from {dir}/"
                ));
            }
        }
    }
    let joined = parts.join("/");
    let full = if joined.starts_with(&format!("{dir}/")) {
        joined
    } else {
        format!("{dir}/{joined}")
    };
    if parts.is_empty() || !is_decision(&full) {
        return Err(format!(
            "{path} names no decision: a decision is a file in {dir}/ other than its {README}"
        ));
    }
    Ok(full)
}

/// A commit in the history of a decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Commit {
    /// Its id.
    pub commit: String,
    /// When it was committed, in ISO 8601.
    pub date: String,
    /// Its message, whole.
    pub message: String,
}

/// What a version did to a decision's file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Change {
    /// It made the file, new or as a copy of another (`from`).
    Added,
    /// It rewrote the file where it was.
    Changed,
    /// It moved the file from another path (`from`), its text changed or not.
    Renamed,
    /// It deleted the file: the decision was withdrawn.
    Deleted,
}

/// One version of a decision's file on the project's main: what the commit of main that made it did to the file, the
/// file's path and text as it left them, and, for a merge, the commits it brought that changed the file, whose
/// messages say what changed and why where the merge's own does not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Version {
    #[serde(flatten)]
    pub made_by: Commit,
    pub change: Change,
    /// The file's path in this version; for a deletion, the path it had.
    pub path: String,
    /// For a rename or a copy, the path the file came from.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    /// The file's text in this version; none for a deletion.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// For a merge, the commits it brought that changed the file, newest first.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub brought: Vec<Commit>,
}

/// How a commit is written for the history to be read back: the record marker, then the id, the parents, the date and
/// the message, each closed by the field marker, so that a message holds anything but the record marker.
const FORMAT: &str = "--format=%x1e%H%x1f%P%x1f%cI%x1f%B%x1f";

/// The marker each commit's record starts with.
const RECORD: char = '\u{1e}';

/// The marker that closes each field of a commit's record.
const FIELD: char = '\u{1f}';

/// The history of a decision's file, by its path from the repository's root ([`named`]), on the project's main as its
/// checkout last fetched it (`origin/<main>`): each version main had, newest first, as the commits of main's
/// first-parent line that changed the file made them, the file followed through its renames
/// (`git log --first-parent --follow`), a deletion included. A commit of the line that merged a branch is one version,
/// with the commits of the branch that changed the file beside it. None when no commit of main ever had a file there.
pub fn history(p: &Project, path: &str) -> Result<Vec<Version>, String> {
    let repo = &p.checkout;
    let main = main_commit(p)?;
    let log = git(
        repo,
        &[
            "log",
            "--no-color",
            "--first-parent",
            "--diff-merges=first-parent",
            "--follow",
            "-z",
            "--name-status",
            FORMAT,
            &main,
            "--",
            path,
        ],
    )?;
    let mut versions = Vec::new();
    let mut merges = Vec::new();
    for record in log.split(RECORD).filter(|r| !r.trim_matches(['\0', '\n']).is_empty()) {
        let (made_by, parents, rest) = commit_of(record)?;
        // After the fields, the status of the file and its paths, each closed by a NUL: `M`, `A` or `D` and the path,
        // or `R<score>` or `C<score>` and the path it came from and the path it went to.
        let status: Vec<&str> = rest
            .split('\0')
            .map(|s| s.trim_start_matches('\n'))
            .filter(|s| !s.is_empty())
            .collect();
        let (change, at, from) = match status.as_slice() {
            [s, at] if s.starts_with('A') => (Change::Added, *at, None),
            [s, at] if s.starts_with('M') || s.starts_with('T') => (Change::Changed, *at, None),
            [s, at] if s.starts_with('D') => (Change::Deleted, *at, None),
            [s, from, at] if s.starts_with('R') => (Change::Renamed, *at, Some(from.to_string())),
            [s, from, at] if s.starts_with('C') => (Change::Added, *at, Some(from.to_string())),
            other => {
                return Err(format!(
                    "the history of {path} at {} cannot be read: git gave the change {other:?}",
                    made_by.commit
                ));
            }
        };
        if parents.len() > 1 {
            merges.push((versions.len(), parents[0].clone()));
        }
        versions.push(Version {
            made_by,
            change,
            path: at.to_string(),
            from,
            text: None,
            brought: Vec::new(),
        });
    }
    let kept: Vec<(usize, String)> = versions
        .iter()
        .enumerate()
        .filter(|(_, v)| v.change != Change::Deleted)
        .map(|(i, v)| (i, format!("{}:{}", v.made_by.commit, v.path)))
        .collect();
    let names: Vec<&str> = kept.iter().map(|(_, name)| name.as_str()).collect();
    for ((i, _), text) in kept.iter().zip(blob_texts(repo, &names)?) {
        versions[*i].text = Some(text);
    }
    for (i, first_parent) in merges {
        let v = &versions[i];
        let range = format!("{first_parent}..{}", v.made_by.commit);
        let log = git(repo, &["log", "--no-color", "--follow", FORMAT, &range, "--", &v.path])?;
        let mut brought = Vec::new();
        for record in log.split(RECORD).filter(|r| !r.trim().is_empty()) {
            let (c, _, _) = commit_of(record)?;
            if c.commit != v.made_by.commit {
                brought.push(c);
            }
        }
        versions[i].brought = brought;
    }
    Ok(versions)
}

/// A commit's record as [`FORMAT`] writes it, without its marker: the commit, its parents, and what follows its fields.
fn commit_of(record: &str) -> Result<(Commit, Vec<String>, &str), String> {
    let bad = || format!("git log gave a commit it cannot be read from: {record:?}");
    let mut fields = record.splitn(4, FIELD);
    let (Some(id), Some(parents), Some(date), Some(rest)) =
        (fields.next(), fields.next(), fields.next(), fields.next())
    else {
        return Err(bad());
    };
    // The message is what stands before the last marker: it may hold the marker itself.
    let (message, after) = rest.rsplit_once(FIELD).ok_or_else(bad)?;
    let commit = Commit {
        commit: id.trim().to_string(),
        date: date.to_string(),
        message: message.trim_end().to_string(),
    };
    Ok((commit, parents.split_whitespace().map(str::to_string).collect(), after))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// A repository on the remote, and a checkout of it whose `origin/main` the history is read at, in a directory of
    /// their own; and another clone that commits and pushes, as works do.
    struct Repos {
        root: PathBuf,
        checkout: PathBuf,
        elsewhere: PathBuf,
    }

    fn run(dir: &Path, args: &[&str]) -> String {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args([
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap().trim().to_string()
    }

    impl Repos {
        fn new(name: &str) -> Repos {
            let root = std::env::temp_dir().join(format!("wf-agents-decisions-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(&root).unwrap();
            let origin = root.join("origin.git").to_string_lossy().into_owned();
            run(&root, &["init", "-q", "--bare", "-b", "main", &origin]);
            let (checkout, elsewhere) = (root.join("checkout"), root.join("elsewhere"));
            run(&root, &["clone", "-q", &origin, &checkout.to_string_lossy()]);
            run(&root, &["clone", "-q", &origin, &elsewhere.to_string_lossy()]);
            run(&elsewhere, &["symbolic-ref", "HEAD", "refs/heads/main"]);
            Repos {
                root,
                checkout,
                elsewhere,
            }
        }

        /// Writes the files given (a text of none deletes one) in the other clone and commits them with the message.
        fn commit(&self, files: &[(&str, Option<&str>)], message: &str) -> String {
            for (path, text) in files {
                let at = self.elsewhere.join(path);
                match text {
                    Some(t) => {
                        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
                        std::fs::write(at, t).unwrap();
                    }
                    None => std::fs::remove_file(at).unwrap(),
                }
            }
            run(&self.elsewhere, &["add", "-A"]);
            run(&self.elsewhere, &["commit", "-q", "-m", message]);
            run(&self.elsewhere, &["rev-parse", "HEAD"])
        }

        /// Pushes the other clone's main and fetches it into the checkout: main as the checkout last fetched it.
        fn publish(&self) -> String {
            run(&self.elsewhere, &["push", "-q", "origin", "main"]);
            run(
                &self.checkout,
                &["fetch", "-q", "origin", "+refs/heads/main:refs/remotes/origin/main"],
            );
            run(&self.checkout, &["rev-parse", "origin/main"])
        }

        fn project(&self, decisions: &str) -> Project {
            project(&self.checkout, Some(decisions))
        }
    }

    /// A project of the checkout given that keeps its decisions where `decisions` says, or sets nothing.
    fn project(checkout: &Path, decisions: Option<&str>) -> Project {
        let mut p = serde_json::json!({
            "repo": "o/p", "checkout": checkout, "design_goal": "simple", "test_paths": ["tests/"],
            "ci": { "fast": "fast", "full": ["ci"], "merge_checks": ["check"] }
        });
        if let Some(d) = decisions {
            p["decisions"] = serde_json::json!(d);
        }
        serde_json::from_value(p).unwrap()
    }

    impl Drop for Repos {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    // A project that sets nothing keeps its decisions in `decisions`; the directory is read as one path whatever way
    // it is spelled.
    #[test]
    fn a_project_keeps_its_decisions_in_decisions_unless_it_names_another_directory() {
        let at = Path::new("/p");
        assert_eq!(dir(&project(at, None)), "decisions");
        assert_eq!(dir(&project(at, Some("./docs/decided/"))), "docs/decided");
    }

    // The decisions in force are every file under the directory as the commit has it, a README aside, each by its path
    // with its text; none when there is no directory or only a README. What the checkout's files or its own commits
    // hold is never read, a directory that is a file is an error, and another directory is read where a project names
    // one.
    #[test]
    fn the_decisions_in_force_are_the_files_of_the_directory_at_the_commit_but_its_readme() {
        let r = Repos::new("in-force");
        let p = r.project("decisions");
        let none = r.commit(&[("AGENTS.md", Some("How to work.\n"))], "Start");
        r.publish();
        assert_eq!(in_force(&p, &none).unwrap(), Vec::new());
        let readme = r.commit(
            &[("decisions/README.md", Some("What this holds.\n"))],
            "Say what decisions are",
        );
        r.publish();
        assert_eq!(in_force(&p, &readme).unwrap(), Vec::new());
        let some = r.commit(
            &[
                (
                    "decisions/one-flow.md",
                    Some("# One flow\n\nOne flow for every project.\n"),
                ),
                ("decisions/later/README.md", Some("What this holds.\n")),
                ("decisions/later/notes.txt", Some("A decision not in Markdown.\n")),
                ("elsewhere.md", Some("No decision.\n")),
            ],
            "Decide",
        );
        r.publish();
        let given = in_force(&p, &some).unwrap();
        assert_eq!(
            given,
            [
                (
                    "decisions/later/notes.txt".to_string(),
                    "A decision not in Markdown.\n".to_string()
                ),
                (
                    "decisions/one-flow.md".to_string(),
                    "# One flow\n\nOne flow for every project.\n".to_string()
                ),
            ]
        );
        // The checkout's own files and commits are not read: only the commit asked for.
        std::fs::create_dir_all(r.checkout.join("decisions")).unwrap();
        std::fs::write(r.checkout.join("decisions/on-disk.md"), "Not committed.\n").unwrap();
        assert_eq!(in_force(&p, &some).unwrap(), given);
        assert_eq!(in_force(&p, &readme).unwrap(), Vec::new());
        // Another directory, where the project names one.
        let other = r.project("decisions/later");
        assert_eq!(
            in_force(&other, &some).unwrap(),
            [(
                "decisions/later/notes.txt".to_string(),
                "A decision not in Markdown.\n".to_string()
            )]
        );
        // A directory that is a file.
        let file = r.project("elsewhere.md");
        let e = in_force(&file, &some).unwrap_err();
        assert!(
            e.contains("elsewhere.md") && e.contains(&some) && e.contains("not a directory"),
            "{e}"
        );
    }

    // An agent names a decision by its path from the root, as the decisions are given, or by its path in the
    // directory; a path that leaves the repository, the directory itself and its README name none.
    #[test]
    fn a_decision_is_named_by_its_path_from_the_root_or_in_the_directory() {
        let p = project(Path::new("/p"), None);
        assert_eq!(named(&p, "decisions/one.md").unwrap(), "decisions/one.md");
        assert_eq!(named(&p, " ./decisions/one.md ").unwrap(), "decisions/one.md");
        assert_eq!(named(&p, "one.md").unwrap(), "decisions/one.md");
        assert_eq!(named(&p, "later/two.md").unwrap(), "decisions/later/two.md");
        for bad in ["../one.md", "/etc/hosts", "decisions/../x.md"] {
            let e = named(&p, bad).unwrap_err();
            assert!(e.contains(bad) && e.contains("inside the repository"), "{e}");
        }
        for none in ["", ".", "README.md", "decisions/README.md", "decisions/later/README.md"] {
            let e = named(&p, none).unwrap_err();
            assert!(e.contains("names no decision"), "{none}: {e}");
        }
    }

    // The history of a decision is each version main's first-parent line had of it, newest first: made, rewritten,
    // moved, rewritten on a branch merged in, whose commits stand beside the merge, and withdrawn; each with its
    // commit, date and whole message, and its text but for the deletion. A commit of the checkout's own, not on main,
    // is none of it, and a path main never had has none.
    #[test]
    fn a_decision_s_history_is_each_version_main_had_with_the_commit_that_made_it() {
        let r = Repos::new("history");
        let p = r.project("decisions");
        let text = |what: &str| format!("# A\n\nWe do {what}, since it is simplest.\n\nNot chosen: the other way.\n");
        let made = r.commit(&[("decisions/a.md", Some(&text("A")))], "Decide A");
        let rewritten = r.commit(
            &[("decisions/a.md", Some(&text("A2")))],
            "Do A2 in place of A\n\nA2 holds where A did not.",
        );
        run(&r.elsewhere, &["mv", "decisions/a.md", "decisions/b.md"]);
        let moved = r.commit(&[], "Name the decision for what it decides");
        // A branch rewrites it twice, and is merged with a commit of its own.
        run(&r.elsewhere, &["checkout", "-q", "-b", "side"]);
        let draft = r.commit(&[("decisions/b.md", Some(&text("B")))], "Draft B");
        let finished = r.commit(
            &[("decisions/b.md", Some(&text("B, always")))],
            "Do B, always\n\nThe owner chose it.",
        );
        r.commit(&[("other.txt", Some("x\n"))], "Something else");
        run(&r.elsewhere, &["checkout", "-q", "main"]);
        run(&r.elsewhere, &["merge", "-q", "--no-ff", "side", "-m", "Merge side"]);
        let merge = run(&r.elsewhere, &["rev-parse", "HEAD"]);
        let withdrawn = r.commit(&[("decisions/b.md", None)], "Withdraw B\n\nNothing needs it now.");
        r.publish();
        // A commit only the checkout has.
        std::fs::create_dir_all(r.checkout.join("decisions")).unwrap();
        std::fs::write(r.checkout.join("decisions/b.md"), "Local.\n").unwrap();
        run(&r.checkout, &["add", "-A"]);
        run(&r.checkout, &["commit", "-q", "-m", "Local only"]);

        let versions = history(&p, "decisions/b.md").unwrap();
        // Each version as its commit, its change, its path, the path it came from and its text.
        type Seen<'a> = (&'a str, Change, &'a str, Option<&'a str>, Option<&'a str>);
        let seen: Vec<Seen> = versions
            .iter()
            .map(|v| {
                (
                    v.made_by.commit.as_str(),
                    v.change,
                    v.path.as_str(),
                    v.from.as_deref(),
                    v.text.as_deref(),
                )
            })
            .collect();
        let (b, a2, a) = (text("B, always"), text("A2"), text("A"));
        assert_eq!(
            seen,
            [
                (withdrawn.as_str(), Change::Deleted, "decisions/b.md", None, None),
                (
                    merge.as_str(),
                    Change::Changed,
                    "decisions/b.md",
                    None,
                    Some(b.as_str())
                ),
                (
                    moved.as_str(),
                    Change::Renamed,
                    "decisions/b.md",
                    Some("decisions/a.md"),
                    Some(a2.as_str())
                ),
                (
                    rewritten.as_str(),
                    Change::Changed,
                    "decisions/a.md",
                    None,
                    Some(a2.as_str())
                ),
                (made.as_str(), Change::Added, "decisions/a.md", None, Some(a.as_str())),
            ]
        );
        assert_eq!(versions[0].made_by.message, "Withdraw B\n\nNothing needs it now.");
        assert_eq!(
            versions[3].made_by.message,
            "Do A2 in place of A\n\nA2 holds where A did not."
        );
        assert_eq!(versions[1].made_by.message, "Merge side");
        let date = run(&r.elsewhere, &["show", "-s", "--format=%cI", &withdrawn]);
        assert_eq!(versions[0].made_by.date, date);
        // The merge carries the commits of its branch that changed the file, newest first, the others left out.
        let brought: Vec<(&str, &str)> = versions[1]
            .brought
            .iter()
            .map(|c| (c.commit.as_str(), c.message.as_str()))
            .collect();
        assert_eq!(
            brought,
            [
                (finished.as_str(), "Do B, always\n\nThe owner chose it."),
                (draft.as_str(), "Draft B")
            ]
        );
        assert!(versions.iter().enumerate().all(|(i, v)| i == 1 || v.brought.is_empty()));
        // Written as the tool gives it: no field that holds nothing.
        let given = serde_json::to_value(&versions[0]).unwrap();
        assert_eq!(given["change"], "deleted");
        assert!(given.get("text").is_none() && given.get("from").is_none() && given.get("brought").is_none());
        assert_eq!(serde_json::to_value(&versions[2]).unwrap()["from"], "decisions/a.md");
        // The old path's history is the old file's alone: main has had no file there since it moved.
        let old: Vec<String> = history(&p, "decisions/a.md")
            .unwrap()
            .into_iter()
            .map(|v| v.made_by.commit)
            .collect();
        assert_eq!(old, [moved, rewritten, made]);
        assert_eq!(history(&p, "decisions/never.md").unwrap(), Vec::new());
    }
}
