//! The routes of the observations: the observations of the returns the engine took, the newest first, each unread
//! until the owner reads it; those an architect adds, with their projects, titles and tags; the changes made to them,
//! all or none, each with its reason; and the count of each tag.

use axum::http::StatusCode;
use serde_json::{Value, json};
use store::Store;

use crate::{Running, START, get_json, pass, post, running_scripted};

/// The trial flow's script with the plan's first round handing in its plan and saying two observations.
const SCRIPT: &str = r#"
[[flows.trial.rounds]]
cell = "plan"
hands_in = [{ block = "plan" }]
observations = ["The greeting has no test.", "The request names no language."]
"#;

/// The engine's loop over the trial flow with that script, and the trial item added and let on at a second: by 1.2
/// seconds its plan's return has been taken.
async fn planned() -> Running {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("store.sqlite")).unwrap();
    let running = running_scripted(Box::new(store), dir, service::Script::read(SCRIPT).unwrap());
    let item = json!({
        "title": "Greet the user",
        "dependsOn": [],
        "project": "trial",
        "flow": "trial",
        "writes": [{ "block": "request", "content": "Greet the user, please.", "value": null }],
    });
    assert_eq!(post(&running.router, "/api/v1/items", &item).await.0, StatusCode::OK);
    pass(1000).await;
    let resumed = post(&running.router, "/api/v1/works/1/acts", &json!({ "kind": "resume" })).await;
    assert_eq!(resumed, (StatusCode::NO_CONTENT, Value::Null));
    pass(2000).await;
    running
}

/// An observation of the plan's first round, as the route gives it: of and reported by the project of its item.
fn of_the_plan(number: u64, text: &str, read: Value) -> Value {
    json!({
        "number": number,
        "text": text,
        "title": null,
        "tags": [],
        "project": "trial",
        "reportedBy": "trial",
        "round": {
            "work": 1,
            "item": 1,
            "itemTitle": "Greet the user",
            "cell": "plan",
            "run": 1,
            "round": 1,
        },
        "kept": START + 1200,
        "read": read,
        "takenInBy": null,
        "handled": false,
    })
}

/// An observation an architect adds, of this project and reported by that one.
fn architects(project: &str, reported_by: &str) -> Value {
    json!({
        "text": "The merge cell names no tests.",
        "title": "Name the tests of the merge cell",
        "tags": ["flow", "tests"],
        "project": project,
        "reportedBy": reported_by,
    })
}

/// The answer to a POST of changes of observations for this reason.
async fn changing(running: &Running, why: &str, changes: Value) -> (StatusCode, Value) {
    post(
        &running.router,
        "/api/v1/observations/changes",
        &json!({ "why": why, "changes": changes }),
    )
    .await
}

/// The numbers of the observations the route lists, the newest first.
async fn numbers(running: &Running) -> Vec<u64> {
    let page = get_json(&running.router, "/api/v1/observations").await;
    (page["observations"].as_array().unwrap().iter())
        .map(|o| o["number"].as_u64().unwrap())
        .collect()
}

#[tokio::test(start_paused = true)]
async fn the_observations_of_a_return_taken_are_listed_newest_first_and_unread() {
    let running = planned().await;
    let listed = get_json(&running.router, "/api/v1/observations").await;
    assert_eq!(
        listed,
        json!({ "observations": [
            of_the_plan(2, "The request names no language.", Value::Null),
            of_the_plan(1, "The greeting has no test.", Value::Null),
        ] })
    );
}

#[tokio::test(start_paused = true)]
async fn an_observation_the_owner_reads_is_read_from_then_on_and_one_there_is_none_of_is_refused() {
    let running = planned().await;
    let router = &running.router;
    let now = START + 3000;
    assert_eq!(
        post(router, "/api/v1/observations/1/read", &json!({})).await,
        (StatusCode::OK, json!({ "read": now }))
    );
    // Read again later, it keeps the time it was first read.
    pass(1000).await;
    assert_eq!(
        post(router, "/api/v1/observations/1/read", &json!({})).await,
        (StatusCode::OK, json!({ "read": now }))
    );
    let listed = get_json(router, "/api/v1/observations").await;
    assert_eq!(
        listed["observations"],
        json!([
            of_the_plan(2, "The request names no language.", Value::Null),
            of_the_plan(1, "The greeting has no test.", json!(now)),
        ])
    );
    assert_eq!(
        post(router, "/api/v1/observations/3/read", &json!({})).await,
        (StatusCode::NOT_FOUND, json!({ "error": "There is no observation O3." }))
    );
    let (status, _) = post(router, "/api/v1/observations/O1/read", &json!({})).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test(start_paused = true)]
async fn a_number_past_what_the_store_can_number_names_no_observation() {
    let running = planned().await;
    // One past the largest the store gives, the largest a path's number reads as, and one past that.
    for number in ["9223372036854775808", "18446744073709551615", "18446744073709551616"] {
        assert_eq!(
            post(
                &running.router,
                &format!("/api/v1/observations/{number}/read"),
                &json!({})
            )
            .await,
            (
                StatusCode::NOT_FOUND,
                json!({ "error": format!("There is no observation O{number}.") })
            ),
            "{number}"
        );
    }
    // Nothing was read.
    let listed = get_json(&running.router, "/api/v1/observations").await;
    let read: Vec<&Value> = (listed["observations"].as_array().unwrap().iter())
        .map(|o| &o["read"])
        .collect();
    assert_eq!(read, [&Value::Null, &Value::Null]);
}

#[tokio::test(start_paused = true)]
async fn adding_an_observation_checks_both_projects_and_the_title_length() {
    let running = planned().await;
    let router = &running.router;
    // One of the project `trial`, reported by `other`, is kept after the two the plan's return kept, with no round.
    assert_eq!(
        post(router, "/api/v1/observations", &architects("trial", "other")).await,
        (StatusCode::OK, json!({ "number": 3 }))
    );
    let listed = get_json(router, "/api/v1/observations").await;
    assert_eq!(
        listed["observations"][0],
        json!({
            "number": 3,
            "text": "The merge cell names no tests.",
            "title": "Name the tests of the merge cell",
            "tags": ["flow", "tests"],
            "project": "trial",
            "reportedBy": "other",
            "round": null,
            "kept": START + 3000,
            "read": null,
            "takenInBy": null,
            "handled": false,
        })
    );
    // Each of these is refused for its own reason, and nothing is kept.
    let mut untitled = architects("trial", "trial");
    untitled["title"] = json!("a b c d e f g h i j k l m n o p q r s t u");
    let mut blank = architects("trial", "trial");
    blank["text"] = json!("  ");
    let mut tagged = architects("trial", "trial");
    tagged["tags"] = json!(["two words"]);
    let refused = [
        (architects("nowhere", "trial"), "nowhere"),
        (architects("trial", "nowhere"), "nowhere"),
        (untitled, "21 words"),
        (blank, "says nothing"),
        (tagged, "two words"),
    ];
    for (asked, said) in refused {
        let (status, body) = post(router, "/api/v1/observations", &asked).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{asked}");
        let error = body["error"].as_str().expect("an error");
        assert!(error.contains(said), "{error:?} lacks {said:?}");
    }
    let (status, _) = post(router, "/api/v1/observations", &json!({ "text": "No project." })).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(numbers(&running).await, [3, 2, 1]);
}

#[tokio::test(start_paused = true)]
async fn changes_are_refused_whole_with_every_reason() {
    let running = planned().await;
    let refused = changing(
        &running,
        "Tidying.",
        json!([
            { "kind": "title", "observation": 1, "title": "A good title" },
            { "kind": "tag", "observation": 99, "tag": "flow" },
            { "kind": "tag", "observation": 2, "tag": "two words" },
            { "kind": "takenIn", "observation": 1, "item": 7 },
        ]),
    )
    .await;
    assert_eq!(refused.0, StatusCode::UNPROCESSABLE_ENTITY);
    let error = refused.1["error"].as_str().expect("an error");
    for said in [
        "Change 2 (O99): there is no such observation",
        "change 3 (O2): \"two words\" is no tag",
        "change 4 (O1): there is no item #7",
        "none of the changes was made",
    ] {
        assert!(error.contains(said), "{error:?} lacks {said:?}");
    }
    assert!(!error.contains("change 1"), "{error:?} refuses the good change");
    // The first, which was good, was not made.
    let listed = get_json(&running.router, "/api/v1/observations").await;
    assert_eq!(listed["observations"][1]["title"], Value::Null);
    // A change that leaves an observation as it is changes nothing, and is not refused.
    let made = changing(
        &running,
        "Titling.",
        json!([
            { "kind": "title", "observation": 1, "title": "A good title" },
            { "kind": "title", "observation": 1, "title": "A good title" },
        ]),
    )
    .await;
    assert_eq!(made, (StatusCode::OK, json!({ "changed": [true, false] })));
    // Without a reason, with no change, or with an unreadable one, nothing is asked.
    for body in [
        json!({ "why": " ", "changes": [{ "kind": "delete", "observation": 1 }] }),
        json!({ "why": "Why.", "changes": [] }),
    ] {
        let (status, _) = post(&running.router, "/api/v1/observations/changes", &body).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    }
    let (status, _) = post(
        &running.router,
        "/api/v1/observations/changes",
        &json!({ "why": "Why.", "changes": [{ "kind": "merge", "observation": 1 }] }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test(start_paused = true)]
async fn the_list_gives_title_tags_projects_round_and_handled_and_leaves_out_a_deleted_one() {
    let running = planned().await;
    let router = &running.router;
    let made = changing(
        &running,
        "Sorting what the plan found.",
        json!([
            { "kind": "title", "observation": 1, "title": "Test the greeting" },
            { "kind": "tag", "observation": 1, "tag": "tests" },
            { "kind": "takenIn", "observation": 1, "item": 1 },
            { "kind": "delete", "observation": 2 },
        ]),
    )
    .await;
    assert_eq!(made, (StatusCode::OK, json!({ "changed": [true, true, true, true] })));
    // The item that takes it in has merged by now: an item takes an observation in whatever it is doing.
    let mut expected = of_the_plan(1, "The greeting has no test.", Value::Null);
    expected["title"] = json!("Test the greeting");
    expected["tags"] = json!(["tests"]);
    expected["takenInBy"] = json!(1);
    expected["handled"] = json!(true);
    assert_eq!(
        get_json(router, "/api/v1/observations").await,
        json!({ "observations": [expected] })
    );
    // A deleted observation takes no change.
    let again = changing(
        &running,
        "Again.",
        json!([{ "kind": "tag", "observation": 2, "tag": "flow" }]),
    )
    .await;
    assert_eq!(again.0, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(again.1["error"].as_str().unwrap().contains("deleted"), "{again:?}");
}

#[tokio::test(start_paused = true)]
async fn the_tags_route_counts_observations() {
    let running = planned().await;
    let router = &running.router;
    assert_eq!(
        get_json(router, "/api/v1/observations/tags").await,
        json!({ "tags": [] })
    );
    assert_eq!(
        post(router, "/api/v1/observations", &architects("trial", "trial"))
            .await
            .0,
        StatusCode::OK
    );
    let made = changing(
        &running,
        "Sorting.",
        json!([
            { "kind": "tag", "observation": 1, "tag": "tests" },
            { "kind": "takenIn", "observation": 1, "item": 1 },
        ]),
    )
    .await;
    assert_eq!(made.0, StatusCode::OK, "{made:?}");
    // `tests` is carried by O1, which is handled, and by O3, which is not; `flow` only by O3.
    assert_eq!(
        get_json(router, "/api/v1/observations/tags").await,
        json!({ "tags": [
            { "tag": "flow", "carrying": 1, "unhandled": 1 },
            { "tag": "tests", "carrying": 2, "unhandled": 1 },
        ] })
    );
}
