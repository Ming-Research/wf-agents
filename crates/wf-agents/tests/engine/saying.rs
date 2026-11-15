//! `wf-agents say`: the owner's words to a line of a work, given to the cell the engine chooses, which the command says,
//! or refused with why.

use crate::{Engine, Place, SCRIPT};

// The owner's words to a line go to the cell the engine gives them to, the one holding the work on that line, which
// the act answers with and the command says; words to a line none of whose cells the owner's words run are refused
// with why. Well under a second alone, the engine run as its program on a script of short rounds; no other test says
// words through the command or the act's route.
#[test]
fn words_to_a_line_are_said_to_the_cell_the_engine_gave_them_to_and_refused_where_none_takes_them() {
    let place = Place::new(SCRIPT);
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
    engine.says(&["resume", "1"]);
    // The plan asks the owner its question and waits for the answer.
    let waits = engine.says(&["wait"]);
    assert!(waits.contains("waits for the owner at plan for "), "{waits}");
    assert_eq!(
        engine.says(&["say", "1", "Keep it short.", "--line", "engineer"]),
        "Said to plan of work 1, where the line engineer stands.\n"
    );
    let refused = engine.refuses(&["say", "1", "Keep it short.", "--line", "reviewer"]);
    assert_eq!(
        refused.trim_end(),
        "The owner's words run no cell of the line reviewer."
    );
}
