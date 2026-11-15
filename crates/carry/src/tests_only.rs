//! A line that changes only the project's tests (`tests_only` in its file), as each round of it ends: what its run
//! changed on the work's branch is held to the paths of the project's tests (`[tests] paths` of `project.toml`), which
//! git takes as pathspecs.
//!
//! What the run changed is what the branch's head, as the round ends, differs in from the branch's head as the run's
//! first round began, kept beside the run's rounds once for the run ([`began`]): a file a round sent back left changed
//! is seen again as each later round of the run ends, and what other lines committed before the run is not the run's.
//! Of those files only the work's own change counts, what the head holds that main, where the branch left it, does
//! not, as the look at the protected paths takes it ([`crate::protected`]): a file the run's own merge of main brought
//! in as main has it is main's. A round that does the cell's work is sent back for each such file that is none of the
//! tests; one that only asks for calls is not, and the round of its run that does the cell's work is held to the same.
//!
//! The look holds the line to its tests where nothing after it does, so a look that cannot be made fails the round,
//! saying why, rather than take its change as tests. Git is run as the look at the protected paths runs it: the
//! configuration that could make it run a program is refused first, and no external diff or conversion is run. Each
//! path git gives is taken as it is, never as a pattern.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use decide::RunId;
use store::WorkNumber;

use crate::agents::ROUNDS;
use crate::git::{self, Place};
use crate::settings::Project;

/// The paths of the project's tests its declaration names; none where it declares no `[tests]`.
pub(crate) fn paths(declared: &flow::agents::Project) -> &[String] {
    match &declared.tests {
        Some(tests) => &tests.paths,
        None => &[],
    }
}

/// Where the head of the work's branch as the run's first round began is kept.
pub(crate) fn began_at(data: &Path, work: WorkNumber, run: RunId) -> PathBuf {
    data.join(ROUNDS)
        .join(work.0.to_string())
        .join(format!("{}.head", run.0))
}

/// The head of the work's branch the run began on: as it was kept for the run; where none is kept, as at the run's
/// first round before its program runs, the branch's head now, kept for the run's later rounds.
pub(crate) async fn began(data: &Path, work: WorkNumber, run: RunId, place: &Place) -> Result<String, String> {
    let at = began_at(data, work, run);
    let named = |e: std::io::Error| format!("{}: {e}", at.display());
    match tokio::fs::read_to_string(&at).await {
        Ok(kept) => return Ok(kept.trim().to_string()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(named(e)),
    }
    let head = git::git(&place.dir, &["rev-parse", "--verify", &place.reference()]).await?;
    if let Some(dir) = at.parent() {
        tokio::fs::create_dir_all(dir).await.map_err(named)?;
    }
    // Written whole or not at all, so that a stop in the middle leaves no head the run's rounds cannot read.
    let writing = at.with_extension("head.part");
    tokio::fs::write(&writing, format!("{head}\n")).await.map_err(named)?;
    tokio::fs::rename(&writing, &at).await.map_err(named)?;
    Ok(head)
}

/// The path of each file the run changed, from the head it `began` on to the branch's head, that is of the work's own
/// change against main as `main` holds it, and that none of the paths of the project's `tests` takes in, in the order
/// git gives them; or why it cannot be said. A project that names no tests keeps the engine from starting
/// ([`crate::system::faults`]), and is a look that cannot be made here.
pub(crate) async fn outside(
    project: &Project,
    place: &Place,
    main: &str,
    began: &str,
    tests: &[String],
) -> Result<Vec<String>, String> {
    if tests.is_empty() {
        return Err("project.toml names no paths of the project's tests in [tests]".to_string());
    }
    git::configuration(project, place).await?;
    let head = place.reference();
    let changed = names(place, began, &head, &[]).await?;
    if changed.is_empty() {
        return Ok(changed);
    }
    let base = git::git(&place.dir, &["merge-base", main, &head]).await?;
    let own: BTreeSet<String> = names(place, &base, &head, &[]).await?.into_iter().collect();
    let within: BTreeSet<String> = names(place, began, &head, tests).await?.into_iter().collect();
    Ok((changed.into_iter())
        .filter(|path| own.contains(path) && !within.contains(path))
        .collect())
}

/// The path of each file that differs between the two commits, of those the pathspecs take in, all where none is
/// given; a file moved is both of its paths.
async fn names(place: &Place, from: &str, to: &str, pathspecs: &[String]) -> Result<Vec<String>, String> {
    let mut args = vec![
        "diff",
        "--no-ext-diff",
        "--no-textconv",
        "--no-renames",
        "--name-only",
        "-z",
        from,
        to,
        "--",
    ];
    args.extend(pathspecs.iter().map(String::as_str));
    let listed = git::git(&place.dir, &args).await?;
    Ok((listed.split('\0'))
        .filter(|path| !path.is_empty())
        .map(str::to_string)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::tests::{Repo, marking, run};

    const WORK: WorkNumber = WorkNumber(1);
    const RUN: RunId = RunId(3);

    /// The paths of the tests' project's tests.
    fn tests() -> Vec<String> {
        vec![
            ":(glob)crates/**/tests/**".to_string(),
            ":(glob)**/*.test.ts".to_string(),
        ]
    }

    /// A started work, its place.
    async fn started() -> (Repo, Place) {
        let repo = Repo::new("true");
        let started = git::start(&repo.project, &repo.place(WORK), "main").await.unwrap();
        (repo, started.place)
    }

    /// Writes the file in the directory, its own directory made where it has none, and commits it.
    fn commits(dir: &Path, file: &str, text: &str) {
        if let Some(parent) = dir.join(file).parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        crate::git::tests::commit(dir, file, text);
    }

    /// Removes the file in the directory and commits that.
    fn removes(dir: &Path, file: &str) {
        run(dir, &["rm", "-q", file]);
        run(dir, &["commit", "-q", "-m", &format!("Remove {file}")]);
    }

    /// What the run changed outside the tests, as a round of it ends, from where it began.
    async fn found(repo: &Repo, place: &Place, began: &str) -> Result<Vec<String>, String> {
        outside(&repo.project, place, "main", began, &tests()).await
    }

    #[tokio::test]
    async fn a_run_that_changes_only_tests_changes_nothing_outside_them() {
        let (repo, place) = started().await;
        let began = began(&repo.data, WORK, RUN, &place).await.unwrap();
        commits(&place.dir, "crates/greeting/tests/names.rs", "#[test] fn greets() {}\n");
        commits(&place.dir, "client/src/greeting.test.ts", "it('greets', () => {});\n");
        assert_eq!(found(&repo, &place, &began).await, Ok(Vec::new()));
    }

    #[tokio::test]
    async fn each_file_the_run_changed_outside_the_tests_is_named_added_changed_or_deleted() {
        let (repo, place) = started().await;
        commits(&place.dir, "src/lib.rs", "pub fn greet() {}\n");
        let began = began(&repo.data, WORK, RUN, &place).await.unwrap();
        commits(&place.dir, "crates/greeting/tests/names.rs", "#[test] fn greets() {}\n");
        commits(&place.dir, "Cargo.lock", "# A lock.\n");
        commits(&place.dir, "src/lib.rs", "pub fn greet() { todo!() }\n");
        removes(&place.dir, "greeting.txt");
        let mut named = found(&repo, &place, &began).await.unwrap();
        named.sort();
        assert_eq!(named, ["Cargo.lock", "greeting.txt", "src/lib.rs"]);
    }

    // What another line committed before the run is not the run's, and a file the run changed and put back as it was
    // is no change of it.
    #[tokio::test]
    async fn what_was_committed_before_the_run_or_taken_back_in_it_is_not_its_change() {
        let (repo, place) = started().await;
        commits(&place.dir, "src/lib.rs", "pub fn greet() {}\n");
        let began = began(&repo.data, WORK, RUN, &place).await.unwrap();
        commits(&place.dir, "notes.md", "A note.\n");
        removes(&place.dir, "notes.md");
        commits(&place.dir, "crates/greeting/tests/names.rs", "#[test] fn greets() {}\n");
        assert_eq!(found(&repo, &place, &began).await, Ok(Vec::new()));
    }

    // The head the run began on is kept for the run as its first round asks for it: a later round of the run, after a
    // round sent back that left a file changed, is held to the same, and another run begins where the branch then is.
    #[tokio::test]
    async fn the_head_a_run_began_on_is_kept_for_its_later_rounds_and_another_run_begins_anew() {
        let (repo, place) = started().await;
        let first = began(&repo.data, WORK, RUN, &place).await.unwrap();
        commits(&place.dir, "src/lib.rs", "pub fn greet() {}\n");
        let later = began(&repo.data, WORK, RUN, &place).await.unwrap();
        assert_eq!(later, first, "the run's later round is held to where the run began");
        commits(&place.dir, "crates/greeting/tests/names.rs", "#[test] fn greets() {}\n");
        assert_eq!(found(&repo, &place, &later).await, Ok(vec!["src/lib.rs".to_string()]));
        let next = began(&repo.data, WORK, RunId(RUN.0 + 1), &place).await.unwrap();
        assert_ne!(next, first);
        assert_eq!(found(&repo, &place, &next).await, Ok(Vec::new()));
    }

    // A file the run's own merge of main brings in as main has it is main's, not the run's; one the run changes as well
    // is the run's.
    #[tokio::test]
    async fn what_main_brings_in_is_not_the_runs_change() {
        let (repo, place) = started().await;
        let began = began(&repo.data, WORK, RUN, &place).await.unwrap();
        commits(&repo.project.repo, "src/main.rs", "fn main() {}\n");
        commits(&repo.project.repo, "README.md", "Read me.\n");
        run(&place.dir, &["merge", "-q", "--no-edit", "main"]);
        commits(&place.dir, "crates/greeting/tests/names.rs", "#[test] fn greets() {}\n");
        assert_eq!(found(&repo, &place, &began).await, Ok(Vec::new()));
        commits(&place.dir, "README.md", "Read me first.\n");
        assert_eq!(found(&repo, &place, &began).await, Ok(vec!["README.md".to_string()]));
    }

    // A file is named by its path as it is, whatever it holds, never quoted or escaped as git quotes a path it prints
    // a line for; and a directory of tests whose name reads as a pattern is still the tests' directory.
    #[tokio::test]
    async fn a_file_is_named_by_its_path_as_it_is() {
        let (repo, place) = started().await;
        let began = began(&repo.data, WORK, RUN, &place).await.unwrap();
        commits(&place.dir, "crates/a*/tests/x.rs", "#[test] fn x() {}\n");
        commits(&place.dir, "notes/a \"large\" note*.md", "Large.\n");
        assert_eq!(
            found(&repo, &place, &began).await,
            Ok(vec!["notes/a \"large\" note*.md".to_string()])
        );
    }

    #[tokio::test]
    async fn a_project_that_names_no_tests_is_no_look_that_can_be_made() {
        let (repo, place) = started().await;
        let began = began(&repo.data, WORK, RUN, &place).await.unwrap();
        commits(&place.dir, "crates/greeting/tests/names.rs", "#[test] fn greets() {}\n");
        let none = outside(&repo.project, &place, "main", &began, &[]).await;
        assert!(matches!(&none, Err(why) if why.contains("[tests]")), "{none:?}");
    }

    #[tokio::test]
    async fn a_configuration_that_names_a_program_is_refused_before_the_look() {
        let (repo, place) = started().await;
        let began = began(&repo.data, WORK, RUN, &place).await.unwrap();
        commits(&place.dir, "src/lib.rs", "pub fn greet() {}\n");
        let (program, marked) = (repo.data.join("external"), repo.data.join("external-ran"));
        marking(&program, &marked);
        let program = program.display().to_string();
        run(&repo.project.repo, &["config", "diff.external", &program]);
        let refused = found(&repo, &place, &began).await;
        assert!(
            matches!(&refused, Err(why) if why.contains("diff.external")),
            "{refused:?}"
        );
        assert!(!marked.exists(), "the external diff was run");
    }
}
