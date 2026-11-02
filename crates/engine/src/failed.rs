//! Agent rounds that do not end in a return the cell takes, and the engine's own steps before them that fail in
//! passing, handled as the flow's rounds section says, which gives every agent's cell a shared row for each. A return
//! sent back runs the cell again, and so many in a row pause the work. A run that ends without a return is told apart
//! by what the command line reports, and is waited out for every work, run again, or pauses the work; it never counts
//! as a return sent back, and never asks for a written answer. A row that runs again is bounded: the work took it so
//! many times in a row, and the next time pauses it. "In a row" has one reading ([`since_outside`]).

use std::time::Duration;

use db::Actor;
use db::rusqlite::{OptionalExtension, params};
use flow::format::{Atom, Failure, Handle, Rounds, STEP_NETWORK};
use flow::{CompiledCell, CompiledGraph};
use serde_json::json;

use crate::agent::{self, Reply};
use crate::work::{self, Ended, RoundId, Work};
use crate::{Advance, Engine, actions};

/// Why the engine starts no new round for any work, and until when.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Halt {
    /// The kind of failed run, as the flow names it.
    pub kind: String,
    /// Until when, in milliseconds since the epoch.
    pub until: i64,
    /// What the command line reported.
    pub why: String,
}

/// How long the engine waits, each time a cause it does not know the end of is found to last (the network is down,
/// the login has expired), before it looks again: at the network itself, or by letting one round find out.
pub(crate) const PROBE: i64 = 30_000;

/// Every how many looks in a row at the network, each finding it down, a round tries anyway: five minutes of looks.
pub(crate) const LOOKS: u32 = 10;

/// How long a look at the network waits for the service's host to take a connection.
const LOOK_WAIT: Duration = Duration::from_secs(5);

/// Whether the agents' service can be reached from here: its host, the one `ANTHROPIC_BASE_URL` names or Anthropic's
/// API, resolves and takes a connection within a few seconds. Looking costs no round and writes nothing in a session.
pub fn network_reachable() -> bool {
    use std::net::{TcpStream, ToSocketAddrs};
    let host = std::env::var("ANTHROPIC_BASE_URL")
        .ok()
        .and_then(|u| {
            let rest = u.split_once("://").map_or(u.as_str(), |(_, r)| r).to_string();
            rest.split('/').next().map(str::to_string)
        })
        .filter(|h| !h.is_empty())
        .unwrap_or_else(|| "api.anthropic.com".to_string());
    let at = if host.contains(':') {
        host
    } else {
        format!("{host}:443")
    };
    at.to_socket_addrs()
        .ok()
        .and_then(|mut a| a.next())
        .is_some_and(|a| TcpStream::connect_timeout(&a, LOOK_WAIT).is_ok())
}

/// The longest a run that failed in passing waits before it runs again.
const RETRY_MAX: Duration = Duration::from_secs(300);

/// How long the engine waits for a usage limit whose reset the command line did not say in words it reads.
const UNKNOWN_RESET: i64 = 1_800_000;

fn sql(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// A time in milliseconds since the epoch, as the owner reads it: the day and minute in UTC.
pub fn utc(ms: i64) -> String {
    let secs = ms.div_euclid(1000);
    let days = secs.div_euclid(86_400);
    let rest = secs.rem_euclid(86_400);
    // The civil date of a day count from 1970-01-01, as Howard Hinnant's days_from_civil inverts it.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02} UTC", rest / 3600, rest % 3600 / 60)
}

/// The rounds section of the flow a work runs on. A flow version stored before the flow said it runs by the section
/// of the flow built into the engine.
pub fn section(e: &mut Engine, g: &CompiledGraph) -> Result<Rounds, String> {
    if let Some(r) = &g.rounds {
        return Ok(r.clone());
    }
    let now = e.flow_now;
    e.flow(now)?
        .graph(crate::ENGINEER)
        .and_then(|x| x.rounds.clone())
        .ok_or_else(|| "the flow has no rounds section".to_string())
}

fn kind_of<'s>(s: &'s Rounds, name: &str) -> Option<&'s Failure> {
    s.failed.kinds.iter().find(|k| k.name == name)
}

/// A round of a work, as far as counting the rows the work took needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Took {
    pub round: RoundId,
    /// The cell the round ran at.
    pub cell: String,
    /// The row the round took, as the view `round_rows` says; none for a round still open and for one that ended with
    /// the work paused.
    pub row: Option<String>,
    pub started_at: i64,
    pub ended_at: Option<i64>,
}

/// Whether a round took a row from outside the engine: a row taken because an agent returned, a tool ran or the owner
/// answered, at a cell of one of those. The engine's own rows (the shared ones of a round that ended in no return, a
/// line's new session, a table with no row), every row of a cell of the engine's, and a row that waits (the engine
/// chose it without running the cell, and the work did not move) are not. A round of a cell or a row the work's graph
/// no longer has, as when a flow edit renamed or removed it, is not known to be the engine's, so it ends the count.
fn from_outside(g: &CompiledGraph, t: &Took) -> bool {
    let Some(row) = t.row.as_deref() else {
        return false;
    };
    let Some(cell) = g.cell(&t.cell) else {
        return true;
    };
    let Some(r) = cell.rows.iter().find(|r| r.name == row) else {
        return true;
    };
    cell.kind != "engine" && !r.waits && !r.conditions.iter().any(Atom::about_no_return)
}

/// The rounds of a work since it last took a row from outside the engine ([`from_outside`]), newest first; a round
/// that started before the owner last let the work go on ends them too. This is what "in a row" means for every
/// bounded row of the rounds section: how many times the work took the row in these rounds ([`count`]), whatever else
/// the engine did between, which neither counts nor starts them afresh.
pub fn since_outside(e: &Engine, w: &Work, g: &CompiledGraph) -> Result<Vec<Took>, String> {
    let resumed = work::resumed_at(e.db.conn(), w.id)?;
    let mut st =
        e.db.conn()
            .prepare(
                "SELECT x.round, x.cell, x.took, r.started_at, r.ended_at
                 FROM round_rows x JOIN rounds r ON r.work = x.work AND r.n = x.round
                 WHERE x.work = ?1 ORDER BY x.round DESC",
            )
            .map_err(sql)?;
    let rounds = st
        .query_map([w.id], |r| {
            Ok(Took {
                round: RoundId::new(w.id, r.get(0)?),
                cell: r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                row: r.get(2)?,
                started_at: r.get(3)?,
                ended_at: r.get(4)?,
            })
        })
        .map_err(sql)?;
    let mut out = Vec::new();
    for t in rounds {
        let t = t.map_err(sql)?;
        if resumed.is_some_and(|at| t.started_at <= at) || from_outside(g, &t) {
            break;
        }
        out.push(t);
    }
    Ok(out)
}

/// Whether a round is the record of a step of the engine's before an agent's round that failed in passing
/// ([`networked`]): the agent ran no round, and is told of none.
pub fn was_step(r: &crate::work::Round) -> bool {
    r.rejected.as_ref().is_some_and(|x| x["failed"]["kind"] == STEP_NETWORK)
}

/// How many times the work took a row in the rounds [`since_outside`] gave.
pub fn count(took: &[Took], row: &str) -> usize {
    took.iter().filter(|t| t.row.as_deref() == Some(row)).count()
}

/// The text a round was sent back with, for the owner.
pub fn sent_back_reason(e: &Engine, round: RoundId) -> String {
    e.db.conn()
        .query_row(
            "SELECT rejected FROM rounds WHERE work = ?1 AND n = ?2",
            params![round.work, round.n],
            |r| r.get::<_, Option<String>>(0),
        )
        .ok()
        .flatten()
        .unwrap_or_default()
}

/// Whether the error text of a step of the engine's is a failure of the network that passes by itself, as git and ssh
/// word it: a connection refused, reset or closed, a host that does not resolve, a timeout, a server that fails. Text
/// it does not know is not the network's, so that a step that fails for any other cause (the configuration, a
/// permission) is not run again.
pub fn network_failure(why: &str) -> bool {
    const SIGNS: &[&str] = &[
        "open failed",
        "administratively prohibited",
        "stdio forwarding failed",
        "could not resolve host",
        "name or service not known",
        "temporary failure in name resolution",
        "nodename nor servname provided",
        "connection reset",
        "connection refused",
        "connection closed",
        "connection abort",
        "failed to connect",
        "couldn't connect to server",
        "timed out",
        "network is unreachable",
        "network is down",
        "no route to host",
        "broken pipe",
        "remote end hung up",
        "unexpected disconnect",
        "early eof",
        "rpc failed",
        "returned error: 5",
        "exchange_identification",
    ];
    let why = why.to_lowercase();
    SIGNS.iter().any(|s| why.contains(s))
}

/// While the work's last round, which took a row that runs again, waits to run again, why; none when the cell's round
/// may run now. A failed run waits twice as long each time as the time before, from `retry_wait` ([`Engine`]'s), up to
/// [`RETRY_MAX`]; a step of the engine's waits as many seconds as the flow's backoff says for the time it failed, a
/// second lasting `backoff_unit`.
pub fn waiting(retry_wait: Duration, backoff_unit: Duration, took: &[Took], s: &Rounds) -> Option<String> {
    let last = took.first()?;
    let row = last.row.as_deref()?;
    let n = (count(took, row) as u32).saturating_sub(1);
    let ended = last.ended_at.unwrap_or(last.started_at);
    if kind_of(s, row).is_some_and(|k| k.handle == Handle::Retry) {
        let wait = retry_wait.saturating_mul(1 << n.min(10)).min(RETRY_MAX).as_millis() as i64;
        let due = ended + wait;
        return (db::now() < due).then(|| {
            format!(
                "{} ended without a return; the cell runs again at {}",
                last.round,
                utc(due)
            )
        });
    }
    let case = s.engine.iter().find(|c| c.name == row && c.in_row.is_some())?;
    let secs = case.backoff.get(n as usize).or(case.backoff.last()).copied()?;
    let due = ended + backoff_unit.saturating_mul(secs).as_millis() as i64;
    (db::now() < due).then(|| {
        format!(
            "{}: a step before the cell's round failed ({row}); it is taken again at {}",
            last.round,
            utc(due)
        )
    })
}

/// Runs a step of the engine's before an agent's round that reaches the network, as every such step is run: when it
/// fails the way the network does by itself ([`network_failure`]) and the flow's rounds section has the case
/// `step_network`, the failure is recorded on the cell's round, the step waits as long as the backoff says
/// ([`waiting`], at the cell's next step) and is run again, and the failure past the case's bound pauses the work and
/// tells the owner, the architect's own work as much as any. A failure while the engine's own look finds the network
/// down stops every work as a failed run of the kind `offline` does; any other failure pauses the work, as a step it
/// cannot take does. `took` is [`since_outside`]. Gives none when the step passed, and else what the step came to.
pub fn networked(
    e: &mut Engine,
    w: &Work,
    cell: &CompiledCell,
    section: &Rounds,
    took: &[Took],
    step: impl FnOnce(&Engine) -> Result<(), String>,
) -> Result<Option<Advance>, String> {
    let Err(why) = step(&*e) else {
        return Ok(None);
    };
    let why = format!("cell {}: {why}", cell.id);
    if e.network_down() {
        return e.wait_offline(w.id, &why).map(Some);
    }
    let case = section.engine.iter().find(|c| c.name == STEP_NETWORK);
    let counter = case.and_then(|c| c.counts.clone());
    let bound = case.and_then(|c| c.in_row).filter(|_| network_failure(&why));
    let Some(bound) = bound else {
        return pause(e, w, &why).map(Some);
    };
    let before = count(took, STEP_NETWORK);
    let over = before >= bound as usize;
    let mut failed = json!({ "kind": STEP_NETWORK, "what": why });
    if over {
        failed["over"] = json!(bound);
    }
    let input = json!({ "engine": { STEP_NETWORK: why } });
    let commit = e.config.commit.clone();
    let ended = Ended {
        rejected: Some(json!({ "failed": failed })),
        ..Ended::default()
    };
    let round =
        e.db.write(Actor::Engine, |tx| {
            let id = work::start_round(tx, w.id, &cell.id, None, &input, &commit, None).map_err(db::Error::Abort)?;
            work::finish_round(tx, id, &ended).map_err(db::Error::Abort)?;
            Ok(id)
        })
        .map_err(sql)?;
    e.say(&format!("{round} could not take its step ({STEP_NETWORK}): {why}"));
    if !over && let Some(counter) = counter {
        crate::limits::tell_past(e, w, &counter)?;
    }
    if over {
        let why = format!(
            "{why}; the step failed {} times in a row, and running it again has not helped",
            before + 1
        );
        return pause(e, w, &why).map(Some);
    }
    Ok(Some(Advance::Moved))
}

/// Pauses a work and tells the owner why; once the owner lets it go on, its cell runs again.
pub fn pause(e: &mut Engine, w: &Work, why: &str) -> Result<Advance, String> {
    let text = match e.subject(w)? {
        Some(item) => format!("Work {} on item {} ({}) is paused: {why}", w.id, item.id, item.title),
        None => format!("Work {} ({}) is paused: {why}", w.id, w.of_architect()),
    };
    e.db.write(Actor::Engine, |tx| {
        tx.execute(
            "UPDATE works SET paused = ?2, paused_by = 'engine' WHERE id = ?1",
            params![w.id, why],
        )?;
        actions::notify(tx, Some(w.id), &text).map_err(db::Error::Abort)
    })
    .map_err(sql)?;
    e.say(&format!("work {} paused: {why}", w.id));
    Ok(Advance::Stopped)
}

/// The cell's round before `round` in its work, when its run ended as the same kind of failed run too: none when the
/// round before at the cell did anything else. A record of a step before the cell's round ([`was_step`]) is no run of
/// the cell, and is passed over.
fn refused_before(e: &Engine, round: RoundId, cell: &str, kind: &str) -> Result<Option<RoundId>, String> {
    let before: Option<(i64, Option<String>)> =
        e.db.conn()
            .query_row(
                "SELECT n, rejected FROM rounds WHERE work = ?1 AND n < ?2 AND cell = ?3
                   AND coalesce(json_extract(rejected, '$.failed.kind'), '') <> ?4 ORDER BY n DESC LIMIT 1",
                params![round.work, round.n, cell, STEP_NETWORK],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(sql)?;
    Ok(before.and_then(|(n, rejected)| {
        let v: serde_json::Value = serde_json::from_str(rejected.as_deref()?).ok()?;
        (v["failed"]["kind"] == kind).then(|| RoundId::new(round.work, n))
    }))
}

/// What a round whose run ended without a return was, and what came of it.
pub struct Failed<'a> {
    pub w: &'a Work,
    pub g: &'a CompiledGraph,
    pub cell: &'a CompiledCell,
    pub round: RoundId,
    pub section: &'a Rounds,
}

impl Failed<'_> {
    /// Ends the round, recording which kind of failed run it was, and does what the flow says for that kind.
    pub fn handle(&self, e: &mut Engine, ended: Ended, reply: &Reply) -> Result<Advance, String> {
        let what = reply
            .error
            .clone()
            .unwrap_or_else(|| "the round handed in nothing".to_string());
        let now = db::now();
        let name = agent::failure_kind(&what);
        let kind = kind_of(self.section, name)
            .or_else(|| kind_of(self.section, "transient"))
            .cloned()
            .ok_or("the flow's rounds section names no kind of failed run")?;
        let until = match (kind.handle, kind.name.as_str()) {
            (Handle::Wait, "usage_limit") => {
                Some(agent::resets_at(&what, now, &agent::zone_offset).unwrap_or(now + UNKNOWN_RESET))
            }
            (Handle::Wait, _) => Some(now + PROBE),
            _ => None,
        };
        let mut failed = json!({ "kind": kind.name, "what": what });
        if let Some(t) = until {
            failed["until"] = json!(t);
        }
        // A kind run again that has failed as often in a row as its bound says is not run again: the round records the
        // bound it passed, and the work pauses.
        let past = match (kind.handle, kind.in_row) {
            (Handle::Retry, Some(bound)) => {
                let took = since_outside(e, self.w, self.g)?;
                (count(&took, &kind.name) >= bound as usize).then_some(bound)
            }
            _ => None,
        };
        if let Some(bound) = past {
            failed["over"] = json!(bound);
        }
        e.say(&format!(
            "{} ended without a return ({}): {what}",
            self.round, kind.name
        ));
        let ended = Ended {
            rejected: Some(json!({ "failed": failed })),
            transition: None,
            ..ended
        };
        let round = self.round;
        e.db.write(Actor::Engine, |tx| {
            work::finish_round(tx, round, &ended).map_err(db::Error::Abort)
        })
        .map_err(sql)?;
        match kind.handle {
            Handle::Wait => {
                let until = until.unwrap_or(now);
                let first = e.halt_every_work(Halt {
                    kind: kind.name.clone(),
                    until,
                    why: what.clone(),
                });
                if first {
                    let text = if kind.name == "usage_limit" {
                        format!(
                            "No agent round starts for any work until {}: the command line reports {} ({what}). Each work goes on from its cell then.",
                            utc(until),
                            kind.name
                        )
                    } else if kind.name == "offline" {
                        format!(
                            "No agent round starts for any work while the command line reports {} ({what}): {} Each work goes on from its cell once the network is back.",
                            kind.name, kind.desc
                        )
                    } else {
                        format!(
                            "No agent round starts for any work while the command line reports {} ({what}): {} One round tries every half minute, and each work goes on from its cell once one runs.",
                            kind.name, kind.desc
                        )
                    };
                    let w = self.w.id;
                    e.db.write(Actor::Engine, |tx| {
                        actions::notify(tx, Some(w), &text).map_err(db::Error::Abort)
                    })
                    .map_err(sql)?;
                }
                Ok(Advance::Waits(format!("{} until {}", kind.name, utc(until))))
            }
            // The cell runs again once its wait is over, until it passes or its bound is passed.
            Handle::Retry => match past {
                None => {
                    if let Some(counter) = &kind.counts {
                        crate::limits::tell_past(e, self.w, counter)?;
                    }
                    Ok(Advance::Moved)
                }
                Some(bound) => {
                    let why = format!(
                        "cell {}: the agent's run ended without a return ({}) once more after {bound} times in a row, and running it again has not helped: {what}",
                        self.cell.id, kind.name
                    );
                    pause(e, self.w, &why)
                }
            },
            // The same session would give the service the same context again: the first time, the line's session ends
            // and the cell runs again in a new one; when the cell's round before was refused too, the work pauses.
            Handle::Fresh => {
                let before = refused_before(e, round, &self.cell.id, &kind.name)?;
                if let Some(first) = before {
                    let why = format!(
                        "cell {}: the service refused the agent's run in {first} and again in {round}, in a new session ({}): {what}",
                        self.cell.id, kind.name
                    );
                    return pause(e, self.w, &why);
                }
                e.db.write(Actor::Engine, |tx| {
                    tx.execute(
                        "UPDATE sessions SET ended = ?3 WHERE ended IS NULL
                           AND id = (SELECT session FROM rounds WHERE work = ?1 AND n = ?2)",
                        params![round.work, round.n, db::now()],
                    )?;
                    Ok(())
                })
                .map_err(sql)?;
                e.say(&format!(
                    "{round} was refused ({}): its line's session ends, and {} runs again in a new one",
                    kind.name, self.cell.id
                ));
                Ok(Advance::Moved)
            }
            // A return to repair is repaired within its round (`unparsed`); one that comes here was not, and waits for
            // the owner as a pause does.
            Handle::Pause | Handle::Repair => {
                let why = format!(
                    "cell {}: the agent's run ended without a return ({}): {what}",
                    self.cell.id, kind.name
                );
                pause(e, self.w, &why)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_time_reads_as_its_day_and_minute() {
        assert_eq!(utc(0), "1970-01-01 00:00 UTC");
        assert_eq!(utc(1_792_962_000_000), "2026-10-25 21:00 UTC");
        assert_eq!(utc(951_782_400_000), "2000-02-29 00:00 UTC");
    }

    /// The engineer's graph of the flow built into the engine, compiled once for every test here.
    fn engineer() -> &'static CompiledGraph {
        static GRAPH: std::sync::OnceLock<CompiledGraph> = std::sync::OnceLock::new();
        GRAPH.get_or_init(|| {
            let relations = db::relations(db::SCHEMA).unwrap();
            let tables = db::index_of(db::SCHEMA).unwrap();
            let f = flow::compile(flow::FLOW, &flow::Context { relations, tables }).unwrap();
            f.graph(crate::ENGINEER).unwrap().clone()
        })
    }

    /// The rounds section of the flow built into the engine.
    fn section() -> Rounds {
        engineer().rounds.clone().unwrap()
    }

    /// `n` rounds newest first that took `row`, all ended at `ended`.
    fn took(row: &str, n: i64, ended: i64) -> Vec<Took> {
        (0..n)
            .map(|i| Took {
                round: RoundId::new(1, n - i),
                cell: "R1".into(),
                row: Some(row.into()),
                started_at: ended,
                ended_at: Some(ended),
            })
            .collect()
    }

    // Each time a failed run runs again it waits twice as long, up to five minutes; a step that failed waits as many
    // seconds as the flow's backoff says for the time it failed, the last for every time after (a unit of a minute
    // tells 5, 15, 45 minutes, 2 and 5 hours apart); a round that took any other row, and none, wait for nothing.
    #[test]
    fn a_row_that_runs_again_waits_as_the_flow_says() {
        let (section, ended) = (section(), db::now());
        let due = |n: i64| {
            waiting(
                Duration::from_secs(10),
                Duration::from_secs(1),
                &took("transient", n, ended),
                &section,
            )
            .unwrap()
        };
        assert!(due(1).ends_with(&utc(ended + 10_000)));
        assert!(due(2).ends_with(&utc(ended + 20_000)));
        assert!(due(9).ends_with(&utc(ended + 300_000)));
        let minute = Duration::from_secs(60);
        let due = |n: i64| waiting(Duration::from_secs(10), minute, &took(STEP_NETWORK, n, ended), &section).unwrap();
        for (n, minutes) in [(1, 5), (2, 15), (3, 45), (4, 120), (5, 300), (6, 300)] {
            assert!(due(n).ends_with(&utc(ended + minutes * 60_000)), "{n}: {}", due(n));
        }
        let sent_back = took("sent_back", 3, ended);
        assert_eq!(waiting(Duration::from_secs(10), minute, &sent_back, &section), None);
        assert_eq!(waiting(Duration::from_secs(10), minute, &[], &section), None);
    }

    // Only a row taken from outside the engine ends the rows in a row: a return of an agent does; a shared row of a
    // round that ended in no return, a row that waits, and a round still open do not; a round of a cell or a row the
    // graph does not have, as after a flow edit, is not known to be the engine's and does.
    #[test]
    fn only_a_row_from_outside_the_engine_ends_the_rows_in_a_row() {
        let g = engineer();
        let at = |cell: &str, row: Option<&str>| Took {
            round: RoundId::new(1, 1),
            cell: cell.into(),
            row: row.map(str::to_string),
            started_at: 0,
            ended_at: Some(0),
        };
        let returned = g
            .cell("R1")
            .unwrap()
            .rows
            .iter()
            .find(|r| !r.waits && !r.conditions.iter().any(Atom::about_no_return))
            .unwrap();
        assert!(from_outside(g, &at("R1", Some(&returned.name))));
        for shared in [
            "sent_back",
            "transient",
            "transient_too_often",
            STEP_NETWORK,
            "step_network_too_often",
        ] {
            assert!(!from_outside(g, &at("R1", Some(shared))), "{shared}");
        }
        assert!(!from_outside(g, &at("R1", None)));
        assert!(from_outside(g, &at("ZZ", Some("gone"))));
        assert!(from_outside(g, &at("R1", Some("no_such_row"))));
        // A row that waits at a cell that is not the engine's: the engine took it without the cell running.
        let (cell, waits) = g
            .cells
            .iter()
            .filter(|c| c.kind != "engine")
            .find_map(|c| {
                c.rows
                    .iter()
                    .find(|r| r.waits && !r.conditions.iter().any(Atom::about_no_return))
                    .map(|r| (c.id.clone(), r.name.clone()))
            })
            .unwrap();
        assert!(!from_outside(g, &at(&cell, Some(&waits))), "{cell} {waits}");
    }

    // The cause of the step of work 243 that paused is the network's; a missing permission, an unknown branch, a
    // configuration fault and text it does not know are not, so that a step that fails for those is not run again.
    #[test]
    fn a_step_is_run_again_only_for_a_failure_the_network_gives() {
        assert!(network_failure(
            "channel 0: open failed: administratively prohibited: open failed\nstdio forwarding failed\n\
             ssh_exchange_identification: Connection closed by remote host\nfatal: Could not read from remote repository."
        ));
        assert!(network_failure(
            "ssh: Could not resolve hostname github.com: Name or service not known"
        ));
        assert!(network_failure(
            "fatal: unable to access 'https://github.com/o/r/': Failed to connect to github.com port 443: Connection timed out"
        ));
        assert!(network_failure(
            "error: RPC failed; HTTP 502 curl 22 The requested URL returned error: 502"
        ));
        assert!(!network_failure(
            "git@github.com: Permission denied (publickey).\nfatal: Could not read from remote repository."
        ));
        assert!(!network_failure("fatal: couldn't find remote ref main"));
        assert!(!network_failure("no project p in the configuration"));
        assert!(!network_failure(""));
    }
}
