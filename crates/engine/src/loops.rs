//! A work that takes step after step at cells that run no agent: the engine's own cells, the tool cells, the cells that
//! ask someone, and an agent's cell whose round the engine takes itself. Such a run costs nothing the agents' budgets or
//! the flow's counters see, and nothing else stops it, so the flow's rounds section bounds it with a case of the
//! engine's (`steps_without_agent`, a shared row of every agent's cell): how many steps a work may take in a row with
//! no agent's round among them. The work's next step takes the case instead, which pauses the work where it stands and
//! says at which cells the steps were. The first time, a work on an item has its project's architect look at why, by a
//! matter to set straight, as a work whose time budget is used up does; the next time, it is the owner's to look at.
//! A work of the architect's has no architect above it, and is the owner's each time. Once the work is let go on, it
//! may take as many again; a round the owner asks for is not held to the bound.

use db::Actor;
use db::rusqlite::{Connection, Transaction, params};
use serde_json::json;

use crate::work::{self, Work};
use crate::{ENGINEER, Engine, failed};

fn sql(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// The cells of the work's last `most` steps, newest first, when none of them was its progress and each started
/// after the owner last let the work go on; none when it took fewer such steps in a row. A step is the work's progress
/// when an agent ran in it (it ran in a session) or it handled a matter of the architect's queue, which empties the
/// queue by one.
pub fn steps_without_agent(conn: &Connection, work: i64, most: u32) -> Result<Option<Vec<Option<String>>>, String> {
    let mut st = conn
        .prepare("SELECT session IS NOT NULL, cell, started_at, n FROM rounds WHERE work = ?1 ORDER BY n DESC LIMIT ?2")
        .map_err(sql)?;
    let rows = st
        .query_map(params![work, most], |r| {
            Ok((
                r.get::<_, bool>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, i64>(3)?,
            ))
        })
        .map_err(sql)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sql)?;
    if rows.len() < most as usize || rows.iter().any(|(agent, _, _, _)| *agent) {
        return Ok(None);
    }
    // Every step of every work asks this: the rest is looked up only for a run this long.
    let (oldest_at, oldest_n) = rows.last().map_or((i64::MAX, i64::MAX), |(_, _, at, n)| (*at, *n));
    let handled: bool = conn
        .query_row(
            "SELECT EXISTS (SELECT 1 FROM architect_matters WHERE handled_work = ?1 AND handled_round >= ?2)",
            params![work, oldest_n],
            |r| r.get(0),
        )
        .map_err(sql)?;
    if handled || work::resumed_at(conn, work)?.is_some_and(|at| oldest_at <= at) {
        return Ok(None);
    }
    Ok(Some(rows.into_iter().map(|(_, cell, _, _)| cell).collect()))
}

/// How many times the work's project's architect was asked to look at why the work took so many steps with no agent's
/// round.
fn asked_before(conn: &Connection, work: i64) -> Result<i64, String> {
    conn.query_row(
        "SELECT count(*) FROM architect_matters WHERE kind = 'set_straight'
           AND json_extract(body, '$.work') = ?1 AND json_extract(body, '$.steps_without_agent') IS NOT NULL",
        [work],
        |r| r.get(0),
    )
    .map_err(sql)
}

/// What the architect is asked: why a work on an item took `most` steps with no agent's round, at `cells`, and what
/// to change, while it waits paused at cell `at` for the owner's Start.
struct Look<'a> {
    work: i64,
    item: &'a crate::Item,
    most: u32,
    cells: &'a [String],
    at: &'a str,
}

/// Queues the architect's look at a work that took so many steps with no agent's round, as a matter to set straight.
fn queue_look(tx: &Transaction<'_>, l: &Look<'_>) -> Result<(), String> {
    let m = db::Matter {
        project: l.item.project.clone(),
        kind: "set_straight".into(),
        detail: None,
        what: format!(
            "Work {} on item {} ({}) took {} steps in a row with no agent's round among them, at cells {}, and waits \
             paused at cell {} for the owner's Start: why it took them, and what to change",
            l.work,
            l.item.id,
            l.item.title,
            l.most,
            crate::cards::and_list(l.cells),
            l.at
        ),
        body: json!({ "work": l.work, "steps_without_agent": l.most, "cells": l.cells, "at": l.at }),
        item: Some(l.item.id),
        raised: None,
    };
    db::queue_matter(tx, &m).map_err(sql)?;
    Ok(())
}

/// Takes the flow's case for a work that took as many steps in a row with no agent's round as the case allows, in
/// place of its next step: the work pauses where it stands, saying at which cells the steps were, and the first time
/// its project's architect is asked to look at why, the next time the owner. Gives whether it took the case.
pub fn take_if_past(e: &mut Engine, w: &Work) -> Result<bool, String> {
    let g = e.graph_of(w)?;
    let section = failed::section(e, &g)?;
    let Some((name, most)) = section.engine.iter().find_map(|k| Some((k.name.clone(), k.steps?))) else {
        return Ok(false);
    };
    let Some(steps) = steps_without_agent(e.db.conn(), w.id, most)? else {
        return Ok(false);
    };
    // The cells in the order the work first came to each.
    let mut cells: Vec<String> = Vec::new();
    for c in steps.iter().rev().flatten() {
        if !cells.contains(c) {
            cells.push(c.clone());
        }
    }
    let item = match e.subject(w)? {
        Some(i) if w.graph == ENGINEER && asked_before(e.db.conn(), w.id)? == 0 => Some(i),
        _ => None,
    };
    let took = format!(
        "it took {most} steps in a row with no agent's round among them, at cells {}, and stands at cell {} (the \
         flow's {name})",
        crate::cards::and_list(&cells),
        w.pos.at
    );
    let then = format!("once it is let go on, it may take {most} such steps again");
    let why = match &item {
        Some(_) => format!("{took}; its project's architect is asked to look at why, and {then}"),
        None if w.graph == ENGINEER => format!(
            "{took}; its project's architect was asked to look at why the time before, so it is the owner's to look at \
             now, and {then}"
        ),
        None => format!("{took}; {then}"),
    };
    if let Some(item) = &item {
        let look = Look {
            work: w.id,
            item,
            most,
            cells: &cells,
            at: &w.pos.at,
        };
        e.db.write(Actor::Engine, |tx| queue_look(tx, &look).map_err(db::Error::Abort))
            .map_err(sql)?;
    }
    e.say(&format!("work {} paused: {why}", w.id));
    e.pause(w.id, &why)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use db::Db;

    use super::*;

    /// A round of work `work` at `cell`, started at `at`, in a session of its own, ended with it, when `agent`; gives
    /// its number.
    fn round(db: &mut Db, work: i64, cell: &str, at: i64, agent: bool) -> i64 {
        db.write(Actor::Engine, |tx| {
            let n: i64 = tx.query_row(
                "SELECT coalesce(max(n), 0) + 1 FROM rounds WHERE work = ?1",
                [work],
                |r| r.get(0),
            )?;
            let session = if agent {
                tx.execute(
                    "INSERT INTO sessions (work, line, cli, cli_session, ended) VALUES (?1, ?2, 'claude', ?3, ?4)",
                    params![work, cell, format!("s{work}-{n}"), at],
                )?;
                Some(tx.last_insert_rowid())
            } else {
                None
            };
            tx.execute(
                "INSERT INTO rounds (work, n, cell, session, input, transition, config_commit, started_at, ended_at)
                 VALUES (?1, ?2, ?3, ?4, '{}', 'x', 'c', ?5, ?5)",
                params![work, n, cell, session, at],
            )?;
            Ok(n)
        })
        .unwrap()
    }

    /// A database with work 1 on an item and work 2 of project p's architect.
    fn works() -> Db {
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
            tx.execute(
                "INSERT INTO works (item, flow_version, graph, at, project) VALUES (NULL, 1, 'architect', 'Q0', 'p')",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        db
    }

    fn named(cells: &[&str]) -> Option<Vec<Option<String>>> {
        Some(cells.iter().map(|c| Some(c.to_string())).collect())
    }

    // The last steps are a run without an agent only when none of them was an agent's round and each started after the
    // owner last let the work go on: an agent's round, or the owner's Start, begins the count afresh.
    #[test]
    fn a_run_of_steps_without_an_agent_is_counted_since_its_last_round_or_the_owners_start() {
        let mut db = works();
        let steps = |db: &Db, most: u32| steps_without_agent(db.conn(), 1, most).unwrap();
        assert_eq!(steps(&db, 1), None);
        round(&mut db, 1, "I1", 1, true);
        assert_eq!(steps(&db, 1), None);
        for (at, cell) in (2..).zip(["X1", "A2", "X1", "A2"]) {
            round(&mut db, 1, cell, at, false);
        }
        assert_eq!(steps(&db, 4), named(&["A2", "X1", "A2", "X1"]));
        assert_eq!(steps(&db, 3), named(&["A2", "X1", "A2"]));
        assert_eq!(steps(&db, 5), None);
        // An agent's round begins the count afresh.
        round(&mut db, 1, "I1", 6, true);
        round(&mut db, 1, "X1", 7, false);
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
        round(&mut db, 1, "A2", resumed + 1, false);
        assert_eq!(steps(&db, 1), named(&["A2"]));
        assert_eq!(steps(&db, 2), None);
    }

    // An architect's work that takes the owner's approved policies one after another goes from the look at its queue to
    // routing the matter, to the policy taking effect, which handles the matter, and to where its batch stands, with
    // no agent's round: each matter handled is its progress, so five in a row are no run without an agent, while the
    // same steps handling nothing are.
    #[test]
    fn an_architect_work_that_handles_a_matter_at_each_turn_takes_no_steps_without_an_agent() {
        let mut db = works();
        let mut at = 1;
        for _ in 0..5 {
            for cell in ["Q0", "Q2", "P12", "B1"] {
                let n = round(&mut db, 2, cell, at, false);
                at += 1;
                if cell == "P12" {
                    db.write(Actor::Engine, |tx| {
                        tx.execute(
                            "INSERT INTO architect_matters (project, kind, what, body, at, handled_work, handled_round)
                             VALUES ('p', 'owner_words', 'approved', '{}', 1, 2, ?1)",
                            [n],
                        )?;
                        Ok(())
                    })
                    .unwrap();
                }
            }
        }
        assert_eq!(steps_without_agent(db.conn(), 2, 20).unwrap(), None);
        assert_eq!(steps_without_agent(db.conn(), 2, 2).unwrap(), None);
        assert_eq!(steps_without_agent(db.conn(), 2, 1).unwrap(), named(&["B1"]));
        // The same turns handling no matter are a run without an agent.
        for _ in 0..5 {
            for cell in ["Q0", "Q2", "P12", "B1"] {
                round(&mut db, 2, cell, at, false);
                at += 1;
            }
        }
        assert_eq!(
            steps_without_agent(db.conn(), 2, 20).unwrap().map(|v| v.len()),
            Some(20)
        );
    }
}
