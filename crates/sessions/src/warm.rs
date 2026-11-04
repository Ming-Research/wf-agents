//! Keeping an idle session's prompt cache warm. The CLI keeps a session's prompt in the cache a while after its last
//! call (an hour for `claude`, [`config::CliKind::cache_lifetime`]); a round after that writes the whole prompt again,
//! at many times the price of reading it. While a work goes on or waits, a session of one of its lines that the flow
//! can still come back to is sent one minimal message five minutes before the cache lets it go ([`after_ms`]), so that
//! the cache is read instead of expiring, and again every 55 minutes, as many times in one wait as the reads cost no
//! more than writing the session again would beyond reading it, at the session's model's prices ([`most`]). This works
//! around the cache's lifetime; each ping records where the work stood and for what the line waited, so that a line
//! that waits long shows where it comes from. A session's last call is told in one place ([`last_call`]), for the
//! pings, for the chooser of a session whose line expires with the cache, and for the tell of a round that missed it.
//!
//! The base of a project's architect, which each matter's session and each second eye forks, is kept warm for the
//! forks. A fork reads the base from the cache only up to the end of a call the cache keeps whose prompt the fork's
//! begins with, under what the fork puts before the messages, and each fork writes only its own end: so a base whose end
//! no call cached under the forks' command line is written again whole by every fork. Before a fork is made, the base
//! is warmed by a ping under the fork's command line, unless its last call is such a ping less than [`after_ms`] ago
//! ([`before_fork`]): once after the base is built, whose last round's call ends before its return; once after an
//! install changed what comes before its messages; and once after its cache may have let it go. While its architect
//! has a work running, whose next matters fork it, the base is pinged as a waiting session is ([`base_due`]). Its pings
//! count among the most of its wait ([`most`]), and a fork holds those it was sent before the fork was made
//! ([`held_pings`]), for its last call and its cost.

use std::path::{Path, PathBuf};

use db::rusqlite::{Connection, params};
use db::{Actor, Db};
use flow::CompiledCell;
use serde_json::{Value, json};

use crate::choose::{self, Plan};
use crate::cli::{Agents, Next, Request};
use crate::cost;
use crate::round::Round;

/// How long before the cache lets a session's prompt go it is pinged.
const MARGIN_MS: i64 = 5 * 60_000;

/// How long after a session's last call it is pinged, when the cache keeps a prompt `life` milliseconds after a call
/// ([`config::CliKind::cache_lifetime`]): 55 minutes of an hour. A ping later than `life` would write the prompt whole
/// again, for nothing.
pub fn after_ms(life: i64) -> i64 {
    life - MARGIN_MS
}

/// Whether a round's usage says the model was called: the CLI reported something after it started the session (a first
/// call, or the result), not only that it started. The CLI records `{"started": true}` at its init event, before any
/// call, so a round that resumed a session and failed there (a usage limit, the network) holds only that and read
/// nothing from the cache.
fn called(usage: &Value) -> bool {
    usage.as_object().is_some_and(|u| u.keys().any(|k| k != "started"))
}

/// A keep-warm ping a CLI session holds, as `pings` keeps it.
#[derive(Debug, Clone)]
pub struct Pinged {
    pub id: i64,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    /// Tokens and cost as the CLI reported them; none for a ping that failed before the CLI called the model, or runs.
    pub usage: Option<Value>,
    pub compactions: i64,
    /// The call it ran, as a round's record keeps one (`rounds.input.call`); none for a ping that ran its session's last
    /// round's command line again, whose call is that round's.
    pub call: Option<Value>,
}

/// The keep-warm pings a CLI session holds, oldest first: those sent to it, under any of its rows, and for a fork those
/// its base was sent before the fork's first round started, which the fork holds as it holds the base's rounds from
/// before then ([`crate::round::of_cli_session`]).
pub fn held_pings(conn: &Connection, cli_session: &str) -> Result<Vec<Pinged>, String> {
    let base = choose::forked_from(conn, cli_session)?.filter(|b| b != cli_session);
    let until: Option<i64> = match &base {
        Some(_) => conn
            .query_row(
                "SELECT min(r.started_at) FROM rounds r JOIN sessions s ON s.id = r.session WHERE s.cli_session = ?1",
                [cli_session],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?,
        None => None,
    };
    let mut st = conn
        .prepare(
            "SELECT p.id, p.started_at, p.ended_at, p.usage, p.compactions, p.call FROM pings p
             JOIN sessions s ON s.id = p.session
             WHERE s.cli_session = ?1 OR (s.cli_session = ?2 AND p.started_at <= ?3)
             ORDER BY p.started_at, p.id",
        )
        .map_err(|e| e.to_string())?;
    st.query_map(params![cli_session, base, until.unwrap_or(i64::MAX)], |r| {
        Ok(Pinged {
            id: r.get(0)?,
            started_at: r.get(1)?,
            ended_at: r.get(2)?,
            usage: crate::round::json_col(r.get(3)?),
            compactions: r.get(4)?,
            call: crate::round::json_col(r.get(5)?),
        })
    })
    .map_err(|e| e.to_string())?
    .collect::<Result<_, _>>()
    .map_err(|e| e.to_string())
}

/// When a session's last call ended: its latest round that called the model, or its latest keep-warm ping that
/// reached the CLI, whichever came last; those are the calls that read the cache and so kept the prompt in it a
/// lifetime longer. A round that never ran the command line, one that failed after the CLI started and before any call
/// (its usage says only that it started), and a ping that failed before the CLI did leave the cache as it was and
/// count for nothing. `rounds` are the session's, oldest first, those of the architect's earlier works too, as
/// [`crate::held::of_session`] gives them; the pings it holds count ([`held_pings`]): those of every row that has the
/// CLI session, and for a fork those its base was sent before the fork was made.
pub fn last_call(conn: &Connection, cli_session: &str, rounds: &[Round]) -> Result<Option<i64>, String> {
    let ping = held_pings(conn, cli_session)?
        .into_iter()
        .filter(|p| p.usage.is_some())
        .filter_map(|p| p.ended_at)
        .max();
    Ok(last_round(rounds).max(ping))
}

/// When a session's latest round that called the model ended.
fn last_round(rounds: &[Round]) -> Option<i64> {
    rounds
        .iter()
        .filter(|r| r.usage.as_ref().is_some_and(called))
        .filter_map(|r| r.ended_at)
        .max()
}

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
    /// What the work waited on (pause, slot, owner, architect, items or row), `work` when it went on without the
    /// line, or `fork` when the ping warmed an architect's base before a fork of it.
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

/// One ping of a session, as `pings` keeps it: when it started and ended.
struct Sent {
    started_at: i64,
    ended_at: Option<i64>,
}

/// Whether the session of a line of the work is due a ping at `now`, given the cells of the line the flow can come back
/// to in that session, on what the line's rounds run on: none when the CLI has no prompt cache to keep, when they are
/// not all on one line, when the line's next round will start a new session anyway (a session past the limit, on
/// another model or under another login account, whose cache has let it go for a line that starts a new session then,
/// or a cell its line starts one at), while a round runs, before the ping is due since its last call ([`after_ms`]) or
/// once the cache has let it go, a minute after a ping that failed, and after the most pings in one wait that its
/// model's prices make worth it ([`most`]).
pub fn due(
    conn: &Connection,
    work: i64,
    cells: &[&CompiledCell],
    limit: u64,
    on: choose::On,
    now: i64,
) -> Result<Option<Due>, String> {
    let Some(life) = on.cache else {
        return Ok(None);
    };
    let Some(line) = cells.first().and_then(|c| c.line.clone()) else {
        return Ok(None);
    };
    if cells.iter().any(|c| c.line.as_ref() != Some(&line)) {
        return Ok(None);
    }
    let mut going: Option<(i64, String, Vec<Round>)> = None;
    for c in cells {
        match choose::plan(conn, work, c, limit, on, now, None)? {
            Plan::Going {
                id,
                cli_session,
                rounds,
                ..
            } => going = Some((id, cli_session, rounds)),
            Plan::New(_) | Plan::Fork { .. } => return Ok(None),
        }
    }
    let Some((session, cli_session, rounds)) = going else {
        return Ok(None);
    };
    let Some(idle) = idle(conn, session, cli_session, rounds)? else {
        return Ok(None);
    };
    Ok(idle.on_time(conn, life, now)?.then_some(idle.due))
}

/// A session no round runs in, as keeping it warm reads it: what a ping of it is sent with, and when the latest ping
/// of its wait was tried.
struct Idle {
    due: Due,
    tried_at: Option<i64>,
}

impl Idle {
    /// Whether a ping of the wait was tried less than [`RETRY_MS`] before `now`.
    fn tried_lately(&self, now: i64) -> bool {
        self.tried_at.is_some_and(|t| now - t < RETRY_MS)
    }

    /// Whether the session is due its ping at `now` by the time since its last call: from [`after_ms`] after it until
    /// the cache lets it go (`life`), and not within [`RETRY_MS`] of a ping tried.
    fn on_time(&self, conn: &Connection, life: i64, now: i64) -> Result<bool, String> {
        let Some(read_at) = last_call(conn, &self.due.cli_session, &self.due.rounds)? else {
            return Ok(false);
        };
        Ok((after_ms(life)..life).contains(&(now - read_at)) && !self.tried_lately(now))
    }
}

/// A session as keeping it warm reads it, its rounds given oldest first: none while a round of it runs, for one none of
/// whose rounds ran the command line and told what it used, and after the most pings since its last round ended that
/// its model's prices make worth it ([`most`]).
fn idle(conn: &Connection, session: i64, cli_session: String, rounds: Vec<Round>) -> Result<Option<Idle>, String> {
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
    let tried_at = sent.iter().map(|p| p.ended_at.unwrap_or(p.started_at)).max();
    Ok(Some(Idle {
        due: Due {
            session,
            cli_session,
            rounds,
            last,
            pings: sent.len(),
        },
        tried_at,
    }))
}

/// The base of a project's architect (`base`: its row and the CLI's id for it) as keeping it warm reads it, as [`idle`]
/// has it: none when its rounds ran on another model or under another login account than its forks run on (`on`),
/// whose forks read nothing of it.
fn base_idle(conn: &Connection, base: (i64, &str), on: choose::On) -> Result<Option<Idle>, String> {
    let (row, cli_session) = base;
    let rounds = crate::round::of_cli_session(conn, cli_session)?;
    if choose::model(&rounds).is_some_and(|m| m != on.model) {
        return Ok(None);
    }
    if let (Some(was), Some(home)) = (choose::home(&rounds), on.home)
        && !choose::same_dir(Path::new(&was), home)
    {
        return Ok(None);
    }
    idle(conn, row, cli_session.to_string(), rounds)
}

/// Whether the base of a project's architect (`base`: its row and the CLI's id for it) is due its keep-warm ping at
/// `now`, which the engine asks while the architect has a work running, whose next matters fork the base: as a waiting
/// session is, from [`after_ms`] after its last call until the cache lets it go, and at most [`most`] times since its
/// last round; none when the CLI has no prompt cache, and none on another model or account than its forks run on
/// (`on`). On the trial's board a ping of a session of 112,536 tokens on Opus 5.5 cost $0.033, where a second eye that
/// found the end of a base of 106,594 tokens not cached wrote 72,713 of them again, $0.57 beyond reading them: one fork
/// that reads the base pays for some seventeen pings, sixteen hours of keeping it warm.
pub fn base_due(conn: &Connection, base: (i64, &str), on: choose::On, now: i64) -> Result<Option<Due>, String> {
    let Some(life) = on.cache else {
        return Ok(None);
    };
    let Some(idle) = base_idle(conn, base, on)? else {
        return Ok(None);
    };
    Ok(idle.on_time(conn, life, now)?.then_some(idle.due))
}

/// Whether the base of a project's architect (`base`: its row and the CLI's id for it) is warmed at `now` before a fork
/// of it is made, the fork's call being `fork` as its round's record keeps it. A fork reads the base from the prompt
/// cache up to the end of the latest call the cache keeps whose prompt the fork's begins with: one under what the fork
/// puts before the messages (the model, the tools with the schema of the return among them, the system prompt, the
/// command line's version), and the call of the base's own last round ends before the return it handed in, which every
/// fork would write again. On the trial's board, after an install that changed that schema, the first fork of a base of
/// 106,594 tokens wrote it whole, and the second eye forked from it minutes later read only 33,881 tokens of it and
/// wrote 134,241, as did the next matter's session: forks write their own ends, never the base's. So the base is
/// warmed by one ping under the fork's command line, whose end is the base's, unless its last call is such a ping less
/// than [`after_ms`] ago: once after it is built, once after an install changed what comes before its messages, and
/// once its cache may have let it go. The ping reads the base when the cache still has it and writes it once when it
/// does not, and every fork after it reads it. None when the CLI has no prompt cache, for a base on another model or
/// account than its forks run on (`on`), within [`RETRY_MS`] of a ping tried, and past the most pings of its wait
/// ([`most`]).
pub fn before_fork(
    conn: &Connection,
    base: (i64, &str),
    fork: &Value,
    on: choose::On,
    now: i64,
) -> Result<Option<Due>, String> {
    let Some(life) = on.cache else {
        return Ok(None);
    };
    let Some(idle) = base_idle(conn, base, on)? else {
        return Ok(None);
    };
    if idle.tried_lately(now) {
        return Ok(None);
    }
    let round = last_round(&idle.due.rounds);
    let wanted = crate::miss::before_messages_of(fork);
    let warm = held_pings(conn, base.1)?
        .into_iter()
        .filter(|p| p.usage.is_some())
        .max_by_key(|p| p.ended_at)
        .is_some_and(|p| {
            p.ended_at
                .is_some_and(|at| round.is_none_or(|r| at >= r) && now - at < after_ms(life))
                && p.call
                    .as_ref()
                    .is_some_and(|c| crate::miss::before_messages_of(c) == wanted)
        });
    Ok((!warm).then_some(idle.due))
}

/// The pings of a CLI session that started after a time, oldest first.
fn pings_since(conn: &Connection, cli_session: &str, after: i64) -> Result<Vec<Sent>, String> {
    let mut st = conn
        .prepare(
            "SELECT p.started_at, p.ended_at FROM pings p
             JOIN sessions s ON s.id = p.session
             WHERE s.cli_session = ?1 AND p.started_at > ?2 ORDER BY p.started_at",
        )
        .map_err(|e| e.to_string())?;
    st.query_map(params![cli_session, after], |r| {
        Ok(Sent {
            started_at: r.get(0)?,
            ended_at: r.get(1)?,
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

/// Sends a session its ping at `now`, its last round's command line run again ([`replayed`]), and records it, with
/// where the work waited and what the ping cost; gives the ping's row.
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
        fork: false,
        system: None,
        new_session: due.cli_session.clone(),
        prompt: MESSAGE.to_string(),
        images: Vec::new(),
        schema: json!({}),
        envelope: json!({}),
        allowed: Vec::new(),
        refused: Vec::new(),
        readable: Vec::new(),
        replay: Some(line),
        regive: None,
        deliver: None,
        bare: false,
    };
    ping_with(db, agents, &req, due, waited, None, now)
}

/// The call a ping runs as a round's record keeps one ([`crate::cli::call`], each large text kept in `contents` and
/// referred to there), with the texts to keep there.
pub type Recorded<'a> = (&'a Value, &'a [(String, String)]);

/// Sends a session the ping `req` at `now`, which resumes it with [`MESSAGE`], and records it, with where the work
/// waited, the call it ran when it is not the session's last round's command line run again (`recorded`), and what the
/// ping cost; gives the ping's row.
pub fn ping_with(
    db: &mut Db,
    agents: &dyn Agents,
    req: &Request,
    due: &Due,
    waited: &Waited,
    recorded: Option<Recorded<'_>>,
    now: i64,
) -> Result<i64, String> {
    let session = due.session;
    let call = recorded.map(|(c, _)| c.to_string());
    let kept = recorded.map(|(_, k)| k).unwrap_or_default();
    let id = db
        .write(Actor::Engine, |tx| {
            db::keep(tx, kept)?;
            tx.execute(
                "INSERT INTO pings (session, waited, started_at, call) VALUES (?1, ?2, ?3, ?4)",
                params![session, waited.json().to_string(), now, call],
            )?;
            Ok(tx.last_insert_rowid())
        })
        .map_err(|e| e.to_string())?;
    let began = db::now();
    let reply = agents.run(req, &|_| Next::Go);
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
