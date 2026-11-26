//! `POST /api/v1/works/{work}/acts`: what the owner does to a work, each as the engine takes it, or refused with why.

use axum::http::StatusCode;
use serde_json::{Value, json};
use service::Scripted;
use store::Store;

use crate::{
    edit, get_json, greeted, pass, post, post_text, running, running_compiled, running_scripted, trial_flow,
    trial_has_with_words, trial_script,
};

const ACTS: &str = "/api/v1/works/1/acts";

#[tokio::test(start_paused = true)]
async fn each_act_is_taken_as_what_happened_to_the_work() {
    let running = running();
    let router = &running.router;
    greeted(router).await;
    let done = (StatusCode::NO_CONTENT, Value::Null);
    assert_eq!(post(router, ACTS, &json!({ "kind": "pause" })).await, done);
    assert_eq!(get_json(router, "/api/v1/works/1").await["summary"]["paused"], "owner");
    assert_eq!(post(router, ACTS, &json!({ "kind": "resume" })).await, done);
    let opinion = json!({ "kind": "write", "block": "opinions", "content": "Keep it short.", "value": null });
    assert_eq!(post(router, ACTS, &opinion).await, done);
    let page = get_json(router, "/api/v1/works/1").await;
    let kinds: Vec<&str> = (page["steps"].as_array().unwrap().iter())
        .map(|s| s["kind"].as_str().unwrap())
        .collect();
    assert_eq!(
        kinds,
        [
            "paused", "wrote", "wrote", "resumed", "run", "paused", "resumed", "wrote"
        ]
    );
    let opinions = &page["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["name"] == "opinions")
        .unwrap()["versions"];
    assert_eq!(opinions[0]["by"], json!({ "kind": "owner" }));

    assert_eq!(post(router, ACTS, &json!({ "kind": "cancel" })).await, done);
    assert_eq!(
        get_json(router, "/api/v1/works/1").await["summary"]["now"],
        json!({ "kind": "cancelled" })
    );
}

#[tokio::test(start_paused = true)]
async fn an_act_that_cannot_be_taken_is_refused_with_why() {
    let running = running();
    let router = &running.router;
    greeted(router).await;
    let error = |words: &str| json!({ "error": words });
    let unprocessable = StatusCode::UNPROCESSABLE_ENTITY;
    let answer = |call: u64| json!({ "kind": "result", "call": call, "result": "Yes." });
    let go_on = |call: u64| json!({ "kind": "goOn", "call": call });
    assert_eq!(
        post(router, "/api/v1/works/9/acts", &json!({ "kind": "pause" })).await,
        (StatusCode::NOT_FOUND, error("There is no work W9."))
    );
    assert_eq!(
        post(router, ACTS, &answer(9)).await,
        (unprocessable, error("The work has no call 9."))
    );
    let write = json!({ "kind": "write", "block": "code", "content": "fn main() {}", "value": null });
    assert_eq!(
        post(router, ACTS, &write).await,
        (unprocessable, error("People do not write the block code."))
    );
    assert_eq!(
        post(router, ACTS, &json!({ "kind": "letGo", "cell": "deploy" })).await,
        (unprocessable, error("The flow has no cell deploy."))
    );
    // The work waits for the owner's call at the plan's run, and at no cell past its limit: no cell is let go on.
    assert_eq!(
        post(router, ACTS, &json!({ "kind": "letGo", "cell": "build" })).await,
        (
            unprocessable,
            error(
                "The cell build is not at its limit: a cell is let go on only where the work waits at it for the \
                 owner past its limit."
            )
        )
    );
    for act in [
        r#"{ "kind": "jump" }"#,
        r#"{ "kind": "pause", "at": 3 }"#,
        r#"{ "kind": "write", "block": "opinions", "content": "Short.", "value": null, "for": "build" }"#,
    ] {
        let (status, json) = post_text(router, ACTS, act).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{act}");
        assert!(
            json["error"].as_str().unwrap().starts_with("The act cannot be read: "),
            "{json}"
        );
    }
    let (status, json) = post_text(router, ACTS, r#"{ "kind": "jump" }"#).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        json["error"].as_str().unwrap().starts_with("The act cannot be read: "),
        "{json}"
    );

    // The plan's run at its limit is let go on, and takes no words of a person as its result.
    assert_eq!(
        post(router, ACTS, &answer(2)).await,
        (
            unprocessable,
            error("Call 2 is a run at its limit: it is let go on and takes no words, and words are said to its cell.")
        )
    );
    assert_eq!(post(router, ACTS, &go_on(2)).await.0, StatusCode::NO_CONTENT);
    assert_eq!(
        post(router, ACTS, &go_on(2)).await,
        (unprocessable, error("Call 2 has its result already."))
    );
    // The plan hands in and the build asks for the CI: a tool's call, which no person answers and is no run at a limit.
    pass(4000).await;
    assert_eq!(
        post(router, ACTS, &answer(4)).await,
        (
            unprocessable,
            error("Call 4 is a tool's: the tool gives its result, not a person.")
        )
    );
    assert_eq!(
        post(router, ACTS, &go_on(4)).await,
        (
            unprocessable,
            error("Call 4 is no run at its limit, which is all that is let go on.")
        )
    );
}

/// The trial's script with the review objecting to each of the first four builds: the build goes round three times,
/// which is its limit, and the work waits at it for the owner.
fn objecting() -> service::Script {
    let mut script = trial_script();
    let rounds = &mut script.flows.get_mut("trial").unwrap().rounds;
    let objects = (rounds.iter())
        .find(|r| r.cell == "review" && r.run == 1)
        .unwrap()
        .clone();
    rounds.retain(|r| r.cell != "review" || r.run == 1);
    for run in 2..=4 {
        let mut again = objects.clone();
        again.run = run;
        for hand_in in again.hands_in.iter_mut().flatten() {
            if hand_in.block == "opinions" {
                hand_in.content = Some(format!("Not yet, as review {run} finds."));
            }
        }
        rounds.push(again);
    }
    script
}

// A cell the work waits at past its limit is let go on, as the owner's wait at it says, and the let-go is one of the
// work's steps.
#[tokio::test(start_paused = true)]
async fn a_cell_at_its_limit_is_let_go_on_and_shown_among_the_steps() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("store.sqlite")).unwrap();
    let running = running_scripted(Box::new(store), dir, objecting());
    let router = &running.router;
    greeted(router).await;
    let done = (StatusCode::NO_CONTENT, Value::Null);
    assert_eq!(post(router, ACTS, &json!({ "kind": "goOn", "call": 2 })).await, done);
    let let_go = json!({ "kind": "letGo", "cell": "build" });
    let waits_at_the_build =
        |page: &Value| (page["summary"]["waits"].as_array().unwrap().iter()).any(|w| w["ends"] == let_go);
    let mut waited = false;
    for _ in 0..240 {
        if waits_at_the_build(&get_json(router, "/api/v1/works/1").await) {
            waited = true;
            break;
        }
        pass(250).await;
    }
    assert!(
        waited,
        "the work did not come to wait at the build past its limit within a minute"
    );
    assert_eq!(post(router, ACTS, &let_go).await, done);
    let page = get_json(router, "/api/v1/works/1").await;
    assert!(!waits_at_the_build(&page), "{}", page["summary"]["waits"]);
    let kinds: Vec<&str> = (page["steps"].as_array().unwrap().iter())
        .map(|s| s["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds.iter().filter(|k| **k == "letGo").count(), 1, "{kinds:?}");
}

// What the owner says to a cell or a line, and the option the owner takes for a question, are acts of the route as the
// others are: each is read, handed to the engine with what it names, and refused in words that name it where nothing
// takes it. The trial flow has no question and no cell with words of its own, so each is refused here, and each refusal
// says the line, the cell or the question the act named. An answer is an option and nothing else, a reason with it is
// not an act.
#[tokio::test(start_paused = true)]
async fn words_said_and_an_answer_given_are_taken_as_acts_and_refused_naming_what_they_were_given_to() {
    let running = running();
    let router = &running.router;
    greeted(router).await;
    let unprocessable = StatusCode::UNPROCESSABLE_ENTITY;
    let say = |to: Value| json!({ "kind": "say", "to": to, "words": "Keep it short." });
    let refused = |words: &str| (unprocessable, json!({ "error": words }));
    assert_eq!(
        post(router, ACTS, &say(json!({ "kind": "line", "line": "drafter" }))).await,
        refused("The flow has no cell on the line drafter.")
    );
    assert_eq!(
        post(router, ACTS, &say(json!({ "kind": "cell", "cell": "deploy" }))).await,
        refused("The flow has no cell deploy.")
    );
    assert_eq!(
        post(router, ACTS, &say(json!({ "kind": "line", "line": "engineer" }))).await,
        refused("No cell of the line engineer has words of its own.")
    );
    assert_eq!(
        post(router, ACTS, &say(json!({ "kind": "cell", "cell": "build" }))).await,
        refused("The words would reach build, which has no words of its own.")
    );
    let answer = json!({ "kind": "answer", "question": "check", "version": 1, "option": 2 });
    assert_eq!(
        post(router, ACTS, &answer).await,
        refused("The work has no question check.")
    );
    let with_a_reason =
        json!({ "kind": "answer", "question": "check", "version": 1, "option": 2, "why": "It is short." });
    let (status, json) = post(router, ACTS, &with_a_reason).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{json}");
    assert!(
        json["error"].as_str().unwrap().starts_with("The act cannot be read: "),
        "{json}"
    );
}

// What the owner has read of a work is an act of the route as the others are: it names versions of the work's blocks by
// their numbers, and is taken once the work holds each of them, and refused in words that name the first it does not,
// with nothing kept of the versions beside it. The page of the work tells of each version whether it was read.
#[tokio::test(start_paused = true)]
async fn a_read_is_taken_for_the_versions_the_work_holds_and_refused_naming_one_it_does_not() {
    let running = running();
    let router = &running.router;
    greeted(router).await;
    let done = (StatusCode::NO_CONTENT, Value::Null);
    let opinion = json!({ "kind": "write", "block": "opinions", "content": "Keep it short.", "value": null });
    assert_eq!(post(router, ACTS, &opinion).await, done);
    let read_of = |page: &Value| {
        page["blocks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|b| b["name"] == "opinions")
            .unwrap()["versions"][0]["read"]
            .clone()
    };
    let read = |versions: Value| json!({ "kind": "read", "versions": versions });
    assert_eq!(read_of(&get_json(router, "/api/v1/works/1").await), json!(false));
    let both = read(json!([{ "block": "opinions", "version": 1 }, { "block": "opinions", "version": 7 }]));
    assert_eq!(
        post(router, ACTS, &both).await,
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            json!({ "error": "The work has no version 7 of the block opinions." })
        )
    );
    assert_eq!(
        read_of(&get_json(router, "/api/v1/works/1").await),
        json!(false),
        "a read that is refused keeps nothing"
    );
    let one = read(json!([{ "block": "opinions", "version": 1 }]));
    assert_eq!(post(router, ACTS, &one).await, done);
    assert_eq!(read_of(&get_json(router, "/api/v1/works/1").await), json!(true));
    // Read again, it changes nothing and is no refusal.
    assert_eq!(post(router, ACTS, &one).await, done);
    let unknown = read(json!([{ "block": "nothing_here", "version": 1 }]));
    let (status, json) = post(router, ACTS, &unknown).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{json}");
}

/// The trial flow with a cell of words of its own: the plan has `plan_words`, and its first round hands in the plan and
/// a reply, and the item's card shows the plan.
fn trial_with_words() -> (flow::Compiled, service::Script) {
    // A cell that can be spoken to may ask, so the plan has its questions beside its words.
    let own = "[blocks.plan]\n\n[blocks.plan_questions]\noutside = \"answer\"\nshown = true\n\
        [blocks.plan_questions.writes]\n\
        text = \"{question}, {version}: {option}, {does}.\"\n\
        asked = \"{question}, {version}: {ask}{options}{recommends}{waits}\"\n\
        asked_anew = \"{question}, {version} for {was}: {ask}{options}{recommends}{waits}\"\n\
        option = \" [{option}. {does}]\"\nrecommends = \" Recommends {option}.\"\nwaits = \" Waits.\"\n\
        goes_on = \" Goes on.\"\nwithdrawn = \"{question}, {version}: {was} withdrawn.{reason}\"\n\
        why = \" Why: {why}\"\n\
        [blocks.plan_words]\noutside = \"words\"\n[blocks.plan_words.writes]\n\
        owner = \"The owner: {words}\"\nreply = \"{reply}\"\n";
    let source = edit(trial_flow(), None, "[blocks.plan]\n", own);
    let source = edit(
        source,
        Some("plan"),
        "watches = [\"request\"]",
        "watches = [\"request\", \"plan_questions\", \"plan_words\"]",
    );
    let source = edit(
        source,
        Some("plan"),
        "outputs = [\"plan\"]",
        "outputs = [\"plan\", \"plan_questions\", \"plan_words\"]",
    );
    let source = edit(
        source,
        Some("plan"),
        "request = \"last_and_latest\"",
        "request = \"last_and_latest\"\nplan_questions = \"all\"\nplan_words = \"all\"",
    );
    let mut flow = flow::compile(&source, &trial_has_with_words(&source)).unwrap();
    flow.card = vec!["plan".into()];
    let script = service::Script::read(
        r#"
        [[flows.trial.rounds]]
        cell = "plan"
        hands_in = [{ block = "plan", content = "The plan." }, { block = "plan_words", content = "Kept it short." }]
        "#,
    )
    .unwrap();
    (flow, script)
}

// What the owner has read of a work lowers what the items say of it, as the page reads them: the plan the card shows is
// new and the reply to the owner's words is not read until each is, and the item is given again, on the cursor of the
// page, once a read of it is kept; a read that is refused or repeated changes nothing the page is given.
#[tokio::test(start_paused = true)]
async fn what_the_owner_reads_lowers_what_the_items_say_of_the_work_and_gives_the_item_again() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("store.sqlite")).unwrap();
    let (flow, script) = trial_with_words();
    let (running, _, _) = running_compiled(
        Box::new(store),
        dir,
        Scripted::new(script).carriers(),
        flow,
        crate::levels(),
        web::items_cache::BOUND_MS,
    );
    let router = &running.router;
    let done = (StatusCode::NO_CONTENT, Value::Null);
    let item = json!({
        "title": "Greet the user",
        "dependsOn": [],
        "project": "trial",
        "flow": "trial",
        "writes": [{ "block": "request", "content": "Greet the user, please.", "value": null }],
    });
    assert_eq!(post(router, "/api/v1/items", &item).await.0, StatusCode::OK);
    pass(1000).await;
    // The owner speaks to the plan, and the work goes on: the plan hands in its plan and a reply.
    let say = json!({ "kind": "say", "to": { "kind": "cell", "cell": "plan" }, "words": "Keep it short." });
    assert_eq!(
        post(router, ACTS, &say).await,
        (StatusCode::OK, json!({ "reached": "plan" }))
    );
    assert_eq!(post(router, ACTS, &json!({ "kind": "resume" })).await, done);
    let mut work = Value::Null;
    for _ in 0..100 {
        pass(250).await;
        work = get_json(router, "/api/v1/items").await["items"][0]["work"].clone();
        if work["repliesUnread"] == 1 {
            break;
        }
    }
    assert_eq!(
        (work["fresh"].clone(), work["repliesUnread"].clone()),
        (json!("new"), json!(1)),
        "{work}"
    );
    // Held where it stands, so that nothing but a read changes what the items say.
    assert_eq!(post(router, ACTS, &json!({ "kind": "pause" })).await, done);
    pass(20_000).await;
    let page = get_json(router, "/api/v1/works/1").await;
    let version = |block: &str, reply: bool| {
        let block = page["blocks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|b| b["name"] == block)
            .unwrap();
        let found = block["versions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["reply"] == reply)
            .unwrap();
        found["number"].clone()
    };
    let (plan, reply) = (version("plan", false), version("plan_words", true));
    let first = get_json(router, "/api/v1/items").await;
    let items_since = |since: String| async move {
        let page = get_json(router, &format!("/api/v1/items?since={since}")).await;
        let numbers: Vec<u64> = (page["items"].as_array().unwrap().iter())
            .map(|i| i["number"].as_u64().unwrap())
            .collect();
        let work = page["items"].get(0).map_or(Value::Null, |i| i["work"].clone());
        (
            numbers,
            work["fresh"].clone(),
            work["repliesUnread"].clone(),
            page["cursor"].as_str().unwrap().to_string(),
        )
    };
    let cursor = first["cursor"].as_str().unwrap().to_string();
    let read = |versions: Value| json!({ "kind": "read", "versions": versions });
    // A read of a version the work has not is refused and keeps nothing, not the version beside it: nothing is given.
    let refused = read(json!([{ "block": "plan_words", "version": reply }, { "block": "plan_words", "version": 99 }]));
    assert_eq!(post(router, ACTS, &refused).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(items_since(cursor.clone()).await.0, Vec::<u64>::new());
    // The reply read: its count falls to nothing, the plan is still new, and the item is given again.
    let one = read(json!([{ "block": "plan_words", "version": reply }]));
    assert_eq!(post(router, ACTS, &one).await, done);
    let (given, fresh, replies, cursor) = items_since(cursor).await;
    assert_eq!((given, fresh, replies), (vec![1], json!("new"), json!(0)));
    // The plan read: it is no longer new, and the item is given again.
    let plan_read = read(json!([{ "block": "plan", "version": plan }]));
    assert_eq!(post(router, ACTS, &plan_read).await, done);
    let (given, fresh, replies, cursor) = items_since(cursor).await;
    assert_eq!((given, fresh, replies), (vec![1], json!("none"), json!(0)));
    // Read again, nothing changes, and the item is not given again.
    assert_eq!(post(router, ACTS, &plan_read).await, done);
    assert_eq!(items_since(cursor).await.0, Vec::<u64>::new());
}

// A read of nothing, of a version numbered nought and of a work there is not are the edges of the read act. Reference:
// the act's own rule, that a read is taken once the work holds each version it names (versions count from one), and
// that what is read already is no change; the other acts' answer for a work there is not.
#[tokio::test(start_paused = true)]
async fn a_read_of_nothing_is_taken_and_changes_nothing_and_version_nought_and_an_unknown_work_are_refused() {
    let running = running();
    let router = &running.router;
    greeted(router).await;
    let done = (StatusCode::NO_CONTENT, Value::Null);
    let opinion = json!({ "kind": "write", "block": "opinions", "content": "Keep it short.", "value": null });
    assert_eq!(post(router, ACTS, &opinion).await, done);
    let steps = |page: &Value| page["steps"].as_array().unwrap().len();
    let before = get_json(router, "/api/v1/works/1").await;
    let nothing = json!({ "kind": "read", "versions": [] });
    assert_eq!(post(router, ACTS, &nothing).await, done);
    assert_eq!(steps(&get_json(router, "/api/v1/works/1").await), steps(&before));
    let nought = json!({ "kind": "read", "versions": [{ "block": "opinions", "version": 0 }] });
    assert_eq!(post(router, ACTS, &nought).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    let one = json!({ "kind": "read", "versions": [{ "block": "opinions", "version": 1 }] });
    let (status, json) = post(router, "/api/v1/works/9/acts", &one).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{json}");
}

// The page of a work and its item agree on what is a reply and what is read, through a second reply: the owner's own
// words are no reply, the reply to words said after a read is unread again whatever was read before, and the number of
// versions the page marks as replies not read is the number the items give. Reference: the one definition of a reply
// (a cell's run writing the cell's words block with something in it after the owner spoke there), applied by hand to
// the script's two rounds.
#[tokio::test(start_paused = true)]
async fn a_reply_to_later_words_is_not_read_for_what_was_read_before_and_the_page_and_the_item_count_alike() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("store.sqlite")).unwrap();
    let (flow, _) = trial_with_words();
    let script = service::Script::read(
        r#"
        [[flows.trial.rounds]]
        cell = "plan"
        hands_in = [{ block = "plan", content = "The plan." }, { block = "plan_words", content = "Kept it short." }]

        [[flows.trial.rounds]]
        cell = "plan"
        hands_in = [{ block = "plan", content = "The plan." }, { block = "plan_words", content = "Made it shorter." }]
        "#,
    )
    .unwrap();
    let (running, _, _) = running_compiled(
        Box::new(store),
        dir,
        Scripted::new(script).carriers(),
        flow,
        crate::levels(),
        web::items_cache::BOUND_MS,
    );
    let router = &running.router;
    let done = (StatusCode::NO_CONTENT, Value::Null);
    let item = json!({
        "title": "Greet the user",
        "dependsOn": [],
        "project": "trial",
        "flow": "trial",
        "writes": [{ "block": "request", "content": "Greet the user, please.", "value": null }],
    });
    assert_eq!(post(router, "/api/v1/items", &item).await.0, StatusCode::OK);
    pass(1000).await;
    let say = |words: &str| json!({ "kind": "say", "to": { "kind": "cell", "cell": "plan" }, "words": words });
    let unread_replies = || async {
        let work = get_json(router, "/api/v1/items").await["items"][0]["work"].clone();
        let page = get_json(router, "/api/v1/works/1").await;
        let marked = (page["blocks"].as_array().unwrap().iter())
            .filter(|b| b["name"] == "plan_words")
            .flat_map(|b| b["versions"].as_array().unwrap().iter())
            .filter(|v| v["reply"] == true && v["read"] == false)
            .count();
        (work["repliesUnread"].clone(), json!(marked))
    };
    let until = |count: u64| async move {
        for _ in 0..100 {
            pass(250).await;
            if unread_replies().await.0 == count {
                break;
            }
        }
    };
    assert_eq!(post(router, ACTS, &say("Keep it short.")).await.0, StatusCode::OK);
    assert_eq!(post(router, ACTS, &json!({ "kind": "resume" })).await, done);
    until(1).await;
    assert_eq!(unread_replies().await, (json!(1), json!(1)));
    assert_eq!(post(router, ACTS, &json!({ "kind": "pause" })).await, done);
    pass(20_000).await;
    // Read the reply, then the owner speaks again and the cell answers again: one reply not read, and the owner's two
    // versions of words are marked no reply.
    let page = get_json(router, "/api/v1/works/1").await;
    let words = (page["blocks"].as_array().unwrap().iter())
        .find(|b| b["name"] == "plan_words")
        .unwrap()["versions"]
        .clone();
    let reply = (words.as_array().unwrap().iter()).find(|v| v["reply"] == true).unwrap()["number"].clone();
    let read = json!({ "kind": "read", "versions": [{ "block": "plan_words", "version": reply }] });
    assert_eq!(post(router, ACTS, &read).await, done);
    assert_eq!(unread_replies().await, (json!(0), json!(0)));
    assert_eq!(post(router, ACTS, &say("Shorter.")).await.0, StatusCode::OK);
    assert_eq!(post(router, ACTS, &json!({ "kind": "resume" })).await, done);
    until(1).await;
    assert_eq!(unread_replies().await, (json!(1), json!(1)));
    let page = get_json(router, "/api/v1/works/1").await;
    let words = (page["blocks"].as_array().unwrap().iter())
        .find(|b| b["name"] == "plan_words")
        .unwrap()["versions"]
        .clone();
    let kinds: Vec<(bool, bool)> = (words.as_array().unwrap().iter())
        .map(|v| (v["reply"] == true, v["read"] == true))
        .collect();
    // The owner's words, the first reply (read), the owner's words, the second reply (not read).
    assert_eq!(
        kinds,
        [(false, false), (true, true), (false, false), (true, false)],
        "{words}"
    );
}
