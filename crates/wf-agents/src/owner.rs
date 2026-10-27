//! The owner's commands: creating items and ordering the queue, answering asks of the owner and ruling on design
//! cards, interjecting, pausing, and the views of where everything stands. Every write is made as the owner, so
//! `changes` records it as theirs.

use std::path::Path;

use db::rusqlite::{OptionalExtension, params};
use db::{Actor, Db};
use engine::query_json;
use serde_json::Value;

fn sql(e: impl std::fmt::Display) -> String {
    e.to_string()
}

pub fn open(data: &Path) -> Result<Db, String> {
    let path = config::database(data);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    Db::open(&path).map_err(|e| format!("{}: {e}", path.display()))
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
    /// Items it depends on, each with what it needs of it.
    pub depends_on: Vec<(i64, String)>,
    /// Puts it last in the dispatch queue.
    pub queue: bool,
    pub imported: Imported,
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
        for (d, needs) in &x.depends_on {
            db::add_dependency(tx, id, *d, needs)?;
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

pub fn depend(data: &Path, item: i64, on: i64, needs: &str) -> Result<(), String> {
    open(data)?
        .write(Actor::Owner, |tx| db::add_dependency(tx, item, on, needs))
        .map_err(sql)
}

pub fn cancel(data: &Path, item: i64, why: &str) -> Result<(), String> {
    open(data)?
        .write(Actor::Owner, |tx| {
            tx.execute("UPDATE items SET cancelled = ?2 WHERE id = ?1", params![item, why])?;
            tx.execute("DELETE FROM dispatch_queue WHERE item = ?1", [item])?;
            tx.execute(
                "UPDATE works SET ended = ?2 WHERE item = ?1 AND ended IS NULL",
                params![item, format!("item cancelled: {why}")],
            )?;
            Ok(())
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
}

/// Answers an ask of the owner, as the page does.
pub fn answer(data: &Path, a: Answer) -> Result<String, String> {
    let mut db = open(data)?;
    let given = api::Answer {
        text: a.text,
        why: a.why,
        reply: a.reply,
    };
    api::answer(&mut db, a.ask, &given)
}

/// How the owner rules on a design card.
pub struct Ruling {
    pub design_card: i64,
    /// Confirm or disagree.
    pub verdict: String,
    pub text: String,
    /// The approach chosen instead of the one recommended.
    pub option: Option<String>,
}

/// Rules on a design card, as the page does.
pub fn rule(data: &Path, r: Ruling) -> Result<String, String> {
    let mut db = open(data)?;
    let given = api::Ruling {
        verdict: r.verdict,
        text: r.text,
        option: r.option,
    };
    api::rule(&mut db, r.design_card, &given)
}

pub fn say(data: &Path, work: i64, line: &str, text: &str) -> Result<i64, String> {
    open(data)?
        .write(Actor::Owner, |tx| {
            tx.execute(
                "INSERT INTO interjections (work, line, speaker, text) VALUES (?1, ?2, 'owner', ?3)",
                params![work, line, text],
            )?;
            Ok(tx.last_insert_rowid())
        })
        .map_err(sql)
}

/// Lets a paused work go on, as the page does.
pub fn resume(data: &Path, work: i64) -> Result<String, String> {
    let mut db = open(data)?;
    api::resume(&mut db, work)
}

pub fn pause(data: &Path, work: i64, why: Option<&str>) -> Result<(), String> {
    open(data)?
        .write(Actor::Owner, |tx| {
            tx.execute("UPDATE works SET paused = ?2 WHERE id = ?1", params![work, why])?;
            Ok(())
        })
        .map_err(sql)
}

/// Prints rows as a table, one line per row, each value cut to `width`.
fn table(rows: &[Value], width: usize) -> String {
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
    Ok(table(&query_json(db.conn(), q, args, 10_000)?, width))
}

pub fn items(data: &Path) -> Result<String, String> {
    show(
        data,
        "SELECT i.id, i.project, i.title, i.deliver, i.level, i.cancelled,
                (SELECT position FROM dispatch_queue q WHERE q.item = i.id) AS queued,
                (SELECT group_concat(depends_on) FROM item_links l WHERE l.item = i.id) AS depends_on,
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
                (SELECT group_concat(a.id) FROM owner_asks a JOIN rounds r ON r.id = a.round
                 WHERE r.work = w.id AND a.answer IS NULL) AS open_asks,
                (SELECT group_concat(c.id) FROM design_cards c JOIN rounds r ON r.id = c.round
                 WHERE r.work = w.id AND c.status = 'open') AS open_design_cards,
                (SELECT count(*) FROM interjections x WHERE x.work = w.id AND x.delivered_round IS NULL) AS waiting_says,
                (SELECT r.cell || ' ' || coalesce(r.transition, CASE WHEN r.rejected IS NOT NULL THEN 'sent back' ELSE 'running' END)
                 FROM rounds r WHERE r.work = w.id ORDER BY r.id DESC LIMIT 1) AS last
         FROM works w LEFT JOIN items i ON i.id = w.item ORDER BY w.id",
        &[],
        40,
    )
}

pub fn asks(data: &Path, all: bool) -> Result<String, String> {
    let q = format!(
        "SELECT a.id AS ask, r.work, w.item, r.cell, a.body, a.answer FROM owner_asks a
         JOIN rounds r ON r.id = a.round JOIN works w ON w.id = r.work {} ORDER BY a.id",
        if all { "" } else { "WHERE a.answer IS NULL" }
    );
    show(data, &q, &[], 80)
}

pub fn ask(data: &Path, id: i64) -> Result<String, String> {
    let db = open(data)?;
    let rows = query_json(
        db.conn(),
        "SELECT a.id AS ask, r.work, r.cell, a.body, a.answer FROM owner_asks a
         JOIN rounds r ON r.id = a.round WHERE a.id = ?1",
        &[&id],
        1,
    )?;
    let row = rows.first().ok_or_else(|| format!("no ask {id}"))?;
    serde_json::to_string_pretty(row).map_err(sql)
}

pub fn design_cards(data: &Path, all: bool) -> Result<String, String> {
    let q = format!(
        "SELECT c.id AS design_card, c.status, r.work, w.item, r.cell, c.body FROM design_cards c
         JOIN rounds r ON r.id = c.round JOIN works w ON w.id = r.work {} ORDER BY c.id",
        if all { "" } else { "WHERE c.status <> 'replaced'" }
    );
    show(data, &q, &[], 80)
}

pub fn design_card(data: &Path, id: i64) -> Result<String, String> {
    let db = open(data)?;
    let rows = query_json(
        db.conn(),
        "SELECT c.id AS design_card, c.status, r.work, r.cell, c.body, c.rulings, c.replaced_by FROM design_cards c
         JOIN rounds r ON r.id = c.round WHERE c.id = ?1",
        &[&id],
        1,
    )?;
    let row = rows.first().ok_or_else(|| format!("no design card {id}"))?;
    serde_json::to_string_pretty(row).map_err(sql)
}

pub fn history(data: &Path, work: i64, full: bool) -> Result<String, String> {
    if full {
        let db = open(data)?;
        let rows = query_json(
            db.conn(),
            "SELECT * FROM work_history WHERE work = ?1 ORDER BY round",
            &[&work],
            10_000,
        )?;
        // Each round as it was given: the texts its record keeps once put back.
        let rows: Vec<Value> = rows.iter().map(|r| db::put_back(db.conn(), r)).collect();
        return serde_json::to_string_pretty(&rows).map_err(sql);
    }
    show(
        data,
        "SELECT r.id AS round, r.cell, r.transition, json_extract(r.rejected, '$') AS rejected,
                json_extract(r.output, '$.progress') AS progress, r.model, r.compactions,
                (r.ended_at - r.started_at) / 1000 AS secs, json_extract(r.usage, '$.cost_usd') AS cost_usd
         FROM rounds r WHERE r.work = ?1 ORDER BY r.id",
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

/// What went wrong or ran long: returns sent back, compactions, counters past the project's limits, works paused.
pub fn anomalies(data: &Path) -> Result<String, String> {
    let c = config::committed(data).map_err(sql)?.config;
    let db = open(data)?;
    let mut out = String::new();
    out.push_str("Returns sent back:\n");
    out.push_str(&table(
        &query_json(
            db.conn(),
            "SELECT work, id AS round, cell, rejected FROM rounds WHERE rejected IS NOT NULL ORDER BY id",
            &[],
            10_000,
        )?,
        100,
    ));
    out.push_str("\nCompactions:\n");
    out.push_str(&table(
        &query_json(
            db.conn(),
            "SELECT work, id AS round, cell, compactions FROM rounds WHERE compactions > 0",
            &[],
            10_000,
        )?,
        40,
    ));
    out.push_str("\nCounters past the project's limits:\n");
    let rows = query_json(
        db.conn(),
        "SELECT k.work, i.project, k.counter, k.times FROM work_counters k JOIN works w ON w.id = k.work
         JOIN items i ON i.id = w.item",
        &[],
        10_000,
    )?;
    let over: Vec<Value> = rows
        .into_iter()
        .filter(|r| {
            let limit = c
                .projects
                .get(r["project"].as_str().unwrap_or_default())
                .and_then(|p| p.limits.get(r["counter"].as_str().unwrap_or_default()));
            limit.is_some_and(|l| r["times"].as_u64().unwrap_or(0) > *l)
        })
        .collect();
    out.push_str(&table(&over, 40));
    out.push_str("\nWorks paused:\n");
    out.push_str(&table(
        &query_json(
            db.conn(),
            "SELECT id AS work, item, at, paused FROM works WHERE paused IS NOT NULL",
            &[],
            1000,
        )?,
        100,
    ));
    Ok(out)
}

/// Time and cost of each work, and of its rounds by cell.
pub fn cost(data: &Path, work: Option<i64>) -> Result<String, String> {
    match work {
        Some(w) => show(
            data,
            "SELECT cell, count(*) AS rounds, sum(ended_at - started_at) / 1000 AS secs,
                    round(sum(coalesce(json_extract(usage, '$.cost_usd'), 0)), 2) AS cost_usd
             FROM rounds WHERE work = ?1 GROUP BY cell ORDER BY min(id)",
            &[&w],
            40,
        ),
        None => show(
            data,
            "SELECT work, count(*) AS rounds, (max(coalesce(ended_at, started_at)) - min(started_at)) / 1000 AS secs,
                    round(sum(coalesce(json_extract(usage, '$.cost_usd'), 0)), 2) AS cost_usd
             FROM rounds GROUP BY work ORDER BY work",
            &[],
            40,
        ),
    }
}
