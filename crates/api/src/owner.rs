//! What the owner decides, on the page and on the command line alike: an answer to an ask of the owner, a ruling on a
//! design card, and letting a paused work go on. Each is written as the owner, so `changes` records it as theirs, and the engine reads it as it
//! goes on.

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

/// Rules on a design card that is not replaced: the ruling is added to its rulings and it is ruled. A disagreement
/// reaches the work's engineer as an interjection; a confirmation needs none.
pub fn rule(db: &mut Db, design_card: i64, r: &Ruling) -> Result<String, String> {
    let found: Option<(String, i64)> = db
        .conn()
        .query_row(
            "SELECT c.status, r.work FROM design_cards c JOIN rounds r ON r.id = c.round WHERE c.id = ?1",
            [design_card],
            |x| Ok((x.get(0)?, x.get(1)?)),
        )
        .optional()
        .map_err(sql)?;
    let Some((status, work)) = found else {
        return Err(format!("no design card {design_card}"));
    };
    if status == "replaced" {
        return Err(format!(
            "design card {design_card} is replaced: it can no longer be ruled"
        ));
    }
    if !matches!(r.verdict.as_str(), "confirm" | "disagree") {
        return Err("the verdict on a design card is confirm or disagree".into());
    }
    if r.verdict == "disagree" && r.text.trim().is_empty() {
        return Err("a disagreement says why".into());
    }
    let mut ruling = json!({ "by": "owner", "at": db::now(), "text": r.text, "verdict": r.verdict });
    if let Some(o) = &r.option {
        ruling["option"] = json!(o);
    }
    let disagree = r.verdict == "disagree";
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
            let text = format!("{} {design_card}: disagree. {}", db::DESIGN_CARD_RULING, r.text);
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
        tx.execute("UPDATE works SET paused = NULL WHERE id = ?1", [work])?;
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
                "INSERT INTO rounds (work, cell, input, config_commit, started_at) VALUES (1, '4', '{}', 'c', 1)",
                [],
            )?;
            tx.execute("INSERT INTO owner_asks (round, body) VALUES (1, '{\"to\": \"owner\"}')", [])?;
            tx.execute("INSERT INTO design_cards (round, body, status) VALUES (1, '[]', 'open')", [])?;
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
            tx.execute("UPDATE works SET paused = 'cell 13: no' WHERE id = 1", [])?;
            Ok(())
        })
        .unwrap();
        assert_eq!(resume(&mut db, 1).unwrap(), "work 1 goes on");
        let paused: Option<String> = db
            .conn()
            .query_row("SELECT paused FROM works WHERE id = 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(paused, None);
        assert!(resume(&mut db, 9).is_err());
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

    // A disagreement needs a reason and reaches the engineer as an interjection; a confirmation does not.
    #[test]
    fn the_owner_rules_on_a_design_card() {
        let mut db = setup();
        let no_reason = Ruling {
            verdict: "disagree".into(),
            text: " ".into(),
            option: None,
        };
        assert!(rule(&mut db, 1, &no_reason).is_err());
        let disagree = Ruling {
            verdict: "disagree".into(),
            text: "Keep the old name.".into(),
            option: Some("B".into()),
        };
        assert_eq!(rule(&mut db, 1, &disagree).unwrap(), "design card 1 ruled");
        let (status, rulings): (String, String) = db
            .conn()
            .query_row("SELECT status, rulings FROM design_cards WHERE id = 1", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        let rulings: Value = serde_json::from_str(&rulings).unwrap();
        assert_eq!(status, "ruled");
        assert_eq!(rulings[0]["option"], "B");
        let said: String = db
            .conn()
            .query_row("SELECT text FROM interjections WHERE work = 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(said, "Ruling on design card 1: disagree. Keep the old name.");
        let confirm = Ruling {
            verdict: "confirm".into(),
            text: String::new(),
            option: None,
        };
        rule(&mut db, 1, &confirm).unwrap();
        let n: i64 = db
            .conn()
            .query_row("SELECT count(*) FROM interjections", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1);
        assert!(
            rule(
                &mut db,
                1,
                &Ruling {
                    verdict: "maybe".into(),
                    text: "x".into(),
                    option: None
                }
            )
            .is_err()
        );
    }
}
