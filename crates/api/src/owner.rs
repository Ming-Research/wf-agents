//! What the owner decides, on the page and on the command line alike: an answer to an ask of the owner, a ruling on a
//! design card, what the owner says to a work's line, pausing a work and lifting a work's pause, the switch that keeps
//! waiting sessions warm while every work is paused, forwarding a fault of wf-agents to an architect, the setting that
//! forwards each new one as it comes, and ending a talk with an architect. Each is written as the owner, so `changes`
//! records it as theirs, and the engine reads it as it goes on. What the owner says to the architect is
//! `db::say_to_architect`.

use db::rusqlite::{Connection, OptionalExtension, params};
use db::{Actor, Db, Matter, queue_matter};
use serde_json::{Value, json};

use crate::{Answer, ProjectsSaid, Ruling, WorkSaid};

fn sql(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// Says something to a work's engineer as the owner: an interjection, handed to the engineer's round that runs after
/// its next tool use, or at the top of its next round. The owner speaks to the engineer alone of a work's lines: its
/// helpers (the advisor, the review, the adversarial testing) take nothing from the owner. A work at its end (`E1`) is
/// told nothing more: its way on no longer changes. Gives the interjection.
pub fn interject(db: &mut Db, work: i64, line: &str, text: &str) -> Result<i64, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("what is said to a work says something".into());
    }
    if line != "engineer" {
        return Err(format!(
            "the owner speaks to a work's engineer, not to its {line}: its helpers take nothing from the owner"
        ));
    }
    live(db, work)?;
    db.write(Actor::Owner, |tx| {
        tx.execute(
            "INSERT INTO interjections (work, line, speaker, text) VALUES (?1, ?2, 'owner', ?3)",
            params![work, line, text],
        )?;
        Ok(tx.last_insert_rowid())
    })
    .map_err(sql)
}

/// That a work can still be said something: it is not ended, and an engineer's work is not at its end (`E1`), whose
/// way on no longer changes.
fn live(db: &Db, work: i64) -> Result<(), String> {
    let found: Option<(Option<String>, String, String)> = db
        .conn()
        .query_row("SELECT ended, graph, at FROM works WHERE id = ?1", [work], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .optional()
        .map_err(sql)?;
    match found {
        None => return Err(format!("no work {work}")),
        Some((Some(why), _, _)) => return Err(format!("work {work} has ended: {why}")),
        // A work at its end no longer changes its way on: what would change it is a new item.
        Some((None, graph, at)) if graph == "engineer" && db::is_cell("engineer", "E1", &at) => {
            return Err(format!(
                "work {work} is at its end and its way on no longer changes: a change becomes a new item"
            ));
        }
        Some(_) => {}
    }
    Ok(())
}

/// What the engineer is told of what the owner said to its project's architect alone, in the work's group.
const TOLD_TO_ARCHITECT: &str = "Said to the architect, for you to know";

/// What the owner says in a work's group, whose members are the owner, the engineer and the project's architect: to
/// the engineer, as an interjection; to the architect, as the owner's words naming the work, which the engineer is
/// given as a word that needs no answer; or to both, the engineer's interjection naming the architect's matter, so that
/// the group shows the words once. What is said to the engineer alone the architect is given at its next pick. With a
/// question named, the words answer that question of the engineer's, which the architect holds.
pub fn say_to_work(db: &mut Db, work: i64, said: &WorkSaid) -> Result<String, String> {
    if let Some(m) = said.question {
        return answer_question(db, work, m, &said.text);
    }
    let to = said.to.as_deref().unwrap_or("engineer");
    if to == "engineer" {
        let id = interject(db, work, to, &said.text)?;
        return Ok(format!("interjection {id} goes to the engineer of work {work}"));
    }
    if to != "architect" && to != "both" {
        return Err(format!(
            "what is said in a work's group is for its engineer, its architect or both, not for the {to}"
        ));
    }
    let text = said.text.trim();
    if text.is_empty() {
        return Err("what is said to a work says something".into());
    }
    live(db, work)?;
    let (item, project) = item_of(db, work)?;
    let m = Matter {
        project: project.clone(),
        kind: "owner_words".into(),
        detail: Some("said".into()),
        what: format!("The owner says: {}", db::one_sentence(text)),
        body: json!({ "text": text, "where": "work", "work": work }),
        item,
        ..Matter::default()
    };
    let (copy, informs) = if to == "both" {
        (text.to_string(), 0)
    } else {
        (format!("{TOLD_TO_ARCHITECT}: {text}"), 1)
    };
    let id = db
        .write(Actor::Owner, |tx| {
            let matter = queue_matter(tx, &m)?;
            tx.execute(
                "INSERT INTO interjections (work, line, speaker, text, informs, matter)
                 VALUES (?1, 'engineer', 'owner', ?2, ?3, ?4)",
                params![work, copy, informs, matter],
            )?;
            Ok(matter)
        })
        .map_err(sql)?;
    Ok(match to {
        "both" => format!("the engineer of work {work} and the architect of {project}, as matter {id}, have it"),
        _ => format!("the architect of {project} has it as matter {id}, and the engineer of work {work} is told"),
    })
}

/// What the owner says in a group of the architects of projects the configuration connects (`projects`, the group's,
/// in order): to the architect named, or to every one; each comes to its queue as the owner's words naming the group,
/// the projects it was said to listed in each, so that the group shows the words once.
pub fn say_to_projects(db: &mut Db, projects: &[String], said: &ProjectsSaid) -> Result<String, String> {
    let text = said.text.trim();
    if text.is_empty() {
        return Err("what is said to the architects says something".into());
    }
    let to: Vec<String> = match said.to.as_deref() {
        Some(p) if projects.iter().any(|x| x == p) => vec![p.to_string()],
        Some(p) => return Err(format!("project {p} is not in the group {}", said.key)),
        None => projects.to_vec(),
    };
    let ids = db
        .write(Actor::Owner, |tx| {
            let mut ids = Vec::new();
            for p in &to {
                let m = Matter {
                    project: p.clone(),
                    kind: "owner_words".into(),
                    detail: Some("said".into()),
                    what: format!("The owner says: {}", db::one_sentence(text)),
                    body: json!({ "text": text, "where": "projects", "group": said.key, "to": to }),
                    ..Matter::default()
                };
                ids.push(queue_matter(tx, &m)?);
            }
            Ok(ids)
        })
        .map_err(sql)?;
    let listed: Vec<String> = ids.iter().map(i64::to_string).collect();
    Ok(format!(
        "the architects of {} have it as matters {}",
        to.join(", "),
        listed.join(", ")
    ))
}

/// The item of an engineer's work, and its project, whose architect the work's group holds.
fn item_of(db: &Db, work: i64) -> Result<(Option<i64>, String), String> {
    let of: Option<(Option<i64>, Option<String>)> = db
        .conn()
        .query_row(
            "SELECT w.item, i.project FROM works w LEFT JOIN items i ON i.id = w.item WHERE w.id = ?1",
            [work],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(sql)?;
    match of {
        Some((item, Some(project))) => Ok((item, project)),
        _ => Err(format!(
            "work {work} is on no item of a project, so it has no architect to say it to"
        )),
    }
}

/// The owner's answer, in a work's group, to a question of its engineer's that an architect holds: the answer becomes
/// the reply of the architect's matter, said by the owner, which the work takes as it takes an architect's reply, and
/// the matter leaves the architect's queue, saying so; the architect is told what the owner answered at its next pick.
/// A question the architect is handling now, or that is not open, is refused.
fn answer_question(db: &mut Db, work: i64, matter: i64, text: &str) -> Result<String, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("an answer says something".into());
    }
    let open: Option<(i64, Option<String>)> = db
        .conn()
        .query_row(
            "SELECT m.work, m.project FROM architect_matters m
             WHERE m.id = ?1 AND m.reply IS NULL AND m.handled_round IS NULL AND m.withdrawn IS NULL
               AND EXISTS (SELECT 1 FROM blockers b
                           WHERE b.matter = m.id AND b.ended IS NULL AND b.holder = 'architect')",
            [matter],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(sql)?;
    match open {
        Some((w, _)) if w == work => {}
        _ => {
            return Err(format!(
                "matter {matter} is no question of work {work} that an architect holds now"
            ));
        }
    }
    // A work of the architect that picked it is answering it: the owner's words go to the architect instead.
    let picked: i64 = db
        .conn()
        .query_row(
            "SELECT count(*) FROM rounds p JOIN works w ON w.id = p.work
             WHERE w.graph = 'architect' AND w.ended IS NULL AND p.transition IS NOT NULL
               AND ?1 IN (json_extract(p.output, '$.outcome.pick.matter'), json_extract(p.output, '$.queue.matter'))",
            [matter],
            |r| r.get(0),
        )
        .map_err(sql)?;
    if picked > 0 {
        return Err(format!(
            "the architect is answering matter {matter} now: say it to the architect instead"
        ));
    }
    let reply = json!({
        "by": "owner",
        "at": db::now(),
        "text": text,
        "why": "The owner answered the question in the work's group.",
    });
    db.write(Actor::Owner, |tx| {
        tx.execute(
            "UPDATE architect_matters SET reply = ?2, withdrawn = ?3 WHERE id = ?1",
            params![
                matter,
                reply.to_string(),
                format!("the owner answered it in the group of work {work}")
            ],
        )?;
        Ok(())
    })
    .map_err(sql)?;
    Ok(format!(
        "the question of work {work} is answered, and its architect is told"
    ))
}

/// Ends, as the owner, the talk with a project's architect that waits for the owner (Done): the architect's next turn
/// of it hands in what it came to and takes its way on. Refused when no talk is open.
pub fn end_talk(db: &mut Db, project: &str) -> Result<String, String> {
    let ended = db.write(Actor::Owner, |tx| db::end_talk(tx, project)).map_err(sql)?;
    match ended {
        Some(m) => Ok(format!(
            "the talk about matter {m} ends: the architect hands in what it came to"
        )),
        None => Err(format!("the architect of {project} has no talk open with you")),
    }
}

/// Replies, as the owner, to a question a project's architect pinned in their one-to-one: the owner's words, said with
/// the question ticked, answer it, and its matter comes back to the architect's queue with them, first, to the cell
/// that asked. A question whose matter is gone (handled, or withdrawn as its item was cancelled or its work taken over)
/// is no longer pinned: the reply then comes to the architect's queue as the owner's words, with the question, and the
/// architect tells the owner. Refused for a question of no architect of the project, or one replied to already.
pub fn reply_to_question(db: &mut Db, project: &str, ask: i64, text: &str) -> Result<String, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("a reply says something".into());
    }
    let asked: Option<(String, Option<String>, String, Option<String>)> = db
        .conn()
        .query_row(
            "SELECT a.body, a.answer, w.graph, w.project FROM owner_asks a JOIN works w ON w.id = a.work
             WHERE a.id = ?1",
            [ask],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()
        .map_err(sql)?;
    let Some((body, answered, graph, of)) = asked else {
        return Err(format!("no question {ask}"));
    };
    if graph != "architect" || of.as_deref() != Some(project) {
        return Err(format!("question {ask} is not one the architect of {project} put"));
    }
    if answered.is_some() {
        return Err(format!("question {ask} is replied to already"));
    }
    let body: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
    let parked: Option<i64> = db
        .conn()
        .query_row(
            "SELECT id FROM architect_matters WHERE asked = ?1 AND handled_round IS NULL AND withdrawn IS NULL",
            [ask],
            |r| r.get(0),
        )
        .optional()
        .map_err(sql)?;
    let answer = json!({ "by": "owner", "at": db::now(), "text": text });
    db.write(Actor::Owner, |tx| {
        tx.execute(
            "UPDATE owner_asks SET answer = ?2 WHERE id = ?1",
            params![ask, answer.to_string()],
        )?;
        if parked.is_none() {
            let question = body["question"].clone();
            let m = Matter {
                project: project.to_string(),
                kind: "owner_words".into(),
                detail: Some("answer".into()),
                what: format!("The owner replied to question {ask}: {}", db::one_sentence(text)),
                body: json!({ "text": text, "where": "page", "question": question, "owner_ask": ask,
                              "gone": "the matter the question parked is closed" }),
                ..Matter::default()
            };
            queue_matter(tx, &m)?;
        }
        Ok(())
    })
    .map_err(sql)?;
    Ok(match parked {
        Some(m) => format!("question {ask} is replied to: matter {m} comes back to the architect with the reply"),
        None => {
            format!("question {ask} is replied to: its matter is closed, so the architect has the reply as your words")
        }
    })
}

/// The option the owner chose, for an ask of the architect's that offers options.
fn choice_of(a: &Answer) -> Option<String> {
    a.choice
        .as_deref()
        .map(str::trim)
        .filter(|c| !c.is_empty())
        .map(str::to_string)
}

/// The matters an ask of an architect holds, by their numbers: those it was put about, which come back to their
/// architect's queue once it is answered.
fn holds_of(body: &Value) -> Vec<i64> {
    match &body["holds"] {
        Value::Array(xs) => xs.iter().filter_map(Value::as_i64).collect(),
        v => v.as_i64().into_iter().collect(),
    }
}

/// Answers an ask an architect put, by its kind: the owner's view of a study or approval of a policy, with the option
/// chosen alone, which ends the talk the architect has open with the owner; an answer to what the architect asked,
/// which brings back the matters the ask holds, each to the cell that asked about it, or, for an ask that held neither a
/// matter nor a work of the architect, comes to the architect's queue as the owner's words. An ask of the changes an earlier architect collected for approval is closed: item
/// changes are reviewed now.
fn architect_answer(db: &mut Db, ask: i64, body: &Value, a: &Answer) -> Result<String, String> {
    let at = db::now();
    let text = a.text.trim();
    let kind = body["kind"].as_str().unwrap_or_default();
    match kind {
        // The owner decides on a study or a text by its buttons alone, words going to the architect in their talk; a
        // decision while the architect talks with the owner ends that talk, as Done does.
        "study" | "policy_approval" => {
            let offered: Vec<&str> = body["choices"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|c| c[0].as_str())
                .collect();
            let chosen = choice_of(a)
                .filter(|c| offered.contains(&c.as_str()))
                .ok_or_else(|| format!("the answer chooses one of: {}", offered.join(", ")))?;
            let answer = json!({ "by": "owner", "at": at, "choice": chosen });
            let project = body["project"].as_str().unwrap_or_default().to_string();
            db.write(Actor::Owner, |tx| {
                tx.execute(
                    "UPDATE owner_asks SET answer = ?2 WHERE id = ?1",
                    params![ask, answer.to_string()],
                )?;
                db::end_talk(tx, &project)?;
                Ok(())
            })
            .map_err(sql)?;
        }
        "changes" => {
            return Err(format!(
                "ask {ask} put changes to the owner's approval, which the architect no longer asks: its item changes \
                 are reviewed, and the architect is told to go on without it"
            ));
        }
        _ => {
            if text.is_empty() {
                return Err("an answer says something".into());
            }
            let answer = json!({ "by": "owner", "at": at, "text": text });
            // An ask that held no work of the architect and no matter, as an earlier architect put open questions,
            // comes back to the queue as the owner's words.
            let open = kind == "open" && holds_of(body).is_empty();
            let asked = if kind == "open" {
                body["choice"].clone()
            } else {
                body["asked"].clone()
            };
            db.write(Actor::Owner, |tx| {
                tx.execute(
                    "UPDATE owner_asks SET answer = ?2 WHERE id = ?1",
                    params![ask, answer.to_string()],
                )?;
                if open {
                    let project = match body["project"].as_str() {
                        Some(p) => Some(p.to_string()),
                        None => {
                            tx.query_row("SELECT min(project) FROM items", [], |r| r.get::<_, Option<String>>(0))?
                        }
                    };
                    if let Some(project) = project {
                        let m = Matter {
                            project,
                            kind: "owner_words".into(),
                            detail: Some("answer".into()),
                            what: format!("The owner answered ask {ask}: {}", db::one_sentence(text)),
                            body: json!({ "ask": asked, "answer": text, "owner_ask": ask }),
                            ..Matter::default()
                        };
                        queue_matter(tx, &m)?;
                    }
                }
                Ok(())
            })
            .map_err(sql)?;
        }
    }
    Ok(format!("ask {ask} answered"))
}

/// Answers an ask of the owner that waits for an answer. An ask an earlier engine put for the architect, whose answer
/// the owner gave for it, is the architect's reply too; an ask the architect put is answered by its kind.
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
    let body: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
    if body["from"] == "architect" {
        return architect_answer(db, ask, &body, a);
    }
    if a.text.trim().is_empty() {
        return Err("an answer says something".into());
    }
    let mut answer = json!({ "by": "owner", "at": db::now(), "text": a.text });
    // The owner may hand a question it holds back to the architect, with the answer as a decision to carry out.
    match a.to.as_deref() {
        None => {}
        Some("architect") if body["to"] == "owner" => answer["to"] = json!("architect"),
        Some(other) => return Err(format!("ask {ask} cannot be handed to {other}")),
    }
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
/// confirms every decision is a word to the engineer that needs no answer and turns nothing (`informs`). The owner
/// alone rules on a design card; once the owner confirms every decision, the architect has nothing to look at in it:
/// the card leaves the architect's queue, unless a work of the architect has it in hand.
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
        } else {
            // A ruling that confirms every decision is a word to the engineer, which needs no answer and turns nothing.
            tx.execute(
                "INSERT INTO interjections (work, line, speaker, text, informs) VALUES (?1, 'engineer', 'owner', ?2, 1)",
                params![
                    work,
                    format!("The owner confirmed every decision of design card {design_card}: nothing changes.")
                ],
            )?;
            tx.execute(
                &format!(
                    "DELETE FROM architect_matters
                     WHERE kind = 'design_card' AND json_extract(body, '$.design_card') = ?1 AND handled_round IS NULL
                       AND asked IS NULL AND id NOT IN (
                         SELECT coalesce(json_extract(p.output, '$.queue.matter'),
                                         json_extract(p.output, '$.outcome.pick.matter')) FROM rounds p
                         JOIN works w ON w.id = p.work
                         WHERE w.graph = 'architect' AND w.ended IS NULL AND p.cell IN {}
                           AND p.transition IS NOT NULL)",
                    db::cells_sql("architect", &["Q0", "Q1"])
                ),
                [design_card],
            )?;
        }
        Ok(())
    })
    .map_err(sql)?;
    Ok(format!("design card {design_card} ruled"))
}

/// Pauses a work that goes on: a round of it under way ends first, and then it takes no step by itself, whatever the
/// global state, until the owner starts it again; letting every work go on does not lift it. Why is the owner's words,
/// or that the owner paused it.
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

/// What the owner's Start of a work says, once the work is one that can go on: not ended.
pub fn can_start(conn: &Connection, work: i64) -> Result<String, String> {
    let ended: Option<Option<String>> = conn
        .query_row("SELECT ended FROM works WHERE id = ?1", [work], |r| r.get(0))
        .optional()
        .map_err(sql)?;
    match ended {
        None => Err(format!("no work {work}")),
        Some(Some(_)) => Err(format!("work {work} has ended")),
        Some(None) => Ok(format!("work {work} goes on")),
    }
}

/// Pauses the work of a project's architect that runs, as the owner, as [`pause`] pauses a work; none when no work of
/// it runs or it is paused already.
pub fn pause_architect(db: &mut Db, project: &str) -> Result<String, String> {
    let work: Option<i64> = db
        .conn()
        .query_row(
            "SELECT max(id) FROM works WHERE graph = 'architect' AND project = ?1 AND ended IS NULL AND paused IS NULL",
            [project],
            |r| r.get(0),
        )
        .map_err(sql)?;
    match work {
        Some(w) => pause(db, w, None),
        None => Ok(format!("the architect of {project} has nothing running to pause")),
    }
}

/// The project an ask of the owner's names, or the configuration's one project when it names none: each project has
/// its architect, and the owner names which.
pub fn project_named(config: &config::Config, given: Option<&str>) -> Result<String, String> {
    let names: Vec<&String> = config.projects.keys().collect();
    match given.map(str::trim).filter(|g| !g.is_empty()) {
        Some(g) if config.projects.contains_key(g) => Ok(g.to_string()),
        Some(g) => Err(format!(
            "no project {g}; the projects: {}",
            names.iter().map(|n| n.as_str()).collect::<Vec<_>>().join(", ")
        )),
        None => match names.as_slice() {
            [one] => Ok((*one).clone()),
            _ => Err(format!(
                "name the project, one of: {}",
                names.iter().map(|n| n.as_str()).collect::<Vec<_>>().join(", ")
            )),
        },
    }
}

/// Closes an observation, as the owner, saying why: it is closed as the owner's, whatever its architect made of it.
pub fn close_observation(db: &mut Db, id: i64, why: &str) -> Result<String, String> {
    let why = why.trim();
    if why.is_empty() {
        return Err("closing an observation says why".into());
    }
    let open: Option<Option<String>> = db
        .conn()
        .query_row("SELECT closed FROM observations_all WHERE id = ?1", [id], |r| r.get(0))
        .optional()
        .map_err(sql)?;
    match open {
        None => return Err(format!("no observation {id}")),
        Some(Some(_)) => return Err(format!("observation {id} is closed already")),
        Some(None) => {}
    }
    db.write(Actor::Owner, |tx| {
        tx.execute(
            "UPDATE observations SET closed = 'owner', closed_why = ?2 WHERE id = ?1",
            params![id, why],
        )?;
        Ok(())
    })
    .map_err(sql)?;
    Ok(format!("observation {id} closed"))
}

/// Lifts a paused work's pause, as the owner: its cell runs again. The engine closes a round the pause left open, with
/// the reason. A work the architect stopped is the owner's release of it, which the architect is told.
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
    let released = db.write(Actor::Owner, |tx| db::lift_pause(tx, work)).map_err(sql)?;
    Ok(if released {
        format!("work {work} goes on; the architect, which had stopped it, is told")
    } else {
        format!("work {work} goes on")
    })
}

/// The owner's switch named so, which a switch with no row has as it is at first: on.
fn switch(conn: &Connection, name: &str) -> Result<bool, String> {
    let v: Option<i64> = conn
        .query_row("SELECT value FROM owner_switches WHERE name = ?1", [name], |r| r.get(0))
        .optional()
        .map_err(sql)?;
    Ok(v.is_none_or(|v| v != 0))
}

/// Whether the sessions of waiting works are kept warm while every work is paused: the owner's switch, on at first.
pub fn keep_warm_paused(conn: &Connection) -> Result<bool, String> {
    switch(conn, "keep_warm_paused")
}

/// Switches keeping the sessions of waiting works warm while every work is paused, as the owner.
pub fn set_keep_warm_paused(db: &mut Db, on: bool) -> Result<String, String> {
    db.write(Actor::Owner, |tx| {
        tx.execute(
            "INSERT INTO owner_switches (name, value) VALUES ('keep_warm_paused', ?1)
             ON CONFLICT (name) DO UPDATE SET value = excluded.value",
            [i64::from(on)],
        )?;
        Ok(())
    })
    .map_err(sql)?;
    Ok(if on {
        "waiting sessions are kept warm while every work is paused".into()
    } else {
        "no waiting session is kept warm while every work is paused".into()
    })
}

/// Forwards a fault of wf-agents, the engine's notice of it, to a project's architect, as the owner: it comes to the
/// architect's queue as a report of the fault, and the notice is read. A fault goes to each architect once.
pub fn forward_fault(db: &mut Db, notice: i64, project: &str) -> Result<String, String> {
    let matter = db
        .write(Actor::Owner, |tx| {
            db::forward_fault(tx, notice, project, db::Forwarder::Owner)
        })
        .map_err(sql)?;
    Ok(format!(
        "fault {notice} goes to the architect of {} as matter {matter}",
        project.trim()
    ))
}

/// The project whose architect each new fault of wf-agents goes to as it comes, by the owner's setting; none for
/// nobody, as it is at first.
pub fn fault_forwarding(conn: &Connection) -> Result<Option<String>, String> {
    db::fault_forwarding(conn).map_err(sql)
}

/// Sets, as the owner, where each new fault of wf-agents goes as it comes: to a project's architect, or with none to
/// nobody, the owner then forwarding a fault by hand or not at all.
pub fn set_fault_forwarding(db: &mut Db, project: Option<&str>) -> Result<String, String> {
    db.write(Actor::Owner, |tx| db::set_fault_forwarding(tx, project))
        .map_err(sql)?;
    Ok(match project {
        Some(p) => format!("each new fault of wf-agents goes to the architect of {}", p.trim()),
        None => "new faults of wf-agents go to nobody: you forward each or not".into(),
    })
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
            tx.execute("INSERT INTO works (item, flow_version, graph, at) VALUES (1, 1, 'engineer', 'A3')", [])?;
            tx.execute(
                "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES (1, 1, 'I1', '{}', 'c', 1)",
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

    // The owner answers in a work's group a question of its engineer's that the architect holds: the answer is the
    // matter's reply, said by the owner, and the matter leaves the queue; a question no architect holds, one of another
    // work, and one a work of the architect has in hand are refused.
    #[test]
    fn the_owner_answers_a_question_the_architect_holds() {
        let mut db = setup();
        db.write(Actor::Engine, |tx| {
            tx.execute("UPDATE works SET at = 'A2' WHERE id = 1", [])?;
            tx.execute(
                "INSERT INTO architect_matters (project, kind, detail, what, body, item, work, round, at)
                 VALUES ('p', 'engineer_question', 'split', 'Work 1 asks', '{}', 1, 1, 1, 5),
                        ('p', 'engineer_question', 'split', 'Work 1 asks again', '{}', 1, 1, 1, 6)",
                [],
            )?;
            tx.execute(
                "INSERT INTO blockers (work, cell, round, kind, what, holder, project, matter, at, since)
                 VALUES (1, 'I1', 1, 'question', 'Split it?', 'architect', 'p', 1, 5, 5)",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        let said = |question: i64, text: &str| WorkSaid {
            text: text.into(),
            to: None,
            question: Some(question),
        };
        assert!(say_to_work(&mut db, 1, &said(2, "No.")).is_err());
        assert!(say_to_work(&mut db, 2, &said(1, "No.")).is_err());
        assert!(say_to_work(&mut db, 1, &said(1, " ")).is_err());
        say_to_work(&mut db, 1, &said(1, "Do not split it.")).unwrap();
        let (reply, withdrawn): (String, Option<String>) = db
            .conn()
            .query_row("SELECT reply, withdrawn FROM architect_matters WHERE id = 1", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        let reply: Value = serde_json::from_str(&reply).unwrap();
        assert_eq!(
            (reply["by"].as_str(), reply["text"].as_str()),
            (Some("owner"), Some("Do not split it."))
        );
        assert!(withdrawn.is_some_and(|w| w.contains("group of work 1")));
        let queued: i64 = db
            .conn()
            .query_row("SELECT count(*) FROM architect_queue WHERE matter = 1", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(queued, 0);
        // Answered, it is no longer open to answer.
        assert!(say_to_work(&mut db, 1, &said(1, "Again.")).is_err());
    }

    // The owner lets a paused work go on; one that is not paused is refused.
    #[test]
    fn the_owner_lets_a_paused_work_go_on() {
        let mut db = setup();
        assert!(resume(&mut db, 1).is_err());
        db.write(Actor::Engine, |tx| {
            tx.execute(
                "UPDATE works SET paused = 'cell tools: no', paused_by = 'engine' WHERE id = 1",
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
        // A work the architect stopped is the owner's release of it, which the architect is told as the owner's words.
        db.write(Actor::Engine, |tx| {
            tx.execute(
                "UPDATE works SET paused = ?1, paused_by = 'engine' WHERE id = 1",
                [format!("{}: it waits for the parser", db::STOPPED_BY_ARCHITECT)],
            )?;
            Ok(())
        })
        .unwrap();
        assert_eq!(
            resume(&mut db, 1).unwrap(),
            "work 1 goes on; the architect, which had stopped it, is told"
        );
        let told: (String, String, i64) = db
            .conn()
            .query_row("SELECT kind, body ->> 'text', item FROM architect_matters", [], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .unwrap();
        assert_eq!(
            told,
            (
                "owner_words".to_string(),
                "I started work 1, which you had stopped (it waits for the parser): it goes on.".to_string(),
                1
            )
        );
        let actor: String = db
            .conn()
            .query_row(
                "SELECT actor FROM changes WHERE table_name = 'architect_matters'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(actor, "owner");
    }

    // The switch that keeps waiting sessions warm while every work is paused is on at first, and the owner's once set.
    #[test]
    fn keeping_sessions_warm_while_paused_is_the_owner_s_switch() {
        let mut db = setup();
        assert!(keep_warm_paused(db.conn()).unwrap());
        set_keep_warm_paused(&mut db, false).unwrap();
        assert!(!keep_warm_paused(db.conn()).unwrap());
        set_keep_warm_paused(&mut db, true).unwrap();
        assert!(keep_warm_paused(db.conn()).unwrap());
        let actors: Vec<String> = db
            .conn()
            .prepare("SELECT actor FROM changes WHERE table_name = 'owner_switches' ORDER BY id")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(actors, ["owner", "owner"]);
    }

    // The owner pauses a work that goes on, once, and a work of the architect the same way; why is the owner's words
    // when given.
    #[test]
    fn the_owner_pauses_a_work_once() {
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
        assert_eq!(
            pause_architect(&mut db, "p").unwrap(),
            "the architect of p has nothing running to pause"
        );
        db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO works (item, flow_version, graph, at, project) VALUES (NULL, 1, 'architect', 'U1', 'p')",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        assert_eq!(pause_architect(&mut db, "p").unwrap(), "work 2 pauses");
        assert_eq!(paused(&db, 2).1.as_deref(), Some("owner"));
        // The owner's words, when given, say why.
        resume(&mut db, 1).unwrap();
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
            choice: None,
            to: None,
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

    // A disagreement needs a reason or another approach and reaches the engineer as an interjection; a ruling that
    // confirms every decision reaches it as a word that needs no answer. Once the owner confirms every decision, the
    // card waits no longer for the architect to look at it.
    #[test]
    fn the_owner_rules_on_a_design_card() {
        let mut db = setup();
        db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO architect_matters (project, kind, what, body, item, work, round, at)
                 VALUES ('p', 'design_card', 'Design card 1 to look at', '{\"design_card\":1}', 1, 1, 1, 1)",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        let looks = |db: &Db| -> i64 {
            db.conn()
                .query_row(
                    "SELECT count(*) FROM architect_queue WHERE kind = 'design_card'",
                    [],
                    |r| r.get(0),
                )
                .unwrap()
        };
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
        assert_eq!(looks(&db), 1);
        let said: String = db
            .conn()
            .query_row("SELECT text FROM interjections WHERE work = 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            said,
            "Ruling on design card 1:\n- decision 1: disagree: Keep the old name."
        );
        rule(&mut db, 1, &ruling(vec![one(1, "confirm", None, None)])).unwrap();
        let said: Vec<(String, i64)> = db
            .conn()
            .prepare("SELECT text, informs FROM interjections ORDER BY id")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(said.len(), 2, "{said:?}");
        assert_eq!(
            said[1],
            (
                "The owner confirmed every decision of design card 1: nothing changes.".to_string(),
                1
            )
        );
        assert_eq!(looks(&db), 0);
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

    /// An ask the architect of project p put, on a work of the architect, with its body.
    fn architect_ask(db: &mut Db, body: Value) -> i64 {
        db.write(Actor::Engine, |tx| {
            let works: i64 = tx.query_row("SELECT count(*) FROM works WHERE graph = 'architect'", [], |r| r.get(0))?;
            if works == 0 {
                tx.execute(
                    "INSERT INTO works (id, item, flow_version, graph, at, project)
                     VALUES (9, NULL, 1, 'architect', 'A3', 'p')",
                    [],
                )?;
                tx.execute(
                    "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES (9, 1, 'A3', '{}', 'c', 1)",
                    [],
                )?;
            }
            tx.execute("INSERT INTO owner_asks (work, round, body) VALUES (9, 1, ?1)", [body.to_string()])?;
            Ok(tx.last_insert_rowid())
        })
        .unwrap()
    }

    /// An answer with the option chosen that `given` names, as the page sends it.
    fn with_reply(text: &str, given: Value) -> Answer {
        Answer {
            text: text.into(),
            why: None,
            reply: None,
            choice: given["choice"].as_str().map(str::to_string),
            to: None,
        }
    }

    // The owner's view of the architect's study chooses one of the options the ask offers, and the answer keeps it
    // alone, the owner's words going to the architect in their talk; a talk with the architect that waits for the
    // owner's reply ends with it, as Done ends it.
    #[test]
    fn the_owner_chooses_an_option_of_the_architect_s_study() {
        let mut db = setup();
        db.write(Actor::Owner, |tx| {
            tx.execute(
                "INSERT INTO architect_matters (project, kind, detail, what, body, at, talk_held)
                 VALUES ('p', 'owner_words', 'said', 'The owner says: Why?', ?1, 1, 2)",
                [json!({ "text": "Why?", "where": "page" }).to_string()],
            )?;
            Ok(())
        })
        .unwrap();
        let choices = json!([
            ["satisfied", "Write it"],
            ["study_on", "More"],
            ["policy_stays", "None"]
        ]);
        let ask = architect_ask(
            &mut db,
            json!({ "from": "architect", "project": "p", "work": 9, "cell": "P3", "kind": "study", "study": {},
                    "choices": choices }),
        );
        assert!(answer(&mut db, ask, &with_reply("Fine.", json!({ "choice": "maybe" }))).is_err());
        answer(&mut db, ask, &with_reply("Fine.", json!({ "choice": "policy_stays" }))).unwrap();
        let got: String = db
            .conn()
            .query_row("SELECT answer FROM owner_asks WHERE id = ?1", [ask], |r| r.get(0))
            .unwrap();
        let got: Value = serde_json::from_str(&got).unwrap();
        assert_eq!((&got["choice"], got.get("text")), (&json!("policy_stays"), None));
        let talks: Vec<Option<i64>> = {
            let mut st = db.conn().prepare("SELECT talk_ended FROM architect_matters").unwrap();
            st.query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect()
        };
        assert!(matches!(talks.as_slice(), [Some(_)]), "{talks:?}");
        assert!(end_talk(&mut db, "p").is_err());
    }

    // An ask an earlier architect put of the changes it collected for approval is not answered: the architect's item
    // changes are reviewed now.
    #[test]
    fn an_ask_of_changes_for_approval_is_closed() {
        let mut db = setup();
        let ask = architect_ask(
            &mut db,
            json!({ "from": "architect", "work": 9, "cell": "A15", "kind": "changes", "changes": [] }),
        );
        let e = answer(&mut db, ask, &with_reply("Yes.", json!({ "choice": "approve_all" }))).unwrap_err();
        assert!(e.contains("no longer asks"), "{e}");
    }

    // The owner's answer to an ask that held a matter brings the matter back to its architect's queue, with the cell
    // that asked to go back to; an ask that held neither a matter nor a work of the architect comes back as the owner's
    // words.
    #[test]
    fn an_answer_brings_back_the_matter_the_ask_held() {
        let mut db = setup();
        db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO architect_matters (project, kind, detail, what, body, at)
                 VALUES ('p', 'engineer_question', 'split', 'w', '{}', 1)",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        let ask = architect_ask(
            &mut db,
            json!({ "from": "architect", "project": "p", "work": 9, "cell": "A3", "kind": "ask", "asked": [],
                    "holds": [1] }),
        );
        db.write(Actor::Engine, |tx| {
            tx.execute("UPDATE architect_matters SET asked = ?1, resume_cell = 'H3'", [ask])?;
            Ok(())
        })
        .unwrap();
        let queued = |db: &Db| -> Vec<(i64, i64)> {
            let mut st = db
                .conn()
                .prepare("SELECT matter, answered FROM architect_queue")
                .unwrap();
            st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
                .unwrap()
                .map(|x| x.unwrap())
                .collect()
        };
        assert_eq!(queued(&db), Vec::<(i64, i64)>::new());
        answer(&mut db, ask, &with_reply("Split it in two.", json!({}))).unwrap();
        assert_eq!(queued(&db), [(1, 1)]);
        let open = architect_ask(
            &mut db,
            json!({ "from": "architect", "work": 9, "cell": "A4", "kind": "open", "choice": { "question": "q" } }),
        );
        answer(&mut db, open, &with_reply("Do it.", json!({}))).unwrap();
        let said: (String, String, String) = db
            .conn()
            .query_row(
                "SELECT project, kind, body ->> 'answer' FROM architect_matters WHERE id = 2",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(said, ("p".into(), "owner_words".into(), "Do it.".into()));
    }

    // The owner closes an open observation once, saying why.
    #[test]
    fn the_owner_closes_an_observation() {
        let mut db = setup();
        db.write(Actor::Architect, |tx| {
            tx.execute(
                "INSERT INTO observations (project, what, source) VALUES ('p', 'A slow parse.', 'A report.')",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        assert!(close_observation(&mut db, 1, " ").is_err());
        assert_eq!(
            close_observation(&mut db, 1, "Not worth it.").unwrap(),
            "observation 1 closed"
        );
        assert!(close_observation(&mut db, 1, "Again.").is_err());
        let closed: (String, String) = db
            .conn()
            .query_row(
                "SELECT closed, closed_why FROM observations_all WHERE id = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(closed, ("owner".into(), "Not worth it.".into()));
    }
}
