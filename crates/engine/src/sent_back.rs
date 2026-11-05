//! Returns sent back, as the flow's rounds section says what the engine does with them (`rounds.sentBack`). Where the
//! section has `look`, each return sent back queues a matter to set straight for the project's architect in the write
//! that ends its round ([`matter`]), and the work goes on at once: it waits for nothing. As many returns in a row as
//! `pauseAfter` pause the work at the cell ([`pause`]); the reason says every problem of the last return, one line each
//! by its check and field. Where the section has `look` too, the architect may let such a pause go on itself
//! ([`db::architect_may_resume`]), and the matters of the work's returns are then ones the work waits on (the view
//! `architect_queue`).

use db::rusqlite::{Connection, params};
use flow::format::Rounds;
use serde_json::{Value, json};

use crate::failed::{self, Took};
use crate::work::{RoundId, Work};
use crate::{Advance, Engine};

/// The detail of the matter to set straight that asks the architect why a return was sent back.
pub const SENT_BACK: &str = "sent_back";

fn sql(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// What a round was sent back with, as `rounds.rejected` holds it; null when the round has none.
fn rejected_of(conn: &Connection, work: i64, n: i64) -> Result<Value, String> {
    let stored: Option<String> = conn
        .query_row(
            "SELECT rejected FROM rounds WHERE work = ?1 AND n = ?2",
            params![work, n],
            |r| r.get(0),
        )
        .map_err(sql)?;
    Ok(stored
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(Value::Null))
}

/// The names of the checks that found the problems, each once, in the order found.
fn checks_of(problems: &Value) -> Vec<String> {
    let mut checks: Vec<String> = Vec::new();
    for p in problems.as_array().into_iter().flatten() {
        if let Some(c) = p["check"].as_str().filter(|c| !checks.iter().any(|x| x == c)) {
            checks.push(c.to_string());
        }
    }
    checks
}

/// The matter to queue for the architect of the work's project when a return of the work at `cell` was sent back with
/// `rejected` (the column as `rounds.rejected` keeps it): why it was sent back, and what to change. None where the
/// section does not say `look`, and for a work on no item. `problems` holds the column's own, `{check, field, text}`
/// each, so that the matters of one check and field can be told apart and counted; `reason` says them as the owner
/// reads them. The matter is for the architect to look at after the fact: it says nothing of the work waiting.
pub fn matter(
    e: &Engine,
    w: &Work,
    section: &Rounds,
    cell: &str,
    round: RoundId,
    rejected: &Value,
) -> Result<Option<db::Matter>, String> {
    if !section.sent_back.look {
        return Ok(None);
    }
    let Some(item) = e.subject(w)? else {
        return Ok(None);
    };
    let problems = rejected
        .get("problems")
        .cloned()
        .filter(Value::is_array)
        .unwrap_or(json!([]));
    let checks = checks_of(&problems);
    let by = if checks.is_empty() {
        String::new()
    } else {
        format!(" for the checks {}", crate::cards::and_list(&checks))
    };
    Ok(Some(db::Matter {
        project: item.project.clone(),
        kind: "set_straight".into(),
        detail: Some(SENT_BACK.into()),
        what: format!(
            "Work {} on item {} ({}) had a return sent back at cell {cell}{by}: why it was sent back, and what to \
             change",
            w.id, item.id, item.title
        ),
        body: json!({
            "work": w.id,
            "round": round.n,
            "cell": cell,
            "problems": problems,
            "reason": db::sent_back_lines(rejected).join("\n"),
        }),
        item: Some(item.id),
        // The matter is not the work's question: it is raised by no round the work waits at.
        raised: None,
    }))
}

/// Pauses the work at `cell`, where `sent_back` (the latest first) returns in a row were sent back as often as the
/// section says. Where the section says `look` and the work is on an item, its project's architect has the matter of
/// each return and may let the work go on, as the owner may; otherwise the pause is the owner's alone.
pub fn pause(e: &mut Engine, w: &Work, cell: &str, section: &Rounds, sent_back: &[&Took]) -> Result<Advance, String> {
    let last = sent_back.first().ok_or("no return was sent back")?.round;
    let rejected = rejected_of(e.db.conn(), last.work, last.n)?;
    let list = db::sent_back_lines(&rejected).join("\n");
    let n = sent_back.len();
    let why = if section.sent_back.look && e.subject(w)?.is_some() {
        format!(
            "{} {n} times in a row, the last at cell {cell}, and its project's architect is asked to look at why; the \
             architect or the owner lets the work go on. Problems of the last return:\n{list}",
            db::SENT_BACK_FOR_ARCHITECT
        )
    } else {
        format!("{n} returns in a row were sent back, the last at cell {cell}, for:\n{list}")
    };
    failed::pause(e, w, &why)
}
