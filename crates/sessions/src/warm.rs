//! Keeping an idle session's prompt cache warm. The CLI keeps a session's prompt in the cache an hour after its last
//! call; a round after that writes the whole prompt again, at many times the price of reading it. While a work goes on
//! or waits, a session of one of its lines that the flow can still come back to is sent one minimal message 55 minutes
//! after its last call, so that the cache is read instead of expiring, and again every 55 minutes, as many times in one
//! wait as the reads cost no more than writing the session again would beyond reading it, at the session's model's
//! prices ([`most`]). This works around the cache's lifetime; each ping records where the work stood and for what the
//! line waited, so that a line that waits long shows where it comes from.

use std::path::PathBuf;

use db::rusqlite::{Connection, params};
use db::{Actor, Db};
use flow::CompiledCell;
use serde_json::{Value, json};

use crate::choose::{self, Plan};
use crate::cli::{Agents, Request};
use crate::cost;
use crate::round::Round;

/// How long after a session's last call it is pinged.
pub const AFTER_MS: i64 = 55 * 60_000;

/// How long the cache keeps a session's prompt after its last call: a ping later than that would write it whole
/// again, for nothing.
pub const CACHE_MS: i64 = 60 * 60_000;

/// The most pings in one wait of a session on a model whose context is `prompt` tokens: as many as cost, each reading
/// the session from the cache, no more than writing the session again for an hour would beyond reading it, which a ping
/// saves when the session's next round comes before the cache lets it go. At the prices of the command line's catalog
/// that is 39 on Opus 5.5 and Sonnet 5.5, whose write for an hour costs forty reads, 19 on Haiku 5.5 (twenty) and 79
/// on Fable 5.1 (eighty). None for a model with no prices, whose worth cannot be told.
pub fn most(model: &str, prompt: i64) -> usize {
    let Some(p) = cost::prices(model, prompt).filter(|p| p.read > 0.0) else {
        return 0;
    };
    // The small margin keeps a ratio such as (4.0 - 0.1) / 0.1, which floating point makes 38.99..., at 39.
    let pings = (p.write_1h - p.read) / p.read + 1e-9;
    if pings <= 0.0 { 0 } else { pings.floor() as usize }
}

/// How long after a ping that failed the next is tried, while the cache still keeps the prompt.
pub const RETRY_MS: i64 = 60_000;

/// What a ping says to the session.
pub const MESSAGE: &str = "# Keep-warm ping\n\nYour line waits for its next round, and this message from the session \
    manager only keeps your session's prompt cache from expiring. Do nothing and use no tool: hand in the smallest \
    return the format allows, with an empty progress.\n";

/// Where a work stood while a line of it waited, and for what, as a ping records it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Waited {
    pub cell: String,
    /// What the work waited on (pause, owner, architect, items or tool), or `work` when it went on without the line.
    pub on: String,
    pub what: String,
}

impl Waited {
    pub fn json(&self) -> Value {
        json!({ "cell": self.cell, "on": self.on, "what": self.what })
    }
}

/// A session due a ping.
#[derive(Debug, Clone)]
pub struct Due {
    pub session: i64,
    pub cli_session: String,
    /// Its rounds, oldest first.
    pub rounds: Vec<Round>,
    /// The round whose command line the ping runs again: the session's latest that ran its CLI.
    pub last: Round,
    /// The pings already sent in this wait.
    pub pings: usize,
}

/// One ping of a session, as `pings` keeps it: when it started and ended, and whether the CLI read the session.
struct Sent {
    started_at: i64,
    ended_at: Option<i64>,
    read: bool,
}

/// Whether the session of a line of the work is due a ping at `now`, given the cells of the line the flow can come back
/// to in that session, on what the line's rounds run on: none when they are not all on one line, when the line's next
/// round will start a new session anyway (a session past the limit, on another model or under another login account,
/// or a cell its line starts one at), while a round runs, before 55 minutes have passed since its last call or once the
/// cache has let it go, a minute after a ping that failed, and after the most pings in one wait that its model's prices
/// make worth it ([`most`]).
pub fn due(
    conn: &Connection,
    work: i64,
    cells: &[&CompiledCell],
    limit: u64,
    on: choose::On,
    now: i64,
) -> Result<Option<Due>, String> {
    let Some(line) = cells.first().and_then(|c| c.line.clone()) else {
        return Ok(None);
    };
    if cells.iter().any(|c| c.line.as_ref() != Some(&line)) {
        return Ok(None);
    }
    let mut going: Option<(i64, String, Vec<Round>)> = None;
    for c in cells {
        match choose::plan(conn, work, c, limit, on)? {
            Plan::Going {
                id,
                cli_session,
                rounds,
                ..
            } => going = Some((id, cli_session, rounds)),
            Plan::New(_) => return Ok(None),
        }
    }
    let Some((session, cli_session, rounds)) = going else {
        return Ok(None);
    };
    if rounds.iter().any(|r| r.ended_at.is_none()) {
        return Ok(None);
    }
    let Some(last) = rounds
        .iter()
        .rev()
        .find(|r| r.input["call"]["command"].is_array() && r.usage.as_ref().is_some_and(|u| u.get("usage").is_some()))
        .cloned()
    else {
        return Ok(None);
    };
    // The wait began when the session's last round ended.
    let Some(waits_from) = rounds.last().and_then(|r| r.ended_at) else {
        return Ok(None);
    };
    let sent = pings_since(conn, &cli_session, waits_from)?;
    let model = choose::model(&rounds).unwrap_or_default();
    if sent.len() >= most(&model, choose::context(&rounds).unwrap_or(0)) {
        return Ok(None);
    }
    let read_at = sent
        .iter()
        .filter(|p| p.read)
        .filter_map(|p| p.ended_at)
        .chain([waits_from])
        .max()
        .unwrap_or(waits_from);
    let tried_at = sent.iter().map(|p| p.ended_at.unwrap_or(p.started_at)).max();
    let since = now - read_at;
    if !(AFTER_MS..CACHE_MS).contains(&since) || tried_at.is_some_and(|t| now - t < RETRY_MS) {
        return Ok(None);
    }
    Ok(Some(Due {
        session,
        cli_session,
        rounds,
        last,
        pings: sent.len(),
    }))
}

/// The pings of a CLI session that started after a time, oldest first.
fn pings_since(conn: &Connection, cli_session: &str, after: i64) -> Result<Vec<Sent>, String> {
    let mut st = conn
        .prepare(
            "SELECT p.started_at, p.ended_at, json_extract(p.usage, '$.usage') IS NOT NULL FROM pings p
             JOIN sessions s ON s.id = p.session
             WHERE s.cli_session = ?1 AND p.started_at > ?2 ORDER BY p.started_at",
        )
        .map_err(|e| e.to_string())?;
    st.query_map(params![cli_session, after], |r| {
        Ok(Sent {
            started_at: r.get(0)?,
            ended_at: r.get(1)?,
            read: r.get(2)?,
        })
    })
    .map_err(|e| e.to_string())?
    .collect::<Result<_, _>>()
    .map_err(|e| e.to_string())
}

/// The command line a ping runs: the last round's, as it ran, resuming the session instead of starting it.
pub fn replayed(command: &[String], cli_session: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut skip = false;
    for a in command {
        if skip {
            skip = false;
            continue;
        }
        if a == "--resume" || a == "--session-id" {
            skip = true;
            continue;
        }
        out.push(a.clone());
    }
    out.extend(["--resume".to_string(), cli_session.to_string()]);
    out
}

/// Sends a session its ping at `now` and records it, with where the work waited and what the ping cost; gives the
/// ping's row.
pub fn ping(
    db: &mut Db,
    agents: &dyn Agents,
    config: &config::Config,
    due: &Due,
    waited: &Waited,
    now: i64,
) -> Result<i64, String> {
    let call = &due.last.input["call"];
    let command: Vec<String> = db::put_back(db.conn(), &call["command"])
        .as_array()
        .map(|xs| xs.iter().filter_map(Value::as_str).map(str::to_string).collect())
        .unwrap_or_default();
    let line = replayed(&command, &due.cli_session);
    let cli_name: String = db
        .conn()
        .query_row("SELECT cli FROM sessions WHERE id = ?1", [due.session], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    let cli = config
        .install
        .agents
        .clis
        .get(&cli_name)
        .cloned()
        .unwrap_or_else(|| config::Cli {
            kind: config::CliKind::Claude,
            command: line.first().cloned().unwrap_or_default(),
            args: Vec::new(),
            config_dir: None,
        });
    let model = line
        .iter()
        .position(|a| a == "--model")
        .and_then(|i| line.get(i + 1))
        .cloned()
        .unwrap_or_default();
    let req = Request {
        work: due.last.id.work,
        cell: due.last.cell.clone().unwrap_or_default(),
        line: String::new(),
        cli,
        model,
        effort: None,
        worktree: PathBuf::from(call["cwd"].as_str().unwrap_or(".")),
        config_dir: call["env"][crate::cli::CONFIG_DIR].as_str().map(PathBuf::from),
        resume: Some(due.cli_session.clone()),
        system: None,
        new_session: due.cli_session.clone(),
        prompt: MESSAGE.to_string(),
        images: Vec::new(),
        schema: json!({}),
        envelope: json!({}),
        refused: Vec::new(),
        readable: Vec::new(),
        replay: Some(line),
        regive: None,
        deliver: None,
        bare: false,
    };
    let session = due.session;
    let id = db
        .write(Actor::Engine, |tx| {
            tx.execute(
                "INSERT INTO pings (session, waited, started_at) VALUES (?1, ?2, ?3)",
                params![session, waited.json().to_string(), now],
            )?;
            Ok(tx.last_insert_rowid())
        })
        .map_err(|e| e.to_string())?;
    let began = db::now();
    let reply = agents.run(&req, &|_| {});
    let ended = now + (db::now() - began);
    let before = cost::spent(db.conn(), &due.cli_session, &due.rounds)?;
    let usage = reply
        .usage
        .get("usage")
        .is_some()
        .then(|| cost::own_cost(reply.usage.clone(), before));
    db.write(Actor::Engine, |tx| {
        tx.execute(
            "UPDATE pings SET model = ?2, usage = ?3, compactions = ?4, error = ?5, ended_at = ?6 WHERE id = ?1",
            params![
                id,
                reply.model,
                usage.as_ref().map(Value::to_string),
                reply.compactions,
                reply.error,
                ended
            ],
        )?;
        Ok(())
    })
    .map_err(|e| e.to_string())?;
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    // A wait has as many pings as cost no more than writing the session again for an hour beyond reading it, at its
    // model's prices: forty reads make a write on Opus 5.5 and Sonnet 5.5, twenty on Haiku 5.5, whatever its context,
    // eighty on Fable 5.1; a model with no prices has none.
    #[test]
    fn a_wait_has_as_many_pings_as_its_models_prices_make_worth_it() {
        assert_eq!(most("claude-sonnet-5-5", 200_000), 39);
        assert_eq!(most("claude-opus-5-5", 200_000), 39);
        assert_eq!(most("claude-haiku-5-5", 50_000), 19);
        assert_eq!(most("claude-haiku-5-5", 150_000), 19);
        assert_eq!(most("claude-fable-5-1", 200_000), 79);
        assert_eq!(most("claude-sonnet-5-5[1m]", 200_000), 39);
        assert_eq!(most("some-other-model", 200_000), 0);
        for (model, n) in [("claude-haiku-5-5", 19), ("claude-fable-5-1", 79)] {
            let p = cost::prices(model, 1_000).unwrap();
            assert!(n as f64 * p.read <= p.write_1h - p.read + 1e-12, "{model}");
            assert!((n + 1) as f64 * p.read > p.write_1h - p.read, "{model}");
        }
    }

    // A ping runs the last round's command line as it ran, resuming the session whether that round started it or
    // resumed it.
    #[test]
    fn a_ping_resumes_the_session_with_the_last_rounds_command_line() {
        let s = |xs: &[&str]| xs.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        let started = s(&[
            "claude",
            "-p",
            "--model",
            "m",
            "--json-schema",
            "{}",
            "--session-id",
            "s1",
        ]);
        assert_eq!(
            replayed(&started, "s1"),
            s(&["claude", "-p", "--model", "m", "--json-schema", "{}", "--resume", "s1"])
        );
        let resumed = s(&["claude", "-p", "--effort", "high", "--resume", "s1"]);
        assert_eq!(replayed(&resumed, "s1"), resumed);
    }
}
