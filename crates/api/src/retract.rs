//! Taking back the owner's words that no agent has taken up yet. The page shows such words as not taken up yet and
//! offers to put them back in the box they were said from; the same judgment, made again in the write that deletes
//! them, decides whether they can be.
//!
//! The words are rows the owner's writes made: a message to a work's line (an interjection, as the owner's
//! conversations call it), an owner_words matter of an architect's queue, or both (the words said to the architect, or
//! to both, in a work's group, one message delivered to the line and to the architect as the matter). They can be
//! taken back while no agent has them, and an agent has them once:
//! - a round of the work took the message (`deliveries.taken_round`), or it is in the input of a round that has not
//!   ended (`rounds.input.engine.interjections`, which the round's message gave and which is recorded as taken only
//!   when the round ends or its first tool use asks for more);
//! - a matter was picked by a round of the architect's work, handled, withdrawn or held by a question to the owner.
//!
//! The architect's cell `N0` picks the matter that comes next in the engine's own step, after its round started: words
//! in the queue wait for it to end, so that a pick made from a queue that still held them is not left pointing at
//! deleted words. The engine's start of a round of an interjection's line checks, in the write that records the round,
//! that the interjections its message gave are still there.
//!
//! What the architect can read of words that no pick took (the new session's account of the one-to-one, what came
//! after the base) is not a reason to keep them: taken up means what the page has always meant by it.

use db::attachments::{self, Words};
use db::rusqlite::{Connection, OptionalExtension};
use db::{Actor, Db};
use serde_json::Value;

use crate::read::Error;
use crate::{ConversationEntry, ImageSent, Retract, TakeBack, TakenBack};

/// What may be done with the owner's words of one entry.
enum Verdict {
    /// No agent has them: the rows that hold them, and what goes back in the box.
    Free(Held),
    /// The architect is choosing what to take next, which may be these words: they wait for a moment.
    Choosing,
    /// An agent has them, or has taken them up: why they cannot be taken back.
    Given(String),
    /// Words that are not taken back at all: why.
    Kept(String),
}

/// The rows of words that can be taken back, and what goes back in the box.
struct Held {
    /// The interjection to a work's line; none for words said to the architect alone outside a work's group.
    interjection: Option<i64>,
    /// The architect's matter that holds the words, if one does.
    matter: Option<i64>,
    text: String,
    /// Whom the words were for, as the box names them.
    to: Vec<String>,
}

/// What an interjection row says of itself.
struct Said {
    speaker: String,
    informs: bool,
    delivered: Option<i64>,
    matter: Option<i64>,
    text: String,
    card: Option<i64>,
    graph: String,
    /// Whether a round of its work that has not ended was given it.
    given: bool,
}

/// What an architect's matter says of itself.
struct Queued {
    project: String,
    kind: String,
    detail: Option<String>,
    body: String,
    /// Whether it was handled, withdrawn or is held by a question to the owner.
    taken: bool,
}

/// Judges the words of an entry of a conversation, by its source and its number.
fn judge(conn: &Connection, source: &str, id: i64) -> db::rusqlite::Result<Verdict> {
    match source {
        "interjection" => interjection(conn, id),
        "words" => matter(conn, id, None),
        _ => Ok(Verdict::Kept(format!("words of the kind {source} are not taken back"))),
    }
}

fn interjection(conn: &Connection, id: i64) -> db::rusqlite::Result<Verdict> {
    let found = conn
        .query_row(
            "SELECT j.speaker, j.informs, j.taken_round, j.matter, j.text, j.design_card, w.graph,
                    EXISTS (SELECT 1 FROM rounds r, json_each(r.input, '$.engine.interjections') g
                            WHERE r.work = j.work AND r.ended_at IS NULL AND g.value = j.id)
             FROM line_sayings j JOIN works w ON w.id = j.work WHERE j.id = ?1",
            [id],
            |r| {
                Ok(Said {
                    speaker: r.get(0)?,
                    informs: r.get(1)?,
                    delivered: r.get(2)?,
                    matter: r.get(3)?,
                    text: r.get(4)?,
                    card: r.get(5)?,
                    graph: r.get(6)?,
                    given: r.get(7)?,
                })
            },
        )
        .optional()?;
    let Some(j) = found else {
        return Ok(Verdict::Kept("these words are not there any more".into()));
    };
    if j.speaker != "owner" || db::relays_an_answer(&j.text) {
        return Ok(Verdict::Kept(
            "these are not words the owner typed: an answer is recorded on its ask".into(),
        ));
    }
    if j.card.is_some() {
        return Ok(Verdict::Kept(
            "words asking about a decision card are not taken back".into(),
        ));
    }
    if j.informs && j.matter.is_none() {
        return Ok(Verdict::Kept("a word that needs no answer is not taken back".into()));
    }
    if j.delivered.is_some() || j.given {
        return Ok(Verdict::Given("an agent has these words already".into()));
    }
    let engineer = j.graph == "engineer";
    match j.matter {
        Some(m) => {
            let to = if j.informs || !engineer {
                vec!["architect".to_string()]
            } else {
                vec!["engineer".to_string(), "architect".to_string()]
            };
            matter(conn, m, Some((id, to)))
        }
        None => Ok(Verdict::Free(Held {
            interjection: Some(id),
            matter: None,
            text: j.text,
            to: vec![if engineer { "engineer" } else { "architect" }.to_string()],
        })),
    }
}

/// The architect's matter that holds the owner's words, alone or beside the interjection that tells the work's engineer
/// (`via`, with whom the words were for).
fn matter(conn: &Connection, id: i64, via: Option<(i64, Vec<String>)>) -> db::rusqlite::Result<Verdict> {
    let found = conn
        .query_row(
            "SELECT project, kind, detail, body, handled_round IS NOT NULL OR withdrawn IS NOT NULL OR asked IS NOT NULL
             FROM architect_matters WHERE id = ?1",
            [id],
            |r| {
                Ok(Queued {
                    project: r.get(0)?,
                    kind: r.get(1)?,
                    detail: r.get(2)?,
                    body: r.get(3)?,
                    taken: r.get(4)?,
                })
            },
        )
        .optional()?;
    let Some(m) = found else {
        return Ok(Verdict::Kept("these words are not there any more".into()));
    };
    let project = m.project;
    let body: Value = serde_json::from_str(&m.body).unwrap_or(Value::Null);
    if m.kind != "owner_words" || m.detail.as_deref() != Some("said") {
        return Ok(Verdict::Kept("these are not words the owner typed".into()));
    }
    if body["where"] == "projects" {
        return Ok(Verdict::Kept(
            "words said to several architects at once are not taken back".into(),
        ));
    }
    if m.taken {
        return Ok(Verdict::Given("the architect has taken these words up".into()));
    }
    // Picked by a round of an architect's work, in whichever work: it has read them.
    let picked: bool = conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM rounds p JOIN works w ON w.id = p.work
                        WHERE w.graph = 'architect' AND p.transition IS NOT NULL
                          AND ?1 IN (json_extract(p.output, '$.outcome.pick.matter'),
                                     json_extract(p.output, '$.queue.matter'), json_extract(p.output, '$.next.matter')))",
        [id],
        |r| r.get(0),
    )?;
    if picked {
        return Ok(Verdict::Given("the architect has taken these words up".into()));
    }
    let choosing: bool = conn.query_row(
        &format!(
            "SELECT EXISTS (SELECT 1 FROM rounds r JOIN works w ON w.id = r.work
                            WHERE w.graph = 'architect' AND w.project = ?1 AND r.ended_at IS NULL AND r.cell IN {})",
            db::cells_sql("architect", &["N0"])
        ),
        [&project],
        |r| r.get(0),
    )?;
    if choosing {
        return Ok(Verdict::Choosing);
    }
    let text = body["text"].as_str().unwrap_or_default().to_string();
    let (interjection, to) = match via {
        Some((j, to)) => (Some(j), to),
        None => (None, vec!["architect".to_string()]),
    };
    Ok(Verdict::Free(Held {
        interjection,
        matter: Some(id),
        text,
        to,
    }))
}

/// Sets, on each entry of the owner's words that the page shows as not taken up yet, whether they can be taken back:
/// by the judgment [`take_back`] makes again in its own write. Words said to the architect's work in hand and to its
/// policy setting at once are two interjections shown as one entry, and are not taken back.
pub(crate) fn states(conn: &Connection, entries: &mut [ConversationEntry]) -> Result<(), Error> {
    for e in entries.iter_mut() {
        if e.sender != "owner" || !e.pending {
            continue;
        }
        if e.source == "interjection" && e.recipient == "both" && e.matter.is_none() {
            continue;
        }
        e.retract = match judge(conn, &e.source, e.id)? {
            Verdict::Free(_) | Verdict::Choosing => Retract::Yes,
            Verdict::Given(_) => Retract::Given,
            Verdict::Kept(_) => Retract::No,
        };
    }
    Ok(())
}

/// Takes back the owner's words of an entry, as the owner: judges them again in the write that deletes them, and, only
/// if no agent has them, gives their text, images and whom they were for, and deletes the rows that held them, which
/// `changes` keeps. Refused otherwise, saying why, and nothing is deleted.
pub fn take_back(db: &mut Db, t: &TakeBack) -> Result<TakenBack, String> {
    db.write(Actor::Owner, |tx| {
        let held = match judge(tx, &t.source, t.id)? {
            Verdict::Free(h) => h,
            Verdict::Choosing => {
                return Err(db::Error::Abort(
                    "the architect is choosing what to take next: try again in a moment".into(),
                ));
            }
            Verdict::Given(why) | Verdict::Kept(why) => return Err(db::Error::Abort(why)),
        };
        let ids = match (held.interjection, held.matter) {
            (Some(j), _) => attachments::of_interjection(tx, j)?,
            (None, Some(m)) => attachments::of(tx, Words::Matter(m))?,
            (None, None) => Vec::new(),
        };
        let mut images = Vec::new();
        for id in ids {
            if let Some((media_type, bytes)) = attachments::bytes(tx, id)? {
                images.push(ImageSent {
                    media_type,
                    data: attachments::base64(&bytes),
                });
            }
        }
        if let Some(j) = held.interjection {
            tx.execute("DELETE FROM attachments WHERE message = ?1", [j])?;
            tx.execute("DELETE FROM deliveries WHERE message = ?1", [j])?;
            tx.execute("DELETE FROM messages WHERE id = ?1", [j])?;
        }
        if let Some(m) = held.matter {
            tx.execute("DELETE FROM attachments WHERE matter = ?1", [m])?;
            tx.execute("DELETE FROM architect_matters WHERE id = ?1", [m])?;
        }
        Ok(TakenBack {
            text: held.text,
            images,
            to: held.to,
        })
    })
    .map_err(|e| e.to_string())
}
