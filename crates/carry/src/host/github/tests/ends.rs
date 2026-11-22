//! What the GitHub host does around a work's pull request, its check and its end: the pull request is a draft named as
//! the work's merge is, whose body says what the work wrote of itself, both following the work, made ready as it
//! merges; a failure read is read as it is, and a work that ended without merging has its pull request closed.

use super::super::api::{Check, Outcome};
use super::*;

/// A check of a GitHub Actions job of this id, at its own link.
fn job(name: &str, id: u64, outcome: Outcome) -> Check {
    Check {
        id,
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
    assert!(body.contains("\n---\n\nThis pull request carries work W1"), "{body}");
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
        .verdict(
            &branch,
            &head,
            &hub.main_on_github(),
            hub.round(),
            &Mutex::new(()),
            &mut Stop::never(),
        )
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

/// What was said to the loop through the channel, as it was said: the events of the work.
pub(super) fn heard(heard: &mut tokio::sync::mpsc::UnboundedReceiver<service::Message>) -> Vec<engine::Event> {
    let mut events = Vec::new();
    while let Ok(message) = heard.try_recv() {
        if let service::Message::Happened { event, .. } = message {
            events.push(event);
        }
    }
    events
}

#[tokio::test]
async fn each_check_says_the_pull_request_it_checks_the_work_through_for_the_work_to_keep() {
    let hub = Hub::new();
    let head = hub.committed().await;
    hub.green(&head);
    let (to, mut said) = tokio::sync::mpsc::unbounded_channel();
    let reply = service::RoundReply::new(WORK, RunId(1), to);
    let mut stop = Stop::never().hosting(reply.hosting());
    let place = hub.place();
    // Opened by the first check and found again by the second, it is said by each: the loop keeps it once.
    for _ in 0..2 {
        let main = hub.main_on_github();
        (hub.host
            .verdict(&branch(&place), &head, &main, hub.round(), &Mutex::new(()), &mut stop)
            .await)
            .expect("checked")
            .expect("nothing stopped it");
    }
    let pull = hub.github().pull_requests()[0].clone();
    let pulled = engine::Event::Pulled {
        number: pull.number,
        url: pull.url,
    };
    assert_eq!(heard(&mut said), [pulled.clone(), pulled]);
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
    for name in ["Work W1", "Greet the user by name"] {
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
            "Work W1".to_string(),
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
        .verdict(
            &named(&place, name),
            commit,
            &hub.main_on_github(),
            hub.round(),
            &Mutex::new(()),
            &mut Stop::never(),
        )
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
        .verdict(
            &about(&place, &wrote),
            &head,
            &hub.main_on_github(),
            hub.round(),
            &Mutex::new(()),
            &mut Stop::never(),
        )
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
        .verdict(
            &about(&place, &long),
            &head,
            &hub.main_on_github(),
            hub.round(),
            &Mutex::new(()),
            &mut Stop::never(),
        )
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
    // Each judging has runs of its own, whose numbers alone differ.
    let same = |text: Option<String>| {
        text.map(|t| {
            let parts: Vec<&str> = t.split("actions/runs/").collect();
            let unnumbered = (parts[1..].iter()).map(|part| part.trim_start_matches(|c: char| c.is_ascii_digit()));
            [parts[0].to_string()]
                .into_iter()
                .chain(unnumbered.map(|rest| format!("actions/runs/N{rest}")))
                .collect::<String>()
        })
    };
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

// Two rounds of the check asked for at once go through one pull request: the one that read the list of them before the
// other's was opened finds the other's where its own is refused.
#[tokio::test]
async fn two_rounds_of_the_check_at_once_go_through_one_pull_request() {
    let hub = Hub::new();
    let head = hub.committed().await;
    hub.green(&head);
    hub.verdict(&head).await.unwrap().expect("checked");
    hub.github().hide_pulls_once();
    hub.verdict(&head)
        .await
        .expect("checked through the one opened")
        .expect("nothing stopped it");
    assert_eq!(hub.opened(), 1);
}

// A call of `run_tests` that the engine stopped before it read the result left a branch of its own on GitHub: it is the
// work's, and goes when the work ends, merged or not, whatever other works have left.
#[tokio::test]
async fn a_work_that_ended_has_the_branches_its_calls_of_run_tests_left_deleted_merged_or_not() {
    for merged in [true, false] {
        let hub = Hub::new();
        let head = hub.committed().await;
        let place = hub.place();
        let bare = hub.github().bare.display().to_string();
        let left = format!("wf-try/{}/call/9", place.branch);
        for name in [left.as_str(), "wf-try/another-work/call/1"] {
            run(
                &hub.repo.project.repo,
                &["push", "-q", &bare, &format!("{head}:refs/heads/{name}")],
            );
        }
        hub.host.end(&branch(&place), merged).await.expect("it ends");
        let deleted: Vec<Asked> = (hub.github().asked().into_iter())
            .filter(|a| matches!(a, Asked::Delete(_)))
            .collect();
        assert_eq!(deleted, [Asked::Delete(left)], "merged: {merged}");
    }
}

// The branches a call left are harmless beside the pull request: where they cannot be listed, the work's pull request is
// closed all the same, and the failure is said.
#[tokio::test]
async fn a_work_whose_left_branches_cannot_be_listed_has_its_pull_request_closed_all_the_same_and_it_is_said() {
    let mut hub = Hub::new();
    let head = hub.committed().await;
    hub.green(&head);
    hub.verdict(&head).await.unwrap().unwrap();
    let place = hub.place();
    hub.host.config.url = hub.repo.data.join("no-such-repository.git").display().to_string();
    let why = hub
        .host
        .end(&branch(&place), false)
        .await
        .expect_err("the listing fails");
    assert!(why.contains("could not be listed on GitHub"), "{why}");
    assert_eq!(hub.github().pull_requests()[0].state, State::Closed);
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

/// A host whose words for what it writes on a pull request are not those of the repository's files: every part of them
/// is marked, so that what the host writes is told from what it has no words for.
fn hub_with_other_words() -> Hub {
    let mut hub = Hub::new();
    hub.host.config.words = flow::agents::PullRequest {
        about: "ABOUT\n{about}\nEND\n".to_string(),
        body: "BODY {work}.".to_string(),
        cut: "CUT".to_string(),
        merged: "MERGED {name} no.{number}".to_string(),
        closed: "CLOSED".to_string(),
        withheld: flow::agents::Withheld {
            data: "<D>".to_string(),
            repository: "<R>".to_string(),
            home: "<H>".to_string(),
        },
    };
    let words = hub.host.config.words.withheld.clone();
    hub.host.config.withheld = withheld(&hub.repo.data, &hub.repo.project.repo, None, &words);
    hub
}

// What the engine writes on the pull request is the words of its file, and nothing else: with other words, the body,
// the mark of a cut, what stands for a path of this machine, the title the merge makes and the comment a closed one
// has are those, with the work's number written as a work's is.
#[tokio::test]
async fn what_the_engine_writes_on_the_pull_request_is_the_words_of_its_file_and_nothing_else() {
    let hub = hub_with_other_words();
    let head = hub.committed().await;
    hub.green(&head);
    let data = hub.repo.data.display().to_string();
    let place = hub.place();
    let long = format!("See {data}/x. {}", "word ".repeat(10_000));
    hub.host
        .verdict(
            &about(&place, &long),
            &head,
            &hub.main_on_github(),
            hub.round(),
            &Mutex::new(()),
            &mut Stop::never(),
        )
        .await
        .unwrap()
        .unwrap();
    let body = &hub.github().pull_requests()[0].body;
    assert!(body.starts_with("ABOUT\n```text\nSee <D>/x. word"), "{body}");
    assert!(body.ends_with("\nCUT\n```\nEND\nBODY W1."), "{body}");
    assert!(!body.contains(&data), "{body}");
    (hub.host.merge(&named(&place, "Greet"), Some(&head)).await).expect("it merges");
    assert_eq!(
        run(&hub.github().bare, &["log", "-1", "--format=%s", "main"]),
        "MERGED Greet no.1"
    );

    let hub = hub_with_other_words();
    let head = hub.committed().await;
    hub.green(&head);
    hub.verdict(&head).await.unwrap().unwrap();
    hub.host.end(&branch(&hub.place()), false).await.expect("it closes");
    let closed: Vec<Asked> = (hub.github().asked().into_iter())
        .filter(|a| matches!(a, Asked::Close { .. }))
        .collect();
    assert_eq!(
        closed,
        [Asked::Close {
            number: 1,
            comment: "CLOSED".to_string()
        }]
    );
}
