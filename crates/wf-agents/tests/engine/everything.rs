//! The owner's pause of everything from the command line: set and lifted, said with the slots by `list`, holding back a
//! work added going on, which then goes on by itself.

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
            "--going",
            "--title",
            "Greet the user",
            "--flow",
            "trial",
            "--write",
            "request=Greet the user, please."
        ]),
        "Added item 1 with its work 1, which goes on by itself when it may.\n"
    );
    assert_eq!(
        engine.says(&["list"]),
        format!(
            "{paused}; 0 of 2 slots taken.\nItem 1 \"Greet the user\", work 1 on trial: held back by the pause of \
             everything\n"
        )
    );
    // A work let on under the pause of everything is said to be held back still.
    assert_eq!(
        engine.says(&["resume", "1"]),
        "Work 1 is let on, and held back by the pause of everything until `wf-agents resume --all`.\n"
    );
    assert!(engine.says(&["resume", "--all"]).starts_with("Works go on; "));
    engine.until(&["list"], |line| {
        line.starts_with("Item 1 \"Greet the user\", work 1 on trial: waits at plan")
    });
}

#[test]
fn pause_and_resume_take_a_work_or_all_and_not_both() {
    let place = Place::new(SCRIPT);
    let engine = Engine::start(&place);
    assert!(!engine.refuses(&["pause"]).is_empty());
    assert!(!engine.refuses(&["resume", "1", "--all"]).is_empty());
}
