//! Interjections handed to a round that runs: the command line's hook after each tool use asks the session manager for
//! the interjections waiting for the round's line, which it hands in to the round and records as delivered in it, and
//! the command line adds them to the session before the agent goes on. Those still waiting when the round ends go at
//! the top of the next round's message.

use db::rusqlite::params;
use db::{Actor, Db};
use serde_json::{Value, json};

/// The interjections waiting for a work's line, handed in to its running round: each recorded as delivered in the
/// round, and what the hook gives the session, as the command line reads a hook's output after a tool use; none when
/// none waits.
pub fn deliver(db: &mut Db, work: i64, round: i64, line: &str) -> Result<Option<String>, String> {
    let said: Vec<(i64, String, String)> = db
        .write(Actor::Engine, |tx| {
            // A round that has ended, such as the one a keep-warm ping replays the command line of, takes none.
            let open: bool = tx.query_row(
                "SELECT count(*) > 0 FROM rounds WHERE work = ?1 AND n = ?2 AND ended_at IS NULL",
                params![work, round],
                |r| r.get(0),
            )?;
            if !open {
                return Ok(Vec::new());
            }
            let mut st = tx.prepare(
                "SELECT id, speaker, text FROM interjections
                 WHERE work = ?1 AND line = ?2 AND delivered_round IS NULL ORDER BY id",
            )?;
            let rows: Vec<(i64, String, String)> = st
                .query_map(params![work, line], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
                .collect::<Result<_, _>>()?;
            for (id, _, _) in &rows {
                tx.execute(
                    "UPDATE interjections SET delivered_round = ?2 WHERE id = ?1",
                    params![id, round],
                )?;
            }
            Ok(rows)
        })
        .map_err(|e| e.to_string())?;
    if said.is_empty() {
        return Ok(None);
    }
    let lines: Vec<String> = said
        .iter()
        .map(|(id, speaker, text)| format!("### ij:{id}\n\nFrom the {speaker}: {text}"))
        .collect();
    let context = format!(
        "Said to you while this round runs; answer in this round's return, as format:common says.\n\n{}",
        lines.join("\n\n")
    );
    Ok(Some(
        json!({ "hookSpecificOutput": { "hookEventName": "PostToolUse", "additionalContext": context } }).to_string(),
    ))
}

/// The interjections a round was handed while it ran, as `waiting_interjections` gives a row: those recorded as
/// delivered in the round and not among those its message gave.
pub fn delivered_during(db: &Db, work: i64, round: i64, given: &[i64]) -> Result<Vec<Value>, String> {
    let mut st = db
        .conn()
        .prepare(
            "SELECT work, id, line, speaker, text FROM interjections WHERE work = ?1 AND delivered_round = ?2
             ORDER BY id",
        )
        .map_err(|e| e.to_string())?;
    let rows = st
        .query_map(params![work, round], |r| {
            Ok(json!({
                "work": r.get::<_, i64>(0)?,
                "interjection": r.get::<_, i64>(1)?,
                "line": r.get::<_, String>(2)?,
                "speaker": r.get::<_, String>(3)?,
                "text": r.get::<_, String>(4)?,
            }))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<Value>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows
        .into_iter()
        .filter(|r| !r["interjection"].as_i64().is_some_and(|i| given.contains(&i)))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    // An interjection waiting for the line is handed in to the round that runs, once, and recorded as delivered in it;
    // a round that has ended takes none.
    #[test]
    fn an_interjection_is_handed_in_to_the_running_round_once() {
        let mut db = Db::in_memory().unwrap();
        db.write(Actor::Engine, |tx| {
            tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
            tx.execute(
                "INSERT INTO works (flow_version, graph, at) VALUES (1, 'engineer', 'research.problem')",
                [],
            )?;
            tx.execute(
                "INSERT INTO rounds (work, n, cell, input, config_commit, started_at) VALUES (1, 1, 'research.problem', '{}', 'c', 1)",
                [],
            )?;
            tx.execute(
                "INSERT INTO interjections (work, line, speaker, text)
                 VALUES (1, 'engineer', 'owner', 'Look at the log.'), (1, 'review', 'owner', 'Not yours.')",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        let out = deliver(&mut db, 1, 1, "engineer").unwrap().unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["hookSpecificOutput"]["hookEventName"], "PostToolUse");
        let said = v["hookSpecificOutput"]["additionalContext"].as_str().unwrap();
        assert!(
            said.contains("### ij:1\n\nFrom the owner: Look at the log.") && !said.contains("Not yours"),
            "{said}"
        );
        assert_eq!(deliver(&mut db, 1, 1, "engineer").unwrap(), None);
        let during = delivered_during(&db, 1, 1, &[]).unwrap();
        assert_eq!(during.len(), 1);
        assert_eq!(during[0]["speaker"], "owner");
        assert!(delivered_during(&db, 1, 1, &[1]).unwrap().is_empty());
        db.write(Actor::Engine, |tx| {
            tx.execute("UPDATE rounds SET ended_at = 2 WHERE work = 1 AND n = 1", [])?;
            Ok(())
        })
        .unwrap();
        assert_eq!(deliver(&mut db, 1, 1, "review").unwrap(), None);
    }
}
