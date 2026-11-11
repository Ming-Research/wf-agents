//! What an agent returns at the end of a round: one form for every cell, given to the command line as its JSON
//! schema, so that it is part of the prefix every round shares and the prompt cache keeps. A return hands in blocks
//! or asks for calls: a tool to run, or a question for the owner. Whether a block is one the cell may write, and a
//! value one its choice offers, the engine checks, and sends back what does not meet it.

use decide::{BlockId, Who};
use engine::Came;
use serde::Deserialize;
use serde_json::Value;
use work::{Ask, Handed};

/// The form of every return, as a JSON schema. It has no `oneOf` or `anyOf` at its top, which the model's interface
/// refuses.
pub const SCHEMA: &str = r#"{"type":"object","properties":{"hands_in":{"type":"array","items":{"type":"object","properties":{"block":{"type":"string"},"content":{"type":"string"},"value":{"type":"string"},"matters":{"type":"boolean"}},"required":["block","content","matters"],"additionalProperties":false}},"asks":{"type":"array","items":{"anyOf":[{"type":"object","properties":{"tool":{"type":"string"},"input":{"type":"string"}},"required":["tool","input"],"additionalProperties":false},{"type":"object","properties":{"question":{"type":"string"}},"required":["question"],"additionalProperties":false}]}}},"required":["hands_in","asks"],"additionalProperties":false}"#;

/// A return as the agent writes it.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Return {
    hands_in: Vec<HandIn>,
    asks: Vec<AskIn>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct HandIn {
    block: String,
    content: String,
    value: Option<String>,
    /// Whether the change matters to the cells that read the block. It has no default, so that each hand-in says it on
    /// purpose: an agent that left it out was taken to mean that it did not.
    matters: bool,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum AskIn {
    Tool(ToolIn),
    Question(QuestionIn),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ToolIn {
    tool: String,
    input: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct QuestionIn {
    question: String,
}

/// The object a text holds when nothing but whitespace and closing brackets follows it: the model at times writes a
/// form with a closing bracket too many. None for any other text, which may be content.
fn object_of(text: &str) -> Option<Value> {
    let text = text.trim();
    let mut values = serde_json::Deserializer::from_str(text).into_iter::<Value>();
    let object = values.next()?.ok().filter(Value::is_object)?;
    let rest = &text[values.byte_offset()..];
    rest.chars()
        .all(|c| c.is_whitespace() || c == '}' || c == ']')
        .then_some(object)
}

/// The array a list of the form comes as: a string holding one is parsed, a list left out or null is empty.
fn list_of(value: Option<Value>) -> Value {
    match value {
        None | Some(Value::Null) => Value::Array(Vec::new()),
        Some(Value::String(text)) => match serde_json::from_str::<Value>(text.trim()) {
            Ok(list @ Value::Array(_)) => list,
            _ => Value::String(text),
        },
        Some(other) => other,
    }
}

/// A return put into the shape of the form when it is not in it but means it, as the old engine read one
/// (`as_taken`), kept to the one fixed form: the whole form, or the one object that wraps it, written as a string; the
/// form wrapped one level in an object of one key; a list left out, null or written as a string; and, in an ask, the
/// null fields of the other form. A content is never changed and a field the form requires never made up, so what
/// this does not reach fails as it is read. The record of the round keeps the return as the command line printed it
/// (`kept`), before this.
fn normalised(structured: &Value) -> Value {
    let mut form = match structured {
        Value::String(text) => object_of(text).unwrap_or_else(|| structured.clone()),
        other => other.clone(),
    };
    let inner = match &form {
        Value::Object(o) if !o.contains_key("hands_in") && !o.contains_key("asks") && o.len() == 1 => {
            match o.values().next() {
                Some(Value::String(text)) => object_of(text),
                Some(inner @ Value::Object(_)) => Some(inner.clone()),
                _ => None,
            }
        }
        _ => None,
    };
    if let Some(inner) = inner {
        form = inner;
    }
    let Value::Object(o) = &mut form else { return form };
    for name in ["hands_in", "asks"] {
        let list = list_of(o.remove(name));
        o.insert(name.to_string(), list);
    }
    if let Some(Value::Array(asks)) = o.get_mut("asks") {
        for ask in asks {
            if let Value::Object(ask) = ask {
                ask.retain(|_, v| !v.is_null());
            }
        }
    }
    form
}

/// What came back, from the structured return; or why it is not one. A return that asks for anything asks, and what
/// it hands in beside is not read; one that asks for nothing hands in, if only nothing.
pub fn came(structured: &serde_json::Value) -> Result<Came, String> {
    let r: Return =
        serde_json::from_value(normalised(structured)).map_err(|e| format!("the return is not of its form: {e}"))?;
    if !r.asks.is_empty() {
        let asks = (r.asks.into_iter())
            .map(|ask| match ask {
                AskIn::Tool(tool) => Ask::Tool {
                    name: tool.tool,
                    input: tool.input,
                },
                // A question an agent asks is the owner's: the owner decides whom else to ask.
                AskIn::Question(question) => Ask::Person {
                    who: Who::Owner,
                    question: question.question,
                },
            })
            .collect();
        return Ok(Came::Asks(asks));
    }
    let handed = (r.hands_in.into_iter())
        .map(|h| Handed {
            block: BlockId(h.block),
            content: h.content,
            matters: h.matters,
            value: h.value,
        })
        .collect();
    Ok(Came::HandsIn(handed))
}

#[cfg(test)]
mod adversarial;

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn the_schema_is_json_with_no_choice_at_its_top() {
        let schema: serde_json::Value = serde_json::from_str(SCHEMA).unwrap();
        for word in ["oneOf", "anyOf", "allOf"] {
            assert!(schema.get(word).is_none(), "{word} at the top");
        }
    }

    #[test]
    fn a_return_that_asks_asks() {
        let r = json!({"hands_in": [], "asks": [{"tool": "check", "input": ""}, {"question": "Which name?"}]});
        let asks = vec![
            Ask::Tool {
                name: "check".to_string(),
                input: String::new(),
            },
            Ask::Person {
                who: Who::Owner,
                question: "Which name?".to_string(),
            },
        ];
        assert_eq!(came(&r), Ok(Came::Asks(asks)));
    }

    #[test]
    fn a_return_that_hands_in_hands_in() {
        let r = json!({"hands_in": [
            {"block": "plan", "content": "Greet by name.", "matters": true},
            {"block": "verdict", "content": "Fine.", "value": "yes", "matters": false},
        ], "asks": []});
        let handed = vec![
            Handed {
                block: "plan".into(),
                content: "Greet by name.".to_string(),
                matters: true,
                value: None,
            },
            Handed {
                block: "verdict".into(),
                content: "Fine.".to_string(),
                matters: false,
                value: Some("yes".to_string()),
            },
        ];
        assert_eq!(came(&r), Ok(Came::HandsIn(handed)));
    }

    #[test]
    fn a_return_of_nothing_hands_in_nothing() {
        assert_eq!(
            came(&json!({"hands_in": [], "asks": []})),
            Ok(Came::HandsIn(Vec::new()))
        );
    }

    fn nothing() -> Result<Came, String> {
        Ok(Came::HandsIn(Vec::new()))
    }

    fn plan() -> Result<Came, String> {
        Ok(Came::HandsIn(vec![Handed {
            block: "plan".into(),
            content: "Greet.".to_string(),
            matters: true,
            value: None,
        }]))
    }

    fn form() -> Value {
        json!({"hands_in": [{"block": "plan", "content": "Greet.", "matters": true}], "asks": []})
    }

    #[test]
    fn a_form_written_as_a_string_is_read() {
        let text = form().to_string();
        assert_eq!(came(&json!(text)), plan());
        assert_eq!(came(&json!(format!("{text}}}"))), plan());
        assert_eq!(came(&json!(format!("{text}\n]}}"))), plan());
        assert!(came(&json!(format!("{text} and more"))).is_err());
    }

    #[test]
    fn a_form_wrapped_in_one_key_is_read() {
        assert_eq!(came(&json!({"outcome": form()})), plan());
        assert_eq!(came(&json!({"outcome": form().to_string()})), plan());
        assert!(came(&json!({"outcome": form(), "more": 1})).is_err());
        assert!(came(&json!({"foo": 1})).is_err());
    }

    #[test]
    fn a_list_left_out_is_empty() {
        assert_eq!(came(&json!({})), nothing());
        assert_eq!(came(&json!({"outcome": {}})), nothing());
        assert_eq!(came(&json!({"hands_in": null, "asks": null})), nothing());
        assert_eq!(came(&json!({"hands_in": []})), nothing());
        let asks = came(&json!({"asks": [{"question": "Which?"}]}));
        assert!(matches!(asks, Ok(Came::Asks(_))));
    }

    #[test]
    fn a_list_written_as_a_string_is_read() {
        let r = json!({"hands_in": form()["hands_in"].to_string(), "asks": "[]"});
        assert_eq!(came(&r), plan());
    }

    #[test]
    fn a_null_of_the_other_form_is_left_out() {
        let tool = json!({"hands_in": [], "asks": [{"tool": "check", "input": "", "question": null}]});
        let question = json!({"hands_in": [], "asks": [{"question": "Which?", "tool": null, "input": null}]});
        assert!(matches!(came(&tool), Ok(Came::Asks(a)) if matches!(a[0], Ask::Tool { .. })));
        assert!(matches!(came(&question), Ok(Came::Asks(a)) if matches!(a[0], Ask::Person { .. })));
        let value =
            json!({"hands_in": [{"block": "plan", "content": "Greet.", "matters": true, "value": null}], "asks": []});
        assert_eq!(came(&value), plan());
    }

    #[test]
    fn what_is_required_is_never_made_up() {
        for r in [
            json!({"hands_in": [{"block": "plan", "content": "x", "matters": null}], "asks": []}),
            json!({"hands_in": [{"block": "plan", "content": null, "matters": true}], "asks": []}),
            json!({"hands_in": [{"block": null, "content": "x", "matters": true}], "asks": []}),
            json!({"hands_in": [], "asks": [{"tool": null}]}),
            json!({"hands_in": [], "asks": [{"question": null}]}),
            json!({"hands_in": [], "asks": [{"tool": "check", "input": null}]}),
        ] {
            assert!(came(&r).is_err(), "{r} is taken");
        }
    }

    #[test]
    fn a_return_in_its_form_is_read_as_it_was() {
        for r in [form(), json!({"hands_in": [], "asks": [{"question": "Which?"}]})] {
            assert_eq!(normalised(&r), r);
        }
    }

    #[test]
    fn a_malformed_return_is_none() {
        for r in [
            json!({"hands_in": [{"block": "plan", "matters": true}], "asks": []}),
            json!({"hands_in": [{"block": "plan", "content": "Greet by name."}], "asks": []}),
            json!({"hands_in": [], "asks": [{"tool": "check"}]}),
            json!({"hands_in": [], "asks": [], "more": 1}),
            json!("hands in nothing"),
        ] {
            assert!(came(&r).is_err(), "{r} is taken");
        }
    }
}
