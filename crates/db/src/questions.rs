//! The questions each project's architect pinned for the owner's reply in their one-to-one: those that wait, and the
//! one the architect waits on now. The engine reads them at the look at the queue (`Q0`), which waits for the reply to
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
        m.resume_cell AS cell, coalesce((SELECT min(n.at) FROM engine_notices n WHERE n.ask = a.id), r.started_at) AS at,
        m.id AS matter, m.what AS about
 FROM owner_asks a JOIN rounds r ON r.work = a.work AND r.n = a.round JOIN works w ON w.id = a.work
 JOIN architect_matters m ON m.asked = a.id
 WHERE w.graph = 'architect' AND w.project = ?1 AND a.answer IS NULL
   AND m.handled_round IS NULL AND m.withdrawn IS NULL
 ORDER BY at DESC, a.id DESC";

/// The question the architect of `project` waits for the owner's reply to at the look at the queue (`Q0`'s row
/// `waits`): the newest question pending ([`QUESTIONS_PENDING`]), when it was put less than `wait_ms` (the
/// installation's wait for the owner's reply) before `now` and the owner said nothing to the architect since in their
/// one-to-one: no words, no entry of a talk, and no reply to another question. None otherwise: the question then waits
/// at the back of the queue, and blocks nothing.
pub fn question_waited_on(conn: &Connection, project: &str, wait_ms: i64, now: i64) -> Result<Option<i64>> {
    // The first row is the newest.
    let newest: Option<(i64, Option<i64>)> = conn
        .query_row(QUESTIONS_PENDING, [project], |r| Ok((r.get("question")?, r.get("at")?)))
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
              + (SELECT count(*) FROM talk_entries e JOIN architect_matters m ON m.id = e.matter
                 WHERE m.project = ?1 AND e.at > ?2)
              + (SELECT count(*) FROM owner_asks a JOIN works w ON w.id = a.work
                 WHERE w.graph = 'architect' AND w.project = ?1 AND json_extract(a.answer, '$.at') > ?2)",
        rusqlite::params![project, at],
        |r| r.get(0),
    )?;
    Ok((said == 0).then_some(id))
}
