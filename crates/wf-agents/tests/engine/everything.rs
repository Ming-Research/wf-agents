//! The owner's pause of everything from the command line: set and lifted, said with the slots by `list`, holding back a
//! work the owner let on, which then goes on by itself once it is lifted. And the stop of the engine, which is asked for
//! on the command line alone.

use crate::{Engine, Place, SCRIPT};

#[test]
fn everything_is_paused_and_let_on_again_from_the_command_line() {
    let place = Place::new(SCRIPT);
    let engine = Engine::start(&place);
    let paused = "Every work is paused: none begins a round but to answer a note, until `wf-agents resume --all`";
    assert_eq!(
        engine.says(&["pause", "--all"]),
        format!("{paused}; 0 of 2 slots taken.\n")
    );
    assert_eq!(
        engine.says(&[
            "add",
            "--title",
            "Greet the user",
            "--flow",
            "trial",
            "--project",
            "script",
            "--write",
            "request=Greet the user, please."
        ]),
        "Added item 1 with its work 1, paused: let it on with `wf-agents resume 1`.\n"
    );
    // A work let on under the pause of everything is said to be held back still.
    assert_eq!(
        engine.says(&["resume", "1"]),
        "Work 1 is let on, and held back by the pause of everything until `wf-agents resume --all`.\n"
    );
    assert_eq!(
        engine.says(&["list"]),
        format!(
            "{paused}; 0 of 2 slots taken.\n#1 \"Greet the user\" at level 1, project script, W1 on trial: held back at plan by the \
             pause of everything\n"
        )
    );
    assert!(engine.says(&["resume", "--all"]).starts_with("Works go on; "));
    engine.until(&["list"], |line| {
        line.starts_with("#1 \"Greet the user\" at level 1, project script, W1 on trial: waits at plan")
    });
}

#[test]
fn pause_and_resume_take_a_work_or_all_and_not_both() {
    let place = Place::new(SCRIPT);
    let engine = Engine::start(&place);
    assert!(!engine.refuses(&["pause"]).is_empty());
    assert!(!engine.refuses(&["resume", "1", "--all"]).is_empty());
}

// A stop at once is not done without --yes, which it says, the engine still answering after it; a stop with nothing
// under way ends the engine at once, with status 0; and the engine started again stands as before, the stop over.
// Fails where a stop at once is sent without its yes, or where a stop leaves the engine running or ends it as a fault.
// About a second alone, most of it two starts of the engine; the one test of the stop through the program itself.
#[test]
fn a_stop_at_once_waits_for_its_yes_and_a_stop_with_nothing_under_way_ends_the_engine() {
    let place = Place::new(SCRIPT);
    let mut engine = Engine::start(&place);
    assert!(
        !engine.refuses(&["stop", "--yes"]).is_empty(),
        "--yes is for a stop at once"
    );
    assert_eq!(
        engine.says(&["stop", "--now"]),
        "Nothing is stopped: no round is under way, and `wf-agents stop --now --yes` stops the engine at once.\n"
    );
    assert!(
        engine.says(&["list"]).starts_with("Works go on; "),
        "the engine still answers"
    );
    assert_eq!(engine.says(&["stop"]), "The engine stopped; no round was under way.\n");
    assert!(engine.ended().success(), "a stop ends the engine with status 0");
    let again = Engine::start(&place);
    assert!(again.says(&["list"]).starts_with("Works go on; "));
}
