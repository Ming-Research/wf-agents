//! The commit of the code before a work's change with some files of its last commit put on it.

use super::*;

/// A started work whose last commit changes the greeting and adds its test, giving the repository, main's head and the
/// commit.
async fn changed() -> (Repo, String, String) {
    let repo = Repo::new("true");
    let main = run(&repo.project.repo, &["rev-parse", "main"]);
    let started = start(&repo.project, &repo.place(WORK), "main").await.unwrap();
    std::fs::write(started.place.dir.join("greeting.txt"), "Bye.\n").unwrap();
    run(&started.place.dir, &["add", "greeting.txt"]);
    let head = commit(&started.place.dir, "test.sh", "grep Bye greeting.txt\n");
    (repo, main, head)
}

fn files(names: &[&str]) -> Vec<String> {
    names.iter().map(|n| n.to_string()).collect()
}

#[tokio::test]
async fn the_code_before_the_change_is_main_with_the_files_put_on_it_as_the_last_commit_holds_them() {
    let (repo, main, head) = changed().await;
    let place = repo.place(WORK);
    let made = before(&repo.project, &place, "refs/heads/main", &files(&["test.sh"])).await;
    let before = made.unwrap().expect("made");
    assert_eq!(before.head, head);
    assert_eq!(before.base, main);
    assert_eq!(before.files, files(&["test.sh"]));
    let at = |path: &str| run(&repo.project.repo, &["show", &format!("{}:{path}", before.commit)]);
    assert_eq!(at("greeting.txt"), "Hello.", "the code is main's");
    assert_eq!(at("test.sh"), "grep Bye greeting.txt", "the test is the commit's");
    let parents = run(
        &repo.project.repo,
        &["rev-list", "--parents", "-n", "1", &before.commit],
    );
    assert_eq!(parents, format!("{} {main}", before.commit));
    assert_eq!(
        run(&place.dir, &["status", "--porcelain"]),
        "",
        "the worktree is let be"
    );
    // Made again of the same files, it is the same commit, whose CI is read rather than run again.
    let again = crate::git::before(&repo.project, &place, "refs/heads/main", &files(&["test.sh"])).await;
    assert_eq!(again.unwrap().unwrap().commit, before.commit);
}

#[tokio::test]
async fn a_file_the_last_commit_lacks_or_holds_otherwise_than_the_worktree_is_said() {
    let (repo, _, head) = changed().await;
    let place = repo.place(WORK);
    let said = before(
        &repo.project,
        &place,
        "refs/heads/main",
        &files(&["missing.sh", "test.sh"]),
    )
    .await;
    let said = said.unwrap().expect_err("not made");
    assert!(
        said.contains(&format!("missing.sh is not a file of your last commit {head}")),
        "{said}"
    );
    std::fs::write(place.dir.join("test.sh"), "grep Hello greeting.txt\n").unwrap();
    let said = before(&repo.project, &place, "refs/heads/main", &files(&["test.sh"])).await;
    let said = said.unwrap().expect_err("not made");
    assert!(said.contains("not committed") && said.contains("test.sh"), "{said}");
}

#[tokio::test]
async fn the_code_before_the_change_is_not_made_in_a_repository_whose_configuration_names_a_program() {
    let (repo, _, _) = changed().await;
    run(&repo.project.repo, &["config", "filter.crypt.smudge", "cat"]);
    let refused = before(
        &repo.project,
        &repo.place(WORK),
        "refs/heads/main",
        &files(&["test.sh"]),
    )
    .await;
    let refused = refused.expect_err("refused");
    assert!(refused.contains("filter.crypt.smudge"), "{refused}");
}
