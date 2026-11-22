//! A work's page reads the version of its flow the work runs on: a work that ended, the version it ended on, whatever
//! flow is installed since, no flow, and the time it kept as it ended; a work going on, the version installed, and the
//! whole flow for what the owner does with it (decisions `a-works-past-is-drawn-from-its-outline` and
//! `a-work-keeps-what-its-time-went-to`).

use std::path::Path;

use engine::{Came, Effect, Event};
use serde_json::Value;
use service::Scripted;
use store::Store;

use crate::{START, edit, get_json, running_flow, trial_flow, trial_has, trial_script};

/// Keeps in the store at this path two works on the trial flow as it is, before the loop's clock starts: the first ran
/// its plan, the owner pausing it as the plan ran, was let on a second after the plan ended, and was cancelled a second
/// into its build, so that its time went to a model, the owner's pause and a model again, a second each; the second
/// added and not let on.
fn kept(path: &Path) {
    let at = |ms: u64| work::Time(START - 10_000 + ms);
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
            flow: "trial".to_string(),
            writes: vec![driver::FirstWrite {
                block: decide::BlockId("request".to_string()),
                content: "Please.".to_string(),
                value: None,
            }],
        };
        driver.add(at(0), new).unwrap();
    }
    let first = store::WorkNumber(1);
    let round = |todos: &[driver::Todo]| {
        (todos.iter())
            .find_map(|todo| match todo.effect {
                Effect::Round { run, .. } => Some(run),
                _ => None,
            })
            .expect("a round is given")
    };
    let plan = round(&driver.happened(at(1000), first, Event::Resumed).unwrap());
    let paused = Event::Paused {
        by: decide::Pauser::Owner,
        why: None,
    };
    driver.happened(at(1500), first, paused).unwrap();
    let handed = engine::Handed {
        block: decide::BlockId("plan".to_string()),
        content: "The plan.".to_string(),
        matters: true,
        value: None,
    };
    let came = Came::hands_in(vec![handed]);
    let spent = work::Spent::default();
    driver
        .happened(at(2000), first, Event::Returned { run: plan, came, spent })
        .unwrap();
    let build = round(&driver.happened(at(3000), first, Event::Resumed).unwrap());
    let cancel = vec![(store::ItemNumber(1), driver::Asked::Cancel)];
    driver.change(at(4000), Some("Not wanted"), cancel).unwrap();
    // The build's round, stopped, ends, and the work lets go of what it held.
    let came = Came::default();
    let todos = driver
        .happened(
            at(4000),
            first,
            Event::Returned {
                run: build,
                came,
                spent,
            },
        )
        .unwrap();
    assert!(
        todos.iter().any(|todo| matches!(todo.effect, Effect::Release { .. })),
        "{todos:?}"
    );
    driver.happened(at(4000), first, Event::Released).unwrap();
}

/// The title the page's outline gives the cell.
fn title<'a>(page: &'a Value, cell: &str) -> &'a Value {
    let cells = page["outline"]["cells"].as_array().expect("the outline's cells");
    &cells.iter().find(|c| c["id"] == cell).expect("the cell is outlined")["title"]
}

#[tokio::test(start_paused = true)]
async fn the_page_of_a_work_that_ended_reads_the_version_it_ended_on_and_of_one_going_on_the_version_installed() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.sqlite");
    kept(&path);
    // The review is called otherwise in the version installed since.
    let source = edit(
        trial_flow(),
        Some("review"),
        "title = \"Review\"",
        "title = \"Judge the build\"",
    );
    let carriers = Scripted::new(trial_script()).carriers();
    let running = running_flow(Box::new(Store::open(&path).unwrap()), dir, carriers, &source);
    let router = &running.router;

    let ended = get_json(router, "/api/v1/works/1").await;
    assert_eq!(title(&ended, "review"), "Review", "{}", ended["outline"]);
    assert_eq!(ended["outline"]["kept"], true);
    // A work that ended is given no flow, and no cell as the walk reads it now; each cell no run of which finished did
    // not run.
    assert_eq!(ended["flow"], Value::Null);
    assert_eq!(ended["cells"], serde_json::json!([]));
    let states: Vec<&Value> = (ended["summary"]["cells"].as_array().expect("its cells").iter())
        .map(|c| &c["state"])
        .collect();
    assert_eq!(states, ["done", "notRun", "notRun", "notRun"], "{}", ended["summary"]);
    // Its time is drawn from what it kept as it ended: a model's second, the owner's pause, and a model's again.
    let bar = ["m", "p", "m"].map(|mark| mark.repeat(32)).concat();
    assert_eq!(ended["summary"]["bar"], bar.as_str());

    let going = get_json(router, "/api/v1/works/2").await;
    assert_eq!(title(&going, "review"), "Judge the build");
    let flow_title = (going["flow"]["cells"].as_array().expect("the flow's cells").iter())
        .find(|c| c["id"] == "review")
        .map(|c| c["title"].clone());
    assert_eq!(flow_title, Some(Value::from("Judge the build")));
}
