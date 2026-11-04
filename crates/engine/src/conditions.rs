//! The owner's merge conditions: what the owner set to be done before an item's work merges, as `merge_conditions` keeps
//! them. The merge gate holds the work until each is judged: it asks the owner of the first still open, and the view
//! `item_merge_conditions` works out from the answers where each stands, which is the one place that says whose answer
//! judges one. Words that judge nothing go to the engineer, and the gate asks again with what it reports next. The
//! implementation and the fix report on each they are given (`owner_conditions`), and that report is before the owner
//! when the owner judges. The gate's levels, in order, are items, set_aside, conditions and owner ([`crate::values`]).
//! A general mechanism: nothing here is of one project.

use db::rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde_json::{Value, json};

use crate::actions::Ctx;
use crate::approval::GATE;
use crate::checks::{Problems, id_misses};
use crate::decisions::Source;
use crate::work::{Item, json_col, query_json, rounds_at};

fn sql(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// The engine's input of the conditions still open.
pub const INPUT: &str = "merge_conditions";

/// The gate's value when it has nothing to ask of a merge condition.
pub const CLEAR: &str = "clear";

/// The gate's value when the owner is to be asked of a merge condition, or is asked and has not answered.
pub const ASK: &str = "conditions";

/// The gate's value, and its row, when an answer judged nothing and has not gone to the engineer yet.
pub const ANSWERED: &str = "condition_answered";

/// The gate's row that asks the owner to judge a merge condition.
pub const JUDGES: &str = "owner_judges";

/// The field of the implementation's and the fix's return that reports on each condition.
const REPORTED: &str = "owner_conditions";

/// The cells whose returns report on a condition.
const REPORTS: [&str; 2] = ["I1", "T6"];

/// A merge condition as the view `item_merge_conditions` gives it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Condition {
    id: i64,
    what: String,
    source: String,
    source_id: i64,
    quote: String,
    said_by: String,
    recorded_by: String,
    recorded_round: Option<String>,
    at: i64,
}

impl Condition {
    /// Its number, as the agents and the owner read it.
    fn number(&self) -> String {
        format!("C{}", self.id)
    }

    /// What a cell is given of it.
    fn given(&self) -> Value {
        json!({
            "id": self.number(),
            "what": self.what,
            "source": { "from": self.source, "id": self.source_id, "quote": self.quote, "said_by": self.said_by },
            "recorded": { "by": self.recorded_by, "round": self.recorded_round, "at": crate::utc(self.at) },
        })
    }
}

/// The conditions of an item not judged yet, oldest first.
fn open(conn: &Connection, item: i64) -> Result<Vec<Condition>, String> {
    let rows = query_json(
        conn,
        "SELECT id, what, source, source_id, quote, said_by, recorded_by, recorded_work, recorded_round, at
         FROM item_merge_conditions WHERE item = ?1 AND status = 'open' ORDER BY id",
        &[&item],
        10_000,
    )?;
    Ok(rows.iter().map(condition).collect())
}

fn condition(r: &Value) -> Condition {
    let text = |k: &str| r[k].as_str().unwrap_or_default().to_string();
    Condition {
        id: r["id"].as_i64().unwrap_or_default(),
        what: text("what"),
        source: text("source"),
        source_id: r["source_id"].as_i64().unwrap_or_default(),
        quote: text("quote"),
        said_by: text("said_by"),
        recorded_by: text("recorded_by"),
        recorded_round: match (r["recorded_work"].as_i64(), r["recorded_round"].as_i64()) {
            (Some(w), Some(n)) => Some(format!("R{w}-{n}")),
            _ => None,
        },
        at: r["at"].as_i64().unwrap_or_default(),
    }
}

/// The engine's input `merge_conditions`: the item's conditions still open; none when it has none.
pub fn input(conn: &Connection, item: i64) -> Result<Option<Value>, String> {
    let open = open(conn, item)?;
    Ok((!open.is_empty()).then(|| Value::Array(open.iter().map(Condition::given).collect())))
}

/// The gate's latest ask of a condition in this work: its id, the number of the round that put it, and the answer once
/// it came.
fn latest_ask(conn: &Connection, work: i64, condition: i64) -> Result<Option<(i64, i64, Option<Value>)>, String> {
    let row: Option<(i64, i64, Option<String>)> = conn
        .query_row(
            "SELECT id, round, answer FROM owner_asks
             WHERE work = ?1 AND json_extract(body, '$.condition') = ?2 ORDER BY id DESC LIMIT 1",
            params![work, condition],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .map_err(sql)?;
    Ok(row.map(|(id, n, a)| (id, n, json_col(a))))
}

/// Where the item's merge conditions stand, for the merge gate's value `gates` once every item depended on is merged and
/// every finding set aside is judged: [`CLEAR`] when each is judged; [`ANSWERED`] when the first still open has an answer
/// that judged nothing and has not gone to the engineer; [`ASK`] when the owner is to be asked of it, or is asked and has
/// not answered. An ask the engine closed as the work stopped waiting, or an answer already given to the engineer, is
/// asked again.
pub fn gate(conn: &Connection, item: i64, work: i64) -> Result<&'static str, String> {
    let Some(first) = open(conn, item)?.into_iter().next() else {
        return Ok(CLEAR);
    };
    let Some((_, n, answer)) = latest_ask(conn, work, first.id)? else {
        return Ok(ASK);
    };
    let Some(answer) = answer else {
        return Ok(ASK);
    };
    if answer["by"] == "engine" {
        return Ok(ASK);
    }
    let relayed = rounds_at(conn, work, GATE)?
        .iter()
        .any(|r| r.id.n > n && r.transition.as_deref() == Some(ANSWERED));
    Ok(if relayed { ASK } else { ANSWERED })
}

/// Whether the gate's ask of the first condition still open stands unanswered: the gate's row [`JUDGES`] has then nothing
/// new to do, and otherwise asks of it.
pub fn asked(conn: &Connection, item: i64, work: i64) -> Result<bool, String> {
    let Some(first) = open(conn, item)?.into_iter().next() else {
        return Ok(false);
    };
    Ok(matches!(latest_ask(conn, work, first.id)?, Some((_, _, None))))
}

/// The latest report of the engineer on a condition, by its number: the round, and the row it handed in; none when it
/// has not reported on it.
fn report(conn: &Connection, work: i64, within: &str, number: &str) -> Result<Option<Value>, String> {
    let mut rounds = Vec::new();
    for cell in REPORTS {
        rounds.extend(rounds_at(conn, work, cell)?);
    }
    rounds.sort_by_key(|r| std::cmp::Reverse(r.id.n));
    Ok(rounds.iter().filter(|r| r.transition.is_some()).find_map(|r| {
        let rows = r.output.as_ref()?.get(within)?.get(REPORTED)?.as_array()?;
        let row = rows.iter().find(|x| x["id"] == number)?;
        Some(json!({ "round": r.id.to_string(), "row": row }))
    }))
}

/// Who a condition's record says said its words, in words.
fn said_by_words(said_by: &str) -> &'static str {
    match said_by {
        "owner" => "the owner",
        "relayed" => "the owner, relayed by the delegate and not checked",
        "delegate" => "the delegate that speaks for the owner, in its own words",
        _ => "someone the record does not name",
    }
}

/// Where the record a condition's words are in stands on the owner's page: the round that asked, for an ask, where what
/// was asked reads; the conversation the words were said in, for the rest.
fn record_link(conn: &Connection, source: &str, id: i64) -> Result<String, String> {
    match source {
        "ask" => {
            let (work, n, graph, project): (i64, i64, String, Option<String>) = conn
                .query_row(
                    "SELECT a.work, a.round, w.graph, w.project FROM owner_asks a JOIN works w ON w.id = a.work
                     WHERE a.id = ?1",
                    [id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
                )
                .map_err(sql)?;
            Ok(if graph == crate::ENGINEER {
                format!("/works/{work}?round=R{work}-{n}")
            } else {
                format!(
                    "/works/architect?project={}&round=R{work}-{n}",
                    project.unwrap_or_default()
                )
            })
        }
        "interjection" => {
            let work: i64 = conn
                .query_row("SELECT work FROM interjections WHERE id = ?1", [id], |r| r.get(0))
                .map_err(sql)?;
            Ok(format!("/messages?c=work:{work}"))
        }
        _ => {
            let (project, work): (String, Option<i64>) = conn
                .query_row(
                    "SELECT project, coalesce(work, json_extract(body, '$.work')) FROM architect_matters WHERE id = ?1",
                    [id],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .map_err(sql)?;
            Ok(match work {
                Some(w) => format!("/messages?c=work:{w}"),
                None => format!("/messages?c=architect:{project}"),
            })
        }
    }
}

/// The ask of a condition: its sentence, the words it rests on and who said them, the link to the record they are in, who
/// recorded it and when, the engineer's latest report and the pull request, for the owner to judge it with the two
/// options; the same in words and as a value. Words alone, or an answer that is not the owner's own, go to the engineer.
fn put(ctx: &Ctx, tx: &Transaction<'_>, item: &Item, c: &Condition) -> Result<i64, String> {
    let w = &ctx.work;
    let number = c.number();
    let link = record_link(tx, &c.source, c.source_id)?;
    let report = report(tx, w.id, &ctx.within, &number)?;
    let pull_request = match (w.pr, ctx.project.as_ref()) {
        (Some(pr), Some(p)) => Some(format!("https://github.com/{}/pull/{pr}", p.repo)),
        _ => None,
    };
    let recorded = match (&c.recorded_round, c.recorded_by.as_str()) {
        (Some(round), _) => format!("the architect's item change in {round}"),
        (None, "delegate") => "the delegate's tick".to_string(),
        (None, _) => "the owner's tick".to_string(),
    };
    let reported = match &report {
        Some(r) => format!(
            "The engineer's latest report, in {}: {}",
            r["round"].as_str().unwrap_or_default(),
            match r["row"]["answer"].as_str() {
                Some("done") => format!("done. {}", r["row"]["what"].as_str().unwrap_or_default()),
                _ => format!("not done. {}", r["row"]["why"].as_str().unwrap_or_default()),
            }
        ),
        None => "The engineer has not reported on it yet.".to_string(),
    };
    let mut text = vec![
        format!("Merge condition {number}: {}", c.what),
        format!(
            "The words it rests on: \"{}\", said by {}, in [{} {}]({link}). Recorded by {recorded} on {}.",
            c.quote,
            said_by_words(&c.said_by),
            c.source,
            c.source_id,
            crate::utc(c.at)
        ),
        reported,
    ];
    if let Some(pr) = &pull_request {
        text.push(format!("The pull request: {pr}"));
    }
    let body = json!({
        "to": "owner",
        "work": w.id,
        "item": item.id,
        "title": item.title,
        "condition": c.id,
        "asked": [{
            "what": "The merge condition to judge before the work merges",
            "value": {
                "points": [{ "what": text.join("\n\n") }],
                "choice": {
                    "question": format!("Is merge condition {number} met, or do you withdraw it?"),
                    "options": [
                        { "name": "met", "does": "It is done as you set it: the gate goes on" },
                        { "name": "withdrawn", "does": "It no longer has to be done: the gate goes on" },
                    ],
                },
                "condition": {
                    "id": number,
                    "what": c.what,
                    "source": {
                        "from": c.source,
                        "id": c.source_id,
                        "quote": c.quote,
                        "said_by": c.said_by,
                        "link": link,
                    },
                    "recorded": { "by": c.recorded_by, "round": c.recorded_round, "at": crate::utc(c.at) },
                    "report": report,
                    "pull_request": pull_request,
                },
            },
        }],
    });
    tx.execute(
        "INSERT INTO owner_asks (work, round, body) VALUES (?1, ?2, ?3)",
        params![w.id, ctx.round.n, body.to_string()],
    )
    .map_err(sql)?;
    let id = tx.last_insert_rowid();
    let told = format!(
        "Work {} on item {} ({}) waits for you to judge merge condition {number} before it merges: {} Choose met or \
         withdrawn; words alone go to its engineer. (ask {id})",
        w.id, item.id, item.title, c.what
    );
    crate::actions::notify_ask(tx, w.id, id, &told)?;
    Ok(id)
}

/// `ask_owner_condition`: asks the owner to judge the first condition still open, unless the gate's latest ask of it
/// waits for its answer, and has the ask stand for the work's blocker, which says which condition it waits on. With none
/// open, as when the owner judged the last while this step ran, it does nothing and the gate reads them again.
pub fn ask(ctx: &Ctx, tx: &Transaction<'_>) -> Result<(), String> {
    let item = ctx
        .item
        .as_ref()
        .ok_or_else(|| format!("work {} asks of a merge condition, and is on no item", ctx.work.id))?;
    let Some(first) = open(tx, item.id)?.into_iter().next() else {
        return Ok(());
    };
    let id = match latest_ask(tx, ctx.work.id, first.id)? {
        Some((id, _, None)) => id,
        _ => put(ctx, tx, item, &first)?,
    };
    if let Some(b) = crate::blockers::open(tx, ctx.work.id)? {
        crate::blockers::set_ask(tx, b.id, id)?;
        let what = format!(
            "{} waits for the owner to judge merge condition {} before it merges",
            crate::blockers::about(ctx),
            first.number()
        );
        crate::blockers::set_what(tx, b.id, &what)?;
    }
    Ok(())
}

/// What the relay of an answer that judged nothing says it answered and the answer: the first condition still open's
/// latest ask, which stays open until the owner judges it. None when it has no answer.
pub fn answered(ctx: &Ctx, tx: &Transaction<'_>) -> Result<Option<(i64, Value, String)>, String> {
    let Some(item) = ctx.item.as_ref() else {
        return Ok(None);
    };
    let Some(first) = open(tx, item.id)?.into_iter().next() else {
        return Ok(None);
    };
    let Some((id, _, Some(answer))) = latest_ask(tx, ctx.work.id, first.id)? else {
        return Ok(None);
    };
    let asked = format!(
        "Asked to judge merge condition {} (\"{}\") before the work merges, which stays open until the owner judges it",
        first.number(),
        first.what
    );
    Ok(Some((id, answer, asked)))
}

/// The numbers of the conditions still open that a round is held to report on, those recorded by the time it started,
/// which it was given, and of all those still open, in that order. A condition recorded while the round ran it was not
/// given, and a report on it is no fault.
pub fn due(conn: &Connection, item: i64, work: i64) -> Result<(Vec<String>, Vec<String>), String> {
    let started: i64 = conn
        .query_row(
            "SELECT started_at FROM rounds WHERE work = ?1 ORDER BY n DESC LIMIT 1",
            [work],
            |r| r.get(0),
        )
        .map_err(sql)?;
    let open = open(conn, item)?;
    let due = open.iter().filter(|c| c.at <= started).map(Condition::number).collect();
    Ok((due, open.iter().map(Condition::number).collect()))
}

/// What is wrong with a return's report on the conditions (`reported`, the field as it was handed in): a report when no
/// condition stands, a number that is none of those still open (`open`), one reported twice, and one it was given (`due`)
/// that it left out, each kind said once with all its numbers.
pub fn report_problems(due: &[String], open: &[String], reported: Option<&Value>) -> Problems {
    let mut problems = Problems::default();
    let Some(reported) = reported else {
        problems.about(
            due,
            |x| format!("merge condition {x} is not reported on: answer it done or not_done in owner_conditions"),
            |xs| format!("merge conditions {xs} are not reported on: answer each done or not_done in owner_conditions"),
        );
        return problems;
    };
    if open.is_empty() {
        problems.push("you were given no merge condition: leave owner_conditions out");
        return problems;
    }
    let given: Vec<String> = reported
        .as_array()
        .into_iter()
        .flatten()
        .map(|r| r["id"].as_str().unwrap_or_default().to_string())
        .collect();
    let misses = id_misses(open, &given);
    problems.about(
        &misses.unknown,
        |x| format!("{x} is not a merge condition you were given"),
        |xs| format!("{xs} are not merge conditions you were given"),
    );
    problems.about(
        &misses.twice,
        |x| format!("merge condition {x} is reported on twice"),
        |xs| format!("merge conditions {xs} are reported on twice"),
    );
    let left_out: Vec<String> = due.iter().filter(|d| !given.contains(d)).cloned().collect();
    problems.about(
        &left_out,
        |x| format!("merge condition {x} is not reported on: answer it done or not_done"),
        |xs| format!("merge conditions {xs} are not reported on: answer each done or not_done"),
    );
    problems
}

/// A merge condition an item change of the architect records: the sentence it wrote is checked by [`crate::arch_changes`];
/// here the record its source names, that its quotation is found in it letter for letter, and that it is not the
/// architect's own words. Gives who the record says said the words.
pub fn source_said_by(conn: &Connection, project: &str, source: &Value) -> Result<&'static str, String> {
    let from = source["from"].as_str().unwrap_or_default();
    let id = source["id"].as_i64().unwrap_or_default();
    let quote = source["quote"].as_str().unwrap_or_default();
    let record = crate::decisions::record(conn, project, from, id)?;
    if quote.trim().is_empty() || !record.texts.iter().any(|t| t.contains(quote)) {
        return Err(format!(
            "\"{quote}\" is not found as it is in {from} {id}: quote its words letter for letter"
        ));
    }
    match record.source_of(quote) {
        Source::Owner => Ok("owner"),
        Source::Relayed => Ok("relayed"),
        Source::Delegate => Ok("delegate"),
        Source::Unrecorded => Ok("unrecorded"),
        Source::Architect => Err(format!(
            "{from} {id} holds the architect's own words: a merge condition rests on what the owner or the delegate said"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(xs: &[&str]) -> Vec<String> {
        xs.iter().map(|x| x.to_string()).collect()
    }

    fn rows(xs: &[&str]) -> Value {
        Value::Array(
            xs.iter()
                .map(|x| json!({ "answer": "done", "id": x, "what": "w" }))
                .collect(),
        )
    }

    fn found(due: &[&str], open: &[&str], reported: Option<&Value>) -> String {
        report_problems(&ids(due), &ids(open), reported).texts().join("\n")
    }

    // A round given conditions answers each once, and no other; one given none leaves the field out, so that a work on
    // an item with no condition is asked nothing; a condition recorded while the round ran it was not given is not held
    // against it, whichever it does.
    #[test]
    fn a_report_answers_each_condition_given_once_and_no_other() {
        assert_eq!(found(&["C1", "C2"], &["C1", "C2"], Some(&rows(&["C1", "C2"]))), "");
        assert!(found(&["C1", "C2"], &["C1", "C2"], Some(&rows(&["C1"]))).contains("C2 is not reported on"));
        assert!(found(&["C1"], &["C1"], None).contains("C1 is not reported on"));
        assert!(found(&["C1"], &["C1"], Some(&rows(&["C1", "C7"]))).contains("C7 is not a merge condition"));
        assert!(found(&["C1"], &["C1"], Some(&rows(&["C1", "C1"]))).contains("C1 is reported on twice"));
        assert_eq!(found(&[], &[], None), "");
        assert!(found(&[], &[], Some(&rows(&["C1"]))).contains("leave owner_conditions out"));
        assert!(found(&[], &[], Some(&json!([]))).contains("leave owner_conditions out"));
        assert_eq!(found(&["C1"], &["C1", "C2"], Some(&rows(&["C1"]))), "");
        assert_eq!(found(&["C1"], &["C1", "C2"], Some(&rows(&["C1", "C2"]))), "");
        assert_eq!(found(&[], &["C2"], None), "");
    }
}
