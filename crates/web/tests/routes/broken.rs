//! A work whose summary cannot be read keeps no other work from being given: `GET /api/v1/items`, `/api/v1/pulls` and
//! `/api/v1/attention` answer as they do where no work is broken, and the broken work says its state is broken, with
//! what is certain of it (decision `a-work-whose-state-cannot-be-worked-out-fails-alone`).

use std::path::Path;

use decide::{BlockId, Pauser};
use engine::{Came, Effect, Event};
use serde_json::{Value, json};
use store::{
    AccountRecord, EverythingPaused, Fault, First, Item, ItemChange, ItemNumber, KeptFlow, KeptOutline,
    PauseOfEverything, StopRecord, Storage, Store, WorkHead, WorkNumber,
};
use work::{Time, Work};

use crate::{START, get_json, levels, running_kept, running_on, trial_flow, trial_has};

/// A store that gives the first work as the store keeps it, but for the rounds' given texts, which it says were never
/// kept: the work's rounds then cannot be read into its summary. It changes nothing in what it keeps.
struct Unreadable {
    store: Store,
}

impl Storage for Unreadable {
    fn add(&mut self, item: &First, flow: &str, at: Time, first: &Work) -> Result<(ItemNumber, WorkNumber), Fault> {
        self.store.add(item, flow, at, first)
    }

    fn keep_flow(&mut self, kept: &KeptFlow) -> Result<(), Fault> {
        self.store.keep_flow(kept)
    }

    fn outlines(&self) -> Result<Vec<KeptOutline>, Fault> {
        self.store.outlines()
    }

    fn change(
        &mut self,
        at: Time,
        why: Option<&str>,
        changes: &[(ItemNumber, ItemChange)],
        works: &[(WorkNumber, &Work)],
    ) -> Result<(), Fault> {
        self.store.change(at, why, changes, works)
    }

    fn items(&self) -> Result<Vec<Item>, Fault> {
        self.store.items()
    }

    fn works(&self) -> Result<Vec<WorkHead>, Fault> {
        self.store.works()
    }

    fn load(&self, work: WorkNumber) -> Result<Work, Fault> {
        let mut loaded = self.store.load(work)?;
        if work == WorkNumber(1) {
            loaded.given.clear();
        }
        Ok(loaded)
    }

    fn save(&mut self, work: WorkNumber, given: &Work) -> Result<(), Fault> {
        self.store.save(work, given)
    }

    fn everything_paused(&self) -> Result<Option<EverythingPaused>, Fault> {
        self.store.everything_paused()
    }

    fn pause_everything(&mut self, paused: bool, by: Pauser, at: Time) -> Result<(), Fault> {
        self.store.pause_everything(paused, by, at)
    }

    fn everything_pauses(&self) -> Result<Vec<PauseOfEverything>, Fault> {
        self.store.everything_pauses()
    }

    fn keeping_warm(&self) -> Result<bool, Fault> {
        self.store.keeping_warm()
    }

    fn keep_warm(&mut self, warm: bool, at: Time) -> Result<(), Fault> {
        self.store.keep_warm(warm, at)
    }

    fn moving_on(&self) -> Result<bool, Fault> {
        self.store.moving_on()
    }

    fn set_moving_on(&mut self, moving: bool, at: Time) -> Result<(), Fault> {
        self.store.set_moving_on(moving, at)
    }

    fn accounts(&self) -> Result<Vec<AccountRecord>, Fault> {
        self.store.accounts()
    }

    fn keep_account(&mut self, record: &AccountRecord) -> Result<(), Fault> {
        self.store.keep_account(record)
    }

    fn stops(&self) -> Result<Vec<StopRecord>, Fault> {
        self.store.stops()
    }

    fn keep_stop(&mut self, record: &StopRecord) -> Result<(), Fault> {
        self.store.keep_stop(record)
    }
}

/// Keeps in the store at this path two works on the trial flow before the loop's clock starts: the first ran its plan
/// as the owner paused it, and stands paused at the next cell; the second was added and not let on.
fn kept(path: &Path) {
    let at = |ms: u64| Time(START - 10_000 + ms);
    let flow = flow::compile(&trial_flow(), &trial_has(&flow::Source::default())).unwrap();
    let store = Store::open(path).unwrap();
    let (mut driver, _) = driver::Driver::open(Box::new(store), vec![flow], 2, None, at(0)).unwrap();
    for title in ["First", "Second"] {
        let new = driver::NewItem {
            title: title.to_string(),
            level: "2".to_string(),
            project: "trial".to_string(),
            depends_on: Vec::new(),
            tags: Vec::new(),
            continues: None,
            flow: "trial".to_string(),
            writes: vec![driver::FirstWrite {
                block: BlockId("request".to_string()),
                content: "Please.".to_string(),
                value: None,
            }],
            why: None,
            waits_for: Vec::new(),
        };
        driver.add(at(0), new).unwrap();
    }
    let first = WorkNumber(1);
    let todos = driver.happened(at(1000), first, Event::Resumed).unwrap();
    let plan = (todos.iter())
        .find_map(|todo| match todo.effect {
            Effect::Round { run, .. } => Some(run),
            _ => None,
        })
        .expect("a round is given");
    let paused = Event::Paused {
        by: Pauser::Owner,
        why: None,
        waits_for: Vec::new(),
    };
    driver.happened(at(1500), first, paused).unwrap();
    let handed = engine::Handed {
        block: BlockId("plan".to_string()),
        content: "The plan.".to_string(),
        matters: true,
        value: None,
    };
    let came = Came::hands_in(vec![handed]);
    let spent = work::Spent::default();
    driver
        .happened(at(2000), first, Event::Returned { run: plan, came, spent })
        .unwrap();
}

/// What the three reads say of the two works, kept in a directory of its own: through the store as it is, or through
/// the one that cannot give the first work's rounds.
async fn read(unreadable: bool) -> (Value, Value, Value) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.sqlite");
    kept(&path);
    let store = Store::open(&path).unwrap();
    let store: Box<dyn Storage + Send> = match unreadable {
        true => Box::new(Unreadable { store }),
        false => Box::new(store),
    };
    let running = running_on(store, dir);
    let router = &running.router;
    (
        get_json(router, "/api/v1/items").await,
        get_json(router, "/api/v1/pulls").await,
        get_json(router, "/api/v1/attention").await,
    )
}

// Where the first work's rounds cannot be read, the items are still given: it says its state is broken, unread, with
// what is certain of it, and the second item is as it is where nothing is broken. Fails where the read answers 500, or
// where the broken work's summary is given as a work that did nothing, without saying that it is unread.
#[tokio::test(start_paused = true)]
async fn a_work_whose_rounds_cannot_be_read_is_given_broken_and_unread_and_the_other_items_as_they_are() {
    let (whole, ..) = read(false).await;
    let (broken, ..) = read(true).await;
    let (before, after) = (&whole["items"], &broken["items"]);
    assert_eq!(after.as_array().map(Vec::len), Some(2), "{broken}");
    // The work as it was read, paused at the cell after the plan, with the cost and the cells it has.
    assert_eq!(before[0]["work"]["now"]["kind"], "paused", "{whole}");
    assert!(before[0]["work"].get("unread").is_none(), "{whole}");
    assert!(!before[0]["work"]["cells"].as_array().unwrap().is_empty());
    // As it is given where its rounds cannot be read: what is certain of it, and nothing made up.
    let work = &after[0]["work"];
    assert_eq!(work["now"]["kind"], "broken", "{work}");
    let words = work["now"]["words"].as_str().expect("the words say why");
    assert!(words.contains("was given nothing"), "{words}");
    assert_eq!(work["unread"], json!(true), "{work}");
    assert_eq!(work["cells"], json!([]), "{work}");
    assert_eq!(work["waits"], json!([]), "{work}");
    for certain in ["number", "item", "flow", "added", "paused", "last", "level"] {
        assert_eq!(work[certain], before[0]["work"][certain], "{certain}: {work}");
    }
    // Its item is as it was, and so is every other item.
    let mut item = after[0].clone();
    item["work"] = before[0]["work"].clone();
    assert_eq!(item, before[0]);
    assert_eq!(
        after[1], before[1],
        "the other item is as it is where no work is broken"
    );
}

// The pull requests and what waits are read through the same items and the same works, and answer as well.
#[tokio::test(start_paused = true)]
async fn the_pull_requests_and_what_waits_are_given_where_a_work_cannot_be_read() {
    let (_, whole_pulls, whole_attention) = read(false).await;
    let (_, pulls, attention) = read(true).await;
    assert_eq!(pulls, whole_pulls);
    assert_eq!(attention, whole_attention);
}

// A broken work's summary is worked out when what it is made from changes, as any work's is, and not at each read: a
// second read builds none again, and gives the page nothing, so that what is said of it on the engine's standard error
// is said once for each time it is worked out. Fails where the cache works a broken work's summary out again at each
// read.
#[tokio::test(start_paused = true)]
async fn a_broken_work_is_worked_out_once_until_what_it_is_made_from_changes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.sqlite");
    kept(&path);
    let store = Unreadable {
        store: Store::open(&path).unwrap(),
    };
    let carriers = service::Scripted::new(crate::trial_script()).carriers();
    let (running, cache, _) = running_kept(
        Box::new(store),
        dir,
        carriers,
        &trial_flow(),
        levels(),
        web::items_cache::BOUND_MS,
    );
    let router = &running.router;
    let first = get_json(router, "/api/v1/items").await;
    assert_eq!(first["items"][0]["work"]["now"]["kind"], "broken", "{first}");
    assert_eq!(cache.built(), 2, "each item was worked out once");
    let cursor = first["cursor"].as_str().unwrap();
    let again = get_json(router, &format!("/api/v1/items?since={cursor}")).await;
    assert_eq!(again["items"], json!([]), "{again}");
    assert_eq!(cache.built(), 2, "nothing changed, so nothing was worked out again");
}
