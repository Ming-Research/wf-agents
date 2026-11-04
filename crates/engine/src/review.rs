//! The review of a work's design, which deep research ends with: first the architect's, who reviews the design at the
//! project's level only (whether it fits the plan, the direction and the short-term goals, its scope, whether to split
//! it and how it is ordered with other items), then the owner's, who is given the design with the architect's comments,
//! approves it or says what to change, and chooses an option for each decision point. The work does not wait for
//! either: cell [`CELL`] puts the design to the architect and the work implements by it; once the architect replied the
//! design goes to the owner ([`after_reply`]), and the merge gate waits for the owner's answer ([`waits`]). An answer
//! other than approved reaches the engineer as the owner's words. The architect's review is a matter of its queue, an
//! engineer's question of the kind [`KIND`], which its reply answers; the owner's, an ask of the owner in the work's
//! group, of the kind [`ASK`]. Each design is reviewed once: a design written again after the review is reviewed
//! again.

use db::attachments::{self, Words};
use db::rusqlite::{Connection, OptionalExtension, Transaction, params};
use db::{Matter, queue_matter};
use serde_json::{Value, json};

use crate::ENGINEER;
use crate::actions::Ctx;

/// The engineer's cell that puts a design to its review.
pub const CELL: &str = "D3";

/// The engineer's cell whose return is the design reviewed: thinking again outside the frame hands it in whole.
const DESIGN: &str = "D2";

/// The detail of the engineer's question that asks the architect for its review.
pub const KIND: &str = "design_review";

/// The kind of the owner's ask that puts the design to the owner, which the work does not wait for.
pub const ASK: &str = "design_review";

fn sql(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// The architect's matter that reviews a design, as the database has it.
struct Asked {
    id: i64,
    reply: Option<Value>,
    withdrawn: bool,
    /// The work's round that queued it.
    round: i64,
}

/// The number of the work's latest round of the design, which the review is of; none before the first.
fn design_round(conn: &Connection, work: i64) -> Result<Option<i64>, String> {
    conn.query_row(
        &format!(
            "SELECT max(n) FROM rounds WHERE work = ?1 AND cell IN {} AND transition IS NOT NULL",
            db::cells_sql(ENGINEER, &[DESIGN])
        ),
        [work],
        |r| r.get(0),
    )
    .map_err(sql)
}

/// The latest matter that asked the architect to review the design the work handed in at round `since` or after.
fn asked(conn: &Connection, work: i64, since: i64) -> Result<Option<Asked>, String> {
    let row: Option<(i64, Option<String>, bool, i64)> = conn
        .query_row(
            "SELECT id, reply, withdrawn IS NOT NULL, round FROM architect_matters
             WHERE work = ?1 AND kind = 'engineer_question' AND detail = ?2 AND round > ?3
             ORDER BY id DESC LIMIT 1",
            params![work, KIND, since],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()
        .map_err(sql)?;
    Ok(row.map(|(id, reply, withdrawn, round)| Asked {
        id,
        reply: crate::work::json_col(reply),
        withdrawn,
        round,
    }))
}

/// The latest ask of the owner that put the design to the owner at round `since` or after, with its answer if it came.
fn owner_ask(conn: &Connection, work: i64, since: i64) -> Result<Option<(i64, Option<Value>)>, String> {
    let row: Option<(i64, Option<String>)> = conn
        .query_row(
            &format!(
                "SELECT a.id, a.answer FROM owner_asks a JOIN rounds r ON r.work = a.work AND r.n = a.round
                 WHERE a.work = ?1 AND r.cell IN {} AND a.round >= ?2 ORDER BY a.id DESC LIMIT 1",
                db::cells_sql(ENGINEER, &[CELL])
            ),
            params![work, since],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(sql)?;
    Ok(row.map(|(id, a)| (id, crate::work::json_col(a))))
}

/// The engine's value `review`: where the review of the work's latest design stands. `architect` while the architect
/// has not replied to it, or the question was withdrawn and is to be queued again; `owner` once it replied, until the
/// owner answers the ask that put the design to the owner; `answered` then.
pub fn stage(conn: &Connection, work: i64) -> Result<&'static str, String> {
    let since = design_round(conn, work)?.unwrap_or(0);
    let Some(m) = asked(conn, work, since)? else {
        return Ok("architect");
    };
    if m.reply.is_none() {
        return Ok("architect");
    }
    Ok(match owner_ask(conn, work, m.round)? {
        Some((_, Some(_))) => "answered",
        _ => "owner",
    })
}

/// Who holds the blocker of a work at the review: the architect until it replied, the owner after.
pub fn holder(conn: &Connection, work: i64) -> Result<&'static str, String> {
    Ok(match stage(conn, work)? {
        "architect" => "architect",
        _ => "owner",
    })
}

/// The design reviewed, as the round of thinking again outside the frame handed it in: the design and what that round
/// examined; with the address of the round, which the owner's page opens.
fn design(tx: &Transaction<'_>, work: i64, n: i64, within: &str) -> Result<Value, String> {
    let output: Option<String> = tx
        .query_row(
            "SELECT output FROM rounds WHERE work = ?1 AND n = ?2",
            params![work, n],
            |r| r.get(0),
        )
        .optional()
        .map_err(sql)?
        .flatten();
    let output = crate::work::json_col(output).unwrap_or(Value::Null);
    Ok(json!({
        "design": output[within]["design"],
        "examined": output[within]["examined"],
        "changed": output[within]["changed"],
        "link": format!("/works/{work}?round=R{work}-{n}"),
    }))
}

/// `post_design_review`: queues the latest design for the architect's review, unless a review of it is queued and not
/// answered nor withdrawn, and has the matter stand for the work's blocker.
pub fn post(ctx: &Ctx, tx: &Transaction<'_>) -> Result<(), String> {
    let work = ctx.work.id;
    let item = ctx
        .item
        .as_ref()
        .ok_or_else(|| format!("work {work} reviews a design, and is on no item"))?;
    let since = design_round(tx, work)?.ok_or_else(|| format!("work {work} has no design to review"))?;
    let live = asked(tx, work, since)?.filter(|m| m.reply.is_none() && !m.withdrawn);
    let matter = match live {
        Some(m) => m.id,
        None => {
            let reviewed = design(tx, work, since, &ctx.within)?;
            let body = json!({
                "to": "architect",
                "work": work,
                "item": item.id,
                "title": item.title,
                "for": { "project": item.project, "work": work },
                "asked": [{
                    "what": "The design to review at the project's level, before the owner reviews it with your \
                             comments: whether it fits the plan, the direction and the short-term goals, whether its \
                             scope is right, whether it must be split, and how it is ordered with other items",
                    "value": reviewed,
                }],
                "design_link": reviewed["link"],
                "within": ctx.within,
            });
            let m = Matter {
                project: item.project.clone(),
                kind: "engineer_question".into(),
                detail: Some(KIND.into()),
                what: format!(
                    "Work {work} on item {} ({}): its design waits for your review at the project's level, before \
                     the owner's",
                    item.id, item.title
                ),
                body,
                item: Some(item.id),
                raised: Some((work, ctx.round.n)),
            };
            queue_matter(tx, &m).map_err(sql)?
        }
    };
    if let Some(b) = crate::blockers::open(tx, work)? {
        crate::blockers::set_matter(tx, b.id, matter)?;
    }
    Ok(())
}

/// `ask_owner_review`: puts the latest design before the owner with the architect's comments, unless an ask of it is
/// put and not answered, and has the ask stand for the work's blocker if one is open.
pub fn ask_owner(ctx: &Ctx, tx: &Transaction<'_>) -> Result<(), String> {
    let work = ctx.work.id;
    let item = ctx
        .item
        .as_ref()
        .ok_or_else(|| format!("work {work} reviews a design, and is on no item"))?;
    let ask = put_owner_ask(tx, work, item.id, &item.title, ctx.round.n, &ctx.within)?;
    if let Some(b) = crate::blockers::open(tx, work)? {
        crate::blockers::set_ask(tx, b.id, ask)?;
    }
    Ok(())
}

/// Puts the latest design before the owner with the architect's comments as an ask of round `n` of the work, unless an
/// ask of it is put and not answered; gives the ask.
fn put_owner_ask(tx: &Transaction<'_>, work: i64, item: i64, title: &str, n: i64, within: &str) -> Result<i64, String> {
    let since = design_round(tx, work)?.ok_or_else(|| format!("work {work} has no design to review"))?;
    let m = asked(tx, work, since)?
        .filter(|m| m.reply.is_some())
        .ok_or_else(|| format!("work {work}: the architect has not reviewed its design yet"))?;
    if let Some((id, None)) = owner_ask(tx, work, m.round)? {
        return Ok(id);
    }
    let reviewed = design(tx, work, since, within)?;
    let comments = m.reply.clone().unwrap_or(Value::Null);
    let body = json!({
        "to": "owner",
        "kind": ASK,
        "work": work,
        "item": item,
        "title": title,
        "asked": [
            { "what": "The design to review", "value": reviewed },
            { "what": "The architect's comments, at the project's level", "value": comments },
        ],
        "design_link": reviewed["link"],
    });
    tx.execute(
        "INSERT INTO owner_asks (work, round, body) VALUES (?1, ?2, ?3)",
        params![work, n, body.to_string()],
    )
    .map_err(sql)?;
    let id = tx.last_insert_rowid();
    let said = comments["text"]
        .as_str()
        .map(|t| format!(" The architect said: {}", db::one_sentence(t)))
        .unwrap_or_default();
    let text = format!(
        "The design of work {work} on item {item} ({title}) waits for your review: reply exactly approved to approve it, \
         or say what to change and choose an option for each decision point. The work implements by it meanwhile, and \
         merges once you answered. The design: {}.{said} (ask {id})",
        reviewed["link"].as_str().unwrap_or_default()
    );
    crate::actions::notify_ask(tx, work, id, &text)?;
    Ok(id)
}

/// Once the architect replied to the review of a design whose work went on to implement, puts the design to the owner
/// in the work's latest round at the review's cell; a matter of another kind is left alone.
pub fn after_reply(tx: &Transaction<'_>, matter: i64) -> Result<(), String> {
    let row: Option<(Option<i64>, Option<String>)> = tx
        .query_row(
            "SELECT work, detail FROM architect_matters WHERE id = ?1 AND kind = 'engineer_question'",
            [matter],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(sql)?;
    match row {
        Some((Some(work), Some(detail))) if detail == KIND => ask_owner_due(tx, work).map(|_| ()),
        _ => Ok(()),
    }
}

/// Puts the latest design to the owner once the architect replied to its review and no ask of it was put, unless the
/// work waits at the review's cell, whose rows put it there (an older version of the flow); gives the ask of the
/// review that waits for the owner's answer, if any.
pub fn ask_owner_due(tx: &Transaction<'_>, work: i64) -> Result<Option<i64>, String> {
    let Some(since) = design_round(tx, work)? else {
        return Ok(None);
    };
    let Some(m) = asked(tx, work, since)?.filter(|m| m.reply.is_some()) else {
        return Ok(None);
    };
    match owner_ask(tx, work, m.round)? {
        Some((id, None)) => return Ok(Some(id)),
        Some((_, Some(_))) => return Ok(None),
        None => {}
    }
    if crate::blockers::open(tx, work)?.is_some_and(|b| b.cell == CELL) {
        return Ok(None);
    }
    let body: String = tx
        .query_row("SELECT body FROM architect_matters WHERE id = ?1", [m.id], |r| r.get(0))
        .map_err(sql)?;
    let body: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
    let (Some(item), Some(n)) = (body["item"].as_i64(), review_round(tx, work)?) else {
        return Ok(None);
    };
    let title = body["title"].as_str().unwrap_or_default();
    let within = body["within"].as_str().unwrap_or("outcome");
    put_owner_ask(tx, work, item, title, n, within).map(Some)
}

/// The work's latest round at the review's cell.
fn review_round(conn: &Connection, work: i64) -> Result<Option<i64>, String> {
    conn.query_row(
        &format!(
            "SELECT max(n) FROM rounds WHERE work = ?1 AND cell IN {}",
            db::cells_sql(ENGINEER, &[CELL])
        ),
        [work],
        |r| r.get(0),
    )
    .map_err(sql)
}

/// What the merge gate waits for of the review of the work's latest design: `architect` while the architect has not
/// replied to it, `owner` while the owner has not answered; none when the work wrote no design, or the owner answered.
pub fn waits(conn: &Connection, work: i64) -> Result<Option<&'static str>, String> {
    if design_round(conn, work)?.is_none() {
        return Ok(None);
    }
    Ok(match stage(conn, work)? {
        "answered" => None,
        s => Some(s),
    })
}

/// The matter that holds the review of the latest design for the merge gate while the architect has not replied.
pub fn pending_matter(conn: &Connection, work: i64) -> Result<Option<i64>, String> {
    let since = design_round(conn, work)?.unwrap_or(0);
    Ok(asked(conn, work, since)?
        .filter(|m| m.reply.is_none() && !m.withdrawn)
        .map(|m| m.id))
}

/// Whether the owner approved the work's latest design at its review: answered exactly approved.
pub fn approved(conn: &Connection, work: i64) -> Result<bool, String> {
    let since = design_round(conn, work)?.unwrap_or(0);
    let Some(m) = asked(conn, work, since)?.filter(|m| m.reply.is_some()) else {
        return Ok(false);
    };
    Ok(match owner_ask(conn, work, m.round)? {
        Some((_, Some(a))) => a["text"].as_str().unwrap_or_default().trim() == crate::approval::APPROVED,
        _ => false,
    })
}

/// The engine's input `design_review`: what the architect replied to the review of the latest design, and the owner's
/// answer, ending with the line that names the images the owner sent with it; none before the architect replied.
pub fn input(conn: &Connection, work: i64) -> Result<Option<Value>, String> {
    let since = design_round(conn, work)?.unwrap_or(0);
    let Some(m) = asked(conn, work, since)?.filter(|m| m.reply.is_some()) else {
        return Ok(None);
    };
    let owner = match owner_ask(conn, work, m.round)? {
        Some((id, Some(answer))) => {
            let images = attachments::of(conn, Words::Ask(id)).map_err(sql)?;
            let text = attachments::with_named(answer["text"].as_str().unwrap_or_default(), &images);
            json!({ "text": text, "by": answer["by"], "speaker": answer["speaker"] })
        }
        _ => Value::Null,
    };
    Ok(Some(json!({ "architect": m.reply, "owner": owner })))
}
