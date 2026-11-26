//! `GET /api/v1/attention`: what needs whoever plans the work, what came since a mark at once and what stands whole
//! again once the time set for it has passed, answered at once or held open until there is something or the time asked
//! for has passed; and what it counts, as the page counts it.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use axum::http::StatusCode;
use serde_json::{Value, json};
use service::{Along, Round, RoundReply, Rounds, Scripted};
use store::Store;

use crate::{
    START, edit, get_json, greeted, pass, post, refused, running, running_flow, running_scripted, times, trial_dir,
    trial_flow, trial_script,
};

/// The trial item's line once its plan's run reached its limit of returns and called the owner, call 2, five and a half
/// seconds in.
fn asked() -> Value {
    json!({
        "kind": "forAPerson",
        "of": { "work": 1, "item": 1, "title": "Greet the user" },
        "since": START + 5500,
        "now": {
            "kind": "waits",
            "at": "plan",
            "on": [{
                "kind": "call",
                "call": 2,
                "who": "owner",
                "what": { "kind": "atLimit", "limit": { "of": "returns", "sentBack": 2 } },
                "made": START + 5500,
                "at": null,
                "queuedFor": null,
                "held": null,
            }],
        },
        // What it waits on the owner for, as the engine's one list of it says.
        "waits": [{
            "key": "limit",
            "why": "The cell's run had 2 returns sent back, its limit.",
            "ends": { "kind": "goOn", "call": 2 },
        }],
    })
}

/// The trial item standing at a root, which it does from when it was added: its flow has an agent's cell, so it is no
/// deliverable, and no item depends on it.
fn rooted() -> Value {
    json!({ "kind": "root", "of": { "work": 1, "item": 1, "title": "Greet the user" }, "since": START })
}

/// An item of the trial flow, added held with no reason, depending on these items.
fn item(title: &str, on: Value) -> Value {
    json!({
        "title": title,
        "dependsOn": on,
        "project": "trial",
        "flow": "trial",
        "writes": [{ "block": "request", "content": "Please.", "value": null }],
    })
}

/// The kinds of the answer's lines, in their order.
fn kinds(answer: &Value) -> Vec<String> {
    (answer["lines"].as_array().unwrap().iter())
        .map(|line| line["kind"].as_str().unwrap().to_string())
        .collect()
}

/// The answer's mark.
fn mark(answer: &Value) -> String {
    answer["mark"].as_str().unwrap().to_string()
}

#[tokio::test(start_paused = true)]
async fn what_stands_is_given_whole_with_no_mark_and_nothing_again_from_the_mark_given_with_it() {
    let running = running();
    let router = &running.router;
    greeted(router).await;
    let answer = get_json(router, "/api/v1/attention").await;
    assert_eq!(answer["lines"], json!([asked(), rooted()]));
    assert_eq!((&answer["whole"], &answer["leftOut"]), (&json!(true), &json!(0)));
    assert_eq!(answer["again"], json!(times().again));
    let again = get_json(router, &format!("/api/v1/attention?since={}", mark(&answer))).await;
    assert_eq!(again["lines"], json!([]), "{again}");
    assert_eq!(again["whole"], json!(false));
}

// What stands and is not handled is given whole again once the time set for it has passed since it was last given
// whole, and not before; a request held open with the mark is answered as it comes due. Fails where what stands is
// given once and forgotten, or given again before its time. Paused time; the set time is 5 minutes.
#[tokio::test(start_paused = true)]
async fn what_stands_is_given_again_once_the_time_set_for_it_has_passed_and_not_before() {
    let running = running();
    let router = running.router.clone();
    greeted(&router).await;
    let first = get_json(&router, "/api/v1/attention").await;
    assert_eq!(first["lines"], json!([asked(), rooted()]));
    pass(times().again - 1000).await;
    let early = get_json(&router, &format!("/api/v1/attention?since={}", mark(&first))).await;
    assert_eq!(early["lines"], json!([]), "given again before its time: {early}");
    pass(1000).await;
    let due = get_json(&router, &format!("/api/v1/attention?since={}", mark(&early))).await;
    assert_eq!(due["whole"], json!(true), "{due}");
    assert_eq!(due["lines"], json!([asked(), rooted()]));
    // Held open, the request is answered as what stands comes due again, though nothing happens.
    let since = mark(&due);
    let held =
        tokio::spawn(async move { get_json(&router, &format!("/api/v1/attention?since={since}&wait=3600")).await });
    pass(times().again - 1000).await;
    assert!(!held.is_finished(), "answered before what stands came due");
    pass(2000).await;
    let again = held.await.unwrap();
    assert_eq!(again["whole"], json!(true), "{again}");
    assert_eq!(again["lines"], json!([asked(), rooted()]));
}

// What `wait` says waits on the owner is what the page counts in red, thing by thing and work by work, and the
// questions it says the owner may answer ahead are as many as the page counts in orange, read from the same store at
// once. Fails where a wait the page counts is left out of what `wait` gives, or one is given that the page does not
// count. The trial's script asks the owner no question of a cell's own, so that the count in orange is nought on both
// sides.
#[tokio::test(start_paused = true)]
async fn what_wait_gives_as_waiting_on_the_owner_is_what_the_page_counts() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("store.sqlite")).unwrap();
    let mut script = trial_script();
    let plan = script.flows.get_mut("trial").unwrap();
    let mut failing = plan.rounds[0].clone();
    failing.round = 4;
    failing.asks = None;
    failing.took = Default::default();
    failing.fails = Some("The agent's program could not start.".to_string());
    plan.rounds.push(failing);
    let running = running_scripted(Box::new(store), dir, script);
    let router = &running.router;
    greeted(router).await;
    // A second item waits at its limit as well, and the first, let go on, fails its round and is paused by the engine.
    assert_eq!(
        post(router, "/api/v1/items", &item("Second", json!([]))).await.0,
        StatusCode::OK
    );
    let resume = json!({ "kind": "resume" });
    assert_eq!(
        post(router, "/api/v1/works/2/acts", &resume).await.0,
        StatusCode::NO_CONTENT
    );
    let go_on = json!({ "kind": "goOn", "call": 2 });
    assert_eq!(
        post(router, "/api/v1/works/1/acts", &go_on).await.0,
        StatusCode::NO_CONTENT
    );
    pass(6000).await;
    let items = get_json(router, "/api/v1/items").await;
    let mut counted: BTreeMap<u64, Vec<Value>> = BTreeMap::new();
    let mut ahead = 0;
    for item in items["items"].as_array().unwrap() {
        let work = &item["work"];
        let waits = work["waits"].as_array().unwrap();
        if !waits.is_empty() {
            counted.insert(work["number"].as_u64().unwrap(), waits.clone());
        }
        if !matches!(work["now"]["kind"].as_str(), Some("merged" | "cancelled")) {
            ahead += work["ahead"].as_u64().unwrap();
        }
    }
    assert_eq!(counted.len(), 2, "both works wait on the owner: {items}");
    let answer = get_json(router, "/api/v1/attention").await;
    assert_eq!(answer["leftOut"], json!(0));
    let lines = answer["lines"].as_array().unwrap();
    let given: BTreeMap<u64, Vec<Value>> = (lines.iter())
        .filter(|line| line["kind"] == "forAPerson")
        .map(|line| {
            (
                line["of"]["work"].as_u64().unwrap(),
                line["waits"].as_array().unwrap().clone(),
            )
        })
        .collect();
    assert_eq!(given, counted);
    let asked_ahead: usize = (lines.iter())
        .filter(|line| line["kind"] == "ahead")
        .map(|line| line["questions"].as_array().unwrap().len())
        .sum();
    assert_eq!(asked_ahead as u64, ahead);
}

// A work the owner paused and said no reason for, an item that stands at a root and is no deliverable, and slots free
// while items could be let on stand to be handled, each since it came to; a pause that says why is none, an item an
// open item depends on stands at no root, and an item that waits for one that has not ended is none to let on.
#[tokio::test(start_paused = true)]
async fn a_pause_with_no_reason_an_item_at_a_root_and_slots_free_while_items_could_be_let_on_are_given() {
    let running = running();
    let router = &running.router;
    assert_eq!(
        post(router, "/api/v1/items", &item("Base", json!([]))).await.0,
        StatusCode::OK
    );
    pass(1000).await;
    assert_eq!(
        post(router, "/api/v1/items", &item("After", json!([1]))).await.0,
        StatusCode::OK
    );
    pass(500).await;
    let answer = get_json(router, "/api/v1/attention").await;
    assert_eq!(
        kinds(&answer),
        ["unexplained", "unexplained", "slots", "root"],
        "{answer}"
    );
    let lines = &answer["lines"];
    assert_eq!(
        (&lines[0]["of"]["work"], &lines[0]["since"]),
        (&json!(1), &json!(START))
    );
    assert_eq!(
        (&lines[1]["of"]["work"], &lines[1]["since"]),
        (&json!(2), &json!(START + 1000))
    );
    assert_eq!(
        (&lines[2]["free"], &lines[2]["items"], &lines[2]["since"]),
        (&json!(2), &json!(1), &json!(START)),
        "the base alone could be let on"
    );
    assert_eq!(
        (&lines[3]["of"]["work"], &lines[3]["since"]),
        (&json!(2), &json!(START + 1000))
    );
    // Paused again with a reason, the base stands no more, and is still to be let on from when it was first paused:
    // what came since the mark is only that the work after it waits for it, paused anew, which is said once.
    let marked = mark(&answer);
    pass(500).await;
    let why = json!({ "kind": "pause", "why": "The owner says when." });
    assert_eq!(
        post(router, "/api/v1/works/1/acts", &why).await.0,
        StatusCode::NO_CONTENT
    );
    pass(500).await;
    let whole = get_json(router, "/api/v1/attention").await;
    assert_eq!(kinds(&whole), ["unexplained", "slots", "root"], "{whole}");
    let since = get_json(router, &format!("/api/v1/attention?since={marked}")).await;
    assert_eq!(kinds(&since), ["idle"], "{since}");
    assert_eq!(
        (&since["lines"][0]["of"]["work"], &since["lines"][0]["since"]),
        (&json!(2), &json!(START + 2000))
    );
    let after = get_json(router, &format!("/api/v1/attention?since={}", mark(&since))).await;
    assert_eq!(kinds(&after), Vec::<String>::new(), "{after}");
}

/// The trial's acts, each of whose rounds is said to wait for the runner `14900k` from as it begins, which no record of
/// the work tells.
struct Queueing {
    acts: Arc<dyn Rounds>,
}

impl Rounds for Queueing {
    fn give(&self, round: Round, reply: RoundReply) {
        let saying = reply.saying(round.number);
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            saying.say(Some(Along::Runs {
                at: "https://ci.example/runs/9".to_string(),
                queued_for: Some("14900k".to_string()),
            }));
        });
        self.acts.give(round, reply);
    }

    fn stop(&self, round: Round, reply: RoundReply) {
        self.acts.stop(round, reply);
    }
}

// A work whose merge takes its project's turn, its act waiting for a runner for forty minutes while another work waits
// for the turn: the round waiting for the runner is given once it has waited the set time, and the turn once the act
// has been under way the set time, each not before, and each says since when what it is about began. Paused time, a
// minute at a step; the set times are 15 and 30 minutes.
#[tokio::test(start_paused = true)]
async fn a_round_waiting_for_a_runner_and_a_turn_held_while_another_waits_are_given_once_they_have_lasted_the_set_time()
{
    let taking = edit(
        trial_flow(),
        Some("merge"),
        "outputs = [\"merged\"]",
        "outputs = [\"merged\"]\ntakes_turn = true",
    );
    let text = std::fs::read_to_string(trial_dir().join("script.toml")).unwrap();
    let long = text.replacen("took = { ms = 500 }", "took = { ms = 2400000 }", 1);
    assert_ne!(long, text, "the script's merge takes half a second");
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("store.sqlite")).unwrap();
    let mut carriers = Scripted::new(service::Script::read(&long).unwrap()).carriers();
    carriers.acts = Arc::new(Queueing {
        acts: carriers.acts.clone(),
    });
    let began = tokio::time::Instant::now();
    let running = running_flow(Box::new(store), dir, carriers, &taking);
    let router = &running.router;
    for title in ["First", "Second"] {
        assert_eq!(
            post(router, "/api/v1/items", &item(title, json!([]))).await.0,
            StatusCode::OK
        );
    }
    for work in 1..=2 {
        let resume = json!({ "kind": "resume" });
        assert_eq!(
            post(router, &format!("/api/v1/works/{work}/acts"), &resume).await.0,
            StatusCode::NO_CONTENT
        );
    }
    pass(6000).await;
    for work in 1..=2 {
        let go_on = json!({ "kind": "goOn", "call": 2 });
        assert_eq!(
            post(router, &format!("/api/v1/works/{work}/acts"), &go_on).await.0,
            StatusCode::NO_CONTENT
        );
    }
    // Until one work holds the turn with its merge and the other waits for it.
    let mut holding = None;
    for _ in 0..400 {
        pass(250).await;
        let items = get_json(router, "/api/v1/items").await;
        for item in items["items"].as_array().unwrap() {
            for waited in item["work"]["now"]["on"].as_array().into_iter().flatten() {
                if waited["kind"] == "turn" {
                    holding = waited["holder"].as_u64();
                }
            }
        }
        if holding.is_some() {
            break;
        }
    }
    let holder = holding.expect("a work came to wait for the turn of the other");
    let summary = get_json(router, &format!("/api/v1/works/{holder}")).await["summary"].clone();
    assert_eq!(
        (&summary["now"]["kind"], &summary["now"]["cell"]),
        (&json!("round"), &json!("merge")),
        "{summary}"
    );
    let under_way = summary["now"]["since"].as_u64().unwrap();
    let of = |answer: &Value, kind: &str| -> Vec<Value> {
        (answer["lines"].as_array().unwrap().iter())
            .filter(|line| line["kind"] == kind)
            .cloned()
            .collect()
    };
    let (mut queued, mut turned) = (false, false);
    for _ in 0..40 {
        pass(60_000).await;
        let now = START + began.elapsed().as_millis() as u64;
        let answer = get_json(router, "/api/v1/attention").await;
        let (runner, turn) = (of(&answer, "queued"), of(&answer, "turn"));
        // The act said it waits for the runner ten milliseconds after it began.
        if now < under_way + 10 + times().queued {
            assert_eq!(runner, Vec::<Value>::new(), "given before its time, at {now}");
        } else {
            assert_eq!(runner.len(), 1, "{answer}");
            assert_eq!(
                (&runner[0]["of"]["work"], &runner[0]["runner"]),
                (&json!(holder), &json!("14900k"))
            );
            assert_eq!(runner[0]["since"], json!(under_way + 10));
            queued = true;
        }
        if now < under_way + times().turn {
            assert_eq!(turn, Vec::<Value>::new(), "given before its time, at {now}");
        } else {
            assert_eq!(turn.len(), 1, "{answer}");
            assert_eq!(
                (&turn[0]["holder"]["work"], &turn[0]["since"], &turn[0]["project"]),
                (&json!(holder), &json!(under_way), &json!("trial"))
            );
            assert_eq!(turn[0]["waiting"].as_array().unwrap().len(), 1);
            turned = true;
            break;
        }
    }
    assert!(queued && turned, "the round and the turn were each given");
}

#[tokio::test(start_paused = true)]
async fn a_work_the_engine_paused_as_its_round_failed_is_given_with_the_reason_since_it_failed() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("store.sqlite")).unwrap();
    let mut script = trial_script();
    let plan = script.flows.get_mut("trial").unwrap();
    let mut failing = plan.rounds[0].clone();
    failing.round = 4;
    failing.asks = None;
    failing.took = Default::default();
    failing.fails = Some("The agent's program could not start.".to_string());
    plan.rounds.push(failing);
    let running = running_scripted(Box::new(store), dir, script);
    let router = &running.router;
    greeted(router).await;
    let marked = mark(&get_json(router, "/api/v1/attention").await);
    let answer = json!({ "kind": "goOn", "call": 2 });
    assert_eq!(
        post(router, "/api/v1/works/1/acts", &answer).await.0,
        StatusCode::NO_CONTENT
    );
    // The plan's fourth round, begun as the owner let its run go on at 6 seconds, fails a second and a half later.
    pass(2000).await;
    let answer = get_json(router, &format!("/api/v1/attention?since={marked}")).await;
    assert_eq!(
        answer["lines"],
        json!([{
            "kind": "forAPerson",
            "of": { "work": 1, "item": 1, "title": "Greet the user" },
            "since": START + 7500,
            "now": { "kind": "paused", "at": "plan", "by": "engine", "why": "The round failed. The cause: The agent's program could not start." },
            "waits": [{
                "key": "pause",
                "why": "The engine paused the work. The round failed. The cause: The agent's program could not start.",
                "ends": { "kind": "resume" },
            }],
        }])
    );
}

#[tokio::test(start_paused = true)]
async fn a_request_held_open_is_answered_as_soon_as_something_comes() {
    let running = running();
    let router = running.router.clone();
    assert_eq!(
        post(&router, "/api/v1/items", &item("Greet the user", json!([])))
            .await
            .0,
        StatusCode::OK
    );
    pass(1000).await;
    let resumed = post(&router, "/api/v1/works/1/acts", &json!({ "kind": "resume" })).await;
    assert_eq!(resumed.0, StatusCode::NO_CONTENT);
    let marked = mark(&get_json(&router, "/api/v1/attention").await);
    let held =
        tokio::spawn(async move { get_json(&router, &format!("/api/v1/attention?since={marked}&wait=600")).await });
    pass(1000).await;
    assert!(!held.is_finished(), "answered while the plan's rounds were under way");
    pass(4000).await;
    let answer = held.await.unwrap();
    assert_eq!(kinds(&answer), ["forAPerson"], "{answer}");
    assert_eq!(answer["lines"][0]["of"]["work"], json!(1));
}

// A request held open with no mark, nothing standing as it was asked, gives what came while it was held, from the time
// it was asked.
#[tokio::test(start_paused = true)]
async fn a_request_held_open_with_no_mark_gives_what_came_while_it_was_held() {
    let running = running();
    let router = running.router.clone();
    let asking = router.clone();
    let held = tokio::spawn(async move { get_json(&asking, "/api/v1/attention?wait=600").await });
    pass(1000).await;
    assert!(!held.is_finished(), "answered with nothing standing");
    assert_eq!(
        post(&router, "/api/v1/items", &item("Greet the user", json!([])))
            .await
            .0,
        StatusCode::OK
    );
    pass(1000).await;
    assert!(held.is_finished(), "what came was not given");
    let answer = held.await.unwrap();
    assert_eq!(answer["whole"], json!(false), "{answer}");
    assert_eq!(kinds(&answer), ["unexplained", "slots", "root"], "{answer}");
}

// A work that ended while a request was held is given, once.
#[tokio::test(start_paused = true)]
async fn a_work_that_ended_while_a_request_was_held_is_given_once() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("store.sqlite")).unwrap();
    let mut script = trial_script();
    // The plan's first round hands in the plan, so that nothing of the work is for a person before it merges.
    script.flows.get_mut("trial").unwrap().rounds[0].asks = None;
    let running = running_scripted(Box::new(store), dir, script);
    let router = running.router.clone();
    assert_eq!(
        post(&router, "/api/v1/items", &item("Greet the user", json!([])))
            .await
            .0,
        StatusCode::OK
    );
    let resumed = post(&router, "/api/v1/works/1/acts", &json!({ "kind": "resume" })).await;
    assert_eq!(resumed.0, StatusCode::NO_CONTENT);
    pass(500).await;
    let marked = mark(&get_json(&router, "/api/v1/attention").await);
    let asking = router.clone();
    let held =
        tokio::spawn(async move { get_json(&asking, &format!("/api/v1/attention?since={marked}&wait=600")).await });
    pass(1000).await;
    assert!(!held.is_finished(), "answered while the work was going on");
    pass(60_000).await;
    assert!(held.is_finished(), "the work never merged, or its merge was not given");
    let answer = held.await.unwrap();
    assert_eq!(kinds(&answer), ["ended"], "{answer}");
    assert_eq!(answer["lines"][0]["now"], json!({ "kind": "merged" }), "{answer}");
    let after = get_json(&router, &format!("/api/v1/attention?since={}", mark(&answer))).await;
    assert_eq!(kinds(&after), Vec::<String>::new(), "{after}");
}

#[tokio::test(start_paused = true)]
async fn a_request_held_open_with_nothing_is_answered_with_no_line_once_its_time_has_passed() {
    let running = running();
    let router = running.router.clone();
    let held = tokio::spawn(async move { get_json(&router, "/api/v1/attention?wait=5").await });
    pass(4900).await;
    assert!(!held.is_finished(), "answered before its time");
    pass(200).await;
    let answer = held.await.unwrap();
    assert_eq!(
        (&answer["whole"], &answer["lines"], &answer["leftOut"]),
        (&json!(true), &json!([]), &json!(0)),
        "{answer}"
    );
}

#[tokio::test(start_paused = true)]
async fn a_mark_that_cannot_be_read_or_is_later_than_the_engines_clock_is_refused() {
    let running = running();
    let router = &running.router;
    greeted(router).await;
    let (status, words) = refused(router, "/api/v1/attention?since=nonsense").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{words}");
    let (status, words) = refused(router, "/api/v1/attention?since=7").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{words}");
    let later = START + 86_400_000;
    let (status, words) = refused(router, &format!("/api/v1/attention?since={later}.{later}.0")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{words}");
    assert!(words.contains("later than the engine's clock"), "{words}");
    let (status, words) = refused(router, &format!("/api/v1/attention?wait={}", u64::MAX)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{words}");
    let (status, words) = refused(router, "/api/v1/attention?after=1").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{words}");
}
