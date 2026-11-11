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
        earlier: Vec::new(),
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
    let body = &pulls[0].body;
    assert!(body.contains("\n```text\nGreets the user by name.\n```\n"), "{body}");
    assert!(body.contains("\n---\n\nThis pull request carries work 1"), "{body}");
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
    assert!(
        body.contains("```text\nGreets the user by name, and by title"),
        "{body}"
    );
}

#[tokio::test]
async fn what_the_work_wrote_is_published_with_no_mention_reference_closing_word_or_path_of_this_machine() {
    let hub = Hub::new();
    let head = hub.committed().await;
    hub.github().checks(&head, vec![vec![passed("lint"), passed("test")]]);
    let data = hub.repo.data.display().to_string();
    let repo = hub.repo.project.repo.display().to_string();
    let wrote = format!(
        "@octocat fixes #12, Closes https://github.com/o/r/issues/3, resolved o/r#4.\nSee {data}/worktrees/1-x and \
         {repo}/src.\n```\nA fence of its own.\n```"
    );
    let place = hub.place();
    hub.host
        .verdict(&about(&place, &wrote), &head, None, &mut Stop::never())
        .await
        .unwrap()
        .unwrap();
    let body = &hub.github().pull_requests()[0].body;
    let z = '\u{200B}';
    for kept_from_acting in [
        format!("@{z}octocat"),
        format!("f{z}ixes #{z}12"),
        format!("C{z}loses https://github.com/o/r/issues/3"),
        format!("r{z}esolved o/r#{z}4"),
    ] {
        assert!(body.contains(&kept_from_acting), "{kept_from_acting:?} in {body}");
    }
    assert!(
        !body.contains(&data) && !body.contains(&repo),
        "a path of this machine: {body}"
    );
    assert!(
        body.contains("<the data directory>/worktrees/1-x and <the repository>/src."),
        "{body}"
    );
    // Its own fence does not end the block it is published in.
    assert!(body.contains("\n````text\n") && body.contains("\n````\n"), "{body}");
    // A long text is cut, saying so.
    let long = "word ".repeat(10_000);
    hub.host
        .verdict(&about(&place, &long), &head, None, &mut Stop::never())
        .await
        .unwrap()
        .unwrap();
    let body = &hub.github().pull_requests()[0].body;
    assert!(body.chars().count() < 25_000, "{} characters", body.chars().count());
    assert!(body.contains("(cut: the whole is on the engine's page)"), "{body}");
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
async fn a_check_red_on_main_as_well_holds_the_work_saying_so_with_what_its_log_says() {
    let hub = Hub::new();
    let head = hub.committed().await;
    let main = main_of(&hub);
    hub.github()
        .checks(&head, vec![vec![passed("lint"), job("test", 7, red())]]);
    hub.github()
        .checks(&main, vec![vec![passed("lint"), job("test", 3, red())]]);
    hub.github().log(7, "error: the greeting is wrong\n");
    let verdict = hub.verdict(&head).await.unwrap().unwrap();
    let failed = verdict.failed.expect("a check red on the commit holds the work");
    assert!(
        failed.contains("test (failure) https://github.example/actions/runs/1/job/7"),
        "{failed}"
    );
    let also = format!(
        "It is also red on main at {main} (https://github.example/actions/runs/1/job/3); whether the change adds a \
         failure of its own is not known."
    );
    assert!(failed.contains(&also), "{failed}");
    assert!(failed.contains("the greeting is wrong"), "its log is kept: {failed}");
    assert!(!failed.contains("not the change's"), "{failed}");
}

#[tokio::test]
async fn a_failure_read_before_is_never_run_again() {
    let hub = Hub::new();
    let head = hub.committed().await;
    hub.github()
        .checks(&head, vec![vec![passed("lint"), job("test", 7, red())]]);
    let seen = hub.verdict(&head).await.unwrap().unwrap().failed.expect("it fails");
    let place = hub.place();
    let again = (hub
        .host
        .verdict(&branch(&place), &head, Some(&seen), &mut Stop::never())
        .await)
        .unwrap()
        .unwrap();
    assert!(again.failed.is_some(), "a failed run is a failed check");
    assert!(!hub.github().asked().iter().any(|a| matches!(a, Asked::Rerun(_))));
    let said = format!("{again:?}");
    assert!(
        !said.contains("now and then") && !said.contains("not the change's"),
        "{said}"
    );
}

#[tokio::test]
async fn every_run_of_a_check_on_the_commit_is_said_as_it_is() {
    let hub = Hub::new();
    let head = hub.committed().await;
    // A person ran the red check again on GitHub, and its new run passed.
    let mut test = job("test", 8, Outcome::Passed);
    test.earlier = vec!["failure https://github.example/actions/runs/1/job/7".to_string()];
    hub.github().checks(&head, vec![vec![passed("lint"), test]]);
    let verdict = hub.verdict(&head).await.unwrap().unwrap();
    assert_eq!(verdict.failed, None, "the latest run counts");
    let said = "test: its latest run on this commit passed (https://github.example/actions/runs/1/job/8); its earlier \
                runs on it: failure https://github.example/actions/runs/1/job/7.";
    assert_eq!(verdict.passed_over, [said]);
    let place = hub.place();
    let told = (hub.host.check(&branch(&place), None, &mut Stop::never()).await)
        .unwrap()
        .unwrap();
    assert!(told.contains(said), "{told}");
}

#[tokio::test]
async fn an_agents_check_holds_a_check_red_on_main_as_well_as_the_check_before_the_merge_does() {
    let hub = Hub::new();
    let head = hub.committed().await;
    let main = main_of(&hub);
    hub.github()
        .checks(&head, vec![vec![job("lint", 5, red()), passed("test")]]);
    hub.github()
        .checks(&main, vec![vec![job("lint", 2, red()), passed("test")]]);
    let place = hub.place();
    let told = (hub.host.check(&branch(&place), None, &mut Stop::never()).await)
        .unwrap()
        .unwrap();
    assert!(told.contains("did not pass"), "{told}");
    assert!(told.contains(&format!("It is also red on main at {main}")), "{told}");
}

#[tokio::test]
async fn a_checked_commit_whose_latest_run_of_a_named_check_failed_does_not_merge_whatever_main_is() {
    let hub = Hub::new();
    let checked = hub.committed().await;
    hub.github()
        .checks(&checked, vec![vec![passed("lint"), passed("test")]]);
    hub.verdict(&checked).await.unwrap().unwrap();
    // Run again on GitHub after the check, the commit's test failed; main is green.
    hub.github()
        .checks(&checked, vec![vec![passed("lint"), job("test", 9, red())]]);
    let refused = hub.merge(&checked).await.expect_err("refused");
    assert!(
        refused.contains(&format!("test failed on {checked}")) && refused.contains("job/9"),
        "{refused}"
    );
    assert!(!hub.github().asked().iter().any(|a| matches!(a, Asked::Merge { .. })));
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
async fn only_the_works_pull_request_into_main_is_closed() {
    let hub = Hub::new();
    let checked = hub.pushed_and_checked_after(hub.committed().await).await;
    hub.github().set_base(1, "elsewhere");
    let place = hub.place();
    hub.host
        .end(&branch(&place), false)
        .await
        .expect("nothing of the work's to close");
    assert!(!hub.github().asked().iter().any(|a| matches!(a, Asked::Close { .. })));
    assert_eq!(hub.github().pull_requests()[0].state, State::Open, "{checked}");
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
