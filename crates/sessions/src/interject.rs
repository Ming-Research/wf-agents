//! Interjections handed to a round that runs: the command line's hook after each tool use asks the session manager for
//! what is said to the round's line and waits (the hub's messages to the line, `line_sayings`), which it hands in to
//! the round and records as taken in it, and the command line adds them to the session before the agent goes on. Those
//! still waiting when the round ends go at the top of the next round's message. What the owner said with images waits
//! for that message too: the hook adds text only, and the images go to the agent with their words.
//!
//! The round's message has given the interjections that waited when the round began, and the round's record lists them
//! (`rounds.input`, `$.engine.interjections`); the hook records them as delivered in the round, so that the owner's
//! page does not show them as not taken up while it runs, but does not hand them in again. An interjection that needs
//! no answer (`informs`) is handed in under its own lead, which says so.

use db::rusqlite::params;
use db::{Actor, Db};
use serde_json::{Value, json};

/// What the hook says before the interjections that may need an answer.
const ASKS: &str = "Said to you while this round runs; answer in this round's return, as format:common says.";

/// What the hook says before the interjections that need no answer (`informs`).
const TELLS: &str = "Told to you while this round runs; none of it needs an answer.";

/// The interjections waiting for a work's line, handed in to its running round: each recorded as delivered in the
/// round, and what the hook gives the session, as the command line reads a hook's output after a tool use; none when
/// none waits. One that the round's message gave, images included, is recorded as delivered in the round but not handed
/// in again, since the agent has it. One that came since and carries images, its own or those of the owner's words its
/// matter holds, is left for the line's next message, which gives the words and the images together. Those that may
/// need an answer come first, then those that need none, each under its own lead.
pub fn deliver(db: &mut Db, work: i64, round: i64, line: &str) -> Result<Option<String>, String> {
    let said: Vec<(i64, bool, String)> = db
        .write(Actor::Engine, |tx| {
            // A round that has ended, such as the one a keep-warm ping replays the command line of, takes none.
            let open: bool = tx.query_row(
                "SELECT count(*) > 0 FROM rounds WHERE work = ?1 AND n = ?2 AND ended_at IS NULL",
                params![work, round],
                |r| r.get(0),
            )?;
            if !open {
                return Ok(Vec::new());
            }
            let mut st = tx.prepare(
                "SELECT j.id, j.speaker, j.text, j.design_card, c.key, j.informs,
                        j.id IN (SELECT g.value FROM rounds r, json_each(r.input, '$.engine.interjections') g
                                 WHERE r.work = ?1 AND r.n = ?3) AS given,
                        EXISTS (SELECT 1 FROM attachments a
                                WHERE a.message = j.id OR a.matter = j.matter) AS imaged
                 FROM line_sayings j LEFT JOIN design_cards c ON c.id = j.design_card
                 WHERE j.work = ?1 AND j.line = ?2 AND j.taken_round IS NULL
                 ORDER BY j.id",
            )?;
            // What the round's message gave, images included, the agent has: it is recorded as delivered and not
            // handed in again. What came since is handed in and recorded, unless it carries images.
            let mut handed = Vec::new();
            let mut recorded = Vec::new();
            let rows = st.query_map(params![work, line, round], |r| {
                let words = db::interjection_words(
                    &r.get::<_, String>(1)?,
                    &r.get::<_, String>(2)?,
                    r.get(3)?,
                    r.get::<_, Option<String>>(4)?.as_deref(),
                );
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, bool>(5)?,
                    words,
                    r.get::<_, bool>(6)?,
                    r.get::<_, bool>(7)?,
                ))
            })?;
            for row in rows {
                let (id, informs, words, given, imaged) = row?;
                if given {
                    recorded.push(id);
                } else if !imaged {
                    recorded.push(id);
                    handed.push((id, informs, words));
                }
            }
            for id in &recorded {
                db::take(tx, *id, work, round)?;
            }
            Ok(handed)
        })
        .map_err(|e| e.to_string())?;
    if said.is_empty() {
        return Ok(None);
    }
    // Each kind under its lead, left out when it has none.
    let under = |told: bool, lead: &str| {
        let lines: Vec<String> = said
            .iter()
            .filter(|(_, informs, _)| *informs == told)
            .map(|(id, _, words)| format!("### ij:{id}\n\n{words}"))
            .collect();
        (!lines.is_empty()).then(|| format!("{lead}\n\n{}", lines.join("\n\n")))
    };
    let context = [under(false, ASKS), under(true, TELLS)]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join("\n\n");
    Ok(Some(
        json!({ "hookSpecificOutput": { "hookEventName": "PostToolUse", "additionalContext": context } }).to_string(),
    ))
}

/// The interjections a round was handed while it ran, as `waiting_interjections` gives a row: those recorded as
/// delivered in the round and not among those its message gave.
pub fn delivered_during(db: &Db, work: i64, round: i64, given: &[i64]) -> Result<Vec<Value>, String> {
    let mut st = db
        .conn()
        .prepare(
            "SELECT j.work, j.id, j.line, j.speaker, j.text, j.design_card, c.key FROM line_sayings j
             LEFT JOIN design_cards c ON c.id = j.design_card WHERE j.work = ?1 AND j.taken_round = ?2
             ORDER BY j.id",
        )
        .map_err(|e| e.to_string())?;
    let rows = st
        .query_map(params![work, round], |r| {
            Ok(json!({
                "work": r.get::<_, i64>(0)?,
                "interjection": r.get::<_, i64>(1)?,
                "line": r.get::<_, String>(2)?,
                "speaker": r.get::<_, String>(3)?,
                "text": r.get::<_, String>(4)?,
                "design_card": r.get::<_, Option<i64>>(5)?,
                "card_key": r.get::<_, Option<String>>(6)?,
            }))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<Value>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows
        .into_iter()
        .filter(|r| !r["interjection"].as_i64().is_some_and(|i| given.contains(&i)))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    // An interjection waiting for the line is handed in to the round that runs, once, and recorded as delivered in it,
    // the owner's question about a decision card naming the card; a round that has ended takes none.
    #[test]
    fn an_interjection_is_handed_in_to_the_running_round_once() {
        let mut db = Db::in_memory().unwrap();
        db.write(Actor::Engine, |tx| {
            tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
            tx.execute(
                "INSERT INTO works (flow_version, graph, at) VALUES (1, 'engineer', 'R1')",
                [],
            )?;
            tx.execute(
                "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES (1, 1, 'R1', '{}', 'c', 1)",
                [],
            )?;
            let owner = db::Sender::Owner;
            db::say_to_line(tx, &db::ToLine::new(owner, 1, "engineer", "Look at the log."), None)?;
            db::say_to_line(tx, &db::ToLine::new(owner, 1, "review", "Not yours."), None)?;
            tx.execute(
                "INSERT INTO design_cards (work, round, key, body, status) VALUES (1, 1, 'cache-size', '{}', 'open')",
                [],
            )?;
            let about_card = db::ToLine {
                design_card: Some(1),
                ..db::ToLine::new(owner, 1, "engineer", "Why this size?")
            };
            db::say_to_line(tx, &about_card, None)?;
            Ok(())
        })
        .unwrap();
        let out = deliver(&mut db, 1, 1, "engineer").unwrap().unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["hookSpecificOutput"]["hookEventName"], "PostToolUse");
        let said = v["hookSpecificOutput"]["additionalContext"].as_str().unwrap();
        assert!(
            said.contains("### ij:1\n\nFrom the owner: Look at the log.") && !said.contains("Not yours"),
            "{said}"
        );
        assert!(
            said.contains("### ij:3\n\nFrom the owner, about decision card 1 (cache-size): Why this size?"),
            "{said}"
        );
        assert_eq!(deliver(&mut db, 1, 1, "engineer").unwrap(), None);
        let during = delivered_during(&db, 1, 1, &[]).unwrap();
        assert_eq!(during.len(), 2);
        assert_eq!(during[0]["speaker"], "owner");
        assert_eq!(
            (&during[1]["design_card"], &during[1]["card_key"]),
            (&json!(1), &json!("cache-size"))
        );
        assert!(delivered_during(&db, 1, 1, &[1, 3]).unwrap().is_empty());
        db.write(Actor::Engine, |tx| {
            tx.execute("UPDATE rounds SET ended_at = 2 WHERE work = 1 AND n = 1", [])?;
            Ok(())
        })
        .unwrap();
        assert_eq!(deliver(&mut db, 1, 1, "review").unwrap(), None);
    }

    // What the owner said with images is not handed in during a round, since the hook adds text only: it waits for the
    // line's next message, which gives the words and the images together.
    #[test]
    fn an_interjection_with_images_waits_for_the_next_message() {
        let mut db = Db::in_memory().unwrap();
        db.write(Actor::Owner, |tx| {
            tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
            tx.execute(
                "INSERT INTO works (flow_version, graph, at) VALUES (1, 'engineer', 'R1')",
                [],
            )?;
            tx.execute(
                "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES (1, 1, 'R1', '{}', 'c', 1)",
                [],
            )?;
            let see = db::say_to_line(tx, &db::ToLine::new(db::Sender::Owner, 1, "engineer", "See this."), None)?;
            let image = db::attachments::Image {
                media_type: "image/png".into(),
                bytes: b"\x89PNG\r\n\x1a\n".to_vec(),
            };
            db::attachments::attach(tx, db::attachments::Words::Message(see), &[image])?;
            db::say_to_line(tx, &db::ToLine::new(db::Sender::Owner, 1, "engineer", "And this."), None)?;
            Ok(())
        })
        .unwrap();
        let out = deliver(&mut db, 1, 1, "engineer").unwrap().unwrap();
        assert!(out.contains("ij:2") && !out.contains("ij:1"), "{out}");
        let waiting: i64 = db
            .conn()
            .query_row("SELECT count(*) FROM line_sayings WHERE taken_round IS NULL", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(waiting, 1);
    }
}
