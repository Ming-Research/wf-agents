//! The architects' matters, as `architect_matters` keeps them: what comes to each project's architect's queue from
//! engineers' questions, side reports, design cards, the owner's words and answers, other projects' architects, the
//! architect's own rounds and the engine. Each is queued in the transaction of what raised it, by the engine and by
//! the owner's operations alike.

use rusqlite::{OptionalExtension, Transaction, params};
use serde_json::Value;

use crate::{Actor, Db, Result, STOPPED_BY_ARCHITECT, now};

/// A matter to queue for a project's architect.
#[derive(Debug, Clone, Default)]
pub struct Matter {
    /// The project whose architect it is for.
    pub project: String,
    /// Its kind, which decides its path: set_straight, engineer_question, report, design_card, failure, owner_words,
    /// other_architect or slots_free.
    pub kind: String,
    /// What more its kind says, such as an engineer's question's own kind; none when nothing more.
    pub detail: Option<String>,
    /// What it is, in one sentence.
    pub what: String,
    /// Its material, JSON.
    pub body: Value,
    /// The item it concerns, if any.
    pub item: Option<i64>,
    /// The round that raised it, by its work and its number; none for one the owner or the engine raised.
    pub raised: Option<(i64, i64)>,
}

/// Queues a matter for a project's architect, and gives its id. A matter in the queue wakes the project's architect
/// when no wake of it runs.
pub fn queue_matter(tx: &Transaction<'_>, m: &Matter) -> Result<i64> {
    tx.execute(
        "INSERT INTO architect_matters (project, kind, detail, what, body, item, work, round, at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            m.project,
            m.kind,
            m.detail,
            m.what,
            m.body.to_string(),
            m.item,
            m.raised.map(|r| r.0),
            m.raised.map(|r| r.1),
            now()
        ],
    )?;
    Ok(tx.last_insert_rowid())
}

/// The project of an item, which its architect's matters go to; none for an item there is not.
pub fn project_of_item(tx: &Transaction<'_>, item: i64) -> Result<Option<String>> {
    Ok(tx
        .query_row("SELECT project FROM items WHERE id = ?1", [item], |r| r.get(0))
        .optional()?)
}

/// Lifts a work's pause, in a write made as the owner, as the owner's Start of it does: a work the architect stopped is
/// then the owner's release of it, which the architect of its project is told as the owner's words. Gives whether the
/// architect had stopped it.
pub fn lift_pause(tx: &Transaction<'_>, work: i64) -> Result<bool> {
    let (paused, by, item): (Option<String>, Option<String>, Option<i64>) =
        tx.query_row("SELECT paused, paused_by, item FROM works WHERE id = ?1", [work], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })?;
    tx.execute("UPDATE works SET paused = NULL, paused_by = NULL WHERE id = ?1", [work])?;
    let stopped = (by.as_deref() == Some("engine"))
        .then(|| paused.as_deref()?.strip_prefix(STOPPED_BY_ARCHITECT))
        .flatten();
    let Some(why) = stopped else {
        return Ok(false);
    };
    let why = why.trim_start_matches(':').trim().trim_end_matches('.');
    let text = format!("I started work {work}, which you had stopped ({why}): it goes on.");
    let Some(project) = item.map(|i| project_of_item(tx, i)).transpose()?.flatten() else {
        return Ok(true);
    };
    let m = Matter {
        project,
        kind: "owner_words".into(),
        detail: Some("release".into()),
        what: format!("The owner says: {}", one_sentence(&text)),
        body: serde_json::json!({ "text": text, "work": work }),
        item,
        ..Matter::default()
    };
    queue_matter(tx, &m)?;
    Ok(true)
}

/// Says something to a project's architect, as the owner: the words come to its queue as a matter of the kind
/// owner_words, which wakes it. Where it was said goes with it: in the conversation about a work, the work and its item;
/// on an item's page, the item.
pub fn say_to_architect(
    db: &mut Db,
    project: &str,
    item: Option<i64>,
    work: Option<i64>,
    text: &str,
) -> std::result::Result<String, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("what is said to the architect says something".into());
    }
    if project.trim().is_empty() {
        return Err("what is said to an architect names its project".into());
    }
    let m = Matter {
        project: project.to_string(),
        kind: "owner_words".into(),
        detail: Some("said".into()),
        what: format!("The owner says: {}", one_sentence(text)),
        body: match (work, item) {
            (Some(w), _) => serde_json::json!({ "text": text, "where": "work", "work": w }),
            (None, Some(_)) => serde_json::json!({ "text": text, "where": "item" }),
            (None, None) => serde_json::json!({ "text": text, "where": "page" }),
        },
        item,
        ..Matter::default()
    };
    let id = db
        .write(Actor::Owner, |tx| queue_matter(tx, &m))
        .map_err(|e| e.to_string())?;
    Ok(format!("the architect of {project} has it as matter {id}"))
}

/// The first sentence of a text, at most 200 characters: up to its first full stop, question or exclamation mark, or
/// line break.
pub fn one_sentence(text: &str) -> String {
    let text = text.trim();
    let mut out = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\n' {
            break;
        }
        out.push(c);
        // The full stop, exclamation and question marks of Chinese and Japanese text end a sentence on their own.
        let ends = matches!(c, '\u{3002}' | '\u{ff01}' | '\u{ff1f}')
            || (matches!(c, '.' | '!' | '?') && chars.peek().is_none_or(|n| n.is_whitespace()));
        if ends {
            break;
        }
    }
    if out.chars().count() > 200 {
        out = out.chars().take(199).collect::<String>() + "…";
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sentence_ends_at_its_mark_or_line() {
        assert_eq!(one_sentence("a\u{3002}b"), "a\u{3002}");
        assert_eq!(one_sentence("Run it. Then paste"), "Run it.");
        assert_eq!(one_sentence("v1.2 is out\nmore"), "v1.2 is out");
    }

    // What the owner says to a project's architect comes to its queue, and something empty is refused.
    #[test]
    fn the_owner_s_words_come_to_the_architect_s_queue() {
        let mut db = Db::in_memory().unwrap();
        assert!(say_to_architect(&mut db, "p", None, None, "  ").is_err());
        assert!(say_to_architect(&mut db, "", None, None, "Go on.").is_err());
        assert_eq!(
            say_to_architect(&mut db, "p", None, None, "Put the parser first. It blocks the rest.").unwrap(),
            "the architect of p has it as matter 1"
        );
        let (project, kind, what, body): (String, String, String, String) = db
            .conn()
            .query_row(
                "SELECT q.project, m.kind, m.what, m.body FROM architect_queue q
                 JOIN architect_matters m ON m.id = q.matter",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        assert_eq!((project.as_str(), kind.as_str()), ("p", "owner_words"));
        assert_eq!(what, "The owner says: Put the parser first.");
        assert_eq!(
            body,
            r#"{"text":"Put the parser first. It blocks the rest.","where":"page"}"#
        );
    }
}
