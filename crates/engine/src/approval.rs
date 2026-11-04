//! The owner's approval of a change that touches a path the project's configuration has the owner approve
//! (`owner_approves`, such as `flow/flow.json` for wf-agents): the merge gate (cell [`GATE`]) holds such a change until
//! the owner approved it, which the owner did at the review of the work's design (taken in at cell `D4` as approved),
//! or by replying exactly approved to the gate's ask of the change since the work's last fix. Any other reply goes to
//! the engineer as the owner's words. A general mechanism, which each project sets in its configuration: nothing here
//! is of one project.

use std::path::Path;

use db::rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde_json::{Value, json};

use crate::ENGINEER;
use crate::actions::Ctx;
use crate::work::{Work, rounds_at};

/// The merge gate, whose ask of the owner puts the change to the owner's approval.
pub const GATE: &str = "M1";

/// The cell that takes in the review of a design, whose row `approved` is the owner's approval of the work's design.
const REVIEWED: &str = "D4";

/// The cell that fixes: a fix after the owner answered is a change the owner has not seen.
const FIX: &str = "T6";

/// The gate's row that gives the owner's answer to the engineer.
const RELAYED: &str = "owner_answered";

/// The words that approve a change, exactly.
pub const APPROVED: &str = "approved";

fn sql(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// The files of the work's change against main that match a path of the project's `owner_approves`; none for a project
/// that names none, or a work with no worktree yet.
pub fn touched(p: &config::Project, w: &Work) -> Result<Vec<String>, String> {
    if p.owner_approves.is_empty() {
        return Ok(Vec::new());
    }
    let Some(wt) = w.worktree.as_deref() else {
        return Ok(Vec::new());
    };
    changed_under(wt, &p.main, &p.owner_approves)
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
fn design_approved(conn: &Connection, work: i64) -> Result<bool, String> {
    Ok(rounds_at(conn, work, REVIEWED)?
        .iter()
        .any(|r| r.transition.as_deref() == Some("approved")))
}

/// The gate's latest ask of the owner to approve the change, its asks of merge conditions apart: its id, the number of
/// the round that put it, and the owner's answer once it came.
fn latest_ask(conn: &Connection, work: i64) -> Result<Option<(i64, i64, Option<Value>)>, String> {
    let row: Option<(i64, i64, Option<String>)> = conn
        .query_row(
            &format!(
                "SELECT a.id, a.round, a.answer FROM owner_asks a JOIN rounds r ON r.work = a.work AND r.n = a.round
                 WHERE a.work = ?1 AND r.cell IN {} AND json_extract(a.body, '$.condition') IS NULL
                 ORDER BY a.id DESC LIMIT 1",
                db::cells_sql(ENGINEER, &[GATE])
            ),
            [work],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .map_err(sql)?;
    Ok(row.map(|(id, n, a)| (id, n, crate::work::json_col(a))))
}

/// Whether a round of the cell `cell` came after round `n` of the work.
fn after(conn: &Connection, work: i64, cell: &str, n: i64, transition: Option<&str>) -> Result<bool, String> {
    Ok(rounds_at(conn, work, cell)?
        .iter()
        .any(|r| r.id.n > n && transition.is_none_or(|t| r.transition.as_deref() == Some(t))))
}

/// Where the owner's approval of the change stands, for the merge gate's value `gates` once every item depended on is
/// merged: `clear` when the change needs none or has it; `answered` when the owner answered the ask of this change
/// otherwise than approved, and the answer has not gone to the engineer yet; `owner` when the owner is to be asked, or
/// is asked and has not answered. An ask the engine closed as the work stopped waiting has no answer of the owner's, and
/// is put again.
pub fn gate(conn: &Connection, p: &config::Project, w: &Work) -> Result<&'static str, String> {
    if touched(p, w)?.is_empty() || design_approved(conn, w.id)? {
        return Ok("clear");
    }
    let Some((_, n, answer)) = latest_ask(conn, w.id)? else {
        return Ok("owner");
    };
    let Some(answer) = answer else {
        return Ok("owner");
    };
    if answer["by"] == "engine" {
        return Ok("owner");
    }
    // A fix after the answer is a change the owner has not seen; an answer relayed, one the engineer has.
    if after(conn, w.id, FIX, n, None)? || after(conn, w.id, GATE, n, Some(RELAYED))? {
        return Ok("owner");
    }
    Ok(if answer["text"].as_str().unwrap_or_default().trim() == APPROVED {
        "clear"
    } else {
        "answered"
    })
}

/// The engine's input `owner_approval`: the files the owner approves, and the pull request's files to compare them with
/// main on; whether the owner approved the design at its review. None when the change touches no such path.
pub fn input(conn: &Connection, p: &config::Project, w: &Work) -> Result<Option<Value>, String> {
    let files = touched(p, w)?;
    if files.is_empty() {
        return Ok(None);
    }
    let pr = w.pr.map(|n| format!("https://github.com/{}/pull/{n}/files", p.repo));
    Ok(Some(json!({
        "files": files,
        "pull_request": pr,
        "design_approved": design_approved(conn, w.id)?,
    })))
}

/// `ask_owner_approval`: puts the change before the owner, unless the gate's latest ask waits for its answer, and has
/// the ask stand for the work's blocker.
pub fn ask(ctx: &Ctx, tx: &Transaction<'_>) -> Result<(), String> {
    let w = &ctx.work;
    let item = ctx
        .item
        .as_ref()
        .ok_or_else(|| format!("work {} asks the owner's approval, and is on no item", w.id))?;
    let p = ctx.project()?;
    let id = match latest_ask(tx, w.id)? {
        Some((id, _, None)) => id,
        _ => {
            let change = input(tx, p, w)?.unwrap_or(Value::Null);
            let files: Vec<String> = change["files"]
                .as_array()
                .map(|fs| fs.iter().filter_map(|f| f.as_str().map(str::to_string)).collect())
                .unwrap_or_default();
            let body = json!({
                "to": "owner",
                "work": w.id,
                "item": item.id,
                "title": item.title,
                "asked": [{ "what": "The change to approve before it merges", "value": change }],
            });
            tx.execute(
                "INSERT INTO owner_asks (work, round, body) VALUES (?1, ?2, ?3)",
                params![w.id, ctx.round.n, body.to_string()],
            )
            .map_err(sql)?;
            let id = tx.last_insert_rowid();
            let compare = change["pull_request"]
                .as_str()
                .map(|u| format!(" Compare them with main: {u}."))
                .unwrap_or_default();
            let text = format!(
                "Work {} on item {} ({}) changes what you approve before it merges: {}.{compare} Reply exactly {APPROVED} \
                 to approve it; any other reply goes to its engineer. (ask {id})",
                w.id,
                item.id,
                item.title,
                files.join(", ")
            );
            crate::actions::notify_ask(tx, w.id, id, &text)?;
            id
        }
    };
    if let Some(b) = crate::blockers::open(tx, w.id)? {
        crate::blockers::set_ask(tx, b.id, id)?;
        // The blocker stays open from a merge condition's ask to this one, and says what it waits on now.
        let what = format!(
            "{} waits for the owner's approval of the change before it merges",
            crate::blockers::about(ctx)
        );
        crate::blockers::set_what(tx, b.id, &what)?;
    }
    Ok(())
}

/// `relay_owner_answer`: gives the answer to the gate's ask the row is for, of the change or of a merge condition, to the
/// engineer as the owner's words, saying what was asked and who answered, since a delegate may have.
pub fn relay(ctx: &Ctx, tx: &Transaction<'_>) -> Result<(), String> {
    let w = &ctx.work;
    let asked = if ctx.row.name == crate::conditions::ANSWERED {
        crate::conditions::answered(ctx, tx)?
    } else {
        match latest_ask(tx, w.id)? {
            Some((id, _, Some(answer))) => {
                Some((id, answer, "Asked to approve the change before it merges".to_string()))
            }
            _ => None,
        }
    };
    let Some((id, answer, what)) = asked else {
        return Ok(());
    };
    let images = db::attachments::of(tx, db::attachments::Words::Ask(id)).map_err(sql)?;
    let text = db::attachments::with_named(answer["text"].as_str().unwrap_or_default(), &images);
    let declared = answer["speaker"]
        .as_str()
        .filter(|s| matches!(*s, "owner" | "delegate"));
    let who = match declared {
        Some("delegate") => "the delegate that speaks for the owner",
        _ => "the owner",
    };
    let words = match answer["choice"].as_str() {
        Some(choice) if text.trim().is_empty() => format!("chose {choice}"),
        Some(choice) => format!("chose {choice}: {text}"),
        None => text,
    };
    tx.execute(
        "INSERT INTO interjections (work, line, speaker, declared, text) VALUES (?1, 'engineer', 'owner', ?2, ?3)",
        params![w.id, declared, format!("{what}, {who} replied: {words}")],
    )
    .map_err(sql)?;
    Ok(())
}
