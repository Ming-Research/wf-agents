//! The work's branch takes the latest main in as each run of a cell that changes the code starts, so that no agent
//! works on an old picture of main and a conflict with what other works merged is met while the work is being done,
//! not first at the check before its merge. Main is merged as the check before the merge merges it, as a commit of
//! the engine's; a conflict, found without changing anything, leaves the branch as it is and is given as the files in
//! conflict, for the agent to merge itself; a worktree with changes not committed is left as it is, since a merge
//! could lose them. What came of it is kept beside the run's rounds, once for the run, so that it is done once and
//! every round of the run that is given its task is told the same.

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

/// What taking main in did.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Outcome {
    /// The branch held main's head at this commit already: nothing was to be taken in.
    Holds { main: String },
    /// Main's head at `main` was merged into the branch as the commit `merge`; `files` are those main changed since
    /// the branch last took it in.
    TookIn {
        main: String,
        merge: String,
        files: Vec<String>,
    },
    /// Main's head at `main` does not merge into the branch without a conflict in these files: nothing was taken in.
    Conflict { main: String, files: Vec<String> },
    /// The worktree holds these changes not committed, or a merge under way, which taking main's head at `main` in
    /// could lose: nothing was taken in.
    Uncommitted { main: String, changes: String },
    /// Main could not be taken in, for this reason.
    Failed { why: String },
}

/// Where what came of taking main in as the run started is kept.
pub(crate) fn kept_at(data: &Path, work: WorkNumber, run: RunId) -> PathBuf {
    data.join(ROUNDS)
        .join(work.0.to_string())
        .join(format!("{}.main.json", run.0))
}

/// Takes main's head, as the reference `main` holds it, into the work's branch in its worktree, and says what came of
/// it. Refused, as every act is, where the repository's configuration names a program git could run.
pub(crate) async fn take_in(project: &Project, place: &Place, main: &str) -> CaughtUp {
    let outcome = match tried(project, place, main).await {
        Ok(outcome) => outcome,
        Err(why) => Outcome::Failed { why },
    };
    CaughtUp {
        name: project.main.clone(),
        outcome,
    }
}

async fn tried(project: &Project, place: &Place, main: &str) -> Result<Outcome, String> {
    let dir = git::worktree(place)?;
    git::configuration(project, place).await?;
    git::on_branch(place).await?;
    let repo = &project.repo;
    let main = git::git(repo, &["rev-parse", "--verify", main]).await?;
    let changes = git::uncommitted(&dir).await?;
    if git::merging(&dir).await? || !changes.is_empty() {
        return Ok(Outcome::Uncommitted { main, changes });
    }
    let head = git::git(repo, &["rev-parse", "--verify", &place.reference()]).await?;
    if git::said(repo, &["merge-base", "--is-ancestor", &main, &head])
        .await?
        .ok
    {
        return Ok(Outcome::Holds { main });
    }
    if let Some(files) = git::conflicts(project, &main, &head).await? {
        let files = files.lines().map(str::to_string).collect();
        return Ok(Outcome::Conflict { main, files });
    }
    let base = git::git(repo, &["merge-base", &head, &main]).await?;
    let changed = git::git(repo, &["diff", "--name-only", "--no-renames", &base, &main]).await?;
    let files = changed.lines().map(str::to_string).collect();
    let (merge, _) = git::take_in(&dir, &main, &project.main).await?;
    Ok(Outcome::TookIn { main, merge, files })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::tests::{Repo, commit, run};

    const WORK: WorkNumber = WorkNumber(1);

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
    async fn main_that_merges_cleanly_is_taken_in_with_the_files_it_changed() {
        let (repo, place, main) = moved("other.txt").await;
        let caught = take_in(&repo.project, &place, "refs/heads/main").await;
        assert_eq!(caught.name, "main");
        let Outcome::TookIn {
            main: took,
            merge,
            files,
        } = caught.outcome
        else {
            panic!("{caught:?}");
        };
        assert_eq!(
            (took.as_str(), files.as_slice()),
            (main.as_str(), &["other.txt".to_string()][..])
        );
        assert_eq!(
            run(&place.dir, &["rev-parse", "HEAD"]),
            merge,
            "the branch is at the merge"
        );
        let parents = run(&place.dir, &["rev-list", "--parents", "-n", "1", "HEAD"]);
        assert_eq!(parents.split(' ').nth(2), Some(main.as_str()), "it merges main's head");
        // Taken in again, main is held, and nothing more is merged.
        let again = take_in(&repo.project, &place, "refs/heads/main").await;
        assert_eq!(again.outcome, Outcome::Holds { main });
        assert_eq!(run(&place.dir, &["rev-parse", "HEAD"]), merge);
    }

    #[tokio::test]
    async fn main_in_conflict_is_given_as_its_files_and_the_branch_is_left_as_it_was() {
        let (repo, place, main) = moved("greeting.txt").await;
        let head = run(&place.dir, &["rev-parse", "HEAD"]);
        let caught = take_in(&repo.project, &place, "refs/heads/main").await;
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
        let caught = take_in(&repo.project, &place, "refs/heads/main").await;
        let Outcome::Uncommitted { main: at, changes } = caught.outcome else {
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
        let caught = take_in(&repo.project, &started.place, "refs/heads/main").await;
        assert_eq!(caught.outcome, Outcome::Holds { main });
    }

    #[tokio::test]
    async fn a_work_with_no_worktree_could_not_take_main_in_and_says_why() {
        let repo = Repo::new("true");
        let caught = take_in(&repo.project, &repo.place(WORK), "refs/heads/main").await;
        let Outcome::Failed { why } = caught.outcome else {
            panic!("{caught:?}");
        };
        assert!(why.contains("the work has no worktree"), "{why}");
    }
}
