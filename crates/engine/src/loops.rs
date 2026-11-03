//! A work that goes round cells that run no agent: the engine's own cells, the tool cells, the cells that ask someone,
//! and an agent's cell whose round the engine takes itself. Such a loop costs nothing the agents' budgets or the flow's
//! counters see, and nothing else stops it, so the installation bounds the steps a work takes in a row with no agent's
//! round among them (`agents.steps_without_agent`): the work's next step past the bound pauses it instead, telling the
//! owner the cells it went round. Once the owner lets it go on, it may take as many again; a round the owner asks for
//! is not held to the bound.

use db::rusqlite::{Connection, params};

use crate::Engine;
use crate::work::{self, Work};

fn sql(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// The cells of the work's last `most` steps, newest first, when none of them was an agent's round (one in a session)
/// and each started after the owner last let the work go on; none when it took fewer such steps in a row.
pub fn steps_without_agent(conn: &Connection, work: i64, most: u32) -> Result<Option<Vec<Option<String>>>, String> {
    let mut st = conn
        .prepare("SELECT session IS NOT NULL, cell, started_at FROM rounds WHERE work = ?1 ORDER BY n DESC LIMIT ?2")
        .map_err(sql)?;
    let rows = st
        .query_map(params![work, most], |r| {
            Ok((
                r.get::<_, bool>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, i64>(2)?,
            ))
        })
        .map_err(sql)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sql)?;
    if rows.len() < most as usize || rows.iter().any(|(agent, _, _)| *agent) {
        return Ok(None);
    }
    // Every step of every work asks this: where the owner let the work go on is looked up only for a run this long.
    let oldest = rows.last().map_or(i64::MAX, |(_, _, started_at)| *started_at);
    if work::resumed_at(conn, work)?.is_some_and(|at| oldest <= at) {
        return Ok(None);
    }
    Ok(Some(rows.into_iter().map(|(_, cell, _)| cell).collect()))
}

/// Why the work's next step pauses it instead: it took as many steps in a row with no agent's round as the installation
/// allows; none while it took fewer.
pub fn past_bound(e: &Engine, w: &Work) -> Result<Option<String>, String> {
    let most = e.config.config.install.agents.steps_without_agent.get();
    let Some(steps) = steps_without_agent(e.db.conn(), w.id, most)? else {
        return Ok(None);
    };
    // The cells in the order the work first came to each.
    let mut cells: Vec<String> = Vec::new();
    for c in steps.iter().rev().flatten() {
        if !cells.contains(c) {
            cells.push(c.clone());
        }
    }
    let last = steps.first().cloned().flatten().unwrap_or_default();
    Ok(Some(format!(
        "{most} steps in a row ran no agent, at cells {}, the last at {last}: the work goes round a loop that nothing \
         else stops. Start it once its cause is found; it may then take {most} such steps again (the installation's \
         agents.steps_without_agent)",
        crate::cards::and_list(&cells)
    )))
}

#[cfg(test)]
mod tests {
    use db::{Actor, Db};

    use super::*;

    /// A round of work 1 at `cell`, started at `at`, in a session of its own, ended with it, when `agent`.
    fn round(db: &mut Db, cell: &str, at: i64, agent: bool) {
        db.write(Actor::Engine, |tx| {
            let n: i64 = tx.query_row("SELECT coalesce(max(n), 0) + 1 FROM rounds WHERE work = 1", [], |r| {
                r.get(0)
            })?;
            let session = if agent {
                tx.execute(
                    "INSERT INTO sessions (work, line, cli, cli_session, ended) VALUES (1, ?1, 'claude', ?2, ?3)",
                    params![cell, format!("s{n}"), at],
                )?;
                Some(tx.last_insert_rowid())
            } else {
                None
            };
            tx.execute(
                "INSERT INTO rounds (work, n, cell, session, input, transition, config_commit, started_at, ended_at)
                 VALUES (1, ?1, ?2, ?3, '{}', 'x', 'c', ?4, ?4)",
                params![n, cell, session, at],
            )?;
            Ok(())
        })
        .unwrap();
    }

    // The last steps are a run without an agent only when none of them was an agent's round and each started after the
    // owner last let the work go on: an agent's round, or the owner's Start, begins the count afresh.
    #[test]
    fn a_run_of_steps_without_an_agent_is_counted_since_its_last_round_or_the_owners_start() {
        let mut db = Db::in_memory().unwrap();
        db.write(Actor::Engine, |tx| {
            tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
            tx.execute(
                "INSERT INTO items (project, title, problem, why, deliver, level)
                 VALUES ('p', 't', 'x', 'y', 'change', 'l')",
                [],
            )?;
            tx.execute(
                "INSERT INTO works (item, flow_version, graph, at) VALUES (1, 1, 'engineer', 'X1')",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        let steps = |db: &Db, most: u32| steps_without_agent(db.conn(), 1, most).unwrap();
        let named = |cells: &[&str]| Some(cells.iter().map(|c| Some(c.to_string())).collect::<Vec<_>>());
        assert_eq!(steps(&db, 1), None);
        round(&mut db, "I1", 1, true);
        assert_eq!(steps(&db, 1), None);
        for (at, cell) in (2..).zip(["X1", "A2", "X1", "A2"]) {
            round(&mut db, cell, at, false);
        }
        assert_eq!(steps(&db, 4), named(&["A2", "X1", "A2", "X1"]));
        assert_eq!(steps(&db, 3), named(&["A2", "X1", "A2"]));
        assert_eq!(steps(&db, 5), None);
        // An agent's round begins the count afresh.
        round(&mut db, "I1", 6, true);
        round(&mut db, "X1", 7, false);
        assert_eq!(steps(&db, 2), None);
        assert_eq!(steps(&db, 1), named(&["X1"]));
        // So does the owner's Start after a pause: only the steps after it count.
        db.write(Actor::Engine, |tx| {
            tx.execute("UPDATE works SET paused = 'p', paused_by = 'engine' WHERE id = 1", [])?;
            Ok(())
        })
        .unwrap();
        db.write(Actor::Owner, |tx| {
            tx.execute("UPDATE works SET paused = NULL, paused_by = NULL WHERE id = 1", [])?;
            Ok(())
        })
        .unwrap();
        assert_eq!(steps(&db, 1), None);
        let resumed = work::resumed_at(db.conn(), 1).unwrap().unwrap();
        round(&mut db, "A2", resumed + 1, false);
        assert_eq!(steps(&db, 1), named(&["A2"]));
        assert_eq!(steps(&db, 2), None);
    }
}
