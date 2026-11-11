//! A work whose round fails, as the commands say it: paused by the engine with the reason, its cell's round failed;
//! and, let on, it goes on to its merge.

use crate::{Engine, Place, SCRIPT};

const WHY: &str = "the agent's program could not start";

#[test]
fn a_failed_round_and_the_engines_pause_are_said_with_the_reason() {
    let asks = r#"asks = [{ question = "Should the greeting name the user?" }]"#;
    assert!(SCRIPT.contains(asks));
    let place = Place::new(&SCRIPT.replace(asks, &format!("fails = \"{WHY}\"")));
    let engine = Engine::start(&place);
    engine.says(&[
        "add",
        "--held",
        "--title",
        "Greet the user",
        "--flow",
        "trial",
        "--write",
        "request=Greet the user, please.",
    ]);
    assert_eq!(engine.says(&["resume", "1"]), "Work 1 goes on.\n");
    let paused = format!("Item 1 \"Greet the user\" at level 1, work 1 on trial: paused by the engine: {WHY}");
    engine.until(&["list"], |line| line == paused);
    let shown = engine.says(&["show", "1"]);
    assert!(
        shown.contains(&format!("\nNow: paused by the engine: {WHY}.\nPaused: yes.\n")),
        "{shown}"
    );
    assert!(
        shown.contains(&format!(
            "\n  plan: its last round failed in run 1, and is given again once the work is let on: {WHY}\n"
        )),
        "{shown}"
    );
    assert!(
        shown.contains(&format!(
            "\n  run 1 of plan: 1 round, open, its last round failed: {WHY}"
        )),
        "{shown}"
    );

    assert_eq!(engine.says(&["resume", "1"]), "Work 1 goes on.\n");
    engine.until(&["list"], |line| {
        line == "Item 1 \"Greet the user\" at level 1, work 1 on trial: merged"
    });
}
