//! Reading what GitHub answers, and what `gh` is given.

use super::*;

fn run(id: u64, name: &str, status: &str, conclusion: Option<&str>) -> Value {
    json!({
        "id": id,
        "name": name,
        "status": status,
        "conclusion": conclusion,
        "html_url": format!("https://github.com/o/r/runs/{id}"),
        "output": { "title": "Lint", "summary": "" },
        "app": { "slug": "github-actions" },
    })
}

#[test]
fn the_latest_run_of_each_check_counts() {
    let runs = [
        run(7, "check", "completed", Some("failure")),
        // Run again by a person, and passed.
        run(9, "check", "completed", Some("success")),
        run(8, "page", "in_progress", None),
    ];
    let checks = checks(&runs, &[]);
    assert_eq!(checks.len(), 2);
    assert_eq!(
        (checks[0].name.as_str(), &checks[0].outcome),
        ("check", &Outcome::Passed)
    );
    assert_eq!(checks[0].job, Some(9));
    assert_eq!(checks[0].url.as_deref(), Some("https://github.com/o/r/runs/9"));
    assert_eq!(checks[0].summary.as_deref(), Some("Lint"));
    assert_eq!(
        (checks[1].name.as_str(), &checks[1].outcome),
        ("page", &Outcome::Pending)
    );
    // The earlier run is kept to be said as it is, beside the latest.
    assert_eq!(checks[0].earlier, ["failure https://github.com/o/r/runs/7"]);
    assert!(checks[1].earlier.is_empty());
}

#[test]
fn only_a_failure_or_a_timeout_judges_the_code() {
    let outcome = |conclusion| check_run("c", &run(1, "c", "completed", conclusion)).outcome;
    for passed in ["success", "neutral", "skipped"] {
        assert_eq!(outcome(Some(passed)), Outcome::Passed, "{passed}");
    }
    for failed in ["failure", "timed_out"] {
        assert_eq!(outcome(Some(failed)), Outcome::Failed(failed.to_string()));
    }
    for broken in ["cancelled", "action_required", "stale", "startup_failure"] {
        assert_eq!(outcome(Some(broken)), Outcome::Broken(broken.to_string()));
    }
    assert_eq!(outcome(None), Outcome::Broken("no conclusion".to_string()));
    let queued = check_run("c", &run(1, "c", "queued", None));
    assert_eq!(queued.outcome, Outcome::Pending);
    let mut other = run(1, "c", "completed", Some("failure"));
    other["app"]["slug"] = json!("another-app");
    assert_eq!(
        check_run("c", &other).job,
        None,
        "only a job of GitHub Actions has a log GitHub keeps"
    );
}

#[test]
fn a_commit_status_is_a_check_by_its_context() {
    let statuses = [
        json!({ "context": "ci/a", "state": "success", "target_url": "https://ci.example/a", "description": "" }),
        json!({ "context": "ci/b", "state": "failure", "description": "Two tests failed" }),
        json!({ "context": "ci/c", "state": "error" }),
        json!({ "context": "ci/d", "state": "pending" }),
    ];
    let checks = checks(&[], &statuses);
    let outcomes: Vec<(&str, &Outcome)> = checks.iter().map(|c| (c.name.as_str(), &c.outcome)).collect();
    assert_eq!(
        outcomes,
        [
            ("ci/a", &Outcome::Passed),
            ("ci/b", &Outcome::Failed("failure".to_string())),
            ("ci/c", &Outcome::Broken("error".to_string())),
            ("ci/d", &Outcome::Pending),
        ]
    );
    assert_eq!(checks[0].url.as_deref(), Some("https://ci.example/a"));
    assert_eq!(checks[0].summary, None);
    assert_eq!(checks[1].summary.as_deref(), Some("Two tests failed"));
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

#[tokio::test]
async fn a_jobs_log_is_read_with_the_escapes_that_colour_it() {
    // As gh does: a body with terminal escapes is printed only where it is allowed; else the status and headers are,
    // and gh exits with an error.
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("gh");
    std::fs::write(
        &script,
        "#!/bin/sh\nprintf 'HTTP/2.0 200 OK\\r\\n\\r\\n'\ncase \"$*\" in *--allow-escape-sequences*) printf '\\033[31merror\\033[0m: failed\\n' ;; *) echo 'the response contains terminal escape sequences' >&2; exit 1 ;; esac\n",
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let gh = Gh::new(&script.display().to_string(), "o/r", "the-token", dir.path());
    assert_eq!(gh.log(7).await, Ok("\u{1b}[31merror\u{1b}[0m: failed\n".to_string()));
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
