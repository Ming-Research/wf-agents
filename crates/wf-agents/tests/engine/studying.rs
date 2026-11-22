//! Two flows side by side: the engine runs the engineer's flow and the study flow as the repository defines them, with
//! the test carriers. A study ends with its conclusion, which `wf-agents text` gives, and an engineer's item that
//! depends on the study starts once the study has ended.

use std::path::PathBuf;
use std::process::Command;

use crate::{Engine, Place};

/// A script whose rounds take a few milliseconds, and whose study hands in this conclusion.
const SCRIPT: &str = r#"
[took]
ms = 20

[[flows.study.rounds]]
cell = "R1"
hands_in = [{ block = "conclusion", content = "Make two items: the reader, then the writer." }]
"#;

/// A flow's definition in the repository.
fn defined(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../flow").join(name)
}

/// `wf-agents run` on the engineer's flow and the study flow, on any free port.
fn run_both(place: &Place) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_wf-agents"));
    command
        .arg("run")
        .arg("--data")
        .arg(place.data())
        .arg("--flow")
        .arg(defined("engineer"))
        .arg("--flow")
        .arg(defined("study"))
        .arg("--script")
        .arg(place.script())
        .args(["--port", "0"]);
    command
}

#[test]
fn a_study_ends_with_its_conclusion_which_the_architect_reads_and_lets_on_the_item_that_depends_on_it() {
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
            "background=What is to be studied.",
        ];
        args.extend(after.map(|after| ["--depends-on", after]).into_iter().flatten());
        engine.says(&args)
    };
    add("Study it", "study", None);
    add("Build on it", "engineer", Some("1"));
    engine.says(&["resume", "2"]);
    // The item that depends on the study waits for it to have ended, not for the owner.
    let list = engine.says(&["list"]);
    assert!(
        list.contains(
            "\n#2 \"Build on it\" at level 1, project script, W2 on engineer, after #1: waits at S1 for the items it \
             depends on to merge: #1\n"
        ),
        "{list}"
    );

    engine.says(&["resume", "1"]);
    engine.until(&["list", "--all"], |line| {
        line == "#2 \"Build on it\" at level 1, project script, W2 on engineer, after #1: merged"
    });
    let list = engine.says(&["list", "--all"]);
    assert!(
        list.contains("#1 \"Study it\" at level 1, project script, W1 on study: merged\n"),
        "{list}"
    );
    // What the study handed in stays in its item, for whoever splits the work after it.
    let shown = engine.says(&["show", "1"]);
    assert!(shown.contains("\n  conclusion: 1 version, the latest "), "{shown}");
    let text = engine.says(&["text", "1", "conclusion"]);
    assert!(text.contains("Make two items: the reader, then the writer."), "{text}");
}
