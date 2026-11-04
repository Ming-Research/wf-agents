//! The review of a work's design, at cell [`CELL`], where deep research ends: the work stops there, first for the
//! architect, who reviews the design at the project's level only (whether it fits the plan, the direction and the
//! short-term goals, its scope, whether to split it and how it is ordered with other items), then for the owner, who is
//! given the design with the architect's comments, approves it, says what to change or says to split it, and chooses an
//! option for each decision point. The architect's review is a matter of its queue, an engineer's question of the kind
//! [`KIND`], which its reply answers; the owner's, an ask of the owner in the work's group. Each design is reviewed
//! once: a design written again after the review is reviewed again.

use db::attachments::{self, Words};
use db::rusqlite::{Connection, OptionalExtension, Transaction, params};
use db::{Matter, queue_matter};
use serde_json::{Value, json};

use crate::ENGINEER;
use crate::actions::Ctx;

/// The engineer's cell where a design waits for its review.
pub const CELL: &str = "D3";

/// The engineer's cell whose return is the design reviewed: thinking again outside the frame hands it in whole.
const DESIGN: &str = "D2";

/// The detail of the engineer's question that asks the architect for its review.
pub const KIND: &str = "design_review";

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

/// The latest ask of the owner that put the design to the owner after round `since`, with its answer if it came.
fn owner_ask(conn: &Connection, work: i64, since: i64) -> Result<Option<(i64, Option<Value>)>, String> {
    let row: Option<(i64, Option<String>)> = conn
        .query_row(
            &format!(
                "SELECT a.id, a.answer FROM owner_asks a JOIN rounds r ON r.work = a.work AND r.n = a.round
                 WHERE a.work = ?1 AND r.cell IN {} AND a.round > ?2 ORDER BY a.id DESC LIMIT 1",
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
/// put and not answered, and has the ask stand for the work's blocker.
pub fn ask_owner(ctx: &Ctx, tx: &Transaction<'_>) -> Result<(), String> {
    let work = ctx.work.id;
    let item = ctx
        .item
        .as_ref()
        .ok_or_else(|| format!("work {work} reviews a design, and is on no item"))?;
    let since = design_round(tx, work)?.ok_or_else(|| format!("work {work} has no design to review"))?;
    let m = asked(tx, work, since)?
        .filter(|m| m.reply.is_some())
        .ok_or_else(|| format!("work {work}: the architect has not reviewed its design yet"))?;
    let ask = match owner_ask(tx, work, m.round)? {
        Some((id, None)) => id,
        _ => {
            let reviewed = design(tx, work, since, &ctx.within)?;
            let comments = m.reply.clone().unwrap_or(Value::Null);
            let body = json!({
                "to": "owner",
                "work": work,
                "item": item.id,
                "title": item.title,
                "asked": [
                    { "what": "The design to review", "value": reviewed },
                    { "what": "The architect's comments, at the project's level", "value": comments },
                ],
                "design_link": reviewed["link"],
            });
            tx.execute(
                "INSERT INTO owner_asks (work, round, body) VALUES (?1, ?2, ?3)",
                params![work, ctx.round.n, body.to_string()],
            )
            .map_err(sql)?;
            let id = tx.last_insert_rowid();
            let said = comments["text"]
                .as_str()
                .map(|t| format!(" The architect said: {}", db::one_sentence(t)))
                .unwrap_or_default();
            let text = format!(
                "The design of work {work} on item {} ({}) waits for your review: approve it, say what to change, or \
                 say to split it along its parts, and choose an option for each decision point. The design: {}.{said} \
                 (ask {id})",
                item.id,
                item.title,
                reviewed["link"].as_str().unwrap_or_default()
            );
            crate::actions::notify_ask(tx, work, id, &text)?;
            id
        }
    };
    if let Some(b) = crate::blockers::open(tx, work)? {
        crate::blockers::set_ask(tx, b.id, ask)?;
    }
    Ok(())
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
