//! `wf-agents fork`, through the running engine: what cannot be forked is said so, as the engine says it.

use crate::{Engine, Place, SCRIPT};

#[test]
fn a_fork_of_a_round_that_did_not_run_or_of_no_work_is_refused_with_why() {
    let place = Place::new(SCRIPT);
    let engine = Engine::start(&place);
    let question = place.data().join("question.md");
    std::fs::write(&question, "Which words can be read two ways?").expect("written");
    let question = question.to_string_lossy().to_string();
    let added = engine.says(&[
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
    assert!(added.starts_with("Added item 1 with its work 1"), "{added}");
    // A scripted round runs on no agents' program, so no round of the work ran on one.
    let said = engine.refuses(&["fork", "1", "1", "--question", &question]);
    assert_eq!(said.trim_end(), "Work 1 has no round of run 1 that ran.");
    let said = engine.refuses(&["fork", "1", "1", "--round", "2", "--question", &question]);
    assert_eq!(said.trim_end(), "Work 1 has no round 2 of run 1 that ran.");
    let said = engine.refuses(&["fork", "9", "1", "--question", &question]);
    assert_eq!(said.trim_end(), "There is no work 9.");
    let empty = place.data().join("empty.md");
    std::fs::write(&empty, " \n").expect("written");
    let said = engine.refuses(&["fork", "1", "1", "--question", &empty.to_string_lossy()]);
    assert_eq!(said.trim_end(), "The question says nothing.");
}
