//! The owner's commands: creating items and ordering the queue, answering asks of the owner and ruling on design
//! cards, interjecting, pausing, and the views of where everything stands. Every write is made as the owner, so
//! `changes` records it as theirs.

use std::path::Path;

use db::Actor;
use db::rusqlite::{OptionalExtension, params};
use engine::database::open;
use engine::{Counted, query_json, query_json_counted};
use serde_json::Value;

/// The most rows an owner command lists from a query; a table that is longer ends there, and says how many rows it
/// leaves out.
const TABLE_ROWS: usize = 10_000;

fn sql(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// What a new item says, as the owner writes it.
pub struct NewItem {
    pub project: String,
    pub title: String,
    pub problem: String,
    pub why: String,
    pub approach: Option<String>,
    pub deliver: String,
    pub level: String,
    /// Items it depends on.
    pub depends_on: Vec<Dependency>,
    /// Puts it last in the dispatch queue.
    pub queue: bool,
    pub imported: Imported,
}

/// An item another depends on, the kind of the dependency and what the other needs of it.
pub struct Dependency {
    pub on: i64,
    /// blocking: the other does not start until this one is merged; parallel: it may start before.
    pub kind: String,
    pub needs: String,
}

/// What an import knows of work done on an item outside this flow; all none for an item made here.
#[derive(Default)]
pub struct Imported {
    /// Where the item came from, such as an item of another board.
    pub from: Option<String>,
    /// The branch work on it had, and that branch's pull request.
    pub branch: Option<String>,
    pub pr: Option<i64>,
}

pub fn add_item(data: &Path, x: NewItem) -> Result<i64, String> {
    let c = config::committed(data).map_err(sql)?.config;
    if !c.projects.contains_key(&x.project) {
        return Err(format!("no project {} in the configuration", x.project));
    }
    if c.level(&x.level).is_none() {
        return Err(format!("no level {} in the configuration", x.level));
    }
    c.item_level(&x.level)?;
    let max = c.install.items.title_max as usize;
    let len = x.title.chars().filter(|c| !c.is_whitespace()).count();
    if len > max {
        return Err(format!("the title is {len} long; at most {max}, spaces not counted"));
    }
    if !matches!(x.deliver.as_str(), "change" | "report") {
        return Err("deliver is change or report".into());
    }
    if x.imported.pr.is_some() && x.imported.branch.is_none() {
        return Err("an imported pull request is recorded with its branch".into());
    }
    let mut db = open(data)?;
    db.write(Actor::Owner, |tx| {
        tx.execute(
            "INSERT INTO items (project, title, problem, why, approach, deliver, level, imported_from, imported_branch,
                                imported_pr)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                x.project,
                x.title,
                x.problem,
                x.why,
                x.approach,
                x.deliver,
                x.level,
                x.imported.from,
                x.imported.branch,
                x.imported.pr
            ],
        )?;
        let id = tx.last_insert_rowid();
        for d in &x.depends_on {
            db::add_dependency(tx, id, d.on, &d.needs, &d.kind)?;
        }
        if x.queue {
            let last: f64 = tx.query_row("SELECT coalesce(max(position), 0) FROM dispatch_queue", [], |r| {
                r.get(0)
            })?;
            tx.execute(
                "INSERT INTO dispatch_queue (item, position) VALUES (?1, ?2)",
                params![id, last + 1.0],
            )?;
        }
        Ok(id)
    })
    .map_err(sql)
}

pub fn depend(data: &Path, item: i64, d: &Dependency) -> Result<(), String> {
    open(data)?
        .write(Actor::Owner, |tx| db::add_dependency(tx, item, d.on, &d.needs, &d.kind))
        .map_err(sql)
}

/// Cancels an item, as the architect's cancellation does: its works end as every work ends, and the items that
/// depended on it are told.
pub fn cancel(data: &Path, item: i64, why: &str) -> Result<(), String> {
    let mut base = open(data)?;
    let projects = config::committed(data).map_err(sql)?.config.projects;
    base.write(Actor::Owner, |tx| {
        engine::ending::cancel_item(tx, item, why, "owner", &projects).map_err(db::Error::Abort)
    })
    .map_err(sql)
}

/// Where an item goes in the queue.
pub enum Place {
    First,
    Last,
    Before(i64),
    After(i64),
}

pub fn queue(data: &Path, item: i64, place: Place) -> Result<(), String> {
    open(data)?
        .write(Actor::Owner, |tx| {
            let pos = |i: i64| -> db::Result<Option<f64>> {
                Ok(tx
                    .query_row("SELECT position FROM dispatch_queue WHERE item = ?1", [i], |r| r.get(0))
                    .optional()?)
            };
            tx.execute("DELETE FROM dispatch_queue WHERE item = ?1", [item])?;
            let edge = |q: &str| -> db::Result<Option<f64>> { Ok(tx.query_row(q, [], |r| r.get(0))?) };
            let p = match place {
                Place::First => edge("SELECT min(position) FROM dispatch_queue")?.map_or(1.0, |m| m - 1.0),
                Place::Last => edge("SELECT max(position) FROM dispatch_queue")?.map_or(1.0, |m| m + 1.0),
                Place::Before(o) | Place::After(o) => {
                    let at = pos(o)?.ok_or_else(|| db::Error::Abort(format!("item {o} is not in the queue")))?;
                    let before = matches!(place, Place::Before(_));
                    let q = if before {
                        "SELECT max(position) FROM dispatch_queue WHERE position < ?1"
                    } else {
                        "SELECT min(position) FROM dispatch_queue WHERE position > ?1"
                    };
                    let next: Option<f64> = tx.query_row(q, [at], |r| r.get(0))?;
                    match next {
                        Some(n) => (n + at) / 2.0,
                        None if before => at - 1.0,
                        None => at + 1.0,
                    }
                }
            };
            tx.execute(
                "INSERT INTO dispatch_queue (item, position) VALUES (?1, ?2)",
                params![item, p],
            )?;
            Ok(())
        })
        .map_err(sql)
}

/// How the owner answers an ask of the owner.
pub struct Answer {
    pub ask: i64,
    pub text: String,
    /// For what the architect is asked: why it was decided this way.
    pub why: Option<String>,
    /// For what the architect is asked: the rest of the architect's reply, as JSON (children, rest, waits).
    pub reply: Option<Value>,
    /// For an ask of the architect's that offers options, the option chosen.
    pub choice: Option<String>,
}

/// Answers an ask of the owner, as the page does.
pub fn answer(data: &Path, a: Answer) -> Result<String, String> {
    let mut db = open(data)?;
    let given = api::Answer {
        text: a.text,
        why: a.why,
        reply: a.reply,
        choice: a.choice,
        to: None,
    };
    api::answer(&mut db, a.ask, &given)
}

/// How the owner rules on a design card: the decisions confirmed, by number or `all`; those disagreed with, each as
/// `<number>=<why>`; and those another approach is chosen for, each as `<number>=<approach>`.
pub struct Ruling {
    pub design_card: i64,
    pub confirm: Vec<String>,
    pub disagree: Vec<String>,
    pub choose: Vec<String>,
}

/// Rules on a design card, as the page does: a verdict on each decision, all at once. A decision disagreed with, or
/// another approach chosen for, is not confirmed by `all`.
pub fn rule(data: &Path, r: Ruling) -> Result<String, String> {
    let mut db = open(data)?;
    let count: i64 = db
        .conn()
        .query_row(
            "SELECT max(1, coalesce(json_array_length(body), 1)) FROM design_cards WHERE id = ?1",
            [r.design_card],
            |x| x.get(0),
        )
        .optional()
        .map_err(sql)?
        .unwrap_or(1);
    let number = |s: &str| {
        s.trim()
            .parse::<i64>()
            .map_err(|_| format!("not the number of a decision: {s}"))
    };
    let verdict = |decision: i64, v: &str| api::DecisionRuling {
        decision,
        verdict: v.to_string(),
        text: None,
        option: None,
    };
    let mut by: std::collections::BTreeMap<i64, api::DecisionRuling> = std::collections::BTreeMap::new();
    for c in &r.confirm {
        let all: Vec<i64> = if c.trim() == "all" {
            (1..=count).collect()
        } else {
            vec![number(c)?]
        };
        for n in all {
            by.insert(n, verdict(n, "confirm"));
        }
    }
    for (given, choosing) in [(&r.disagree, false), (&r.choose, true)] {
        for x in given {
            let (n, said) = x
                .split_once('=')
                .ok_or_else(|| format!("write it as <number>=<text>: {x}"))?;
            let n = number(n)?;
            let d = by.entry(n).or_insert_with(|| verdict(n, "disagree"));
            d.verdict = "disagree".into();
            if choosing {
                d.option = Some(said.trim().to_string());
            } else {
                d.text = Some(said.trim().to_string());
            }
        }
    }
    let given = api::Ruling {
        decisions: by.into_values().collect(),
    };
    api::rule(&mut db, r.design_card, &given)
}

/// Says something to a project's architect, as the page does: the project named, or the configuration's one project.
pub fn say_to_architect(data: &Path, project: Option<&str>, item: Option<i64>, text: &str) -> Result<String, String> {
    let committed = config::committed(data).map_err(|e| e.to_string())?;
    let project = api::project_named(&committed.config, project)?;
    let mut db = open(data)?;
    db::say_to_architect(&mut db, &project, item, None, text)
}

/// Says something in a running work's group, as the page does: to its engineer, its project's architect or both.
pub fn say(data: &Path, work: i64, to: &str, text: &str) -> Result<String, String> {
    let said = api::WorkSaid {
        text: text.to_string(),
        to: Some(to.to_string()),
        question: None,
    };
    api::say_to_work(&mut open(data)?, work, &said)
}

/// Lifts a work's pause, as its Start on the page does.
pub fn resume(data: &Path, work: i64) -> Result<String, String> {
    let mut db = open(data)?;
    api::resume(&mut db, work)
}

/// Pauses a work, as the page does, saying why.
pub fn pause(data: &Path, work: i64, why: &str) -> Result<String, String> {
    let mut db = open(data)?;
    api::pause(&mut db, work, Some(why))
}

/// Prints rows as a table, one line per row, each value cut to `width`, and, when the query had more rows than were
/// read, after it the line that says how many it left out.
fn table(rows: &Counted, width: usize) -> String {
    table_body(&rows.rows, width) + &left_out_line(rows.left_out, rows.rows.len())
}

/// The last line of a table that leaves rows out, which is no row of the table: how many, after the first `read` rows.
/// Nothing when it leaves none.
fn left_out_line(left_out: usize, read: usize) -> String {
    match left_out {
        0 => String::new(),
        1 => format!("(1 row left out after the first {read})\n"),
        n => format!("({n} rows left out after the first {read})\n"),
    }
}

/// The last line of the table of counters past their limits when more counters were there than were read: how many were
/// never weighed against their limits, after the first `read`. Nothing when there are none.
fn unchecked_line(unchecked: usize, read: usize) -> String {
    match unchecked {
        0 => String::new(),
        1 => format!("(1 counter after the first {read} was not checked against the limits)\n"),
        n => format!("({n} counters after the first {read} were not checked against the limits)\n"),
    }
}

/// Prints rows as a table, one line per row, each value cut to `width`.
fn table_body(rows: &[Value], width: usize) -> String {
    let Some(first) = rows.first().and_then(Value::as_object) else {
        return "(none)\n".into();
    };
    let cols: Vec<&String> = first.keys().collect();
    let cell = |v: &Value| -> String {
        let s = match v {
            Value::String(s) => s.clone(),
            Value::Null => String::new(),
            o => o.to_string(),
        };
        let s = s.replace('\n', " ");
        if s.chars().count() > width {
            format!("{}…", s.chars().take(width).collect::<String>())
        } else {
            s
        }
    };
    let mut out = cols.iter().map(|c| c.as_str()).collect::<Vec<_>>().join("\t");
    out.push('\n');
    for r in rows {
        out.push_str(&cols.iter().map(|c| cell(&r[c.as_str()])).collect::<Vec<_>>().join("\t"));
        out.push('\n');
    }
    out
}

fn show(data: &Path, q: &str, args: &[&dyn db::rusqlite::ToSql], width: usize) -> Result<String, String> {
    let db = open(data)?;
    Ok(table(&query_json_counted(db.conn(), q, args, TABLE_ROWS)?, width))
}

pub fn items(data: &Path) -> Result<String, String> {
    show(
        data,
        "SELECT i.id, i.project, i.title, i.deliver, i.level, i.cancelled,
                (SELECT position FROM dispatch_queue q WHERE q.item = i.id) AS queued,
                (SELECT group_concat(depends_on || ' ' || kind, ', ') FROM item_links l WHERE l.item = i.id)
                  AS depends_on,
                (SELECT max(w.id) FROM works w WHERE w.item = i.id) AS work
         FROM items i ORDER BY i.id",
        &[],
        40,
    )
}

pub fn queue_list(data: &Path) -> Result<String, String> {
    show(
        data,
        "SELECT item, project, title, level, position FROM ready_items",
        &[],
        40,
    )
}

pub fn status(data: &Path) -> Result<String, String> {
    show(
        data,
        "SELECT w.id AS work, w.item, i.title, w.at, w.pr, w.paused, w.ended,
                (SELECT group_concat(a.id) FROM owner_asks a WHERE a.work = w.id AND a.answer IS NULL) AS open_asks,
                (SELECT group_concat(c.id) FROM design_cards c
                 WHERE c.work = w.id AND c.status = 'open') AS open_design_cards,
                (SELECT count(*) FROM interjections x WHERE x.work = w.id AND x.delivered_round IS NULL) AS waiting_says,
                (SELECT 'R' || r.work || '-' || r.n || ' ' || r.cell || ' '
                        || coalesce(r.transition, CASE WHEN r.rejected IS NOT NULL THEN 'sent back' ELSE 'running' END)
                 FROM rounds r WHERE r.work = w.id ORDER BY r.n DESC LIMIT 1) AS last
         FROM works w LEFT JOIN items i ON i.id = w.item ORDER BY w.id",
        &[],
        40,
    )
}

pub fn asks(data: &Path, all: bool) -> Result<String, String> {
    let q = format!(
        "SELECT a.id AS ask, a.work, 'R' || a.work || '-' || a.round AS round, w.item, r.cell, a.body, a.answer
         FROM owner_asks a JOIN rounds r ON r.work = a.work AND r.n = a.round JOIN works w ON w.id = a.work {}
         ORDER BY a.id",
        if all { "" } else { "WHERE a.answer IS NULL" }
    );
    show(data, &q, &[], 80)
}

pub fn ask(data: &Path, id: i64) -> Result<String, String> {
    let db = open(data)?;
    let rows = query_json(
        db.conn(),
        "SELECT a.id AS ask, a.work, 'R' || a.work || '-' || a.round AS round, r.cell, a.body, a.answer
         FROM owner_asks a JOIN rounds r ON r.work = a.work AND r.n = a.round WHERE a.id = ?1",
        &[&id],
        1,
    )?;
    let row = rows.first().ok_or_else(|| format!("no ask {id}"))?;
    serde_json::to_string_pretty(row).map_err(sql)
}

pub fn design_cards(data: &Path, all: bool) -> Result<String, String> {
    let q = format!(
        "SELECT c.id AS design_card, c.status, c.work, 'R' || c.work || '-' || c.round AS round, w.item, r.cell, c.body
         FROM design_cards c JOIN rounds r ON r.work = c.work AND r.n = c.round JOIN works w ON w.id = c.work {}
         ORDER BY c.id",
        if all { "" } else { "WHERE c.status <> 'replaced'" }
    );
    show(data, &q, &[], 80)
}

pub fn design_card(data: &Path, id: i64) -> Result<String, String> {
    let db = open(data)?;
    let rows = query_json(
        db.conn(),
        "SELECT c.id AS design_card, c.status, c.work, 'R' || c.work || '-' || c.round AS round, r.cell, c.body,
                c.rulings, c.replaced_by
         FROM design_cards c JOIN rounds r ON r.work = c.work AND r.n = c.round WHERE c.id = ?1",
        &[&id],
        1,
    )?;
    let row = rows.first().ok_or_else(|| format!("no design card {id}"))?;
    serde_json::to_string_pretty(row).map_err(sql)
}

pub fn history(data: &Path, work: i64, full: bool) -> Result<String, String> {
    if full {
        let db = open(data)?;
        // The listing is JSON, which no line of text can be added to, so it is not cut: every round of the work.
        let rows = query_json(
            db.conn(),
            "SELECT 'R' || work || '-' || round AS name, * FROM work_history WHERE work = ?1 ORDER BY round",
            &[&work],
            usize::MAX,
        )?;
        // Each round as it was given: the texts its record keeps once put back.
        let rows: Vec<Value> = rows.iter().map(|r| db::put_back(db.conn(), r)).collect();
        return serde_json::to_string_pretty(&rows).map_err(sql);
    }
    show(
        data,
        "SELECT 'R' || r.work || '-' || r.n AS round, r.cell, r.transition, json_extract(r.rejected, '$') AS rejected,
                json_extract(r.output, '$.progress') AS progress, r.model, r.compactions,
                (r.ended_at - r.started_at) / 1000 AS secs, json_extract(r.usage, '$.cost_usd') AS cost_usd
         FROM rounds r WHERE r.work = ?1 ORDER BY r.n",
        &[&work],
        60,
    )
}

pub fn counters(data: &Path, work: i64) -> Result<String, String> {
    show(
        data,
        "SELECT counter, times FROM work_counters WHERE work = ?1 ORDER BY counter",
        &[&work],
        40,
    )
}

/// What went wrong or ran long: returns sent back, compactions, counters past the project's limits, works paused, and
/// the keep-warm pings, each a wait of over an hour, with where the work waited and for what.
pub fn anomalies(data: &Path) -> Result<String, String> {
    let c = config::committed(data).map_err(sql)?.config;
    let db = open(data)?;
    let mut out = String::new();
    out.push_str("Returns sent back:\n");
    out.push_str(&table(
        &query_json_counted(
            db.conn(),
            "SELECT work, 'R' || work || '-' || n AS round, cell, rejected FROM rounds WHERE rejected IS NOT NULL
             ORDER BY work, n",
            &[],
            TABLE_ROWS,
        )?,
        100,
    ));
    out.push_str("\nCompactions:\n");
    out.push_str(&table(
        &query_json_counted(
            db.conn(),
            "SELECT work, 'R' || work || '-' || n AS round, cell, compactions FROM rounds WHERE compactions > 0
             ORDER BY work, n",
            &[],
            TABLE_ROWS,
        )?,
        40,
    ));
    out.push_str("\nCounters past the project's limits:\n");
    let counters = query_json_counted(
        db.conn(),
        "SELECT k.work, i.project, k.counter, k.times FROM work_counters k JOIN works w ON w.id = k.work
         JOIN items i ON i.id = w.item",
        &[],
        TABLE_ROWS,
    )?;
    let over: Vec<Value> = counters
        .rows
        .into_iter()
        .filter(|r| {
            let limit = c
                .projects
                .get(r["project"].as_str().unwrap_or_default())
                .and_then(|p| p.limits.get(r["counter"].as_str().unwrap_or_default()));
            limit.is_some_and(|l| r["times"].as_u64().unwrap_or(0) > *l)
        })
        .collect();
    // The rows are cut before they are weighed against the limits, so the counters left out were not looked at, and
    // any of them may be past its limit; the table holds the ones past their limits among those read, not the rows
    // read, and its last line says so in its own words.
    out.push_str(&table_body(&over, 40));
    out.push_str(&unchecked_line(counters.left_out, TABLE_ROWS));
    out.push_str("\nWorks paused:\n");
    out.push_str(&table(
        &query_json_counted(
            db.conn(),
            "SELECT id AS work, item, at, paused FROM works WHERE paused IS NOT NULL",
            &[],
            1000,
        )?,
        100,
    ));
    out.push_str("\nKeep-warm pings, each a wait of over an hour:\n");
    out.push_str(&table(
        &query_json_counted(
            db.conn(),
            "SELECT s.work, s.line, json_extract(p.waited, '$.cell') AS cell, json_extract(p.waited, '$.on') AS waited_on,
                    json_extract(p.waited, '$.what') AS what, p.started_at AS at,
                    round(json_extract(p.usage, '$.cost_usd'), 4) AS cost_usd, p.error
             FROM pings p JOIN sessions s ON s.id = p.session ORDER BY s.work, p.id",
            &[],
            TABLE_ROWS,
        )?,
        100,
    ));
    Ok(out)
}

/// Time and cost of each work, its keep-warm pings counted, and of its rounds by cell, with its pings apart.
pub fn cost(data: &Path, work: Option<i64>) -> Result<String, String> {
    match work {
        Some(w) => {
            let db = open(data)?;
            let cells = query_json_counted(
                db.conn(),
                "SELECT cell, count(*) AS rounds, sum(ended_at - started_at) / 1000 AS secs,
                        round(sum(coalesce(json_extract(usage, '$.cost_usd'), 0)), 2) AS cost_usd
                 FROM rounds WHERE work = ?1 GROUP BY cell ORDER BY min(n)",
                &[&w],
                TABLE_ROWS,
            )?;
            let mut out = table_body(&cells.rows, 40);
            let (pings, cost): (i64, f64) = db
                .conn()
                .query_row(
                    "SELECT count(*), coalesce(sum(json_extract(p.usage, '$.cost_usd')), 0) FROM pings p
                     JOIN sessions s ON s.id = p.session WHERE s.work = ?1",
                    [w],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .map_err(sql)?;
            if pings > 0 {
                out.push_str(&format!("keep-warm pings\t{pings}\t\t{cost:.2}\n"));
            }
            // The pings row is no row of the table either, and what the table leaves out is said last of all.
            out.push_str(&left_out_line(cells.left_out, cells.rows.len()));
            Ok(out)
        }
        None => show(
            data,
            "SELECT work, count(*) AS rounds, (max(coalesce(ended_at, started_at)) - min(started_at)) / 1000 AS secs,
                    round(sum(coalesce(json_extract(usage, '$.cost_usd'), 0)) + coalesce((SELECT
                      sum(json_extract(p.usage, '$.cost_usd')) FROM pings p JOIN sessions s ON s.id = p.session
                      WHERE s.work = rounds.work), 0), 2) AS cost_usd
             FROM rounds GROUP BY work ORDER BY work",
            &[],
            40,
        ),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    // A table that leaves no row out is its rows, as it always was; one that leaves rows out ends with one more line,
    // which holds no tab so that it reads as no row, and says "1 row" for one. The text is worked out by hand: the
    // head from the first row's keys, a newline in a value as a space, and a null as nothing.
    #[test]
    fn a_table_ends_with_a_line_for_the_rows_it_leaves_out() {
        let rows = vec![json!({"a": 1, "b": "x\ny"}), json!({"a": 2, "b": null})];
        let counted = |left_out| Counted {
            rows: rows.clone(),
            left_out,
        };
        let body = "a\tb\n1\tx y\n2\t\n";
        assert_eq!(table(&counted(0), 40), body);
        assert_eq!(
            table(&counted(1), 40),
            format!("{body}(1 row left out after the first 2)\n")
        );
        assert_eq!(
            table(&counted(5), 40),
            format!("{body}(5 rows left out after the first 2)\n")
        );
        assert!(!left_out_line(5, 2).contains('\t'));
        // No rows at all is still "(none)".
        let none = Counted {
            rows: vec![],
            left_out: 0,
        };
        assert_eq!(table(&none, 40), "(none)\n");
    }

    // The counters table says what it did not look at, not rows of itself left out; no tab, one or many.
    #[test]
    fn the_line_for_counters_not_checked_says_so() {
        assert_eq!(unchecked_line(0, 10), "");
        assert_eq!(
            unchecked_line(1, 10),
            "(1 counter after the first 10 was not checked against the limits)\n"
        );
        assert_eq!(
            unchecked_line(4, 10),
            "(4 counters after the first 10 were not checked against the limits)\n"
        );
        assert!(!unchecked_line(4, 10).contains('\t'));
    }
}
