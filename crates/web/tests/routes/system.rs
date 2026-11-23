//! `GET /api/v1/system`: how the system went over a span, worked out from the works' records: what was produced, what
//! was spent and on what, and the rounds that failed; the span cut into the weeks the page gives.

use axum::http::StatusCode;
use serde_json::{Value, json};
use store::Store;

use crate::{START, get_json, greeted, pass, post, refused, running_scripted, trial_script};

const DAY: u64 = 24 * 60 * 60 * 1000;

/// The trial item, its plan's first round handing in the plan, added and let on: within a minute it has merged.
async fn merged() -> crate::Running {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("store.sqlite")).unwrap();
    let mut script = trial_script();
    script.flows.get_mut("trial").unwrap().rounds[0].asks = None;
    let running = running_scripted(Box::new(store), dir, script);
    let router = &running.router;
    let item = json!({
        "title": "Greet the user",
        "dependsOn": [],
        "project": "trial",
        "flow": "trial",
        "writes": [{ "block": "request", "content": "Greet the user, please.", "value": null }],
    });
    assert_eq!(post(router, "/api/v1/items", &item).await.0, StatusCode::OK);
    let resumed = post(router, "/api/v1/works/1/acts", &json!({ "kind": "resume" })).await;
    assert_eq!(resumed, (StatusCode::NO_CONTENT, Value::Null));
    pass(60_000).await;
    assert_eq!(
        get_json(router, "/api/v1/works/1").await["summary"]["now"],
        json!({ "kind": "merged" })
    );
    running
}

#[tokio::test(start_paused = true)]
async fn all_time_gives_the_merge_and_what_was_spent_on_each_cell_and_model() {
    let running = merged().await;
    let router = &running.router;
    let system = get_json(router, "/api/v1/system").await;
    let work = get_json(router, "/api/v1/works/1").await;
    let spent = work["summary"]["spent"].clone();
    assert_eq!(system["from"], Value::Null);
    assert_eq!(system["spent"], spent, "every round ended in all time: {system}");
    assert_eq!(system["byWork"], json!([{ "work": 1, "item": 1, "spent": spent }]));
    let merged = &system["merged"].as_array().unwrap()[..];
    assert_eq!(merged.len(), 1, "{system}");
    assert_eq!(
        (merged[0]["work"].clone(), merged[0]["item"].clone()),
        (json!(1), json!(1))
    );
    assert_eq!(merged[0]["from"], work["summary"]["from"]);
    assert_eq!(merged[0]["spent"], spent);
    assert!(merged[0]["at"].as_u64().unwrap() > START, "{system}");
    assert_eq!(system["added"], json!([{ "work": 1, "item": 1, "at": START }]));
    assert_eq!(system["cancelled"], json!([]));
    assert_eq!(system["failed"], json!([]));
    let cells: Vec<&str> = (system["byCell"].as_array().unwrap().iter())
        .map(|c| c["cell"].as_str().unwrap())
        .collect();
    for cell in ["plan", "build", "review", "merge"] {
        assert!(cells.contains(&cell), "{cell} spent nothing: {system}");
    }
    let rounds = |list: &Value| -> u64 {
        (list.as_array().unwrap().iter())
            .map(|x| x["spent"]["rounds"].as_u64().unwrap())
            .sum()
    };
    assert_eq!(rounds(&system["byCell"]), spent["rounds"].as_u64().unwrap());
    assert_eq!(rounds(&system["byModel"]), spent["rounds"].as_u64().unwrap());
    // The merge is the engine's own act, its one round on no model, apart from the agents' rounds.
    let acts = (system["byModel"].as_array().unwrap().iter())
        .find(|m| m["on"]["kind"] == "act")
        .expect("the merge's round");
    assert_eq!(acts["spent"]["rounds"], 1, "{system}");
    assert_eq!(system["weeks"], json!([]), "all time is not cut into weeks");
    // 2026-11-10 is a Tuesday: everything happened in the week that began on the Monday before, and nothing in the
    // week before it.
    let (week, before) = (START - DAY, START - 8 * DAY);
    let weeks = get_json(router, &format!("/api/v1/system?weeks={week},{before}")).await;
    assert_eq!(weeks["from"], before, "the span begins with its earliest week");
    let nothing =
        json!({ "usdMicros": 0, "inputTokens": 0, "cachedTokens": 0, "outputTokens": 0, "workingMs": 0, "rounds": 0 });
    assert_eq!(
        weeks["weeks"],
        json!([
            {
                "start": week, "spent": spent, "pinged": nothing, "merged": 1, "cancelled": 0, "added": 1, "sentBack": 0,
                "failed": 0
            },
            {
                "start": before, "spent": nothing, "pinged": nothing, "merged": 0, "cancelled": 0, "added": 0,
                "sentBack": 0, "failed": 0
            },
        ])
    );
}

#[tokio::test(start_paused = true)]
async fn a_span_counts_only_what_happened_in_it() {
    let running = merged().await;
    let router = &running.router;
    let later = START + 120_000;
    let system = get_json(router, &format!("/api/v1/system?weeks={later}")).await;
    assert_eq!(system["from"], later);
    assert_eq!(system["spent"]["rounds"], 0, "{system}");
    assert_eq!(system["merged"], json!([]));
    assert_eq!(system["added"], json!([]));
    assert_eq!(system["byWork"], json!([]));
    assert_eq!(
        system["weeks"].as_array().unwrap().len(),
        1,
        "the span's one week, with nothing: {system}"
    );
    let (status, words) = refused(router, "/api/v1/system?weeks=1,yesterday").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(words.starts_with("The span cannot be read"), "{words}");
}

#[tokio::test(start_paused = true)]
async fn a_round_that_failed_is_given_with_its_cause() {
    const WHY: &str = "The agent's program could not start.";
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("store.sqlite")).unwrap();
    let mut script = trial_script();
    let plan = script.flows.get_mut("trial").unwrap();
    let mut failing = plan.rounds[0].clone();
    failing.round = 4;
    failing.asks = None;
    failing.fails = Some(WHY.to_string());
    plan.rounds.push(failing);
    let running = running_scripted(Box::new(store), dir, script);
    let router = &running.router;
    greeted(router).await;
    let answer = json!({ "kind": "goOn", "call": 2 });
    assert_eq!(
        post(router, "/api/v1/works/1/acts", &answer).await.0,
        StatusCode::NO_CONTENT
    );
    pass(2000).await;
    let system = get_json(router, &format!("/api/v1/system?weeks={}", START - DAY)).await;
    let failed = &system["failed"].as_array().unwrap()[..];
    assert_eq!(failed.len(), 1, "{system}");
    assert_eq!(failed[0]["cell"], "plan");
    assert_eq!(failed[0]["failure"], "other");
    assert_eq!(failed[0]["why"], WHY);
    assert_eq!(system["weeks"][0]["failed"], 1);
}

/// What every agent's round runs on, in the history below.
fn opus() -> work::RanOn {
    work::RanOn {
        level: "2".to_string(),
        model: "opus".to_string(),
        effort: None,
        directory: None,
        account: None,
    }
}

/// The run of the work's round given to be done.
fn round_of(driver: &driver::Driver, work: store::WorkNumber) -> decide::RunId {
    (driver.pending().into_iter())
        .find_map(|todo| match todo.effect {
            engine::Effect::Round { run, .. } if todo.work == work => Some(run),
            _ => None,
        })
        .expect("a round of the work is given")
}

/// The driver takes what happened to the work at this time.
fn happened(driver: &mut driver::Driver, at: u64, work: store::WorkNumber, event: engine::Event) {
    driver.happened(work::Time(at), work, event).unwrap();
}

/// A work of the trial flow added and let on at `at`, whose plan hands in the plan `ms` later, its one try on the
/// model having grown its context by 100 tokens over three calls and cost `usd` millionths of a dollar; the try of
/// its build, which then begins, begins on the model.
fn planned(driver: &mut driver::Driver, at: u64, ms: u64, usd: u64) -> store::WorkNumber {
    let new = driver::NewItem {
        title: "Plan".to_string(),
        level: "2".to_string(),
        project: "trial".to_string(),
        depends_on: Vec::new(),
        tags: Vec::new(),
        continues: None,
        flow: "trial".to_string(),
        writes: vec![driver::FirstWrite {
            block: decide::BlockId("request".to_string()),
            content: "Please.".to_string(),
            value: None,
        }],
        why: None,
        waits_for: Vec::new(),
    };
    let (_, work, _) = driver.add(work::Time(at), new).unwrap();
    happened(driver, at, work, engine::Event::Resumed);
    let run = round_of(driver, work);
    let ran = engine::Event::Ran {
        run,
        attempt: 1,
        on: opus(),
    };
    happened(driver, at, work, ran);
    let measures = work::Measures {
        calls: 3,
        added: 100,
        ..work::Measures::default()
    };
    let measured = engine::Event::Measured {
        run,
        attempt: 1,
        measures,
    };
    happened(driver, at, work, measured);
    let plan = engine::Handed {
        block: decide::BlockId("plan".to_string()),
        content: "The plan.".to_string(),
        matters: true,
        value: None,
    };
    let spent = engine::Spent {
        usd_micros: usd,
        ..engine::Spent::default()
    };
    let returned = engine::Event::Returned {
        run,
        came: engine::Came::hands_in(vec![plan]),
        spent,
    };
    happened(driver, at + ms, work, returned);
    let build = engine::Event::Ran {
        run: round_of(driver, work),
        attempt: 1,
        on: opus(),
    };
    happened(driver, at + ms, work, build);
    work
}

// What the System view and a work's page are given of how rounds went against the lines of their history, as the
// driver works it out from the records: here, of plans whose third cost and took far more than the two before it, and
// of the builds under way after them.
#[test]
fn what_went_past_the_lines_of_its_history_and_what_is_under_way_are_given_against_their_lines() {
    const S: u64 = 1000;
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("store.sqlite")).unwrap();
    let flow = flow::compile(
        &flow::read(&crate::trial_dir().join("flow")).unwrap(),
        &crate::trial_has(&flow::Source::default()),
    )
    .unwrap();
    let policy = driver::Policy {
        samples: 2,
        model: Box::new(|_, _| Some(opus().model)),
        changed: None,
        warm: None,
    };
    let (mut driver, _) =
        driver::Driver::open(Box::new(store), vec![flow], 10, Some(policy), work::Time(START)).unwrap();
    planned(&mut driver, START, 10 * S, 1_000);
    planned(&mut driver, START + 60 * S, 20 * S, 2_000);
    let third = planned(&mut driver, START + 120 * S, 100 * S, 50_000);
    let now = work::Time(START + 600 * S);
    let system = serde_json::to_value(web::system::System::of(&driver, now, &[]).unwrap()).unwrap();
    let line = |value: u64, samples: u64, scope: &str| json!({ "value": value, "samples": samples, "scope": scope });
    let past = system["past"].as_array().unwrap();
    assert_eq!(past.len(), 1, "{system}");
    assert_eq!(
        (&past[0]["kind"], &past[0]["work"], &past[0]["cell"], &past[0]["ended"]),
        (
            &json!("round"),
            &json!(third.0),
            &json!("plan"),
            &json!(START + 220 * S)
        )
    );
    assert_eq!(
        past[0]["judged"],
        json!({
            "cost": { "value": 50_000, "line": line(2_500, 2, "cell"), "past": true },
            "time": { "value": 100_000, "line": line(25_000, 2, "cell"), "past": true },
        })
    );
    // The builds under way, the oldest first: the first began with one plan behind it, too few for any line; the last
    // with three, the flow's agents' rounds giving its lines, and it has taken longer so far than they explain.
    let under_way = system["underWay"].as_array().unwrap();
    assert_eq!(under_way.len(), 3, "{system}");
    assert_eq!(under_way[0]["cell"], "build");
    assert_eq!(under_way[0]["time"], Value::Null);
    assert_eq!(
        under_way[0]["bounds"],
        json!({ "added": null, "rate": null, "cost": null })
    );
    let last = &under_way[2];
    assert_eq!(
        (&last["kind"], &last["work"], &last["began"]),
        (&json!("round"), &json!(third.0), &json!(START + 220 * S))
    );
    assert_eq!(
        last["time"],
        json!({ "value": 380_000, "line": line(180_000, 3, "flow"), "past": true })
    );
    assert_eq!(
        last["bounds"],
        json!({ "added": line(100, 3, "flow"), "rate": line(50, 3, "flow"), "cost": line(98_000, 3, "flow") })
    );
    // A work's page gives each of its rounds the same.
    let page = web::works::WorkPage::of(&driver, &crate::levels(), third, now)
        .unwrap()
        .unwrap();
    let page = serde_json::to_value(page).unwrap();
    let plan = (page["steps"].as_array().unwrap().iter())
        .find(|step| step["kind"] == "run" && step["cell"] == "plan")
        .expect("the plan's run");
    assert_eq!(plan["rounds"][0]["judged"], past[0]["judged"]);
}

// A fork's cost counts in the System view as a keep-warm ping's does, in the total, the week it ended in and the
// work's spend, and a work whose only spend in a span is a fork is listed for that span. One work of the trial flow,
// its plan costing 1,000 millionths of a dollar and a fork of its round, long after, 7,000; the span that begins
// between them. Milliseconds.
#[test]
fn what_a_work_spent_on_a_fork_is_counted_in_the_system_view_in_its_total_its_week_and_its_work() {
    const S: u64 = 1000;
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("store.sqlite")).unwrap();
    let flow = flow::compile(
        &flow::read(&crate::trial_dir().join("flow")).unwrap(),
        &crate::trial_has(&flow::Source::default()),
    )
    .unwrap();
    let policy = driver::Policy {
        samples: 2,
        model: Box::new(|_, _| Some(opus().model)),
        changed: None,
        warm: None,
    };
    let (mut driver, _) =
        driver::Driver::open(Box::new(store), vec![flow], 10, Some(policy), work::Time(START)).unwrap();
    let work = planned(&mut driver, START, 10 * S, 1_000);
    let run = driver.work(work).unwrap().work.records.rounds[0].run;
    let spent = engine::Spent {
        usd_micros: 7_000,
        input_tokens: 10,
        cached_tokens: 20,
        output_tokens: 3,
        ..engine::Spent::default()
    };
    happened(
        &mut driver,
        START + 60 * S,
        work,
        engine::Event::Forked { run, round: 1, spent },
    );
    let now = work::Time(START + 600 * S);
    let usd = |value: &Value| value["usdMicros"].as_u64().unwrap();
    let all = serde_json::to_value(web::system::System::of(&driver, now, &[]).unwrap()).unwrap();
    assert_eq!(
        (usd(&all["spent"]), &all["spent"]["rounds"]),
        (8_000, &json!(1)),
        "{all}"
    );
    assert_eq!(usd(&all["pinged"]), 0, "a fork is no ping: {all}");
    assert_eq!(usd(&all["byWork"][0]["spent"]), 8_000, "{all}");
    // The span that begins after the plan ended holds the fork alone: its spend, no round, and its work listed.
    let span = [START + 30 * S];
    let late = serde_json::to_value(web::system::System::of(&driver, now, &span).unwrap()).unwrap();
    assert_eq!(
        (usd(&late["spent"]), &late["spent"]["rounds"]),
        (7_000, &json!(0)),
        "{late}"
    );
    assert_eq!(late["byWork"].as_array().unwrap().len(), 1, "{late}");
    assert_eq!(usd(&late["byWork"][0]["spent"]), 7_000, "{late}");
    assert_eq!(usd(&late["weeks"][0]["spent"]), 7_000, "{late}");
    assert_eq!(usd(&late["weeks"][0]["pinged"]), 0, "{late}");
    // The work's page, which the list of the works reads its Cost from, counts the fork in what the work spent and
    // gives it apart, in no round.
    let page = web::works::WorkPage::of(&driver, &crate::levels(), work, now)
        .unwrap()
        .unwrap();
    let page = serde_json::to_value(page).unwrap();
    let summary = &page["summary"];
    assert_eq!(usd(&summary["spent"]), 8_000, "{summary}");
    assert_eq!(summary["spent"]["rounds"], all["spent"]["rounds"], "{summary}");
    assert_eq!(
        (usd(&summary["forked"]), &summary["forked"]["rounds"]),
        (7_000, &json!(1)),
        "{summary}"
    );
    assert_eq!(usd(&summary["pinged"]), 0, "{summary}");
}

/// A work of the trial flow added and let on at `at`, whose plan asks for a tool at once and has the result, of this
/// many bytes, two seconds later: the call, the work it is of, and when it had its result.
fn answered(driver: &mut driver::Driver, at: u64, bytes: usize) -> (store::WorkNumber, decide::CallId, u64) {
    let new = driver::NewItem {
        title: "Ask".to_string(),
        level: "2".to_string(),
        project: "trial".to_string(),
        depends_on: Vec::new(),
        tags: Vec::new(),
        continues: None,
        flow: "trial".to_string(),
        writes: vec![driver::FirstWrite {
            block: decide::BlockId("request".to_string()),
            content: "Please.".to_string(),
            value: None,
        }],
        why: None,
        waits_for: Vec::new(),
    };
    let (_, work, _) = driver.add(work::Time(at), new).unwrap();
    happened(driver, at, work, engine::Event::Resumed);
    let run = round_of(driver, work);
    let ran = engine::Event::Ran {
        run,
        attempt: 1,
        on: opus(),
    };
    happened(driver, at, work, ran);
    let ask = engine::Ask::Tool {
        name: "search".to_string(),
        input: "what search is run on".to_string(),
    };
    let returned = engine::Event::Returned {
        run,
        came: engine::Came::asks(vec![ask]),
        spent: engine::Spent::default(),
    };
    happened(driver, at, work, returned);
    let call = (driver.pending().into_iter())
        .find_map(|todo| match todo.effect {
            engine::Effect::Call { call, .. } if todo.work == work => Some(call),
            _ => None,
        })
        .expect("the tool is called");
    let done = at + 2_000;
    let result = "x".repeat(bytes);
    happened(driver, done, work, engine::Event::Done { call, result });
    (work, call, done)
}

// What the System view and a work's page are given of how the results of calls of a tool went against the line of
// their sizes: here, of searches that took as long as one another, whose third returned far more than the two before
// it, and a fourth that returned less than the line.
#[test]
fn a_call_past_its_line_by_the_size_of_its_result_alone_is_given_with_its_size_against_its_line() {
    const S: u64 = 1000;
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("store.sqlite")).unwrap();
    let flow = flow::compile(
        &flow::read(&crate::trial_dir().join("flow")).unwrap(),
        &crate::trial_has(&flow::Source::default()),
    )
    .unwrap();
    let policy = driver::Policy {
        samples: 2,
        model: Box::new(|_, _| Some(opus().model)),
        changed: None,
        warm: None,
    };
    let (mut driver, _) =
        driver::Driver::open(Box::new(store), vec![flow], 10, Some(policy), work::Time(START)).unwrap();
    answered(&mut driver, START, 5_000);
    answered(&mut driver, START + 60 * S, 10_000);
    let (third, call, done) = answered(&mut driver, START + 120 * S, 60_000);
    answered(&mut driver, START + 180 * S, 5_000);
    let now = work::Time(START + 600 * S);
    let system = serde_json::to_value(web::system::System::of(&driver, now, &[]).unwrap()).unwrap();
    let line = |value: u64, samples: u64| json!({ "value": value, "samples": samples, "scope": "tool" });
    let past = system["past"].as_array().unwrap();
    assert_eq!(past.len(), 1, "{system}");
    assert_eq!(
        (
            &past[0]["kind"],
            &past[0]["work"],
            &past[0]["call"],
            &past[0]["tool"],
            &past[0]["done"]
        ),
        (
            &json!("call"),
            &json!(third.0),
            &json!(call.0),
            &json!("search"),
            &json!(done)
        )
    );
    assert_eq!(
        past[0]["size"],
        json!({ "value": 60_000, "line": line(12_500, 2), "past": true })
    );
    assert_eq!(
        past[0]["time"],
        json!({ "value": 2_000, "line": line(2_000, 2), "past": false }),
        "it took as long as the calls before it"
    );
    // A work's page gives the call the same.
    let page = web::works::WorkPage::of(&driver, &crate::levels(), third, now)
        .unwrap()
        .unwrap();
    let page = serde_json::to_value(page).unwrap();
    let row = (page["steps"].as_array().unwrap().iter())
        .filter(|step| step["kind"] == "run")
        .flat_map(|step| step["calls"].as_array().unwrap().iter())
        .find(|row| row["call"] == call.0)
        .expect("the call's row");
    assert_eq!(row["size"], past[0]["size"]);
}
