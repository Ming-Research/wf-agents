//! What an agent returns at the end of a round: one form for every cell, given to the command line as its JSON
//! schema, so that it is part of the prefix every round shares and the prompt cache keeps. A return hands in blocks
//! or asks for calls: a tool to run, or a question for the owner. Whether a block is one the cell may write, and a
//! value one its choice offers, the engine checks, and sends back what does not meet it.

use decide::{BlockId, Who};
use engine::Came;
use serde::Deserialize;
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

/// What came back, from the structured return; or why it is not one. A return that asks for anything asks, and what
/// it hands in beside is not read; one that asks for nothing hands in, if only nothing.
pub fn came(structured: &serde_json::Value) -> Result<Came, String> {
    let r: Return =
        serde_json::from_value(structured.clone()).map_err(|e| format!("the return is not of its form: {e}"))?;
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

    #[test]
    fn a_malformed_return_is_none() {
        for r in [
            json!({"hands_in": []}),
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
