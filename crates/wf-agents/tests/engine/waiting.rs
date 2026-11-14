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
fn wait_gives_the_question_once_it_is_asked_then_nothing_from_its_mark_then_the_merge() {
    let place = Place::new(SCRIPT);
    let engine = Engine::start(&place);
    assert_eq!(
        engine.says(&["wait", "--for", "0"]),
        "Nothing waits for you.\nNext: wf-agents wait --since 0\n"
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
    // With no time to wait, it waits until the plan has asked.
    let said = engine.says(&["wait"]);
    let (lines, mark) = split(&said);
    assert!(
        lines.starts_with(
            "Item 1 \"Greet the user\", work 1: waits at plan for the owner to answer call 2: \"Should the greeting \
             name the user?\", for "
        ),
        "{said}"
    );
    assert_eq!(lines.lines().count(), 1, "{said}");
    assert_eq!(
        engine.says(&["wait", "--since", mark, "--for", "0"]),
        format!("Nothing for you since the mark.\nNext: wf-agents wait --since {mark}\n")
    );
    engine.says(&["answer", "1", "2", "Yes, by name."]);
    let said = engine.says(&["wait", "--since", mark]);
    let (lines, _) = split(&said);
    assert!(
        lines.starts_with("Item 1 \"Greet the user\", work 1: merged, ") && lines.ends_with(" ago."),
        "{said}"
    );
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
