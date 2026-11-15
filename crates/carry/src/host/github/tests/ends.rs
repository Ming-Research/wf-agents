//! What the GitHub host does around a work's pull request, its check and its end: the pull request is a draft named as
//! the work's merge is, whose body says what the work wrote of itself, both following the work, made ready as it
//! merges; a failure read is read as it is, and a work that ended without merging has its pull request closed.

use super::super::api::{Check, Outcome};
use super::*;

/// A check of a GitHub Actions job, at its own link.
fn job(name: &str, id: u64, outcome: Outcome) -> Check {
    Check {
        url: Some(format!("https://github.example/actions/runs/1/job/{id}")),
        ..check(name, outcome)
    }
}

fn red() -> Outcome {
    Outcome::Failed("failure".into())
}

/// The work's branch, saying this of itself.
fn about<'a>(place: &'a Place, about: &'a str) -> Branch<'a> {
    Branch { about, ..branch(place) }
}

#[tokio::test]
async fn the_pull_request_is_opened_a_draft_saying_what_the_work_wrote_and_follows_it() {
    let hub = Hub::new();
    let head = hub.committed().await;
    hub.green(&head);
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
        .verdict(&branch, &head, hub.round(), &mut Stop::never())
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

/// The work's branch, its merge named so.
fn named<'a>(place: &'a Place, title: &'a str) -> Branch<'a> {
    Branch { title, ..branch(place) }
}

#[tokio::test]
async fn the_pull_request_is_named_as_the_works_merge_is_and_follows_it_to_the_commit_its_merge_makes() {
    let hub = Hub::new();
    let checked = hub.committed().await;
    hub.green(&checked);
    // Checked before the work wrote the name of its merge, then after.
    for name in ["Work 1", "Greet the user by name"] {
        assert_eq!(titled(&hub, &checked, name).await, name);
    }
    // Named anew after its check, it merges by the name it has now.
    let place = hub.place();
    let name = "Greet the user by name, and by title where one is known";
    (hub.host.merge(&named(&place, name), Some(&checked)).await).expect("it merges");
    assert_eq!(hub.github().pull_requests()[0].title, name);
    assert_eq!(
        run(&hub.github().bare, &["log", "-1", "--format=%s", "main"]),
        format!("{name} (#1)"),
        "the squash is named as the merge is"
    );
    let titles: Vec<String> = (hub.github().asked().into_iter())
        .filter_map(|asked| match asked {
            Asked::Open { title, .. } | Asked::Edit { title, .. } | Asked::Merge { title, .. } => Some(title),
            _ => None,
        })
        .collect();
    assert_eq!(
        titles,
        [
            "Work 1".to_string(),
            "Greet the user by name".to_string(),
            name.to_string(),
            format!("{name} (#1)"),
        ]
    );
}

#[tokio::test]
async fn the_name_of_a_merge_is_published_with_no_mention_reference_or_closing_word_and_cut_to_a_bound() {
    let hub = Hub::new();
    let checked = hub.committed().await;
    hub.green(&checked);
    let z = '\u{200B}';
    assert_eq!(
        titled(&hub, &checked, "Fixes #12 for @octocat").await,
        format!("F{z}ixes #{z}12 for @{z}octocat")
    );
    let title = titled(&hub, &checked, &"word ".repeat(100)).await;
    assert_eq!(title.chars().count(), 200, "{title}");
    assert!(title.ends_with('…'), "{title}");
}

/// The title of the work's pull request once the work, its merge named so, is checked at the commit.
async fn titled(hub: &Hub, commit: &str, name: &str) -> String {
    let place = hub.place();
    (hub.host
        .verdict(&named(&place, name), commit, hub.round(), &mut Stop::never())
        .await)
        .unwrap()
        .unwrap();
    hub.github().pull_requests()[0].title.clone()
}

#[tokio::test]
async fn what_the_work_wrote_is_published_with_no_mention_reference_closing_word_or_path_of_this_machine() {
    let hub = Hub::new();
    let head = hub.committed().await;
    hub.green(&head);
    let data = hub.repo.data.display().to_string();
    let repo = hub.repo.project.repo.display().to_string();
    let wrote = format!(
        "@octocat fixes #12, Closes https://github.com/o/r/issues/3, resolved o/r#4.\nSee {data}/worktrees/1-x and \
         {repo}/src.\n```\nA fence of its own.\n```"
    );
    let place = hub.place();
    hub.host
        .verdict(&about(&place, &wrote), &head, hub.round(), &mut Stop::never())
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
        .verdict(&about(&place, &long), &head, hub.round(), &mut Stop::never())
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
    hub.green(&checked);
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
async fn a_failure_is_said_as_it_is_each_time_the_commit_is_judged() {
    let hub = Hub::new();
    let head = hub.committed().await;
    let jobs = vec![passed("lint"), job("test", 7, red())];
    hub.github().fast(&head, jobs, Some(("test-results", fast::junit())));
    let first = hub.verdict(&head).await.unwrap().unwrap();
    assert!(first.failed.is_some());
    let again = hub.verdict(&head).await.unwrap().unwrap();
    // Each judging is a run of its own, whose number alone differs.
    let same = |text: Option<String>| text.map(|t| t.replace("runs/1001", "runs/1000"));
    assert_eq!(
        same(again.failed),
        same(first.failed),
        "a failed run is a failed check, each time it is judged"
    );
}

#[tokio::test]
async fn a_work_that_ended_without_merging_has_its_pull_request_closed_saying_so_once() {
    let hub = Hub::new();
    let head = hub.committed().await;
    hub.green(&head);
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
async fn a_reopened_work_has_the_pull_request_its_cancel_closed_opened_again_once_however_often_it_starts() {
    let hub = Hub::new();
    let head = hub.committed().await;
    hub.green(&head);
    hub.verdict(&head).await.unwrap().unwrap();
    let place = hub.place();
    // Nothing was closed: nothing is opened.
    hub.host.reopen(&branch(&place)).await.expect("nothing to open");
    hub.host.end(&branch(&place), false).await.expect("it closes");
    hub.host.reopen(&branch(&place)).await.expect("it opens again");
    assert_eq!(hub.github().pull_requests()[0].state, State::Open);
    // Asked again, as at a start given again, the open one is left as it is.
    hub.host.reopen(&branch(&place)).await.expect("nothing to open");
    let reopened: Vec<Asked> = (hub.github().asked().into_iter())
        .filter(|a| matches!(a, Asked::Reopen(_)))
        .collect();
    assert_eq!(reopened, [Asked::Reopen(1)]);
    // One merged is never opened again.
    hub.github().merged_by_a_person(1, &head);
    hub.host.reopen(&branch(&place)).await.expect("nothing to open");
    assert_eq!(hub.github().pull_requests()[0].state, State::Merged);
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
