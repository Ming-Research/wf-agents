//! What a work does while the engine carries out something for it that takes its time, a tool's call or an act of
//! its own, each where it stands as what carries it out says; and what its time bar says of the stretches in which
//! nothing ran.

use axum::http::StatusCode;
use serde_json::{Value, json};
use store::Store;

use crate::{START, get_json, greeted, pass, post, running, running_scripted, trial_script};

const ACTS: &str = "/api/v1/works/1/acts";
/// The links of the CI runs the tool and the merge's act say they run as.
const CI: &str = "https://github.example/actions/runs/11";
const MERGE: &str = "https://github.example/actions/runs/12";

/// The work's summary once this holds of it, the paused clock let on a quarter of a second at a time; it fails where
/// it does not hold within a minute.
async fn until(router: &axum::Router, holds: impl Fn(&Value) -> bool) -> Value {
    for _ in 0..240 {
        let summary = get_json(router, "/api/v1/works/1").await["summary"].clone();
        if holds(&summary) {
            return summary;
        }
        pass(250).await;
    }
    panic!("the work did not come to it within a minute");
}

/// The state of this cell in the summary.
fn state<'a>(summary: &'a Value, cell: &str) -> &'a Value {
    let cells = summary["cells"].as_array().unwrap();
    &cells.iter().find(|c| c["cell"] == cell).unwrap()["state"]
}

#[tokio::test(start_paused = true)]
async fn a_tools_call_and_an_act_under_way_are_calls_where_they_say_they_run() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("store.sqlite")).unwrap();
    let mut script = trial_script();
    script.tools.get_mut("ci").unwrap().at = Some(CI.to_string());
    let rounds = &mut script.flows.get_mut("trial").unwrap().rounds;
    let merge = rounds.iter_mut().find(|r| r.cell == "merge").unwrap();
    merge.took.ms = Some(4000);
    merge.at = Some(MERGE.to_string());
    let running = running_scripted(Box::new(store), dir, script);
    let router = &running.router;
    greeted(router).await;
    let answer = json!({ "kind": "result", "call": 2, "result": "Yes." });
    assert_eq!(post(router, ACTS, &answer).await.0, StatusCode::NO_CONTENT);

    // The build's call of the CI runs as the run it says, since it was made; the build is a cell with a call under way.
    let called = until(router, |s| s["now"]["on"][0]["what"]["kind"] == "tool").await;
    let made = called["now"]["on"][0]["made"].as_u64().unwrap();
    assert!(made > START, "{called}");
    assert_eq!(
        called["now"],
        json!({
            "kind": "waits",
            "at": "build",
            "on": [{
                "kind": "call",
                "call": 4,
                "who": null,
                "what": { "kind": "tool", "name": "ci", "input": "the work's branch" },
                "made": made,
                "at": CI,
            }],
        })
    );
    assert_eq!(state(&called, "build"), "calling");

    // The merge's act runs as the CI run it says: a round under way, the cell's call.
    let runs = until(router, |s| s["now"]["kind"] == "round").await;
    assert_eq!(runs["now"]["cell"], "merge");
    assert_eq!(runs["now"]["at"], MERGE);
    assert_eq!(state(&runs, "merge"), "calling");

    // Once it has ended, nothing is said of where it stood.
    let merged = until(router, |s| s["now"]["kind"] == "merged").await;
    assert_eq!(state(&merged, "merge"), "done");
}

/// The bar's characters in order, each run of one character said once.
fn runs_of(bar: &str) -> String {
    let mut said = String::new();
    for c in bar.chars() {
        if !said.ends_with(c) {
            said.push(c);
        }
    }
    said
}

#[tokio::test(start_paused = true)]
async fn a_stretch_of_the_bar_nothing_ran_in_says_what_the_work_waited_for() {
    let running = running();
    let router = &running.router;
    greeted(router).await;
    // The plan's run waits at its limit for the owner half a minute; let go on, its next round begins, and the work is
    // paused under it, which holds the build half a minute more.
    pass(30_000).await;
    let answer = json!({ "kind": "result", "call": 2, "result": "Yes." });
    assert_eq!(post(router, ACTS, &answer).await.0, StatusCode::NO_CONTENT);
    assert_eq!(
        post(router, ACTS, &json!({ "kind": "pause" })).await.0,
        StatusCode::NO_CONTENT
    );
    pass(30_000).await;
    assert_eq!(
        post(router, ACTS, &json!({ "kind": "resume" })).await.0,
        StatusCode::NO_CONTENT
    );
    pass(1000).await;
    let bar = get_json(router, "/api/v1/works/1").await["summary"]["bar"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(bar.chars().count(), 96, "{bar}");
    // A model's rounds, the wait for the owner, the plan's next round, the pause, and the build's round.
    assert_eq!(runs_of(&bar), "mompm", "{bar}");
}
