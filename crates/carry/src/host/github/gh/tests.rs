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

// Reference: the Unix time of a day is 86 400 s times the days counted from 1970-01-01 on the Gregorian calendar, which
// this walk counts one day at a time, with no formula, and a day past a month's last is no day.
#[test]
fn the_epoch_of_every_day_from_1970_to_2400_is_what_counting_the_days_one_by_one_gives() {
    let leap = |year: i64| (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let mut days = 0i64;
    for year in 1970..=2400i64 {
        let february = if leap(year) { 29 } else { 28 };
        for (month, length) in [31, february, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
            .into_iter()
            .enumerate()
        {
            let month = month + 1;
            for day in 1..=length {
                let at = |time: &str| epoch(&format!("{year:04}-{month:02}-{day:02}T{time}Z"));
                assert_eq!(at("00:00:00"), Some(days * 86_400), "{year}-{month}-{day}");
                assert_eq!(at("23:59:59"), Some(days * 86_400 + 86_399), "{year}-{month}-{day}");
                assert_eq!(at("12:34:56"), Some(days * 86_400 + 45_296), "{year}-{month}-{day}");
                days += 1;
            }
            let past = epoch(&format!("{year:04}-{month:02}-{:02}T00:00:00Z", length + 1));
            assert_eq!(past, None, "{year}-{month} has no day {}", length + 1);
        }
    }
}

// Reference: the Unix times the system's `date -u` gives for these stamps, and the two constants every Unix reference
// gives for the first and the last day of the years 1 to 9999.
#[test]
fn a_stamp_is_the_unix_time_the_systems_date_gives_for_it() {
    for (stamp, unix) in [
        ("2026-11-14T10:03:12Z", 1_794_650_592),
        ("2024-02-29T23:59:59Z", 1_709_251_199),
        ("2024-12-31T23:59:59Z", 1_735_689_599),
        ("2026-02-28T00:00:00Z", 1_772_236_800),
        ("2038-01-19T03:14:07Z", 2_147_483_647),
        ("2100-03-01T00:00:00Z", 4_107_542_400),
        ("2000-02-29T00:00:00Z", 951_782_400),
        ("1999-12-31T23:59:59Z", 946_684_799),
        ("2200-03-01T00:00:00Z", 7_263_216_000),
        ("2400-02-29T12:00:00Z", 13_574_606_400),
        ("0001-01-01T00:00:00Z", -62_135_596_800),
        ("9999-12-31T23:59:59Z", 253_402_300_799),
    ] {
        assert_eq!(epoch(stamp), Some(unix), "{stamp}");
    }
    assert_eq!(
        seconds_between("0001-01-01T00:00:00Z", "9999-12-31T23:59:59Z"),
        Some(253_402_300_799 + 62_135_596_800)
    );
}

// A stamp is the one form GitHub gives and no other: a sign, a blank, a lower-case mark or a letter that is not an
// ASCII digit in a field, or a form of another length, gives no time and never a wrong one.
#[test]
fn a_stamp_with_anything_but_digits_in_a_field_or_in_another_form_is_no_stamp() {
    for stamp in [
        "",
        "2026-11-14T10:03:12",
        "2026-11-14T10:03:12ZZ",
        "2026-11-14t10:03:12Z",
        "2026-11-14T10:03:12z",
        "+026-11-14T10:03:12Z",
        "2026-+1-14T10:03:12Z",
        "2026-11--4T10:03:12Z",
        "2026-11-14T 0:03:12Z",
        "2026-11-14T10:0 :12Z",
        "2026-11-14T10:03:1-Z",
        "2026-11-14T10:03:\u{e9}Z",
        "2026-11-14T10:03:12\u{e9}",
        "\u{663}026-11-14T10:03:12Z",
        "2026-00-14T10:03:12Z",
        "2026-11-00T10:03:12Z",
        "2026-11-31T10:03:12Z",
        "2026-11-14T10:60:00Z",
        "2026-11-14T10:00:60Z",
    ] {
        assert_eq!(epoch(stamp), None, "{stamp:?}");
        assert_eq!(seconds_between("2026-11-14T10:00:00Z", stamp), None, "{stamp:?}");
        assert_eq!(seconds_between(stamp, "2026-11-14T10:00:00Z"), None, "{stamp:?}");
    }
}

// Only a conclusion that judged the code gives a time: `neutral` counts as passed, while the others that end a job
// without judging it, GitHub's `action_required`, `stale` and `startup_failure`, give none.
#[test]
fn a_neutral_job_is_timed_and_one_that_ended_without_judging_the_code_is_not() {
    let took = |conclusion: &str| {
        job(&json!({
            "name": "test", "status": "completed", "conclusion": conclusion,
            "started_at": "2026-11-14T10:00:00Z", "completed_at": "2026-11-14T10:00:07Z",
        }))
        .map(|j| (j.outcome, j.took))
    };
    assert_eq!(took("neutral"), Some((Outcome::Passed, Some(7))));
    for conclusion in ["action_required", "stale", "startup_failure", "cancelled"] {
        let (outcome, took) = took(conclusion).expect("a job");
        assert!(matches!(outcome, Outcome::Broken(_)), "{conclusion}");
        assert_eq!(took, None, "{conclusion}");
    }
}

#[test]
fn a_run_is_read_by_its_status_and_one_that_completed_cancelled_is_said_so() {
    let run = |status: &str, conclusion: Option<&str>| {
        run(&json!({
            "id": 43658592831u64,
            "display_title": "ci 0f3a",
            "status": status,
            "conclusion": conclusion,
            "html_url": "https://github.com/o/r/actions/runs/43658592831",
        }))
        .expect("a run")
    };
    let going = run("in_progress", None);
    assert_eq!(
        going,
        Run {
            id: 43658592831,
            title: "ci 0f3a".to_string(),
            started: true,
            completed: false,
            cancelled: false,
            url: "https://github.com/o/r/actions/runs/43658592831".to_string(),
        }
    );
    let queued = run("queued", None);
    assert!(!queued.started && !queued.completed && !queued.cancelled);
    let cancelled = run("completed", Some("cancelled"));
    assert!(cancelled.completed && cancelled.cancelled);
    for judged in ["success", "failure", "timed_out"] {
        let ended = run("completed", Some(judged));
        assert!(ended.completed && !ended.cancelled, "{judged}");
    }
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
fn the_requests_that_open_edit_ready_merge_and_close_a_pull_request_are_as_githubs_api_takes_them() {
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
    let (method, path, body) = merging("o/r", 12, "c0ffee", "squash", "Greet the user by name (#12)");
    assert_eq!((method, path.as_str()), ("PUT", "repos/o/r/pulls/12/merge"));
    assert_eq!(
        body,
        json!({ "sha": "c0ffee", "merge_method": "squash", "commit_title": "Greet the user by name (#12)" })
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
