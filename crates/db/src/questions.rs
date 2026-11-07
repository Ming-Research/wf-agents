//! The questions each project's architect pinned for the owner's reply in their one-to-one: those that wait, and the
//! one the architect waits on now. The engine reads them at the look at the queue (`N0`), which waits for the reply to
//! that one, and gives the cells that may ask those that wait; the owner's interface pins those that wait at the bottom
//! of the one-to-one, the reply ticked to the one waited on. One reading for both, so that the page ticks the question
//! the engine waits on.

use rusqlite::{Connection, OptionalExtension};

use crate::Result;

/// The questions a project's architect (`?1`) put to the owner that wait for the owner's reply, newest first: each an
/// ask of the owner a cell of the architect's graph put, not answered, that parks a matter still open; with the
/// question, the cell that asked, when it was put (when the owner was told of it, else when its round started) and the
/// matter it parked.
pub const QUESTIONS_PENDING: &str = "SELECT a.id AS question, json_extract(a.body, '$.question') AS text,
        m.resume_cell AS cell,
        coalesce((SELECT min(n.at) FROM messages n WHERE n.ask = a.id), r.started_at) AS at,
        m.id AS matter, m.what AS about
 FROM work_asks a JOIN rounds r ON r.work = a.work AND r.n = a.round JOIN works w ON w.id = a.work
 JOIN architect_matters m ON m.asked = a.id
 WHERE w.graph = 'architect' AND w.project = ?1 AND a.answer IS NULL
   AND m.handled_round IS NULL AND m.withdrawn IS NULL
 ORDER BY at DESC, a.id DESC";

/// The question the architect of `project` waits for the owner's reply to at the look at the queue (`N0`'s row
/// `waits`): the newest question pending ([`QUESTIONS_PENDING`]) that parks a matter other than an open question, when
/// it was put less than `wait_ms` (the installation's wait for the owner's reply) before `now` and the owner said
/// nothing to the architect since in their one-to-one: no words, no entry of a talk, and no reply to another question.
/// None otherwise: the question then waits at the back of the queue, and blocks nothing. A question that parks an open
/// question of an engineer's design is never waited on: the engineer's work goes on without its answer, so the architect
/// has no reason to stop for it either, and it stays pinned in the one-to-one all the same.
pub fn question_waited_on(conn: &Connection, project: &str, wait_ms: i64, now: i64) -> Result<Option<i64>> {
    // The newest of the questions that do not park an open question; the pending query lists them newest first.
    let newest: Option<(i64, Option<i64>)> = conn
        .query_row(
            &format!(
                "SELECT p.question, p.at FROM ({QUESTIONS_PENDING}) p JOIN architect_matters m ON m.id = p.matter
                 WHERE m.kind <> 'open_question' ORDER BY p.at DESC, p.question DESC LIMIT 1"
            ),
            [project],
            |r| Ok((r.get("question")?, r.get("at")?)),
        )
        .optional()?;
    let Some((id, Some(at))) = newest else {
        return Ok(None);
    };
    if now - at >= wait_ms {
        return Ok(None);
    }
    let said: i64 = conn.query_row(
        "SELECT (SELECT count(*) FROM architect_matters
                 WHERE project = ?1 AND kind = 'owner_words' AND json_extract(body, '$.where') = 'page' AND at > ?2)
              + (SELECT count(*) FROM deliveries d JOIN messages e ON e.id = d.message
                   JOIN architect_matters m ON m.id = d.matter
                 WHERE d.recipient = 'architect' AND e.sender = 'owner' AND m.project = ?1 AND e.at > ?2
                   AND NOT EXISTS (SELECT 1 FROM deliveries x WHERE x.message = e.id AND x.recipient = 'line'))
              + (SELECT count(*) FROM work_asks a JOIN works w ON w.id = a.work
                 WHERE w.graph = 'architect' AND w.project = ?1 AND json_extract(a.answer, '$.at') > ?2)",
        rusqlite::params![project, at],
        |r| r.get(0),
    )?;
    Ok((said == 0).then_some(id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Actor, Db};

    /// A database with a work of project p's architect and, for each of `asks` (kind of the matter it parks, and when
    /// it was put; the newest last), a round of its own, an ask of the owner that round put, and the matter it parks.
    fn asked(asks: &[(&str, i64)]) -> Db {
        let mut db = Db::in_memory().unwrap();
        db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO flow_versions (id, digest, compiled) VALUES (1, 'd', '{}')",
                [],
            )?;
            tx.execute(
                "INSERT INTO works (id, item, flow_version, graph, at, project)
                 VALUES (1, NULL, 1, 'architect', 'H3', 'p')",
                [],
            )?;
            for (n, (kind, at)) in (1..).zip(asks) {
                tx.execute(
                    "INSERT INTO rounds (work, n, cell, input, config_commit, started_at)
                     VALUES (1, ?1, 'H3', '{}', 'c', ?2)",
                    rusqlite::params![n, at],
                )?;
                tx.execute(
                    "INSERT INTO messages (kind, sender, work, round, text, body, at) VALUES ('ask', 'engineer', 1, ?1, '', json_object('question', 'Which?'), 0)",
                    [n],
                )?;
                let ask = tx.last_insert_rowid();
                tx.execute(
                    "INSERT INTO architect_matters (project, kind, what, body, at, asked, resume_cell)
                     VALUES ('p', ?1, 'Which?', '{}', ?2, ?3, 'H3')",
                    rusqlite::params![kind, at, ask],
                )?;
            }
            Ok(())
        })
        .unwrap();
        db
    }

    // The architect waits for the owner's reply to the newest question it just put, unless that question pins an open
    // question of an engineer's design: that engineer's work goes on without the answer, so the question is never waited
    // on, though it stays among the pending ones, and the architect waits for the one put before it. After the wait, or
    // with nothing pending, there is nothing to wait on.
    #[test]
    fn a_question_that_pins_an_open_question_is_not_waited_on() {
        let wait = 300_000;
        let db = asked(&[("engineer_question", 1_000), ("open_question", 2_000)]);
        let pending = |db: &Db| -> Vec<i64> {
            let mut st = db.conn().prepare(QUESTIONS_PENDING).unwrap();
            st.query_map(["p"], |r| r.get("question"))
                .unwrap()
                .map(|x| x.unwrap())
                .collect()
        };
        // Both are pending, the open question's the newest; the architect waits for the other.
        assert_eq!(pending(&db), [2, 1]);
        assert_eq!(question_waited_on(db.conn(), "p", wait, 2_500).unwrap(), Some(1));
        // Once the wait ran out, it waits for nothing.
        assert_eq!(question_waited_on(db.conn(), "p", wait, 1_000 + wait).unwrap(), None);
        // With only the open question pending, there is nothing to wait on.
        let db = asked(&[("open_question", 1_000)]);
        assert_eq!(pending(&db), [1]);
        assert_eq!(question_waited_on(db.conn(), "p", wait, 1_500).unwrap(), None);
        // A question of another kind, put alone, is waited on.
        let db = asked(&[("report", 1_000)]);
        assert_eq!(question_waited_on(db.conn(), "p", wait, 1_500).unwrap(), Some(1));
    }
}
