//! The commands that change an item after it was added, against a running engine: its title, what it depends on, and a
//! cancel that says why, each kept and shown with what the item was before.

use crate::{Engine, Place, SCRIPT};

#[test]
fn an_item_not_begun_is_retitled_made_to_wait_and_cancelled_saying_why_from_the_command_line() {
    let place = Place::new(SCRIPT);
    let engine = Engine::start(&place);
    let add = |title: &'static str| {
        [
            "add",
            "--title",
            title,
            "--flow",
            "trial",
            "--project",
            "script",
            "--write",
            "request=Please.",
        ]
    };
    engine.says(&add("Greet the user"));
    engine.says(&add("Say goodbye"));
    assert_eq!(
        engine.says(&["set-title", "2", "Say goodbye by name"]),
        "Item 2 is titled \"Say goodbye by name\"; the title it had is kept in its history.\n"
    );
    assert_eq!(
        engine.says(&["add-dependency", "2", "1"]),
        "Item 2 depends on item 1: its work waits for it to merge.\n"
    );
    assert_eq!(
        engine.refuses(&["add-dependency", "1", "2"]),
        "Item 1 cannot depend on item 2, which depends on it already: 2 depends on 1.\n"
    );
    let list = engine.says(&["list"]);
    assert!(
        list.contains("\nItem 2 \"Say goodbye by name\" at level 1, work 2 on trial, after 1: "),
        "{list}"
    );
    let shown = engine.says(&["show", "2"]);
    assert!(
        shown.contains("\nItem 2 was added as \"Say goodbye\" at level 1; changed since:\n"),
        "{shown}"
    );
    assert!(shown.contains(": titled \"Say goodbye by name\"\n"), "{shown}");
    assert!(shown.contains(": came to depend on item 1\n"), "{shown}");
    assert_eq!(
        engine.says(&["remove-dependency", "2", "1"]),
        "Item 2 no longer depends on item 1.\n"
    );
    assert_eq!(
        engine.refuses(&["remove-dependency", "2", "1"]),
        "Item 2 does not depend on item 1.\n"
    );
    assert_eq!(
        engine.says(&["cancel", "2", "--why", "Joined into item 1."]),
        "Work 2 is cancelled.\n"
    );
    let shown = engine.until(&["show", "2"], |line| line.starts_with("Cancelled at "));
    assert!(shown.ends_with(": \"Joined into item 1.\"."), "{shown}");
    assert_eq!(
        engine.refuses(&["set-title", "2", "Other"]),
        "Item 2 is cancelled: an item that has ended takes no change, and is what it ended as.\n"
    );
}
