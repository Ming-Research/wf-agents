//! What the GitHub host does around a work's pull request, its checks and its end: the pull request is a draft whose
//! title and body say what the work wrote of itself and follow it, made ready as it merges; a check red on main as
//! well holds no work, and one that failed before on the same commit is run again and told apart where it then
//! passes; main red stops merges; and a work that ended without merging has its pull request closed.

use super::*;

/// A check of a GitHub Actions job, at its own link.
fn job(name: &str, id: u64, outcome: Outcome) -> Check {
    Check {
        name: name.to_string(),
        outcome,
        url: Some(format!("https://github.example/actions/runs/1/job/{id}")),
        summary: None,
        job: Some(id),
    }
}

fn red() -> Outcome {
    Outcome::Failed("failure".into())
}

/// The work's branch, saying this of itself.
fn about<'a>(place: &'a Place, about: &'a str) -> Branch<'a> {
    Branch { about, ..branch(place) }
}

/// The head of main on GitHub.
fn main_of(hub: &Hub) -> String {
    run(&hub.github().bare, &["rev-parse", "main"])
}

#[tokio::test]
async fn the_pull_request_is_opened_a_draft_saying_what_the_work_wrote_and_follows_it() {
    let hub = Hub::new();
    let head = hub.committed().await;
    hub.github().checks(&head, vec![vec![passed("lint"), passed("test")]]);
    hub.verdict(&head).await.unwrap().unwrap();
    let pulls = hub.github().pull_requests();
    assert_eq!(pulls.len(), 1);
    assert!(pulls[0].draft, "it is opened as a draft");
    assert_eq!(pulls[0].title, "Greet the user");
    assert!(
        pulls[0]
            .body
            .starts_with("Greets the user by name.\n\n---\n\nThis pull request carries work 1"),
        "{}",
        pulls[0].body
    );
    let edits = |hub: &Hub| {
        (hub.github().asked().into_iter())
            .filter(|a| matches!(a, Asked::Edit { .. }))
            .count()
    };
    // Checked again with nothing new said, nothing is edited.
    hub.verdict(&head).await.unwrap().unwrap();
    assert_eq!(edits(&hub), 0);
    // The work wrote of itself anew: the pull request says it.
    let place = hub.place();
    let branch = about(&place, "Greets the user by name, and by title where one is known.");
    hub.host
        .verdict(&branch, &head, None, &mut Stop::never())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(edits(&hub), 1);
    let body = &hub.github().pull_requests()[0].body;
    assert!(body.starts_with("Greets the user by name, and by title"), "{body}");
}

#[tokio::test]
async fn the_draft_is_made_ready_as_it_merges() {
    let hub = Hub::new();
    let checked = hub.committed().await;
    hub.github()
        .checks(&checked, vec![vec![passed("lint"), passed("test")]]);
    hub.verdict(&checked).await.unwrap().unwrap();
    hub.merge(&checked).await.expect("it merges");
    let asked: Vec<Asked> = (hub.github().asked().into_iter())
        .filter(|a| matches!(a, Asked::Ready(_) | Asked::Merge { .. }))
        .collect();
    assert!(
        matches!(asked.as_slice(), [Asked::Ready(_), Asked::Merge { .. }]),
        "{asked:?}"
    );
    assert!(!hub.github().pull_requests()[0].draft);
}

#[tokio::test]
async fn a_check_red_on_main_as_well_holds_no_work_and_is_said() {
    let hub = Hub::new();
    let head = hub.committed().await;
    let main = main_of(&hub);
    hub.github()
        .checks(&head, vec![vec![passed("lint"), job("test", 7, red())]]);
    hub.github()
        .checks(&main, vec![vec![passed("lint"), job("test", 3, red())]]);
    let verdict = hub.verdict(&head).await.unwrap().unwrap();
    assert_eq!(verdict.failed, None, "{verdict:?}");
    assert_eq!(verdict.passed_over.len(), 1, "{verdict:?}");
    let said = &verdict.passed_over[0];
    assert!(
        said.starts_with("test failed https://github.example/actions/runs/1/job/7") && said.contains(&main),
        "{said}"
    );
    assert!(said.contains("not the change's"), "{said}");
    // A check red on the commit alone holds it.
    let hub = Hub::new();
    let head = hub.committed().await;
    hub.github()
        .checks(&head, vec![vec![passed("lint"), job("test", 7, red())]]);
    hub.github()
        .checks(&main_of(&hub), vec![vec![passed("lint"), passed("test")]]);
    let verdict = hub.verdict(&head).await.unwrap().unwrap();
    assert!(verdict.failed.is_some_and(|f| f.contains("test (failure)")));
}

#[tokio::test]
async fn a_failure_seen_before_on_the_same_commit_is_run_again_and_passing_then_is_told_apart() {
    let hub = Hub::new();
    let head = hub.committed().await;
    hub.github()
        .checks(&head, vec![vec![passed("lint"), job("test", 7, red())]]);
    let first = hub.verdict(&head).await.unwrap().unwrap();
    let seen = first.failed.expect("it fails");
    assert!(
        !hub.github().asked().iter().any(|a| matches!(a, Asked::Rerun(_))),
        "a failure read the first time is not run again"
    );
    // Checked again with nothing changed, the same failure is run again; its new run passes.
    hub.github()
        .rerun_gives(7, &head, vec![vec![passed("lint"), job("test", 8, Outcome::Passed)]]);
    let place = hub.place();
    let again = (hub
        .host
        .verdict(&branch(&place), &head, Some(&seen), &mut Stop::never())
        .await)
        .unwrap()
        .unwrap();
    assert_eq!(again.failed, None, "{again:?}");
    assert!(hub.github().asked().contains(&Asked::Rerun(7)));
    assert_eq!(
        again.passed_over,
        [super::super::super::now_and_then(
            "test",
            " https://github.example/actions/runs/1/job/7",
            " https://github.example/actions/runs/1/job/8"
        )]
    );
}

#[tokio::test]
async fn a_failure_seen_before_that_fails_again_when_run_again_holds_the_work_saying_so() {
    let hub = Hub::new();
    let head = hub.committed().await;
    hub.github()
        .checks(&head, vec![vec![passed("lint"), job("test", 7, red())]]);
    let seen = hub.verdict(&head).await.unwrap().unwrap().failed.unwrap();
    // The run asked for is not there at once: the red one it runs again is not taken for it.
    hub.github().rerun_gives(
        7,
        &head,
        vec![
            vec![passed("lint"), job("test", 7, red())],
            vec![passed("lint"), job("test", 8, Outcome::Pending)],
            vec![passed("lint"), job("test", 8, red())],
        ],
    );
    let place = hub.place();
    let again = (hub
        .host
        .verdict(&branch(&place), &head, Some(&seen), &mut Stop::never())
        .await)
        .unwrap()
        .unwrap();
    let failed = again.failed.expect("it fails again");
    assert!(failed.contains("job/8"), "the new run is told: {failed}");
    assert!(failed.contains("failed again when run again"), "{failed}");
    assert!(again.passed_over.is_empty());
    // A failure seen on another commit is not run again.
    let hub = Hub::new();
    let head = hub.committed().await;
    hub.github()
        .checks(&head, vec![vec![passed("lint"), job("test", 9, red())]]);
    let place = hub.place();
    hub.host
        .verdict(&branch(&place), &head, Some(&seen), &mut Stop::never())
        .await
        .unwrap()
        .unwrap();
    assert!(!hub.github().asked().iter().any(|a| matches!(a, Asked::Rerun(_))));
}

#[tokio::test]
async fn an_agents_check_judges_what_failed_as_the_check_before_the_merge_does() {
    let hub = Hub::new();
    let head = hub.committed().await;
    let main = main_of(&hub);
    hub.github()
        .checks(&head, vec![vec![job("lint", 5, red()), job("test", 7, red())]]);
    hub.github()
        .checks(&main, vec![vec![job("lint", 2, red()), passed("test")]]);
    let place = hub.place();
    let first = (hub.host.check(&branch(&place), None, &mut Stop::never()).await)
        .unwrap()
        .unwrap();
    assert!(
        first.contains("lint failed") && first.contains("not the change's"),
        "{first}"
    );
    assert!(first.contains("test (failure)"), "{first}");
    // Asked again with nothing changed, the agent's own answer names the red run: it is run again.
    hub.github().rerun_gives(
        7,
        &head,
        vec![vec![job("lint", 5, red()), job("test", 8, Outcome::Passed)]],
    );
    let again = (hub.host.check(&branch(&place), Some(&first), &mut Stop::never()).await)
        .unwrap()
        .unwrap();
    assert!(hub.github().asked().contains(&Asked::Rerun(7)));
    assert!(
        again.starts_with(&format!("The CI passed on the commit {head}")),
        "{again}"
    );
    assert!(again.contains("fails now and then"), "{again}");
}

#[tokio::test]
async fn no_work_merges_onto_a_main_whose_ci_is_red() {
    let hub = Hub::new();
    let checked = hub.committed().await;
    hub.github()
        .checks(&checked, vec![vec![passed("lint"), passed("test")]]);
    hub.verdict(&checked).await.unwrap().unwrap();
    let main = main_of(&hub);
    // A named check red on main stops the merge though another is not done; one that is not named counts for nothing.
    let checks = vec![
        check("lint", Outcome::Pending),
        job("test", 3, red()),
        job("coverage", 2, red()),
    ];
    hub.github().checks(&main, vec![checks]);
    let refused = hub.merge(&checked).await.expect_err("refused");
    assert!(
        refused.starts_with(&format!(
            "main is red at {main}: test https://github.example/actions/runs/1/job/3. "
        )),
        "{refused}"
    );
    assert!(refused.contains("once it is green again, let the work on"), "{refused}");
    assert!(!hub.github().asked().iter().any(|a| matches!(a, Asked::Merge { .. })));
    // Green again, it merges.
    hub.github().checks(&main, vec![vec![passed("lint"), passed("test")]]);
    hub.merge(&checked).await.expect("it merges");
}

#[tokio::test]
async fn a_work_that_ended_without_merging_has_its_pull_request_closed_saying_so_once() {
    let hub = Hub::new();
    let head = hub.committed().await;
    hub.github().checks(&head, vec![vec![passed("lint"), passed("test")]]);
    hub.verdict(&head).await.unwrap().unwrap();
    let place = hub.place();
    hub.host.end(&branch(&place), false).await.expect("it closes");
    assert_eq!(hub.github().pull_requests()[0].state, State::Closed);
    // Given again, as after the engine started again, there is nothing open to close.
    hub.host.end(&branch(&place), false).await.expect("nothing to close");
    let closed: Vec<Asked> = (hub.github().asked().into_iter())
        .filter(|a| matches!(a, Asked::Close { .. }))
        .collect();
    assert_eq!(
        closed,
        [Asked::Close {
            number: 1,
            comment: "Its work ended without merging.".to_string()
        }]
    );
    assert_eq!(
        run(&hub.github().bare, &["rev-parse", &place.reference()]),
        head,
        "the branch stays"
    );
}

#[tokio::test]
async fn a_pull_request_merged_or_none_at_all_is_not_closed() {
    let hub = Hub::new();
    let place = hub.place();
    hub.committed().await;
    hub.host.end(&branch(&place), false).await.expect("nothing to close");
    let checked = hub.pushed_and_checked().await;
    hub.github().merged_by_a_person(1, &checked);
    hub.host.end(&branch(&place), false).await.expect("nothing to close");
    assert!(!hub.github().asked().iter().any(|a| matches!(a, Asked::Close { .. })));
    assert_eq!(hub.github().pull_requests()[0].state, State::Merged);
}
