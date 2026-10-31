//! The architects' matters, as `architect_matters` keeps them: what comes to each project's architect's queue from
//! engineers' questions, side reports, design cards, the owner's words and answers, other projects' architects, the
//! architect's own rounds, the faults of wf-agents forwarded to it, and the engine. Each is queued in the transaction
//! of what raised it, by the engine and by the owner's operations alike.

use std::sync::LazyLock;

use regex::Regex;
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde_json::Value;
use unicode_segmentation::UnicodeSegmentation;

use crate::attachments::{self, Image, Words};
use crate::{Actor, Db, Error, Result, STOPPED_BY_ARCHITECT, now};

/// A matter to queue for a project's architect.
#[derive(Debug, Clone, Default)]
pub struct Matter {
    /// The project whose architect it is for.
    pub project: String,
    /// Its kind, which decides its path: set_straight, engineer_question, report, design_card, failure, owner_words,
    /// other_architect, slots_free, or check_policy and make_plan, which come at the head of the queue.
    pub kind: String,
    /// What more its kind says, such as a side report's kind; none when nothing more, as for an engineer's question.
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

/// Withdraws from the architect's queue the look at each design card of a work that the owner ruled or another
/// replaced, saying why, as a matter whose reason went: the owner rules on a design card, and the architect's look at
/// one is for the owner to have before ruling. Called wherever a card is ruled or replaced, in that write.
pub fn withdraw_card_looks(tx: &Transaction<'_>, work: i64) -> Result<()> {
    tx.execute(
        "UPDATE architect_matters SET withdrawn = (
           SELECT CASE WHEN c.status = 'ruled' THEN 'the owner ruled design card ' || c.id
                       WHEN c.replaced_by IS NOT NULL THEN 'design card ' || c.id || ' was replaced by design card '
                         || c.replaced_by
                       ELSE 'design card ' || c.id || ' was replaced' END
           FROM design_cards c WHERE c.id = json_extract(architect_matters.body, '$.design_card'))
         WHERE kind = 'design_card' AND handled_round IS NULL AND withdrawn IS NULL
           AND json_extract(body, '$.design_card') IN (
             SELECT id FROM design_cards WHERE work = ?1 AND status IN ('ruled', 'replaced'))",
        [work],
    )?;
    Ok(())
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
    let why = without_final_terminal(why.trim_start_matches(':').trim());
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

/// Says something to a project's architect, as the owner: the words, and the images sent with them, come to its queue
/// as a matter of the kind owner_words, which starts a work of it. Where it was said goes with it: in the conversation
/// about a work, the work and its item; on an item's page, the item.
pub fn say_to_architect(
    db: &mut Db,
    project: &str,
    item: Option<i64>,
    work: Option<i64>,
    text: &str,
    images: &[Image],
) -> std::result::Result<String, String> {
    let text = text.trim();
    if text.is_empty() && images.is_empty() {
        return Err("what is said to the architect says something".into());
    }
    if project.trim().is_empty() {
        return Err("what is said to an architect names its project".into());
    }
    attachments::check(images)?;
    let m = Matter {
        project: project.to_string(),
        kind: "owner_words".into(),
        detail: Some("said".into()),
        what: attachments::owner_says(text, images.len()),
        body: match (work, item) {
            (Some(w), _) => serde_json::json!({ "text": text, "where": "work", "work": w }),
            (None, Some(_)) => serde_json::json!({ "text": text, "where": "item" }),
            (None, None) => serde_json::json!({ "text": text, "where": "page" }),
        },
        item,
        ..Matter::default()
    };
    let id = db
        .write(Actor::Owner, |tx| {
            let id = queue_matter(tx, &m)?;
            attachments::attach(tx, Words::Matter(id), images)?;
            Ok(id)
        })
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

/// The first sentence of a text, at most 200 characters: where Unicode's rules of sentences (UAX #29) end it, in any
/// language, at a line break at the latest.
pub fn one_sentence(text: &str) -> String {
    let mut out = text
        .trim()
        .split_sentence_bounds()
        .next()
        .unwrap_or_default()
        .trim()
        .to_string();
    if out.chars().count() > 200 {
        out = out.chars().take(199).collect::<String>() + "…";
    }
    out
}

/// A text without the sentence terminals it ends with, in any language: the characters Unicode gives the property
/// `Sentence_Terminal`.
pub fn without_final_terminal(text: &str) -> &str {
    static FINAL: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\p{Sentence_Terminal}+$").expect("the pattern is valid"));
    FINAL.find(text).map_or(text, |m| &text[..m.start()])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sentence_ends_at_its_mark_or_line() {
        assert_eq!(one_sentence("Run it. Then paste"), "Run it.");
        assert_eq!(one_sentence("Is it red? Then fix it"), "Is it red?");
        assert_eq!(one_sentence("v1.2 is out\nmore"), "v1.2 is out");
        // A long sentence is cut by characters, never inside one: an emoji takes four bytes.
        let cut = one_sentence(&"\u{1f680}".repeat(250));
        assert_eq!(cut.chars().count(), 200);
        assert!(cut.ends_with("\u{1f680}\u{2026}"));
    }

    // A final sentence terminal is trimmed whichever Unicode names so, the interrobang too; a closing bracket or a sign
    // that ends no sentence stays.
    #[test]
    fn a_final_sentence_terminal_is_trimmed_in_any_language() {
        assert_eq!(without_final_terminal("The tests were wrong."), "The tests were wrong");
        assert_eq!(without_final_terminal("Why?!"), "Why");
        assert_eq!(without_final_terminal("Done\u{203d}"), "Done");
        assert_eq!(without_final_terminal("v1.2 is out"), "v1.2 is out");
        assert_eq!(without_final_terminal("(see 12)"), "(see 12)");
        assert_eq!(without_final_terminal("down 50%"), "down 50%");
    }

    // What the owner says to a project's architect comes to its queue, and something empty is refused.
    #[test]
    fn the_owner_s_words_come_to_the_architect_s_queue() {
        let mut db = Db::in_memory().unwrap();
        assert!(say_to_architect(&mut db, "p", None, None, "  ", &[]).is_err());
        assert!(say_to_architect(&mut db, "", None, None, "Go on.", &[]).is_err());
        assert_eq!(
            say_to_architect(
                &mut db,
                "p",
                None,
                None,
                "Put the parser first. It blocks the rest.",
                &[]
            )
            .unwrap(),
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
