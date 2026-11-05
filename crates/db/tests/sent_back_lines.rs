//! The corners of the lines a return sent back is said with (`db::sent_back_lines`) and of who may let go on a work
//! paused for it (`db::architect_may_resume`), read through the public interface.

use serde_json::json;

// The reference is `db::problem_line`, which its own comment calls the only place the line is made: every line is that
// line for the entry's check, field and text, in the order the entries are kept, one for each entry, even for two
// entries that say the same, and a field that is null, missing or not a text is a problem about the return as a whole.
#[test]
fn every_kept_problem_is_one_line_in_its_order_whatever_its_field() {
    let rejected = json!({
        "check": "evidence_holds (the return): no evidence",
        "problems": [
            { "check": "tests_named", "field": "outcome.wrote", "text": "no such test" },
            { "check": "tests_named", "field": "outcome.wrote", "text": "no such test" },
            { "check": "evidence_holds", "text": "no evidence" },
            { "check": "evidence_holds", "field": 4, "text": "a number is no field" },
            { "check": "format", "field": "", "text": "an empty field" },
        ],
    });
    let want = [
        db::problem_line("tests_named", Some("outcome.wrote"), "no such test"),
        db::problem_line("tests_named", Some("outcome.wrote"), "no such test"),
        db::problem_line("evidence_holds", None, "no evidence"),
        db::problem_line("evidence_holds", None, "a number is no field"),
        db::problem_line("format", Some(""), "an empty field"),
    ];
    assert_eq!(db::sent_back_lines(&rejected), want);
}

// A record from before the problems were kept is said by the text it was sent back with, line by line; so is one whose
// `problems` is empty or holds nothing a line can be made of, so that a return sent back is never said by nothing while
// the column holds its reason. The check's text comes before the schema's, as `sent_back_text` reads it.
#[test]
fn a_record_without_usable_problems_is_said_by_the_text_it_was_sent_back_with() {
    let text = "first\nsecond\nthird";
    assert_eq!(
        db::sent_back_lines(&json!({ "check": text })),
        ["first", "second", "third"]
    );
    assert_eq!(
        db::sent_back_lines(&json!({ "check": text, "problems": [] })),
        ["first", "second", "third"]
    );
    let useless = json!({ "format": "the schema says no", "problems": [{ "check": 5 }, { "text": "x" }, "y"] });
    assert_eq!(db::sent_back_lines(&useless), ["the schema says no"]);
    let both = json!({ "check": "from the check", "format": "from the schema" });
    assert_eq!(db::sent_back_lines(&both), ["from the check"]);
    // A column that is not an object, or holds a text under neither name, says nothing.
    for nothing in [
        json!(null),
        json!("a text"),
        json!([]),
        json!({ "failed": { "kind": "offline" } }),
        json!({ "check": 3 }),
        json!({ "problems": "not a list" }),
    ] {
        assert!(db::sent_back_lines(&nothing).is_empty(), "{nothing}");
    }
}

// The architect may let go on only what it stopped itself or what the engine paused for a return it was asked to look
// at, and only when the engine paused it. The reference is the two constants the reasons are made from: a reason that
// only resembles one, in another case, with something before it, or with the owner as the one who paused, is not one.
#[test]
fn the_architect_may_let_go_on_only_a_pause_the_engine_made_for_it() {
    let look = format!(
        "{} at cell R1, and its project's architect is asked to look at why",
        db::SENT_BACK_FOR_ARCHITECT
    );
    assert!(db::architect_may_resume(Some(&look), Some("engine")));
    for (reason, by) in [
        (look.clone(), Some("owner")),
        (look.clone(), Some("architect")),
        (look.clone(), None),
        (format!(" {look}"), Some("engine")),
        (look.to_uppercase(), Some("engine")),
        (look.replacen("a return", "the return", 1), Some("engine")),
        // The pause the engine makes when no architect looks, and the one it makes after several in a row.
        (
            "the return was sent back at cell R1, for:\nformat (outcome): x".to_string(),
            Some("engine"),
        ),
        (
            "3 returns in a row were sent back, the last at cell H8, for:\nx".to_string(),
            Some("engine"),
        ),
        (String::new(), Some("engine")),
    ] {
        assert!(!db::architect_may_resume(Some(&reason), by), "{reason:?} by {by:?}");
    }
    assert!(!db::architect_may_resume(None, Some("engine")));
}
