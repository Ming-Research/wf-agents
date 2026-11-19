//! `wf-agents observations`: what agents reported beyond their works, read from a running engine, the newest first.

use crate::{Engine, Place};

/// The trial flow's script with the plan's first round handing in its plan and saying two observations.
const SCRIPT: &str = r#"
[took]
ms = 20

[[flows.trial.rounds]]
cell = "plan"
hands_in = [{ block = "plan" }]
observations = ["The greeting has no test.", "The request names no language."]
"#;

#[test]
fn observations_are_said_newest_first_each_with_where_it_came_from_and_paged_by_number() {
    let place = Place::new(SCRIPT);
    let engine = Engine::start(&place);
    assert_eq!(engine.says(&["observations"]), "There are no observations.\n");
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
    engine.until(&["observations"], |line| line.starts_with("O2, "));
    let said = engine.says(&["observations"]);
    let lines: Vec<&str> = said.lines().collect();
    assert_eq!(lines.len(), 4, "{said}");
    assert!(
        lines[0].starts_with("O2, unread, kept ")
            && lines[0].ends_with(": item #1 \"Greet the user\", W1, cell plan, run 1, round 1"),
        "{said}"
    );
    assert_eq!(lines[1], "  The request names no language.");
    assert!(lines[2].starts_with("O1, unread, kept "), "{said}");
    assert_eq!(lines[3], "  The greeting has no test.");
    // A bound says how many it left out and the number to give next time.
    let first = engine.says(&["observations", "--limit", "1"]);
    assert!(
        first.ends_with("And 1 more, older: wf-agents observations --from 1\n"),
        "{first}"
    );
    let second = engine.says(&["observations", "--from", "1"]);
    assert!(
        second.starts_with("O1, unread, kept ") && !second.contains("O2"),
        "{second}"
    );
    assert_eq!(
        engine.says(&["observations", "--from", "0"]),
        "No observation is numbered O0 or lower.\n"
    );
}
