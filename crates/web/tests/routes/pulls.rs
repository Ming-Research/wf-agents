//! `GET /api/v1/pulls`: the pull requests open on the host of each project's repository, as the carrier of the engine's
//! acts says them, each with the work that keeps it as its latest, by its project and its number.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use axum::http::StatusCode;
use serde_json::{Value, json};
use service::{OpenPull, Round, RoundReply, Rounds, Scripted};
use store::Store;

use crate::{Running, get_json, greeted, pass, post, refused, running_carried, trial_script};

/// What a project's host says.
type Said = Result<Option<Vec<OpenPull>>, String>;

fn open(number: u64, title: &str, draft: bool) -> OpenPull {
    OpenPull {
        number,
        title: title.to_string(),
        url: format!("https://github.com/o/r/pull/{number}"),
        draft,
        updated: crate::START - 3_600_000,
    }
}

/// The trial's acts, whose hosts say what they are given, by the project asked.
struct Hosts {
    acts: Arc<dyn Rounds>,
    trial: Said,
    other: Said,
}

impl Rounds for Hosts {
    fn give(&self, round: Round, reply: RoundReply) {
        self.acts.give(round, reply);
    }

    fn stop(&self, round: Round, reply: RoundReply) {
        self.acts.stop(round, reply);
    }

    fn open_pulls(&self, project: &str) -> Pin<Box<dyn Future<Output = Said> + Send + '_>> {
        let said = match project {
            "trial" => self.trial.clone(),
            _ => self.other.clone(),
        };
        Box::pin(async move { said })
    }
}

/// The trial's agents, whose first round of each work says the pull request #12 it is checked through, and whose second
/// round of the first work says #14 in its place.
struct Pulling {
    agents: Arc<dyn Rounds>,
}

impl Rounds for Pulling {
    fn give(&self, round: Round, reply: RoundReply) {
        let url = |number: u64| format!("https://github.com/o/r/pull/{number}");
        match (round.work.0, round.number) {
            (_, 1) => reply.hosting().pulled(12, url(12)),
            (1, 2) => reply.hosting().pulled(14, url(14)),
            _ => {}
        }
        self.agents.give(round, reply);
    }

    fn stop(&self, round: Round, reply: RoundReply) {
        self.agents.stop(round, reply);
    }
}

/// Two items, one of each project, their works let on, and the hosts of the projects saying these.
async fn hosted(trial: Said, other: Said) -> Running {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("store.sqlite")).unwrap();
    let mut carriers = Scripted::new(trial_script()).carriers();
    carriers.acts = Arc::new(Hosts {
        acts: carriers.acts.clone(),
        trial,
        other,
    });
    carriers.agents = Arc::new(Pulling {
        agents: carriers.agents.clone(),
    });
    let running = running_carried(Box::new(store), dir, carriers);
    let router = &running.router;
    greeted(router).await;
    let item = json!({
        "title": "Greet the other user",
        "dependsOn": [],
        "project": "other",
        "flow": "trial",
        "writes": [{ "block": "request", "content": "Greet them, please.", "value": null }],
    });
    assert_eq!(
        post(router, "/api/v1/items", &item).await,
        (StatusCode::OK, json!({ "item": 2, "work": 2 }))
    );
    assert_eq!(
        post(router, "/api/v1/works/2/acts", &json!({ "kind": "resume" }))
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    pass(5000).await;
    running
}

/// What the answer says of each pull request: its project, its number, and the work that keeps it.
fn kept(pulls: &Value) -> Vec<(String, u64, Option<u64>)> {
    (pulls["pulls"].as_array().unwrap().iter())
        .map(|pull| {
            (
                pull["project"].as_str().unwrap().to_string(),
                pull["number"].as_u64().unwrap(),
                pull["work"]["work"].as_u64(),
            )
        })
        .collect()
}

// A pull request is a work's by its project and its number: two projects that each have a #12 give each its own work,
// a work with two pull requests is given only the latest it keeps, and one no work keeps is given with none.
#[tokio::test(start_paused = true)]
async fn each_open_pull_request_is_given_with_the_work_that_keeps_it_by_its_project_and_number() {
    let running = hosted(
        Ok(Some(vec![
            open(20, "Stray", false),
            open(14, "Greet the user", true),
            open(12, "Greet the user first", false),
        ])),
        Ok(Some(vec![open(12, "Greet the other user", false)])),
    )
    .await;
    let router = &running.router;
    let pulls = get_json(router, "/api/v1/pulls").await;
    assert_eq!(
        kept(&pulls),
        [
            ("other".to_string(), 12, Some(2)),
            ("trial".to_string(), 12, None),
            ("trial".to_string(), 14, Some(1)),
            ("trial".to_string(), 20, None),
        ]
    );
    assert_eq!((&pulls["failed"], &pulls["without"]), (&json!([]), &json!([])));
    let second = &pulls["pulls"][2];
    assert_eq!(
        (
            second["title"].as_str(),
            second["draft"].as_bool(),
            second["updated"].as_u64()
        ),
        (Some("Greet the user"), Some(true), Some(crate::START - 3_600_000))
    );
    assert_eq!(second["work"]["item"], 1);
    assert!(second["work"]["last"].as_u64().is_some(), "{second}");
    assert!(second["work"]["now"]["kind"].is_string(), "{second}");
    // One project alone.
    let other = get_json(router, "/api/v1/pulls?project=other").await;
    assert_eq!(kept(&other), [("other".to_string(), 12, Some(2))]);
    assert_eq!(
        refused(router, "/api/v1/pulls?project=nowhere").await,
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            "projects: the installation names no project \"nowhere\"; it names other, trial.".to_string()
        )
    );
}

// A project whose host cannot say is said by itself with why, never as having none, and the other project's pull
// requests are given as they are.
#[tokio::test(start_paused = true)]
async fn a_project_whose_host_cannot_say_is_given_with_why_and_the_other_projects_pull_requests_are_still_given() {
    let running = hosted(
        Ok(Some(vec![open(14, "Greet the user", false)])),
        Err("gh api GET repos/o/r/pulls failed: HTTP 403: API rate limit exceeded".to_string()),
    )
    .await;
    let pulls = get_json(&running.router, "/api/v1/pulls").await;
    assert_eq!(kept(&pulls), [("trial".to_string(), 14, Some(1))]);
    assert_eq!(
        pulls["failed"],
        json!([{ "project": "other", "why": "gh api GET repos/o/r/pulls failed: HTTP 403: API rate limit exceeded" }])
    );
    assert_eq!(pulls["without"], json!([]));
}

// A host that has no pull requests, as the local repository, is said so, and is no failure.
#[tokio::test(start_paused = true)]
async fn a_host_with_no_pull_requests_is_given_as_without_them() {
    let running = hosted(Ok(Some(Vec::new())), Ok(None)).await;
    let pulls = get_json(&running.router, "/api/v1/pulls").await;
    assert_eq!(pulls, json!({ "pulls": [], "failed": [], "without": ["other"] }));
}
