//! A line that changes only the project's tests (`tests_only` in its file), as each round of it ends with a return of
//! either kind: what its run changed of the work is held to the paths of the project's tests (`[tests] paths` of
//! `project.toml`), which git takes as pathspecs.
//!
//! What the run changed is what the work itself changed from the version of the work's head the run began on, as the
//! records keep it, to the work's head as the round ended: the files of the diff a round is given between two versions
//! of the head, which leaves main's code out ([`head::base`]). With no version to begin on, the branch holding no change
//! of its own as the run began, it begins where the branch left main. Where that diff is the whole change against main,
//! the two heads not being comparable without main's code, the run changed only those of its files the two heads
//! differ in. So a file a round that was sent back left changed is seen again as the next round ends, a file put back
//! as main has it is a change, and what other lines committed before the run is not the run's. A return that asks for
//! calls is held to it as well: taken, it ends its run, and the run entered for the results begins on the head it left.
//!
//! The look holds the line to its tests where nothing after it does, so a look that cannot be made fails the round,
//! saying why, rather than take its change as tests. Git is run as the look at the protected paths runs it: the
//! configuration that could make it run a program is refused first, and no external diff or conversion is run. Each
//! path git gives is taken as it is, never as a pattern.

use std::collections::BTreeSet;
use std::path::Path;

use crate::git::{self, Place};
use crate::head::{self, Base};
use crate::settings::Project;

/// The paths of the project's tests its declaration names; none where it declares no `[tests]`.
pub(crate) fn paths(declared: &flow::agents::Project) -> &[String] {
    match &declared.tests {
        Some(tests) => &tests.paths,
        None => &[],
    }
}

/// The path of each file the run changed that none of the paths of the project's `tests` takes in, in the order git
/// gives them; or why it cannot be said. The run changed what the work itself changed from the version of its head it
/// `began` on, none where there was none, to its head `now` as the round ended, none where the branch holds no change
/// of its own; main being where the reference `main` holds it. A project that names no tests keeps the engine from
/// starting ([`crate::system::faults`]), and is a look that cannot be made here.
pub(crate) async fn outside(
    project: &Project,
    place: &Place,
    main: &str,
    began: Option<&str>,
    now: Option<&str>,
    tests: &[String],
) -> Result<Vec<String>, String> {
    if tests.is_empty() {
        return Err("project.toml names no paths of the project's tests in [tests]".to_string());
    }
    let Some(now) = now else {
        return Ok(Vec::new());
    };
    git::configuration(project, place).await?;
    let dir = &place.dir;
    let began = match began {
        Some(began) => began.to_string(),
        None => git::git(dir, &["merge-base", main, now]).await?,
    };
    let base = head::base(dir, main, &began, now).await?;
    let changed = names(dir, base.taken_from(), now, &[]).await?;
    if changed.is_empty() {
        return Ok(changed);
    }
    let within: BTreeSet<String> = names(dir, base.taken_from(), now, tests).await?.into_iter().collect();
    // Of the whole change against main, the run's are those the two heads differ in.
    let ran: Option<BTreeSet<String>> = match base {
        Base::Work(_) => None,
        Base::Main(_) => Some(names(dir, &began, now, &[]).await?.into_iter().collect()),
    };
    Ok((changed.into_iter())
        .filter(|path| !within.contains(path) && ran.as_ref().is_none_or(|ran| ran.contains(path)))
        .collect())
}

/// The path of each file that differs between the two, a commit or a tree first and a commit second, of those the
/// pathspecs take in, all where none is given; a file moved is both of its paths.
async fn names(dir: &Path, from: &str, to: &str, pathspecs: &[String]) -> Result<Vec<String>, String> {
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
    let listed = git::git(dir, &args).await?;
    Ok((listed.split('\0'))
        .filter(|path| !path.is_empty())
        .map(str::to_string)
        .collect())
}

#[cfg(test)]
mod tests {
    //! Against a work's branch in a repository made for each test, as the agents' rounds read it. Each test makes a
    //! repository of a few commits and runs a few dozen git commands, in well under a second.

    use store::WorkNumber;

    use super::*;
    use crate::git::tests::{Repo, marking, run};

    const WORK: WorkNumber = WorkNumber(1);

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

    /// The work's head as the branch has it now, as its records would keep it: none where it holds no change of its
    /// own.
    async fn work_head(place: &Place) -> Option<String> {
        head::read(place, "main", None).await.unwrap()
    }

    /// What the run changed outside the tests, as a round of it ends, from the work's head it began on.
    async fn found(repo: &Repo, place: &Place, began: Option<&str>) -> Result<Vec<String>, String> {
        let now = work_head(place).await;
        outside(&repo.project, place, "main", began, now.as_deref(), &tests()).await
    }

    // A work with no change of its own begins where its branch left main.
    #[tokio::test]
    async fn a_run_that_changes_only_tests_changes_nothing_outside_them() {
        let (repo, place) = started().await;
        let began = work_head(&place).await;
        assert_eq!(began, None, "the branch holds no change of its own");
        assert_eq!(found(&repo, &place, None).await, Ok(Vec::new()));
        commits(&place.dir, "crates/greeting/tests/names.rs", "#[test] fn greets() {}\n");
        commits(&place.dir, "client/src/greeting.test.ts", "it('greets', () => {});\n");
        assert_eq!(found(&repo, &place, None).await, Ok(Vec::new()));
    }

    #[tokio::test]
    async fn each_file_the_run_changed_outside_the_tests_is_named_added_changed_or_deleted() {
        let (repo, place) = started().await;
        commits(&place.dir, "src/lib.rs", "pub fn greet() {}\n");
        let began = work_head(&place).await;
        commits(&place.dir, "crates/greeting/tests/names.rs", "#[test] fn greets() {}\n");
        commits(&place.dir, "Cargo.lock", "# A lock.\n");
        commits(&place.dir, "src/lib.rs", "pub fn greet() { todo!() }\n");
        removes(&place.dir, "greeting.txt");
        let mut named = found(&repo, &place, began.as_deref()).await.unwrap();
        named.sort();
        assert_eq!(named, ["Cargo.lock", "greeting.txt", "src/lib.rs"]);
    }

    // What another line committed before the run is not the run's, and a file the run changed and put back as it was
    // is no change of it.
    #[tokio::test]
    async fn what_was_committed_before_the_run_or_taken_back_in_it_is_not_its_change() {
        let (repo, place) = started().await;
        commits(&place.dir, "src/lib.rs", "pub fn greet() {}\n");
        let began = work_head(&place).await;
        commits(&place.dir, "notes.md", "A note.\n");
        removes(&place.dir, "notes.md");
        commits(&place.dir, "crates/greeting/tests/names.rs", "#[test] fn greets() {}\n");
        assert_eq!(found(&repo, &place, began.as_deref()).await, Ok(Vec::new()));
    }

    // A file the run puts back as main has it changes what another line made of it: a file of main's that line changed,
    // and one that line added and the run deletes.
    #[tokio::test]
    async fn a_file_the_run_puts_back_as_main_has_it_is_its_change() {
        let (repo, place) = started().await;
        commits(&place.dir, "greeting.txt", "Hello, you.\n");
        commits(&place.dir, "src/lib.rs", "pub fn greet() {}\n");
        let began = work_head(&place).await;
        commits(&place.dir, "greeting.txt", "Hello.\n");
        removes(&place.dir, "src/lib.rs");
        commits(&place.dir, "crates/greeting/tests/names.rs", "#[test] fn greets() {}\n");
        let mut named = found(&repo, &place, began.as_deref()).await.unwrap();
        named.sort();
        assert_eq!(named, ["greeting.txt", "src/lib.rs"]);
    }

    // What the run's merge of main brings in is main's, a file both main and another line changed included; a file the
    // run changes after the merge is the run's.
    #[tokio::test]
    async fn what_main_brings_in_is_not_the_runs_change() {
        let (repo, place) = started().await;
        let story = "One.\nTwo.\nThree.\nFour.\nFive.\n";
        commits(&repo.project.repo, "story.txt", story);
        run(&place.dir, &["merge", "-q", "--no-edit", "main"]);
        commits(&place.dir, "story.txt", &story.replace("One.", "One, the work's."));
        let began = work_head(&place).await;
        let both = story.replace("Five.", "Five, main's.");
        commits(&repo.project.repo, "story.txt", &both);
        commits(&repo.project.repo, "README.md", "Read me.\n");
        run(&place.dir, &["merge", "-q", "--no-edit", "main"]);
        commits(&place.dir, "crates/greeting/tests/names.rs", "#[test] fn greets() {}\n");
        assert_eq!(found(&repo, &place, began.as_deref()).await, Ok(Vec::new()));
        commits(&place.dir, "README.md", "Read me first.\n");
        assert_eq!(
            found(&repo, &place, began.as_deref()).await,
            Ok(vec!["README.md".to_string()])
        );
    }

    // Where the head the run began on does not merge with the main the run took in without a conflict, the diff is the
    // whole change against main, and of it only what the two heads differ in is the run's: its resolution of the
    // conflict, and not what another line committed before the run, nor what main brought in elsewhere.
    #[tokio::test]
    async fn of_the_whole_change_against_main_only_what_the_two_heads_differ_in_is_the_runs() {
        let (repo, place) = started().await;
        commits(&place.dir, "src/lib.rs", "pub fn greet() {}\n");
        commits(&place.dir, "greeting.txt", "Hello, the work.\n");
        let began = work_head(&place).await;
        commits(&repo.project.repo, "greeting.txt", "Hello, main.\n");
        commits(&repo.project.repo, "README.md", "Read me.\n");
        let merging = std::process::Command::new("git")
            .arg("-C")
            .arg(&place.dir)
            .args(["merge", "-q", "--no-edit", "main"])
            .output()
            .expect("git runs");
        assert!(!merging.status.success(), "the two conflict");
        std::fs::write(place.dir.join("greeting.txt"), "Hello, both.\n").unwrap();
        run(&place.dir, &["add", "greeting.txt"]);
        run(&place.dir, &["commit", "-q", "--no-edit"]);
        commits(&place.dir, "crates/greeting/tests/names.rs", "#[test] fn greets() {}\n");
        let now = work_head(&place).await.expect("the work has a head");
        let base = head::base(&place.dir, "main", began.as_deref().unwrap(), &now).await;
        assert!(matches!(base, Ok(Base::Main(_))), "{base:?}");
        assert_eq!(
            found(&repo, &place, began.as_deref()).await,
            Ok(vec!["greeting.txt".to_string()])
        );
    }

    // A file is named by its path as it is, whatever it holds, never quoted or escaped as git quotes a path it prints
    // a line for; and a directory of tests whose name reads as a pattern is still the tests' directory.
    #[tokio::test]
    async fn a_file_is_named_by_its_path_as_it_is() {
        let (repo, place) = started().await;
        commits(&place.dir, "crates/a*/tests/x.rs", "#[test] fn x() {}\n");
        commits(&place.dir, "notes/a \"large\" note*.md", "Large.\n");
        assert_eq!(
            found(&repo, &place, None).await,
            Ok(vec!["notes/a \"large\" note*.md".to_string()])
        );
    }

    #[tokio::test]
    async fn a_project_that_names_no_tests_is_no_look_that_can_be_made() {
        let (repo, place) = started().await;
        commits(&place.dir, "crates/greeting/tests/names.rs", "#[test] fn greets() {}\n");
        let now = work_head(&place).await;
        let none = outside(&repo.project, &place, "main", None, now.as_deref(), &[]).await;
        assert!(matches!(&none, Err(why) if why.contains("[tests]")), "{none:?}");
    }

    #[tokio::test]
    async fn a_configuration_that_names_a_program_is_refused_before_the_look() {
        let (repo, place) = started().await;
        commits(&place.dir, "src/lib.rs", "pub fn greet() {}\n");
        let (program, marked) = (repo.data.join("external"), repo.data.join("external-ran"));
        marking(&program, &marked);
        let program = program.display().to_string();
        run(&repo.project.repo, &["config", "diff.external", &program]);
        let refused = found(&repo, &place, None).await;
        assert!(
            matches!(&refused, Err(why) if why.contains("diff.external")),
            "{refused:?}"
        );
        assert!(!marked.exists(), "the external diff was run");
    }
}
