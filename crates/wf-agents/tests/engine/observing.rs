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

#[test]
fn an_architect_adds_observations_changes_them_and_finds_them_by_tag() {
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
    assert_eq!(
        engine.says(&[
            "add-observation",
            "--project",
            "script",
            "--reported-by",
            "script",
            "--title",
            "Name the tests",
            "--tag",
            "flow",
            "--text",
            "The merge cell names no tests."
        ]),
        "Added observation O1.\n"
    );
    assert_eq!(
        engine.says(&[
            "add-observation",
            "--project",
            "script",
            "--reported-by",
            "script",
            "--text",
            "Another."
        ]),
        "Added observation O2.\n"
    );
    let said = engine.says(&["observations"]);
    let lines: Vec<&str> = said.lines().collect();
    assert_eq!(lines.len(), 6, "{said}");
    assert!(
        lines[0].starts_with("O2, unread, kept ") && lines[0].ends_with(": reported by script"),
        "{said}"
    );
    assert_eq!(lines[1], "  of script");
    assert_eq!(lines[2], "  Another.");
    assert_eq!(lines[4], "  of script; title \"Name the tests\"; tags flow");
    // Found by its tag.
    let found = engine.says(&["observations", "--tag", "flow"]);
    assert!(found.starts_with("O1, ") && !found.contains("O2"), "{found}");
    // An item takes one in, and a tag is added to the other: the one taken in is left out unless all are asked for.
    assert_eq!(
        engine.says(&[
            "change-observations",
            "--why",
            "Item 1 holds it.",
            "O1:taken-in-by=1",
            "2:tag=tests"
        ]),
        "O1 is taken in by #1.\nO2 is tagged tests.\n"
    );
    let left = engine.says(&["observations"]);
    assert!(left.starts_with("O2, ") && !left.contains("O1"), "{left}");
    let all = engine.says(&["observations", "--all"]);
    assert!(
        all.contains("title \"Name the tests\"; tags flow; taken in by #1\n"),
        "{all}"
    );
    // A refusal says each change by its place, and nothing of the batch is made.
    assert_eq!(
        engine.refuses(&[
            "change-observations",
            "--why",
            "Out of date.",
            "2:delete",
            "2:title=Too late",
            "9:delete"
        ]),
        "Change 2 (O2): it is deleted, and takes no change; change 3 (O9): there is no such observation; so none of \
         the changes was made.\n"
    );
    assert_eq!(
        engine.says(&["change-observations", "--why", "Out of date.", "2:delete"]),
        "O2 is deleted: its text and why stay in the records.\n"
    );
    assert!(engine.says(&["observations"]).starts_with("No observation is shown"));
    // The names in use are listed beside the items': a deleted observation's tags count for nothing.
    assert_eq!(engine.says(&["tags"]), "flow: 0 not handled of 1 observations\n");
}
