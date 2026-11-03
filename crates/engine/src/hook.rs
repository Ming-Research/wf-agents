//! What the command line's hook gives a running round after each of the agent's tool uses, which the command line adds
//! to the session before the agent goes on: the interjections waiting for the round's line ([`sessions::interject`]),
//! and the engine's ask. Once the budget of the work's stretch is used up, the ask is to pause ([`crate::limits`]);
//! otherwise, for a round that may hand its progress over, once its own calls have paid as much rent as a new session
//! would cost, to hand over ([`crate::handover`]). The command `wf-agents deliver` runs it in a process of its own; the
//! session's calls are read from the ledger, which the round's watch keeps as the stream tells them.

use std::path::Path;

use serde_json::{Value, json};

/// What the hook gives round `round` of a work after a tool use: `line` is the round's line, none for a cell that takes
/// no interjections, and `hands_over` whether the engine may ask the round to hand over. None when there is nothing to
/// give, as for a round that has ended.
pub fn after_tool(
    data: &Path,
    work: i64,
    round: i64,
    line: Option<&str>,
    hands_over: bool,
) -> Result<Option<String>, String> {
    let mut db = crate::database::open(data)?;
    let said = match line {
        Some(l) => sessions::interject::deliver(&mut db, work, round, l)?,
        None => None,
    };
    let open: bool = db
        .conn()
        .query_row(
            "SELECT count(*) > 0 FROM rounds WHERE work = ?1 AND n = ?2 AND ended_at IS NULL",
            [work, round],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    if !open {
        return Ok(said);
    }
    let budget = match config::committed(data) {
        Ok(c) => crate::limits::used_up(&c.config, db.conn(), work, Some(db::now()))?,
        Err(_) => None,
    };
    let ask = match budget {
        Some(minutes) => Some(crate::limits::ask_to_pause(minutes)),
        None if hands_over && sessions::handover::of_round(db.conn(), work, round)?.is_some_and(|j| j.due()) => {
            Some(crate::handover::ask())
        }
        None => None,
    };
    Ok(with_ask(said, ask))
}

/// The hook's output: what it hands in, with the engine's ask after it, as the command line reads a hook's output after
/// a tool use.
fn with_ask(said: Option<String>, ask: Option<String>) -> Option<String> {
    let Some(ask) = ask else { return said };
    let mut out: Value = said.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_else(
        || json!({ "hookSpecificOutput": { "hookEventName": "PostToolUse", "additionalContext": "" } }),
    );
    let context = &mut out["hookSpecificOutput"]["additionalContext"];
    let before = context.as_str().unwrap_or_default().to_string();
    *context = Value::String(if before.is_empty() {
        ask
    } else {
        format!("{before}\n\n{ask}")
    });
    Some(out.to_string())
}
