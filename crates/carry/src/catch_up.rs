//! The work's branch takes the latest main in as each run of a cell that changes the code starts, so that no agent
//! works on an old picture of main and a conflict with what other works merged is met while the work is being done,
//! not first at the check before its merge. Main is merged as the check before the merge merges it, as a commit of
//! the engine's; a conflict, found without changing anything, leaves the branch as it is and is given as the files in
//! conflict, for the agent to merge itself; a worktree with changes to its files not committed, or a merge under way,
//! is left as it is, since a merge could lose them. The run's merge is made with words that name the run, so that the
//! check before the merge does not take it for its own, and a head that is this run's merge of this main, made before
//! the engine stopped, is what the run took in. What came of it is kept beside the run's rounds, once for the run, so
//! that it is done once and every round of the run that is given its task is told the same.

use std::path::{Path, PathBuf};

use decide::RunId;
use serde::{Deserialize, Serialize};
use store::WorkNumber;

use crate::agents::ROUNDS;
use crate::git::{self, Place};
use crate::settings::Project;

/// What came of taking main in as a run started.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaughtUp {
    /// The name of the main branch.
    pub name: String,
    pub outcome: Outcome,
}

impl CaughtUp {
    /// What the rounds of the run are told of it; nothing where the branch held main already.
    pub fn told(&self) -> Option<context::Main> {
        let name = self.name.clone();
        Some(match &self.outcome {
            Outcome::Holds { .. } => return None,
            Outcome::TookIn { main, merge } => context::Main::TookIn {
                name,
                main: main.clone(),
                merge: merge.clone(),
            },
            Outcome::Conflict { main, files } => context::Main::Conflict {
                name,
                main: main.clone(),
                files: files.clone(),
            },
            // What the worktree holds is told as the files of its changes, each line of git's status short of the
            // two letters of its state and the space after them.
            Outcome::Uncommitted { main, changes, merging } => context::Main::Uncommitted {
                name,
                main: main.clone(),
                files: (changes.lines())
                    .map(|line| line.get(3..).unwrap_or(line).trim().to_string())
                    .collect(),
                merging: *merging,
            },
            // Why is for whoever runs the engine.
            Outcome::Failed { .. } => context::Main::Failed { name },
        })
    }
}

/// What taking main in did.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Outcome {
    /// The branch held main's head at this commit already: nothing was to be taken in.
    Holds { main: String },
    /// Main's head at `main` was merged into the branch as the commit `merge`.
    TookIn { main: String, merge: String },
    /// Main's head at `main` does not merge into the branch without a conflict in these files: nothing was taken in.
    Conflict { main: String, files: Vec<String> },
    /// The worktree holds these changes to its files not committed, or a merge under way where `merging` says so,
    /// which taking main's head at `main` in could lose: nothing was taken in.
    Uncommitted {
        main: String,
        changes: String,
        #[serde(default)]
        merging: bool,
    },
    /// Main could not be taken in, for this reason.
    Failed { why: String },
}

/// Where what came of taking main in as the run started is kept.
pub(crate) fn kept_at(data: &Path, work: WorkNumber, run: RunId) -> PathBuf {
    data.join(ROUNDS)
        .join(work.0.to_string())
        .join(format!("{}.main.json", run.0))
}

/// The words a run's merge of main is made with, as the project's `commits` says.
fn taking_in(project: &Project, main: &str, run: RunId) -> String {
    let run = run.0.to_string();
    let slots = [("main", project.main.as_str()), ("head", main), ("run", &run)];
    flow::agents::fill(&project.commits.run, &slots)
}

/// Takes main's head, as the reference `main` holds it, into the work's branch in its worktree as the run starts, and
/// says what came of it. Refused, as every act is, where the repository's configuration names a program git could run.
pub(crate) async fn take_in(project: &Project, place: &Place, main: &str, run: RunId) -> CaughtUp {
    let outcome = match tried(project, place, main, run).await {
        Ok(outcome) => outcome,
        Err(why) => Outcome::Failed { why },
    };
    CaughtUp {
        name: project.main.clone(),
        outcome,
    }
}

async fn tried(project: &Project, place: &Place, main: &str, run: RunId) -> Result<Outcome, String> {
    let dir = git::worktree(place)?;
    git::configuration(project, place).await?;
    git::on_branch(place).await?;
    let repo = &project.repo;
    let main = git::git(repo, &["rev-parse", "--verify", main]).await?;
    // Files git does not track are left out: a build's output not ignored would otherwise hold every run.
    let changes = git::git(&dir, &["status", "--porcelain", "--untracked-files=no"]).await?;
    let merging = git::merging(&dir).await?;
    if merging || !changes.is_empty() {
        return Ok(Outcome::Uncommitted { main, changes, merging });
    }
    let message = taking_in(project, &main, run);
    let head = git::git(repo, &["rev-parse", "--verify", &place.reference()]).await?;
    if git::said(repo, &["merge-base", "--is-ancestor", &main, &head])
        .await?
        .ok
    {
        // This run's merge, made before the engine stopped and before what came of it was kept.
        if let Some((merge, _)) = git::merged_as(&dir, &main, &message, &project.commits).await? {
            return Ok(Outcome::TookIn { main, merge });
        }
        return Ok(Outcome::Holds { main });
    }
    if let Some(conflict) = git::conflicts(project, &main, &head).await? {
        let files = conflict.files.lines().map(str::to_string).collect();
        return Ok(Outcome::Conflict { main, files });
    }
    let merge = git::merge_as(&dir, &main, &message, &project.commits).await?;
    Ok(Outcome::TookIn { main, merge })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::tests::{Repo, commit, run};

    #[test]
    fn changes_not_committed_are_told_as_their_files_and_a_failure_without_why() {
        let caught = |outcome| CaughtUp {
            name: "main".to_string(),
            outcome,
        };
        let left = caught(Outcome::Uncommitted {
            main: "beef".to_string(),
            changes: " M a.rs\n?? b.rs".to_string(),
            merging: false,
        });
        let told = context::Main::Uncommitted {
            name: "main".to_string(),
            main: "beef".to_string(),
            files: vec!["a.rs".to_string(), "b.rs".to_string()],
            merging: false,
        };
        assert_eq!(left.told(), Some(told));
        let failed = caught(Outcome::Failed {
            why: "the network is down".to_string(),
        });
        let name = "main".to_string();
        assert_eq!(failed.told(), Some(context::Main::Failed { name }));
        assert_eq!(
            caught(Outcome::Holds {
                main: "beef".to_string()
            })
            .told(),
            None
        );
    }

    const WORK: WorkNumber = WorkNumber(1);
    const RUN: RunId = RunId(7);

    /// A started work whose last commit rewrote `greeting.txt`, with main then given a commit of `file`; gives the
    /// repository, the work's place and main's new head.
    async fn moved(file: &str) -> (Repo, Place, String) {
        let repo = Repo::new("true");
        let started = git::start(&repo.project, &repo.place(WORK), "main").await.unwrap();
        commit(&started.place.dir, "greeting.txt", "Hello, you.\n");
        let main = commit(&repo.project.repo, file, "Good day.\n");
        (repo, started.place, main)
    }

    #[tokio::test]
    async fn main_that_merges_cleanly_is_taken_in_and_what_it_changed_is_what_the_merge_shows() {
        let (repo, place, main) = moved("other.txt").await;
        let caught = take_in(&repo.project, &place, "refs/heads/main", RUN).await;
        assert_eq!(caught.name, "main");
        let Outcome::TookIn { main: took, merge } = caught.outcome else {
            panic!("{caught:?}");
        };
        assert_eq!(took, main);
        // What the merge changed is what the command the message gives shows.
        let changed = run(&place.dir, &["diff", "--name-only", &format!("{merge}^1"), &merge]);
        assert_eq!(changed, "other.txt");
        assert_eq!(
            run(&place.dir, &["rev-parse", "HEAD"]),
            merge,
            "the branch is at the merge"
        );
        let parents = run(&place.dir, &["rev-list", "--parents", "-n", "1", "HEAD"]);
        assert_eq!(parents.split(' ').nth(2), Some(main.as_str()), "it merges main's head");
        // Taken in again by a later run, main is held, and nothing more is merged.
        let again = take_in(&repo.project, &place, "refs/heads/main", RunId(8)).await;
        assert_eq!(again.outcome, Outcome::Holds { main });
        assert_eq!(run(&place.dir, &["rev-parse", "HEAD"]), merge);
    }

    #[tokio::test]
    async fn main_taken_in_by_a_branch_with_no_commit_of_its_own_is_a_merge_commit_and_not_a_move_forward() {
        let repo = Repo::new("true");
        let started = git::start(&repo.project, &repo.place(WORK), "main").await.unwrap();
        let main = commit(&repo.project.repo, "other.txt", "Good day.\n");
        let caught = take_in(&repo.project, &started.place, "refs/heads/main", RUN).await;
        let Outcome::TookIn { merge, .. } = caught.outcome else {
            panic!("{caught:?}");
        };
        let parents = run(&started.place.dir, &["rev-list", "--parents", "-n", "1", &merge]);
        assert_eq!(parents.split(' ').count(), 3, "a commit with two parents: {parents}");
        assert_ne!(merge, main, "not main's own commit");
    }

    #[tokio::test]
    async fn a_merge_this_run_made_before_the_engine_stopped_is_what_it_took_in() {
        let (repo, place, main) = moved("other.txt").await;
        // The engine stopped after the merge, before what came of it was kept.
        let merge = git::merge_as(
            &place.dir,
            &main,
            &taking_in(&repo.project, &main, RUN),
            &repo.project.commits,
        )
        .await
        .unwrap();
        let caught = take_in(&repo.project, &place, "refs/heads/main", RUN).await;
        let took = Outcome::TookIn {
            main: main.clone(),
            merge,
        };
        assert_eq!(caught.outcome, took);
        // Another run finds main held: the merge is not its own.
        let later = take_in(&repo.project, &place, "refs/heads/main", RunId(8)).await;
        assert_eq!(later.outcome, Outcome::Holds { main });
    }

    #[tokio::test]
    async fn the_check_before_the_merge_takes_nothing_in_after_a_run_took_main_in() {
        let (repo, place, _) = moved("other.txt").await;
        let caught = take_in(&repo.project, &place, "refs/heads/main", RUN).await;
        let Outcome::TookIn { merge, .. } = caught.outcome else {
            panic!("{caught:?}");
        };
        let host = crate::host::Local::new(repo.project.clone());
        let branch = crate::host::Branch {
            work: WORK,
            title: "Greet the user",
            about: "",
            place: &place,
        };
        let lock = tokio::sync::Mutex::new(());
        let mut stop = crate::held::Stop::never();
        let checked = crate::acts::check(
            &repo.project,
            &host,
            &branch,
            &lock,
            &mut stop,
            &crate::acts::words().check,
        )
        .await
        .expect("the work is checked")
        .expect("nothing stopped it");
        assert_eq!(checked.commit, merge, "the run's merge is what is judged");
        assert!(!checked.took_in, "the check merged nothing of its own");
    }

    #[tokio::test]
    async fn a_file_git_does_not_track_holds_nothing_back_and_a_merge_under_way_is_said() {
        let (repo, place, _) = moved("other.txt").await;
        std::fs::write(place.dir.join("build.out"), "made\n").unwrap();
        let caught = take_in(&repo.project, &place, "refs/heads/main", RUN).await;
        assert!(matches!(caught.outcome, Outcome::TookIn { .. }), "{caught:?}");
        let (repo, place, main) = moved("greeting.txt").await;
        let merging = std::process::Command::new("git")
            .arg("-C")
            .arg(&place.dir)
            .args(["merge", "--no-edit", &main])
            .output()
            .expect("git runs");
        assert!(!merging.status.success(), "the merge meets the conflict");
        let caught = take_in(&repo.project, &place, "refs/heads/main", RUN).await;
        let Outcome::Uncommitted { merging, .. } = caught.outcome else {
            panic!("{caught:?}");
        };
        assert!(merging, "a merge under way is said");
    }

    #[tokio::test]
    async fn main_in_conflict_is_given_as_its_files_and_the_branch_is_left_as_it_was() {
        let (repo, place, main) = moved("greeting.txt").await;
        let head = run(&place.dir, &["rev-parse", "HEAD"]);
        let caught = take_in(&repo.project, &place, "refs/heads/main", RUN).await;
        assert_eq!(
            caught.outcome,
            Outcome::Conflict {
                main,
                files: vec!["greeting.txt".to_string()]
            }
        );
        assert_eq!(run(&place.dir, &["rev-parse", "HEAD"]), head);
        assert_eq!(
            run(&place.dir, &["status", "--porcelain"]),
            "",
            "nothing is left in the worktree"
        );
    }

    #[tokio::test]
    async fn a_worktree_with_changes_not_committed_is_left_as_it_is_and_says_them() {
        let (repo, place, main) = moved("other.txt").await;
        let head = run(&place.dir, &["rev-parse", "HEAD"]);
        std::fs::write(place.dir.join("greeting.txt"), "Hello, all.\n").unwrap();
        let caught = take_in(&repo.project, &place, "refs/heads/main", RUN).await;
        let Outcome::Uncommitted {
            main: at,
            changes,
            merging: false,
        } = caught.outcome
        else {
            panic!("{caught:?}");
        };
        assert_eq!(at, main);
        assert!(changes.contains("greeting.txt"), "{changes}");
        assert_eq!(run(&place.dir, &["rev-parse", "HEAD"]), head);
        let text = std::fs::read_to_string(place.dir.join("greeting.txt")).unwrap();
        assert_eq!(text, "Hello, all.\n", "the change is kept");
    }

    #[tokio::test]
    async fn a_branch_that_holds_main_takes_nothing_in() {
        let repo = Repo::new("true");
        let started = git::start(&repo.project, &repo.place(WORK), "main").await.unwrap();
        commit(&started.place.dir, "greeting.txt", "Hello, you.\n");
        let main = run(&repo.project.repo, &["rev-parse", "main"]);
        let caught = take_in(&repo.project, &started.place, "refs/heads/main", RUN).await;
        assert_eq!(caught.outcome, Outcome::Holds { main });
    }

    #[tokio::test]
    async fn a_work_with_no_worktree_could_not_take_main_in_and_says_why() {
        let repo = Repo::new("true");
        let caught = take_in(&repo.project, &repo.place(WORK), "refs/heads/main", RUN).await;
        let Outcome::Failed { why } = caught.outcome else {
            panic!("{caught:?}");
        };
        assert!(why.contains("the work has no worktree"), "{why}");
    }
}
