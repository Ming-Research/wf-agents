//! A work's page reads the version of its flow the work runs on: a work that ended, the version it ended on, whatever
//! flow is installed since, and no flow; a work going on, the version installed, and the whole flow for what the owner
//! does with it (decision `a-works-past-is-drawn-from-its-outline`).

use std::path::Path;

use serde_json::Value;
use service::Scripted;
use store::Store;

use crate::{START, edit, get_json, running_flow, trial_flow, trial_has, trial_script};

/// Keeps in the store at this path two works on the trial flow as it is: the first cancelled before it ran, the second
/// added and not let on.
fn kept(path: &Path) {
    let flow = flow::compile(&trial_flow(), &trial_has(&flow::Source::default())).unwrap();
    let store = Store::open(path).unwrap();
    let (mut driver, _) = driver::Driver::open(Box::new(store), vec![flow], 2, None, work::Time(START)).unwrap();
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
        driver.add(work::Time(START), new).unwrap();
    }
    let cancel = vec![(store::ItemNumber(1), driver::Asked::Cancel)];
    driver.change(work::Time(START), Some("Not wanted"), cancel).unwrap();
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
    assert_eq!(ended["outline"]["card"], serde_json::json!(["request"]));
    // A work that ended is given no flow, and no cell as the walk reads it now; each of its cells did not run.
    assert_eq!(ended["flow"], Value::Null);
    assert_eq!(ended["cells"], serde_json::json!([]));
    let states: Vec<&Value> = (ended["summary"]["cells"].as_array().expect("its cells").iter())
        .map(|c| &c["state"])
        .collect();
    assert_eq!(states, ["notRun"; 4], "{}", ended["summary"]);

    let going = get_json(router, "/api/v1/works/2").await;
    assert_eq!(title(&going, "review"), "Judge the build");
    let flow_title = (going["flow"]["cells"].as_array().expect("the flow's cells").iter())
        .find(|c| c["id"] == "review")
        .map(|c| c["title"].clone());
    assert_eq!(flow_title, Some(Value::from("Judge the build")));
}
