//! Two flows side by side: the engine runs the engineer's flow and the deliver flow as the repository defines them,
//! with the test carriers, and a deliverable made of an engineer's item is delivered once that item merges, and an
//! engineer's item that depends on the deliverable starts once it is delivered.

use std::path::PathBuf;
use std::process::Command;

use crate::{Engine, Place};

/// A script that says nothing of any cell, so that each round hands in what its cell writes, a choice with its first
/// value; its rounds take a few milliseconds.
const SCRIPT: &str = "[took]\nms = 20\n";

/// A flow's definition in the repository.
fn defined(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../flow").join(name)
}

/// `wf-agents run` on the engineer's flow and the deliver flow, on any free port.
fn run_both(place: &Place) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_wf-agents"));
    command
        .arg("run")
        .arg("--data")
        .arg(place.data())
        .arg("--flow")
        .arg(defined("engineer"))
        .arg("--flow")
        .arg(defined("deliver"))
        .arg("--script")
        .arg(place.script())
        .args(["--port", "0"]);
    command
}

#[test]
fn a_deliverable_made_of_an_engineers_item_is_delivered_once_it_merges_and_lets_on_what_depends_on_it() {
    let place = Place::new(SCRIPT);
    let engine = Engine::start_with(run_both(&place));
    let add = |title: &str, flow: &str, after: Option<&str>| {
        let mut args = vec![
            "add",
            "--title",
            title,
            "--flow",
            flow,
            "--project",
            "script",
            "--write",
            "background=What it is to deliver.",
        ];
        args.extend(after.map(|after| ["--depends-on", after]).into_iter().flatten());
        engine.says(&args)
    };
    add("Build it", "engineer", None);
    assert_eq!(
        add("Deliver it", "deliver", Some("1")),
        "Added item 2 with its work 2, paused: let it on with `wf-agents resume 2`.\n"
    );
    add("Build on it", "engineer", Some("2"));
    engine.says(&["resume", "3"]);
    engine.says(&["resume", "2"]);
    let list = engine.says(&["list"]);
    assert!(
        list.contains(
            "\n#2 \"Deliver it\" at level 1, W2 on deliver, after #1: waits at V1 for the outside \
             (dependencies): dependencies = merged\n"
        ),
        "{list}"
    );
    // The items it depends on merging, not the owner, let it start.
    assert!(
        list.contains(
            "\n#3 \"Build on it\" at level 1, W3 on engineer, after #2: waits at S1 for the outside \
             (dependencies): dependencies = merged\n"
        ),
        "{list}"
    );

    engine.says(&["resume", "1"]);
    engine.until(&["list", "--all"], |line| {
        line == "#3 \"Build on it\" at level 1, W3 on engineer, after #2: merged"
    });
    let list = engine.says(&["list", "--all"]);
    assert!(
        list.contains("#1 \"Build it\" at level 1, W1 on engineer: merged\n"),
        "{list}"
    );
    assert!(
        list.contains("#2 \"Deliver it\" at level 1, W2 on deliver, after #1: merged\n"),
        "{list}"
    );
    let shown = engine.says(&["show", "2"]);
    assert!(shown.contains("\n  V1: done, its record being run "), "{shown}");
    assert!(shown.contains("\n  delivered: 1 version, the latest "), "{shown}");
    assert!(
        shown.contains("\n  background: 1 version, the latest 22 characters, by a person\n"),
        "{shown}"
    );
}
