//! Which session an agent's round runs in: its line's session going on, or a new one, and why. A line starts a new
//! session at its first round in a work, at the work's first round of a cell the flow says it starts one at (a row
//! leads there with the action `open_session`), when the context of its session is past the configuration's limit,
//! when its rounds run on another model or under another login account than its session's, when the CLI's prompt cache
//! has let the session go (for a line whose cells the flow says expire with it), and when the owner asks for one; every
//! other round goes on in the line's session. The architect's line, at its first round in a work, takes up the session
//! the project's previous architect work left, unless it ended. A cell on no line has a session of its own every
//! round.

use std::path::Path;

use db::rusqlite::{Connection, OptionalExtension, params};
use flow::CompiledCell;

use crate::held::{self, Held};
use crate::round::Round;

/// Why a session started, as `sessions.started` names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Start {
    /// The line had no session going on: its first round in the work, or the first after its session ended.
    Line,
    /// The round is the work's first at a cell the line starts a new session at.
    Cell,
    /// The context of the line's session was past the configuration's limit.
    Limit,
    /// The line's rounds run on another model than its session's, whose prompt cache holds nothing of it.
    Model,
    /// The line's rounds run under another login account of the CLI than its session's: in another configuration
    /// directory, which does not hold the session, whose log is in the directory it ran in.
    Account,
    /// The CLI's prompt cache let the line's session go: its last call was a cache lifetime or more ago, and the flow
    /// says the line's cells start a new session then, instead of writing the whole session to the cache again.
    Expired,
    /// A new work of the architect goes on in the CLI session the project's previous architect work left.
    Carried,
    /// The owner asked for a new session with the round's Start or Step on the page.
    Owner,
}

impl Start {
    pub fn name(self) -> &'static str {
        match self {
            Start::Line => "line",
            Start::Cell => "cell",
            Start::Limit => "limit",
            Start::Model => "model",
            Start::Account => "account",
            Start::Expired => "expired",
            Start::Carried => "carried",
            Start::Owner => "owner",
        }
    }
}

/// Where a round runs.
#[derive(Debug)]
pub enum Plan {
    /// In its line's session going on: its row, the CLI's id for it, its rounds oldest first (those of the architect's
    /// earlier works too, for the architect's line), what it holds, and whether the CLI started it, so that it is
    /// resumed; a session the CLI never started is started under the id kept for it. A session `carried` is the one
    /// the project's previous architect work left, whose row is that work's: the round takes it up for its own work
    /// ([`carry_over`]).
    Going {
        id: i64,
        cli_session: String,
        rounds: Vec<Round>,
        held: Box<Held>,
        resume: bool,
        carried: bool,
    },
    /// In a new session, for the reason given; it holds nothing yet.
    New(Start),
}

/// The line a cell's session is on: its own, or for a cell on no line the cell itself, whose session is its own.
pub fn line_of(cell: &CompiledCell) -> String {
    cell.line.clone().unwrap_or_else(|| cell.id.clone())
}

/// The context of a session as its rounds recorded it: the prompt of the last call of its latest round that recorded
/// its calls, which is the input, the cache read and the cache written of that call.
pub fn context(rounds: &[Round]) -> Option<i64> {
    rounds.iter().rev().find_map(|r| {
        let last = r.usage.as_ref()?["usage"]["iterations"].as_array()?.last()?.clone();
        let n = |k: &str| last[k].as_i64().unwrap_or(0);
        Some(n("input_tokens") + n("cache_read_input_tokens") + n("cache_creation_input_tokens"))
    })
}

/// Whether a round at a cell only rewrites the cell's last return, which was sent back: its message says only why,
/// and that is right only in the session that wrote the return.
fn rewrites(rounds: &[Round], cell: &str) -> bool {
    rounds.last().is_some_and(|r| {
        r.cell.as_deref() == Some(cell)
            && r.transition.is_none()
            && r.rejected
                .as_ref()
                .is_some_and(|x| !["interrupted", "failed", "error"].iter().any(|k| x.get(k).is_some()))
    })
}

/// The model a session's rounds ran on: the one its latest round that ran the command line gave it.
pub fn model(rounds: &[Round]) -> Option<String> {
    rounds.iter().rev().find_map(|r| {
        let command = r.input["call"]["command"].as_array()?;
        let at = command.iter().position(|a| a == "--model")?;
        command.get(at + 1)?.as_str().map(str::to_string)
    })
}

/// The configuration directory of the CLI a session's rounds ran with, which holds the login their calls were made
/// under and the session itself: the one its latest round that ran the command line gave it; none when no round
/// recorded one.
pub fn home(rounds: &[Round]) -> Option<String> {
    rounds.iter().rev().find_map(|r| {
        r.input["call"]["command"].as_array()?;
        r.input["call"]["env"][crate::cli::CONFIG_DIR]
            .as_str()
            .map(str::to_string)
    })
}

/// Whether two configuration directories are one: the same path, or the same directory by two paths.
pub fn same_dir(a: &Path, b: &Path) -> bool {
    a == b
        || match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
}

/// What a line's rounds run on now, which a session that ran on another cannot go on with: the model, whose prompt
/// cache holds nothing of another model's session, and the configuration directory of the CLI, the login account the
/// calls are made under, which holds the session; none when the CLI runs with none of its own.
#[derive(Debug, Clone, Copy)]
pub struct On<'a> {
    pub model: &'a str,
    pub home: Option<&'a Path>,
    /// How long, in milliseconds, the CLI's prompt cache keeps a session's prompt after its last call
    /// ([`config::CliKind::cache_lifetime`]); none for a CLI with no prompt cache.
    pub cache: Option<i64>,
}

/// Where the work's next round at a cell runs, on what it runs on, with the configuration's limit of a session's
/// context in tokens and the time it is asked at (`now`, in milliseconds). It reads only: [`open`] starts a new session.
pub fn plan(conn: &Connection, work: i64, cell: &CompiledCell, limit: u64, on: On, now: i64) -> Result<Plan, String> {
    let Some(line) = &cell.line else {
        return Ok(Plan::New(Start::Line));
    };
    let going: Option<(i64, String)> = conn
        .query_row(
            "SELECT id, cli_session FROM sessions WHERE work = ?1 AND line = ?2 AND ended IS NULL",
            params![work, line],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let (id, cli_session, carried) = match going {
        Some((id, cli_session)) => (id, cli_session, false),
        None => match left(conn, work, line)? {
            Some((id, cli_session)) => (id, cli_session, true),
            None => return Ok(Plan::New(Start::Line)),
        },
    };
    let (rounds, held) = held::of_session(conn, &cli_session)?;
    // A fixed place of the flow opens a new session at the work's first round there; a session set up for the line
    // that holds no round yet, as the start of a work sets up the engineer's, is new already.
    if !rounds.is_empty() && opens(conn, work, cell)? {
        return Ok(Plan::New(Start::Cell));
    }
    // Another login account's configuration directory does not hold the session at all, so a new one starts even to
    // rewrite a return sent back.
    if let (Some(was), Some(now)) = (home(&rounds), on.home)
        && !same_dir(Path::new(&was), now)
    {
        return Ok(Plan::New(Start::Account));
    }
    // A return sent back is rewritten in the session that wrote it; the limit, a new model and an expired cache take
    // effect at the round after. A model's prompt cache holds nothing of a session another model ran, so a session on
    // another model would be written again whole: a new one is given only what its cell needs. So would a session
    // whose cache lifetime has run out since its last call, a round's or a keep-warm ping's, when the flow says the
    // cell's line starts a new session then.
    if !rewrites(&rounds, &cell.id) {
        if context(&rounds).is_some_and(|c| c > i64::try_from(limit).unwrap_or(i64::MAX)) {
            return Ok(Plan::New(Start::Limit));
        }
        if model(&rounds).is_some_and(|m| m != on.model) {
            return Ok(Plan::New(Start::Model));
        }
        if cell.cache_expiry
            && let Some(life) = on.cache
            && crate::warm::last_call(conn, &cli_session, &rounds)?.is_some_and(|last| now - last >= life)
        {
            return Ok(Plan::New(Start::Expired));
        }
    }
    let resume = rounds.iter().any(|r| r.usage.as_ref().is_some_and(|u| !u.is_null()));
    Ok(Plan::Going {
        id,
        cli_session,
        rounds,
        held: Box::new(held),
        resume,
        carried,
    })
}

/// Whether the work's next round at a cell opens a new session for its line because the flow fixes one there: the
/// cell is one its line starts a new session at and the work has had no round there yet, or the work last came to it
/// by a row that starts one each time and has had no round there since. A return sent back, an answer brought back
/// and tools run come back to the cell after a round of it, and go on in its session. The session the line has going
/// on before it ends there.
pub fn opens(conn: &Connection, work: i64, cell: &CompiledCell) -> Result<bool, String> {
    let rounds_after = |n: i64| -> Result<i64, String> {
        conn.query_row(
            "SELECT count(*) FROM rounds WHERE work = ?1 AND cell = ?2 AND n > ?3 AND session IS NOT NULL",
            params![work, cell.id, n],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())
    };
    if cell.new_session && rounds_after(0)? == 0 {
        return Ok(true);
    }
    if cell.fresh_session.is_empty() {
        return Ok(false);
    }
    // The latest round that took a row here which starts a new session each time.
    let came: Option<i64> = conn
        .query_row(
            "SELECT max(n) FROM rounds WHERE work = ?1 AND transition IS NOT NULL
               AND cell || '·' || transition IN (SELECT value FROM json_each(?2))",
            params![
                work,
                serde_json::to_string(&cell.fresh_session).map_err(|e| e.to_string())?
            ],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    match came {
        Some(n) => Ok(rounds_after(n)? == 0),
        None => Ok(false),
    }
}

/// The lines of the architect's works whose session goes on from one of a project's architect works to the next: the
/// architect's own, and the pick of its next matter.
const CARRIED: [&str; 2] = ["architect", "picker"];

/// The session a work of a project's architect takes up at its line's first round in the work: the one the project's
/// previous architect work left on the line, its row and the CLI's id for it, unless it ended; none for a line not
/// carried, once the work's line had a session, and at the project's first architect work.
fn left(conn: &Connection, work: i64, line: &str) -> Result<Option<(i64, String)>, String> {
    if !CARRIED.contains(&line) {
        return Ok(None);
    }
    let had: i64 = conn
        .query_row(
            "SELECT count(*) FROM sessions WHERE work = ?1 AND line = ?2",
            params![work, line],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    if had > 0 {
        return Ok(None);
    }
    let last: Option<(i64, String, Option<i64>)> = conn
        .query_row(
            "SELECT s.id, s.cli_session, s.ended FROM sessions s JOIN works x ON x.id = s.work
             WHERE x.graph = 'architect' AND s.line = ?2 AND s.work < ?1
               AND x.project = (SELECT project FROM works WHERE id = ?1 AND graph = 'architect')
             ORDER BY s.id DESC LIMIT 1",
            params![work, line],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    Ok(last.and_then(|(id, cli_session, ended)| ended.is_none().then_some((id, cli_session))))
}

/// Starts a new session of a work's line in the CLI named, under the CLI's id given, for the reason given: the session
/// the line had going on, if any, ends. Gives its row.
pub fn open(conn: &Connection, work: i64, line: &str, cli: &str, cli_session: &str, why: Start) -> Result<i64, String> {
    conn.execute(
        "UPDATE sessions SET ended = ?3 WHERE work = ?1 AND line = ?2 AND ended IS NULL",
        params![work, line, db::now()],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO sessions (work, line, cli, cli_session, started) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![work, line, cli, cli_session, why.name()],
    )
    .map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

/// Begins a work's line's session under the CLI's id given, as the session manager sets it up with the work, unless the
/// line has one going on already.
pub fn begin(conn: &Connection, work: i64, line: &str, cli: &str, cli_session: &str) -> Result<(), String> {
    conn.execute(
        "INSERT INTO sessions (work, line, cli, cli_session) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT (work, line) WHERE ended IS NULL DO NOTHING",
        params![work, line, cli, cli_session],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// A work of a project's architect takes up, at its line's first round in it, the session the project's previous
/// architect work left (`from`, that work's row, as [`plan`] found it): its line's session is a row of this work's with
/// the same CLI session, started as carried, which the round resumes. Gives the row.
pub fn carry_over(conn: &Connection, work: i64, from: i64) -> Result<i64, String> {
    let n = conn
        .execute(
            "INSERT INTO sessions (work, line, cli, cli_session, started)
             SELECT ?1, line, cli, cli_session, ?3 FROM sessions WHERE id = ?2",
            params![work, from, Start::Carried.name()],
        )
        .map_err(|e| e.to_string())?;
    if n != 1 {
        return Err(format!("work {work}: no session {from} to take up"));
    }
    Ok(conn.last_insert_rowid())
}

/// The CLI's id for a session, when the CLI reported another than the one the session was started under.
pub fn rename(conn: &Connection, session: i64, cli_session: &str) -> Result<(), String> {
    conn.execute(
        "UPDATE sessions SET cli_session = ?2 WHERE id = ?1",
        params![session, cli_session],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Ends a session, so that its line's next round starts a new one: a cell on no line's, once its round has.
pub fn end(conn: &Connection, session: i64) -> Result<(), String> {
    conn.execute(
        "UPDATE sessions SET ended = ?2 WHERE id = ?1 AND ended IS NULL",
        params![session, db::now()],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Ends the session a work's line has going on: a batch made ends its review's.
pub fn end_line(conn: &Connection, work: i64, line: &str) -> Result<(), String> {
    conn.execute(
        "UPDATE sessions SET ended = ?3 WHERE work = ?1 AND line = ?2 AND ended IS NULL",
        params![work, line, db::now()],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Ends every session a work has going on, once the work has ended.
pub fn end_work(conn: &Connection, work: i64) -> Result<(), String> {
    conn.execute(
        "UPDATE sessions SET ended = ?2 WHERE work = ?1 AND ended IS NULL",
        params![work, db::now()],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// The reasoning effort a round runs with: its cell's, as the flow declares it, whatever the model its level picks; none
/// for a cell of a flow version stored before cells declared one, which runs at the command line's own.
pub fn effort(cell: &CompiledCell) -> Option<String> {
    cell.effort.clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn round(n: i64, cell: &str, usage: Option<Value>, rejected: Option<Value>) -> Round {
        Round {
            id: crate::RoundId::new(1, n),
            cell: Some(cell.into()),
            session: Some(1),
            input: json!({}),
            output: Some(json!({})),
            transition: rejected.is_none().then(|| "done".to_string()),
            rejected,
            compactions: 0,
            started_at: n,
            ended_at: Some(n),
            usage,
        }
    }

    fn calls(prompts: &[(i64, i64, i64)]) -> Value {
        let it: Vec<Value> = prompts
            .iter()
            .map(|(i, r, w)| json!({ "input_tokens": i, "cache_read_input_tokens": r, "cache_creation_input_tokens": w }))
            .collect();
        json!({ "usage": { "iterations": it } })
    }

    // A session's context is the last call of its latest round that recorded its calls.
    #[test]
    fn a_sessions_context_is_its_latest_last_call() {
        let rs = [
            round(1, "R1", Some(calls(&[(3, 0, 9_000), (2, 9_000, 1_000)])), None),
            round(2, "D1", Some(json!({ "started": true })), None),
        ];
        assert_eq!(context(&rs), Some(10_002));
        assert_eq!(context(&rs[1..]), None);
    }

    // A round runs at its cell's effort whatever the model: implementing at xhigh, choosing the way on at medium.
    #[test]
    fn a_round_runs_at_its_cells_effort() {
        let relations = db::relations(db::SCHEMA).unwrap();
        let tables = db::index_of(db::SCHEMA).unwrap();
        let f = flow::compile(flow::FLOW, &flow::Context { relations, tables }).unwrap();
        let g = f.graph("engineer").unwrap();
        assert_eq!(effort(g.cell("I1").unwrap()).as_deref(), Some("xhigh"));
        assert_eq!(effort(g.cell("A4").unwrap()).as_deref(), Some("medium"));
        let mut older = g.cell("I1").unwrap().clone();
        older.effort = None;
        assert_eq!(effort(&older), None);
    }

    const HOUR: i64 = 60 * 60_000;
    const SONNET: &str = "claude-sonnet-5-5";

    /// A database holding one architect work with one CLI session, `cs1`, and no round yet.
    fn session_db() -> db::Db {
        let mut db = db::Db::in_memory().unwrap();
        db.write(db::Actor::Engine, |tx| {
            tx.execute("INSERT INTO flow_versions (digest, compiled) VALUES ('d', '{}')", [])?;
            tx.execute(
                "INSERT INTO works (flow_version, graph, at, project) VALUES (1, 'architect', 'N1', 'p')",
                [],
            )?;
            tx.execute(
                "INSERT INTO sessions (work, line, cli, cli_session) VALUES (1, 'architect', 'claude', 'cs1')",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        db
    }

    /// A round of that session that ended at `ended`, with the usage the CLI reported, or none when it never ran the
    /// CLI. A round that ran it also has the command line it ran, on the model `SONNET`.
    fn session_round(db: &mut db::Db, n: i64, ended: i64, usage: Option<&str>) {
        let input = if usage.is_some() {
            json!({ "call": { "command": ["claude", "-p", "--model", SONNET] } })
        } else {
            json!({})
        };
        db.write(db::Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO rounds (work, n, cell, session, input, config_commit, started_at, ended_at, usage)
                 VALUES (1, ?1, 'N1', 1, ?2, 'c', ?3, ?4, ?5)",
                db::rusqlite::params![n, input.to_string(), ended - 1_000, ended, usage],
            )?;
            Ok(())
        })
        .unwrap();
    }

    /// A keep-warm ping of that session that ended at `ended`, with the usage the CLI reported, or none when it failed.
    fn session_ping(db: &mut db::Db, ended: i64, usage: Option<&str>) {
        db.write(db::Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO pings (session, waited, usage, started_at, ended_at) VALUES (1, '{}', ?1, ?2, ?3)",
                db::rusqlite::params![usage, ended - 1_000, ended],
            )?;
            Ok(())
        })
        .unwrap();
    }

    const RAN: &str = r#"{"cost_usd": 0.01, "usage": {"iterations": [
        {"input_tokens": 10, "cache_read_input_tokens": 1000, "cache_creation_input_tokens": 0}]}}"#;

    fn architect_cell(expires: bool) -> CompiledCell {
        let relations = db::relations(db::SCHEMA).unwrap();
        let tables = db::index_of(db::SCHEMA).unwrap();
        let f = flow::compile(flow::FLOW, &flow::Context { relations, tables }).unwrap();
        let mut cell = f.graph("architect").unwrap().cell("N1").unwrap().clone();
        assert!(cell.cache_expiry, "the architect's line expires with the cache");
        cell.cache_expiry = expires;
        cell
    }

    fn at(db: &db::Db, cell: &CompiledCell, cache: Option<i64>, now: i64) -> Plan {
        let on = On {
            model: SONNET,
            home: None,
            cache,
        };
        plan(db.conn(), 1, cell, 500_000, on, now).unwrap()
    }

    fn goes_on(p: &Plan) -> bool {
        matches!(p, Plan::Going { .. })
    }

    fn expired(p: &Plan) -> bool {
        matches!(p, Plan::New(Start::Expired))
    }

    // A session of a line that expires with the cache, last called a lifetime or more ago, is replaced for the reason
    // that the cache expired; one last called less than a lifetime ago goes on, to the millisecond.
    #[test]
    fn a_session_the_cache_let_go_is_replaced_on_a_line_that_expires_with_it() {
        let mut db = session_db();
        session_round(&mut db, 1, 1_000_000, Some(RAN));
        let cell = architect_cell(true);
        assert!(goes_on(&at(&db, &cell, Some(HOUR), 1_000_000 + HOUR - 1)));
        assert!(expired(&at(&db, &cell, Some(HOUR), 1_000_000 + HOUR)));
        assert!(expired(&at(&db, &cell, Some(HOUR), 1_000_000 + 3 * HOUR)));
        // A CLI whose cache lives longer lets go later.
        assert!(goes_on(&at(&db, &cell, Some(2 * HOUR), 1_000_000 + HOUR)));
    }

    // A ping that read the cache is a call, so the session goes on while the cache holds from it; a ping that failed
    // and a round that never ran the CLI are no calls and keep nothing in the cache.
    #[test]
    fn only_a_call_that_read_the_cache_keeps_a_session_from_expiring() {
        let cell = architect_cell(true);
        let mut db = session_db();
        session_round(&mut db, 1, 1_000_000, Some(RAN));
        let now = 1_000_000 + 70 * 60_000;
        assert!(expired(&at(&db, &cell, Some(HOUR), now)));
        // A successful ping 10 minutes ago.
        session_ping(&mut db, now - 10 * 60_000, Some(RAN));
        assert!(goes_on(&at(&db, &cell, Some(HOUR), now)));

        let mut failed = session_db();
        session_round(&mut failed, 1, 1_000_000, Some(RAN));
        session_ping(&mut failed, now - 10 * 60_000, None);
        session_round(&mut failed, 2, now - 5 * 60_000, None);
        assert!(expired(&at(&failed, &cell, Some(HOUR), now)));
    }

    // The CLI records that it started at its init event, before any call to the model: a round that resumed a session
    // and failed there (a usage limit, the network) made no call, so it moves the cache's last read no later, while a
    // round cut off after the model's first reply did read the cache.
    #[test]
    fn a_round_that_failed_before_any_call_to_the_model_does_not_move_the_last_call() {
        let cell = architect_cell(true);
        let mut db = session_db();
        session_round(&mut db, 1, 1_000_000, Some(RAN));
        // 50 minutes after the real call, a round whose usage says only that the CLI started.
        session_round(&mut db, 2, 1_000_000 + 50 * 60_000, Some(r#"{"started": true}"#));
        assert!(goes_on(&at(&db, &cell, Some(HOUR), 1_000_000 + HOUR - 1)));
        assert!(expired(&at(&db, &cell, Some(HOUR), 1_000_000 + HOUR)));
        assert!(expired(&at(&db, &cell, Some(HOUR), 1_000_000 + 110 * 60_000 - 1)));
        // The keep-warm ping is due from the real call, not from the failed round.
        let due = |now: i64| {
            let on = On {
                model: SONNET,
                home: None,
                cache: Some(HOUR),
            };
            crate::warm::due(db.conn(), 1, &[&cell], 500_000, on, now)
                .unwrap()
                .is_some()
        };
        assert!(due(1_000_000 + 55 * 60_000));

        // A round cut off after the model's first reply holds that call and moves the last call on.
        let mut cut = session_db();
        session_round(&mut cut, 1, 1_000_000, Some(RAN));
        session_round(
            &mut cut,
            2,
            1_000_000 + 50 * 60_000,
            Some(r#"{"started": true, "first_call": {"cache_read_input_tokens": 1000}}"#),
        );
        assert!(goes_on(&at(&cut, &cell, Some(HOUR), 1_000_000 + HOUR)));
    }

    // A line that does not say its session expires goes on whatever its age, as does a session whose CLI has no
    // prompt cache; `claude` keeps the prompt an hour.
    #[test]
    fn a_session_goes_on_when_its_line_or_cli_has_no_cache_to_expire() {
        let mut db = session_db();
        session_round(&mut db, 1, 1_000_000, Some(RAN));
        let later = 1_000_000 + 3 * HOUR;
        assert!(goes_on(&at(&db, &architect_cell(false), Some(HOUR), later)));
        assert!(goes_on(&at(&db, &architect_cell(true), None, later)));
        assert_eq!(config::CliKind::Claude.cache_lifetime(), Some(HOUR));
    }

    // The keep-warm module pings a session that the cache has not let go, from five minutes before it does, and none
    // that the chooser would replace, none once the cache let it go, and none when the CLI has no cache.
    #[test]
    fn a_session_is_pinged_in_the_last_five_minutes_before_its_cache_lets_it_go() {
        let cell = architect_cell(true);
        let mut db = session_db();
        session_round(&mut db, 1, 1_000_000, Some(RAN));
        let due_at = |cache: Option<i64>, now: i64| {
            let on = On {
                model: SONNET,
                home: None,
                cache,
            };
            crate::warm::due(db.conn(), 1, &[&cell], 500_000, on, now)
                .unwrap()
                .is_some()
        };
        assert!(!due_at(Some(HOUR), 1_000_000 + 55 * 60_000 - 1));
        assert!(due_at(Some(HOUR), 1_000_000 + 55 * 60_000));
        assert!(due_at(Some(HOUR), 1_000_000 + HOUR - 1));
        assert!(!due_at(Some(HOUR), 1_000_000 + HOUR));
        assert!(!due_at(None, 1_000_000 + 55 * 60_000));
        assert_eq!(crate::warm::after_ms(HOUR), 55 * 60_000);
    }

    // Only a return sent back is rewritten in its own session: a run that failed or was cut off is done again.
    #[test]
    fn only_a_return_sent_back_stays_for_its_rewrite() {
        let sent = [round(1, "I1", None, Some(json!({ "check": "no head" })))];
        assert!(rewrites(&sent, "I1"));
        assert!(!rewrites(&sent, "T6"));
        let failed = [round(1, "I1", None, Some(json!({ "failed": { "kind": "transient" } })))];
        assert!(!rewrites(&failed, "I1"));
        assert!(!rewrites(&[round(1, "I1", None, None)], "I1"));
    }
}
