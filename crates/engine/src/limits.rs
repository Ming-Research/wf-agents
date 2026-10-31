//! The limits of a work's counters, which the project's configuration sets: a counter that passes its limit is told to
//! the owner once, and the work goes on; the total time is measured, not counted, and its limit is the budget of one
//! stretch, the time since the agents last handed in pause: once a stretch's budget is used up, the engine asks the
//! agent to pause, after its next tool use or at the top of its next round, and the pause waits at cell `X3` for the
//! owner's Start, which begins the next stretch; the project's architect is then asked to find where the work is stuck.

use db::Actor;
use db::rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde_json::json;

use crate::work::Work;
use crate::{Engine, actions};

/// The counter that is the work's total time, measured in minutes.
pub const TOTAL_TIME: &str = "total_time";

/// The row an agent hands in pause by, which ends a stretch.
const PAUSE: &str = "pause";

fn sql(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// The project a work runs in: its item's, or for a work of the architect, its own.
fn project_of(conn: &Connection, work: i64) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT coalesce(i.project, w.project) FROM works w LEFT JOIN items i ON i.id = w.item WHERE w.id = ?1",
        [work],
        |r| r.get(0),
    )
    .optional()
    .map(Option::flatten)
    .map_err(sql)
}

/// The limit the work's project sets on a counter; none when it sets none.
pub fn limit(config: &config::Config, conn: &Connection, work: i64, counter: &str) -> Result<Option<u64>, String> {
    Ok(project_of(conn, work)?
        .and_then(|p| config.projects.get(&p)?.limits.get(counter).copied())
        .filter(|l| *l > 0))
}

/// The rounds of a work that its total time is made of: its agents' rounds, and those of the tools they had the engine
/// run (cell `X1` of either graph, by every id it has had), as the view `work_counters` reads them.
fn timed() -> String {
    let tools = db::cells_sql("engineer", &["X1"]);
    let architect = db::cells_sql("architect", &["X1"]);
    format!("(session IS NOT NULL OR cell IN {tools} OR cell IN {architect})")
}

/// The number of the round the work's agents last handed in pause by before round `before`; 0 when none did.
fn pause_before(conn: &Connection, work: i64, before: i64) -> Result<i64, String> {
    conn.query_row(
        "SELECT coalesce(max(n), 0) FROM rounds WHERE work = ?1 AND n < ?2 AND transition = ?3",
        params![work, before, PAUSE],
        |r| r.get(0),
    )
    .map_err(sql)
}

/// The time of a stretch of the work, in milliseconds: its timed rounds after round `after` up to round `upto`, with the
/// time of one still running up to `now`, when given.
fn stretch_ms(conn: &Connection, work: i64, after: i64, upto: i64, now: Option<i64>) -> Result<i64, String> {
    conn.query_row(
        &format!(
            "SELECT coalesce(sum(coalesce(ended_at, ?4) - started_at), 0) FROM rounds
             WHERE work = ?1 AND n > ?2 AND n <= ?3 AND (ended_at IS NOT NULL OR ?4 IS NOT NULL) AND {}",
            timed()
        ),
        params![work, after, upto, now],
        |r| r.get(0),
    )
    .map_err(sql)
}

/// A budget in minutes, in milliseconds.
fn ms(minutes: u64) -> i64 {
    i64::try_from(minutes)
        .unwrap_or(i64::MAX / 60_000)
        .saturating_mul(60_000)
}

/// The budget of the work's stretch, in minutes, when the stretch going on has used it up: the time since its agents
/// last handed in pause, with a round still running counted up to `now`, when given; none while it lasts, or when
/// there is no limit.
pub fn used_up(config: &config::Config, conn: &Connection, work: i64, now: Option<i64>) -> Result<Option<u64>, String> {
    let Some(minutes) = limit(config, conn, work, TOTAL_TIME)? else {
        return Ok(None);
    };
    let after = pause_before(conn, work, i64::MAX)?;
    Ok((stretch_ms(conn, work, after, i64::MAX, now)? >= ms(minutes)).then_some(minutes))
}

/// The budget of the stretch the work's last pause ended, in minutes, when that stretch used it up: the pause then was
/// the engine's ask, and the owner's Start begins the next stretch.
pub fn ended_used_up(config: &config::Config, conn: &Connection, work: i64) -> Result<Option<u64>, String> {
    let Some(minutes) = limit(config, conn, work, TOTAL_TIME)? else {
        return Ok(None);
    };
    let last = pause_before(conn, work, i64::MAX)?;
    if last == 0 {
        return Ok(None);
    }
    let after = pause_before(conn, work, last)?;
    Ok((stretch_ms(conn, work, after, last, None)? >= ms(minutes)).then_some(minutes))
}

/// Queues for the project's architect a look at a work on an item whose stretch used up its budget, in minutes, as it
/// waits for the owner's Start: where it is stuck and the root cause, which the architect finds and sets straight
/// (`H1`); once for each pause, by the round the agents paused in. A work on no item, the architect's, has none.
pub fn queue_stuck(tx: &Transaction<'_>, work: i64, minutes: u64) -> Result<(), String> {
    let Some((item, project, title)): Option<(i64, String, String)> = tx
        .query_row(
            "SELECT i.id, i.project, i.title FROM works w JOIN items i ON i.id = w.item WHERE w.id = ?1",
            [work],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .map_err(sql)?
    else {
        return Ok(());
    };
    let paused = pause_before(tx, work, i64::MAX)?;
    let queued: i64 = tx
        .query_row(
            "SELECT count(*) FROM architect_matters WHERE kind = 'set_straight' AND item = ?1
             AND json_extract(body, '$.work') = ?2 AND json_extract(body, '$.paused_round') = ?3",
            params![item, work, paused],
            |r| r.get(0),
        )
        .map_err(sql)?;
    if queued > 0 {
        return Ok(());
    }
    let m = db::Matter {
        project,
        kind: "set_straight".into(),
        detail: None,
        what: format!(
            "Work {work} on item {item} ({title}) used up its time budget for a stretch ({minutes} minutes) and waits \
             for the owner's Start: where it is stuck, and why"
        ),
        body: json!({ "work": work, "paused_round": paused, "minutes": minutes }),
        item: Some(item),
        raised: None,
    };
    db::queue_matter(tx, &m).map_err(sql)?;
    Ok(())
}

/// What asks the agent to pause, once the budget of its stretch, in minutes, is used up.
pub fn ask_to_pause(minutes: u64) -> String {
    format!(
        "The work's time budget for this stretch ({minutes} minutes) is used up: hand in pause now, saying in progress \
         how far you got. The owner's Start begins the next stretch."
    )
}

/// Tells the owner once that a counter of the work passed the limit its project sets on it, as the row that counts it
/// is taken: the work goes on.
pub fn tell_past(e: &mut Engine, w: &Work, counter: &str) -> Result<(), String> {
    if counter == TOTAL_TIME {
        return Ok(());
    }
    let Some(limit) = limit(&e.config.config, e.db.conn(), w.id, counter)? else {
        return Ok(());
    };
    let times: i64 =
        e.db.conn()
            .query_row(
                "SELECT coalesce(max(times), 0) FROM work_counters WHERE work = ?1 AND counter = ?2",
                params![w.id, counter],
                |r| r.get(0),
            )
            .map_err(sql)?;
    if u64::try_from(times).ok() != Some(limit + 1) {
        return Ok(());
    }
    let text = format!(
        "Work {} passed the limit its project sets on {counter}: {times} times, the limit {limit}. It goes on; look at \
         it.",
        w.id
    );
    e.db.write(Actor::Engine, |tx| {
        actions::notify(tx, Some(w.id), &text).map_err(db::Error::Abort)
    })
    .map_err(sql)
}

#[cfg(test)]
mod tests {
    use db::Db;

    use super::*;

    /// A round of work 1 its agents handed in pause by.
    fn paused(db: &mut Db, n: i64) {
        db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO rounds (work, n, cell, input, output, transition, config_commit, started_at, ended_at)
                 VALUES (1, ?1, 'R1', '{}', '{}', 'pause', 'c', ?1, ?1)",
                [n],
            )?;
            Ok(())
        })
        .unwrap();
    }

    // A work on an item that used up its budget queues one look for its project's architect at where it is stuck, by
    // the round its agents paused in, however often the engine comes to the pause; a work on no item queues none.
    #[test]
    fn a_work_that_used_up_its_budget_is_looked_at_once_for_each_pause() {
        let mut db = Db::in_memory().unwrap();
        let looks = |db: &mut Db, work: i64| -> Vec<(String, Option<i64>, String)> {
            db.write(Actor::Engine, |tx| queue_stuck(tx, work, 60).map_err(db::Error::Abort))
                .unwrap();
            let mut st = db
                .conn()
                .prepare("SELECT project, item, body FROM architect_matters WHERE kind = 'set_straight' ORDER BY id")
                .unwrap();
            st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
                .unwrap()
                .map(|x| x.unwrap())
                .collect()
        };
        db.write(Actor::Engine, |tx| {
            tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
            tx.execute(
                "INSERT INTO items (project, title, problem, why, deliver, level)
                 VALUES ('p', 't', 'x', 'y', 'change', 'l')",
                [],
            )?;
            tx.execute(
                "INSERT INTO works (item, flow_version, graph, at) VALUES (1, 1, 'engineer', 'X3')",
                [],
            )?;
            tx.execute(
                "INSERT INTO works (item, flow_version, graph, at, project) VALUES (NULL, 1, 'architect', 'X3', 'p')",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        paused(&mut db, 1);
        let once = looks(&mut db, 1);
        assert_eq!(once.len(), 1);
        assert_eq!((once[0].0.as_str(), once[0].1), ("p", Some(1)));
        let body: serde_json::Value = serde_json::from_str(&once[0].2).unwrap();
        assert_eq!(body, json!({ "work": 1, "paused_round": 1, "minutes": 60 }));
        assert_eq!(looks(&mut db, 1).len(), 1);
        assert_eq!(looks(&mut db, 2).len(), 1);
        // The next stretch's pause is looked at again.
        paused(&mut db, 2);
        assert_eq!(looks(&mut db, 1).len(), 2);
    }
}
