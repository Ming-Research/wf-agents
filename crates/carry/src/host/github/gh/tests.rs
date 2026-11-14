//! Reading what GitHub answers, and what `gh` is given.

use super::*;

#[test]
fn a_job_is_read_by_its_conclusion_and_one_that_did_not_run_is_skipped() {
    let job = |status: &str, conclusion: Option<&str>| {
        job(
            &json!({ "name": "test", "status": status, "conclusion": conclusion, "html_url": "https://github.com/o/r/runs/1/job/2" }),
        )
    };
    assert_eq!(
        job("completed", Some("success")).map(|j| j.outcome),
        Some(Outcome::Passed)
    );
    assert_eq!(
        job("completed", Some("failure")).map(|j| j.outcome),
        Some(Outcome::Failed("failure".to_string()))
    );
    assert_eq!(
        job("completed", Some("cancelled")).map(|j| j.outcome),
        Some(Outcome::Broken("cancelled".to_string()))
    );
    assert_eq!(job("in_progress", None).map(|j| j.outcome), Some(Outcome::Pending));
    assert_eq!(
        job("completed", Some("skipped")).map(|j| j.outcome),
        Some(Outcome::Skipped),
        "a job whose condition was false did not run, and is not left out"
    );
}

// The time of a job is the whole seconds from its `started_at` to its `completed_at`, and only a job that ran to a
// conclusion has one: a cancelled job, one not complete and one that did not run did not finish.
#[test]
fn a_jobs_wall_time_is_read_from_its_two_stamps_when_it_ran_to_a_conclusion() {
    let took = |status: &str, conclusion: Option<&str>, started: Value, completed: Value| {
        job(&json!({
            "name": "test", "status": status, "conclusion": conclusion,
            "started_at": started, "completed_at": completed,
        }))
        .map(|j| j.took)
    };
    let (start, end) = (json!("2026-11-14T10:00:00Z"), json!("2026-11-14T10:03:12Z"));
    assert_eq!(
        took("completed", Some("success"), start.clone(), end.clone()),
        Some(Some(192))
    );
    assert_eq!(
        took("completed", Some("failure"), start.clone(), end.clone()),
        Some(Some(192))
    );
    assert_eq!(
        took("completed", Some("timed_out"), start.clone(), end.clone()),
        Some(Some(192))
    );
    for (status, conclusion) in [
        ("completed", Some("cancelled")),
        ("completed", Some("skipped")),
        ("in_progress", None),
    ] {
        assert_eq!(
            took(status, conclusion, start.clone(), end.clone()),
            Some(None),
            "{status} {conclusion:?} did not finish"
        );
    }
    // No usable stamp, no time: a missing one, a null, an order reversed, and any other form of a time.
    for (started, completed) in [
        (json!(null), end.clone()),
        (start.clone(), json!(null)),
        (end.clone(), start.clone()),
        (start.clone(), json!("2026-11-14T10:03:12.000Z")),
        (start.clone(), json!("2026-11-14T11:03:12+01:00")),
        (start.clone(), json!("2026-13-14T10:03:12Z")),
        (start.clone(), json!("2026-02-29T10:03:12Z")),
        (start.clone(), json!("2026-11-14T24:00:00Z")),
        (start.clone(), json!("2026-11-14 10:03:12Z")),
        (start.clone(), json!("2026-11-1\u{e9}T10:03:12Z")),
    ] {
        assert_eq!(
            took("completed", Some("success"), started.clone(), completed.clone()),
            Some(None),
            "{started} to {completed}"
        );
    }
}

// The seconds between two stamps hold over a day, a month, a year and a leap day, and none is lost at the March of a
// leap year.
#[test]
fn the_seconds_between_two_stamps_hold_across_days_months_years_and_leap_days() {
    let between = |from: &str, to: &str| seconds_between(from, to);
    assert_eq!(between("2026-11-14T10:00:00Z", "2026-11-14T10:00:00Z"), Some(0));
    assert_eq!(between("2026-11-14T23:59:59Z", "2026-11-15T00:00:00Z"), Some(1));
    assert_eq!(between("2026-11-30T23:59:00Z", "2026-12-01T00:01:00Z"), Some(120));
    assert_eq!(between("2026-12-31T23:59:59Z", "2027-01-01T00:00:00Z"), Some(1));
    assert_eq!(
        between("2024-02-28T00:00:00Z", "2024-03-01T00:00:00Z"),
        Some(2 * 86_400),
        "2024 has a 29th of February"
    );
    assert_eq!(
        between("2026-02-28T00:00:00Z", "2026-03-01T00:00:00Z"),
        Some(86_400),
        "2026 has none"
    );
    assert_eq!(
        between("2024-01-01T00:00:00Z", "2025-01-01T00:00:00Z"),
        Some(366 * 86_400)
    );
    assert_eq!(
        between("1999-12-31T00:00:00Z", "2000-03-01T00:00:00Z"),
        Some(61 * 86_400),
        "2000 is a leap year"
    );
    assert_eq!(
        between("2100-02-28T00:00:00Z", "2100-03-01T00:00:00Z"),
        Some(86_400),
        "2100 is not"
    );
    assert_eq!(epoch("1970-01-01T00:00:00Z"), Some(0));
    assert_eq!(epoch("2026-11-14T10:03:12Z"), Some(1_794_650_592));
}

#[test]
fn a_pull_request_is_read_with_its_merge_commit_only_once_merged() {
    let mut value = json!({
        "number": 12,
        "html_url": "https://github.com/o/r/pull/12",
        "state": "open",
        "merged_at": null,
        "head": { "sha": "c0ffee", "ref": "work-1-0a1b2c3d" },
        "base": { "ref": "next" },
        "merge_commit_sha": "test-merge",
        "title": "Greet the user",
        "body": null,
        "draft": true,
        "node_id": "PR_kwDO",
    });
    let open = pull(&value).unwrap();
    assert_eq!(
        open,
        Pull {
            number: 12,
            url: "https://github.com/o/r/pull/12".to_string(),
            state: State::Open,
            head: "c0ffee".to_string(),
            base: "next".to_string(),
            merge_commit: None,
            title: "Greet the user".to_string(),
            body: String::new(),
            draft: true,
            node: "PR_kwDO".to_string(),
        }
    );
    value["state"] = json!("closed");
    assert_eq!(pull(&value).unwrap().state, State::Closed);
    value["merged_at"] = json!("2026-11-10T12:00:00Z");
    value["merge_commit_sha"] = json!("beef");
    let merged = pull(&value).unwrap();
    assert_eq!(
        (merged.state, merged.merge_commit.as_deref()),
        (State::Merged, Some("beef"))
    );
}

#[test]
fn the_requests_that_open_edit_ready_and_close_a_pull_request_are_as_githubs_api_takes_them() {
    let (method, path, body) = opening("o/r", "work-1-x", "main", "Greet the user", "It greets.");
    assert_eq!((method, path.as_str()), ("POST", "repos/o/r/pulls"));
    assert_eq!(
        body,
        json!({ "title": "Greet the user", "body": "It greets.", "head": "work-1-x", "base": "main", "draft": true })
    );
    let (method, path, body) = editing("o/r", 12, "Greet the user", "It greets by name.");
    assert_eq!((method, path.as_str()), ("PATCH", "repos/o/r/pulls/12"));
    assert_eq!(body, json!({ "title": "Greet the user", "body": "It greets by name." }));
    let (method, path, body) = readying("PR_kwDO");
    assert_eq!((method, path.as_str()), ("POST", "graphql"));
    assert_eq!(body["variables"], json!({ "id": "PR_kwDO" }));
    let query = body["query"].as_str().unwrap();
    assert!(
        query.contains("markPullRequestReadyForReview(input: {pullRequestId: $id})"),
        "{query}"
    );
    let [(said, at, comment), (close, pull, closed)] = closing("o/r", 12, "Its work ended without merging.");
    assert_eq!((said, at.as_str()), ("POST", "repos/o/r/issues/12/comments"));
    assert_eq!(comment, json!({ "body": "Its work ended without merging." }));
    assert_eq!((close, pull.as_str()), ("PATCH", "repos/o/r/pulls/12"));
    assert_eq!(closed, json!({ "state": "closed" }));
}

#[test]
fn an_answer_is_read_with_its_status_and_a_refusal_in_githubs_words() {
    let out = "HTTP/2.0 409 Conflict\r\nContent-Type: application/json\r\n\r\n{\"message\":\"Head branch was \
               modified. Review and try the merge again.\"}";
    let answer = included(out).unwrap();
    assert_eq!(answer.status, 409);
    assert_eq!(
        refusal(&answer),
        "HTTP 409: Head branch was modified. Review and try the merge again."
    );
    let listed = "HTTP/2.0 422 Unprocessable Entity\nX: y\n\n{\"message\":\"Validation Failed\",\"errors\":[{\"message\":\
                  \"A pull request already exists\"}]}";
    assert_eq!(
        refusal(&included(listed).unwrap()),
        "HTTP 422: Validation Failed; A pull request already exists"
    );
    assert_eq!(included("gh: not logged in"), None);
}

#[test]
fn gh_is_given_the_token_and_nothing_else_of_the_engines_but_what_git_is() {
    let engine = [
        ("PATH", "/bin"),
        ("HOME", "/home/someone"),
        ("GH_TOKEN", "the owner's own"),
        ("GITHUB_TOKEN", "another"),
        ("ANTHROPIC_API_KEY", "a key"),
    ]
    .map(|(n, v)| (n.to_string(), v.to_string()));
    let env = environment(engine, "the engine's", Path::new("/data/gh"));
    let names: Vec<&str> = env.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(
        names,
        [
            "HOME",
            "PATH",
            "GH_TOKEN",
            "GH_CONFIG_DIR",
            "GH_PROMPT_DISABLED",
            "GH_NO_UPDATE_NOTIFIER",
            "NO_COLOR"
        ]
    );
    let token: Vec<&str> = (env.iter())
        .filter(|(n, _)| n == "GH_TOKEN")
        .map(|(_, v)| v.as_str())
        .collect();
    assert_eq!(token, ["the engine's"]);
    let gh = Gh::new("gh", "o/r", "the engine's", Path::new("/data/gh"));
    assert!(!format!("{gh:?}").contains("the engine's"), "the token is not shown");
}

#[tokio::test]
async fn a_gh_that_never_answers_is_given_up_within_its_limit() {
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("gh");
    std::fs::write(&script, "#!/bin/sh\nexec sleep 30\n").unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let gh = Gh::new(&script.display().to_string(), "o/r", "the-token", dir.path())
        .within(std::time::Duration::from_millis(500));
    let began = std::time::Instant::now();
    let given_up = gh.pulls("work-1").await.expect_err("given up");
    assert!(given_up.contains("did not end within"), "{given_up}");
    assert!(began.elapsed() < std::time::Duration::from_secs(10));
}
