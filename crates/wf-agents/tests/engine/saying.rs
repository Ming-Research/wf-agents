//! `wf-agents say`: the owner's words to a line of a work, given to the cell the engine chooses, which the command says,
//! or refused with why.

use std::path::{Path, PathBuf};

use crate::{Engine, Place, run_on, trial};

/// A script for the trial flow whose rounds take a few milliseconds: the build asks the owner, and so holds the work.
const SCRIPT: &str = r#"
[took]
ms = 20

[[flows.trial.rounds]]
cell = "build"
asks = [{ question = "Which name does the greeting use?", who = "owner" }]
"#;

/// The trial flow, its build run by the owner's words, in a directory of the place, with what is written beside it.
fn spoken_to(place: &Place) -> PathBuf {
    let beside = place.dir.path().join("agents");
    std::fs::create_dir(&beside).unwrap();
    let waits = trial().join("../agents/waits.toml");
    std::fs::copy(&waits, beside.join("waits.toml")).unwrap();
    let flow = place.dir.path().join("flow");
    std::fs::create_dir(&flow).unwrap();
    for entry in std::fs::read_dir(trial()).unwrap() {
        let path = entry.unwrap().path();
        std::fs::copy(&path, flow.join(path.file_name().unwrap())).unwrap();
    }
    let edit = |file: &Path, from: &str, to: &str| {
        let text = std::fs::read_to_string(file).unwrap();
        assert!(text.contains(from), "{}: {from}", file.display());
        std::fs::write(file, text.replacen(from, to, 1)).unwrap();
    };
    edit(
        &flow.join("build.toml"),
        "\"answered\"]",
        "\"answered\", \"build_spoken_to\"]",
    );
    let declared = flow.join("flow.toml");
    let text = std::fs::read_to_string(&declared).unwrap();
    std::fs::write(
        &declared,
        format!("{text}\n[blocks.build_spoken_to]\noutside = \"words\"\n"),
    )
    .unwrap();
    flow
}

// The owner's words to a line go to the cell the engine gives them to, the one holding the work on that line, which
// the act answers with and the command says; words to a line none of whose cells the owner's words run are refused
// with why. A few seconds alone, the engine run as its program on a script of short rounds; no other test says words
// through the command or the act's route.
#[test]
fn words_to_a_line_are_said_to_the_cell_the_engine_gave_them_to_and_refused_where_none_takes_them() {
    let place = Place::new(SCRIPT);
    let flow = spoken_to(&place);
    let engine = Engine::start_with(run_on(&place.data(), &flow, &place.script(), 0));
    engine.says(&[
        "add",
        "--title",
        "Greet the user",
        "--flow",
        "trial",
        "--project",
        "script",
        "--write",
        "request=Greet the user, please.",
    ]);
    engine.says(&["resume", "1"]);
    let waits = engine.says(&["wait"]);
    assert!(waits.contains("waits for the owner at build for "), "{waits}");
    assert_eq!(
        engine.says(&["say", "1", "Keep it short.", "--line", "engineer"]),
        "Said to build of work 1, where the line engineer stands.\n"
    );
    let refused = engine.refuses(&["say", "1", "Keep it short.", "--line", "reviewer"]);
    assert!(
        refused.contains("the owner's words run no cell of the line reviewer"),
        "{refused}"
    );
}
