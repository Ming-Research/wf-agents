//! What the agents' tool `check` answers at once, pushing nothing and waiting for no verdict, where the last commit of
//! the work's branch does not merge into main's head as the host last gave it: GitHub starts no CI for a pull request
//! in conflict, so a check pushed then would wait for a CI that never starts, and the work could not merge anyway.

use crate::git::{self, Place};
use crate::settings::Project;

/// What the agent is told where the work's branch, at its last commit, does not merge into main's head at `main`, the
/// reference that holds it as the host last gave it; none where it merges cleanly, or where what stands in the way of
/// looking is for the host's check to say: the work has no worktree, or a merge is under way in it. Refused, as every
/// act is, where the repository's configuration names a program git could run.
pub(super) async fn found(project: &Project, place: &Place, main: &str) -> Result<Option<String>, String> {
    let Ok(dir) = git::worktree(place) else {
        return Ok(None);
    };
    if git::merging(&dir).await? {
        return Ok(None);
    }
    git::configuration(project, place).await?;
    let repo = &project.repo;
    let head = git::git(repo, &["rev-parse", "--verify", &place.reference()]).await?;
    let main = git::git(repo, &["rev-parse", "--verify", main]).await?;
    let Some(files) = git::conflicts(project, &main, &head).await? else {
        return Ok(None);
    };
    let name = &project.main;
    Ok(Some(format!(
        "The work's branch does not merge into {name} at {main} without a conflict, so nothing was checked: a pull \
         request in conflict gets no CI, and the work could not merge. The files in conflict:\n{files}\nTake {name} in \
         with `git merge {main}`, resolve the conflicts, commit the merge, and ask for the check again."
    )))
}

#[cfg(test)]
mod tests {
    use store::WorkNumber;

    use super::super::{Answer, CHECK, answer};
    use crate::git::tests::{Repo, commit, run};
    use crate::held::Stop;
    use crate::host::Branch;
    use crate::host::fake::{Asked, Fake, NO_TOOLS};

    const WORK: WorkNumber = WorkNumber(1);

    /// A started work whose last commit rewrote the line of `greeting.txt`, with main given a commit of `file` after
    /// it started; gives the repository and main's new head.
    async fn moved(file: &str) -> (Repo, String) {
        let repo = Repo::new("true");
        let started = crate::git::start(&repo.project, &repo.place(WORK), "main")
            .await
            .unwrap();
        commit(&started.place.dir, "greeting.txt", "Hello, you.\n");
        let main = commit(&repo.project.repo, file, "Good day.\n");
        (repo, main)
    }

    /// The check's answer on the fake host, with what the host was asked.
    async fn checked(repo: &Repo) -> (String, Vec<Asked>) {
        checked_with(repo, "").await
    }

    /// The check's answer to this input on the fake host, with what the host was asked.
    async fn checked_with(repo: &Repo, input: &str) -> (String, Vec<Asked>) {
        let host = Fake::new("unused");
        let place = repo.place(WORK);
        let branch = Branch {
            work: WORK,
            title: "Greet the user",
            about: "",
            place: &place,
        };
        match answer(&host, &repo.project, &branch, CHECK, input, &mut Stop::never()).await {
            Answer::Result(said) => (said, host.asked()),
            Answer::Withdrawn => panic!("nothing withdrew it"),
        }
    }

    #[tokio::test]
    async fn a_branch_in_conflict_with_main_is_told_its_files_and_nothing_is_checked() {
        let (repo, main) = moved("greeting.txt").await;
        let (said, asked) = checked(&repo).await;
        assert!(
            said.starts_with(&format!(
                "The work's branch does not merge into main at {main} without a conflict"
            )),
            "{said}"
        );
        assert!(said.contains("\ngreeting.txt\n"), "{said}");
        assert!(said.contains(&format!("git merge {main}")), "{said}");
        assert!(asked.is_empty(), "no check was asked of the host: {asked:?}");
    }

    #[tokio::test]
    async fn a_branch_in_conflict_asks_for_no_fast_run_of_the_tests_it_names() {
        let (repo, main) = moved("greeting.txt").await;
        let (said, asked) = checked_with(&repo, "test: greets the user\ncheck: lint").await;
        assert!(
            said.starts_with(&format!(
                "The work's branch does not merge into main at {main} without a conflict"
            )),
            "{said}"
        );
        assert!(asked.is_empty(), "no run of the named tests was asked: {asked:?}");
    }

    #[tokio::test]
    async fn a_branch_that_merges_cleanly_is_checked_by_the_host_as_before() {
        let (repo, _) = moved("other.txt").await;
        let (said, asked) = checked(&repo).await;
        assert_eq!(said, format!("The check could not be run: {NO_TOOLS}"));
        assert_eq!(asked, [Asked::Check]);
    }

    #[tokio::test]
    async fn a_merge_under_way_in_the_worktree_is_left_for_the_hosts_check_to_say() {
        let (repo, main) = moved("greeting.txt").await;
        let dir = repo.place(WORK).dir;
        // The agent took main in and is resolving the conflict.
        let merging = std::process::Command::new("git")
            .arg("-C")
            .arg(&dir)
            .args(["merge", "--no-edit", &main])
            .output()
            .expect("git runs");
        assert!(!merging.status.success(), "the merge meets the conflict");
        run(&dir, &["rev-parse", "--verify", "MERGE_HEAD"]);
        let (_, asked) = checked(&repo).await;
        assert_eq!(asked, [Asked::Check]);
    }
}
