//! What the owner marks on the page: messages to the owner read. It is the one thing the interface writes to the
//! database itself, as the owner; the engine never reads it.

use std::path::Path;

use db::rusqlite::params;
use db::{Actor, Db};

use crate::read::Error;
use crate::{MarkRead, Marked};

/// Marks messages to the owner read: those named, or every one still unread.
pub fn mark_read(db: &Path, m: &MarkRead) -> Result<Marked, Error> {
    let failed = |e: db::Error| Error::Failed(format!("{}: {e}", db.display()));
    let mut conn = Db::open(db).map_err(failed)?;
    let now = db::now();
    let marked = conn
        .write(Actor::Owner, |tx| {
            if m.all {
                return Ok(tx.execute("UPDATE owner_messages SET read_at = ?1 WHERE read_at IS NULL", [now])?);
            }
            let mut n = 0;
            for id in &m.ids {
                n += tx.execute(
                    "UPDATE owner_messages SET read_at = ?1 WHERE id = ?2 AND read_at IS NULL",
                    params![now, id],
                )?;
            }
            Ok(n)
        })
        .map_err(failed)?;
    Ok(Marked { marked: marked as i64 })
}

#[cfg(test)]
mod tests {
    use super::*;

    // The owner marks a message read, then every one left; one already read stays as it was.
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
            tx.execute("INSERT INTO works (item, flow_version, graph, at) VALUES (1, 1, 'engineer', '1')", [])?;
            tx.execute(
                "INSERT INTO rounds (work, cell, input, config_commit, started_at) VALUES (1, '1', '{}', 'c', 1)",
                [],
            )?;
            for t in ["a", "b", "c"] {
                tx.execute("INSERT INTO owner_messages (round, text) VALUES (1, ?1)", [t])?;
            }
            Ok(())
        })
        .unwrap();
        let one = MarkRead {
            ids: vec![2, 2],
            all: false,
        };
        assert_eq!(mark_read(&path, &one).unwrap(), Marked { marked: 1 });
        let all = MarkRead {
            ids: Vec::new(),
            all: true,
        };
        assert_eq!(mark_read(&path, &all).unwrap(), Marked { marked: 2 });
        let unread: i64 = db
            .conn()
            .query_row("SELECT count(*) FROM owner_messages WHERE read_at IS NULL", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(unread, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
