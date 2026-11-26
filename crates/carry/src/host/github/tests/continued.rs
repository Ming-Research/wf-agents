//! Where a work that continues a branch or a pull request starts, and the pull request it takes the place of: the
//! branch as the local repository and GitHub hold it, a pull request's head, and what is closed once the work's own
//! pull request is open.

use std::path::PathBuf;

use super::*;

/// A branch of this name in the local repository, a commit past main; main stays checked out. Gives its commit.
fn local_branch(hub: &Hub, name: &str, file: &str) -> String {
    let dir = &hub.repo.project.repo;
    run(dir, &["switch", "-q", "-c", name]);
    let made = commit(dir, file, "Written.\n");
    run(dir, &["switch", "-q", "main"]);
    made
}

/// Another commit on the local repository's branch. Gives it.
fn advance_local(hub: &Hub, name: &str, file: &str) -> String {
    let dir = &hub.repo.project.repo;
    run(dir, &["switch", "-q", name]);
    let made = commit(dir, file, "More.\n");
    run(dir, &["switch", "-q", "main"]);
    made
}

/// Pushes the local repository's branch to GitHub.
fn push(hub: &Hub, name: &str) {
    let bare = hub.github().bare.display().to_string();
    run(&hub.repo.project.repo, &["push", "-q", &bare, name]);
}

/// Clones GitHub's repository for someone else, beside the local one, and gives where.
fn clone_of_github(hub: &Hub, dir: &str) -> PathBuf {
    let bare = hub.github().bare.display().to_string();
    run(&hub.repo.data, &["clone", "-q", &bare, dir]);
    let other = hub.repo.data.join(dir);
    run(&other, &["config", "user.name", "Someone"]);
    run(&other, &["config", "user.email", "someone@example.com"]);
    other
}

/// A branch of this name that only GitHub holds, a commit past main, made by someone else. Gives its commit.
fn github_branch(hub: &Hub, name: &str, file: &str) -> String {
    let other = clone_of_github(hub, &format!("made-{name}").replace('/', "-"));
    run(&other, &["switch", "-q", "-c", name]);
    let made = commit(&other, file, "Theirs.\n");
    run(&other, &["push", "-q", "origin", name]);
    made
}

/// Another commit on GitHub's branch, made by someone else. Gives it.
fn advance_on_github(hub: &Hub, name: &str, file: &str) -> String {
    let other = clone_of_github(hub, &format!("moved-{name}").replace('/', "-"));
    run(&other, &["switch", "-q", name]);
    let made = commit(&other, file, "Theirs again.\n");
    run(&other, &["push", "-q", "origin", name]);
    made
}

fn branch_named(name: &str) -> Continues {
    Continues::Branch(name.to_string())
}

#[tokio::test]
async fn a_branch_only_the_local_repository_holds_is_started_from_its_local_head() {
    let hub = Hub::new();
    let local = local_branch(&hub, "old", "old.txt");
    assert_eq!(hub.host.continued(&branch_named("old")).await, Ok(local));
}

#[tokio::test]
async fn a_branch_only_github_holds_is_fetched_and_started_from_its_head() {
    let hub = Hub::new();
    let theirs = github_branch(&hub, "old", "old.txt");
    let found = hub.host.continued(&branch_named("old")).await;
    assert_eq!(found, Ok(theirs.clone()));
    assert_eq!(
        run(&hub.repo.project.repo, &["rev-parse", &format!("{theirs}^{{commit}}")]),
        theirs,
        "the commit is in the local repository"
    );
}

#[tokio::test]
async fn a_branch_both_hold_at_one_commit_is_started_from_it() {
    let hub = Hub::new();
    let local = local_branch(&hub, "old", "old.txt");
    push(&hub, "old");
    assert_eq!(hub.host.continued(&branch_named("old")).await, Ok(local));
}

#[tokio::test]
async fn a_branch_the_local_repository_holds_further_on_is_started_from_the_local_head() {
    // The engine pushes a work's branch only to have it checked: what was committed after is only here.
    let hub = Hub::new();
    local_branch(&hub, "old", "old.txt");
    push(&hub, "old");
    let ahead = advance_local(&hub, "old", "more.txt");
    assert_eq!(hub.host.continued(&branch_named("old")).await, Ok(ahead));
}

#[tokio::test]
async fn a_branch_github_holds_further_on_is_started_from_githubs_head() {
    let hub = Hub::new();
    local_branch(&hub, "old", "old.txt");
    push(&hub, "old");
    let ahead = advance_on_github(&hub, "old", "more.txt");
    assert_eq!(hub.host.continued(&branch_named("old")).await, Ok(ahead));
}

#[tokio::test]
async fn a_branch_that_went_two_ways_is_refused_naming_both_heads() {
    let hub = Hub::new();
    local_branch(&hub, "old", "old.txt");
    push(&hub, "old");
    let here = advance_local(&hub, "old", "here.txt");
    let there = advance_on_github(&hub, "old", "there.txt");
    let refused = hub.host.continued(&branch_named("old")).await.expect_err("refused");
    assert_eq!(refused.kind, Some(Failure::Other));
    assert!(
        refused.why.contains(&here) && refused.why.contains(&there),
        "{}",
        refused.why
    );
    assert!(refused.why.contains("gone two ways"), "{}", refused.why);
}

#[tokio::test]
async fn a_branch_neither_side_holds_is_refused_naming_it() {
    let hub = Hub::new();
    let refused = hub.host.continued(&branch_named("nowhere")).await.expect_err("refused");
    assert_eq!(refused.kind, Some(Failure::Other));
    assert!(refused.why.contains("nowhere"), "{}", refused.why);
}

#[tokio::test]
async fn a_failure_to_reach_github_is_no_refusal_and_is_told_apart_from_a_branch_it_has_not() {
    let hub = Hub::new().with(|config| config.url = "/no/such/repository.git".to_string());
    local_branch(&hub, "old", "old.txt");
    let failed = hub.host.continued(&branch_named("old")).await.expect_err("it fails");
    assert_eq!(
        failed.kind, None,
        "its kind is read from its words, as main's is: {}",
        failed.why
    );
    assert!(
        failed.why.contains("could not be looked up on GitHub"),
        "{}",
        failed.why
    );
}

#[tokio::test]
async fn a_pull_request_of_a_branch_the_repository_has_is_started_from_that_branchs_head() {
    let hub = Hub::new();
    let theirs = github_branch(&hub, "old", "old.txt");
    let number = hub.github().pull_of("old", "main", State::Closed, false);
    assert_eq!(hub.host.continued(&Continues::Pull(number)).await, Ok(theirs));
}

#[tokio::test]
async fn a_pull_request_whose_branch_is_gone_is_started_from_the_head_github_keeps_of_it() {
    let hub = Hub::new();
    let theirs = github_branch(&hub, "old", "old.txt");
    let number = hub.github().pull_of("old", "main", State::Merged, false);
    let bare = hub.github().bare.clone();
    run(&bare, &["update-ref", &format!("refs/pull/{number}/head"), &theirs]);
    run(&bare, &["update-ref", "-d", "refs/heads/old"]);
    assert_eq!(hub.host.continued(&Continues::Pull(number)).await, Ok(theirs));
}

#[tokio::test]
async fn a_pull_request_from_a_fork_is_started_from_the_head_github_keeps_of_it() {
    let hub = Hub::new();
    let branch_head = github_branch(&hub, "fork-branch", "branch.txt");
    let kept = github_branch(&hub, "elsewhere", "kept.txt");
    assert_ne!(branch_head, kept);
    let number = hub.github().pull_of("fork-branch", "main", State::Open, true);
    run(
        &hub.github().bare,
        &["update-ref", &format!("refs/pull/{number}/head"), &kept],
    );
    assert_eq!(hub.host.continued(&Continues::Pull(number)).await, Ok(kept));
}

#[tokio::test]
async fn a_pull_request_github_has_not_is_refused_naming_it() {
    let hub = Hub::new();
    let refused = hub.host.continued(&Continues::Pull(9)).await.expect_err("refused");
    assert_eq!(refused.kind, Some(Failure::Other));
    assert!(refused.why.contains("no pull request 9"), "{}", refused.why);
}

/// The check of the work's head, given the continued branch or pull request, which has the work's own pull request open.
async fn checked_continuing(hub: &Hub, head: &str, continues: &Continues) {
    hub.green(head);
    let place = hub.place();
    let branch = Branch {
        continues: Some(continues),
        ..branch(&place)
    };
    let main = hub.main_on_github();
    hub.host
        .verdict(&branch, head, &main, hub.round(), &Mutex::new(()), &mut Stop::never())
        .await
        .expect("the check runs")
        .expect("nothing stopped it");
}

#[tokio::test]
async fn the_open_pull_request_of_the_continued_branch_is_closed_naming_the_works_own_once() {
    let hub = Hub::new();
    github_branch(&hub, "old", "old.txt");
    let old = hub.github().pull_of("old", "main", State::Open, false);
    let head = hub.committed().await;
    checked_continuing(&hub, &head, &branch_named("old")).await;
    let own = hub.github().pull_requests().len() as u64;
    assert_eq!(own, old + 1, "the work's own pull request was opened");
    let url = format!("https://github.example/pull/{own}");
    let said = fill(&hub.host.config.words.superseded, &[("url", &url)]);
    assert_eq!(hub.github().closed_with(), [(old, said)]);
    // Given again, the old one is closed and nothing is closed twice.
    checked_continuing(&hub, &head, &branch_named("old")).await;
    assert_eq!(hub.github().closed_with().len(), 1);
}

// What the pull request that was continued is closed with is the words of the file, and nothing else.
#[tokio::test]
async fn the_comment_the_continued_pull_request_is_closed_with_is_the_words_of_its_file_and_nothing_else() {
    let mut hub = Hub::new();
    hub.config_mut().words.superseded = "SUPERSEDED BY {url}.".to_string();
    github_branch(&hub, "old", "old.txt");
    let old = hub.github().pull_of("old", "main", State::Open, false);
    let head = hub.committed().await;
    checked_continuing(&hub, &head, &branch_named("old")).await;
    let own = hub.github().pull_requests().len() as u64;
    assert_eq!(
        hub.github().closed_with(),
        [(old, format!("SUPERSEDED BY https://github.example/pull/{own}."))]
    );
}

#[tokio::test]
async fn the_pull_request_an_item_continues_by_number_is_closed_as_well() {
    let hub = Hub::new();
    github_branch(&hub, "old", "old.txt");
    let old = hub.github().pull_of("old", "main", State::Open, false);
    let head = hub.committed().await;
    checked_continuing(&hub, &head, &Continues::Pull(old)).await;
    let closed: Vec<u64> = hub
        .github()
        .closed_with()
        .into_iter()
        .map(|(number, _)| number)
        .collect();
    assert_eq!(closed, [old]);
}

#[tokio::test]
async fn a_pull_request_that_is_not_open_into_main_or_is_a_works_own_is_left_as_it_is() {
    let hub = Hub::new();
    github_branch(&hub, "old", "old.txt");
    hub.github().pull_of("old", "main", State::Closed, false);
    hub.github().pull_of("old", "main", State::Merged, false);
    hub.github().pull_of("old", "next", State::Open, false);
    let theirs = hub.github().pull_of("work-7-0a1b2c3d", "main", State::Open, false);
    let head = hub.committed().await;
    checked_continuing(&hub, &head, &branch_named("old")).await;
    checked_continuing(&hub, &head, &Continues::Pull(theirs)).await;
    assert_eq!(hub.github().closed_with(), []);
}

#[tokio::test]
async fn a_work_that_continues_nothing_asks_github_of_no_other_pull_request() {
    let hub = Hub::new();
    let head = hub.committed().await;
    hub.green(&head);
    hub.verdict(&head).await.unwrap().unwrap();
    let asked: Vec<Asked> = (hub.github().asked().into_iter())
        .filter(|asked| matches!(asked, Asked::Pull(_) | Asked::Close { .. }))
        .collect();
    assert_eq!(asked, []);
}
