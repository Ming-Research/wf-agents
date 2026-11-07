//! Open questions: what a design could not settle by measuring or reading, each with its options and the one it
//! recommends. Once the checks after design passed each goes to the project's architect as a matter of its own, which
//! takes the path an engineer's question takes ([`post`]); the work does not wait, and goes on by what the design
//! recommends. A place the design departs from a decision of its item goes the same way, as an open question of its
//! own, and that one the work waits on before it implements, at cell [`RULING`], for the ruling of whoever made the
//! decision ([`ruling_due`]). The answer is kept in the matter's body, and reaches the engineer as an interjection,
//! which turns the work ([`answer`]). The merge waits until every open question of the work has its answer
//! ([`unanswered`]). The architect answers one itself, or, for what only the owner can decide, pins it in the owner's
//! one-to-one with an ask and replies once the owner answered: the matter holds no answer meanwhile, so the merge keeps
//! waiting, and the owner's words are kept in the answer as they were said. A departure from a decision someone other
//! than the architect made is only the owner's to rule on.

use db::attachments::{self, Words};
use db::rusqlite::{Connection, OptionalExtension, Transaction, params};
use db::{Matter, queue_matter};
use serde_json::{Value, json};

use crate::actions::Ctx;
use crate::work::RoundId;
use crate::{ENGINEER, cards, decisions};

/// The kind of matter an open question is.
pub const KIND: &str = "open_question";

/// The engineer's cell that writes the design, whose return holds its open questions.
const DESIGN: &str = "D2";

/// The cell of the engineer's graph a work is done at: nothing can be said to a work there.
const DONE_AT: &str = "E1";

fn sql(e: impl std::fmt::Display) -> String {
    e.to_string()
}

fn text<'a>(v: &'a Value, k: &str) -> &'a str {
    v[k].as_str().unwrap_or_default().trim()
}

/// Where a round's return is on the owner's page: what the architect links to when it pins a question for the owner.
fn round_link(work: i64, n: i64) -> String {
    format!("/works/{work}?round=R{work}-{n}")
}

/// The engine's cell of the engineer's graph where a work whose design departs from a decision of its item waits, once
/// the checks after design passed, for the ruling of whoever made the decision.
pub const RULING: &str = "D12";

/// What the open question that says a design departs from a decision of its item says before the decision's line.
pub const DEPARTS: &str = "The design departs from this decision of the item: ";

/// Where a design departs from a decision of its item: the decision's number, its line and who made it, as the design's
/// round was given them, the passage of the design that goes otherwise, and what the design does instead and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Departure {
    pub decision: String,
    pub line: String,
    pub by: String,
    pub passage: String,
    pub instead: String,
}

impl Departure {
    /// The open question it is put as, whose answer is the ruling. Each design that departs puts its own: a ruling is on
    /// the design it was asked about, and one that turned a departure down does not let a design written again that
    /// still departs from the same decision go on.
    pub fn question(&self) -> String {
        format!("{DEPARTS}{}", self.line)
    }

    /// Whether the architect made the decision, and so rules on the departure itself.
    pub fn architect_s(&self) -> bool {
        self.by == decisions::Source::Architect.as_str()
    }
}

/// The departures of the work's latest design from the decisions of its item, with the round that wrote it: each row of
/// its `item_decisions` that departs, with the line of the decision and who made it as that round was given them. None
/// before any design. `within` is the field an agent's forms sit in.
pub fn departures(conn: &Connection, work: i64, within: &str) -> Result<Option<(RoundId, Vec<Departure>)>, String> {
    type Row = (i64, Option<String>, Option<String>);
    let row: Option<Row> = conn
        .query_row(
            &format!(
                "SELECT n, json_extract(output, '$.{within}.item_decisions'), json_extract(input, '$.engine.decisions')
                 FROM rounds WHERE work = ?1 AND cell IN {} AND transition IS NOT NULL
                   AND json_extract(output, '$.{within}.design') IS NOT NULL
                 ORDER BY n DESC LIMIT 1",
                db::cells_sql(ENGINEER, &[DESIGN])
            ),
            [work],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .map_err(sql)?;
    let Some((n, rows, given)) = row else {
        return Ok(None);
    };
    let list = |t: Option<String>| -> Vec<Value> {
        t.and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default()
    };
    let given = list(given);
    let departs = list(rows)
        .iter()
        .filter(|r| r["verdict"] == "departs")
        .filter_map(|r| {
            let id = text(r, "decision");
            let listed = given.iter().find(|g| g["decision"] == id)?;
            Some(Departure {
                decision: id.to_string(),
                line: text(listed, "line").to_string(),
                by: text(listed, "by").to_string(),
                passage: text(r, "passage").to_string(),
                instead: text(r, "instead").to_string(),
            })
        })
        .filter(|d| !d.line.is_empty())
        .collect();
    Ok(Some((RoundId::new(work, n), departs)))
}

/// The ruling that lets a departure stand, as the reply to its question gives it (`departure`) and its body keeps it
/// (`ruling`).
pub const STANDS: &str = "stands";

/// The ruling that turns a departure down: the design must not go its own way there.
pub const TURNED_DOWN: &str = "turned_down";

/// Where a departure of a design stands.
#[derive(Debug, PartialEq, Eq)]
enum Ruled {
    /// A ruling on this design or an earlier one of the work let the same departure stand: it waits for nothing.
    Stands,
    /// The question this design put was answered, with the ruling the reply gave, or withdrawn (no ruling).
    Answered(Option<String>),
    /// The question this design put has no answer yet, or was never put.
    Waiting,
}

/// Where departure `d` of the design round `design` wrote stands: let stand by any ruling of the work on the same
/// decision, or ruled on by the answer to the question this design put, which an answer to an earlier design's does not
/// stand for: a ruling that turned a departure down is on the design it was asked about.
fn ruled(conn: &Connection, design: RoundId, d: &Departure) -> Result<Ruled, String> {
    let question = d.question();
    let stood: i64 = conn
        .query_row(
            "SELECT count(*) FROM architect_matters WHERE work = ?1 AND kind = ?2 AND trim(what) = ?3
               AND json_extract(body, '$.answer') IS NOT NULL AND json_extract(body, '$.ruling') = ?4",
            params![design.work, KIND, question, STANDS],
            |r| r.get(0),
        )
        .map_err(sql)?;
    if stood > 0 {
        return Ok(Ruled::Stands);
    }
    type Row = (Option<String>, Option<String>, Option<String>);
    let matter: Option<Row> = conn
        .query_row(
            "SELECT json_extract(body, '$.answer'), json_extract(body, '$.ruling'), withdrawn FROM architect_matters
             WHERE work = ?1 AND round = ?2 AND kind = ?3 AND trim(what) = ?4 ORDER BY id DESC LIMIT 1",
            params![design.work, design.n, KIND, question],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .map_err(sql)?;
    Ok(match matter {
        Some((Some(_), ruling, _)) => Ruled::Answered(ruling),
        Some((None, _, Some(_))) => Ruled::Answered(None),
        _ => Ruled::Waiting,
    })
}

/// Whose ruling the work waits for on where its latest design departs from the decisions of its item, as the engine's
/// value `departures` says it: `none` when each departure stands by a ruling or its question is answered or withdrawn,
/// `owner` while one that departs from a decision anyone but the architect made has no answer, and `architect` while
/// those left are all from the architect's own. A departure whose question was never put waits as one put and
/// unanswered: the work goes on past none that was not ruled on. Only the latest design counts, and only the questions
/// it put itself, or a ruling that let the same departure stand: a question an earlier design put that turned it down
/// rules on that design and not this one.
pub fn ruling_due(conn: &Connection, work: i64, within: &str) -> Result<&'static str, String> {
    let Some((design, departs)) = departures(conn, work, within)? else {
        return Ok("none");
    };
    let (mut owner, mut architect) = (false, false);
    for d in departs {
        if ruled(conn, design, &d)? != Ruled::Waiting {
            continue;
        }
        if d.architect_s() {
            architect = true;
        } else {
            owner = true;
        }
    }
    Ok(if owner {
        "owner"
    } else if architect {
        "architect"
    } else {
        "none"
    })
}

/// Whether a work's question was put already, word for word: a design written again that hands it in does not put it
/// again.
fn put_before(tx: &Transaction<'_>, work: i64, question: &str) -> Result<bool, String> {
    let before: i64 = tx
        .query_row(
            "SELECT count(*) FROM architect_matters WHERE work = ?1 AND kind = ?2 AND trim(what) = ?3",
            params![work, KIND, question],
            |r| r.get(0),
        )
        .map_err(sql)?;
    Ok(before > 0)
}

/// Whether the design written by round `design` put a departure's question already, as the checks after design pass
/// again for the same design: each design that departs puts its own ([`Departure::question`]).
fn departure_put(tx: &Transaction<'_>, design: RoundId, question: &str) -> Result<bool, String> {
    let before: i64 = tx
        .query_row(
            "SELECT count(*) FROM architect_matters WHERE work = ?1 AND round = ?2 AND kind = ?3 AND trim(what) = ?4",
            params![design.work, design.n, KIND, question],
            |r| r.get(0),
        )
        .map_err(sql)?;
    Ok(before > 0)
}

/// The departure questions that designs of the work before the one round `design` wrote put and that have no answer:
/// each is on a design written again since, so it is withdrawn, and an ask of the owner that holds it, unanswered and
/// holding nothing else still open, needs no answer any more. The withdrawn matters, by their numbers.
fn withdraw_superseded(tx: &Transaction<'_>, design: RoundId) -> Result<Vec<String>, String> {
    let mut st = tx
        .prepare(
            "SELECT id, asked FROM architect_matters
             WHERE work = ?1 AND round < ?2 AND kind = ?3 AND json_type(body, '$.departs') = 'object'
               AND json_extract(body, '$.answer') IS NULL AND withdrawn IS NULL AND handled_round IS NULL
             ORDER BY id",
        )
        .map_err(sql)?;
    let stale: Vec<(i64, Option<i64>)> = st
        .query_map(params![design.work, design.n, KIND], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(sql)?
        .collect::<Result<_, _>>()
        .map_err(sql)?;
    let why = format!(
        "the design it is about was written again, in round R{}-{}",
        design.work, design.n
    );
    for (m, _) in &stale {
        tx.execute(
            "UPDATE architect_matters SET withdrawn = ?2 WHERE id = ?1",
            params![m, why],
        )
        .map_err(sql)?;
    }
    for a in stale.iter().filter_map(|(_, a)| *a) {
        let answer = json!({ "by": "engine", "at": db::now(), "text": format!("No answer is needed: {why}.") });
        // An ask another open question still waits on stays open.
        let awaited: bool = tx
            .query_row(
                "SELECT EXISTS (SELECT 1 FROM architect_matters m WHERE m.asked = ?1
                   AND json_extract(m.body, '$.answer') IS NULL AND m.withdrawn IS NULL)",
                [a],
                |r| r.get(0),
            )
            .map_err(sql)?;
        if !awaited {
            db::answer_if_open(tx, a, &answer).map_err(sql)?;
        }
    }
    Ok(stale.iter().map(|(m, _)| m.to_string()).collect())
}

/// `post_open_questions`: each open question of the work's latest design becomes a matter of its project's architect,
/// raised by the round that wrote it, and so does each place the design departs from a decision of its item, which the
/// work waits on at cell [`RULING`] for the ruling of whoever made the decision. An open question the work put before,
/// word for word, as a design written again hands it in, is not put again; one it no longer hands in stays, and is
/// answered as it was asked. A departure is put for each design that departs, since a ruling is on the design it was
/// asked about, unless a ruling let the same departure stand; the departure questions of earlier designs left
/// unanswered are withdrawn ([`withdraw_superseded`]).
pub fn post(ctx: &Ctx, tx: &Transaction<'_>) -> Result<(), String> {
    let Some(item) = &ctx.item else {
        return Ok(());
    };
    let path = format!("{}.open_questions", ctx.within);
    let mut new = Vec::new();
    if let Some((round, questions)) = ctx.last(tx, DESIGN, &path)? {
        for q in questions.as_array().into_iter().flatten() {
            let question = text(q, "question");
            if question.is_empty() || put_before(tx, ctx.work.id, question)? {
                continue;
            }
            let m = Matter {
                project: item.project.clone(),
                kind: KIND.into(),
                detail: None,
                what: question.to_string(),
                body: json!({
                    "question": question,
                    "options": text(q, "options"),
                    "recommended": text(q, "recommended"),
                    "design_link": round_link(round.work, round.n),
                }),
                item: Some(item.id),
                raised: Some((round.work, round.n)),
            };
            new.push(queue_matter(tx, &m).map_err(sql)?.to_string());
        }
    }
    let (mut departed, mut superseded) = (Vec::new(), Vec::new());
    if let Some((round, departs)) = departures(tx, ctx.work.id, &ctx.within)? {
        superseded = withdraw_superseded(tx, round)?;
        for d in departs {
            let question = d.question();
            if departure_put(tx, round, &question)? || ruled(tx, round, &d)? == Ruled::Stands {
                continue;
            }
            let m = Matter {
                project: item.project.clone(),
                kind: KIND.into(),
                detail: None,
                what: question.clone(),
                body: json!({
                    "question": question,
                    "departs": { "decision": d.decision, "line": d.line, "by": d.by },
                    "passage": d.passage,
                    "instead": d.instead,
                    "design_link": round_link(round.work, round.n),
                }),
                item: Some(item.id),
                raised: Some((round.work, round.n)),
            };
            departed.push(queue_matter(tx, &m).map_err(sql)?.to_string());
        }
    }
    if !new.is_empty() {
        crate::actions::log_only(
            tx,
            ctx.work.id,
            &format!(
                "Open questions {} of {} wait for an answer. They block nothing until the merge.",
                cards::and_list(&new),
                ctx.about()
            ),
        )?;
    }
    if !superseded.is_empty() {
        crate::actions::log_only(
            tx,
            ctx.work.id,
            &format!(
                "The design of {} was written again: open questions {}, on where an earlier design departed from \
                 decisions of its item, are withdrawn unanswered.",
                ctx.about(),
                cards::and_list(&superseded)
            ),
        )?;
    }
    if !departed.is_empty() {
        crate::actions::log_only(
            tx,
            ctx.work.id,
            &format!(
                "The design of {} departs from decisions of its item: open questions {} wait for the ruling of whoever \
                 made each, and the work implements nothing until then.",
                ctx.about(),
                cards::and_list(&departed)
            ),
        )?;
    }
    Ok(())
}

/// The work's open questions that have no answer yet, by their matters, oldest first; one withdrawn waits for nothing.
pub fn unanswered(conn: &Connection, work: i64) -> Result<Vec<i64>, String> {
    let mut st = conn
        .prepare(
            "SELECT q.matter FROM work_open_questions q JOIN architect_matters m ON m.id = q.matter
             WHERE q.work = ?1 AND q.answer IS NULL AND m.withdrawn IS NULL ORDER BY q.matter",
        )
        .map_err(sql)?;
    st.query_map([work], |r| r.get(0))
        .map_err(sql)?
        .collect::<Result<_, _>>()
        .map_err(sql)
}

/// Whether a matter is an open question.
pub fn is_one(tx: &Transaction<'_>, matter: i64) -> Result<bool, String> {
    let kind: String = tx
        .query_row("SELECT kind FROM architect_matters WHERE id = ?1", [matter], |r| {
            r.get(0)
        })
        .map_err(sql)?;
    Ok(kind == KIND)
}

/// The architect's reply to an open question, by its own judgment or carrying the owner's answer: what it decided and
/// why is the question's answer, kept in the matter's body with the time it came (a decision that quotes it is dated by
/// that time, not by the time the question was raised), and is said to the engineer of the work that asked as an
/// interjection, which turns the work, unless nothing can be said to it any more. An answer that goes against the
/// design can only be a redirect, which the engineer judges.
///
/// When the owner answered the question the architect pinned for them (an answered ask of kind `question` that holds the
/// matter: the newest, whatever ask parks the matter now, as the batch review's ask about dropped item changes does when
/// the reply came with changes), the answer begins with the owner's words as they were said, and the body records who
/// answered: `answered_by` is `owner` and `owner_ask` the ask that holds the words; otherwise `answered_by` is
/// `architect`. The merge waits on the answer being there (`unanswered`), so a question pinned stays unanswered until
/// this reply writes it. The engineer is told who said which part.
pub fn answer(tx: &Transaction<'_>, matter: i64, reply: &Value) -> Result<(), String> {
    let (work, question, body): (Option<i64>, String, String) = tx
        .query_row(
            "SELECT work, what, body FROM architect_matters WHERE id = ?1",
            [matter],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .map_err(sql)?;
    let owner: Option<(i64, String)> = tx
        .query_row(
            "SELECT a.id, coalesce(json_extract(a.answer, '$.text'), '') FROM work_asks a
             WHERE a.answer IS NOT NULL AND json_extract(a.body, '$.kind') = 'question'
               AND EXISTS (SELECT 1 FROM json_each(a.body, '$.holds') h WHERE h.value = ?1)
             ORDER BY a.id DESC LIMIT 1",
            [matter],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(sql)?;
    let why = text(reply, "why");
    let said = match (text(reply, "text"), why) {
        (said, "") => said.to_string(),
        (said, why) => format!("{said}\n\n{why}"),
    };
    let mut body: Value = serde_json::from_str(&body).map_err(sql)?;
    // The answer kept, and the words the engineer is told it in.
    let (answer, told) = match &owner {
        Some((ask, words)) => {
            let images = attachments::of(tx, Words::Ask(*ask)).map_err(sql)?;
            let words = attachments::with_named(words, &images);
            body["answered_by"] = json!("owner");
            body["owner_ask"] = json!(ask);
            match said.as_str() {
                "" => (words.clone(), format!("The owner answered: {words}")),
                said => (
                    format!("{words}\n\n{said}"),
                    format!("The owner answered: {words}\n\nThe architect added: {said}"),
                ),
            }
        }
        None => {
            body["answered_by"] = json!("architect");
            if let Some(o) = body.as_object_mut() {
                o.remove("owner_ask");
            }
            (said.clone(), said)
        }
    };
    // The ruling on a departure, which decides whether the work may carry on with its design.
    let ruling = text(reply, "departure");
    let told = match ruling {
        STANDS => format!("{told}\n\nThe ruling: the departure stands, and the design may go its own way here."),
        TURNED_DOWN => format!("{told}\n\nThe ruling: the departure is turned down, and the design must change here."),
        _ => told,
    };
    if !ruling.is_empty() {
        body["ruling"] = json!(ruling);
    }
    body["answer"] = json!(answer);
    // When it was answered: a decision that quotes the answer is dated by this, not by when the question was raised.
    body["answered_at"] = json!(db::now());
    tx.execute(
        "UPDATE architect_matters SET body = ?2, reply_held = NULL WHERE id = ?1",
        params![matter, body.to_string()],
    )
    .map_err(sql)?;
    let Some(work) = work else { return Ok(()) };
    let runs: i64 = tx
        .query_row(
            &format!(
                "SELECT count(*) FROM works WHERE id = ?1 AND graph = '{ENGINEER}' AND ended IS NULL AND at NOT IN {}",
                db::cells_sql(ENGINEER, &[DONE_AT])
            ),
            [work],
            |r| r.get(0),
        )
        .map_err(sql)?;
    if runs > 0 {
        let answer = format!("The answer to the open question of your design \"{question}\": {told}");
        crate::hub::say_to_line(tx, &db::ToLine::new(db::Sender::Architect, work, ENGINEER, &answer))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // The address the architect links the owner to is a round's on the page.
    #[test]
    fn a_design_s_link_is_its_round_on_the_page() {
        assert_eq!(round_link(5, 9), "/works/5?round=R5-9");
    }

    // The merge waits for the questions with no answer, not for one answered or withdrawn.
    #[test]
    fn only_unanswered_questions_hold_the_merge() {
        use db::{Actor, Db};
        let mut db = Db::in_memory().unwrap();
        let matter = |tx: &Transaction<'_>, body: &Value, withdrawn: Option<&str>| -> Result<i64, db::Error> {
            tx.execute(
                "INSERT INTO architect_matters (project, kind, what, body, work, round, withdrawn, at)
                 VALUES ('p', ?1, 'Which?', ?2, 1, 1, ?3, 1)",
                params![KIND, body.to_string(), withdrawn],
            )?;
            Ok(tx.last_insert_rowid())
        };
        let open = db
            .write(Actor::Engine, |tx| {
                tx.execute(
                    "INSERT INTO flow_versions (id, digest, compiled) VALUES (1, 'd', '{}')",
                    [],
                )?;
                tx.execute(
                    "INSERT INTO works (id, item, flow_version, graph, at) VALUES (1, NULL, 1, 'engineer', 'M1')",
                    [],
                )?;
                tx.execute(
                    "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES (1, 1, 'D2', '{}', 'c', 1)",
                    [],
                )?;
                let open = matter(tx, &json!({}), None)?;
                matter(tx, &json!({ "answer": "One." }), None)?;
                matter(tx, &json!({}), Some("its work ended"))?;
                Ok(open)
            })
            .unwrap();
        assert_eq!(unanswered(db.conn(), 1), Ok(vec![open]));
    }
}
