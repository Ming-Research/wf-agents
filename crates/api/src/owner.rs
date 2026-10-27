//! What the owner decides, on the page and on the command line alike: an answer to an ask of the owner, a ruling on a
//! design card, pausing a work and letting a paused work go on, and letting every work the owner paused go on. Each is
//! written as the owner, so `changes` records it as theirs, and the engine reads it as it goes on.

use db::rusqlite::{OptionalExtension, params};
use db::{Actor, Db};
use serde_json::{Value, json};

use crate::{Answer, Ruling};

fn sql(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// Answers an ask of the owner that waits for an answer. For what the architect is asked, the answer is also the
/// architect's reply.
pub fn answer(db: &mut Db, ask: i64, a: &Answer) -> Result<String, String> {
    let asked: Option<(String, Option<String>)> = db
        .conn()
        .query_row("SELECT body, answer FROM owner_asks WHERE id = ?1", [ask], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .optional()
        .map_err(sql)?;
    let Some((body, answered)) = asked else {
        return Err(format!("no ask {ask}"));
    };
    if answered.is_some() {
        return Err(format!("ask {ask} is answered already"));
    }
    if a.text.trim().is_empty() {
        return Err("an answer says something".into());
    }
    let mut answer = json!({ "by": "owner", "at": db::now(), "text": a.text });
    let body: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
    if body["to"] == "architect" {
        let mut reply = a.reply.clone().unwrap_or_else(|| json!({}));
        reply["text"] = json!(a.text);
        reply["why"] = json!(
            a.why
                .clone()
                .unwrap_or_else(|| "The owner answered for the architect.".into())
        );
        answer["reply"] = reply;
    }
    db.write(Actor::Owner, |tx| {
        tx.execute(
            "UPDATE owner_asks SET answer = ?2 WHERE id = ?1",
            params![ask, answer.to_string()],
        )?;
        Ok(())
    })
    .map_err(sql)?;
    Ok(format!("ask {ask} answered"))
}

/// Rules on a design card that is not replaced: a verdict on each of its decisions, all at once, is added to its
/// rulings, and it is ruled. The card counts as confirmed only when every decision is. The decisions disagreed with
/// reach the work's engineer as one interjection, each with the approach chosen instead and why; a ruling that
/// confirms every decision needs none.
pub fn rule(db: &mut Db, design_card: i64, r: &Ruling) -> Result<String, String> {
    let found: Option<(String, i64, String)> = db
        .conn()
        .query_row(
            "SELECT status, work, body FROM design_cards WHERE id = ?1",
            [design_card],
            |x| Ok((x.get(0)?, x.get(1)?, x.get(2)?)),
        )
        .optional()
        .map_err(sql)?;
    let Some((status, work, body)) = found else {
        return Err(format!("no design card {design_card}"));
    };
    if status == "replaced" {
        return Err(format!(
            "design card {design_card} is replaced: it can no longer be ruled"
        ));
    }
    let body: Vec<Value> = match serde_json::from_str(&body).unwrap_or(Value::Null) {
        Value::Array(xs) => xs,
        Value::Null => Vec::new(),
        other => vec![other],
    };
    // A card always has a decision; one whose body holds none is ruled as one decision.
    let count = body.len().max(1) as i64;
    let mut decisions: Vec<&crate::DecisionRuling> = r.decisions.iter().collect();
    decisions.sort_by_key(|d| d.decision);
    let numbers: Vec<i64> = decisions.iter().map(|d| d.decision).collect();
    if numbers != (1..=count).collect::<Vec<_>>() {
        return Err(format!(
            "a ruling on design card {design_card} gives one verdict for each of its {count} decisions, by number"
        ));
    }
    let mut verdicts = Vec::new();
    let mut said = Vec::new();
    for d in decisions {
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
        verdicts.push(v);
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
    let disagree = verdicts.iter().any(|v| v["verdict"] == "disagree");
    let ruling = json!({ "by": "owner", "at": db::now(), "decisions": verdicts });
    db.write(Actor::Owner, |tx| {
        let rulings: String = tx.query_row("SELECT rulings FROM design_cards WHERE id = ?1", [design_card], |x| {
            x.get(0)
        })?;
        let mut list: Vec<Value> = serde_json::from_str(&rulings).unwrap_or_default();
        list.push(ruling.clone());
        tx.execute(
            "UPDATE design_cards SET rulings = ?2, status = 'ruled' WHERE id = ?1",
            params![design_card, Value::Array(list).to_string()],
        )?;
        if disagree {
            let lines: Vec<String> = said.iter().map(|s| format!("- {s}")).collect();
            let text = format!("{} {design_card}:\n{}", db::DESIGN_CARD_RULING, lines.join("\n"));
            tx.execute(
                "INSERT INTO interjections (work, line, speaker, text) VALUES (?1, 'engineer', 'owner', ?2)",
                params![work, text],
            )?;
        }
        Ok(())
    })
    .map_err(sql)?;
    Ok(format!("design card {design_card} ruled"))
}

/// Pauses a work that goes on: a round of it under way ends first, and then it takes no step by itself until the owner
/// lets it go on, by itself or with every work the owner paused. Why is the owner's words, or that the owner paused it.
pub fn pause(db: &mut Db, work: i64, why: Option<&str>) -> Result<String, String> {
    let found: Option<(Option<String>, Option<String>)> = db
        .conn()
        .query_row("SELECT paused, ended FROM works WHERE id = ?1", [work], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .optional()
        .map_err(sql)?;
    match found {
        None => return Err(format!("no work {work}")),
        Some((_, Some(_))) => return Err(format!("work {work} has ended")),
        Some((Some(_), None)) => return Err(format!("work {work} is paused already")),
        Some((None, None)) => {}
    }
    let why = why
        .map(str::trim)
        .filter(|w| !w.is_empty())
        .unwrap_or(db::PAUSED_BY_OWNER);
    db.write(Actor::Owner, |tx| {
        tx.execute(
            "UPDATE works SET paused = ?2, paused_by = 'owner' WHERE id = ?1",
            params![work, why],
        )?;
        Ok(())
    })
    .map_err(sql)?;
    Ok(format!("work {work} pauses"))
}

/// Lets every work the owner paused go on, as the owner's Start all does beside letting the engine start works again;
/// a work the engine paused stays paused until the owner lets it go on by itself.
pub fn start_all(db: &mut Db) -> Result<String, String> {
    let n = db
        .write(Actor::Owner, |tx| {
            Ok(tx.execute(
                "UPDATE works SET paused = NULL, paused_by = NULL WHERE paused_by = 'owner' AND ended IS NULL",
                [],
            )?)
        })
        .map_err(sql)?;
    Ok(format!("{n} works paused by the owner go on"))
}

/// Lets a paused work go on: its cell runs again. The engine closes a round the pause left open, with the reason.
pub fn resume(db: &mut Db, work: i64) -> Result<String, String> {
    let paused: Option<Option<String>> = db
        .conn()
        .query_row("SELECT paused FROM works WHERE id = ?1", [work], |r| r.get(0))
        .optional()
        .map_err(sql)?;
    match paused {
        None => return Err(format!("no work {work}")),
        Some(None) => return Err(format!("work {work} is not paused")),
        Some(Some(_)) => {}
    }
    db.write(Actor::Owner, |tx| {
        tx.execute("UPDATE works SET paused = NULL, paused_by = NULL WHERE id = ?1", [work])?;
        Ok(())
    })
    .map_err(sql)?;
    Ok(format!("work {work} goes on"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> Db {
        let mut db = Db::in_memory().unwrap();
        db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO items (project, title, problem, why, deliver, level) VALUES ('p', 't', 'x', 'y', 'change', '1')",
                [],
            )?;
            tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
            tx.execute("INSERT INTO works (item, flow_version, graph, at) VALUES (1, 1, 'engineer', '16')", [])?;
            tx.execute(
                "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES (1, 1, '4', '{}', 'c', 1)",
                [],
            )?;
            tx.execute(
                "INSERT INTO owner_asks (work, round, body) VALUES (1, 1, '{\"to\": \"owner\"}')",
                [],
            )?;
            tx.execute(
                "INSERT INTO design_cards (work, round, body, status) VALUES (1, 1, '[]', 'open')",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        db
    }

    // The owner lets a paused work go on; one that is not paused is refused.
    #[test]
    fn the_owner_lets_a_paused_work_go_on() {
        let mut db = setup();
        assert!(resume(&mut db, 1).is_err());
        db.write(Actor::Engine, |tx| {
            tx.execute(
                "UPDATE works SET paused = 'cell 13: no', paused_by = 'engine' WHERE id = 1",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        assert_eq!(resume(&mut db, 1).unwrap(), "work 1 goes on");
        let paused: (Option<String>, Option<String>) = db
            .conn()
            .query_row("SELECT paused, paused_by FROM works WHERE id = 1", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(paused, (None, None));
        assert!(resume(&mut db, 9).is_err());
    }

    // The owner pauses a work that goes on, once; Start all lets go on the works the owner paused, and not one the
    // engine paused.
    #[test]
    fn the_owner_pauses_a_work_and_start_all_lets_go_on_only_the_owner_s() {
        let mut db = setup();
        let paused = |db: &Db, id: i64| -> (Option<String>, Option<String>) {
            db.conn()
                .query_row("SELECT paused, paused_by FROM works WHERE id = ?1", [id], |r| {
                    Ok((r.get(0)?, r.get(1)?))
                })
                .unwrap()
        };
        assert_eq!(pause(&mut db, 1, None).unwrap(), "work 1 pauses");
        assert_eq!(
            paused(&db, 1),
            (Some(db::PAUSED_BY_OWNER.to_string()), Some("owner".to_string()))
        );
        assert!(pause(&mut db, 1, None).is_err());
        assert!(pause(&mut db, 9, None).is_err());
        db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO works (item, flow_version, graph, at) VALUES (1, 1, 'engineer', '4')",
                [],
            )?;
            tx.execute(
                "UPDATE works SET paused = 'cell 4: no', paused_by = 'engine' WHERE id = 2",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        assert_eq!(start_all(&mut db).unwrap(), "1 works paused by the owner go on");
        assert_eq!(paused(&db, 1), (None, None));
        assert_eq!(
            paused(&db, 2),
            (Some("cell 4: no".to_string()), Some("engine".to_string()))
        );
        // The owner's words, when given, say why.
        pause(&mut db, 1, Some("Wait for the release.")).unwrap();
        assert_eq!(paused(&db, 1).0.as_deref(), Some("Wait for the release."));
    }

    // The owner answers an ask once; a second answer is refused.
    #[test]
    fn the_owner_answers_an_ask_once() {
        let mut db = setup();
        let a = Answer {
            text: "Chosen option: B. Keep it small.".into(),
            why: None,
            reply: None,
        };
        assert_eq!(answer(&mut db, 1, &a).unwrap(), "ask 1 answered");
        let got: String = db
            .conn()
            .query_row("SELECT answer FROM owner_asks WHERE id = 1", [], |r| r.get(0))
            .unwrap();
        let got: Value = serde_json::from_str(&got).unwrap();
        assert_eq!(
            (got["by"].as_str(), got["text"].as_str()),
            (Some("owner"), Some(a.text.as_str()))
        );
        assert!(answer(&mut db, 1, &a).is_err());
        assert!(answer(&mut db, 2, &a).is_err());
    }

    fn one(decision: i64, verdict: &str, text: Option<&str>, option: Option<&str>) -> crate::DecisionRuling {
        crate::DecisionRuling {
            decision,
            verdict: verdict.into(),
            text: text.map(str::to_string),
            option: option.map(str::to_string),
        }
    }

    fn ruling(decisions: Vec<crate::DecisionRuling>) -> Ruling {
        Ruling { decisions }
    }

    // A disagreement needs a reason or another approach and reaches the engineer as an interjection; a confirmation
    // does not.
    #[test]
    fn the_owner_rules_on_a_design_card() {
        let mut db = setup();
        assert!(rule(&mut db, 1, &ruling(vec![one(1, "disagree", Some(" "), None)])).is_err());
        let disagree = ruling(vec![one(1, "disagree", Some("Keep the old name."), None)]);
        assert_eq!(rule(&mut db, 1, &disagree).unwrap(), "design card 1 ruled");
        let (status, rulings): (String, String) = db
            .conn()
            .query_row("SELECT status, rulings FROM design_cards WHERE id = 1", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        let rulings: Value = serde_json::from_str(&rulings).unwrap();
        assert_eq!(status, "ruled");
        assert_eq!(rulings[0]["decisions"][0]["text"], "Keep the old name.");
        let said: String = db
            .conn()
            .query_row("SELECT text FROM interjections WHERE work = 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            said,
            "Ruling on design card 1:\n- decision 1: disagree: Keep the old name."
        );
        rule(&mut db, 1, &ruling(vec![one(1, "confirm", None, None)])).unwrap();
        let n: i64 = db
            .conn()
            .query_row("SELECT count(*) FROM interjections", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1);
        assert!(rule(&mut db, 1, &ruling(vec![one(1, "maybe", Some("x"), None)])).is_err());
        assert!(rule(&mut db, 1, &ruling(Vec::new())).is_err());
    }

    // A card of three decisions is ruled on each at once: two confirmed and one disagreed with, choosing another of
    // its approaches, records a verdict for each, and only the one disagreed with reaches the engineer with its node,
    // the approach chosen and why; a ruling that leaves a decision out, or names an approach the decision does not
    // have, is refused.
    #[test]
    fn the_owner_confirms_some_decisions_and_disagrees_with_one() {
        let mut db = setup();
        let decision =
            |node: &str| json!({ "node": node, "options": [{ "name": "A" }, { "name": "B" }, { "name": "C" }] });
        let body = json!([
            decision("design/a.md"),
            decision("design/b.md"),
            decision("design/c.md")
        ]);
        db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO design_cards (work, round, body, status) VALUES (1, 1, ?1, 'open')",
                [body.to_string()],
            )?;
            Ok(())
        })
        .unwrap();
        let partial = ruling(vec![one(1, "confirm", None, None), one(3, "confirm", None, None)]);
        assert!(
            rule(&mut db, 2, &partial)
                .unwrap_err()
                .contains("each of its 3 decisions")
        );
        let unknown = ruling(vec![
            one(1, "confirm", None, None),
            one(2, "disagree", None, Some("Z")),
            one(3, "confirm", None, None),
        ]);
        assert!(rule(&mut db, 2, &unknown).unwrap_err().contains("no approach"));
        let mixed = ruling(vec![
            one(3, "confirm", None, None),
            one(1, "confirm", None, None),
            one(2, "disagree", Some("One table is enough."), Some("B")),
        ]);
        rule(&mut db, 2, &mixed).unwrap();
        let rulings: String = db
            .conn()
            .query_row("SELECT rulings FROM design_cards WHERE id = 2", [], |r| r.get(0))
            .unwrap();
        let rulings: Value = serde_json::from_str(&rulings).unwrap();
        let verdicts: Vec<(i64, &str)> = rulings[0]["decisions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|d| (d["decision"].as_i64().unwrap(), d["verdict"].as_str().unwrap()))
            .collect();
        assert_eq!(verdicts, [(1, "confirm"), (2, "disagree"), (3, "confirm")]);
        assert!(!db::ruling_confirms(&rulings[0], 3) && db::ruling_disagrees(&rulings[0]));
        let said: String = db
            .conn()
            .query_row("SELECT text FROM interjections WHERE work = 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            said,
            "Ruling on design card 2:\n- decision 1 confirmed\n- decision 2 (design/b.md): disagree, choosing the \
             approach \"B\": One table is enough.\n- decision 3 confirmed"
        );
    }
}
