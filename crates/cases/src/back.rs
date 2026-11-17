//! What a round of a case's history returns, written as an agent writes a return of the cell's form, so that the
//! engine reads it as it reads an agent's (crate `carry`, module `returns`) and checks it as any: a return of the kind
//! that does the cell's work, dealing with all that ran the round, each block it hands in in `outputs`, a choice as its
//! value with its line of why, a shaped block as its shape, and each block handed in as changing nothing in meaning
//! named in `meaning_unchanged`.

use serde_json::{Map, Value, json};

use crate::checked::Round;

/// The return of a round of the history, dealing with these, by the names of what the cell watches.
pub fn outcome(round: &Round, dealt_with: &[String]) -> Value {
    let mut outputs = Map::new();
    let mut unchanged = Vec::new();
    for hand in &round.hands_in {
        let value = match (&hand.shaped, &hand.value) {
            (Some(shaped), _) => serde_json::to_value(shaped).unwrap_or(Value::Null),
            (None, Some(value)) => json!({ "value": value, "why": hand.content }),
            (None, None) => Value::String(hand.content.clone()),
        };
        if hand.unchanged {
            unchanged.push(hand.block.to_string());
        }
        outputs.insert(hand.block.to_string(), value);
    }
    json!({
        "outcome": {
            "dealt_with": dealt_with,
            "meaning_unchanged": unchanged,
            "observations": [],
            "protected_changes": {},
            "outputs": outputs,
        }
    })
}

#[cfg(test)]
mod tests {
    use decide::{BlockId, CellId};

    use super::*;
    use crate::checked::Hand;

    #[test]
    fn a_round_returns_each_block_in_the_form_s_shape_dealing_with_what_ran_it() {
        let hand = |block: &str, content: &str, value: Option<&str>, unchanged: bool| Hand {
            block: BlockId::from(block),
            content: content.to_string(),
            value: value.map(str::to_string),
            unchanged,
            shaped: None,
        };
        let mut asked = toml::Table::new();
        asked.insert("name".into(), toml::Value::String("big".into()));
        let round = Round {
            cell: CellId::from("A1"),
            hands_in: vec![
                hand("plan", "Cut seven.", None, false),
                hand("deep", "One round.", Some("no"), false),
                hand("acceptance", "As before.", None, true),
                Hand {
                    shaped: Some(toml::Value::Array(vec![toml::Value::Table(asked)])),
                    ..hand("a1_questions", "", None, false)
                },
            ],
            commit: None,
        };
        let back = outcome(&round, &["background".to_string()]);
        let expected = json!({
            "outcome": {
                "dealt_with": ["background"],
                "meaning_unchanged": ["acceptance"],
                "observations": [],
                "protected_changes": {},
                "outputs": {
                    "plan": "Cut seven.",
                    "deep": { "value": "no", "why": "One round." },
                    "acceptance": "As before.",
                    "a1_questions": [{ "name": "big" }],
                }
            }
        });
        assert_eq!(back, expected);
    }
}
