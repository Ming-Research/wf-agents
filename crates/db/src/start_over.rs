//! Starting a work over, as the owner or its project's architect asks: the request is an action of the outbox, written
//! in the asker's transaction, which the engine carries out by taking the work over as it takes over a work on an
//! older flow version. A round of the work that runs when it is asked hands in nothing; the takeover cuts it off.

use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde_json::json;

use crate::{Actor, Error, Result, cells_sql, now};

/// The outbox's action that starts a work over.
pub const START_OVER: &str = "start_over";

/// Why a work cannot be started over now, or none when it can: only a work on an item that goes on can be, one not yet
/// asked to be. A work of the architect cannot, nor one ended or at the end of its graph.
pub fn cannot_start_over(conn: &Connection, work: i64) -> Result<Option<String>> {
    let found: Option<(String, String, Option<String>)> = conn
        .query_row("SELECT graph, at, ended FROM works WHERE id = ?1", [work], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .optional()?;
    let Some((graph, at, ended)) = found else {
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
    } else if start_over_asked(conn, work)? {
        Some(format!("work {work} is being started over already"))
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

/// Whether a work is asked to be started over and the engine has not done it yet.
pub fn start_over_asked(conn: &Connection, work: i64) -> Result<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM outbox WHERE work = ?1 AND kind = ?2)",
        params![work, START_OVER],
        |r| r.get(0),
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Db;

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
        assert_eq!(asked(&mut db, 1), Err("work 1 is being started over already".into()));
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
