//! The architects' matters, as `architect_matters` keeps them: what comes to each project's architect's queue from
//! engineers' questions, side reports, design cards, the owner's words and answers, other projects' architects, the
//! architect's own rounds, the faults of wf-agents forwarded to it, and the engine. Each is queued in the transaction
//! of what raised it, by the engine and by the owner's operations alike.

use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde_json::Value;

use crate::{Actor, Db, Error, Result, STOPPED_BY_ARCHITECT, now};

/// A matter to queue for a project's architect.
#[derive(Debug, Clone, Default)]
pub struct Matter {
    /// The project whose architect it is for.
    pub project: String,
    /// Its kind, which decides its path: set_straight, engineer_question, report, design_card, failure, owner_words,
    /// other_architect, slots_free or set_policy.
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

/// Queues a matter for a project's architect, and gives its id. A matter in the queue starts a work of the project's
/// architect when none runs, but for flow feedback, which waits for a work something else started.
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
/// owner_words, which starts a work of it. Where it was said goes with it: in the conversation about a work, the work
/// and its item; on an item's page, the item.
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

/// The detail of a report that holds a fault of wf-agents forwarded to a project's architect.
pub const FAULT: &str = "fault";

/// The owner's setting that forwards each new fault of wf-agents to a project's architect as it comes, as
/// `owner_choices` names it.
const FORWARD_FAULTS: &str = "forward_faults";

/// Who forwards a fault of wf-agents to an architect: the owner, by hand, or the owner's setting, which forwards each
/// new fault as it comes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Forwarder {
    Owner,
    Setting,
}

impl Forwarder {
    /// As the matter's material names it (`by`).
    pub fn as_str(self) -> &'static str {
        match self {
            Forwarder::Owner => "owner",
            Forwarder::Setting => "setting",
        }
    }
}

/// Forwards a fault of wf-agents, the engine's notice `notice`, to a project's architect: a matter of the kind report
/// with the detail fault, which names the notice and comes to the architect's queue as a report does, its material
/// gathered from the notice when it is handled; the notice is then read. A fault goes to each project's architect
/// once: a notice that reports no fault, or that went to that architect already, is refused. Gives the matter.
pub fn forward_fault(tx: &Transaction<'_>, notice: i64, project: &str, by: Forwarder) -> Result<i64> {
    let found: Option<(Option<String>, String)> = tx
        .query_row("SELECT fault, text FROM engine_notices WHERE id = ?1", [notice], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .optional()?;
    let Some((fault, text)) = found else {
        return Err(Error::Abort(format!("there is no notice {notice}")));
    };
    if fault.is_none() {
        return Err(Error::Abort(format!("notice {notice} reports no fault of wf-agents")));
    }
    let project = project.trim();
    if project.is_empty() {
        return Err(Error::Abort(
            "a fault is forwarded to a project's architect, named".into(),
        ));
    }
    let went: bool = tx.query_row(
        "SELECT EXISTS (SELECT 1 FROM architect_matters WHERE notice = ?1 AND project = ?2)",
        params![notice, project],
        |r| r.get(0),
    )?;
    if went {
        return Err(Error::Abort(format!(
            "fault {notice} went to the architect of {project} already"
        )));
    }
    let who = match by {
        Forwarder::Owner => "The owner forwards",
        Forwarder::Setting => "The owner's setting forwards",
    };
    tx.execute(
        "INSERT INTO architect_matters (project, kind, detail, what, body, notice, at)
         VALUES (?1, 'report', ?2, ?3, ?4, ?5, ?6)",
        params![
            project,
            FAULT,
            format!("{who}: {}", one_sentence(&text)),
            serde_json::json!({ "by": by.as_str() }).to_string(),
            notice,
            now()
        ],
    )?;
    let id = tx.last_insert_rowid();
    tx.execute(
        "UPDATE engine_notices SET read_at = ?2 WHERE id = ?1 AND read_at IS NULL",
        params![notice, now()],
    )?;
    Ok(id)
}

/// The project whose architect the owner's setting forwards each new fault of wf-agents to as it comes; none for
/// nobody, as it is at first.
pub fn fault_forwarding(conn: &Connection) -> Result<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT value FROM owner_choices WHERE name = ?1",
            [FORWARD_FAULTS],
            |r| r.get(0),
        )
        .optional()?)
}

/// Sets the project whose architect each new fault of wf-agents is forwarded to as it comes; none puts it back to
/// nobody. Made in a write as the owner, `changes` keeps who set it, and when.
pub fn set_fault_forwarding(tx: &Transaction<'_>, project: Option<&str>) -> Result<()> {
    match project.map(str::trim) {
        Some("") => Err(Error::Abort(
            "new faults are forwarded to a project's architect, named".into(),
        )),
        Some(p) => {
            tx.execute(
                "INSERT INTO owner_choices (name, value) VALUES (?1, ?2)
                 ON CONFLICT (name) DO UPDATE SET value = excluded.value WHERE value IS NOT excluded.value",
                params![FORWARD_FAULTS, p],
            )?;
            Ok(())
        }
        None => {
            tx.execute("DELETE FROM owner_choices WHERE name = ?1", [FORWARD_FAULTS])?;
            Ok(())
        }
    }
}

/// Forwards a new fault of wf-agents, the notice `notice`, as the owner's setting says: to the architect of the
/// project it names, or, set to nobody, nowhere. Gives the matter.
pub fn forward_new_fault(tx: &Transaction<'_>, notice: i64) -> Result<Option<i64>> {
    match fault_forwarding(tx)? {
        Some(project) => forward_fault(tx, notice, &project, Forwarder::Setting).map(Some),
        None => Ok(None),
    }
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
