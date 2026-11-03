//! The architects' matters, as `architect_matters` keeps them: what comes to each project's architect's queue from
//! engineers' questions, side reports, decision cards, the owner's words and answers, other projects' architects, the
//! architect's own rounds, the faults of wf-agents forwarded to it, and the engine. Each is queued in the transaction
//! of what raised it, by the engine and by the owner's operations alike.

use std::sync::LazyLock;

use regex::Regex;
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde_json::Value;
use unicode_segmentation::UnicodeSegmentation;

use crate::attachments::{self, Image, Words};
use crate::{Actor, Db, Error, Result, STOPPED_BY_ARCHITECT, now, withdraw_resume};

/// A matter to queue for a project's architect.
#[derive(Debug, Clone, Default)]
pub struct Matter {
    /// The project whose architect it is for.
    pub project: String,
    /// Its kind, which decides its path: set_straight, engineer_question, open_question, report, design_card, failure,
    /// owner_words, other_architect, slots_free, or check_policy and make_plan, which come at the head of the queue.
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

/// The project of an item, which its architect's matters go to; none for an item there is not.
pub fn project_of_item(tx: &Transaction<'_>, item: i64) -> Result<Option<String>> {
    Ok(tx
        .query_row("SELECT project FROM items WHERE id = ?1", [item], |r| r.get(0))
        .optional()?)
}

/// What the owner leaves with the Start of a work the architect stopped, and who the caller declares to leave it. The
/// owner may leave none. Only a work the architect stopped takes it: the architect, which asked the owner to do
/// something before the work goes on, is told the note with the release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartNote {
    pub text: String,
    pub speaker: Speaker,
}

impl StartNote {
    /// The note a request declares: its text and the speaker's name. None for a request with no text; an error for a
    /// text with no speaker declared, or one that is neither the owner nor a delegate.
    pub fn declared(text: Option<&str>, speaker: Option<&str>) -> std::result::Result<Option<StartNote>, String> {
        let Some(text) = text.map(str::trim).filter(|t| !t.is_empty()) else {
            return Ok(None);
        };
        let speaker = Speaker::parse(speaker.unwrap_or_default())?;
        Ok(Some(StartNote {
            text: text.to_string(),
            speaker,
        }))
    }
}

/// Lifts a work's pause, in a write made as the owner, as the owner's Start of it does: a work the architect stopped is
/// then the owner's release of it, which the architect of its project is told as the owner's words, with the `note` the
/// owner left with the Start (kept as `note` in the matter, with the speaker declared as `speaker`; a Start with none
/// leaves neither). A request to lift the pause still in the outbox goes with it, so that none outlives the pause it
/// was for and lifts a later one. Gives whether the architect had stopped it.
pub fn lift_pause(tx: &Transaction<'_>, work: i64, note: Option<&StartNote>) -> Result<bool> {
    let (paused, by, item): (Option<String>, Option<String>, Option<i64>) =
        tx.query_row("SELECT paused, paused_by, item FROM works WHERE id = ?1", [work], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })?;
    tx.execute("UPDATE works SET paused = NULL, paused_by = NULL WHERE id = ?1", [work])?;
    withdraw_resume(tx, work)?;
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
    let mut body = serde_json::json!({ "text": text, "work": work });
    let mut what = format!("The owner says: {}", one_sentence(&text));
    if let Some(n) = note {
        body["note"] = Value::String(n.text.clone());
        body["speaker"] = Value::String(n.speaker.as_str().into());
        what.push_str(" A note comes with it.");
    }
    let m = Matter {
        project,
        kind: "owner_words".into(),
        detail: Some("release".into()),
        what,
        body,
        item,
        ..Matter::default()
    };
    queue_matter(tx, &m)?;
    Ok(true)
}

/// Who says the words that come to an architect as the owner's, as the caller declares it: the owner at the page, or a
/// session that speaks for the owner, a delegate. Every way in declares one. The engine cannot tell them apart, since
/// both reach it through the same interface, so the declaration is the caller's own: a delegate that declares itself
/// the owner is not caught. The architect's record of a decision says which was declared; words kept before the
/// declaration was asked for have none, and are told as unrecorded, never as the owner's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Speaker {
    Owner,
    Delegate,
}

impl Speaker {
    /// As the records keep it (`speaker` in the body of the owner's words, in the answer to an ask).
    pub fn as_str(self) -> &'static str {
        match self {
            Speaker::Owner => "owner",
            Speaker::Delegate => "delegate",
        }
    }

    /// The speaker a request declares, as the page and the command line name it; an error for a name that is neither,
    /// and for none.
    pub fn parse(name: &str) -> std::result::Result<Speaker, String> {
        match name.trim() {
            "owner" => Ok(Speaker::Owner),
            "delegate" => Ok(Speaker::Delegate),
            other => Err(format!(
                "the speaker, owner or delegate, says who speaks: {}",
                if other.is_empty() {
                    "none was given".to_string()
                } else {
                    format!("{other} is neither")
                }
            )),
        }
    }
}

/// Who a request declares to speak, and which parts of what a delegate says it declares to be the owner's own words
/// (`relayed`): a delegate's message can hold words the owner said to it, which the engine cannot check, since the owner
/// said them in the delegate's session and not through the interface. The declaration is the caller's own and is
/// kept as it was made; a decision that quotes a relayed part is told as the owner's, relayed by the delegate, not
/// checked, and one that quotes the rest of the message as the delegate's. A speaker alone declares no relayed part.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declared {
    pub speaker: Speaker,
    pub relayed: Vec<String>,
}

impl From<Speaker> for Declared {
    fn from(speaker: Speaker) -> Declared {
        Declared {
            speaker,
            relayed: Vec::new(),
        }
    }
}

impl Declared {
    /// What a request declares, by the speaker's name and the relayed parts it gives; an error for a speaker that is
    /// neither the owner nor a delegate, for none, and for relayed parts declared by anyone but a delegate, since the
    /// owner at the page relays nothing.
    pub fn parse(name: &str, relayed: &[String]) -> std::result::Result<Declared, String> {
        let declared = Declared {
            speaker: Speaker::parse(name)?,
            relayed: relayed.to_vec(),
        };
        declared.only_a_delegate_relays()?;
        Ok(declared)
    }

    /// That nothing is relayed unless a delegate declares it: the owner at the page relays nothing, since it says its
    /// own words.
    fn only_a_delegate_relays(&self) -> std::result::Result<(), String> {
        if self.speaker != Speaker::Delegate && !self.relayed.is_empty() {
            return Err(format!(
                "only a delegate relays the owner's words: {} says its own",
                self.speaker.as_str()
            ));
        }
        Ok(())
    }

    /// The relayed parts as the record keeps them, a JSON array, once each is found letter for letter in `stored`, the
    /// text the record keeps (the words after they were trimmed, as they are kept); none when nothing is relayed. An
    /// error names the part that is blank or is not in the text, and nothing is written then. The one place that
    /// checks what a caller relays: every way in goes through it.
    pub fn relayed_in(&self, stored: &str) -> std::result::Result<Option<Value>, String> {
        self.only_a_delegate_relays()?;
        if self.relayed.is_empty() {
            return Ok(None);
        }
        for r in &self.relayed {
            if r.trim().is_empty() {
                return Err("a relayed part is blank: give the words of the owner's that the message holds".into());
            }
            if !stored.contains(r.as_str()) {
                return Err(format!(
                    "the relayed part \"{r}\" is not found letter for letter in the words said, which are \
                     \"{stored}\": give a part of them as it is"
                ));
            }
        }
        Ok(Some(Value::Array(
            self.relayed.iter().map(|r| Value::String(r.clone())).collect(),
        )))
    }

    /// `body`, an object, with the speaker recorded in it as `speaker` and the relayed parts, if any, as `relayed`;
    /// `stored` is the text the body keeps the words in.
    pub fn record(&self, mut body: Value, stored: &str) -> std::result::Result<Value, String> {
        let relayed = self.relayed_in(stored)?;
        if let Some(o) = body.as_object_mut() {
            o.insert("speaker".into(), Value::String(self.speaker.as_str().into()));
            if let Some(r) = relayed {
                o.insert("relayed".into(), r);
            }
        }
        Ok(body)
    }
}

/// Says something to a project's architect, as the owner: the words, and the images sent with them, come to its queue
/// as a matter of the kind owner_words, which starts a work of it. Where it was said goes with it: in the conversation
/// about a work, the work and its item; on an item's page, the item. `speaker` is who the caller declares to say it.
pub fn say_to_architect(
    db: &mut Db,
    project: &str,
    item: Option<i64>,
    work: Option<i64>,
    text: &str,
    images: &[Image],
    speaker: impl Into<Declared>,
) -> std::result::Result<String, String> {
    let speaker: Declared = speaker.into();
    let text = text.trim();
    if text.is_empty() && images.is_empty() {
        return Err("what is said to the architect says something".into());
    }
    if project.trim().is_empty() {
        return Err("what is said to an architect names its project".into());
    }
    attachments::check(images)?;
    let body = match (work, item) {
        (Some(w), _) => serde_json::json!({ "text": text, "where": "work", "work": w }),
        (None, Some(_)) => serde_json::json!({ "text": text, "where": "item" }),
        (None, None) => serde_json::json!({ "text": text, "where": "page" }),
    };
    let m = Matter {
        project: project.to_string(),
        kind: "owner_words".into(),
        detail: Some("said".into()),
        what: attachments::owner_says(text, images.len()),
        body: speaker.record(body, text)?,
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
        assert!(say_to_architect(&mut db, "p", None, None, "  ", &[], Speaker::Owner).is_err());
        assert!(say_to_architect(&mut db, "", None, None, "Go on.", &[], Speaker::Owner).is_err());
        assert_eq!(
            say_to_architect(
                &mut db,
                "p",
                None,
                None,
                "Put the parser first. It blocks the rest.",
                &[],
                Speaker::Owner
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
        // The speaker the caller declared is kept with the words, whatever order the keys are written in.
        assert_eq!(
            serde_json::from_str::<Value>(&body).unwrap(),
            serde_json::json!({
                "text": "Put the parser first. It blocks the rest.",
                "where": "page",
                "speaker": "owner",
            })
        );
    }

    // The speaker a request declares goes into the owner's words as it was declared; a request that declares none, or a
    // name that is neither the owner nor a delegate, is refused.
    #[test]
    fn the_speaker_a_request_declares_is_kept_with_the_words() {
        assert_eq!(Speaker::parse("owner"), Ok(Speaker::Owner));
        assert_eq!(Speaker::parse(" delegate "), Ok(Speaker::Delegate));
        assert!(Speaker::parse("").unwrap_err().contains("none was given"));
        assert!(
            Speaker::parse("architect")
                .unwrap_err()
                .contains("architect is neither")
        );
        let mut db = Db::in_memory().unwrap();
        for speaker in [Speaker::Owner, Speaker::Delegate] {
            say_to_architect(&mut db, "p", None, Some(3), "Go on.", &[], speaker).unwrap();
        }
        let kept: Vec<Option<String>> = db
            .conn()
            .prepare("SELECT json_extract(body, '$.speaker') FROM architect_matters ORDER BY id")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<std::result::Result<_, _>>()
            .unwrap();
        assert_eq!(kept, [Some("owner".into()), Some("delegate".into())]);
    }

    // A delegate may declare which parts of what it says are the owner's own words: the parts are kept with the words, as
    // given, once each is found letter for letter in the words as they are kept (trimmed). A part that is blank or is not
    // in them, and parts declared by the owner, are refused, and nothing is kept.
    #[test]
    fn the_parts_a_delegate_relays_are_kept_with_the_words_once_found_in_them() {
        let parts = |rs: &[&str]| rs.iter().map(|r| r.to_string()).collect::<Vec<String>>();
        assert!(Declared::parse("owner", &[]).is_ok());
        let by_owner = Declared::parse("owner", &parts(&["Go on."]));
        assert!(by_owner.unwrap_err().contains("only a delegate relays"));
        assert!(Declared::parse("delegate", &parts(&["Go on."])).is_ok());
        let mut db = Db::in_memory().unwrap();
        let mut say = |text: &str, rs: &[&str]| {
            let by = Declared::parse("delegate", &parts(rs)).unwrap();
            say_to_architect(&mut db, "p", None, None, text, &[], by)
        };
        // The words are kept trimmed, and so is what a part is found in.
        say("  Yes. Put the parser first.  ", &["Put the parser first."]).unwrap();
        let text = "Yes. Put the parser first.";
        assert!(
            say(text, &["Put the parser first.", " "])
                .unwrap_err()
                .contains("blank")
        );
        let err = say(text, &["put the parser first."]).unwrap_err();
        assert!(err.contains("is not found letter for letter"), "{err}");
        let err = say(text, &["Yes.  Put"]).unwrap_err();
        assert!(err.contains("is not found letter for letter"), "{err}");
        // A blank at the edge of a part is not in the trimmed words.
        assert!(say("  Yes. Put it first.  ", &[" Yes."]).is_err());
        let rows = |db: &Db| -> i64 {
            let count = "SELECT count(*) FROM architect_matters";
            db.conn().query_row(count, [], |r| r.get(0)).unwrap()
        };
        assert_eq!(rows(&db), 1);
        let one = "SELECT body FROM architect_matters";
        let body: String = db.conn().query_row(one, [], |r| r.get(0)).unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&body).unwrap(),
            serde_json::json!({
                "text": "Yes. Put the parser first.",
                "where": "page",
                "speaker": "delegate",
                "relayed": ["Put the parser first."],
            })
        );
        // Declared by the owner at the page, nothing is relayed.
        let owner_relays = Declared {
            speaker: Speaker::Owner,
            relayed: vec!["Yes.".into()],
        };
        assert!(say_to_architect(&mut db, "p", None, None, "Yes.", &[], owner_relays).is_err());
        assert_eq!(rows(&db), 1);
    }
}
