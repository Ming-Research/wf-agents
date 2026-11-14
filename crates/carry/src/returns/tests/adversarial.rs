//! Adversarial tests of the reading of a return. Reference: the acceptance of the work (a return in the form, or one of
//! the few shapes it names, is read; anything else, an unknown field, a required field made up, is an `Err`), and the
//! JSON grammar (RFC 8259) for what a string holding a form is.

use super::*;

fn form() -> Value {
    json!({"outcome": {"dealt_with": ["on_request"], "outputs": {"plan": "Greet."}}})
}

fn planned(content: &str) -> Result<Came, String> {
    Ok(Came {
        dealt_with: Some(vec!["request".into()]),
        writes: vec![handed("plan", content, true, None)],
        ..Came::default()
    })
}

#[test]
fn a_null_unknown_field_in_a_question_still_fails() {
    for r in [
        json!({"outcome": {"questions": [{"name": "q", "withdrawn": "x", "bogus": null}]}}),
        json!({"outcome": {"questions": [{"name": "q", "waits": false, "recommends": 1, "ask": "Which?",
            "options": [], "bogus": null}]}}),
    ] {
        assert!(read(&r).is_err(), "{r} is taken");
    }
}

#[test]
fn a_null_unknown_field_in_a_kind_or_the_form_still_fails() {
    for r in [
        json!({"outcome": {"dealt_with": [], "bogus": null}}),
        json!({"outcome": {}, "bogus": null}),
    ] {
        assert!(read(&r).is_err(), "{r} is taken");
    }
}

#[test]
fn a_question_with_both_shapes_filled_fails() {
    let r = json!({"outcome": {"questions": [{"name": "q", "waits": false, "recommends": 1, "ask": "Which?",
        "options": [], "withdrawn": "No longer."}]}});
    assert!(read(&r).is_err());
}

#[test]
fn only_one_level_of_wrapping_is_taken() {
    assert!(read(&json!({"a": {"b": form()}})).is_err());
    assert!(read(&json!({"a": [form()]})).is_err());
    assert!(read(&json!({"a": 1})).is_err());
    assert!(read(&json!({"a": null})).is_err());
    assert!(read(&json!({"a": "nothing"})).is_err());
}

#[test]
fn a_kind_is_never_unwrapped() {
    assert!(read(&json!({"outcome": form()})).is_err());
    assert!(read(&json!({"calls": form()})).is_err());
}

#[test]
fn a_list_string_that_is_not_a_list_fails() {
    for list in [json!("{}"), json!("nothing"), json!(1), json!({})] {
        let r = json!({"outcome": {"dealt_with": list}});
        assert!(read(&r).is_err(), "{r} is taken");
        let r = json!({"calls": {"query": list}});
        assert!(read(&r).is_err(), "{r} is taken");
    }
}

#[test]
fn a_return_that_is_not_an_object_or_string_fails() {
    for r in [json!(null), json!(1), json!(true), json!([form()]), json!([])] {
        assert!(read(&r).is_err(), "{r} is taken");
    }
}

#[test]
fn a_string_of_a_non_object_or_with_other_trailing_text_fails() {
    let text = form().to_string();
    assert!(read(&json!(format!("[{text}]"))).is_err());
    assert!(read(&json!(format!("{text}{text}"))).is_err());
    assert!(read(&json!(format!("{text})"))).is_err());
    assert!(read(&json!(format!("```json\n{text}\n```"))).is_err());
    assert!(read(&json!("")).is_err());
    assert!(read(&json!("{")).is_err());
}

#[test]
fn a_content_that_looks_like_json_is_kept_whole() {
    let content = r#"{"outcome": {}}}"#;
    let r = json!({"outcome": {"dealt_with": ["on_request"], "outputs": {"plan": content}}});
    assert_eq!(read(&r), planned(content));
    assert_eq!(read(&json!({"o": r.clone()})), planned(content));
    assert_eq!(read(&json!(r.to_string())), planned(content));
}

#[test]
fn a_wrapped_string_form_with_trailing_brackets_is_read() {
    let text = format!("{}]}}", form());
    assert_eq!(read(&json!({"return": text})), planned("Greet."));
}

#[test]
fn a_return_of_both_kinds_is_read_whole_for_the_run_s_check_to_send_back() {
    let mut r = form();
    r["calls"] = json!({"query": [{"sql": "select 1"}]});
    let came = read(&r).expect("it is read");
    assert_eq!(came.dealt_with, Some(vec!["request".into()]));
    assert_eq!(came.asks.len(), 1);
    assert_eq!(came.writes.len(), 1);
}

#[test]
fn a_choice_of_more_or_other_members_than_its_value_and_why_is_not_one() {
    for choice in [
        json!({"value": "yes"}),
        json!({"value": "yes", "why": "x", "more": "y"}),
        json!({"value": 1, "why": "x"}),
        json!({"value": "yes", "because": "x"}),
        json!(["yes", "x"]),
        json!(null),
    ] {
        let r = json!({"outcome": {"dealt_with": ["on_request"], "outputs": {"ok": choice}}});
        let came = read(&r).expect("it is read");
        assert_eq!(came.found, [Unmet::NoValue { block: "ok".into() }], "{choice}");
        assert!(came.writes.is_empty(), "{choice}");
    }
}
