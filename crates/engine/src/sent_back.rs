//! The pause at returns sent back. A work whose returns were sent back as often in a row as the flow's rounds section
//! says (`rounds.sentBack.pauseAfter`) is paused at the cell, and the reason says every problem of the last return, one
//! line each by its check and field. Where the section has `look`, the pause is the project's architect's to look at: it
//! comes after the first return sent back, and the engine queues a matter to set straight for the architect in the same
//! write, with each problem as the return was sent back with it, so that the cause is found and recorded; the architect
//! may let the work go on itself ([`db::architect_may_resume`]). Without `look`, or for a work on no item, the pause is
//! the owner's alone.

use db::rusqlite::{Connection, params};
use flow::format::Rounds;
use serde_json::{Value, json};

use crate::failed::{self, Took};
use crate::work::{Item, Work};
use crate::{Advance, Engine};

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

/// The matter for the architect: why a return of the work was sent back, and what to change, while the work waits paused
/// at the cell. `problems` holds `rounds.rejected`'s own, `{check, field, text}` each, so that the matters of one check
/// and field can be told apart and counted; `reason` says them as the owner reads them.
fn matter(w: &Work, item: &Item, cell: &str, round: i64, rejected: &Value, lines: &[String]) -> db::Matter {
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
    db::Matter {
        project: item.project.clone(),
        kind: "set_straight".into(),
        detail: Some(SENT_BACK.into()),
        what: format!(
            "Work {} on item {} ({}) had a return sent back at cell {cell}{by}, and waits paused at the cell: why it \
             was sent back, and what to change",
            w.id, item.id, item.title
        ),
        body: json!({
            "work": w.id,
            "round": round,
            "cell": cell,
            "problems": problems,
            "reason": lines.join("\n"),
        }),
        item: Some(item.id),
        raised: None,
    }
}

/// The detail of the matter to set straight that asks the architect why a return was sent back.
pub const SENT_BACK: &str = "sent_back";

/// Pauses the work at `cell`, where `sent_back` (the latest first) returns in a row were sent back as often as the
/// section says; for a work on an item, with the architect's look when the section says it.
pub fn pause(e: &mut Engine, w: &Work, cell: &str, section: &Rounds, sent_back: &[&Took]) -> Result<Advance, String> {
    let last = sent_back.first().ok_or("no return was sent back")?.round;
    let rejected = rejected_of(e.db.conn(), last.work, last.n)?;
    let lines = db::sent_back_lines(&rejected);
    let item = match e.subject(w)? {
        Some(i) if section.sent_back.look => Some(i),
        _ => None,
    };
    let list = lines.join("\n");
    let Some(item) = item else {
        let how = match sent_back.len() {
            1 => "the return was sent back".to_string(),
            n => format!("{n} returns in a row were sent back, the last"),
        };
        let why = format!("{how} at cell {cell}, for:\n{list}");
        return failed::pause(e, w, &why);
    };
    let why = format!(
        "{} at cell {cell}, and its project's architect is asked to look at why; the architect or the owner lets the \
         work go on. Problems:\n{list}",
        db::SENT_BACK_FOR_ARCHITECT
    );
    let m = matter(w, &item, cell, last.n, &rejected, &lines);
    failed::pause_and(e, w, &why, |tx| {
        db::queue_matter(tx, &m).map_err(sql)?;
        Ok(())
    })
}
