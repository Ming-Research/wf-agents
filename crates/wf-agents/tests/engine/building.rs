//! `wf-agents play --states`: the build of every state of the page (design document `testing`, 18 and 19), on the
//! repository's flows and on the stalling flow beside the test flows, with items the test makes up and a fixed start:
//! every state its list names as made was reached by the item it names, and each kind of state the exploration and the
//! rules give was made by one at least.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

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
    "waits for a result",
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

/// The build of these flows into a data directory of its own: whether it succeeded, what it printed, and its list.
fn build(flows: &[PathBuf]) -> (bool, String, String) {
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
    let out = command.output().expect("the command runs");
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let mut list = std::fs::read_to_string(data.join("states.md")).unwrap_or_default();
    for entry in std::fs::read_dir(&data).into_iter().flatten().flatten() {
        if let Ok(store) = std::fs::read_to_string(entry.path().join("states.md")) {
            list.push_str(&store);
        }
    }
    (out.status.success(), said, list)
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
    let (ok, said, list) = build(&flows);
    assert!(ok, "{said}\n{list}");
    let none: Vec<&&str> = KINDS.iter().filter(|kind| made(&list, kind) == 0).collect();
    assert!(none.is_empty(), "no state of these kinds was made: {none:?}\n{list}");
    println!("built in {:?}", started.elapsed());
}
