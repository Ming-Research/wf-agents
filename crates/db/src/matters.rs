//! The architect's matters, as `architect_matters` keeps them: what comes to the architect's queue from engineers'
//! questions, side reports, design cards, the owner's words, answers and approvals, the architect's own rounds and the
//! engine. Each is queued in the transaction of what raised it, by the engine and by the owner's operations alike.

use rusqlite::{Transaction, params};
use serde_json::Value;

use crate::{Actor, Db, Result, STOPPED_BY_ARCHITECT, now};

/// A matter to queue for the architect.
#[derive(Debug, Clone, Default)]
pub struct Matter {
    /// One of the flow's `arch_about` values.
    pub kind: String,
    /// What it is, in one sentence.
    pub what: String,
    /// Its material, JSON.
    pub body: Value,
    /// The item it concerns, if any.
    pub item: Option<i64>,
    /// The round that raised it, by its work and its number; none for one the owner or the engine raised.
    pub raised: Option<(i64, i64)>,
    /// When it comes due; none for a matter due at once.
    pub due: Option<i64>,
    /// The item whose work ending makes it due, for a next look set on an item.
    pub due_item: Option<i64>,
    /// The held matter an owner's answer is for.
    pub answers: Option<i64>,
}

/// Queues a matter for the architect, and gives its id. A matter due wakes the architect when no wake runs.
pub fn queue_matter(tx: &Transaction<'_>, m: &Matter) -> Result<i64> {
    tx.execute(
        "INSERT INTO architect_matters (kind, what, body, item, work, round, at, due, due_item, answers)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            m.kind,
            m.what,
            m.body.to_string(),
            m.item,
            m.raised.map(|r| r.0),
            m.raised.map(|r| r.1),
            now(),
            m.due,
            m.due_item,
            m.answers
        ],
    )?;
    Ok(tx.last_insert_rowid())
}

/// Lifts a work's pause, in a write made as the owner, as the owner's Start of it does: a work the architect stopped is
/// then the owner's release of it, which the architect is told as the owner's words. Gives whether the architect had
/// stopped it.
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
    let why = why.trim_start_matches(':').trim();
    let text = format!("I started work {work}, which you had stopped ({why}): it goes on.");
    let m = Matter {
        kind: "owner_words".into(),
        what: format!("The owner says: {}", one_sentence(&text)),
        body: serde_json::json!({ "text": text }),
        item,
        ..Matter::default()
    };
    queue_matter(tx, &m)?;
    Ok(true)
}

/// Says something to the architect, as the owner: the words come to its queue as a matter of the kind owner_words,
/// which wakes it.
pub fn say_to_architect(db: &mut Db, text: &str) -> std::result::Result<String, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("what is said to the architect says something".into());
    }
    let m = Matter {
        kind: "owner_words".into(),
        what: format!("The owner says: {}", one_sentence(text)),
        body: serde_json::json!({ "text": text }),
        ..Matter::default()
    };
    let id = db
        .write(Actor::Owner, |tx| queue_matter(tx, &m))
        .map_err(|e| e.to_string())?;
    Ok(format!("the architect has it as matter {id}"))
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

    // What the owner says to the architect comes to its queue, due at once, and something empty is refused.
    #[test]
    fn the_owner_s_words_come_to_the_architect_s_queue() {
        let mut db = Db::in_memory().unwrap();
        assert!(say_to_architect(&mut db, "  ").is_err());
        assert_eq!(
            say_to_architect(&mut db, "Put the parser first. It blocks the rest.").unwrap(),
            "the architect has it as matter 1"
        );
        let (kind, what, body): (String, String, String) = db
            .conn()
            .query_row(
                "SELECT m.kind, m.what, m.body FROM architect_queue q JOIN architect_matters m ON m.id = q.matter",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(kind, "owner_words");
        assert_eq!(what, "The owner says: Put the parser first.");
        assert_eq!(body, r#"{"text":"Put the parser first. It blocks the rest."}"#);
    }
}
