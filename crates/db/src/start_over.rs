//! What is asked of a work from outside its step, as the owner or its project's architect asks it: starting it over,
//! cancelling its item and lifting its pause. Each request is an action of the outbox, written in the asker's
//! transaction, so that the command line's process and an architect's step can ask as the engine's loop can, and a
//! restart loses none; the loop carries it out once the work has no step in flight. A round of the work that runs when
//! a stop is asked hands in nothing, and the merge step stops before it merges.

use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde_json::json;

use crate::{Actor, Error, Result, cells_sql, now};

/// The outbox's action that starts a work over.
pub const START_OVER: &str = "start_over";

/// The outbox's action that cancels an item, whose arguments name the item.
pub const CANCEL_ITEM: &str = "cancel_item";

/// The outbox's action that lifts a work's pause.
pub const RESUME_WORK: &str = "resume_work";

/// The condition, on a row `w` of `works`, that the work is asked to stop: to start over, or by its item's cancel,
/// which names the item so that a work that took the item over since is asked too.
const STOP_ASKED: &str = "EXISTS (SELECT 1 FROM outbox o
       WHERE (o.kind = 'start_over' AND o.work = w.id)
          OR (o.kind = 'cancel_item' AND json_extract(o.args, '$.item') = w.item))";

/// Why a work cannot be started over now, or none when it can: only a work on an item that goes on can be, one not
/// asked to stop, whose item's merge does not stand on main. A work of the architect cannot, nor one ended or at the
/// end of its graph.
pub fn cannot_start_over(conn: &Connection, work: i64) -> Result<Option<String>> {
    let found: Option<(String, String, Option<String>, Option<i64>)> = conn
        .query_row("SELECT graph, at, ended, item FROM works WHERE id = ?1", [work], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })
        .optional()?;
    let Some((graph, at, ended, item)) = found else {
        return Ok(Some(format!("no work {work}")));
    };
    let done: bool = conn.query_row(
        &format!("SELECT ?1 IN {}", cells_sql("engineer", &["E1"])),
        [&at],
        |r| r.get(0),
    )?;
    Ok(if graph != "engineer" {
        Some(format!(
            "work {work} is the architect's: only a work on an item can be started over"
        ))
    } else if ended.is_some() {
        Some(format!("work {work} has ended"))
    } else if done {
        Some(format!("work {work} is done"))
    } else if stop_asked(conn, work)? {
        Some(format!("work {work} is stopping already"))
    } else if item.map(|i| merge_stands(conn, i)).transpose()?.unwrap_or(false) {
        Some(format!("work {work} has merged, and its merge stands on main"))
    } else {
        None
    })
}

/// Asks the engine to start a work over, as `by` asks it, the architect saying why: refused, with why, when it cannot
/// be ([`cannot_start_over`]).
pub fn start_over(tx: &Transaction<'_>, work: i64, by: Actor, why: Option<&str>) -> Result<()> {
    if let Some(no) = cannot_start_over(tx, work)? {
        return Err(Error::Abort(no));
    }
    let mut args = json!({ "by": by.name() });
    if let Some(why) = why.map(str::trim).filter(|w| !w.is_empty()) {
        args["why"] = json!(why);
    }
    tx.execute(
        "INSERT INTO outbox (work, kind, args, run_after) VALUES (?1, ?2, ?3, ?4)",
        params![work, START_OVER, args.to_string(), now()],
    )?;
    Ok(())
}

/// Asks the engine to cancel an item once its work, `work` as asked, has no step in flight, as `by`, the owner or its
/// project's architect, asks it, saying why. An item asked already stays as it was first asked.
pub fn ask_cancel(tx: &Transaction<'_>, work: i64, item: i64, why: &str, by: &str) -> Result<()> {
    let asked: bool = tx.query_row(
        "SELECT EXISTS (SELECT 1 FROM outbox WHERE kind = ?1 AND json_extract(args, '$.item') = ?2)",
        params![CANCEL_ITEM, item],
        |r| r.get(0),
    )?;
    if asked {
        return Ok(());
    }
    let args = json!({ "item": item, "why": why, "by": by });
    tx.execute(
        "INSERT INTO outbox (work, kind, args, run_after) VALUES (?1, ?2, ?3, ?4)",
        params![work, CANCEL_ITEM, args.to_string(), now()],
    )?;
    Ok(())
}

/// Asks the engine to lift a work's pause once its machine has a slot free, as `by` asks it. A work asked already stays
/// as it was first asked.
pub fn ask_resume(tx: &Transaction<'_>, work: i64, by: Actor) -> Result<()> {
    let asked: bool = tx.query_row(
        "SELECT EXISTS (SELECT 1 FROM outbox WHERE work = ?1 AND kind = ?2)",
        params![work, RESUME_WORK],
        |r| r.get(0),
    )?;
    if asked {
        return Ok(());
    }
    let args = json!({ "by": by.name() });
    tx.execute(
        "INSERT INTO outbox (work, kind, args, run_after) VALUES (?1, ?2, ?3, ?4)",
        params![work, RESUME_WORK, args.to_string(), now()],
    )?;
    Ok(())
}

/// Withdraws a request to lift a work's pause: the pause it was for is gone or replaced, as by the engine's pause for a
/// problem, which the owner looks at first, and it must not lift another.
pub fn withdraw_resume(tx: &Transaction<'_>, work: i64) -> Result<()> {
    tx.execute(
        "DELETE FROM outbox WHERE work = ?1 AND kind = ?2",
        params![work, RESUME_WORK],
    )?;
    Ok(())
}

/// The works asked to have their pause lifted, in the order of their ids.
pub fn resume_asked(conn: &Connection) -> Result<Vec<i64>> {
    let mut st = conn.prepare("SELECT DISTINCT work FROM outbox WHERE kind = ?1 ORDER BY work")?;
    let works = st
        .query_map([RESUME_WORK], |r| r.get(0))?
        .collect::<rusqlite::Result<Vec<i64>>>()?;
    Ok(works)
}

/// Whether a work is asked to stop, to start over or by its item's cancel, and the engine has not done it yet: it takes
/// no new step, what a round of it hands in is not taken, and its merge step does not merge.
pub fn stop_asked(conn: &Connection, work: i64) -> Result<bool> {
    let asked: Option<bool> = conn
        .query_row(
            &format!("SELECT {STOP_ASKED} FROM works w WHERE w.id = ?1"),
            [work],
            |r| r.get(0),
        )
        .optional()?;
    Ok(asked.unwrap_or(false))
}

/// The works that go on and are asked to stop, as [`stop_asked`] says, in the order of their ids: the owner's page
/// shows them stopping.
pub fn stopping(conn: &Connection) -> Result<Vec<i64>> {
    let mut st = conn.prepare(&format!(
        "SELECT w.id FROM works w WHERE w.ended IS NULL AND w.at NOT IN {} AND {STOP_ASKED} ORDER BY w.id",
        cells_sql("engineer", &["E1"])
    ))?;
    let works = st
        .query_map([], |r| r.get(0))?
        .collect::<rusqlite::Result<Vec<i64>>>()?;
    Ok(works)
}

/// Whether an item's merge stands on main: a work on it took the merge cell's row `merged`, and watching main after it
/// did not find that the merge broke main, by the row that reverts the merge. A merge reverted and merged again stands.
pub fn merge_stands(conn: &Connection, item: i64) -> Result<bool> {
    let (merge, watch) = (cells_sql("engineer", &["M2"]), cells_sql("engineer", &["L1"]));
    Ok(conn.query_row(
        &format!(
            "SELECT EXISTS (SELECT 1 FROM rounds m JOIN works w ON w.id = m.work
                 WHERE w.item = ?1 AND m.cell IN {merge} AND m.transition = 'merged'
                   AND NOT EXISTS (SELECT 1 FROM rounds l
                                     WHERE l.work = m.work AND l.n > m.n AND l.cell IN {watch}
                                       AND l.transition = 'broken_by_merge'))"
        ),
        [item],
        |r| r.get(0),
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Db;

    // A merge stands once a work took the merge cell's row `merged`, stops standing when watching main reverts it, and
    // stands again once merged again; the rows of another item say nothing of this one.
    #[test]
    fn a_merge_stands_until_it_is_reverted_and_again_once_it_merges_again() {
        let mut db = Db::in_memory().unwrap();
        let round = |db: &mut Db, work: i64, n: i64, cell: &str, transition: &str| {
            db.write(Actor::Engine, |tx| {
                tx.execute(
                    "INSERT INTO rounds (work, n, cell, input, output, transition, config_commit, started_at, ended_at)
                     VALUES (?1, ?2, ?3, '{}', '{}', ?4, 'c', 1, 1)",
                    params![work, n, cell, transition],
                )?;
                Ok(())
            })
            .unwrap();
        };
        db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO flow_versions (id, digest, compiled) VALUES (1, 'd', '{}')",
                [],
            )?;
            for _ in 0..2 {
                tx.execute(
                    "INSERT INTO items (project, title, problem, why, deliver, level) VALUES ('p', 't', 'x', 'y', 'change', 'l')",
                    [],
                )?;
                let item = tx.last_insert_rowid();
                tx.execute(
                    "INSERT INTO works (item, flow_version, graph, at) VALUES (?1, 1, 'engineer', 'L1')",
                    [item],
                )?;
            }
            Ok(())
        })
        .unwrap();
        let stands = |db: &Db, item: i64| merge_stands(db.conn(), item).unwrap();
        assert!(!stands(&db, 1));
        round(&mut db, 1, 1, "M2", "merged");
        assert!(stands(&db, 1) && !stands(&db, 2));
        round(&mut db, 1, 2, "L1", "green");
        assert!(stands(&db, 1));
        round(&mut db, 1, 3, "L1", "broken_by_merge");
        assert!(!stands(&db, 1));
        round(&mut db, 1, 4, "M2", "merged");
        assert!(stands(&db, 1));
    }

    // A work on an item that goes on can be started over once; one of the architect, one ended and one done cannot.
    #[test]
    fn only_a_work_on_an_item_that_goes_on_can_be_started_over() {
        let mut db = Db::in_memory().unwrap();
        db.write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO flow_versions (id, digest, compiled) VALUES (1, 'd', '{}')",
                [],
            )?;
            tx.execute(
                "INSERT INTO items (project, title, problem, why, deliver, level) VALUES ('p', 't', 'x', 'y', 'change', 'l')",
                [],
            )?;
            for (graph, at, ended) in [
                ("engineer", "R2", None),
                ("architect", "Q1", None),
                ("engineer", "R1", Some("item cancelled: no")),
                ("engineer", "E1", None),
            ] {
                let item = (graph == "engineer").then_some(1);
                let project = (graph == "architect").then_some("p");
                tx.execute(
                    "INSERT INTO works (item, project, flow_version, graph, at, ended) VALUES (?1, ?2, 1, ?3, ?4, ?5)",
                    params![item, project, graph, at, ended],
                )?;
            }
            Ok(())
        })
        .unwrap();
        let asked = |db: &mut Db, work: i64| {
            db.write(Actor::Owner, |tx| start_over(tx, work, Actor::Owner, None))
                .map_err(|e| e.to_string())
        };
        assert_eq!(asked(&mut db, 1), Ok(()));
        assert_eq!(asked(&mut db, 1), Err("work 1 is stopping already".into()));
        assert_eq!(
            asked(&mut db, 2),
            Err("work 2 is the architect's: only a work on an item can be started over".into())
        );
        assert_eq!(asked(&mut db, 3), Err("work 3 has ended".into()));
        assert_eq!(asked(&mut db, 4), Err("work 4 is done".into()));
        assert_eq!(asked(&mut db, 9), Err("no work 9".into()));
        let args: String = db
            .conn()
            .query_row(
                "SELECT args FROM outbox WHERE work = 1 AND kind = 'start_over'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(args, r#"{"by":"owner"}"#);
    }
}
