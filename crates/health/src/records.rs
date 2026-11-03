//! The measures computed from the records, each from the rows that fall in the time read and no others: every
//! statement reads its table by a time it is indexed by, so that reading a week costs the same however long the
//! history is. A join from a table read by time to the rows it names is written `CROSS JOIN`, which keeps SQLite from
//! reading the named table first.

use std::collections::{BTreeMap, BTreeSet};

use db::rusqlite::types::Value as Sql;
use serde_json::Value;

use crate::round::{Outcome, context_end, outcome};
use crate::{Fact, MainLog, Reader, args, project_arg};

/// The graph works on items run.
const ENGINEER: &str = "engineer";

/// The project an engineer's work or an architect's is of, for a statement that reads works `w` and items `i`.
const PROJECT_OF_WORK: &str = "(?1 IS NULL OR coalesce(i.project, w.project) = ?1)";

/// The checks of main after a merge (cell `L1`) that found main broken by the merge, which the engine reverted.
pub(crate) fn broken_merges(r: &Reader<'_>, project: Option<&str>, from: i64, to: i64) -> Result<Vec<Fact>, String> {
    let sql = format!(
        "SELECT r.ended_at FROM rounds r CROSS JOIN works w ON w.id = r.work JOIN items i ON i.id = w.item
         WHERE r.ended_at >= ?2 AND r.ended_at < ?3 AND r.cell IN {}
           AND json_extract(r.output, '$.main.kind') = 'broken_by_merge' AND (?1 IS NULL OR i.project = ?1)",
        db::cells_sql(ENGINEER, &["L1"])
    );
    r.rows(&sql, args(project, from, to), |row| {
        Ok(Fact::event(row.get(0)?, "", 1.0))
    })
}

/// The subject a round's cell is, as `<graph>/<cell code>`: the code the cell has now, whatever id the round's flow
/// version gave it.
fn cell_subject(graph: &str, cell: Option<&str>) -> String {
    match cell {
        Some(c) => format!("{graph}/{}", db::code_of(graph, c).unwrap_or(c)),
        None => format!("{graph}/off the graph"),
    }
}

/// The context each agent's round that reported its usage ended with, by its cell.
pub(crate) fn round_context(r: &Reader<'_>, project: Option<&str>, from: i64, to: i64) -> Result<Vec<Fact>, String> {
    let sql = format!(
        "SELECT r.ended_at, w.graph, r.cell, r.usage FROM rounds r CROSS JOIN works w ON w.id = r.work
           LEFT JOIN items i ON i.id = w.item
         WHERE r.ended_at >= ?2 AND r.ended_at < ?3 AND r.session IS NOT NULL AND r.usage IS NOT NULL
           AND {PROJECT_OF_WORK}"
    );
    let rows = r.rows(&sql, args(project, from, to), |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, String>(3)?,
        ))
    })?;
    Ok(rows
        .into_iter()
        .filter_map(|(at, graph, cell, usage)| {
            let usage: Value = serde_json::from_str(&usage).ok()?;
            let end = context_end(&usage)?;
            Some(Fact::event(at, cell_subject(&graph, cell.as_deref()), end as f64))
        })
        .collect())
}

/// How each agent's round that ended came out.
fn outcomes(r: &Reader<'_>, project: Option<&str>, from: i64, to: i64) -> Result<Vec<(i64, Outcome)>, String> {
    let sql = format!(
        "SELECT r.ended_at, r.transition, r.rejected FROM rounds r CROSS JOIN works w ON w.id = r.work
           LEFT JOIN items i ON i.id = w.item
         WHERE r.ended_at >= ?2 AND r.ended_at < ?3 AND r.session IS NOT NULL AND {PROJECT_OF_WORK}"
    );
    r.rows(&sql, args(project, from, to), |row| {
        let rejected: Option<String> = row.get(2)?;
        let rejected: Option<Value> = rejected.and_then(|s| serde_json::from_str(&s).ok());
        Ok((
            row.get(0)?,
            outcome(row.get::<_, Option<String>>(1)?.as_deref(), rejected.as_ref(), true),
        ))
    })
}

/// Each agent's round that ended: 1 when its return was sent back, 0 otherwise.
pub(crate) fn sent_back(r: &Reader<'_>, project: Option<&str>, from: i64, to: i64) -> Result<Vec<Fact>, String> {
    Ok(outcomes(r, project, from, to)?
        .into_iter()
        .map(|(at, o)| Fact::event(at, "", f64::from(u8::from(o == Outcome::SentBack))))
        .collect())
}

/// Each agent's round that ended: 1 when its run failed or was interrupted, 0 otherwise.
pub(crate) fn failed(r: &Reader<'_>, project: Option<&str>, from: i64, to: i64) -> Result<Vec<Fact>, String> {
    Ok(outcomes(r, project, from, to)?
        .into_iter()
        .map(|(at, o)| {
            let failed = matches!(o, Outcome::Failed | Outcome::Interrupted);
            Fact::event(at, "", f64::from(u8::from(failed)))
        })
        .collect())
}

/// The owner's words to each architect that a round handled, each with how long they waited for that round's end.
pub(crate) fn owner_reply(r: &Reader<'_>, project: Option<&str>, from: i64, to: i64) -> Result<Vec<Fact>, String> {
    r.rows(
        "SELECT x.ended_at, x.ended_at - m.at FROM architect_matters m
           CROSS JOIN rounds x ON x.work = m.handled_work AND x.n = m.handled_round
         WHERE m.kind = 'owner_words' AND m.at < ?3 AND x.ended_at >= ?2 AND x.ended_at < ?3
           AND (?1 IS NULL OR m.project = ?1)",
        args(project, from, to),
        |row| Ok(Fact::event(row.get(0)?, "", row.get::<_, i64>(1)? as f64)),
    )
}

/// Each time a work stood stopped, waiting for someone, as a span from when it stopped to when it went on, or now: a
/// blocker, by its kind, and a pause of a work not ended, as `paused`.
pub(crate) fn stops(r: &Reader<'_>, project: Option<&str>, from: i64, to: i64) -> Result<Vec<Fact>, String> {
    let sql = format!(
        "SELECT b.at, b.ended_at, b.kind FROM blockers b JOIN works w ON w.id = b.work LEFT JOIN items i ON i.id = w.item
         WHERE b.at < ?3 AND (b.at >= ?2 OR b.ended_at IS NULL OR b.ended_at > ?2) AND {PROJECT_OF_WORK}"
    );
    let mut out = r.rows(&sql, args(project, from, to), |row| {
        let until: Option<i64> = row.get(1)?;
        Ok(Fact::span(
            row.get(0)?,
            until.unwrap_or(r.now),
            row.get::<_, String>(2)?,
        ))
    })?;
    out.extend(pauses(r, project, from, to)?);
    Ok(out)
}

/// Whether a work's row, as a change to it holds it, is paused and not ended.
const PAUSED_OLD: &str =
    "(json_extract(c.old_row, '$.paused') IS NOT NULL AND json_extract(c.old_row, '$.ended') IS NULL)";
const PAUSED_NEW: &str =
    "(json_extract(c.new_row, '$.paused') IS NOT NULL AND json_extract(c.new_row, '$.ended') IS NULL)";

/// Each pause of a work, as the changes to the works record when each began and ended, that went on at any time from
/// `from` up to `to`: from when it began, or from before `from` for one that began earlier, to when it ended or the
/// work ended, or now. A pause going on at `from` is found by its end since, or, still going on, by the work's row.
fn pauses(r: &Reader<'_>, project: Option<&str>, from: i64, to: i64) -> Result<Vec<Fact>, String> {
    let sql = format!(
        "SELECT c.at, w.id, {PAUSED_NEW} FROM changes c CROSS JOIN works w ON w.id = json_extract(c.row_key, '$.id')
           LEFT JOIN items i ON i.id = w.item
         WHERE c.table_name = 'works' AND c.at >= ?2 AND {PAUSED_OLD} <> {PAUSED_NEW} AND {PROJECT_OF_WORK}
         ORDER BY c.at, c.id"
    );
    // Each work's pauses beginning (true) and ending (false) from `from` on, in order.
    let mut turns: BTreeMap<i64, Vec<(i64, bool)>> = BTreeMap::new();
    let since = vec![project_arg(project), Sql::Integer(from)];
    for (at, work, on) in r.rows(&sql, since, |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?, row.get::<_, bool>(2)?))
    })? {
        turns.entry(work).or_default().push((at, on));
    }
    let sql = format!(
        "SELECT w.id FROM works w LEFT JOIN items i ON i.id = w.item
         WHERE w.paused IS NOT NULL AND w.ended IS NULL AND {PROJECT_OF_WORK}"
    );
    let paused_now: BTreeSet<i64> = r
        .rows(&sql, vec![project_arg(project)], |row| row.get(0))?
        .into_iter()
        .collect();
    let mut spans = Vec::new();
    for ts in turns.values() {
        // A first turn that ends a pause ends one that began before `from`.
        let mut began = ts.first().filter(|(_, on)| !on).map(|_| i64::MIN);
        for &(at, on) in ts {
            match (on, began) {
                (true, None) => began = Some(at),
                (false, Some(b)) => {
                    spans.push((b, at));
                    began = None;
                }
                _ => {}
            }
        }
        if let Some(b) = began {
            spans.push((b, r.now));
        }
    }
    // A work paused now that has not turned since `from` was paused all along.
    let all_along = paused_now.iter().filter(|w| !turns.contains_key(w)).count();
    spans.extend(std::iter::repeat_n((i64::MIN, r.now), all_along));
    Ok(spans
        .into_iter()
        .filter(|&(b, e)| b < to && (b >= from || e > from))
        .map(|(b, e)| Fact::span(b, e, "paused"))
        .collect())
}

/// Each merge by the flow (cell `M2`) from `from` up to `to`, as a statement's `FROM` and `WHERE`: the round `r` that
/// merged, its work `w` and its item `i`.
fn merges_sql(select: &str) -> String {
    format!(
        "SELECT {select} FROM rounds r CROSS JOIN works w ON w.id = r.work JOIN items i ON i.id = w.item
         WHERE r.ended_at >= ?2 AND r.ended_at < ?3 AND r.transition = 'merged' AND r.cell IN {}
           AND (?1 IS NULL OR i.project = ?1)",
        db::cells_sql(ENGINEER, &["M2"])
    )
}

/// Each item merged, with how long it took from the first round of any of its works to its merge.
pub(crate) fn to_merge(r: &Reader<'_>, project: Option<&str>, from: i64, to: i64) -> Result<Vec<Fact>, String> {
    let sql = merges_sql(
        "r.ended_at, r.ended_at - (SELECT min(x.started_at) FROM works v CROSS JOIN rounds x ON x.work = v.id
                                   WHERE v.item = w.item)",
    );
    r.rows(&sql, args(project, from, to), |row| {
        Ok(Fact::event(row.get(0)?, "", row.get::<_, i64>(1)? as f64))
    })
}

/// Each item merged, with how long its works waited for CI before the merge: each run of ci, ci_before and full_ci
/// from when it was queued to when it ended, added up.
pub(crate) fn ci_wait(r: &Reader<'_>, project: Option<&str>, from: i64, to: i64) -> Result<Vec<Fact>, String> {
    let sql = merges_sql(
        "r.ended_at, (SELECT coalesce(sum(t.ended_at - t.queued_at), 0) FROM works v
                      CROSS JOIN tool_runs t ON t.work = v.id
                      WHERE v.item = w.item AND t.tool IN ('ci', 'ci_before', 'full_ci')
                        AND t.ended_at IS NOT NULL AND t.ended_at <= r.ended_at)",
    );
    r.rows(&sql, args(project, from, to), |row| {
        Ok(Fact::event(row.get(0)?, "", row.get::<_, i64>(1)? as f64))
    })
}

/// Each item merged.
pub(crate) fn items_merged(r: &Reader<'_>, project: Option<&str>, from: i64, to: i64) -> Result<Vec<Fact>, String> {
    r.rows(&merges_sql("r.ended_at"), args(project, from, to), |row| {
        Ok(Fact::event(row.get(0)?, "", 1.0))
    })
}

/// Each merge by the flow whose commit is not on main's first-parent line, when its round ended: of the merges that
/// ended no later than the newest commit of main read was made, since main as read cannot hold one that ended after it.
/// A merge recorded with no commit is not on main either.
pub(crate) fn not_on_main(
    r: &Reader<'_>,
    project: Option<&str>,
    main: &MainLog,
    from: i64,
    to: i64,
) -> Result<Vec<Fact>, String> {
    let merges = r.rows(
        &merges_sql("r.ended_at, json_extract(r.output, '$.result.merge_commit')"),
        args(project, from, to),
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?)),
    )?;
    Ok(merges
        .into_iter()
        .filter(|(ended, commit)| *ended <= main.tip_at && commit.as_deref().is_none_or(|c| !main.holds(c)))
        .map(|(ended, _)| Fact::event(ended, "", 1.0))
        .collect())
}

/// Each commit on main's first-parent line made from `from` up to `to`, by its committer time, that came onto main
/// without the flow: no merge by the flow (cell `M2`) has it as its commit, nor any revert the engine made of a merge
/// that broke main (`reverts`). A commit is told by its full id, which belongs to one repository; the project is not
/// read. The merges are read by their commit (index `rounds_merge_commit`), the expression written as the index has it.
pub(crate) fn direct_pushes(
    r: &Reader<'_>,
    _project: Option<&str>,
    main: &MainLog,
    from: i64,
    to: i64,
) -> Result<Vec<Fact>, String> {
    let made: BTreeMap<&str, i64> = main
        .at
        .iter()
        .filter(|(_, at)| (from..to).contains(*at))
        .map(|(id, at)| (id.as_str(), *at))
        .collect();
    if made.is_empty() {
        return Ok(Vec::new());
    }
    let ids = Value::from(made.keys().copied().collect::<Vec<&str>>()).to_string();
    let sql = format!(
        "SELECT s.value FROM json_each(?1) s
         WHERE NOT EXISTS (SELECT 1 FROM rounds r
                           WHERE json_extract(r.output, '$.result.merge_commit') = s.value
                             AND r.transition = 'merged' AND r.cell IN {})
           AND NOT EXISTS (SELECT 1 FROM reverts v WHERE v.revert_commit = s.value)",
        db::cells_sql(ENGINEER, &["M2"])
    );
    let pushed = r.rows(&sql, vec![Sql::Text(ids)], |row| row.get::<_, String>(0))?;
    Ok(pushed
        .iter()
        .filter_map(|id| made.get(id.as_str()))
        .map(|&at| Fact::event(at, "", 1.0))
        .collect())
}

/// Each item made here, by the change that inserted it, with when: an item imported from outside the flow was not
/// made here.
pub(crate) fn made(r: &Reader<'_>, project: Option<&str>, from: i64, to: i64) -> Result<Vec<(i64, i64)>, String> {
    r.rows(
        "SELECT i.id, c.at FROM changes c CROSS JOIN items i ON i.id = json_extract(c.row_key, '$.id')
         WHERE c.table_name = 'items' AND c.at >= ?2 AND c.at < ?3 AND c.old_row IS NULL AND i.imported_from IS NULL
           AND (?1 IS NULL OR i.project = ?1)
         ORDER BY c.at, c.id",
        args(project, from, to),
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
}

/// Each item made here.
pub(crate) fn items_made(r: &Reader<'_>, project: Option<&str>, from: i64, to: i64) -> Result<Vec<Fact>, String> {
    Ok(made(r, project, from, to)?
        .into_iter()
        .map(|(_, at)| Fact::event(at, "", 1.0))
        .collect())
}

/// Each time an item was cancelled.
pub(crate) fn items_cancelled(r: &Reader<'_>, project: Option<&str>, from: i64, to: i64) -> Result<Vec<Fact>, String> {
    r.rows(
        "SELECT c.at FROM changes c
         WHERE c.table_name = 'items' AND c.at >= ?2 AND c.at < ?3
           AND json_extract(c.new_row, '$.cancelled') IS NOT NULL AND json_extract(c.old_row, '$.cancelled') IS NULL
           AND (?1 IS NULL OR json_extract(c.new_row, '$.project') = ?1)",
        args(project, from, to),
        |row| Ok(Fact::event(row.get(0)?, "", 1.0)),
    )
}

/// What each round that reported its cost cost, by the graph of its work, when it started.
pub(crate) fn spend(r: &Reader<'_>, project: Option<&str>, from: i64, to: i64) -> Result<Vec<Fact>, String> {
    let sql = format!(
        "SELECT r.started_at, w.graph, json_extract(r.usage, '$.cost_usd') FROM rounds r
           CROSS JOIN works w ON w.id = r.work LEFT JOIN items i ON i.id = w.item
         WHERE r.started_at >= ?2 AND r.started_at < ?3 AND json_extract(r.usage, '$.cost_usd') IS NOT NULL
           AND {PROJECT_OF_WORK}"
    );
    r.rows(&sql, args(project, from, to), |row| {
        Ok(Fact::event(row.get(0)?, row.get::<_, String>(1)?, row.get(2)?))
    })
}
