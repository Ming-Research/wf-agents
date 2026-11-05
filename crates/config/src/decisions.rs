//! A project's decisions in force, as its repository keeps them: one file for each decision in the directory its
//! configuration names ([`Project::decisions`]), read at one commit of its main as the constitution is. The
//! directory holds only what is in force: a decision replaced or withdrawn lives on in the repository's history alone.

use std::collections::BTreeMap;
use std::path::{Component, Path};

use super::{Found, Project, blob_texts, resolve, walk_files};

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
}
