//! `GET /api/v1/items` as the items are kept between reads: an item nothing happened to is not worked out again, a page
//! that gives the cursor it was last given is given only what changed since, what git says of a work is looked at off
//! the read and told when it ended, and what is kept is what a whole working out gives.

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use decide::run::Failure;
use serde_json::{Value, json};
use service::{Along, Diffstat, OfWork, Round, RoundReply, Rounds, Scripted, Seen};
use store::Store;
use tower::ServiceExt;
use web::items_cache::{BOUND_MS, ItemsCache};

use crate::{Running, get_json, greeted, levels, next, pass, post, projects, running_kept, trial_flow, trial_script};

/// The CI run a round is said to stand as, where the test has it said.
const RUN: &str = "https://ci.example/runs/7";

/// The trial's acts, whose repository says each work changed one file and as many lines as the test says, as the
/// test lets it: a look that waits until it is let end, or fails; and how many looks were asked of it. Where the test
/// sets `link_after`, each round begun is said to stand as a CI run that many milliseconds after it began, which no
/// record of the work tells.
struct Looking {
    acts: Arc<dyn Rounds>,
    link_after: AtomicU64,
    asked: AtomicUsize,
    lines: AtomicU64,
    waits: AtomicBool,
    fails: AtomicBool,
    let_end: tokio::sync::Semaphore,
}

impl Rounds for Looking {
    fn give(&self, round: Round, reply: RoundReply) {
        let after = self.link_after.load(Ordering::SeqCst);
        if after > 0 {
            let saying = reply.saying(round.number);
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(after)).await;
                saying.say(Some(Along::Runs { at: RUN.to_string() }));
            });
        }
        self.acts.give(round, reply);
    }

    fn stop(&self, round: Round, reply: RoundReply) {
        self.acts.stop(round, reply);
    }

    fn seen(&self, _: OfWork) -> Pin<Box<dyn Future<Output = Result<Seen, String>> + Send + '_>> {
        Box::pin(async move {
            self.asked.fetch_add(1, Ordering::SeqCst);
            if self.waits.load(Ordering::SeqCst) {
                self.let_end.acquire().await.expect("the semaphore is open").forget();
            }
            if self.fails.load(Ordering::SeqCst) {
                return Err("git is not there".to_string());
            }
            Ok(Seen {
                changed: Some(Diffstat {
                    files: 1,
                    added: self.lines.load(Ordering::SeqCst),
                    removed: 0,
                }),
                pull: None,
            })
        })
    }
}

impl Looking {
    fn asked(&self) -> usize {
        self.asked.load(Ordering::SeqCst)
    }
}

/// The trial flow run with this script, its acts' repository as [`Looking`] says it, its items kept this long where
/// something is under way.
fn kept(script: service::Script, bound: u64) -> (Running, Arc<ItemsCache>, service::Handle, Arc<Looking>) {
    // Slots enough that one work's round takes none that another waits for, unless the script sets them.
    let mut script = script;
    script.slots = script.slots.or(Some(8));
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("store.sqlite")).unwrap();
    let mut carriers = Scripted::new(script).carriers();
    let looking = Arc::new(Looking {
        acts: carriers.acts.clone(),
        link_after: AtomicU64::new(0),
        asked: AtomicUsize::new(0),
        lines: AtomicU64::new(3),
        waits: AtomicBool::new(false),
        fails: AtomicBool::new(false),
        let_end: tokio::sync::Semaphore::new(0),
    });
    carriers.acts = looking.clone();
    let (running, cache, handle) = running_kept(Box::new(store), dir, carriers, &trial_flow(), levels(), bound);
    (running, cache, handle, looking)
}

/// An item of the trial flow, added held.
fn item(title: &str, on: Value, project: &str) -> Value {
    json!({
        "title": title,
        "dependsOn": on,
        "project": project,
        "flow": "trial",
        "writes": [{ "block": "request", "content": "Please.", "value": null }],
    })
}

/// The items read, as the page reads them: with this cursor, or with none.
async fn read(router: &Router, since: Option<&str>) -> Value {
    match since {
        Some(since) => get_json(router, &format!("/api/v1/items?since={since}")).await,
        None => get_json(router, "/api/v1/items").await,
    }
}

/// The numbers of the items a read gives.
fn numbers(read: &Value) -> Vec<u64> {
    (read["items"].as_array().unwrap())
        .iter()
        .map(|item| item["number"].as_u64().unwrap())
        .collect()
}

fn cursor(read: &Value) -> String {
    read["cursor"].as_str().unwrap().to_string()
}

const GO_ON: &str = r#"{ "kind": "goOn", "call": 2 }"#;

// A page reads everything once and then what changed; an item nothing happened to is not worked out again, and its
// work, which the act on another did not touch, is not given again.
#[tokio::test(start_paused = true)]
async fn an_item_nothing_happened_to_is_not_worked_out_again_and_not_given_again() {
    let (running, cache, _, _) = kept(trial_script(), BOUND_MS);
    let router = &running.router;
    greeted(router).await;
    assert_eq!(
        post(router, "/api/v1/items", &item("Second", json!([]), "trial"))
            .await
            .0,
        StatusCode::OK
    );
    let first = read(router, None).await;
    assert_eq!((first["full"].clone(), numbers(&first)), (json!(true), vec![1, 2]));
    let built = cache.built();
    assert_eq!(built, 2, "each item was worked out once");

    pass(2000).await;
    let quiet = read(router, Some(&cursor(&first))).await;
    assert_eq!((quiet["full"].clone(), numbers(&quiet)), (json!(false), vec![]));
    assert_eq!(cursor(&quiet), cursor(&first));
    assert_eq!(cache.built(), built, "nothing was worked out again");

    // The owner lets the first work go on: it begins a round, which the second does not.
    let act: Value = serde_json::from_str(GO_ON).unwrap();
    assert_eq!(
        post(router, "/api/v1/works/1/acts", &act).await.0,
        StatusCode::NO_CONTENT
    );
    let moved = read(router, Some(&cursor(&first))).await;
    assert_eq!(numbers(&moved), vec![1]);
    assert_eq!(
        cache.built(),
        built + 1,
        "only the work that moved was worked out again"
    );
    assert_ne!(cursor(&moved), cursor(&first));
}

// A cursor that is of another process, or that cannot be read, or is ahead of the items, gives every item and says so.
#[tokio::test(start_paused = true)]
async fn a_cursor_of_another_process_or_unreadable_gives_every_item_and_says_so() {
    let (running, _, _, _) = kept(trial_script(), BOUND_MS);
    let router = &running.router;
    greeted(router).await;
    let first = read(router, None).await;
    let own = cursor(&first);
    let (boot, _) = own.split_once('.').unwrap();
    for since in [
        "garbage".to_string(),
        "ffff.1".to_string(),
        "x.y".to_string(),
        String::new(),
        format!("{boot}.999999"),
    ] {
        let read = read(router, Some(&since)).await;
        assert_eq!(
            (read["full"].clone(), numbers(&read)),
            (json!(true), vec![1]),
            "{since:?}"
        );
    }
    let own_again = read(router, Some(&own)).await;
    assert_eq!((own_again["full"].clone(), numbers(&own_again)), (json!(false), vec![]));
}

// What whoever plans the work changes of an item reaches the page as that item alone.
#[tokio::test(start_paused = true)]
async fn a_change_to_one_item_reaches_the_page_and_no_other_item_is_given() {
    let (running, cache, _, _) = kept(trial_script(), BOUND_MS);
    let router = &running.router;
    for (title, on) in [("First", json!([])), ("Second", json!([])), ("Third", json!([]))] {
        assert_eq!(
            post(router, "/api/v1/items", &item(title, on, "trial")).await.0,
            StatusCode::OK
        );
    }
    let first = read(router, None).await;
    let built = cache.built();
    let asked = json!({
        "why": "The third waits for the first",
        "changes": [
            { "kind": "title", "item": 3, "title": "Third, narrowed" },
            { "kind": "dependsOn", "item": 3, "on": 1 },
        ],
    });
    assert_eq!(
        post(router, "/api/v1/items/changes", &asked).await,
        (StatusCode::OK, json!({ "changed": [true, true] }))
    );
    let moved = read(router, Some(&cursor(&first))).await;
    assert_eq!(numbers(&moved), vec![3]);
    assert_eq!(moved["items"][0]["title"], "Third, narrowed");
    assert_eq!(moved["items"][0]["dependsOn"], json!([1]));
    assert_eq!(cache.built(), built + 1);
}

// The items a page holds, a whole read and then what each read gave since, are what a working out of every item gives,
// which a cache of its own does as it reads them from nothing: after each step of a history that takes in what every
// summary is made from.
struct Page {
    items: BTreeMap<u64, Value>,
    cursor: Option<String>,
}

impl Page {
    async fn read(&mut self, router: &Router) {
        let read = read(router, self.cursor.as_deref()).await;
        if read["full"] == json!(true) {
            self.items.clear();
        }
        for item in read["items"].as_array().unwrap() {
            self.items.insert(item["number"].as_u64().unwrap(), item.clone());
        }
        self.cursor = Some(cursor(&read));
    }

    /// Reads and says whether what the page holds is what a cache that has nothing gives. A work with a round or a call
    /// under way has its time counted to now when it is worked out, so that its time bar is left out where it has one.
    async fn is_whole(&mut self, router: &Router, handle: &service::Handle, at: &str) {
        self.read(router).await;
        pass(50).await;
        let whole = web::interface::router(handle.clone(), levels(), projects());
        let from_nothing = read(&whole, None).await;
        let mut wholly: BTreeMap<u64, Value> = BTreeMap::new();
        for item in from_nothing["items"].as_array().unwrap() {
            wholly.insert(item["number"].as_u64().unwrap(), item.clone());
        }
        let unbarred = |items: &BTreeMap<u64, Value>| -> BTreeMap<u64, Value> {
            items
                .iter()
                .map(|(number, item)| {
                    let mut item = item.clone();
                    let under_way = (item["work"]["cells"].as_array().unwrap())
                        .iter()
                        .any(|cell| cell["state"] == "running" || cell["state"] == "calling");
                    if under_way {
                        item["work"]["bar"] = Value::Null;
                    }
                    (*number, item)
                })
                .collect()
        };
        assert_eq!(unbarred(&self.items), unbarred(&wholly), "{at}");
    }
}

#[tokio::test(start_paused = true)]
async fn the_items_a_page_holds_are_what_a_working_out_of_every_item_gives_after_each_step() {
    let (running, _, handle, _) = kept(trial_script(), BOUND_MS);
    let router = &running.router;
    let mut page = Page {
        items: BTreeMap::new(),
        cursor: None,
    };
    for (title, on, project) in [
        ("First", json!([]), "trial"),
        ("Second", json!([1]), "trial"),
        ("Third", json!([]), "other"),
    ] {
        assert_eq!(
            post(router, "/api/v1/items", &item(title, on, project)).await.0,
            StatusCode::OK
        );
    }
    page.is_whole(router, &handle, "the items added").await;

    // A work let on begins a round.
    let resume = json!({ "kind": "resume" });
    assert_eq!(
        post(router, "/api/v1/works/1/acts", &resume).await.0,
        StatusCode::NO_CONTENT
    );
    pass(500).await;
    page.is_whole(router, &handle, "a round under way").await;
    assert_eq!(page.items[&1]["work"]["now"]["kind"], "round");

    // Its run goes round to its limit, and the work waits for the owner.
    pass(6000).await;
    page.is_whole(router, &handle, "a work waiting for the owner").await;
    assert_eq!(page.items[&1]["work"]["now"]["kind"], "waits");

    // The pause of everything holds every work, and lifted does not.
    let pause = json!({ "kind": "pause" });
    assert_eq!(
        post(router, "/api/v1/engine/acts", &pause).await.0,
        StatusCode::NO_CONTENT
    );
    page.is_whole(router, &handle, "the pause of everything").await;
    assert_eq!(
        post(router, "/api/v1/engine/acts", &json!({ "kind": "resume" }))
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    page.is_whole(router, &handle, "the pause lifted").await;

    // An item is retitled.
    let asked = json!({
        "why": "Say what it is",
        "changes": [{ "kind": "title", "item": 3, "title": "Third, narrowed" }],
    });
    assert_eq!(post(router, "/api/v1/items/changes", &asked).await.0, StatusCode::OK);
    page.is_whole(router, &handle, "an item retitled").await;
    assert_eq!(page.items[&3]["title"], "Third, narrowed");

    // The owner lets the first work go on to its merge, and the second, which waited for it, is let on by its end.
    let go_on: Value = serde_json::from_str(GO_ON).unwrap();
    assert_eq!(
        post(router, "/api/v1/works/1/acts", &go_on).await.0,
        StatusCode::NO_CONTENT
    );
    for step in 0..240 {
        pass(250).await;
        page.is_whole(router, &handle, &format!("step {step} to the merge"))
            .await;
        if page.items[&1]["work"]["now"]["kind"] == "merged" {
            break;
        }
    }
    assert_eq!(page.items[&1]["work"]["now"]["kind"], "merged");
    assert_eq!(page.items[&2]["work"]["now"]["kind"], "paused");
    page.is_whole(router, &handle, "the merge").await;
}

// What holds every work at once changes with no record of any work: the agents' command line cannot run, and the engine's
// check finds it well again. The items a page holds are what a working out of every item gives all the while.
#[tokio::test(start_paused = true)]
async fn the_items_a_page_holds_are_what_a_working_out_of_every_item_gives_while_the_agents_wait_comes_and_goes() {
    let mut script = trial_script();
    let plan = script.flows.get_mut("trial").unwrap();
    let mut failing = plan.rounds[0].clone();
    failing.round = 4;
    failing.asks = None;
    failing.took = Default::default();
    failing.fails = Some("no account is in use".to_string());
    failing.failure = Some(Failure::CannotRun);
    plan.rounds.push(failing);
    let (running, _, handle, _) = kept(script, BOUND_MS);
    let router = &running.router;
    let mut page = Page {
        items: BTreeMap::new(),
        cursor: None,
    };
    greeted(router).await;
    assert_eq!(
        post(router, "/api/v1/items", &item("Second", json!([]), "trial"))
            .await
            .0,
        StatusCode::OK
    );
    page.is_whole(router, &handle, "a work waiting for the owner").await;
    let waits_for_agents = |page: &Page, number: u64| {
        (page.items[&number]["work"]["waits"].as_array().unwrap())
            .iter()
            .any(|wait| wait["key"] == "agents")
    };
    assert!(!waits_for_agents(&page, 1));

    let go_on: Value = serde_json::from_str(GO_ON).unwrap();
    assert_eq!(
        post(router, "/api/v1/works/1/acts", &go_on).await.0,
        StatusCode::NO_CONTENT
    );
    // Within the half minute before the engine's first check of the outside, the wait holds, and every work is told it.
    pass(5000).await;
    page.is_whole(router, &handle, "the agents' command line cannot run")
        .await;
    assert!(waits_for_agents(&page, 1));

    // The check finds it well: the wait no longer holds, with no record of either work.
    pass(40_000).await;
    page.is_whole(router, &handle, "the wait no longer holds").await;
    assert!(!waits_for_agents(&page, 1));
}

// A round said to stand as a CI run, after it began: no record of the work tells it, and its summary says where it runs.
#[tokio::test(start_paused = true)]
async fn the_items_a_page_holds_are_what_a_working_out_of_every_item_gives_when_a_round_is_said_to_run_as_a_ci_run() {
    let script = trial_script_text().replacen("ms = 1500", "ms = 600000", 1);
    let (running, _, handle, looking) = kept(service::Script::read(&script).unwrap(), BOUND_MS);
    let router = &running.router;
    looking.link_after.store(2000, Ordering::SeqCst);
    let mut page = Page {
        items: BTreeMap::new(),
        cursor: None,
    };
    assert_eq!(
        post(router, "/api/v1/items", &item("First", json!([]), "trial"))
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        post(router, "/api/v1/works/1/acts", &json!({ "kind": "resume" }))
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    pass(1000).await;
    page.is_whole(router, &handle, "a round under way, not said to run as a CI run")
        .await;
    assert_eq!(page.items[&1]["work"]["now"]["kind"], "round");
    assert_eq!(page.items[&1]["work"]["now"]["at"], Value::Null);

    pass(2000).await;
    page.is_whole(router, &handle, "the round said to run as a CI run")
        .await;
    assert_eq!(page.items[&1]["work"]["now"]["at"], RUN);
}

// What holds every work at once changes what each says it waits for though no record of it grew: the engine is asked to
// stop while a work waits for a slot.
#[tokio::test(start_paused = true)]
async fn the_items_a_page_holds_are_what_a_working_out_of_every_item_gives_when_the_engine_is_asked_to_stop() {
    let script = trial_script_text().replacen("ms = 1500", "ms = 600000", 1);
    let mut script = service::Script::read(&script).unwrap();
    script.slots = Some(1);
    let (running, _, handle, _) = kept(script, BOUND_MS);
    let router = &running.router;
    let mut page = Page {
        items: BTreeMap::new(),
        cursor: None,
    };
    for title in ["First", "Second"] {
        assert_eq!(
            post(router, "/api/v1/items", &item(title, json!([]), "trial")).await.0,
            StatusCode::OK
        );
    }
    for work in [1, 2] {
        assert_eq!(
            post(
                router,
                &format!("/api/v1/works/{work}/acts"),
                &json!({ "kind": "resume" })
            )
            .await
            .0,
            StatusCode::NO_CONTENT
        );
        pass(500).await;
    }
    page.is_whole(router, &handle, "a work waiting for the slot the other takes")
        .await;
    assert_eq!(page.items[&2]["work"]["now"]["on"][0]["kind"], "slot");

    handle.stop(false).await.unwrap();
    page.is_whole(router, &handle, "the engine asked to stop").await;
    assert_eq!(page.items[&2]["work"]["now"]["on"][0]["kind"], "stopping");
}

// A work with a round under way changes with time and its worktree with no record: its summary is worked out again, and
// git asked again, once the bound has passed and not before; a work with nothing under way is not.
#[tokio::test(start_paused = true)]
async fn a_work_under_way_is_worked_out_again_after_the_bound_and_not_before() {
    let script = trial_script_text().replacen("ms = 1500", "ms = 600000", 1);
    let (running, cache, _, looking) = kept(service::Script::read(&script).unwrap(), 10_000);
    let router = &running.router;
    for title in ["First", "Second"] {
        assert_eq!(
            post(router, "/api/v1/items", &item(title, json!([]), "trial")).await.0,
            StatusCode::OK
        );
    }
    assert_eq!(
        post(router, "/api/v1/works/1/acts", &json!({ "kind": "resume" }))
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    pass(1000).await;
    let first = read(router, None).await;
    assert_eq!(first["items"][0]["work"]["now"]["kind"], "round", "{first}");
    let (built, asked) = (cache.built(), looking.asked());

    pass(3000).await;
    let before = read(router, Some(&cursor(&first))).await;
    assert_eq!(numbers(&before), Vec::<u64>::new());
    assert_eq!((cache.built(), looking.asked()), (built, asked), "not before the bound");

    // What its worktree holds changes at any moment: the look after the bound finds more lines.
    looking.lines.store(9, Ordering::SeqCst);
    pass(10_000).await;
    let after = read(router, Some(&cursor(&first))).await;
    pass(100).await;
    assert_eq!(
        cache.built(),
        built + 1,
        "the work under way was worked out again, the other was not"
    );
    assert_eq!(looking.asked(), asked + 1, "and git was asked of it");
    let found = read(router, Some(&cursor(&after))).await;
    assert_eq!(numbers(&found), vec![1]);
    assert_eq!(found["items"][0]["work"]["changed"]["added"], 9);
}

/// The trial flow's script, as its text.
fn trial_script_text() -> String {
    std::fs::read_to_string(crate::trial_dir().join("script.toml")).unwrap()
}

// A read does not wait for what git says of a work whose lines it has: it gives them as they were, starts one look,
// and the page is told, and given the new lines, once the look ends; a work no look has ended for is waited for.
#[tokio::test(start_paused = true)]
async fn a_read_does_not_wait_for_git_and_the_page_is_told_when_the_look_ends() {
    let (running, _, _, looking) = kept(trial_script(), BOUND_MS);
    let router = &running.router;
    greeted(router).await;
    let first = read(router, None).await;
    assert_eq!(first["items"][0]["work"]["changed"]["added"], 3);
    assert_eq!(looking.asked(), 1);
    let request = Request::builder().uri("/api/v1/changes").body(Body::empty()).unwrap();
    let mut stream = router.clone().oneshot(request).await.unwrap().into_body();
    next(&mut stream).await;

    // The work moves, which makes what git said old, and the look takes as long as the test lets it.
    looking.waits.store(true, Ordering::SeqCst);
    looking.lines.store(7, Ordering::SeqCst);
    let pause = json!({ "kind": "pause" });
    assert_eq!(
        post(router, "/api/v1/works/1/acts", &pause).await.0,
        StatusCode::NO_CONTENT
    );
    let answered = tokio::time::timeout(Duration::from_secs(1), read(router, Some(&cursor(&first)))).await;
    let held = answered.expect("the read does not wait for the look");
    assert_eq!(
        held["items"][0]["work"]["changed"]["added"], 3,
        "the lines as they were"
    );
    pass(10).await;
    assert_eq!(looking.asked(), 2);
    // A second read while the look runs starts no second one.
    let again = tokio::time::timeout(Duration::from_secs(1), read(router, Some(&cursor(&held))))
        .await
        .expect("nor does this one");
    pass(10).await;
    assert_eq!(looking.asked(), 2, "{again}");

    // The look ends: the stream says that items changed, and the next read gives the new lines.
    looking.let_end.add_permits(1);
    pass(100).await;
    let mut said = String::new();
    while !said.contains("\"items\":true") {
        said = next(&mut stream).await;
    }
    let found = read(router, Some(&cursor(&again))).await;
    assert_eq!(numbers(&found), vec![1]);
    assert_eq!(found["items"][0]["work"]["changed"]["added"], 7);
}

// A work no look has ended for has no lines to give, and the read waits for the first.
#[tokio::test(start_paused = true)]
async fn a_read_waits_for_the_first_look_at_a_work_and_for_no_other() {
    let (running, _, _, looking) = kept(trial_script(), BOUND_MS);
    let router = &running.router;
    looking.waits.store(true, Ordering::SeqCst);
    assert_eq!(
        post(router, "/api/v1/items", &item("First", json!([]), "trial"))
            .await
            .0,
        StatusCode::OK
    );
    let waiting = tokio::time::timeout(Duration::from_secs(5), read(router, None)).await;
    assert!(waiting.is_err(), "the read waits for the first look");
    looking.let_end.add_permits(1);
    pass(100).await;
    let read = read(router, None).await;
    assert_eq!(read["items"][0]["work"]["changed"]["added"], 3);
}

// A look that could not be made gives no lines, from the read on that follows it, and not those of what the work was:
// it is made again once the bound has passed and not at every read.
#[tokio::test(start_paused = true)]
async fn a_look_that_failed_gives_no_lines_and_is_made_again_after_the_bound() {
    let (running, _, _, looking) = kept(trial_script(), 10_000);
    let router = &running.router;
    greeted(router).await;
    let first = read(router, None).await;
    assert_eq!(first["items"][0]["work"]["changed"]["added"], 3);

    looking.fails.store(true, Ordering::SeqCst);
    let pause = json!({ "kind": "pause" });
    assert_eq!(
        post(router, "/api/v1/works/1/acts", &pause).await.0,
        StatusCode::NO_CONTENT
    );
    let held = read(router, Some(&cursor(&first))).await;
    assert_eq!(numbers(&held), vec![1]);
    assert_eq!(
        held["items"][0]["work"]["changed"]["added"], 3,
        "while the look runs, the lines as they were"
    );
    pass(100).await;
    let asked = looking.asked();
    let failed = read(router, Some(&cursor(&held))).await;
    assert_eq!(numbers(&failed), vec![1]);
    assert_eq!(
        failed["items"][0]["work"]["changed"],
        Value::Null,
        "the lines of what the work was are gone"
    );

    // Not made again at every read, but once the bound has passed.
    for _ in 0..3 {
        pass(1000).await;
        read(router, Some(&cursor(&failed))).await;
    }
    assert_eq!(looking.asked(), asked, "until the work moves again or the bound passes");
    looking.fails.store(false, Ordering::SeqCst);
    pass(10_000).await;
    read(router, None).await;
    pass(100).await;
    let mended = read(router, None).await;
    assert_eq!(mended["items"][0]["work"]["changed"]["added"], 3);
}

// The page of a work and the list give one summary of it, and the command line's list of pull requests reads the items
// kept too.
#[tokio::test(start_paused = true)]
async fn the_page_of_a_work_and_the_list_give_the_same_summary() {
    let (running, cache, _, _) = kept(trial_script(), BOUND_MS);
    let router = &running.router;
    greeted(router).await;
    let listed = read(router, None).await["items"][0]["work"].clone();
    let built = cache.built();
    let page = get_json(router, "/api/v1/works/1").await["summary"].clone();
    assert_eq!(page, listed);
    let (status, _, _) = crate::ask(router, axum::http::Method::GET, "/api/v1/pulls").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(cache.built(), built, "neither worked the summary out again");
}
