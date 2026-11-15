//! `wf-agents play --states`: the build of every state of the page (design document `testing`, 18 and 19), on the
//! repository's flows and on the stalling flow beside the test flows, with items the test makes up and a fixed start:
//! every state its list names as made was reached by the item it names, and each kind of state the exploration and the
//! rules give was made by one at least.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use crate::Engine;

/// The directory of the repository's flows.
fn repository() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../flow")
}

/// The stalling flow beside the test flows.
fn stalled() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../flow/tests/laid_out/stalled")
}

/// The time the build's clock starts at: 2026-11-10, at midnight.
const START: u64 = 1_794_268_800_000;

/// Every kind of state a build of these flows makes, by the words its list counts them by.
const KINDS: &[&str] = &[
    "under way",
    "waits for an answer",
    "waits for a tool",
    "waits for a precondition",
    "waits for an item it depends on",
    "waits for the owner at a limit",
    "waits for the outside",
    "waits for the engine's own question",
    "waits for a slot",
    "merged",
    "cancelled",
    "paused by the owner",
    "stalled",
    "never let on",
    "everything paused",
    "fails passing",
    "fails unread",
    "fails flagged",
    "fails guarded",
    "fails usage_limit",
    "fails cannot_run",
    "fails offline",
    "fails unavailable",
    "fails prompt_too_long",
    "fails no_model",
    "fails other",
];

/// An items file of the installation's data, made up: enough items of each flow, every fifth depending on the one
/// before it.
fn items(dir: &Path) -> PathBuf {
    let mut text = String::new();
    let mut place = 0;
    for flow in ["engineer", "deliver", "stalled"] {
        for n in 1..=200 {
            place += 1;
            text.push_str(&format!(
                "[[items]]\ntitle = \"The {flow} item {n}\"\nbackground = \"What the {flow} item {n} is for.\"\n\
                 flow = \"{flow}\"\n"
            ));
            if n % 5 == 0 {
                text.push_str(&format!("depends_on = [{}]\n", place - 1));
            }
        }
    }
    let file = dir.join("test.toml");
    std::fs::write(&file, text).expect("the items are written");
    file
}

/// What a build left: whether it succeeded, what it printed, its list and every store's, and the directory that holds
/// its data directory, `built`.
struct Built {
    ok: bool,
    said: String,
    list: String,
    dir: tempfile::TempDir,
}

/// The build of these flows into a data directory of its own.
fn build(flows: &[PathBuf]) -> Built {
    let dir = tempfile::tempdir().expect("a directory");
    let data = dir.path().join("built");
    std::fs::create_dir(&data).expect("the data directory");
    let items = items(dir.path());
    let mut command = Command::new(env!("CARGO_BIN_EXE_wf-agents"));
    command.args(["play", "--states", "--start", &START.to_string()]);
    command.arg("--items").arg(&items).arg("--data").arg(&data);
    for flow in flows {
        command.arg("--flow").arg(flow);
    }
    // Stopped short of the test runner's limit for it, so that what it said so far is shown.
    let printed = dir.path().join("printed");
    let file = std::fs::File::create(&printed).expect("the file of what it prints");
    let mut child = (command.stdout(file.try_clone().expect("the file again")).stderr(file))
        .spawn()
        .expect("the command runs");
    let began = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().expect("the command is waited for") {
            break Some(status);
        }
        if began.elapsed().as_secs() > 280 {
            let _ = child.kill();
            break None;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    };
    let said = std::fs::read_to_string(&printed).unwrap_or_default();
    let said = match status {
        Some(_) => said,
        None => format!("{said}\nstopped after 280 s"),
    };
    let mut list = std::fs::read_to_string(data.join("states.md")).unwrap_or_default();
    for entry in std::fs::read_dir(&data).into_iter().flatten().flatten() {
        if let Ok(store) = std::fs::read_to_string(entry.path().join("states.md")) {
            list.push_str(&store);
        }
    }
    Built {
        ok: status.is_some_and(|s| s.success()),
        said,
        list,
        dir,
    }
}

/// Serves a fresh copy of the main store as its notes say: as it starts it holds every work, the hold after a restart
/// (a round having been left under way), and let on, a work whose round was left under way still has it under way.
fn served(built: &Built, flows: &[PathBuf]) {
    let main = built.dir.path().join("built/main");
    let copy = built.dir.path().join("served");
    std::fs::create_dir(&copy).expect("the copy's directory");
    for entry in std::fs::read_dir(main.join("data")).expect("the store's data directory") {
        let entry = entry.expect("an entry of it");
        std::fs::copy(entry.path(), copy.join(entry.file_name())).expect("a file of the store is copied");
    }
    let mut command = Command::new(env!("CARGO_BIN_EXE_wf-agents"));
    command.arg("run").arg("--data").arg(&copy);
    for flow in flows {
        command.arg("--flow").arg(flow);
    }
    command
        .arg("--script")
        .arg(main.join("serve/test.toml"))
        .args(["--port", "0"]);
    let engine = Engine::start_with(command);
    let held = engine.says(&["list"]);
    assert!(
        held.starts_with("The engine started with works going on and holds every work"),
        "{held}"
    );
    engine.says(&["resume", "--all"]);
    engine.until(&["list"], |line| line.contains("is under way"));
}

/// How many states of this kind the list counts as made.
fn made(list: &str, kind: &str) -> usize {
    (list.lines())
        .find_map(|line| line.strip_prefix(&format!("- {kind}: ")))
        .and_then(|rest| rest.split(' ').next())
        .and_then(|n| n.parse().ok())
        .unwrap_or(0)
}

// Every state the build lists as made was reached by the item it names, and each kind of state was made by one at
// least: the build fails where a state it played was not reached. The repository's flows are explored, and each store
// played, in this one test alone; it prints its time.
#[test]
fn every_state_the_build_lists_is_reached_and_every_kind_has_one() {
    let started = Instant::now();
    let flows = [repository().join("engineer"), repository().join("deliver"), stalled()];
    let built = build(&flows);
    let list = &built.list;
    assert!(built.ok, "{}\n{list}", built.said);
    let none: Vec<&&str> = KINDS.iter().filter(|kind| made(list, kind) == 0).collect();
    assert!(none.is_empty(), "no state of these kinds was made: {none:?}\n{list}");
    let moved = list
        .lines()
        .any(|line| line.starts_with("- main-moved: ") && !line.starts_with("- main-moved: 0 "));
    assert!(moved, "no state was made in the store where main moves\n{list}");
    println!("built in {:?}", started.elapsed());
    served(&built, &flows[..2]);
}
