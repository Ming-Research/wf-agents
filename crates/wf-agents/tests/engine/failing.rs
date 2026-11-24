//! A work whose round fails, as the commands say it: paused by the engine with the reason, its cell's round failed;
//! and, let on, it goes on to its merge.

use crate::{Engine, Place, SCRIPT, plain_line};

const WHY: &str = "The agent's program could not start";

#[test]
fn a_failed_round_and_the_engines_pause_are_said_with_the_reason() {
    // The plan's first round fails in place of being sent back; the rounds after it are sent back, and no more than
    // twice, so that its run never reaches its limit of returns.
    let first = "cell = \"plan\"\nasks = []";
    assert!(SCRIPT.contains(first));
    let place = Place::new(&SCRIPT.replacen(first, &format!("cell = \"plan\"\nfails = \"{WHY}\""), 1));
    let engine = Engine::start(&place);
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
    assert_eq!(engine.says(&["resume", "1"]), "Work W1 goes on.\n");
    // The engine's pause and its reason, each a sentence.
    let paused = format!(
        "#1 (project script) \"Greet the user\" at level 1, W1 on trial: paused at plan by the engine. The round failed. The cause: {WHY}."
    );
    engine.until(&["list"], |line| plain_line(line) == paused);
    let shown = engine.says(&["show", "1"]);
    assert!(
        shown.contains(&format!(
            "\nNow: paused at plan by the engine. The round failed. The cause: {WHY}.\nPaused: by the engine.\n"
        )),
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

    assert_eq!(engine.says(&["resume", "1"]), "Work W1 goes on.\n");
    engine.until(&["list", "--all"], |line| {
        plain_line(line) == "#1 (project script) \"Greet the user\" at level 1, W1 on trial: merged"
    });
}
