//! `wf-agents wait`: what needs whoever plans the work, waited for and said a line each, with the mark to give next.

use crate::{Engine, Place, SCRIPT};

/// The mark the answer says to give next, and the answer without that last line.
fn split(said: &str) -> (&str, &str) {
    let (lines, next) = said.trim_end().rsplit_once('\n').unwrap_or(("", said.trim_end()));
    let mark = next
        .strip_prefix("Next: wf-agents wait --since ")
        .unwrap_or_else(|| panic!("the answer does not end with the mark: {said:?}"));
    (lines, mark)
}

#[test]
fn wait_gives_the_owners_call_once_the_plan_reaches_its_limit_then_nothing_from_its_mark_then_the_merge() {
    let place = Place::new(SCRIPT);
    let engine = Engine::start(&place);
    assert_eq!(
        engine.says(&["wait", "--for", "0"]),
        "Nothing waits for the owner.\nNext: wf-agents wait --since 0\n"
    );
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
    // With no time to wait, it waits until the plan's run has reached its limit of returns.
    let said = engine.says(&["wait"]);
    let (lines, mark) = split(&said);
    assert!(
        lines.starts_with("Item 1 \"Greet the user\", work 1: waits for the owner at plan for ")
            && lines.ends_with(": The cell's run had 2 returns sent back, its limit."),
        "{said}"
    );
    assert_eq!(lines.lines().count(), 1, "{said}");
    assert_eq!(
        engine.says(&["wait", "--since", mark, "--for", "0"]),
        format!("Nothing for the owner since the mark.\nNext: wf-agents wait --since {mark}\n")
    );
    engine.says(&["let-go", "1", "--call", "2"]);
    let said = engine.says(&["wait", "--since", mark]);
    let (lines, _) = split(&said);
    assert!(
        lines.starts_with("Item 1 \"Greet the user\", work 1: merged, ") && lines.ends_with(" ago."),
        "{said}"
    );
}

// A pause that waits for an item says why and for what; its work is none of the items to let on while the item has not
// ended; when the item has merged, `wait` says so, the work stays paused until the owner lets it on, and letting it on
// takes the line away.
#[test]
fn a_pause_that_waits_for_an_item_is_said_with_its_reason_and_wait_says_that_the_item_has_merged() {
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
    engine.says(&[
        "add",
        "--title",
        "Say goodbye",
        "--flow",
        "trial",
        "--project",
        "script",
        "--write",
        "request=Bye.",
        "--why",
        "After the greeting.",
        "--waits-for",
        "1",
    ]);
    let shown = engine.says(&["show", "2"]);
    assert!(
        shown.contains("paused at plan until #1 merges or is cancelled: After the greeting."),
        "{shown}"
    );
    let candidates = engine.says(&["candidates"]);
    assert!(
        candidates.contains("1 item could be let on now")
            && candidates.contains("wf-agents resume 1")
            && !candidates.contains("wf-agents resume 2")
            && candidates.contains("1 paused item is not among them: its pause waits for items"),
        "{candidates}"
    );
    assert!(
        engine
            .refuses(&["pause", "1", "--waits-for", "1"])
            .contains("A pause cannot wait for item #1 of its own work"),
    );
    assert!(
        engine
            .refuses(&["pause", "2", "--waits-for", "9"])
            .contains("There is no item #9")
    );

    engine.says(&["resume", "1"]);
    engine.says(&["wait"]);
    engine.says(&["let-go", "1", "--call", "2"]);
    let line = engine.until(&["wait", "--since", "0", "--for", "0"], |line| {
        line.starts_with("Item 2 ")
    });
    assert!(
        line.starts_with(
            "Item 2 \"Say goodbye\", work 2: its pause waited for #1, which has merged or been cancelled, "
        ) && line.ends_with(": it is paused still, and only `wf-agents resume 2` lets it on."),
        "{line}"
    );
    let shown = engine.says(&["show", "2"]);
    assert!(
        shown.contains(
            "paused at plan, and what it waited for is over: #1 merged or was cancelled: After the greeting."
        ),
        "{shown}"
    );
    let candidates = engine.says(&["candidates"]);
    assert!(
        candidates.contains("wf-agents resume 2") && !candidates.contains("paused item"),
        "{candidates}"
    );

    engine.says(&["resume", "2"]);
    let said = engine.says(&["wait", "--since", "0", "--for", "0"]);
    assert!(!said.contains("its pause waited for"), "{said}");
}

#[test]
fn wait_refuses_a_mark_that_is_none_and_one_of_other_records() {
    let place = Place::new(SCRIPT);
    let engine = Engine::start(&place);
    assert_eq!(
        engine.refuses(&["wait", "--since", "soon"]),
        "\"soon\" is not a mark: a mark is a number, then each work with its place, such as 12,3.41,7.118.\n"
    );
    let refused = engine.refuses(&["wait", "--since", "4", "--for", "0"]);
    assert!(refused.starts_with("The mark is not of these records"), "{refused}");
}
