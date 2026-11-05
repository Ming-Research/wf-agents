//! What the owner decides, on the page and on the command line alike: an answer to an ask of the owner, a ruling on a
//! decision card, what the owner says to a work's line, pausing a work and lifting a work's pause, starting a work over,
//! the switch that keeps waiting sessions warm while every work is paused, holding a project's new starts and releasing
//! a hold on them, forwarding a fault of wf-agents to an architect, the setting that forwards each new one as it comes,
//! removing a dependency between items, and ending a talk with an architect. Each is written as the owner, so `changes`
//! records it as theirs, and the engine reads it as it goes on; a start over is an action of the outbox, which the
//! engine carries out and records as the owner's in the end it writes. What the owner says to the architect is
//! `db::say_to_architect`. The owner's words and answers may carry images, kept with them in the same write, and may
//! be images alone.

use db::attachments::{self, Image, Words};
use db::rusqlite::{Connection, OptionalExtension, params};
use db::{Actor, Db, Declared, Matter, queue_matter};
use serde_json::{Value, json};

use crate::{Answer, ImageSent, ProjectsSaid, Ruling, WorkSaid};

fn sql(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// The images the owner sent with words, as the page hands them in, taken as the database keeps them: each a PNG, JPEG,
/// GIF or WebP image given as base64, of at most 5 MB, at most 10 to a message; refused otherwise, saying why.
pub fn images(sent: &[ImageSent]) -> Result<Vec<Image>, String> {
    let mut out = Vec::new();
    for (i, s) in sent.iter().enumerate() {
        let bytes =
            attachments::from_base64(&s.data).ok_or_else(|| format!("image {} is not given as base64", i + 1))?;
        out.push(Image {
            media_type: s.media_type.clone(),
            bytes,
        });
    }
    attachments::check(&out)?;
    Ok(out)
}

/// Whether words say nothing: no text and no image.
fn says_nothing(text: &str, images: &[Image]) -> bool {
    text.trim().is_empty() && images.is_empty()
}

/// Says something to a work's engineer as the owner: an interjection, handed to the engineer's round that runs after
/// its next tool use, or at the top of its next round. The owner speaks to the engineer alone of a work's lines: its
/// helpers (the advisor, the review, the adversarial testing) take nothing from the owner. A work at its end (`E1`) is
/// told nothing more: its way on no longer changes. The images sent with the words are kept with them. Gives the
/// interjection, which keeps who the caller declared to speak (`speaker`).
pub fn interject(
    db: &mut Db,
    work: i64,
    line: &str,
    text: &str,
    images: &[Image],
    speaker: impl Into<Declared>,
) -> Result<i64, String> {
    interject_about(db, work, line, text, images, None, &speaker.into())
}

/// [`interject`], the words asking about a decision card of the work (`card`) when they do.
fn interject_about(
    db: &mut Db,
    work: i64,
    line: &str,
    text: &str,
    images: &[Image],
    card: Option<i64>,
    speaker: &Declared,
) -> Result<i64, String> {
    let text = text.trim();
    if says_nothing(text, images) {
        return Err("what is said to a work says something".into());
    }
    let relayed = speaker.relayed_in(text)?.map(|r| r.to_string());
    attachments::check(images)?;
    if line != "engineer" {
        return Err(format!(
            "the owner speaks to a work's engineer, not to its {line}: its helpers take nothing from the owner"
        ));
    }
    live(db, work)?;
    db.write(Actor::Owner, |tx| {
        tx.execute(
            "INSERT INTO interjections (work, line, speaker, declared, relayed, text, design_card)
             VALUES (?1, ?2, 'owner', ?3, ?4, ?5, ?6)",
            params![work, line, speaker.speaker.as_str(), relayed, text, card],
        )?;
        let id = tx.last_insert_rowid();
        attachments::attach(tx, Words::Interjection(id), images)?;
        Ok(id)
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
/// question named, the words answer that question of the engineer's, which the architect holds. The images sent with
/// the words go with them: an interjection's own, or the matter's that holds them, which the engineer told is given too.
/// With a decision card named, the words ask about that card, one of the work's that waits for the owner's ruling: to
/// the architect unless said otherwise, which is given the card with them, and to the engineer as an interjection
/// naming it; no ruling, the card waits on.
pub fn say_to_work(db: &mut Db, work: i64, said: &WorkSaid) -> Result<String, String> {
    let images = images(&said.images)?;
    let speaker = Declared::parse(&said.speaker, said.relayed.as_deref().unwrap_or_default())?;
    if let Some(m) = said.question {
        if said.design_card.is_some() {
            return Err("words answer an engineer's question or ask about a decision card, not both".into());
        }
        return answer_question(db, work, m, &said.text, &images, &speaker);
    }
    if let Some(card) = said.design_card {
        card_waits(db, work, card)?;
    }
    let card = said.design_card;
    let to = said
        .to
        .as_deref()
        .unwrap_or(if card.is_some() { "architect" } else { "engineer" });
    if to == "engineer" {
        let id = interject_about(db, work, to, &said.text, &images, card, &speaker)?;
        return Ok(format!("interjection {id} goes to the engineer of work {work}"));
    }
    if to != "architect" && to != "both" {
        return Err(format!(
            "what is said in a work's group is for its engineer, its architect or both, not for the {to}"
        ));
    }
    let text = said.text.trim();
    if says_nothing(text, &images) {
        return Err("what is said to a work says something".into());
    }
    live(db, work)?;
    let (item, project) = item_of(db, work)?;
    let mut body = speaker.record(json!({ "text": text, "where": "work", "work": work }), text)?;
    let what = match card {
        Some(c) => {
            body["design_card"] = json!(c);
            format!(
                "About decision card {c}: {}",
                attachments::owner_says(text, images.len())
            )
        }
        None => attachments::owner_says(text, images.len()),
    };
    let m = Matter {
        project: project.clone(),
        kind: "owner_words".into(),
        detail: Some("said".into()),
        what,
        body,
        item,
        ..Matter::default()
    };
    let (copy, informs) = match (to, text.is_empty()) {
        ("both", _) => (text.to_string(), 0),
        (_, true) => (TOLD_TO_ARCHITECT.to_string(), 1),
        (_, false) => (format!("{TOLD_TO_ARCHITECT}: {text}"), 1),
    };
    let id = db
        .write(Actor::Owner, |tx| {
            let matter = queue_matter(tx, &m)?;
            attachments::attach(tx, Words::Matter(matter), &images)?;
            tx.execute(
                "INSERT INTO interjections (work, line, speaker, text, informs, matter, design_card)
                 VALUES (?1, 'engineer', 'owner', ?2, ?3, ?4, ?5)",
                params![work, copy, informs, matter, card],
            )?;
            Ok(matter)
        })
        .map_err(sql)?;
    Ok(match to {
        "both" => format!("the engineer of work {work} and the architect of {project}, as matter {id}, have it"),
        _ => format!("the architect of {project} has it as matter {id}, and the engineer of work {work} is told"),
    })
}

/// That a decision card is the work's and waits for the owner's ruling, as a question about it asks of one.
fn card_waits(db: &Db, work: i64, card: i64) -> Result<(), String> {
    let found: Option<(i64, String)> = db
        .conn()
        .query_row("SELECT work, status FROM design_cards WHERE id = ?1", [card], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .optional()
        .map_err(sql)?;
    match found {
        None => Err(format!("no decision card {card}")),
        Some((w, _)) if w != work => Err(format!("decision card {card} is not of work {work}")),
        Some((_, status)) if status != "open" => Err(format!(
            "decision card {card} is {status}: a question is asked about a card that waits for your ruling"
        )),
        Some(_) => Ok(()),
    }
}

/// What the owner says in a group of the architects of projects the configuration connects (`projects`, the group's,
/// in order): to the architect named, or to every one; each comes to its queue as the owner's words naming the group,
/// the projects it was said to listed in each, so that the group shows the words once; each holds the images sent with
/// them too, as its architect is given them with the words.
pub fn say_to_projects(db: &mut Db, projects: &[String], said: &ProjectsSaid) -> Result<String, String> {
    let images = images(&said.images)?;
    let speaker = Declared::parse(&said.speaker, said.relayed.as_deref().unwrap_or_default())?;
    let text = said.text.trim();
    if says_nothing(text, &images) {
        return Err("what is said to the architects says something".into());
    }
    let to: Vec<String> = match said.to.as_deref() {
        Some(p) if projects.iter().any(|x| x == p) => vec![p.to_string()],
        Some(p) => return Err(format!("project {p} is not in the group {}", said.key)),
        None => projects.to_vec(),
    };
    let body = speaker.record(
        json!({ "text": text, "where": "projects", "group": said.key, "to": to }),
        text,
    )?;
    let ids = db
        .write(Actor::Owner, |tx| {
            let mut ids = Vec::new();
            for p in &to {
                let m = Matter {
                    project: p.clone(),
                    kind: "owner_words".into(),
                    detail: Some("said".into()),
                    what: attachments::owner_says(text, images.len()),
                    body: body.clone(),
                    ..Matter::default()
                };
                let id = queue_matter(tx, &m)?;
                attachments::attach(tx, Words::Matter(id), &images)?;
                ids.push(id);
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
/// A question the architect is handling now, or that is not open, is refused. The images sent with the answer are the
/// matter's. The reply keeps who the caller declared to speak (`speaker`), as the owner's words do.
fn answer_question(
    db: &mut Db,
    work: i64,
    matter: i64,
    text: &str,
    images: &[Image],
    speaker: &Declared,
) -> Result<String, String> {
    let text = text.trim();
    if says_nothing(text, images) {
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
               AND ?1 IN (json_extract(p.output, '$.outcome.pick.matter'), json_extract(p.output, '$.queue.matter'),
                          json_extract(p.output, '$.next.matter'))",
            [matter],
            |r| r.get(0),
        )
        .map_err(sql)?;
    if picked > 0 {
        return Err(format!(
            "the architect is answering matter {matter} now: say it to the architect instead"
        ));
    }
    let reply = speaker.record(
        json!({
            "by": "owner",
            "at": db::now(),
            "text": text,
            "why": "The owner answered the question in the work's group.",
        }),
        text,
    )?;
    db.write(Actor::Owner, |tx| {
        tx.execute(
            "UPDATE architect_matters SET reply = ?2, withdrawn = ?3 WHERE id = ?1",
            params![
                matter,
                reply.to_string(),
                format!("the owner answered it in the group of work {work}")
            ],
        )?;
        attachments::attach(tx, Words::Matter(matter), images)?;
        Ok(())
    })
    .map_err(sql)?;
    Ok(format!(
        "the question of work {work} is answered, and its architect is told"
    ))
}

/// Replies, as the owner, to a question a project's architect pinned in their one-to-one: the owner's words, said with
/// the question ticked, answer it, and its matter comes back to the architect's queue with them, first, to the cell
/// that asked. A question whose matter is gone (handled, or withdrawn as its item was cancelled or its work taken over)
/// is no longer pinned: the reply then comes to the architect's queue as the owner's words, with the question, and the
/// architect tells the owner. Refused for a question of no architect of the project, or one replied to already. The
/// images sent with the reply are kept with the answer, and go with it either way. `speaker` is who the caller declares
/// to reply, kept with the answer and with the words.
pub fn reply_to_question(
    db: &mut Db,
    project: &str,
    ask: i64,
    text: &str,
    images: &[Image],
    speaker: impl Into<Declared>,
) -> Result<String, String> {
    let speaker: Declared = speaker.into();
    let text = text.trim();
    if says_nothing(text, images) {
        return Err("a reply says something".into());
    }
    attachments::check(images)?;
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
    let answer = speaker.record(json!({ "by": "owner", "at": db::now(), "text": text }), text)?;
    // The words kept as the owner's own to the architect carry the same declaration.
    let said = speaker.record(
        json!({ "text": text, "where": "page", "question": body["question"].clone(), "owner_ask": ask,
                "gone": "the matter the question parked is closed" }),
        text,
    )?;
    db.write(Actor::Owner, |tx| {
        tx.execute(
            "UPDATE owner_asks SET answer = ?2 WHERE id = ?1",
            params![ask, answer.to_string()],
        )?;
        attachments::attach(tx, Words::Ask(ask), images)?;
        if parked.is_none() {
            let m = Matter {
                project: project.to_string(),
                kind: "owner_words".into(),
                detail: Some("answer".into()),
                what: replied(&format!("The owner replied to question {ask}"), text, images.len()),
                body: said,
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

/// What an answer of the owner's says of itself as a matter: who answered what, then the answer's first sentence, or
/// how many images it sent alone.
fn replied(lead: &str, text: &str, images: usize) -> String {
    match (text.trim().is_empty(), images) {
        (false, _) => format!("{lead}: {}", db::one_sentence(text)),
        (true, 1) => format!("{lead} with an image"),
        (true, n) => format!("{lead} with {n} images"),
    }
}

/// The matters an ask of an architect holds, by their numbers: those it was put about, which come back to their
/// architect's queue once it is answered.
fn holds_of(body: &Value) -> Vec<i64> {
    match &body["holds"] {
        Value::Array(xs) => xs.iter().filter_map(Value::as_i64).collect(),
        v => v.as_i64().into_iter().collect(),
    }
}

/// Answers an ask an architect put, by its kind: an answer to what the architect asked, which brings back the matters
/// the ask holds, each to the cell that asked about it, or, for an ask that held neither a matter nor a work of the
/// architect, comes to the architect's queue as the owner's words. An ask of the changes an earlier architect collected
/// for approval is closed: item changes are reviewed now. The images sent with the answer are kept with it.
fn architect_answer(db: &mut Db, ask: i64, body: &Value, a: &Answer, images: &[Image]) -> Result<String, String> {
    let at = db::now();
    let text = a.text.trim();
    let speaker = Declared::parse(&a.speaker, a.relayed.as_deref().unwrap_or_default())?;
    let kind = body["kind"].as_str().unwrap_or_default();
    match kind {
        "changes" => {
            return Err(format!(
                "ask {ask} put changes to the owner's approval, which the architect no longer asks: its item changes \
                 are reviewed, and the architect is told to go on without it"
            ));
        }
        _ => {
            if says_nothing(text, images) {
                return Err("an answer says something".into());
            }
            let answer = speaker.record(json!({ "by": "owner", "at": at, "text": text }), text)?;
            // An ask that held no work of the architect and no matter, as an earlier architect put open questions,
            // comes back to the queue as the owner's words.
            let open = kind == "open" && holds_of(body).is_empty();
            let asked = if kind == "open" {
                body["choice"].clone()
            } else {
                body["asked"].clone()
            };
            let said = speaker.record(json!({ "ask": asked, "answer": text, "owner_ask": ask }), text)?;
            db.write(Actor::Owner, |tx| {
                tx.execute(
                    "UPDATE owner_asks SET answer = ?2 WHERE id = ?1",
                    params![ask, answer.to_string()],
                )?;
                attachments::attach(tx, Words::Ask(ask), images)?;
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
                            what: replied(&format!("The owner answered ask {ask}"), text, images.len()),
                            body: said,
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
    let images = images(&a.images)?;
    if body["from"] == "architect" {
        return architect_answer(db, ask, &body, a, &images);
    }
    if says_nothing(&a.text, &images) {
        return Err("an answer says something".into());
    }
    let speaker = Declared::parse(&a.speaker, a.relayed.as_deref().unwrap_or_default())?;
    let mut answer = speaker.record(json!({ "by": "owner", "at": db::now(), "text": a.text }), &a.text)?;
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
    let relay = relayed_words(db.conn(), ask, &body, &a.text)?;
    db.write(Actor::Owner, |tx| {
        tx.execute(
            "UPDATE owner_asks SET answer = ?2 WHERE id = ?1",
            params![ask, answer.to_string()],
        )?;
        attachments::attach(tx, Words::Ask(ask), &images)?;
        if let Some((work, text)) = &relay {
            let declared = answer["speaker"]
                .as_str()
                .filter(|s| matches!(*s, "owner" | "delegate"));
            tx.execute(
                "INSERT INTO interjections (work, line, speaker, declared, text) VALUES (?1, 'engineer', 'owner', ?2, ?3)",
                params![work, declared, text],
            )?;
            tx.execute(
                "UPDATE owner_asks SET body = json_set(body, '$.relayed', ?2) WHERE id = ?1",
                params![ask, db::now()],
            )?;
        }
        Ok(())
    })
    .map_err(sql)?;
    Ok(format!("ask {ask} answered"))
}

/// The kinds of ask a work goes on past, and what the owner was asked in each: their answers reach the engineer at
/// once, unless they approve exactly.
const GONE_ON_PAST: [(&str, &str); 3] = [
    ("design_review", "the review of the design the work goes on by"),
    (
        "points",
        "the decision points and departures the research left, which the work goes on by as it recommended",
    ),
    ("change", "whether the change may merge"),
];

/// What an answer to an ask its work goes on past says to the engineer, and to which work: none for an answer of
/// exactly approved, for an ask of another kind, for a work that ended, or where the cell the work waits at takes the
/// answer in itself: the merge gate the answer of the change it asked, the review's cell on an older version of the
/// flow the answer of the review.
fn relayed_words(conn: &Connection, ask: i64, body: &Value, text: &str) -> Result<Option<(i64, String)>, String> {
    let Some((kind, what)) = GONE_ON_PAST.iter().find(|(k, _)| body["kind"] == *k) else {
        return Ok(None);
    };
    if text.trim() == "approved" {
        return Ok(None);
    }
    let Some(work) = body["work"].as_i64() else {
        return Ok(None);
    };
    let ended: bool = conn
        .query_row("SELECT ended IS NOT NULL FROM works WHERE id = ?1", [work], |r| {
            r.get(0)
        })
        .optional()
        .map_err(sql)?
        .unwrap_or(true);
    let held_at: Option<String> = conn
        .query_row(
            "SELECT cell FROM blockers WHERE ask = ?1 AND ended IS NULL",
            [ask],
            |r| r.get(0),
        )
        .optional()
        .map_err(sql)?;
    let taken_in = matches!(
        (*kind, held_at.as_deref()),
        ("change", Some(_)) | ("design_review", Some("D3"))
    );
    if ended || taken_in {
        return Ok(None);
    }
    Ok(Some((
        work,
        format!("Asked about {what}, the owner answered (ask {ask}): {}", text.trim()),
    )))
}

/// Rules on a decision card that is current: the owner confirms the option it recommends, chooses another of its
/// options, or disagrees saying why, with words of the owner's if any; the ruling is added to its rulings, and it is
/// ruled. A choice or a disagreement reaches the work's engineer as an interjection naming the card and its key; a
/// confirmation is a word to the engineer that needs no answer and turns nothing (`informs`). A card a newer version of
/// its key replaced, or one withdrawn, can no longer be ruled. The owner alone rules on a card.
pub fn rule(db: &mut Db, design_card: i64, r: &Ruling) -> Result<String, String> {
    let found: Option<(String, i64, String, String, Option<i64>)> = db
        .conn()
        .query_row(
            "SELECT status, work, key, body, replaced_by FROM design_cards WHERE id = ?1",
            [design_card],
            |x| Ok((x.get(0)?, x.get(1)?, x.get(2)?, x.get(3)?, x.get(4)?)),
        )
        .optional()
        .map_err(sql)?;
    let Some((status, work, key, body, replaced_by)) = found else {
        return Err(format!("no decision card {design_card}"));
    };
    match (status.as_str(), replaced_by) {
        ("replaced", Some(by)) => {
            return Err(format!(
                "decision card {design_card} was replaced by decision card {by}: rule on that one"
            ));
        }
        ("replaced", None) => {
            return Err(format!(
                "decision card {design_card} is replaced: it can no longer be ruled"
            ));
        }
        ("withdrawn", _) => {
            return Err(format!(
                "decision card {design_card} was withdrawn: it can no longer be ruled"
            ));
        }
        _ => {}
    }
    let body: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
    if !body.is_object() {
        return Err(format!(
            "design card {design_card} is of a flow version before decision cards: it can no longer be ruled"
        ));
    }
    let text = r.text.as_deref().map(str::trim).filter(|t| !t.is_empty());
    let option = r.option.as_deref().map(str::trim).filter(|o| !o.is_empty());
    let recommended = body["recommended"].as_str().unwrap_or_default();
    let names: Vec<&str> = body["options"]
        .as_array()
        .map(|os| os.iter().filter_map(|o| o["name"].as_str()).collect())
        .unwrap_or_default();
    let said = match (r.verdict.as_str(), option) {
        ("confirm", None) => None,
        ("confirm", Some(_)) => return Err("a confirmation takes no option: choose another option instead".into()),
        ("choose", None) => return Err("a choice names the option chosen".into()),
        ("choose", Some(o)) if o == recommended => {
            return Err(format!("{o:?} is the option the card recommends: confirm it"));
        }
        ("choose", Some(o)) if !names.contains(&o) => {
            return Err(format!("decision card {design_card} has no option {o:?}"));
        }
        ("choose", Some(o)) => Some(format!(
            "the owner chooses the option \"{o}\" instead of \"{recommended}\"{}",
            text.map(|t| format!(": {t}")).unwrap_or_default()
        )),
        ("disagree", Some(_)) => return Err("to choose another option, choose it; a disagreement says why".into()),
        ("disagree", None) => match text {
            Some(t) => Some(format!("the owner disagrees: {t}")),
            None => return Err("a disagreement says why".into()),
        },
        (v, _) => return Err(format!("the verdict is confirm, choose or disagree, not {v:?}")),
    };
    let mut ruling = json!({ "by": "owner", "at": db::now(), "verdict": r.verdict });
    if let Some(o) = option {
        ruling["option"] = json!(o);
    }
    if let Some(t) = text {
        ruling["text"] = json!(t);
    }
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
        match &said {
            Some(s) => {
                let text = format!("{} {design_card} ({key}): {s}", db::CARD_RULING);
                tx.execute(
                    "INSERT INTO interjections (work, line, speaker, text) VALUES (?1, 'engineer', 'owner', ?2)",
                    params![work, text],
                )?;
            }
            None => {
                // A confirmation is a word to the engineer, which needs no answer and turns nothing.
                let words = text.map(|t| format!(" The owner said: {t}")).unwrap_or_default();
                tx.execute(
                    "INSERT INTO interjections (work, line, speaker, text, informs) VALUES (?1, 'engineer', 'owner', ?2, 1)",
                    params![
                        work,
                        format!("The owner confirmed decision card {design_card} ({key}): nothing changes.{words}")
                    ],
                )?;
            }
        }
        Ok(())
    })
    .map_err(sql)?;
    Ok(format!("decision card {design_card} ruled"))
}

/// Pauses a work that goes on: a round of it under way ends first, and then it takes no step by itself, whatever the
/// global state, until the owner starts it again; letting every work go on does not lift it. Why is the owner's words,
/// or that the owner paused it. A work paused already stays as it is, but for a resume of it that still waits for a free
/// slot, which this later word withdraws. It reads and writes in one transaction, so that the loop's lifting of a pause
/// falls wholly before or wholly after it, and the answer says what holds.
pub fn pause(db: &mut Db, work: i64, why: Option<&str>) -> Result<String, String> {
    let why = why
        .map(str::trim)
        .filter(|w| !w.is_empty())
        .unwrap_or(db::PAUSED_BY_OWNER);
    db.write(Actor::Owner, |tx| {
        let found: Option<(Option<String>, Option<String>)> = tx
            .query_row("SELECT paused, ended FROM works WHERE id = ?1", [work], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .optional()?;
        match found {
            None => Err(db::Error::Abort(format!("no work {work}"))),
            Some((_, Some(_))) => Err(db::Error::Abort(format!("work {work} has ended"))),
            Some((Some(_), None)) => {
                // A resume that still waits for a slot is an earlier word, the owner's or the architect's; this is the later.
                if !db::resume_asked(tx)?.contains(&work) {
                    return Err(db::Error::Abort(format!("work {work} is paused already")));
                }
                db::withdraw_resume(tx, work)?;
                Ok(format!("work {work} stays paused: the resume asked is withdrawn"))
            }
            Some((None, None)) => {
                tx.execute(
                    "UPDATE works SET paused = ?2, paused_by = 'owner' WHERE id = ?1",
                    params![work, why],
                )?;
                Ok(format!("work {work} pauses"))
            }
        }
    })
    .map_err(sql)
}

/// Starts a work over, as the owner: the engine takes it over as it takes over a work on an older flow version, saying
/// the owner started it over. A round of it under way is cut off, what it hands in not taken, once its agent's process
/// has ended; a new work on its item, on the flow version the engine runs, takes over its branch, worktree and pull
/// request from the first cell and is told what exists, the owner's rulings and answers included; it is paused as the
/// owner had paused this one. Only a work on an item that goes on can be started over ([`db::cannot_start_over`]).
pub fn start_over(db: &mut Db, work: i64) -> Result<String, String> {
    db.write(Actor::Owner, |tx| db::start_over(tx, work, Actor::Owner, None))
        .map_err(sql)?;
    Ok(format!("work {work} is started over"))
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

/// Removes a dependency between two items, as the owner: `item` no longer depends on `on`, and a work on `item` that
/// waits for `on` goes on without it, since its wait reads the dependencies as they are. Nobody is told, as nobody is
/// when a dependency is made. A project's architect removes one the same way by a change of its batch of item changes,
/// `remove_dependency`, which the batch's review makes.
pub fn remove_dependency(db: &mut Db, item: i64, on: i64) -> Result<String, String> {
    let removed = db
        .write(Actor::Owner, |tx| {
            Ok(tx.execute(
                "DELETE FROM item_links WHERE item = ?1 AND depends_on = ?2",
                params![item, on],
            )?)
        })
        .map_err(sql)?;
    if removed == 0 {
        return Err(format!("item {item} does not depend on item {on}"));
    }
    Ok(format!("item {item} no longer depends on item {on}"))
}

/// Asks to lift a paused work's pause, as the owner: the engine lifts it once the work's machine has a slot free, as it
/// does for the owner's Start, and its cell then runs again. The engine closes a round the pause left open, with the
/// reason. A work the architect stopped is the owner's release of it, which the architect is told. It reads and writes
/// in one transaction, so that a request is only ever written under the pause it is for: one written after the loop
/// lifted that pause would wait on through the work's next step and lift the pause that comes after.
pub fn resume(db: &mut Db, work: i64) -> Result<String, String> {
    let stopped = db
        .write(Actor::Owner, |tx| {
            let paused: Option<(Option<String>, Option<String>)> = tx
                .query_row("SELECT paused, paused_by FROM works WHERE id = ?1", [work], |r| {
                    Ok((r.get(0)?, r.get(1)?))
                })
                .optional()?;
            let stopped = match paused {
                None => return Err(db::Error::Abort(format!("no work {work}"))),
                Some((None, _)) => return Err(db::Error::Abort(format!("work {work} is not paused"))),
                Some((p, by)) => db::stopped_by_architect(p.as_deref(), by.as_deref()),
            };
            db::ask_resume(tx, work, Actor::Owner)?;
            Ok(stopped)
        })
        .map_err(sql)?;
    Ok(if stopped {
        format!("work {work} goes on once its machine has a slot free; the architect, which had stopped it, is told")
    } else {
        format!("work {work} goes on once its machine has a slot free")
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

/// Holds a project's new starts, as the owner: the engine starts no new work on the project's items from its next
/// look on, while works already running go on to their merge and works waiting for a result go on after it. Why is the
/// owner's words and cannot be empty. The project is named or, when the configuration has one project only, left out;
/// one the configuration does not have is refused, so that no hold is written for nothing. A hold of the owner's
/// already there keeps its time and takes the new words. The page and the command line both write it through here.
pub fn hold_starts(db: &mut Db, config: &config::Config, project: Option<&str>, why: &str) -> Result<String, String> {
    let project = project_named(config, project)?;
    let why = why.trim();
    if why.is_empty() {
        return Err("holding new starts says why".into());
    }
    db.write(Actor::Owner, |tx| {
        db::hold_starts(tx, &project, db::HoldReason::Owner, Some(why))
    })
    .map_err(sql)?;
    Ok(format!(
        "new works of {project} do not start; works already running go on"
    ))
}

/// Releases a hold on a project's new starts, as the owner, who may release any of the three: their own (`owner`), the
/// architect's (`architect`) and the architect's to set the project straight (`set_straight`). Refused, saying so, when
/// the project has no hold of that reason, since nothing was released; the refusal lists the holds the project does
/// have, for the command line has no other view of them. Both the page and the command line release through here.
pub fn release_hold(
    db: &mut Db,
    config: &config::Config,
    project: Option<&str>,
    reason: &str,
) -> Result<String, String> {
    let project = project_named(config, project)?;
    let reason = db::HoldReason::parse(reason)?;
    // The holds that remain are read in the same write as the delete, so that the refusal names what is there now.
    let remaining = db
        .write(Actor::Owner, |tx| {
            if db::release_starts(tx, &project, reason)? {
                Ok(None)
            } else {
                db::holds_of(tx, &project).map(Some)
            }
        })
        .map_err(sql)?;
    if let Some(held) = remaining {
        let held = if held.is_empty() {
            "it holds none".to_string()
        } else {
            format!("it holds: {}", held.join(", "))
        };
        return Err(format!("{project} has no hold of the reason {}; {held}", reason.name()));
    }
    Ok(format!(
        "the hold of {} on {project} is released; its new works start again unless another hold remains",
        reason.name()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A decision card of two options, recommending the first.
    const CARD: &str = r#"{"key": "red-run", "question": "Stop at the first failure or run every test?",
        "options": [{"name": "A"}, {"name": "B"}], "recommended": "A"}"#;

    fn setup() -> Db {
        filled(Db::in_memory().unwrap())
    }

    /// A database given one item with one work at `A3`, whose round has an ask of the owner and a decision card.
    fn filled(mut db: Db) -> Db {
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
                "INSERT INTO design_cards (work, round, key, body, status) VALUES (1, 1, 'red-run', ?1, 'open')",
                [CARD],
            )?;
            Ok(())
        })
        .unwrap();
        db
    }

    /// An image the owner sends, as the page hands it in.
    fn png() -> ImageSent {
        ImageSent {
            media_type: "image/png".into(),
            data: db::attachments::base64(b"\x89PNG\r\n\x1a\n"),
        }
    }

    /// The ids of the images kept with words.
    fn kept(db: &Db, words: Words) -> usize {
        attachments::of(db.conn(), words).unwrap().len()
    }

    // The images the owner sends are kept with the words they came with, in the write that keeps them: an
    // interjection's own; the matter's that holds words said to the architect or to both, which the engineer is told
    // too; and an answer's, to an ask. Words may be images alone, and a matter of images alone says how many; what
    // the owner may not send is refused, and nothing is kept.
    #[test]
    fn the_owner_s_images_are_kept_with_the_words() {
        let mut db = setup();
        let said = |to: &str, text: &str, images: Vec<ImageSent>| WorkSaid {
            text: text.into(),
            speaker: "owner".into(),
            relayed: None,
            to: Some(to.into()),
            question: None,
            design_card: None,
            images,
        };
        say_to_work(&mut db, 1, &said("engineer", "", vec![png(), png()])).unwrap();
        assert_eq!(kept(&db, Words::Interjection(1)), 2);
        say_to_work(&mut db, 1, &said("both", "See.", vec![png()])).unwrap();
        let (matter, what): (i64, String) = db
            .conn()
            .query_row("SELECT id, what FROM architect_matters", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(what, "The owner says, with an image: See.");
        assert_eq!(kept(&db, Words::Matter(matter)), 1);
        assert_eq!(attachments::of_interjection(db.conn(), 2).unwrap().len(), 1);
        say_to_work(&mut db, 1, &said("architect", " ", vec![png(), png()])).unwrap();
        let what: String = db
            .conn()
            .query_row("SELECT what FROM architect_matters ORDER BY id DESC LIMIT 1", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(what, "The owner sends 2 images");
        assert!(say_to_work(&mut db, 1, &said("engineer", " ", Vec::new())).is_err());
        let pdf = ImageSent {
            media_type: "application/pdf".into(),
            data: db::attachments::base64(b"%PDF"),
        };
        assert!(say_to_work(&mut db, 1, &said("both", "No.", vec![pdf])).is_err());
        assert!(say_to_work(&mut db, 1, &said("engineer", "No.", vec![png(); 11])).is_err());
        let interjections: i64 = db
            .conn()
            .query_row("SELECT count(*) FROM interjections", [], |r| r.get(0))
            .unwrap();
        assert_eq!(interjections, 3);
        let a = Answer {
            text: String::new(),
            speaker: "owner".into(),
            relayed: None,
            why: None,
            reply: None,
            to: None,
            images: vec![png()],
        };
        answer(&mut db, 1, &a).unwrap();
        assert_eq!(kept(&db, Words::Ask(1)), 1);
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
            speaker: "owner".into(),
            relayed: None,
            to: None,
            question: Some(question),
            design_card: None,
            images: Vec::new(),
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
            (reply["by"].as_str(), reply["text"].as_str(), reply["speaker"].as_str()),
            (Some("owner"), Some("Do not split it."), Some("owner"))
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
        // The reply keeps who the caller declared: a delegate's answer is not told as the owner's.
        db.write(Actor::Engine, |tx| {
            // A work waits on one question at a time: the first one is over.
            tx.execute(
                "UPDATE blockers SET ended = 'resolved', outcome = '{}', ended_at = 6 WHERE matter = 1",
                [],
            )?;
            tx.execute(
                "INSERT INTO blockers (work, cell, round, kind, what, holder, project, matter, at, since)
                 VALUES (1, 'I1', 1, 'question', 'Split it again?', 'architect', 'p', 2, 6, 6)",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        let by_delegate = WorkSaid {
            speaker: "delegate".into(),
            ..said(2, "Split it.")
        };
        // A part the words do not hold is refused, and the question stays open.
        let wrong = WorkSaid {
            relayed: Some(vec!["Merge it.".into()]),
            ..by_delegate.clone()
        };
        assert!(
            say_to_work(&mut db, 1, &wrong)
                .unwrap_err()
                .contains("not found letter for letter")
        );
        let reply = "SELECT reply FROM architect_matters WHERE id = 2";
        let none: Option<String> = db.conn().query_row(reply, [], |r| r.get(0)).unwrap();
        assert_eq!(none, None);
        let relaying = WorkSaid {
            relayed: Some(vec!["Split".into()]),
            ..by_delegate
        };
        say_to_work(&mut db, 1, &relaying).unwrap();
        let kept = "SELECT json_extract(reply, '$.speaker'), json_extract(reply, '$.relayed')
                    FROM architect_matters WHERE id = 2";
        let (speaker, relayed): (String, String) =
            db.conn().query_row(kept, [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        assert_eq!((speaker.as_str(), relayed.as_str()), ("delegate", r#"["Split"]"#));
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
        assert_eq!(
            resume(&mut db, 1).unwrap(),
            "work 1 goes on once its machine has a slot free"
        );
        let paused = |db: &Db| -> (Option<String>, Option<String>) {
            db.conn()
                .query_row("SELECT paused, paused_by FROM works WHERE id = 1", [], |r| {
                    Ok((r.get(0)?, r.get(1)?))
                })
                .unwrap()
        };
        // Asked, and not lifted: the engine lifts it once the work's machine has a slot free, and the request goes then.
        assert_eq!(db::resume_asked(db.conn()).unwrap(), [1]);
        assert_eq!(
            paused(&db),
            (Some("cell tools: no".to_string()), Some("engine".to_string()))
        );
        db.write(Actor::Owner, |tx| db::lift_pause(tx, 1)).unwrap();
        assert_eq!(paused(&db), (None, None));
        assert_eq!(db::resume_asked(db.conn()).unwrap(), Vec::<i64>::new());
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
            "work 1 goes on once its machine has a slot free; the architect, which had stopped it, is told"
        );
        db.write(Actor::Owner, |tx| db::lift_pause(tx, 1)).unwrap();
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
        // A resume that still waits for a slot is withdrawn: the owner's Pause is the later word.
        db.write(Actor::Owner, |tx| db::ask_resume(tx, 1, Actor::Owner))
            .unwrap();
        assert_eq!(
            pause(&mut db, 1, None).unwrap(),
            "work 1 stays paused: the resume asked is withdrawn"
        );
        assert_eq!(db::resume_asked(db.conn()).unwrap(), Vec::<i64>::new());
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
        db.write(Actor::Owner, |tx| db::lift_pause(tx, 1)).unwrap();
        pause(&mut db, 1, Some("Wait for the release.")).unwrap();
        assert_eq!(paused(&db, 1).0.as_deref(), Some("Wait for the release."));
    }

    // The owner's Pause that meets the loop lifting the pause it would withdraw waits for the lifting to end, and then
    // pauses the work, which goes on by then. Reading before the lifting ended and writing after it, the answer was that
    // the work stays paused, while it went on.
    #[test]
    fn the_owner_s_pause_that_meets_the_lifting_of_a_pause_says_what_holds() {
        let dir = std::env::temp_dir().join(format!("wf-agents-pause-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("meet.db");
        let mut db = filled(Db::create(&path).unwrap());
        db.write(Actor::Owner, |tx| {
            tx.execute(
                "UPDATE works SET paused = 'cell tools: no', paused_by = 'engine' WHERE id = 1",
                [],
            )?;
            db::ask_resume(tx, 1, Actor::Owner)
        })
        .unwrap();
        let (lifting, begun) = std::sync::mpsc::channel();
        let the_loop = {
            let path = path.clone();
            std::thread::spawn(move || {
                let mut db = Db::open(&path).unwrap();
                db.write(Actor::Owner, |tx| {
                    db::lift_pause(tx, 1)?;
                    lifting.send(()).unwrap();
                    // Long enough for a Pause that reads apart from its write to read this pause as still there.
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    Ok(())
                })
                .unwrap();
            })
        };
        begun.recv().unwrap();
        assert_eq!(pause(&mut db, 1, None).unwrap(), "work 1 pauses");
        the_loop.join().unwrap();
        let paused: (Option<String>, Option<String>) = db
            .conn()
            .query_row("SELECT paused, paused_by FROM works WHERE id = 1", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(
            paused,
            (Some(db::PAUSED_BY_OWNER.to_string()), Some("owner".to_string()))
        );
        drop(db);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A configuration of the named projects, read from files in a directory of its own, which it removes.
    fn config_of(name: &str, projects: &[&str]) -> config::Config {
        let dir = std::env::temp_dir().join(format!("wf-agents-holds-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("projects")).unwrap();
        std::fs::create_dir_all(dir.join("rules")).unwrap();
        let install = "[owner]\nlanguage = \"English\"\n[items]\ntitle_max = 40\n[machines]\nworkstation = \"here\"\n\
                       heavy = \"builds\"\npoll_interval = 60\n[machines.hosts.here]\nslots = 1\n\
                       [agents.clis.claude]\nkind = \"claude\"\ncommand = \"claude\"\n[agents.levels.1]\n\
                       desc = \"all work\"\ncli = \"claude\"\nmodel = \"m\"\n[gh]\nallow = [\"pr view\"]\n\
                       [query]\nmax_rows = 10\ntimeout_ms = 1000\n";
        std::fs::write(dir.join("install.toml"), install).unwrap();
        let project = "repo = \"o/p\"\ncheckout = \"/tmp\"\ndesign_goal = \"simple\"\ntest_paths = [\"tests/\"]\n\
                       rules = [\"ground\"]\n[ci]\nfast = \"ci\"\nfull = [\"ci\"]\nmerge_checks = [\"check\"]\n\
                       [limits]\nfixes = 5\n";
        for p in projects {
            std::fs::write(dir.join(format!("projects/{p}.toml")), project).unwrap();
        }
        std::fs::write(dir.join("rules/ground.md"), "Facts first.\n").unwrap();
        let config = config::working(&dir).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        config
    }

    /// The projects and reasons of the holds in the database, in order.
    fn held(db: &Db) -> Vec<(String, String)> {
        let mut st = db
            .conn()
            .prepare("SELECT project, reason FROM dispatch_holds ORDER BY project, reason")
            .unwrap();
        st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(|x| x.unwrap())
            .collect()
    }

    // A release of a reason the project does not hold is refused, and the refusal lists the holds the project has now,
    // since the command line has no other view of them. It deletes nothing and leaves another project's holds out.
    #[test]
    fn a_refused_release_lists_the_holds_the_project_has() {
        let config = config_of("refused", &["p", "q"]);
        let mut db = Db::in_memory().unwrap();
        let refusal = |db: &mut Db| release_hold(db, &config, Some("p"), "owner").unwrap_err();
        assert_eq!(refusal(&mut db), "p has no hold of the reason owner; it holds none");
        db.write(Actor::Architect, |tx| {
            db::hold_starts(tx, "p", db::HoldReason::Architect, Some("Wrong order."))?;
            db::hold_starts(tx, "p", db::HoldReason::SetStraight, None)?;
            db::hold_starts(tx, "q", db::HoldReason::Architect, None)
        })
        .unwrap();
        assert_eq!(
            refusal(&mut db),
            "p has no hold of the reason owner; it holds: architect, set_straight"
        );
        assert_eq!(held(&db).len(), 3, "a refused release deletes nothing");
        hold_starts(&mut db, &config, Some("p"), "Leaving.").unwrap();
        release_hold(&mut db, &config, Some("p"), "owner").unwrap();
        assert_eq!(
            refusal(&mut db),
            "p has no hold of the reason owner; it holds: architect, set_straight"
        );
    }

    // The project may be left out of a hold or a release only where the configuration has one project; with two,
    // both are refused naming them and write nothing, and naming the project works.
    #[test]
    fn leaving_the_project_out_of_a_hold_or_a_release_works_only_with_one_project() {
        let two = config_of("two", &["p", "q"]);
        let mut db = Db::in_memory().unwrap();
        let err = hold_starts(&mut db, &two, None, "x").unwrap_err();
        assert!(err.contains("name the project, one of: p, q"), "{err}");
        assert!(held(&db).is_empty());
        hold_starts(&mut db, &two, Some("q"), "x").unwrap();
        let err = release_hold(&mut db, &two, None, "owner").unwrap_err();
        assert!(err.contains("name the project, one of: p, q"), "{err}");
        assert_eq!(
            held(&db),
            [("q".to_string(), "owner".to_string())],
            "a refused release deletes nothing"
        );
        release_hold(&mut db, &two, Some("q"), "owner").unwrap();
        assert!(held(&db).is_empty());
        let one = config_of("one", &["p"]);
        hold_starts(&mut db, &one, None, "x").unwrap();
        assert_eq!(held(&db), [("p".to_string(), "owner".to_string())]);
        release_hold(&mut db, &one, None, "owner").unwrap();
        assert!(held(&db).is_empty());
    }

    // The owner answers an ask once; a second answer is refused.
    #[test]
    fn the_owner_answers_an_ask_once() {
        let mut db = setup();
        let a = Answer {
            text: "Chosen option: B. Keep it small.".into(),
            speaker: "owner".into(),
            relayed: None,
            why: None,
            reply: None,
            to: None,
            images: Vec::new(),
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

    fn ruling(verdict: &str, option: Option<&str>, text: Option<&str>) -> Ruling {
        Ruling {
            verdict: verdict.into(),
            option: option.map(str::to_string),
            text: text.map(str::to_string),
        }
    }

    // A decision card is ruled on its own: a disagreement says why and a choice names another of its options, each
    // reaching the engineer as an interjection naming the card and its key; a confirmation reaches it as a word that
    // needs no answer.
    #[test]
    fn the_owner_rules_on_a_decision_card() {
        let mut db = setup();
        assert!(rule(&mut db, 1, &ruling("disagree", None, Some(" "))).is_err());
        assert!(
            rule(&mut db, 1, &ruling("choose", Some("A"), None))
                .unwrap_err()
                .contains("confirm it")
        );
        assert!(
            rule(&mut db, 1, &ruling("choose", Some("Z"), None))
                .unwrap_err()
                .contains("no option")
        );
        assert!(rule(&mut db, 1, &ruling("maybe", None, Some("x"))).is_err());
        let disagree = ruling("disagree", None, Some("Keep the old name."));
        assert_eq!(rule(&mut db, 1, &disagree).unwrap(), "decision card 1 ruled");
        let (status, rulings): (String, String) = db
            .conn()
            .query_row("SELECT status, rulings FROM design_cards WHERE id = 1", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        let rulings: Value = serde_json::from_str(&rulings).unwrap();
        assert_eq!(status, "ruled");
        assert_eq!(rulings[0]["text"], "Keep the old name.");
        rule(&mut db, 1, &ruling("choose", Some("B"), Some("One table is enough."))).unwrap();
        rule(&mut db, 1, &ruling("confirm", None, None)).unwrap();
        let said: Vec<(String, i64)> = db
            .conn()
            .prepare("SELECT text, informs FROM interjections ORDER BY id")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(
            said,
            [
                (
                    "Ruling on decision card 1 (red-run): the owner disagrees: Keep the old name.".to_string(),
                    0
                ),
                (
                    "Ruling on decision card 1 (red-run): the owner chooses the option \"B\" instead of \"A\": One \
                     table is enough."
                        .to_string(),
                    0
                ),
                (
                    "The owner confirmed decision card 1 (red-run): nothing changes.".to_string(),
                    1
                ),
            ]
        );
    }

    // The owner asks about a decision card that waits, on the card in the work's group: no ruling, the card waits on.
    // To the architect by default, the question is the owner's words naming the card, which the engineer is told; to
    // the engineer, an interjection naming it; either is the group's entry about the card. A card of another work, or
    // one ruled, is not asked about.
    #[test]
    fn the_owner_asks_about_a_decision_card() {
        let mut db = setup();
        let asks = |to: Option<&str>, text: &str, card: i64| WorkSaid {
            text: text.into(),
            speaker: "owner".into(),
            relayed: None,
            to: to.map(str::to_string),
            question: None,
            design_card: Some(card),
            images: Vec::new(),
        };
        assert!(say_to_work(&mut db, 1, &asks(None, "Why not one table?", 9)).is_err());
        say_to_work(&mut db, 1, &asks(None, "Why not one table?", 1)).unwrap();
        let (what, body): (String, String) = db
            .conn()
            .query_row("SELECT what, body FROM architect_matters", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(what, "About decision card 1: The owner says: Why not one table?");
        assert_eq!(serde_json::from_str::<Value>(&body).unwrap()["design_card"], 1);
        say_to_work(&mut db, 1, &asks(Some("engineer"), "Is the log kept?", 1)).unwrap();
        let said: Vec<(String, i64, Option<i64>)> = db
            .conn()
            .prepare("SELECT text, informs, design_card FROM interjections ORDER BY id")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(
            said,
            [
                (
                    "Said to the architect, for you to know: Why not one table?".to_string(),
                    1,
                    Some(1)
                ),
                ("Is the log kept?".to_string(), 0, Some(1)),
            ]
        );
        let cards: Vec<(String, Option<i64>)> = db
            .conn()
            .prepare("SELECT source, design_card FROM conversation_entries WHERE source = 'interjection' ORDER BY id")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(
            cards,
            [
                ("interjection".to_string(), Some(1)),
                ("interjection".to_string(), Some(1))
            ]
        );
        let status: String = db
            .conn()
            .query_row("SELECT status FROM design_cards WHERE id = 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(status, "open");
        rule(&mut db, 1, &ruling("confirm", None, None)).unwrap();
        assert!(say_to_work(&mut db, 1, &asks(None, "And now?", 1)).is_err());
    }

    // A card a newer version of its key replaced, one withdrawn, and a design card of before decision cards are not
    // ruled.
    #[test]
    fn a_card_replaced_or_withdrawn_is_not_ruled() {
        let mut db = setup();
        // Card 1 of setup is replaced by card 2, a newer version of its key.
        db.write(Actor::Engine, |tx| {
            tx.execute("UPDATE design_cards SET status = 'replaced' WHERE id = 1", [])?;
            tx.execute(
                "INSERT INTO design_cards (work, round, key, body, status) VALUES (1, 1, 'red-run', '{}', 'open')",
                [],
            )?;
            tx.execute("UPDATE design_cards SET replaced_by = 2 WHERE id = 1", [])?;
            tx.execute(
                "INSERT INTO design_cards (work, round, key, body, status, withdrawn)
                 VALUES (1, 1, 'gone', '{}', 'withdrawn', 'No longer asked.')",
                [],
            )?;
            tx.execute(
                "INSERT INTO design_cards (work, round, key, body, status) VALUES (1, 1, 'card-4', '[]', 'open')",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        let confirm = ruling("confirm", None, None);
        assert!(
            rule(&mut db, 1, &confirm)
                .unwrap_err()
                .contains("replaced by decision card 2")
        );
        assert!(rule(&mut db, 3, &confirm).unwrap_err().contains("withdrawn"));
        assert!(
            rule(&mut db, 4, &confirm)
                .unwrap_err()
                .contains("before decision cards")
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

    /// An answer in words, as the page sends it.
    fn with_reply(text: &str) -> Answer {
        Answer {
            text: text.into(),
            speaker: "owner".into(),
            relayed: None,
            why: None,
            reply: None,
            to: None,
            images: Vec::new(),
        }
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
        let e = answer(&mut db, ask, &with_reply("Yes.")).unwrap_err();
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
        answer(&mut db, ask, &with_reply("Split it in two.")).unwrap();
        assert_eq!(queued(&db), [(1, 1)]);
        let open = architect_ask(
            &mut db,
            json!({ "from": "architect", "work": 9, "cell": "A4", "kind": "open", "choice": { "question": "q" } }),
        );
        answer(&mut db, open, &with_reply("Do it.")).unwrap();
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

    // The owner removes a dependency once; the row goes, recorded as the owner's change.
    #[test]
    fn the_owner_removes_a_dependency() {
        let mut db = setup();
        db.write(Actor::Architect, |tx| {
            tx.execute(
                "INSERT INTO items (project, title, problem, why, deliver, level) VALUES ('p', 'u', 'x', 'y', 'change', '1')",
                [],
            )?;
            db::add_dependency(tx, 1, 2, "its parser", "parallel")
        })
        .unwrap();
        assert_eq!(
            remove_dependency(&mut db, 1, 2).unwrap(),
            "item 1 no longer depends on item 2"
        );
        assert!(remove_dependency(&mut db, 1, 2).is_err());
        let (links, actor): (i64, String) = db
            .conn()
            .query_row(
                "SELECT (SELECT count(*) FROM item_links), (SELECT actor FROM changes WHERE table_name = 'item_links'
                 AND new_row IS NULL)",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!((links, actor.as_str()), (0, "owner"));
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

    /// A database with a work of the project's architect (work 2) that put two asks: a question (ask 1) and an open
    /// question (ask 2), neither with a matter parked on it.
    fn with_asks_of_the_architect() -> Db {
        let mut db = setup();
        db.write(Actor::Engine, |tx| {
            tx.execute("INSERT INTO works (flow_version, graph, at, project) VALUES (1, 'architect', 'H8', 'p')", [])?;
            tx.execute(
                "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES (2, 1, 'H8', '{}', 'c', 1)",
                [],
            )?;
            tx.execute(
                "INSERT INTO owner_asks (work, round, body) VALUES
                 (2, 1, '{\"from\":\"architect\",\"project\":\"p\",\"kind\":\"question\",\"question\":\"Which?\"}'),
                 (2, 1, '{\"from\":\"architect\",\"project\":\"p\",\"kind\":\"open\",\"choice\":\"Which?\"}')",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        db
    }

    // Every way the owner's words and answers come in keeps who the caller declared to speak: the words said in a work's
    // group to the architect, those said to the architects of a group, the reply to the architect's question in the
    // one-to-one, an answer to an ask of an engineer and to an ask of the architect that comes back as the owner's
    // words. A request that declares none, or a name that is neither the owner nor a delegate, is refused, and nothing
    // is kept.
    #[test]
    fn every_way_in_keeps_who_was_declared_to_speak() {
        let mut db = with_asks_of_the_architect();
        let said = |speaker: &str| WorkSaid {
            text: "Keep it.".into(),
            speaker: speaker.into(),
            relayed: None,
            to: Some("architect".into()),
            question: None,
            design_card: None,
            images: Vec::new(),
        };
        let answer_by = |speaker: &str| Answer {
            text: "Yes.".into(),
            speaker: speaker.into(),
            relayed: None,
            why: None,
            reply: None,
            to: None,
            images: Vec::new(),
        };
        for bad in ["", "architect"] {
            assert!(say_to_work(&mut db, 1, &said(bad)).is_err());
            assert!(answer(&mut db, 1, &answer_by(bad)).is_err());
        }
        say_to_work(&mut db, 1, &said("delegate")).unwrap();
        // Said to the engineer alone, the words keep who was declared too.
        let to_engineer = |speaker: &str| WorkSaid {
            to: Some("engineer".into()),
            ..said(speaker)
        };
        say_to_work(&mut db, 1, &to_engineer("delegate")).unwrap();
        say_to_work(&mut db, 1, &to_engineer("owner")).unwrap();
        say_to_projects(
            &mut db,
            &["p".to_string()],
            &ProjectsSaid {
                key: "g".into(),
                text: "Both of you.".into(),
                speaker: "owner".into(),
                relayed: None,
                to: None,
                images: Vec::new(),
            },
        )
        .unwrap();
        // The first ask of the architect's is a question with no matter parked on it: its reply comes as the words.
        reply_to_question(&mut db, "p", 2, "Neither.", &[], db::Speaker::Delegate).unwrap();
        answer(&mut db, 1, &answer_by("delegate")).unwrap();
        answer(&mut db, 3, &answer_by("owner")).unwrap();
        let one = |sql: &str| -> Vec<Option<String>> {
            db.conn()
                .prepare(sql)
                .unwrap()
                .query_map([], |r| r.get(0))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap()
        };
        assert_eq!(
            one("SELECT json_extract(body, '$.speaker') FROM architect_matters ORDER BY id"),
            [
                Some("delegate".to_string()),
                Some("owner".to_string()),
                Some("delegate".to_string()),
                Some("owner".to_string())
            ]
        );
        assert_eq!(
            one("SELECT declared FROM interjections WHERE matter IS NULL ORDER BY id"),
            [Some("delegate".to_string()), Some("owner".to_string())]
        );
        assert_eq!(
            one("SELECT json_extract(answer, '$.speaker') FROM owner_asks ORDER BY id"),
            [
                Some("delegate".to_string()),
                Some("delegate".to_string()),
                Some("owner".to_string())
            ]
        );
    }

    // The parts a delegate declares to be the owner's own words are kept where the speaker is kept, on every way in: the
    // words said to the architect of a work and to the architects of a group, those said to the engineer alone, the reply
    // to the architect's question, and the answers to an ask. A part the words do not hold, or one the owner declares,
    // is refused and nothing is kept.
    #[test]
    fn the_parts_a_delegate_relays_are_kept_on_every_way_in() {
        let mut db = with_asks_of_the_architect();
        let relaying = |rs: &[&str]| Some(rs.iter().map(|r| r.to_string()).collect::<Vec<String>>());
        let said = |to: &str, text: &str, rs: &[&str]| WorkSaid {
            text: text.into(),
            speaker: "delegate".into(),
            relayed: relaying(rs),
            to: Some(to.into()),
            question: None,
            design_card: None,
            images: Vec::new(),
        };
        say_to_work(&mut db, 1, &said("architect", "Fine. Keep it.", &["Keep it."])).unwrap();
        say_to_work(&mut db, 1, &said("engineer", "Fine. Hold it.", &["Hold it."])).unwrap();
        let group = ProjectsSaid {
            key: "g".into(),
            text: "Both. Go.".into(),
            speaker: "delegate".into(),
            relayed: relaying(&["Go."]),
            to: None,
            images: Vec::new(),
        };
        say_to_projects(&mut db, &["p".to_string()], &group).unwrap();
        let stop = Declared::parse("delegate", &["Stop.".to_string()]).unwrap();
        reply_to_question(&mut db, "p", 2, "Neither. Stop.", &[], stop).unwrap();
        let answered = |text: &str, rs: &[&str]| Answer {
            speaker: "delegate".into(),
            relayed: relaying(rs),
            ..with_reply(text)
        };
        answer(&mut db, 1, &answered("Yes. Do it.", &["Do it."])).unwrap();
        answer(&mut db, 3, &answered("Yes. Done.", &["Done."])).unwrap();
        // What a column keeps, row by row, each row's text or `-` for none.
        let one = |db: &Db, sql: &str| -> String {
            let mut st = db.conn().prepare(sql).unwrap();
            let rows = st.query_map([], |r| r.get::<_, Option<String>>(0)).unwrap();
            rows.map(|x| x.unwrap().unwrap_or("-".into()))
                .collect::<Vec<String>>()
                .join(" ")
        };
        let matters = "SELECT json_extract(body, '$.relayed') FROM architect_matters ORDER BY id";
        let kept = one(&db, matters);
        assert_eq!(kept, r#"["Keep it."] ["Go."] ["Stop."] ["Done."]"#);
        let alone = "SELECT relayed FROM interjections WHERE matter IS NULL ORDER BY id";
        assert_eq!(one(&db, alone), r#"["Hold it."]"#);
        let asks = "SELECT json_extract(answer, '$.relayed') FROM owner_asks ORDER BY id";
        assert_eq!(one(&db, asks), r#"["Do it."] ["Stop."] ["Done."]"#);
        // A part the words do not hold, or one the owner declares, is refused wherever the words go, and nothing is kept.
        let wrong = said("architect", "Fine. Keep it.", &["Keep that."]);
        assert!(
            say_to_work(&mut db, 1, &wrong)
                .unwrap_err()
                .contains("not found letter for letter")
        );
        let wrong = said("engineer", "Fine. Hold it.", &["Hold that."]);
        assert!(
            say_to_work(&mut db, 1, &wrong)
                .unwrap_err()
                .contains("not found letter for letter")
        );
        let by_owner = WorkSaid {
            speaker: "owner".into(),
            ..said("architect", "Fine. Keep it.", &["Keep it."])
        };
        assert!(
            say_to_work(&mut db, 1, &by_owner)
                .unwrap_err()
                .contains("only a delegate relays")
        );
        let blank = WorkSaid {
            relayed: relaying(&[" "]),
            ..said("engineer", "Fine. Hold it.", &[])
        };
        assert!(say_to_work(&mut db, 1, &blank).unwrap_err().contains("blank"));
        // Nothing was kept by the refusals.
        assert_eq!(one(&db, matters), kept);
        assert_eq!(one(&db, alone), r#"["Hold it."]"#);
    }
}
