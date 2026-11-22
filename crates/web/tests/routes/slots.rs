//! `POST /api/v1/settings` over the engine's loop, for the slots: a change of them made on the page is written into the
//! file and taken by the engine at once. Slots added go to the works that wait for one; with fewer, no round under way
//! is stopped, and the engine says more are taken than it has; whoever listens is told, and the page shows the slots
//! as the file gives them, with none beside them.

use std::sync::Arc;

use axum::Router;
use axum::http::StatusCode;
use serde_json::{Value, json};
use service::{Changed, Handle, Scripted, Service, TokioClock};
use store::Store;
use web::Flows;
use web::settings::Install;

use crate::engine::add_let_on;
use crate::{START, get_json, levels, post, projects, trial_dir, trial_flow, trial_has, trial_script};

/// The route of the settings.
const PATH: &str = "/api/v1/settings";

/// The settings' file as the engine starts on it: one slot.
const FILE: &str = "[agents]\ncommand = \"claude\"\nslots = 1\n";

/// The stand-in for the engine's reading of the settings: the file's tables as they are, refused where they give no
/// slot.
fn read(text: &str) -> Result<Value, String> {
    let table: toml::Table = toml::from_str(text).map_err(|e| e.to_string())?;
    let json = serde_json::to_value(&table).map_err(|e| e.to_string())?;
    match json["agents"]["slots"].as_u64() {
        Some(slots) if slots > 0 => Ok(json),
        _ => Err("agents.slots: at least one round must be able to run".to_string()),
    }
}

/// The interface over the engine's loop on the trial flow, started with the one slot [`FILE`] gives, and the routes of
/// the settings over that file; with the way to the loop and the directory the store and the file are in.
fn serving() -> (Router, Handle, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("store.sqlite")).unwrap();
    let mut script = trial_script();
    script.slots = Some(1);
    let flow = flow::compile(&trial_flow(), &trial_has(&flow::Source::default())).unwrap();
    let clock = Arc::new(TokioClock::new(work::Time(START)));
    let carriers = Scripted::new(script).carriers();
    let (service, inbox) = Service::open(Box::new(store), vec![flow], carriers, clock).unwrap();
    let handle = service.handle();
    tokio::spawn(service.run(inbox));
    let file = dir.path().join("install.toml");
    std::fs::write(&file, FILE).unwrap();
    let install = Install {
        file,
        read: Box::new(read),
        running: read(FILE).unwrap(),
        git: &[],
    };
    let flows = Flows::dirs(trial_dir().join("flow"), Box::new(trial_has));
    let router = web::with_works(flows, handle.clone(), levels(), projects())
        .merge(web::settings::router(install, handle.clone()));
    (router, handle, dir)
}

/// The owner sets the slots on the page, against the file as the page reads it now; gives the answer.
async fn set_slots(router: &Router, slots: u64) -> (StatusCode, Value) {
    let base = get_json(router, PATH).await["base"].clone();
    let change = json!({ "base": base, "set": [{ "key": "agents.slots", "value": slots }] });
    post(router, PATH, &change).await
}

/// The slots as the page shows them: as the file writes them, and what the engine runs with beside them.
async fn shown(router: &Router) -> (Value, Value) {
    let page = get_json(router, PATH).await;
    let agents = (page["tables"].as_array().expect("the tables"))
        .iter()
        .find(|table| table["name"] == "agents")
        .expect("the table of the agents")
        .clone();
    let slots = (agents["keys"].as_array().expect("its keys"))
        .iter()
        .find(|key| key["key"] == "slots")
        .expect("the slots")
        .clone();
    (slots["written"].clone(), slots["running"].clone())
}

/// What the engine says of its slots: how many it has, and how many are taken.
async fn engine(router: &Router) -> (Value, Value) {
    let engine = get_json(router, "/api/v1/engine").await;
    (engine["slots"].clone(), engine["taken"].clone())
}

/// What each of these works does now, by its kind.
async fn now(router: &Router, works: &[u64]) -> Vec<Value> {
    let mut kinds = Vec::new();
    for work in works {
        kinds.push(get_json(router, &format!("/api/v1/works/{work}")).await["summary"]["now"]["kind"].clone());
    }
    kinds
}

#[tokio::test(start_paused = true)]
async fn the_slots_set_on_the_page_are_written_and_taken_by_the_engine_at_once() {
    let (router, handle, dir) = serving();
    let router = &router;
    for (title, work) in [("First", 1), ("Second", 2), ("Third", 3)] {
        add_let_on(router, title, work).await;
    }
    assert_eq!(engine(router).await, (json!(1), json!(1)));
    assert_eq!(now(router, &[1, 2, 3]).await, ["round", "waits", "waits"]);
    // Three slots: the file holds them, and the two works that waited for one go on at once.
    assert_eq!(
        set_slots(router, 3).await,
        (
            StatusCode::OK,
            json!({ "said": "agents.slots = 3 instead of 1", "committed": false })
        )
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("install.toml")).unwrap(),
        FILE.replace("slots = 1", "slots = 3")
    );
    assert_eq!(engine(router).await, (json!(3), json!(3)));
    assert_eq!(now(router, &[1, 2, 3]).await, ["round", "round", "round"]);
    assert_eq!(shown(router).await, (json!("3"), Value::Null));
    // One slot: no round under way is stopped, the engine says three are taken of one, and whoever listens is told,
    // though no work changed.
    let mut changes = handle.changes();
    assert_eq!(set_slots(router, 1).await.0, StatusCode::OK);
    assert_eq!(engine(router).await, (json!(1), json!(3)));
    assert_eq!(now(router, &[1, 2, 3]).await, ["round", "round", "round"]);
    let notices: Vec<Changed> = std::iter::from_fn(|| changes.try_recv().ok()).collect();
    let nothing = Changed {
        works: Vec::new(),
        items: false,
        copies: false,
        accounts: false,
    };
    assert_eq!(notices, [nothing]);
    assert_eq!(shown(router).await, (json!("1"), Value::Null));
}
