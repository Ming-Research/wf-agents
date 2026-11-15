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

/// Every kind of state a build of these flows makes, by the words its list counts them by, with how many states of it
/// it makes: a state a change of a flow or of the build makes or no longer makes changes its count here in the same
/// change.
const KINDS: &[(&str, usize)] = &[
    ("under way", 0),
    ("waits for an answer", 0),
    ("waits for a tool", 0),
    ("waits for a precondition", 0),
    ("waits for an item it depends on", 0),
    ("waits for the owner at a limit", 0),
    ("waits for the outside", 0),
    ("waits for the engine's own question", 0),
    ("waits for a slot", 0),
    ("waits for the turn", 0),
    ("waits for what every work shares", 0),
    ("merged", 0),
    ("cancelled", 0),
    ("paused by the owner", 0),
    ("stalled", 0),
    ("never let on", 0),
    ("everything paused", 0),
    ("fails passing", 0),
    ("fails unread", 0),
    ("fails flagged", 0),
    ("fails guarded", 0),
    ("fails usage_limit", 0),
    ("fails cannot_run", 0),
    ("fails offline", 0),
    ("fails unavailable", 0),
    ("fails prompt_too_long", 0),
    ("fails no_model", 0),
    ("fails other", 0),
];

/// Why a state is not brought about, each a reason the build gives: every state its list says it did not make gives
/// one of these.
const NOT_MADE: &[&str] = &[
    "an act waits for no such state",
    "the engine's wait before a round is tried again",
    "no case of the exploration waits for an answer",
    "no case of the exploration has a round of the flow's first agent's cell",
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

/// How a store is served, as the build writes it beside the store.
#[derive(serde::Deserialize)]
struct Serving {
    data: PathBuf,
    flows: Vec<PathBuf>,
    script: PathBuf,
    held: bool,
    let_on: bool,
}

/// Serves a fresh copy of each store as its notes say: as it starts it holds every work where a round was left under
/// way, the hold after a restart, and nothing otherwise; let on where its notes say so; and of the main store, a work
/// whose round was left under way still has it under way.
fn served(built: &Built) {
    let stores = std::fs::read_dir(built.dir.path().join("built")).expect("the stores");
    let mut served = 0;
    for store in stores.flatten().filter(|s| s.path().is_dir()) {
        let notes = std::fs::read_to_string(store.path().join("serving.toml")).expect("how the store is served");
        let how: Serving = toml::from_str(&notes).expect("how the store is served reads");
        let copy = built
            .dir
            .path()
            .join(format!("served-{}", store.file_name().to_string_lossy()));
        std::fs::create_dir(&copy).expect("the copy's directory");
        for entry in std::fs::read_dir(&how.data).expect("the store's data directory") {
            let entry = entry.expect("an entry of it");
            std::fs::copy(entry.path(), copy.join(entry.file_name())).expect("a file of the store is copied");
        }
        let mut command = Command::new(env!("CARGO_BIN_EXE_wf-agents"));
        command.arg("run").arg("--data").arg(&copy);
        for flow in &how.flows {
            command.arg("--flow").arg(flow);
        }
        command.arg("--script").arg(&how.script).args(["--port", "0"]);
        let engine = Engine::start_with(command);
        let listed = engine.says(&["list", "--all"]);
        let name = store.file_name().to_string_lossy().into_owned();
        let held = listed.starts_with("The engine started with works going on and holds every work");
        assert_eq!(held, how.held, "{name}: {listed}");
        if how.let_on {
            engine.says(&["resume", "--all"]);
        }
        if name == "main" {
            engine.until(&["list"], |line| line.contains("is under way"));
        }
        served += 1;
    }
    assert!(served > 5, "{served} stores served");
}

/// How many states of this kind the list counts as made.
fn made(list: &str, kind: &str) -> usize {
    (list.lines())
        .find_map(|line| line.strip_prefix(&format!("- {kind}: ")))
        .and_then(|rest| rest.split(' ').next())
        .and_then(|n| n.parse().ok())
        .unwrap_or(0)
}

// Every state the build lists as made was reached by the item it names, each kind of state was made as often as the
// flows give it, and each state not brought about is so for a reason the build gives; each store, served as its notes
// say, starts as they say. The build is the product's command, and builds here what the test installation builds,
// exploring the engineer's flow at the bound its own tests explore it at: built at a smaller one, the page's states
// would be other ones (Article 3). A minute and a half or so alone, the main store's play the most of it; the only
// test that builds the repository's flows.
#[test]
fn every_state_the_build_lists_is_reached_and_every_kind_has_one() {
    let started = Instant::now();
    let flows = [repository().join("engineer"), repository().join("deliver"), stalled()];
    let built = build(&flows);
    let list = &built.list;
    assert!(built.ok, "{}\n{list}", built.said);
    let counted: Vec<(&str, usize)> = KINDS.iter().map(|(kind, _)| (*kind, made(list, kind))).collect();
    assert_eq!(
        counted, KINDS,
        "the kinds of state made, against what these flows give\n{list}"
    );
    let unmade: Vec<&str> = (list.lines())
        .skip_while(|line| *line != "## Not brought about")
        .skip(1)
        .take_while(|line| !line.starts_with("# "))
        .filter(|line| line.starts_with("- ") && !NOT_MADE.iter().any(|why| line.contains(why)))
        .collect();
    assert!(
        unmade.is_empty(),
        "not brought about for no reason the build gives: {unmade:#?}"
    );
    println!("built in {:?}", started.elapsed());
    served(&built);
}
