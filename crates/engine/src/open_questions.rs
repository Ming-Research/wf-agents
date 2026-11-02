//! Open questions: what a design could not settle by measuring or reading, each with its options and the one it
//! recommends. Once the checks after design passed each goes to the project's architect as a matter of its own, which
//! takes the path an engineer's question takes ([`post`]); the work does not wait, and goes on by what the design
//! recommends. The answer is kept in the matter's body, and reaches the engineer as an interjection, which turns the
//! work ([`answer`]). The merge waits until every open question of the work has its answer ([`unanswered`]).

use db::rusqlite::{Connection, Transaction, params};
use db::{Matter, queue_matter};
use serde_json::{Value, json};

use crate::actions::Ctx;
use crate::{ENGINEER, cards, decisions};

/// The kind of matter an open question is.
pub const KIND: &str = "open_question";

/// The engineer's cell that writes the design, whose return holds its open questions.
const DESIGN: &str = "D2";

/// The cell of the engineer's graph a work is done at: nothing can be said to a work there.
const DONE_AT: &str = "E1";

/// The cell of the engineer's graph a work merges at.
const MERGE: &str = "M2";

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

/// `post_open_questions`: each open question of the work's latest design becomes a matter of its project's architect,
/// raised by the round that wrote it. A question the work put before, word for word, as a design written again hands
/// it in, is not put again; one it no longer hands in stays, and is answered as it was asked.
pub fn post(ctx: &Ctx, tx: &Transaction<'_>) -> Result<(), String> {
    let path = format!("{}.open_questions", ctx.within);
    let Some((round, questions)) = ctx.last(tx, DESIGN, &path)? else {
        return Ok(());
    };
    let Some(item) = &ctx.item else {
        return Ok(());
    };
    let mut new = Vec::new();
    for q in questions.as_array().into_iter().flatten() {
        let question = text(q, "question");
        let before: i64 = tx
            .query_row(
                "SELECT count(*) FROM architect_matters WHERE work = ?1 AND kind = ?2 AND trim(what) = ?3",
                params![ctx.work.id, KIND, question],
                |r| r.get(0),
            )
            .map_err(sql)?;
        if question.is_empty() || before > 0 {
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
pub fn answer(tx: &Transaction<'_>, matter: i64, reply: &Value) -> Result<(), String> {
    let (work, question): (Option<i64>, String) = tx
        .query_row(
            "SELECT work, what FROM architect_matters WHERE id = ?1",
            [matter],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(sql)?;
    let why = text(reply, "why");
    let answer = match (text(reply, "text"), why) {
        (said, "") => said.to_string(),
        (said, why) => format!("{said}\n\n{why}"),
    };
    tx.execute(
        "UPDATE architect_matters SET body = json_set(body, '$.answer', ?2, '$.answered_at', ?3), reply_held = NULL
         WHERE id = ?1",
        params![matter, answer, db::now()],
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
        tx.execute(
            "INSERT INTO interjections (work, line, speaker, text) VALUES (?1, ?2, 'architect', ?3)",
            params![
                work,
                ENGINEER,
                format!("The answer to the open question of your design \"{question}\": {answer}")
            ],
        )
        .map_err(sql)?;
    }
    Ok(())
}

/// `write_learned`: the decisions the work's records bear out go under the head `Decisions:` of its item's `approach`,
/// each as the line the engine makes of it ([`crate::decisions`]); what the work learned, with the commits it merged,
/// goes at the end, under the head `Learned:`. Written as every change to an item is, in the transaction of the round,
/// so that `changes` keeps it. A work that learned nothing a later agent would miss still records its commits, which
/// lead from the code's history back to the item.
pub fn write_learned(ctx: &Ctx, tx: &Transaction<'_>) -> Result<(), String> {
    let Some(item) = &ctx.item else {
        return Ok(());
    };
    let learned = ctx.unwrapped["learned"].as_str().unwrap_or_default().trim();
    let commits = merged(tx, ctx.work.id)?;
    let now: Option<String> = tx
        .query_row("SELECT approach FROM items WHERE id = ?1", [item.id], |r| r.get(0))
        .map_err(sql)?;
    let mut approach = now.unwrap_or_default();
    for d in ctx.unwrapped["decisions"].as_array().into_iter().flatten() {
        let r = decisions::resolve(tx, &item.project, d, "engineer", Some(item.id)).map_err(|why| why.join("\n"))?;
        approach = decisions::insert(&approach, &r.line);
    }
    let approach = with_learned(&approach, learned, &commits);
    tx.execute(
        "UPDATE items SET approach = ?2 WHERE id = ?1",
        params![item.id, approach],
    )
    .map_err(sql)?;
    Ok(())
}

/// The commits a work merged, oldest first: each its squashed merge into main, the head of what it changed.
fn merged(tx: &Transaction<'_>, work: i64) -> Result<Vec<String>, String> {
    let mut st = tx
        .prepare(&format!(
            "SELECT json_extract(output, '$.result.merge_commit') FROM rounds
             WHERE work = ?1 AND cell IN {} AND transition = 'merged' ORDER BY n",
            db::cells_sql(ENGINEER, &[MERGE])
        ))
        .map_err(sql)?;
    st.query_map([work], |r| r.get::<_, Option<String>>(0))
        .map_err(sql)?
        .filter_map(|x| x.transpose())
        .collect::<Result<_, _>>()
        .map_err(sql)
}

/// An item's `approach` with what a work learned and the commits it merged at its end; the head `Learned:` is written
/// once, by the first work that adds to it.
fn with_learned(approach: &str, learned: &str, commits: &[String]) -> String {
    let approach = approach.trim_end();
    let mut parts: Vec<String> = Vec::new();
    if !approach.is_empty() {
        parts.push(approach.to_string());
    }
    let added: Vec<String> = [
        learned.to_string(),
        if commits.is_empty() {
            String::new()
        } else {
            format!("Commits: {}", commits.join(", "))
        },
    ]
    .into_iter()
    .filter(|x| !x.is_empty())
    .collect();
    if added.is_empty() {
        return approach.to_string();
    }
    let added = added.join("\n");
    if approach.lines().any(|l| l.trim_end() == "Learned:") {
        parts.push(added);
    } else {
        parts.push(format!("Learned:\n{added}"));
    }
    parts.join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    // What a work learned goes at the end of the item's approach under one head, with the commits it merged: a first
    // work writes the head, a later one adds below it, and a work that learned nothing still records its commits.
    #[test]
    fn what_a_work_learned_goes_under_one_head_with_its_commits() {
        let commits = ["abc123".to_string()];
        assert_eq!(
            with_learned("Cache the parse.", "Fact (2026-11-02): It is slow (R1-2)", &commits),
            "Cache the parse.\n\nLearned:\nFact (2026-11-02): It is slow (R1-2)\nCommits: abc123"
        );
        assert_eq!(with_learned("", "", &commits), "Learned:\nCommits: abc123");
        let second = with_learned("Do it.\n\nLearned:\nCommits: abc123\n", "", &["def456".to_string()]);
        assert_eq!(second, "Do it.\n\nLearned:\nCommits: abc123\n\nCommits: def456");
        assert_eq!(with_learned("Do it.\n", "", &[]), "Do it.");
    }

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
