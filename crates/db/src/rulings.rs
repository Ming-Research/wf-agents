//! Ruling on a design card, as the owner and the architect both do: a verdict on each of its decisions, all at once,
//! added to its rulings, with the decisions disagreed with reaching the work's engineer as one interjection.

use rusqlite::{OptionalExtension, Transaction, params};
use serde_json::{Value, json};

use crate::{DESIGN_CARD_RULING, now};

/// A verdict on one decision of a design card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verdict {
    /// The decision's number on the card, from 1.
    pub decision: i64,
    /// confirm or disagree.
    pub verdict: String,
    /// Another of the decision's approaches, chosen instead, by its name.
    pub option: Option<String>,
    /// Why, for a disagreement.
    pub text: Option<String>,
}

/// Rules on a design card that is not replaced, as `by` (the owner or the architect): a verdict on each of its
/// decisions, all at once, is added to its rulings. The card is ruled once both the architect and the owner have
/// ruled on it; it counts as confirmed only when the owner's ruling confirms every decision, and where the two differ
/// the owner's word goes. The decisions disagreed with reach the work's engineer as one interjection from `by`, each
/// with the approach chosen instead and why; a ruling that confirms every decision needs none. Gives what was done.
pub fn rule_design_card(tx: &Transaction<'_>, card: i64, by: &str, verdicts: &[Verdict]) -> Result<String, String> {
    let sql = |e: rusqlite::Error| e.to_string();
    let found: Option<(String, i64, String, String)> = tx
        .query_row(
            "SELECT status, work, body, rulings FROM design_cards WHERE id = ?1",
            [card],
            |x| Ok((x.get(0)?, x.get(1)?, x.get(2)?, x.get(3)?)),
        )
        .optional()
        .map_err(sql)?;
    let Some((status, work, body, rulings)) = found else {
        return Err(format!("no design card {card}"));
    };
    if status == "replaced" {
        return Err(format!("design card {card} is replaced: it can no longer be ruled"));
    }
    let body: Vec<Value> = match serde_json::from_str(&body).unwrap_or(Value::Null) {
        Value::Array(xs) => xs,
        Value::Null => Vec::new(),
        other => vec![other],
    };
    // A card always has a decision; one whose body holds none is ruled as one decision.
    let count = body.len().max(1) as i64;
    let mut sorted: Vec<&Verdict> = verdicts.iter().collect();
    sorted.sort_by_key(|d| d.decision);
    let numbers: Vec<i64> = sorted.iter().map(|d| d.decision).collect();
    if numbers != (1..=count).collect::<Vec<_>>() {
        return Err(format!(
            "a ruling on design card {card} gives one verdict for each of its {count} decisions, by number"
        ));
    }
    let mut decisions = Vec::new();
    let mut said = Vec::new();
    for d in sorted {
        let k = d.decision;
        let text = d.text.as_deref().map(str::trim).filter(|t| !t.is_empty());
        let option = d.option.as_deref().map(str::trim).filter(|o| !o.is_empty());
        let decision = body.get(k as usize - 1);
        match d.verdict.as_str() {
            "confirm" if option.is_none() => {}
            "confirm" => return Err(format!("decision {k}: another approach is chosen only by disagreeing")),
            "disagree" if text.is_none() && option.is_none() => {
                return Err(format!(
                    "decision {k}: a disagreement says why, or chooses another approach"
                ));
            }
            "disagree" => {}
            _ => return Err(format!("decision {k}: the verdict is confirm or disagree")),
        }
        let names: Vec<&str> = decision
            .and_then(|x| x["options"].as_array())
            .map(|os| os.iter().filter_map(|o| o["name"].as_str()).collect())
            .unwrap_or_default();
        if let Some(o) = option
            && !names.is_empty()
            && !names.contains(&o)
        {
            return Err(format!("decision {k} has no approach {o:?}"));
        }
        let mut v = json!({ "decision": k, "verdict": d.verdict });
        if let Some(o) = option {
            v["option"] = json!(o);
        }
        if let Some(t) = text {
            v["text"] = json!(t);
        }
        decisions.push(v);
        if d.verdict == "disagree" {
            let node = decision
                .and_then(|x| x["node"].as_str())
                .map(|n| format!(" ({n})"))
                .unwrap_or_default();
            let chosen = option
                .map(|o| format!(", choosing the approach \"{o}\""))
                .unwrap_or_default();
            let why = text.map(|t| format!(": {t}")).unwrap_or_default();
            said.push(format!("decision {k}{node}: disagree{chosen}{why}"));
        } else {
            said.push(format!("decision {k} confirmed"));
        }
    }
    let disagree = decisions.iter().any(|v| v["verdict"] == "disagree");
    let mut list: Vec<Value> = serde_json::from_str(&rulings).unwrap_or_default();
    list.push(json!({ "by": by, "at": now(), "decisions": decisions }));
    let both = ["owner", "architect"]
        .iter()
        .all(|who| list.iter().any(|r| r["by"] == *who));
    let status = if both { "ruled" } else { status.as_str() };
    tx.execute(
        "UPDATE design_cards SET rulings = ?2, status = ?3 WHERE id = ?1",
        params![card, Value::Array(list).to_string(), status],
    )
    .map_err(sql)?;
    if disagree {
        let lines: Vec<String> = said.iter().map(|s| format!("- {s}")).collect();
        let text = format!("{DESIGN_CARD_RULING} {card}:\n{}", lines.join("\n"));
        tx.execute(
            "INSERT INTO interjections (work, line, speaker, text) VALUES (?1, 'engineer', ?2, ?3)",
            params![work, by, text],
        )
        .map_err(sql)?;
    }
    Ok(format!("design card {card} ruled"))
}
