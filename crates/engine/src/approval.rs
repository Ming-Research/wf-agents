//! The owner's approval of a change that touches a path the project's configuration has the owner approve
//! (`owner_approves`, such as `flow/flow.json` for wf-agents): the merge gate (cell `M1`) holds such a change until the
//! owner approved it, which the owner did at the review of the work's design (cell `D4`, taken in by `D5`), or by
//! replying exactly approved to the ask of cell [`APPROVE`] since the work's last fix. A general mechanism, which each
//! project sets in its configuration: nothing here is of one project.

use std::path::Path;

use db::rusqlite::OptionalExtension;
use serde_json::{Value, json};

use crate::Engine;
use crate::work::{Work, rounds_at};

/// The owner's cell where a change waits for the owner's approval.
pub const APPROVE: &str = "M3";

/// The cell that takes in the review of a design, whose row `approved` is the owner's approval of the work's design.
const REVIEWED: &str = "D5";

/// The cell that fixes: a fix after the owner approved a change is a change the owner has not seen.
const FIX: &str = "T6";

/// The words that approve a change, exactly.
pub const APPROVED: &str = "approved";

fn sql(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// The files of the work's change against main that match a path of the project's `owner_approves`; none for a project
/// that names none, or a work with no worktree yet.
pub fn touched(e: &Engine, w: &Work) -> Result<Vec<String>, String> {
    let project = e.project_of(w)?;
    if project.owner_approves.is_empty() {
        return Ok(Vec::new());
    }
    let Some(wt) = w.worktree.as_deref() else {
        return Ok(Vec::new());
    };
    changed_under(wt, &project.main, &project.owner_approves)
}

/// The files the branch in `wt` changes against the commit it forked from `main`, that lie under `paths`.
fn changed_under(wt: &Path, main: &str, paths: &[String]) -> Result<Vec<String>, String> {
    let fork = crate::git::fork_point(wt, main)?;
    Ok(crate::git::changed(wt, &fork, "HEAD")?
        .into_iter()
        .filter(|f| crate::git::under(f, paths))
        .collect())
}

/// Whether the owner approved the work's design at its review: the review was taken in as approved.
fn design_approved(e: &Engine, w: &Work) -> Result<bool, String> {
    Ok(rounds_at(e.db.conn(), w.id, REVIEWED)?
        .iter()
        .any(|r| r.transition.as_deref() == Some("approved")))
}

/// The owner's answer to the latest ask of cell [`APPROVE`], with the number of the round that put it; none before
/// the first, or while it waits.
fn latest_answer(e: &Engine, w: &Work) -> Result<Option<(i64, String)>, String> {
    let row: Option<(i64, Option<String>)> = e
        .db
        .conn()
        .query_row(
            &format!(
                "SELECT a.round, a.answer FROM owner_asks a JOIN rounds r ON r.work = a.work AND r.n = a.round
                 WHERE a.work = ?1 AND r.cell IN {} ORDER BY a.id DESC LIMIT 1",
                db::cells_sql(crate::ENGINEER, &[APPROVE])
            ),
            [w.id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(sql)?;
    Ok(row.and_then(|(n, a)| {
        let a: Value = serde_json::from_str(&a?).ok()?;
        Some((n, a["text"].as_str().unwrap_or_default().to_string()))
    }))
}

/// Whether the owner replied exactly approved to the latest ask of cell [`APPROVE`]: the engine's value
/// `owner_approved`.
pub fn owner_approved(e: &Engine, w: &Work) -> Result<bool, String> {
    Ok(latest_answer(e, w)?.is_some_and(|(_, t)| t.trim() == APPROVED))
}

/// Whether the owner approved the change as it stands: its design at the review, or exactly this change, since no fix
/// came after the ask the owner approved.
pub fn approved(e: &Engine, w: &Work) -> Result<bool, String> {
    if design_approved(e, w)? {
        return Ok(true);
    }
    let Some((asked, text)) = latest_answer(e, w)? else {
        return Ok(false);
    };
    if text.trim() != APPROVED {
        return Ok(false);
    }
    let fixed_after = rounds_at(e.db.conn(), w.id, FIX)?.iter().any(|r| r.id.n > asked);
    Ok(!fixed_after)
}

/// Whether the merge gate waits for the owner's approval: the change touches a path the owner approves, and the owner
/// has not approved it.
pub fn due(e: &Engine, w: &Work) -> Result<bool, String> {
    Ok(!touched(e, w)?.is_empty() && !approved(e, w)?)
}

/// The engine's input `owner_approval`: the files the owner approves, and the pull request's files to compare them with
/// main on; whether the owner approved the design at its review, or this change since. None when the change touches no
/// such path.
pub fn input(e: &Engine, w: &Work) -> Result<Option<Value>, String> {
    let files = touched(e, w)?;
    if files.is_empty() {
        return Ok(None);
    }
    let project = e.project_of(w)?;
    let pr = w
        .pr
        .map(|n| format!("https://github.com/{}/pull/{n}/files", project.repo));
    Ok(Some(json!({
        "files": files,
        "pull_request": pr,
        "design_approved": design_approved(e, w)?,
        "change_approved": approved(e, w)?,
        "reply": format!("Reply exactly {APPROVED} to approve; anything else goes to the work's engineer."),
    })))
}
