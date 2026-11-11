//! Adversarial tests of the reading of a return. Reference: the acceptance of the work (a return in the form, or one of
//! the few shapes it names, is read; anything else, an unknown field, a required field made up, is an `Err`), and the
//! JSON grammar (RFC 8259) for what a string holding a form is.

use super::*;
use serde_json::json;

fn form() -> Value {
    json!({"hands_in": [{"block": "plan", "content": "Greet.", "matters": true}], "asks": []})
}

fn handed(content: &str) -> Result<Came, String> {
    Ok(Came::HandsIn(vec![Handed {
        block: "plan".into(),
        content: content.to_string(),
        matters: true,
        value: None,
    }]))
}

#[test]
fn a_null_unknown_field_in_an_ask_still_fails() {
    for r in [
        json!({"hands_in": [], "asks": [{"question": "Which?", "bogus": null}]}),
        json!({"hands_in": [], "asks": [{"tool": "check", "input": "", "bogus": null}]}),
    ] {
        assert!(came(&r).is_err(), "{r} is taken");
    }
}

#[test]
fn a_null_unknown_field_in_a_hand_in_or_the_form_still_fails() {
    for r in [
        json!({"hands_in": [{"block": "plan", "content": "x", "matters": true, "bogus": null}], "asks": []}),
        json!({"hands_in": [], "asks": [], "bogus": null}),
    ] {
        assert!(came(&r).is_err(), "{r} is taken");
    }
}

#[test]
fn an_ask_with_both_forms_filled_fails() {
    let r = json!({"hands_in": [], "asks": [{"tool": "check", "input": "", "question": "Which?"}]});
    assert!(came(&r).is_err());
}

#[test]
fn only_one_level_of_wrapping_is_taken() {
    assert!(came(&json!({"a": {"b": form()}})).is_err());
    assert!(came(&json!({"a": [form()]})).is_err());
    assert!(came(&json!({"a": 1})).is_err());
    assert!(came(&json!({"a": null})).is_err());
    assert!(came(&json!({"a": "nothing"})).is_err());
}

#[test]
fn a_list_string_that_is_not_a_list_fails() {
    assert!(came(&json!({"hands_in": "{}", "asks": []})).is_err());
    assert!(came(&json!({"hands_in": "nothing", "asks": []})).is_err());
    assert!(came(&json!({"hands_in": 1, "asks": []})).is_err());
    assert!(came(&json!({"hands_in": {}, "asks": []})).is_err());
}

#[test]
fn a_return_that_is_not_an_object_or_string_fails() {
    for r in [json!(null), json!(1), json!(true), json!([form()]), json!([])] {
        assert!(came(&r).is_err(), "{r} is taken");
    }
}

#[test]
fn a_string_of_a_non_object_or_with_other_trailing_text_fails() {
    let text = form().to_string();
    assert!(came(&json!(format!("[{text}]"))).is_err());
    assert!(came(&json!(format!("{text}{text}"))).is_err());
    assert!(came(&json!(format!("{text})"))).is_err());
    assert!(came(&json!(format!("```json\n{text}\n```"))).is_err());
    assert!(came(&json!("")).is_err());
    assert!(came(&json!("{")).is_err());
}

#[test]
fn a_content_that_looks_like_json_is_kept_whole() {
    let content = r#"{"hands_in": []}}"#;
    let r = json!({"hands_in": [{"block": "plan", "content": content, "matters": true}], "asks": []});
    assert_eq!(came(&r), handed(content));
    let w = json!({"o": {"hands_in": [{"block": "plan", "content": content, "matters": true}]}});
    assert_eq!(came(&w), handed(content));
    assert_eq!(came(&json!(r.to_string())), handed(content));
}

#[test]
fn a_wrapped_string_form_with_trailing_brackets_is_read() {
    let text = format!("{}]}}", form());
    assert_eq!(came(&json!({"outcome": text})), handed("Greet."));
}

#[test]
fn asks_win_over_hands_in_when_normalised() {
    let r = json!({"o": {"hands_in": form()["hands_in"], "asks": [{"question": "Which?"}]}});
    assert!(matches!(came(&r), Ok(Came::Asks(a)) if a.len() == 1));
    let r = json!({"hands_in": form()["hands_in"], "asks": null});
    assert_eq!(came(&r), handed("Greet."));
}

#[test]
fn an_unwrap_never_fires_on_a_return_with_a_list() {
    let r = json!({"asks": [{"question": "Which?"}]});
    assert!(matches!(came(&r), Ok(Came::Asks(_))));
    let r = json!({"hands_in": {"hands_in": [], "asks": []}});
    assert!(came(&r).is_err());
}

#[test]
fn a_value_of_a_hand_in_is_kept() {
    let r = json!({"hands_in": [{"block": "ok", "content": "x", "matters": false, "value": "yes"}], "asks": []});
    assert_eq!(
        came(&r),
        Ok(Came::HandsIn(vec![Handed {
            block: "ok".into(),
            content: "x".to_string(),
            matters: false,
            value: Some("yes".to_string()),
        }]))
    );
}
