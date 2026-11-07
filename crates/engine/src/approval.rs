//! The owner's approvals a work asks for while it goes on, and the merge gate (cell [`GATE`]) that waits for them: the
//! decision points and departures the research leaves (asked as the work starts implementing, [`ask_points`]), the
//! review of its design (see `review`), and the approval of a change that touches a path the project's configuration
//! has the owner approve (`owner_approves`, such as `flow/flow.json` for wf-agents), asked as soon as the work's branch
//! touches one ([`ask_early`]). The work does not wait for any of them until the gate, which holds the merge while one
//! has no answer. The change is approved by the owner's approval of the work's design, or by replying exactly approved
//! to its ask; any other reply goes to the engineer as the owner's words, and the change is asked again. A general
//! mechanism, which each project sets in its configuration: nothing here is of one project.

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

/// The kind of the owner's ask of the change, put before the merge gate.
const CHANGE: &str = "change";

/// The kind of the owner's ask of the decision points and departures the research left.
pub const POINTS: &str = "points";

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

/// Whether the owner approved the work's design at its review: answered it exactly approved, or, on an older version
/// of the flow, the review was taken in as approved.
fn design_approved(conn: &Connection, work: i64) -> Result<bool, String> {
    Ok(crate::review::approved(conn, work)?
        || rounds_at(conn, work, REVIEWED)?
            .iter()
            .any(|r| r.transition.as_deref() == Some("approved")))
}

/// An ask of the change: its id, the number of the round that put it, the owner's answer once it came, and whether the
/// answer went to the engineer as the owner's words when it came.
type ChangeAsk = (i64, i64, Option<Value>, bool);

/// The latest ask of the change.
fn latest_ask(conn: &Connection, work: i64) -> Result<Option<ChangeAsk>, String> {
    let row: Option<(i64, i64, Option<String>, bool)> = conn
        .query_row(
            &format!(
                "SELECT a.id, a.round, a.answer, json_extract(a.body, '$.relayed') IS NOT NULL FROM owner_asks a
                 JOIN rounds r ON r.work = a.work AND r.n = a.round
                 WHERE a.work = ?1 AND (r.cell IN {} OR json_extract(a.body, '$.kind') = ?2) ORDER BY a.id DESC LIMIT 1",
                db::cells_sql(ENGINEER, &[GATE])
            ),
            params![work, CHANGE],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()
        .map_err(sql)?;
    Ok(row.map(|(id, n, a, relayed)| (id, n, crate::work::json_col(a), relayed)))
}

/// The work's asks of the decision points and departures the research left that have no answer yet, oldest first.
fn unanswered_points(conn: &Connection, work: i64) -> Result<Vec<i64>, String> {
    let mut st = conn
        .prepare(
            "SELECT id FROM owner_asks WHERE work = ?1 AND answer IS NULL AND json_extract(body, '$.kind') = ?2
             ORDER BY id",
        )
        .map_err(sql)?;
    st.query_map(params![work, POINTS], |r| r.get(0))
        .map_err(sql)?
        .collect::<Result<_, _>>()
        .map_err(sql)
}

/// Whether a round of the cell `cell` came after round `n` of the work.
fn after(conn: &Connection, work: i64, cell: &str, n: i64, transition: Option<&str>) -> Result<bool, String> {
    Ok(rounds_at(conn, work, cell)?
        .iter()
        .any(|r| r.id.n > n && transition.is_none_or(|t| r.transition.as_deref() == Some(t))))
}

/// What the merge gate waits for of the owner once every item depended on is merged and every finding set aside is
/// judged: `approvals` while the review of the work's design or an ask of the decision points the research left has no
/// answer; then, for a change that touches a path the owner approves, `clear` when the owner approved it, `answered`
/// when the owner answered its ask otherwise than approved and the answer has not gone to the engineer yet, and `owner`
/// when the owner is to be asked, or is asked and has not answered; `clear` otherwise.
pub fn gate(conn: &Connection, p: &config::Project, w: &Work) -> Result<&'static str, String> {
    if crate::review::waits(conn, w.id)?.is_some() || !unanswered_points(conn, w.id)?.is_empty() {
        return Ok("approvals");
    }
    if touched(p, w)?.is_empty() || design_approved(conn, w.id)? {
        return Ok("clear");
    }
    let Some((_, n, answer, relayed)) = latest_ask(conn, w.id)? else {
        return Ok("owner");
    };
    let Some(answer) = answer else {
        return Ok("owner");
    };
    if answer["text"].as_str().unwrap_or_default().trim() == APPROVED {
        return Ok("clear");
    }
    // An answer that went to the engineer, when it came or from this gate, is one the engineer has: the change it
    // made since is asked again.
    if relayed || after(conn, w.id, GATE, n, Some(RELAYED))? {
        return Ok("owner");
    }
    Ok("answered")
}

/// Whether the owner's approval of the change is to be asked now: it touches a path the owner approves, neither the
/// design's approval nor an answer of approved covers it, and no ask of it waits for its answer or for the engineer to
/// have the answer.
fn change_to_ask(conn: &Connection, p: &config::Project, w: &Work) -> Result<bool, String> {
    if touched(p, w)?.is_empty() || design_approved(conn, w.id)? {
        return Ok(false);
    }
    Ok(match latest_ask(conn, w.id)? {
        None => true,
        Some((_, _, None, _)) => false,
        Some((_, n, Some(a), relayed)) => {
            a["text"].as_str().unwrap_or_default().trim() != APPROVED
                && (relayed || after(conn, w.id, GATE, n, Some(RELAYED))?)
        }
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

/// `ask_owner_approval`: puts the change before the owner, unless its latest ask waits for its answer, and has the ask
/// stand for the work's blocker.
pub fn ask(ctx: &Ctx, tx: &Transaction<'_>) -> Result<(), String> {
    let w = &ctx.work;
    let id = match latest_ask(tx, w.id)? {
        Some((id, _, None, _)) => id,
        _ => put_change_ask(ctx, tx)?,
    };
    if let Some(b) = crate::blockers::open(tx, w.id)? {
        crate::blockers::set_ask(tx, b.id, id)?;
    }
    Ok(())
}

/// `ask_change_approval`: once the work's branch touches a path the owner approves, puts the change before the owner at
/// once, so that the answer can come while the work goes on; the work does not wait for it before the merge gate.
pub fn ask_early(ctx: &Ctx, tx: &Transaction<'_>) -> Result<(), String> {
    let Ok(p) = ctx.project() else {
        return Ok(());
    };
    if ctx.item.is_none() || !change_to_ask(tx, p, &ctx.work)? {
        return Ok(());
    }
    put_change_ask(ctx, tx).map(|_| ())
}

/// Puts the change before the owner as an ask of the round, and notifies the owner; gives the ask.
fn put_change_ask(ctx: &Ctx, tx: &Transaction<'_>) -> Result<i64, String> {
    let w = &ctx.work;
    let item = ctx
        .item
        .as_ref()
        .ok_or_else(|| format!("work {} asks the owner's approval, and is on no item", w.id))?;
    let p = ctx.project()?;
    let change = input(tx, p, w)?.unwrap_or(Value::Null);
    let files: Vec<String> = change["files"]
        .as_array()
        .map(|fs| fs.iter().filter_map(|f| f.as_str().map(str::to_string)).collect())
        .unwrap_or_default();
    let body = json!({
        "to": "owner",
        "kind": CHANGE,
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
        "Work {} on item {} ({}) changes what you approve before it merges: {}.{compare} Reply exactly {APPROVED} to \
         approve it; any other reply goes to its engineer. The work goes on meanwhile, and merges once you approved. \
         (ask {id})",
        w.id,
        item.id,
        item.title,
        files.join(", ")
    );
    crate::hub::tell_of_ask(tx, w.id, id, &text)?;
    Ok(id)
}

/// `ask_owner_points`: puts the decision points and the departures from decisions the research left before the owner
/// as one ask, as the work starts implementing by what the research recommends; nothing when it left none.
pub fn ask_points(ctx: &Ctx, tx: &Transaction<'_>) -> Result<(), String> {
    let w = &ctx.work;
    let Some(item) = ctx.item.as_ref() else {
        return Ok(());
    };
    let out = &ctx.output[ctx.within.as_str()];
    let points = out["points"].as_array().cloned().unwrap_or_default();
    let departs = (out["departs"]["verdict"] == "yes").then(|| out["departs"].clone());
    if points.is_empty() && departs.is_none() {
        return Ok(());
    }
    let mut asked = Vec::new();
    if !points.is_empty() {
        asked.push(json!({
            "what": "The decision points the research left, which the work follows by its recommendation until you \
                     choose",
            "value": points,
        }));
    }
    if let Some(d) = departs {
        asked.push(json!({
            "what": "Where the change departs from a decision, which the work does meanwhile as the research \
                     recommends",
            "value": d,
        }));
    }
    let link = format!("/works/{}?round=R{}-{}", w.id, ctx.round.work, ctx.round.n);
    let body = json!({
        "to": "owner",
        "kind": POINTS,
        "work": w.id,
        "item": item.id,
        "title": item.title,
        "asked": asked,
        "research_link": link,
    });
    tx.execute(
        "INSERT INTO owner_asks (work, round, body) VALUES (?1, ?2, ?3)",
        params![w.id, ctx.round.n, body.to_string()],
    )
    .map_err(sql)?;
    let id = tx.last_insert_rowid();
    let text = format!(
        "The research of work {} on item {} ({}) left you to choose: choose an option for each decision point and rule \
         on each departure, or reply exactly {APPROVED} to take what it recommends. The work implements by the \
         recommendation meanwhile, and merges once you answered. The research: {link}. (ask {id})",
        w.id, item.id, item.title
    );
    crate::hub::tell_of_ask(tx, w.id, id, &text)
}

/// `hold_for_approvals`: has the work's blocker at the merge gate stand for what it waits for: the architect's review of
/// its design, then the owner's answer to the review, then the oldest ask of the decision points without an answer.
pub fn hold(ctx: &Ctx, tx: &Transaction<'_>) -> Result<(), String> {
    let w = ctx.work.id;
    let Some(b) = crate::blockers::open(tx, w)? else {
        return Ok(());
    };
    if let Some(m) = crate::review::pending_matter(tx, w)? {
        return crate::blockers::set_matter(tx, b.id, m);
    }
    let ask = match crate::review::ask_owner_due(tx, w)? {
        Some(a) => Some(a),
        None => unanswered_points(tx, w)?.first().copied(),
    };
    match ask {
        Some(a) => crate::blockers::set_ask(tx, b.id, a),
        None => Ok(()),
    }
}

/// `relay_owner_answer`: gives the owner's answer to the gate's latest ask to the engineer, as the owner's words.
pub fn relay(ctx: &Ctx, tx: &Transaction<'_>) -> Result<(), String> {
    let w = &ctx.work;
    let Some((id, _, Some(answer), _)) = latest_ask(tx, w.id)? else {
        return Ok(());
    };
    let images = db::attachments::of(tx, db::attachments::Words::Ask(id)).map_err(sql)?;
    let text = db::attachments::with_named(answer["text"].as_str().unwrap_or_default(), &images);
    let declared = answer["speaker"]
        .as_str()
        .filter(|s| matches!(*s, "owner" | "delegate"));
    let relayed = format!("{} {text}", db::RELAY_APPROVAL);
    let relay = db::ToLine {
        speaker: declared,
        ..db::ToLine::new(db::Sender::Owner, w.id, "engineer", &relayed)
    };
    db::say_to_line(tx, &relay, None).map_err(sql)?;
    Ok(())
}
