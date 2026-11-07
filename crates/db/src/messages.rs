//! The message hub's tables: `messages`, each message written once, and `deliveries`, each recipient it went to and
//! what became of it there. A fault of wf-agents goes to the flow's keeper and every other message to the owner; a
//! fault is passed on from the keeper to a project's architect as a matter of its queue. The engine writes what it and
//! the agents' returns say, each in the transaction of what made it; the owner's operations mark what the owner read
//! and pass faults on.

use rusqlite::{OptionalExtension, Transaction, params};
use serde_json::Value;

use crate::matters::one_sentence;
use crate::{Error, Result, now};

/// The detail of a report that holds a fault of wf-agents passed on to a project's architect.
pub const FAULT: &str = "fault";

/// Who sends a message, as `messages.sender` names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sender {
    /// A line of an engineer's work.
    Engineer,
    /// A line of a project's architect's work.
    Architect,
    Engine,
}

impl Sender {
    pub fn as_str(self) -> &'static str {
        match self {
            Sender::Engineer => "engineer",
            Sender::Architect => "architect",
            Sender::Engine => "engine",
        }
    }
}

/// Whom a message goes to as it is sent: the owner, or the flow's keeper, for a fault of wf-agents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Recipient {
    Owner,
    Keeper,
}

impl Recipient {
    pub fn as_str(self) -> &'static str {
        match self {
            Recipient::Owner => "owner",
            Recipient::Keeper => "keeper",
        }
    }
}

/// A message to send: who sends it, the work it is about and, for an agent's, the round whose return holds it; its
/// text, and what more it says (`messages.body`).
#[derive(Debug, Clone)]
pub struct Message<'a> {
    pub sender: Sender,
    pub work: Option<i64>,
    pub round: Option<i64>,
    pub text: &'a str,
    pub body: Option<Value>,
}

/// Sends a message now: written once, and delivered to `to`. Gives the message.
pub fn send(tx: &Transaction<'_>, m: &Message<'_>, to: Recipient) -> Result<i64> {
    tx.execute(
        "INSERT INTO messages (sender, work, round, text, body, at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            m.sender.as_str(),
            m.work,
            m.round,
            m.text,
            m.body.as_ref().map(Value::to_string),
            now()
        ],
    )?;
    let id = tx.last_insert_rowid();
    tx.execute(
        "INSERT INTO deliveries (message, recipient) VALUES (?1, ?2)",
        params![id, to.as_str()],
    )?;
    Ok(id)
}

/// Passes a fault of wf-agents, a message the flow's keeper received, on to a project's architect, as the owner: a
/// matter of the kind report with the detail fault comes to the architect's queue as a report does, its material
/// gathered from the message when it is handled, and the message's delivery to the architect names it; the keeper's
/// delivery is then read. Which architect may take it is the caller's to judge. A message the keeper did not receive,
/// or that went to that architect already, is refused. Gives the matter.
pub fn pass_on(tx: &Transaction<'_>, message: i64, project: &str) -> Result<i64> {
    let text: Option<String> = tx
        .query_row(
            "SELECT m.text FROM messages m JOIN deliveries d ON d.message = m.id
             WHERE m.id = ?1 AND d.recipient = 'keeper'",
            [message],
            |r| r.get(0),
        )
        .optional()?;
    let Some(text) = text else {
        return Err(Error::Abort(format!(
            "message {message} is no fault of wf-agents the flow's keeper received"
        )));
    };
    let project = project.trim();
    if project.is_empty() {
        return Err(Error::Abort(
            "a fault is passed on to a project's architect, named".into(),
        ));
    }
    let went: bool = tx.query_row(
        "SELECT EXISTS (SELECT 1 FROM deliveries d JOIN architect_matters m ON m.id = d.matter
                        WHERE d.message = ?1 AND m.project = ?2)",
        params![message, project],
        |r| r.get(0),
    )?;
    if went {
        return Err(Error::Abort(format!(
            "message {message} went to the architect of {project} already"
        )));
    }
    tx.execute(
        "INSERT INTO architect_matters (project, kind, detail, what, body, at) VALUES (?1, 'report', ?2, ?3, ?4, ?5)",
        params![
            project,
            FAULT,
            format!("The owner passes on: {}", one_sentence(&text)),
            serde_json::json!({ "by": "owner" }).to_string(),
            now()
        ],
    )?;
    let matter = tx.last_insert_rowid();
    tx.execute(
        "INSERT INTO deliveries (message, recipient, matter) VALUES (?1, 'architect', ?2)",
        params![message, matter],
    )?;
    tx.execute(
        "UPDATE deliveries SET read_at = ?2 WHERE message = ?1 AND recipient = 'keeper' AND read_at IS NULL",
        params![message, now()],
    )?;
    Ok(matter)
}

/// Marks messages read for the owner and the keeper, at `at`: those named, each once, or with none named every one
/// still unread. Gives how many were marked.
pub fn mark_read(tx: &Transaction<'_>, ids: Option<&[i64]>, at: i64) -> Result<usize> {
    let Some(ids) = ids else {
        return Ok(tx.execute(
            "UPDATE deliveries SET read_at = ?1 WHERE read_at IS NULL AND recipient IN ('owner', 'keeper')",
            [at],
        )?);
    };
    let mut n = 0;
    for id in ids {
        n += tx.execute(
            "UPDATE deliveries SET read_at = ?2
             WHERE message = ?1 AND read_at IS NULL AND recipient IN ('owner', 'keeper')",
            params![id, at],
        )?;
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Actor, Db};

    /// A database with a work of an item and its first round.
    fn db() -> Db {
        let mut db = Db::in_memory().unwrap();
        db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO items (project, title, problem, why, deliver, level) VALUES ('p', 't', 'x', 'y', 'change', '1')",
                [],
            )?;
            tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
            tx.execute("INSERT INTO works (item, flow_version, graph, at) VALUES (1, 1, 'engineer', 'R1')", [])?;
            tx.execute(
                "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES (1, 1, 'R1', '{}', 'c', 1)",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        db
    }

    fn rows(db: &Db, sql: &str) -> Vec<String> {
        let mut st = db.conn().prepare(sql).unwrap();
        st.query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect()
    }

    // An agent's message goes to the owner, the engine's fault to the flow's keeper; each is written once with its
    // delivery, unread.
    #[test]
    fn a_fault_goes_to_the_keeper_and_every_other_message_to_the_owner() {
        let mut db = db();
        db.write(Actor::Engine, |tx| {
            let told = Message {
                sender: Sender::Engineer,
                work: Some(1),
                round: Some(1),
                text: "Look at the log.",
                body: None,
            };
            send(tx, &told, Recipient::Owner)?;
            let fault = Message {
                sender: Sender::Engine,
                work: Some(1),
                round: None,
                text: "R1-1 missed its session's prompt cache.",
                body: Some(serde_json::json!({ "kind": "cache_miss" })),
            };
            send(tx, &fault, Recipient::Keeper)?;
            Ok(())
        })
        .unwrap();
        assert_eq!(
            rows(
                &db,
                "SELECT m.sender || ' ' || d.recipient || ' ' || coalesce(d.read_at, 'unread') || ' ' || m.text
                 FROM messages m JOIN deliveries d ON d.message = m.id ORDER BY m.id"
            ),
            [
                "engineer owner unread Look at the log.",
                "engine keeper unread R1-1 missed its session's prompt cache."
            ]
        );
        // An agent's message names its round, the engine's none.
        let bad = db.write(Actor::Engine, |tx| {
            let m = Message {
                sender: Sender::Engine,
                work: Some(1),
                round: Some(1),
                text: "x",
                body: None,
            };
            send(tx, &m, Recipient::Owner)
        });
        assert!(bad.is_err());
    }

    // A fault is passed on to a project's architect as a report of its queue, which its delivery names, and the
    // keeper's delivery is read; the same fault to the same architect again, or a message the keeper did not receive,
    // is refused.
    #[test]
    fn a_fault_is_passed_on_to_an_architect_once() {
        let mut db = db();
        let (told, fault) = db
            .write(Actor::Engine, |tx| {
                let told = Message {
                    sender: Sender::Engineer,
                    work: Some(1),
                    round: Some(1),
                    text: "Look at the log.",
                    body: None,
                };
                let fault = Message {
                    sender: Sender::Engine,
                    work: Some(1),
                    round: None,
                    text: "R1-1 missed its session's prompt cache. It wrote 90% again.",
                    body: Some(serde_json::json!({ "kind": "cache_miss" })),
                };
                Ok((send(tx, &told, Recipient::Owner)?, send(tx, &fault, Recipient::Keeper)?))
            })
            .unwrap();
        let matter = db.write(Actor::Owner, |tx| pass_on(tx, fault, "wf")).unwrap();
        assert_eq!(
            rows(
                &db,
                "SELECT m.project || ' ' || m.kind || ' ' || m.detail || ' ' || m.what || ' ' || json_extract(m.body, '$.by')
                 FROM architect_matters m JOIN deliveries d ON d.matter = m.id"
            ),
            ["wf report fault The owner passes on: R1-1 missed its session's prompt cache. owner"]
        );
        assert_eq!(matter, 1);
        assert_eq!(
            rows(
                &db,
                "SELECT recipient FROM deliveries WHERE message = 2 AND read_at IS NULL"
            ),
            ["architect"]
        );
        let again = db.write(Actor::Owner, |tx| pass_on(tx, fault, "wf")).unwrap_err();
        assert!(again.to_string().contains("already"), "{again}");
        let not_a_fault = db.write(Actor::Owner, |tx| pass_on(tx, told, "wf")).unwrap_err();
        assert!(not_a_fault.to_string().contains("no fault"), "{not_a_fault}");
    }

    // The owner marks messages read, those named or every one left; one already read stays as it was, and an
    // architect's delivery has nothing to mark.
    #[test]
    fn the_owner_marks_messages_read() {
        let mut db = db();
        db.write(Actor::Engine, |tx| {
            for t in ["a", "b", "c"] {
                let m = Message {
                    sender: Sender::Engineer,
                    work: Some(1),
                    round: Some(1),
                    text: t,
                    body: None,
                };
                send(tx, &m, Recipient::Owner)?;
            }
            Ok(())
        })
        .unwrap();
        assert_eq!(db.write(Actor::Owner, |tx| mark_read(tx, Some(&[2, 2]), 5)).unwrap(), 1);
        assert_eq!(db.write(Actor::Owner, |tx| mark_read(tx, None, 6)).unwrap(), 2);
        assert_eq!(
            rows(&db, "SELECT CAST(read_at AS TEXT) FROM deliveries ORDER BY message"),
            ["6", "5", "6"]
        );
    }
}
