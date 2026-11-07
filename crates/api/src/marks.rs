//! What the owner marks on the page: messages to the owner and to the flow's keeper read, agents' and the engine's
//! alike. It is written as the owner; the engine reads no more of it than whether a fault's message is still open.

use std::path::Path;

use db::rusqlite::params;
use db::{Actor, Db};

use crate::read::Error;
use crate::{MarkRead, Marked, ReadTo};

/// Marks messages to the owner or the keeper read: those named, or every one still unread.
pub fn mark_read(db: &Path, m: &MarkRead) -> Result<Marked, Error> {
    let failed = |e: db::Error| Error::Failed(format!("{}: {e}", db.display()));
    let mut conn = Db::open(db).map_err(failed)?;
    let now = db::now();
    let ids = (!m.all).then_some(m.ids.as_slice());
    let marked = conn
        .write(Actor::Owner, |tx| db::mark_read(tx, ids, now))
        .map_err(failed)?;
    Ok(Marked { marked: marked as i64 })
}

/// Marks how far the owner has read a conversation, as the page does when it shows one: up to the latest entry shown,
/// never back; and the agents' and the engine's messages to the owner in it up to there read, faults aside, which the
/// owner marks read where they are shown.
pub fn read_to(db: &Path, r: &ReadTo) -> Result<Marked, Error> {
    let failed = |e: db::Error| Error::Failed(format!("{}: {e}", db.display()));
    let mut conn = Db::open(db).map_err(failed)?;
    let now = db::now();
    let marked = conn
        .write(Actor::Owner, |tx| {
            tx.execute(
                "INSERT INTO conversation_reads (conversation, at) VALUES (?1, ?2)
                 ON CONFLICT (conversation) DO UPDATE SET at = max(at, excluded.at)",
                params![r.key, r.at],
            )?;
            Ok(tx.execute(
                "UPDATE deliveries SET read_at = ?3 WHERE read_at IS NULL AND recipient = 'owner' AND message IN (
                   SELECT id FROM conversation_entries
                   WHERE conversation = ?1 AND source IN ('message', 'notice') AND at <= ?2)",
                params![r.key, r.at, now],
            )?)
        })
        .map_err(failed)?;
    Ok(Marked { marked: marked as i64 })
}

#[cfg(test)]
mod tests {
    use super::*;

    // The owner marks a message read, then every one left, the engine's and the keeper's among them; one already read
    // stays as it was.
    #[test]
    fn the_owner_marks_messages_read() {
        let dir = std::env::temp_dir().join(format!("wf-agents-marks-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("db.sqlite");
        let mut db = Db::create(&path).unwrap();
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
            for t in ["a", "b", "c"] {
                let m = db::Message {
                    sender: db::Sender::Engineer,
                    work: Some(1),
                    round: Some(1),
                    text: t,
                    body: None,
                };
                db::send(tx, &m, db::Recipient::Owner)?;
            }
            let stopped = db::Message {
                sender: db::Sender::Engine,
                work: Some(1),
                round: None,
                text: "Work 1 stopped.",
                body: None,
            };
            db::send(tx, &stopped, db::Recipient::Owner)?;
            let fault = db::Message {
                sender: db::Sender::Engine,
                work: None,
                round: None,
                text: "Missed.",
                body: Some(serde_json::json!({ "kind": "cache_miss" })),
            };
            db::send(tx, &fault, db::Recipient::Keeper)?;
            Ok(())
        })
        .unwrap();
        let one = MarkRead {
            ids: vec![2, 2, 4],
            all: false,
        };
        assert_eq!(mark_read(&path, &one).unwrap(), Marked { marked: 2 });
        let all = MarkRead {
            ids: Vec::new(),
            all: true,
        };
        assert_eq!(mark_read(&path, &all).unwrap(), Marked { marked: 3 });
        let unread: i64 = db
            .conn()
            .query_row("SELECT count(*) FROM deliveries WHERE read_at IS NULL", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(unread, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
